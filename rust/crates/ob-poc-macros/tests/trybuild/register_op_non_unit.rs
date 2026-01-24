//! This should fail: #[register_custom_op] only works on unit structs

use ob_poc_macros::register_custom_op;

#[register_custom_op]
pub struct BadOp {
    inner: String,
}

fn main() {}
