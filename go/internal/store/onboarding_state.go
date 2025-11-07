package store

import (
	"context"
	"database/sql"
	"fmt"
	"time"
)

// OnboardingState represents the different stages of onboarding progression
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

// OnboardingSession represents an active onboarding session
type OnboardingSession struct {
	OnboardingID       string          `json:"onboarding_id"`
	CBUID              string          `json:"cbu_id"`
	CurrentState       OnboardingState `json:"current_state"`
	CurrentVersion     int             `json:"current_version"`
	LatestDSLVersionID *string         `json:"latest_dsl_version_id,omitempty"`
	CreatedAt          time.Time       `json:"created_at"`
	UpdatedAt          time.Time       `json:"updated_at"`
}

// DSLVersionWithState represents a DSL version with state information
type DSLVersionWithState struct {
	VersionID       string          `json:"version_id"`
	CBUID           string          `json:"cbu_id"`
	DSLText         string          `json:"dsl_text"`
	OnboardingState OnboardingState `json:"onboarding_state"`
	VersionNumber   int             `json:"version_number"`
	CreatedAt       time.Time       `json:"created_at"`
}

// CreateOnboardingSession creates a new onboarding session for a CBU
func (s *Store) CreateOnboardingSession(ctx context.Context, cbuID string) (*OnboardingSession, error) {
	var session OnboardingSession
	err := s.db.QueryRowContext(ctx, `
		INSERT INTO "dsl-ob-poc".onboarding_sessions (cbu_id, current_state, current_version)
		VALUES ($1, $2, $3)
		RETURNING onboarding_id, cbu_id, current_state, current_version, latest_dsl_version_id, created_at, updated_at`,
		cbuID, StateCreated, 1).Scan(
		&session.OnboardingID,
		&session.CBUID,
		&session.CurrentState,
		&session.CurrentVersion,
		&session.LatestDSLVersionID,
		&session.CreatedAt,
		&session.UpdatedAt)

	if err != nil {
		return nil, fmt.Errorf("failed to create onboarding session: %w", err)
	}

	return &session, nil
}

// GetOnboardingSession retrieves an onboarding session by CBU ID
func (s *Store) GetOnboardingSession(ctx context.Context, cbuID string) (*OnboardingSession, error) {
	var session OnboardingSession
	err := s.db.QueryRowContext(ctx, `
		SELECT onboarding_id, cbu_id, current_state, current_version, latest_dsl_version_id, created_at, updated_at
		FROM "dsl-ob-poc".onboarding_sessions
		WHERE cbu_id = $1`,
		cbuID).Scan(
		&session.OnboardingID,
		&session.CBUID,
		&session.CurrentState,
		&session.CurrentVersion,
		&session.LatestDSLVersionID,
		&session.CreatedAt,
		&session.UpdatedAt)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("no onboarding session found for CBU: %s", cbuID)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get onboarding session: %w", err)
	}

	return &session, nil
}

// UpdateOnboardingState updates the state and version of an onboarding session
func (s *Store) UpdateOnboardingState(ctx context.Context, cbuID string, newState OnboardingState, dslVersionID string) error {
	result, err := s.db.ExecContext(ctx, `
		UPDATE "dsl-ob-poc".onboarding_sessions
		SET current_state = $1,
		    current_version = current_version + 1,
		    latest_dsl_version_id = $2,
		    updated_at = (now() at time zone 'utc')
		WHERE cbu_id = $3`,
		newState, dslVersionID, cbuID)

	if err != nil {
		return fmt.Errorf("failed to update onboarding state: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to check affected rows: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("no onboarding session found for CBU: %s", cbuID)
	}

	return nil
}

// InsertDSLWithState inserts a new DSL version with state information
func (s *Store) InsertDSLWithState(ctx context.Context, cbuID, dslText string, state OnboardingState) (string, error) {
	var versionID string
	err := s.db.QueryRowContext(ctx, `
		INSERT INTO "dsl-ob-poc".dsl_ob (cbu_id, dsl_text, onboarding_state)
		VALUES ($1, $2, $3)
		RETURNING version_id`,
		cbuID, dslText, state).Scan(&versionID)

	if err != nil {
		return "", fmt.Errorf("failed to insert DSL with state: %w", err)
	}

	return versionID, nil
}

// GetLatestDSLWithState retrieves the most recent DSL with state information
func (s *Store) GetLatestDSLWithState(ctx context.Context, cbuID string) (*DSLVersionWithState, error) {
	var dslVersion DSLVersionWithState
	err := s.db.QueryRowContext(ctx, `
		SELECT version_id, cbu_id, dsl_text, onboarding_state, version_number, created_at
		FROM "dsl-ob-poc".dsl_ob
		WHERE cbu_id = $1
		ORDER BY version_number DESC
		LIMIT 1`,
		cbuID).Scan(
		&dslVersion.VersionID,
		&dslVersion.CBUID,
		&dslVersion.DSLText,
		&dslVersion.OnboardingState,
		&dslVersion.VersionNumber,
		&dslVersion.CreatedAt)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("no DSL found for CBU: %s", cbuID)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get latest DSL with state: %w", err)
	}

	return &dslVersion, nil
}

// GetDSLHistoryWithState returns all DSL versions with state information
func (s *Store) GetDSLHistoryWithState(ctx context.Context, cbuID string) ([]DSLVersionWithState, error) {
	rows, err := s.db.QueryContext(ctx, `
		SELECT version_id, cbu_id, dsl_text, onboarding_state, version_number, created_at
		FROM "dsl-ob-poc".dsl_ob
		WHERE cbu_id = $1
		ORDER BY version_number ASC`,
		cbuID)

	if err != nil {
		return nil, fmt.Errorf("failed to query DSL history with state: %w", err)
	}
	defer rows.Close()

	var history []DSLVersionWithState
	for rows.Next() {
		var version DSLVersionWithState
		if scanErr := rows.Scan(
			&version.VersionID,
			&version.CBUID,
			&version.DSLText,
			&version.OnboardingState,
			&version.VersionNumber,
			&version.CreatedAt); scanErr != nil {
			return nil, fmt.Errorf("failed to scan DSL version: %w", scanErr)
		}
		history = append(history, version)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating DSL history: %w", rowsErr)
	}

	return history, nil
}

// GetDSLByVersion retrieves a specific DSL version by version number
func (s *Store) GetDSLByVersion(ctx context.Context, cbuID string, versionNumber int) (*DSLVersionWithState, error) {
	var dslVersion DSLVersionWithState
	err := s.db.QueryRowContext(ctx, `
		SELECT version_id, cbu_id, dsl_text, onboarding_state, version_number, created_at
		FROM "dsl-ob-poc".dsl_ob
		WHERE cbu_id = $1 AND version_number = $2`,
		cbuID, versionNumber).Scan(
		&dslVersion.VersionID,
		&dslVersion.CBUID,
		&dslVersion.DSLText,
		&dslVersion.OnboardingState,
		&dslVersion.VersionNumber,
		&dslVersion.CreatedAt)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("no DSL version %d found for CBU: %s", versionNumber, cbuID)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get DSL version: %w", err)
	}

	return &dslVersion, nil
}

// ListOnboardingSessions returns all active onboarding sessions
func (s *Store) ListOnboardingSessions(ctx context.Context) ([]OnboardingSession, error) {
	rows, err := s.db.QueryContext(ctx, `
		SELECT onboarding_id, cbu_id, current_state, current_version, latest_dsl_version_id, created_at, updated_at
		FROM "dsl-ob-poc".onboarding_sessions
		ORDER BY updated_at DESC`)

	if err != nil {
		return nil, fmt.Errorf("failed to query onboarding sessions: %w", err)
	}
	defer rows.Close()

	var sessions []OnboardingSession
	for rows.Next() {
		var session OnboardingSession
		if scanErr := rows.Scan(
			&session.OnboardingID,
			&session.CBUID,
			&session.CurrentState,
			&session.CurrentVersion,
			&session.LatestDSLVersionID,
			&session.CreatedAt,
			&session.UpdatedAt); scanErr != nil {
			return nil, fmt.Errorf("failed to scan onboarding session: %w", scanErr)
		}
		sessions = append(sessions, session)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating onboarding sessions: %w", rowsErr)
	}

	return sessions, nil
}
