#!/bin/bash
# HTTP-level test of a server module (Apache mod_tessera or the nginx module)
# hosting the stub application hosts/test/app.rs.
#
#   hosts/test/http_test.sh BASE_URL MISSING_URL MAX_BODY_BYTES EXPECTED_GREETING
#
#   BASE_URL       location served by the stub app, e.g. http://127.0.0.1:18080/app
#   MISSING_URL    location configured with a library that does not exist (-> 500)
#   MAX_BODY_BYTES configured body limit (TesseraMaxBody / tessera_max_body)
#   EXPECTED_GREETING value of TESSERA_GREETING set via TesseraEnv / tessera_env
set -u
base=$1
missing=$2
max=$3
greeting=$4
host=$(echo "$base" | sed -E 's#^https?://([^/]+).*#\1#')
basepath=$(echo "$base" | sed -E 's#^https?://[^/]+##')
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
pass=0
fail=0

ok() { pass=$((pass + 1)); echo "PASS $1"; }
ko() { fail=$((fail + 1)); echo "FAIL $1"; [ -n "${2:-}" ] && echo "     $2"; }

# expect NAME STATUS FILE PATTERN...  (patterns are fixed strings in headers+body)
expect() {
    local name=$1 status=$2 file=$3
    shift 3
    local got
    got=$(grep -a '^HTTP/' "$file.h" | tail -1 | awk '{print $2}')
    if [ "$got" != "$status" ]; then
        ko "$name" "status $got, expected $status"
        return
    fi
    for pat in "$@"; do
        if ! grep -qiF -- "$pat" "$file.h" "$file.b"; then
            ko "$name" "missing: $pat"
            return
        fi
    done
    ok "$name"
}

req() { # FILE curl-args...
    local f=$1
    shift
    curl -s -o "$f.b" -D "$f.h" --max-time 30 "$@"
}

# 1. GET with raw path, query and custom header
req "$tmp/get" -H 'X-Custom: hello 1' "$base/hello%20world?a=1&b=two"
expect "GET with query, raw path, custom header, env" 201 "$tmp/get" \
    "x-tessera-test: yes" "content-type: text/plain" "method=GET" \
    "path=$basepath/hello%20world" "query=a=1&b=two" "x-custom=hello 1" \
    "remote=127.0.0.1" "greeting=$greeting" "body-length=0"

# 2. POST with a body
req "$tmp/post" -X POST --data-binary 'name=tessera&x=1' "$base/form"
expect "POST body" 201 "$tmp/post" "method=POST" "body-length=16" "body=name=tessera&x=1"

# 3. chunked POST
printf 'chunked-body-data' > "$tmp/chunk.in"
req "$tmp/chunk" -X POST -H 'Transfer-Encoding: chunked' --data-binary @"$tmp/chunk.in" "$base/chunked"
expect "chunked POST body" 201 "$tmp/chunk" "body-length=17" "body=chunked-body-data"

# 4. body just under the limit is accepted and echoed intact
head -c $((max - 1)) /dev/urandom > "$tmp/big.in"
req "$tmp/big" -X POST -H 'Content-Type: application/octet-stream' --data-binary @"$tmp/big.in" "$base/big"
expect "body of max-1 bytes" 201 "$tmp/big" "body-length=$((max - 1))"
if tail -c $((max - 1)) "$tmp/big.b" | cmp -s - "$tmp/big.in"; then ok "large body echoed byte-exact"; else ko "large body echoed byte-exact"; fi

# 5. body over the limit -> 413 (Content-Length and chunked)
head -c $((max + 1)) /dev/zero > "$tmp/huge.in"
req "$tmp/huge" -X POST --data-binary @"$tmp/huge.in" "$base/huge"
expect "body over limit (Content-Length) -> 413" 413 "$tmp/huge"
req "$tmp/hugec" -X POST -H 'Transfer-Encoding: chunked' --data-binary @"$tmp/huge.in" "$base/huge"
expect "body over limit (chunked) -> 413" 413 "$tmp/hugec"

# 6. status codes from the application
req "$tmp/nf" "$base/notfound"
expect "404 from application" 404 "$tmp/nf" "not found"
req "$tmp/err" "$base/fail"
expect "runtime error in application -> 500" 500 "$tmp/err"

# 7. HEAD: headers only
curl -s -I --max-time 30 "$base/head" > "$tmp/head.out"
if head -1 "$tmp/head.out" | grep -q ' 201' && grep -qi '^x-tessera-test: yes' "$tmp/head.out" \
    && ! grep -q 'method=' "$tmp/head.out"; then ok "HEAD (headers, no body)"; else ko "HEAD (headers, no body)"; fi

# 8. library that cannot be loaded -> 500
req "$tmp/miss" "$missing/x"
expect "unloadable library -> 500" 500 "$tmp/miss"

# 9. 200 concurrent requests, 20 at a time
seq 1 200 | xargs -P 20 -I{} curl -s --max-time 30 "$base/c?n={}" -o "$tmp/c.{}"
bad=0
for i in $(seq 1 200); do grep -qF "query=n=$i" "$tmp/c.$i" 2>/dev/null || bad=$((bad + 1)); done
if [ $bad -eq 0 ]; then ok "200 concurrent requests"; else ko "200 concurrent requests" "$bad wrong"; fi

echo "$pass passed, $fail failed"
[ $fail -eq 0 ]
