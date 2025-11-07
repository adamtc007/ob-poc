package store

import (
	"context"
	"regexp"
	"testing"
	"time"

	sqlmock "github.com/DATA-DOG/go-sqlmock"
)

func TestGetDSLHistory_ReturnsOrderedResults(t *testing.T) {
	db, mock, err := sqlmock.New()
	if err != nil {
		t.Fatalf("failed to create sqlmock: %v", err)
	}
	defer db.Close()

	s := &Store{db: db}

	cbu := "CBU-1234"
	t1 := time.Now().Add(-2 * time.Minute).Truncate(time.Second)
	t2 := t1.Add(1 * time.Minute)

	rows := sqlmock.NewRows([]string{"version_id", "created_at", "dsl_text"}).
		AddRow("11111111-1111-1111-1111-111111111111", t1, "(dsl version 1)").
		AddRow("22222222-2222-2222-2222-222222222222", t2, "(dsl version 2)")

	query := regexp.QuoteMeta(`SELECT version_id::text, created_at, dsl_text
         FROM "dsl-ob-poc".dsl_ob
         WHERE cbu_id = $1
         ORDER BY created_at ASC`)
	mock.ExpectQuery(query).WithArgs(cbu).WillReturnRows(rows)

	history, err := s.GetDSLHistory(context.Background(), cbu)
	if err != nil {
		t.Fatalf("GetDSLHistory returned error: %v", err)
	}
	if len(history) != 2 {
		t.Fatalf("expected 2 versions, got %d", len(history))
	}
	if history[0].VersionID != "11111111-1111-1111-1111-111111111111" {
		t.Errorf("unexpected first version id: %s", history[0].VersionID)
	}
	if history[1].VersionID != "22222222-2222-2222-2222-222222222222" {
		t.Errorf("unexpected second version id: %s", history[1].VersionID)
	}

	if mockErr := mock.ExpectationsWereMet(); mockErr != nil {
		t.Fatalf("unmet sqlmock expectations: %v", mockErr)
	}
}
