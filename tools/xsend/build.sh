#!/bin/sh
# Builds the xsend test helper into tools/xsend/xsend.
#
# Two window systems, one command line: on unix the helper is an X11 client
# built from runtime/core.rs + runtime/ui.rs + main.rs; on Windows it is the
# self-contained windows.rs, which posts the same events to a window handle.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
runtime="$here/../../runtime"
rustc_bin=${TESSERA_RUSTC:-rustc}
command -v "$rustc_bin" >/dev/null 2>&1 || rustc_bin="$HOME/.cargo/bin/rustc"

case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*)
        "$rustc_bin" --edition 2021 -O -o "$here/xsend.exe" "$here/windows.rs"
        exit 0
        ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
{
    cat "$runtime/core.rs"
    printf '#[derive(Clone, PartialEq)]\nenum T_WindowEvent {\n    V_Click(i64, i64, i64),\n    V_Key(Rc<String>),\n    V_KeyUp(Rc<String>),\n    V_Close,\n}\n'
    cat "$runtime/ui.rs"
    cat "$here/main.rs"
} > "$tmp/xsend.rs"
"$rustc_bin" --edition 2021 -O -o "$here/xsend" "$tmp/xsend.rs"
