// Web server for test harness UI.
package main

import (
	"context"
	"embed"
	"encoding/json"
	"flag"
	"html/template"
	"log"
	"net/http"

	"github.com/adamtc007/ob-poc/go/internal/harness"
	"github.com/adamtc007/ob-poc/go/internal/rustclient"
)

//go:embed templates/*.html
var templates embed.FS

//go:embed static/*
var static embed.FS

var (
	rustURL string
	client  *rustclient.Client
	tmpl    *template.Template
)

func main() {
	addr := flag.String("addr", ":8181", "Listen address")
	flag.StringVar(&rustURL, "rust-url", "", "Rust API URL (optional, default http://localhost:3001)")
	flag.Parse()

	if rustURL != "" {
		client = rustclient.NewClient(rustURL)
	}

	var err error
	tmpl, err = template.ParseFS(templates, "templates/*.html")
	if err != nil {
		log.Fatalf("parsing templates: %v", err)
	}

	http.HandleFunc("/", handleIndex)
	http.HandleFunc("/health", handleHealth)
	http.HandleFunc("/api/run", handleRunSuite)
	http.HandleFunc("/api/validate", handleValidate)
	http.HandleFunc("/api/config", handleConfig)
	http.Handle("/static/", http.FileServer(http.FS(static)))

	if rustURL != "" {
		log.Printf("Starting server on %s (Rust API: %s)", *addr, rustURL)
	} else {
		log.Printf("Starting server on %s (standalone mode - no Rust API)", *addr)
	}
	log.Fatal(http.ListenAndServe(*addr, nil))
}

func handleIndex(w http.ResponseWriter, r *http.Request) {
	var health *rustclient.HealthResponse
	var verbs *rustclient.VerbsResponse

	if client != nil {
		ctx := r.Context()
		health, _ = client.Health(ctx)
		verbs, _ = client.ListVerbs(ctx)
	}

	data := map[string]any{
		"Health":    health,
		"Verbs":     verbs,
		"RustURL":   rustURL,
		"Connected": client != nil && health != nil,
	}
	if err := tmpl.ExecuteTemplate(w, "index.html", data); err != nil {
		http.Error(w, err.Error(), 500)
	}
}

func handleConfig(w http.ResponseWriter, r *http.Request) {
	if r.Method == "POST" {
		var req struct {
			RustURL string `json:"rust_url"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			jsonError(w, err.Error(), 400)
			return
		}
		rustURL = req.RustURL
		if rustURL != "" {
			client = rustclient.NewClient(rustURL)
		} else {
			client = nil
		}
		jsonResponse(w, map[string]string{"status": "ok", "rust_url": rustURL})
		return
	}
	jsonResponse(w, map[string]any{"rust_url": rustURL, "connected": client != nil})
}

func handleHealth(w http.ResponseWriter, r *http.Request) {
	if client == nil {
		jsonError(w, "Rust API not configured", 503)
		return
	}
	ctx := r.Context()
	health, err := client.Health(ctx)
	if err != nil {
		jsonError(w, err.Error(), 503)
		return
	}
	jsonResponse(w, health)
}

func handleValidate(w http.ResponseWriter, r *http.Request) {
	if r.Method != "POST" {
		http.Error(w, "POST required", 405)
		return
	}
	if client == nil {
		jsonError(w, "Rust API not configured", 503)
		return
	}
	var req struct {
		DSL string `json:"dsl"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		jsonError(w, err.Error(), 400)
		return
	}

	result, err := client.ValidateDSL(r.Context(), req.DSL)
	if err != nil {
		jsonError(w, err.Error(), 500)
		return
	}
	jsonResponse(w, result)
}

func handleRunSuite(w http.ResponseWriter, r *http.Request) {
	if r.Method != "POST" {
		http.Error(w, "POST required", 405)
		return
	}
	if rustURL == "" {
		jsonError(w, "Rust API not configured", 503)
		return
	}

	var req struct {
		Cases []harness.Case `json:"cases"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		jsonError(w, err.Error(), 400)
		return
	}

	suite := harness.Suite{
		Name:  "Ad-hoc Suite",
		Cases: req.Cases,
	}

	runner := harness.NewRunner(rustURL)
	result, err := runner.Run(context.Background(), suite)
	if err != nil {
		jsonError(w, err.Error(), 500)
		return
	}
	jsonResponse(w, result)
}

func jsonResponse(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(v)
}

func jsonError(w http.ResponseWriter, msg string, code int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	json.NewEncoder(w).Encode(map[string]string{"error": msg})
}
