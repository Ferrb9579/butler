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

`welcome-fresh-light-1280x800.png` is the first comparable fresh Welcome
fixture. Both were captured in [run 45321949520][welcome-run] (stable T3 Code
v0.1.11; Butler `cf64dfd67d119ae00fe65bd8761f302525660cd2`). The raw diff is
60,133 of 1,024,000 pixels (5.87%); different text, missing icons, clipped
buttons, shifted spacing, and native window controls mean it is not parity.

[welcome-run]: https://github.com/Ferrb9579/butler/actions/runs/45321949520

## Providers after first-run setup (not yet at parity)

- [T3 Code](t3/providers-fresh-light-1280x800.png) · [Butler](butler/providers-fresh-light-1280x800.png)
- Both are unmodified 1280×800 native Linux X11 captures from a fresh profile
  after skipping first-run setup and opening Providers from the gated draft.
- [Native run 45360893581][providers-run]: T3 Code v0.1.11
  (`59651c1761f6c63e9b6056839067066968e6a406`), Butler
  `19ad912f07cfd6225060b41edfd81ee756822fa9`. This Butler capture includes
  Inter Variable, enable switches, the original provider marks, and the
  responsive editor rows. Name commit-on-blur landed afterward.
- Raw absolute error: 501,810 of 1,024,000 pixels differ (49.00%).
  Cursor is selected and disabled in both. T3 shows missing CLI diagnostics
  for Codex and Claude; Butler shows offline placeholders. Setup UI, font
  weights, row height in diagnostic states, and window chrome still differ.
- [Run 45359147124][providers-inter] included Inter and the switches before
  the marks and responsive editor. [Run 45354812510][providers-old] also
  predates Inter and the switches. Both older pairs remain in Git history.

[providers-run]: https://github.com/Ferrb9579/butler/actions/runs/45360893581
[providers-inter]: https://github.com/Ferrb9579/butler/actions/runs/45359147124
[providers-old]: https://github.com/Ferrb9579/butler/actions/runs/45354812510
