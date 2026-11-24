#!/usr/bin/env python3
"""Extract Butler's font from the local T3 Code v0.1.11 Linux AppImage.

Requires Python, FontTools 4.61.1 and either Brotli or brotlicffi. No network
or package installation is performed. With the default local extraction:
    python3 scripts/extract-reference-inter.py

An explicit path to the AppImage's extracted resources/app.asar is optional.
Pass --check to verify the two tracked outputs without changing them.
"""

import argparse
import hashlib
import io
import json
import struct
from pathlib import Path

from fontTools.ttLib import TTFont


BUTLER = Path(__file__).resolve().parents[1]
DEFAULT_ASAR = BUTLER / "artifacts/reference/squashfs-root/resources/app.asar"
ASAR_SHA256 = "387ad9454269ccd5da65b3f3aa7cea0682d3f0710eec3fcd30b0e1a42a2540bd"
CLIENT_PATH = "apps/server/dist/client"
FONT_MEMBER = f"{CLIENT_PATH}/assets/inter-latin-wght-normal-Dx4kXJAl.woff2"
FONT_SHA256 = "3100e775e8616cd2611beecfa23a4263d7037586789b43f035236a2e6fbd4c62"
TTF_PATH = BUTLER / "assets/Inter-Latin-Variable.ttf"
TTF_SHA256 = "03492ce7cbb5ca54283b27bc7fd32b91e7f086cdf127d471670ed1d782784300"
NOTICE_MEMBER = f"{CLIENT_PATH}/third-party-licenses.json"
NOTICE_PATH = BUTLER / "LICENSES/Inter-OFL-1.1.txt"
NOTICE_SHA256 = "bfe1e5d7dd50372dc9a580b1e31ce9bdaa5541a39b1ccf4874fdad2bfb92c89e"


def verify(data, expected, label):
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected:
        raise ValueError(f"{label}: SHA-256 {actual}, expected {expected}")


def read_inputs(path):
    with path.open("rb") as archive:
        if hashlib.file_digest(archive, "sha256").hexdigest() != ASAR_SHA256:
            raise ValueError("not the pinned T3 Code v0.1.11 Linux AppImage app.asar")
        archive.seek(0)
        # Electron ASAR begins with a Pickle holding the metadata Pickle size,
        # then the metadata Pickle stores a UTF-8 JSON string at byte 16.
        prefix_size, header_size, payload_size, json_size = struct.unpack(
            "<4I", archive.read(16)
        )
        if prefix_size != 4 or header_size != payload_size + 4 or json_size > payload_size - 4:
            raise ValueError("invalid ASAR metadata Pickle")
        files = json.loads(archive.read(json_size))
        data_offset = 8 + header_size

        def member(name):
            node = files
            for part in name.split("/"):
                node = node["files"][part]
            archive.seek(data_offset + int(node["offset"]))
            content = archive.read(node["size"])
            if len(content) != node["size"]:
                raise ValueError(f"truncated ASAR member: {name}")
            return content

        woff2 = member(FONT_MEMBER)
        verify(woff2, FONT_SHA256, FONT_MEMBER)
        entries = json.loads(member(NOTICE_MEMBER))["entries"]
        [entry] = [
            e
            for e in entries
            if e.get("name") == "@fontsource-variable/inter"
            and e.get("version") == "5.3.0"
            and e.get("license") == "OFL-1.1"
        ]
        notice = entry["noticeText"].encode("utf-8")
        verify(notice, NOTICE_SHA256, "Inter OFL notice")
        return woff2, notice


def convert(woff2):
    # Fontique reads SFNT, so remove only the WOFF2 container and retain the
    # packaged outlines, variation tables, timestamps and internal family.
    with TTFont(io.BytesIO(woff2), recalcTimestamp=False) as font:
        font.flavor = None
        output = io.BytesIO()
        font.save(output)
    ttf = output.getvalue()
    verify(ttf, TTF_SHA256, "Inter variable TTF")
    # Parse only after saving so FontTools doesn't rewrite untouched name/fvar tables.
    with TTFont(io.BytesIO(ttf)) as font:
        weight = [
            (a.axisTag, a.minValue, a.defaultValue, a.maxValue) for a in font["fvar"].axes
        ]
        if font["name"].getDebugName(1) != "Inter" or weight != [("wght", 100, 400, 900)]:
            raise ValueError("unexpected Inter family or weights")
    return ttf


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("asar", nargs="?", type=Path, default=DEFAULT_ASAR)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    woff2, notice = read_inputs(args.asar)
    for path, payload in ((TTF_PATH, convert(woff2)), (NOTICE_PATH, notice)):
        if args.check:
            verify(path.read_bytes(), hashlib.sha256(payload).hexdigest(), str(path))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
        print(f"{hashlib.sha256(payload).hexdigest()}  {path.relative_to(BUTLER)}")


if __name__ == "__main__":
    main()
