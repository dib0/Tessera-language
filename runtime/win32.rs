// ---------------------------------------------------------------------------
// Tessera `ui` runtime for Windows: one native window, drawn with GDI.
//
// The counterpart of runtime/ui.rs (the X11 client used on unix). It offers the
// same functions with the same meaning, so a Tessera program that uses the `ui`
// effect compiles and behaves the same on both systems: one window per program,
// drawing goes to an off-screen bitmap and becomes visible on `present()`.
//
// This is the only part of the runtime besides runtime/adapter.rs that contains
// `unsafe` code; the window system has no other interface than its C functions.
// The unsafe surface is the `extern "system"` declarations below and their call
// sites. Everything above them -- colours, coordinates, key names, the event
// queue -- is ordinary safe Rust. Generated programs that use `ui` therefore say
// `#![cfg_attr(windows, deny(unsafe_code))]` instead of `forbid`, and only this
// module is `#[allow(unsafe_code)]`; on unix the blanket `forbid` still holds.
//
// Mapping from the X11 backend, so both stay comparable:
//   pixmap          -> a memory DC with a compatible bitmap (the back buffer)
//   PolyFillRectangle -> FillRect with the DC brush
//   PolyLine        -> a geometric pen with round caps, MoveToEx + LineTo
//   PolyArc/PolyFillArc -> Ellipse with a null brush / with the DC brush
//   PolyText8       -> TextOutW with a transparent background
//   PutImage        -> StretchDIBits from a top-down 32-bit BI_RGB bitmap
//   CopyArea        -> BitBlt from the memory DC to the window DC
//   Expose          -> WM_PAINT, which queues a repaint the event loop answers
// ---------------------------------------------------------------------------

use std::cell::RefCell;
use std::collections::VecDeque;

// --- the C interface ------------------------------------------------------

type T_Handle = *mut std::ffi::c_void;

#[repr(C)]
struct T_Point { x: i32, y: i32 }

#[repr(C)]
struct T_Rect { left: i32, top: i32, right: i32, bottom: i32 }

#[repr(C)]
struct T_Msg { hwnd: T_Handle, message: u32, w: usize, l: isize, time: u32, pt: T_Point }

#[repr(C)]
struct T_PaintStruct { hdc: T_Handle, erase: i32, paint: T_Rect, restore: i32, inc_update: i32, reserved: [u8; 32] }

#[repr(C)]
struct T_WndClass {
    style: u32,
    proc_: Option<unsafe extern "system" fn(T_Handle, u32, usize, isize) -> isize>,
    class_extra: i32,
    window_extra: i32,
    instance: T_Handle,
    icon: T_Handle,
    cursor: T_Handle,
    background: T_Handle,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct T_LogBrush { style: u32, color: u32, hatch: usize }

#[repr(C)]
struct T_BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pixels_per_meter: i32,
    y_pixels_per_meter: i32,
    colors_used: u32,
    colors_important: u32,
}

#[repr(C)]
struct T_TextMetric {
    height: i32,
    ascent: i32,
    descent: i32,
    internal_leading: i32,
    external_leading: i32,
    average_char_width: i32,
    max_char_width: i32,
    weight: i32,
    overhang: i32,
    digitized_aspect_x: i32,
    digitized_aspect_y: i32,
    first_char: u16,
    last_char: u16,
    default_char: u16,
    break_char: u16,
    italic: u8,
    underlined: u8,
    struck_out: u8,
    pitch_and_family: u8,
    char_set: u8,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> T_Handle;
    fn GetLastError() -> u32;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const T_WndClass) -> u16;
    fn CreateWindowExW(ex_style: u32, class: *const u16, title: *const u16, style: u32,
                       x: i32, y: i32, width: i32, height: i32,
                       parent: T_Handle, menu: T_Handle, instance: T_Handle, param: T_Handle) -> T_Handle;
    fn DefWindowProcW(window: T_Handle, message: u32, w: usize, l: isize) -> isize;
    fn AdjustWindowRect(rect: *mut T_Rect, style: u32, menu: i32) -> i32;
    fn ShowWindow(window: T_Handle, how: i32) -> i32;
    fn UpdateWindow(window: T_Handle) -> i32;
    fn SetForegroundWindow(window: T_Handle) -> i32;
    fn GetDC(window: T_Handle) -> T_Handle;
    fn LoadCursorW(instance: T_Handle, name: *const u16) -> T_Handle;
    fn PeekMessageW(message: *mut T_Msg, window: T_Handle, first: u32, last: u32, remove: u32) -> i32;
    fn GetMessageW(message: *mut T_Msg, window: T_Handle, first: u32, last: u32) -> i32;
    fn TranslateMessage(message: *const T_Msg) -> i32;
    fn DispatchMessageW(message: *const T_Msg) -> isize;
    fn BeginPaint(window: T_Handle, paint: *mut T_PaintStruct) -> T_Handle;
    fn EndPaint(window: T_Handle, paint: *const T_PaintStruct) -> i32;
    fn ToUnicode(key: u32, scan_code: u32, state: *const u8, buffer: *mut u16, size: i32, flags: u32) -> i32;
    fn ScreenToClient(window: T_Handle, point: *mut T_Point) -> i32;
    fn FillRect(dc: T_Handle, rect: *const T_Rect, brush: T_Handle) -> i32;
    fn SetProcessDPIAware() -> i32;
    fn PostQuitMessage(code: i32);
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(dc: T_Handle) -> T_Handle;
    fn CreateCompatibleBitmap(dc: T_Handle, width: i32, height: i32) -> T_Handle;
    fn SelectObject(dc: T_Handle, object: T_Handle) -> T_Handle;
    fn DeleteObject(object: T_Handle) -> i32;
    fn GetStockObject(index: i32) -> T_Handle;
    fn BitBlt(dst: T_Handle, x: i32, y: i32, width: i32, height: i32,
              src: T_Handle, src_x: i32, src_y: i32, rop: u32) -> i32;
    fn SetDCBrushColor(dc: T_Handle, color: u32) -> u32;
    fn ExtCreatePen(style: u32, width: u32, brush: *const T_LogBrush, style_count: u32, style_data: *const u32) -> T_Handle;
    fn MoveToEx(dc: T_Handle, x: i32, y: i32, previous: *mut T_Point) -> i32;
    fn LineTo(dc: T_Handle, x: i32, y: i32) -> i32;
    fn Ellipse(dc: T_Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn SetTextColor(dc: T_Handle, color: u32) -> u32;
    fn SetBkMode(dc: T_Handle, mode: i32) -> i32;
    fn TextOutW(dc: T_Handle, x: i32, y: i32, text: *const u16, count: i32) -> i32;
    fn GetTextMetricsW(dc: T_Handle, metrics: *mut T_TextMetric) -> i32;
    fn CreateFontW(height: i32, width: i32, escapement: i32, orientation: i32, weight: i32,
                   italic: u32, underline: u32, strike_out: u32, char_set: u32,
                   out_precision: u32, clip_precision: u32, quality: u32, pitch_and_family: u32,
                   face: *const u16) -> T_Handle;
    fn SetStretchBltMode(dc: T_Handle, mode: i32) -> i32;
    fn StretchDIBits(dc: T_Handle, x: i32, y: i32, width: i32, height: i32,
                     src_x: i32, src_y: i32, src_width: i32, src_height: i32,
                     bits: *const u8, info: *const T_BitmapInfoHeader, usage: u32, rop: u32) -> i32;
}

const WS_CAPTION: u32 = 0x00c0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_MINIMIZEBOX: u32 = 0x0002_0000;
const CS_OWNDC: u32 = 0x0020;
const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;
const SW_SHOW: i32 = 5;
const SRCCOPY: u32 = 0x00cc_0020;
const PM_REMOVE: u32 = 1;
const IDC_ARROW: usize = 32512;

const WM_CLOSE: u32 = 0x0010;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000f;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MOUSEWHEEL: u32 = 0x020a;
const WM_QUIT: u32 = 0x0012;

const NULL_BRUSH: i32 = 5;
const DC_BRUSH: i32 = 18;
const SYSTEM_FIXED_FONT: i32 = 16;
const TRANSPARENT: i32 = 1;
const COLORONCOLOR: i32 = 3;
const DIB_RGB_COLORS: u32 = 0;
const PS_GEOMETRIC_SOLID_ROUND: u32 = 0x0001_0000; // PS_GEOMETRIC | PS_SOLID | PS_ENDCAP_ROUND | PS_JOIN_ROUND
const FW_BOLD: i32 = 700;
const NONANTIALIASED_QUALITY: u32 = 3;
const FIXED_PITCH_MODERN: u32 = 1 | 48;

// --- state ----------------------------------------------------------------

struct T_Ui {
    window: T_Handle,
    window_dc: T_Handle,
    memory_dc: T_Handle,
    bitmap: T_Handle,
    pen: T_Handle,
    font: T_Handle,
    width: i32,
    height: i32,
    char_width: i64,
    color: i64,
    line_width: i64,
    text_color: i64,
}

/// What the window procedure saw. `Repaint` is not a Tessera event: like X11's
/// Expose it only means the window must be shown again from the back buffer.
enum T_Seen { Click(i64, i64, i64), Key(String), KeyUp(String), Close, Repaint }

thread_local! {
    static T_UI: RefCell<Option<T_Ui>> = RefCell::new(None);
    static T_SEEN: RefCell<VecDeque<T_Seen>> = RefCell::new(VecDeque::new());
    // Whether Shift is down, counted from the messages themselves rather than
    // read from the keyboard: that way a program also sees the right key names
    // for events a test helper posts to the window.
    static T_SHIFT: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

fn t_ui_fail(message: String, loc: &str) -> ! {
    t_fail("R012", message, loc)
}

fn t_with_ui<R>(loc: &str, f: impl FnOnce(&mut T_Ui) -> R) -> R {
    T_UI.with(|cell| {
        let mut slot = cell.borrow_mut();
        match slot.as_mut() {
            Some(ui) => f(ui),
            None => t_ui_fail("no window is open; call open_window first".to_string(), loc),
        }
    })
}

/// A Tessera colour (0xRRGGBB) as a Windows COLORREF (0x00BBGGRR).
fn t_colorref(color: i64) -> u32 {
    let c = color as u32;
    ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff)
}

fn t_coord(v: i64) -> i32 { v.clamp(-32768, 32767) as i32 }
fn t_size(v: i64) -> i32 { v.clamp(0, 65535) as i32 }

fn t_wide(text: &str) -> Vec<u16> {
    let mut v: Vec<u16> = text.encode_utf16().collect();
    v.push(0);
    v
}

// --- the window procedure -------------------------------------------------

fn t_push(event: T_Seen) {
    T_SEEN.with(|q| q.borrow_mut().push_back(event));
}

/// The name of a key, in the same spelling as the X11 backend: the character it
/// produces for printable keys, a word for the others, nothing for the rest.
/// `with_shift` is false for a release, which is what the X11 backend reports.
fn t_key_name(key: u32, scan_code: u32, with_shift: bool) -> Option<String> {
    let named = match key {
        0x1b => "Escape",
        0x0d => "Enter",
        0x08 => "Backspace",
        0x09 => "Tab",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x10 | 0xa0 | 0xa1 => "Shift",
        0x11 | 0xa2 | 0xa3 => "Control",
        0x12 | 0xa4 | 0xa5 => "Alt",
        0x2e => "Delete",
        0x24 => "Home",
        0x23 => "End",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x70..=0x7b => return Some(format!("F{}", key - 0x70 + 1)),
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    // A printable key: let the keyboard layout say which character it makes.
    // Only Shift is part of the state, so that Ctrl+C and Caps Lock report the
    // same name as the plain key, exactly as the X11 backend does.
    let mut state = [0u8; 256];
    if with_shift && T_SHIFT.with(|s| s.get()) {
        state[0x10] = 0x80;
        state[0xa0] = 0x80;
    }
    let mut buffer = [0u16; 8];
    // Flag 4: do not disturb the keyboard state (dead keys stay pending).
    #[allow(unsafe_code)]
    let count = unsafe { ToUnicode(key, scan_code, state.as_ptr(), buffer.as_mut_ptr(), buffer.len() as i32, 4) };
    if count != 1 { return None; }
    let c = char::from_u32(buffer[0] as u32)?;
    if (c as u32) >= 0x20 && (c as u32) <= 0x7e { Some(c.to_string()) } else { None }
}

unsafe extern "system" fn t_window_proc(window: T_Handle, message: u32, w: usize, l: isize) -> isize {
    match message {
        WM_CLOSE => {
            // Like the X11 WM_DELETE_WINDOW protocol: tell the program, and let
            // it decide. The window stays open until the program stops.
            t_push(T_Seen::Close);
            0
        }
        WM_DESTROY => {
            // Not the close button (that is WM_CLOSE, which this window keeps):
            // something outside the program took the window away. Tell the
            // program, and end the message loop so it cannot wait forever.
            t_push(T_Seen::Close);
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_ERASEBKGND => 1, // the back buffer covers every pixel; erasing only flickers
        WM_PAINT => {
            let mut paint = T_PaintStruct { hdc: std::ptr::null_mut(), erase: 0,
                paint: T_Rect { left: 0, top: 0, right: 0, bottom: 0 }, restore: 0, inc_update: 0, reserved: [0; 32] };
            unsafe {
                BeginPaint(window, &mut paint);
                EndPaint(window, &paint);
            }
            t_push(T_Seen::Repaint);
            0
        }
        WM_KILLFOCUS => {
            // Keys released while another window had the focus are never seen,
            // so Shift must not stay down across a switch.
            T_SHIFT.with(|s| s.set(false));
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if matches!(w as u32, 0x10 | 0xa0 | 0xa1) { T_SHIFT.with(|s| s.set(true)); }
            if let Some(name) = t_key_name(w as u32, ((l >> 16) & 0xff) as u32, true) {
                t_push(T_Seen::Key(name));
            }
            if message == WM_SYSKEYDOWN { unsafe { DefWindowProcW(window, message, w, l) } } else { 0 }
        }
        WM_KEYUP | WM_SYSKEYUP => {
            if matches!(w as u32, 0x10 | 0xa0 | 0xa1) { T_SHIFT.with(|s| s.set(false)); }
            if let Some(name) = t_key_name(w as u32, ((l >> 16) & 0xff) as u32, false) {
                t_push(T_Seen::KeyUp(name));
            }
            if message == WM_SYSKEYUP { unsafe { DefWindowProcW(window, message, w, l) } } else { 0 }
        }
        WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN => {
            let button = match message { WM_LBUTTONDOWN => 1, WM_MBUTTONDOWN => 2, _ => 3 };
            let x = (l & 0xffff) as u16 as i16 as i64;
            let y = ((l >> 16) & 0xffff) as u16 as i16 as i64;
            t_push(T_Seen::Click(x, y, button));
            0
        }
        WM_MOUSEWHEEL => {
            // The wheel reports screen coordinates, and X11 reports it as
            // button 4 (away from the user) or 5 (towards the user).
            let mut point = T_Point { x: (l & 0xffff) as u16 as i16 as i32, y: ((l >> 16) & 0xffff) as u16 as i16 as i32 };
            unsafe { ScreenToClient(window, &mut point); }
            let delta = ((w >> 16) & 0xffff) as u16 as i16;
            t_push(T_Seen::Click(point.x as i64, point.y as i64, if delta > 0 { 4 } else { 5 }));
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w, l) },
    }
}

// --- opening the window ---------------------------------------------------

pub fn t_open_window(title: Rc<String>, width: i64, height: i64, loc: &str) {
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        t_ui_fail(format!("window size {}x{} is outside 1..4096", width, height), loc)
    }
    if T_UI.with(|cell| cell.borrow().is_some()) {
        t_ui_fail("a window is already open".to_string(), loc)
    }

    let class_name = t_wide("TesseraWindow");
    let window_title = t_wide(title.as_str());
    let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX; // a fixed size, like the X11 size hints

    #[allow(unsafe_code)]
    let (window, window_dc) = unsafe {
        // Ask for real pixels: without this a scaled display would stretch the
        // window afterwards and every drawing would be blurred.
        SetProcessDPIAware();
        let instance = GetModuleHandleW(std::ptr::null());
        let class = T_WndClass {
            style: CS_OWNDC,
            proc_: Some(t_window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: std::ptr::null_mut(),
            cursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16),
            background: std::ptr::null_mut(),
            menu_name: std::ptr::null(),
            class_name: class_name.as_ptr(),
        };
        // A second window in the same program is refused above, and registering
        // the same class twice is the only other way this fails.
        RegisterClassW(&class);
        let mut rect = T_Rect { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        AdjustWindowRect(&mut rect, style, 0);
        let window = CreateWindowExW(0, class_name.as_ptr(), window_title.as_ptr(), style,
            CW_USEDEFAULT, CW_USEDEFAULT, rect.right - rect.left, rect.bottom - rect.top,
            std::ptr::null_mut(), std::ptr::null_mut(), instance, std::ptr::null_mut());
        if window.is_null() {
            t_ui_fail(format!("cannot open a window (Windows error {})", GetLastError()), loc)
        }
        (window, GetDC(window))
    };
    if window_dc.is_null() {
        t_ui_fail("cannot open a window: the display context is not available".to_string(), loc)
    }

    #[allow(unsafe_code)]
    let (memory_dc, bitmap, font, char_width) = unsafe {
        let memory_dc = CreateCompatibleDC(window_dc);
        let bitmap = CreateCompatibleBitmap(window_dc, width as i32, height as i32);
        if memory_dc.is_null() || bitmap.is_null() {
            t_ui_fail("cannot open a window: the back buffer could not be created".to_string(), loc)
        }
        SelectObject(memory_dc, bitmap);

        // A fixed-pitch font without smoothing, as close as Windows gets to the
        // bitmap font the X11 backend asks for. `text_width` counts characters,
        // so the font must be fixed-pitch for the measurement to be right.
        let face = t_wide("Consolas");
        let mut font = CreateFontW(18, 0, 0, 0, FW_BOLD, 0, 0, 0, 0, 0, 0,
            NONANTIALIASED_QUALITY, FIXED_PITCH_MODERN, face.as_ptr());
        if font.is_null() { font = GetStockObject(SYSTEM_FIXED_FONT); }
        SelectObject(memory_dc, font);
        let mut metrics: T_TextMetric = std::mem::zeroed();
        GetTextMetricsW(memory_dc, &mut metrics);
        SetBkMode(memory_dc, TRANSPARENT);
        SetStretchBltMode(memory_dc, COLORONCOLOR);

        // A white back buffer, like the X11 backend clears it to.
        SetDCBrushColor(memory_dc, 0x00ff_ffff);
        let all = T_Rect { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        FillRect(memory_dc, &all, GetStockObject(DC_BRUSH));

        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        SetForegroundWindow(window);
        (memory_dc, bitmap, font, metrics.average_char_width.max(1) as i64)
    };

    T_UI.with(|cell| *cell.borrow_mut() = Some(T_Ui {
        window,
        window_dc,
        memory_dc,
        bitmap,
        pen: std::ptr::null_mut(),
        font,
        width: width as i32,
        height: height as i32,
        char_width,
        color: -1,
        line_width: -1,
        text_color: -1,
    }));
}

impl T_Ui {
    /// The pen used for lines and outlines. A geometric pen with round caps and
    /// joins, which is what the X11 graphics context asks for.
    fn set_pen(&mut self, color: i64, line_width: i64) {
        if color == self.color && line_width == self.line_width { return; }
        let brush = T_LogBrush { style: 0, color: t_colorref(color), hatch: 0 };
        #[allow(unsafe_code)]
        unsafe {
            let pen = ExtCreatePen(PS_GEOMETRIC_SOLID_ROUND, line_width.clamp(1, 65535) as u32,
                &brush, 0, std::ptr::null());
            if pen.is_null() { return; }
            SelectObject(self.memory_dc, pen);
            if !self.pen.is_null() { DeleteObject(self.pen); }
            self.pen = pen;
        }
        self.color = color;
        self.line_width = line_width;
    }

    fn set_brush(&self, color: i64) {
        #[allow(unsafe_code)]
        unsafe {
            SetDCBrushColor(self.memory_dc, t_colorref(color));
            SelectObject(self.memory_dc, GetStockObject(DC_BRUSH));
        }
    }

    fn present(&mut self) {
        #[allow(unsafe_code)]
        unsafe {
            BitBlt(self.window_dc, 0, 0, self.width, self.height, self.memory_dc, 0, 0, SRCCOPY);
        }
        // Keep the window answering the system even in a program that only
        // draws: on Windows a window that never reads its messages freezes.
        t_pump(false);
    }
}

/// Handles the messages Windows has for this program. With `wait`, blocks until
/// at least one arrives. Returns false when the window system asked the program
/// to stop (WM_QUIT), which cannot happen before a WM_CLOSE the program ignored.
fn t_pump(wait: bool) -> bool {
    let mut message: T_Msg = T_Msg { hwnd: std::ptr::null_mut(), message: 0, w: 0, l: 0, time: 0, pt: T_Point { x: 0, y: 0 } };
    #[allow(unsafe_code)]
    unsafe {
        if wait {
            let got = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
            if got <= 0 { return false; }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            if message.message == WM_QUIT { return false; }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    true
}

// --- drawing --------------------------------------------------------------

pub fn t_fill_rectangle(x: i64, y: i64, width: i64, height: i64, color: i64, loc: &str) {
    if width <= 0 || height <= 0 { return; }
    t_with_ui(loc, |ui| {
        ui.set_brush(color);
        let rect = T_Rect {
            left: t_coord(x),
            top: t_coord(y),
            right: t_coord(x).saturating_add(t_size(width)),
            bottom: t_coord(y).saturating_add(t_size(height)),
        };
        #[allow(unsafe_code)]
        unsafe {
            FillRect(ui.memory_dc, &rect, GetStockObject(DC_BRUSH));
        }
    })
}

pub fn t_draw_line(x1: i64, y1: i64, x2: i64, y2: i64, thickness: i64, color: i64, loc: &str) {
    if thickness <= 0 { return; }
    t_with_ui(loc, |ui| {
        ui.set_pen(color, thickness);
        #[allow(unsafe_code)]
        unsafe {
            MoveToEx(ui.memory_dc, t_coord(x1), t_coord(y1), std::ptr::null_mut());
            LineTo(ui.memory_dc, t_coord(x2), t_coord(y2));
        }
    })
}

/// The bounding box GDI wants: X11 gives a width and a height that both ends
/// belong to, GDI an exclusive right and bottom edge.
fn t_circle_box(center_x: i64, center_y: i64, radius: i64) -> (i32, i32, i32, i32) {
    (t_coord(center_x - radius), t_coord(center_y - radius),
     t_coord(center_x + radius) + 1, t_coord(center_y + radius) + 1)
}

pub fn t_draw_circle(center_x: i64, center_y: i64, radius: i64, thickness: i64, color: i64, loc: &str) {
    if radius <= 0 || thickness <= 0 { return; }
    t_with_ui(loc, |ui| {
        ui.set_pen(color, thickness);
        let (left, top, right, bottom) = t_circle_box(center_x, center_y, radius);
        #[allow(unsafe_code)]
        unsafe {
            SelectObject(ui.memory_dc, GetStockObject(NULL_BRUSH));
            Ellipse(ui.memory_dc, left, top, right, bottom);
        }
    })
}

pub fn t_fill_circle(center_x: i64, center_y: i64, radius: i64, color: i64, loc: &str) {
    if radius <= 0 { return; }
    t_with_ui(loc, |ui| {
        // The outline is drawn in the same colour, so the filled disc covers the
        // same pixels as the X11 PolyFillArc.
        ui.set_pen(color, 1);
        ui.set_brush(color);
        let (left, top, right, bottom) = t_circle_box(center_x, center_y, radius);
        #[allow(unsafe_code)]
        unsafe {
            Ellipse(ui.memory_dc, left, top, right, bottom);
        }
    })
}

pub fn t_draw_text(x: i64, y: i64, text: Rc<String>, color: i64, loc: &str) {
    // The same replacement the X11 backend makes, so a program draws the same
    // text on both systems.
    let shown: String = text.chars().map(|c| if (c as u32) >= 32 && (c as u32) < 256 { c } else { '?' }).collect();
    if shown.is_empty() { return; }
    t_with_ui(loc, |ui| {
        if color != ui.text_color {
            #[allow(unsafe_code)]
            unsafe { SetTextColor(ui.memory_dc, t_colorref(color)); }
            ui.text_color = color;
        }
        let wide: Vec<u16> = shown.encode_utf16().collect();
        // `y` is the top of the line: TextOut places the cell top there, so the
        // baseline lands on y + ascent, where the X11 backend draws it.
        #[allow(unsafe_code)]
        unsafe {
            TextOutW(ui.memory_dc, t_coord(x), t_coord(y), wide.as_ptr(), wide.len() as i32);
        }
    })
}

pub fn t_text_width(text: Rc<String>, loc: &str) -> i64 {
    t_with_ui(loc, |ui| ui.char_width * text.chars().count() as i64)
}

pub fn t_present(loc: &str) {
    t_with_ui(loc, |ui| ui.present())
}

/// Draws rows of `width` colors at (x, y), each pixel as a scale x scale square.
/// The colors are already in the byte order a 32-bit BI_RGB bitmap wants
/// (0x00RRGGBB), so the rows go to the back buffer unchanged and Windows does
/// the scaling with COLORONCOLOR, which repeats pixels without blending.
pub fn t_draw_pixels(x: i64, y: i64, width: i64, colors: &Rc<Vec<i64>>, scale: i64, loc: &str) {
    if width < 1 || width > 4096 || !(1..=16).contains(&scale) || colors.len() as i64 % width != 0 {
        t_fail("R004", format!("draw_pixels: width {} and scale {} do not fit {} colors (scale 1..16, width 1..4096, colors a multiple of width)", width, scale, colors.len()), loc)
    }
    if let Some(bad) = colors.iter().find(|c| !(0..=0xffffff).contains(*c)) {
        t_fail("R011", format!("draw_pixels: {} is not a color (0..16777215)", bad), loc)
    }
    if colors.is_empty() { return; }
    let rows = colors.len() as i64 / width;
    let pixels: Vec<u32> = colors.iter().map(|c| *c as u32).collect();
    let info = T_BitmapInfoHeader {
        size: std::mem::size_of::<T_BitmapInfoHeader>() as u32,
        width: width as i32,
        height: -(rows as i32), // negative: the first row in memory is the top one
        planes: 1,
        bit_count: 32,
        compression: 0,
        size_image: 0,
        x_pixels_per_meter: 0,
        y_pixels_per_meter: 0,
        colors_used: 0,
        colors_important: 0,
    };
    t_with_ui(loc, |ui| {
        #[allow(unsafe_code)]
        unsafe {
            StretchDIBits(ui.memory_dc, t_coord(x), t_coord(y),
                t_size(width * scale), t_size(rows * scale),
                0, 0, width as i32, rows as i32,
                pixels.as_ptr() as *const u8, &info, DIB_RGB_COLORS, SRCCOPY);
        }
    })
}

// --- events ---------------------------------------------------------------

fn t_take_event(ui: &mut T_Ui) -> Option<Rc<T_WindowEvent>> {
    loop {
        let seen = T_SEEN.with(|q| q.borrow_mut().pop_front())?;
        match seen {
            T_Seen::Repaint => ui.present(),
            T_Seen::Click(x, y, button) => return Some(Rc::new(T_WindowEvent::V_Click(x, y, button))),
            T_Seen::Key(name) => return Some(Rc::new(T_WindowEvent::V_Key(Rc::new(name)))),
            T_Seen::KeyUp(name) => return Some(Rc::new(T_WindowEvent::V_KeyUp(Rc::new(name)))),
            T_Seen::Close => return Some(Rc::new(T_WindowEvent::V_Close)),
        }
    }
}

pub fn t_next_event(loc: &str) -> Rc<T_WindowEvent> {
    t_with_ui(loc, |ui| {
        loop {
            if let Some(event) = t_take_event(ui) { return event; }
            if !t_pump(true) {
                // The window is gone. Anything the message loop saw on the way
                // out, such as the Close that WM_DESTROY queues, is reported
                // first; only then is there nothing left to wait for.
                if let Some(event) = t_take_event(ui) { return event; }
                t_ui_fail("the window was closed by the system".to_string(), loc)
            }
        }
    })
}

pub fn t_poll_event(loc: &str) -> Option<Rc<T_WindowEvent>> {
    t_with_ui(loc, |ui| {
        t_pump(false);
        t_take_event(ui)
    })
}
