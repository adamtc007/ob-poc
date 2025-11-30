// DSL Viewer - TypeScript Types
// Defines the data structures for visualizing persisted agent-generated DSL

/**
 * DSL Instance summary for listing
 */
export interface DslInstance {
  instanceId: string;
  businessReference: string;
  domainName: string;
  currentVersion: number;
  status: string;
  updatedAt: string | null;
}

/**
 * Execution step info for the execution plan display
 */
export interface ExecutionStepInfo {
  step: number;
  verb: string;
  bindAs: string | null;
  injections: string[];
}

/**
 * Full DSL display data for a specific version
 */
export interface DslDisplayData {
  businessReference: string;
  domainName: string;
  version: number;
  dslSource: string;
  astJson: object | null;
  executionPlan: ExecutionStepInfo[];
  compilationStatus: string;
  createdAt: string | null;
}

/**
 * Version history entry
 */
export interface DslVersionInfo {
  version: number;
  operationType: string;
  compilationStatus: string;
  createdAt: string | null;
}

/**
 * Application state for the DSL Viewer
 */
export interface AppState {
  instances: DslInstance[];
  selectedInstance: string | null; // business_reference
  selectedVersion: number | null;
  displayData: DslDisplayData | null;
  versionHistory: DslVersionInfo[];
  loading: boolean;
  error: string | null;
}

/**
 * UI Configuration options
 */
export interface UiConfig {
  fontSize: number;
  showLineNumbers: boolean;
  theme: "light" | "dark";
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
  | { type: "LOAD_INSTANCES"; payload: DslInstance[] }
  | { type: "SELECT_INSTANCE"; payload: string }
  | { type: "SELECT_VERSION"; payload: number }
  | { type: "LOAD_DSL_START" }
  | { type: "LOAD_DSL_SUCCESS"; payload: DslDisplayData }
  | { type: "LOAD_HISTORY_SUCCESS"; payload: DslVersionInfo[] }
  | { type: "LOAD_DSL_ERROR"; payload: string }
  | { type: "UPDATE_UI_CONFIG"; payload: Partial<UiConfig> }
  | { type: "CLEAR_ERROR" }
  | { type: "REFRESH" };

/**
 * DSL Viewer API interface
 */
export interface DslViewerApi {
  listInstances(): Promise<DslInstance[]>;
  showDsl(businessRef: string, version?: number): Promise<DslDisplayData>;
  getHistory(businessRef: string): Promise<DslVersionInfo[]>;
}

/**
 * Component props interfaces
 */
export interface InstanceSelectorProps {
  instances: DslInstance[];
  selectedInstance: string | null;
  selectedVersion: number | null;
  versionHistory: DslVersionInfo[];
  onInstanceChange: (businessRef: string) => void;
  onVersionChange: (version: number) => void;
  loading: boolean;
}

export interface DslSourcePanelProps {
  dslSource: string;
  config: UiConfig;
}

export interface ExecutionPlanPanelProps {
  executionPlan: ExecutionStepInfo[];
  config: UiConfig;
}

export interface StatusBarProps {
  loading: boolean;
  error: string | null;
  displayData: DslDisplayData | null;
}

/**
 * Error types for better error handling
 */
export class DslApiError extends Error {
  constructor(
    message: string,
    public readonly status?: number,
    public readonly code?: string,
  ) {
    super(message);
    this.name = "DslApiError";
  }
}

/**
 * Constants for default values
 */
export const DEFAULT_UI_CONFIG: UiConfig = {
  fontSize: 14,
  showLineNumbers: true,
  theme: "light",
  splitRatio: 0.5,
};

export const DEFAULT_API_CONFIG: ApiConfig = {
  baseUrl: "http://localhost:3000/api/dsl",
  timeout: 10000,
  retryAttempts: 3,
};

export const DEFAULT_APP_STATE: AppState = {
  instances: [],
  selectedInstance: null,
  selectedVersion: null,
  displayData: null,
  versionHistory: [],
  loading: false,
  error: null,
};
