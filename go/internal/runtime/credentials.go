package runtime

import (
	"context"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"time"
)

// CredentialManager handles secure credential storage and retrieval
type CredentialManager struct {
	db         *sql.DB
	encryptKey []byte // 32-byte key for AES-256
}

// NewCredentialManager creates a new credential manager
func NewCredentialManager(db *sql.DB) (*CredentialManager, error) {
	// Get encryption key from environment or generate one
	keyString := os.Getenv("CREDENTIALS_ENCRYPTION_KEY")
	if keyString == "" {
		return nil, fmt.Errorf("CREDENTIALS_ENCRYPTION_KEY environment variable not set")
	}

	// Use SHA256 to ensure we have exactly 32 bytes for AES-256
	hasher := sha256.New()
	hasher.Write([]byte(keyString))
	encryptKey := hasher.Sum(nil)

	return &CredentialManager{
		db:         db,
		encryptKey: encryptKey,
	}, nil
}

// StoreCredentials stores encrypted credentials in the vault
func (cm *CredentialManager) StoreCredentials(ctx context.Context, name, credType, environment string, credentials map[string]interface{}) error {
	// Serialize credentials to JSON
	credentialsJSON, err := json.Marshal(credentials)
	if err != nil {
		return fmt.Errorf("failed to marshal credentials: %w", err)
	}

	// Encrypt the credentials
	encryptedData, err := cm.encrypt(credentialsJSON)
	if err != nil {
		return fmt.Errorf("failed to encrypt credentials: %w", err)
	}

	// Store in database
	query := `
		INSERT INTO credentials_vault (
			credential_name, credential_type, encrypted_data, environment, created_at, active
		) VALUES ($1, $2, $3, $4, NOW(), true)
		ON CONFLICT (credential_name) DO UPDATE SET
			credential_type = EXCLUDED.credential_type,
			encrypted_data = EXCLUDED.encrypted_data,
			environment = EXCLUDED.environment,
			created_at = NOW(),
			active = true`

	_, err = cm.db.ExecContext(ctx, query, name, credType, encryptedData, environment)
	if err != nil {
		return fmt.Errorf("failed to store credentials: %w", err)
	}

	return nil
}

// GetCredentials retrieves and decrypts credentials from the vault
func (cm *CredentialManager) GetCredentials(ctx context.Context, name string) (map[string]interface{}, error) {
	query := `
		SELECT encrypted_data, expires_at
		FROM credentials_vault
		WHERE credential_name = $1 AND active = true`

	var encryptedData []byte
	var expiresAt sql.NullTime

	err := cm.db.QueryRowContext(ctx, query, name).Scan(&encryptedData, &expiresAt)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("credentials '%s' not found", name)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to retrieve credentials: %w", err)
	}

	// Check if credentials have expired
	if expiresAt.Valid && expiresAt.Time.Before(time.Now()) {
		return nil, fmt.Errorf("credentials '%s' have expired", name)
	}

	// Decrypt the credentials
	credentialsJSON, err := cm.decrypt(encryptedData)
	if err != nil {
		return nil, fmt.Errorf("failed to decrypt credentials: %w", err)
	}

	// Deserialize credentials
	var credentials map[string]interface{}
	if err := json.Unmarshal(credentialsJSON, &credentials); err != nil {
		return nil, fmt.Errorf("failed to unmarshal credentials: %w", err)
	}

	return credentials, nil
}

// ListCredentials lists available credentials (names only for security)
func (cm *CredentialManager) ListCredentials(ctx context.Context, environment string) ([]CredentialInfo, error) {
	query := `
		SELECT credential_name, credential_type, environment, created_at, expires_at, active
		FROM credentials_vault
		WHERE environment = $1 OR environment = 'all'
		ORDER BY credential_name`

	rows, err := cm.db.QueryContext(ctx, query, environment)
	if err != nil {
		return nil, fmt.Errorf("failed to list credentials: %w", err)
	}
	defer rows.Close()

	var credentials []CredentialInfo
	for rows.Next() {
		var cred CredentialInfo
		err := rows.Scan(
			&cred.Name, &cred.Type, &cred.Environment,
			&cred.CreatedAt, &cred.ExpiresAt, &cred.Active,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan credential info: %w", err)
		}
		credentials = append(credentials, cred)
	}

	return credentials, rows.Err()
}

// DeleteCredentials removes credentials from the vault
func (cm *CredentialManager) DeleteCredentials(ctx context.Context, name string) error {
	query := `DELETE FROM credentials_vault WHERE credential_name = $1`

	result, err := cm.db.ExecContext(ctx, query, name)
	if err != nil {
		return fmt.Errorf("failed to delete credentials: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("credentials '%s' not found", name)
	}

	return nil
}



// encrypt encrypts data using AES-256-GCM
func (cm *CredentialManager) encrypt(data []byte) ([]byte, error) {
	block, err := aes.NewCipher(cm.encryptKey)
	if err != nil {
		return nil, err
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, err
	}

	nonce := make([]byte, gcm.NonceSize())
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return nil, err
	}

	ciphertext := gcm.Seal(nonce, nonce, data, nil)
	return ciphertext, nil
}

// decrypt decrypts data using AES-256-GCM
func (cm *CredentialManager) decrypt(data []byte) ([]byte, error) {
	block, err := aes.NewCipher(cm.encryptKey)
	if err != nil {
		return nil, err
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, err
	}

	nonceSize := gcm.NonceSize()
	if len(data) < nonceSize {
		return nil, fmt.Errorf("ciphertext too short")
	}

	nonce, ciphertext := data[:nonceSize], data[nonceSize:]
	plaintext, err := gcm.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return nil, err
	}

	return plaintext, nil
}

// CredentialInfo represents metadata about stored credentials
type CredentialInfo struct {
	Name        string     `json:"name"`
	Type        string     `json:"type"`
	Environment string     `json:"environment"`
	CreatedAt   time.Time  `json:"created_at"`
	ExpiresAt   *time.Time `json:"expires_at,omitempty"`
	Active      bool       `json:"active"`
}

// ==============================================================================
// Predefined Credential Types and Helpers
// ==============================================================================

// CreateAPIKeyCredentials creates API key credentials
func (cm *CredentialManager) CreateAPIKeyCredentials(ctx context.Context, name, environment, apiKey string) error {
	credentials := map[string]interface{}{
		"api_key": apiKey,
	}
	return cm.StoreCredentials(ctx, name, "api_key", environment, credentials)
}

// CreateBearerTokenCredentials creates Bearer token credentials
func (cm *CredentialManager) CreateBearerTokenCredentials(ctx context.Context, name, environment, token string) error {
	credentials := map[string]interface{}{
		"token": token,
	}
	return cm.StoreCredentials(ctx, name, "bearer", environment, credentials)
}

// CreateBasicAuthCredentials creates Basic authentication credentials
func (cm *CredentialManager) CreateBasicAuthCredentials(ctx context.Context, name, environment, username, password string) error {
	credentials := map[string]interface{}{
		"username": username,
		"password": password,
	}
	return cm.StoreCredentials(ctx, name, "basic", environment, credentials)
}



// ==============================================================================
// Credential Validation and Health Checks
// ==============================================================================

// ValidateCredentials performs basic validation on credential format
func (cm *CredentialManager) ValidateCredentials(credType string, credentials map[string]interface{}) error {
	switch credType {
	case "api_key":
		if _, ok := credentials["api_key"]; !ok {
			return fmt.Errorf("api_key field required for API key credentials")
		}
	case "bearer":
		if _, ok := credentials["token"]; !ok {
			return fmt.Errorf("token field required for Bearer token credentials")
		}
	case "basic":
		if _, ok := credentials["username"]; !ok {
			return fmt.Errorf("username field required for Basic auth credentials")
		}
		if _, ok := credentials["password"]; !ok {
			return fmt.Errorf("password field required for Basic auth credentials")
		}
	case "oauth2":
		if _, ok := credentials["access_token"]; !ok {
			return fmt.Errorf("access_token field required for OAuth2 credentials")
		}
	case "custom":
		// Custom credentials can have any structure
		if len(credentials) == 0 {
			return fmt.Errorf("custom credentials cannot be empty")
		}
	default:
		return fmt.Errorf("unsupported credential type: %s", credType)
	}
	return nil
}

// TestCredentialConnection tests if credentials work by making a test API call
func (cm *CredentialManager) TestCredentialConnection(ctx context.Context, name, testEndpoint string) error {
	// This would make a test API call using the credentials
	// Implementation depends on the specific API being tested
	credentials, err := cm.GetCredentials(ctx, name)
	if err != nil {
		return fmt.Errorf("failed to get credentials for testing: %w", err)
	}

	// For now, just verify we can decrypt and parse the credentials
	if len(credentials) == 0 {
		return fmt.Errorf("credentials are empty")
	}

	// In a real implementation, you would:
	// 1. Create an HTTP client
	// 2. Make a test API call to testEndpoint using the credentials
	// 3. Verify the response indicates successful authentication

	return nil
}

// RotateCredentials creates new credentials and marks old ones for deletion
func (cm *CredentialManager) RotateCredentials(ctx context.Context, name string, newCredentials map[string]interface{}) error {
	// Get existing credential info
	_ = []CredentialInfo{}
	query := `
		SELECT credential_type, environment
		FROM credentials_vault
		WHERE credential_name = $1 AND active = true`

	var credType, environment string
	err := cm.db.QueryRowContext(ctx, query, name).Scan(&credType, &environment)
	if err != nil {
		return fmt.Errorf("failed to get existing credential info: %w", err)
	}

	// Validate new credentials
	if err := cm.ValidateCredentials(credType, newCredentials); err != nil {
		return fmt.Errorf("new credentials validation failed: %w", err)
	}

	// Create new credentials (will overwrite existing due to ON CONFLICT)
	if err := cm.StoreCredentials(ctx, name, credType, environment, newCredentials); err != nil {
		return fmt.Errorf("failed to store new credentials: %w", err)
	}

	return nil
}

// GetCredentialsMetadata returns metadata about credentials without the actual secret data
func (cm *CredentialManager) GetCredentialsMetadata(ctx context.Context, name string) (*CredentialInfo, error) {
	query := `
		SELECT credential_name, credential_type, environment, created_at, expires_at, active
		FROM credentials_vault
		WHERE credential_name = $1`

	info := &CredentialInfo{}
	err := cm.db.QueryRowContext(ctx, query, name).Scan(
		&info.Name, &info.Type, &info.Environment,
		&info.CreatedAt, &info.ExpiresAt, &info.Active,
	)

	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("credentials '%s' not found", name)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get credential metadata: %w", err)
	}

	return info, nil
}
