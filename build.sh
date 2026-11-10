#!/bin/sh
# Builds the whole toolchain:
#   compiler/target/release/tessera  stage 0 compiler (Rust)
#   bin/tessera-bootstrap            compiler built from the seed bootstrap/tessera1.rs
#   bin/tessera1                     stage 1 compiler (stage1/, built by the bootstrap compiler)
#   tools/xsend/xsend                test helper that sends clicks/keys to windows
set -eu
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

echo "== stage 0 (Rust)"
(cd compiler && cargo build --release --quiet)
tessera0=compiler/target/release/tessera

echo "== embedding runtime/ into stage1/runtime_text.tes"
$tessera0 run tools/embed_runtime.tes -- stage1/runtime_text.tes core_runtime=runtime/core.rs ui_runtime=runtime/ui.rs \
    extra_runtime=runtime/extra.rs web_runtime=runtime/web.rs adapter_runtime=runtime/adapter.rs \
    std_http=std/http.tes std_json=std/json.tes std_url=std/url.tes std_html=std/html.tes

echo "== bootstrap compiler (rustc on the generated seed bootstrap/tessera1.rs)"
mkdir -p bin
rustc_bin=${TESSERA_RUSTC:-rustc}
command -v "$rustc_bin" >/dev/null 2>&1 || rustc_bin="$HOME/.cargo/bin/rustc"
"$rustc_bin" --edition 2021 -C opt-level=2 --crate-name tessera_program -o bin/tessera-bootstrap bootstrap/tessera1.rs

echo "== stage 1 (stage1/, compiled by the bootstrap compiler)"
bin/tessera-bootstrap build stage1/main.tes -o bin/tessera1

echo "== self-hosting check: stage 1 rebuilds itself and must produce identical output"
bin/tessera1 build stage1/main.tes -o bin/tessera2
bin/tessera1 emit-rust stage1/main.tes -o bin/by-stage1.rs
bin/tessera2 emit-rust stage1/main.tes -o bin/by-stage2.rs
cmp bin/by-stage1.rs bin/by-stage2.rs
rm bin/tessera2 bin/tessera2.rs bin/by-stage1.rs bin/by-stage2.rs
echo "ok: fixpoint reached"

echo "== test helper"
tools/xsend/build.sh

echo "done. Try: bin/tessera1 build examples/tictactoe/tictactoe.tes -o bin/tictactoe && bin/tictactoe"
