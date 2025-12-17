/**
 * Allianz API Discovery & Extraction
 * 
 * Uses Playwright to intercept the internal fund data API
 * Discovered: https://regulatory.allianzgi.com/en-GB/api/funddata/funds/...
 * 
 * This approach is faster and more reliable than HTML scraping
 */

import { chromium } from 'playwright';
import { writeFileSync, mkdirSync, existsSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const DATA_DIR = join(__dirname, '..', 'data');
const BASE_URL = 'https://regulatory.allianzgi.com';
const FUND_LIST_URL = `${BASE_URL}/en-gb/b2c/luxemburg-en/funds/mutual-funds`;

/**
 * Intercept all fund data API responses
 */
async function interceptFundApi(browser) {
  const page = await browser.newPage();
  const apiResponses = [];
  
  // Intercept API responses
  page.on('response', async (response) => {
    const url = response.url();
    
    // Capture fund data API calls
    if (url.includes('/api/funddata/') || url.includes('/api/fund')) {
      try {
        const contentType = response.headers()['content-type'] || '';
        if (contentType.includes('json')) {
          const data = await response.json();
          apiResponses.push({
            url: url,
            status: response.status(),
            data: data,
            captured_at: new Date().toISOString(),
          });
          console.log(`   📡 Captured: ${url.substring(0, 80)}...`);
        }
      } catch (e) {
        // Skip non-JSON responses
      }
    }
  });
  
  return { page, apiResponses };
}

/**
 * Navigate through the fund list to trigger API calls
 */
async function triggerFundApis(page, maxFunds = 50) {
  console.log('📋 Loading fund list page...');
  await page.goto(FUND_LIST_URL, { waitUntil: 'networkidle' });
  
  // Accept cookies
  try {
    const cookieBtn = page.locator('button:has-text("Accept all")');
    if (await cookieBtn.isVisible({ timeout: 3000 })) {
      await cookieBtn.click();
      console.log('   Accepted cookies');
    }
  } catch (e) {}
  
  // Accept jurisdiction
  try {
    const confirmBtn = page.locator('button:has-text("Confirm")');
    if (await confirmBtn.isVisible({ timeout: 3000 })) {
      await confirmBtn.click();
      console.log('   Accepted jurisdiction');
    }
  } catch (e) {}
  
  await page.waitForTimeout(3000);
  
  // Find and click on fund links to trigger API calls
  console.log('🔍 Finding fund links...');
  
  const fundLinks = await page.evaluate(() => {
    const links = [];
    document.querySelectorAll('a[href*="/funds/mutual-funds/"]').forEach(a => {
      const href = a.getAttribute('href');
      const text = a.textContent?.trim();
      if (href && text && text.length > 3 && !href.includes('View All')) {
        links.push({ url: href, name: text });
      }
    });
    // Remove duplicates
    return [...new Map(links.map(l => [l.url, l])).values()];
  });
  
  console.log(`   Found ${fundLinks.length} unique fund links`);
  
  // Visit each fund page to trigger API calls
  const toVisit = fundLinks.slice(0, maxFunds);
  console.log(`\n📥 Visiting ${toVisit.length} fund pages to capture API data...\n`);
  
  for (let i = 0; i < toVisit.length; i++) {
    const link = toVisit[i];
    const fullUrl = link.url.startsWith('http') ? link.url : `${BASE_URL}${link.url}`;
    
    console.log(`[${i + 1}/${toVisit.length}] ${link.name.substring(0, 50)}`);
    
    try {
      await page.goto(fullUrl, { waitUntil: 'networkidle', timeout: 30000 });
      await page.waitForTimeout(1500);
    } catch (e) {
      console.log(`   ⚠ Timeout/error, continuing...`);
    }
  }
}

/**
 * Parse captured API data into fund structure
 */
function parseApiResponses(apiResponses) {
  const funds = new Map();
  
  for (const response of apiResponses) {
    const data = response.data;
    
    // Skip non-fund data
    if (!data || typeof data !== 'object') continue;
    
    // The API returns fund details - extract what we can
    // Structure varies, so we try multiple paths
    const fundData = data.fund || data.shareClass || data;
    
    if (fundData.isin || fundData.name || fundData.fundName) {
      const key = fundData.isin || fundData.id || response.url;
      
      funds.set(key, {
        // Identifiers
        isin: fundData.isin || null,
        fund_id: fundData.id || fundData.fundId || null,
        
        // Names
        fund_name: fundData.fundName || fundData.name || null,
        share_class_name: fundData.shareClassName || fundData.shareClass || null,
        
        // Key data
        currency: fundData.currency || fundData.baseCurrency || null,
        nav: fundData.nav || fundData.netAssetValue || null,
        nav_date: fundData.navDate || fundData.priceDate || null,
        
        // Regulatory
        sfdr_category: fundData.sfdrClassification || fundData.sfdr || fundData.sustainabilityRating || null,
        morningstar_rating: fundData.morningstarRating || null,
        
        // Investment
        investment_objective: fundData.investmentObjective || fundData.objective || null,
        benchmark: fundData.benchmark || fundData.benchmarkName || null,
        asset_class: fundData.assetClass || null,
        
        // Management
        management_company: fundData.managementCompany || fundData.manager || null,
        launch_date: fundData.launchDate || fundData.inceptionDate || null,
        
        // Documents
        documents: fundData.documents || [],
        
        // Raw for debugging
        _api_url: response.url,
        _raw: fundData,
      });
    }
  }
  
  return Array.from(funds.values());
}

/**
 * Main execution
 */
async function main() {
  console.log('═══════════════════════════════════════════════');
  console.log('  Allianz API Interceptor');
  console.log('═══════════════════════════════════════════════\n');
  
  if (!existsSync(DATA_DIR)) {
    mkdirSync(DATA_DIR, { recursive: true });
  }
  
  const args = process.argv.slice(2);
  const maxFunds = args.includes('--all') ? 9999 : 
                   parseInt(args.find(a => a.startsWith('--max='))?.split('=')[1]) || 30;
  
  console.log(`Max funds to visit: ${maxFunds}`);
  console.log('(Use --all for all funds, --max=N for specific limit)\n');
  
  const browser = await chromium.launch({ headless: true });
  
  try {
    const { page, apiResponses } = await interceptFundApi(browser);
    
    await triggerFundApis(page, maxFunds);
    
    console.log(`\n📊 Captured ${apiResponses.length} API responses`);
    
    // Save raw API responses
    const apiPath = join(DATA_DIR, 'allianz-api-raw.json');
    writeFileSync(apiPath, JSON.stringify(apiResponses, null, 2));
    console.log(`✅ Saved raw API data: ${apiPath}`);
    
    // Parse into fund structure
    const funds = parseApiResponses(apiResponses);
    console.log(`📁 Parsed ${funds.length} unique funds`);
    
    // Save parsed data
    const output = {
      source: 'regulatory.allianzgi.com API',
      scraped_at: new Date().toISOString(),
      api_calls_captured: apiResponses.length,
      funds_parsed: funds.length,
      funds: funds,
    };
    
    const outputPath = join(DATA_DIR, 'allianz-fund-details.json');
    writeFileSync(outputPath, JSON.stringify(output, null, 2));
    console.log(`✅ Saved parsed data: ${outputPath}`);
    
    // Summary
    console.log('\n═══════════════════════════════════════════════');
    console.log('  Summary');
    console.log('═══════════════════════════════════════════════');
    console.log(`  API calls captured: ${apiResponses.length}`);
    console.log(`  Unique funds:       ${funds.length}`);
    
    // SFDR breakdown
    const sfdrCounts = {};
    for (const fund of funds) {
      const cat = fund.sfdr_category || 'Unknown';
      sfdrCounts[cat] = (sfdrCounts[cat] || 0) + 1;
    }
    console.log('\nSFDR Categories:');
    for (const [cat, count] of Object.entries(sfdrCounts)) {
      console.log(`  ${cat}: ${count}`);
    }
    
    // Sample output
    if (funds.length > 0) {
      console.log('\nSample fund:');
      const sample = funds.find(f => f.isin) || funds[0];
      console.log(JSON.stringify({
        isin: sample.isin,
        fund_name: sample.fund_name,
        currency: sample.currency,
        sfdr_category: sample.sfdr_category,
        management_company: sample.management_company,
      }, null, 2));
    }
    
  } finally {
    await browser.close();
  }
}

main().catch(console.error);
