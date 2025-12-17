/**
 * Allianz Global Investors Fund Scraper
 * 
 * Scrapes fund data from regulatory.allianzgi.com
 * Outputs JSON for import into ob-poc
 * 
 * Usage:
 *   npm run scrape              # All jurisdictions
 *   npm run scrape:lu           # Luxembourg only
 *   node scrape-allianz.js --test  # Test mode (first 5 funds)
 */

import { chromium } from 'playwright';
import { program } from 'commander';
import * as fs from 'fs';
import * as path from 'path';

// ============================================================================
// Configuration
// ============================================================================

const CONFIG = {
  baseUrl: 'https://regulatory.allianzgi.com',
  jurisdictions: {
    LU: {
      name: 'Luxembourg',
      fundListUrl: '/en-gb/b2c/luxemburg-en/funds/mutual-funds',
      manco: {
        name: 'Allianz Global Investors GmbH',
        jurisdiction: 'DE',
        branch: 'Allianz Global Investors GmbH, Luxembourg Branch',
        regNumber: 'B-159495'
      }
    },
    IE: {
      name: 'Ireland', 
      fundListUrl: '/en-gb/b2c/ireland-en/funds/mutual-funds',
      manco: {
        name: 'Allianz Global Investors Ireland Limited',
        jurisdiction: 'IE',
        regNumber: '332926'
      }
    },
    DE: {
      name: 'Germany',
      fundListUrl: '/en-gb/b2c/germany-en/funds/mutual-funds',
      manco: {
        name: 'Allianz Global Investors GmbH',
        jurisdiction: 'DE',
        regNumber: 'HRB 9340'
      }
    }
  },
  selectors: {
    // These will need adjustment based on actual page structure
    fundTable: '[data-testid="fund-table"], .fund-list, table.funds',
    fundRow: 'tr[data-fund-id], .fund-row, tbody tr',
    fundName: '[data-field="name"], .fund-name, td:first-child',
    isin: '[data-field="isin"], .isin, td:nth-child(2)',
    shareClass: '[data-field="share-class"], .share-class',
    currency: '[data-field="currency"], .currency',
    assetClass: '[data-field="asset-class"], .asset-class',
    sfdrCategory: '[data-field="sfdr"], .sfdr',
    navPrice: '[data-field="nav"], .nav-price',
    // Document links
    kiidLink: 'a[href*="kiid"], a[href*="KIID"], a:contains("KIID")',
    prospectusLink: 'a[href*="prospectus"], a:contains("Prospectus")',
    factsheetLink: 'a[href*="factsheet"], a:contains("Factsheet")'
  },
  output: {
    dir: './output',
    fundsFile: 'allianz-funds.json',
    shareClassesFile: 'allianz-share-classes.json',
    documentsFile: 'allianz-documents.json'
  },
  delays: {
    pageLoad: 3000,
    betweenRequests: 500,
    afterFilter: 1500
  }
};

// ============================================================================
// Data Models (matches ob-poc import schema)
// ============================================================================

/**
 * @typedef {Object} ManagementCompany
 * @property {string} name
 * @property {string} jurisdiction - ISO country code
 * @property {string} [branch]
 * @property {string} [regNumber]
 */

/**
 * @typedef {Object} Fund  
 * @property {string} name - Fund name (sub-fund name)
 * @property {string} umbrellaName - Parent SICAV name
 * @property {string} jurisdiction - Domicile
 * @property {string} legalStructure - SICAV, FCP, etc.
 * @property {string} ucitsCompliant
 * @property {string} assetClass - Equity, Fixed Income, Multi Asset
 * @property {string} sfdrCategory - Article 6, 8, or 9
 * @property {string} investmentObjective
 * @property {ManagementCompany} manco
 * @property {ShareClass[]} shareClasses
 * @property {Document[]} documents
 */

/**
 * @typedef {Object} ShareClass
 * @property {string} name - Share class name (A, I, W, etc.)
 * @property {string} isin
 * @property {string} currency
 * @property {string} [wkn] - German security ID
 * @property {string} distributionPolicy - Accumulating or Distributing
 * @property {number} [ter] - Total Expense Ratio
 * @property {number} [nav] - Latest NAV
 * @property {string} [navDate]
 * @property {string} [inceptionDate]
 */

/**
 * @typedef {Object} Document
 * @property {string} type - KIID, Prospectus, Factsheet, AnnualReport
 * @property {string} url
 * @property {string} language
 * @property {string} [date]
 */

// ============================================================================
// Scraper Class
// ============================================================================

class AllianzScraper {
  constructor(options = {}) {
    this.browser = null;
    this.page = null;
    this.options = {
      headless: options.headless ?? true,
      jurisdiction: options.jurisdiction ?? 'LU',
      testMode: options.testMode ?? false,
      maxFunds: options.maxFunds ?? Infinity
    };
    this.results = {
      funds: [],
      shareClasses: [],
      documents: [],
      errors: []
    };
  }

  async init() {
    console.log('🚀 Launching browser...');
    this.browser = await chromium.launch({ 
      headless: this.options.headless,
      slowMo: this.options.testMode ? 100 : 0
    });
    
    const context = await this.browser.newContext({
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
      viewport: { width: 1920, height: 1080 },
      locale: 'en-GB'
    });
    
    this.page = await context.newPage();
    
    // Log network requests in test mode
    if (this.options.testMode) {
      this.page.on('request', req => {
        if (req.url().includes('api') || req.url().includes('fund')) {
          console.log(`  📡 ${req.method()} ${req.url()}`);
        }
      });
    }
  }

  async close() {
    if (this.browser) {
      await this.browser.close();
    }
  }

  async acceptCookies() {
    try {
      // Handle cookie consent
      const cookieButton = await this.page.$('button:has-text("Accept"), button:has-text("Confirm"), #onetrust-accept-btn-handler');
      if (cookieButton) {
        await cookieButton.click();
        await this.page.waitForTimeout(500);
      }
      
      // Handle terms acceptance
      const termsCheckbox = await this.page.$('input[type="checkbox"]');
      if (termsCheckbox) {
        await termsCheckbox.check();
      }
      
      const confirmButton = await this.page.$('button:has-text("Confirm")');
      if (confirmButton) {
        await confirmButton.click();
        await this.page.waitForTimeout(1000);
      }
    } catch (e) {
      console.log('  ℹ️  No cookie/terms dialog found');
    }
  }

  async navigateToFundList() {
    const jurisdictionConfig = CONFIG.jurisdictions[this.options.jurisdiction];
    if (!jurisdictionConfig) {
      throw new Error(`Unknown jurisdiction: ${this.options.jurisdiction}`);
    }

    const url = CONFIG.baseUrl + jurisdictionConfig.fundListUrl;
    console.log(`📍 Navigating to ${url}`);
    
    await this.page.goto(url, { waitUntil: 'networkidle' });
    await this.page.waitForTimeout(CONFIG.delays.pageLoad);
    
    await this.acceptCookies();
  }

  async discoverApiEndpoint() {
    /**
     * The Allianz site uses a JavaScript framework that fetches fund data via API.
     * This method intercepts network requests to discover the actual data endpoint.
     */
    console.log('🔍 Discovering API endpoint...');
    
    const apiRequests = [];
    
    this.page.on('response', async (response) => {
      const url = response.url();
      const contentType = response.headers()['content-type'] || '';
      
      if (contentType.includes('application/json') && 
          (url.includes('fund') || url.includes('api') || url.includes('price'))) {
        try {
          const data = await response.json();
          apiRequests.push({ url, data });
          console.log(`  ✅ Found API: ${url}`);
        } catch (e) {
          // Not JSON
        }
      }
    });

    // Trigger a filter change to provoke API call
    await this.page.waitForTimeout(2000);
    
    // Try clicking filter dropdowns
    const filterButtons = await this.page.$$('select, [role="listbox"], .filter-dropdown');
    for (const btn of filterButtons.slice(0, 2)) {
      try {
        await btn.click();
        await this.page.waitForTimeout(500);
        await this.page.keyboard.press('Escape');
      } catch (e) {}
    }

    await this.page.waitForTimeout(2000);
    
    return apiRequests;
  }

  async scrapeFundListFromDom() {
    /**
     * Fallback: scrape directly from rendered DOM if API discovery fails
     */
    console.log('📋 Scraping fund list from DOM...');
    
    const funds = await this.page.evaluate((selectors) => {
      const results = [];
      
      // Try multiple selector strategies
      let rows = document.querySelectorAll('table tbody tr');
      if (rows.length === 0) {
        rows = document.querySelectorAll('.fund-row, .fund-item, [data-fund]');
      }
      if (rows.length === 0) {
        rows = document.querySelectorAll('div[class*="fund"], li[class*="fund"]');
      }
      
      rows.forEach((row, idx) => {
        const getText = (el, selector) => {
          const found = selector ? el.querySelector(selector) : el;
          return found?.textContent?.trim() || '';
        };
        
        const getLink = (el, selector) => {
          const link = el.querySelector(selector);
          return link?.href || '';
        };
        
        const rawText = row.textContent || '';
        
        // Extract fund data from raw text patterns
        // Pattern: "Click to see all N shareclasses belowFUND NAMEAsset Class | as of DATE | ISINShareClassNAV..."
        
        // Extract ISIN (LU followed by 10 digits)
        let isin = '';
        const isinMatch = rawText.match(/LU\d{10}/);
        if (isinMatch) {
          isin = isinMatch[0];
        }
        // Try IE ISIN
        if (!isin) {
          const ieIsinMatch = rawText.match(/IE[A-Z0-9]{10}/);
          if (ieIsinMatch) {
            isin = ieIsinMatch[0];
          }
        }
        
        // Extract fund name - between "below" and asset class keywords
        let fundName = '';
        const nameMatch = rawText.match(/below([A-Za-z][A-Za-z0-9\s\-\+\&]+?)(Fixed Income|Equity|Multi Asset|Alternatives|Money Market)/);
        if (nameMatch) {
          fundName = nameMatch[1].trim();
        } else {
          // Fallback: try to get from the link text or first cell
          const linkEl = row.querySelector('a[href*="fund"]');
          if (linkEl) {
            fundName = linkEl.textContent?.replace(/Click.*below/i, '').trim();
          }
        }
        
        // Extract asset class
        let assetClass = '';
        const assetMatch = rawText.match(/(Fixed Income|Equity|Multi Asset|Alternatives|Money Market)/i);
        if (assetMatch) {
          assetClass = assetMatch[1];
        }
        
        // Extract share class (A, I, W, WT, CT, etc.)
        let shareClass = '';
        const shareMatch = rawText.match(/\b([AIWC]T?\d?)\s*\(([A-Z]{3})\)/);
        if (shareMatch) {
          shareClass = shareMatch[1];
        }
        
        // Extract currency
        let currency = '';
        const currMatch = rawText.match(/\b(EUR|USD|GBP|CHF|JPY)\b/);
        if (currMatch) {
          currency = currMatch[1];
        }
        
        // Extract NAV (number with decimals after share class)
        let nav = '';
        const navMatch = rawText.match(/\)[\s]*([\d,]+\.\d{2,4})/);
        if (navMatch) {
          nav = navMatch[1].replace(',', '');
        }
        
        // Extract date
        let navDate = '';
        const dateMatch = rawText.match(/as of (\d{2}\/\d{2}\/\d{4})/);
        if (dateMatch) {
          navDate = dateMatch[1];
        }
        
        const fund = {
          _rowIndex: idx,
          _rawText: rawText.substring(0, 200),
          name: fundName,
          isin: isin,
          shareClass: shareClass,
          currency: currency,
          assetClass: assetClass,
          nav: nav,
          navDate: navDate,
          detailLink: getLink(row, 'a[href*="fund"]')
        };
        
        if (fund.name || fund.isin) {
          results.push(fund);
        }
      });
      
      return results;
    }, CONFIG.selectors);

    console.log(`  Found ${funds.length} fund entries in DOM`);
    return funds;
  }

  async scrapeFundDetail(fundUrl) {
    /**
     * Navigate to individual fund page and extract detailed data
     */
    console.log(`  📄 Scraping detail: ${fundUrl}`);
    
    const detailPage = await this.browser.newPage();
    try {
      await detailPage.goto(fundUrl, { waitUntil: 'networkidle' });
      await detailPage.waitForTimeout(CONFIG.delays.pageLoad);
      
      const details = await detailPage.evaluate(() => {
        const getText = (selector) => {
          const el = document.querySelector(selector);
          return el?.textContent?.trim() || '';
        };
        
        const getLinks = (selector) => {
          return Array.from(document.querySelectorAll(selector)).map(a => ({
            text: a.textContent?.trim(),
            href: a.href
          }));
        };
        
        return {
          name: getText('h1, .fund-name'),
          objective: getText('[class*="objective"], [class*="description"]'),
          assetClass: getText('[class*="asset-class"]'),
          sfdrCategory: getText('[class*="sfdr"], [class*="article"]'),
          ter: getText('[class*="ter"], [class*="ongoing"]'),
          inceptionDate: getText('[class*="inception"], [class*="launch"]'),
          documents: getLinks('a[href*=".pdf"]'),
          shareClasses: [] // Would need more complex extraction
        };
      });
      
      return details;
    } finally {
      await detailPage.close();
    }
  }

  async scrapeAll() {
    await this.init();
    
    try {
      await this.navigateToFundList();
      
      // First, try to discover the API
      const apiData = await this.discoverApiEndpoint();
      
      let funds;
      if (apiData.length > 0 && apiData[0].data) {
        console.log('✅ Using API data');
        funds = this.transformApiData(apiData[0].data);
      } else {
        console.log('⚠️  Falling back to DOM scraping');
        funds = await this.scrapeFundListFromDom();
      }
      
      // Limit in test mode
      if (this.options.testMode) {
        funds = funds.slice(0, 5);
        console.log(`🧪 Test mode: limiting to ${funds.length} funds`);
      }
      
      if (this.options.maxFunds < Infinity) {
        funds = funds.slice(0, this.options.maxFunds);
      }
      
      this.results.funds = funds;
      
      // Optionally scrape detail pages
      // for (const fund of funds) {
      //   if (fund.detailLink) {
      //     const details = await this.scrapeFundDetail(fund.detailLink);
      //     Object.assign(fund, details);
      //   }
      // }
      
      return this.results;
      
    } finally {
      await this.close();
    }
  }

  transformApiData(apiResponse) {
    /**
     * Transform API response to our schema.
     * This will need adjustment based on actual API structure.
     */
    if (Array.isArray(apiResponse)) {
      return apiResponse.map(item => ({
        name: item.name || item.fundName || item.Name,
        isin: item.isin || item.ISIN || item.Isin,
        shareClass: item.shareClass || item.ShareClass,
        currency: item.currency || item.Currency,
        nav: item.nav || item.NAV || item.price,
        assetClass: item.assetClass || item.AssetClass,
        sfdrCategory: item.sfdr || item.SFDR || item.article
      }));
    }
    
    // Handle nested structure
    if (apiResponse.funds || apiResponse.data || apiResponse.results) {
      return this.transformApiData(apiResponse.funds || apiResponse.data || apiResponse.results);
    }
    
    return [];
  }

  async saveResults() {
    const outputDir = CONFIG.output.dir;
    
    if (!fs.existsSync(outputDir)) {
      fs.mkdirSync(outputDir, { recursive: true });
    }
    
    const jurisdictionConfig = CONFIG.jurisdictions[this.options.jurisdiction];
    const timestamp = new Date().toISOString().split('T')[0];
    
    const output = {
      metadata: {
        scrapedAt: new Date().toISOString(),
        jurisdiction: this.options.jurisdiction,
        jurisdictionName: jurisdictionConfig.name,
        manco: jurisdictionConfig.manco,
        fundCount: this.results.funds.length
      },
      funds: this.results.funds,
      errors: this.results.errors
    };
    
    const filename = `allianz-${this.options.jurisdiction.toLowerCase()}-${timestamp}.json`;
    const filepath = path.join(outputDir, filename);
    
    fs.writeFileSync(filepath, JSON.stringify(output, null, 2));
    console.log(`💾 Saved to ${filepath}`);
    
    return filepath;
  }
}

// ============================================================================
// CLI
// ============================================================================

program
  .name('scrape-allianz')
  .description('Scrape Allianz Global Investors fund data')
  .option('-j, --jurisdiction <code>', 'Jurisdiction code (LU, IE, DE)', 'LU')
  .option('-t, --test', 'Test mode - scrape first 5 funds only')
  .option('-m, --max <number>', 'Maximum funds to scrape', parseInt)
  .option('--headed', 'Run browser in headed mode (visible)')
  .parse();

const opts = program.opts();

async function main() {
  console.log('╔════════════════════════════════════════════════════╗');
  console.log('║   Allianz Global Investors Fund Scraper            ║');
  console.log('╚════════════════════════════════════════════════════╝');
  console.log();
  console.log(`Jurisdiction: ${opts.jurisdiction}`);
  console.log(`Test mode: ${opts.test ? 'Yes' : 'No'}`);
  console.log();
  
  const scraper = new AllianzScraper({
    jurisdiction: opts.jurisdiction,
    testMode: opts.test,
    maxFunds: opts.max || Infinity,
    headless: !opts.headed
  });
  
  try {
    const results = await scraper.scrapeAll();
    
    console.log();
    console.log('📊 Results Summary:');
    console.log(`   Funds found: ${results.funds.length}`);
    console.log(`   Errors: ${results.errors.length}`);
    
    const filepath = await scraper.saveResults();
    
    console.log();
    console.log('✅ Done!');
    console.log(`   Output: ${filepath}`);
    
    // Show sample in test mode
    if (opts.test && results.funds.length > 0) {
      console.log();
      console.log('📝 Sample fund:');
      console.log(JSON.stringify(results.funds[0], null, 2));
    }
    
  } catch (error) {
    console.error('❌ Error:', error.message);
    if (opts.test) {
      console.error(error.stack);
    }
    process.exit(1);
  }
}

main();
