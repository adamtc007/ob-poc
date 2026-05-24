use anyhow::{bail, Context, Result};
use dsl_core::resolver::{ManifestOptions, ResolverManifest};
use sem_os_core::resolver::{resolve_template, ResolverInputs};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let inputs = ResolverInputs::from_workspace_config_dir(PathBuf::from("config"))?;

    if let Some(pair) = arg_value(&args, "--pair") {
        let (workspace, shape) = parse_pair(pair)?;
        print_manifest(&inputs, workspace, shape)?;
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--all") {
        for workspace in inputs.dag_taxonomies.keys() {
            for shape in inputs.constellation_maps.keys() {
                if let Ok(template) = resolve_template(shape.clone(), workspace.clone(), &inputs) {
                    let manifest =
                        ResolverManifest::from_template(&template, &ManifestOptions::default());
                    println!("{}", manifest.to_text());
                }
            }
        }
        return Ok(());
    }

    print_manifest(&inputs, "cbu", "struct.lux.ucits.sicav")
}

fn print_manifest(inputs: &ResolverInputs, workspace: &str, shape: &str) -> Result<()> {
    let template = resolve_template(shape.to_string(), workspace.to_string(), inputs)
        .with_context(|| format!("failed to resolve {workspace}/{shape}"))?;
    let options = if workspace == "cbu" && shape == "struct.lux.ucits.sicav" {
        ManifestOptions::with_required_slots([
            "cbu",
            "entity_proper_person",
            "entity_limited_company_ubo",
            "manco",
            "share_class",
            "cbu_evidence",
            "management_company",
            "depositary",
            "investment_manager",
            "mandate",
            "administrator",
            "auditor",
        ])
    } else {
        ManifestOptions::default()
    };
    let manifest = ResolverManifest::from_template(&template, &options);
    print!("{}", manifest.to_text());
    Ok(())
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
}

fn parse_pair(pair: &str) -> Result<(&str, &str)> {
    let Some((workspace, shape)) = pair.split_once('/') else {
        bail!("--pair must be formatted as workspace/shape");
    };
    Ok((workspace, shape))
}
