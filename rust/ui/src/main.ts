import { api } from './api';
import {
  getState,
  subscribe,
  setSession,
  setLoading,
  setError,
  addMessage,
  updateFromChatResponse,
  clearDsl,
  setState,
} from './state';
import { render, showLoading, hideLoading, showError } from './ui';
import './style.css';

// ============================================================================
// Event Handlers
// ============================================================================

async function handleCreateSession(): Promise<void> {
  try {
    setLoading(true);
    const response = await api.createSession({});
    setSession(response.session_id, response.state);
    addMessage('agent', 'Session started. Describe what you want to create!');
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    setError(`Failed to create session: ${message}`);
    showError(`Failed to create session: ${message}`);
  } finally {
    setLoading(false);
  }
}

async function handleSendMessage(): Promise<void> {
  const state = getState();
  if (!state.sessionId) {
    alert('Create a session first');
    return;
  }

  const input = document.getElementById('message') as HTMLInputElement;
  const message = input.value.trim();
  if (!message) return;

  input.value = '';
  addMessage('user', message);
  showLoading();
  setLoading(true);

  try {
    const response = await api.chat(state.sessionId, { message });
    hideLoading();

    // Build agent response content
    let content = response.message;
    if (response.intents.length > 0) {
      const validCount = response.validation_results.filter((v) => v.valid).length;
      const totalCount = response.intents.length;
      const badgeClass = validCount === totalCount ? 'valid' : 'invalid';
      content += ` <span class="badge ${badgeClass}">${validCount}/${totalCount} valid</span>`;
    }
    addMessage('agent', content);

    updateFromChatResponse(
      response.intents,
      response.validation_results,
      response.assembled_dsl?.statements ?? [],
      response.session_state,
      response.can_execute
    );
  } catch (err) {
    hideLoading();
    const errorMsg = err instanceof Error ? err.message : 'Unknown error';
    setError(errorMsg);
    showError(errorMsg);
  }
}

async function handleExecuteDsl(): Promise<void> {
  const state = getState();
  if (!state.sessionId || !state.canExecute) return;

  setLoading(true);

  try {
    const response = await api.execute(state.sessionId, { dry_run: false });

    if (response.success) {
      addMessage(
        'agent',
        `<span class="success-message">Successfully executed ${response.results.length} DSL statement(s)</span>`
      );
      clearDsl();
    } else {
      const errors = response.errors.join('<br>');
      addMessage(
        'agent',
        `<span class="error-message">Execution failed:<br>${errors}</span>`
      );
    }

    setState({ sessionState: response.new_state });
    await refreshSessionState();
  } catch (err) {
    const errorMsg = err instanceof Error ? err.message : 'Unknown error';
    showError(`Execution error: ${errorMsg}`);
  } finally {
    setLoading(false);
  }
}

async function handleClearDsl(): Promise<void> {
  const state = getState();
  if (!state.sessionId) return;

  try {
    const response = await api.clear(state.sessionId);
    clearDsl();
    setState({ sessionState: response.state });
    addMessage('agent', 'DSL cleared.');
  } catch (err) {
    const errorMsg = err instanceof Error ? err.message : 'Unknown error';
    showError(`Error: ${errorMsg}`);
  }
}

async function refreshSessionState(): Promise<void> {
  const state = getState();
  if (!state.sessionId) return;

  try {
    const response = await api.getSession(state.sessionId);
    setState({
      assembledDsl: response.assembled_dsl,
      sessionState: response.state,
      canExecute: response.can_execute,
    });
  } catch (err) {
    console.error('Failed to refresh session state:', err);
  }
}

// ============================================================================
// Initialization
// ============================================================================

function bindEvents(): void {
  // New Session button
  const newSessionBtn = document.querySelector('.new-session-btn');
  if (newSessionBtn) {
    newSessionBtn.addEventListener('click', handleCreateSession);
  }

  // Send button
  const sendBtn = document.getElementById('send-btn');
  if (sendBtn) {
    sendBtn.addEventListener('click', handleSendMessage);
  }

  // Message input (Enter key)
  const messageInput = document.getElementById('message');
  if (messageInput) {
    messageInput.addEventListener('keypress', (e) => {
      if (e.key === 'Enter') handleSendMessage();
    });
  }

  // Execute button
  const executeBtn = document.getElementById('execute-btn');
  if (executeBtn) {
    executeBtn.addEventListener('click', handleExecuteDsl);
  }

  // Clear button
  const clearBtn = document.getElementById('clear-btn');
  if (clearBtn) {
    clearBtn.addEventListener('click', handleClearDsl);
  }
}

function init(): void {
  // Subscribe to state changes
  subscribe(render);

  // Bind event handlers
  bindEvents();

  // Initial render
  render(getState());

  // Focus message input
  const messageInput = document.getElementById('message');
  if (messageInput) messageInput.focus();
}

// Start the app
document.addEventListener('DOMContentLoaded', init);
