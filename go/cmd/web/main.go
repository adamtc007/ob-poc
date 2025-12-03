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
	flag.StringVar(&rustURL, "rust-url", "http://localhost:3000", "Rust API URL")
	flag.Parse()

	client = rustclient.NewClient(rustURL)

	var err error
	tmpl, err = template.ParseFS(templates, "templates/*.html")
	if err != nil {
		log.Fatalf("parsing templates: %v", err)
	}

	http.HandleFunc("/", handleIndex)
	http.HandleFunc("/health", handleHealth)
	http.HandleFunc("/api/run", handleRunSuite)
	http.HandleFunc("/api/validate", handleValidate)
	http.Handle("/static/", http.FileServer(http.FS(static)))

	log.Printf("Starting server on %s (Rust API: %s)", *addr, rustURL)
	log.Fatal(http.ListenAndServe(*addr, nil))
}

func handleIndex(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	health, _ := client.Health(ctx)
	domains, _ := client.ListDomains(ctx)

	data := map[string]any{
		"Health":  health,
		"Domains": domains,
		"RustURL": rustURL,
	}
	if err := tmpl.ExecuteTemplate(w, "index.html", data); err != nil {
		http.Error(w, err.Error(), 500)
	}
}

func handleHealth(w http.ResponseWriter, r *http.Request) {
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
