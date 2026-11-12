// Test helper: sends synthetic events to a window of a graphical Tessera
// program on Windows, so those programs can be tested without a person
// clicking. The Windows counterpart of tools/xsend/main.rs (X11).
//
//   xsend list                              every visible top-level window
//   xsend <window> click <x> <y> [button]   1 left, 2 middle, 3 right, 4/5 wheel
//   xsend <window> key <name>               a single ASCII character, Escape, Enter,
//                                           Backspace, Tab, Left, Up, Right, Down,
//                                           Shift, Control, Alt, Delete, Home, End,
//                                           PageUp, PageDown, F1 to F12
//   xsend <window> keyup <name>             the release of such a key
//   xsend <window> close                    the close request of the title bar
//
// <window> is a handle as `list` prints it (0x... or decimal) or a piece of the
// window title; `Tessera` is enough for most programs.
//
// The events are posted to the window's own queue, so the window does not have
// to be in front and nothing is typed into whatever the person is doing.
//
// Built by tools/xsend/build.sh. This file is the whole program; it needs no
// part of the Tessera runtime.

#![allow(non_snake_case)]

use std::ffi::c_void;

type Handle = *mut c_void;

#[link(name = "user32")]
extern "system" {
    fn PostMessageW(window: Handle, message: u32, w: usize, l: isize) -> i32;
    fn EnumWindows(callback: unsafe extern "system" fn(Handle, isize) -> i32, param: isize) -> i32;
    fn GetWindowTextW(window: Handle, text: *mut u16, count: i32) -> i32;
    fn IsWindowVisible(window: Handle) -> i32;
    fn IsWindow(window: Handle) -> i32;
    fn GetClientRect(window: Handle, rect: *mut Rect) -> i32;
    fn ClientToScreen(window: Handle, point: *mut Point) -> i32;
    fn VkKeyScanW(character: u16) -> i16;
    fn MapVirtualKeyW(code: u32, kind: u32) -> u32;
}

#[repr(C)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }

#[repr(C)]
struct Point { x: i32, y: i32 }

const WM_CLOSE: u32 = 0x0010;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MOUSEWHEEL: u32 = 0x020a;
const VK_SHIFT: u32 = 0x10;

/// Called once per top-level window; `param` is the list to fill.
unsafe extern "system" fn collect(window: Handle, param: isize) -> i32 {
    if IsWindowVisible(window) == 0 {
        return 1;
    }
    let mut buffer = [0u16; 512];
    let length = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32);
    if length > 0 {
        let found = &mut *(param as *mut Vec<(Handle, String)>);
        found.push((window, String::from_utf16_lossy(&buffer[..length as usize])));
    }
    1
}

fn windows() -> Vec<(Handle, String)> {
    let mut found: Vec<(Handle, String)> = Vec::new();
    unsafe { EnumWindows(collect, &mut found as *mut Vec<(Handle, String)> as isize) };
    found
}

fn fail(message: &str) -> ! {
    eprintln!("{}", message);
    std::process::exit(2)
}

/// A handle written as 0x... or in decimal, or a piece of a window title.
fn window_of(argument: &str) -> Handle {
    let number = match argument.strip_prefix("0x").or_else(|| argument.strip_prefix("0X")) {
        Some(hex) => usize::from_str_radix(hex, 16).ok(),
        None => argument.parse::<usize>().ok(),
    };
    if let Some(n) = number {
        let window = n as Handle;
        if unsafe { IsWindow(window) } != 0 {
            return window;
        }
        fail(&format!("there is no window {}", argument));
    }
    let matching: Vec<(Handle, String)> = windows().into_iter().filter(|(_, t)| t.contains(argument)).collect();
    match matching.len() {
        0 => fail(&format!("no window has `{}` in its title; try `xsend list`", argument)),
        1 => matching[0].0,
        _ => {
            let titles: Vec<String> = matching.iter().map(|(_, t)| format!("`{}`", t)).collect();
            fail(&format!("`{}` matches {} windows ({}); use a handle from `xsend list`", argument, matching.len(), titles.join(", ")))
        }
    }
}

/// The virtual-key code of a key name, and whether Shift belongs to it. The
/// names are the ones runtime/win32.rs reports, which are the X11 ones.
fn key_of(name: &str) -> (u32, bool) {
    let named = match name {
        "Escape" => 0x1b,
        "Enter" => 0x0d,
        "Backspace" => 0x08,
        "Tab" => 0x09,
        "Left" => 0x25,
        "Up" => 0x26,
        "Right" => 0x27,
        "Down" => 0x28,
        "Shift" => 0x10,
        "Control" => 0x11,
        "Alt" => 0x12,
        "Delete" => 0x2e,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        _ => 0,
    };
    if named != 0 {
        return (named, false);
    }
    if let Some(digits) = name.strip_prefix('F') {
        if let Ok(n) = digits.parse::<u32>() {
            if (1..=12).contains(&n) {
                return (0x70 + n - 1, false);
            }
        }
    }
    let mut characters = name.chars();
    let (character, rest) = (characters.next(), characters.next());
    match (character, rest) {
        (Some(c), None) if (c as u32) < 128 => {
            let scan = unsafe { VkKeyScanW(c as u16) };
            if scan == -1 {
                fail(&format!("`{}` is not on this keyboard layout", name))
            }
            ((scan as u16 & 0xff) as u32, (scan >> 8) & 1 != 0)
        }
        _ => fail(&format!("unsupported key `{}`", name)),
    }
}

fn post(window: Handle, message: u32, w: usize, l: isize) {
    if unsafe { PostMessageW(window, message, w, l) } == 0 {
        fail("the window did not accept the event");
    }
}

fn key_message(window: Handle, key: u32, down: bool) {
    let scan = unsafe { MapVirtualKeyW(key, 0) };
    let repeat = 1isize | ((scan as isize & 0xff) << 16);
    if down {
        post(window, WM_KEYDOWN, key as usize, repeat);
    } else {
        post(window, WM_KEYUP, key as usize, repeat | 0xc000_0000u32 as isize);
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: xsend list | xsend <window> click <x> <y> [button] | key <name> | keyup <name> | close";
    if arguments.is_empty() {
        fail(usage);
    }
    if arguments[0] == "list" {
        for (window, title) in windows() {
            println!("0x{:x}  {}", window as usize, title);
        }
        return;
    }
    if arguments.len() < 2 {
        fail(usage);
    }
    let window = window_of(&arguments[0]);
    match arguments[1].as_str() {
        "click" if arguments.len() >= 4 => {
            let x: i32 = arguments[2].parse().unwrap_or_else(|_| fail("x must be a number"));
            let y: i32 = arguments[3].parse().unwrap_or_else(|_| fail("y must be a number"));
            let button: u32 = arguments.get(4).and_then(|b| b.parse().ok()).unwrap_or(1);
            let position = ((y as isize & 0xffff) << 16) | (x as isize & 0xffff);
            match button {
                1 => { post(window, WM_LBUTTONDOWN, 1, position); post(window, WM_LBUTTONUP, 0, position); }
                2 => { post(window, WM_MBUTTONDOWN, 0x10, position); post(window, WM_MBUTTONUP, 0, position); }
                3 => { post(window, WM_RBUTTONDOWN, 2, position); post(window, WM_RBUTTONUP, 0, position); }
                4 | 5 => {
                    // The wheel reports where the pointer is on the screen.
                    let mut point = Point { x, y };
                    unsafe { ClientToScreen(window, &mut point) };
                    let screen = ((point.y as isize & 0xffff) << 16) | (point.x as isize & 0xffff);
                    let delta: i16 = if button == 4 { 120 } else { -120 };
                    post(window, WM_MOUSEWHEEL, ((delta as u16 as usize) << 16) | 0, screen);
                }
                _ => fail("button must be 1 (left), 2 (middle), 3 (right), 4 or 5 (wheel)"),
            }
        }
        "key" | "keyup" if arguments.len() >= 3 => {
            let (key, shifted) = key_of(&arguments[2]);
            let down = arguments[1] == "key";
            // A capital letter or a shifted symbol needs Shift around it, just
            // as it does on a real keyboard.
            if shifted && down {
                key_message(window, VK_SHIFT, true);
            }
            key_message(window, key, down);
            if shifted && down {
                key_message(window, VK_SHIFT, false);
            }
        }
        "close" => post(window, WM_CLOSE, 0, 0),
        "size" => {
            let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            unsafe { GetClientRect(window, &mut rect) };
            println!("{}x{}", rect.right - rect.left, rect.bottom - rect.top);
        }
        _ => fail(usage),
    }
}
