//! O1 (no-panic) + O3 (decode/encode roundtrip) over `Hash::from_hex`.
//!
//! `Hash::from_hex` is the only externally-reachable decoder in the
//! substrate's public API surface (`edge_kind_from_payload` /
//! `structure_class_from_payload` are private — they're exercised
//! indirectly by the `fold_control` target instead). It parses the
//! `lexicon_hash`/`payload_hash` text columns coming back off the durable
//! store, so it must never panic on adversarial input, and any string it
//! accepts must round-trip through `to_hex`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate::Hash;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    if let Ok(h) = Hash::from_hex(s) {
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64, "to_hex must always emit 64 hex chars");
        assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "to_hex must be lowercase hex: {hex:?}"
        );
        let reparsed = Hash::from_hex(&hex).expect("to_hex output must always re-parse");
        assert_eq!(h, reparsed, "from_hex(to_hex(h)) must equal h");
    }
});
