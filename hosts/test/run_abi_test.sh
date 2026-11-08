#!/bin/sh
# Builds runtime/adapter.rs as a cdylib together with an app.rs (default: the
# stub hosts/test/app.rs) and runs the C ABI test against it.
#
#   hosts/test/run_abi_test.sh [path/to/app.rs]
#
# The build mirrors what the compiler does for `--library`: copy app.rs and
# adapter.rs into one directory and run rustc on adapter.rs.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
app_rs=${1:-$here/app.rs}
work=$(mktemp -d "${TMPDIR:-/tmp}/tessera-abi.XXXXXX")
trap 'rm -rf "$work"' EXIT

cp "$app_rs" "$work/app.rs"
cp "$repo/runtime/adapter.rs" "$work/adapter.rs"
echo "== rustc (cdylib)"
rustc --edition 2021 --crate-type cdylib -C opt-level=2 -C panic=unwind \
    -o "$work/libapp.so" "$work/adapter.rs"
echo "== exported symbols"
nm -D --defined-only "$work/libapp.so" | grep ' tessera_' || true
echo "== gcc abi_test"
gcc -O2 -Wall -Wextra -Werror -I"$repo/hosts/include" -o "$work/abi_test" "$here/abi_test.c" -ldl -lpthread
echo "== run"
"$work/abi_test" "$work/libapp.so"
