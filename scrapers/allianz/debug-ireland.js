/**
 * Debug Ireland site structure
 */

import { chromium } from 'playwright';

async function debugIreland() {
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  const apiCalls = [];
  
  page.on('response', async (response) => {
    const url = response.url();
    const contentType = response.headers()['content-type'] || '';
    
    // Log ALL API-like calls
    if (url.includes('api') || url.includes('fund') || contentType.includes('json')) {
      console.log(`📡 ${response.status()} ${url.substring(0, 100)}`);
      
      if (contentType.includes('json')) {
        try {
          const data = await response.json();
          apiCalls.push({ url, data });
        } catch (e) {}
      }
    }
  });
  
  console.log('Loading Ireland fund list...');
  await page.goto('https://regulatory.allianzgi.com/en-gb/b2c/ireland-en/funds/mutual-funds', {
    waitUntil: 'networkidle',
    timeout: 60000
  });
  
  // Accept disclaimer
  try {
    await page.click('button:has-text("Confirm")', { timeout: 5000 });
    console.log('Clicked confirm');
  } catch (e) {}
  
  await page.waitForTimeout(5000);
  
  // Check what loaded
  const pageInfo = await page.evaluate(() => {
    const tables = document.querySelectorAll('table');
    const fundLinks = document.querySelectorAll('a[href*="fund"]');
    const fundRows = document.querySelectorAll('[class*="fund"], tr');
    
    // Find any data in the page
    const bodyText = document.body.innerText;
    const isinMatches = bodyText.match(/[A-Z]{2}[A-Z0-9]{10}/g) || [];
    
    return {
      tables: tables.length,
      fundLinks: Array.from(fundLinks).slice(0, 20).map(a => ({ href: a.href, text: a.textContent?.trim().substring(0, 50) })),
      fundRows: fundRows.length,
      isinsFound: [...new Set(isinMatches)].slice(0, 20),
      htmlLength: document.body.innerHTML.length
    };
  });
  
  console.log('\n--- Page Analysis ---');
  console.log('Tables:', pageInfo.tables);
  console.log('Fund-like rows:', pageInfo.fundRows);
  console.log('Fund links:', pageInfo.fundLinks);
  console.log('ISINs found in page:', pageInfo.isinsFound);
  console.log('HTML length:', pageInfo.htmlLength);
  
  console.log('\n--- API Calls ---');
  apiCalls.forEach(c => {
    console.log(`\n${c.url}`);
    if (c.data) {
      const keys = Object.keys(c.data);
      console.log(`  Keys: ${keys.join(', ')}`);
      if (c.data.FundList) console.log(`  FundList length: ${c.data.FundList.length}`);
      if (Array.isArray(c.data)) console.log(`  Array length: ${c.data.length}`);
    }
  });
  
  // Try scrolling to trigger lazy load
  console.log('\n--- Trying scroll ---');
  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
  await page.waitForTimeout(3000);
  
  // Check for different page structure - maybe it's not a table
  const altStructure = await page.evaluate(() => {
    // Look for React/Vue data stores
    const scripts = Array.from(document.querySelectorAll('script'));
    let dataScript = null;
    scripts.forEach(s => {
      if (s.textContent && (s.textContent.includes('fundData') || s.textContent.includes('FundList'))) {
        dataScript = s.textContent.substring(0, 500);
      }
    });
    
    // Check for any repeating structures
    const cards = document.querySelectorAll('[class*="card"], [class*="item"], [class*="row"]');
    
    return {
      dataScript,
      cardCount: cards.length,
      firstCard: cards[0]?.outerHTML?.substring(0, 300)
    };
  });
  
  console.log('\nAlternate structure:', altStructure);
  
  console.log('\nKeeping browser open for 20s for manual inspection...');
  await page.waitForTimeout(20000);
  
  await browser.close();
}

debugIreland().catch(console.error);
