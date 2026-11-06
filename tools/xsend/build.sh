#!/bin/sh
# Builds the xsend test helper into tools/xsend/xsend.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
runtime="$here/../../runtime"
rustc_bin=${TESSERA_RUSTC:-rustc}
command -v "$rustc_bin" >/dev/null 2>&1 || rustc_bin="$HOME/.cargo/bin/rustc"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
{
    cat "$runtime/core.rs"
    printf '#[derive(Clone, PartialEq)]\nenum T_WindowEvent {\n    V_Click(i64, i64, i64),\n    V_Key(Rc<String>),\n    V_Close,\n}\n'
    cat "$runtime/ui.rs"
    cat "$here/main.rs"
} > "$tmp/xsend.rs"
"$rustc_bin" --edition 2021 -O -o "$here/xsend" "$tmp/xsend.rs"
