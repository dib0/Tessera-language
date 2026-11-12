// ---------------------------------------------------------------------------
// Tessera `audio` runtime (draft 0.19), included in a program that plays sound.
//
// One mixer thread adds up to 16 voices and sends 16-bit stereo frames at
// 22,050 per second to the sound device. Where that device is differs per
// system, and `TSink` is the only part that knows:
//
//   unix     a player that reads raw samples from standard input (`aplay`,
//            `pw-cat`, or the program named in TESSERA_AUDIO_PLAYER). Safe
//            Rust: a child process and a pipe.
//   Windows  the sound card itself through waveOut (runtime/winmm.rs), because
//            Windows ships no such player. TESSERA_AUDIO_PLAYER still works and
//            is tried first.
//
// Without any device, sounds are dropped silently.
// ---------------------------------------------------------------------------

struct TVoice { samples: Vec<u8>, position: u64, step: u64, left: i32, right: i32 }
struct TMixer { voices: Vec<TVoice>, running: bool }
static T_MIXER: std::sync::OnceLock<Option<std::sync::Arc<std::sync::Mutex<TMixer>>>> = std::sync::OnceLock::new();

/// Where mixed frames go. `write` blocks while the device is full, which is
/// what paces the mixer thread, and returns false when the device is gone.
enum TSink {
    Player(std::process::ChildStdin),
    #[cfg(windows)]
    Device(t_sound_backend::TWaveOut),
}

impl TSink {
    fn write(&mut self, frames: &[u8]) -> bool {
        match self {
            TSink::Player(input) => {
                use std::io::Write;
                input.write_all(frames).is_ok()
            }
            #[cfg(windows)]
            TSink::Device(device) => device.write(frames),
        }
    }
}

/// The player named in TESSERA_AUDIO_PLAYER, or one of the usual ones.
fn t_audio_player() -> Option<TSink> {
    use std::process::{Command, Stdio};
    let mut candidates: Vec<Vec<String>> = Vec::new();
    if let Ok(custom) = std::env::var("TESSERA_AUDIO_PLAYER") {
        if !custom.trim().is_empty() { candidates.push(custom.split_whitespace().map(|s| s.to_string()).collect()); }
    }
    if !cfg!(windows) {
        candidates.push(["aplay", "-q", "-t", "raw", "-f", "S16_LE", "-r", "22050", "-c", "2", "--buffer-time=60000", "-"].iter().map(|s| s.to_string()).collect());
        candidates.push(["pw-cat", "--playback", "--format=s16", "--rate=22050", "--channels=2", "--latency=40ms", "-"].iter().map(|s| s.to_string()).collect());
    }
    for argv in candidates {
        let child = Command::new(&argv[0]).args(&argv[1..]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = child {
            if let Some(input) = child.stdin.take() { return Some(TSink::Player(input)); }
        }
    }
    None
}

#[cfg(windows)]
fn t_audio_device() -> Option<TSink> {
    if let Some(player) = t_audio_player() { return Some(player); }
    t_sound_backend::TWaveOut::open().map(TSink::Device)
}

#[cfg(not(windows))]
fn t_audio_device() -> Option<TSink> {
    t_audio_player()
}

fn t_start_audio() -> Option<std::sync::Arc<std::sync::Mutex<TMixer>>> {
    let mut sink = t_audio_device()?;
    let mixer = std::sync::Arc::new(std::sync::Mutex::new(TMixer { voices: Vec::new(), running: true }));
    let shared = mixer.clone();
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; 256 * 4];
        // Pace by the clock: stay about 60 ms ahead of real time. Writing as fast as the
        // device accepts would queue its whole buffer (about 0.75 s for a pipe) in front of
        // every new sound. When far behind (the program was paused), skip ahead instead of
        // catching up, which would build the same queue.
        let started = std::time::Instant::now();
        let lead: u64 = 22050 * 60 / 1000;
        let mut written: u64 = 0;
        loop {
            let due = started.elapsed().as_micros() as u64 * 22050 / 1_000_000 + lead;
            if written + 256 > due {
                std::thread::sleep(std::time::Duration::from_micros((written + 256 - due) * 1_000_000 / 22050));
                continue;
            }
            if due > written + 2 * lead {
                written = due - lead;
            }
            written += 256;
            {
                let mut m = match shared.lock() { Ok(m) => m, Err(_) => return };
                if !m.running { return; }
                for frame in 0..256 {
                    let (mut l, mut r) = (0i32, 0i32);
                    for v in m.voices.iter_mut() {
                        let index = (v.position >> 16) as usize;
                        if index < v.samples.len() {
                            let s = (v.samples[index] as i32 - 128) * 256;
                            l += s * v.left / 100;
                            r += s * v.right / 100;
                            v.position += v.step;
                        }
                    }
                    m.voices.retain(|v| ((v.position >> 16) as usize) < v.samples.len());
                    let l = l.clamp(-32768, 32767) as i16;
                    let r = r.clamp(-32768, 32767) as i16;
                    buffer[frame * 4..frame * 4 + 2].copy_from_slice(&l.to_le_bytes());
                    buffer[frame * 4 + 2..frame * 4 + 4].copy_from_slice(&r.to_le_bytes());
                }
            }
            if !sink.write(&buffer) {
                if let Ok(mut m) = shared.lock() { m.running = false; }
                return;
            }
        }
    });
    Some(mixer)
}

fn t_play_sound(samples: &Rc<Vec<u8>>, rate: i64, volume: i64, pan: i64, loc: &str) {
    if !(1000..=96000).contains(&rate) || !(0..=100).contains(&volume) || !(-100..=100).contains(&pan) {
        t_fail("R004", format!("play_sound: rate {} (1000..96000), volume {} (0..100) or pan {} (-100..100) out of range", rate, volume, pan), loc)
    }
    let mixer = match T_MIXER.get_or_init(t_start_audio) { Some(m) => m, None => return };
    if let Ok(mut m) = mixer.lock() {
        if !m.running || samples.is_empty() || volume == 0 { return; }
        if m.voices.len() >= 16 { m.voices.remove(0); }
        // Equal-power-ish panning: the far side fades to a third at full pan.
        let left = (volume * (300 - (pan.max(0) * 2)) / 300) as i32;
        let right = (volume * (300 + (pan.min(0) * 2)) / 300) as i32;
        let step = ((rate as u64) << 16) / 22050;
        m.voices.push(TVoice { samples: samples.to_vec(), position: 0, step, left, right });
    }
}
