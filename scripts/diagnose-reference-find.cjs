// Run as Node using the checksum-pinned T3 binary. Open the disposable fixture
// using its real sidebar route, then keep that Electron window for X11 capture.
// Emit bounded geometry and booleans; only on-screen OCR verifies a frame.
"use strict";

const path = require("node:path");
const fs = require("node:fs");
const { setTimeout: delay } = require("node:timers/promises");

async function main() {
  let app;
  const stop = new AbortController();
  const onStop = () => stop.abort();
  process.once("SIGTERM", onStop);
  process.once("SIGINT", onStop);
  const ready = process.argv[4];
  const size = { width: Number(process.argv[5]), height: Number(process.argv[6]) };
  const diagnosticPng = process.argv[7];
  const failures = { page: 0, console: 0 };
  const report = (summary) => fs.writeFileSync(process.argv[3], JSON.stringify(summary) + "\n", {
    mode: 0o600,
  });
  let stage = "start";
  try {
    const bundle = path.resolve(process.argv[2]);
    const { _electron } = require(path.join(
      bundle, "resources/app.asar/node_modules/playwright-core",
    ));
    const env = { ...process.env };
    delete env.ELECTRON_RUN_AS_NODE;
    delete env.DEBUG;
    delete env.PWDEBUG;
    stage = "launch";
    app = await _electron.launch({
      executablePath: path.join(bundle, "t3code"),
      env,
      chromiumSandbox: true,
      timeout: 15_000,
    });
    stage = "window";
    const page = await app.firstWindow({ timeout: 10_000 });
    page.on("pageerror", () => failures.page++);
    page.on("console", (message) => {
      if (message.type() === "error") failures.console++;
    });
    stage = "native-window";
    const native = await app.browserWindow(page);
    const xid = await native.evaluate((window, size) => {
      // The pinned Linux reference is x86-64. An X11 resource ID fits in
      // the low 32 bits of the native unsigned-long handle.
      const handle = window.getNativeWindowHandle();
      if (handle.length < 4) return 0;
      window.setBounds({ x: 0, y: 0, ...size });
      return handle.readUInt32LE(0);
    }, size);
    if (!Number.isSafeInteger(xid) || xid <= 0) throw new Error("Invalid Electron XID");
    // Size before navigation; a native resize after the selected view has
    // rendered may rebuild the virtual timeline.
    stage = "viewport-size";
    await page.waitForFunction(
      ({ width, height }) => innerWidth === width && innerHeight === height,
      size, { timeout: 8_000 },
    );

    stage = "thread";
    const thread = page.locator("li[data-thread-item]", { hasText: "Release launch check" });
    await thread.click({ timeout: 12_000 });
    stage = "transcript";
    const user = "Does the release open?";
    const assistant = "The release opens and shows this thread.";
    const timeline = page.locator('[data-timeline-row-kind="message"]');
    await Promise.all([user, assistant].map((phrase) =>
      timeline.getByText(phrase, { exact: true })
        .waitFor({ state: "visible", timeout: 10_000 })
        .catch(() => {}),
    ));
    const readTranscript = () => page.evaluate(({ user, assistant }) => {
      const rows = [...document.querySelectorAll('[data-timeline-row-kind="message"]')];
      const geometry = (rect) => rect ? {
        x: Math.round(rect.x), y: Math.round(rect.y),
        width: Math.round(rect.width), height: Math.round(rect.height),
      } : null;
      const count = (id, phrase) => {
        const row = rows.find((element) => element.dataset.messageId === id);
        if (!row) return {
          row: false, phrase: false, intersectsViewport: false, rect: null,
          textRect: null, textTopHits: 0,
        };
        const rect = row.getBoundingClientRect();
        const viewport = row.closest(".messages-timeline-scroll")?.getBoundingClientRect();
        // Sample the actual phrase rather than the much larger timeline row.
        // Both fixture messages are plain single text nodes in the Find body.
        let textRect = null;
        let textTopHits = 0;
        const body = row.querySelector("[data-thread-find-text]");
        if (body) {
          const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
          for (let node; (node = walker.nextNode());) {
            const start = node.textContent.indexOf(phrase);
            if (start < 0) continue;
            const range = document.createRange();
            range.setStart(node, start);
            range.setEnd(node, start + phrase.length);
            const phraseRect = range.getClientRects()[0];
            if (!phraseRect || phraseRect.width <= 0 || phraseRect.height <= 0) continue;
            textRect = geometry(phraseRect);
            const y = phraseRect.top + phraseRect.height * 0.25;
            textTopHits = [0.2, 0.5, 0.8].filter((fraction) => {
              const x = phraseRect.left + phraseRect.width * fraction;
              return row.contains(document.elementFromPoint(x, y));
            }).length;
            break;
          }
        }
        return {
          row: true,
          phrase: row.innerText.includes(phrase),
          rect: geometry(rect),
          textRect,
          textTopHits,
          intersectsViewport:
            rect.width > 0 && rect.height > 0 &&
            rect.right > Math.max(0, viewport?.left ?? 0) &&
            rect.bottom > Math.max(0, viewport?.top ?? 0) &&
            rect.left < Math.min(innerWidth, viewport?.right ?? innerWidth) &&
            rect.top < Math.min(innerHeight, viewport?.bottom ?? innerHeight),
        };
      };
      return {
        viewport: { width: innerWidth, height: innerHeight },
        timeline: geometry(document.querySelector(".messages-timeline-scroll")?.getBoundingClientRect()),
        loading: document.querySelectorAll('[data-timeline-loading="true"]').length,
        rows: rows.length,
        user: count("launch-check-user", user),
        assistant: count("launch-check-assistant", assistant),
      };
    }, { user, assistant });
    stage = "provider-status-dismiss";
    const status = page.locator('.chat-banner-lane [role="alert"]').filter({
      has: page.getByText("Codex provider status", { exact: true }),
    });
    await status.waitFor({ state: "visible", timeout: 4_000 }).catch(() => {});
    let bannerDismissed = false;
    let beforeDismiss = null;
    if (await status.isVisible()) {
      const { user, assistant } = await readTranscript();
      beforeDismiss = { user, assistant };
      // This is the app's normal, non-persistent dismissal in this
      // disposable profile. Never touch its provider settings or Refresh.
      await status.getByRole("button", { name: "Dismiss Codex provider error", exact: true })
        .click({ timeout: 4_000 });
      await status.waitFor({ state: "hidden", timeout: 4_000 });
      bannerDismissed = true;
    }
    const summary = await readTranscript();
    const nativeSize = await native.evaluate((window) => {
      const { width, height } = window.getBounds();
      return { width, height };
    });
    const result = { stage: "complete", failures, nativeSize, bannerDismissed, beforeDismiss, ...summary };
    report(result);
    if (ready !== undefined) {
      if (nativeSize.width !== size.width || nativeSize.height !== size.height ||
          summary.viewport.width !== size.width || summary.viewport.height !== size.height ||
          !summary.user.phrase || !summary.user.intersectsViewport ||
          !summary.assistant.phrase || !summary.assistant.intersectsViewport) {
        process.exitCode = 1;
        return;
      }
      const pid = app.process().pid;
      if (!Number.isSafeInteger(pid) || pid <= 0) throw new Error("Invalid Electron PID");
      // This private file tells bash which real window to capture, never how
      // to evaluate a frame. Rename so bash only sees a complete PID and XID.
      fs.writeFileSync(`${ready}.tmp`, `${pid} ${xid}\n`, { mode: 0o600, flag: "wx" });
      fs.renameSync(`${ready}.tmp`, ready);
      app.once("close", onStop);
      const expires = Date.now() + 155_000;
      let diagnosed = false;
      while (!stop.signal.aborted && Date.now() < expires) {
        if (!diagnosed && diagnosticPng && fs.existsSync(`${ready}.diagnostic`)) {
          diagnosed = true;
          // Triggered only if the physical transcript is blank. Capture the
          // same-size renderer separately, never as a native pair candidate.
          let screenshot = "failed";
          try {
            if (await page.evaluate(
              ({ width, height }) => innerWidth === width && innerHeight === height &&
                document.querySelector("[data-chat-header]")?.innerText.includes("Release launch check"),
              size,
            )) {
              await page.screenshot({ path: diagnosticPng, scale: "css", timeout: 5_000 });
              screenshot = "captured";
            }
          } catch {}
          report({ ...result, rendererDiagnostic: screenshot });
          fs.writeFileSync(`${ready}.diagnosed`, "", { mode: 0o600, flag: "wx" });
        }
        await delay(250, undefined, { signal: stop.signal, ref: false }).catch(() => {});
      }
    }
  } catch {
    // Never include Playwright errors, logs, HTML or URLs in the artifact.
    report({ stage, failures, incomplete: true });
    process.exitCode = 1;
  } finally {
    if (app) {
      const child = app.process();
      await Promise.race([app.close().catch(() => {}), delay(2_000, undefined, { ref: false })]);
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    }
    process.off("SIGTERM", onStop);
    process.off("SIGINT", onStop);
  }
}

void main();
