package store

import (
	"database/sql/driver"
	"encoding/json"
	"errors"
	"fmt"

	"dsl-ob-poc/internal/dictionary"
)

// JSONBSourceMetadata wraps dictionary.SourceMetadata for JSONB database storage
type JSONBSourceMetadata struct {
	dictionary.SourceMetadata
}

// Value implements the driver.Valuer interface for database storage
func (j JSONBSourceMetadata) Value() (driver.Value, error) {
	if j.SourceMetadata.Primary == "" {
		return nil, nil
	}
	return json.Marshal(j.SourceMetadata)
}

// Scan implements the sql.Scanner interface for database retrieval
func (j *JSONBSourceMetadata) Scan(value interface{}) error {
	if value == nil {
		j.SourceMetadata = dictionary.SourceMetadata{}
		return nil
	}

	var bytes []byte
	switch v := value.(type) {
	case []byte:
		bytes = v
	case string:
		bytes = []byte(v)
	default:
		return errors.New("cannot scan non-string/[]byte value into JSONBSourceMetadata")
	}

	return json.Unmarshal(bytes, &j.SourceMetadata)
}

// JSONBSinkMetadata wraps dictionary.SinkMetadata for JSONB database storage
type JSONBSinkMetadata struct {
	dictionary.SinkMetadata
}

// Value implements the driver.Valuer interface for database storage
func (j JSONBSinkMetadata) Value() (driver.Value, error) {
	if j.SinkMetadata.Primary == "" {
		return nil, nil
	}
	return json.Marshal(j.SinkMetadata)
}

// Scan implements the sql.Scanner interface for database retrieval
func (j *JSONBSinkMetadata) Scan(value interface{}) error {
	if value == nil {
		j.SinkMetadata = dictionary.SinkMetadata{}
		return nil
	}

	var bytes []byte
	switch v := value.(type) {
	case []byte:
		bytes = v
	case string:
		bytes = []byte(v)
	default:
		return errors.New("cannot scan non-string/[]byte value into JSONBSinkMetadata")
	}

	return json.Unmarshal(bytes, &j.SinkMetadata)
}

// JSONBStringArray handles string arrays in JSONB format
type JSONBStringArray []string

// Value implements the driver.Valuer interface
func (j JSONBStringArray) Value() (driver.Value, error) {
	if len(j) == 0 {
		return nil, nil
	}
	return json.Marshal([]string(j))
}

// Scan implements the sql.Scanner interface
func (j *JSONBStringArray) Scan(value interface{}) error {
	if value == nil {
		*j = []string{}
		return nil
	}

	var bytes []byte
	switch v := value.(type) {
	case []byte:
		bytes = v
	case string:
		bytes = []byte(v)
	default:
		return fmt.Errorf("cannot scan %T into JSONBStringArray", value)
	}

	var arr []string
	err := json.Unmarshal(bytes, &arr)
	if err != nil {
		return err
	}
	*j = JSONBStringArray(arr)
	return nil
}

// JSONBGeneric handles generic JSONB data
type JSONBGeneric map[string]interface{}

// Value implements the driver.Valuer interface
func (j JSONBGeneric) Value() (driver.Value, error) {
	if len(j) == 0 {
		return nil, nil
	}
	return json.Marshal(map[string]interface{}(j))
}

// Scan implements the sql.Scanner interface
func (j *JSONBGeneric) Scan(value interface{}) error {
	if value == nil {
		*j = make(map[string]interface{})
		return nil
	}

	var bytes []byte
	switch v := value.(type) {
	case []byte:
		bytes = v
	case string:
		bytes = []byte(v)
	default:
		return fmt.Errorf("cannot scan %T into JSONBGeneric", value)
	}

	var data map[string]interface{}
	err := json.Unmarshal(bytes, &data)
	if err != nil {
		return err
	}
	*j = JSONBGeneric(data)
	return nil
}
