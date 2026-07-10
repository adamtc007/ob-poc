//! Exit criterion (a): `ExecutionEnvelope::seal` must be unreachable from
//! outside the crate. Taking the function itself as a value (rather than
//! calling it) is enough to trigger the privacy error — no valid proof
//! arguments exist outside the crate anyway, since every proof type's
//! constructor is equally module-private.

fn main() {
    let _ = ob_poc_control_plane::envelope::ExecutionEnvelope::seal;
}
