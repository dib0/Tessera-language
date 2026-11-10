"""Independent check for M2: lump counts and the player 1 start of a map, with Python's struct."""
import struct
import sys

map_name = sys.argv[1].upper() if len(sys.argv) > 1 else "E1M1"
path = sys.argv[2] if len(sys.argv) > 2 else "WAD/freedoom1.wad"
data = open(path, "rb").read()
_, count, directory = struct.unpack_from("<4sii", data, 0)
lumps = []
for i in range(count):
    offset, size, raw = struct.unpack_from("<ii8s", data, directory + 16 * i)
    lumps.append((raw.split(b"\0")[0].decode("ascii").upper(), offset, size))
marker = max(i for i, l in enumerate(lumps) if l[0] == map_name)
parts = {}
for name, offset, size in lumps[marker + 1:marker + 12]:
    parts.setdefault(name, (offset, size))
sizes = {"THINGS": 10, "VERTEXES": 4, "LINEDEFS": 14, "SIDEDEFS": 30, "SECTORS": 26, "SEGS": 12, "SSECTORS": 4, "NODES": 28}
print("map", map_name)
for label, lump in [("things", "THINGS"), ("vertexes", "VERTEXES"), ("linedefs", "LINEDEFS"), ("sidedefs", "SIDEDEFS"), ("sectors", "SECTORS"), ("segs", "SEGS"), ("subsectors", "SSECTORS"), ("nodes", "NODES")]:
    print(label, parts[lump][1] // sizes[lump])
offset, size = parts["THINGS"]
start = None
for i in range(size // 10):
    x, y, angle, kind, flags = struct.unpack_from("<hhhHh", data, offset + 10 * i)
    if kind == 1:
        start = (x, y, angle)
        break
print("player 1 start %d %d angle %d" % start if start else "player 1 start none")
