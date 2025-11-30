// DSL Viewer - API Client
// TypeScript API client for communicating with the DSL Viewer backend

import {
  DslInstance,
  DslDisplayData,
  DslVersionInfo,
  DslViewerApi,
  DslApiError,
  ApiConfig,
  DEFAULT_API_CONFIG,
} from "./types";

/**
 * API response types (snake_case from backend)
 */
interface ListResponse {
  instances: Array<{
    instance_id: string;
    business_reference: string;
    domain_name: string;
    current_version: number;
    status: string;
    updated_at: string | null;
  }>;
  total: number;
}

interface ShowResponse {
  business_reference: string;
  domain_name: string;
  version: number;
  dsl_source: string;
  ast_json: object | null;
  execution_plan: Array<{
    step: number;
    verb: string;
    bind_as: string | null;
    injections: string[];
  }>;
  compilation_status: string;
  created_at: string | null;
}

interface HistoryResponse {
  business_reference: string;
  versions: Array<{
    version: number;
    operation_type: string;
    compilation_status: string;
    created_at: string | null;
  }>;
}

/**
 * HTTP API client for the DSL Viewer backend
 */
export class DslViewerApiClient implements DslViewerApi {
  private config: ApiConfig;
  private abortController?: AbortController;

  constructor(config: Partial<ApiConfig> = {}) {
    this.config = { ...DEFAULT_API_CONFIG, ...config };
  }

  /**
   * List all DSL instances
   */
  async listInstances(): Promise<DslInstance[]> {
    const url = `${this.config.baseUrl}/list`;

    try {
      const response = await this.fetchWithRetry(url, {
        method: "GET",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new DslApiError(
          `Failed to list instances: ${response.statusText}`,
          response.status,
        );
      }

      const data = (await response.json()) as ListResponse;
      return data.instances.map((inst) => ({
        instanceId: inst.instance_id,
        businessReference: inst.business_reference,
        domainName: inst.domain_name,
        currentVersion: inst.current_version,
        status: inst.status,
        updatedAt: inst.updated_at,
      }));
    } catch (error) {
      if (error instanceof DslApiError) {
        throw error;
      }
      throw new DslApiError(
        `Network error while listing instances: ${error instanceof Error ? error.message : "Unknown error"}`,
        0,
        "NETWORK_ERROR",
      );
    }
  }

  /**
   * Get DSL for display (optionally with specific version)
   */
  async showDsl(
    businessRef: string,
    version?: number,
  ): Promise<DslDisplayData> {
    const url = version
      ? `${this.config.baseUrl}/show/${encodeURIComponent(businessRef)}/${version}`
      : `${this.config.baseUrl}/show/${encodeURIComponent(businessRef)}`;

    try {
      const response = await this.fetchWithRetry(url, {
        method: "GET",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        if (response.status === 404) {
          throw new DslApiError(
            `DSL instance not found: ${businessRef}`,
            404,
            "NOT_FOUND",
          );
        }
        throw new DslApiError(
          `Failed to fetch DSL: ${response.statusText}`,
          response.status,
        );
      }

      const data = (await response.json()) as ShowResponse;
      return {
        businessReference: data.business_reference,
        domainName: data.domain_name,
        version: data.version,
        dslSource: data.dsl_source,
        astJson: data.ast_json,
        executionPlan: data.execution_plan.map((step) => ({
          step: step.step,
          verb: step.verb,
          bindAs: step.bind_as,
          injections: step.injections,
        })),
        compilationStatus: data.compilation_status,
        createdAt: data.created_at,
      };
    } catch (error) {
      if (error instanceof DslApiError) {
        throw error;
      }
      throw new DslApiError(
        `Network error while fetching DSL: ${error instanceof Error ? error.message : "Unknown error"}`,
        0,
        "NETWORK_ERROR",
      );
    }
  }

  /**
   * Get version history for a business reference
   */
  async getHistory(businessRef: string): Promise<DslVersionInfo[]> {
    const url = `${this.config.baseUrl}/history/${encodeURIComponent(businessRef)}`;

    try {
      const response = await this.fetchWithRetry(url, {
        method: "GET",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        if (response.status === 404) {
          throw new DslApiError(
            `DSL instance not found: ${businessRef}`,
            404,
            "NOT_FOUND",
          );
        }
        throw new DslApiError(
          `Failed to fetch history: ${response.statusText}`,
          response.status,
        );
      }

      const data = (await response.json()) as HistoryResponse;
      return data.versions.map((v) => ({
        version: v.version,
        operationType: v.operation_type,
        compilationStatus: v.compilation_status,
        createdAt: v.created_at,
      }));
    } catch (error) {
      if (error instanceof DslApiError) {
        throw error;
      }
      throw new DslApiError(
        `Network error while fetching history: ${error instanceof Error ? error.message : "Unknown error"}`,
        0,
        "NETWORK_ERROR",
      );
    }
  }

  /**
   * Cancel any ongoing requests
   */
  cancelRequests(): void {
    if (this.abortController) {
      this.abortController.abort();
    }
  }

  /**
   * Update API configuration
   */
  updateConfig(newConfig: Partial<ApiConfig>): void {
    this.config = { ...this.config, ...newConfig };
  }

  /**
   * Check if the API backend is available
   */
  async healthCheck(): Promise<boolean> {
    try {
      await this.listInstances();
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Fetch with retry logic and timeout handling
   */
  private async fetchWithRetry(
    url: string,
    options: RequestInit,
  ): Promise<Response> {
    this.abortController = new AbortController();
    const timeoutId = setTimeout(
      () => this.abortController?.abort(),
      this.config.timeout,
    );

    const fetchOptions: RequestInit = {
      ...options,
      signal: this.abortController.signal,
    };

    let lastError: Error | null = null;

    for (let attempt = 0; attempt < this.config.retryAttempts; attempt++) {
      try {
        const response = await fetch(url, fetchOptions);
        clearTimeout(timeoutId);
        return response;
      } catch (error) {
        lastError =
          error instanceof Error ? error : new Error("Unknown fetch error");

        // Don't retry on abort signal (user cancelled or timeout)
        if (error instanceof DOMException && error.name === "AbortError") {
          throw new DslApiError(
            "Request was cancelled or timed out",
            0,
            "TIMEOUT",
          );
        }

        // Wait before retrying (exponential backoff)
        if (attempt < this.config.retryAttempts - 1) {
          await this.delay(Math.pow(2, attempt) * 1000);
        }
      }
    }

    clearTimeout(timeoutId);
    throw lastError || new Error("All retry attempts failed");
  }

  /**
   * Utility method for delays
   */
  private delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

/**
 * Mock API client for development and testing
 */
export class MockDslViewerApiClient implements DslViewerApi {
  private mockInstances: DslInstance[] = [
    {
      instanceId: "mock-uuid-1",
      businessReference: "test-hedge-fund-alpha",
      domainName: "cbu",
      currentVersion: 3,
      status: "ACTIVE",
      updatedAt: new Date().toISOString(),
    },
    {
      instanceId: "mock-uuid-2",
      businessReference: "onboarding-acme-corp",
      domainName: "cbu",
      currentVersion: 1,
      status: "ACTIVE",
      updatedAt: new Date(Date.now() - 86400000).toISOString(),
    },
  ];

  async listInstances(): Promise<DslInstance[]> {
    await this.delay(200);
    return [...this.mockInstances];
  }

  async showDsl(
    businessRef: string,
    version?: number,
  ): Promise<DslDisplayData> {
    await this.delay(300);

    const instance = this.mockInstances.find(
      (i) => i.businessReference === businessRef,
    );
    if (!instance) {
      throw new DslApiError(
        `Instance not found: ${businessRef}`,
        404,
        "NOT_FOUND",
      );
    }

    const ver = version ?? instance.currentVersion;

    return {
      businessReference: businessRef,
      domainName: instance.domainName,
      version: ver,
      dslSource: `;; DSL for ${businessRef} v${ver}
;; Generated by agent session

(cbu.create
  :name "Hedge Fund Alpha"
  :jurisdiction "LU"
  :client-type "fund"
  :as @fund)

(entity.create-proper-person
  :cbu-id @fund
  :first-name "John"
  :last-name "Smith"
  :date-of-birth "1980-01-15"
  :as @john)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @john
  :role "BENEFICIAL_OWNER"
  :ownership-percentage 60)

(document.catalog
  :cbu-id @fund
  :entity-id @john
  :document-type "PASSPORT")`,
      astJson: null,
      executionPlan: [
        { step: 0, verb: "cbu.create", bindAs: "@fund", injections: [] },
        {
          step: 1,
          verb: "entity.create-proper-person",
          bindAs: "@john",
          injections: ["cbu-id <- $0"],
        },
        {
          step: 2,
          verb: "cbu.assign-role",
          bindAs: null,
          injections: ["cbu-id <- $0", "entity-id <- $1"],
        },
        {
          step: 3,
          verb: "document.catalog",
          bindAs: null,
          injections: ["cbu-id <- $0", "entity-id <- $1"],
        },
      ],
      compilationStatus: "COMPILED",
      createdAt: new Date().toISOString(),
    };
  }

  async getHistory(businessRef: string): Promise<DslVersionInfo[]> {
    await this.delay(200);

    const instance = this.mockInstances.find(
      (i) => i.businessReference === businessRef,
    );
    if (!instance) {
      throw new DslApiError(
        `Instance not found: ${businessRef}`,
        404,
        "NOT_FOUND",
      );
    }

    const versions: DslVersionInfo[] = [];
    for (let v = 1; v <= instance.currentVersion; v++) {
      versions.push({
        version: v,
        operationType: v === 1 ? "CREATE" : "EXECUTE",
        compilationStatus: "COMPILED",
        createdAt: new Date(
          Date.now() - (instance.currentVersion - v) * 3600000,
        ).toISOString(),
      });
    }
    return versions;
  }

  private delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

/**
 * Factory function to create the appropriate API client
 */
export function createApiClient(
  useMock: boolean = false,
  config?: Partial<ApiConfig>,
): DslViewerApi {
  if (useMock) {
    console.log("Using Mock API Client for DSL Viewer development");
    return new MockDslViewerApiClient();
  } else {
    console.log(
      `Using Real API Client - Backend: ${config?.baseUrl || DEFAULT_API_CONFIG.baseUrl}`,
    );
    return new DslViewerApiClient(config);
  }
}
