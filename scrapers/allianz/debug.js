/**
 * Debug script to inspect the Allianz regulatory site structure
 * and find the actual API endpoints
 */

import { chromium } from 'playwright';

async function debug() {
  const browser = await chromium.launch({ headless: false }); // Visible for debugging
  const context = await browser.newContext();
  const page = await context.newPage();
  
  // Intercept all network requests
  const apiCalls = [];
  page.on('response', async (response) => {
    const url = response.url();
    if (url.includes('api') || url.includes('fund') || url.includes('json')) {
      const contentType = response.headers()['content-type'] || '';
      if (contentType.includes('json')) {
        try {
          const data = await response.json();
          apiCalls.push({ url, data: JSON.stringify(data).substring(0, 500) });
          console.log(`📡 API: ${url}`);
        } catch (e) {}
      }
    }
  });
  
  console.log('Navigating to fund list...');
  await page.goto('https://regulatory.allianzgi.com/en-gb/b2c/luxemburg-en/funds/mutual-funds', {
    waitUntil: 'networkidle',
    timeout: 60000
  });
  
  // Accept disclaimer
  try {
    await page.click('button:has-text("Confirm")', { timeout: 5000 });
    console.log('Clicked confirm');
    await page.waitForTimeout(3000);
  } catch (e) {
    console.log('No confirm button');
  }
  
  // Wait for content
  await page.waitForTimeout(5000);
  
  // Get page HTML
  const html = await page.content();
  console.log('\n--- Page structure ---');
  console.log('HTML length:', html.length);
  
  // Look for fund-related elements
  const fundElements = await page.evaluate(() => {
    const results = {
      tables: document.querySelectorAll('table').length,
      links: [],
      dataAttrs: []
    };
    
    // Find all links
    document.querySelectorAll('a').forEach(a => {
      if (a.href && a.href.includes('fund')) {
        results.links.push(a.href);
      }
    });
    
    // Find elements with data attributes
    document.querySelectorAll('[data-fund], [data-isin], [class*="fund"]').forEach(el => {
      results.dataAttrs.push({
        tag: el.tagName,
        class: el.className,
        text: el.textContent?.substring(0, 100)
      });
    });
    
    // Find any JSON data in page
    const scripts = document.querySelectorAll('script');
    scripts.forEach(s => {
      if (s.textContent && s.textContent.includes('fund')) {
        results.scriptData = s.textContent.substring(0, 500);
      }
    });
    
    return results;
  });
  
  console.log('\nTables found:', fundElements.tables);
  console.log('Fund-related links:', fundElements.links.slice(0, 10));
  console.log('Data attrs:', fundElements.dataAttrs.slice(0, 5));
  
  console.log('\n--- API calls captured ---');
  apiCalls.forEach(a => console.log(a.url));
  
  // Keep browser open for inspection
  console.log('\nBrowser staying open for 30 seconds...');
  await page.waitForTimeout(30000);
  
  await browser.close();
}

debug().catch(console.error);
