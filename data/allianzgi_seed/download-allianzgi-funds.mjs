import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";

const OUT_DIR = path.resolve("out");
await fs.mkdir(OUT_DIR, { recursive: true });

const SOURCES = [
  // Luxembourg (use "facilities-services" variant: tends to be less geo-redirecty)
  { code: "LU", manco_code: "AGI_LUX", url: "https://regulatory.allianzgi.com/en-gb/facilities-services/luxemburg-en/funds/mutual-funds" },

  // UK
  { code: "GB", manco_code: "AGI_UK", url: "https://regulatory.allianzgi.com/en-gb/b2c/united-kingdom-en/funds/mutual-funds" },

  // Ireland
  { code: "IE", manco_code: "AGI_IE", url: "https://regulatory.allianzgi.com/en-ie/b2c/ireland-en/funds/mutual-funds" },

  // Germany
  { code: "DE", manco_code: "AGI_DE", url: "https://regulatory.allianzgi.com/de-de/b2c/deutschland-de/funds/mutual-funds" },

  // Switzerland (German)
  { code: "CH", manco_code: "AGI_CH", url: "https://regulatory.allianzgi.com/de-ch/b2c/schweiz-de/funds/mutual-funds" },
];

async function clickIfVisible(page, labelRegex) {
  const locator = page.getByText(labelRegex, { exact: false });
  if (await locator.count()) {
    const first = locator.first();
    if (await first.isVisible().catch(() => false)) {
      await first.click({ timeout: 3000 }).catch(() => {});
      return true;
    }
  }
  return false;
}

async function acceptGating(page) {
  // Common patterns: checkbox + Confirm / Accept
  const checkboxes = page.getByRole("checkbox");
  if (await checkboxes.count()) {
    const cb = checkboxes.first();
    if (await cb.isVisible().catch(() => false)) {
      await cb.check().catch(() => {});
    }
  }
  await clickIfVisible(page, /Confirm|Bestätigen|Accept|OK|Continue|Fortsetzen/i);
}

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();

for (const src of SOURCES) {
  console.log(`Downloading ${src.code} from ${src.url}`);
  try {
    await page.goto(src.url, { waitUntil: "domcontentloaded", timeout: 30000 });

    // handle gate(s) if present
    await acceptGating(page);
    await page.waitForTimeout(2000); // let JS settle

    // Click "Download All" and save whatever the site gives us
    const [download] = await Promise.all([
      page.waitForEvent("download", { timeout: 30000 }),
      page.getByText(/Download All|Alle herunterladen/i).click({ timeout: 20000 }),
    ]);

    const suggested = download.suggestedFilename();
    const outName = `${src.code}__${src.manco_code}__${suggested}`.replace(/[^\w.\-]+/g, "_");
    const outPath = path.join(OUT_DIR, outName);
    await download.saveAs(outPath);

    console.log(`Saved: ${outPath}`);
  } catch (err) {
    console.error(`Failed ${src.code}: ${err.message}`);
  }
}

await browser.close();
console.log("Done.");
