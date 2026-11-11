"""Independent check for M8: reads `tesseradoom game E1M1 --no-monsters < checks/m8_route.txt` and
verifies that items were picked up, that the lift lowered, that both doors on the route opened, and
that the player ended up behind the second door (sector 52)."""
import re
import sys

text = sys.stdin.read()
states = re.findall(r"^tic (\d+) at (-?\d+) (-?\d+) angle \d+ sector (\d+) health (\d+) armor \d+ ammo ([\d/]+) weapon \d+ keys \[[^\]]*\] items (\d+)", text, re.M)
sectors = re.findall(r"^sector (\d+) floor (-?\d+) ceiling (-?\d+)", text, re.M)
first, last = states[0], states[-1]
heights = {}
for number, floor, ceiling in sectors:
    heights.setdefault(int(number), []).append((int(floor), int(ceiling)))
checks = {
    "items picked up": int(last[6]) >= 2,
    "ammunition picked up": sum(map(int, last[5].split("/"))) > sum(map(int, first[5].split("/"))),
    "lift lowered": heights[98][-1][0] < 0,
    "door sector 10 was closed, then opened": heights[10][0][1] <= heights[10][0][0] and heights[10][-1][1] - heights[10][-1][0] >= 56,
    "door sector 48 was closed, then opened": heights[48][0][1] <= heights[48][0][0] and heights[48][-1][1] - heights[48][-1][0] >= 56,
    "player behind the second door": last[3] == "52",
}
for name, ok in checks.items():
    print(("ok   " if ok else "FAIL ") + name)
print("final state: tic %s at %s %s sector %s, health %s, ammo %s, items %s" % (last[0], last[1], last[2], last[3], last[4], last[5], last[6]))
sys.exit(0 if all(checks.values()) else 1)
