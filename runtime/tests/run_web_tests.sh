#!/usr/bin/env bash
# Builds and runs the runtime/web.rs tests: core_stub.rs + web.rs + web_test.rs
# concatenated into one file that starts with #![forbid(unsafe_code)].
# Usage: runtime/tests/run_web_tests.sh [opt-level] [extra test-harness args...]
#
# Runs on Linux, macOS and on Windows in Git Bash, where the programs need the
# `.exe` extension and a path the shell's tools and a native program both read.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
opt="${1:-0}"
shift || true

exe=
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) exe=.exe ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
if [[ -n $exe ]]; then
    work="$(cygpath -m "$work")"
    # Git Bash rewrites an argument that looks like a unix path before a native
    # program sees it, so a request for `/fail` would arrive as
    # `C:/Program Files/Git/fail`. These two names turn that off.
    export MSYS2_ARG_CONV_EXCL='*'
    export MSYS_NO_PATHCONV=1
fi
cat "$here/core_stub.rs" "$here/../web.rs" "$here/web_test.rs" > "$work/web_tests.rs"
head -n1 "$work/web_tests.rs" | grep -qx '#!\[forbid(unsafe_code)\]'
rustc --edition 2021 --test -C opt-level="$opt" -o "$work/web_tests$exe" "$work/web_tests.rs"
"$work/web_tests$exe" "$@"

# End-to-end checks of tw_web_main through a real executable.
cat "$here/core_stub.rs" "$here/../web.rs" "$here/web_app_main.rs" > "$work/web_app.rs"
rustc --edition 2021 -C opt-level="$opt" -o "$work/web_app$exe" "$work/web_app.rs"
app="$work/web_app$exe"
fail() { echo "e2e FAILED: $*" >&2; exit 1; }
passed=0
ok() { passed=$((passed + 1)); }

out="$("$app" request POST '/p?x=1' -H 'X-A: b' --body hello)" || fail "request mode exit code"
[[ "$out" == $'HTTP/1.1 200 OK\ncontent-type: text/plain\n\nPOST /p x=1 hello' ]] || fail "request mode output: $out"; ok
out="$("$app" request GET /fail 2>"$work/err")" || fail "request /fail exit code"
[[ "$out" == HTTP/1.1\ 500\ * ]] || fail "request /fail output: $out"
grep -q 'runtime error R010: failed on purpose' "$work/err" || fail "request /fail log"; ok
set +e; "$app" bogus 2>/dev/null; rc=$?; "$app" 2>/dev/null; rc2=$?; set -e
[[ $rc == 2 && $rc2 == 2 ]] || fail "usage exit codes $rc $rc2"; ok
out="$(printf 'abcdef' | env GATEWAY_INTERFACE=CGI/1.1 REQUEST_METHOD=POST REQUEST_URI='/c?q=2' QUERY_STRING=q=2 CONTENT_LENGTH=3 "$app")"
[[ "$out" == $'Status: 200 OK\r\ncontent-type: text/plain\r\nContent-Length: 15\r\n\r\nPOST /c q=2 abc' ]] || fail "CGI output: $out"; ok
out="$(env GATEWAY_INTERFACE=CGI/1.1 REQUEST_METHOD=POST CONTENT_LENGTH=99999999999 "$app" </dev/null)"
[[ "$out" == Status:\ 413\ * ]] || fail "CGI 413: $out"; ok
# serve, and IIS-style start without arguments (HTTP_PLATFORM_PORT)
http_get() { # port path
  exec 3<>"/dev/tcp/127.0.0.1/$1" || return 1
  printf 'GET %s HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n' "$2" >&3
  cat <&3; exec 3<&-
}
for mode in serve iis; do
  for attempt in 1 2 3 4 5; do
    port=$((20000 + RANDOM % 20000))
    if [[ $mode == serve ]]; then "$app" serve --port "$port" 2>"$work/serve.err" & else env HTTP_PLATFORM_PORT="$port" "$app" 2>"$work/serve.err" & fi
    pid=$!
    for _ in $(seq 50); do grep -q "listening on http://127.0.0.1:$port" "$work/serve.err" 2>/dev/null && break; kill -0 $pid 2>/dev/null || break; sleep 0.1; done
    if grep -q "listening on" "$work/serve.err"; then break; fi
    wait $pid 2>/dev/null || true
  done
  out="$(http_get "$port" '/s?y=3')" || { kill $pid; fail "$mode: connect"; }
  kill $pid; wait $pid 2>/dev/null || true
  [[ "$out" == HTTP/1.1\ 200\ OK* && "$out" == *"GET /s y=3 " ]] || fail "$mode output: $out"; ok
done
echo "e2e: $passed passed"
