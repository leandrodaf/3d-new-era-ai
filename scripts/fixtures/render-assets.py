"""A valid demo bundle with a used OBJ/MTL/PNG and an unused large import."""
import io
import json
import struct
import sys
import zlib
import zipfile

with zipfile.ZipFile(sys.argv[1]) as source:
    project = json.loads(source.read("project.json"))
home = project["variants"][project["active"]]["home"]
home["name"] = "Asset snapshot fixture"
piece = next(f for f in home["furniture"] if f["catalog"] == "sofa-3")
piece["model"] = "models/sample.obj"


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"\x00\xff\x00\x00\xff"))
png += chunk(b"IEND", b"")
out = io.BytesIO()
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as bundle:
    bundle.writestr("project.json", json.dumps(project))
    bundle.writestr("models/sample.obj", "mtllib sample.mtl\nusemtl red\nv 0 0 0\nv 100 0 0\nv 0 100 100\nf 1 2 3\n")
    bundle.writestr("models/sample.mtl", "newmtl red\nKd 1 1 1\nmap_Kd wood grain.png\n")
    bundle.writestr("models/wood grain.png", png)
    bundle.writestr("unused-import.bin", bytes(16 * 1024 * 1024))
sys.stdout.buffer.write(out.getvalue())
