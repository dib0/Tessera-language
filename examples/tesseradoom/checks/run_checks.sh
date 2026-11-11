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

# Features beyond the roadmap (sound events, projectiles, teleporters, crushers, lights, switches, saving).
printf 'give\nweapon 5\nfire\nwait 80\n' | "$td" game E1M1 --no-monsters > "$build/f1"
grep -q "ammo 200/50/49/300 weapon 4.*missiles 1 .*sounds: DSRLAUNC" "$build/f1" && tail -1 "$build/f1" | grep -q "missiles 0"
check "rocket fired (sound, rocket in flight, one rocket used) and exploded on a wall" $?

(printf 'place 848 760 90\n'; for i in $(seq 1 40); do printf 'wait 5\n'; done) | "$td" game E1M1 > "$build/f2"
grep -q "missiles [1-9]" "$build/f2" && grep -q "DSFIRSHT" "$build/f2" && grep -q "DSFIRXPL" "$build/f2"
check "imps throw fireballs that fly and explode" $?

printf 'place -32 210 90\nforward 64\n' | "$td" game E4M6 --no-monsters | tail -1 | grep -q "at 1888 1808 angle 270"
check "teleporter (E4M6) moves the player to its destination" $?

printf 'place 100 32 180\nforward 50\nsector 639\nwait 20\nsector 639\nwait 40\nsector 639\n' | "$td" game E2M9 --no-monsters | grep "^sector 639" | awk '{print $6}' | tr '\n' ' ' | grep -q "^-14 -54 -42 $"
check "crusher (E2M9) moves its ceiling down and back up" $?

(for i in $(seq 1 16); do printf 'sector 32\nwait 5\n'; done) | "$td" game E1M1 --no-monsters | grep "^sector 32" | awk '{print $8}' | sort -u | wc -l | grep -q "^2$"
check "blinking light (E1M1 sector 32) changes between two levels" $?

printf 'place -360 1296 180\nuse\nswitch 615\n' | "$td" game E1M1 --no-monsters > "$build/f3"
grep -q "^switch 615 -1" "$build/f3" && grep -q "exited 1 next E1M2" "$build/f3"
check "exit switch changes texture for good and ends the level (next E1M2)" $?

printf 'give\nweapon 5\nfire\nwait 100\nwait 300\nsave %s\nwait 400\n' "$build/s1.sav" | "$td" game E1M2 | tail -1 | sed 's/ sounds:.*//' > "$build/a"
printf 'load %s\nsave %s\nwait 400\n' "$build/s1.sav" "$build/s2.sav" | "$td" game E1M2 | tail -1 | sed 's/ sounds:.*//' > "$build/b"
cmp -s "$build/s1.sav" "$build/s2.sav" && cmp -s "$build/a" "$build/b"
check "saved games load back identically and continue the same way (400 tics with monsters)" $?

exit $failed
