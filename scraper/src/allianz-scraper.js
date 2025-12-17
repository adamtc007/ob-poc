/**
 * Allianz Regulatory Site Scraper
 * 
 * Uses Playwright to scrape fund details from regulatory.allianzgi.com
 * Extracts: SFDR category, investment strategy, KID links, documents
 * 
 * Prerequisites:
 *   npm install
 *   npx playwright install chromium
 * 
 * Input: data/cssf-allianz-funds.json (from cssf-download.js)
 * Output: data/allianz-fund-details.json
 */

import { chromium } from 'playwright';
import { readFileSync, writeFileSync, existsSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const DATA_DIR = join(__dirname, '..', 'data');

// Allianz regulatory site base URLs
const BASE_URL = 'https://regulatory.allianzgi.com';
const FUND_LIST_URL = `${BASE_URL}/en-gb/b2c/luxemburg-en/funds/mutual-funds`;

// Rate limiting
const DELAY_BETWEEN_REQUESTS = 1500; // ms

/**
 * Accept cookie consent and jurisdiction disclaimer
 */
async function acceptDisclosures(page) {
  try {
    // Accept cookies if present
    const cookieButton = page.locator('button:has-text("Accept all")');
    if (await cookieButton.isVisible({ timeout: 3000 })) {
      await cookieButton.click();
      console.log('   Accepted cookies');
    }
  } catch (e) {
    // Cookie dialog not present
  }
  
  try {
    // Accept jurisdiction disclaimer
    const confirmButton = page.locator('button:has-text("Confirm")');
    if (await confirmButton.isVisible({ timeout: 3000 })) {
      await confirmButton.click();
      console.log('   Accepted jurisdiction disclaimer');
    }
  } catch (e) {
    // Disclaimer not present
  }
  
  await page.waitForTimeout(1000);
}

/**
 * Extract fund list from the main funds page
 * The page is JS-rendered, so we need to wait for data to load
 */
async function extractFundList(page) {
  console.log('📋 Extracting fund list...');
  
  await page.goto(FUND_LIST_URL, { waitUntil: 'networkidle' });
  await acceptDisclosures(page);
  
  // Wait for fund table to load
  await page.waitForTimeout(3000);
  
  // Try to find the fund table/grid
  // The structure varies, so we'll try multiple selectors
  const funds = await page.evaluate(() => {
    const results = [];
    
    // Look for fund links - they typically have ISIN or fund name patterns
    const links = document.querySelectorAll('a[href*="/funds/mutual-funds/"]');
    
    for (const link of links) {
      const href = link.getAttribute('href');
      const text = link.textContent?.trim();
      
      // Skip navigation links
      if (!text || text.length < 5) continue;
      if (href.includes('View All')) continue;
      
      results.push({
        name: text,
        url: href,
      });
    }
    
    return results;
  });
  
  console.log(`   Found ${funds.length} fund links`);
  return funds;
}

/**
 * Extract detailed info from a fund's detail page
 */
async function extractFundDetails(page, fundUrl) {
  const fullUrl = fundUrl.startsWith('http') ? fundUrl : `${BASE_URL}${fundUrl}`;
  
  await page.goto(fullUrl, { waitUntil: 'networkidle' });
  await page.waitForTimeout(1000);
  
  const details = await page.evaluate(() => {
    const result = {
      page_title: document.title,
      fund_name: null,
      share_class: null,
      isin: null,
      currency: null,
      sfdr_category: null,
      morningstar_rating: null,
      investment_objective: null,
      key_risks: [],
      documents: [],
      nav: null,
      nav_date: null,
      inception_date: null,
      management_company: null,
      benchmark: null,
    };
    
    // Fund name - usually in h1 or specific class
    const h1 = document.querySelector('h1');
    if (h1) result.fund_name = h1.textContent?.trim();
    
    // Look for key data in table rows or definition lists
    const rows = document.querySelectorAll('tr, dl, .data-row, [class*="fund-data"]');
    
    for (const row of rows) {
      const text = row.textContent?.toLowerCase() || '';
      const value = row.querySelector('td:last-child, dd, .value')?.textContent?.trim();
      
      if (text.includes('isin') && value) {
        result.isin = value.match(/[A-Z]{2}[A-Z0-9]{10}/)?.[0] || value;
      }
      if (text.includes('currency') && value) {
        result.currency = value;
      }
      if (text.includes('sfdr') && value) {
        result.sfdr_category = value;
      }
      if (text.includes('morningstar') && value) {
        result.morningstar_rating = value;
      }
      if (text.includes('inception') && value) {
        result.inception_date = value;
      }
      if (text.includes('benchmark') && value) {
        result.benchmark = value;
      }
    }
    
    // Investment objective - look for specific sections
    const objectiveSections = document.querySelectorAll('[class*="objective"], [class*="strategy"], .fund-objective');
    for (const section of objectiveSections) {
      const text = section.textContent?.trim();
      if (text && text.length > 50) {
        result.investment_objective = text.substring(0, 1000);
        break;
      }
    }
    
    // Key risks
    const riskElements = document.querySelectorAll('[class*="risk"] li, .key-risks li');
    for (const el of riskElements) {
      const text = el.textContent?.trim();
      if (text) result.key_risks.push(text);
    }
    
    // Documents (KID, Prospectus, etc.)
    const docLinks = document.querySelectorAll('a[href*=".pdf"]');
    for (const link of docLinks) {
      const href = link.getAttribute('href');
      const text = link.textContent?.trim();
      if (href && text) {
        result.documents.push({
          name: text,
          url: href.startsWith('http') ? href : `https://regulatory.allianzgi.com${href}`,
          type: text.toLowerCase().includes('kid') ? 'KID' :
                text.toLowerCase().includes('prospectus') ? 'Prospectus' :
                text.toLowerCase().includes('annual') ? 'Annual Report' :
                text.toLowerCase().includes('semi') ? 'Semi-Annual Report' :
                'Other',
        });
      }
    }
    
    // NAV
    const navElements = document.querySelectorAll('[class*="nav"], [class*="price"]');
    for (const el of navElements) {
      const text = el.textContent || '';
      const match = text.match(/[\d,]+\.\d{2,4}/);
      if (match) {
        result.nav = match[0];
        break;
      }
    }
    
    return result;
  });
  
  return details;
}

/**
 * Intercept XHR requests to find the API endpoint
 */
async function findApiEndpoint(page) {
  console.log('🔍 Looking for API endpoints...');
  
  const apiCalls = [];
  
  page.on('response', async (response) => {
    const url = response.url();
    if (url.includes('api') || url.includes('json') || url.includes('fund')) {
      const contentType = response.headers()['content-type'] || '';
      if (contentType.includes('json')) {
        apiCalls.push({
          url: url,
          status: response.status(),
        });
        console.log(`   Found API: ${url}`);
      }
    }
  });
  
  await page.goto(FUND_LIST_URL, { waitUntil: 'networkidle' });
  await acceptDisclosures(page);
  await page.waitForTimeout(5000);
  
  return apiCalls;
}

/**
 * Scrape with pagination handling
 */
async function scrapeAllFunds(browser, maxFunds = 100) {
  const page = await browser.newPage();
  const allFunds = [];
  
  try {
    // First, find if there's an API
    const apiCalls = await findApiEndpoint(page);
    if (apiCalls.length > 0) {
      console.log('\n📡 API endpoints discovered:');
      apiCalls.forEach(api => console.log(`   ${api.url}`));
    }
    
    // Get fund list from page
    const fundLinks = await extractFundList(page);
    
    // Limit for testing
    const toProcess = fundLinks.slice(0, maxFunds);
    console.log(`\n📥 Processing ${toProcess.length} funds...\n`);
    
    for (let i = 0; i < toProcess.length; i++) {
      const fund = toProcess[i];
      console.log(`[${i + 1}/${toProcess.length}] ${fund.name}`);
      
      try {
        const details = await extractFundDetails(page, fund.url);
        allFunds.push({
          ...fund,
          ...details,
          scraped_at: new Date().toISOString(),
        });
        console.log(`   ✓ ISIN: ${details.isin || 'N/A'}, SFDR: ${details.sfdr_category || 'N/A'}`);
      } catch (error) {
        console.log(`   ✗ Error: ${error.message}`);
        allFunds.push({
          ...fund,
          error: error.message,
          scraped_at: new Date().toISOString(),
        });
      }
      
      // Rate limiting
      await page.waitForTimeout(DELAY_BETWEEN_REQUESTS);
    }
    
  } finally {
    await page.close();
  }
  
  return allFunds;
}

/**
 * Main execution
 */
async function main() {
  console.log('═══════════════════════════════════════════════');
  console.log('  Allianz Regulatory Site Scraper');
  console.log('═══════════════════════════════════════════════\n');
  
  // Parse command line args
  const args = process.argv.slice(2);
  const maxFunds = args.includes('--all') ? 9999 : 
                   parseInt(args.find(a => a.startsWith('--max='))?.split('=')[1]) || 20;
  
  console.log(`Max funds to scrape: ${maxFunds}`);
  console.log('(Use --all for all funds, --max=N for specific limit)\n');
  
  const browser = await chromium.launch({
    headless: true, // Set to false to see browser
  });
  
  try {
    const funds = await scrapeAllFunds(browser, maxFunds);
    
    // Save results
    const output = {
      source: 'regulatory.allianzgi.com',
      scraped_at: new Date().toISOString(),
      jurisdiction: 'Luxembourg',
      total_funds: funds.length,
      successful: funds.filter(f => !f.error).length,
      failed: funds.filter(f => f.error).length,
      funds: funds,
    };
    
    const outputPath = join(DATA_DIR, 'allianz-fund-details.json');
    writeFileSync(outputPath, JSON.stringify(output, null, 2));
    console.log(`\n✅ Saved: ${outputPath}`);
    
    // Summary
    console.log('\n═══════════════════════════════════════════════');
    console.log('  Summary');
    console.log('═══════════════════════════════════════════════');
    console.log(`  Total scraped:  ${funds.length}`);
    console.log(`  Successful:     ${output.successful}`);
    console.log(`  Failed:         ${output.failed}`);
    console.log('═══════════════════════════════════════════════\n');
    
    // SFDR breakdown
    const sfdrCounts = {};
    for (const fund of funds) {
      const cat = fund.sfdr_category || 'Unknown';
      sfdrCounts[cat] = (sfdrCounts[cat] || 0) + 1;
    }
    console.log('SFDR Categories:');
    for (const [cat, count] of Object.entries(sfdrCounts)) {
      console.log(`  ${cat}: ${count}`);
    }
    
  } finally {
    await browser.close();
  }
}

main().catch(console.error);
