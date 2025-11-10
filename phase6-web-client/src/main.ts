// Phase 6: DSL/AST Visualization - Main Application Entry Point
// TypeScript main application that initializes the web client

import { DslVisualizerApp } from './app';
import { createApiClient } from './api';
import { DEFAULT_API_CONFIG, DEFAULT_UI_CONFIG } from './types';

/**
 * Application configuration
 */
interface AppConfig {
  apiBaseUrl?: string;
  useMockApi?: boolean;
  enableDebugMode?: boolean;
}

/**
 * Initialize and start the Phase 6 DSL/AST Visualization application
 */
async function initializeApp(): Promise<void> {
  console.log('🚀 Phase 6: DSL/AST Visualization Client - Starting...');

  // Get configuration from environment or use defaults
  const config: AppConfig = {
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL || DEFAULT_API_CONFIG.baseUrl,
    useMockApi: import.meta.env.VITE_USE_MOCK_API === 'true' || !import.meta.env.VITE_API_BASE_URL,
    enableDebugMode: import.meta.env.DEV || false
  };

  // Enable debug logging in development
  if (config.enableDebugMode) {
    console.log('🔧 Debug mode enabled');
    console.log('📋 Configuration:', config);
  }

  try {
    // Create API client
    const apiClient = createApiClient(config.useMockApi, {
      baseUrl: config.apiBaseUrl!,
      timeout: 10000,
      retryAttempts: 3
    });

    // Test API connectivity if using real API
    if (!config.useMockApi) {
      console.log('🔍 Testing API connectivity...');
      const isHealthy = await apiClient.healthCheck?.();
      if (!isHealthy) {
        console.warn('⚠️ API health check failed, falling back to mock data');
        // Could fallback to mock client here if needed
      } else {
        console.log('✅ API connectivity confirmed');
      }
    }

    // Initialize the main application
    const app = new DslVisualizerApp(apiClient, {
      ...DEFAULT_UI_CONFIG,
      theme: (localStorage.getItem('dsl-visualizer-theme') as 'light' | 'dark') || 'light'
    });

    // Mount the application to the DOM
    const appElement = document.getElementById('app');
    if (!appElement) {
      throw new Error('Application mount point not found - missing #app element');
    }

    await app.mount(appElement);
    console.log('✅ Phase 6 application initialized successfully');

    // Set up global error handling
    window.addEventListener('unhandledrejection', (event) => {
      console.error('🚨 Unhandled promise rejection:', event.reason);
      app.handleGlobalError(event.reason);
    });

    window.addEventListener('error', (event) => {
      console.error('🚨 Global error:', event.error);
      app.handleGlobalError(event.error);
    });

    // Set up graceful shutdown
    window.addEventListener('beforeunload', () => {
      console.log('🛑 Application shutting down...');
      app.cleanup();
    });

    // Enable hot reload in development
    if (config.enableDebugMode && import.meta.hot) {
      import.meta.hot.accept('./app', (newModule) => {
        if (newModule) {
          console.log('🔄 Hot reloading application...');
          // Could implement hot reload logic here
        }
      });
    }

  } catch (error) {
    console.error('🚨 Failed to initialize Phase 6 application:', error);
    showInitializationError(error);
  }
}

/**
 * Show a user-friendly error message when the app fails to initialize
 */
function showInitializationError(error: unknown): void {
  const appElement = document.getElementById('app');
  if (appElement) {
    const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
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
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
      ">
        <h1 style="font-size: 2rem; margin-bottom: 1rem;">🚨 Application Failed to Start</h1>
        <p style="font-size: 1.1rem; margin-bottom: 2rem; max-width: 600px; line-height: 1.5;">
          The Phase 6 DSL/AST Visualization client encountered an error during initialization.
        </p>
        <div style="
          background: rgba(255, 255, 255, 0.1);
          padding: 1rem;
          border-radius: 8px;
          font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
          font-size: 0.9rem;
          max-width: 800px;
          word-break: break-word;
        ">
          ${errorMessage}
        </div>
        <button onclick="window.location.reload()" style="
          margin-top: 2rem;
          padding: 0.75rem 2rem;
          background: rgba(255, 255, 255, 0.2);
          border: none;
          border-radius: 4px;
          color: white;
          font-size: 1rem;
          cursor: pointer;
          transition: background-color 0.2s;
        " onmouseover="this.style.backgroundColor='rgba(255, 255, 255, 0.3)'"
           onmouseout="this.style.backgroundColor='rgba(255, 255, 255, 0.2)'">
          🔄 Reload Application
        </button>
      </div>
    `;
  }
}

/**
 * Display loading screen while the application initializes
 */
function showLoadingScreen(): void {
  const appElement = document.getElementById('app');
  if (appElement) {
    appElement.innerHTML = `
      <div style="
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100vh;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
      ">
        <div style="
          width: 60px;
          height: 60px;
          border: 4px solid rgba(255, 255, 255, 0.3);
          border-radius: 50%;
          border-top-color: white;
          animation: spin 1s ease-in-out infinite;
          margin-bottom: 2rem;
        "></div>
        <h1 style="font-size: 1.5rem; margin-bottom: 0.5rem;">🚀 Phase 6: DSL/AST Visualizer</h1>
        <p style="font-size: 1rem; opacity: 0.9;">Loading application...</p>
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
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initializeApp);
} else {
  // DOM is already ready
  setTimeout(initializeApp, 100); // Small delay to show loading screen
}

// Export for debugging in development
if (import.meta.env.DEV) {
  (window as any).Phase6Debug = {
    reinitialize: initializeApp,
    config: {
      apiBaseUrl: import.meta.env.VITE_API_BASE_URL,
      useMockApi: import.meta.env.VITE_USE_MOCK_API === 'true'
    }
  };
}
