"""Independent check for M3: draws a map's linedefs with PIL, to compare with `tesseradoom ascii`."""
import struct
import sys
from PIL import Image, ImageDraw

map_name, out = sys.argv[1].upper(), sys.argv[2]
path = sys.argv[3] if len(sys.argv) > 3 else "WAD/freedoom1.wad"
data = open(path, "rb").read()
_, count, directory = struct.unpack_from("<4sii", data, 0)
lumps = [struct.unpack_from("<ii8s", data, directory + 16 * i) for i in range(count)]
lumps = [(raw.split(b"\0")[0].decode().upper(), o, s) for o, s, raw in lumps]
marker = max(i for i, l in enumerate(lumps) if l[0] == map_name)
parts = {name: (o, s) for name, o, s in lumps[marker + 1:marker + 12]}
o, s = parts["VERTEXES"]
verts = [struct.unpack_from("<hh", data, o + 4 * i) for i in range(s // 4)]
o, s = parts["LINEDEFS"]
lines = [struct.unpack_from("<HHhhhHH", data, o + 14 * i) for i in range(s // 14)]
xs, ys = [v[0] for v in verts], [v[1] for v in verts]
scale = 800 / (max(xs) - min(xs))
img = Image.new("RGB", (820, int((max(ys) - min(ys)) * scale) + 20), "black")
d = ImageDraw.Draw(img)
p = lambda v: (10 + (v[0] - min(xs)) * scale, 10 + (max(ys) - v[1]) * scale)
for a, b, flags, special, tag, front, back in lines:
    d.line([p(verts[a]), p(verts[b])], fill="red" if back == 0xFFFF else "yellow", width=1)
img.save(out)
