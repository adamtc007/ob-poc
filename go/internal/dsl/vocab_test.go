package dsl

import (
	"strings"
	"testing"
)

func TestDSLVocabularyBasicUsage(t *testing.T) {
	vocab := NewDSLVocabulary()

	t.Run("Case Management Verbs", func(t *testing.T) {
		result := vocab.Case.Create("CBU-1234", "UCITS equity fund")
		expected := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund"))`
		if result != expected {
			t.Errorf("Expected %q, got %q", expected, result)
		}

		approve := vocab.Case.Approve("user-123", "2024-01-01T10:00:00Z")
		if !strings.Contains(approve, "case.approve") {
			t.Errorf("Approve verb should contain 'case.approve'")
		}
	})

	t.Run("Product Service Verbs", func(t *testing.T) {
		result := vocab.Product.AddProducts("CUSTODY", "FUND_ACCOUNTING")
		expected := `(products.add "CUSTODY" "FUND_ACCOUNTING")`
		if result != expected {
			t.Errorf("Expected %q, got %q", expected, result)
		}

		services := vocab.Product.DiscoverServices("CUSTODY", "CustodyService", "SettlementService")
		if !strings.Contains(services, "services.discover") {
			t.Errorf("Services should contain 'services.discover'")
		}
		if !strings.Contains(services, "for.product") {
			t.Errorf("Services should contain 'for.product'")
		}
	})

	t.Run("KYC Compliance Verbs", func(t *testing.T) {
		docs := []string{"CertificateOfIncorporation", "W8BEN-E"}
		jurisdictions := []string{"LU", "US"}
		result := vocab.KYC.StartWithDocuments(docs, jurisdictions)

		if !strings.Contains(result, "kyc.start") {
			t.Errorf("KYC should contain 'kyc.start'")
		}
		if !strings.Contains(result, "documents") {
			t.Errorf("KYC should contain 'documents'")
		}
		if !strings.Contains(result, "jurisdictions") {
			t.Errorf("KYC should contain 'jurisdictions'")
		}
		if !strings.Contains(result, "CertificateOfIncorporation") {
			t.Errorf("KYC should contain document names")
		}
	})

	t.Run("Resource Infrastructure Verbs", func(t *testing.T) {
		attrID := GenerateTestUUID("attr")
		result := vocab.Resource.PlanResource("CustodyAccount", "CustodyTech", attrID)

		if !strings.Contains(result, "resources.plan") {
			t.Errorf("Resource should contain 'resources.plan'")
		}
		if !strings.Contains(result, "resource.create") {
			t.Errorf("Resource should contain 'resource.create'")
		}
		if !strings.Contains(result, attrID) {
			t.Errorf("Resource should contain the generated UUID")
		}
	})

	t.Run("Attribute Data Verbs", func(t *testing.T) {
		attrID := GenerateTestUUID("test")
		result := vocab.Attribute.BindValue(attrID, "CBU-1234")

		if !strings.Contains(result, "values.bind") {
			t.Errorf("Binding should contain 'values.bind'")
		}
		if !strings.Contains(result, attrID) {
			t.Errorf("Binding should contain the attribute ID")
		}
		if !strings.Contains(result, "CBU-1234") {
			t.Errorf("Binding should contain the value")
		}
	})
}

func TestCompleteOnboardingWorkflow(t *testing.T) {
	vocab := NewDSLVocabulary()

	// Test a complete onboarding workflow using the vocabulary
	caseBlock := vocab.Case.Create("CBU-1234", "UCITS equity fund domiciled in LU")
	productBlock := vocab.Product.AddProducts("CUSTODY", "FUND_ACCOUNTING")

	kycBlock := vocab.KYC.StartWithDocuments(
		[]string{"CertificateOfIncorporation", "ArticlesOfAssociation", "W8BEN-E"},
		[]string{"LU"})

	servicesBlock := vocab.Product.DiscoverServices("CUSTODY", "CustodyService", "SettlementService")

	attrID := GenerateTestUUID("custody-attr")
	resourceBlock := vocab.Resource.PlanResource("CustodyAccount", "CustodyTech", attrID)

	bindingBlock := vocab.Attribute.BindValue(attrID, "CBU-1234")

	// Combine all blocks
	completeDSL := CombineDSLBlocks(
		caseBlock,
		productBlock,
		kycBlock,
		servicesBlock,
		resourceBlock,
		bindingBlock,
	)

	// Validate the complete DSL
	if !strings.Contains(completeDSL, "case.create") {
		t.Errorf("Complete DSL should contain case creation")
	}
	if !strings.Contains(completeDSL, "products.add") {
		t.Errorf("Complete DSL should contain products")
	}
	if !strings.Contains(completeDSL, "kyc.start") {
		t.Errorf("Complete DSL should contain KYC")
	}
	if !strings.Contains(completeDSL, "services.discover") {
		t.Errorf("Complete DSL should contain services")
	}
	if !strings.Contains(completeDSL, "resources.plan") {
		t.Errorf("Complete DSL should contain resources")
	}
	if !strings.Contains(completeDSL, "values.bind") {
		t.Errorf("Complete DSL should contain value binding")
	}

	// Verify UUID consistency
	uuidCount := strings.Count(completeDSL, attrID)
	if uuidCount != 2 { // Should appear in both resource planning and value binding
		t.Errorf("Expected UUID to appear 2 times, got %d", uuidCount)
	}

	t.Logf("Complete DSL workflow:\n%s", completeDSL)
}

func TestAdvancedVerbCategories(t *testing.T) {
	temporal := TemporalSchedulingVerbs{}
	risk := RiskMonitoringVerbs{}
	data := DataLifecycleVerbs{}

	t.Run("Temporal Scheduling", func(t *testing.T) {
		schedule := temporal.CreateSchedule("task-123", "0 9 * * 1")
		if !strings.Contains(schedule, "schedule.create") {
			t.Errorf("Schedule should contain 'schedule.create'")
		}

		deadline := temporal.SetDeadline("task-123", "2024-12-31")
		if !strings.Contains(deadline, "deadline.set") {
			t.Errorf("Deadline should contain 'deadline.set'")
		}
	})

	t.Run("Risk Monitoring", func(t *testing.T) {
		assess := risk.AssessRisk("factor-123", 0.75)
		if !strings.Contains(assess, "risk.assess") {
			t.Errorf("Risk assessment should contain 'risk.assess'")
		}
		if !strings.Contains(assess, "0.75") {
			t.Errorf("Risk assessment should contain weight value")
		}

		monitor := risk.SetupMonitor("metric-123", 100)
		if !strings.Contains(monitor, "monitor.setup") {
			t.Errorf("Monitor should contain 'monitor.setup'")
		}
	})

	t.Run("Data Lifecycle", func(t *testing.T) {
		collect := data.CollectData("source-123", "dest-456")
		if !strings.Contains(collect, "data.collect") {
			t.Errorf("Data collection should contain 'data.collect'")
		}

		archive := data.ArchiveData("data-123", "7years")
		if !strings.Contains(archive, "data.archive") {
			t.Errorf("Data archive should contain 'data.archive'")
		}
	})
}

func TestDSLVocabularyPermutations(t *testing.T) {
	vocab := NewDSLVocabulary()

	// Test different entity types
	entityTypes := []string{"limited-company", "partnership", "trust", "individual"}
	jurisdictions := []string{"LU", "US", "UK", "DE"}

	for _, entityType := range entityTypes {
		for _, jurisdiction := range jurisdictions {
			result := vocab.Entity.Register(entityType, jurisdiction)
			if !strings.Contains(result, entityType) {
				t.Errorf("Entity registration should contain entity type %s", entityType)
			}
			if !strings.Contains(result, jurisdiction) {
				t.Errorf("Entity registration should contain jurisdiction %s", jurisdiction)
			}
		}
	}

	// Test product combinations
	productCombinations := [][]string{
		{"CUSTODY"},
		{"CUSTODY", "FUND_ACCOUNTING"},
		{"CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENT"},
		{"PRIME_BROKERAGE", "TRANSFER_AGENT"},
	}

	for _, products := range productCombinations {
		result := vocab.Product.AddProducts(products...)
		for _, product := range products {
			if !strings.Contains(result, product) {
				t.Errorf("Product combination should contain %s", product)
			}
		}
	}

	// Test KYC document variations
	documentSets := [][]string{
		{"CertificateOfIncorporation", "ArticlesOfAssociation"},
		{"CertificateOfLimitedPartnership", "PartnershipAgreement", "W9"},
		{"CertificateOfIncorporation", "ArticlesOfAssociation", "W8BEN-E", "Prospectus"},
	}

	for _, docs := range documentSets {
		result := vocab.KYC.StartWithDocuments(docs, []string{"US"})
		for _, doc := range docs {
			if !strings.Contains(result, doc) {
				t.Errorf("KYC block should contain document %s", doc)
			}
		}
	}
}

func TestUUIDGeneration(t *testing.T) {
	// Test UUID generation consistency
	uuid1 := GenerateTestUUID("test")
	uuid2 := GenerateTestUUID("test")

	if uuid1 == uuid2 {
		t.Errorf("Generated UUIDs should be different: %s == %s", uuid1, uuid2)
	}

	if !strings.HasPrefix(uuid1, "test-uuid-") {
		t.Errorf("UUID should have correct prefix: %s", uuid1)
	}
}

func TestTimestampGeneration(t *testing.T) {
	timestamp := GenerateTimestamp()

	// Basic format check - should be ISO 8601
	if !strings.Contains(timestamp, "T") || !strings.HasSuffix(timestamp, "Z") {
		t.Errorf("Timestamp should be in ISO 8601 format: %s", timestamp)
	}
}

func TestDSLBlockCombination(t *testing.T) {
	blocks := []string{
		"(case.create (cbu.id \"CBU-1234\"))",
		"(products.add \"CUSTODY\")",
		"", // Empty block should be filtered out
		"(kyc.start (documents (document \"W8BEN-E\")))",
	}

	result := CombineDSLBlocks(blocks...)

	// Should not contain empty blocks
	if strings.Contains(result, "\n\n\n") {
		t.Errorf("Combined DSL should not have triple newlines (empty blocks)")
	}

	// Should contain all non-empty blocks
	if !strings.Contains(result, "case.create") {
		t.Errorf("Combined DSL should contain case.create")
	}
	if !strings.Contains(result, "products.add") {
		t.Errorf("Combined DSL should contain products.add")
	}
	if !strings.Contains(result, "kyc.start") {
		t.Errorf("Combined DSL should contain kyc.start")
	}
}

// Benchmark tests for performance validation
func BenchmarkDSLVocabularyCreation(b *testing.B) {
	for i := 0; i < b.N; i++ {
		_ = NewDSLVocabulary()
	}
}

func BenchmarkComplexDSLGeneration(b *testing.B) {
	vocab := NewDSLVocabulary()

	for i := 0; i < b.N; i++ {
		caseBlock := vocab.Case.Create("CBU-1234", "UCITS equity fund")
		productBlock := vocab.Product.AddProducts("CUSTODY", "FUND_ACCOUNTING")
		kycBlock := vocab.KYC.StartWithDocuments(
			[]string{"CertificateOfIncorporation", "W8BEN-E"},
			[]string{"LU"})

		_ = CombineDSLBlocks(caseBlock, productBlock, kycBlock)
	}
}
