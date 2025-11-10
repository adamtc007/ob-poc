// DEPRECATED: This file is marked for deletion as part of multi-domain migration.
//
// Migration Status: Phase 4 - Create Onboarding Domain
// New Location: internal/domains/onboarding/vocab.go
// Deprecation Date: 2024-01-XX
// Planned Deletion: After Phase 6 complete and all tests passing
//
// DO NOT MODIFY THIS FILE - It is kept for reference and regression testing only.
// See MIGRATION_DEPRECATION_TRACKER.md for details.
//
// This file contains 68 onboarding verbs across 9 categories that will be
// migrated to the new domain-specific vocabulary system.

package dsl

import (
	"fmt"
	"strings"
	"time"
)

// vocab.go defines the comprehensive vocabulary of DSL onboarding verbs
// This allows systematic testing and validation of DSL coverage across all onboarding scenarios

// =============================================================================
// DSL Vocabulary: Onboarding Domain Verbs
// =============================================================================

// CaseManagementVerbs defines case lifecycle management operations
type CaseManagementVerbs struct{}

func (CaseManagementVerbs) Create(cbuID, naturePurpose string) string {
	return fmt.Sprintf("(case.create (cbu.id %q) (nature-purpose %q))", cbuID, naturePurpose)
}

func (CaseManagementVerbs) Approve(approverID, timestamp string) string {
	return fmt.Sprintf("(case.approve (approver.id %q) (timestamp %q))", approverID, timestamp)
}

// EntityIdentityVerbs defines entity registration and identity management
type EntityIdentityVerbs struct{}

func (EntityIdentityVerbs) Register(entityType, jurisdiction string) string {
	return fmt.Sprintf("(entity.register (type %q) (jurisdiction %q))", entityType, jurisdiction)
}

// ProductServiceVerbs defines product and service management operations
type ProductServiceVerbs struct{}

func (ProductServiceVerbs) AddProducts(products ...string) string {
	quotedProducts := make([]string, len(products))
	for i, p := range products {
		quotedProducts[i] = fmt.Sprintf("%q", p)
	}
	return fmt.Sprintf("(products.add %s)", strings.Join(quotedProducts, " "))
}

func (ProductServiceVerbs) Configure(product, settingsID string) string {
	return fmt.Sprintf("(products.configure (product %q) (settings.id %q))", product, settingsID)
}

func (ProductServiceVerbs) DiscoverServices(product string, services ...string) string {
	var serviceBlocks []string
	for _, service := range services {
		serviceBlocks = append(serviceBlocks, fmt.Sprintf("(service %q)", service))
	}
	return fmt.Sprintf("(services.discover\n  (for.product %q\n    %s\n  )\n)",
		product, strings.Join(serviceBlocks, "\n    "))
}

func (ProductServiceVerbs) ProvisionService(serviceID, configID string) string {
	return fmt.Sprintf("(services.provision (service.id %q) (config.id %q))", serviceID, configID)
}

func (ProductServiceVerbs) ActivateService(serviceID, effectiveDate string) string {
	return fmt.Sprintf("(services.activate (service.id %q) (effective-date %q))", serviceID, effectiveDate)
}

// KYCComplianceVerbs defines KYC and compliance operations
type KYCComplianceVerbs struct{}

func (KYCComplianceVerbs) Start(requirementsID string) string {
	return fmt.Sprintf("(kyc.start (requirements.id %q))", requirementsID)
}

func (KYCComplianceVerbs) StartWithDocuments(documents []string, jurisdictions []string) string {
	var b strings.Builder
	b.WriteString("(kyc.start\n")

	if len(documents) > 0 {
		b.WriteString("  (documents\n")
		for _, doc := range documents {
			b.WriteString(fmt.Sprintf("    (document %q)\n", doc))
		}
		b.WriteString("  )\n")
	}

	if len(jurisdictions) > 0 {
		b.WriteString("  (jurisdictions\n")
		for _, jur := range jurisdictions {
			b.WriteString(fmt.Sprintf("    (jurisdiction %q)\n", jur))
		}
		b.WriteString("  )\n")
	}

	b.WriteString(")")
	return b.String()
}

func (KYCComplianceVerbs) CollectDocument(documentID, docType string) string {
	return fmt.Sprintf("(kyc.collect (document.id %q) (type %q))", documentID, docType)
}

func (KYCComplianceVerbs) VerifyDocument(documentID, verifierID string) string {
	return fmt.Sprintf("(kyc.verify (document.id %q) (verifier.id %q))", documentID, verifierID)
}

func (KYCComplianceVerbs) AssessRisk(riskRating, rationaleID string) string {
	return fmt.Sprintf("(kyc.assess (risk-rating %q) (rationale.id %q))", riskRating, rationaleID)
}

func (KYCComplianceVerbs) Screen(list, resultID string) string {
	return fmt.Sprintf("(compliance.screen (list %q) (result.id %q))", list, resultID)
}

func (KYCComplianceVerbs) Monitor(triggerID, frequency string) string {
	return fmt.Sprintf("(compliance.monitor (trigger.id %q) (frequency %q))", triggerID, frequency)
}

// ResourceInfrastructureVerbs defines resource and infrastructure management
type ResourceInfrastructureVerbs struct{}

func (ResourceInfrastructureVerbs) PlanResource(name, owner, varAttrID string) string {
	return fmt.Sprintf("(resources.plan\n  (resource.create %q\n    (owner %q)\n    (var (attr-id %q))\n  )\n)",
		name, owner, varAttrID)
}

func (ResourceInfrastructureVerbs) ProvisionResource(resourceID, providerID string) string {
	return fmt.Sprintf("(resources.provision (resource.id %q) (provider.id %q))", resourceID, providerID)
}

func (ResourceInfrastructureVerbs) ConfigureResource(resourceID, configID string) string {
	return fmt.Sprintf("(resources.configure (resource.id %q) (config.id %q))", resourceID, configID)
}

func (ResourceInfrastructureVerbs) TestResource(resourceID, testSuiteID string) string {
	return fmt.Sprintf("(resources.test (resource.id %q) (test-suite.id %q))", resourceID, testSuiteID)
}

func (ResourceInfrastructureVerbs) DeployResource(resourceID, environment string) string {
	return fmt.Sprintf("(resources.deploy (resource.id %q) (environment %q))", resourceID, environment)
}

// AttributeDataVerbs defines attribute and data binding operations
type AttributeDataVerbs struct{}

func (AttributeDataVerbs) DefineAttribute(attrID, attrType string) string {
	return fmt.Sprintf("(attributes.define (attr.id %q) (type %q))", attrID, attrType)
}

func (AttributeDataVerbs) ResolveAttribute(attrID, sourceID string) string {
	return fmt.Sprintf("(attributes.resolve (attr.id %q) (source.id %q))", attrID, sourceID)
}

func (AttributeDataVerbs) BindValue(attrID, value string) string {
	return fmt.Sprintf("(values.bind (bind (attr-id %q) (value %q)))", attrID, value)
}

func (AttributeDataVerbs) ValidateValue(attrID, ruleID string) string {
	return fmt.Sprintf("(values.validate (attr.id %q) (rule.id %q))", attrID, ruleID)
}

func (AttributeDataVerbs) EncryptValue(attrID, keyID string) string {
	return fmt.Sprintf("(values.encrypt (attr.id %q) (key.id %q))", attrID, keyID)
}

// WorkflowStateVerbs defines workflow and state management operations
type WorkflowStateVerbs struct{}

func (WorkflowStateVerbs) Transition(from, to string) string {
	return fmt.Sprintf("(workflow.transition (from %q) (to %q))", from, to)
}

func (WorkflowStateVerbs) Gate(conditionID string, required bool) string {
	return fmt.Sprintf("(workflow.gate (condition.id %q) (required %t))", conditionID, required)
}

func (WorkflowStateVerbs) CreateTask(taskID, taskType string) string {
	return fmt.Sprintf("(tasks.create (task.id %q) (type %q))", taskID, taskType)
}

func (WorkflowStateVerbs) AssignTask(taskID, assigneeID string) string {
	return fmt.Sprintf("(tasks.assign (task.id %q) (assignee.id %q))", taskID, assigneeID)
}

func (WorkflowStateVerbs) CompleteTask(taskID, outcome string) string {
	return fmt.Sprintf("(tasks.complete (task.id %q) (outcome %q))", taskID, outcome)
}

// NotificationCommunicationVerbs defines notification and communication operations
type NotificationCommunicationVerbs struct{}

func (NotificationCommunicationVerbs) SendNotification(recipientID, templateID string) string {
	return fmt.Sprintf("(notify.send (recipient.id %q) (template.id %q))", recipientID, templateID)
}

func (NotificationCommunicationVerbs) RequestCommunication(partyID, documentID string) string {
	return fmt.Sprintf("(communicate.request (party.id %q) (document.id %q))", partyID, documentID)
}

func (NotificationCommunicationVerbs) TriggerEscalation(conditionID, level string) string {
	return fmt.Sprintf("(escalate.trigger (condition.id %q) (level %q))", conditionID, level)
}

func (NotificationCommunicationVerbs) LogAudit(eventID, actorID string) string {
	return fmt.Sprintf("(audit.log (event.id %q) (actor.id %q))", eventID, actorID)
}

// IntegrationExternalVerbs defines integration and external system operations
type IntegrationExternalVerbs struct{}

func (IntegrationExternalVerbs) QueryExternal(system, endpointID string) string {
	return fmt.Sprintf("(external.query (system %q) (endpoint.id %q))", system, endpointID)
}

func (IntegrationExternalVerbs) SyncExternal(systemID, dataID string) string {
	return fmt.Sprintf("(external.sync (system.id %q) (data.id %q))", systemID, dataID)
}

func (IntegrationExternalVerbs) CallAPI(endpointID, payloadID string) string {
	return fmt.Sprintf("(api.call (endpoint.id %q) (payload.id %q))", endpointID, payloadID)
}

func (IntegrationExternalVerbs) RegisterWebhook(urlID, events string) string {
	return fmt.Sprintf("(webhook.register (url.id %q) (events %q))", urlID, events)
}

// =============================================================================
// DSL Vocabulary Registry - Central access point for all verb categories
// =============================================================================

type DSLVocabulary struct {
	Case         CaseManagementVerbs
	Entity       EntityIdentityVerbs
	Product      ProductServiceVerbs
	KYC          KYCComplianceVerbs
	Resource     ResourceInfrastructureVerbs
	Attribute    AttributeDataVerbs
	Workflow     WorkflowStateVerbs
	Notification NotificationCommunicationVerbs
	Integration  IntegrationExternalVerbs
}

// NewDSLVocabulary creates a new vocabulary registry
func NewDSLVocabulary() *DSLVocabulary {
	return &DSLVocabulary{
		Case:         CaseManagementVerbs{},
		Entity:       EntityIdentityVerbs{},
		Product:      ProductServiceVerbs{},
		KYC:          KYCComplianceVerbs{},
		Resource:     ResourceInfrastructureVerbs{},
		Attribute:    AttributeDataVerbs{},
		Workflow:     WorkflowStateVerbs{},
		Notification: NotificationCommunicationVerbs{},
		Integration:  IntegrationExternalVerbs{},
	}
}

// =============================================================================
// Advanced Verb Categories for Extended Functionality
// =============================================================================

// TemporalSchedulingVerbs defines time-based and scheduling operations
type TemporalSchedulingVerbs struct{}

func (TemporalSchedulingVerbs) CreateSchedule(taskID, cron string) string {
	return fmt.Sprintf("(schedule.create (task.id %q) (cron %q))", taskID, cron)
}

func (TemporalSchedulingVerbs) SetDeadline(taskID, date string) string {
	return fmt.Sprintf("(deadline.set (task.id %q) (date %q))", taskID, date)
}

func (TemporalSchedulingVerbs) ScheduleReminder(notificationID, offset string) string {
	return fmt.Sprintf("(reminder.schedule (notification.id %q) (offset %q))", notificationID, offset)
}

// RiskMonitoringVerbs defines risk assessment and monitoring operations
type RiskMonitoringVerbs struct{}

func (RiskMonitoringVerbs) AssessRisk(factorID string, weight float64) string {
	return fmt.Sprintf("(risk.assess (factor.id %q) (weight %.2f))", factorID, weight)
}

func (RiskMonitoringVerbs) SetupMonitor(metricID string, threshold int) string {
	return fmt.Sprintf("(monitor.setup (metric.id %q) (threshold %d))", metricID, threshold)
}

func (RiskMonitoringVerbs) TriggerAlert(conditionID, severity string) string {
	return fmt.Sprintf("(alert.trigger (condition.id %q) (severity %q))", conditionID, severity)
}

// DataLifecycleVerbs defines data management and lifecycle operations
type DataLifecycleVerbs struct{}

func (DataLifecycleVerbs) CollectData(sourceID, destinationID string) string {
	return fmt.Sprintf("(data.collect (source.id %q) (destination.id %q))", sourceID, destinationID)
}

func (DataLifecycleVerbs) TransformData(transformerID, inputID string) string {
	return fmt.Sprintf("(data.transform (transformer.id %q) (input.id %q))", transformerID, inputID)
}

func (DataLifecycleVerbs) ArchiveData(dataID, retention string) string {
	return fmt.Sprintf("(data.archive (data.id %q) (retention %q))", dataID, retention)
}

func (DataLifecycleVerbs) PurgeData(dataID, reason string) string {
	return fmt.Sprintf("(data.purge (data.id %q) (reason %q))", dataID, reason)
}

// =============================================================================
// Utility Functions for DSL Construction
// =============================================================================

// GenerateUUID creates a mock UUID for testing (simple incrementing counter for now)
var uuidCounter int

func GenerateTestUUID(prefix string) string {
	uuidCounter++
	return fmt.Sprintf("%s-uuid-%d", prefix, uuidCounter)
}

// GenerateTimestamp creates a formatted timestamp for DSL operations
func GenerateTimestamp() string {
	return time.Now().UTC().Format("2006-01-02T15:04:05Z")
}

// CombineDSLBlocks combines multiple DSL blocks with proper formatting
func CombineDSLBlocks(blocks ...string) string {
	var nonEmptyBlocks []string
	for _, block := range blocks {
		if strings.TrimSpace(block) != "" {
			nonEmptyBlocks = append(nonEmptyBlocks, block)
		}
	}
	return strings.Join(nonEmptyBlocks, "\n\n")
}

// =============================================================================
// Example Usage Documentation
// =============================================================================

/*
Example Usage:

vocab := NewDSLVocabulary()

// Create a complete onboarding DSL workflow
caseBlock := vocab.Case.Create("CBU-1234", "UCITS equity fund domiciled in LU")
productBlock := vocab.Product.AddProducts("CUSTODY", "FUND_ACCOUNTING")
kycBlock := vocab.KYC.StartWithDocuments(
    []string{"CertificateOfIncorporation", "W8BEN-E"},
    []string{"LU"})
resourceBlock := vocab.Resource.PlanResource("CustodyAccount", "CustodyTech", GenerateTestUUID("attr"))

completeDSL := CombineDSLBlocks(caseBlock, productBlock, kycBlock, resourceBlock)
fmt.Println(completeDSL)
*/
