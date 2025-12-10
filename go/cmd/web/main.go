// Web server for OB-POC UI using Gin framework.
package main

import (
	"bytes"
	"context"
	"embed"
	"flag"
	"fmt"
	"html/template"
	"io"
	"io/fs"
	"log"
	"net/http"
	"net/url"

	"encoding/json"

	"github.com/adamtc007/ob-poc/go/internal/harness"
	"github.com/adamtc007/ob-poc/go/internal/rustclient"
	"github.com/gin-gonic/gin"
)

//go:embed templates/*.html
var templates embed.FS

//go:embed static/*
var static embed.FS

var (
	rustURL  string
	agentURL string
	client   *rustclient.Client
	tmpl     *template.Template
)

func main() {
	addr := flag.String("addr", ":8181", "Listen address")
	flag.StringVar(&rustURL, "rust-url", "http://127.0.0.1:3001", "Rust DSL API URL")
	flag.StringVar(&agentURL, "agent-url", "http://127.0.0.1:3000", "Rust Agent API URL")
	flag.Parse()

	if rustURL != "" {
		client = rustclient.NewClient(rustURL)
	}

	var err error
	tmpl, err = template.ParseFS(templates, "templates/*.html")
	if err != nil {
		log.Fatalf("parsing templates: %v", err)
	}

	// Use release mode in production
	gin.SetMode(gin.ReleaseMode)

	r := gin.New()
	r.Use(gin.Logger())
	r.Use(gin.Recovery())
	r.Use(corsMiddleware())

	// Static files - use fs.Sub to strip the "static" prefix from embed.FS
	staticSub, err := fs.Sub(static, "static")
	if err != nil {
		log.Fatalf("failed to create static sub-filesystem: %v", err)
	}
	r.StaticFS("/static", http.FS(staticSub))

	// HTML routes
	r.GET("/", handleIndex)
	r.GET("/health", handleHealth)

	// API routes
	api := r.Group("/api")
	{
		// Config endpoints
		api.GET("/config", handleConfigGet)
		api.POST("/config", handleConfigPost)
		api.POST("/run", handleRunSuite)
		api.POST("/validate", handleValidate)

		// Agent proxy endpoints
		agent := api.Group("/agent")
		{
			agent.POST("/session", handleAgentSession)
			agent.POST("/chat", handleAgentChat)
			agent.POST("/generate", handleAgentGenerate)
			agent.POST("/execute", handleAgentExecute)
			agent.POST("/bind", handleAgentBind)
			agent.POST("/complete", handleAgentComplete)
		}

		// DSL endpoints
		dsl := api.Group("/dsl")
		{
			dsl.POST("/execute", handleDirectExecute)
			dsl.POST("/analyze-errors", handleAnalyzeErrors)
			dsl.POST("/validate-with-fixes", handleValidateWithFixes)
			dsl.POST("/resolve-ref", handleResolveRef)
		}

		// Entity endpoints
		api.POST("/entities/search", handleEntitySearch)
		api.POST("/entity/search", handleEntitySearchForFinder)
		api.GET("/cbus", handleListCbus)
		api.GET("/cbu/:id/graph", handleCbuGraph)

		// KYC endpoints
		api.GET("/kyc/case/:id", handleKycCase)
	}

	log.Printf("Starting Gin server on %s", *addr)
	log.Printf("  DSL API: %s", rustURL)
	log.Printf("  Agent API: %s", agentURL)
	if err := r.Run(*addr); err != nil {
		log.Fatal(err)
	}
}

// corsMiddleware adds CORS headers for cross-origin requests
func corsMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Header("Access-Control-Allow-Origin", "*")
		c.Header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
		c.Header("Access-Control-Allow-Headers", "Content-Type")
		if c.Request.Method == "OPTIONS" {
			c.AbortWithStatus(204)
			return
		}
		c.Next()
	}
}

// ============================================================================
// Proxy Helpers
// ============================================================================

// proxyPostJSON forwards a JSON POST request to a backend URL
func proxyPostJSON(c *gin.Context, targetURL string) {
	body, err := io.ReadAll(c.Request.Body)
	if err != nil {
		c.JSON(400, gin.H{"error": "Failed to read request: " + err.Error()})
		return
	}

	resp, err := http.Post(targetURL, "application/json", bytes.NewReader(body))
	if err != nil {
		c.JSON(503, gin.H{"error": "Backend connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}

	c.Data(resp.StatusCode, "application/json", respBody)
}

// proxyGet forwards a GET request to a backend URL
func proxyGet(c *gin.Context, targetURL string) {
	resp, err := http.Get(targetURL)
	if err != nil {
		c.JSON(503, gin.H{"error": "Backend connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}

	c.Data(resp.StatusCode, "application/json", respBody)
}

// ============================================================================
// HTML Handlers
// ============================================================================

func handleIndex(c *gin.Context) {
	var health *rustclient.HealthResponse
	var verbs *rustclient.VerbsResponse

	if client != nil {
		ctx := c.Request.Context()
		health, _ = client.Health(ctx)
		verbs, _ = client.ListVerbs(ctx)
	}

	// Check agent health for connected status
	agentHealthy := false
	if resp, err := http.Get(agentURL + "/api/agent/health"); err == nil {
		resp.Body.Close()
		agentHealthy = resp.StatusCode == 200
	}

	data := map[string]any{
		"Health":    health,
		"Verbs":     verbs,
		"RustURL":   rustURL,
		"AgentURL":  agentURL,
		"Connected": agentHealthy,
	}

	c.Header("Content-Type", "text/html; charset=utf-8")
	if err := tmpl.ExecuteTemplate(c.Writer, "index.html", data); err != nil {
		c.String(500, err.Error())
	}
}

func handleHealth(c *gin.Context) {
	if client == nil {
		c.JSON(503, gin.H{"error": "Rust API not configured"})
		return
	}
	ctx := c.Request.Context()
	health, err := client.Health(ctx)
	if err != nil {
		c.JSON(503, gin.H{"error": err.Error()})
		return
	}
	c.JSON(200, health)
}

// ============================================================================
// Config Handlers
// ============================================================================

func handleConfigGet(c *gin.Context) {
	c.JSON(200, gin.H{
		"rust_url":  rustURL,
		"agent_url": agentURL,
		"connected": client != nil,
	})
}

func handleConfigPost(c *gin.Context) {
	var req struct {
		RustURL  string `json:"rust_url"`
		AgentURL string `json:"agent_url"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}
	if req.RustURL != "" {
		rustURL = req.RustURL
		client = rustclient.NewClient(rustURL)
	}
	if req.AgentURL != "" {
		agentURL = req.AgentURL
	}
	c.JSON(200, gin.H{"status": "ok", "rust_url": rustURL, "agent_url": agentURL})
}

func handleValidate(c *gin.Context) {
	if client == nil {
		c.JSON(503, gin.H{"error": "Rust API not configured"})
		return
	}
	var req struct {
		DSL string `json:"dsl"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	result, err := client.ValidateDSL(c.Request.Context(), req.DSL)
	if err != nil {
		c.JSON(500, gin.H{"error": err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleRunSuite(c *gin.Context) {
	if rustURL == "" {
		c.JSON(503, gin.H{"error": "Rust API not configured"})
		return
	}

	var req struct {
		Cases []harness.Case `json:"cases"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	suite := harness.Suite{
		Name:  "Ad-hoc Suite",
		Cases: req.Cases,
	}

	runner := harness.NewRunner(rustURL)
	result, err := runner.Run(context.Background(), suite)
	if err != nil {
		c.JSON(500, gin.H{"error": err.Error()})
		return
	}
	c.JSON(200, result)
}

// ============================================================================
// Agent Proxy Handlers
// ============================================================================

func handleAgentSession(c *gin.Context) {
	resp, err := http.Post(agentURL+"/api/session", "application/json", bytes.NewReader([]byte("{}")))
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(body, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleAgentChat(c *gin.Context) {
	var req struct {
		SessionID string `json:"session_id"`
		Message   string `json:"message"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(map[string]string{"message": req.Message})
	resp, err := http.Post(agentURL+"/api/session/"+req.SessionID+"/chat", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleAgentGenerate(c *gin.Context) {
	var req struct {
		Instruction string `json:"instruction"`
		Domain      string `json:"domain,omitempty"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(req)
	resp, err := http.Post(agentURL+"/api/agent/generate", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleAgentExecute(c *gin.Context) {
	var req struct {
		SessionID string `json:"session_id"`
		DSL       string `json:"dsl"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(map[string]string{"dsl": req.DSL})
	resp, err := http.Post(agentURL+"/api/session/"+req.SessionID+"/execute", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleAgentBind(c *gin.Context) {
	log.Printf("[BIND-GO] Received bind request")

	var req struct {
		SessionID   string `json:"session_id"`
		Name        string `json:"name"`
		ID          string `json:"id"`
		EntityType  string `json:"entity_type"`
		DisplayName string `json:"display_name"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		log.Printf("[BIND-GO] Decode error: %v", err)
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}
	log.Printf("[BIND-GO] Request: session=%s, name=%s, id=%s, type=%s, display=%s",
		req.SessionID, req.Name, req.ID, req.EntityType, req.DisplayName)

	// Forward to Rust API
	reqBody, _ := json.Marshal(map[string]string{
		"name":         req.Name,
		"id":           req.ID,
		"entity_type":  req.EntityType,
		"display_name": req.DisplayName,
	})
	targetURL := agentURL + "/api/session/" + req.SessionID + "/bind"
	log.Printf("[BIND-GO] Forwarding to: %s", targetURL)

	resp, err := http.Post(targetURL, "application/json", bytes.NewReader(reqBody))
	if err != nil {
		log.Printf("[BIND-GO] Connection failed: %v", err)
		c.JSON(503, gin.H{"error": "Agent connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	log.Printf("[BIND-GO] Rust response status: %d", resp.StatusCode)
	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	log.Printf("[BIND-GO] Rust response body: %s", string(respBody))

	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleAgentComplete(c *gin.Context) {
	var req struct {
		EntityType string `json:"entity_type"`
		Query      string `json:"query"`
		Limit      int    `json:"limit,omitempty"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(req)
	resp, err := http.Post(agentURL+"/api/agent/complete", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent API connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent API: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

// ============================================================================
// DSL Handlers
// ============================================================================

func handleDirectExecute(c *gin.Context) {
	var req struct {
		DSL string `json:"dsl"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(req)
	resp, err := http.Post(agentURL+"/execute", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent API connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from Agent API: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleAnalyzeErrors(c *gin.Context) {
	var req struct {
		DSL    string   `json:"dsl"`
		Errors []string `json:"errors"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(req)
	resp, err := http.Post(rustURL+"/analyze-errors", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "DSL API connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from DSL API: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleValidateWithFixes(c *gin.Context) {
	var req struct {
		DSL string `json:"dsl"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	reqBody, _ := json.Marshal(req)
	resp, err := http.Post(rustURL+"/validate-with-fixes", "application/json", bytes.NewReader(reqBody))
	if err != nil {
		c.JSON(503, gin.H{"error": "DSL API connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}
	var result map[string]any
	if err := json.Unmarshal(respBody, &result); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from DSL API: " + err.Error()})
		return
	}
	c.JSON(200, result)
}

func handleResolveRef(c *gin.Context) {
	proxyPostJSON(c, agentURL+"/api/dsl/resolve-ref")
}

// ============================================================================
// Entity Handlers
// ============================================================================

func handleListCbus(c *gin.Context) {
	proxyGet(c, agentURL+"/api/cbu")
}

func handleCbuGraph(c *gin.Context) {
	id := c.Param("id")
	proxyGet(c, agentURL+"/api/cbu/"+id+"/graph")
}

func handleEntitySearchForFinder(c *gin.Context) {
	var req struct {
		EntityType string `json:"entity_type"`
		Query      string `json:"query"`
		Limit      int    `json:"limit,omitempty"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": "Invalid JSON: " + err.Error()})
		return
	}

	// Build query params for Rust GET endpoint
	limit := req.Limit
	if limit == 0 {
		limit = 10
	}
	searchURL := fmt.Sprintf("%s/api/entity/search?type=%s&q=%s&limit=%d",
		agentURL,
		url.QueryEscape(req.EntityType),
		url.QueryEscape(req.Query),
		limit)

	resp, err := http.Get(searchURL)
	if err != nil {
		c.JSON(503, gin.H{"error": "Agent API connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}

	// Transform response to match JS-expected format
	var rustResp struct {
		Matches []struct {
			Token   string  `json:"token"`
			Display string  `json:"display"`
			Score   float64 `json:"score"`
		} `json:"matches"`
		Total     int  `json:"total"`
		Truncated bool `json:"truncated"`
	}
	if err := json.Unmarshal(respBody, &rustResp); err != nil {
		// If parsing fails, pass through as-is
		c.Data(resp.StatusCode, "application/json", respBody)
		return
	}

	// Transform to JS-expected EntitySearchResult format
	type JSResult struct {
		EntityID       string  `json:"entity_id"`
		Name           string  `json:"name"`
		EntityType     string  `json:"entity_type"`
		EntityTypeCode *string `json:"entity_type_code"`
		Jurisdiction   *string `json:"jurisdiction"`
		Similarity     float64 `json:"similarity"`
	}
	results := make([]JSResult, 0, len(rustResp.Matches))
	for _, m := range rustResp.Matches {
		results = append(results, JSResult{
			EntityID:       m.Token,
			Name:           m.Display,
			EntityType:     req.EntityType,
			EntityTypeCode: nil,
			Jurisdiction:   nil,
			Similarity:     m.Score,
		})
	}

	c.JSON(200, gin.H{
		"results": results,
		"total":   rustResp.Total,
	})
}

func handleEntitySearch(c *gin.Context) {
	var req struct {
		Query        string `json:"query"`
		Limit        int    `json:"limit,omitempty"`
		Jurisdiction string `json:"jurisdiction,omitempty"`
		EntityType   string `json:"entity_type,omitempty"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": err.Error()})
		return
	}

	// Build query params for Rust GET endpoint
	limit := req.Limit
	if limit == 0 {
		limit = 10
	}
	searchURL := fmt.Sprintf("%s/api/entities/search?q=%s&limit=%d",
		rustURL,
		url.QueryEscape(req.Query),
		limit)
	if req.EntityType != "" {
		searchURL += "&entity_type=" + url.QueryEscape(req.EntityType)
	}

	resp, err := http.Get(searchURL)
	if err != nil {
		c.JSON(503, gin.H{"error": "DSL API connection failed: " + err.Error()})
		return
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		c.JSON(502, gin.H{"error": "Failed to read response: " + err.Error()})
		return
	}

	// Transform response to match expected JS format
	var rustResp struct {
		Results []struct {
			ID      *string `json:"id"`
			Token   string  `json:"token"`
			Display string  `json:"display"`
			Score   float64 `json:"score"`
		} `json:"results"`
		Total int `json:"total"`
	}
	if err := json.Unmarshal(respBody, &rustResp); err != nil {
		c.JSON(502, gin.H{"error": "Invalid JSON from DSL API: " + err.Error()})
		return
	}

	// Transform to JS-expected format
	type JSResult struct {
		EntityID       string  `json:"entity_id"`
		Name           string  `json:"name"`
		EntityType     string  `json:"entity_type"`
		EntityTypeCode *string `json:"entity_type_code"`
		Jurisdiction   *string `json:"jurisdiction"`
		Similarity     float64 `json:"similarity"`
	}
	results := make([]JSResult, 0, len(rustResp.Results))
	for _, r := range rustResp.Results {
		entityID := r.Token
		if r.ID != nil {
			entityID = *r.ID
		}
		results = append(results, JSResult{
			EntityID:       entityID,
			Name:           r.Display,
			EntityType:     "entity",
			EntityTypeCode: nil,
			Jurisdiction:   nil,
			Similarity:     r.Score,
		})
	}

	c.JSON(200, gin.H{
		"results":       results,
		"create_option": fmt.Sprintf("Create new entity \"%s\"", req.Query),
	})
}

// ============================================================================
// KYC Handlers
// ============================================================================

func handleKycCase(c *gin.Context) {
	caseID := c.Param("id")
	if caseID == "" {
		c.JSON(400, gin.H{"error": "Case ID required"})
		return
	}
	proxyGet(c, rustURL+"/query/kyc/cases/"+caseID)
}
