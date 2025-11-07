package entities

import (
	"time"

	"github.com/google/uuid"
)

// ============================================================================
// CORE ENTITY STRUCTURES
// ============================================================================

// EntityType represents the different types of entities supported
type EntityType struct {
	EntityTypeID uuid.UUID `json:"entity_type_id" db:"entity_type_id"`
	Name         string    `json:"name" db:"name"`
	Description  string    `json:"description" db:"description"`
	TableName    string    `json:"table_name" db:"table_name"`
	CreatedAt    time.Time `json:"created_at" db:"created_at"`
	UpdatedAt    time.Time `json:"updated_at" db:"updated_at"`
}

// Entity represents the central entity registry
type Entity struct {
	EntityID     uuid.UUID `json:"entity_id" db:"entity_id"`
	EntityTypeID uuid.UUID `json:"entity_type_id" db:"entity_type_id"`
	ExternalID   *string   `json:"external_id" db:"external_id"`
	Name         string    `json:"name" db:"name"`
	CreatedAt    time.Time `json:"created_at" db:"created_at"`
	UpdatedAt    time.Time `json:"updated_at" db:"updated_at"`

	// Relationships
	EntityType *EntityType `json:"entity_type,omitempty"`
}

// Role represents roles entities can play within a CBU
type Role struct {
	RoleID      uuid.UUID `json:"role_id" db:"role_id"`
	Name        string    `json:"name" db:"name"`
	Description string    `json:"description" db:"description"`
	CreatedAt   time.Time `json:"created_at" db:"created_at"`
	UpdatedAt   time.Time `json:"updated_at" db:"updated_at"`
}

// CBUEntityRole links CBUs to entities through roles
type CBUEntityRole struct {
	CBUEntityRoleID uuid.UUID `json:"cbu_entity_role_id" db:"cbu_entity_role_id"`
	CBUID           uuid.UUID `json:"cbu_id" db:"cbu_id"`
	EntityID        uuid.UUID `json:"entity_id" db:"entity_id"`
	RoleID          uuid.UUID `json:"role_id" db:"role_id"`
	CreatedAt       time.Time `json:"created_at" db:"created_at"`

	// Relationships
	Entity *Entity `json:"entity,omitempty"`
	Role   *Role   `json:"role,omitempty"`
}

// ============================================================================
// ENTITY TYPE IMPLEMENTATIONS
// ============================================================================

// LimitedCompany represents a limited liability company or corporation
type LimitedCompany struct {
	LimitedCompanyID   uuid.UUID  `json:"limited_company_id" db:"limited_company_id"`
	CompanyName        string     `json:"company_name" db:"company_name"`
	RegistrationNumber *string    `json:"registration_number" db:"registration_number"`
	Jurisdiction       *string    `json:"jurisdiction" db:"jurisdiction"`
	IncorporationDate  *time.Time `json:"incorporation_date" db:"incorporation_date"`
	RegisteredAddress  *string    `json:"registered_address" db:"registered_address"`
	BusinessNature     *string    `json:"business_nature" db:"business_nature"`
	CreatedAt          time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt          time.Time  `json:"updated_at" db:"updated_at"`
}

// Partnership represents various partnership structures
type Partnership struct {
	PartnershipID            uuid.UUID  `json:"partnership_id" db:"partnership_id"`
	PartnershipName          string     `json:"partnership_name" db:"partnership_name"`
	PartnershipType          *string    `json:"partnership_type" db:"partnership_type"` // 'General', 'Limited', 'Limited Liability'
	Jurisdiction             *string    `json:"jurisdiction" db:"jurisdiction"`
	FormationDate            *time.Time `json:"formation_date" db:"formation_date"`
	PrincipalPlaceBusiness   *string    `json:"principal_place_business" db:"principal_place_business"`
	PartnershipAgreementDate *time.Time `json:"partnership_agreement_date" db:"partnership_agreement_date"`
	CreatedAt                time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt                time.Time  `json:"updated_at" db:"updated_at"`

	// Partnership-specific relationships
	PartnershipInterests []PartnershipInterest         `json:"partnership_interests,omitempty"`
	ControlMechanisms    []PartnershipControlMechanism `json:"control_mechanisms,omitempty"`
}

// Individual represents a natural person
type Individual struct {
	ProperProperPersonID uuid.UUID  `json:"proper_proper_person_id" db:"proper_proper_person_id"`
	FirstName            string     `json:"first_name" db:"first_name"`
	LastName             string     `json:"last_name" db:"last_name"`
	MiddleNames          *string    `json:"middle_names" db:"middle_names"`
	DateOfBirth          *time.Time `json:"date_of_birth" db:"date_of_birth"`
	Nationality          *string    `json:"nationality" db:"nationality"`
	ResidenceAddress     *string    `json:"residence_address" db:"residence_address"`
	IDDocumentType       *string    `json:"id_document_type" db:"id_document_type"`
	IDDocumentNumber     *string    `json:"id_document_number" db:"id_document_number"`
	CreatedAt            time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt            time.Time  `json:"updated_at" db:"updated_at"`
}

// Trust represents a trust structure
type Trust struct {
	TrustID           uuid.UUID  `json:"trust_id" db:"trust_id"`
	TrustName         string     `json:"trust_name" db:"trust_name"`
	TrustType         *string    `json:"trust_type" db:"trust_type"` // 'Discretionary', 'Fixed Interest', 'Unit Trust', 'Charitable'
	Jurisdiction      string     `json:"jurisdiction" db:"jurisdiction"`
	EstablishmentDate *time.Time `json:"establishment_date" db:"establishment_date"`
	TrustDeedDate     *time.Time `json:"trust_deed_date" db:"trust_deed_date"`
	TrustPurpose      *string    `json:"trust_purpose" db:"trust_purpose"`
	GoverningLaw      *string    `json:"governing_law" db:"governing_law"`
	CreatedAt         time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt         time.Time  `json:"updated_at" db:"updated_at"`

	// Trust-specific relationships
	TrustParties       []TrustParty            `json:"trust_parties,omitempty"`
	BeneficiaryClasses []TrustBeneficiaryClass `json:"beneficiary_classes,omitempty"`
}

// ============================================================================
// TRUST-SPECIFIC RELATIONSHIP STRUCTURES
// ============================================================================

// TrustParty represents the different roles within a trust
type TrustParty struct {
	TrustPartyID    uuid.UUID  `json:"trust_party_id" db:"trust_party_id"`
	TrustID         uuid.UUID  `json:"trust_id" db:"trust_id"`
	EntityID        uuid.UUID  `json:"entity_id" db:"entity_id"`
	PartyRole       string     `json:"party_role" db:"party_role"` // 'SETTLOR', 'TRUSTEE', 'BENEFICIARY', 'PROTECTOR'
	PartyType       string     `json:"party_type" db:"party_type"` // 'PROPER_PERSON', 'CORPORATE_TRUSTEE', 'BENEFICIARY_CLASS'
	AppointmentDate *time.Time `json:"appointment_date" db:"appointment_date"`
	ResignationDate *time.Time `json:"resignation_date" db:"resignation_date"`
	IsActive        bool       `json:"is_active" db:"is_active"`
	CreatedAt       time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt       time.Time  `json:"updated_at" db:"updated_at"`

	// Relationships
	Entity          *Entity               `json:"entity,omitempty"`
	ProtectorPowers []TrustProtectorPower `json:"protector_powers,omitempty"`
}

// TrustBeneficiaryClass represents beneficiary classes (e.g., "all grandchildren")
type TrustBeneficiaryClass struct {
	BeneficiaryClassID uuid.UUID `json:"beneficiary_class_id" db:"beneficiary_class_id"`
	TrustID            uuid.UUID `json:"trust_id" db:"trust_id"`
	ClassName          string    `json:"class_name" db:"class_name"`
	ClassDefinition    *string   `json:"class_definition" db:"class_definition"`
	ClassType          *string   `json:"class_type" db:"class_type"` // 'DESCENDANTS', 'SPOUSE_FAMILY', 'CHARITABLE_CLASS'
	MonitoringRequired bool      `json:"monitoring_required" db:"monitoring_required"`
	CreatedAt          time.Time `json:"created_at" db:"created_at"`
	UpdatedAt          time.Time `json:"updated_at" db:"updated_at"`
}

// TrustProtectorPower represents powers held by trust protectors
type TrustProtectorPower struct {
	ProtectorPowerID uuid.UUID `json:"protector_power_id" db:"protector_power_id"`
	TrustPartyID     uuid.UUID `json:"trust_party_id" db:"trust_party_id"`
	PowerType        string    `json:"power_type" db:"power_type"` // 'TRUSTEE_APPOINTMENT', 'TRUSTEE_REMOVAL', 'DISTRIBUTION_VETO'
	PowerDescription *string   `json:"power_description" db:"power_description"`
	IsActive         bool      `json:"is_active" db:"is_active"`
	CreatedAt        time.Time `json:"created_at" db:"created_at"`
}

// ============================================================================
// PARTNERSHIP-SPECIFIC RELATIONSHIP STRUCTURES
// ============================================================================

// PartnershipInterest represents ownership and control structure for partnerships
type PartnershipInterest struct {
	InterestID              uuid.UUID  `json:"interest_id" db:"interest_id"`
	PartnershipID           uuid.UUID  `json:"partnership_id" db:"partnership_id"`
	EntityID                uuid.UUID  `json:"entity_id" db:"entity_id"`
	PartnerType             string     `json:"partner_type" db:"partner_type"` // 'GENERAL_PARTNER', 'LIMITED_PARTNER', 'MANAGING_PARTNER'
	CapitalCommitment       *float64   `json:"capital_commitment" db:"capital_commitment"`
	OwnershipPercentage     *float64   `json:"ownership_percentage" db:"ownership_percentage"`
	VotingRights            *float64   `json:"voting_rights" db:"voting_rights"`
	ProfitSharingPercentage *float64   `json:"profit_sharing_percentage" db:"profit_sharing_percentage"`
	AdmissionDate           *time.Time `json:"admission_date" db:"admission_date"`
	WithdrawalDate          *time.Time `json:"withdrawal_date" db:"withdrawal_date"`
	IsActive                bool       `json:"is_active" db:"is_active"`
	CreatedAt               time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt               time.Time  `json:"updated_at" db:"updated_at"`

	// Relationships
	Entity *Entity `json:"entity,omitempty"`
}

// PartnershipControlMechanism represents how control is exercised
type PartnershipControlMechanism struct {
	ControlMechanismID uuid.UUID  `json:"control_mechanism_id" db:"control_mechanism_id"`
	PartnershipID      uuid.UUID  `json:"partnership_id" db:"partnership_id"`
	EntityID           uuid.UUID  `json:"entity_id" db:"entity_id"`
	ControlType        string     `json:"control_type" db:"control_type"` // 'MANAGEMENT_AGREEMENT', 'GP_CONTROL', 'INVESTMENT_COMMITTEE'
	ControlDescription *string    `json:"control_description" db:"control_description"`
	EffectiveDate      *time.Time `json:"effective_date" db:"effective_date"`
	TerminationDate    *time.Time `json:"termination_date" db:"termination_date"`
	IsActive           bool       `json:"is_active" db:"is_active"`
	CreatedAt          time.Time  `json:"created_at" db:"created_at"`

	// Relationships
	Entity *Entity `json:"entity,omitempty"`
}

// ============================================================================
// UBO IDENTIFICATION RESULTS
// ============================================================================

// UBORegistry represents the results of UBO identification across all entity types
type UBORegistry struct {
	UBOID               uuid.UUID  `json:"ubo_id" db:"ubo_id"`
	CBUID               uuid.UUID  `json:"cbu_id" db:"cbu_id"`
	SubjectEntityID     uuid.UUID  `json:"subject_entity_id" db:"subject_entity_id"`
	UBOProperPersonID   uuid.UUID  `json:"ubo_proper_person_id" db:"ubo_proper_person_id"`
	RelationshipType    string     `json:"relationship_type" db:"relationship_type"` // 'DIRECT_OWNERSHIP', 'TRUST_SETTLOR', 'PARTNERSHIP_GP_CONTROL'
	QualifyingReason    string     `json:"qualifying_reason" db:"qualifying_reason"` // 'OWNERSHIP_THRESHOLD', 'TRUST_CREATOR', 'ULTIMATE_CONTROL'
	OwnershipPercentage *float64   `json:"ownership_percentage" db:"ownership_percentage"`
	ControlType         *string    `json:"control_type" db:"control_type"`
	WorkflowType        string     `json:"workflow_type" db:"workflow_type"`               // 'STANDARD_CORPORATE', 'TRUST_SPECIFIC', 'PARTNERSHIP_DUAL_PRONG'
	RegulatoryFramework *string    `json:"regulatory_framework" db:"regulatory_framework"` // 'EU_5MLD', 'FATF_TRUST_GUIDANCE', 'US_CDD'
	VerificationStatus  string     `json:"verification_status" db:"verification_status"`   // 'PENDING', 'VERIFIED', 'FAILED'
	ScreeningResult     string     `json:"screening_result" db:"screening_result"`         // 'CLEARED', 'FLAGGED', 'BLOCKED'
	RiskRating          *string    `json:"risk_rating" db:"risk_rating"`                   // 'LOW', 'MEDIUM', 'HIGH', 'VERY_HIGH'
	IdentifiedAt        time.Time  `json:"identified_at" db:"identified_at"`
	VerifiedAt          *time.Time `json:"verified_at" db:"verified_at"`
	CreatedAt           time.Time  `json:"created_at" db:"created_at"`
	UpdatedAt           time.Time  `json:"updated_at" db:"updated_at"`

	// Relationships
	SubjectEntity *Entity `json:"subject_entity,omitempty"`
	UBOPerson     *Entity `json:"ubo_person,omitempty"`
}

// ============================================================================
// ENTITY TYPE CONSTANTS
// ============================================================================

const (
	EntityTypeLimitedCompany = "LIMITED_COMPANY"
	EntityTypePartnership    = "PARTNERSHIP"
	EntityTypeProperPerson   = "PROPER_PERSON"
	EntityTypeTrust          = "TRUST"
)

// Partnership Types
const (
	PartnershipTypeGeneral          = "General"
	PartnershipTypeLimited          = "Limited"
	PartnershipTypeLimitedLiability = "Limited Liability"
)

// Trust Types
const (
	TrustTypeDiscretionary = "Discretionary"
	TrustTypeFixedInterest = "Fixed Interest"
	TrustTypeUnitTrust     = "Unit Trust"
	TrustTypeCharitable    = "Charitable"
)

// Party Roles (Trust)
const (
	TrustPartyRoleSettlor     = "SETTLOR"
	TrustPartyRoleTrustee     = "TRUSTEE"
	TrustPartyRoleBeneficiary = "BENEFICIARY"
	TrustPartyRoleProtector   = "PROTECTOR"
)

// Party Types (Trust)
const (
	TrustPartyTypeNaturalPerson    = "PROPER_PERSON"
	TrustPartyTypeCorporateTrustee = "CORPORATE_TRUSTEE"
	TrustPartyTypeBeneficiaryClass = "BENEFICIARY_CLASS"
)

// Partner Types (Partnership)
const (
	PartnerTypeGeneral  = "GENERAL_PARTNER"
	PartnerTypeLimited  = "LIMITED_PARTNER"
	PartnerTypeManaging = "MANAGING_PARTNER"
)

// UBO Relationship Types
const (
	UBORelationshipDirectOwnership      = "DIRECT_OWNERSHIP"
	UBORelationshipIndirectOwnership    = "INDIRECT_OWNERSHIP"
	UBORelationshipTrustSettlor         = "TRUST_SETTLOR"
	UBORelationshipTrustTrustee         = "TRUST_TRUSTEE"
	UBORelationshipTrustBeneficiary     = "TRUST_BENEFICIARY"
	UBORelationshipTrustProtector       = "TRUST_PROTECTOR"
	UBORelationshipPartnershipOwnership = "LIMITED_PARTNER_OWNERSHIP"
	UBORelationshipPartnershipControl   = "GENERAL_PARTNER_CONTROL"
)

// UBO Workflow Types
const (
	UBOWorkflowStandardCorporate    = "STANDARD_CORPORATE"
	UBOWorkflowTrustSpecific        = "TRUST_SPECIFIC"
	UBOWorkflowPartnershipDualProng = "PARTNERSHIP_DUAL_PRONG"
	UBOWorkflowRecursiveAnalysis    = "RECURSIVE_ANALYSIS"
)

// Verification Status
const (
	VerificationStatusPending  = "PENDING"
	VerificationStatusVerified = "VERIFIED"
	VerificationStatusFailed   = "FAILED"
)

// Screening Results
const (
	ScreeningResultCleared = "CLEARED"
	ScreeningResultFlagged = "FLAGGED"
	ScreeningResultBlocked = "BLOCKED"
	ScreeningResultPending = "PENDING"
)

// ============================================================================
// HELPER METHODS
// ============================================================================

// GetFullName returns the full name for an individual
func (i *Individual) GetFullName() string {
	fullName := i.FirstName
	if i.MiddleNames != nil && *i.MiddleNames != "" {
		fullName += " " + *i.MiddleNames
	}
	fullName += " " + i.LastName
	return fullName
}

// IsNaturalPerson returns true if the entity type represents a natural person
func (e *Entity) IsNaturalPerson() bool {
	return e.EntityType != nil && e.EntityType.Name == EntityTypeProperPerson
}

// IsCorporateEntity returns true if the entity type represents a corporate entity
func (e *Entity) IsCorporateEntity() bool {
	if e.EntityType == nil {
		return false
	}
	return e.EntityType.Name == EntityTypeLimitedCompany ||
		e.EntityType.Name == EntityTypePartnership
}

// RequiresUBOAnalysis returns true if the entity type requires UBO analysis
func (e *Entity) RequiresUBOAnalysis() bool {
	if e.EntityType == nil {
		return false
	}
	// All entity types except individuals require UBO analysis
	return e.EntityType.Name != EntityTypeProperPerson
}

// GetUBOWorkflowType returns the appropriate UBO workflow type for the entity
func (e *Entity) GetUBOWorkflowType() string {
	if e.EntityType == nil {
		return UBOWorkflowStandardCorporate
	}

	switch e.EntityType.Name {
	case EntityTypeTrust:
		return UBOWorkflowTrustSpecific
	case EntityTypePartnership:
		return UBOWorkflowPartnershipDualProng
	case EntityTypeLimitedCompany:
		return UBOWorkflowStandardCorporate
	default:
		return UBOWorkflowStandardCorporate
	}
}

// IsCurrentlyActive returns true if the trust party is currently active
func (tp *TrustParty) IsCurrentlyActive() bool {
	return tp.IsActive && (tp.ResignationDate == nil || tp.ResignationDate.After(time.Now()))
}

// IsCurrentlyActive returns true if the partnership interest is currently active
func (pi *PartnershipInterest) IsCurrentlyActive() bool {
	return pi.IsActive && (pi.WithdrawalDate == nil || pi.WithdrawalDate.After(time.Now()))
}

// ExceedsOwnershipThreshold returns true if the partnership interest exceeds the given threshold
func (pi *PartnershipInterest) ExceedsOwnershipThreshold(threshold float64) bool {
	return pi.OwnershipPercentage != nil && *pi.OwnershipPercentage >= threshold
}
