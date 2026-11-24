// Diagnostic only: run as Node using the already checksum-pinned T3 Electron binary.
// Read the synthetic renderer; emit counts only. Native screenshots stay the gate.
"use strict";

const path = require("node:path");
const fs = require("node:fs");
const { setTimeout: delay } = require("node:timers/promises");

async function main() {
  let app;
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
    const summary = await page.evaluate(({ user, assistant }) => {
      const rows = [...document.querySelectorAll('[data-timeline-row-kind="message"]')];
      const count = (id, phrase) => {
        const row = rows.find((element) => element.dataset.messageId === id);
        if (!row) return { row: false, phrase: false, intersectsViewport: false };
        const rect = row.getBoundingClientRect();
        const viewport = row.closest(".messages-timeline-scroll")?.getBoundingClientRect();
        return {
          row: true,
          phrase: row.innerText.includes(phrase),
          intersectsViewport:
            rect.width > 0 && rect.height > 0 &&
            rect.right > Math.max(0, viewport?.left ?? 0) &&
            rect.bottom > Math.max(0, viewport?.top ?? 0) &&
            rect.left < Math.min(innerWidth, viewport?.right ?? innerWidth) &&
            rect.top < Math.min(innerHeight, viewport?.bottom ?? innerHeight),
        };
      };
      return {
        loading: document.querySelectorAll('[data-timeline-loading="true"]').length,
        rows: rows.length,
        user: count("launch-check-user", user),
        assistant: count("launch-check-assistant", assistant),
      };
    }, { user, assistant });
    report({ stage: "complete", failures, ...summary });
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
  }
}

void main();
