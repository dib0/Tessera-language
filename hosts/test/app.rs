#![forbid(unsafe_code)]
#![allow(dead_code)]
// Test stub standing in for a compiler-generated `app.rs` (library build).
// It implements exactly the items runtime/adapter.rs needs:
//   RawRequest, RawResponse, tessera_app, tw_library_handle.
// The real implementation comes from the Tessera runtime (runtime/web.rs).
//
// Test application behaviour:
//   .../notfound   -> 404 text
//   .../sleep      -> sleeps 2 s, then answers like the default case
//   .../fail       -> panics (tw_library_handle must turn this into a 500)
//   anything else  -> 201, header x-tessera-test: yes, text/plain body echoing
//                     method, path, query, remote, x-custom header,
//                     env TESSERA_GREETING, body length and body bytes.

pub struct RawRequest {
    pub method: String,
    pub path: String,
    pub query: String,
    pub remote_addr: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn tessera_app(request: RawRequest) -> RawResponse {
    if request.path.ends_with("/notfound") {
        return RawResponse {
            status: 404,
            headers: vec![("content-type".into(), "text/plain; charset=utf-8".into())],
            body: b"not found\n".to_vec(),
        };
    }
    if request.path.ends_with("/sleep") {
        std::thread::sleep(std::time::Duration::from_millis(2000));
    }
    if request.path.ends_with("/fail") {
        panic!("deliberate application failure");
    }
    let custom = request
        .headers
        .iter()
        .find(|(n, _)| n == "x-custom")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let greeting = std::env::var("TESSERA_GREETING").unwrap_or_default();
    let mut body = format!(
        "method={}\npath={}\nquery={}\nremote={}\nx-custom={}\ngreeting={}\nbody-length={}\nbody=",
        request.method,
        request.path,
        request.query,
        request.remote_addr,
        custom,
        greeting,
        request.body.len()
    )
    .into_bytes();
    body.extend_from_slice(&request.body);
    RawResponse {
        status: 201,
        headers: vec![
            ("content-type".into(), "text/plain; charset=utf-8".into()),
            ("x-tessera-test".into(), "yes".into()),
        ],
        body,
    }
}

// ---- wire format -------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u32(&mut self) -> Option<u32> {
        let end = self.pos.checked_add(4)?;
        let b = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn bytes(&mut self) -> Option<Vec<u8>> {
        let len = self.u32()? as usize;
        let end = self.pos.checked_add(len)?;
        let b = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(b.to_vec())
    }
    fn str(&mut self) -> Option<String> {
        String::from_utf8(self.bytes()?).ok()
    }
}

fn decode_request(input: &[u8]) -> Option<RawRequest> {
    let mut r = Reader { data: input, pos: 0 };
    if r.u32()? != 1 {
        return None;
    }
    let method = r.str()?;
    let path = r.str()?;
    let query = r.str()?;
    let remote_addr = r.str()?;
    let count = r.u32()? as usize;
    let mut headers = Vec::new();
    for _ in 0..count {
        let name = r.str()?;
        let value = r.str()?;
        headers.push((name, value));
    }
    let body = r.bytes()?;
    if r.pos != input.len() {
        return None;
    }
    Some(RawRequest { method, path, query, remote_addr, headers, body })
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u32(out, b.len() as u32);
    out.extend_from_slice(b);
}

fn encode_response(resp: &RawResponse) -> Vec<u8> {
    let mut out = Vec::with_capacity(resp.body.len() + 64);
    put_u32(&mut out, 1);
    put_u32(&mut out, resp.status as u32);
    put_u32(&mut out, resp.headers.len() as u32);
    for (n, v) in &resp.headers {
        put_bytes(&mut out, n.as_bytes());
        put_bytes(&mut out, v.as_bytes());
    }
    put_bytes(&mut out, &resp.body);
    out
}

fn simple(status: u16, text: &str) -> RawResponse {
    RawResponse {
        status,
        headers: vec![("content-type".into(), "text/plain; charset=utf-8".into())],
        body: text.as_bytes().to_vec(),
    }
}

pub fn tw_library_handle(app: fn(RawRequest) -> RawResponse, input: &[u8]) -> Vec<u8> {
    let resp = match decode_request(input) {
        None => simple(400, "bad request\n"),
        Some(req) => {
            let worker = std::thread::Builder::new()
                .stack_size(8 << 20)
                .spawn(move || app(req));
            match worker {
                Err(_) => simple(500, "internal server error\n"),
                Ok(handle) => match handle.join() {
                    Ok(resp) => resp,
                    Err(_) => simple(500, "internal server error\n"),
                },
            }
        }
    };
    encode_response(&resp)
}
