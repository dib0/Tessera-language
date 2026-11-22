
// ===========================================================================
// The `page` effect (spec section 21.5): the web page of a browser host.
// Embedded when a program uses it. In WebAssembly every call is one `get`
// from the host with the key "page/" and a JSON array of the operation and
// its arguments (runtime/wasm.rs, browser/tessera.js); a list of elements
// comes back as their ids separated by NUL. Anywhere else there is no page,
// and every call is runtime error R022.
// ===========================================================================

fn t_page_call(op: &str, args: &[&str], loc: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = loc;
        let mut key = String::from("page/[");
        t_page_json(&mut key, op);
        for a in args {
            key.push(',');
            t_page_json(&mut key, a);
        }
        key.push(']');
        return crate::wasm_get(&key).map(|v| String::from_utf8_lossy(&v).into_owned());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = args;
        t_fail("R022", format!("`{}` needs a web page: run the program in a browser with browser/tessera.js", op), loc)
    }
}
#[cfg(target_arch = "wasm32")]
fn t_page_json(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
fn t_page_element(id: i64) -> Rc<T_PageElement> { Rc::new(T_PageElement { f_id: id }) }
fn t_page_elements(found: Option<String>) -> Rc<Vec<Rc<T_PageElement>>> {
    let text = found.unwrap_or_default();
    Rc::new(text.split('\0').filter(|part| !part.is_empty()).filter_map(|part| part.parse::<i64>().ok()).map(t_page_element).collect())
}
fn t_page_id(e: &Rc<T_PageElement>) -> String { e.f_id.to_string() }
fn t_page_select(selector: Rc<String>, loc: &str) -> Rc<Vec<Rc<T_PageElement>>> {
    t_page_elements(t_page_call("page_select", &[&selector], loc))
}
fn t_element_select(e: Rc<T_PageElement>, selector: Rc<String>, loc: &str) -> Rc<Vec<Rc<T_PageElement>>> {
    t_page_elements(t_page_call("element_select", &[&t_page_id(&e), &selector], loc))
}
fn t_element_text(e: Rc<T_PageElement>, loc: &str) -> Rc<String> {
    Rc::new(t_page_call("element_text", &[&t_page_id(&e)], loc).unwrap_or_default())
}
fn t_set_element_text(e: Rc<T_PageElement>, text: Rc<String>, loc: &str) {
    let _ = t_page_call("set_element_text", &[&t_page_id(&e), &text], loc);
}
fn t_element_attribute(e: Rc<T_PageElement>, name: Rc<String>, loc: &str) -> Option<Rc<String>> {
    t_page_call("element_attribute", &[&t_page_id(&e), &name], loc).map(Rc::new)
}
fn t_set_element_attribute(e: Rc<T_PageElement>, name: Rc<String>, value: Rc<String>, loc: &str) {
    let _ = t_page_call("set_element_attribute", &[&t_page_id(&e), &name, &value], loc);
}
fn t_remove_element_attribute(e: Rc<T_PageElement>, name: Rc<String>, loc: &str) {
    let _ = t_page_call("remove_element_attribute", &[&t_page_id(&e), &name], loc);
}
fn t_element_has_class(e: Rc<T_PageElement>, name: Rc<String>, loc: &str) -> bool {
    t_page_call("element_has_class", &[&t_page_id(&e), &name], loc).as_deref() == Some("true")
}
fn t_add_element_class(e: Rc<T_PageElement>, name: Rc<String>, loc: &str) {
    let _ = t_page_call("add_element_class", &[&t_page_id(&e), &name], loc);
}
fn t_remove_element_class(e: Rc<T_PageElement>, name: Rc<String>, loc: &str) {
    let _ = t_page_call("remove_element_class", &[&t_page_id(&e), &name], loc);
}
fn t_set_element_style(e: Rc<T_PageElement>, property: Rc<String>, value: Rc<String>, loc: &str) {
    let _ = t_page_call("set_element_style", &[&t_page_id(&e), &property, &value], loc);
}
fn t_create_element(tag: Rc<String>, loc: &str) -> Rc<T_PageElement> {
    t_page_element(t_page_call("create_element", &[&tag], loc).and_then(|id| id.parse::<i64>().ok()).unwrap_or(0))
}
fn t_append_element(parent: Rc<T_PageElement>, child: Rc<T_PageElement>, loc: &str) {
    let _ = t_page_call("append_element", &[&t_page_id(&parent), &t_page_id(&child)], loc);
}
fn t_insert_element_before(parent: Rc<T_PageElement>, child: Rc<T_PageElement>, before: Rc<T_PageElement>, loc: &str) {
    let _ = t_page_call("insert_element_before", &[&t_page_id(&parent), &t_page_id(&child), &t_page_id(&before)], loc);
}
// A compiled program has no names for its function values, which the host needs to call a handler
// back; handlers work in interpreted programs (`tessera.js` `run`), see spec section 21.5.
fn t_on_event(e: Rc<T_PageElement>, kind: Rc<String>, handler: fn(Rc<T_PageEvent>), loc: &str) {
    let _ = (e, kind, handler);
    t_fail("R022", "`on_event` needs the interpreter: run the program with browser/tessera.js".to_string(), loc)
}
fn t_on_timer(e: Rc<T_PageElement>, ms: i64, handler: fn(Rc<T_PageEvent>), loc: &str) {
    let _ = (e, ms, handler);
    t_fail("R022", "`on_timer` needs the interpreter: run the program with browser/tessera.js".to_string(), loc)
}
fn t_prevent_default(loc: &str) {
    let _ = t_page_call("prevent_default", &[], loc);
}
fn t_page_confirm(message: Rc<String>, loc: &str) -> bool {
    t_page_call("page_confirm", &[&message], loc).as_deref() == Some("true")
}
fn t_copy_to_clipboard(text: Rc<String>, loc: &str) {
    let _ = t_page_call("copy_to_clipboard", &[&text], loc);
}
fn t_media_matches(query: Rc<String>, loc: &str) -> bool {
    t_page_call("media_matches", &[&query], loc).as_deref() == Some("true")
}
fn t_stored_text(key: Rc<String>, loc: &str) -> Option<Rc<String>> {
    t_page_call("stored_text", &[&key], loc).map(Rc::new)
}
fn t_store_text(key: Rc<String>, value: Rc<String>, loc: &str) {
    let _ = t_page_call("store_text", &[&key, &value], loc);
}
