# Screenshot comparisons

Use the same PNG filename in both directories for the same screen, fixture,
theme, and viewport. For example:

| T3 Code | Butler |
| --- | --- |
| `t3/new-thread-empty-dark-1280x800.png` | `butler/new-thread-empty-dark-1280x800.png` |

Capture pairs with `scripts/compare-ui.sh`. Commit both PNGs together. Pixel
diffs and counts go under `artifacts/ui`. Use fixtures without personal data or
credentials. This directory contains a pair only after both captures succeed.

## Diagnostic baseline (not a parity fixture)

`native-startup-baseline-1280x800.png` captures each app's unmodified
startup on the same Linux X11 runner. T3 Code v0.1.11
(`59651c1761f6c63e9b6056839067066968e6a406`) shows first-run Welcome;
Butler `6a406e01f3e497514300d17eded2849e69ea5678` shows its early draft
shell. The machine name in the reference is from the disposable runner.
The images differ across the full frame; do not use them as a pass/fail
snapshot for a matched screen. [Original run and diff][startup-run].

[startup-run]: https://github.com/Ferrb9579/butler/actions/runs/45318341970
