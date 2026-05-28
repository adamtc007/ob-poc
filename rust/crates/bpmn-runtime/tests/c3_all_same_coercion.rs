use bpmn_runtime::{apply_merge_protocol, ActiveToken, MergeResult, WriteLogEntry};
use dsl_lowering::{JourneyMergeClause, JourneyParallelJoin};
use serde_json::json;
use uuid::Uuid;

#[test]
fn test_c3_all_same_bypass_is_coerced() {
    let t1 = ActiveToken {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        current_node: "join1".to_string(),
        fork_ref: None,
        branch_lineage: vec!["fork1".to_string()],
        write_log: vec![WriteLogEntry {
            location: "tags".to_string(),
            value: json!("us"),
        }],
    };
    let t2 = ActiveToken {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        current_node: "join1".to_string(),
        fork_ref: None,
        branch_lineage: vec!["fork1".to_string()],
        write_log: vec![WriteLogEntry {
            location: "tags".to_string(),
            value: json!("us"),
        }],
    };

    let join_spec = JourneyParallelJoin {
        name: "join1".to_string(),
        expects: vec!["fork1".to_string()],
        merge: vec![JourneyMergeClause {
            location: "tags".to_string(),
            operator: "union".to_string(),
            custom_verb: None,
        }],
    };

    let result = apply_merge_protocol(&[t1, t2], Some(&join_spec));
    match result {
        MergeResult::Ok(data) => {
            let val = data.get("tags").expect("Expected tags location");
            // The all_same bypass would incorrectly return "us".
            // It MUST be coerced through the union operator to ["us", "us"].
            assert_eq!(
                val,
                &json!(["us", "us"]),
                "Expected union operator to convert two identical strings to an array, not bypass"
            );
        }
        _ => panic!("Expected MergeResult::Ok"),
    }
}
