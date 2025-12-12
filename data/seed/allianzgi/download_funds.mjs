/**
 * AllianzGI Fund Data Downloader
 * ============================================================================
 * Playwright script to download "Download All" CSV exports from AllianzGI's
 * regulatory fund explorer pages.
 *
 * Usage:
 *   npm init -y
 *   npm i -D playwright
 *   npx playwright install chromium
 *   node download_funds.mjs
 *
 * Output: ./out/*.csv (or xlsx depending on what AllianzGI exports)
 * ============================================================================
 */

import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";

const OUT_DIR = path.resolve("out");
await fs.mkdir(OUT_DIR, { recursive: true });

// AllianzGI regulatory fund list pages with "Download All" capability
const SOURCES = [
  // Luxembourg (use "facilities-services" variant - less geo-redirect issues)
  {
    code: "LU",
    manco_code: "AGI_LUX",
    url: "https://regulatory.allianzgi.com/en-gb/facilities-services/luxemburg-en/funds/mutual-funds",
    expected_funds: "~206 funds / ~1972 share classes",
  },

  // United Kingdom
  {
    code: "GB",
    manco_code: "AGI_UK",
    url: "https://regulatory.allianzgi.com/en-gb/b2c/united-kingdom-en/funds/mutual-funds",
    expected_funds: "~70 funds / ~242 share classes",
  },

  // Ireland
  {
    code: "IE",
    manco_code: "AGI_IE",
    url: "https://regulatory.allianzgi.com/en-ie/b2c/ireland-en/funds/mutual-funds",
    expected_funds: "~50 funds / ~780 share classes",
  },

  // Germany
  {
    code: "DE",
    manco_code: "AGI_DE",
    url: "https://regulatory.allianzgi.com/de-de/b2c/deutschland-de/funds/mutual-funds",
    expected_funds: "German-domiciled funds",
  },

  // Switzerland (German language)
  {
    code: "CH",
    manco_code: "AGI_CH",
    url: "https://regulatory.allianzgi.com/de-ch/b2c/schweiz-de/funds/mutual-funds",
    expected_funds: "Swiss-registered funds",
  },
];

/**
 * Click an element if it's visible on the page
 */
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

/**
 * Handle investor type gate / cookie consent
 */
async function acceptGating(page) {
  // Common patterns: checkbox + Confirm / Accept
  const checkboxes = page.getByRole("checkbox");
  if (await checkboxes.count()) {
    const cb = checkboxes.first();
    if (await cb.isVisible().catch(() => false)) {
      await cb.check().catch(() => {});
    }
  }
  // Click any confirm/accept buttons
  await clickIfVisible(
    page,
    /Confirm|Bestätigen|Accept|OK|Continue|Fortsetzen/i,
  );

  // Wait a moment for any animations/redirects
  await page.waitForTimeout(1000);
}

/**
 * Wait for fund table to load (typically lazy-loaded via JS)
 */
async function waitForFundTable(page) {
  // Most AllianzGI pages load the fund table with a small delay
  // Look for common table indicators
  try {
    await page.waitForSelector('table, [class*="fund"], [class*="list"]', {
      timeout: 10000,
    });
    await page.waitForTimeout(2000); // Let the full list populate
  } catch (e) {
    console.log("  Warning: Could not detect fund table - proceeding anyway");
  }
}

// Main execution
console.log("AllianzGI Fund Data Downloader");
console.log("==============================\n");

const browser = await chromium.launch({
  headless: false, // Run visible so user can manually accept popups
  slowMo: 100, // Slow down actions for visibility
});

const results = [];

for (const src of SOURCES) {
  console.log(`[${src.code}] Downloading from ${src.manco_code}...`);
  console.log(`    URL: ${src.url}`);
  console.log(`    Expected: ${src.expected_funds}`);

  const page = await browser.newPage();

  try {
    await page.goto(src.url, { waitUntil: "domcontentloaded", timeout: 30000 });

    // Handle investor type gating/consent
    await acceptGating(page);

    // Give user time to manually handle any popups (15 seconds)
    console.log(`    ⏳ Waiting 15s for manual popup handling...`);
    await page.waitForTimeout(15000);

    // Scroll down to make Download All visible
    await page.evaluate(() =>
      window.scrollTo(0, document.body.scrollHeight / 2),
    );
    await page.waitForTimeout(1000);

    // Wait for fund table to load
    await waitForFundTable(page);

    // Look for and click "Download All" button
    const downloadButton = page.getByText(/Download All/i).first();

    if ((await downloadButton.count()) === 0) {
      console.log(`    ⚠️  No "Download All" button found - skipping`);
      results.push({ ...src, status: "NO_BUTTON" });
      await page.close();
      continue;
    }

    // Use JavaScript to find and click the download link directly
    console.log(`    🖱️  Clicking Download All via JavaScript...`);

    // Trigger download using evaluate to click the element directly
    const [download] = await Promise.all([
      page.waitForEvent("download", { timeout: 30000 }),
      page.evaluate(() => {
        // Find all links/buttons containing "Download All"
        const elements = [...document.querySelectorAll("a, button, span")];
        const downloadEl = elements.find(
          (el) => el.textContent && el.textContent.includes("Download All"),
        );
        if (downloadEl) {
          // Find the actual clickable parent (usually an <a> tag)
          const clickable =
            downloadEl.closest("a") ||
            downloadEl.closest("button") ||
            downloadEl;
          clickable.click();
          return true;
        }
        return false;
      }),
    ]);

    // Save the file
    const suggested = download.suggestedFilename();
    const outName = `${src.code}__${src.manco_code}__${suggested}`.replace(
      /[^\w.\-]+/g,
      "_",
    );
    const outPath = path.join(OUT_DIR, outName);
    await download.saveAs(outPath);

    console.log(`    ✅ Saved: ${outPath}`);
    results.push({ ...src, status: "SUCCESS", file: outPath });
  } catch (error) {
    console.log(`    ❌ Error: ${error.message}`);
    results.push({ ...src, status: "ERROR", error: error.message });
  }

  await page.close();
}

await browser.close();

// Print summary
console.log("\n==============================");
console.log("Download Summary:");
console.log("==============================\n");

for (const r of results) {
  const icon = r.status === "SUCCESS" ? "✅" : "❌";
  console.log(`${icon} [${r.code}] ${r.manco_code}: ${r.status}`);
  if (r.file) console.log(`      → ${r.file}`);
  if (r.error) console.log(`      → ${r.error}`);
}

// Write results to JSON for downstream processing
const resultsPath = path.join(OUT_DIR, "download_results.json");
await fs.writeFile(resultsPath, JSON.stringify(results, null, 2));
console.log(`\nResults saved to: ${resultsPath}`);

console.log("\nNext steps:");
console.log("1. Check the ./out/ directory for CSV/XLSX files");
console.log("2. Use csv_to_dsl.py to convert to DSL commands");
console.log("3. Execute the generated DSL to load into CBU");
