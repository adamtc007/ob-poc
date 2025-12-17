/**
 * Debug detail page APIs
 */

import { chromium } from 'playwright';

async function debugDetailApi() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  
  const apiCalls = [];
  
  page.on('response', async (response) => {
    const url = response.url();
    const ct = response.headers()['content-type'] || '';
    if (ct.includes('json') && !url.includes('cookie') && !url.includes('twitter')) {
      try {
        const data = await response.json();
        apiCalls.push({ url: url.substring(0, 100), keys: Object.keys(data).slice(0, 10) });
      } catch (e) {}
    }
  });
  
  const url = 'https://regulatory.allianzgi.com/en-GB/B2C/Luxemburg-EN/funds/mutual-funds/allianz-income-and-growth-am-usd';
  
  console.log('Loading detail page...');
  await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 });
  
  try {
    await page.click('button:has-text("Confirm")', { timeout: 5000 });
    await page.waitForTimeout(2000);
  } catch (e) {}
  
  await page.waitForTimeout(3000);
  
  console.log('\n--- API Calls on Detail Page ---');
  apiCalls.forEach(c => {
    console.log(`${c.url}`);
    console.log(`  Keys: ${c.keys.join(', ')}`);
  });
  
  await browser.close();
}

debugDetailApi().catch(console.error);
