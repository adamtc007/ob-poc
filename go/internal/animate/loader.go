package animate

import (
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

// LoadScenario loads a scenario from a YAML file.
func LoadScenario(path string) (*Scenario, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("reading scenario file: %w", err)
	}

	var scenario Scenario
	if err := yaml.Unmarshal(data, &scenario); err != nil {
		return nil, fmt.Errorf("parsing scenario YAML: %w", err)
	}

	// Set defaults
	if scenario.TypingSpeedMs == 0 {
		scenario.TypingSpeedMs = 30 // Default typing speed
	}
	if scenario.PauseAfterMs == 0 {
		scenario.PauseAfterMs = 1000 // Default pause
	}

	return &scenario, nil
}

// ListScenarios lists all scenario files in a directory.
func ListScenarios(dir string) ([]string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, fmt.Errorf("reading scenarios directory: %w", err)
	}

	var scenarios []string
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		ext := filepath.Ext(entry.Name())
		if ext == ".yaml" || ext == ".yml" {
			scenarios = append(scenarios, filepath.Join(dir, entry.Name()))
		}
	}

	return scenarios, nil
}

// LoadAllScenarios loads all scenarios from a directory.
func LoadAllScenarios(dir string) ([]*Scenario, error) {
	paths, err := ListScenarios(dir)
	if err != nil {
		return nil, err
	}

	var scenarios []*Scenario
	for _, path := range paths {
		scenario, err := LoadScenario(path)
		if err != nil {
			return nil, fmt.Errorf("loading %s: %w", path, err)
		}
		scenarios = append(scenarios, scenario)
	}

	return scenarios, nil
}
