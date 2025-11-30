// DSL Viewer - Main Application Entry Point
// TypeScript main application that initializes the web client

import { DslVisualizerApp } from "./app";
import { createApiClient } from "./api";
import { DEFAULT_API_CONFIG, DEFAULT_UI_CONFIG } from "./types";

/**
 * Application configuration
 */
interface AppConfig {
  apiBaseUrl?: string;
  useMockApi?: boolean;
  enableDebugMode?: boolean;
}

/**
 * Initialize and start the DSL Viewer application
 */
async function initializeApp(): Promise<void> {
  console.log("DSL Viewer - Starting...");

  // Get configuration from environment or use defaults
  const config: AppConfig = {
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL || DEFAULT_API_CONFIG.baseUrl,
    useMockApi: import.meta.env.VITE_USE_MOCK_API === "true",
    enableDebugMode: import.meta.env.DEV || false,
  };

  // Enable debug logging in development
  if (config.enableDebugMode) {
    console.log("Debug mode enabled");
    console.log("Configuration:", config);
  }

  try {
    // Create API client
    const apiClient = createApiClient(config.useMockApi, {
      baseUrl: config.apiBaseUrl!,
      timeout: 10000,
      retryAttempts: 3,
    });

    // Load saved UI config from localStorage
    const savedConfig = localStorage.getItem("dsl-viewer-config");
    const uiConfig = savedConfig
      ? { ...DEFAULT_UI_CONFIG, ...JSON.parse(savedConfig) }
      : DEFAULT_UI_CONFIG;

    // Initialize the main application
    const app = new DslVisualizerApp(apiClient, uiConfig);

    // Mount the application to the DOM
    const appElement = document.getElementById("app");
    if (!appElement) {
      throw new Error(
        "Application mount point not found - missing #app element",
      );
    }

    await app.mount(appElement);
    console.log("DSL Viewer initialized successfully");

    // Set up global error handling
    window.addEventListener("unhandledrejection", (event) => {
      console.error("Unhandled promise rejection:", event.reason);
      app.handleGlobalError(event.reason);
    });

    window.addEventListener("error", (event) => {
      console.error("Global error:", event.error);
      app.handleGlobalError(event.error);
    });

    // Set up graceful shutdown
    window.addEventListener("beforeunload", () => {
      app.cleanup();
    });
  } catch (error) {
    console.error("Failed to initialize DSL Viewer:", error);
    showInitializationError(error);
  }
}

/**
 * Show a user-friendly error message when the app fails to initialize
 */
function showInitializationError(error: unknown): void {
  const appElement = document.getElementById("app");
  if (appElement) {
    const errorMessage =
      error instanceof Error ? error.message : "Unknown error occurred";
    appElement.innerHTML = `
      <div style="
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100vh;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        text-align: center;
        padding: 2rem;
        background: #1a1a2e;
        color: #eee;
      ">
        <h1 style="font-size: 2rem; margin-bottom: 1rem; color: #e94560;">Application Failed to Start</h1>
        <p style="font-size: 1.1rem; margin-bottom: 2rem; max-width: 600px; line-height: 1.5;">
          The DSL Viewer encountered an error during initialization.
        </p>
        <div style="
          background: #16213e;
          padding: 1rem;
          border-radius: 8px;
          font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
          font-size: 0.9rem;
          max-width: 800px;
          word-break: break-word;
          border: 1px solid #0f3460;
        ">
          ${errorMessage}
        </div>
        <button onclick="window.location.reload()" style="
          margin-top: 2rem;
          padding: 0.75rem 2rem;
          background: #0f3460;
          border: 1px solid #e94560;
          border-radius: 4px;
          color: white;
          font-size: 1rem;
          cursor: pointer;
        ">
          Reload Application
        </button>
      </div>
    `;
  }
}

/**
 * Display loading screen while the application initializes
 */
function showLoadingScreen(): void {
  const appElement = document.getElementById("app");
  if (appElement) {
    appElement.innerHTML = `
      <div style="
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100vh;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #1a1a2e;
        color: #eee;
      ">
        <div style="
          width: 50px;
          height: 50px;
          border: 4px solid #0f3460;
          border-radius: 50%;
          border-top-color: #e94560;
          animation: spin 1s ease-in-out infinite;
          margin-bottom: 2rem;
        "></div>
        <h1 style="font-size: 1.5rem; margin-bottom: 0.5rem; color: #e94560;">DSL Viewer</h1>
        <p style="font-size: 1rem; opacity: 0.7;">Loading...</p>
        <style>
          @keyframes spin {
            to { transform: rotate(360deg); }
          }
        </style>
      </div>
    `;
  }
}

// Show loading screen immediately
showLoadingScreen();

// Wait for DOM to be ready, then initialize the app
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeApp);
} else {
  setTimeout(initializeApp, 100);
}
