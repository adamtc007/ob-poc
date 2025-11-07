package store

import (
	"context"
	"testing"

	"github.com/DATA-DOG/go-sqlmock"
)

func TestCreateRole(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	name := "Investment Manager"
	description := "Manages investment strategies"
	expectedID := "123e4567-e89b-12d3-a456-426614174000"

	mock.ExpectQuery(`INSERT INTO "dsl-ob-poc".roles \(name, description\) VALUES \(\$1, \$2\) RETURNING role_id`).
		WithArgs(name, description).
		WillReturnRows(sqlmock.NewRows([]string{"role_id"}).AddRow(expectedID))

	roleID, err := store.CreateRole(ctx, name, description)
	if err != nil {
		t.Fatalf("CreateRole failed: %v", err)
	}

	if roleID != expectedID {
		t.Errorf("Expected role ID %s, got %s", expectedID, roleID)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestListRoles(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	rows := sqlmock.NewRows([]string{
		"role_id", "name", "description",
	}).AddRow(
		"123e4567-e89b-12d3-a456-426614174000",
		"Investment Manager",
		"Manages investment strategies",
	).AddRow(
		"987fcdeb-51a2-43f7-8765-ba9876543210",
		"Asset Owner",
		"Owns the assets being managed",
	)

	mock.ExpectQuery(`SELECT role_id, name, description FROM "dsl-ob-poc".roles ORDER BY name`).
		WillReturnRows(rows)

	roles, err := store.ListRoles(ctx)
	if err != nil {
		t.Fatalf("ListRoles failed: %v", err)
	}

	if len(roles) != 2 {
		t.Fatalf("Expected 2 roles, got %d", len(roles))
	}

	if roles[0].RoleID != "123e4567-e89b-12d3-a456-426614174000" {
		t.Errorf("Expected first role ID '123e4567-e89b-12d3-a456-426614174000', got '%s'", roles[0].RoleID)
	}

	if roles[0].Name != "Investment Manager" {
		t.Errorf("Expected first role name 'Investment Manager', got '%s'", roles[0].Name)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestGetRoleByID(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	roleID := "123e4567-e89b-12d3-a456-426614174000"
	expectedRole := Role{
		RoleID:      roleID,
		Name:        "Investment Manager",
		Description: "Manages investment strategies",
	}

	rows := sqlmock.NewRows([]string{
		"role_id", "name", "description",
	}).AddRow(
		expectedRole.RoleID,
		expectedRole.Name,
		expectedRole.Description,
	)

	mock.ExpectQuery(`SELECT role_id, name, description FROM "dsl-ob-poc".roles WHERE role_id = \$1`).
		WithArgs(roleID).
		WillReturnRows(rows)

	role, err := store.GetRoleByID(ctx, roleID)
	if err != nil {
		t.Fatalf("GetRoleByID failed: %v", err)
	}

	if role.RoleID != expectedRole.RoleID {
		t.Errorf("Expected role ID '%s', got '%s'", expectedRole.RoleID, role.RoleID)
	}

	if role.Name != expectedRole.Name {
		t.Errorf("Expected role name '%s', got '%s'", expectedRole.Name, role.Name)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestUpdateRole(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	roleID := "123e4567-e89b-12d3-a456-426614174000"
	name := "Updated Role"
	description := "Updated Description"

	mock.ExpectExec(`UPDATE "dsl-ob-poc".roles SET name = \$1, description = \$2, updated_at = \$3 WHERE role_id = \$4`).
		WithArgs(name, description, sqlmock.AnyArg(), roleID).
		WillReturnResult(sqlmock.NewResult(0, 1))

	err = store.UpdateRole(ctx, roleID, name, description)
	if err != nil {
		t.Fatalf("UpdateRole failed: %v", err)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}

func TestDeleteRole(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("Failed to create mock DB: %v", err)
	}
	defer db.Close()

	store := &Store{db: db}
	ctx := context.Background()

	roleID := "123e4567-e89b-12d3-a456-426614174000"

	mock.ExpectExec(`DELETE FROM "dsl-ob-poc".roles WHERE role_id = \$1`).
		WithArgs(roleID).
		WillReturnResult(sqlmock.NewResult(0, 1))

	err = store.DeleteRole(ctx, roleID)
	if err != nil {
		t.Fatalf("DeleteRole failed: %v", err)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("There were unfulfilled expectations: %s", err)
	}
}
