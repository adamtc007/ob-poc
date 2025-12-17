/**
 * Convert scraped Allianz fund data to ob-poc DSL
 * 
 * Takes the JSON output from scrape-allianz.js and generates
 * DSL statements for importing funds as CBUs.
 * 
 * Usage:
 *   node to-dsl.js output/allianz-lu-2024-12-16.json > allianz-import.dsl
 */

import * as fs from 'fs';

// ============================================================================
// Template: Fund Onboarding
// ============================================================================

/**
 * Generate DSL for a single fund (with all its share classes as one CBU)
 */
function fundToDsl(fund, manco) {
  const lines = [];
  
  // Block for this fund
  lines.push(`(block ; Fund: ${fund.name}`);
  
  // Ensure ManCo entity exists
  lines.push(`  (bind @manco (entity.ensure-company`);
  lines.push(`    :name "${manco.name}"`);
  lines.push(`    :jurisdiction "${manco.jurisdiction}"`);
  lines.push(`    :registration-number "${manco.regNumber || ''}"))`);
  
  // Create CBU for the fund
  lines.push(`  (bind @cbu (cbu.create`);
  lines.push(`    :name "${escapeString(fund.name)}"`);
  lines.push(`    :cbu-type "FUND"`);
  lines.push(`    :jurisdiction "${fund.isin?.substring(0, 2) || 'LU'}"`);
  if (fund.isin) {
    lines.push(`    :isin "${fund.isin}"`);
  }
  if (fund.currency) {
    lines.push(`    :base-currency "${fund.currency}"`);
  }
  if (fund.assetClass) {
    lines.push(`    :asset-class "${fund.assetClass}"`);
  }
  if (fund.sfdrCategory) {
    lines.push(`    :sfdr-category "${fund.sfdrCategory}"`);
  }
  lines.push(`    ))`);
  
  // Assign ManCo role
  lines.push(`  (cbu.assign-role`);
  lines.push(`    :cbu @cbu`);
  lines.push(`    :entity @manco`);
  lines.push(`    :role MANCO)`);
  
  // Request standard products
  lines.push(`  (onboarding.request`);
  lines.push(`    :cbu @cbu`);
  lines.push(`    :products ["custody" "fund-accounting"])`);
  
  // Create KYC case
  lines.push(`  (kyc.create-case`);
  lines.push(`    :cbu @cbu`);
  lines.push(`    :case-type "standard")`);
  
  lines.push(`)`);  // End block
  lines.push('');
  
  return lines.join('\n');
}

/**
 * Generate DSL for share class as separate entity
 * (Alternative approach - each share class as its own record)
 */
function shareClassToDsl(fund, shareClass, manco) {
  const name = `${fund.name} - ${shareClass.name}`;
  
  return `
(block ; Share Class: ${name}
  (bind @fund (cbu.lookup :isin "${fund.isin}"))
  (cbu.add-share-class
    :cbu @fund
    :share-class "${shareClass.name}"
    :isin "${shareClass.isin}"
    :currency "${shareClass.currency}"
    :distribution-policy "${shareClass.distributionPolicy || 'ACCUMULATING'}"))
`;
}

/**
 * Escape string for DSL
 */
function escapeString(str) {
  if (!str) return '';
  return str
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/\n/g, '\\n');
}

// ============================================================================
// Main
// ============================================================================

function main() {
  const inputFile = process.argv[2];
  
  if (!inputFile) {
    console.error('Usage: node to-dsl.js <input-file.json>');
    process.exit(1);
  }
  
  const data = JSON.parse(fs.readFileSync(inputFile, 'utf-8'));
  const { metadata, funds } = data;
  
  // Header comment
  console.log(`;; ============================================================================`);
  console.log(`;; Allianz Fund Import - ${metadata.jurisdictionName}`);
  console.log(`;; Generated: ${new Date().toISOString()}`);
  console.log(`;; Source: ${inputFile}`);
  console.log(`;; Funds: ${funds.length}`);
  console.log(`;; ManCo: ${metadata.manco.name}`);
  console.log(`;; ============================================================================`);
  console.log('');
  
  // Generate DSL for each fund
  for (const fund of funds) {
    if (!fund.name) continue;
    console.log(fundToDsl(fund, metadata.manco));
  }
  
  console.log(`;; End of import - ${funds.length} funds`);
}

main();
