package cli

import (
	"context"
	"fmt"
	"strings"

	"dsl-ob-poc/internal/datastore"
)

// RunCBUCreate creates a new CBU
func RunCBUCreate(ctx context.Context, ds datastore.DataStore, args []string) error {
	name, description, naturePurpose, err := parseCBUCreateArgs(args)
	if err != nil {
		return err
	}

	cbuID, err := ds.CreateCBU(ctx, name, description, naturePurpose)
	if err != nil {
		return fmt.Errorf("failed to create CBU: %w", err)
	}

	fmt.Printf("Created CBU: %s (ID: %s)\n", name, cbuID)
	return nil
}

// RunCBUList lists all CBUs
func RunCBUList(ctx context.Context, ds datastore.DataStore, args []string) error {
	cbus, err := ds.ListCBUs(ctx)
	if err != nil {
		return fmt.Errorf("failed to list CBUs: %w", err)
	}

	if len(cbus) == 0 {
		fmt.Println("No CBUs found.")
		return nil
	}

	fmt.Println("CBUs:")
	for _, cbu := range cbus {
		fmt.Printf("  ID: %s\n", cbu.CBUID)
		fmt.Printf("  Name: %s\n", cbu.Name)
		fmt.Printf("  Description: %s\n", cbu.Description)
		fmt.Printf("  Nature/Purpose: %s\n", cbu.NaturePurpose)
		fmt.Println("  ---")
	}

	return nil
}

// RunCBUGet retrieves a specific CBU
func RunCBUGet(ctx context.Context, ds datastore.DataStore, args []string) error {
	cbuID, err := parseCBUGetArgs(args)
	if err != nil {
		return err
	}

	cbu, err := ds.GetCBUByID(ctx, cbuID)
	if err != nil {
		return fmt.Errorf("failed to get CBU: %w", err)
	}

	fmt.Printf("CBU Details:\n")
	fmt.Printf("  ID: %s\n", cbu.CBUID)
	fmt.Printf("  Name: %s\n", cbu.Name)
	fmt.Printf("  Description: %s\n", cbu.Description)
	fmt.Printf("  Nature/Purpose: %s\n", cbu.NaturePurpose)

	return nil
}

// RunCBUUpdate updates a CBU
func RunCBUUpdate(ctx context.Context, ds datastore.DataStore, args []string) error {
	cbuID, name, description, naturePurpose, err := parseCBUUpdateArgs(args)
	if err != nil {
		return err
	}

	err = ds.UpdateCBU(ctx, cbuID, name, description, naturePurpose)
	if err != nil {
		return fmt.Errorf("failed to update CBU: %w", err)
	}

	fmt.Printf("Updated CBU: %s\n", cbuID)
	return nil
}

// RunCBUDelete deletes a CBU
func RunCBUDelete(ctx context.Context, ds datastore.DataStore, args []string) error {
	cbuID, err := parseCBUGetArgs(args)
	if err != nil {
		return err
	}

	err = ds.DeleteCBU(ctx, cbuID)
	if err != nil {
		return fmt.Errorf("failed to delete CBU: %w", err)
	}

	fmt.Printf("Deleted CBU: %s\n", cbuID)
	return nil
}

func parseCBUCreateArgs(args []string) (name, description, naturePurpose string, err error) {
	for _, arg := range args {
		switch {
		case strings.HasPrefix(arg, "--name="):
			name = strings.TrimPrefix(arg, "--name=")
		case strings.HasPrefix(arg, "--description="):
			description = strings.TrimPrefix(arg, "--description=")
		case strings.HasPrefix(arg, "--nature-purpose="):
			naturePurpose = strings.TrimPrefix(arg, "--nature-purpose=")
		}
	}

	if name == "" {
		return "", "", "", fmt.Errorf("--name is required")
	}

	return name, description, naturePurpose, nil
}

func parseCBUGetArgs(args []string) (cbuID string, err error) {
	for _, arg := range args {
		if strings.HasPrefix(arg, "--id=") {
			cbuID = strings.TrimPrefix(arg, "--id=")
		}
	}

	if cbuID == "" {
		return "", fmt.Errorf("--id is required")
	}

	return cbuID, nil
}

func parseCBUUpdateArgs(args []string) (cbuID, name, description, naturePurpose string, err error) {
	for _, arg := range args {
		if strings.HasPrefix(arg, "--id=") {
			cbuID = strings.TrimPrefix(arg, "--id=")
		} else if strings.HasPrefix(arg, "--name=") {
			name = strings.TrimPrefix(arg, "--name=")
		} else if strings.HasPrefix(arg, "--description=") {
			description = strings.TrimPrefix(arg, "--description=")
		} else if strings.HasPrefix(arg, "--nature-purpose=") {
			naturePurpose = strings.TrimPrefix(arg, "--nature-purpose=")
		}
	}

	if cbuID == "" {
		return "", "", "", "", fmt.Errorf("--id is required")
	}

	return cbuID, name, description, naturePurpose, nil
}
