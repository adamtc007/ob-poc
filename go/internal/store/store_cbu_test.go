package store

import (
	"context"
	"testing"

	"github.com/DATA-DOG/go-sqlmock"
)

func TestCreateCBU(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	name := "Test CBU"
	description := "Test CBU Description"
	naturePurpose := "Test purpose"
	expectedID := "123e4567-e89b-12d3-a456-426614174000"

	mock.ExpectQuery(`INSERT INTO "dsl-ob-poc".cbus \(name, description, nature_purpose\) VALUES \(\$1, \$2, \$3\) RETURNING cbu_id`).
		WithArgs(name, description, naturePurpose).
		WillReturnRows(sqlmock.NewRows([]string{"cbu_id"}).AddRow(expectedID))

	cbuID, err := store.CreateCBU(ctx, name, description, naturePurpose)
	if err != nil {
		t.Fatalf("CreateCBU failed: %v", err)
	}

	if cbuID != expectedID {
		t.Errorf("Expected CBU ID %s, got %s", expectedID, cbuID)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestListCBUs(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	rows := sqlmock.NewRows([]string{
		"cbu_id", "name", "description", "nature_purpose",
	}).AddRow(
		"123e4567-e89b-12d3-a456-426614174000",
		"CBU-1",
		"First CBU",
		"Test purpose 1",
	).AddRow(
		"987fcdeb-51a2-43f7-8765-ba9876543210",
		"CBU-2",
		"Second CBU",
		"Test purpose 2",
	)

	mock.ExpectQuery(`SELECT cbu_id, name, description, nature_purpose FROM "dsl-ob-poc".cbus ORDER BY name`).
		WillReturnRows(rows)

	cbus, err := store.ListCBUs(ctx)
	if err != nil {
		t.Fatalf("ListCBUs failed: %v", err)
	}

	if len(cbus) != 2 {
		t.Fatalf("Expected 2 CBUs, got %d", len(cbus))
	}

	if cbus[0].CBUID != "123e4567-e89b-12d3-a456-426614174000" {
		t.Errorf("Expected first CBU ID '123e4567-e89b-12d3-a456-426614174000', got '%s'", cbus[0].CBUID)
	}

	if cbus[0].Name != "CBU-1" {
		t.Errorf("Expected first CBU name 'CBU-1', got '%s'", cbus[0].Name)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestGetCBUByID(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	cbuID := "123e4567-e89b-12d3-a456-426614174000"
	expectedCBU := CBU{
		CBUID:         cbuID,
		Name:          "Test CBU",
		Description:   "Test Description",
		NaturePurpose: "Test Purpose",
	}

	rows := sqlmock.NewRows([]string{
		"cbu_id", "name", "description", "nature_purpose",
	}).AddRow(
		expectedCBU.CBUID,
		expectedCBU.Name,
		expectedCBU.Description,
		expectedCBU.NaturePurpose,
	)

	mock.ExpectQuery(`SELECT cbu_id, name, description, nature_purpose FROM "dsl-ob-poc".cbus WHERE cbu_id = \$1`).
		WithArgs(cbuID).
		WillReturnRows(rows)

	cbu, err := store.GetCBUByID(ctx, cbuID)
	if err != nil {
		t.Fatalf("GetCBUByID failed: %v", err)
	}

	if cbu.CBUID != expectedCBU.CBUID {
		t.Errorf("Expected CBU ID '%s', got '%s'", expectedCBU.CBUID, cbu.CBUID)
	}

	if cbu.Name != expectedCBU.Name {
		t.Errorf("Expected CBU name '%s', got '%s'", expectedCBU.Name, cbu.Name)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestUpdateCBU(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	cbuID := "123e4567-e89b-12d3-a456-426614174000"
	name := "Updated CBU"
	description := "Updated Description"
	naturePurpose := "Updated Purpose"

	mock.ExpectExec(`UPDATE "dsl-ob-poc".cbus SET name = \$1, description = \$2, nature_purpose = \$3, updated_at = \$4 WHERE cbu_id = \$5`).
		WithArgs(name, description, naturePurpose, sqlmock.AnyArg(), cbuID).
		WillReturnResult(sqlmock.NewResult(0, 1))

	err = store.UpdateCBU(ctx, cbuID, name, description, naturePurpose)
	if err != nil {
		t.Fatalf("UpdateCBU failed: %v", err)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestDeleteCBU(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	cbuID := "123e4567-e89b-12d3-a456-426614174000"

	mock.ExpectExec(`DELETE FROM "dsl-ob-poc".cbus WHERE cbu_id = \$1`).
		WithArgs(cbuID).
		WillReturnResult(sqlmock.NewResult(0, 1))

	err = store.DeleteCBU(ctx, cbuID)
	if err != nil {
		t.Fatalf("DeleteCBU failed: %v", err)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}
