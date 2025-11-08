package store

import "time"

// Lightweight types only. All direct DB access has been removed.

type CBU struct {
    CBUID         string `json:"cbu_id"`
    Name          string `json:"name"`
    Description   string `json:"description"`
    NaturePurpose string `json:"nature_purpose"`
}

type Product struct {
    ProductID   string `json:"product_id"`
    Name        string `json:"name"`
    Description string `json:"description"`
}

type Service struct {
    ServiceID   string `json:"service_id"`
    Name        string `json:"name"`
    Description string `json:"description"`
}

type ProdResource struct {
    ResourceID      string `json:"resource_id"`
    Name            string `json:"name"`
    Description     string `json:"description"`
    Owner           string `json:"owner"`
    DictionaryGroup string `json:"dictionary_group"`
}

type Role struct {
    RoleID      string `json:"role_id"`
    Name        string `json:"name"`
    Description string `json:"description"`
}

type EntityType struct {
    EntityTypeID string `json:"entity_type_id"`
    Name         string `json:"name"`
    Description  string `json:"description"`
    TableName    string `json:"table_name"`
}

type Entity struct {
    EntityID     string `json:"entity_id"`
    EntityTypeID string `json:"entity_type_id"`
    ExternalID   string `json:"external_id"`
    Name         string `json:"name"`
}

type CBUEntityRole struct {
    CBUEntityRoleID string `json:"cbu_entity_role_id"`
    CBUID           string `json:"cbu_id"`
    EntityID        string `json:"entity_id"`
    RoleID          string `json:"role_id"`
}

type LimitedCompany struct {
    LimitedCompanyID   string     `json:"limited_company_id"`
    CompanyName        string     `json:"company_name"`
    RegistrationNumber string     `json:"registration_number"`
    Jurisdiction       string     `json:"jurisdiction"`
    IncorporationDate  *time.Time `json:"incorporation_date"`
    RegisteredAddress  string     `json:"registered_address"`
    BusinessNature     string     `json:"business_nature"`
}

type Partnership struct {
    PartnershipID            string     `json:"partnership_id"`
    PartnershipName          string     `json:"partnership_name"`
    PartnershipType          string     `json:"partnership_type"`
    Jurisdiction             string     `json:"jurisdiction"`
    FormationDate            *time.Time `json:"formation_date"`
    PrincipalPlaceBusiness   string     `json:"principal_place_business"`
    PartnershipAgreementDate *time.Time `json:"partnership_agreement_date"`
}

type Individual struct {
    ProperProperPersonID string     `json:"proper_proper_person_id"`
    FirstName            string     `json:"first_name"`
    LastName             string     `json:"last_name"`
    MiddleNames          string     `json:"middle_names"`
    DateOfBirth          *time.Time `json:"date_of_birth"`
    Nationality          string     `json:"nationality"`
    ResidenceAddress     string     `json:"residence_address"`
    IDDocumentType       string     `json:"id_document_type"`
    IDDocumentNumber     string     `json:"id_document_number"`
}

// Onboarding state (store-level)
type OnboardingState string

const (
    StateCreated             OnboardingState = "CREATED"
    StateProductsAdded       OnboardingState = "PRODUCTS_ADDED"
    StateKYCDiscovered       OnboardingState = "KYC_DISCOVERED"
    StateServicesDiscovered  OnboardingState = "SERVICES_DISCOVERED"
    StateResourcesDiscovered OnboardingState = "RESOURCES_DISCOVERED"
    StateAttributesPopulated OnboardingState = "ATTRIBUTES_POPULATED"
    StateCompleted           OnboardingState = "COMPLETED"
)

// Versioned DSL artefacts (lightweight for mocks)
type DSLVersion struct {
    VersionID string
    CreatedAt time.Time
    DSLText   string
}

type DSLVersionWithState struct {
    VersionID       string
    CBUID           string
    DSLText         string
    OnboardingState OnboardingState
    VersionNumber   int
    CreatedAt       time.Time
}

type OnboardingSession struct {
    SessionID string
    CBUID     string
    State     OnboardingState
    CreatedAt time.Time
    UpdatedAt time.Time
}

// Orchestration persistence data
type OrchestrationSessionData struct {
    SessionID      string                 `json:"session_id"`
    PrimaryDomain  string                 `json:"primary_domain"`
    CBUID          *string                `json:"cbu_id,omitempty"`
    EntityType     *string                `json:"entity_type,omitempty"`
    EntityName     *string                `json:"entity_name,omitempty"`
    Jurisdiction   *string                `json:"jurisdiction,omitempty"`
    Products       []string               `json:"products,omitempty"`
    Services       []string               `json:"services,omitempty"`
    WorkflowType   *string                `json:"workflow_type,omitempty"`
    CurrentState   string                 `json:"current_state"`
    VersionNumber  int                    `json:"version_number"`
    UnifiedDSL     string                 `json:"unified_dsl"`
    SharedContext  map[string]interface{} `json:"shared_context"`
    ExecutionPlan  map[string]interface{} `json:"execution_plan"`
    EntityRefs     map[string]string      `json:"entity_refs"`
    AttributeRefs  map[string]string      `json:"attribute_refs"`
    DomainSessions []DomainSessionData    `json:"domain_sessions"`
    StateHistory   []StateTransitionData  `json:"state_history"`
    CreatedAt      time.Time              `json:"created_at"`
    UpdatedAt      time.Time              `json:"updated_at"`
    LastUsed       time.Time              `json:"last_used"`
}

type DomainSessionData struct {
    DomainName      string                 `json:"domain_name"`
    DomainSessionID string                 `json:"domain_session_id"`
    State           string                 `json:"state"`
    ContributedDSL  string                 `json:"contributed_dsl"`
    Context         map[string]interface{} `json:"context"`
    Dependencies    []string               `json:"dependencies"`
    LastActivity    time.Time              `json:"last_activity"`
}

type StateTransitionData struct {
    FromState   string    `json:"from_state"`
    ToState     string    `json:"to_state"`
    Domain      string    `json:"domain,omitempty"`
    Reason      string    `json:"reason,omitempty"`
    GeneratedBy string    `json:"generated_by,omitempty"`
    Timestamp   time.Time `json:"timestamp"`
}

// Product requirements (Phase 5) lightweight types
type ProductRequirements struct {
    ProductID        string      `json:"product_id"`
    ProductName      string      `json:"product_name"`
    EntityTypes      []string    `json:"entity_types"`
    RequiredDSL      []string    `json:"required_dsl"`
    Attributes       []string    `json:"attributes"`
    Compliance       []map[string]interface{} `json:"compliance"`
    Prerequisites    []string    `json:"prerequisites"`
    ConditionalRules []map[string]interface{} `json:"conditional_rules"`
    CreatedAt        time.Time   `json:"created_at"`
    UpdatedAt        time.Time   `json:"updated_at"`
}

type EntityProductMapping struct {
    EntityType     string    `json:"entity_type"`
    ProductID      string    `json:"product_id"`
    Compatible     bool      `json:"compatible"`
    Restrictions   []string  `json:"restrictions"`
    RequiredFields []string  `json:"required_fields"`
    CreatedAt      time.Time `json:"created_at"`
}

