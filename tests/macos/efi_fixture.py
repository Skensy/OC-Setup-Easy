"""Create a FAT32 disk image and give its partition the GPT EFI System type."""
import struct
import subprocess
import sys
import uuid
import zlib
from pathlib import Path

image = Path(sys.argv[1])
subprocess.run(["hdiutil", "create", "-size", "96m", "-type", "UDTO", "-layout",
                "GPTSPUD", "-fs", "MS-DOS FAT32", "-volname", "OCFixture", image], check=True)
# hdiutil uses Basic Data for FAT32. Change both GPT copies in the regular file.
with image.open("r+b") as disk:
    for lba in (1, image.stat().st_size // 512 - 1):
        disk.seek(lba * 512)
        header = bytearray(disk.read(512))
        assert header[:8] == b"EFI PART"
        offset, count, size = struct.unpack_from("<QII", header, 72)
        disk.seek(offset * 512)
        entries = bytearray(disk.read(count * size))
        assert uuid.UUID(bytes_le=bytes(entries[:16])) == uuid.UUID("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7")
        entries[:16] = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b").bytes_le
        disk.seek(offset * 512)
        disk.write(entries)
        struct.pack_into("<I", header, 88, zlib.crc32(entries))
        struct.pack_into("<I", header, 16, 0)
        length, = struct.unpack_from("<I", header, 12)
        struct.pack_into("<I", header, 16, zlib.crc32(header[:length]))
        disk.seek(lba * 512)
        disk.write(header)
