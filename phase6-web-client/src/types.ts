// Phase 6: DSL/AST Visualization - TypeScript Types
// Defines the data structures for DSL domains, versions, and AST representation

/**
 * Represents a DSL domain with its available versions
 */
export interface DslDomain {
  name: string;
  description: string;
  versions: DslVersion[];
}

/**
 * Represents a specific version of a DSL domain
 */
export interface DslVersion {
  version: string;
  description: string;
  exampleCount: number;
  lastUpdated: string;
}

/**
 * Request structure for fetching DSL and AST content
 */
export interface DslRequest {
  domain: string;
  version: string;
}

/**
 * Response structure containing DSL source and generated AST
 */
export interface DslResponse {
  dslSource: string;
  astText: string;
  domain: string;
  version: string;
  metadata: DslMetadata;
}

/**
 * Metadata about the DSL/AST content
 */
export interface DslMetadata {
  parseTime?: number;
  verbCount: number;
  commentCount: number;
  domainCount: number;
  lastGenerated: string;
}

/**
 * Application state for the DSL/AST visualizer
 */
export interface AppState {
  domains: DslDomain[];
  selectedDomain: string | null;
  selectedVersion: string | null;
  dslSource: string;
  astText: string;
  loading: boolean;
  error: string | null;
  lastUpdated: Date | null;
}

/**
 * UI Configuration options
 */
export interface UiConfig {
  fontSize: number;
  showLineNumbers: boolean;
  autoFormatAst: boolean;
  theme: 'light' | 'dark';
  splitRatio: number; // 0.0 to 1.0, represents left panel width ratio
}

/**
 * API client configuration
 */
export interface ApiConfig {
  baseUrl: string;
  timeout: number;
  retryAttempts: number;
}

/**
 * Event types for application state management
 */
export type AppEvent =
  | { type: 'LOAD_DOMAINS'; payload: DslDomain[] }
  | { type: 'SELECT_DOMAIN'; payload: string }
  | { type: 'SELECT_VERSION'; payload: string }
  | { type: 'LOAD_DSL_START' }
  | { type: 'LOAD_DSL_SUCCESS'; payload: DslResponse }
  | { type: 'LOAD_DSL_ERROR'; payload: string }
  | { type: 'UPDATE_UI_CONFIG'; payload: Partial<UiConfig> }
  | { type: 'CLEAR_ERROR' };

/**
 * DSL Manager API interface
 */
export interface DslManagerApi {
  getDomains(): Promise<DslDomain[]>;
  getDslAndAst(request: DslRequest): Promise<DslResponse>;
}

/**
 * Component props interfaces
 */
export interface DomainSelectorProps {
  domains: DslDomain[];
  selectedDomain: string | null;
  selectedVersion: string | null;
  onDomainChange: (domain: string) => void;
  onVersionChange: (version: string) => void;
  loading: boolean;
}

export interface CodeEditorProps {
  content: string;
  title: string;
  language: 'dsl' | 'ast';
  config: UiConfig;
  readonly?: boolean;
}

export interface StatusBarProps {
  loading: boolean;
  error: string | null;
  lastUpdated: Date | null;
  metadata: DslMetadata | null;
}

/**
 * Utility types for type safety
 */
export type DomainName = 'Document' | 'ISDA' | 'Multi-Domain' | 'KYC' | 'UBO';
export type VersionNumber = 'v3.1' | 'v3.0' | 'v2.1';

/**
 * Error types for better error handling
 */
export class DslApiError extends Error {
  constructor(
    message: string,
    public readonly status?: number,
    public readonly code?: string
  ) {
    super(message);
    this.name = 'DslApiError';
  }
}

export class ValidationError extends Error {
  constructor(
    message: string,
    public readonly field: string
  ) {
    super(message);
    this.name = 'ValidationError';
  }
}

/**
 * Constants for default values
 */
export const DEFAULT_UI_CONFIG: UiConfig = {
  fontSize: 14,
  showLineNumbers: true,
  autoFormatAst: true,
  theme: 'light',
  splitRatio: 0.5
};

export const DEFAULT_API_CONFIG: ApiConfig = {
  baseUrl: 'http://localhost:8080/api',
  timeout: 10000,
  retryAttempts: 3
};

export const DEFAULT_APP_STATE: AppState = {
  domains: [],
  selectedDomain: null,
  selectedVersion: null,
  dslSource: '',
  astText: '',
  loading: false,
  error: null,
  lastUpdated: null
};
