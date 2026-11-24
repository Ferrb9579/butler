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

## About and Open source licenses (diagnostic pairs)

| Screen | T3 Code | Butler | Raw differing pixels |
| --- | --- | --- | --- |
| About | [PNG](t3/about-fresh-light-1280x800.png) | [PNG](butler/about-fresh-light-1280x800.png) | 578,903 of 1,024,000 |
| Open source licenses | [PNG](t3/open-source-licenses-fresh-light-1280x800.png) | [PNG](butler/open-source-licenses-fresh-light-1280x800.png) | 710,308 of 1,024,000 |

[Native run 45365107987][about-run], fresh Linux X11 profiles, light theme,
1280×800. T3 Code was v0.1.11
(`59651c1761f6c63e9b6056839067066968e6a406`), Butler was
`faf89de1e82ea383c244b5d43018b4bc02dba39a`. Both genuine apps show the same
routes; neither pair is a parity pass. Butler's manifest covers its three
bundled notices only; T3's inventory and scope/search controls differ.
Butler's About also lacks updater and mobile controls. The images predate
retaining the sidebar scroll position across Butler routes.

[about-run]: https://github.com/Ferrb9579/butler/actions/runs/45365107987

## New project form (diagnostic pairs)

| Fixture | T3 Code | Butler | Raw differing pixels |
| --- | --- | --- | --- |
| Empty name | [PNG](t3/new-project-empty-light-1280x800.png) | [PNG](butler/new-project-empty-light-1280x800.png) | 435,108 of 1,024,000 |
| Same typed title | [PNG](t3/new-project-named-light-1280x800.png) | [PNG](butler/new-project-named-light-1280x800.png) | 435,011 of 1,024,000 |

[Native run 45368061459][new-project-run], fresh Linux X11 profiles,
light theme and 1280×800. T3 Code was v0.1.11
(`59651c1761f6c63e9b6056839067066968e6a406`); Butler was
`fff3424d8cb67d3d2eb21a30ab5668d638e2529e`. The named form uses
`Native Created Project` in both apps. The reference form was not submitted;
the separate Butler workflow did create and reopen its temporary project.
The two data roots differ, as do the footer shortcuts, icons, focus-on-open,
spacing, and underlying window chrome. These are genuine route pairs,
not a pixel parity pass.

[new-project-run]: https://github.com/Ferrb9579/butler/actions/runs/45368061459

## Git URL entry (diagnostic pair)

- [T3 Code](t3/git-url-typed-light-1280x800.png) · [Butler](butler/git-url-typed-light-1280x800.png)
- [Native run 45374623866][git-url-run] captured both real apps after typing
  `ButlerFixture/CaseRepo` in Git URL on separate fresh Linux X11 profiles,
  at 1280×800 in the light theme. T3 Code was the pinned v0.1.11
  (`59651c1761f6c63e9b6056839067066968e6a406`); Butler was
  `cc9de3c3aa6c2aa1f2da926ea765dbb51f459828`.
- Raw differing pixels: 417,974 of 1,024,000. Footer controls, dimensions
  and window chrome still differ. This is not a pixel-parity pass.
- The fixture stopped before Clone and never contacted a Git remote. The
  destination frames in this run were diagnostics; only the typed pair
  passed automated verification in both apps.

[git-url-run]: https://github.com/Ferrb9579/butler/actions/runs/45374623866

## Git URL destination (diagnostic pair)

[T3 Code](t3/git-url-destination-light-1280x800.png) · [Butler](butler/git-url-destination-light-1280x800.png)

[Native run 45376452356](https://github.com/Ferrb9579/butler/actions/runs/45376452356)
verified both original 1280×800 light-mode destination screens, T3 Code v0.1.11
(`59651c1761f6c63e9b6056839067066968e6a406`) and Butler
`461e1d99e341d6ea9a4f93393c54cd6d054ae22c`. Both show the same synthetic Git
URL and `~/CaseRepo` preview. Continue was the last action; no Clone,
checkout, or remote request was tested. Layout, footer, and window chrome
still differ.
