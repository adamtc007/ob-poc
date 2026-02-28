/**
 * Chat Store
 *
 * State management for the Agent Chat UI using Zustand.
 */

import { create } from 'zustand';
import type {
  ChatSession,
  ChatSessionSummary,
  ChatMessage,
  DecisionPacket,
  VerbProfile,
} from '../types/chat';

/** Surface metadata from SessionVerbSurface */
export interface VerbSurfaceMeta {
  fingerprint: string;
  totalRegistry: number;
  finalCount: number;
}

/** Chat state */
interface ChatState {
  // Sessions
  sessions: ChatSessionSummary[];
  currentSession: ChatSession | null;
  isLoadingSessions: boolean;

  // Messages
  isStreaming: boolean;
  streamingContent: string;
  pendingDecision: DecisionPacket | null;

  // Input
  inputValue: string;

  // Available verbs (populated on every chat response)
  availableVerbs: VerbProfile[];

  // Verb surface metadata (fingerprint + counts)
  verbSurfaceMeta: VerbSurfaceMeta | null;

  // Errors
  error: string | null;
}

/** Chat actions */
interface ChatActions {
  // Sessions
  setSessions: (sessions: ChatSessionSummary[]) => void;
  setCurrentSession: (session: ChatSession | null) => void;
  setLoadingSessions: (loading: boolean) => void;

  // Messages
  addMessage: (message: ChatMessage) => void;
  updateMessage: (messageId: string, updates: Partial<ChatMessage>) => void;
  setStreaming: (streaming: boolean) => void;
  appendStreamingContent: (content: string) => void;
  clearStreamingContent: () => void;

  // Decision packets
  setPendingDecision: (decision: DecisionPacket | null) => void;

  // Available verbs
  setAvailableVerbs: (verbs: VerbProfile[], meta?: VerbSurfaceMeta) => void;

  // Input
  setInputValue: (value: string) => void;
  clearInput: () => void;

  // Errors
  setError: (error: string | null) => void;

  // Utils
  reset: () => void;
}

const initialState: ChatState = {
  sessions: [],
  currentSession: null,
  isLoadingSessions: false,
  isStreaming: false,
  streamingContent: '',
  pendingDecision: null,
  availableVerbs: [],
  verbSurfaceMeta: null,
  inputValue: '',
  error: null,
};

export const useChatStore = create<ChatState & ChatActions>((set, get) => ({
  ...initialState,

  setSessions: (sessions) => set({ sessions }),

  setCurrentSession: (currentSession) => set({
    currentSession,
    pendingDecision: null,
    streamingContent: '',
    isStreaming: false,
  }),

  setLoadingSessions: (isLoadingSessions) => set({ isLoadingSessions }),

  addMessage: (message) => {
    const { currentSession } = get();
    if (!currentSession) return;

    set({
      currentSession: {
        ...currentSession,
        messages: [...currentSession.messages, message],
      },
    });

    // Check for decision packet
    if (message.decision_packet) {
      set({ pendingDecision: message.decision_packet });
    }
  },

  updateMessage: (messageId, updates) => {
    const { currentSession } = get();
    if (!currentSession) return;

    set({
      currentSession: {
        ...currentSession,
        messages: currentSession.messages.map(m =>
          m.id === messageId ? { ...m, ...updates } : m
        ),
      },
    });
  },

  setStreaming: (isStreaming) => set({ isStreaming }),

  appendStreamingContent: (content) => {
    set(state => ({ streamingContent: state.streamingContent + content }));
  },

  clearStreamingContent: () => set({ streamingContent: '' }),

  setPendingDecision: (pendingDecision) => set({ pendingDecision }),

  setAvailableVerbs: (availableVerbs, meta) => set({ availableVerbs, verbSurfaceMeta: meta ?? null }),

  setInputValue: (inputValue) => set({ inputValue }),

  clearInput: () => set({ inputValue: '' }),

  setError: (error) => set({ error }),

  reset: () => set(initialState),
}));

export default useChatStore;
