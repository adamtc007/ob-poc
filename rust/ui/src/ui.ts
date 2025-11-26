import type { AppState, VerbIntent, IntentValidation, ParamValue } from './types';

// ============================================================================
// DOM Helpers
// ============================================================================

function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Element not found: ${id}`);
  return el;
}

function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatParamValue(value: ParamValue): string {
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

// ============================================================================
// Component Renderers
// ============================================================================

export function renderSessionInfo(state: AppState): void {
  const infoEl = $('session-info');
  const stateEl = $('session-state');

  if (state.sessionId) {
    infoEl.textContent = `Session: ${state.sessionId.substring(0, 8)}...`;
    infoEl.className = 'session-info active';
  } else {
    infoEl.textContent = 'No active session';
    infoEl.className = 'session-info';
  }

  if (state.sessionState) {
    stateEl.textContent = state.sessionState.replace(/_/g, ' ');
    stateEl.className = `session-state ${state.sessionState}`;
    stateEl.style.display = 'inline-block';
  } else {
    stateEl.style.display = 'none';
  }
}

export function renderChat(state: AppState): void {
  const chat = $('chat');

  if (state.messages.length === 0) {
    chat.innerHTML =
      '<div class="empty-state">Create a session to start chatting</div>';
    return;
  }

  chat.innerHTML = state.messages
    .map(
      (msg) => `
      <div class="message ${msg.role}">
        <div class="message-role">${msg.role === 'user' ? 'You' : 'Agent'}</div>
        <div class="message-content">${msg.content}</div>
      </div>
    `
    )
    .join('');

  chat.scrollTop = chat.scrollHeight;
}

export function renderIntentsPanel(state: AppState): void {
  const panel = $('intents-panel');
  const countEl = $('intent-count');

  if (state.intents.length === 0) {
    panel.innerHTML = '<div class="empty-state">Intents will appear here</div>';
    countEl.style.display = 'none';
    return;
  }

  countEl.textContent = String(state.intents.length);
  countEl.style.display = 'inline-block';

  panel.innerHTML = state.intents
    .map((intent, i) => renderIntentCard(intent, state.validations[i]))
    .join('');
}

function renderIntentCard(
  intent: VerbIntent,
  validation?: IntentValidation
): string {
  const isValid = validation?.valid ?? true;
  const validClass = isValid ? 'valid' : 'invalid';

  let html = `<div class="intent-card ${validClass}">`;
  html += `<div class="intent-verb">${escapeHtml(intent.verb)}</div>`;

  // Parameters
  const paramEntries = Object.entries(intent.params);
  if (paramEntries.length > 0) {
    html += '<div class="intent-params">';
    for (const [key, value] of paramEntries) {
      html += `
        <div class="intent-param">
          <span class="intent-param-key">:${escapeHtml(key)}</span>
          <span class="intent-param-value">${escapeHtml(formatParamValue(value))}</span>
        </div>
      `;
    }
    html += '</div>';
  }

  // References
  const refEntries = Object.entries(intent.refs);
  if (refEntries.length > 0) {
    html += '<div class="intent-refs">';
    for (const [key, ref] of refEntries) {
      html += `<span class="intent-ref">:${escapeHtml(key)} = ${escapeHtml(ref)}</span> `;
    }
    html += '</div>';
  }

  // Validation errors
  if (validation && !validation.valid && validation.errors.length > 0) {
    html += '<div class="validation-errors">';
    html += validation.errors.map((e) => escapeHtml(e.message)).join('<br>');
    html += '</div>';
  }

  html += '</div>';
  return html;
}

export function renderDslPreview(state: AppState): void {
  const preview = $('dsl-preview');
  const countEl = $('dsl-count');

  if (state.assembledDsl.length === 0) {
    preview.innerHTML =
      '<div class="empty-state" style="color: #666;">DSL will appear here</div>';
    countEl.style.display = 'none';
    return;
  }

  countEl.textContent = String(state.assembledDsl.length);
  countEl.style.display = 'inline-block';

  preview.innerHTML = state.assembledDsl
    .map(
      (dsl, i) => `
      <div class="dsl-item">
        <span class="dsl-comment"># Statement ${i + 1}</span>
        <pre class="dsl-code">${escapeHtml(dsl)}</pre>
      </div>
    `
    )
    .join('');
}

export function renderButtons(state: AppState): void {
  const messageInput = $('message') as HTMLInputElement;
  const sendBtn = $('send-btn') as HTMLButtonElement;
  const executeBtn = $('execute-btn') as HTMLButtonElement;
  const clearBtn = $('clear-btn') as HTMLButtonElement;

  const hasSession = state.sessionId !== null;
  const isLoading = state.loading;

  messageInput.disabled = !hasSession || isLoading;
  sendBtn.disabled = !hasSession || isLoading;
  executeBtn.disabled = !state.canExecute || isLoading;
  clearBtn.disabled = state.assembledDsl.length === 0 || isLoading;

  if (isLoading) {
    executeBtn.textContent = 'Executing...';
  } else {
    executeBtn.textContent = 'Execute All DSL';
  }
}

export function showLoading(): void {
  const chat = $('chat');
  const loadingDiv = document.createElement('div');
  loadingDiv.id = 'loading-message';
  loadingDiv.className = 'message agent loading';
  loadingDiv.innerHTML =
    '<div class="message-role">Agent</div><div class="message-content">Extracting intents...</div>';
  chat.appendChild(loadingDiv);
  chat.scrollTop = chat.scrollHeight;
}

export function hideLoading(): void {
  const loadingDiv = document.getElementById('loading-message');
  if (loadingDiv) loadingDiv.remove();
}

export function showError(message: string): void {
  const chat = $('chat');
  const errorDiv = document.createElement('div');
  errorDiv.className = 'message agent';
  errorDiv.innerHTML = `
    <div class="message-role">Agent</div>
    <div class="message-content error-message">${escapeHtml(message)}</div>
  `;
  chat.appendChild(errorDiv);
  chat.scrollTop = chat.scrollHeight;
}

// ============================================================================
// Full Render
// ============================================================================

export function render(state: AppState): void {
  renderSessionInfo(state);
  renderChat(state);
  renderIntentsPanel(state);
  renderDslPreview(state);
  renderButtons(state);
}
