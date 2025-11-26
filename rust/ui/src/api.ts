import type {
  CreateSessionRequest,
  CreateSessionResponse,
  ChatRequest,
  ChatResponse,
  SessionStateResponse,
  ExecuteRequest,
  ExecuteResponse,
} from './types';

const API_BASE = window.location.origin;

class ApiError extends Error {
  constructor(
    public status: number,
    message: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown
): Promise<T> {
  const options: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };

  if (body !== undefined) {
    options.body = JSON.stringify(body);
  }

  const response = await fetch(`${API_BASE}${path}`, options);

  if (!response.ok) {
    throw new ApiError(response.status, `HTTP ${response.status}`);
  }

  // Handle empty responses (like DELETE)
  const text = await response.text();
  if (!text) {
    return undefined as T;
  }

  return JSON.parse(text);
}

export const api = {
  /** Create a new agent session */
  createSession(req: CreateSessionRequest = {}): Promise<CreateSessionResponse> {
    return request('POST', '/api/session', req);
  },

  /** Get current session state */
  getSession(sessionId: string): Promise<SessionStateResponse> {
    return request('GET', `/api/session/${sessionId}`);
  },

  /** Delete a session */
  deleteSession(sessionId: string): Promise<void> {
    return request('DELETE', `/api/session/${sessionId}`);
  },

  /** Send a chat message and extract intents */
  chat(sessionId: string, req: ChatRequest): Promise<ChatResponse> {
    return request('POST', `/api/session/${sessionId}/chat`, req);
  },

  /** Execute accumulated DSL */
  execute(sessionId: string, req: ExecuteRequest = {}): Promise<ExecuteResponse> {
    return request('POST', `/api/session/${sessionId}/execute`, req);
  },

  /** Clear accumulated DSL */
  clear(sessionId: string): Promise<SessionStateResponse> {
    return request('POST', `/api/session/${sessionId}/clear`);
  },
};
