#!/bin/sh
# Runs the independent milestone checks (M1 to M9) against WAD/freedoom1.wad.
# Needs python3 and Pillow (for the M3 picture). Skips everything when the WAD is missing.
set -u
cd "$(dirname "$0")/.."
if [ ! -f WAD/freedoom1.wad ]; then
    echo "skipped: WAD/freedoom1.wad is missing (see README.md)"
    exit 0
fi
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
td="$build/tesseradoom"
../../bin/tessera1 build main.tes -o "$td" --opt 3 >/dev/null || { echo "FAIL build"; exit 1; }
failed=0
check() {
    if [ "$2" -eq 0 ]; then echo "ok   $1"; else echo "FAIL $1"; failed=1; fi
}

"$td" info > "$build/m1" && python3 -I checks/wadinfo.py > "$build/m1ref" && cmp -s "$build/m1" "$build/m1ref"
check "M1 WAD type, lump count and map names match Python" $?

ok=0
for map in E1M1 E2M5 E3M7 E4M9; do
    "$td" map $map > "$build/m2" && python3 -I checks/mapdump.py $map > "$build/m2ref" && cmp -s "$build/m2" "$build/m2ref" || ok=1
done
check "M2 map counts and player start match Python (E1M1, E2M5, E3M7, E4M9)" $ok

"$td" ascii E1M1 100 > "$build/m3" && [ "$(grep -c '#' "$build/m3")" -gt 30 ] && grep -q P "$build/m3"
check "M3 ASCII map drawn (compare with: python3 checks/mapimage.py E1M1 map.png)" $?

ok=0
for map in E1M1 E1M2 E2M3 E3M1; do
    "$td" columns $map | python3 -I checks/raycast.py $map --compare > "$build/m4" || ok=1
    grep -q "different line 0, worst row difference [01], columns off by more than 1 row 0" "$build/m4" || ok=1
done
check "M4 raycast columns match a floating-point raycaster within one row" $ok

"$td" render E1M1 "$build/m5.ppm" > /dev/null && [ "$(head -c 2 "$build/m5.ppm")" = "P6" ]
check "M5 BSP frame written as PPM (visual check; no reference engine available)" $?

ok=0
for seed in 1 2; do
    python3 -I checks/walkcheck.py --commands 5000 $seed | "$td" walk E1M1 | python3 -I checks/walkcheck.py E1M1 > /dev/null || ok=1
done
check "M6 10000 random steps never closer than 16 units to a wall or through one" $ok

fps=$("$td" bench E1M1 200 --still | sed 's/.*: \([0-9]*\) frames per second.*/\1/')
[ "$fps" -ge 35 ]
check "M7/D3 rendering at least 35 frames per second (measured $fps)" $?

"$td" game E1M1 --no-monsters < checks/m8_route.txt | python3 -I checks/m8check.py > "$build/m8"
check "M8 items, lift and two doors on a route through E1M1" $?

ok=0
for demo in DEMO1 DEMO4; do
    map=$("$td" demo $demo 0 | head -1 | awk '{print $4}')
    "$td" demo $demo --no-monsters | python3 -I checks/walkcheck.py $map > /dev/null || ok=1
done
check "M9 demo playback (without monsters) stays clear of walls; parity not checked (no reference)" $ok

exit $failed
