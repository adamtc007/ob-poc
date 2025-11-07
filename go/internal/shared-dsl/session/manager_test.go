package session

import (
	"strings"
	"sync"
	"testing"
	"time"
)

// =============================================================================
// Session Creation and Retrieval Tests
// =============================================================================

func TestNewManager(t *testing.T) {
	mgr := NewManager()
	if mgr == nil {
		t.Fatal("NewManager returned nil")
	}

	if mgr.Count() != 0 {
		t.Errorf("Expected 0 sessions, got %d", mgr.Count())
	}
}

func TestNewSession(t *testing.T) {
	session := NewSession("test-session", "onboarding")

	if session.SessionID != "test-session" {
		t.Errorf("Expected SessionID 'test-session', got '%s'", session.SessionID)
	}

	if session.Domain != "onboarding" {
		t.Errorf("Expected domain 'onboarding', got '%s'", session.Domain)
	}

	if session.BuiltDSL != "" {
		t.Errorf("Expected empty BuiltDSL, got '%s'", session.BuiltDSL)
	}

	if session.Context.Data == nil {
		t.Error("Expected Context.Data to be initialized")
	}

	if session.History == nil {
		t.Error("Expected History to be initialized")
	}
}

func TestNewSession_AutoGenerateID(t *testing.T) {
	session := NewSession("", "onboarding")

	if session.SessionID == "" {
		t.Error("Expected auto-generated SessionID")
	}

	// Should be a valid UUID format
	if len(session.SessionID) != 36 {
		t.Errorf("Expected UUID length 36, got %d", len(session.SessionID))
	}
}

func TestManager_GetOrCreate_CreateNew(t *testing.T) {
	mgr := NewManager()

	session := mgr.GetOrCreate("test-1", "onboarding")

	if session.SessionID != "test-1" {
		t.Errorf("Expected SessionID 'test-1', got '%s'", session.SessionID)
	}

	if session.Domain != "onboarding" {
		t.Errorf("Expected domain 'onboarding', got '%s'", session.Domain)
	}

	if mgr.Count() != 1 {
		t.Errorf("Expected 1 session, got %d", mgr.Count())
	}
}

func TestManager_GetOrCreate_GetExisting(t *testing.T) {
	mgr := NewManager()

	// Create first session
	session1 := mgr.GetOrCreate("test-1", "onboarding")
	session1.BuiltDSL = "(case.create)"

	// Get same session
	session2 := mgr.GetOrCreate("test-1", "")

	if session1 != session2 {
		t.Error("Expected to get the same session instance")
	}

	if session2.BuiltDSL != "(case.create)" {
		t.Error("Expected existing DSL to be preserved")
	}

	if mgr.Count() != 1 {
		t.Errorf("Expected 1 session, got %d", mgr.Count())
	}
}

func TestManager_GetOrCreate_SwitchDomain(t *testing.T) {
	mgr := NewManager()

	// Create session with onboarding domain
	session := mgr.GetOrCreate("test-1", "onboarding")
	if session.Domain != "onboarding" {
		t.Errorf("Expected 'onboarding', got '%s'", session.Domain)
	}

	// Get with different domain
	session = mgr.GetOrCreate("test-1", "hedge-fund-investor")
	if session.Domain != "hedge-fund-investor" {
		t.Errorf("Expected 'hedge-fund-investor', got '%s'", session.Domain)
	}
}

func TestManager_Get_NotFound(t *testing.T) {
	mgr := NewManager()

	_, err := mgr.Get("nonexistent")
	if err == nil {
		t.Error("Expected error for nonexistent session")
	}

	if !strings.Contains(err.Error(), "not found") {
		t.Errorf("Expected 'not found' error, got: %v", err)
	}
}

func TestManager_Get_Existing(t *testing.T) {
	mgr := NewManager()

	mgr.GetOrCreate("test-1", "onboarding")

	session, err := mgr.Get("test-1")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}

	if session.SessionID != "test-1" {
		t.Errorf("Expected SessionID 'test-1', got '%s'", session.SessionID)
	}
}

func TestManager_Delete(t *testing.T) {
	mgr := NewManager()

	mgr.GetOrCreate("test-1", "onboarding")

	if mgr.Count() != 1 {
		t.Fatalf("Expected 1 session before delete")
	}

	err := mgr.Delete("test-1")
	if err != nil {
		t.Fatalf("Delete failed: %v", err)
	}

	if mgr.Count() != 0 {
		t.Errorf("Expected 0 sessions after delete, got %d", mgr.Count())
	}
}

func TestManager_Delete_NotFound(t *testing.T) {
	mgr := NewManager()

	err := mgr.Delete("nonexistent")
	if err == nil {
		t.Error("Expected error when deleting nonexistent session")
	}
}

func TestManager_List(t *testing.T) {
	mgr := NewManager()

	mgr.GetOrCreate("test-1", "onboarding")
	mgr.GetOrCreate("test-2", "hedge-fund-investor")
	mgr.GetOrCreate("test-3", "onboarding")

	list := mgr.List()
	if len(list) != 3 {
		t.Errorf("Expected 3 sessions, got %d", len(list))
	}

	// Check all IDs are present
	found := make(map[string]bool)
	for _, id := range list {
		found[id] = true
	}

	if !found["test-1"] || !found["test-2"] || !found["test-3"] {
		t.Error("Expected all session IDs in list")
	}
}

// =============================================================================
// DSL Accumulation Tests (CRITICAL - DSL-as-State Pattern)
// =============================================================================

func TestSession_AccumulateDSL_Empty(t *testing.T) {
	session := NewSession("test", "onboarding")

	dsl := "(case.create (cbu.id \"CBU-1234\"))"
	err := session.AccumulateDSL(dsl)
	if err != nil {
		t.Fatalf("AccumulateDSL failed: %v", err)
	}

	if session.GetDSL() != dsl {
		t.Errorf("Expected DSL '%s', got '%s'", dsl, session.GetDSL())
	}
}

func TestSession_AccumulateDSL_Multiple(t *testing.T) {
	session := NewSession("test", "onboarding")

	dsl1 := "(case.create (cbu.id \"CBU-1234\"))"
	dsl2 := "(products.add \"CUSTODY\")"
	dsl3 := "(kyc.start)"

	session.AccumulateDSL(dsl1)
	session.AccumulateDSL(dsl2)
	session.AccumulateDSL(dsl3)

	result := session.GetDSL()

	// All three should be present
	if !strings.Contains(result, dsl1) {
		t.Error("Expected first DSL in accumulated result")
	}
	if !strings.Contains(result, dsl2) {
		t.Error("Expected second DSL in accumulated result")
	}
	if !strings.Contains(result, dsl3) {
		t.Error("Expected third DSL in accumulated result")
	}

	// Should be separated by double newlines
	if !strings.Contains(result, "\n\n") {
		t.Error("Expected DSL blocks separated by double newlines")
	}
}

func TestSession_AccumulateDSL_EmptyString(t *testing.T) {
	session := NewSession("test", "onboarding")

	session.AccumulateDSL("(case.create)")

	// Accumulate empty string should not change DSL
	err := session.AccumulateDSL("")
	if err != nil {
		t.Fatalf("AccumulateDSL with empty string failed: %v", err)
	}

	if session.GetDSL() != "(case.create)" {
		t.Error("Empty accumulation should not modify DSL")
	}
}

func TestManager_AccumulateDSL_OnboardingOnly(t *testing.T) {
	mgr := NewManager()
	session := mgr.GetOrCreate("test", "onboarding")

	// Simulate onboarding workflow
	mgr.AccumulateDSL("test", "(case.create (cbu.id \"CBU-1234\"))")
	mgr.AccumulateDSL("test", "(products.add \"CUSTODY\" \"FUND_ACCOUNTING\")")
	mgr.AccumulateDSL("test", "(kyc.start)")

	dsl := session.GetDSL()

	if !strings.Contains(dsl, "case.create") {
		t.Error("Expected case.create in accumulated DSL")
	}
	if !strings.Contains(dsl, "products.add") {
		t.Error("Expected products.add in accumulated DSL")
	}
	if !strings.Contains(dsl, "kyc.start") {
		t.Error("Expected kyc.start in accumulated DSL")
	}
}

func TestManager_AccumulateDSL_HedgeFundOnly(t *testing.T) {
	mgr := NewManager()
	session := mgr.GetOrCreate("test", "hedge-fund-investor")

	// Simulate hedge fund workflow
	mgr.AccumulateDSL("test", "(investor.start-opportunity (legal-name \"Acme Corp\"))")
	mgr.AccumulateDSL("test", "(kyc.begin (investor \"uuid-123\"))")
	mgr.AccumulateDSL("test", "(subscription.submit (amount 100000))")

	dsl := session.GetDSL()

	if !strings.Contains(dsl, "investor.start-opportunity") {
		t.Error("Expected investor.start-opportunity in accumulated DSL")
	}
	if !strings.Contains(dsl, "kyc.begin") {
		t.Error("Expected kyc.begin in accumulated DSL")
	}
	if !strings.Contains(dsl, "subscription.submit") {
		t.Error("Expected subscription.submit in accumulated DSL")
	}
}

func TestManager_AccumulateDSL_CrossDomain(t *testing.T) {
	mgr := NewManager()
	session := mgr.GetOrCreate("test", "onboarding")

	// Start with onboarding
	mgr.AccumulateDSL("test", "(case.create (cbu.id \"CBU-1234\"))")

	// Switch to hedge fund domain
	mgr.SwitchDomain("test", "hedge-fund-investor")
	mgr.AccumulateDSL("test", "(investor.start-opportunity (legal-name \"Acme Corp\"))")

	// Switch back to onboarding
	mgr.SwitchDomain("test", "onboarding")
	mgr.AccumulateDSL("test", "(products.add \"CUSTODY\")")

	dsl := session.GetDSL()

	// All DSL should be accumulated regardless of domain switches
	if !strings.Contains(dsl, "case.create") {
		t.Error("Expected onboarding DSL in accumulated result")
	}
	if !strings.Contains(dsl, "investor.start-opportunity") {
		t.Error("Expected hedge fund DSL in accumulated result")
	}
	if !strings.Contains(dsl, "products.add") {
		t.Error("Expected onboarding DSL (after switch) in accumulated result")
	}

	// Verify order is preserved
	caseIdx := strings.Index(dsl, "case.create")
	investorIdx := strings.Index(dsl, "investor.start-opportunity")
	productsIdx := strings.Index(dsl, "products.add")

	if !(caseIdx < investorIdx && investorIdx < productsIdx) {
		t.Error("Expected DSL to be accumulated in chronological order")
	}
}

// =============================================================================
// Context Management Tests
// =============================================================================

func TestSession_UpdateContext_InvestorID(t *testing.T) {
	session := NewSession("test", "hedge-fund-investor")

	updates := map[string]interface{}{
		"investor_id": "uuid-investor-123",
	}

	err := session.UpdateContext(updates)
	if err != nil {
		t.Fatalf("UpdateContext failed: %v", err)
	}

	ctx := session.GetContext()
	if ctx.InvestorID != "uuid-investor-123" {
		t.Errorf("Expected InvestorID 'uuid-investor-123', got '%s'", ctx.InvestorID)
	}
}

func TestSession_UpdateContext_MultipleEntities(t *testing.T) {
	session := NewSession("test", "hedge-fund-investor")

	updates := map[string]interface{}{
		"investor_id":   "uuid-investor",
		"fund_id":       "uuid-fund",
		"class_id":      "uuid-class",
		"investor_name": "Acme Capital LP",
		"investor_type": "CORPORATE",
		"domicile":      "CH",
	}

	session.UpdateContext(updates)

	ctx := session.GetContext()

	if ctx.InvestorID != "uuid-investor" {
		t.Error("InvestorID not updated")
	}
	if ctx.FundID != "uuid-fund" {
		t.Error("FundID not updated")
	}
	if ctx.ClassID != "uuid-class" {
		t.Error("ClassID not updated")
	}
	if ctx.InvestorName != "Acme Capital LP" {
		t.Error("InvestorName not updated")
	}
	if ctx.InvestorType != "CORPORATE" {
		t.Error("InvestorType not updated")
	}
	if ctx.Domicile != "CH" {
		t.Error("Domicile not updated")
	}
}

func TestSession_UpdateContext_CBUID(t *testing.T) {
	session := NewSession("test", "onboarding")

	updates := map[string]interface{}{
		"cbu_id": "CBU-1234",
	}

	session.UpdateContext(updates)

	ctx := session.GetContext()
	if ctx.CBUID != "CBU-1234" {
		t.Errorf("Expected CBUID 'CBU-1234', got '%s'", ctx.CBUID)
	}
}

func TestSession_UpdateContext_CurrentState(t *testing.T) {
	session := NewSession("test", "onboarding")

	updates := map[string]interface{}{
		"current_state": "KYC_PENDING",
	}

	session.UpdateContext(updates)

	ctx := session.GetContext()
	if ctx.CurrentState != "KYC_PENDING" {
		t.Errorf("Expected CurrentState 'KYC_PENDING', got '%s'", ctx.CurrentState)
	}
}

func TestSession_UpdateContext_CustomData(t *testing.T) {
	session := NewSession("test", "onboarding")

	updates := map[string]interface{}{
		"custom_field_1": "value1",
		"custom_field_2": 12345,
		"custom_field_3": true,
	}

	session.UpdateContext(updates)

	ctx := session.GetContext()

	if val, ok := ctx.Data["custom_field_1"]; !ok || val != "value1" {
		t.Error("custom_field_1 not stored correctly")
	}
	if val, ok := ctx.Data["custom_field_2"]; !ok || val != 12345 {
		t.Error("custom_field_2 not stored correctly")
	}
	if val, ok := ctx.Data["custom_field_3"]; !ok || val != true {
		t.Error("custom_field_3 not stored correctly")
	}
}

func TestManager_UpdateContext(t *testing.T) {
	mgr := NewManager()
	mgr.GetOrCreate("test", "onboarding")

	updates := map[string]interface{}{
		"cbu_id":        "CBU-1234",
		"current_state": "CREATED",
	}

	err := mgr.UpdateContext("test", updates)
	if err != nil {
		t.Fatalf("UpdateContext failed: %v", err)
	}

	session, _ := mgr.Get("test")
	ctx := session.GetContext()

	if ctx.CBUID != "CBU-1234" {
		t.Error("CBUID not updated via manager")
	}
	if ctx.CurrentState != "CREATED" {
		t.Error("CurrentState not updated via manager")
	}
}

func TestContext_GetSetContextValue(t *testing.T) {
	ctx := Context{
		Data: make(map[string]interface{}),
	}

	ctx.SetContextValue("test_key", "test_value")

	val, exists := ctx.GetContextValue("test_key")
	if !exists {
		t.Error("Expected key to exist")
	}
	if val != "test_value" {
		t.Errorf("Expected 'test_value', got '%v'", val)
	}

	_, exists = ctx.GetContextValue("nonexistent")
	if exists {
		t.Error("Expected nonexistent key to not exist")
	}
}

// =============================================================================
// Message History Tests
// =============================================================================

func TestSession_AddMessage(t *testing.T) {
	session := NewSession("test", "onboarding")

	session.AddMessage("user", "Create case CBU-1234", "", nil)

	history := session.GetHistory()
	if len(history) != 1 {
		t.Fatalf("Expected 1 message, got %d", len(history))
	}

	msg := history[0]
	if msg.Role != "user" {
		t.Errorf("Expected role 'user', got '%s'", msg.Role)
	}
	if msg.Content != "Create case CBU-1234" {
		t.Errorf("Expected content 'Create case CBU-1234', got '%s'", msg.Content)
	}
}

func TestSession_AddMessage_WithDSL(t *testing.T) {
	session := NewSession("test", "onboarding")

	dsl := "(case.create (cbu.id \"CBU-1234\"))"
	metadata := map[string]interface{}{
		"verb":  "case.create",
		"state": "CREATED",
	}

	session.AddMessage("agent", "Created case", dsl, metadata)

	history := session.GetHistory()
	msg := history[0]

	if msg.DSL != dsl {
		t.Errorf("Expected DSL '%s', got '%s'", dsl, msg.DSL)
	}
	if msg.Metadata["verb"] != "case.create" {
		t.Error("Expected metadata to be preserved")
	}
}

func TestSession_AddMessage_Multiple(t *testing.T) {
	session := NewSession("test", "onboarding")

	session.AddMessage("user", "Message 1", "", nil)
	session.AddMessage("agent", "Response 1", "", nil)
	session.AddMessage("user", "Message 2", "", nil)
	session.AddMessage("agent", "Response 2", "", nil)

	history := session.GetHistory()
	if len(history) != 4 {
		t.Fatalf("Expected 4 messages, got %d", len(history))
	}

	// Verify order
	if history[0].Content != "Message 1" {
		t.Error("Message order not preserved")
	}
	if history[3].Content != "Response 2" {
		t.Error("Message order not preserved")
	}
}

// =============================================================================
// Domain Switching Tests
// =============================================================================

func TestManager_SwitchDomain(t *testing.T) {
	mgr := NewManager()
	session := mgr.GetOrCreate("test", "onboarding")

	if session.Domain != "onboarding" {
		t.Fatalf("Initial domain should be 'onboarding'")
	}

	err := mgr.SwitchDomain("test", "hedge-fund-investor")
	if err != nil {
		t.Fatalf("SwitchDomain failed: %v", err)
	}

	// Get session again to verify
	session, _ = mgr.Get("test")
	if session.Domain != "hedge-fund-investor" {
		t.Errorf("Expected domain 'hedge-fund-investor', got '%s'", session.Domain)
	}
}

func TestManager_SwitchDomain_NotFound(t *testing.T) {
	mgr := NewManager()

	err := mgr.SwitchDomain("nonexistent", "onboarding")
	if err == nil {
		t.Error("Expected error when switching domain for nonexistent session")
	}
}

// =============================================================================
// Session Reset Tests
// =============================================================================

func TestSession_Reset(t *testing.T) {
	session := NewSession("test", "onboarding")

	// Build up state
	session.AccumulateDSL("(case.create)")
	session.UpdateContext(map[string]interface{}{
		"cbu_id": "CBU-1234",
	})
	session.AddMessage("user", "Test message", "", nil)

	// Reset
	session.Reset()

	// Verify everything is cleared
	if session.GetDSL() != "" {
		t.Error("Expected DSL to be cleared after reset")
	}

	ctx := session.GetContext()
	if ctx.CBUID != "" {
		t.Error("Expected context to be cleared after reset")
	}

	history := session.GetHistory()
	if len(history) != 0 {
		t.Error("Expected history to be cleared after reset")
	}
}

// =============================================================================
// Session Cleanup Tests
// =============================================================================

func TestManager_CleanupExpired(t *testing.T) {
	mgr := NewManager()

	// Create 3 sessions
	session1 := mgr.GetOrCreate("test-1", "onboarding")
	session2 := mgr.GetOrCreate("test-2", "onboarding")
	session3 := mgr.GetOrCreate("test-3", "onboarding")

	// Manually set LastUsed times
	session1.LastUsed = time.Now().Add(-2 * time.Hour)
	session2.LastUsed = time.Now().Add(-30 * time.Minute)
	session3.LastUsed = time.Now()

	// Cleanup sessions older than 1 hour
	removed := mgr.CleanupExpired(1 * time.Hour)

	if removed != 1 {
		t.Errorf("Expected 1 session removed, got %d", removed)
	}

	if mgr.Count() != 2 {
		t.Errorf("Expected 2 sessions remaining, got %d", mgr.Count())
	}

	// Verify test-1 was removed
	_, err := mgr.Get("test-1")
	if err == nil {
		t.Error("Expected test-1 to be removed")
	}

	// Verify test-2 and test-3 remain
	_, err = mgr.Get("test-2")
	if err != nil {
		t.Error("Expected test-2 to remain")
	}
	_, err = mgr.Get("test-3")
	if err != nil {
		t.Error("Expected test-3 to remain")
	}
}

// =============================================================================
// Concurrency Tests
// =============================================================================

func TestManager_ConcurrentAccess(t *testing.T) {
	mgr := NewManager()

	const numGoroutines = 100
	var wg sync.WaitGroup
	wg.Add(numGoroutines)

	// Multiple goroutines creating/accessing sessions
	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			defer wg.Done()
			sessionID := "concurrent-test"
			mgr.GetOrCreate(sessionID, "onboarding")
		}(i)
	}

	wg.Wait()

	// Should only have 1 session (all goroutines accessed the same one)
	if mgr.Count() != 1 {
		t.Errorf("Expected 1 session after concurrent access, got %d", mgr.Count())
	}
}

func TestSession_ConcurrentDSLAccumulation(t *testing.T) {
	session := NewSession("test", "onboarding")

	const numGoroutines = 50
	var wg sync.WaitGroup
	wg.Add(numGoroutines)

	// Multiple goroutines accumulating DSL
	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			defer wg.Done()
			dsl := "(test.operation)"
			session.AccumulateDSL(dsl)
		}(i)
	}

	wg.Wait()

	// All DSL should be accumulated
	result := session.GetDSL()
	count := strings.Count(result, "(test.operation)")

	if count != numGoroutines {
		t.Errorf("Expected %d operations, got %d", numGoroutines, count)
	}
}

func TestSession_ConcurrentContextUpdates(t *testing.T) {
	session := NewSession("test", "onboarding")

	const numGoroutines = 50
	var wg sync.WaitGroup
	wg.Add(numGoroutines)

	// Multiple goroutines updating context
	for i := 0; i < numGoroutines; i++ {
		go func() {
			defer wg.Done()
			updates := map[string]interface{}{
				"current_state": "STATE",
			}
			session.UpdateContext(updates)
		}()
	}

	wg.Wait()

	// Context should be set (last write wins)
	ctx := session.GetContext()
	if ctx.CurrentState != "STATE" {
		t.Error("Expected CurrentState to be set")
	}
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

func TestManager_UpdateContext_SessionNotFound(t *testing.T) {
	mgr := NewManager()

	err := mgr.UpdateContext("nonexistent", map[string]interface{}{})
	if err == nil {
		t.Error("Expected error when updating context for nonexistent session")
	}
}

func TestManager_AccumulateDSL_SessionNotFound(t *testing.T) {
	mgr := NewManager()

	err := mgr.AccumulateDSL("nonexistent", "(test)")
	if err == nil {
		t.Error("Expected error when accumulating DSL for nonexistent session")
	}
}

func TestSession_GetContext_ImmutableCopy(t *testing.T) {
	session := NewSession("test", "onboarding")

	session.UpdateContext(map[string]interface{}{
		"cbu_id": "CBU-1234",
	})

	// Get context copy
	ctx1 := session.GetContext()
	ctx1.CBUID = "MODIFIED"

	// Get context again - should be unchanged
	ctx2 := session.GetContext()
	if ctx2.CBUID != "CBU-1234" {
		t.Error("Context modifications should not affect session")
	}
}

func TestSession_GetHistory_ImmutableCopy(t *testing.T) {
	session := NewSession("test", "onboarding")

	session.AddMessage("user", "Original", "", nil)

	// Get history copy
	history1 := session.GetHistory()
	history1[0].Content = "MODIFIED"

	// Get history again - should be unchanged
	history2 := session.GetHistory()
	if history2[0].Content != "Original" {
		t.Error("History modifications should not affect session")
	}
}
