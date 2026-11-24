# Pinned T3 Code icon exports

The SVG drawing commands come from the T3 Code v0.1.11 Linux AppImage
corresponding to source commit `59651c1761f6c63e9b6056839067066968e6a406`.
[sources.json](sources.json) records their exact client bundle members.
The extractor verifies the embedded `app.asar` SHA-256 before reading any
member and does not execute the bundle.

- `lucide/` contains needed Lucide React 0.564.0 UI icons, using their
  shipped paths and the original 24px view box, current color, and round
  stroke caps and joins. See [Lucide ISC and Feather MIT](../../LICENSES/Lucide-ISC-and-Feather-MIT.txt).
- `t3/` contains T3's own SnapShots frame and composer submit glyph.
  The latter is an original 14px glyph, not Lucide ArrowUp. See
  [T3 Code MIT](../../LICENSES/T3-Code-MIT.txt).

Run `python3 scripts/export-reference-icons.py --check` on the local pinned
AppImage extraction, using FontTools 4.61.1, to verify every SVG and the
Rust path module used by the desktop build. Omit `--check` to regenerate.
The Rust geometry uses deterministic Bézier conversion for SVG arcs;
it requires no SVG loader, third-party design system or network at runtime.
The raw exports preserve Lucide's nominal 2-unit stroke. Butler applies the
pinned T3 stylesheet's 1.75-unit display stroke to default Lucide icons,
including SnapShots; T3's standalone submit arrow keeps its own 1.8-unit
stroke in its 14px view box. A control with an explicit stroke in T3 should
retain that override when it is ported.
The assets alone do not implement the associated screens or behaviors.

Butler retains its own product identity. Vendor and OS marks are not part
of these exports, and the existing three provider identifiers stay
separate from this set. Extend the manifest from pinned source when a
new surface calls for another licensed glyph.

The separately sourced [ACP Registry mark](../brands/README.md) is not an
AppImage export. Its official brand permission, raw asset, and deterministic
native geometry are verified independently.
