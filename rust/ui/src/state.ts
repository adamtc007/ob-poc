import type {
  AppState,
  SessionState,
  VerbIntent,
  IntentValidation,
  ChatMessage,
  TemplateSummary,
  FormTemplate,
} from "./types";

type Listener = (state: AppState) => void;

const initialState: AppState = {
  sessionId: null,
  sessionState: null,
  messages: [],
  intents: [],
  validations: [],
  assembledDsl: [],
  canExecute: false,
  loading: false,
  error: null,

  // Template state
  templates: [],
  selectedTemplateId: null,
  selectedTemplate: null,
  formValues: {},
  renderedDsl: null,
  executionLog: [],
};

let state: AppState = { ...initialState };
const listeners: Set<Listener> = new Set();

export function getState(): Readonly<AppState> {
  return state;
}

export function setState(updates: Partial<AppState>): void {
  state = { ...state, ...updates };
  listeners.forEach((fn) => fn(state));
}

export function resetState(): void {
  state = { ...initialState };
  listeners.forEach((fn) => fn(state));
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

// Convenience updaters
export function setLoading(loading: boolean): void {
  setState({ loading, error: null });
}

export function setError(error: string): void {
  setState({ error, loading: false });
}

export function setSession(
  sessionId: string,
  sessionState: SessionState,
): void {
  setState({
    sessionId,
    sessionState,
    messages: [],
    intents: [],
    validations: [],
    assembledDsl: [],
    canExecute: false,
    error: null,
  });
}

export function addMessage(role: "user" | "agent", content: string): void {
  const message: ChatMessage = {
    id: crypto.randomUUID(),
    role,
    content,
    timestamp: new Date().toISOString(),
  };
  setState({ messages: [...state.messages, message] });
}

export function updateFromChatResponse(
  intents: VerbIntent[],
  validations: IntentValidation[],
  assembledDsl: string[],
  sessionState: SessionState,
  canExecute: boolean,
): void {
  setState({
    intents,
    validations,
    assembledDsl,
    sessionState,
    canExecute,
    loading: false,
  });
}

export function clearDsl(): void {
  setState({
    intents: [],
    validations: [],
    assembledDsl: [],
    canExecute: false,
  });
}

// Template state management
export function setTemplates(templates: TemplateSummary[]): void {
  setState({ templates });
}

export function selectTemplate(template: FormTemplate | null): void {
  setState({
    selectedTemplateId: template?.id ?? null,
    selectedTemplate: template,
    formValues: template ? getDefaultValues(template) : {},
    renderedDsl: null,
  });
}

export function setFormValue(name: string, value: unknown): void {
  setState({
    formValues: { ...state.formValues, [name]: value },
  });
}

export function setRenderedDsl(dsl: string | null): void {
  setState({ renderedDsl: dsl });
}

export function addLogEntry(entry: string): void {
  setState({
    executionLog: [
      ...state.executionLog,
      `[${new Date().toLocaleTimeString()}] ${entry}`,
    ],
  });
}

function getDefaultValues(template: FormTemplate): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const slot of template.slots) {
    if (slot.default_value !== undefined) {
      values[slot.name] = slot.default_value;
    }
  }
  return values;
}
