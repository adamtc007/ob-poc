//! Compatibility identity helpers backed by the application semantic pack.

use uuid::Uuid;

use super::types::ObjectType;

fn identity_namespace() -> Uuid {
    ob_poc_semantic_policy::identity_namespace()
        .expect("embedded ob-poc semantic policy declares its v1 identity namespace")
}

/// Compute a deterministic object identifier using the namespace declared by
/// the admitted application pack.
///
/// # Examples
///
/// ```
/// use ob_poc::sem_reg::ids::object_id_for;
/// use ob_poc::sem_reg::types::ObjectType;
///
/// let first = object_id_for(ObjectType::VerbContract, "kyc.resolve_ubo");
/// let second = object_id_for(ObjectType::VerbContract, "kyc.resolve_ubo");
/// assert_eq!(first, second);
/// ```
pub fn object_id_for(object_type: ObjectType, fqn: &str) -> Uuid {
    sem_os_core::ids::object_id_for(identity_namespace(), object_type, fqn)
}

/// Compute a stable canonical definition fingerprint using the namespace
/// declared by the admitted application pack.
pub(crate) fn definition_hash(definition: &serde_json::Value) -> String {
    sem_os_core::ids::definition_hash(identity_namespace(), definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_remains_deterministic_and_type_scoped() {
        let verb = object_id_for(ObjectType::VerbContract, "kyc.resolve_ubo");
        assert_eq!(
            verb,
            Uuid::parse_str("0058fae8-e8bf-51b5-bef5-f9db54637fdd").unwrap(),
            "the persisted v1 identity must remain byte-for-byte compatible"
        );
        assert_eq!(
            verb,
            object_id_for(ObjectType::VerbContract, "kyc.resolve_ubo")
        );
        assert_ne!(
            verb,
            object_id_for(ObjectType::AttributeDef, "kyc.resolve_ubo")
        );
    }

    #[test]
    fn definition_hash_is_key_order_independent() {
        let first = serde_json::json!({"a": 1, "b": 2});
        let second = serde_json::json!({"b": 2, "a": 1});
        assert_eq!(definition_hash(&first), definition_hash(&second));
        assert_eq!(
            definition_hash(&first),
            "ebe76008-f2c0-5048-b9dc-0417d6ac3b74",
            "the persisted v1 definition fingerprint must remain byte-for-byte compatible"
        );
    }
}
