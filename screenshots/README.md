# Screenshot comparisons

See the [published screenshot audit](AUDIT.md) for visible icon, layout,
and fixture differences, with historical captures distinguished from current
diagnostics.

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

## Refreshed Welcome and New Project (native audit follow-up)

| State | T3 Code | Butler | Raw differing pixels |
| --- | --- | --- | --- |
| Welcome, agent setup expanded | [PNG](t3/welcome-expanded-refresh-light-1280x800.png) | [PNG](butler/welcome-expanded-refresh-light-1280x800.png) | 52,885 of 1,024,000 |
| New Project, empty | [PNG](t3/new-project-empty-refresh-light-1280x800.png) | [PNG](butler/new-project-empty-refresh-light-1280x800.png) | 436,288 of 1,024,000 |
| New Project, typed name | [PNG](t3/new-project-named-refresh-light-1280x800.png) | [PNG](butler/new-project-named-refresh-light-1280x800.png) | 436,202 of 1,024,000 |

Unchanged 1280×800 X11 originals from
[native run 45446456654](https://github.com/Ferrb9579/butler/actions/runs/45446456654)
and [artifact 14947110100](https://github.com/Ferrb9579/butler/actions/runs/45446456654/artifacts/14947110100),
with T3 Code v0.1.11 `59651c1761f6c63e9b6056839067066968e6a406` and Butler
`a77b8cd309b6420c21abdab3fe84b66a26854d26`. The older pairs are retained above
and below; raw counts are not pixel-parity verdicts.

- Welcome shows the 10×10 leading Plus, far-right up-chevron at the same x
  position, and original ACP Registry mark at 16×6 drawn pixels in both apps.
  Butler's Plus is 2 px right/down and the registry mark is 2 px right and
  8 px lower. Seven other provider logos, control states and chrome still
  differ. No agent was enabled.
- New Project now shows the source Back, folder-create, folder-plus and
  trailing chevron alongside Enter, Backspace and Escape hints. Up/Down
  navigation and initial focus still differ; Butler's footer starts 10 px
  higher. Both use the typed title `Native Created Project`; the local
  temporary project roots differ, and Butler truncates the expanded path.
  The reference form was not submitted.

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

## Saved transcript before Find (layout comparison)

- [T3 Code](t3/saved-transcript-light-1280x800.png) · [Butler](butler/saved-transcript-light-1280x800.png)
- Original 1280×800 Linux X11 images from [native run 45411570052](https://github.com/Ferrb9579/butler/actions/runs/45411570052), T3 Code v0.1.11 (`59651c1761f6c63e9b6056839067066968e6a406`) and Butler `c621b6686bc098c82e522caf1259ebffc3876fbf`. Both show the same two synthetic saved messages in a selected thread, before Find opens. T3's provider status alert was dismissed only in that test session.
- Raw absolute error: 277,075 of 1,024,000 pixels differ (27.06%). T3 still has a details panel, a saved-thread composer and a richer sidebar; Butler's user bubble extends farther right. The sidebar fixture titles differ. This is a transcript layout comparison, **not a verified Find pair or a visual parity pass**.

## Saved thread Find, first match (behavior comparison)

- [T3 Code](t3/saved-thread-find-first-match-light-1280x800.png) · [Butler](butler/saved-thread-find-first-match-light-1280x800.png)
- Original, unchanged 1280×800 X11 frames from [native run 45414278088](https://github.com/Ferrb9579/butler/actions/runs/45414278088), T3 Code v0.1.11 (`59651c1761f6c63e9b6056839067066968e6a406`) and Butler `82c42b4411f8f4800928cfddd9e7b72c1bda6415`. Both have the same two fixture messages with `release` and count `1/2` visibly present. The original run's T3 verifier misread the highlighted text. Independent inspection and the corrected strict OCR gate verified the unchanged frames after the run.
- 275,416 of 1,024,000 pixels differ (26.90%). T3 highlights both hits and retains its details panel and composer. Butler still lacks that inline background and those saved-thread surfaces. The T3 Next and no-match frames were not captured in this run and aren't represented here.

## Saved thread Find, Next and no match (behavior comparison)

| State | T3 Code | Butler | Raw differing pixels |
| --- | --- | --- | --- |
| Next: `release`, `2/2` | [PNG](t3/saved-thread-find-next-light-1280x800.png) | [PNG](butler/saved-thread-find-next-light-1280x800.png) | 274,874 of 1,024,000 |
| No match: `absent`, `0/0` | [PNG](t3/saved-thread-find-no-match-light-1280x800.png) | [PNG](butler/saved-thread-find-no-match-light-1280x800.png) | 274,508 of 1,024,000 |

[Native run 45416871021](https://github.com/Ferrb9579/butler/actions/runs/45416871021)
verified all three Find interaction states in both apps with two identical
synthetic saved messages. These are the original 1280×800 X11 frames for the
newly verified states; T3 Code was v0.1.11
(`59651c1761f6c63e9b6056839067066968e6a406`) and Butler was
`fef5acc4ecffa496d56315f8a5341ac387c8b7a7`. The workflow typed into the Find
field and clicked Next without submitting any message. This verifies the
interaction and counts in this fixture, not visual parity.

## Saved-thread composer and details (layout comparison)

- [T3 Code](t3/saved-thread-composer-light-1280x800.png) · [Butler](butler/saved-thread-composer-light-1280x800.png)
- Original 1280×800 X11 frames from [native run 45421205351](https://github.com/Ferrb9579/butler/actions/runs/45421205351), T3 Code `59651c1761f6c63e9b6056839067066968e6a406`, Butler `7129b926f41e53ac64f3a6d8daa4a46d0a381844`. Both show the same saved transcript with Find closed, empty composer, and visible workspace details. T3 retained its default provider selection with the missing-provider alert dismissed. No message was submitted.
- The dock spans the same x304–955 and ends at y779 in both; Butler's top is 3 px higher. Butler shows the unavailable-provider prompt, a disabled Send and one real details row. Provider, microphone, attachment and other details controls are not yet mirrored. 265,425 of 1,024,000 raw pixels differ. This is a layout comparison, not visual parity or proof of native text entry.

## Compact saved-thread details (native behavior and layout)

- [T3 Code](t3/saved-thread-compact-details-open-light-1000x800.png) · [Butler](butler/saved-thread-compact-details-open-light-1000x800.png)
- Original 1000×800 X11 frames from [native run 45426627050](https://github.com/Ferrb9579/butler/actions/runs/45426627050), pinned T3 Code `59651c1761f6c63e9b6056839067066968e6a406` and Butler `e7a41e2d075d21cc5fccd57c3cb22009d3725e39`. Both start closed, open through their ordinary controls, and close with Escape. The popup area returns to its exact pre-open pixels in each app. No project or provider action was used.
- The opened card spans x708–987 from y48 in both. Butler has one workspace row; T3 has additional editor, project-script and Git rows. 268,151 of 800,000 raw pixels differ with the cards open. The closed and dismissed images remain in the original artifact, not duplicate files in the repository.

## Saved-thread Git branch in wide Details (native behavior and layout)

- [T3 Code](t3/saved-thread-branch-details-light-1280x800.png) · [Butler](butler/saved-thread-branch-details-light-1280x800.png)
- Original 1280×800 X11 frames from [native run 45433209304](https://github.com/Ferrb9579/butler/actions/runs/45433209304), [artifact 14939738090](https://github.com/Ferrb9579/butler/actions/runs/45433209304/artifacts/14939738090). T3 Code v0.1.11 was pinned to `59651c1761f6c63e9b6056839067066968e6a406`; Butler was `aa8ca966c86fc4cef0af03748f8e1e3f6e17c09e`.
- Both use the same disposable local checkout and selected synthetic transcript. The actual branch `copperleaf` appears in each inline Details card and is absent in the non-Git controls. No remote, provider, branch action, or clipboard interaction was used.
- 271,013 of 1,024,000 raw pixels differ. This matches the visible branch state, not the rest of the card or pixel parity.

## Saved assistant Markdown paragraphs (native behavior and layout)

| State | T3 Code | Butler | Raw differing pixels |
| --- | --- | --- | --- |
| Soft line break: `line continues`, `1/1` | [PNG](t3/saved-assistant-markdown-soft-break-light-1280x800.png) | [PNG](butler/saved-assistant-markdown-soft-break-light-1280x800.png) | 268,603 of 1,024,000 |
| Paragraph boundary: `here. Second`, `0/0` | [PNG](t3/saved-assistant-markdown-paragraph-boundary-light-1280x800.png) | [PNG](butler/saved-assistant-markdown-paragraph-boundary-light-1280x800.png) | 267,897 of 1,024,000 |

Original unchanged 1280×800 X11 captures from
[native run 45441515696](https://github.com/Ferrb9579/butler/actions/runs/45441515696)
and [artifact 14942874898](https://github.com/Ferrb9579/butler/actions/runs/45441515696/artifacts/14942874898).
T3 Code v0.1.11 was `59651c1761f6c63e9b6056839067066968e6a406`; Butler was
`7780aa5d769462d6f4c1a718f9cdc5138fa2cc87`. The synthetic saved reply is
identical, both paragraphs are visible, and both Find states passed. The
before-Find originals remain in the artifact. No provider was used. This
does not verify unsupported Markdown, match background (IG-008), or pixel
parity; the sidebar, composer and window chrome still differ.
