# T3 Code source parity

Reference: T3 Code v0.1.11, Git commit `59651c1761f6c63e9b6056839067066968e6a406`.
Read the relevant TSX, CSS variables, reusable component definitions and tests
at that commit before porting a screen. Translate their states and constraints
into Incular, then compare native screenshots using matched window size,
theme, settings, provider state and test data. A passing capture is not itself
an assertion of identical pixels or behavior.

Paths below are relative to the pinned upstream tree.

## Settings / Providers

| Source | Resolved reference at default 16 px root | Butler status |
| --- | --- | --- |
| `apps/web/src/components/AppRouteModal.tsx:68–110` | Below 640 px: full bleed with safe-area/native insets. 640–1023: outer inset 24 px, top max(24 px, caption height). 1024+: 40 px inset. Framed popup is centered, capped at 1280×960, border 1 px, radius 16 px, shadow 0 25 50 −12 px black at 40%. | Dimensions for zero native inset translated and tested. Caption/safe-area integration and modal focus/dismissal behavior not equivalent yet. |
| `apps/web/src/index.css:445–451`; `components/ui/dialog-styles.ts` | In light theme, background color at 60% over a 4 px blur; dark 64%. Backdrop transition 200 ms; popup scale 0.985 → 1 and opacity over 150 ms ease-out. | Light backdrop translated; animation and dark parity pending. |
| `apps/web/src/routes/_chat.settings.tsx:124–158`; `components/WorkspaceBreadcrumb.tsx:11–71` | Header 48 px, bottom border 1 px, horizontal start/end inset 20/8 px, breadcrumb gap 12 px desktop (8 px compact), 14/20 px weight 500, close hit area 28×28 px on desktop. | Desktop geometry translated. Exact Lucide close icon, compact and focus states pending. |
| `apps/web/src/components/settings/SettingsSidebarNav.tsx:257–439`; `components/ui/sidebar.tsx:685–775` | Sidebar width including divider 240 px desktop, 56 px below 768. Group inset 8 px. Navigation rows 32 px high, 8 px radius, inset 10/6 px, 16 px icons, gap 8 px, row spacing 4 px, group spacing 16 px. | Translated desktop and rail widths, rows, gaps, captions and original glyphs. Search and navigation beyond Providers are still non-interactive. |
| `apps/web/src/components/WorkspacePageContainer.tsx:7–23`; `components/settings/settingsLayout.tsx:653–660`; `components/settings/SettingsScopeSentence.tsx` | Centered 896 px max-width page; sides 24 px desktop or 20 px compact, top/bottom 24/48 px. Scope row and section gap 32 px. Scope adds 16 px desktop/12 px compact inset; outlined triggers are 28/32 px tall at desktop/compact, radius 8 px, and wrap in label/control pairs. | Translated the centered page, gutters, wrap and trigger geometry/signs. Project/device selection and scope menus are not implemented. |
| `apps/web/src/components/settings/ProviderSettingsPanel.tsx:1027–1145`; `components/settings/ProviderInstanceCard.tsx:988–1004,1173–1254` | Outer split card uses 272 px left list and fill right detail, height clamped to 512–704 px from viewport height minus 176 px. List rows min 72 px, inset 16/12 px, gap 12 px, 20 px icon box with 16 px glyph, text 14 px and status 12 px; selected ink at 6%; add row 44 px. | Translated responsive list/detail structure, height cap, scrolling, divider and base row sizing. Provider brand icons and real status/toggle states still differ. |
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
`main.tsx:4`). The current Butler font does not match; exact font embedding
and platform fallback are separate from token and geometry parity.

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
