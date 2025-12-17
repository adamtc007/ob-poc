/**
 * OpenFIGI Enrichment Script
 * 
 * Takes scraped fund data and enriches with OpenFIGI lookups.
 * OpenFIGI provides: security type, exchange, ticker, etc.
 * 
 * API: https://www.openfigi.com/api
 * Rate limit: 25 requests/minute (no auth), 250/minute (with key)
 */

import * as fs from 'fs';
import * as path from 'path';

const OPENFIGI_URL = 'https://api.openfigi.com/v3/mapping';

// Rate limiting
const BATCH_SIZE = 10;  // ISINs per request (max 100)
const DELAY_MS = 2500;  // Delay between batches (stay under rate limit)

/**
 * Look up ISINs via OpenFIGI
 * @param {string[]} isins - Array of ISINs to look up
 * @returns {Promise<Object[]>} - Array of results
 */
async function lookupIsins(isins) {
  const jobs = isins.map(isin => ({
    idType: 'ID_ISIN',
    idValue: isin
  }));

  const response = await fetch(OPENFIGI_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(jobs)
  });

  if (!response.ok) {
    throw new Error(`OpenFIGI error: ${response.status} ${response.statusText}`);
  }

  return response.json();
}

/**
 * Enrich fund data with OpenFIGI results
 */
async function enrichFunds(inputFile) {
  console.log(`📂 Reading ${inputFile}...`);
  
  const data = JSON.parse(fs.readFileSync(inputFile, 'utf-8'));
  const funds = data.funds || [];
  
  // Extract unique ISINs
  const isins = [...new Set(
    funds
      .map(f => f.isin)
      .filter(isin => isin && /^[A-Z]{2}[A-Z0-9]{10}$/.test(isin))
  )];
  
  console.log(`🔍 Found ${isins.length} unique ISINs to look up`);
  
  // Process in batches
  const enrichments = new Map();
  
  for (let i = 0; i < isins.length; i += BATCH_SIZE) {
    const batch = isins.slice(i, i + BATCH_SIZE);
    console.log(`  Batch ${Math.floor(i/BATCH_SIZE) + 1}/${Math.ceil(isins.length/BATCH_SIZE)}: ${batch.length} ISINs`);
    
    try {
      const results = await lookupIsins(batch);
      
      results.forEach((result, idx) => {
        const isin = batch[idx];
        if (result.data && result.data.length > 0) {
          // Take first result (usually the primary listing)
          const figi = result.data[0];
          enrichments.set(isin, {
            figi: figi.figi,
            name: figi.name,
            ticker: figi.ticker,
            exchCode: figi.exchCode,
            securityType: figi.securityType,
            securityType2: figi.securityType2,
            marketSector: figi.marketSector
          });
        }
      });
      
    } catch (error) {
      console.error(`  ⚠️ Batch error: ${error.message}`);
    }
    
    // Rate limit delay
    if (i + BATCH_SIZE < isins.length) {
      await new Promise(resolve => setTimeout(resolve, DELAY_MS));
    }
  }
  
  console.log(`✅ Enriched ${enrichments.size}/${isins.length} ISINs`);
  
  // Apply enrichments to funds
  const enrichedFunds = funds.map(fund => {
    if (fund.isin && enrichments.has(fund.isin)) {
      return {
        ...fund,
        figi: enrichments.get(fund.isin)
      };
    }
    return fund;
  });
  
  // Write output
  const outputFile = inputFile.replace('.json', '-enriched.json');
  const output = {
    ...data,
    metadata: {
      ...data.metadata,
      enrichedAt: new Date().toISOString(),
      figiEnrichedCount: enrichments.size
    },
    funds: enrichedFunds
  };
  
  fs.writeFileSync(outputFile, JSON.stringify(output, null, 2));
  console.log(`💾 Saved to ${outputFile}`);
  
  return outputFile;
}

// CLI
const inputFile = process.argv[2];

if (!inputFile) {
  console.log('Usage: node enrich-figi.js <input-file.json>');
  console.log('');
  console.log('Example:');
  console.log('  node enrich-figi.js output/allianz-lu-2024-12-16.json');
  process.exit(1);
}

enrichFunds(inputFile).catch(err => {
  console.error('❌ Error:', err.message);
  process.exit(1);
});
