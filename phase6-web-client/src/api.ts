// Phase 6: DSL/AST Visualization - API Client
// TypeScript API client for communicating with the DSL Manager backend

import {
  DslDomain,
  DslRequest,
  DslResponse,
  DslManagerApi,
  DslApiError,
  ApiConfig,
  DEFAULT_API_CONFIG
} from './types';

/**
 * HTTP API client for the DSL Manager backend
 * Handles all communication with the Rust backend server
 */
export class DslManagerApiClient implements DslManagerApi {
  private config: ApiConfig;
  private abortController?: AbortController;

  constructor(config: Partial<ApiConfig> = {}) {
    this.config = { ...DEFAULT_API_CONFIG, ...config };
  }

  /**
   * Fetch available DSL domains from the backend
   */
  async getDomains(): Promise<DslDomain[]> {
    const url = `${this.config.baseUrl}/domains`;

    try {
      const response = await this.fetchWithRetry(url, {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
          'Content-Type': 'application/json'
        }
      });

      if (!response.ok) {
        throw new DslApiError(
          `Failed to fetch domains: ${response.statusText}`,
          response.status
        );
      }

      const domains = await response.json() as DslDomain[];
      return this.validateDomainsResponse(domains);
    } catch (error) {
      if (error instanceof DslApiError) {
        throw error;
      }
      throw new DslApiError(
        `Network error while fetching domains: ${error instanceof Error ? error.message : 'Unknown error'}`,
        0,
        'NETWORK_ERROR'
      );
    }
  }

  /**
   * Fetch DSL source and generated AST for a specific domain and version
   */
  async getDslAndAst(request: DslRequest): Promise<DslResponse> {
    this.validateRequest(request);

    const url = new URL(`${this.config.baseUrl}/dsl`);
    url.searchParams.set('domain', request.domain);
    url.searchParams.set('version', request.version);

    try {
      const response = await this.fetchWithRetry(url.toString(), {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
          'Content-Type': 'application/json'
        }
      });

      if (!response.ok) {
        throw new DslApiError(
          `Failed to fetch DSL content: ${response.statusText}`,
          response.status
        );
      }

      const dslResponse = await response.json() as DslResponse;
      return this.validateDslResponse(dslResponse);
    } catch (error) {
      if (error instanceof DslApiError) {
        throw error;
      }
      throw new DslApiError(
        `Network error while fetching DSL content: ${error instanceof Error ? error.message : 'Unknown error'}`,
        0,
        'NETWORK_ERROR'
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
      const url = `${this.config.baseUrl}/health`;
      const response = await fetch(url, {
        method: 'GET',
        signal: AbortSignal.timeout(5000) // 5 second timeout
      });
      return response.ok;
    } catch {
      return false;
    }
  }

  /**
   * Fetch with retry logic and timeout handling
   */
  private async fetchWithRetry(url: string, options: RequestInit): Promise<Response> {
    this.abortController = new AbortController();
    const timeoutId = setTimeout(() => this.abortController?.abort(), this.config.timeout);

    const fetchOptions: RequestInit = {
      ...options,
      signal: this.abortController.signal
    };

    let lastError: Error | null = null;

    for (let attempt = 0; attempt < this.config.retryAttempts; attempt++) {
      try {
        const response = await fetch(url, fetchOptions);
        clearTimeout(timeoutId);
        return response;
      } catch (error) {
        lastError = error instanceof Error ? error : new Error('Unknown fetch error');

        // Don't retry on abort signal (user cancelled or timeout)
        if (error instanceof DOMException && error.name === 'AbortError') {
          throw new DslApiError('Request was cancelled or timed out', 0, 'TIMEOUT');
        }

        // Wait before retrying (exponential backoff)
        if (attempt < this.config.retryAttempts - 1) {
          await this.delay(Math.pow(2, attempt) * 1000);
        }
      }
    }

    clearTimeout(timeoutId);
    throw lastError || new Error('All retry attempts failed');
  }

  /**
   * Validate domains API response
   */
  private validateDomainsResponse(domains: any): DslDomain[] {
    if (!Array.isArray(domains)) {
      throw new DslApiError('Invalid domains response: expected array', 0, 'VALIDATION_ERROR');
    }

    return domains.map((domain, index) => {
      if (!domain.name || typeof domain.name !== 'string') {
        throw new DslApiError(`Invalid domain at index ${index}: missing or invalid name`, 0, 'VALIDATION_ERROR');
      }

      if (!domain.description || typeof domain.description !== 'string') {
        throw new DslApiError(`Invalid domain at index ${index}: missing or invalid description`, 0, 'VALIDATION_ERROR');
      }

      if (!Array.isArray(domain.versions)) {
        throw new DslApiError(`Invalid domain at index ${index}: versions must be an array`, 0, 'VALIDATION_ERROR');
      }

      const validatedVersions = domain.versions.map((version: any, vIndex: number) => {
        if (!version.version || typeof version.version !== 'string') {
          throw new DslApiError(`Invalid version at domain ${index}, version ${vIndex}: missing or invalid version`, 0, 'VALIDATION_ERROR');
        }

        return {
          version: version.version,
          description: version.description || '',
          exampleCount: typeof version.exampleCount === 'number' ? version.exampleCount : 0,
          lastUpdated: version.lastUpdated || new Date().toISOString()
        };
      });

      return {
        name: domain.name,
        description: domain.description,
        versions: validatedVersions
      };
    });
  }

  /**
   * Validate DSL response
   */
  private validateDslResponse(response: any): DslResponse {
    if (typeof response !== 'object' || response === null) {
      throw new DslApiError('Invalid DSL response: expected object', 0, 'VALIDATION_ERROR');
    }

    if (typeof response.dslSource !== 'string') {
      throw new DslApiError('Invalid DSL response: dslSource must be a string', 0, 'VALIDATION_ERROR');
    }

    if (typeof response.astText !== 'string') {
      throw new DslApiError('Invalid DSL response: astText must be a string', 0, 'VALIDATION_ERROR');
    }

    if (typeof response.domain !== 'string') {
      throw new DslApiError('Invalid DSL response: domain must be a string', 0, 'VALIDATION_ERROR');
    }

    if (typeof response.version !== 'string') {
      throw new DslApiError('Invalid DSL response: version must be a string', 0, 'VALIDATION_ERROR');
    }

    return {
      dslSource: response.dslSource,
      astText: response.astText,
      domain: response.domain,
      version: response.version,
      metadata: {
        parseTime: response.metadata?.parseTime || undefined,
        verbCount: response.metadata?.verbCount || 0,
        commentCount: response.metadata?.commentCount || 0,
        domainCount: response.metadata?.domainCount || 1,
        lastGenerated: response.metadata?.lastGenerated || new Date().toISOString()
      }
    };
  }

  /**
   * Validate request parameters
   */
  private validateRequest(request: DslRequest): void {
    if (!request.domain || typeof request.domain !== 'string') {
      throw new DslApiError('Invalid request: domain is required and must be a string', 0, 'VALIDATION_ERROR');
    }

    if (!request.version || typeof request.version !== 'string') {
      throw new DslApiError('Invalid request: version is required and must be a string', 0, 'VALIDATION_ERROR');
    }
  }

  /**
   * Utility method for delays
   */
  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

/**
 * Mock API client for development and testing
 * Provides sample data without requiring a backend server
 */
export class MockDslManagerApiClient implements DslManagerApi {
  private domains: DslDomain[] = [
    {
      name: 'Document',
      description: 'Document management and verification workflows',
      versions: [
        {
          version: 'v3.1',
          description: 'Latest document DSL with enhanced verification',
          exampleCount: 5,
          lastUpdated: '2024-11-22'
        },
        {
          version: 'v3.0',
          description: 'Stable document DSL with basic operations',
          exampleCount: 3,
          lastUpdated: '2024-10-15'
        }
      ]
    },
    {
      name: 'ISDA',
      description: 'Derivative trading and ISDA documentation workflows',
      versions: [
        {
          version: 'v3.1',
          description: 'Complete ISDA derivative lifecycle support',
          exampleCount: 8,
          lastUpdated: '2024-11-22'
        }
      ]
    },
    {
      name: 'Multi-Domain',
      description: 'Cross-domain integrated workflows',
      versions: [
        {
          version: 'v3.1',
          description: 'Unified multi-domain workflow examples',
          exampleCount: 3,
          lastUpdated: '2024-11-22'
        }
      ]
    }
  ];

  async getDomains(): Promise<DslDomain[]> {
    // Simulate network delay
    await this.delay(300);
    return [...this.domains];
  }

  async getDslAndAst(request: DslRequest): Promise<DslResponse> {
    // Simulate network delay
    await this.delay(500);

    const { dslSource, astText } = this.getMockContent(request.domain, request.version);

    return {
      dslSource,
      astText,
      domain: request.domain,
      version: request.version,
      metadata: {
        parseTime: Math.random() * 1000,
        verbCount: (dslSource.match(/^\(/gm) || []).length,
        commentCount: (dslSource.match(/^;;/gm) || []).length,
        domainCount: new Set(dslSource.match(/\w+\./g) || []).size,
        lastGenerated: new Date().toISOString()
      }
    };
  }

  private getMockContent(domain: string, version: string): { dslSource: string; astText: string } {
    const examples = {
      'Document': {
        dslSource: `;; Document Management Workflow
;; Phase 6 mock example

(document.catalog
  :document-id "doc-mock-001"
  :document-type "CONTRACT"
  :issuer "mock-authority"
  :title "Phase 6 Mock Demo"
  :parties ["party-a" "party-b"]
  :jurisdiction "US")

(document.verify
  :document-id "doc-mock-001"
  :verification-method "DIGITAL_SIGNATURE"
  :verification-result "AUTHENTIC")`,

        astText: `Program {
  forms: [
    Comment(" Document Management Workflow"),
    Comment(" Phase 6 mock example"),
    Verb(VerbForm {
      verb: "document.catalog",
      pairs: {
        Key("document-id"): Literal(String("doc-mock-001")),
        Key("document-type"): Literal(String("CONTRACT")),
        Key("issuer"): Literal(String("mock-authority")),
        Key("title"): Literal(String("Phase 6 Mock Demo")),
        Key("parties"): List([
          Literal(String("party-a")),
          Literal(String("party-b"))
        ]),
        Key("jurisdiction"): Literal(String("US"))
      }
    }),
    Verb(VerbForm {
      verb: "document.verify",
      pairs: {
        Key("document-id"): Literal(String("doc-mock-001")),
        Key("verification-method"): Literal(String("DIGITAL_SIGNATURE")),
        Key("verification-result"): Literal(String("AUTHENTIC"))
      }
    })
  ]
}`
      },
      'ISDA': {
        dslSource: `;; ISDA Derivative Workflow
;; Mock ISDA example

(isda.establish_master
  :agreement-id "ISDA-MOCK-001"
  :party-a "bank-mock"
  :party-b "fund-mock"
  :version "2002")

(isda.execute_trade
  :trade-id "TRADE-MOCK-001"
  :product-type "IRS"
  :notional-amount 50000000.0)`,

        astText: `Program {
  forms: [
    Comment(" ISDA Derivative Workflow"),
    Comment(" Mock ISDA example"),
    Verb(VerbForm {
      verb: "isda.establish_master",
      pairs: {
        Key("agreement-id"): Literal(String("ISDA-MOCK-001")),
        Key("party-a"): Literal(String("bank-mock")),
        Key("party-b"): Literal(String("fund-mock")),
        Key("version"): Literal(String("2002"))
      }
    }),
    Verb(VerbForm {
      verb: "isda.execute_trade",
      pairs: {
        Key("trade-id"): Literal(String("TRADE-MOCK-001")),
        Key("product-type"): Literal(String("IRS")),
        Key("notional-amount"): Literal(Number(50000000.0))
      }
    })
  ]
}`
      },
      'Multi-Domain': {
        dslSource: `;; Multi-Domain Mock Workflow

(entity :id "mock-entity" :label "Company")
(document.catalog :document-id "mock-doc" :document-type "CONTRACT")
(kyc.verify :customer-id "mock-entity" :outcome "APPROVED")`,

        astText: `Program {
  forms: [
    Comment(" Multi-Domain Mock Workflow"),
    Verb(VerbForm {
      verb: "entity",
      pairs: {
        Key("id"): Literal(String("mock-entity")),
        Key("label"): Literal(String("Company"))
      }
    }),
    Verb(VerbForm {
      verb: "document.catalog",
      pairs: {
        Key("document-id"): Literal(String("mock-doc")),
        Key("document-type"): Literal(String("CONTRACT"))
      }
    }),
    Verb(VerbForm {
      verb: "kyc.verify",
      pairs: {
        Key("customer-id"): Literal(String("mock-entity")),
        Key("outcome"): Literal(String("APPROVED"))
      }
    })
  ]
}`
      }
    };

    return examples[domain as keyof typeof examples] || {
      dslSource: ';; No mock data available',
      astText: 'Program { forms: [] }'
    };
  }

  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

/**
 * Factory function to create the appropriate API client
 */
export function createApiClient(useMock: boolean = false, config?: Partial<ApiConfig>): DslManagerApi {
  if (useMock || !config?.baseUrl) {
    console.log('🔧 Using Mock API Client for Phase 6 development');
    return new MockDslManagerApiClient();
  } else {
    console.log(`🌐 Using Real API Client - Backend: ${config.baseUrl}`);
    return new DslManagerApiClient(config);
  }
}
