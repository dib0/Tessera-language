// Minimal web application used by run_web_tests.sh to test tw_web_main end to end.
fn e2e_app(req: RawRequest) -> RawResponse {
    if req.path == "/fail" { t_fail("R010", "failed on purpose".to_string(), "e2e.tess:1:1") }
    let body = format!("{} {} {} {}", req.method, req.path, req.query, String::from_utf8_lossy(&req.body));
    RawResponse { status: 200, headers: vec![("content-type".to_string(), "text/plain".to_string())], body: body.into_bytes() }
}
fn main() { tw_web_main(e2e_app) }
