//! T10.1 (B2 ratification, condition 2): `ControlPlaneDecision` being pub
//! — and carrying `ExecutionEnvelope` in its `ApprovedStp` arm — must not
//! transitively enable constructing an `ExecutionEnvelope` from outside
//! the crate. The enum's own visibility is irrelevant here: struct-literal
//! construction is blocked by `ExecutionEnvelope`'s private fields
//! regardless of what public type wraps it. Combined with
//! `seal_is_crate_private.rs` (no constructor function reachable) and
//! `envelope_not_deserializable.rs` (no deserialize path), this closes
//! the third route someone might try: build one via the pub decision
//! type's own pub variant.

fn main() {
    let _ = ob_poc_control_plane::decision::ControlPlaneDecision::ApprovedStp(Box::new(
        ob_poc_control_plane::envelope::ExecutionEnvelope {},
    ));
}
