package cli

import (
	"context"
	"fmt"
	"strings"

	"dsl-ob-poc/internal/datastore"
)

// RunRoleCreate creates a new role
func RunRoleCreate(ctx context.Context, ds datastore.DataStore, args []string) error {
	name, description, err := parseRoleCreateArgs(args)
	if err != nil {
		return err
	}

	roleID, err := ds.CreateRole(ctx, name, description)
	if err != nil {
		return fmt.Errorf("failed to create role: %w", err)
	}

	fmt.Printf("Created role: %s (ID: %s)\n", name, roleID)
	return nil
}

// RunRoleList lists all roles
func RunRoleList(ctx context.Context, ds datastore.DataStore, args []string) error {
	roles, err := ds.ListRoles(ctx)
	if err != nil {
		return fmt.Errorf("failed to list roles: %w", err)
	}

	if len(roles) == 0 {
		fmt.Println("No roles found.")
		return nil
	}

	fmt.Println("Roles:")
	for _, role := range roles {
		fmt.Printf("  ID: %s\n", role.RoleID)
		fmt.Printf("  Name: %s\n", role.Name)
		fmt.Printf("  Description: %s\n", role.Description)
		fmt.Println("  ---")
	}

	return nil
}

// RunRoleGet retrieves a specific role
func RunRoleGet(ctx context.Context, ds datastore.DataStore, args []string) error {
	roleID, err := parseRoleGetArgs(args)
	if err != nil {
		return err
	}

	role, err := ds.GetRoleByID(ctx, roleID)
	if err != nil {
		return fmt.Errorf("failed to get role: %w", err)
	}

	fmt.Printf("Role Details:\n")
	fmt.Printf("  ID: %s\n", role.RoleID)
	fmt.Printf("  Name: %s\n", role.Name)
	fmt.Printf("  Description: %s\n", role.Description)

	return nil
}

// RunRoleUpdate updates a role
func RunRoleUpdate(ctx context.Context, ds datastore.DataStore, args []string) error {
	roleID, name, description, err := parseRoleUpdateArgs(args)
	if err != nil {
		return err
	}

	err = ds.UpdateRole(ctx, roleID, name, description)
	if err != nil {
		return fmt.Errorf("failed to update role: %w", err)
	}

	fmt.Printf("Updated role: %s\n", roleID)
	return nil
}

// RunRoleDelete deletes a role
func RunRoleDelete(ctx context.Context, ds datastore.DataStore, args []string) error {
	roleID, err := parseRoleGetArgs(args)
	if err != nil {
		return err
	}

	err = ds.DeleteRole(ctx, roleID)
	if err != nil {
		return fmt.Errorf("failed to delete role: %w", err)
	}

	fmt.Printf("Deleted role: %s\n", roleID)
	return nil
}

func parseRoleCreateArgs(args []string) (name, description string, err error) {
	for _, arg := range args {
		if strings.HasPrefix(arg, "--name=") {
			name = strings.TrimPrefix(arg, "--name=")
		} else if strings.HasPrefix(arg, "--description=") {
			description = strings.TrimPrefix(arg, "--description=")
		}
	}

	if name == "" {
		return "", "", fmt.Errorf("--name is required")
	}

	return name, description, nil
}

func parseRoleGetArgs(args []string) (roleID string, err error) {
	for _, arg := range args {
		if strings.HasPrefix(arg, "--id=") {
			roleID = strings.TrimPrefix(arg, "--id=")
		}
	}

	if roleID == "" {
		return "", fmt.Errorf("--id is required")
	}

	return roleID, nil
}

func parseRoleUpdateArgs(args []string) (roleID, name, description string, err error) {
	for _, arg := range args {
		if strings.HasPrefix(arg, "--id=") {
			roleID = strings.TrimPrefix(arg, "--id=")
		} else if strings.HasPrefix(arg, "--name=") {
			name = strings.TrimPrefix(arg, "--name=")
		} else if strings.HasPrefix(arg, "--description=") {
			description = strings.TrimPrefix(arg, "--description=")
		}
	}

	if roleID == "" {
		return "", "", "", fmt.Errorf("--id is required")
	}

	return roleID, name, description, nil
}
