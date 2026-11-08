// Tessera shared-library adapter (C ABI) for in-process web server modules.
//
// This file is the crate root of a `--crate-type cdylib` build:
//
//   rustc --edition 2021 --crate-type cdylib -C opt-level=2 -C panic=unwind \
//         -o libapp.so adapter.rs
//
// next to the generated `app.rs`. It is the ONLY part of the Tessera runtime
// that contains `unsafe` code. The unsafe surface is limited to:
//   1. turning the caller's (pointer, length) request into a `&[u8]`,
//   2. writing the response pointer/length into the caller's out-parameters,
//   3. reconstructing the `Box<[u8]>` in `tessera_free`.
// Everything else (decoding, running the application, limits, error -> 500,
// encoding) happens in safe code in `app.rs` (`tw_library_handle`).
//
// The wire format and C declarations are documented in hosts/include/tessera.h.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "app.rs"]
mod app;

use std::panic::{catch_unwind, AssertUnwindSafe};

/// ABI version implemented by this library. Hosts refuse to use a library
/// that reports a different version.
pub const TESSERA_ABI_VERSION: i32 = 1;

/// Result codes of `tessera_handle`.
const OK: i32 = 0;
const ERR_NULL_ARGUMENT: i32 = 1;
const ERR_INTERNAL: i32 = 2;

#[no_mangle]
pub extern "C" fn tessera_abi_version() -> i32 {
    TESSERA_ABI_VERSION
}

/// Handle one wire-format request.
///
/// On success returns 0 and stores a Rust-allocated buffer in `*response` /
/// `*response_len`; the caller must release it with `tessera_free` using the
/// same pointer and length. Application errors are reported as HTTP 500 in
/// the response, not as a nonzero return value. A nonzero return value means
/// invalid arguments (1) or an internal failure that should never happen (2);
/// in that case `*response` is set to NULL and `*response_len` to 0 (when
/// those pointers are non-null).
///
/// # Safety
/// `request` must point to `request_len` readable bytes (it may be NULL only
/// when `request_len` is 0). `response` and `response_len` must be valid,
/// writable, non-NULL pointers. The request buffer is only read during the call.
#[no_mangle]
pub unsafe extern "C" fn tessera_handle(
    request: *const u8,
    request_len: usize,
    response: *mut *mut u8,
    response_len: *mut usize,
) -> i32 {
    if response.is_null() || response_len.is_null() {
        return ERR_NULL_ARGUMENT;
    }
    // SAFETY: both out-pointers were checked for NULL above; the caller
    // guarantees they are valid and writable.
    unsafe {
        *response = std::ptr::null_mut();
        *response_len = 0;
    }
    if request.is_null() && request_len != 0 {
        return ERR_NULL_ARGUMENT;
    }
    let input: &[u8] = if request_len == 0 {
        &[]
    } else {
        // SAFETY: non-NULL (checked above) and the caller guarantees
        // `request_len` readable bytes that stay valid for this call.
        unsafe { std::slice::from_raw_parts(request, request_len) }
    };

    // Unwinding across `extern "C"` is undefined behaviour / an abort, so
    // catch everything here. tw_library_handle is designed never to panic;
    // this is a last line of defence.
    let result = catch_unwind(AssertUnwindSafe(|| app::tw_library_handle(app::tessera_app, input)));
    let bytes: Vec<u8> = match result {
        Ok(bytes) => bytes,
        Err(_) => return ERR_INTERNAL,
    };

    let boxed: Box<[u8]> = bytes.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *mut u8;
    // SAFETY: out-pointers checked for NULL above. Ownership of the buffer
    // passes to the caller until tessera_free(ptr, len).
    unsafe {
        *response = ptr;
        *response_len = len;
    }
    OK
}

/// Release a response buffer returned by `tessera_handle`.
///
/// # Safety
/// `response` and `response_len` must be exactly the values produced by one
/// successful `tessera_handle` call, and each buffer must be freed only once.
/// Passing NULL is allowed and does nothing.
#[no_mangle]
pub unsafe extern "C" fn tessera_free(response: *mut u8, response_len: usize) {
    if response.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: (response, response_len) came from Box::<[u8]>::into_raw in
        // tessera_handle (a zero-length box yields a dangling non-NULL pointer,
        // which is also reconstructed correctly here).
        let slice = std::ptr::slice_from_raw_parts_mut(response, response_len);
        drop(unsafe { Box::from_raw(slice) });
    }));
}
