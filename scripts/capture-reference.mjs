import { createRequire } from "node:module";
import { resolve } from "node:path";

const [referenceDir, url, selector, width, height, output] = process.argv.slice(2);
const require = createRequire(resolve(referenceDir, "apps/web/package.json"));
const { chromium } = require("playwright");

const browser = await chromium.launch(
  process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
);
try {
  const page = await browser.newPage({
    viewport: { width: Number(width), height: Number(height) },
    deviceScaleFactor: 1,
    colorScheme: process.env.UI_THEME ?? "dark",
    storageState: process.env.T3_STORAGE_STATE || undefined,
  });
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.locator(selector).waitFor({ state: "visible" });
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await page.screenshot({ path: output, animations: "disabled" });
} finally {
  await browser.close();
}
