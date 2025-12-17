/**
 * CSSF Fund Registry Download & Parse
 * 
 * Downloads the official Luxembourg CSSF registry of UCIs (UCITS, SIFs, SICARs)
 * and filters for Allianz funds.
 * 
 * Output: data/cssf-allianz-funds.json
 */

import fetch from 'node-fetch';
import AdmZip from 'adm-zip';
import { parse } from 'csv-parse/sync';
import { writeFileSync, mkdirSync, existsSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// CSSF official registry URL
const CSSF_ZIP_URL = 'https://www.cssf.lu/wp-content/uploads/OPC_FIS_SICAR_COMP_TP_TOUS_NON_FERMES.zip';

// Output directory
const DATA_DIR = join(__dirname, '..', 'data');

/**
 * Download and extract the CSSF ZIP file
 */
async function downloadCssfData() {
  console.log('📥 Downloading CSSF registry data...');
  
  const response = await fetch(CSSF_ZIP_URL);
  if (!response.ok) {
    throw new Error(`Failed to download: ${response.status} ${response.statusText}`);
  }
  
  const buffer = Buffer.from(await response.arrayBuffer());
  console.log(`   Downloaded ${(buffer.length / 1024 / 1024).toFixed(2)} MB`);
  
  return buffer;
}

/**
 * Extract CSV files from ZIP
 */
function extractCsvFiles(zipBuffer) {
  console.log('📦 Extracting ZIP contents...');
  
  const zip = new AdmZip(zipBuffer);
  const entries = zip.getEntries();
  
  const csvFiles = {};
  for (const entry of entries) {
    if (entry.entryName.endsWith('.csv')) {
      console.log(`   Found: ${entry.entryName}`);
      const rawBuffer = entry.getData();
      
      // CSSF files are UTF-16 LE encoded
      // Detect BOM and decode accordingly
      let content;
      if (rawBuffer[0] === 0xFF && rawBuffer[1] === 0xFE) {
        // UTF-16 LE with BOM
        content = rawBuffer.toString('utf16le').slice(1); // Remove BOM
        console.log(`   Encoding: UTF-16 LE`);
      } else if (rawBuffer[0] === 0xFE && rawBuffer[1] === 0xFF) {
        // UTF-16 BE with BOM
        content = Buffer.from(rawBuffer).swap16().toString('utf16le').slice(1);
        console.log(`   Encoding: UTF-16 BE`);
      } else {
        // Assume UTF-8
        content = rawBuffer.toString('utf-8');
        console.log(`   Encoding: UTF-8`);
      }
      
      csvFiles[entry.entryName] = content;
    }
  }
  
  return csvFiles;
}

/**
 * Parse CSV and filter for Allianz funds
 */
function parseAndFilterAllianz(csvContent, filename) {
  console.log(`🔍 Parsing ${filename}...`);
  
  // CSSF uses tab delimiter
  const records = parse(csvContent, {
    columns: true,
    delimiter: '\t',
    skip_empty_lines: true,
    relax_column_count: true,
    relax_quotes: true,
    skip_records_with_error: true,
    trim: true,
  });
  
  console.log(`   Total records: ${records.length}`);
  
  // Filter for Allianz - check multiple name fields
  const allianzRecords = records.filter(record => {
    const searchFields = [
      record['NOMOPC'] || record['NOM_OPC'] || '',
      record['NOMCOMPARTIMENT'] || record['NOM_COMPARTIMENT'] || '',
      record['NOMTYPEPART'] || record['NOM_TYPE_PARTS'] || '',
      record['DENOMINATION'] || '',
      record['PROMOTEUR'] || '',
    ];
    
    return searchFields.some(field => 
      field.toLowerCase().includes('allianz')
    );
  });
  
  console.log(`   Allianz records: ${allianzRecords.length}`);
  
  return allianzRecords;
}

/**
 * Transform CSSF records to normalized schema
 */
function normalizeRecords(records) {
  return records.map(record => ({
    // Fund level - from actual CSSF column names
    fund_id: record['NNNNNNNN'] || null,
    fund_name: record['NOMOPC'] || null,
    fund_type: record['E'] || null,  // O = OPC (UCI)
    legal_form: null,  // Not in this file
    
    // Compartment/Sub-fund level
    compartment_id: record['CCCCCCCC'] || null,
    compartment_name: record['NOMCOMPARTIMENT'] || null,
    
    // Share class level
    share_class_id: record['PPPP'] || null,
    share_class_name: record['NOMTYPEPART'] || null,
    isin: null,  // Not in this CSSF file - will come from Allianz scraper
    currency: record['DEVISECOMP'] || null,
    
    // Dates
    launch_date: record['AGREEMENTCOMP'] || null,
    
    // These fields not in this file - need different CSSF file or Allianz site
    promoter: null,
    management_company: null,
    depositary: null,
    ucits_type: null,
    sfdr_classification: null,
    
    // Raw record for debugging
    _raw: record,
  }));
}

/**
 * Aggregate share classes into fund structure
 */
function aggregateToFunds(normalizedRecords) {
  const fundsMap = new Map();
  
  for (const record of normalizedRecords) {
    const fundKey = record.fund_id || record.fund_name;
    
    if (!fundsMap.has(fundKey)) {
      fundsMap.set(fundKey, {
        fund_id: record.fund_id,
        fund_name: record.fund_name,
        fund_type: record.fund_type,
        legal_form: record.legal_form,
        promoter: record.promoter,
        management_company: record.management_company,
        depositary: record.depositary,
        launch_date: record.launch_date,
        ucits_type: record.ucits_type,
        compartments: new Map(),
      });
    }
    
    const fund = fundsMap.get(fundKey);
    const compartmentKey = record.compartment_id || record.compartment_name || 'default';
    
    if (!fund.compartments.has(compartmentKey)) {
      fund.compartments.set(compartmentKey, {
        compartment_id: record.compartment_id,
        compartment_name: record.compartment_name,
        share_classes: [],
      });
    }
    
    if (record.isin || record.share_class_name) {
      fund.compartments.get(compartmentKey).share_classes.push({
        share_class_id: record.share_class_id,
        share_class_name: record.share_class_name,
        isin: record.isin,
        currency: record.currency,
      });
    }
  }
  
  // Convert to array structure
  return Array.from(fundsMap.values()).map(fund => ({
    ...fund,
    compartments: Array.from(fund.compartments.values()),
  }));
}

/**
 * Main execution
 */
async function main() {
  console.log('═══════════════════════════════════════════════');
  console.log('  CSSF Fund Registry - Allianz Extraction');
  console.log('═══════════════════════════════════════════════\n');
  
  // Ensure output directory exists
  if (!existsSync(DATA_DIR)) {
    mkdirSync(DATA_DIR, { recursive: true });
  }
  
  try {
    // Download ZIP
    const zipBuffer = await downloadCssfData();
    
    // Extract CSVs
    const csvFiles = extractCsvFiles(zipBuffer);
    
    // Process each CSV file
    let allAllianzRecords = [];
    
    for (const [filename, content] of Object.entries(csvFiles)) {
      const allianzRecords = parseAndFilterAllianz(content, filename);
      allAllianzRecords = allAllianzRecords.concat(allianzRecords);
    }
    
    console.log(`\n📊 Total Allianz records found: ${allAllianzRecords.length}`);
    
    // Normalize and aggregate
    const normalized = normalizeRecords(allAllianzRecords);
    const funds = aggregateToFunds(normalized);
    
    console.log(`📁 Aggregated into ${funds.length} funds\n`);
    
    // Output results
    const output = {
      source: 'CSSF Luxembourg',
      source_url: CSSF_ZIP_URL,
      extracted_at: new Date().toISOString(),
      filter: 'Allianz',
      summary: {
        total_funds: funds.length,
        total_compartments: funds.reduce((sum, f) => sum + f.compartments.length, 0),
        total_share_classes: funds.reduce((sum, f) => 
          sum + f.compartments.reduce((s, c) => s + c.share_classes.length, 0), 0
        ),
      },
      funds: funds,
    };
    
    // Save JSON
    const jsonPath = join(DATA_DIR, 'cssf-allianz-funds.json');
    writeFileSync(jsonPath, JSON.stringify(output, null, 2));
    console.log(`✅ Saved: ${jsonPath}`);
    
    // Save raw records for debugging
    const rawPath = join(DATA_DIR, 'cssf-allianz-raw.json');
    writeFileSync(rawPath, JSON.stringify(allAllianzRecords, null, 2));
    console.log(`✅ Saved: ${rawPath}`);
    
    // Print summary
    console.log('\n═══════════════════════════════════════════════');
    console.log('  Summary');
    console.log('═══════════════════════════════════════════════');
    console.log(`  Funds:         ${output.summary.total_funds}`);
    console.log(`  Compartments:  ${output.summary.total_compartments}`);
    console.log(`  Share Classes: ${output.summary.total_share_classes}`);
    console.log('═══════════════════════════════════════════════\n');
    
    // Sample output
    if (funds.length > 0) {
      console.log('Sample fund:');
      console.log(JSON.stringify(funds[0], null, 2));
    }
    
  } catch (error) {
    console.error('❌ Error:', error.message);
    process.exit(1);
  }
}

main();
