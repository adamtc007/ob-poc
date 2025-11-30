//! Template Registry - Built-in templates for common operations

use super::slot_types::*;
use crate::services::EntityType;
use std::collections::HashMap;

pub struct TemplateRegistry {
    templates: HashMap<String, FormTemplate>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            templates: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    pub fn get(&self, id: &str) -> Option<&FormTemplate> {
        self.templates.get(id)
    }

    pub fn list(&self) -> Vec<&FormTemplate> {
        self.templates.values().collect()
    }

    pub fn list_by_domain(&self, domain: &str) -> Vec<&FormTemplate> {
        self.templates
            .values()
            .filter(|t| t.domain == domain)
            .collect()
    }

    fn register(&mut self, template: FormTemplate) {
        self.templates.insert(template.id.clone(), template);
    }

    fn register_builtins(&mut self) {
        // CBU templates
        self.register(Self::create_cbu_template());
        self.register(Self::attach_entity_template());
        self.register(Self::attach_beneficial_owner_template());

        // Entity templates
        self.register(Self::create_person_template());
        self.register(Self::create_company_template());

        // Document templates
        self.register(Self::request_document_template());
    }

    // =========================================================================
    // CBU Templates
    // =========================================================================

    fn create_cbu_template() -> FormTemplate {
        FormTemplate {
            id: "cbu.create".into(),
            name: "Create CBU".into(),
            description: "Create a new Client Business Unit".into(),
            verb: "cbu.ensure".into(),
            domain: "cbu".into(),
            tags: vec!["cbu".into(), "create".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_name".into(),
                    label: "CBU Name".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(200),
                        multiline: false,
                    },
                    required: true,
                    placeholder: Some("Apex Capital Partners".into()),
                    dsl_param: Some("cbu-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "client_type".into(),
                    label: "Client Type".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption {
                                value: "COMPANY".into(),
                                label: "Company".into(),
                                description: Some("Limited company or corporation".into()),
                            },
                            EnumOption {
                                value: "INDIVIDUAL".into(),
                                label: "Individual".into(),
                                description: Some("Natural person".into()),
                            },
                            EnumOption {
                                value: "TRUST".into(),
                                label: "Trust".into(),
                                description: Some("Trust or foundation".into()),
                            },
                            EnumOption {
                                value: "PARTNERSHIP".into(),
                                label: "Partnership".into(),
                                description: Some("Partnership or LP".into()),
                            },
                        ],
                    },
                    required: true,
                    default_value: Some(serde_json::json!("COMPANY")),
                    dsl_param: Some("client-type".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "jurisdiction".into(),
                    label: "Jurisdiction".into(),
                    slot_type: SlotType::Country,
                    required: true,
                    placeholder: Some("GB".into()),
                    dsl_param: Some("jurisdiction".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "nature_purpose".into(),
                    label: "Nature & Purpose".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(1000),
                        multiline: true,
                    },
                    required: false,
                    placeholder: Some("Hedge fund managing high net worth client assets".into()),
                    dsl_param: Some("nature-purpose".into()),
                    ..Default::default()
                },
            ],
        }
    }

    fn attach_entity_template() -> FormTemplate {
        FormTemplate {
            id: "cbu.attach-entity".into(),
            name: "Attach Entity to CBU".into(),
            description: "Link an existing entity to a CBU with a specific role".into(),
            verb: "cbu.assign-role".into(),
            domain: "cbu".into(),
            tags: vec!["cbu".into(), "entity".into(), "relationship".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_id".into(),
                    label: "CBU".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Cbu],
                        scope: RefScope::WithinSession,
                        allow_create: false,
                    },
                    required: true,
                    help_text: Some("Select the CBU to attach to".into()),
                    dsl_param: Some("cbu-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "entity_id".into(),
                    label: "Entity".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![
                            EntityType::Person,
                            EntityType::Company,
                            EntityType::Trust,
                        ],
                        scope: RefScope::Global,
                        allow_create: true,
                    },
                    required: true,
                    help_text: Some("Search for an entity or create new".into()),
                    dsl_param: Some("entity-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "role".into(),
                    label: "Role".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption {
                                value: "PRINCIPAL".into(),
                                label: "Principal".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "DIRECTOR".into(),
                                label: "Director".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "SHAREHOLDER".into(),
                                label: "Shareholder".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "BENEFICIAL_OWNER".into(),
                                label: "Beneficial Owner".into(),
                                description: Some("Person with >25% ownership".into()),
                            },
                            EnumOption {
                                value: "SIGNATORY".into(),
                                label: "Signatory".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "AUTHORIZED_PERSON".into(),
                                label: "Authorized Person".into(),
                                description: None,
                            },
                        ],
                    },
                    required: true,
                    dsl_param: Some("role".into()),
                    ..Default::default()
                },
            ],
        }
    }

    fn attach_beneficial_owner_template() -> FormTemplate {
        FormTemplate {
            id: "cbu.attach-beneficial-owner".into(),
            name: "Attach Beneficial Owner".into(),
            description: "Link a beneficial owner (>25% ownership) to a CBU".into(),
            verb: "cbu.assign-role".into(),
            domain: "cbu".into(),
            tags: vec!["cbu".into(), "beneficial-owner".into(), "compliance".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_id".into(),
                    label: "CBU".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Cbu],
                        scope: RefScope::WithinSession,
                        allow_create: false,
                    },
                    required: true,
                    dsl_param: Some("cbu-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "entity_id".into(),
                    label: "Beneficial Owner".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Person, EntityType::Company],
                        scope: RefScope::Global,
                        allow_create: true,
                    },
                    required: true,
                    dsl_param: Some("entity-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "role".into(),
                    label: "Role".into(),
                    slot_type: SlotType::Enum {
                        options: vec![EnumOption {
                            value: "BENEFICIAL_OWNER".into(),
                            label: "Beneficial Owner".into(),
                            description: None,
                        }],
                    },
                    required: true,
                    default_value: Some(serde_json::json!("BENEFICIAL_OWNER")),
                    dsl_param: Some("role".into()),
                    ..Default::default()
                },
            ],
        }
    }

    // =========================================================================
    // Entity Templates
    // =========================================================================

    fn create_person_template() -> FormTemplate {
        FormTemplate {
            id: "entity.create-person".into(),
            name: "Create Person".into(),
            description: "Create a new natural person entity".into(),
            verb: "entity.create-proper-person".into(),
            domain: "entity".into(),
            tags: vec!["entity".into(), "person".into(), "create".into()],
            slots: vec![
                SlotDefinition {
                    name: "first_name".into(),
                    label: "First Name".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(100),
                        multiline: false,
                    },
                    required: true,
                    placeholder: Some("John".into()),
                    dsl_param: Some("first-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "last_name".into(),
                    label: "Last Name".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(100),
                        multiline: false,
                    },
                    required: true,
                    placeholder: Some("Smith".into()),
                    dsl_param: Some("last-name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "nationality".into(),
                    label: "Nationality".into(),
                    slot_type: SlotType::Country,
                    required: false,
                    ..Default::default()
                },
                SlotDefinition {
                    name: "date_of_birth".into(),
                    label: "Date of Birth".into(),
                    slot_type: SlotType::Date,
                    required: false,
                    dsl_param: Some("date-of-birth".into()),
                    ..Default::default()
                },
            ],
        }
    }

    fn create_company_template() -> FormTemplate {
        FormTemplate {
            id: "entity.create-company".into(),
            name: "Create Company".into(),
            description: "Create a new limited company entity".into(),
            verb: "entity.create-limited-company".into(),
            domain: "entity".into(),
            tags: vec!["entity".into(), "company".into(), "create".into()],
            slots: vec![
                SlotDefinition {
                    name: "name".into(),
                    label: "Company Name".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(200),
                        multiline: false,
                    },
                    required: true,
                    placeholder: Some("Acme Holdings Ltd".into()),
                    dsl_param: Some("name".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "company_number".into(),
                    label: "Registration Number".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(50),
                        multiline: false,
                    },
                    required: false,
                    placeholder: Some("12345678".into()),
                    dsl_param: Some("company-number".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "jurisdiction".into(),
                    label: "Jurisdiction".into(),
                    slot_type: SlotType::Country,
                    required: true,
                    dsl_param: Some("jurisdiction".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "incorporation_date".into(),
                    label: "Incorporation Date".into(),
                    slot_type: SlotType::Date,
                    required: false,
                    dsl_param: Some("incorporation-date".into()),
                    ..Default::default()
                },
            ],
        }
    }

    // =========================================================================
    // Document Templates
    // =========================================================================

    fn request_document_template() -> FormTemplate {
        FormTemplate {
            id: "document.catalog".into(),
            name: "Catalog Document".into(),
            description: "Catalog a document for an entity".into(),
            verb: "document.catalog".into(),
            domain: "document".into(),
            tags: vec!["document".into(), "catalog".into()],
            slots: vec![
                SlotDefinition {
                    name: "cbu_id".into(),
                    label: "CBU".into(),
                    slot_type: SlotType::EntityRef {
                        allowed_types: vec![EntityType::Cbu],
                        scope: RefScope::WithinSession,
                        allow_create: false,
                    },
                    required: false,
                    dsl_param: Some("cbu-id".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "document_type".into(),
                    label: "Document Type".into(),
                    slot_type: SlotType::Enum {
                        options: vec![
                            EnumOption {
                                value: "PASSPORT".into(),
                                label: "Passport".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "ID_CARD".into(),
                                label: "ID Card".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "PROOF_OF_ADDRESS".into(),
                                label: "Proof of Address".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "CERT_OF_INCORP".into(),
                                label: "Certificate of Incorporation".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "FINANCIAL_STATEMENT".into(),
                                label: "Financial Statement".into(),
                                description: None,
                            },
                            EnumOption {
                                value: "SOURCE_OF_WEALTH".into(),
                                label: "Source of Wealth".into(),
                                description: None,
                            },
                        ],
                    },
                    required: true,
                    dsl_param: Some("doc-type".into()),
                    ..Default::default()
                },
                SlotDefinition {
                    name: "title".into(),
                    label: "Document Title".into(),
                    slot_type: SlotType::Text {
                        max_length: Some(200),
                        multiline: false,
                    },
                    required: false,
                    placeholder: Some("Passport - John Smith".into()),
                    dsl_param: Some("title".into()),
                    ..Default::default()
                },
            ],
        }
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}
