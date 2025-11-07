package cli

// maskConnectionString masks sensitive parts of database connection string for display
func maskConnectionString(connStr string) string {
	if len(connStr) > 20 {
		return connStr[:10] + "..." + connStr[len(connStr)-10:]
	}
	return "***"
}

// min returns the minimum of two integers
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
