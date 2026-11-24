# Published screenshot audit

Reviewed all 32 original PNGs (16 Butler/T3 Code pairs) tracked on Butler
`7780aa5d769462d6f4c1a718f9cdc5138fa2cc87` on 2026-11-24. Fifteen
pairs are 1280×800, one is 1000×800. All files decode and have a same-size
partner. The published images are earlier captures, not current-build proofs.
Read [capture provenance](README.md) alongside this audit.

The table records what is visible in each published pair. The first eight
main-screen findings and Welcome were also compared with unchanged current
diagnostics from [run 45438475842](https://github.com/Ferrb9579/butler/actions/runs/45438475842)
(Butler `5d59956`, same app source as `7780aa5`, pinned T3 Code v0.1.11).
“Current” refers to an observed current diagnostic, not merely a checked-in icon.

Follow-up: [three refreshed original pairs](README.md#refreshed-welcome-and-new-project-native-audit-follow-up)
supersede the historical Welcome and New Project pictures below as evidence of
the current UI. Welcome's leading Plus, right chevron and ACP Registry mark
are present. The other seven provider logos remain absent. New Project's
replacement icons are visible; Up/Down navigation, initial focus, footer
position and the truncated typed path still differ.

| Published pair | Visible differences / disposition |
| --- | --- |
| [About](butler/about-fresh-light-1280x800.png) | Sidebar is scrolled differently; captured Butler omits the selected About row. Latest Butler does show selection, but it remains lower than T3. Missing update, track, command, mobile, Welcome and privacy actions; missing install/platform icons go with those missing features. |
| [Git URL destination](butler/git-url-destination-light-1280x800.png) | Published destination uses folder-plus; latest Butler shows the source open-folder glyph. Link and Back are visible. Clone's Enter badge, shortcut footer and Open in Files are still absent; dialog and rows are vertically compressed. |
| [Git URL typed](butler/git-url-typed-light-1280x800.png) | Back, Continue and its Enter badge are present. Butler has a centered Close instead of the navigation/keyboard footer; extra whitespace under the hint persists in latest. |
| [Native startup baseline](butler/native-startup-baseline-1280x800.png) | Deliberate historical diagnostic: light T3 Welcome vs an obsolete dark Butler draft shell. Exclude from visual comparison, not evidence that current Welcome is dark. |
| [New Project empty](butler/new-project-empty-light-1280x800.png) | Published Butler substitutes a dot/small Back and plus/return glyphs, and has no right-chevron or shortcut footer. Latest native shows the source Back, folder-create, folder-plus and chevron plus Enter/Backspace/Escape hints; Up/Down Navigate, initial focus/caret and spacing still differ. |
| [New Project named](butler/new-project-named-light-1280x800.png) | Same typed title in both. Same stale icon findings as empty; current Butler still truncates its generated path. The underlying fixture directories are different, so do not treat path differences or their blur as visual regressions. |
| [Open-source licenses](butler/open-source-licenses-fresh-light-1280x800.png) | Published Butler has 3 bundled notices, no search/scope controls, no per-row external-link glyph. Latest Butler displays 5 real notices and the search icon; scope and external-link actions are still missing. Butler should show its own truthful dependency inventory rather than copying T3's package names/count. |
| [Providers](butler/providers-fresh-light-1280x800.png) | Codex/Claude/Cursor marks already match the source identities. T3 shows red missing-provider status dots, refresh and add affordances, account/API/variable setup; Butler has offline placeholders instead. Row height and right-side spacing differ. The missing icons correspond mostly to missing state/actions; don't add decorative buttons. |
| [Branch Details](butler/saved-thread-branch-details-light-1280x800.png) | Both show actual `copperleaf` with a branch glyph. Butler shows desktop-monitor vs T3 cloud because the workspace/device label is different, omits editor/script/publish/changes icons and selector chevrons, and has a shorter card. Existing offline branch caption is intentionally read-only. |
| [Compact Details](butler/saved-thread-compact-details-open-light-1000x800.png) | Same card start at x708/y48. Butler shows the one workspace row; T3 also has editor, add-script and Initialize Git rows and icons. Both folder icons are present. Machine labels are supplied by different fixtures. |
| [Saved composer](butler/saved-thread-composer-light-1280x800.png) | Dock width/bottom align; Butler is 3 px taller. No paperclip, model/provider selector mark or mic on the offline dock. T3 shows configured composer UI; Butler correctly renders disabled Send with an enable-provider prompt. Details and sidebar controls still differ. |
| [Find first match](butler/saved-thread-find-first-match-light-1280x800.png) | Both show `release` at 1/2 with chevrons and Close. T3 paints inline amber highlights; Butler does not (IG-008). Published Butler predates the saved dock/details. |
| [Find next match](butler/saved-thread-find-next-light-1280x800.png) | Both show `release` at 2/2. Same missing highlight and historical shell differences as first match; compare the count/navigation, not the whole older shell. |
| [Find no match](butler/saved-thread-find-no-match-light-1280x800.png) | Both show red `0/0` for `absent`. No highlight is expected. Real shell and fixture differences remain; this is not evidence of a missing Find result. |
| [Saved transcript](butler/saved-transcript-light-1280x800.png) | Same two synthetic message bodies; title and sidebar content differ. T3 has Details and composer, absent in this historical Butler frame; both exist in later captures. The old Butler bubble is farther right and its assistant row starts higher. |
| [Welcome](butler/welcome-fresh-light-1280x800.png) | Published frame has clipped button text and no provider marks. Latest frame has readable buttons, phone, Codex, Claude and Cursor marks. **Still missing**: Antigravity, fx, Kiro, OpenCode, Grok, Muse Code, Pi and ACP registry marks; current Claude Install also lacks the source terminal icon and phone Setup lacks the down chevron. Group-header alignment, disabled control shades, privacy footer and native window controls differ. |

## Priorities and recapture policy

1. Port the **confirmed missing Welcome marks** from the pinned reference,
   tracking asset provenance. Do not invent substitutes or add unavailable
   installation actions just to produce the same pixels.
2. Recapture Welcome and the two New Project frames after confirmed fixes.
   Use equal dimensions/theme and matching input, project path presentation,
   sidebar scroll and device configuration wherever possible. Preserve
   historical originals and clearly label unmatched fixtures.
3. Fix still-missing Git dialog affordances only with real underlying
   actions; measure modal/layout differences locally rather than relying on
   full-screen pixel counts from different paths or blurred shells.
4. Keep Incular issues in [INCULAR-GAPS.md](../INCULAR-GAPS.md) without app
   workarounds. IG-008 covers missing inline Find highlight background;
   other empty panels and controls above are Butler feature gaps, not proven
   Incular differences.
