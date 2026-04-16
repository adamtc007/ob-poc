use std::collections::HashMap;

use anyhow::{Context, Result};
use ob_poc::bpmn_integration::{BpmnLiteConnection, CompleteJobRequest};
use sha2::{Digest, Sha256};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let grpc_url = args.next().context(
        "usage: complete_bpmn_job <grpc_url> <job_key> <current_payload> <completion_payload>",
    )?;
    let job_key = args.next().context(
        "usage: complete_bpmn_job <grpc_url> <job_key> <current_payload> <completion_payload>",
    )?;
    let current_payload = args.next().context(
        "usage: complete_bpmn_job <grpc_url> <job_key> <current_payload> <completion_payload>",
    )?;
    let completion_payload = args.next().context(
        "usage: complete_bpmn_job <grpc_url> <job_key> <current_payload> <completion_payload>",
    )?;

    let client = BpmnLiteConnection::connect(&grpc_url).await?;
    let expected_hash = Sha256::digest(current_payload.as_bytes()).to_vec();

    client
        .complete_job(CompleteJobRequest {
            job_key,
            domain_payload: completion_payload,
            domain_payload_hash: expected_hash,
            orch_flags: HashMap::new(),
        })
        .await?;

    println!("completed");
    Ok(())
}
