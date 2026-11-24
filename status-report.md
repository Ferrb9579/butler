# Butler status report

## 2026-11-24 (IST)

- Published the Rust scaffold and screenshot comparison workflow in `bab8858`.
- Pinned Incular to Git commit `71feef92286dfd6f8accd9c17c14f292da746490` and checked out T3 Code at `3f09ad91` as the reference.
- Installed Rust and Cargo 1.100. Cargo resolved dependencies. The first build check stopped at missing Linux Fontconfig development files.
- Shell and JavaScript syntax checks passed. The first Butler window is a startup smoke test. Reference captures, pixel comparison, and interaction parity remain pending.
- Installed the required Linux development files in the cloud workspace. `cargo check --locked` passed on Rust 1.100.
- Installed T3 Code's pinned Node 26.13.1 and pnpm 12.14.0. Reference dependencies and runtime capture are still pending.
- Added the first native workspace shell with editable search and prompt fields. The locked compile check passed. Live reference capture is blocked by the npm registry restriction and an incomplete offline cache.
- The full native build passed after fixing workspace-local library links. Added a matched screenshot naming workflow. A verified T3 Code and Butler image pair is still pending.
- Rehydrated the checkout and confirmed the official T3 Code v0.1.11 Linux AppImage checksum. Added a GitHub-hosted native startup capture to avoid relying on the unavailable local display and npm install. It produces two 1280×800 captures, a pixel diff, and provenance when the native windows both open. The stable reference is `59651c1761f6c63e9b6056839067066968e6a406`, distinct from the source inspection checkout. CI and real image results are pending; default startup is not a matched interaction fixture.
- Ported the pinned upstream first-run routing decision and stalled-startup recovery to a standalone Rust module. Four unit tests pass without native dependencies. This is startup logic only: the Welcome view, durable setup state, and integration into the desktop shell are not wired yet. Included the upstream MIT notice.
- Native capture passed on GitHub Actions run `45318341970`. Both windows rendered at 1280×800. The diagnostic pair shows T3 Code's Welcome setup against Butler's dark draft shell; all 1,024,000 pixels differ. Saved both original PNGs as an unmatched startup baseline. This establishes which screen to implement first, not parity.
- Added atomic, profile-local first-run completion storage. The locked library suite passes all seven tests. Completion survives reopening, profiles stay isolated, and partial or unreadable markers do not count as completed setup. The view is not wired yet.
