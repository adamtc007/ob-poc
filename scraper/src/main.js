/**
 * Main Orchestrator - Allianz Fund Data Pipeline
 * 
 * Combines data from:
 * 1. CSSF official registry (authoritative fund list, ISINs)
 * 2. Allianz regulatory site (SFDR, investment mandates, documents)
 * 
 * Output: data/allianz-import.json (ready for ob-poc import)
 */

import { readFileSync, writeFileSync, existsSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { execSync } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const DATA_DIR = join(__dirname, '..', 'data');

/**
 * Merge CSSF and Allianz scraped data
 */
function mergeData(cssfData, allianzData) {
  console.log('🔗 Merging data sources...');
  
  // Create ISIN lookup from Allianz scraped data
  const allianzByIsin = new Map();
  for (const fund of (allianzData?.funds || [])) {
    if (fund.isin) {
      allianzByIsin.set(fund.isin, fund);
    }
  }
  
  console.log(`   CSSF funds: ${cssfData?.funds?.length || 0}`);
  console.log(`   Allianz details: ${allianzByIsin.size} ISINs`);
  
  // Enrich CSSF data with Allianz details
  const enrichedFunds = [];
  
  for (const fund of (cssfData?.funds || [])) {
    const enrichedCompartments = [];
    
    for (const compartment of (fund.compartments || [])) {
      const enrichedShareClasses = [];
      
      for (const shareClass of (compartment.share_classes || [])) {
        const allianzDetails = allianzByIsin.get(shareClass.isin);
        
        enrichedShareClasses.push({
          ...shareClass,
          // Enriched from Allianz site
          sfdr_category: allianzDetails?.sfdr_category || null,
          morningstar_rating: allianzDetails?.morningstar_rating || null,
          nav: allianzDetails?.nav || null,
          nav_date: allianzDetails?.nav_date || null,
          documents: allianzDetails?.documents || [],
        });
      }
      
      enrichedCompartments.push({
        ...compartment,
        share_classes: enrichedShareClasses,
        // Compartment-level data from first share class with details
        investment_objective: compartment.share_classes
          .map(sc => allianzByIsin.get(sc.isin)?.investment_objective)
          .find(obj => obj) || null,
        key_risks: compartment.share_classes
          .map(sc => allianzByIsin.get(sc.isin)?.key_risks)
          .find(risks => risks?.length > 0) || [],
        benchmark: compartment.share_classes
          .map(sc => allianzByIsin.get(sc.isin)?.benchmark)
          .find(b => b) || null,
      });
    }
    
    enrichedFunds.push({
      ...fund,
      compartments: enrichedCompartments,
    });
  }
  
  return enrichedFunds;
}

/**
 * Transform to ob-poc import format
 */
function transformToObPocFormat(funds) {
  console.log('🔄 Transforming to ob-poc import format...');
  
  const entities = [];
  const cbus = [];
  const relationships = [];
  
  // Track unique ManCos
  const mancos = new Map();
  
  for (const fund of funds) {
    // ManCo entity
    const mancoName = fund.management_company || 'Allianz Global Investors GmbH';
    if (!mancos.has(mancoName)) {
      mancos.set(mancoName, {
        type: 'entity',
        entity_type: 'LIMITED_COMPANY',
        name: mancoName,
        jurisdiction: 'LU',
        roles: ['MANCO'],
      });
    }
    
    // Each compartment becomes a CBU
    for (const compartment of (fund.compartments || [])) {
      const cbuName = compartment.compartment_name || fund.fund_name;
      
      // CBU
      const cbu = {
        type: 'cbu',
        name: cbuName,
        fund_structure: fund.legal_form,
        ucits_type: fund.ucits_type,
        products: ['custody', 'fund-accounting'], // Default products
        manco: mancoName,
        
        // Share class info
        share_classes: compartment.share_classes.map(sc => ({
          name: sc.share_class_name,
          isin: sc.isin,
          currency: sc.currency,
          sfdr_category: sc.sfdr_category,
        })),
        
        // Investment info
        investment_objective: compartment.investment_objective,
        benchmark: compartment.benchmark,
        key_risks: compartment.key_risks,
        
        // Documents
        documents: compartment.share_classes
          .flatMap(sc => sc.documents || [])
          .filter((doc, idx, arr) => 
            arr.findIndex(d => d.url === doc.url) === idx
          ),
      };
      
      cbus.push(cbu);
      
      // Relationship: ManCo -> CBU
      relationships.push({
        from_type: 'entity',
        from_name: mancoName,
        to_type: 'cbu',
        to_name: cbuName,
        role: 'MANCO',
      });
    }
  }
  
  return {
    entities: Array.from(mancos.values()),
    cbus: cbus,
    relationships: relationships,
  };
}

/**
 * Generate DSL for import
 */
function generateDsl(importData) {
  console.log('📝 Generating DSL...');
  
  const lines = [];
  
  // Header comment
  lines.push(`;; Allianz Luxembourg Funds Import`);
  lines.push(`;; Generated: ${new Date().toISOString()}`);
  lines.push(`;; Source: CSSF + regulatory.allianzgi.com`);
  lines.push('');
  
  // Create ManCo entities
  lines.push(';; === Management Companies ===');
  for (const entity of importData.entities) {
    lines.push(`(entity.create-company`);
    lines.push(`  :name "${entity.name}"`);
    lines.push(`  :jurisdiction "${entity.jurisdiction}"`);
    lines.push(`  :entity-type "${entity.entity_type}")`);
    lines.push('');
  }
  
  // Create CBUs (first 10 for sample)
  lines.push(';; === CBUs (Funds) ===');
  const sampleCbus = importData.cbus.slice(0, 10);
  for (const cbu of sampleCbus) {
    lines.push(`(cbu.create`);
    lines.push(`  :name "${cbu.name.replace(/"/g, '\\"')}"`);
    if (cbu.share_classes[0]?.isin) {
      lines.push(`  :isin "${cbu.share_classes[0].isin}"`);
    }
    lines.push(`  :products ["custody" "fund-accounting"])`);
    lines.push('');
  }
  
  lines.push(`;; ... and ${importData.cbus.length - 10} more CBUs`);
  
  return lines.join('\n');
}

/**
 * Main execution
 */
async function main() {
  console.log('═══════════════════════════════════════════════');
  console.log('  Allianz Fund Data Pipeline');
  console.log('═══════════════════════════════════════════════\n');
  
  // Check for input files
  const cssfPath = join(DATA_DIR, 'cssf-allianz-funds.json');
  const allianzPath = join(DATA_DIR, 'allianz-fund-details.json');
  
  // Run CSSF download if needed
  if (!existsSync(cssfPath)) {
    console.log('📥 CSSF data not found, running download...\n');
    execSync('node src/cssf-download.js', { 
      cwd: join(__dirname, '..'),
      stdio: 'inherit',
    });
  }
  
  // Load CSSF data
  let cssfData = null;
  if (existsSync(cssfPath)) {
    cssfData = JSON.parse(readFileSync(cssfPath, 'utf-8'));
    console.log(`✓ Loaded CSSF data: ${cssfData.funds?.length || 0} funds`);
  } else {
    console.log('⚠ CSSF data not available');
  }
  
  // Load Allianz scraped data (optional enrichment)
  let allianzData = null;
  if (existsSync(allianzPath)) {
    allianzData = JSON.parse(readFileSync(allianzPath, 'utf-8'));
    console.log(`✓ Loaded Allianz details: ${allianzData.funds?.length || 0} funds`);
  } else {
    console.log('ℹ Allianz details not available (run: npm run allianz)');
  }
  
  console.log('');
  
  // Merge data sources
  const mergedFunds = mergeData(cssfData, allianzData);
  
  // Transform to ob-poc format
  const importData = transformToObPocFormat(mergedFunds);
  
  // Save import file
  const importPath = join(DATA_DIR, 'allianz-import.json');
  writeFileSync(importPath, JSON.stringify({
    source: 'Allianz Fund Pipeline',
    generated_at: new Date().toISOString(),
    summary: {
      entities: importData.entities.length,
      cbus: importData.cbus.length,
      relationships: importData.relationships.length,
    },
    data: importData,
  }, null, 2));
  console.log(`\n✅ Saved: ${importPath}`);
  
  // Generate DSL sample
  const dsl = generateDsl(importData);
  const dslPath = join(DATA_DIR, 'allianz-import-sample.dsl');
  writeFileSync(dslPath, dsl);
  console.log(`✅ Saved: ${dslPath}`);
  
  // Summary
  console.log('\n═══════════════════════════════════════════════');
  console.log('  Import Ready');
  console.log('═══════════════════════════════════════════════');
  console.log(`  ManCos:         ${importData.entities.length}`);
  console.log(`  CBUs (Funds):   ${importData.cbus.length}`);
  console.log(`  Relationships:  ${importData.relationships.length}`);
  console.log('═══════════════════════════════════════════════\n');
  
  // Sample CBUs
  console.log('Sample CBUs:');
  for (const cbu of importData.cbus.slice(0, 5)) {
    const isin = cbu.share_classes[0]?.isin || 'N/A';
    const sfdr = cbu.share_classes[0]?.sfdr_category || 'N/A';
    console.log(`  • ${cbu.name}`);
    console.log(`    ISIN: ${isin}, SFDR: ${sfdr}`);
  }
  if (importData.cbus.length > 5) {
    console.log(`  ... and ${importData.cbus.length - 5} more`);
  }
}

main().catch(console.error);
