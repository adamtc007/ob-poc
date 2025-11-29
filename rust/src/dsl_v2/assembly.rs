//! DSL Assembly with Typestate Builder Pattern
//!
//! The problem: Agents generating raw DSL are non-deterministic and error-prone.
//!
//! The solution: Constrain the output space using:
//! 1. Intent Classification (enum) - What template/pattern?
//! 2. Slot Extraction (struct) - What parameters?
//! 3. Typestate Builder - Compile-time valid state transitions
//! 4. Deterministic Assembly - Builder produces validated DSL
//!
//! ```
//! User: "Onboard John Smith with his UK passport"
//!         │
//!         ▼
//! ┌──────────────────────────────────────┐
//! │ Intent Classifier (constrained)      │
//! │ → KycIntent::OnboardIndividual       │
//! └──────────────────────────────────────┘
//!         │
//!         ▼
//! ┌──────────────────────────────────────┐
//! │ Slot Extractor (structured JSON)     │
//! │ { "name": "John Smith",              │
//! │   "document_type": "PASSPORT_GBR" }  │
//! └──────────────────────────────────────┘
//!         │
//!         ▼
//! ┌──────────────────────────────────────┐
//! │ Typestate Builder (compile-time)     │
//! │ OnboardingBuilder::<NoCbu>::new()    │
//! │   .create_cbu("John Smith")          │  // → CbuCreated state
//! │   .add_document("PASSPORT_GBR")      │  // Only valid after CBU
//! │   .extract_attributes()              │  // Only valid after doc
//! │   .build()                           │
//! └──────────────────────────────────────┘
//!         │
//!         ▼
//! ┌──────────────────────────────────────┐
//! │ Deterministic DSL Output             │
//! │ (cbu.create :name "John Smith"       │
//! │   :as @cbu)                          │
//! │ (document.catalog                    │
//! │   :document-type "PASSPORT_GBR"      │
//! │   :cbu-id @cbu :as @doc)             │
//! │ (document.extract :document-id @doc) │
//! └──────────────────────────────────────┘
//! ```

use std::marker::PhantomData;

// =============================================================================
// TYPESTATE MARKERS - Encode valid states at compile time
// =============================================================================

/// No CBU exists yet - initial state
pub struct NoCbu;

/// CBU has been created - can now add documents/entities
pub struct CbuCreated {
    pub cbu_id: String, // Binding like "@cbu"
}

/// Documents have been added - can extract attributes
pub struct DocumentsAdded {
    pub cbu_id: String,
    pub document_ids: Vec<String>,
}

/// Entities have been linked
pub struct EntitiesLinked {
    pub cbu_id: String,
    pub entity_ids: Vec<String>,
}

/// Ready to finalize
pub struct Complete {
    pub cbu_id: String,
}

// =============================================================================
// DSL OPERATIONS - What gets generated
// =============================================================================

#[derive(Debug, Clone)]
pub enum DslOperation {
    CreateCbu {
        name: String,
        binding: String,
        jurisdiction: Option<String>,
        client_type: Option<String>,
    },
    CatalogDocument {
        document_type: String,
        cbu_id_ref: String,
        binding: String,
        title: Option<String>,
    },
    ExtractDocument {
        document_id_ref: String,
    },
    CreateEntity {
        entity_type: String,
        name: String,
        binding: String,
    },
    LinkEntityToCbu {
        cbu_id_ref: String,
        entity_id_ref: String,
        role: String,
    },
    LinkDocumentToEntity {
        document_id_ref: String,
        entity_id_ref: String,
    },
}

impl DslOperation {
    /// Convert to DSL string
    pub fn to_dsl(&self) -> String {
        match self {
            DslOperation::CreateCbu {
                name,
                binding,
                jurisdiction,
                client_type,
            } => {
                let mut args = format!(r#"(cbu.create :name "{}" :as {}"#, name, binding);
                if let Some(j) = jurisdiction {
                    args.push_str(&format!(r#" :jurisdiction "{}""#, j));
                }
                if let Some(ct) = client_type {
                    args.push_str(&format!(r#" :client-type "{}""#, ct));
                }
                args.push(')');
                args
            }
            DslOperation::CatalogDocument {
                document_type,
                cbu_id_ref,
                binding,
                title,
            } => {
                let mut args = format!(
                    r#"(document.catalog :document-type "{}" :cbu-id {} :as {}"#,
                    document_type, cbu_id_ref, binding
                );
                if let Some(t) = title {
                    args.push_str(&format!(r#" :title "{}""#, t));
                }
                args.push(')');
                args
            }
            DslOperation::ExtractDocument { document_id_ref } => {
                format!("(document.extract :document-id {})", document_id_ref)
            }
            DslOperation::CreateEntity {
                entity_type,
                name,
                binding,
            } => {
                format!(
                    r#"(entity.create-{} :name "{}" :as {})"#,
                    entity_type, name, binding
                )
            }
            DslOperation::LinkEntityToCbu {
                cbu_id_ref,
                entity_id_ref,
                role,
            } => {
                format!(
                    r#"(cbu.assign-role :cbu-id {} :entity-id {} :role "{}")"#,
                    cbu_id_ref, entity_id_ref, role
                )
            }
            DslOperation::LinkDocumentToEntity {
                document_id_ref,
                entity_id_ref,
            } => {
                format!(
                    "(document.link-entity :document-id {} :entity-id {})",
                    document_id_ref, entity_id_ref
                )
            }
        }
    }
}

// =============================================================================
// TYPESTATE BUILDER - Compile-time state machine
// =============================================================================

/// Builder that enforces valid state transitions at compile time
pub struct OnboardingBuilder<State> {
    operations: Vec<DslOperation>,
    binding_counter: u32,
    _state: PhantomData<State>,
    state_data: Option<StateData>,
}

#[derive(Clone)]
struct StateData {
    cbu_binding: String,
    document_bindings: Vec<String>,
    entity_bindings: Vec<String>,
}

impl OnboardingBuilder<NoCbu> {
    /// Start a new onboarding flow - initial state
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            binding_counter: 0,
            _state: PhantomData,
            state_data: None,
        }
    }

    /// Create CBU - transitions to CbuCreated state
    /// This is the ONLY way to get to CbuCreated state
    pub fn create_cbu(mut self, name: &str) -> OnboardingBuilder<CbuCreated> {
        let binding = format!("@cbu{}", self.binding_counter);
        self.binding_counter += 1;

        self.operations.push(DslOperation::CreateCbu {
            name: name.to_string(),
            binding: binding.clone(),
            jurisdiction: None,
            client_type: None,
        });

        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: Some(StateData {
                cbu_binding: binding,
                document_bindings: Vec::new(),
                entity_bindings: Vec::new(),
            }),
        }
    }

    /// Create CBU with options
    pub fn create_cbu_with(
        mut self,
        name: &str,
        jurisdiction: Option<&str>,
        client_type: Option<&str>,
    ) -> OnboardingBuilder<CbuCreated> {
        let binding = format!("@cbu{}", self.binding_counter);
        self.binding_counter += 1;

        self.operations.push(DslOperation::CreateCbu {
            name: name.to_string(),
            binding: binding.clone(),
            jurisdiction: jurisdiction.map(|s| s.to_string()),
            client_type: client_type.map(|s| s.to_string()),
        });

        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: Some(StateData {
                cbu_binding: binding,
                document_bindings: Vec::new(),
                entity_bindings: Vec::new(),
            }),
        }
    }
}

impl OnboardingBuilder<CbuCreated> {
    /// Add a document - stays in CbuCreated (can add more)
    pub fn add_document(mut self, document_type: &str) -> OnboardingBuilder<DocumentsAdded> {
        let binding = format!("@doc{}", self.binding_counter);
        self.binding_counter += 1;

        let state_data = self.state_data.as_ref().unwrap();
        let cbu_binding = state_data.cbu_binding.clone();

        self.operations.push(DslOperation::CatalogDocument {
            document_type: document_type.to_string(),
            cbu_id_ref: cbu_binding.clone(),
            binding: binding.clone(),
            title: None,
        });

        let mut new_state = state_data.clone();
        new_state.document_bindings.push(binding);

        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: Some(new_state),
        }
    }

    /// Create and link an entity
    pub fn add_entity(
        mut self,
        entity_type: &str,
        name: &str,
        role: &str,
    ) -> OnboardingBuilder<EntitiesLinked> {
        let binding = format!("@ent{}", self.binding_counter);
        self.binding_counter += 1;

        let state_data = self.state_data.as_ref().unwrap();
        let cbu_binding = state_data.cbu_binding.clone();

        self.operations.push(DslOperation::CreateEntity {
            entity_type: entity_type.to_string(),
            name: name.to_string(),
            binding: binding.clone(),
        });

        self.operations.push(DslOperation::LinkEntityToCbu {
            cbu_id_ref: cbu_binding,
            entity_id_ref: binding.clone(),
            role: role.to_string(),
        });

        let mut new_state = state_data.clone();
        new_state.entity_bindings.push(binding);

        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: Some(new_state),
        }
    }

    /// Skip documents/entities and go straight to complete
    pub fn finalize(self) -> OnboardingBuilder<Complete> {
        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: self.state_data,
        }
    }
}

impl OnboardingBuilder<DocumentsAdded> {
    /// Add another document
    pub fn add_document(mut self, document_type: &str) -> Self {
        let binding = format!("@doc{}", self.binding_counter);
        self.binding_counter += 1;

        let state_data = self.state_data.as_ref().unwrap();
        let cbu_binding = state_data.cbu_binding.clone();

        self.operations.push(DslOperation::CatalogDocument {
            document_type: document_type.to_string(),
            cbu_id_ref: cbu_binding,
            binding: binding.clone(),
            title: None,
        });

        let mut new_state = state_data.clone();
        new_state.document_bindings.push(binding);
        self.state_data = Some(new_state);
        self
    }

    /// Extract attributes from all documents
    pub fn extract_attributes(mut self) -> Self {
        let state_data = self.state_data.as_ref().unwrap();
        for doc_binding in &state_data.document_bindings {
            self.operations.push(DslOperation::ExtractDocument {
                document_id_ref: doc_binding.clone(),
            });
        }
        self
    }

    /// Add entity and link to CBU
    pub fn add_entity(
        mut self,
        entity_type: &str,
        name: &str,
        role: &str,
    ) -> OnboardingBuilder<EntitiesLinked> {
        let binding = format!("@ent{}", self.binding_counter);
        self.binding_counter += 1;

        let state_data = self.state_data.as_ref().unwrap();
        let cbu_binding = state_data.cbu_binding.clone();

        self.operations.push(DslOperation::CreateEntity {
            entity_type: entity_type.to_string(),
            name: name.to_string(),
            binding: binding.clone(),
        });

        self.operations.push(DslOperation::LinkEntityToCbu {
            cbu_id_ref: cbu_binding,
            entity_id_ref: binding.clone(),
            role: role.to_string(),
        });

        let mut new_state = state_data.clone();
        new_state.entity_bindings.push(binding);

        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: Some(new_state),
        }
    }

    /// Finalize without entities
    pub fn finalize(self) -> OnboardingBuilder<Complete> {
        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: self.state_data,
        }
    }
}

impl OnboardingBuilder<EntitiesLinked> {
    /// Add another entity
    pub fn add_entity(mut self, entity_type: &str, name: &str, role: &str) -> Self {
        let binding = format!("@ent{}", self.binding_counter);
        self.binding_counter += 1;

        let state_data = self.state_data.as_ref().unwrap();
        let cbu_binding = state_data.cbu_binding.clone();

        self.operations.push(DslOperation::CreateEntity {
            entity_type: entity_type.to_string(),
            name: name.to_string(),
            binding: binding.clone(),
        });

        self.operations.push(DslOperation::LinkEntityToCbu {
            cbu_id_ref: cbu_binding,
            entity_id_ref: binding.clone(),
            role: role.to_string(),
        });

        let mut new_state = state_data.clone();
        new_state.entity_bindings.push(binding);
        self.state_data = Some(new_state);
        self
    }

    /// Finalize
    pub fn finalize(self) -> OnboardingBuilder<Complete> {
        OnboardingBuilder {
            operations: self.operations,
            binding_counter: self.binding_counter,
            _state: PhantomData,
            state_data: self.state_data,
        }
    }
}

impl OnboardingBuilder<Complete> {
    /// Build the final DSL program
    pub fn build(self) -> DslProgram {
        DslProgram {
            operations: self.operations,
        }
    }
}

// =============================================================================
// DSL PROGRAM - The output
// =============================================================================

#[derive(Debug, Clone)]
pub struct DslProgram {
    pub operations: Vec<DslOperation>,
}

impl DslProgram {
    /// Convert to DSL source code
    pub fn to_dsl(&self) -> String {
        self.operations
            .iter()
            .map(|op| op.to_dsl())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// =============================================================================
// INTENT + SLOTS - What the agent produces (constrained)
// =============================================================================

/// Intents the agent can classify to (small, fixed set)
#[derive(Debug, Clone, PartialEq)]
pub enum KycIntent {
    /// Onboard a new individual client
    OnboardIndividual,
    /// Onboard a new corporate client
    OnboardCorporate,
    /// Add a document to existing CBU
    AddDocument,
    /// Add an entity role to existing CBU
    AddEntityRole,
    /// Extract attributes from a document
    ExtractDocument,
    /// Link document to entity
    LinkDocumentToEntity,
}

/// Slots extracted from user input (structured, validated)
#[derive(Debug, Clone, Default)]
pub struct OnboardingSlots {
    pub client_name: Option<String>,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,   // individual, corporate
    pub document_type: Option<String>, // PASSPORT_GBR, DRIVERS_LICENSE_USA_CA
    pub entity_type: Option<String>,   // natural-person, limited-company
    pub entity_name: Option<String>,
    pub entity_role: Option<String>, // beneficial_owner, director, signatory
    pub existing_cbu_id: Option<String>,
    pub existing_document_id: Option<String>,
    pub existing_entity_id: Option<String>,
}

// =============================================================================
// FACTORY - Assembles DSL from Intent + Slots
// =============================================================================

pub struct DslAssemblyFactory;

impl DslAssemblyFactory {
    /// Assemble DSL from classified intent and extracted slots
    /// This is DETERMINISTIC - same intent + slots = same DSL
    pub fn assemble(intent: KycIntent, slots: &OnboardingSlots) -> Result<DslProgram, String> {
        match intent {
            KycIntent::OnboardIndividual => Self::assemble_individual_onboarding(slots),
            KycIntent::OnboardCorporate => Self::assemble_corporate_onboarding(slots),
            KycIntent::AddDocument => Self::assemble_add_document(slots),
            KycIntent::AddEntityRole => Self::assemble_add_entity_role(slots),
            _ => Err(format!("Intent {:?} not yet implemented", intent)),
        }
    }

    fn assemble_individual_onboarding(slots: &OnboardingSlots) -> Result<DslProgram, String> {
        let name = slots
            .client_name
            .as_ref()
            .ok_or("client_name is required for individual onboarding")?;

        // Add document if provided
        if let Some(doc_type) = &slots.document_type {
            let program = OnboardingBuilder::new()
                .create_cbu_with(name, slots.jurisdiction.as_deref(), Some("individual"))
                .add_document(doc_type)
                .extract_attributes()
                .finalize()
                .build();
            Ok(program)
        } else {
            let program = OnboardingBuilder::new()
                .create_cbu_with(name, slots.jurisdiction.as_deref(), Some("individual"))
                .finalize()
                .build();
            Ok(program)
        }
    }

    fn assemble_corporate_onboarding(slots: &OnboardingSlots) -> Result<DslProgram, String> {
        let name = slots
            .client_name
            .as_ref()
            .ok_or("client_name is required for corporate onboarding")?;

        let builder = OnboardingBuilder::new().create_cbu_with(
            name,
            slots.jurisdiction.as_deref(),
            Some("corporate"),
        );

        // Add certificate of incorporation by default for corporate
        let builder = if let Some(doc_type) = &slots.document_type {
            builder.add_document(doc_type)
        } else {
            builder.add_document("CERT_OF_INCORPORATION")
        };

        // Add entity if provided
        let builder = if let (Some(ent_name), Some(role)) = (&slots.entity_name, &slots.entity_role)
        {
            let ent_type = slots.entity_type.as_deref().unwrap_or("limited-company");
            builder.add_entity(ent_type, ent_name, role).finalize()
        } else {
            builder.finalize()
        };

        Ok(builder.build())
    }

    fn assemble_add_document(slots: &OnboardingSlots) -> Result<DslProgram, String> {
        let cbu_id = slots
            .existing_cbu_id
            .as_ref()
            .ok_or("existing_cbu_id is required to add document")?;
        let doc_type = slots
            .document_type
            .as_ref()
            .ok_or("document_type is required")?;

        // For adding to existing CBU, we generate simpler DSL
        let program = DslProgram {
            operations: vec![
                DslOperation::CatalogDocument {
                    document_type: doc_type.clone(),
                    cbu_id_ref: cbu_id.clone(),
                    binding: "@doc0".to_string(),
                    title: None,
                },
                DslOperation::ExtractDocument {
                    document_id_ref: "@doc0".to_string(),
                },
            ],
        };

        Ok(program)
    }

    fn assemble_add_entity_role(slots: &OnboardingSlots) -> Result<DslProgram, String> {
        let cbu_id = slots
            .existing_cbu_id
            .as_ref()
            .ok_or("existing_cbu_id is required")?;
        let entity_name = slots
            .entity_name
            .as_ref()
            .ok_or("entity_name is required")?;
        let role = slots
            .entity_role
            .as_ref()
            .ok_or("entity_role is required")?;
        let entity_type = slots.entity_type.as_deref().unwrap_or("natural-person");

        let program = DslProgram {
            operations: vec![
                DslOperation::CreateEntity {
                    entity_type: entity_type.to_string(),
                    name: entity_name.clone(),
                    binding: "@ent0".to_string(),
                },
                DslOperation::LinkEntityToCbu {
                    cbu_id_ref: cbu_id.clone(),
                    entity_id_ref: "@ent0".to_string(),
                    role: role.clone(),
                },
            ],
        };

        Ok(program)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typestate_prevents_invalid_transitions() {
        // This compiles:
        let _program = OnboardingBuilder::new()
            .create_cbu("John Smith")
            .add_document("PASSPORT_GBR")
            .extract_attributes()
            .finalize()
            .build();

        // This would NOT compile (uncomment to verify):
        // let _invalid = OnboardingBuilder::new()
        //     .add_document("PASSPORT_GBR");  // ERROR: no method `add_document` on NoCbu
    }

    #[test]
    fn test_individual_onboarding_dsl() {
        let program = OnboardingBuilder::new()
            .create_cbu("John Smith")
            .add_document("PASSPORT_GBR")
            .extract_attributes()
            .finalize()
            .build();

        let dsl = program.to_dsl();
        println!("Generated DSL:\n{}", dsl);

        assert!(dsl.contains("cbu.create"));
        assert!(dsl.contains("John Smith"));
        assert!(dsl.contains("PASSPORT_GBR"));
        assert!(dsl.contains("document.extract"));
    }

    #[test]
    fn test_corporate_onboarding_with_entity() {
        let program = OnboardingBuilder::new()
            .create_cbu_with("Acme Corp", Some("UK"), Some("corporate"))
            .add_document("CERT_OF_INCORPORATION")
            .add_entity("natural-person", "Jane Director", "director")
            .add_entity("natural-person", "Bob Owner", "beneficial_owner")
            .finalize()
            .build();

        let dsl = program.to_dsl();
        println!("Corporate DSL:\n{}", dsl);

        assert!(dsl.contains("Acme Corp"));
        assert!(dsl.contains("director"));
        assert!(dsl.contains("beneficial_owner"));
    }

    #[test]
    fn test_factory_individual_onboarding() {
        let slots = OnboardingSlots {
            client_name: Some("John Smith".to_string()),
            jurisdiction: Some("UK".to_string()),
            document_type: Some("PASSPORT_GBR".to_string()),
            ..Default::default()
        };

        let program = DslAssemblyFactory::assemble(KycIntent::OnboardIndividual, &slots).unwrap();

        let dsl = program.to_dsl();
        println!("Factory DSL:\n{}", dsl);

        assert!(dsl.contains("cbu.create"));
        assert!(dsl.contains("John Smith"));
    }

    #[test]
    fn test_deterministic_output() {
        // Same inputs should produce identical outputs
        let slots = OnboardingSlots {
            client_name: Some("Test Client".to_string()),
            document_type: Some("PASSPORT_USA".to_string()),
            ..Default::default()
        };

        let dsl1 = DslAssemblyFactory::assemble(KycIntent::OnboardIndividual, &slots)
            .unwrap()
            .to_dsl();
        let dsl2 = DslAssemblyFactory::assemble(KycIntent::OnboardIndividual, &slots)
            .unwrap()
            .to_dsl();

        assert_eq!(dsl1, dsl2, "Same inputs must produce identical DSL");
    }
}
