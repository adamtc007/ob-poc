package cli

import (
	"bytes"
	"context"
	"io"
	"os"
	"regexp"
	"testing"

	"dsl-ob-poc/internal/datastore"
)

func TestRunHistory_PrintsHistory(t *testing.T) {
	// Use mock store for simpler testing
	config := datastore.Config{
		Type:         datastore.MockStore,
		MockDataPath: "../../data/mocks",
	}

	ds, err := datastore.NewDataStore(config)
	if err != nil {
		t.Fatalf("failed to create mock data store: %v", err)
	}
	defer ds.Close()

	cbu := "CBU-1234"

	// Capture stdout
	origStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w
	defer func() { os.Stdout = origStdout }()

	err = RunHistory(context.Background(), ds, []string{"--cbu", cbu})
	w.Close()
	if err != nil {
		t.Fatalf("RunHistory returned error: %v", err)
	}

	var buf bytes.Buffer
	_, _ = io.Copy(&buf, r)
	out := buf.String()

	// Test for mock data - should show multiple versions
	if !regexp.MustCompile(`Found\s+\d+\s+versions`).MatchString(out) {
		t.Errorf("output did not report versions count: %s", out)
	}
	if !regexp.MustCompile(`State:\s+CREATED`).MatchString(out) {
		t.Errorf("output missing CREATED state: %s", out)
	}
	if !regexp.MustCompile(`Current State:\s+CREATED`).MatchString(out) {
		t.Errorf("output missing current state: %s", out)
	}
}
