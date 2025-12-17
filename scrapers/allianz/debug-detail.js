/**
 * Debug fund detail page structure
 */

import { chromium } from 'playwright';

async function debug() {
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage();
  
  const url = 'https://regulatory.allianzgi.com/en-gb/b2c/luxemburg-en/funds/mutual-funds/allianz-green-bond-pt-h2-usd-usd';
  
  console.log('Loading:', url);
  await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 });
  
  // Accept disclaimer
  try {
    await page.click('button:has-text("Confirm")', { timeout: 5000 });
    await page.waitForTimeout(3000);
  } catch (e) {}
  
  await page.waitForTimeout(3000);
  
  // Analyze page structure
  const analysis = await page.evaluate(() => {
    const sections = [];
    
    // Get all headings
    document.querySelectorAll('h1, h2, h3, h4, .section-title, [class*="title"], [class*="heading"]').forEach(el => {
      sections.push({
        tag: el.tagName,
        class: el.className,
        text: el.textContent.trim().substring(0, 100)
      });
    });
    
    // Look for investment strategy section
    const strategySection = document.querySelector('[class*="strategy"], [class*="objective"], [id*="strategy"], [id*="objective"]');
    
    // Look for key facts / overview
    const keyFacts = document.querySelector('[class*="key-fact"], [class*="overview"], [class*="fund-info"]');
    
    // Find all accordion / expandable sections
    const accordions = document.querySelectorAll('[class*="accordion"], [class*="collapse"], [class*="expand"]');
    
    // Get specific data points by text patterns
    const pageText = document.body.innerText;
    const patterns = {
      objective: pageText.match(/Investment (?:Objective|Strategy)[:\s]*([^.]+\.)/i),
      benchmark: pageText.match(/Benchmark[:\s]+([^\n]+)/i),
      fee: pageText.match(/(?:Management Fee|Ongoing Charge|TER)[:\s]*([\d.]+%?)/i),
      aum: pageText.match(/(?:Fund Size|Net Assets|AUM)[:\s]*([^\n]+)/i),
      inception: pageText.match(/(?:Inception|Launch) Date[:\s]*([^\n]+)/i)
    };
    
    return {
      sections: sections.slice(0, 20),
      hasStrategy: !!strategySection,
      strategyClass: strategySection?.className,
      hasKeyFacts: !!keyFacts,
      keyFactsClass: keyFacts?.className,
      accordionCount: accordions.length,
      patterns: Object.fromEntries(
        Object.entries(patterns).map(([k, v]) => [k, v ? v[1] : null])
      ),
      sampleText: pageText.substring(0, 2000)
    };
  });
  
  console.log('\n=== Page Structure ===');
  console.log('Sections found:');
  analysis.sections.forEach(s => console.log(`  ${s.tag} [${s.class}]: ${s.text}`));
  
  console.log('\nStrategy section:', analysis.hasStrategy, analysis.strategyClass);
  console.log('Key facts:', analysis.hasKeyFacts, analysis.keyFactsClass);
  console.log('Accordions:', analysis.accordionCount);
  
  console.log('\n=== Extracted Data ===');
  console.log('Objective:', analysis.patterns.objective);
  console.log('Benchmark:', analysis.patterns.benchmark);
  console.log('Fee:', analysis.patterns.fee);
  console.log('AUM:', analysis.patterns.aum);
  console.log('Inception:', analysis.patterns.inception);
  
  console.log('\n=== Sample Text ===');
  console.log(analysis.sampleText);
  
  console.log('\nBrowser staying open for 30 seconds...');
  await page.waitForTimeout(30000);
  
  await browser.close();
}

debug().catch(console.error);
