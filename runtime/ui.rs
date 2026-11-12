
// ---------------------------------------------------------------------------
// Tessera `ui` runtime for unix: a minimal X11 client in safe Rust (local display).
// Speaks the X11 core protocol directly over the X server's Unix socket, so
// generated programs need no libraries. One window per program; drawing goes
// to an off-screen pixmap and becomes visible on `present()`.
// ---------------------------------------------------------------------------

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Read;
use std::os::unix::net::UnixStream;

struct T_Ui {
    stream: UnixStream,
    out: Vec<u8>,
    seq: u16,
    id_base: u32,
    id_mask: u32,
    next_id: u32,
    window: u32,
    pixmap: u32,
    gc: u32,
    width: u16,
    height: u16,
    wm_delete: u32,
    events: VecDeque<Vec<u8>>,
    input: Vec<u8>,
    min_keycode: u8,
    keysyms_per_keycode: u8,
    keysyms: Vec<u32>,
    char_width: i64,
    font_ascent: i64,
    color: i64,
    line_width: i64,
}

thread_local! {
    static T_UI: RefCell<Option<T_Ui>> = RefCell::new(None);
}

fn t_ui_fail(message: String, loc: &str) -> ! {
    t_fail("R012", message, loc)
}

fn t_u16(b: &[u8], i: usize) -> u16 { u16::from_le_bytes([b[i], b[i + 1]]) }
fn t_i16(b: &[u8], i: usize) -> i16 { i16::from_le_bytes([b[i], b[i + 1]]) }
fn t_u32(b: &[u8], i: usize) -> u32 { u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) }
fn t_put16(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
fn t_put32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }
fn t_pad4(v: &mut Vec<u8>) { while v.len() % 4 != 0 { v.push(0); } }
fn t_coord(v: i64) -> u16 { (v.clamp(-32768, 32767) as i16) as u16 }
fn t_size(v: i64) -> u16 { v.clamp(0, 65535) as u16 }

fn t_read_exact(stream: &mut UnixStream, buf: &mut [u8], loc: &str) {
    if let Err(e) = stream.read_exact(buf) {
        t_ui_fail(format!("the connection to the X server was lost: {}", e), loc)
    }
}

fn t_x_authority(display_number: &str) -> (Vec<u8>, Vec<u8>) {
    let path = match std::env::var_os("XAUTHORITY") {
        Some(p) => std::path::PathBuf::from(p),
        None => match std::env::var_os("HOME") {
            Some(h) => std::path::Path::new(&h).join(".Xauthority"),
            None => return (Vec::new(), Vec::new()),
        },
    };
    let data = match std::fs::read(&path) { Ok(d) => d, Err(_) => return (Vec::new(), Vec::new()) };
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default().trim().to_string();
    let mut pos = 0usize;
    let mut fallback: Option<(Vec<u8>, Vec<u8>)> = None;
    let read_field = |pos: &mut usize| -> Option<Vec<u8>> {
        if *pos + 2 > data.len() { return None; }
        let len = u16::from_be_bytes([data[*pos], data[*pos + 1]]) as usize;
        *pos += 2;
        if *pos + len > data.len() { return None; }
        let field = data[*pos..*pos + len].to_vec();
        *pos += len;
        Some(field)
    };
    while pos + 2 <= data.len() {
        let family = u16::from_be_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let (Some(address), Some(number), Some(name), Some(cookie)) =
            (read_field(&mut pos), read_field(&mut pos), read_field(&mut pos), read_field(&mut pos)) else { break };
        if name != b"MIT-MAGIC-COOKIE-1" { continue; }
        let number_matches = number.is_empty() || number == display_number.as_bytes();
        if !number_matches { continue; }
        if family == 65535 || (family == 256 && address == hostname.as_bytes()) {
            return (name, cookie);
        }
        if fallback.is_none() { fallback = Some((name, cookie)); }
    }
    fallback.unwrap_or((Vec::new(), Vec::new()))
}

impl T_Ui {
    fn begin(&mut self, opcode: u8, data: u8) -> usize {
        let start = self.out.len();
        self.out.push(opcode);
        self.out.push(data);
        t_put16(&mut self.out, 0);
        start
    }

    fn end(&mut self, start: usize) {
        t_pad4(&mut self.out);
        let words = ((self.out.len() - start) / 4) as u16;
        self.out[start + 2..start + 4].copy_from_slice(&words.to_le_bytes());
        self.seq = self.seq.wrapping_add(1);
    }

    fn new_id(&mut self) -> u32 {
        let id = self.id_base | (self.next_id & self.id_mask);
        self.next_id += 1;
        id
    }

    fn flush(&mut self, loc: &str) {
        if self.out.is_empty() { return; }
        use std::io::Write;
        if let Err(e) = self.stream.write_all(&self.out) {
            t_ui_fail(format!("the connection to the X server was lost: {}", e), loc)
        }
        self.out.clear();
    }

    /// The next packet from the server: taken from `input` (bytes already received) when complete,
    /// otherwise read. Without `wait`, returns None instead of blocking when no full packet is there.
    fn next_packet(&mut self, wait: bool, loc: &str) -> Option<Vec<u8>> {
        loop {
            if self.input.len() >= 32 {
                let size = if self.input[0] == 1 { 32 + t_u32(&self.input, 4) as usize * 4 } else { 32 };
                if self.input.len() >= size {
                    return Some(self.input.drain(..size).collect());
                }
            }
            let mut buffer = [0u8; 65536];
            if !wait {
                if let Err(e) = self.stream.set_nonblocking(true) {
                    t_ui_fail(format!("the connection to the X server failed: {}", e), loc)
                }
            }
            let result = self.stream.read(&mut buffer);
            if !wait {
                let _ = self.stream.set_nonblocking(false);
            }
            match result {
                Ok(0) => t_ui_fail("the X server closed the connection".to_string(), loc),
                Ok(n) => self.input.extend_from_slice(&buffer[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return None,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => t_ui_fail(format!("the connection to the X server was lost: {}", e), loc),
            }
        }
    }

    fn read_packet(&mut self, loc: &str) -> Vec<u8> {
        match self.next_packet(true, loc) {
            Some(packet) => packet,
            None => t_ui_fail("the X server stopped answering".to_string(), loc),
        }
    }

    /// Turns a packet into an event, or None for packets a program does not see. A key release
    /// directly followed by a press of the same key at the same time is the keyboard's automatic
    /// repeat, not a real release; it is dropped.
    fn event_of(&mut self, packet: Vec<u8>, loc: &str) -> Option<Rc<T_WindowEvent>> {
        match packet[0] & 0x7f {
            0 => t_x_error(&packet, loc),
            12 => {
                // Expose: the back buffer still holds the picture.
                if t_u16(&packet, 16) == 0 { self.present(loc); }
                None
            }
            4 => {
                let x = t_i16(&packet, 24) as i64;
                let y = t_i16(&packet, 26) as i64;
                Some(Rc::new(T_WindowEvent::V_Click(x, y, packet[1] as i64)))
            }
            2 => self.key_name(packet[1], t_u16(&packet, 28)).map(|name| Rc::new(T_WindowEvent::V_Key(Rc::new(name)))),
            3 => {
                if self.events.is_empty() {
                    if let Some(next) = self.next_packet(false, loc) { self.events.push_back(next); }
                }
                if let Some(next) = self.events.front() {
                    if next[0] & 0x7f == 2 && next[1] == packet[1] && t_u32(next, 4) == t_u32(&packet, 4) {
                        return None;
                    }
                }
                self.key_name(packet[1], 0).map(|name| Rc::new(T_WindowEvent::V_KeyUp(Rc::new(name))))
            }
            33 => {
                if t_u32(&packet, 12) == self.wm_delete { Some(Rc::new(T_WindowEvent::V_Close)) } else { None }
            }
            _ => None,
        }
    }

    /// Wait for the reply to the last request. Errors for requests sent at or
    /// after `first_seq` are returned as `Err(code)`; events are queued.
    fn reply_since(&mut self, first_seq: u16, loc: &str) -> Result<Vec<u8>, u8> {
        self.flush(loc);
        loop {
            let packet = self.read_packet(loc);
            match packet[0] {
                1 if t_u16(&packet, 2) == self.seq => return Ok(packet),
                1 => {}
                0 => {
                    let seq = t_u16(&packet, 2);
                    if seq.wrapping_sub(first_seq) <= self.seq.wrapping_sub(first_seq) {
                        return Err(packet[1]);
                    }
                    t_x_error(&packet, loc)
                }
                _ => self.events.push_back(packet),
            }
        }
    }

    fn reply(&mut self, loc: &str) -> Vec<u8> {
        let first = self.seq;
        match self.reply_since(first, loc) {
            Ok(r) => r,
            Err(code) => t_ui_fail(format!("the X server reported error {}", code), loc),
        }
    }

    fn intern_atom(&mut self, name: &str, loc: &str) -> u32 {
        let start = self.begin(16, 0);
        t_put16(&mut self.out, name.len() as u16);
        t_put16(&mut self.out, 0);
        self.out.extend_from_slice(name.as_bytes());
        self.end(start);
        let r = self.reply(loc);
        t_u32(&r, 8)
    }

    fn change_property(&mut self, property: u32, kind: u32, format: u8, data: &[u8]) {
        let start = self.begin(18, 0);
        t_put32(&mut self.out, self.window);
        t_put32(&mut self.out, property);
        t_put32(&mut self.out, kind);
        self.out.push(format);
        self.out.extend_from_slice(&[0, 0, 0]);
        t_put32(&mut self.out, (data.len() / (format as usize / 8)) as u32);
        self.out.extend_from_slice(data);
        self.end(start);
    }

    fn set_gc(&mut self, color: i64, line_width: i64) {
        if color != self.color {
            let start = self.begin(56, 0);
            t_put32(&mut self.out, self.gc);
            t_put32(&mut self.out, 0x4);
            t_put32(&mut self.out, color as u32);
            self.end(start);
            self.color = color;
        }
        if line_width != self.line_width {
            let start = self.begin(56, 0);
            t_put32(&mut self.out, self.gc);
            t_put32(&mut self.out, 0x10);
            t_put32(&mut self.out, line_width.clamp(0, 65535) as u32);
            self.end(start);
            self.line_width = line_width;
        }
    }

    /// Start a drawing request on the back buffer.
    fn draw_request(&mut self, opcode: u8) -> usize {
        let start = self.begin(opcode, 0);
        t_put32(&mut self.out, self.pixmap);
        t_put32(&mut self.out, self.gc);
        start
    }

    fn present(&mut self, loc: &str) {
        let start = self.begin(62, 0);
        t_put32(&mut self.out, self.pixmap);
        t_put32(&mut self.out, self.window);
        t_put32(&mut self.out, self.gc);
        for _ in 0..4 { t_put16(&mut self.out, 0); }
        t_put16(&mut self.out, self.width);
        t_put16(&mut self.out, self.height);
        self.end(start);
        self.flush(loc);
    }

    fn key_name(&self, keycode: u8, state: u16) -> Option<String> {
        if keycode < self.min_keycode || self.keysyms_per_keycode == 0 { return None; }
        let per = self.keysyms_per_keycode as usize;
        let base = (keycode - self.min_keycode) as usize * per;
        let shifted = state & 1 != 0 && per > 1 && self.keysyms.get(base + 1).map_or(false, |&k| k != 0);
        let keysym = *self.keysyms.get(if shifted { base + 1 } else { base })?;
        let name = match keysym {
            0x20..=0x7e => (keysym as u8 as char).to_string(),
            0xff1b => "Escape".into(),
            0xff0d | 0xff8d => "Enter".into(),
            0xff08 => "Backspace".into(),
            0xff09 => "Tab".into(),
            0xff51 => "Left".into(),
            0xff52 => "Up".into(),
            0xff53 => "Right".into(),
            0xff54 => "Down".into(),
            0xffe1 | 0xffe2 => "Shift".into(),
            0xffe3 | 0xffe4 => "Control".into(),
            0xffe9 | 0xffea => "Alt".into(),
            0xffbe..=0xffc9 => format!("F{}", keysym - 0xffbe + 1),
            0xffff => "Delete".into(),
            0xff50 => "Home".into(),
            0xff57 => "End".into(),
            0xff55 => "PageUp".into(),
            0xff56 => "PageDown".into(),
            _ => return None,
        };
        Some(name)
    }
}

fn t_x_error(packet: &[u8], loc: &str) -> ! {
    t_ui_fail(format!("the X server reported error {} for request {}", packet[1], packet[10]), loc)
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

/// Connect and authenticate. Returns the connection, the root window, its depth and the keycode range.
fn t_ui_connect(width: i64, height: i64, loc: &str) -> (T_Ui, u32, u8, u8, u8) {
    let display = match std::env::var("DISPLAY") {
        Ok(d) if !d.is_empty() => d,
        _ => t_ui_fail("cannot open a window: DISPLAY is not set (is a graphical X11 session running?)".to_string(), loc),
    };
    let (host, rest) = match display.split_once(':') {
        Some(parts) => parts,
        None => t_ui_fail(format!("cannot parse DISPLAY `{}`", display), loc),
    };
    if !host.is_empty() && host != "unix" {
        t_ui_fail(format!("only local X displays are supported (DISPLAY is `{}`)", display), loc)
    }
    let number = rest.split('.').next().unwrap_or("0").to_string();
    let mut stream = match UnixStream::connect(format!("/tmp/.X11-unix/X{}", number)) {
        Ok(s) => s,
        Err(e) => t_ui_fail(format!("cannot connect to X display `{}`: {}", display, e), loc),
    };

    // Connection setup.
    let (auth_name, auth_data) = t_x_authority(&number);
    let mut setup = vec![b'l', 0];
    t_put16(&mut setup, 11);
    t_put16(&mut setup, 0);
    t_put16(&mut setup, auth_name.len() as u16);
    t_put16(&mut setup, auth_data.len() as u16);
    t_put16(&mut setup, 0);
    setup.extend_from_slice(&auth_name);
    t_pad4(&mut setup);
    setup.extend_from_slice(&auth_data);
    t_pad4(&mut setup);
    {
        use std::io::Write;
        if let Err(e) = stream.write_all(&setup) {
            t_ui_fail(format!("cannot talk to the X server: {}", e), loc)
        }
    }
    let mut header = [0u8; 8];
    t_read_exact(&mut stream, &mut header, loc);
    let mut info = vec![0u8; t_u16(&header, 6) as usize * 4];
    t_read_exact(&mut stream, &mut info, loc);
    if header[0] != 1 {
        let reason_len = (header[1] as usize).min(info.len());
        let reason = String::from_utf8_lossy(&info[..reason_len]).trim().to_string();
        t_ui_fail(format!("the X server refused the connection: {}", reason), loc)
    }
    let id_base = t_u32(&info, 4);
    let id_mask = t_u32(&info, 8);
    let vendor_len = t_u16(&info, 16) as usize;
    let formats = info[21] as usize;
    let min_keycode = info[26];
    let max_keycode = info[27];
    let screen = 32 + ((vendor_len + 3) / 4) * 4 + formats * 8;
    let root = t_u32(&info, screen);
    let root_depth = info[screen + 38];
    if root_depth != 24 && root_depth != 32 {
        t_ui_fail(format!("only 24-bit color displays are supported (this display has depth {})", root_depth), loc)
    }

    let ui = T_Ui {
        stream,
        out: Vec::new(),
        seq: 0,
        id_base,
        id_mask,
        next_id: 1,
        window: 0,
        pixmap: 0,
        gc: 0,
        width: width as u16,
        height: height as u16,
        wm_delete: 0,
        events: VecDeque::new(),
        input: Vec::new(),
        min_keycode,
        keysyms_per_keycode: 0,
        keysyms: Vec::new(),
        char_width: 8,
        font_ascent: 12,
        color: 0,
        line_width: 1,
    };
    (ui, root, root_depth, min_keycode, max_keycode)
}

pub fn t_open_window(title: Rc<String>, width: i64, height: i64, loc: &str) {
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        t_ui_fail(format!("window size {}x{} is outside 1..4096", width, height), loc)
    }
    if T_UI.with(|cell| cell.borrow().is_some()) {
        t_ui_fail("a window is already open".to_string(), loc)
    }
    let (mut ui, root, root_depth, min_keycode, max_keycode) = t_ui_connect(width, height, loc);
    let wm_protocols = ui.intern_atom("WM_PROTOCOLS", loc);
    ui.wm_delete = ui.intern_atom("WM_DELETE_WINDOW", loc);
    let net_wm_name = ui.intern_atom("_NET_WM_NAME", loc);
    let utf8_string = ui.intern_atom("UTF8_STRING", loc);

    // Window: white background; key, button, exposure and structure events.
    ui.window = ui.new_id();
    let start = ui.begin(1, root_depth);
    t_put32(&mut ui.out, ui.window);
    t_put32(&mut ui.out, root);
    t_put16(&mut ui.out, 0);
    t_put16(&mut ui.out, 0);
    t_put16(&mut ui.out, ui.width);
    t_put16(&mut ui.out, ui.height);
    t_put16(&mut ui.out, 0);
    t_put16(&mut ui.out, 1);
    t_put32(&mut ui.out, 0);
    t_put32(&mut ui.out, 0x2 | 0x800);
    t_put32(&mut ui.out, 0xffffff);
    t_put32(&mut ui.out, 0x1 | 0x2 | 0x4 | 0x8000 | 0x20000);
    ui.end(start);

    let latin1: Vec<u8> = title.chars().map(|c| if (c as u32) < 256 { c as u32 as u8 } else { b'?' }).collect();
    ui.change_property(39, 31, 8, &latin1);
    ui.change_property(net_wm_name, utf8_string, 8, title.as_bytes());
    ui.change_property(wm_protocols, 4, 32, &ui.wm_delete.to_le_bytes());
    let mut hints = vec![0u32; 18];
    hints[0] = 16 | 32;
    hints[5] = width as u32;
    hints[6] = height as u32;
    hints[7] = width as u32;
    hints[8] = height as u32;
    let hint_bytes: Vec<u8> = hints.iter().flat_map(|h| h.to_le_bytes()).collect();
    ui.change_property(40, 41, 32, &hint_bytes);

    // Back buffer.
    ui.pixmap = ui.new_id();
    let start = ui.begin(53, root_depth);
    t_put32(&mut ui.out, ui.pixmap);
    t_put32(&mut ui.out, ui.window);
    t_put16(&mut ui.out, ui.width);
    t_put16(&mut ui.out, ui.height);
    ui.end(start);

    // Font: the first one the server knows.
    let mut font = 0u32;
    for name in ["-misc-fixed-bold-r-normal--18-120-100-100-c-90-iso8859-1", "-misc-fixed-bold-r-normal--15-140-75-75-c-90-iso8859-1", "9x15bold", "9x15", "fixed"] {
        let candidate = ui.new_id();
        let first = ui.seq.wrapping_add(1);
        let start = ui.begin(45, 0);
        t_put32(&mut ui.out, candidate);
        t_put16(&mut ui.out, name.len() as u16);
        t_put16(&mut ui.out, 0);
        ui.out.extend_from_slice(name.as_bytes());
        ui.end(start);
        let start = ui.begin(47, 0);
        t_put32(&mut ui.out, candidate);
        ui.end(start);
        if let Ok(r) = ui.reply_since(first, loc) {
            font = candidate;
            ui.char_width = t_i16(&r, 28).max(1) as i64;
            ui.font_ascent = t_i16(&r, 52) as i64;
            break;
        }
    }

    // Graphics context: foreground, line width, round caps and joins, font, no exposure events.
    ui.gc = ui.new_id();
    let start = ui.begin(55, 0);
    t_put32(&mut ui.out, ui.gc);
    t_put32(&mut ui.out, ui.pixmap);
    let mut mask = 0x4 | 0x10 | 0x40 | 0x80 | 0x10000;
    if font != 0 { mask |= 0x4000; }
    t_put32(&mut ui.out, mask);
    t_put32(&mut ui.out, 0);
    t_put32(&mut ui.out, 1);
    t_put32(&mut ui.out, 2);
    t_put32(&mut ui.out, 1);
    if font != 0 { t_put32(&mut ui.out, font); }
    t_put32(&mut ui.out, 0);
    ui.end(start);

    // Clear the back buffer, show the window.
    ui.set_gc(0xffffff, 1);
    let start = ui.draw_request(70);
    t_put16(&mut ui.out, 0);
    t_put16(&mut ui.out, 0);
    t_put16(&mut ui.out, ui.width);
    t_put16(&mut ui.out, ui.height);
    ui.end(start);
    let start = ui.begin(8, 0);
    t_put32(&mut ui.out, ui.window);
    ui.end(start);

    // Keyboard mapping, for key names.
    let start = ui.begin(101, 0);
    ui.out.push(min_keycode);
    ui.out.push(max_keycode.wrapping_sub(min_keycode).wrapping_add(1));
    t_put16(&mut ui.out, 0);
    ui.end(start);
    let r = ui.reply(loc);
    ui.keysyms_per_keycode = r[1];
    ui.keysyms = r[32..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();

    T_UI.with(|cell| *cell.borrow_mut() = Some(ui));
}

pub fn t_fill_rectangle(x: i64, y: i64, width: i64, height: i64, color: i64, loc: &str) {
    if width <= 0 || height <= 0 { return; }
    t_with_ui(loc, |ui| {
        ui.set_gc(color, ui.line_width);
        let start = ui.draw_request(70);
        t_put16(&mut ui.out, t_coord(x));
        t_put16(&mut ui.out, t_coord(y));
        t_put16(&mut ui.out, t_size(width));
        t_put16(&mut ui.out, t_size(height));
        ui.end(start);
    })
}

pub fn t_draw_line(x1: i64, y1: i64, x2: i64, y2: i64, thickness: i64, color: i64, loc: &str) {
    if thickness <= 0 { return; }
    t_with_ui(loc, |ui| {
        ui.set_gc(color, thickness);
        let start = ui.draw_request(66);
        for v in [x1, y1, x2, y2] { t_put16(&mut ui.out, t_coord(v)); }
        ui.end(start);
    })
}

fn t_arc(opcode: u8, center_x: i64, center_y: i64, radius: i64, thickness: i64, color: i64, loc: &str) {
    if radius <= 0 || thickness <= 0 { return; }
    t_with_ui(loc, |ui| {
        ui.set_gc(color, thickness);
        let start = ui.draw_request(opcode);
        t_put16(&mut ui.out, t_coord(center_x - radius));
        t_put16(&mut ui.out, t_coord(center_y - radius));
        t_put16(&mut ui.out, t_size(radius * 2));
        t_put16(&mut ui.out, t_size(radius * 2));
        t_put16(&mut ui.out, 0);
        t_put16(&mut ui.out, (360 * 64) as u16);
        ui.end(start);
    })
}

pub fn t_draw_circle(center_x: i64, center_y: i64, radius: i64, thickness: i64, color: i64, loc: &str) {
    t_arc(68, center_x, center_y, radius, thickness, color, loc)
}

pub fn t_fill_circle(center_x: i64, center_y: i64, radius: i64, color: i64, loc: &str) {
    t_arc(71, center_x, center_y, radius, 1, color, loc)
}

pub fn t_draw_text(x: i64, y: i64, text: Rc<String>, color: i64, loc: &str) {
    let bytes: Vec<u8> = text.chars().map(|c| if (c as u32) >= 32 && (c as u32) < 256 { c as u32 as u8 } else { b'?' }).collect();
    if bytes.is_empty() { return; }
    t_with_ui(loc, |ui| {
        ui.set_gc(color, ui.line_width);
        let start = ui.draw_request(74);
        t_put16(&mut ui.out, t_coord(x));
        t_put16(&mut ui.out, t_coord(y + ui.font_ascent));
        for chunk in bytes.chunks(254) {
            ui.out.push(chunk.len() as u8);
            ui.out.push(0);
            ui.out.extend_from_slice(chunk);
        }
        ui.end(start);
    })
}

pub fn t_text_width(text: Rc<String>, loc: &str) -> i64 {
    t_with_ui(loc, |ui| ui.char_width * text.chars().count() as i64)
}

pub fn t_present(loc: &str) {
    t_with_ui(loc, |ui| ui.present(loc))
}

pub fn t_next_event(loc: &str) -> Rc<T_WindowEvent> {
    t_with_ui(loc, |ui| {
        ui.flush(loc);
        loop {
            let packet = match ui.events.pop_front() {
                Some(p) => p,
                None => ui.read_packet(loc),
            };
            if let Some(event) = ui.event_of(packet, loc) { return event; }
        }
    })
}

pub fn t_poll_event(loc: &str) -> Option<Rc<T_WindowEvent>> {
    t_with_ui(loc, |ui| {
        ui.flush(loc);
        loop {
            let packet = match ui.events.pop_front() {
                Some(p) => p,
                None => ui.next_packet(false, loc)?,
            };
            if let Some(event) = ui.event_of(packet, loc) { return Some(event); }
        }
    })
}

/// Draws rows of `width` colors at (x, y), each pixel as a scale x scale square, with PutImage
/// (ZPixmap, 32 bits per pixel). The image is sent in bands of rows, as one request may carry at
/// most 262,140 bytes without the BIG-REQUESTS extension.
pub fn t_draw_pixels(x: i64, y: i64, width: i64, colors: &Rc<Vec<i64>>, scale: i64, loc: &str) {
    if width < 1 || width > 4096 || !(1..=16).contains(&scale) || colors.len() as i64 % width != 0 {
        t_fail("R004", format!("draw_pixels: width {} and scale {} do not fit {} colors (scale 1..16, width 1..4096, colors a multiple of width)", width, scale, colors.len()), loc)
    }
    if let Some(bad) = colors.iter().find(|c| !(0..=0xffffff).contains(*c)) {
        t_fail("R011", format!("draw_pixels: {} is not a color (0..16777215)", bad), loc)
    }
    let rows = colors.len() as i64 / width;
    let out_width = (width * scale) as usize;
    let rows_per_band = ((262140 - 24) / (out_width * 4)).max(1);
    t_with_ui(loc, |ui| {
        let mut out_row = 0usize;
        let total_rows = (rows * scale) as usize;
        while out_row < total_rows {
            let band = rows_per_band.min(total_rows - out_row);
            let start = ui.begin(72, 2);
            t_put32(&mut ui.out, ui.pixmap);
            t_put32(&mut ui.out, ui.gc);
            t_put16(&mut ui.out, out_width as u16);
            t_put16(&mut ui.out, band as u16);
            t_put16(&mut ui.out, t_coord(x));
            t_put16(&mut ui.out, t_coord(y + out_row as i64));
            ui.out.push(0);
            ui.out.push(24);
            t_put16(&mut ui.out, 0);
            ui.out.reserve(out_width * band * 4);
            for r in out_row..out_row + band {
                let source = (r / scale as usize) * width as usize;
                for c in &colors[source..source + width as usize] {
                    let pixel = (*c as u32).to_le_bytes();
                    for _ in 0..scale { ui.out.extend_from_slice(&pixel); }
                }
            }
            ui.end(start);
            out_row += band;
            if ui.out.len() > 1 << 20 { ui.flush(loc); }
        }
    })
}
