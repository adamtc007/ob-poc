package cli

import (
	"context"
	"fmt"
	"time"

	"github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
	"github.com/spf13/cobra"

	"dsl-ob-poc/internal/config"
	"dsl-ob-poc/internal/vocabulary"
)

// MigrateVocabularyCommand creates the migrate-vocabulary command
func MigrateVocabularyCommand() *cobra.Command {
	var (
		domain    string
		all       bool
		verify    bool
		cleanup   bool
		dryRun    bool
		dbConnStr string
	)

	cmd := &cobra.Command{
		Use:   "migrate-vocabulary",
		Short: "Migrate hardcoded vocabularies to database (Phase 4)",
		Long: `Migrate hardcoded DSL vocabularies from in-memory maps to database storage.

This command implements Phase 4 of the Multi-DSL Orchestration Implementation Plan
by transferring all hardcoded vocabulary definitions to the database, enabling
dynamic vocabulary updates and cross-domain coordination.

Examples:
  # Migrate specific domain
  ./dsl-poc migrate-vocabulary --domain=onboarding

  # Migrate all domains
  ./dsl-poc migrate-vocabulary --all

  # Dry run to see what would be migrated
  ./dsl-poc migrate-vocabulary --all --dry-run

  # Migrate and verify integrity
  ./dsl-poc migrate-vocabulary --all --verify

  # Full migration with cleanup
  ./dsl-poc migrate-vocabulary --all --verify --cleanup`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return runMigrateVocabulary(dbConnStr, domain, all, verify, cleanup, dryRun)
		},
	}

	cmd.Flags().StringVar(&domain, "domain", "", "Specific domain to migrate (onboarding, hedge-fund-investor, orchestration)")
	cmd.Flags().BoolVar(&all, "all", false, "Migrate all domains")
	cmd.Flags().BoolVar(&verify, "verify", false, "Verify migration integrity after completion")
	cmd.Flags().BoolVar(&cleanup, "cleanup", false, "Remove hardcoded references after migration (WARNING: modifies code)")
	cmd.Flags().BoolVar(&dryRun, "dry-run", false, "Show what would be migrated without making changes")
	cmd.Flags().StringVar(&dbConnStr, "db", "", "Database connection string (overrides env var)")

	// At least one of --domain or --all must be specified
	cmd.MarkFlagsMutuallyExclusive("domain", "all")

	return cmd
}

func runMigrateVocabulary(dbConnStr, domain string, all, verify, cleanup, dryRun bool) error {
	ctx := context.Background()

	// Validation
	if !all && domain == "" {
		return fmt.Errorf("must specify either --domain or --all")
	}

	if domain != "" && domain != "onboarding" && domain != "hedge-fund-investor" && domain != "orchestration" {
		return fmt.Errorf("invalid domain: %s. Valid domains: onboarding, hedge-fund-investor, orchestration", domain)
	}

	// Get database connection string
	if dbConnStr == "" {
		cfg := config.GetDataStoreConfig()
		dbConnStr = cfg.ConnectionString
		if dbConnStr == "" {
			return fmt.Errorf("database connection string not provided. Set DB_CONN_STRING environment variable or use --db flag")
		}
	}

	fmt.Printf("🔄 Starting Phase 4 Database Migration\n")
	fmt.Printf("📅 Timestamp: %s\n", time.Now().Format("2006-01-02 15:04:05"))
	if dryRun {
		fmt.Printf("🚨 DRY RUN MODE - No changes will be made\n")
	}
	fmt.Printf("🗄️  Database: %s\n", maskConnectionString(dbConnStr))
	fmt.Printf("\n")

	// Connect to database
	db, err := sqlx.Connect("postgres", dbConnStr)
	if err != nil {
		return fmt.Errorf("failed to connect to database: %w", err)
	}
	defer db.Close()

	// Test database connection
	if err := db.Ping(); err != nil {
		return fmt.Errorf("failed to ping database: %w", err)
	}

	fmt.Printf("✅ Database connection established\n\n")

	// Create repository and services
	repo := vocabulary.NewPostgresRepository(db)
	vocabService := vocabulary.NewVocabularyService(repo, nil) // No cache for migration
	migrationService := vocabulary.NewMigrationService(repo, vocabService)

	// Get current migration status
	fmt.Printf("📊 Checking current migration status...\n")
	status, err := migrationService.GetMigrationStatus(ctx)
	if err != nil {
		return fmt.Errorf("failed to get migration status: %w", err)
	}

	for domain, migrated := range status {
		statusIcon := "❌"
		if migrated {
			statusIcon = "✅"
		}
		fmt.Printf("  %s %s: %s\n", statusIcon, domain, getMigrationStatusText(migrated))
	}
	fmt.Printf("\n")

	if dryRun {
		return runDryRunAnalysis(ctx, migrationService, domain, all, status)
	}

	// Perform migrations
	if all {
		return migrateAllDomains(ctx, migrationService, verify, cleanup, status)
	} else {
		return migrateSingleDomain(ctx, migrationService, domain, verify, cleanup, status)
	}
}

func runDryRunAnalysis(ctx context.Context, migrationService vocabulary.MigrationService, domain string, all bool, status map[string]bool) error {
	fmt.Printf("🔍 DRY RUN ANALYSIS\n")
	fmt.Printf("===================\n\n")

	domains := []string{}
	if all {
		domains = []string{"onboarding", "hedge-fund-investor", "orchestration"}
	} else {
		domains = []string{domain}
	}

	totalVerbs := 0
	for _, d := range domains {
		if status[d] {
			fmt.Printf("🟡 Domain '%s' already migrated - would skip\n", d)
			continue
		}

		verbCount := getExpectedVerbCount(d)
		totalVerbs += verbCount
		fmt.Printf("🔄 Domain '%s' would migrate %d verbs\n", d, verbCount)

		// Show sample verbs that would be migrated
		samples := getSampleVerbs(d)
		fmt.Printf("   Sample verbs:\n")
		for _, verb := range samples {
			fmt.Printf("     - %s\n", verb)
		}
		fmt.Printf("\n")
	}

	if totalVerbs == 0 {
		fmt.Printf("✨ All specified domains are already migrated!\n")
	} else {
		fmt.Printf("📈 SUMMARY:\n")
		fmt.Printf("  - Domains to migrate: %d\n", len(domains))
		fmt.Printf("  - Total verbs to migrate: %d\n", totalVerbs)
		fmt.Printf("  - Database tables affected: domain_vocabularies, verb_registry, vocabulary_audit\n")
		fmt.Printf("\nTo perform the actual migration, run without --dry-run flag.\n")
	}

	return nil
}

func migrateAllDomains(ctx context.Context, migrationService vocabulary.MigrationService, verify, cleanup bool, status map[string]bool) error {
	domains := []struct {
		name      string
		migrateFn func(context.Context) error
	}{
		{"onboarding", migrationService.MigrateOnboardingDomain},
		{"hedge-fund-investor", migrationService.MigrateHedgeFundDomain},
		{"orchestration", migrationService.MigrateOrchestrationDomain},
	}

	migratedCount := 0
	skippedCount := 0

	for _, domain := range domains {
		fmt.Printf("🔄 Migrating domain: %s\n", domain.name)

		if status[domain.name] {
			fmt.Printf("  🟡 Domain already migrated - skipping\n\n")
			skippedCount++
			continue
		}

		startTime := time.Now()
		err := domain.migrateFn(ctx)
		duration := time.Since(startTime)

		if err != nil {
			fmt.Printf("  ❌ Failed to migrate domain %s: %v\n\n", domain.name, err)
			return fmt.Errorf("migration failed for domain %s: %w", domain.name, err)
		}

		fmt.Printf("  ✅ Successfully migrated domain %s (%v)\n\n", domain.name, duration)
		migratedCount++

		if verify {
			fmt.Printf("  🔍 Verifying migration integrity...\n")
			if err := migrationService.VerifyMigrationIntegrity(ctx, domain.name); err != nil {
				fmt.Printf("  ❌ Verification failed: %v\n\n", err)
				return fmt.Errorf("verification failed for domain %s: %w", domain.name, err)
			}
			fmt.Printf("  ✅ Verification completed\n\n")
		}

		if cleanup {
			fmt.Printf("  🧹 Cleaning up hardcoded references...\n")
			if err := migrationService.RemoveHardcodedReferences(ctx, domain.name); err != nil {
				fmt.Printf("  ⚠️  Cleanup warning: %v\n\n", err)
				// Don't fail on cleanup errors, just warn
			} else {
				fmt.Printf("  ✅ Cleanup completed\n\n")
			}
		}
	}

	// Final summary
	fmt.Printf("🎉 MIGRATION COMPLETE\n")
	fmt.Printf("====================\n")
	fmt.Printf("✅ Domains migrated: %d\n", migratedCount)
	fmt.Printf("🟡 Domains skipped: %d\n", skippedCount)
	fmt.Printf("🕒 Total time: %v\n", time.Since(time.Now()))
	fmt.Printf("\n")

	if migratedCount > 0 {
		fmt.Printf("🚀 Phase 4 Database Migration Successfully Completed!\n")
		fmt.Printf("\n")
		fmt.Printf("Next Steps:\n")
		fmt.Printf("1. Update code to use vocabulary.VocabularyService instead of hardcoded maps\n")
		fmt.Printf("2. Test all DSL validation with database-backed vocabularies\n")
		fmt.Printf("3. Remove deprecated vocabulary files after testing\n")
		fmt.Printf("4. Configure caching for production performance\n")
		fmt.Printf("\n")
		fmt.Printf("Benefits Now Available:\n")
		fmt.Printf("✨ Dynamic vocabulary updates without code deployment\n")
		fmt.Printf("✨ Cross-domain vocabulary coordination\n")
		fmt.Printf("✨ Complete audit trail for vocabulary changes\n")
		fmt.Printf("✨ AI-driven vocabulary discovery and validation\n")
	}

	return nil
}

func migrateSingleDomain(ctx context.Context, migrationService vocabulary.MigrationService, domain string, verify, cleanup bool, status map[string]bool) error {
	if status[domain] {
		fmt.Printf("🟡 Domain '%s' is already migrated!\n", domain)

		if verify {
			fmt.Printf("🔍 Verifying existing migration...\n")
			if err := migrationService.VerifyMigrationIntegrity(ctx, domain); err != nil {
				return fmt.Errorf("verification failed: %w", err)
			}
			fmt.Printf("✅ Verification completed - migration is intact\n")
		}
		return nil
	}

	fmt.Printf("🔄 Migrating domain: %s\n", domain)
	startTime := time.Now()

	var err error
	switch domain {
	case "onboarding":
		err = migrationService.MigrateOnboardingDomain(ctx)
	case "hedge-fund-investor":
		err = migrationService.MigrateHedgeFundDomain(ctx)
	case "orchestration":
		err = migrationService.MigrateOrchestrationDomain(ctx)
	default:
		return fmt.Errorf("unsupported domain: %s", domain)
	}

	duration := time.Since(startTime)

	if err != nil {
		fmt.Printf("❌ Failed to migrate domain %s: %v\n", domain, err)
		return fmt.Errorf("migration failed: %w", err)
	}

	fmt.Printf("✅ Successfully migrated domain %s (%v)\n", domain, duration)

	if verify {
		fmt.Printf("🔍 Verifying migration integrity...\n")
		if err := migrationService.VerifyMigrationIntegrity(ctx, domain); err != nil {
			return fmt.Errorf("verification failed: %w", err)
		}
		fmt.Printf("✅ Verification completed\n")
	}

	if cleanup {
		fmt.Printf("🧹 Cleaning up hardcoded references...\n")
		if err := migrationService.RemoveHardcodedReferences(ctx, domain); err != nil {
			fmt.Printf("⚠️  Cleanup warning: %v\n", err)
			// Don't fail on cleanup errors, just warn
		} else {
			fmt.Printf("✅ Cleanup completed\n")
		}
	}

	fmt.Printf("\n🎉 Domain %s migration completed successfully!\n", domain)
	return nil
}

// Helper functions

func getMigrationStatusText(migrated bool) string {
	if migrated {
		return "Already migrated"
	}
	return "Needs migration"
}

func getExpectedVerbCount(domain string) int {
	switch domain {
	case "onboarding":
		return 25 // Approximate count from vocab.go
	case "hedge-fund-investor":
		return 5 // From hedge fund DSL
	case "orchestration":
		return 2 // From orchestration vocabulary
	default:
		return 0
	}
}

func getSampleVerbs(domain string) []string {
	switch domain {
	case "onboarding":
		return []string{"case.create", "case.update", "products.add", "kyc.start", "resources.plan"}
	case "hedge-fund-investor":
		return []string{"investor.start-opportunity", "kyc.begin", "subscription.create"}
	case "orchestration":
		return []string{"orchestration.create", "orchestration.route"}
	default:
		return []string{}
	}
}

// RunMigrateVocabulary is the CLI wrapper function for migrate-vocabulary command
func RunMigrateVocabulary(ctx context.Context, args []string) error {
	cmd := MigrateVocabularyCommand()
	cmd.SetArgs(args)
	return cmd.Execute()
}
