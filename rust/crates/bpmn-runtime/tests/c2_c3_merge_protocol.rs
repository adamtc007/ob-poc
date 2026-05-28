use bpmn_runtime::{apply_merge_protocol, ActiveToken, MergeResult, WriteLogEntry};
use dsl_lowering::{JourneyMergeClause, JourneyParallelJoin};
use serde_json::json;
use uuid::Uuid;

#[test]
fn test_c2_sequential_writes_deduplication() {
    let t1 = ActiveToken {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        current_node: "join1".to_string(),
        fork_ref: None,
        branch_lineage: vec!["fork1".to_string()],
        write_log: vec![
            WriteLogEntry {
                location: "counter".to_string(),
                value: json!(5.0),
            },
            WriteLogEntry {
                location: "counter".to_string(),
                value: json!(10.0),
            },
        ],
    };

    let join_spec = JourneyParallelJoin {
        name: "join1".to_string(),
        expects: vec!["fork1".to_string()],
        merge: vec![JourneyMergeClause {
            location: "counter".to_string(),
            operator: "sum".to_string(),
            custom_verb: None,
        }],
    };

    let result = apply_merge_protocol(&[t1], Some(&join_spec));
    match result {
        MergeResult::Ok(data) => {
            let val = data.get("counter").expect("Expected counter location");
            assert_eq!(
                val,
                &json!(10.0),
                "Expected sequential writes to be deduplicated to the latest value (10.0)"
            );
        }
        _ => panic!("Expected MergeResult::Ok"),
    }
}

#[test]
fn test_c3_coercion_and_single_branch_operator() {
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

    let join_spec = JourneyParallelJoin {
        name: "join1".to_string(),
        expects: vec!["fork1".to_string()],
        merge: vec![JourneyMergeClause {
            location: "tags".to_string(),
            operator: "union".to_string(),
            custom_verb: None,
        }],
    };

    let result = apply_merge_protocol(&[t1], Some(&join_spec));
    match result {
        MergeResult::Ok(data) => {
            let val = data.get("tags").expect("Expected tags location");
            assert_eq!(
                val,
                &json!(["us"]),
                "Expected union operator to convert single string to array"
            );
        }
        _ => panic!("Expected MergeResult::Ok"),
    }
}

#[test]
fn test_c3_type_domain_mismatch_error() {
    let t1 = ActiveToken {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        current_node: "join1".to_string(),
        fork_ref: None,
        branch_lineage: vec!["fork1".to_string()],
        write_log: vec![WriteLogEntry {
            location: "counter".to_string(),
            value: json!("not a number"),
        }],
    };

    let join_spec = JourneyParallelJoin {
        name: "join1".to_string(),
        expects: vec!["fork1".to_string()],
        merge: vec![JourneyMergeClause {
            location: "counter".to_string(),
            operator: "sum".to_string(),
            custom_verb: None,
        }],
    };

    let result = apply_merge_protocol(&[t1], Some(&join_spec));
    match result {
        MergeResult::Conflict { location, .. } => {
            assert_eq!(location, "counter");
        }
        _ => panic!("Expected MergeResult::Conflict for type domain mismatch"),
    }
}
