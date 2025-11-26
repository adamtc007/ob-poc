import type {
  AppState,
  SessionState,
  VerbIntent,
  IntentValidation,
  ChatMessage,
} from './types';

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

export function setSession(sessionId: string, sessionState: SessionState): void {
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

export function addMessage(role: 'user' | 'agent', content: string): void {
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
  canExecute: boolean
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
