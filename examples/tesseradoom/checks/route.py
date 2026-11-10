"""Plans a walking route for the M8 check: a breadth-first search over a 16-unit grid with the
engine's movement rules (radius 16, step up at most 24, at least 56 units of headroom), where the
sectors named with --open count as open doors. Prints `goto X Y` commands for `tesseradoom game`.
Usage: route.py MAP FROM_X FROM_Y TO_X TO_Y [--floor SECTOR HEIGHT] [--open SECTOR ...]"""
import collections
import math
import struct
import sys

args = sys.argv[1:]
floors = {}
while "--floor" in args:
    at = args.index("--floor")
    floors[int(args[at + 1])] = int(args[at + 2])
    args = args[:at] + args[at + 3:]
opened = set()
if "--open" in args:
    at = args.index("--open")
    opened = {int(a) for a in args[at + 1:]}
    args = args[:at]
map_name, fx, fy, tx, ty = args[0].upper(), *map(int, args[1:5])
data = open("WAD/freedoom1.wad", "rb").read()
_, count, directory = struct.unpack_from("<4sii", data, 0)
lumps = [struct.unpack_from("<ii8s", data, directory + 16 * i) for i in range(count)]
lumps = [(raw.split(b"\0")[0].decode().upper(), o, s) for o, s, raw in lumps]
marker = max(i for i, l in enumerate(lumps) if l[0] == map_name)
parts = {name: (o, s) for name, o, s in lumps[marker + 1:marker + 12]}
def records(name, fmt, size):
    o, s = parts[name]
    return [struct.unpack_from(fmt, data, o + size * i) for i in range(s // size)]
verts = records("VERTEXES", "<hh", 4)
lines = records("LINEDEFS", "<HHHHHHH", 14)
sides = records("SIDEDEFS", "<hh8s8s8sH", 30)
sectors = [list(s) for s in records("SECTORS", "<hh8s8shHH", 26)]
segs = records("SEGS", "<HHhHHh", 12)
ssecs = records("SSECTORS", "<HH", 4)
nodes = records("NODES", "<hhhh8hHH", 28)
for s in opened:
    sectors[s][1] = sectors[s][0] + 100
for s, height in floors.items():
    sectors[s][0] = height

def sector_at(x, y):
    child = len(nodes) - 1
    while True:
        n = nodes[child]
        side = 1 if n[2] * (y - n[1]) - n[3] * (x - n[0]) > 0 else 0
        nxt = n[12] if side == 0 else n[13]
        if nxt & 0x8000:
            seg = segs[ssecs[nxt & 0x7FFF][1]]
            line = lines[seg[3]]
            return sides[line[5] if seg[4] == 0 else line[6]][5]
        child = nxt

def blocks(line, floor):
    a, b, flags, special, tag, front, back = line
    if back == 0xFFFF or flags & 1:
        return True
    f, k = sectors[sides[front][5]], sectors[sides[back][5]]
    top, bottom = min(f[1], k[1]), max(f[0], k[0])
    return top - bottom < 56 or bottom - floor > 24 or top - max(floor, bottom) < 56

def touches(x, y, line, radius=17):
    (ax, ay), (bx, by) = verts[line[0]], verts[line[1]]
    lx, ly = bx - ax, by - ay
    t = max(0, min(1, ((x - ax) * lx + (y - ay) * ly) / (lx * lx + ly * ly or 1)))
    return math.hypot(x - ax - lx * t, y - ay - ly * t) < radius

def fits(x, y, floor):
    return not any(touches(x, y, l) and blocks(l, floor) for l in lines
                   if min(verts[l[0]][0], verts[l[1]][0]) - 17 < x < max(verts[l[0]][0], verts[l[1]][0]) + 17
                   and min(verts[l[0]][1], verts[l[1]][1]) - 17 < y < max(verts[l[0]][1], verts[l[1]][1]) + 17)

step = 16
start, goal = (fx, fy), (tx - (tx - fx) % step, ty - (ty - fy) % step)
prev = {start: None}
queue = collections.deque([start])
while queue:
    x, y = queue.popleft()
    if abs(x - tx) <= step and abs(y - ty) <= step:
        goal = (x, y)
        break
    floor = sectors[sector_at(x, y)][0]
    for dx, dy in ((step, 0), (-step, 0), (0, step), (0, -step)):
        n = (x + dx, y + dy)
        if n not in prev and fits(n[0], n[1], floor):
            prev[n] = (x, y)
            queue.append(n)
else:
    sys.exit("no route found")
path = []
p = goal
while p:
    path.append(p)
    p = prev[p]
path.reverse()
# Keep only the corners of the route.
corners = [path[0]] + [path[i] for i in range(1, len(path) - 1)
                       if (path[i][0] - path[i - 1][0], path[i][1] - path[i - 1][1]) != (path[i + 1][0] - path[i][0], path[i + 1][1] - path[i][1])] + [path[-1]]
for x, y in corners[1:]:
    print("goto", x, y)
