//! `cargo x dag-to-bpmn` — compiles one `dag_taxonomies` slot's state
//! machine into a bpmn-lite DSL workflow template.
//!
//! Thin CLI wrapper over the `dag-to-bpmn` crate — see that crate's
//! `lib.rs` doc comment and
//! `docs/todo/EOP-DD-DAGBPMN-001_DAG-Taxonomy-to-BPMN-Template-Compiler_v0.1.md`
//! for the compiler's design and scope.

use anyhow::{Context, Result};
use std::path::PathBuf;

pub(crate) fn run(dag_file: PathBuf, slot_id: String, out: Option<PathBuf>) -> Result<()> {
    let dags_dir = dag_file
        .parent()
        .with_context(|| format!("{dag_file:?} has no parent directory"))?;
    let workspace_key = dag_file
        .file_stem()
        .and_then(|s| s.to_str())
        .with_context(|| format!("{dag_file:?} has no usable file stem"))?
        .trim_end_matches("_dag")
        .to_string();

    let dags = dsl_core::load_dags_from_dir(dags_dir)
        .with_context(|| format!("failed to load DAG taxonomies from {dags_dir:?}"))?;
    let loaded = dags.get(&workspace_key).with_context(|| {
        format!(
            "no DAG with workspace '{workspace_key}' found in {dags_dir:?} (loaded: {:?})",
            dags.keys().collect::<Vec<_>>()
        )
    })?;

    let process_name = format!("{workspace_key}-{slot_id}");
    match dag_to_bpmn::compile_slot(&loaded.dag, &slot_id, &process_name) {
        Ok(compiled) => {
            println!("=== compiled '{slot_id}' -> {process_name} ===\n");
            println!("{}\n", compiled.dsl_source);
            println!(
                "nodes: {}, edges: {}, start_node: {}",
                compiled.spec.nodes.len(),
                compiled.spec.edges.len(),
                compiled.spec.start_node
            );
            if let Some(out_path) = out {
                std::fs::write(&out_path, &compiled.dsl_source)
                    .with_context(|| format!("failed to write {out_path:?}"))?;
                println!("\nwrote DSL source to {out_path:?}");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("REJECTED '{slot_id}': {e}");
            std::process::exit(1);
        }
    }
}
