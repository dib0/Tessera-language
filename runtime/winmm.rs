// ---------------------------------------------------------------------------
// Tessera `audio` runtime for Windows: the sound card through waveOut (winmm).
//
// The counterpart of the player process the unix backend pipes samples into.
// Windows ships no program that plays raw samples from standard input, so the
// frames go to the device itself. runtime/audio.rs does all the mixing; this
// file only hands finished frames over and waits when the device is full.
//
// Together with runtime/win32.rs and runtime/adapter.rs this is the only part
// of the runtime that contains `unsafe` code; the sound device has no interface
// other than its C functions. The unsafe surface is the `extern "system"`
// declarations and their call sites. A program that plays sound therefore says
// `#![cfg_attr(windows, deny(unsafe_code))]` instead of `forbid`, and only this
// module is `#[allow(unsafe_code)]`; on unix the blanket `forbid` still holds.
//
// How it works: four buffers of 256 frames (about 46 ms of sound) go round. To
// write, the oldest buffer is waited for, refilled and handed back to the
// device, so `write` blocks exactly as a pipe to a player would and paces the
// mixer thread the same way. Every buffer stays at a fixed address for as long
// as the device may read it: they live in a boxed array that is only freed in
// `Drop`, after `waveOutReset` has taken every buffer back.
// ---------------------------------------------------------------------------

type T_Handle = *mut std::ffi::c_void;

const BUFFERS: usize = 4;
const FRAMES: usize = 256;
const BYTES: usize = FRAMES * 4; // 16-bit stereo

const WAVE_MAPPER: usize = 0xffff_ffff;
const WAVE_FORMAT_PCM: u16 = 1;
const WHDR_DONE: u32 = 0x0000_0001;
const MMSYSERR_NOERROR: u32 = 0;
const CALLBACK_NULL: u32 = 0;

#[repr(C)]
struct T_WaveFormat {
    format_tag: u16,
    channels: u16,
    samples_per_second: u32,
    bytes_per_second: u32,
    block_align: u16,
    bits_per_sample: u16,
    extra_size: u16,
}

#[repr(C)]
struct T_WaveHeader {
    data: *mut u8,
    buffer_length: u32,
    bytes_recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut T_WaveHeader,
    reserved: usize,
}

#[link(name = "winmm")]
extern "system" {
    fn waveOutOpen(out: *mut T_Handle, device: usize, format: *const T_WaveFormat,
                   callback: usize, instance: usize, flags: u32) -> u32;
    fn waveOutPrepareHeader(device: T_Handle, header: *mut T_WaveHeader, size: u32) -> u32;
    fn waveOutUnprepareHeader(device: T_Handle, header: *mut T_WaveHeader, size: u32) -> u32;
    fn waveOutWrite(device: T_Handle, header: *mut T_WaveHeader, size: u32) -> u32;
    fn waveOutReset(device: T_Handle) -> u32;
    fn waveOutClose(device: T_Handle) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn Sleep(milliseconds: u32);
}

struct T_Block {
    header: T_WaveHeader,
    samples: [u8; BYTES],
}

pub struct TWaveOut {
    device: T_Handle,
    // Boxed, so the addresses the device was given never move.
    blocks: Box<[T_Block; BUFFERS]>,
    next: usize,
    queued: usize,
}

// The mixer owns the device on its own thread and nothing else touches it.
unsafe impl Send for TWaveOut {}

impl TWaveOut {
    /// Opens the default sound device, or returns None when there is none (a
    /// machine without a sound card, or a session that cannot reach it). A
    /// program then simply plays no sound, as it does without a player on unix.
    pub fn open() -> Option<TWaveOut> {
        let format = T_WaveFormat {
            format_tag: WAVE_FORMAT_PCM,
            channels: 2,
            samples_per_second: 22050,
            bytes_per_second: 22050 * 4,
            block_align: 4,
            bits_per_sample: 16,
            extra_size: 0,
        };
        let mut device: T_Handle = std::ptr::null_mut();
        #[allow(unsafe_code)]
        unsafe {
            if waveOutOpen(&mut device, WAVE_MAPPER, &format, 0, 0, CALLBACK_NULL) != MMSYSERR_NOERROR {
                return None;
            }
        }
        let mut out = TWaveOut {
            device,
            blocks: Box::new(std::array::from_fn(|_| T_Block {
                header: T_WaveHeader {
                    data: std::ptr::null_mut(),
                    buffer_length: BYTES as u32,
                    bytes_recorded: 0,
                    user: 0,
                    // Starts as "the device is done with it", so the first four
                    // writes do not wait.
                    flags: WHDR_DONE,
                    loops: 0,
                    next: std::ptr::null_mut(),
                    reserved: 0,
                },
                samples: [0; BYTES],
            })),
            next: 0,
            queued: 0,
        };
        for i in 0..BUFFERS {
            let block = &mut out.blocks[i];
            block.header.data = block.samples.as_mut_ptr();
        }
        Some(out)
    }

    /// Hands `frames` to the device, waiting until a buffer is free. False once
    /// the device refuses the sound, which stops the mixer.
    pub fn write(&mut self, frames: &[u8]) -> bool {
        if frames.len() != BYTES {
            return false;
        }
        let index = self.next;
        self.next = (self.next + 1) % BUFFERS;
        // Every buffer after the first round was prepared before and must be
        // released before it is prepared again.
        let used_before = self.queued > index;
        if !used_before {
            self.queued = index + 1;
        }
        let device = self.device;
        let size = std::mem::size_of::<T_WaveHeader>() as u32;
        let block: *mut T_Block = &mut self.blocks[index];
        #[allow(unsafe_code)]
        unsafe {
            // Wait for the device to be done with this buffer. Every buffer is
            // 11.6 ms of sound, so a sleep of 1 ms costs at most a tenth of
            // that. The flag is written by the sound driver on its own thread,
            // so it is read afresh every time rather than kept in a register.
            let flags = std::ptr::addr_of!((*block).header.flags);
            let mut waited = 0u32;
            while std::ptr::read_volatile(flags) & WHDR_DONE == 0 {
                Sleep(1);
                waited += 1;
                // A device that never gives a buffer back (unplugged while it
                // was playing) must not hang the mixer thread for good.
                if waited > 1000 {
                    return false;
                }
            }
            let header = std::ptr::addr_of_mut!((*block).header);
            if used_before && waveOutUnprepareHeader(device, header, size) != MMSYSERR_NOERROR {
                return false;
            }
            (*block).samples.copy_from_slice(frames);
            (*block).header.flags = 0;
            waveOutPrepareHeader(device, header, size) == MMSYSERR_NOERROR
                && waveOutWrite(device, header, size) == MMSYSERR_NOERROR
        }
    }
}

impl Drop for TWaveOut {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        unsafe {
            // Takes every buffer back from the device before they are freed.
            waveOutReset(self.device);
            let size = std::mem::size_of::<T_WaveHeader>() as u32;
            for i in 0..self.queued.min(BUFFERS) {
                let block: *mut T_Block = &mut self.blocks[i];
                waveOutUnprepareHeader(self.device, std::ptr::addr_of_mut!((*block).header), size);
            }
            waveOutClose(self.device);
        }
    }
}
