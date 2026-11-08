#!/bin/bash
# End-to-end test of the Apache and nginx modules, as a normal user (no root).
#
#   hosts/test/run_server_tests.sh [WORKDIR] [APP_RS]
#
# WORKDIR defaults to /tmp/tessera-hosts-test. APP_RS defaults to the stub
# hosts/test/app.rs. Needs: rustc, gcc, make, curl, apt-get + dpkg (to fetch
# Apache/APR headers without root), an installed apache2 binary with its
# modules in /usr/lib/apache2/modules (Debian/Ubuntu layout), internet access
# to nginx.org. Ports 18080 (Apache) and 18081 (nginx) must be free.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
work=${1:-/tmp/tessera-hosts-test}
app_rs=${2:-$here/app.rs}
nginx_version=${NGINX_VERSION:-1.30.6}
apache_bin=${APACHE_BIN:-/usr/sbin/apache2}
apache_modules=${APACHE_MODULES:-/usr/lib/apache2/modules}
results=()
status=0

step() { echo; echo "=== $*"; }
record() { results+=("$1: $2"); [ "$2" = PASS ] || status=1; }

mkdir -p "$work"
cd "$work" || exit 2

step "build application library (adapter.rs + $(basename "$app_rs"))"
mkdir -p app
cp "$app_rs" app/app.rs
cp "$repo/runtime/adapter.rs" app/adapter.rs
rustc --edition 2021 --crate-type cdylib -C opt-level=2 -C panic=unwind -o app/libapp.so.new app/adapter.rs || exit 1
mv app/libapp.so.new app/libapp.so   # rename, never overwrite a mapped .so in place

# ---------------------------------------------------------------- Apache
step "Apache: fetch headers (apt-get download, no root)"
if [ ! -d apache-dev/usr/include/apache2 ]; then
    mkdir -p debs && (cd debs && apt-get download apache2-dev libapr1-dev libaprutil1-dev) || exit 1
    for d in debs/*.deb; do dpkg -x "$d" apache-dev; done
fi
inc=$work/apache-dev/usr/include

step "Apache: compile mod_tessera.so"
if gcc -shared -fPIC -O2 -Wall -Wextra -Werror -D_GNU_SOURCE -D_LARGEFILE64_SOURCE \
    -I"$repo/hosts/include" -I"$inc/apache2" -I"$inc/apr-1.0" \
    -o mod_tessera.so "$repo/hosts/apache/mod_tessera.c" -ldl; then
    record "apache module compiles (-Wall -Wextra -Werror)" PASS
else
    record "apache module compiles" FAIL
fi

run_apache() { # MPM
    local mpm=$1 dir=$work/apache-$1
    mkdir -p "$dir"
    cat > "$dir/httpd.conf" <<EOF
ServerRoot $dir
ServerName localhost
Listen 127.0.0.1:18080
PidFile $dir/httpd.pid
ErrorLog $dir/error.log
LogLevel warn tessera:info
Mutex file:$dir default
DefaultRuntimeDir $dir
LoadModule mpm_${mpm}_module $apache_modules/mod_mpm_${mpm}.so
LoadModule authz_core_module $apache_modules/mod_authz_core.so
LoadModule tessera_module $work/mod_tessera.so
TesseraEnv TESSERA_GREETING hello-from-apache
<Location /app>
    SetHandler tessera
    TesseraApplication $work/app/libapp.so
    TesseraMaxBody 1048576
    Require all granted
</Location>
<Location /missing>
    SetHandler tessera
    TesseraApplication $work/app/does-not-exist.so
    Require all granted
</Location>
EOF
    step "Apache ($mpm MPM): start and run HTTP tests"
    "$apache_bin" -f "$dir/httpd.conf" -k start || { record "apache $mpm start" FAIL; return; }
    sleep 1
    if "$here/http_test.sh" http://127.0.0.1:18080/app http://127.0.0.1:18080/missing 1048576 hello-from-apache; then
        record "apache $mpm HTTP tests" PASS
    else
        record "apache $mpm HTTP tests" FAIL
    fi
    "$apache_bin" -f "$dir/httpd.conf" -k stop
    sleep 1
    if grep -qi "exit signal\|segmentation" "$dir/error.log"; then record "apache $mpm no crashes in error.log" FAIL; else record "apache $mpm no crashes in error.log" PASS; fi
}
if [ -f mod_tessera.so ] && [ -x "$apache_bin" ]; then
    run_apache event
    run_apache prefork
fi

# ---------------------------------------------------------------- nginx
step "nginx: fetch nginx-$nginx_version source"
if [ ! -f "nginx-$nginx_version.tar.gz" ]; then
    curl -fsSO "https://nginx.org/download/nginx-$nginx_version.tar.gz" || exit 1
fi

build_nginx() { # NAME extra-configure-args...
    local name=$1
    shift
    rm -rf "src-$name"
    mkdir "src-$name" && tar xzf "nginx-$nginx_version.tar.gz" -C "src-$name" --strip-components=1
    (cd "src-$name" && ./configure --prefix="$work/$name" --without-http_rewrite_module \
        --without-http_gzip_module --with-cc-opt=-O2 --add-module="$repo/hosts/nginx" "$@" \
        > "$work/$name-configure.log" 2>&1 && make -j"$(nproc)" > "$work/$name-make.log" 2>&1 && make install > /dev/null 2>&1)
}

run_nginx() { # NAME THREADS(yes/no)
    local name=$1 threads=$2 dir=$work/$1-run pool="" tpool=""
    mkdir -p "$dir"
    if [ "$threads" = yes ]; then pool="thread_pool tessera threads=8;"; tpool="tessera_thread_pool tessera;"; fi
    cat > "$dir/nginx.conf" <<EOF
worker_processes 1;
error_log $dir/error.log info;
pid $dir/nginx.pid;
$pool
events { worker_connections 256; }
http {
    access_log off;
    client_body_temp_path $dir/client_body;
    proxy_temp_path $dir/proxy;
    fastcgi_temp_path $dir/fastcgi;
    uwsgi_temp_path $dir/uwsgi;
    scgi_temp_path $dir/scgi;
    tessera_env TESSERA_GREETING hello-from-nginx;
    server {
        listen 127.0.0.1:18081;
        location /app/ {
            tessera $work/app/libapp.so;
            tessera_max_body 1m;
            client_max_body_size 1m;
            $tpool
        }
        location /missing/ {
            tessera $work/app/does-not-exist.so;
        }
    }
}
EOF
    step "nginx ($name): start and run HTTP tests"
    "$work/$name/sbin/nginx" -c "$dir/nginx.conf" || { record "$name start" FAIL; return; }
    sleep 1
    if "$here/http_test.sh" http://127.0.0.1:18081/app http://127.0.0.1:18081/missing 1048576 hello-from-nginx; then
        record "$name HTTP tests" PASS
    else
        record "$name HTTP tests" FAIL
    fi
    "$work/$name/sbin/nginx" -c "$dir/nginx.conf" -s stop
    sleep 1
    if grep -qi "exited on signal" "$dir/error.log"; then record "$name no crashed workers" FAIL; else record "$name no crashed workers" PASS; fi
}

step "nginx: build with --with-threads"
if build_nginx nginx-threads --with-threads; then
    record "nginx-threads builds (nginx uses -Werror)" PASS
    run_nginx nginx-threads yes
else
    record "nginx-threads builds" FAIL
fi

step "nginx: build without threads"
if build_nginx nginx-nothreads; then
    record "nginx-nothreads builds (nginx uses -Werror)" PASS
    run_nginx nginx-nothreads no
else
    record "nginx-nothreads builds" FAIL
fi

echo
echo "=== summary"
printf '%s\n' "${results[@]}"
exit $status
