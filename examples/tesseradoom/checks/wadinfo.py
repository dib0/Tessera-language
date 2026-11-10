"""Independent check for M1: WAD type, lump count and map names, computed with Python's struct."""
import struct
import sys

path = sys.argv[1] if len(sys.argv) > 1 else "WAD/freedoom1.wad"
data = open(path, "rb").read()
kind, count, directory = struct.unpack_from("<4sii", data, 0)
names = []
for i in range(count):
    _, _, raw = struct.unpack_from("<ii8s", data, directory + 16 * i)
    names.append(raw.split(b"\0")[0].decode("ascii").upper())
maps = [names[i] for i in range(count - 1) if names[i + 1] == "THINGS"]
print("type", kind.decode())
print("lumps", count)
print("maps %d: %s" % (len(maps), " ".join(maps)))
