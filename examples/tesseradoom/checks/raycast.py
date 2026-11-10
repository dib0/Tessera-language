"""Independent check for M4: a floating-point raycaster over the one-sided linedefs.
Prints per column the nearest line and the wall's top and bottom rows, like `tesseradoom columns`,
and compares with that output (read from standard input) when given `--compare`."""
import math
import struct
import sys

map_name = sys.argv[1].upper()
path = "WAD/freedoom1.wad"
data = open(path, "rb").read()
_, count, directory = struct.unpack_from("<4sii", data, 0)
lumps = [struct.unpack_from("<ii8s", data, directory + 16 * i) for i in range(count)]
lumps = [(raw.split(b"\0")[0].decode().upper(), o, s) for o, s, raw in lumps]
marker = max(i for i, l in enumerate(lumps) if l[0] == map_name)
parts = {name: (o, s) for name, o, s in lumps[marker + 1:marker + 12]}
def records(name, fmt, size):
    o, s = parts[name]
    return [struct.unpack_from(fmt, data, o + size * i) for i in range(s // size)]
verts = records("VERTEXES", "<hh", 4)
lines = records("LINEDEFS", "<HHhhhHH", 14)
sides = records("SIDEDEFS", "<hh8s8s8sH", 30)
sectors = records("SECTORS", "<hh8s8shhh", 26)
segs = records("SEGS", "<HHhHHh", 12)
ssecs = records("SSECTORS", "<HH", 4)
nodes = records("NODES", "<hhhh8h HH".replace(" ", ""), 28)
things = records("THINGS", "<hhhHh", 10)
px, py, deg = next((t[0], t[1], t[2]) for t in things if t[3] == 1)
child = len(nodes) - 1
while True:
    n = nodes[child]
    side = 1 if n[2] * (py - n[1]) - n[3] * (px - n[0]) > 0 else 0
    nxt = n[12] if side == 0 else n[13]
    if nxt & 0x8000:
        sub = ssecs[nxt & 0x7FFF]
        break
    child = nxt
seg = segs[sub[1]]
line = lines[seg[3]]
sector = sides[line[5] if seg[4] == 0 else line[6]][5]
eye = sectors[sector][0] + 41
angle = math.radians(deg)
fx, fy, rx, ry = math.cos(angle), math.sin(angle), math.sin(angle), -math.cos(angle)
result = []
for x in range(320):
    k = (x + 0.5 - 160) / 160
    dx, dy = fx + rx * k, fy + ry * k
    best = None
    for index, (a, b, flags, special, tag, front, back) in enumerate(lines):
        if back != 0xFFFF:
            continue
        (ax, ay), (bx, by) = verts[a], verts[b]
        lx, ly = bx - ax, by - ay
        den = dx * ly - dy * lx
        if den == 0:
            continue
        ex, ey = ax - px, ay - py
        t = (ex * ly - ey * lx) / den
        u = (ex * dy - ey * dx) / den
        if t > 0 and 0 <= u <= 1 and (best is None or t < best[0]):
            best = (t, index, sectors[sides[front][5]])
    if best is None:
        result.append((x, -1, 0, 0))
        continue
    t, index, sec = best
    clamp = lambda v: max(0, min(199, v))
    top = clamp(math.floor(100 - (sec[1] - eye) * 160 / t))
    bottom = clamp(math.floor(100 - (sec[0] - eye) * 160 / t))
    result.append((x, index, top, bottom))
if "--compare" in sys.argv:
    mine = [tuple(map(int, l.split())) for l in sys.stdin if not l.startswith("camera")]
    lines_differ = sum(1 for a, b in zip(mine, result) if a[1] != b[1])
    rows_off = [max(abs(a[2] - b[2]), abs(a[3] - b[3])) for a, b in zip(mine, result) if a[1] == b[1]]
    print("columns %d, different line %d, worst row difference %d, columns off by more than 1 row %d" % (
        len(result), lines_differ, max(rows_off), sum(1 for r in rows_off if r > 1)))
else:
    for r in result:
        print(*r)
