use std::fs;
use ob_poc::gleif::types::{GleifResponse, LeiRecord};

fn main() {
    let content = fs::read_to_string("/tmp/gleif_response.json").expect("read file");
    let value: serde_json::Value = serde_json::from_str(&content).expect("parse json");
    
    let data = value.get("data").and_then(|d| d.as_array()).expect("get data array");
    
    println!("Total records: {}", data.len());
    
    let mut failures = Vec::new();
    for (i, record) in data.iter().enumerate() {
        let record_str = serde_json::to_string(record).unwrap();
        if let Err(e) = serde_json::from_str::<LeiRecord>(&record_str) {
            let lei = record.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            failures.push((i, lei.to_string(), e.to_string()));
        }
    }
    
    if failures.is_empty() {
        println!("All {} records parsed successfully!", data.len());
    } else {
        println!("\n{} records failed to parse:", failures.len());
        for (i, lei, err) in &failures {
            println!("\n  Record {} (lei: {})", i, lei);
            println!("    Error: {}", err);
        }
    }
}
