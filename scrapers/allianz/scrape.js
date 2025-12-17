/**
 * Allianz Fund Scraper v3
 * 
 * Features:
 * - API interception for fund list
 * - Fund detail page scraping for investment objectives
 * - Multi-jurisdiction support (LU, IE, DE, UK)
 * 
 * Usage:
 *   node scrape.js                          # Luxembourg funds
 *   node scrape.js --jurisdiction=ie        # Ireland funds  
 *   node scrape.js --details                # Include fund detail pages
 *   node scrape.js --limit=10 --details     # First 10 with details
 */

import { chromium } from 'playwright';
import { program } from 'commander';
import fs from 'fs/promises';
import path from 'path';

// Jurisdiction configs - CORRECT URLs
const JURISDICTIONS = {
  lu: {
    name: 'Luxembourg',
    baseUrl: 'https://regulatory.allianzgi.com',
    // Note: en-gb prefix for LU site
    fundListPath: '/en-gb/b2c/luxemburg-en/funds/mutual-funds',
    detailPrefix: '/en-gb/b2c/luxemburg-en/funds/mutual-funds/',
    manco: 'Allianz Global Investors GmbH',
    mancoJurisdiction: 'DE',
    apiGuid: 'b664c028-37ee-422b-b2c5-8b7155bacee3'
  },
  ie: {
    name: 'Ireland',
    baseUrl: 'https://regulatory.allianzgi.com',
    // Note: en-ie prefix for Ireland site
    fundListPath: '/en-ie/b2c/ireland-en/funds/mutual-funds',
    detailPrefix: '/en-ie/b2c/ireland-en/funds/mutual-funds/',
    manco: 'Allianz Global Investors Ireland Limited',
    mancoJurisdiction: 'IE',
    apiGuid: null  // Will capture from page
  },
  de: {
    name: 'Germany',
    baseUrl: 'https://regulatory.allianzgi.com',
    fundListPath: '/en-gb/b2c/germany-en/funds/mutual-funds',
    detailPrefix: '/en-gb/b2c/germany-en/funds/mutual-funds/',
    manco: 'Allianz Global Investors GmbH',
    mancoJurisdiction: 'DE',
    apiGuid: null
  },
  uk: {
    name: 'United Kingdom',
    baseUrl: 'https://regulatory.allianzgi.com',
    fundListPath: '/en-gb/b2c/united-kingdom-en/funds/mutual-funds',
    detailPrefix: '/en-gb/b2c/united-kingdom-en/funds/mutual-funds/',
    manco: 'Allianz Global Investors UK Limited',
    mancoJurisdiction: 'GB',
    apiGuid: null
  }
};

program
  .option('-j, --jurisdiction <code>', 'Jurisdiction code (lu, ie, de, uk)', 'lu')
  .option('-o, --output <path>', 'Output directory', './output')
  .option('-l, --limit <number>', 'Limit number of funds for detail scrape', parseInt)
  .option('-d, --details', 'Scrape fund detail pages for investment objectives', false)
  .option('--debug', 'Enable debug logging', false)
  .parse();

const opts = program.opts();
const config = JURISDICTIONS[opts.jurisdiction];

if (!config) {
  console.error(`Unknown jurisdiction: ${opts.jurisdiction}`);
  console.error(`Available: ${Object.keys(JURISDICTIONS).join(', ')}`);
  process.exit(1);
}

console.log(`\n🏦 Allianz Fund Scraper v3`);
console.log(`   Jurisdiction: ${config.name} (${opts.jurisdiction.toUpperCase()})`);
console.log(`   ManCo: ${config.manco}`);
if (opts.details) console.log(`   Detail scraping: ENABLED`);
if (opts.limit) console.log(`   Limit: ${opts.limit} funds`);
console.log('');


async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'
  });
  const page = await context.newPage();
  
  // Capture API responses
  let fundData = null;
  
  page.on('response', async (response) => {
    const url = response.url();
    if (url.includes('/api/funddata/funds/')) {
      try {
        const data = await response.json();
        if (data.FundList || data.funds) {
          fundData = data;
          console.log(`📡 Captured fund API (${(data.FundList || data.funds || []).length} entries)`);
        }
      } catch (e) {}
    }
  });
  
  try {
    // Load fund list page
    const listUrl = `${config.baseUrl}${config.fundListPath}`;
    console.log(`📋 Loading: ${listUrl}`);
    
    await page.goto(listUrl, { waitUntil: 'networkidle', timeout: 60000 });
    
    // Accept disclaimer if present
    try {
      await page.click('button:has-text("Confirm")', { timeout: 5000 });
      console.log('   ✓ Accepted disclaimer');
      await page.waitForTimeout(3000);
    } catch (e) {}
    
    // Wait for data
    await page.waitForTimeout(5000);
    
    if (!fundData) {
      console.log('   Waiting for API response...');
      await page.waitForTimeout(5000);
    }
    
    if (!fundData) {
      throw new Error('Failed to capture fund data from API. Check if URL is correct.');
    }
    
    // Process fund list
    console.log(`\n📊 Processing fund list...`);
    
    const shareClassMap = new Map();
    const fundList = fundData.FundList || fundData.funds || [];
    
    fundList.forEach(row => {
      const fundName = row.FundName || row.fundName;
      if (!fundName) return;
      
      if (!shareClassMap.has(fundName)) {
        shareClassMap.set(fundName, {
          name: fundName,
          fundId: row.FundId,
          assetClass: row.AssetClass,
          legalStructure: row.LegalStructure || 'SICAV',
          sfdr: row.EuSfdrSustainabilityCategory,
          detailSlug: row.viewdetail,
          shareClasses: []
        });
      }
      
      shareClassMap.get(fundName).shareClasses.push({
        isin: row.Isin,
        wkn: row.Wkn,
        className: row.ShareClass,
        currency: row.ShareclassCurrency,
        nav: row.Nav,
        launchDate: row.LaunchDate,
        morningstarRating: row.MorningstarRating
      });
    });
    
    let funds = Array.from(shareClassMap.values()).map(fund => ({
      name: fund.name,
      fundId: fund.fundId,
      assetClass: fund.assetClass,
      legalStructure: fund.legalStructure,
      sfdr: fund.sfdr,
      jurisdiction: opts.jurisdiction.toUpperCase(),
      manco: { name: config.manco, jurisdiction: config.mancoJurisdiction },
      shareClassCount: fund.shareClasses.length,
      shareClasses: fund.shareClasses,
      primaryIsin: fund.shareClasses[0]?.isin,
      detailSlug: fund.detailSlug
    }));
    
    console.log(`   Found ${funds.length} unique funds (${fundList.length} share classes)`);
    
    // Scrape fund details if requested
    if (opts.details) {
      const toScrape = opts.limit ? funds.slice(0, opts.limit) : funds;
      console.log(`\n📄 Scraping ${toScrape.length} fund detail pages...`);
      
      for (let i = 0; i < toScrape.length; i++) {
        const fund = toScrape[i];
        if (!fund.detailSlug) continue;
        
        const detailUrl = `${config.baseUrl}${config.detailPrefix}${fund.detailSlug}`;
        console.log(`   [${i + 1}/${toScrape.length}] ${fund.name}`);
        
        try {
          const details = await scrapeFundDetail(page, detailUrl);
          Object.assign(fund, details);
        } catch (err) {
          console.log(`      ✗ Error: ${err.message}`);
        }
        
        // Rate limiting
        await page.waitForTimeout(800);
      }
    }
    
    // Save outputs
    await saveOutputs(funds, fundList.length, fundData);
    
  } finally {
    await browser.close();
  }
}


async function scrapeFundDetail(page, url) {
  await page.goto(url, { waitUntil: 'networkidle', timeout: 45000 });
  await page.waitForTimeout(2000);
  
  // Check if we need to accept disclaimer on detail page
  try {
    const confirmBtn = await page.$('button:has-text("Confirm")');
    if (confirmBtn) {
      await confirmBtn.click();
      await page.waitForTimeout(2000);
    }
  } catch (e) {}
  
  const details = await page.evaluate(() => {
    const pageText = document.body.innerText;
    
    // Investment objective - extract full sentence(s) after keyword
    let investmentObjective = null;
    const objPatterns = [
      /Investment (?:Objective|Strategy)[:\s]*([^.]+\.[^.]*\.?)/i,
      /The fund[^.]*(?:aims|seeks|invests|objective)[^.]+\./i,
      /(?:aims to|seeks to|objective is to)[^.]+\./i
    ];
    for (const pattern of objPatterns) {
      const match = pageText.match(pattern);
      if (match) {
        investmentObjective = (match[1] || match[0]).trim();
        break;
      }
    }
    
    // Tagline - often under fund name (e.g. "Bond fund following an environmentally responsible approach")
    let tagline = null;
    const taglineEl = document.querySelector('.fund-tagline, [class*="subtitle"], p.c-copy');
    if (taglineEl) {
      const text = taglineEl.textContent.trim();
      if (text.length > 20 && text.length < 200 && !text.match(/^[A-Z]{2}\d/)) {
        tagline = text;
      }
    }
    
    // Benchmark - look for specific patterns
    let benchmark = null;
    const benchPatterns = [
      /Benchmark\s*\n?\s*([A-Z][^\n]+)/i,
      /Reference Index[:\s]*([^\n]+)/i,
      /Index[:\s]*([A-Z][^\n]+)/i
    ];
    for (const pattern of benchPatterns) {
      const match = pageText.match(pattern);
      if (match && match[1].length > 5) {
        benchmark = match[1].trim();
        break;
      }
    }
    
    // SFDR category
    let sfdrCategory = null;
    const sfdrMatch = pageText.match(/(?:SFDR|Sustainability)[:\s]*Article\s+(\d)/i) ||
                      pageText.match(/Article\s+(6|8|9)\s+(?:fund|product)/i);
    if (sfdrMatch) sfdrCategory = `Article ${sfdrMatch[1]}`;
    
    // Risk indicator (SRI) - usually 1-7
    let riskIndicator = null;
    const riskMatch = pageText.match(/Summary Risk Indicator[:\s]*(\d)/i) ||
                      pageText.match(/Risk (?:Level|Rating|Indicator)[:\s]*(\d)/i);
    if (riskMatch) riskIndicator = parseInt(riskMatch[1]);
    
    // Share class currency
    let currency = null;
    const currMatch = pageText.match(/Currency[:\s]*(EUR|USD|GBP|CHF|JPY)/i);
    if (currMatch) currency = currMatch[1].toUpperCase();
    
    // Distribution type
    let distributionType = null;
    const distMatch = pageText.match(/Distribution Type[:\s]*(Accumulating|Distributing)/i) ||
                      pageText.match(/(Accumulating|Distributing)/i);
    if (distMatch) distributionType = distMatch[1];
    
    // Management fee / TER
    let managementFee = null;
    const feeMatch = pageText.match(/Management Fee[:\s]*([\d.]+)\s*%/i) ||
                     pageText.match(/Ongoing Charge[s]?[:\s]*([\d.]+)\s*%/i) ||
                     pageText.match(/TER[:\s]*([\d.]+)\s*%/i);
    if (feeMatch) managementFee = `${feeMatch[1]}%`;
    
    // Document links
    const kiidLink = document.querySelector('a[href*=".pdf"][href*="KI"], a[href*="kiid"], a[href*="KID"]');
    const prospectusLink = document.querySelector('a[href*="prospectus" i]');
    const factsheetLink = document.querySelector('a[href*="factsheet" i], a[href*="fact-sheet" i]');
    
    return {
      investmentObjective,
      tagline,
      benchmark,
      sfdrCategory,
      riskIndicator,
      currency,
      distributionType,
      managementFee,
      documents: {
        kiid: kiidLink?.href || null,
        prospectus: prospectusLink?.href || null,
        factsheet: factsheetLink?.href || null
      }
    };
  });
  
  return details;
}

async function saveOutputs(funds, totalShareClasses, rawData) {
  const outputDir = opts.output;
  await fs.mkdir(outputDir, { recursive: true });
  
  const timestamp = new Date().toISOString().split('T')[0];
  const jurisdiction = opts.jurisdiction;
  
  // Save raw API response
  const rawFile = path.join(outputDir, `allianz-${jurisdiction}-${timestamp}-raw.json`);
  await fs.writeFile(rawFile, JSON.stringify(rawData, null, 2));
  console.log(`\n💾 Raw API: ${rawFile}`);
  
  // Save processed funds
  const output = {
    metadata: {
      jurisdiction: jurisdiction.toUpperCase(),
      jurisdictionName: config.name,
      manco: config.manco,
      scrapedAt: new Date().toISOString(),
      totalFunds: funds.length,
      totalShareClasses: totalShareClasses,
      detailsScraped: opts.details || false,
      source: 'regulatory.allianzgi.com'
    },
    manco: {
      name: config.manco,
      jurisdiction: config.mancoJurisdiction,
      type: 'LIMITED_COMPANY',
      roles: ['MANCO', 'INVESTMENT_MANAGER']
    },
    funds: funds
  };
  
  const outputFile = path.join(outputDir, `allianz-${jurisdiction}-${timestamp}.json`);
  await fs.writeFile(outputFile, JSON.stringify(output, null, 2));
  console.log(`   Funds: ${outputFile}`);
  
  // CSV summary
  const csvFile = path.join(outputDir, `allianz-${jurisdiction}-${timestamp}.csv`);
  const csvHeaders = ['Fund Name', 'Primary ISIN', 'Asset Class', 'SFDR', 'Legal Structure', 'Share Classes'];
  if (opts.details) csvHeaders.push('Has Objective', 'Benchmark');
  
  const csvRows = [csvHeaders.join(',')];
  funds.forEach(f => {
    const row = [
      `"${(f.name || '').replace(/"/g, '""')}"`,
      f.primaryIsin || '',
      f.assetClass || '',
      f.sfdr || '',
      f.legalStructure || '',
      f.shareClassCount || 0
    ];
    if (opts.details) {
      row.push(f.investmentObjective ? 'Y' : 'N');
      row.push(`"${(f.benchmark || '').replace(/"/g, '""')}"`);
    }
    csvRows.push(row.join(','));
  });
  await fs.writeFile(csvFile, csvRows.join('\n'));
  console.log(`   CSV: ${csvFile}`);
  
  // Summary stats
  console.log(`\n✅ Complete: ${funds.length} funds, ${totalShareClasses} share classes`);
  
  const byAsset = {};
  funds.forEach(f => { byAsset[f.assetClass] = (byAsset[f.assetClass] || 0) + 1; });
  console.log(`   Asset Classes: ${Object.entries(byAsset).map(([k,v]) => `${k}(${v})`).join(', ')}`);
  
  const bySfdr = {};
  funds.forEach(f => { bySfdr[f.sfdr] = (bySfdr[f.sfdr] || 0) + 1; });
  console.log(`   SFDR: ${Object.entries(bySfdr).map(([k,v]) => `${k}(${v})`).join(', ')}`);
}

main().catch(err => {
  console.error('Fatal:', err.message);
  if (opts.debug) console.error(err.stack);
  process.exit(1);
});
