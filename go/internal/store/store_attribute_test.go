package store

import (
	"context"
	"encoding/json"
	"testing"

	"github.com/DATA-DOG/go-sqlmock"
)

func TestGetDictionaryAttributeByName(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	// Mock the database response
	rows := sqlmock.NewRows([]string{
		"attribute_id", "name", "long_description", "group_id", "mask", "domain", "vector", "source", "sink",
	}).AddRow(
		"123e4567-e89b-12d3-a456-426614174000",
		"onboard.cbu_id",
		"Client Business Unit identifier",
		"Onboarding",
		"string",
		"Onboarding",
		"",
		`{"type": "manual", "required": true}`,
		`{"type": "database", "table": "cbus"}`,
	)

	mock.ExpectQuery(`SELECT attribute_id, name, long_description, group_id, mask, domain,.*FROM "dsl-ob-poc".dictionary WHERE name = \$1`).
		WithArgs("onboard.cbu_id").
		WillReturnRows(rows)

	// Execute the function
	attr, err := store.GetDictionaryAttributeByName(ctx, "onboard.cbu_id")
	if err != nil {
		t.Fatalf("GetDictionaryAttributeByName failed: %v", err)
	}

	// Verify the result
	if attr.AttributeID != "123e4567-e89b-12d3-a456-426614174000" {
		t.Errorf("Expected AttributeID '123e4567-e89b-12d3-a456-426614174000', got '%s'", attr.AttributeID)
	}

	if attr.Name != "onboard.cbu_id" {
		t.Errorf("Expected Name 'onboard.cbu_id', got '%s'", attr.Name)
	}

	if attr.GroupID != "Onboarding" {
		t.Errorf("Expected GroupID 'Onboarding', got '%s'", attr.GroupID)
	}

	// Verify all expectations were met
	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestGetDictionaryAttributeByID(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	// Mock the database response
	rows := sqlmock.NewRows([]string{
		"attribute_id", "name", "long_description", "group_id", "mask", "domain", "vector", "source", "sink",
	}).AddRow(
		"123e4567-e89b-12d3-a456-426614174000",
		"onboard.cbu_id",
		"Client Business Unit identifier",
		"Onboarding",
		"string",
		"Onboarding",
		"",
		`{"type": "manual", "required": true}`,
		`{"type": "database", "table": "cbus"}`,
	)

	mock.ExpectQuery(`SELECT attribute_id, name, long_description, group_id, mask, domain,.*FROM "dsl-ob-poc".dictionary WHERE attribute_id = \$1`).
		WithArgs("123e4567-e89b-12d3-a456-426614174000").
		WillReturnRows(rows)

	// Execute the function
	attr, err := store.GetDictionaryAttributeByID(ctx, "123e4567-e89b-12d3-a456-426614174000")
	if err != nil {
		t.Fatalf("GetDictionaryAttributeByID failed: %v", err)
	}

	// Verify the result
	if attr.AttributeID != "123e4567-e89b-12d3-a456-426614174000" {
		t.Errorf("Expected AttributeID '123e4567-e89b-12d3-a456-426614174000', got '%s'", attr.AttributeID)
	}

	if attr.Name != "onboard.cbu_id" {
		t.Errorf("Expected Name 'onboard.cbu_id', got '%s'", attr.Name)
	}

	// Verify all expectations were met
	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestGetCBUByName(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	// Mock the database response
	rows := sqlmock.NewRows([]string{
		"cbu_id", "name", "description", "nature_purpose",
	}).AddRow(
		"987fcdeb-51a2-43f7-8765-ba9876543210",
		"CBU-1234",
		"Aviva Investors Global Fund",
		"UCITS equity fund domiciled in LU",
	)

	mock.ExpectQuery(`SELECT cbu_id, name, description, nature_purpose FROM "dsl-ob-poc".cbus WHERE name = \$1`).
		WithArgs("CBU-1234").
		WillReturnRows(rows)

	// Execute the function
	cbu, err := store.GetCBUByName(ctx, "CBU-1234")
	if err != nil {
		t.Fatalf("GetCBUByName failed: %v", err)
	}

	// Verify the result
	if cbu.CBUID != "987fcdeb-51a2-43f7-8765-ba9876543210" {
		t.Errorf("Expected CBUID '987fcdeb-51a2-43f7-8765-ba9876543210', got '%s'", cbu.CBUID)
	}

	if cbu.Name != "CBU-1234" {
		t.Errorf("Expected Name 'CBU-1234', got '%s'", cbu.Name)
	}

	if cbu.Description != "Aviva Investors Global Fund" {
		t.Errorf("Expected Description 'Aviva Investors Global Fund', got '%s'", cbu.Description)
	}

	// Verify all expectations were met
	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestUpsertAttributeValue(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	// Test data
	cbuID := "CBU-1234"
	dslVersion := 1
	attributeID := "123e4567-e89b-12d3-a456-426614174000"
	value := json.RawMessage(`"test-value"`)
	state := "resolved"
	source := map[string]any{"type": "manual", "required": true}

	// Mock the database call
	mock.ExpectExec(`INSERT INTO "dsl-ob-poc".attribute_values.*ON CONFLICT.*DO UPDATE SET`).
		WithArgs(cbuID, dslVersion, attributeID, value, state, sqlmock.AnyArg()).
		WillReturnResult(sqlmock.NewResult(1, 1))

	// Execute the function
	err = store.UpsertAttributeValue(ctx, cbuID, dslVersion, attributeID, value, state, source)
	if err != nil {
		t.Fatalf("UpsertAttributeValue failed: %v", err)
	}

	// Verify all expectations were met
	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestResolveValueFor_NoResolver(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	// Mock get attribute by ID - returns manual source (no table resolver)
	attrRows := sqlmock.NewRows([]string{
		"attribute_id", "name", "long_description", "group_id", "mask", "domain", "vector", "source", "sink",
	}).AddRow(
		"123e4567-e89b-12d3-a456-426614174000",
		"onboard.cbu_id",
		"Client Business Unit identifier",
		"Onboarding",
		"string",
		"Onboarding",
		"",
		`{"type": "manual", "required": true, "format": "CBU-[0-9]+"}`,
		`{"type": "database", "table": "onboarding_cases"}`,
	)

	mock.ExpectQuery(`SELECT attribute_id, name, long_description, group_id, mask, domain,.*FROM "dsl-ob-poc".dictionary WHERE attribute_id = \$1`).
		WithArgs("123e4567-e89b-12d3-a456-426614174000").
		WillReturnRows(attrRows)

	// Execute the function
	value, prov, state, err := store.ResolveValueFor(ctx, "CBU-1234", "123e4567-e89b-12d3-a456-426614174000")
	if err != nil {
		t.Fatalf("ResolveValueFor failed: %v", err)
	}

	// Verify the result - should be pending since no resolver
	if state != "pending" {
		t.Errorf("Expected state 'pending', got '%s'", state)
	}

	if string(value) != "null" {
		t.Errorf("Expected value 'null', got '%s'", string(value))
	}

	// Check provenance indicates no resolver
	if prov["reason"] != "no_resolver" {
		t.Errorf("Expected provenance reason 'no_resolver', got '%v'", prov["reason"])
	}

	// Verify all expectations were met
	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}
