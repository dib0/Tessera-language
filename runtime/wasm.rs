// Tessera WebAssembly adapter: the crate root of `tessera1 build --wasm`.
//
//   rustc --edition 2021 --target wasm32-unknown-unknown --crate-type cdylib \
//         -C opt-level=s -C link-arg=-zstack-size=<bytes> -o app.wasm wasm.rs
//
// next to the generated `app.rs`. In a browser the "operating system" is the
// JavaScript host (browser/tessera.js), and these are its system calls. This
// file, like adapter.rs, win32.rs and winmm.rs, is one of the few parts of the
// runtime allowed to contain `unsafe`: calling the host's imported functions
// and handing it a pointer into this module's memory, nothing else. The
// runtime reaches the host only through the safe functions below
// (`#[cfg(target_arch = "wasm32")]` branches in core.rs, extra.rs and web.rs).
//
// Host interface (import module "tessera"; documented in browser/README.md):
//   get(key, key_len) -> i32       makes the value of `key` ready and returns its
//                                  length in bytes, or -1 when there is none
//   take(destination)              copies the value made ready by `get`
//   put(key, key_len, value, value_len) -> i32   0 when accepted, -1 refused
//   now() -> f64                   milliseconds since 1970 (UTC)
//   monotonic() -> f64             milliseconds from an arbitrary start
//   random(destination, len)       fills with secure random bytes
// Keys: "args" (arguments separated by NUL), "stdin", "env/NAME",
// "file/PATH" (get and put), "stdout", "stderr" and "exit" (put; the code as
// decimal text, sent once just before the program ends), and for the `page`
// effect "page/" followed by a JSON array of an operation and its arguments
// (get; runtime/page.rs).
// Export: `tessera_main() -> i32` runs `main` and returns its exit code. A
// runtime error or `exit` ends the run by a trap (Rust panics abort on this
// target) after "exit" was put; the host treats that trap as the end of the run.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "app.rs"]
mod app;

mod host {
    #[link(wasm_import_module = "tessera")]
    extern "C" {
        pub fn get(key: *const u8, key_len: usize) -> i32;
        pub fn take(destination: *mut u8);
        pub fn put(key: *const u8, key_len: usize, value: *const u8, value_len: usize) -> i32;
        pub fn now() -> f64;
        pub fn monotonic() -> f64;
        pub fn random(destination: *mut u8, len: usize);
    }
}

/// The host's value for `key`, or `None` when it has none.
fn wasm_get(key: &str) -> Option<Vec<u8>> {
    // SAFETY: the pointer and length describe `key`, which is valid for the whole call.
    let length = unsafe { host::get(key.as_ptr(), key.len()) };
    if length < 0 {
        return None;
    }
    let mut value = vec![0u8; length as usize];
    // SAFETY: `value` has exactly the length the host reported for the value it will copy.
    unsafe { host::take(value.as_mut_ptr()) };
    Some(value)
}

/// Gives `value` to the host under `key`; false when the host refuses it.
fn wasm_put(key: &str, value: &[u8]) -> bool {
    // SAFETY: both pointers and lengths describe live slices; the host only reads them during the call.
    unsafe { host::put(key.as_ptr(), key.len(), value.as_ptr(), value.len()) == 0 }
}

/// Standard output (1) or standard error (2).
fn wasm_write(stream: i32, bytes: &[u8]) {
    let _ = wasm_put(if stream == 2 { "stderr" } else { "stdout" }, bytes);
}

fn wasm_now() -> f64 {
    // SAFETY: no arguments; the host returns a number.
    unsafe { host::now() }
}

fn wasm_monotonic() -> f64 {
    // SAFETY: no arguments; the host returns a number.
    unsafe { host::monotonic() }
}

fn wasm_random(buffer: &mut [u8]) {
    // SAFETY: the pointer and length describe `buffer`, which the host fills during the call.
    unsafe { host::random(buffer.as_mut_ptr(), buffer.len()) }
}

/// Runs `main`; returns 0 when it returns. Runtime errors and `exit` end in a trap instead.
#[no_mangle]
pub extern "C" fn tessera_main() -> i32 {
    app::tessera_wasm_main()
}
