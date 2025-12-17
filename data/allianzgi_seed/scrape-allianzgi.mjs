/**
 * Allianz Global Investors - Comprehensive Fund Scraper
 * 
 * Scrapes fund data including:
 * - Fund list with share classes
 * - Detail pages (investment objectives, mandates, SFDR classification)
 * - Document links (KID, Prospectus, Annual Reports)
 * - Management company information
 * 
 * Usage:
 *   npm install
 *   node scrape-allianzgi.mjs [--region LU] [--details] [--docs]
 * 
 * Output: data/external/allianzgi/<region>_comprehensive.json
 */

import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";

const __dirname = path.dirname(new URL(import.meta.url).pathname);
const OUT_DIR = path.resolve(__dirname, "../external/allianzgi");
await fs.mkdir(OUT_DIR, { recursive: true });

// Parse command line args
const args = process.argv.slice(2);
const FETCH_DETAILS = args.includes("--details");
const FETCH_DOCS = args.includes("--docs");
const regionArg = args.find((a, i) => args[i - 1] === "--region");
const SELECTED_REGION = regionArg?.toUpperCase();

const SOURCES = [
  {
    code: "LU",
    name: "Luxembourg",
    manco: {
      name: "Allianz Global Investors GmbH",
      branch: "Luxembourg Branch", 
      jurisdiction: "LU",
      regulator: "CSSF",
      lei: "529900LMMFP4CM8ZOO35"
    },
    listUrl: "https://regulatory.allianzgi.com/en-gb/facilities-services/luxemburg-en/funds/mutual-funds",
    baseUrl: "https://regulatory.allianzgi.com",
    detailPrefix: "/en-gb/facilities-services/luxemburg-en/funds/mutual-funds/"
  },
  {
    code: "IE", 
    name: "Ireland",
    manco: {
      name: "Allianz Global Investors Ireland Limited",
      jurisdiction: "IE",
      regulator: "CBI"
    },
    listUrl: "https://regulatory.allianzgi.com/en-ie/b2c/ireland-en/funds/mutual-funds",
    baseUrl: "https://regulatory.allianzgi.com",
    detailPrefix: "/en-ie/b2c/ireland-en/funds/mutual-funds/"
  },
  {
    code: "GB",
    name: "United Kingdom", 
    manco: {
      name: "Allianz Global Investors UK Limited",
      jurisdiction: "GB",
      regulator: "FCA"
    },
    listUrl: "https://regulatory.allianzgi.com/en-gb/b2c/united-kingdom-en/funds/mutual-funds",
    baseUrl: "https://regulatory.allianzgi.com",
    detailPrefix: "/en-gb/b2c/united-kingdom-en/funds/mutual-funds/"
  },
  {
    code: "DE",
    name: "Germany",
    manco: {
      name: "Allianz Global Investors GmbH",
      jurisdiction: "DE", 
      regulator: "BaFin"
    },
    listUrl: "https://regulatory.allianzgi.com/de-de/b2c/deutschland-de/funds/mutual-funds",
    baseUrl: "https://regulatory.allianzgi.com",
    detailPrefix: "/de-de/b2c/deutschland-de/funds/mutual-funds/"
  },
  {
    code: "CH",
    name: "Switzerland",
    manco: {
      name: "Allianz Global Investors (Schweiz) AG",
      jurisdiction: "CH",
      regulator: "FINMA"
    },
    listUrl: "https://regulatory.allianzgi.com/de-ch/b2c/schweiz-de/funds/mutual-funds",
    baseUrl: "https://regulatory.allianzgi.com",
    detailPrefix: "/de-ch/b2c/schweiz-de/funds/mutual-funds/"
  }
];

// Filter to selected region if specified
const ACTIVE_SOURCES = SELECTED_REGION 
  ? SOURCES.filter(s => s.code === SELECTED_REGION)
  : SOURCES;

if (ACTIVE_SOURCES.length === 0) {
  console.error(`Unknown region: ${SELECTED_REGION}`);
  console.error(`Available: ${SOURCES.map(s => s.code).join(", ")}`);
  process.exit(1);
}


// ============================================================================
// Browser Setup & Cookie Acceptance
// ============================================================================

async function acceptCookiesAndDisclaimer(page) {
  // Accept cookies if present
  try {
    const cookieBtn = page.locator('button:has-text("Accept"), button:has-text("Akzeptieren")');
    if (await cookieBtn.count() > 0 && await cookieBtn.first().isVisible({ timeout: 2000 })) {
      await cookieBtn.first().click();
      await page.waitForTimeout(500);
    }
  } catch {}

  // Handle jurisdiction disclaimer checkbox + confirm
  try {
    const checkbox = page.locator('input[type="checkbox"]');
    if (await checkbox.count() > 0) {
      const cb = checkbox.first();
      if (await cb.isVisible({ timeout: 2000 })) {
        await cb.check();
        await page.waitForTimeout(300);
      }
    }
    
    const confirmBtn = page.locator('button:has-text("Confirm"), button:has-text("Bestätigen"), a:has-text("Confirm")');
    if (await confirmBtn.count() > 0 && await confirmBtn.first().isVisible({ timeout: 2000 })) {
      await confirmBtn.first().click();
      await page.waitForTimeout(1000);
    }
  } catch {}
}

// ============================================================================
// Fund List Extraction
// ============================================================================

async function fetchFundListApi(page, source) {
  // The site loads fund data via an API call - intercept it
  let fundData = null;
  
  page.on('response', async (response) => {
    const url = response.url();
    if (url.includes('/api/funddata/funds/') || url.includes('FundList')) {
      try {
        const json = await response.json();
        if (json.FundList) {
          fundData = json;
        }
      } catch {}
    }
  });

  await page.goto(source.listUrl, { waitUntil: "networkidle", timeout: 60000 });
  await acceptCookiesAndDisclaimer(page);
  await page.waitForTimeout(3000); // Let API calls complete
  
  // If we didn't intercept, try to find embedded data
  if (!fundData) {
    // Try to find data-context-url and fetch directly
    const contextUrl = await page.evaluate(() => {
      const el = document.querySelector('[data-context-url]');
      return el?.getAttribute('data-context-url');
    });
    
    if (contextUrl) {
      const fullUrl = new URL(contextUrl, source.baseUrl).href;
      const response = await page.request.get(fullUrl);
      fundData = await response.json();
    }
  }
  
  // Fallback: try the download button approach
  if (!fundData) {
    console.log(`  Trying download fallback for ${source.code}...`);
    // ... existing download logic could go here
  }
  
  return fundData;
}

// ============================================================================
// Detail Page Scraping
// ============================================================================

async function fetchFundDetails(page, source, fundSlug) {
  const detailUrl = `${source.baseUrl}${source.detailPrefix}${fundSlug}`;
  
  try {
    await page.goto(detailUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
    await acceptCookiesAndDisclaimer(page);
    await page.waitForTimeout(2000);
    
    const details = await page.evaluate(() => {
      const getText = (selector) => {
        const el = document.querySelector(selector);
        return el?.textContent?.trim() || null;
      };
      
      const getMultiple = (selector) => {
        return Array.from(document.querySelectorAll(selector))
          .map(el => el.textContent?.trim())
          .filter(Boolean);
      };
      
      // Extract structured data from the page
      return {
        investmentObjective: getText('.investment-objective, .fund-objective, [data-testid="objective"]'),
        investmentPolicy: getText('.investment-policy, .fund-policy'),
        riskProfile: getText('.risk-profile, .risk-indicator'),
        
        // SFDR / Sustainability
        sfdrClassification: getText('.sfdr-category, [data-testid="sfdr"]'),
        sustainabilityApproach: getText('.sustainability-approach'),
        
        // Management
        portfolioManager: getText('.portfolio-manager, .fund-manager'),
        managementCompany: getText('.management-company'),
        custodian: getText('.custodian, .depositary'),
        
        // Fees
        ongoingCharges: getText('.ongoing-charges, .ter'),
        managementFee: getText('.management-fee'),
        
        // Documents (collect links)
        documents: getMultiple('a[href*=".pdf"]').map(text => text),
        
        // Benchmark
        benchmark: getText('.benchmark'),
        
        // Raw page title as fallback
        pageTitle: document.title
      };
    });
    
    return details;
  } catch (err) {
    console.error(`    Failed to fetch details for ${fundSlug}: ${err.message}`);
    return null;
  }
}


// ============================================================================
// Data Normalization
// ============================================================================

function normalizeShareClass(raw, source) {
  return {
    isin: raw.Isin,
    wkn: raw.Wkn || null,
    shareClassName: raw.ShareClass,
    currency: raw.ShareclassCurrency,
    nav: parseFloat(raw.Nav) || null,
    navDate: raw.AsOfDate?.replace("as of ", "") || null,
    launchDate: raw.LaunchDate?.split(" ")[0] || null,
    
    // Performance
    performance: {
      ytd: parseFloat(raw.YTD) || null,
      oneYear: parseFloat(raw.OneYear) || null,
      threeYear: parseFloat(raw.ThreeYear) || null,
      fiveYear: parseFloat(raw.FiveYear) || null
    },
    
    // Internal IDs
    _fundId: raw.FundId,
    _shareClassId: raw.ScId,
    _detailSlug: raw.viewdetail
  };
}

function normalizeFund(shareClasses, source) {
  const first = shareClasses[0];
  
  return {
    // Fund identification
    fundName: first.FundName,
    fundId: first.FundId,
    
    // Classification
    assetClass: first.AssetClass,
    legalStructure: first.LegalStructure, // SICAV, FCP, etc.
    jurisdiction: source.code,
    
    // Regulatory
    sfdrCategory: first.EuSfdrSustainabilityCategory, // Article 6, 8, 9
    morningstarRating: first.MorningstarRating ? parseInt(first.MorningstarRating) : null,
    
    // Management Company
    managementCompany: source.manco,
    
    // Share classes
    shareClasses: shareClasses.map(sc => normalizeShareClass(sc, source)),
    
    // Metadata
    _detailSlug: first.viewdetail,
    _source: source.code,
    _scrapedAt: new Date().toISOString()
  };
}

function groupByFund(fundList) {
  const byFund = new Map();
  
  for (const item of fundList) {
    const fundName = item.FundName;
    if (!byFund.has(fundName)) {
      byFund.set(fundName, []);
    }
    byFund.get(fundName).push(item);
  }
  
  return byFund;
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  console.log("=".repeat(60));
  console.log("Allianz Global Investors Fund Scraper");
  console.log("=".repeat(60));
  console.log(`Regions: ${ACTIVE_SOURCES.map(s => s.code).join(", ")}`);
  console.log(`Fetch details: ${FETCH_DETAILS}`);
  console.log(`Fetch documents: ${FETCH_DOCS}`);
  console.log("");

  const browser = await chromium.launch({ 
    headless: true,
    args: ['--disable-blink-features=AutomationControlled']
  });
  
  const context = await browser.newContext({
    userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
  });
  
  const page = await context.newPage();

  for (const source of ACTIVE_SOURCES) {
    console.log(`\n[${ source.code }] ${source.name}`);
    console.log("-".repeat(40));
    
    try {
      // 1. Fetch fund list
      console.log("  Fetching fund list...");
      const fundData = await fetchFundListApi(page, source);
      
      if (!fundData?.FundList) {
        console.error(`  ERROR: No fund data found for ${source.code}`);
        continue;
      }
      
      const fundList = fundData.FundList;
      console.log(`  Found ${fundList.length} share classes`);
      
      // 2. Group by fund
      const fundGroups = groupByFund(fundList);
      console.log(`  ${fundGroups.size} unique funds`);
      
      // 3. Normalize
      const funds = [];
      for (const [fundName, shareClasses] of fundGroups) {
        const fund = normalizeFund(shareClasses, source);
        funds.push(fund);
      }
      
      // 4. Optionally fetch details for each fund
      if (FETCH_DETAILS) {
        console.log("  Fetching fund details...");
        let count = 0;
        for (const fund of funds) {
          if (fund._detailSlug) {
            const details = await fetchFundDetails(page, source, fund._detailSlug);
            if (details) {
              fund.details = details;
            }
            count++;
            if (count % 10 === 0) {
              console.log(`    ${count}/${funds.length} details fetched`);
            }
            // Rate limit
            await page.waitForTimeout(500);
          }
        }
        console.log(`    ${count} detail pages scraped`);
      }
      
      // 5. Build output
      const output = {
        metadata: {
          source: source.code,
          sourceName: source.name,
          scrapedAt: new Date().toISOString(),
          fundCount: funds.length,
          shareClassCount: fundList.length,
          managementCompany: source.manco
        },
        funds: funds
      };
      
      // 6. Save
      const outPath = path.join(OUT_DIR, `${source.code.toLowerCase()}_comprehensive.json`);
      await fs.writeFile(outPath, JSON.stringify(output, null, 2));
      console.log(`  Saved: ${outPath}`);
      
    } catch (err) {
      console.error(`  ERROR: ${err.message}`);
      console.error(err.stack);
    }
  }

  await browser.close();
  console.log("\nDone.");
}

main().catch(console.error);
