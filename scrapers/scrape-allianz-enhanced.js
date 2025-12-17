/**
 * Enhanced Allianz Global Investors Fund Scraper
 * 
 * Scrapes comprehensive fund data including:
 * - Fund basic info (name, ISIN, asset class)
 * - Share classes with all variants
 * - SFDR classification (Article 6/8/9)
 * - Investment objective/mandate
 * - Key documents (KIID, Prospectus, Factsheet)
 * - Management company details
 * - Fee information (TER, management fee)
 * 
 * Usage:
 *   npm run scrape                    # All jurisdictions
 *   npm run scrape -- --jurisdiction LU  # Luxembourg only
 *   npm run scrape -- --details       # Include detail page scraping
 *   npm run scrape -- --test          # Test mode (first 5 funds)
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
        regNumber: 'B-159495',
        regulator: 'CSSF',
        address: 'Bockenheimer Landstrasse 42-44, 60323 Frankfurt/M, Germany'
      },
      depositary: {
        name: 'State Street Bank International GmbH, Luxembourg Branch',
        jurisdiction: 'LU'
      }
    },
    IE: {
      name: 'Ireland',
      fundListUrl: '/en-gb/b2c/ireland-en/funds/mutual-funds',
      manco: {
        name: 'Allianz Global Investors Ireland Limited',
        jurisdiction: 'IE',
        regNumber: '332926',
        regulator: 'Central Bank of Ireland'
      }
    },
    DE: {
      name: 'Germany',
      fundListUrl: '/en-gb/b2c/germany-en/funds/mutual-funds',
      manco: {
        name: 'Allianz Global Investors GmbH',
        jurisdiction: 'DE',
        regNumber: 'HRB 9340',
        regulator: 'BaFin'
      }
    }
  },

  // Timing
  delays: {
    pageLoad: 3000,
    afterClick: 500,
    afterFilter: 1500,
    betweenDetailPages: 1000
  },

  output: {
    dir: './output'
  }
};

// ============================================================================
// Enhanced Scraper Class
// ============================================================================

class EnhancedAllianzScraper {
  constructor(options = {}) {
    this.browser = null;
    this.page = null;
    this.options = {
      headless: options.headless ?? true,
      jurisdiction: options.jurisdiction ?? 'LU',
      testMode: options.testMode ?? false,
      maxFunds: options.maxFunds ?? Infinity,
      scrapeDetails: options.scrapeDetails ?? false,
      verbose: options.verbose ?? false
    };
    
    this.results = {
      metadata: {},
      umbrellas: [],  // SICAV/umbrella level
      funds: [],      // Sub-fund level  
      shareClasses: [],
      documents: [],
      errors: []
    };
  }

  log(msg, level = 'info') {
    const prefix = {
      info: '  ',
      success: '✅',
      warning: '⚠️ ',
      error: '❌',
      debug: '🔍'
    }[level] || '  ';
    
    if (level === 'debug' && !this.options.verbose) return;
    console.log(`${prefix} ${msg}`);
  }

  async init() {
    console.log('🚀 Launching browser...');
    this.browser = await chromium.launch({ 
      headless: this.options.headless,
      slowMo: this.options.testMode ? 50 : 0
    });
    
    const context = await this.browser.newContext({
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
      viewport: { width: 1920, height: 1080 },
      locale: 'en-GB'
    });
    
    this.page = await context.newPage();
    
    // Capture XHR responses for API discovery
    this.apiResponses = [];
    this.page.on('response', async (response) => {
      const url = response.url();
      const contentType = response.headers()['content-type'] || '';
      
      if (contentType.includes('application/json')) {
        try {
          const data = await response.json();
          this.apiResponses.push({ url, data, timestamp: Date.now() });
          this.log(`API response: ${url.substring(0, 80)}...`, 'debug');
        } catch (e) {}
      }
    });
  }

  async close() {
    if (this.browser) {
      await this.browser.close();
    }
  }

  async acceptCookiesAndTerms() {
    this.log('Handling consent dialogs...');
    
    try {
      // Wait for potential overlay
      await this.page.waitForTimeout(1000);
      
      // Try various cookie consent buttons
      const consentSelectors = [
        '#onetrust-accept-btn-handler',
        'button:has-text("Accept All")',
        'button:has-text("Accept")',
        'button:has-text("I Accept")',
        '.cookie-accept'
      ];
      
      for (const selector of consentSelectors) {
        const btn = await this.page.$(selector);
        if (btn && await btn.isVisible()) {
          await btn.click();
          this.log('Accepted cookies', 'success');
          await this.page.waitForTimeout(500);
          break;
        }
      }
      
      // Handle jurisdiction/terms acceptance modal
      const checkbox = await this.page.$('input[type="checkbox"]');
      if (checkbox && await checkbox.isVisible()) {
        await checkbox.check();
        await this.page.waitForTimeout(200);
      }
      
      const confirmBtn = await this.page.$('button:has-text("Confirm")');
      if (confirmBtn && await confirmBtn.isVisible()) {
        await confirmBtn.click();
        this.log('Confirmed terms', 'success');
        await this.page.waitForTimeout(1000);
      }
      
    } catch (e) {
      this.log('No consent dialogs found or already accepted', 'debug');
    }
  }

  async navigateToFundList() {
    const jConfig = CONFIG.jurisdictions[this.options.jurisdiction];
    if (!jConfig) {
      throw new Error(`Unknown jurisdiction: ${this.options.jurisdiction}`);
    }

    const url = CONFIG.baseUrl + jConfig.fundListUrl;
    this.log(`Navigating to ${url}`);
    
    await this.page.goto(url, { 
      waitUntil: 'networkidle',
      timeout: 30000 
    });
    await this.page.waitForTimeout(CONFIG.delays.pageLoad);
    
    await this.acceptCookiesAndTerms();
    
    // Store manco info in metadata
    this.results.metadata = {
      scrapedAt: new Date().toISOString(),
      jurisdiction: this.options.jurisdiction,
      jurisdictionName: jConfig.name,
      manco: jConfig.manco,
      depositary: jConfig.depositary,
      sourceUrl: url
    };
  }

  async waitForFundTable() {
    this.log('Waiting for fund table to load...');
    
    // Wait for either table or fund items to appear
    try {
      await this.page.waitForSelector('table tbody tr, .fund-row, [data-fund]', {
        timeout: 15000
      });
    } catch (e) {
      this.log('Fund table not found with standard selectors', 'warning');
    }
    
    // Additional wait for JavaScript rendering
    await this.page.waitForTimeout(2000);
  }

  async scrapeFundListFromDOM() {
    this.log('Extracting fund data from page...');
    
    const funds = await this.page.evaluate(() => {
      const results = [];
      
      // Find all table rows
      const rows = document.querySelectorAll('table tbody tr');
      
      rows.forEach((row, idx) => {
        const rawText = row.textContent || '';
        
        // Skip empty or header rows
        if (!rawText || rawText.length < 20) return;
        
        // Extract ISIN (LU/IE followed by 10 alphanumeric)
        let isin = '';
        const isinMatch = rawText.match(/([A-Z]{2}[A-Z0-9]{10})/);
        if (isinMatch) {
          isin = isinMatch[1];
        }
        
        // Extract fund name from link or text
        let fundName = '';
        const link = row.querySelector('a[href*="fund"]');
        if (link) {
          // Clean up the link text
          fundName = link.textContent
            ?.replace(/Click.*below/i, '')
            ?.replace(/\d+\s*shareclasses?/i, '')
            ?.trim() || '';
        }
        
        // Fallback: extract from raw text before asset class
        if (!fundName) {
          const nameMatch = rawText.match(/below([A-Za-z][A-Za-z0-9\s\-\+\&\.]+?)(Fixed Income|Equity|Multi Asset|Alternatives|Money Market)/i);
          if (nameMatch) {
            fundName = nameMatch[1].trim();
          }
        }
        
        // Extract asset class
        let assetClass = '';
        const assetMatch = rawText.match(/(Fixed Income|Equity|Multi Asset|Alternatives|Money Market)/i);
        if (assetMatch) {
          assetClass = assetMatch[1];
        }
        
        // Extract currency
        let currency = '';
        const currMatch = rawText.match(/\b(EUR|USD|GBP|CHF|JPY|AUD|CAD|SGD|HKD)\b/);
        if (currMatch) {
          currency = currMatch[1];
        }
        
        // Extract share class type (A, I, W, AT, IT, WT, CT, etc.)
        let shareClassType = '';
        const shareMatch = rawText.match(/\b([AIWPC]T?\d?)\s*\(/);
        if (shareMatch) {
          shareClassType = shareMatch[1];
        }
        
        // Extract NAV
        let nav = '';
        let navDate = '';
        const navMatch = rawText.match(/\)[\s]*([\d,]+\.\d{2,4})/);
        if (navMatch) {
          nav = navMatch[1].replace(/,/g, '');
        }
        const dateMatch = rawText.match(/as of (\d{2}\/\d{2}\/\d{4})/);
        if (dateMatch) {
          navDate = dateMatch[1];
        }
        
        // Extract SFDR category if present
        let sfdrCategory = '';
        const sfdrMatch = rawText.match(/Article\s*(\d+)/i);
        if (sfdrMatch) {
          sfdrCategory = `Article ${sfdrMatch[1]}`;
        }
        
        // Get detail page link
        const detailLink = link?.href || '';
        
        // Get share class count
        let shareClassCount = 1;
        const countMatch = rawText.match(/(\d+)\s*shareclass/i);
        if (countMatch) {
          shareClassCount = parseInt(countMatch[1], 10);
        }
        
        // Only add if we have meaningful data
        if (isin || fundName) {
          results.push({
            _rowIndex: idx,
            name: fundName,
            isin: isin,
            shareClassType: shareClassType,
            shareClassCount: shareClassCount,
            currency: currency,
            assetClass: assetClass,
            sfdrCategory: sfdrCategory,
            nav: nav,
            navDate: navDate,
            detailLink: detailLink
          });
        }
      });
      
      return results;
    });

    this.log(`Found ${funds.length} fund/share class entries`, 'success');
    return funds;
  }

  async scrapeFundDetailPage(fund) {
    /**
     * Scrape individual fund detail page for:
     * - Investment objective
     * - Full share class list
     * - Document links (KIID, Prospectus, etc.)
     * - Fee information
     * - SFDR classification
     */
    if (!fund.detailLink) {
      this.log(`No detail link for ${fund.name}`, 'warning');
      return null;
    }

    this.log(`Scraping detail: ${fund.name}`);
    
    const detailPage = await this.browser.newPage();
    
    try {
      await detailPage.goto(fund.detailLink, { 
        waitUntil: 'networkidle',
        timeout: 20000 
      });
      await detailPage.waitForTimeout(CONFIG.delays.pageLoad);
      
      // Accept any consent dialogs on detail page
      try {
        const confirmBtn = await detailPage.$('button:has-text("Confirm")');
        if (confirmBtn && await confirmBtn.isVisible()) {
          await confirmBtn.click();
          await detailPage.waitForTimeout(500);
        }
      } catch (e) {}

      const details = await detailPage.evaluate(() => {
        const getText = (selector) => {
          const el = document.querySelector(selector);
          return el?.textContent?.trim() || '';
        };
        
        const getAllText = (selector) => {
          return Array.from(document.querySelectorAll(selector))
            .map(el => el.textContent?.trim())
            .filter(Boolean);
        };
        
        const getDocLinks = () => {
          const docs = [];
          const pdfLinks = document.querySelectorAll('a[href*=".pdf"]');
          
          pdfLinks.forEach(link => {
            const href = link.href;
            const text = link.textContent?.toLowerCase() || '';
            
            let docType = 'OTHER';
            if (text.includes('kiid') || text.includes('key information')) {
              docType = 'KIID';
            } else if (text.includes('prospectus')) {
              docType = 'PROSPECTUS';
            } else if (text.includes('factsheet') || text.includes('fact sheet')) {
              docType = 'FACTSHEET';
            } else if (text.includes('annual')) {
              docType = 'ANNUAL_REPORT';
            } else if (text.includes('semi-annual') || text.includes('semi annual')) {
              docType = 'SEMI_ANNUAL_REPORT';
            }
            
            docs.push({
              type: docType,
              url: href,
              text: link.textContent?.trim()
            });
          });
          
          return docs;
        };
        
        // Get page content for analysis
        const pageText = document.body?.textContent || '';
        
        // Extract investment objective
        let objective = '';
        const objSection = document.querySelector('[class*="objective"], [class*="description"], .fund-description');
        if (objSection) {
          objective = objSection.textContent?.trim().substring(0, 1000) || '';
        } else {
          // Try to find it in page text
          const objMatch = pageText.match(/investment objective[:\s]*(.*?)(?:investment|risk|opportunities|$)/is);
          if (objMatch) {
            objective = objMatch[1].substring(0, 500).trim();
          }
        }
        
        // Extract SFDR category
        let sfdr = '';
        const sfdrMatch = pageText.match(/Article\s*(\d+)\s*(SFDR)?/i);
        if (sfdrMatch) {
          sfdr = `Article ${sfdrMatch[1]}`;
        }
        // Also check for SRI/ESG indicators
        const sriMatch = pageText.match(/\b(SRI|ESG|sustainable|responsible)\b/i);
        const sustainabilityIndicator = sriMatch ? sriMatch[1] : '';
        
        // Extract TER/fees
        let ter = '';
        const terMatch = pageText.match(/(?:TER|Total Expense Ratio|Ongoing charges?)[\s:]*(\d+\.?\d*)\s*%/i);
        if (terMatch) {
          ter = terMatch[1];
        }
        
        // Extract inception date
        let inceptionDate = '';
        const inceptionMatch = pageText.match(/(?:Inception|Launch|Start)\s*(?:Date)?[\s:]*(\d{1,2}[\/\-\.]\d{1,2}[\/\-\.]\d{2,4})/i);
        if (inceptionMatch) {
          inceptionDate = inceptionMatch[1];
        }
        
        // Extract benchmark
        let benchmark = '';
        const benchSection = document.querySelector('[class*="benchmark"]');
        if (benchSection) {
          benchmark = benchSection.textContent?.trim() || '';
        }
        
        // Get all share classes from the page
        const shareClasses = [];
        const shareRows = document.querySelectorAll('[class*="share-class"], table tr');
        shareRows.forEach(row => {
          const rowText = row.textContent || '';
          const isinMatch = rowText.match(/([A-Z]{2}[A-Z0-9]{10})/);
          if (isinMatch) {
            const currMatch = rowText.match(/\b(EUR|USD|GBP|CHF|JPY)\b/);
            const typeMatch = rowText.match(/\b([AIWPC]T?\d?)\s/);
            shareClasses.push({
              isin: isinMatch[1],
              currency: currMatch ? currMatch[1] : '',
              type: typeMatch ? typeMatch[1] : ''
            });
          }
        });
        
        return {
          investmentObjective: objective,
          sfdrCategory: sfdr,
          sustainabilityIndicator: sustainabilityIndicator,
          ter: ter,
          inceptionDate: inceptionDate,
          benchmark: benchmark,
          documents: getDocLinks(),
          shareClasses: shareClasses
        };
      });

      return details;
      
    } catch (e) {
      this.log(`Error scraping detail for ${fund.name}: ${e.message}`, 'error');
      this.results.errors.push({
        fund: fund.name,
        error: e.message,
        url: fund.detailLink
      });
      return null;
    } finally {
      await detailPage.close();
    }
  }

  groupFundsByUmbrella(funds) {
    /**
     * Group share classes into funds, and funds into umbrellas
     * Most Allianz funds are under "Allianz Global Investors Fund" SICAV
     */
    const umbrellaMap = new Map();
    const fundMap = new Map();
    
    for (const entry of funds) {
      // Determine umbrella (parent SICAV)
      let umbrellaName = 'Allianz Global Investors Fund';
      if (entry.name.includes('AEVN')) {
        umbrellaName = 'AEVN CDO Fund';
      } else if (entry.name.includes('Money Market')) {
        umbrellaName = 'Allianz Money Market Fund';
      }
      
      // Normalize fund name (remove share class suffix)
      const baseName = entry.name
        .replace(/\s*-\s*[AIWPC]T?\d?\s*-?\s*(EUR|USD|GBP|CHF)?$/i, '')
        .replace(/\s+/g, ' ')
        .trim();
      
      const fundKey = `${umbrellaName}::${baseName}`;
      
      if (!fundMap.has(fundKey)) {
        fundMap.set(fundKey, {
          umbrella: umbrellaName,
          name: baseName,
          assetClass: entry.assetClass,
          sfdrCategory: entry.sfdrCategory,
          shareClasses: []
        });
      }
      
      fundMap.get(fundKey).shareClasses.push({
        isin: entry.isin,
        type: entry.shareClassType,
        currency: entry.currency,
        nav: entry.nav,
        navDate: entry.navDate,
        detailLink: entry.detailLink
      });
      
      // Track umbrella
      if (!umbrellaMap.has(umbrellaName)) {
        umbrellaMap.set(umbrellaName, {
          name: umbrellaName,
          legalStructure: 'SICAV',
          jurisdiction: this.options.jurisdiction,
          funds: []
        });
      }
    }
    
    // Link funds to umbrellas
    for (const [fundKey, fundData] of fundMap) {
      const umbrella = umbrellaMap.get(fundData.umbrella);
      umbrella.funds.push(fundData);
    }
    
    return {
      umbrellas: Array.from(umbrellaMap.values()),
      funds: Array.from(fundMap.values())
    };
  }

  async scrapeAll() {
    await this.init();
    
    try {
      await this.navigateToFundList();
      await this.waitForFundTable();
      
      // Scrape basic fund list
      const rawFunds = await this.scrapeFundListFromDOM();
      
      // Limit in test mode
      let fundsToProcess = rawFunds;
      if (this.options.testMode) {
        fundsToProcess = rawFunds.slice(0, 5);
        this.log(`Test mode: processing ${fundsToProcess.length} funds`, 'warning');
      } else if (this.options.maxFunds < Infinity) {
        fundsToProcess = rawFunds.slice(0, this.options.maxFunds);
      }
      
      // Optionally scrape detail pages
      if (this.options.scrapeDetails) {
        this.log('Scraping detail pages...');
        
        for (let i = 0; i < fundsToProcess.length; i++) {
          const fund = fundsToProcess[i];
          const progress = `[${i + 1}/${fundsToProcess.length}]`;
          this.log(`${progress} ${fund.name}`);
          
          const details = await this.scrapeFundDetailPage(fund);
          if (details) {
            Object.assign(fund, details);
          }
          
          await this.page.waitForTimeout(CONFIG.delays.betweenDetailPages);
        }
      }
      
      // Group into structure
      const { umbrellas, funds } = this.groupFundsByUmbrella(fundsToProcess);
      
      this.results.umbrellas = umbrellas;
      this.results.funds = funds;
      this.results.rawEntries = fundsToProcess;
      this.results.metadata.fundCount = funds.length;
      this.results.metadata.shareClassCount = fundsToProcess.length;
      
      return this.results;
      
    } finally {
      await this.close();
    }
  }

  async saveResults() {
    const outputDir = CONFIG.output.dir;
    if (!fs.existsSync(outputDir)) {
      fs.mkdirSync(outputDir, { recursive: true });
    }
    
    const timestamp = new Date().toISOString().split('T')[0];
    const jurisdiction = this.options.jurisdiction.toLowerCase();
    const filename = `allianz-${jurisdiction}-${timestamp}.json`;
    const filepath = path.join(outputDir, filename);
    
    fs.writeFileSync(filepath, JSON.stringify(this.results, null, 2));
    this.log(`Results saved to ${filepath}`, 'success');
    
    return filepath;
  }
}

// ============================================================================
// CLI
// ============================================================================

program
  .name('scrape-allianz')
  .description('Scrape Allianz Global Investors fund data')
  .option('-j, --jurisdiction <code>', 'Jurisdiction (LU, IE, DE)', 'LU')
  .option('-d, --details', 'Scrape individual fund detail pages')
  .option('-t, --test', 'Test mode (first 5 funds only)')
  .option('-m, --max <n>', 'Maximum funds to scrape', parseInt)
  .option('-v, --verbose', 'Verbose output')
  .option('--headed', 'Run browser in headed mode')
  .parse();

const options = program.opts();

async function main() {
  console.log('═'.repeat(60));
  console.log('  Allianz Global Investors Fund Scraper');
  console.log('═'.repeat(60));
  console.log(`  Jurisdiction: ${options.jurisdiction}`);
  console.log(`  Detail pages: ${options.details ? 'Yes' : 'No'}`);
  console.log(`  Test mode: ${options.test ? 'Yes' : 'No'}`);
  console.log('═'.repeat(60));
  
  const scraper = new EnhancedAllianzScraper({
    jurisdiction: options.jurisdiction,
    scrapeDetails: options.details,
    testMode: options.test,
    maxFunds: options.max,
    headless: !options.headed,
    verbose: options.verbose
  });
  
  try {
    const results = await scraper.scrapeAll();
    const outputFile = await scraper.saveResults();
    
    console.log('\n' + '═'.repeat(60));
    console.log('  Summary');
    console.log('═'.repeat(60));
    console.log(`  Umbrellas: ${results.umbrellas.length}`);
    console.log(`  Funds: ${results.funds.length}`);
    console.log(`  Share Classes: ${results.rawEntries.length}`);
    console.log(`  Errors: ${results.errors.length}`);
    console.log(`  Output: ${outputFile}`);
    console.log('═'.repeat(60));
    
  } catch (e) {
    console.error('❌ Scraper failed:', e.message);
    process.exit(1);
  }
}

main();
