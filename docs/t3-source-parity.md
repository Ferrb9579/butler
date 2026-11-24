# T3 Code source parity

Reference: T3 Code v0.1.11, Git commit `59651c1761f6c63e9b6056839067066968e6a406`.
Read the relevant TSX, CSS variables, reusable component definitions and tests
at that commit before porting a screen. Translate their states and constraints
into Incular, then compare native screenshots using matched window size,
theme, settings, provider state and test data. A passing capture is not itself
an assertion of identical pixels or behavior.

Paths below are relative to the pinned upstream tree.

## Welcome onboarding

- `apps/web/src/components/onboarding/WelcomePage.tsx:401–407,679–695`:
  the existing no-agent disclosure fills its content lane with a minimum
  40 px hit target. Its 20×20 px lead box centers the original 16 px Plus
  glyph; the 14/20 px label starts 12 px after the box. The same 16 px
  ChevronDown reaches the far edge and rotates 180° when open, without
  changing the row's layout. In the no-agent state it remains transparent
  at rest, with a 6 px corner and inset focus ring. The retained button
  exposes the real expanded state and one activation. This is the
  existing lane and existing offline panel, not a port of the closed
  provider-logo preview, install, phone, or privacy controls.
- `WelcomePage.tsx:707–723` places the ACP Registry mark in a 20×20 px
  RowIcon slot, with the source 16 px aspect-fit box. The existing
  light catalogue row uses the published pure-black mark; its disabled
  Browse control remains unchanged. Other provider marks are independent.

## Settings / Providers

| Source | Resolved reference at default 16 px root | Butler status |
| --- | --- | --- |
| `apps/web/src/components/AppRouteModal.tsx:68–110` | Below 640 px: full bleed with safe-area/native insets. 640–1023: outer inset 24 px, top max(24 px, caption height). 1024+: 40 px inset. Framed popup is centered, capped at 1280×960, border 1 px, radius 16 px, shadow 0 25 50 −12 px black at 40%. | Dimensions for zero native inset translated and tested. Caption/safe-area integration and modal focus/dismissal behavior not equivalent yet. |
| `apps/web/src/index.css:445–451`; `components/ui/dialog-styles.ts` | In light theme, background color at 60% over a 4 px blur; dark 64%. Backdrop transition 200 ms; popup scale 0.985 → 1 and opacity over 150 ms ease-out. | Light backdrop translated; animation and dark parity pending. |
| `apps/web/src/routes/_chat.settings.tsx:124–158`; `components/WorkspaceBreadcrumb.tsx:11–71` | Header 48 px, bottom border 1 px, horizontal start/end inset 20/8 px, breadcrumb gap 12 px desktop (8 px compact), 14/20 px weight 500, close hit area 28×28 px on desktop. | Desktop geometry translated. Exact Lucide close icon, compact and focus states pending. |
| `apps/web/src/components/settings/SettingsSidebarNav.tsx:257–439`; `components/ui/sidebar.tsx:685–775` | Sidebar width including divider 240 px desktop, 56 px below 768. Group inset 8 px. Navigation rows 32 px high, 8 px radius, inset 10/6 px, 16 px icons, gap 8 px, row spacing 4 px, group spacing 16 px. | Translated desktop and rail widths, rows, gaps, captions and original glyphs. Search and navigation beyond Providers are still non-interactive. |
| `apps/web/src/components/WorkspacePageContainer.tsx:7–23`; `components/settings/settingsLayout.tsx:653–660`; `components/settings/SettingsScopeSentence.tsx` | Centered 896 px max-width page; sides 24 px desktop or 20 px compact, top/bottom 24/48 px. Scope row and section gap 32 px. Scope adds 16 px desktop/12 px compact inset; outlined triggers are 28/32 px tall at desktop/compact, radius 8 px, and wrap in label/control pairs. | Translated the centered page, gutters, wrap and trigger geometry/signs. Project/device selection and scope menus are not implemented. |
| `apps/web/src/components/settings/ProviderSettingsPanel.tsx:1027–1145`; `components/settings/ProviderInstanceCard.tsx:988–1004,1173–1254` | Outer split card uses 272 px left list and fill right detail, height clamped to 512–704 px from viewport height minus 176 px. List rows min 72 px, inset 16/12 px, gap 12 px, 20 px icon box with 16 px glyph, text 14 px and status 12 px; selected ink at 6%; add row 44 px. | Translated responsive list/detail structure, height cap, scrolling, divider and base row sizing. Local enable toggles have the source-derived dimensions and defaults; the preferences persist. The list and editor render the original provider marks; rows expose the source selection name and pressed state. Full-row hover, disabled opacity, diagnostic text wrapping, live availability and project-specific enablement still differ. |
| `apps/web/src/components/chat/ProviderInstanceIcon.tsx:29–43,162–169`; `apps/web/src/components/Icons.tsx:414–436`; `packages/provider-cursor/src/client.ts:21–27` | Codex, Claude and Cursor use original SVG marks on default cards: centered 16px glyph inside 20px wrapper, 12px to list title or 8px to editor title. | Original path geometry, fill rules, and colors rendered using low-level Incular Canvas; the mark uses the provider driver and stays unchanged when its name is edited. Native screenshot recapture remains pending. |
| `apps/web/src/components/settings/settingsLayout.tsx:235–262,526–572`; `ProviderInstanceCard.tsx:1360–1391`; `apps/web/src/components/ui/input.tsx:13–32` | Editor title min 28 px, 10 px to first row; row padding 12 px vertically and 16/12 px horizontally; edge-to-edge divider; Display name field stacks with 12 px gap when the row content is below 512 px, otherwise 224 px field and 32 px column gap. | Translated container sizing, status spacing, divider and editable field layout using Incular LayoutBuilder. Accent picker and exact input focus decoration remain unported. |
| `apps/web/src/components/ui/draft-input.tsx`; `apps/web/src/hooks/useCommitOnBlur.ts` | Name edits stay local while focused; commit only if changed on blur. Non-composing Enter blurs. The field hook does not intercept Escape. | Uses the neutral Incular focus-within callback and the single-line editor’s own Enter behavior. Pointer blur and Enter are tested. Modal-wide Escape and route dismissal behavior are separate and not yet matched. |
| `apps/web/src/components/settings/SettingsPanels.tsx:2449–2539`; `OpenSourceLicenses.tsx:78–154,249–312`; `SettingsSidebarNav.tsx:117–120` | About links to a child route with the parent still selected. Licenses shows grouped drawers, 36/40 px rows, 14 px chevrons, expanded prewrapped mono notice text, and one open at a time. | About exposes the real Butler version and bundled notices, with disclosures and keyboard routing through the Settings shell. Updater, other legal controls, and a complete dependency inventory are not yet implemented. |
| `apps/web/src/components/settings/settingsLayout.tsx:527–571`; `components/settings/ProviderAuthenticationSection.tsx`; `apps/web/src/index.css:1304–1310` | Detail rows use 12/16 px padding and 14/20 px labels; supporting text 12 px. Right-side controls become a separate column at 512 px of row container width. Nested detail groups are transparent and have no shadow. | Editable display name saves locally; no live provider, account, environment variables or model settings. |

The inspected Linux capture (run `45348273688`) is a pre-translation snapshot,
not the source of token values. Keep screenshots and source review distinct.

### Light theme tokens

The released AppImage's built CSS (`apps/server/dist/client/assets/main-C7LLcwbG.css`
in `app.asar`) confirms these OKLCH values. The table lists nearest 8-bit sRGB
for Incular. Alpha modifiers are applied to the role, not substituted with
an unrelated shade.

| Token | Role | sRGB |
| --- | --- | --- |
| zinc-25 | Background | `#FCFCFC` |
| zinc-50 | Sidebar, muted surface | `#FAFAFA` |
| zinc-100 | Accent | `#F4F4F5` |
| zinc-200 | Border | `#E4E4E7` |
| zinc-300 | Input border | `#D4D4D8` |
| zinc-500 | Muted foreground and icon base | `#71717B` |
| zinc-900 | Foreground | `#18181B` |
| white | Card, popover and selected sidebar row | `#FFFFFF` |

Settings popup, sidebar and header now use the light-role palette. The
reference default sans stack is `-apple-system, BlinkMacSystemFont, "Inter
Variable", "Segoe UI", system-ui, sans-serif` (`index.css:168–175` and
`main.tsx:4`). Butler Settings now uses the pinned Linux release’s Latin Inter Variable face.
The other script subsets, platform-specific font precedence, user typography
controls, and text shaping/rendering comparisons are still unverified.

## Saved transcript and Find

- Pinned `components/chat/TimelineUserMessageRow.tsx` uses a right-aligned
  plain-text bubble, at most 80% of the lane, with 12 px padding, 16 px corners
  and stock-light `#ebebed`. The plain assistant row in
  `components/chat/TimelineAssistantRows.tsx` is unfilled and inset 4 px
  horizontally and 2 px vertically. Both translated body styles use 14 px
  regular interface text and a 22.75 px line height.
- `components/ChatMarkdown.tsx` and `index.css:1871–1887` render ordinary
  paragraphs with 0.65 rem collapsed margins: 10.4 px between paragraphs
  at 16 px root size, with no extra margin at either end. Butler's bounded
  CommonMark projection covers ordinary multiline saved assistant prose:
  soft line breaks become spaces, and Find searches each visible paragraph
  separately as in `packages/shared/src/threadFindText.ts`. Single-line
  and unsupported replies, user messages, raw saved text and sidebar
  search keep their previous path. Bold remains literal (IG-006); links,
  HTML, code, and unsupported Markdown are not activated or transformed.
- The native saved-thread fixture has the same two synthetic messages in
  both apps. The pinned reference shows them after dismissal of the stock
  provider-status alert for that session only. Both apps passed second-match
  navigation and the no-match state using the real Find bar. This does not
  verify Markdown, rich attachments, providers, or the inline match
  background blocked by IG-008. See the [capture provenance](../screenshots/README.md).

### Saved local thread details

- Pinned `threadDetailsCardLayout.ts` and `chatCanvasLayout.ts` place the
  280 px inline card 12 px from the right and top, with a 32 px clear gap to
  the conversation lane. `canvasStore.ts` shifts it down 48 px when Find is
  open. The 1280×800 retained paint test checks the actual card at (988, 52)
  and then (988, 100), not just the requested layout.
- The first row shows the real local device and uses the canonical workspace
  path for its tooltip. The pinned header toggle and Mod+Shift+B affect
  only the selected saved thread. Like the pinned `rightPanelStore.ts`,
  Butler stores only explicit inline closures in the local profile and
  clears them when reopened.
- At narrow desktop widths, the normal toggle opens a separate transient
  popover anchored to the whole chat header, following
  `ThreadDetailsCard.tsx` and `ChatView.tsx`. Narrow opens never change the
  persisted inline preference. The same pointer group lets Find remain in
  use while the popover is open. Tests cover outside click, Escape, thread
  switch, and narrow→wide→narrow. Exact keyboard focus-loss dismissal
  remains unverified. The optional native
  compact fixture passed closed, open, and Escape-dismissed at 1000×800.
  The editor, scripts, and other Git rows are not implemented.
- Right-clicking the workspace row in the wide inline card opens T3's
  `BranchToolbar.tsx` Copy full path action. The application writes the exact
  canonical path only on selection. Compact details omits the nested menu
  because of IG-010.
- `ThreadDetailsPanel.tsx` gates the branch section on a real non-scratch
  worktree. `BranchToolbarBranchSelector.tsx` and
  `threadDetailsPanelStyles.ts` supply the exported GitBranch glyph, the
  32 px row, 16 px icon, 10 px horizontal padding and gap, and 13 px
  medium-weight text. Butler displays only an actual named HEAD from the
  active registered project, including an unborn or linked worktree
  branch. This is a read-only selection/open snapshot, not the branch
  selector, detached fallback, external checkout live refresh, or
  mid-label truncation. No branch or Git action is simulated.
- On the wide named branch row, the pinned `BranchToolbarBranchSelector.tsx`
  **Copy branch name** context action copies the exact displayed branch name
  only when selected. It shares the workspace row's real clipboard route
  and does not run another Git command or open a picker. Compact still
  omits nested menus (IG-010).

### Saved-thread composer, without a provider

- Pinned `ChatView.tsx`, `ComposerSurface.tsx`, and `ChatComposer.tsx`
  anchor the dock in the conversation lane with a 20 px desktop bottom
  inset, 16 px corners, a thin outline, and a two-line minimum editor.
  Butler keeps an editable multiline draft per saved thread. Enter inserts
  a newline; Send is explicitly unavailable
  because no provider is configured. Drafts do not write to the transcript.
- Pinned `composerDraftStore.ts` persists nonempty raw prompt text by
  thread and debounces local writes. Butler now does the same for plain text
  only, up to 1 MiB per draft, restoring it from the same local profile.
  Clearing the field removes just that thread's draft. Attachments,
  microphone, model selection, and live send/feedback remain out of scope.

## Implementation boundary

Butler owns its visual components. Do not enable or import Incular's stock
design systems (`incular-controls`, `incular-material`, or equivalents).
Use the pinned framework's low-level widgets, layout, painting, focus,
semantics and input as building blocks. App-local buttons in `src/ui.rs`
use the T3 button source for variants and take explicit per-screen sizes.
Not all T3 button sizes/animations, icons or dark/focus states are ported.
Framework parity gaps belong in `INCULAR-GAPS.md`, not in application shims.

Settings navigation glyphs are the original Lucide 0.564.0 paths from the pinned
AppImage and the custom SnapShots path from the pinned TSX. Butler draws them
with the low-level Incular Icon widget. The notices are in `LICENSES/`.

The Settings overlay follows the pinned `AppRouteModal.tsx:68–81` pointer
dismissal: a tap must start on the backdrop and release outside the panel.
A pointer gesture crossing the panel boundary does not dismiss it. Escape,
keyboard focus restoration, and close animation are not implemented here.

## Local New project flow

- Pinned `CommandPalette.tsx:1090–1122,2432–2586` supplies the New project
  source row, Project name field, name/path preview, Create, and Add existing
  project. Butler opens a blank project draft on local success. Remote targets,
  publish controls, focus-on-open and exact new form visual parity are pending.
- Pinned `apps/server/src/project/ManagedProjectFolders.ts:145–215,393–508`,
  `packages/contracts/src/project.ts:533–544`, and
  `packages/shared/src/path.ts:69–120` specify project name validation,
  normalization and suffixing, Git branch/setup, README/icon scaffold, and
  nonfatal initial commit failure. Butler keeps the title separate from its
  disk slug using its atomic local registry, retains old path-only profiles,
  and identifies Butler truthfully in the generated README. Only the current
  device is supported. The local Git subprocesses run without arbitrary
  repository hooks, filters, templates, signing, or remote calls.
  Butler now runs the bounded create off the UI thread using the framework's
  owner-scoped task primitive. The input's trailing Create/Enter control shows
  Creating and rejects duplicates until completion. Cancel and reference-only
  publishing flows are not ported.
- Normal empty and named New Project use the physical layout from
  `CommandPaletteAddProject.tsx:127–159` (8px summary top inset plus a 48px
  two-line row) and `CommandPaletteResults.tsx:151–158,276–292` and
  `ui/command.tsx:190–205` (8px above and below the 48px Add existing row).
  The retained separator is y249 and the Add existing row begins at y193
  at 1280×800. The summary glyph's internal alignment, path text, autofocus,
  and failed-submit layout remain separate.

In Open source licenses, T3's search at `OpenSourceLicenses.tsx:135–229`
and `thirdPartyLicenses.ts:96–112` maps to an app-owned chip and editor with
metadata-only AND filtering. Counts, no-results text, Escape and empty blur
are translated. Raw input autofocus is still logged in `INCULAR-GAPS.md`.

For Git URL, the first Rust-only slice ports a safe subset of
`packages/client-runtime/src/operations/projects.ts:141–145,222–282`:
known GitHub/GitLab page normalization, public-host shorthand, repository
leaf suggestion and lexical destination handling. Butler validates only
HTTPS inputs without userinfo/query/fragment; it rejects ambiguous path
components instead of silently normalizing traversal. This pure preparation
does not contact hosts, decide they are trusted, start Git, or expose an
unfinished Clone button. Non-HTTPS and broader provider routes remain open.

The project palette reads the pinned `CommandPaletteContent.tsx:94–126`
footer and `CommandPalette.tsx` key handling for the implemented Enter,
empty-name Backspace and Escape actions. The focused editor can use these
shortcuts. Autofocus and arrow-driven result navigation are not ported.

Git URL now opens the review step from the pinned `CommandPalette.tsx`
at lines 2191–2231 before any Git request. It shows a normalized Repository
summary and an editable case-preserving destination. The URL-entry footer
follows the relevant key groups in `CommandPaletteContent.tsx:94–126`,
`ui/command.tsx:224–234` and `ui/kbd.tsx:5–34`: 20px keycaps, 4px within
groups, 12px between groups and a 16px inset. With the URL field focused,
Enter reviews an eligible URL, Backspace returns to Sources only when the
field was already empty, and Escape closes. The existing pointer Close also
remains available. Directional result navigation is not advertised; raw
input autofocus, modal focus and composing Enter remain governed by
IG-001, IG-004 and IG-007.

This first safe clone path
accepts only default-port public GitHub/GitLab HTTPS with a trusted system
Git, requires an existing owned destination parent and an explicit Clone
activation, and registers only a completed no-replace checkout. That
registration order intentionally differs from T3's provisional registry.
The review browses existing parent and child folders while preserving the
suggested repository leaf. Create & Clone for missing parents,
SSH/authenticated hosts, cancellation, and the rest of the reference keyboard
navigation remain open. The New project, Local folder, and Git URL glyphs
use the original pinned Lucide 0.564.0 geometry (cubic paths for arcs).

The active-workspace sidebar ports the 28 px FolderPlus button from
`SidebarThreadHeader.tsx:61–173`. It opens the same Add project palette,
without making project headers switch the active chat. The multi-project
thread destination chooser and the separate filter are still unported.

For multiple available local projects, the sidebar's New thread action
opens a searchable project chooser following `Sidebar.tsx:2966–2995` and
`CommandPaletteProjectItems.tsx:77–102`. Selecting a saved canonical folder
starts a fresh draft and loads only that project's thread list. Cancelling
or filtering leaves the active transcript and registry untouched. Butler's
local recent-first ordering and simple title/path filtering are not the
reference's general project search, fuzzy ranking, shortcuts, or device groups.

The active sidebar now uses T3's inline Search threads control from
`SidebarThreadHeader.tsx:61–99` and `Sidebar.tsx:1499–1537`: typed text
filters persisted titles in the current workspace, with source-shaped
case/accent folding, ranked title matches, stable ID highlight, wrapped
arrows, Enter to open and Escape/clear to restore the list. Message bodies,
branch and PR metadata, fuzzy usage ranking, and cross-project search
remain unported. Search does not invoke a provider.

Sidebar search now adds message-only results after title matches, following
`Sidebar.tsx:1365–1415`, `Sidebar.logic.ts:1101–1137` and the 120 ms debounce
in `state/queries.ts:124–154`. It searches up to 50 recent saved threads in
the current workspace, one file at a time, and shows one actual excerpt per
thread. Selecting a message result opens the saved thread and seeds Find at
that message. Older records and cross-project indexing are still unported.
