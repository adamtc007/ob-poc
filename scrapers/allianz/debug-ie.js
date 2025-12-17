/**
 * Debug Irish site to find API endpoints
 */

import { chromium } from 'playwright';

async function debug() {
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  const apiCalls = [];
  
  page.on('response', async (response) => {
    const url = response.url();
    const contentType = response.headers()['content-type'] || '';
    
    // Log all API-like calls
    if (url.includes('api') || url.includes('fund') || contentType.includes('json')) {
      console.log(`📡 ${response.status()} ${url.substring(0, 120)}`);
      
      if (contentType.includes('json')) {
        try {
          const data = await response.json();
          apiCalls.push({ url, keys: Object.keys(data), sample: JSON.stringify(data).substring(0, 200) });
        } catch (e) {}
      }
    }
  });
  
  // Try Ireland URL
  console.log('\n=== Trying Ireland ===');
  await page.goto('https://regulatory.allianzgi.com/en-gb/b2c/ireland-en/funds/mutual-funds', {
    waitUntil: 'networkidle',
    timeout: 60000
  });
  
  // Accept disclaimer
  try {
    await page.click('button:has-text("Confirm")', { timeout: 5000 });
    await page.waitForTimeout(3000);
  } catch (e) {}
  
  await page.waitForTimeout(5000);
  
  // Check what's on the page
  const pageInfo = await page.evaluate(() => {
    const tables = document.querySelectorAll('table');
    const fundLinks = Array.from(document.querySelectorAll('a')).filter(a => 
      a.href.includes('fund') && !a.href.endsWith('funds/mutual-funds')
    ).map(a => a.href);
    
    return {
      title: document.title,
      tables: tables.length,
      fundLinks: fundLinks.slice(0, 10),
      bodyPreview: document.body.innerText.substring(0, 1000)
    };
  });
  
  console.log('\nPage title:', pageInfo.title);
  console.log('Tables:', pageInfo.tables);
  console.log('Fund links found:', pageInfo.fundLinks.length);
  pageInfo.fundLinks.forEach(l => console.log('  ', l));
  
  console.log('\n=== API Calls with JSON ===');
  apiCalls.forEach(a => {
    console.log(`URL: ${a.url}`);
    console.log(`Keys: ${a.keys.join(', ')}`);
    console.log(`Sample: ${a.sample}\n`);
  });
  
  // Also try the main allianzgi.com site for Ireland
  console.log('\n=== Trying ie.allianzgi.com ===');
  await page.goto('https://ie.allianzgi.com/', { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(3000);
  
  const ieInfo = await page.evaluate(() => ({
    title: document.title,
    fundLinks: Array.from(document.querySelectorAll('a')).filter(a => 
      a.href.includes('fund')
    ).map(a => a.href).slice(0, 10)
  }));
  
  console.log('IE site title:', ieInfo.title);
  console.log('Fund links:', ieInfo.fundLinks);
  
  console.log('\nBrowser open for 20 seconds...');
  await page.waitForTimeout(20000);
  
  await browser.close();
}

debug().catch(console.error);
