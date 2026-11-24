#!/usr/bin/env python3
"""Export the SVG geometry from the pinned T3 Code v0.1.11 AppImage.

Usage: python3 scripts/export-reference-icons.py [resources/app.asar] [--check]
Uses only the local reference archive and FontTools 4.61.1; does not run its JS.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
from xml.sax.saxutils import quoteattr

from fontTools.pens.recordingPen import RecordingPen
from fontTools.svgLib.path import parse_path


ROOT = Path(__file__).resolve().parent.parent
ASAR = ROOT / "artifacts/reference/squashfs-root/resources/app.asar"
ASAR_SHA256 = "387ad9454269ccd5da65b3f3aa7cea0682d3f0710eec3fcd30b0e1a42a2540bd"
ASSET_DIR = ROOT / "assets/icons/lucide"
RUST = ROOT / "src/reference_icons_generated.rs"
T3_ARTWORK = {"snap-shot", "compose-send"}
SEND_PATH = "M7 11.5V2.5M7 2.5L3 6.5M7 2.5L11 6.5"

# Name is the library slug, not the text or tooltip of a particular screen.
NAMES = set("""
archive archive-restore arrow-down arrow-left arrow-right arrow-up bot calendar-clock
chart-no-axes-column check chevron-down chevron-right chevron-up circle-alert
circle-dot cloud corner-left-up external-link eye eye-off file-diff flask-conical folder
folder-git-2 folder-plus folder-tree git-branch git-pull-request-arrow globe
info key-round keyboard link list-filter lock lock-open log-in message-square-dashed
message-square-text messages-square mic monitor palette panel-left panel-left-close panels-top-left paperclip pencil plus
refresh-cw search server settings settings-2 shield-check shield-off
smartphone snap-shot square-menu square-pen terminal text-search trash-2 undo-2 wrench x
""".split())
ICONS = re.compile(r'\(`([a-z0-9][a-z0-9-]*)`,(\[\[`[a-z]+`,\{[^]]+\}\](?:,\[`[a-z]+`,\{[^]]+\}\])*)\]\)')
NODE = re.compile(r'\[`([a-z]+)`,\{([^}]+)\}\]')
ATTR = re.compile(r'([a-zA-Z0-9-]+):`([^`]+)`')


def read_icons(path):
    with path.open("rb") as archive:
        if hashlib.file_digest(archive, "sha256").hexdigest() != ASAR_SHA256:
            raise ValueError("not the pinned T3 Code v0.1.11 Linux AppImage app.asar")
        archive.seek(0)
        prefix, header, payload, length = struct.unpack("<4I", archive.read(16))
        if prefix != 4 or header != payload + 4 or length > payload - 4:
            raise ValueError("invalid ASAR Pickle header")
        files = json.loads(archive.read(length))
        for part in ("apps", "server", "dist", "client", "assets"):
            files = files["files"][part]
        icons = {}
        members = {}
        for member, entry in sorted(files["files"].items()):
            if not member.endswith(".js"):
                continue
            archive.seek(8 + header + int(entry["offset"]))
            blob = archive.read(entry["size"])
            if len(blob) != entry["size"]:
                raise ValueError(f"truncated {member}")
            bundled_js = blob.decode("utf-8")
            if SEND_PATH in bundled_js and "compose-send" not in icons:
                icons["compose-send"] = [("path", {"d": SEND_PATH})]
                members["compose-send"] = member
            for name, nodes in ICONS.findall(bundled_js):
                if name not in NAMES:
                    continue
                shapes = []
                for tag, attrs in NODE.findall(nodes):
                    pairs = ATTR.findall(attrs)
                    if ",".join(f"{k}:`{v}`" for k, v in pairs) != attrs:
                        raise ValueError(f"unsupported SVG attributes in {name}: {attrs}")
                    shapes.append((tag, {k: v for k, v in pairs if k != "key"}))
                if name in icons and shapes != icons[name]:
                    raise ValueError(f"conflicting definitions of {name}")
                if name not in icons:
                    icons[name] = shapes
                    members[name] = member
        if absent := (NAMES | T3_ARTWORK) - icons.keys():
            raise ValueError(f"missing pinned geometries: {sorted(absent)}")
        return icons, members


def svg(name, nodes):
    if name == "compose-send":
        return (
            '<!-- T3 Code v0.1.11 ComposerPrimaryActions.tsx; see LICENSES/T3-Code-MIT.txt. -->\n'
            '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14" fill="none">\n'
            f'  <path d="{SEND_PATH}" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>\n'
            '</svg>\n'
        )
    lines = [
        (
            '<!-- T3 Code v0.1.11 SettingsSidebarNav.tsx; see LICENSES/T3-Code-MIT.txt. -->'
            if name in T3_ARTWORK else
            f'<!-- Lucide React 0.564.0 {name}; see LICENSES/Lucide-ISC-and-Feather-MIT.txt. -->'
        ),
        '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"',
        '  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">',
    ]
    for tag, attrs in nodes:
        params = " ".join(f"{key}={quoteattr(value)}" for key, value in attrs.items())
        lines.append(f"  <{tag} {params}/>")
    return "\n".join(lines + ["</svg>", ""])


def as_path(tag, attrs, filled):
    a = {k: float(v) for k, v in attrs.items() if k not in {"d", "points"}}
    if tag == "path" and set(attrs) == {"d"}:
        return attrs["d"]
    if tag == "line" and set(attrs) == {"x1", "y1", "x2", "y2"}:
        return "M{x1} {y1}L{x2} {y2}".format(**a)
    if tag in ("polyline", "polygon") and set(attrs) == {"points"}:
        return "M" + attrs["points"] + ("Z" if tag == "polygon" else "")
    if tag in ("circle", "ellipse") and set(attrs) == (
        {"cx", "cy", "r"} if tag == "circle" else {"cx", "cy", "rx", "ry"}
    ):
        x, y = a["cx"], a["cy"]
        r, s = (a["r"], a["r"]) if tag == "circle" else (a["rx"], a["ry"])
        return f"M{x+r} {y}a{r} {s} 0 1 1 {-2*r} 0a{r} {s} 0 1 1 {2*r} 0Z"
    if tag == "rect" and {"x", "y", "width", "height"} <= set(attrs) <= {
        "x", "y", "width", "height", "rx", "ry"
    }:
        x, y, w, h = (a[k] for k in ("x", "y", "width", "height"))
        rx = min(a.get("rx", a.get("ry", 0)), w / 2)
        ry = min(a.get("ry", a.get("rx", 0)), h / 2)
        if not rx or not ry:
            return f"M{x} {y}h{w}v{h}h{-w}Z"
        arc = f"a{rx} {ry} 0 0 1"
        return (f"M{x+rx} {y}h{w-2*rx}{arc} {rx} {ry}v{h-2*ry}"
                f"{arc} {-rx} {ry}h{2*rx-w}{arc} {-rx} {-ry}v{2*ry-h}{arc} {rx} {-ry}Z")
    raise ValueError(f"unsupported SVG primitive {tag} {attrs} (filled={filled})")


def rust_geometry(nodes):
    outlines, fills = [], []
    letters = {"moveTo": "M", "lineTo": "L", "curveTo": "C", "closePath": "Z"}
    for tag, attrs in nodes:
        attrs = attrs.copy()
        fill = attrs.pop("fill", "none")
        if fill not in {"none", "currentColor"}:
            raise ValueError(f"unsupported icon fill {fill}")
        pen = RecordingPen()
        parse_path(as_path(tag, attrs, fill != "none"), pen)
        commands = []
        for operation, coordinates in pen.value:
            if operation == "endPath":
                continue
            if operation == "closePath":
                commands.append("Z")
                continue
            if operation not in letters or (operation == "curveTo" and len(coordinates) != 3):
                raise ValueError(f"unsupported {operation}: {coordinates}")
            nums = ", ".join(f"{float(n):.6f}" for point in coordinates for n in point)
            commands.append(f"{letters[operation]}({nums})")
        outlines += commands
        if fill == "currentColor":
            fills += commands
    return outlines, fills


def rust(icons):
    lines = [
        "// @generated by scripts/export-reference-icons.py; don't edit manually.",
        "use super::{",
        "    Glyph,",
        "    Segment::{C, L, M, Z},",
        "};",
        "",
        "#[rustfmt::skip]",
        "pub(super) fn glyph(name: &str) -> Option<&'static Glyph> {",
        "    Some(match name {",
    ]
    for name, nodes in sorted(icons.items()):
        outline, fill = rust_geometry(nodes)
        lines += [f'        "{name}" => {{', '            static GLYPH: Glyph = Glyph::new(']
        for commands in (outline, fill):
            lines += ["                &["] + [f"                    {c}," for c in commands] + ["                ],"]
        lines += ["            );", "            &GLYPH", "        }"]
    return "\n".join(lines + ["        _ => return None,", "    })", "}", ""])


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("asar", type=Path, nargs="?", default=ASAR)
    cli.add_argument("--check", action="store_true")
    args = cli.parse_args()
    icons, members = read_icons(args.asar)
    outputs = {
        (ASSET_DIR.parent / "t3" if name in T3_ARTWORK else ASSET_DIR)
        / f"{name}.svg": svg(name, nodes) for name, nodes in icons.items()
    }
    outputs[ASSET_DIR.parent / "sources.json"] = json.dumps(members, indent=2, sort_keys=True) + "\n"
    outputs[RUST] = rust(icons)
    if extra := set(ASSET_DIR.parent.glob("*/*.svg")) - outputs.keys():
        raise ValueError(f"unexpected stale exported assets: {sorted(extra)}")
    for path, value in outputs.items():
        expected = value.encode()
        if args.check:
            if path.read_bytes() != expected:
                raise ValueError(f"changed export: {path.relative_to(ROOT)}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(expected)
    print(f"Verified {len(icons)} pinned icon geometries{' (check only)' if args.check else ''}.")


if __name__ == "__main__":
    main()
