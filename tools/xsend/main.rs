// Test helper: sends synthetic X11 events to a window, so graphical Tessera
// programs can be tested without a person clicking.
//
//   xsend <window-id> click <x> <y> [button]
//   xsend <window-id> key <name>        (a single ASCII character, Escape, Enter, Left, Up,
//                                        Right, Down, Control, Shift, Alt, F1 to F12)
//   xsend <window-id> keyup <name>      (the release of such a key)
//   xsend <window-id> close             (window manager close request)
//
// Built by build.sh from runtime/core.rs + runtime/ui.rs + this file.

fn xsend_main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: xsend <window-id> click <x> <y> [button] | key <name> | keyup <name> | close";
    if args.len() < 2 { eprintln!("{}", usage); std::process::exit(2) }
    let window = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).unwrap_or_else(|_| { eprintln!("bad window id"); std::process::exit(2) });
    let loc = "xsend";
    let (mut ui, root, _depth, min_keycode, max_keycode) = t_ui_connect(1, 1, loc);
    let mut event = vec![0u8; 32];
    match args[1].as_str() {
        "click" if args.len() >= 4 => {
            let x: i16 = args[2].parse().unwrap_or(0);
            let y: i16 = args[3].parse().unwrap_or(0);
            let button: u8 = args.get(4).and_then(|b| b.parse().ok()).unwrap_or(1);
            event[0] = 4;
            event[1] = button;
            event[8..12].copy_from_slice(&root.to_le_bytes());
            event[12..16].copy_from_slice(&window.to_le_bytes());
            event[24..26].copy_from_slice(&x.to_le_bytes());
            event[26..28].copy_from_slice(&y.to_le_bytes());
            event[30] = 1;
        }
        "key" | "keyup" if args.len() >= 3 => {
            let keysym: u32 = match args[2].as_str() {
                "Escape" => 0xff1b,
                "Enter" => 0xff0d,
                "Left" => 0xff51,
                "Up" => 0xff52,
                "Right" => 0xff53,
                "Down" => 0xff54,
                "Shift" => 0xffe1,
                "Control" => 0xffe3,
                "Alt" => 0xffe9,
                f if f.len() >= 2 && f.starts_with('F') && f[1..].parse::<u32>().map_or(false, |n| (1..=12).contains(&n)) => 0xffbe + f[1..].parse::<u32>().unwrap() - 1,
                s if s.len() == 1 => s.as_bytes()[0] as u32,
                _ => { eprintln!("unsupported key"); std::process::exit(2) }
            };
            let start = ui.begin(101, 0);
            ui.out.push(min_keycode);
            ui.out.push(max_keycode.wrapping_sub(min_keycode).wrapping_add(1));
            t_put16(&mut ui.out, 0);
            ui.end(start);
            let r = ui.reply(loc);
            let per = r[1] as usize;
            let syms: Vec<u32> = r[32..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            let index = syms.iter().position(|&s| s == keysym).unwrap_or_else(|| { eprintln!("key not on keyboard"); std::process::exit(2) });
            event[0] = if args[1] == "keyup" { 3 } else { 2 };
            event[1] = min_keycode + (index / per) as u8;
            event[8..12].copy_from_slice(&root.to_le_bytes());
            event[12..16].copy_from_slice(&window.to_le_bytes());
            event[28] = (index % per != 0) as u8;
            event[30] = 1;
        }
        "close" => {
            let protocols = ui.intern_atom("WM_PROTOCOLS", loc);
            let delete = ui.intern_atom("WM_DELETE_WINDOW", loc);
            event[0] = 33;
            event[1] = 32;
            event[4..8].copy_from_slice(&window.to_le_bytes());
            event[8..12].copy_from_slice(&protocols.to_le_bytes());
            event[12..16].copy_from_slice(&delete.to_le_bytes());
        }
        _ => { eprintln!("{}", usage); std::process::exit(2) }
    }
    let start = ui.begin(25, 0);
    t_put32(&mut ui.out, window);
    t_put32(&mut ui.out, 0);
    ui.out.extend_from_slice(&event);
    ui.end(start);
    // Round trip (GetInputFocus) so the event is delivered before we exit.
    let start = ui.begin(43, 0);
    ui.end(start);
    let _ = ui.reply(loc);
}

fn main() { t_run(xsend_main); }
