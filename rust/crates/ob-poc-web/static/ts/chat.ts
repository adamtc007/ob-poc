// Chat Panel - SSE streaming for agent responses with disambiguation support

import {
  StreamChunk,
  AstStatement,
  AgentCommand,
  ChatResponseV2,
  DisambiguationRequest,
  DisambiguationItem,
  EntityMatch,
  DisambiguationSelection,
} from "./types.js";

/** Info about an unresolved entity reference in the AST */
export interface UnresolvedRef {
  statementIndex: number;
  argKey: string;
  entityType: string;
  searchText: string;
}

export type ChatCallback = {
  onDsl: (source: string) => void;
  onAst: (statements: AstStatement[]) => void;
  onCanExecute: (can: boolean) => void;
  onStatusChange: (status: string) => void;
  onCommand: (command: AgentCommand) => void;
  /** Called when AST contains unresolved entity refs - UI should prompt resolution */
  onUnresolvedRefs?: (refs: UnresolvedRef[]) => void;
};

export class ChatPanel {
  private messagesEl: HTMLElement;
  private inputEl: HTMLTextAreaElement;
  private statusEl: HTMLElement;

  private sessionId: string | null = null;
  private currentCbuId: string | null = null;
  private currentStream: EventSource | null = null;
  private callbacks: ChatCallback;
  private hasPendingDsl: boolean = false;
  private isLoading: boolean = false;

  // Disambiguation state
  private pendingDisambiguation: DisambiguationRequest | null = null;
  private disambiguationSelections: Map<string, string> = new Map();

  constructor(callbacks: ChatCallback) {
    this.messagesEl = document.getElementById("chat-messages")!;
    this.inputEl = document.getElementById("chat-input") as HTMLTextAreaElement;
    this.statusEl = document.getElementById("session-status")!;
    this.callbacks = callbacks;

    this.setupEventListeners();
    this.createSession();
  }

  private setupEventListeners() {
    this.inputEl.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        this.sendMessage();
      }
    });
  }

  async createSession() {
    try {
      const response = await fetch("/api/session", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      const data = await response.json();
      this.sessionId = data.session_id;
      this.updateStatus("new");
      this.appendSystemMessage(
        "Session created. Ask the agent to help with onboarding.",
      );
    } catch (error) {
      this.appendSystemMessage(`Failed to create session: ${error}`);
      this.updateStatus("error");
    }
  }

  setCbuId(cbuId: string | null) {
    this.currentCbuId = cbuId;
  }

  getSessionId(): string | null {
    return this.sessionId;
  }

  /** Recreate session (e.g., after server restart invalidates old session) */
  async recreateSession(): Promise<void> {
    console.log("[Chat] Recreating session...");
    this.sessionId = null;
    await this.createSession();
  }

  private async sendMessage() {
    const text = this.inputEl.value.trim();
    if (!text || !this.sessionId) return;

    this.inputEl.value = "";
    this.appendMessage("user", text);

    // Handle conversational commands when DSL is pending
    const lowerText = text.toLowerCase();
    if (this.hasPendingDsl) {
      if (
        lowerText === "execute" ||
        lowerText === "run" ||
        lowerText === "go"
      ) {
        this.execute();
        return;
      }
      if (
        lowerText === "cancel" ||
        lowerText === "clear" ||
        lowerText === "reset"
      ) {
        this.callbacks.onDsl("");
        this.callbacks.onAst([]);
        this.callbacks.onCanExecute(false);
        this.hasPendingDsl = false;
        this.appendSystemMessage("Cancelled. Start a new request.");
        return;
      }
      // Otherwise, treat as "add more" - send to agent to append
    }

    this.updateStatus("pending");
    this.setLoading(true);

    console.log(
      "[Chat] Sending message with cbu_id:",
      this.currentCbuId,
      "sessionId:",
      this.sessionId,
    );

    try {
      const response = await fetch(`/api/session/${this.sessionId}/chat`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message: text,
          cbu_id: this.currentCbuId,
        }),
      });

      const data = await response.json();

      if (data.stream_id) {
        // SSE streaming response
        this.streamResponse(data.stream_id);
      } else if (data.message) {
        // Immediate response (non-streaming)
        this.appendMessage("assistant", data.message);
        this.updateStatus(data.session_state || "new");

        if (data.dsl_source) {
          this.callbacks.onDsl(data.dsl_source);
          this.hasPendingDsl = true;
        }
        if (data.ast) {
          this.callbacks.onAst(data.ast);

          // Check for unresolved entity references
          const unresolvedRefs = this.extractUnresolvedRefs(data.ast);
          if (unresolvedRefs.length > 0 && this.callbacks.onUnresolvedRefs) {
            // Notify the app about unresolved refs - it will open the entity finder
            this.callbacks.onUnresolvedRefs(unresolvedRefs);
          } else if (data.can_execute) {
            // Only show execute prompt if all refs are resolved
            this.appendSystemMessage("Execute, add more commands, or cancel?");
          }
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
    } catch (error) {
      this.appendSystemMessage(`Error: ${error}`);
      this.updateStatus("error");
      this.setLoading(false);
    }
  }

  private streamResponse(streamId: string) {
    const msgEl = this.appendMessage("assistant", "");

    this.currentStream = new EventSource(`/api/chat/stream?id=${streamId}`);

    this.currentStream.onmessage = (event) => {
      try {
        const chunk: StreamChunk = JSON.parse(event.data);

        switch (chunk.type) {
          case "chunk":
            msgEl.textContent += chunk.content || "";
            break;
          case "dsl":
            if (chunk.source) {
              this.callbacks.onDsl(chunk.source);
            }
            break;
          case "ast":
            if (chunk.statements) {
              this.callbacks.onAst(chunk.statements);
            }
            break;
          case "done":
            this.currentStream?.close();
            this.currentStream = null;
            this.updateStatus("ready");
            if (chunk.can_execute) {
              this.callbacks.onCanExecute(true);
              this.hasPendingDsl = true;
            }
            this.setLoading(false);
            break;
          case "error":
            this.appendSystemMessage(`Error: ${chunk.message}`);
            this.currentStream?.close();
            this.currentStream = null;
            this.updateStatus("error");
            this.setLoading(false);
            break;
        }
      } catch {
        // Ignore parse errors for keepalive comments
      }
    };

    this.currentStream.onerror = () => {
      this.currentStream?.close();
      this.currentStream = null;
      this.setLoading(false);
    };
  }

  private async execute() {
    if (!this.sessionId) return;

    this.updateStatus("executing");
    this.hasPendingDsl = false;

    try {
      const response = await fetch(`/api/session/${this.sessionId}/execute`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });

      const data = await response.json();

      if (data.success) {
        this.appendSystemMessage("DSL executed successfully!");
        this.updateStatus("executed");

        // Show results
        for (const result of data.results || []) {
          if (result.entity_id) {
            this.appendSystemMessage(`Created: ${result.entity_id}`);
          }
        }
      } else {
        this.appendSystemMessage(
          `Execution failed: ${data.errors?.join(", ")}`,
        );
        this.updateStatus("error");
      }
    } catch (error) {
      this.appendSystemMessage(`Execution error: ${error}`);
      this.updateStatus("error");
    }
  }

  private async clear() {
    this.messagesEl.innerHTML = "";
    this.hasPendingDsl = false;
    this.callbacks.onDsl("");
    this.callbacks.onAst([]);
    this.callbacks.onCanExecute(false);

    // Create new session
    await this.createSession();
  }

  private appendMessage(
    role: "user" | "assistant",
    content: string,
  ): HTMLElement {
    const msgEl = document.createElement("div");
    msgEl.className = `chat-message ${role}`;
    msgEl.textContent = content;
    this.messagesEl.appendChild(msgEl);
    this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
    return msgEl;
  }

  private appendSystemMessage(content: string) {
    const msgEl = document.createElement("div");
    msgEl.className = "chat-message system";
    msgEl.textContent = content;
    this.messagesEl.appendChild(msgEl);
    this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
  }

  private updateStatus(status: string) {
    this.statusEl.textContent = status;
    this.statusEl.className = "status-badge";

    if (status === "ready" || status === "ready_to_execute") {
      this.statusEl.classList.add("ready");
    } else if (status === "pending" || status === "pending_validation") {
      this.statusEl.classList.add("pending");
    } else if (status === "error") {
      this.statusEl.classList.add("error");
    } else if (status === "executed") {
      this.statusEl.classList.add("executed");
    }

    this.callbacks.onStatusChange(status);
  }

  private setLoading(loading: boolean) {
    this.isLoading = loading;
    this.inputEl.disabled = loading;
  }

  /**
   * Extract unresolved EntityRefs from the AST.
   * An EntityRef is unresolved if resolved_key is null/undefined.
   */
  private extractUnresolvedRefs(ast: AstStatement[]): UnresolvedRef[] {
    const unresolvedRefs: UnresolvedRef[] = [];

    ast.forEach((stmt, stmtIndex) => {
      if (!stmt.VerbCall) return;

      stmt.VerbCall.arguments.forEach((arg) => {
        this.findUnresolvedInValue(
          arg.value,
          stmtIndex,
          arg.key,
          unresolvedRefs,
        );
      });
    });

    return unresolvedRefs;
  }

  /**
   * Recursively find unresolved EntityRefs in a value
   */
  private findUnresolvedInValue(
    value: unknown,
    stmtIndex: number,
    argKey: string,
    refs: UnresolvedRef[],
  ): void {
    if (!value || typeof value !== "object") return;

    // Check if this is an EntityRef
    if ("EntityRef" in (value as Record<string, unknown>)) {
      const entityRef = (value as Record<string, unknown>).EntityRef as {
        entity_type: string;
        value: string;
        resolved_key?: string | null;
      };

      if (!entityRef.resolved_key) {
        refs.push({
          statementIndex: stmtIndex,
          argKey: argKey,
          entityType: entityRef.entity_type,
          searchText: entityRef.value,
        });
      }
      return;
    }

    // Check if this is a List
    if ("List" in (value as Record<string, unknown>)) {
      const list = (value as Record<string, unknown>).List as unknown[];
      list.forEach((item) =>
        this.findUnresolvedInValue(item, stmtIndex, argKey, refs),
      );
      return;
    }

    // Recurse into other object types
    if (Array.isArray(value)) {
      value.forEach((item) =>
        this.findUnresolvedInValue(item, stmtIndex, argKey, refs),
      );
    } else {
      Object.values(value as Record<string, unknown>).forEach((v) =>
        this.findUnresolvedInValue(v, stmtIndex, argKey, refs),
      );
    }
  }
}
