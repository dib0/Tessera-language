"""Independent check for M6: reads `tesseradoom walk` output (positions as 16.16 fixed point) from
standard input and verifies with floating point that no position is closer than the player radius
to a wall (one-sided or "blocks players" line) and that no move between positions crossed one.
`python3 checks/walkcheck.py --commands N SEED [MAX]` instead prints N random movement commands of
at most MAX units (default 8: one collision step, so the straight line between two reported
positions is exactly the path taken)."""
import math
import random
import struct
import sys

if sys.argv[1] == "--commands":
    rng = random.Random(int(sys.argv[3]))
    longest = int(sys.argv[4]) if len(sys.argv) > 4 else 8
    for _ in range(int(sys.argv[2])):
        kind = rng.choice(["forward"] * 12 + ["back", "left", "right", "turn", "turn"])
        print(kind, rng.randint(-30, 30) if kind == "turn" else rng.randint(1, longest))
    sys.exit()

map_name = sys.argv[1].upper()
data = open("WAD/freedoom1.wad", "rb").read()
_, count, directory = struct.unpack_from("<4sii", data, 0)
lumps = [struct.unpack_from("<ii8s", data, directory + 16 * i) for i in range(count)]
lumps = [(raw.split(b"\0")[0].decode().upper(), o, s) for o, s, raw in lumps]
marker = max(i for i, l in enumerate(lumps) if l[0] == map_name)
parts = {name: (o, s) for name, o, s in lumps[marker + 1:marker + 12]}
o, s = parts["VERTEXES"]
verts = [struct.unpack_from("<hh", data, o + 4 * i) for i in range(s // 4)]
o, s = parts["LINEDEFS"]
lines = [struct.unpack_from("<HHHHHHH", data, o + 14 * i) for i in range(s // 14)]
walls = [(verts[a], verts[b]) for a, b, flags, _, _, front, back in lines if back == 0xFFFF or flags & 1]

def distance(p, a, b):
    lx, ly = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((p[0] - a[0]) * lx + (p[1] - a[1]) * ly) / (lx * lx + ly * ly or 1)))
    return math.hypot(p[0] - a[0] - lx * t, p[1] - a[1] - ly * t)

def crosses(p, q, a, b):
    def orient(u, v, w):
        return (v[0] - u[0]) * (w[1] - u[1]) - (v[1] - u[1]) * (w[0] - u[0])
    return orient(p, q, a) * orient(p, q, b) < 0 and orient(a, b, p) * orient(a, b, q) < 0

positions = [(int(l.split()[1]) / 65536, int(l.split()[2]) / 65536) for l in sys.stdin if l.startswith("at ")]
closest = min(distance(p, a, b) for p in positions for a, b in walls)
crossings = sum(1 for p, q in zip(positions, positions[1:]) for a, b in walls if crosses(p, q, a, b))
travelled = sum(math.hypot(q[0] - p[0], q[1] - p[1]) for p, q in zip(positions, positions[1:]))
distinct = len({(round(x), round(y)) for x, y in positions})
print("positions %d (%d distinct), travelled %.0f units, closest wall %.2f units, wall crossings %d" % (
    len(positions), distinct, travelled, closest, crossings))
sys.exit(0 if closest >= 15.99 and crossings == 0 else 1)
