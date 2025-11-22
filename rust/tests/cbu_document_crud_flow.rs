//! CBU Document-Directed CRUD Integration Test

use ob_poc::cbu_model_dsl::parser::CbuModelParser;
use ob_poc::cbu_crud_template::CbuCrudTemplateService;
use ob_poc::database::{CbuService, NewCbuFields};
use sqlx::PgPool;

fn test_cbu_model_dsl() -> String {
    r#"(cbu-model
  :id "CBU.TEST.GENERIC"
  :version "1.0"
  :description "Test CBU model"
  :applies-to ["Fund"]
  (attributes
    (chunk "core"
      (required @attr("CBU.LEGAL_NAME") @attr("CBU.LEGAL_JURISDICTION"))
      (optional))
    (chunk "contact"
      (required @attr("CBU.REGISTERED_ADDRESS"))
      (optional)))
  (states
    :initial "Proposed"
    :final ["Closed"]
    (state "Proposed" :description "Draft")
    (state "Active" :description "Live"))
  (transitions
    (-> "Proposed" "Active" :verb "cbu.submit" :chunks ["core" "contact"]))
  (roles
    (role "BeneficialOwner" :min 1 :max 10))
)"#.to_string()
}

#[tokio::test]
#[ignore]
async fn test_cbu_model_parsing() {
    let model_dsl = test_cbu_model_dsl();
    let result = CbuModelParser::parse_str(&model_dsl);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    let model = result.unwrap();
    assert_eq!(model.id, "CBU.TEST.GENERIC");
}

#[tokio::test]
#[ignore]
async fn test_template_generation() {
    let model_dsl = test_cbu_model_dsl();
    let model = CbuModelParser::parse_str(&model_dsl).unwrap();
    let url = std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/ob-poc".into());
    let pool = PgPool::connect(&url).await.unwrap();
    let svc = CbuCrudTemplateService::new(pool);
    let templates = svc.generate_templates(&model);
    assert_eq!(templates.len(), 1);
}

#[tokio::test]
#[ignore]
async fn test_cbu_create() {
    let url = std::env::var("DATABASE_URL").unwrap_or("postgresql://localhost/ob-poc".into());
    let pool = PgPool::connect(&url).await.unwrap();
    let svc = CbuService::new(pool);
    let cbu = NewCbuFields {
        name: "TEST CBU".into(),
        description: Some("Test".into()),
        nature_purpose: Some("Test".into()),
    };
    let id = svc.create_cbu(&cbu).await.unwrap();
    let loaded = svc.get_cbu_by_id(id).await.unwrap().unwrap();
    assert_eq!(loaded.name, "TEST CBU");
}
