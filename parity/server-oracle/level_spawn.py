"""Print the world spawn (x y z) stored in a level.dat (gzipped NBT)."""

import gzip
import struct
import sys


def read_nbt(f):
    def rd(n):
        return f.read(n)

    def payload(t):
        if t == 1:
            return struct.unpack(">b", rd(1))[0]
        if t == 2:
            return struct.unpack(">h", rd(2))[0]
        if t == 3:
            return struct.unpack(">i", rd(4))[0]
        if t == 4:
            return struct.unpack(">q", rd(8))[0]
        if t == 5:
            return struct.unpack(">f", rd(4))[0]
        if t == 6:
            return struct.unpack(">d", rd(8))[0]
        if t == 7:
            return rd(struct.unpack(">i", rd(4))[0])
        if t == 8:
            return rd(struct.unpack(">H", rd(2))[0]).decode("utf8", "replace")
        if t == 9:
            element = rd(1)[0]
            return [payload(element) for _ in range(struct.unpack(">i", rd(4))[0])]
        if t == 10:
            out = {}
            while True:
                tag = rd(1)[0]
                if tag == 0:
                    return out
                name = rd(struct.unpack(">H", rd(2))[0]).decode("utf8", "replace")
                out[name] = payload(tag)
        if t == 11:
            n = struct.unpack(">i", rd(4))[0]
            return list(struct.unpack(">%di" % n, rd(4 * n)))
        if t == 12:
            n = struct.unpack(">i", rd(4))[0]
            return list(struct.unpack(">%dq" % n, rd(8 * n)))
        raise ValueError("unknown NBT tag %d" % t)

    tag = rd(1)[0]
    rd(struct.unpack(">H", rd(2))[0])
    return payload(tag)


data = read_nbt(gzip.open(sys.argv[1]))["Data"]
x, y, z = data["spawn"]["pos"]
print(x, y, z)
