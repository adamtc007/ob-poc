//! Exit criterion (c): `ExecutionEnvelope` must not implement
//! `serde::Deserialize` — the runtime must obtain an envelope only via
//! `seal`, never by deserializing one from storage or the wire.

fn main() {
    let _: ob_poc_control_plane::envelope::ExecutionEnvelope = serde_json::from_str("{}").unwrap();
}
