// OB-POC Hybrid UI - Main Application Entry Point

import { ChatPanel, UnresolvedRef } from "./chat.js";
import { DslPanel } from "./dsl.js";
import { AstPanel, UnresolvedRefContext } from "./ast.js";
import { WasmBridge } from "./bridge.js";
import {
  EntityFinderModal,
  ResolveContext,
  EntityMatch,
} from "./entity-finder.js";
import { CbuSummary, AgentCommand } from "./types.js";

class App {
  private chatPanel: ChatPanel;
  private dslPanel: DslPanel;
  private astPanel: AstPanel;
  private wasmBridge: WasmBridge;
  private entityFinder: EntityFinderModal;

  private cbuSelector: HTMLSelectElement;
  private viewModeSelector: HTMLSelectElement;
  private currentCbuId: string | null = null;

  constructor() {
    // Initialize WASM bridge
    this.wasmBridge = new WasmBridge({
      onEntitySelected: (entityId) => this.handleEntitySelected(entityId),
      onCbuChanged: (cbuId) => this.handleCbuChanged(cbuId),
    });

    // Initialize entity finder modal
    this.entityFinder = new EntityFinderModal();

    // Initialize panels
    this.dslPanel = new DslPanel();

    this.astPanel = new AstPanel({
      onNodeSelected: (nodeId) => this.handleAstNodeSelected(nodeId),
      onUnresolvedRefClick: (ctx) => this.handleUnresolvedRefClick(ctx),
    });

    this.chatPanel = new ChatPanel({
      onDsl: (source) => this.dslPanel.setSource(source),
      onAst: (statements) => this.astPanel.setAst(statements),
      onCanExecute: (can) => this.handleCanExecuteChanged(can),
      onStatusChange: (status) => this.handleStatusChanged(status),
      onCommand: (cmd) => this.handleAgentCommand(cmd),
      onUnresolvedRefs: (refs) => this.handleUnresolvedRefs(refs),
    });

    // Setup CBU selector
    this.cbuSelector = document.getElementById(
      "cbu-selector",
    ) as HTMLSelectElement;
    this.viewModeSelector = document.getElementById(
      "view-mode",
    ) as HTMLSelectElement;

    this.setupCbuSelector();
    this.setupViewModeSelector();

    console.log("[App] Hybrid UI initialized");
  }

  private async setupCbuSelector() {
    try {
      const response = await fetch("/api/cbu");
      const cbus: CbuSummary[] = await response.json();

      // Clear existing options (except placeholder)
      while (this.cbuSelector.options.length > 1) {
        this.cbuSelector.remove(1);
      }

      // Add CBU options
      for (const cbu of cbus) {
        const option = document.createElement("option");
        option.value = cbu.cbu_id;
        option.textContent = `${cbu.name}${cbu.jurisdiction ? ` (${cbu.jurisdiction})` : ""}`;
        this.cbuSelector.appendChild(option);
      }

      this.cbuSelector.addEventListener("change", () => {
        const cbuId = this.cbuSelector.value;
        if (cbuId) {
          this.loadCbu(cbuId);
        }
      });
    } catch (error) {
      console.error("[App] Failed to load CBUs:", error);
    }
  }

  private setupViewModeSelector() {
    this.viewModeSelector.addEventListener("change", () => {
      const mode = this.viewModeSelector.value as
        | "KYC_UBO"
        | "SERVICE_DELIVERY"
        | "CUSTODY";
      this.wasmBridge.setViewMode(mode);
    });
  }

  private async loadCbu(cbuId: string) {
    this.currentCbuId = cbuId;
    this.chatPanel.setCbuId(cbuId);

    // Tell WASM to load the CBU graph
    this.wasmBridge.loadCbu(cbuId);

    // Bind CBU to session context so agent can reference it as @cbu
    await this.bindCbuToSession(cbuId);

    // Also load DSL for this CBU if any exists
    try {
      const response = await fetch(`/api/cbu/${cbuId}/dsl`);
      const data = await response.json();
      if (data.source) {
        this.dslPanel.setSource(data.source);
      }

      // Load AST
      const astResponse = await fetch(`/api/cbu/${cbuId}/ast`);
      const astData = await astResponse.json();
      if (astData.statements) {
        this.astPanel.setAst(astData.statements);
      }
    } catch (error) {
      console.error("[App] Failed to load CBU data:", error);
    }
  }

  private async bindCbuToSession(cbuId: string) {
    const sessionId = this.chatPanel.getSessionId();
    console.log(
      "[App] bindCbuToSession called - cbuId:",
      cbuId,
      "sessionId:",
      sessionId,
    );
    if (!sessionId) {
      console.warn(
        "[App] No session to bind CBU to - session not yet created?",
      );
      return;
    }

    // Get CBU name from selector for display
    const selectedOption = this.cbuSelector.selectedOptions[0];
    const displayName = selectedOption?.textContent || cbuId;
    console.log("[App] Binding CBU to session:", {
      sessionId,
      cbuId,
      displayName,
    });

    try {
      const response = await fetch(`/api/session/${sessionId}/bind`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: "cbu",
          id: cbuId,
          entity_type: "cbu",
          display_name: displayName,
        }),
      });

      if (response.ok) {
        const result = await response.json();
        console.log(
          "[App] CBU bound to session:",
          result.binding_name,
          "=",
          cbuId,
        );
      } else if (response.status === 404) {
        // Session expired or server restarted - recreate session and retry
        console.warn("[App] Session not found, recreating...");
        await this.chatPanel.recreateSession();
        // Retry bind with new session
        const newSessionId = this.chatPanel.getSessionId();
        if (newSessionId) {
          const retryResponse = await fetch(
            `/api/session/${newSessionId}/bind`,
            {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                name: "cbu",
                id: cbuId,
                entity_type: "cbu",
                display_name: displayName,
              }),
            },
          );
          if (retryResponse.ok) {
            const result = await retryResponse.json();
            console.log(
              "[App] CBU bound after session recreate:",
              result.binding_name,
            );
          } else {
            console.error(
              "[App] Failed to bind CBU after recreate:",
              await retryResponse.text(),
            );
          }
        }
      } else {
        console.error("[App] Failed to bind CBU:", await response.text());
      }
    } catch (error) {
      // Network error - likely server restart, try recreating session
      console.warn(
        "[App] Network error binding CBU, recreating session...",
        error,
      );
      try {
        await this.chatPanel.recreateSession();
        const newSessionId = this.chatPanel.getSessionId();
        if (newSessionId) {
          const retryResponse = await fetch(
            `/api/session/${newSessionId}/bind`,
            {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                name: "cbu",
                id: cbuId,
                entity_type: "cbu",
                display_name: displayName,
              }),
            },
          );
          if (retryResponse.ok) {
            const result = await retryResponse.json();
            console.log(
              "[App] CBU bound after session recreate:",
              result.binding_name,
            );
          }
        }
      } catch (retryError) {
        console.error("[App] Failed to recreate session:", retryError);
      }
    }
  }

  private handleEntitySelected(entityId: string) {
    console.log("[App] Entity selected in graph:", entityId);
    // Could highlight related DSL/AST nodes here
  }

  private handleCbuChanged(cbuId: string) {
    console.log("[App] CBU changed in graph:", cbuId);
    this.currentCbuId = cbuId;
    this.cbuSelector.value = cbuId;
    this.chatPanel.setCbuId(cbuId);
  }

  private handleAstNodeSelected(nodeId: string) {
    console.log("[App] AST node selected:", nodeId);
    // Could focus related entity in graph or highlight DSL line
  }

  private handleCanExecuteChanged(can: boolean) {
    if (can) {
      console.log("[App] DSL ready to execute");
    }
  }

  private handleAgentCommand(cmd: AgentCommand) {
    console.log("[App] Agent command:", cmd);

    switch (cmd.action) {
      case "show_cbu":
        // Load CBU in graph and update selector
        this.loadCbu(cmd.cbu_id);
        this.cbuSelector.value = cmd.cbu_id;
        break;
      case "highlight_entity":
        this.wasmBridge.focusEntity(cmd.entity_id);
        break;
      case "navigate_dsl":
        // Could scroll DSL panel to line
        console.log("[App] Navigate to DSL line:", cmd.line);
        break;
      case "focus_ast":
        // Could expand/highlight AST node
        console.log("[App] Focus AST node:", cmd.node_id);
        break;
    }
  }

  private handleStatusChanged(status: string) {
    console.log("[App] Session status:", status);

    if (status === "executed") {
      this.dslPanel.markExecuted();

      // Refresh CBU list in case new one was created
      this.setupCbuSelector();
    } else if (status === "error") {
      this.dslPanel.markError();
    }
  }

  /**
   * Handle unresolved entity refs from chat response.
   * Auto-opens the entity finder for the first unresolved ref.
   */
  private handleUnresolvedRefs(refs: UnresolvedRef[]) {
    if (refs.length === 0) return;

    console.log(`[App] ${refs.length} unresolved entity ref(s) detected`);

    // Open the entity finder for the first unresolved ref
    const first = refs[0];
    const resolveCtx: ResolveContext = {
      entityType: first.entityType,
      searchText: first.searchText,
      statementIndex: first.statementIndex,
      argKey: first.argKey,
    };

    // Show a helpful message about what's happening
    const remaining = refs.length - 1;
    const msg =
      remaining > 0
        ? `Resolve "${first.searchText}" (${remaining} more after this)`
        : `Resolve "${first.searchText}"`;
    console.log(`[App] ${msg}`);

    // Open the entity finder modal
    this.entityFinder.open(resolveCtx, (context, match) => {
      this.handleEntityResolution(context, match);
    });
  }

  private handleUnresolvedRefClick(ctx: UnresolvedRefContext) {
    console.log("[App] Unresolved ref clicked:", ctx);

    // Build resolve context for the entity finder
    const resolveCtx: ResolveContext = {
      entityType: ctx.entityType,
      searchText: ctx.searchText,
      statementIndex: ctx.statementIndex,
      argKey: ctx.argKey,
    };

    // Open the entity finder modal with callback
    this.entityFinder.open(resolveCtx, (context, match) => {
      this.handleEntityResolution(context, match);
    });
  }

  private async handleEntityResolution(
    ctx: ResolveContext,
    match: EntityMatch,
  ) {
    console.log("[App] Entity resolved:", match);

    // Call the resolution endpoint to update the DSL/AST
    try {
      const sessionId = this.chatPanel.getSessionId();
      if (!sessionId) {
        console.error("[App] No active session for resolution");
        return;
      }

      const response = await fetch("/api/dsl/resolve-ref", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          session_id: sessionId,
          ref_id: {
            statement_index: ctx.statementIndex,
            arg_key: ctx.argKey,
          },
          resolved_key: match.value,
        }),
      });

      if (!response.ok) {
        const error = await response.text();
        console.error("[App] Resolution failed:", error);
        return;
      }

      const result = await response.json();

      if (!result.success) {
        console.error("[App] Resolution error:", result.error);
        return;
      }

      // Update DSL and AST panels with the resolved data
      if (result.dsl_source) {
        this.dslPanel.setSource(result.dsl_source);
      }
      if (result.ast) {
        this.astPanel.setAst(result.ast);
      }

      console.log(
        "[App] Reference resolved. Remaining:",
        result.resolution_stats?.unresolved_count,
      );
    } catch (error) {
      console.error("[App] Resolution request failed:", error);
    }
  }
}

// Initialize app when DOM is ready
document.addEventListener("DOMContentLoaded", () => {
  new App();
});
