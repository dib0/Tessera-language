#!/bin/sh
# Runs browser/test/test.html in headless Chrome against browser/tessera.wasm and fails on any FAIL.
# Skips (exit 0) when Chrome, python3 or tessera.wasm is missing.
set -eu
cd "$(dirname "$0")/../.."
chrome=""
for c in google-chrome chromium chromium-browser; do command -v "$c" >/dev/null 2>&1 && chrome=$c && break; done
if [ -z "$chrome" ] || ! command -v python3 >/dev/null 2>&1 || [ ! -f browser/tessera.wasm ]; then
    echo "skipped: needs Chrome, python3 and browser/tessera.wasm (bin/tessera1 build stage1/main.tes --wasm -o browser/tessera.wasm)"
    exit 0
fi
port=${TESSERA_TEST_PORT:-8765}
python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1 &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1
profile=$(mktemp -d)
out=$("$chrome" --headless=new --disable-gpu --no-sandbox --user-data-dir="$profile" --virtual-time-budget=120000 \
    --dump-dom "http://127.0.0.1:$port/browser/test/test.html" 2>/dev/null)
rm -rf "$profile"
result=$(printf '%s\n' "$out" | sed -n '/<pre id="result">/,/DONE/p' | sed 's/<pre id="result">//')
printf '%s\n' "$result"
printf '%s\n' "$result" | grep -q DONE || { echo "error: the test page did not finish"; exit 1; }
if printf '%s\n' "$result" | grep -q '^FAIL'; then exit 1; fi
echo "ok: browser tests passed"
