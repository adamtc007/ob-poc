// Chat Panel - SSE streaming for agent responses
export class ChatPanel {
    constructor(callbacks) {
        this.sessionId = null;
        this.currentCbuId = null;
        this.currentStream = null;
        this.hasPendingDsl = false;
        this.isLoading = false;
        this.messagesEl = document.getElementById('chat-messages');
        this.inputEl = document.getElementById('chat-input');
        this.statusEl = document.getElementById('session-status');
        this.callbacks = callbacks;
        this.setupEventListeners();
        this.createSession();
    }
    setupEventListeners() {
        this.inputEl.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                this.sendMessage();
            }
        });
    }
    async createSession() {
        try {
            const response = await fetch('/api/session', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({}),
            });
            const data = await response.json();
            this.sessionId = data.session_id;
            this.updateStatus('new');
            this.appendSystemMessage('Session created. Ask the agent to help with onboarding.');
        }
        catch (error) {
            this.appendSystemMessage(`Failed to create session: ${error}`);
            this.updateStatus('error');
        }
    }
    setCbuId(cbuId) {
        this.currentCbuId = cbuId;
    }
    async sendMessage() {
        const text = this.inputEl.value.trim();
        if (!text || !this.sessionId)
            return;
        this.inputEl.value = '';
        this.appendMessage('user', text);
        // Handle conversational commands when DSL is pending
        const lowerText = text.toLowerCase();
        if (this.hasPendingDsl) {
            if (lowerText === 'execute' ||
                lowerText === 'run' ||
                lowerText === 'go') {
                this.execute();
                return;
            }
            if (lowerText === 'cancel' ||
                lowerText === 'clear' ||
                lowerText === 'reset') {
                this.callbacks.onDsl('');
                this.callbacks.onAst([]);
                this.callbacks.onCanExecute(false);
                this.hasPendingDsl = false;
                this.appendSystemMessage('Cancelled. Start a new request.');
                return;
            }
            // Otherwise, treat as "add more" - send to agent to append
        }
        this.updateStatus('pending');
        this.setLoading(true);
        try {
            const response = await fetch(`/api/session/${this.sessionId}/chat`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    message: text,
                    cbu_id: this.currentCbuId,
                }),
            });
            const data = await response.json();
            if (data.stream_id) {
                // SSE streaming response
                this.streamResponse(data.stream_id);
            }
            else if (data.message) {
                // Immediate response (non-streaming)
                this.appendMessage('assistant', data.message);
                this.updateStatus(data.session_state || 'new');
                if (data.dsl_source) {
                    this.callbacks.onDsl(data.dsl_source);
                    this.hasPendingDsl = true;
                    // Conversational prompt - no buttons
                    this.appendSystemMessage('Execute, add more commands, or cancel?');
                }
                if (data.ast) {
                    this.callbacks.onAst(data.ast);
                }
                if (data.can_execute) {
                    this.callbacks.onCanExecute(true);
                }
                // Process UI commands (show_cbu, highlight_entity, etc.)
                if (data.commands) {
                    for (const cmd of data.commands) {
                        this.callbacks.onCommand(cmd);
                    }
                }
                this.setLoading(false);
            }
        }
        catch (error) {
            this.appendSystemMessage(`Error: ${error}`);
            this.updateStatus('error');
            this.setLoading(false);
        }
    }
    streamResponse(streamId) {
        const msgEl = this.appendMessage('assistant', '');
        this.currentStream = new EventSource(`/api/chat/stream?id=${streamId}`);
        this.currentStream.onmessage = (event) => {
            try {
                const chunk = JSON.parse(event.data);
                switch (chunk.type) {
                    case 'chunk':
                        msgEl.textContent += chunk.content || '';
                        break;
                    case 'dsl':
                        if (chunk.source) {
                            this.callbacks.onDsl(chunk.source);
                        }
                        break;
                    case 'ast':
                        if (chunk.statements) {
                            this.callbacks.onAst(chunk.statements);
                        }
                        break;
                    case 'done':
                        this.currentStream?.close();
                        this.currentStream = null;
                        this.updateStatus('ready');
                        if (chunk.can_execute) {
                            this.callbacks.onCanExecute(true);
                            this.hasPendingDsl = true;
                        }
                        this.setLoading(false);
                        break;
                    case 'error':
                        this.appendSystemMessage(`Error: ${chunk.message}`);
                        this.currentStream?.close();
                        this.currentStream = null;
                        this.updateStatus('error');
                        this.setLoading(false);
                        break;
                }
            }
            catch {
                // Ignore parse errors for keepalive comments
            }
        };
        this.currentStream.onerror = () => {
            this.currentStream?.close();
            this.currentStream = null;
            this.setLoading(false);
        };
    }
    async execute() {
        if (!this.sessionId)
            return;
        this.updateStatus('executing');
        this.hasPendingDsl = false;
        try {
            const response = await fetch(`/api/session/${this.sessionId}/execute`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({}),
            });
            const data = await response.json();
            if (data.success) {
                this.appendSystemMessage('DSL executed successfully!');
                this.updateStatus('executed');
                // Show results
                for (const result of data.results || []) {
                    if (result.entity_id) {
                        this.appendSystemMessage(`Created: ${result.entity_id}`);
                    }
                }
            }
            else {
                this.appendSystemMessage(`Execution failed: ${data.errors?.join(', ')}`);
                this.updateStatus('error');
            }
        }
        catch (error) {
            this.appendSystemMessage(`Execution error: ${error}`);
            this.updateStatus('error');
        }
    }
    async clear() {
        this.messagesEl.innerHTML = '';
        this.hasPendingDsl = false;
        this.callbacks.onDsl('');
        this.callbacks.onAst([]);
        this.callbacks.onCanExecute(false);
        // Create new session
        await this.createSession();
    }
    appendMessage(role, content) {
        const msgEl = document.createElement('div');
        msgEl.className = `chat-message ${role}`;
        msgEl.textContent = content;
        this.messagesEl.appendChild(msgEl);
        this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
        return msgEl;
    }
    appendSystemMessage(content) {
        const msgEl = document.createElement('div');
        msgEl.className = 'chat-message system';
        msgEl.textContent = content;
        this.messagesEl.appendChild(msgEl);
        this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
    }
    updateStatus(status) {
        this.statusEl.textContent = status;
        this.statusEl.className = 'status-badge';
        if (status === 'ready' || status === 'ready_to_execute') {
            this.statusEl.classList.add('ready');
        }
        else if (status === 'pending' || status === 'pending_validation') {
            this.statusEl.classList.add('pending');
        }
        else if (status === 'error') {
            this.statusEl.classList.add('error');
        }
        else if (status === 'executed') {
            this.statusEl.classList.add('executed');
        }
        this.callbacks.onStatusChange(status);
    }
    setLoading(loading) {
        this.isLoading = loading;
        this.inputEl.disabled = loading;
    }
}
