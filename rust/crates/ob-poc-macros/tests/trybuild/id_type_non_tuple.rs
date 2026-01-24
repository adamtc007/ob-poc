//! This should fail: #[derive(IdType)] only works on tuple structs with single Uuid field

use ob_poc_macros::IdType;
use uuid::Uuid;

#[derive(IdType)]
pub struct BadId {
    inner: Uuid,
}

fn main() {}
