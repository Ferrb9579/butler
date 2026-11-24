# Butler

Native Rust desktop application using Incular. The initial workspace includes a sidebar and an editable prompt.

## Run

Install Rust 1.99 or newer and the
[Incular platform dependencies](https://fabortlabs.github.io/incular/docs/introduction/installation/),
then run `cargo run`. Cargo fetches Incular from the Git revision in
`Cargo.toml`.

## Compare a screen on Linux

1. Use the sibling checkout of [T3 Code](https://github.com/pingdotgg/t3code)
   at `3f09ad91`. Install its dependencies using its development instructions,
   run its web client, and install Chromium with
   `pnpm --filter @t3tools/web exec playwright install chromium`.
2. Start Butler in an X11 session at 100% display scaling. Use `xwininfo` and
   select its client window to find the window ID. Set the matching state in
   both apps and choose a CSS selector that appears once T3 Code is ready.
3. Run the comparison with Node.js and ImageMagick 7 installed.

   ```sh
   scripts/compare-ui.sh new-thread-empty http://localhost:5173 'main' 0x4600007
   ```

The script captures T3 Code at Butler's client dimensions and saves matching
PNGs under `screenshots/t3` and `screenshots/butler`. Diffs and differing pixel
counts go under `artifacts/ui`. Exit status 1
indicates a visual difference. The URL, selector, and window ID above are
examples. Set `T3_STORAGE_STATE` to a local Playwright storage-state file if
the reference screen requires sign-in. Keep that file outside the repo.
Use `UI_THEME=light` for light-mode captures. Compare interaction and keyboard
behavior separately using the same project, thread, and provider fixture.

### Native desktop startup diagnostic

The [Native startup capture](.github/workflows/native-startup.yml) GitHub
Actions workflow builds Butler and runs both native Linux windows under Xvfb.
It uses the checksum-verified T3 Code v0.1.11 AppImage, not the newer
source checkout above. Download the `native-startup` artifact for PNGs,
pixel diff, app logs, and exact revisions. It uses clean profiles and records
each app's default startup state. This is a diagnostic, not evidence that the
two apps reached an equivalent fixture or passed UI parity.
