// Enterprise Onboarding Agent - Chat-based interface with context tracking
// Maintains CBU/Case context across conversation for incremental building

document.addEventListener("DOMContentLoaded", () => {
  // ==================== STATE ====================
  let sessionId: string | null = null;

  // Context - human readable + UUIDs
  interface SessionContext {
    cbu?: {
      id: string;
      name: string;
      jurisdiction?: string;
      clientType?: string;
    };
    case?: { id: string; type: string; status?: string };
    entities: Map<
      string,
      { id: string; name: string; type: string; binding: string }
    >;
    bindings: Map<string, string>; // @symbol -> uuid
  }

  // Typed binding info from API (matches Rust BoundEntity)
  interface BoundEntity {
    id: string;
    entity_type: string;
    display_name: string;
  }

  // CBU structure from API (matches cbu.show output)
  interface CbuEntity {
    entity_id: string;
    name: string;
    entity_type: string;
    roles: string[];
    binding?: string;
  }

  interface CbuDocument {
    doc_id: string;
    document_type_code: string;
    document_name?: string;
    status: string;
    extraction_status?: string;
  }

  interface CbuService {
    instance_id: string;
    service_name?: string;
    resource_type?: string;
    status: string;
    instance_name?: string;
  }

  interface CbuScreening {
    screening_id: string;
    screening_type: string;
    status: string;
    result?: string;
    entity_name?: string;
  }

  interface CbuKycCase {
    case_id: string;
    case_type: string;
    status: string;
    risk_rating?: string;
    workstream_count?: number;
  }

  interface CbuSummary {
    entity_count: number;
    document_count: number;
    service_count: number;
    screening_count: number;
    case_count: number;
  }

  interface CbuData {
    cbu_id: string;
    name: string;
    jurisdiction?: string;
    client_type?: string;
    description?: string;
    nature_purpose?: string;
    entities: CbuEntity[];
    documents?: CbuDocument[];
    services?: CbuService[];
    screenings?: CbuScreening[];
    kyc_cases?: CbuKycCase[];
    summary?: CbuSummary;
  }

  // Debug panel state
  interface DebugPanelState {
    dslSource: string;
    ast: unknown[];
    bindings: Record<string, BoundEntity>;
    cbu: CbuData | null;
  }

  let debugState: DebugPanelState = {
    dslSource: "",
    ast: [],
    bindings: {},
    cbu: null,
  };

  let context: SessionContext = {
    entities: new Map(),
    bindings: new Map(),
  };

  // Pending state for confirmation flow
  let pendingDsl: string | null = null;
  let pendingCorrections: ValidationCorrection[] | null = null;

  // Entity search state
  interface EntitySearchResult {
    entity_id: string;
    name: string;
    entity_type: string;
    entity_type_code: string | null;
    jurisdiction: string | null;
    similarity: number;
  }

  interface PendingEntitySelection {
    originalMessage: string;
    searchQuery: string;
    results: EntitySearchResult[];
    createOption: string;
  }

  let pendingEntitySelection: PendingEntitySelection | null = null;

  // ==================== TYPES ====================
  interface ValidationCorrection {
    type: "lookup" | "verb";
    line: number;
    current: string;
    suggested: string;
    confidence: number;
    action: string;
    available: string[];
    arg_name?: string;
  }

  interface ValidateWithFixesResponse {
    valid: boolean;
    parse_error: string | null;
    compile_error: string | null;
    lookup_corrections: Array<{
      line: number;
      arg_name: string;
      current_value: string;
      suggested_value: string;
      available_values: string[];
      confidence: number;
      action: string;
    }>;
    verb_corrections: Array<{
      line: number;
      current_verb: string;
      suggested_verb: string;
      available_verbs: string[];
      confidence: number;
      action: string;
    }>;
    corrected_dsl: string | null;
    status: string;
    message: string | null;
  }

  interface ExecuteResponse {
    success: boolean;
    results?: Array<{
      statement_index: number;
      dsl: string;
      success: boolean;
      message: string;
      entity_id?: string;
      entity_type?: string;
    }>;
    bindings?: Record<string, string>;
    errors?: string[];
    error?: string;
    new_state?: string;
  }

  // ==================== DOM ELEMENTS ====================
  const chatMessages = document.getElementById(
    "chat-messages",
  ) as HTMLDivElement;
  const chatInput = document.getElementById("chat-input") as HTMLInputElement;
  const sendBtn = document.getElementById("send-btn") as HTMLButtonElement;
  const newSessionBtn = document.getElementById(
    "new-session-btn",
  ) as HTMLButtonElement;
  const contextBar = document.getElementById("context-bar") as HTMLDivElement;
  const contextCbu = document.getElementById("context-cbu") as HTMLDivElement;
  const contextCbuName = document.getElementById(
    "context-cbu-name",
  ) as HTMLSpanElement;
  const contextCbuId = document.getElementById(
    "context-cbu-id",
  ) as HTMLSpanElement;
  const contextCase = document.getElementById("context-case") as HTMLDivElement;
  const contextCaseType = document.getElementById(
    "context-case-type",
  ) as HTMLSpanElement;
  const contextCaseId = document.getElementById(
    "context-case-id",
  ) as HTMLSpanElement;
  const contextClearBtn = document.getElementById(
    "context-clear-btn",
  ) as HTMLButtonElement;

  // Panel elements
  const dslCode = document.getElementById("dsl-code") as HTMLDivElement;
  const dslEmpty = document.getElementById("dsl-empty") as HTMLDivElement;
  const astCode = document.getElementById("ast-code") as HTMLPreElement;
  const astEmpty = document.getElementById("ast-empty") as HTMLDivElement;
  const cbuTree = document.getElementById("cbu-tree") as HTMLDivElement;
  const cbuEmpty = document.getElementById("cbu-empty") as HTMLDivElement;
  const copyDslBtn = document.getElementById(
    "copy-dsl-btn",
  ) as HTMLButtonElement;

  // ==================== BINDING NORMALIZATION ====================
  // Map common agent-generated variants to canonical binding names
  const bindingAliases: Record<string, string[]> = {
    cbu: ["cbu_id", "cbu-id", "client", "client_id", "client-id"],
    case: ["case_id", "case-id", "kyc_case", "kyc-case", "kyc_case_id"],
    entity: ["entity_id", "entity-id"],
    company: ["company_id", "company-id", "corp", "corp_id"],
    person: ["person_id", "person-id", "ubo", "ubo_id"],
  };

  function normalizeDslBindings(dsl: string): string {
    let normalized = dsl;

    // For each canonical binding we have in context, replace variants
    for (const [canonical, aliases] of Object.entries(bindingAliases)) {
      if (context.bindings.has(canonical)) {
        for (const alias of aliases) {
          // Replace @alias with @canonical (word boundary to avoid partial matches)
          const regex = new RegExp(`@${alias}\\b`, "g");
          normalized = normalized.replace(regex, `@${canonical}`);
        }
      }
    }

    return normalized;
  }

  // ==================== ENTITY SEARCH ====================
  // Detect potential entity names from user message
  function detectEntityNames(message: string): string[] {
    const names: string[] = [];

    // Pattern: "add/assign/create [Name] as [role]"
    const addPattern =
      /(?:add|assign|make|appoint)\s+([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)\s+(?:as|to be)/gi;
    let match;
    while ((match = addPattern.exec(message)) !== null) {
      names.push(match[1]);
    }

    // Pattern: "[Name] as director/UBO/etc"
    const rolePattern =
      /([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)\s+as\s+(?:director|ubo|beneficial owner|shareholder|officer|manager)/gi;
    while ((match = rolePattern.exec(message)) !== null) {
      if (!names.includes(match[1])) {
        names.push(match[1]);
      }
    }

    // Pattern: "person/individual named [Name]" or "company called [Name]"
    const namedPattern =
      /(?:person|individual|company|entity)\s+(?:named|called)\s+([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)/gi;
    while ((match = namedPattern.exec(message)) !== null) {
      if (!names.includes(match[1])) {
        names.push(match[1]);
      }
    }

    return names;
  }

  async function searchEntities(
    query: string,
    jurisdiction?: string,
  ): Promise<{ results: EntitySearchResult[]; create_option: string }> {
    try {
      const body: Record<string, unknown> = { query, limit: 5 };
      if (jurisdiction) {
        body.jurisdiction = jurisdiction;
      }

      const resp = await fetch("/api/entities/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      if (!resp.ok) {
        return { results: [], create_option: `Create new entity "${query}"` };
      }

      return await resp.json();
    } catch (e) {
      console.error("Entity search failed:", e);
      return { results: [], create_option: `Create new entity "${query}"` };
    }
  }

  // ==================== LSP-STYLE COMPLETIONS ====================
  interface CompletionItem {
    value: string; // UUID or code to insert
    label: string; // Display label
    detail?: string; // Additional info
    score: number; // Relevance (0-1)
  }

  interface CompleteResponse {
    items: CompletionItem[];
    total: number;
  }

  /**
   * Get completions for a given entity type and query.
   * Uses EntityGateway for fast fuzzy matching.
   *
   * @param entityType - Type of entity: "cbu", "entity", "product", "role", "jurisdiction", etc.
   * @param query - Partial text to match
   * @param limit - Max results (default 10)
   */
  async function getCompletions(
    entityType: string,
    query: string,
    limit: number = 10,
  ): Promise<CompletionItem[]> {
    try {
      const resp = await fetch("/api/agent/complete", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ entity_type: entityType, query, limit }),
      });

      if (!resp.ok) {
        console.error("Completion request failed:", resp.status);
        return [];
      }

      const data: CompleteResponse = await resp.json();
      return data.items;
    } catch (e) {
      console.error("Completion error:", e);
      return [];
    }
  }

  // Export for potential use in other modules or debugging
  (window as unknown as Record<string, unknown>).getCompletions =
    getCompletions;

  // ==================== CONTEXT MANAGEMENT ====================
  function updateContextBar() {
    const hasCbu = !!context.cbu;
    const hasCase = !!context.case;

    if (hasCbu || hasCase) {
      contextBar.classList.add("active");
    } else {
      contextBar.classList.remove("active");
    }

    if (hasCbu) {
      contextCbu.style.display = "flex";
      contextCbuName.textContent = context.cbu!.name;
      if (context.cbu!.jurisdiction) {
        contextCbuName.textContent += ` (${context.cbu!.jurisdiction})`;
      }
      contextCbuId.textContent = context.cbu!.id.substring(0, 8) + "...";
    } else {
      contextCbu.style.display = "none";
    }

    if (hasCase) {
      contextCase.style.display = "flex";
      contextCaseType.textContent =
        context.case!.type +
        (context.case!.status ? ` - ${context.case!.status}` : "");
      contextCaseId.textContent = context.case!.id.substring(0, 8) + "...";
    } else {
      contextCase.style.display = "none";
    }
  }

  function clearContext() {
    context = { entities: new Map(), bindings: new Map() };
    updateContextBar();
  }

  // ==================== PANEL MANAGEMENT ====================
  function initPanels() {
    // Copy DSL button
    if (copyDslBtn) {
      copyDslBtn.addEventListener("click", () => {
        if (debugState.dslSource) {
          navigator.clipboard.writeText(debugState.dslSource);
          copyDslBtn.textContent = "Copied!";
          setTimeout(() => {
            copyDslBtn.textContent = "Copy";
          }, 1500);
        }
      });
    }
  }

  function updateDebugPanels() {
    // Update DSL Source panel
    if (debugState.dslSource) {
      dslCode.textContent = debugState.dslSource;
      dslCode.style.display = "block";
      dslEmpty.style.display = "none";
      copyDslBtn.disabled = false;
    } else {
      dslCode.style.display = "none";
      dslEmpty.style.display = "block";
      copyDslBtn.disabled = true;
    }

    // Update AST panel
    if (debugState.ast && debugState.ast.length > 0) {
      astCode.textContent = JSON.stringify(debugState.ast, null, 2);
      astCode.style.display = "block";
      astEmpty.style.display = "none";
    } else {
      astCode.style.display = "none";
      astEmpty.style.display = "block";
    }

    // Update CBU panel
    if (debugState.cbu) {
      cbuTree.innerHTML = renderCbuTree(debugState.cbu);
      cbuTree.style.display = "block";
      cbuEmpty.style.display = "none";
    } else {
      cbuTree.style.display = "none";
      cbuEmpty.style.display = "block";
    }
  }

  function updateDebugFromResponse(data: {
    dsl_source?: string;
    ast?: unknown[];
    bindings?: Record<string, BoundEntity>;
  }) {
    if (data.dsl_source !== undefined) {
      debugState.dslSource = data.dsl_source;
    }
    if (data.ast !== undefined) {
      debugState.ast = data.ast;
    }
    if (data.bindings !== undefined) {
      debugState.bindings = data.bindings;
      // If we have a CBU binding, fetch the CBU data
      for (const [, entity] of Object.entries(data.bindings)) {
        if (entity.entity_type === "cbu" && entity.id) {
          fetchCbuData(entity.id);
          break;
        }
      }
    }
    updateDebugPanels();
  }

  // Fetch CBU data using cbu.show verb and update the debug panel
  async function fetchCbuData(cbuId: string) {
    try {
      // Use cbu.show verb to get full CBU structure
      const dsl = `(cbu.show :cbu-id "${cbuId}")`;
      const resp = await fetch("/api/dsl/execute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ dsl }),
      });

      if (!resp.ok) {
        console.error("Failed to fetch CBU:", resp.status);
        return;
      }

      const data = await resp.json();

      if (data.success && data.results && data.results.length > 0) {
        // The cbu.show verb returns a record with the full CBU structure
        const cbuResult = data.results[0];
        if (cbuResult.result) {
          // Map the cbu.show result to our CbuData interface
          const cbuData = cbuResult.result;
          debugState.cbu = mapCbuShowResult(cbuData, cbuId);
          updateDebugPanels();
        }
      } else {
        console.error("cbu.show failed:", data.error || data.errors);
      }
    } catch (e) {
      console.error("Error fetching CBU:", e);
    }
  }

  // Map cbu.show result to CbuData interface
  function mapCbuShowResult(
    cbuData: Record<string, unknown>,
    cbuId: string,
  ): CbuData {
    return {
      cbu_id: (cbuData.cbu_id as string) || cbuId,
      name: (cbuData.name as string) || "Unknown",
      jurisdiction: cbuData.jurisdiction as string | undefined,
      client_type: cbuData.client_type as string | undefined,
      entities: (
        (cbuData.entities as Array<Record<string, unknown>>) || []
      ).map((e) => ({
        entity_id: e.entity_id as string,
        name: e.name as string,
        entity_type: e.entity_type as string,
        roles: (e.roles as string[]) || [],
      })),
      summary: cbuData.summary as CbuSummary | undefined,
    };
  }

  // Role clustering: UBO/ownership roles at top, business/execution at bottom
  const UBO_ROLES = new Set([
    "BENEFICIAL_OWNER",
    "UBO",
    "SHAREHOLDER",
    "OWNER",
    "SETTLER",
    "SETTLOR",
    "BENEFICIARY",
    "TRUSTEE",
    "PROTECTOR",
    "PARTNER",
    "GENERAL_PARTNER",
    "LIMITED_PARTNER",
    "NOMINEE",
    "CONTROLLING_PERSON",
  ]);

  const BUSINESS_ROLES = new Set([
    "DIRECTOR",
    "OFFICER",
    "CEO",
    "CFO",
    "COO",
    "SECRETARY",
    "MANAGER",
    "AUTHORIZED_SIGNATORY",
    "SIGNATORY",
    "CONTACT",
    "ADMINISTRATOR",
    "CUSTODIAN",
    "AUDITOR",
    "LEGAL_COUNSEL",
    "COMPLIANCE_OFFICER",
  ]);

  function getRoleCategory(role: string): "ubo" | "business" | "other" {
    const upper = role.toUpperCase();
    if (UBO_ROLES.has(upper)) return "ubo";
    if (BUSINESS_ROLES.has(upper)) return "business";
    return "other";
  }

  // Render CBU as horizontal tree: UBO roles at top, business roles at bottom
  function renderCbuTree(cbu: CbuData): string {
    const esc = escapeHtml;
    const lines: string[] = [];

    // CBU header line (compact, horizontal)
    const headerParts = [
      `<span class="t-node">CBU</span>`,
      `<span class="t-val">"${esc(cbu.name)}"</span>`,
    ];
    if (cbu.jurisdiction) {
      headerParts.push(`<span class="t-id">${esc(cbu.jurisdiction)}</span>`);
    }
    if (cbu.client_type) {
      headerParts.push(`<span class="t-type">${esc(cbu.client_type)}</span>`);
    }
    lines.push(headerParts.join(" "));

    // Summary counts (compact, single line)
    if (cbu.summary) {
      const s = cbu.summary;
      const counts = [
        s.entity_count > 0 ? `${s.entity_count} entities` : null,
        s.document_count > 0 ? `${s.document_count} docs` : null,
        s.service_count > 0 ? `${s.service_count} services` : null,
        s.case_count > 0 ? `${s.case_count} cases` : null,
      ].filter(Boolean);
      if (counts.length > 0) {
        lines.push(
          `<span class="t-br">│</span> <span class="t-id">${counts.join(" · ")}</span>`,
        );
      }
    }

    // Group entities by role (only entities with roles)
    const byRole: Record<string, CbuEntity[]> = {};
    for (const entity of cbu.entities || []) {
      const roles =
        entity.roles && entity.roles.length > 0 ? entity.roles : null;
      if (roles) {
        for (const role of roles) {
          if (!byRole[role]) byRole[role] = [];
          byRole[role].push(entity);
        }
      }
    }

    // Sort: UBO/ownership roles first (top), then other, then business (bottom)
    const roles = Object.keys(byRole).sort((a, b) => {
      const catA = getRoleCategory(a);
      const catB = getRoleCategory(b);
      const order = { ubo: 0, other: 1, business: 2 };
      if (order[catA] !== order[catB]) return order[catA] - order[catB];
      return a.localeCompare(b);
    });

    if (roles.length > 0) {
      lines.push(`<span class="t-br">│</span>`);

      roles.forEach((role, ri) => {
        const isLast = ri === roles.length - 1;
        const branch = isLast ? "└" : "├";
        const category = getRoleCategory(role);
        const roleClass =
          category === "ubo"
            ? "t-role-ubo"
            : category === "business"
              ? "t-role-biz"
              : "t-role";

        // Entities inline (horizontal: role → entity1, entity2, ...)
        const entityStrs = byRole[role].map((entity) => {
          const entityType = entity.entity_type?.replace(/_/g, " ") || "";
          const typeStr = entityType
            ? ` <span class="t-type">(${entityType})</span>`
            : "";
          return `<span class="t-val">"${esc(entity.name)}"</span>${typeStr}`;
        });

        lines.push(
          `<span class="t-br">${branch}─</span> <span class="${roleClass}">${esc(role)}</span> → ${entityStrs.join(", ")}`,
        );
      });
    } else {
      lines.push(
        `<span class="t-br">└─</span> <span class="t-id">(no entities with roles)</span>`,
      );
    }

    return lines.join("\n");
  }

  function extractEntitiesFromDsl(
    dsl: string,
  ): Array<{ name: string; type: string; binding?: string }> {
    const entities: Array<{ name: string; type: string; binding?: string }> =
      [];

    // Match entity.create-* patterns
    const entityPattern =
      /\(entity\.create-(\w+)[^)]*:(?:name|first-name)\s+"([^"]+)"[^)]*(?::as\s+@(\w+))?/g;
    let match;
    while ((match = entityPattern.exec(dsl)) !== null) {
      entities.push({
        type: match[1].replace(/-/g, " "),
        name: match[2],
        binding: match[3],
      });
    }

    return entities;
  }

  async function updateContextFromBindings(bindings: Record<string, string>) {
    // Store all bindings
    for (const [key, value] of Object.entries(bindings)) {
      context.bindings.set(key, value);
    }

    // Look for CBU binding - could be "cbu", "cbu_id", or any other name
    // The execute response includes cbu_id as a special key
    const cbuId = bindings.cbu_id || bindings.cbu;
    if (cbuId && !context.cbu) {
      await fetchAndDisplayCbu(cbuId);
    }

    // If we got a case binding, store it
    if (bindings.case && !context.case) {
      context.case = {
        id: bindings.case,
        type: "KYC Case",
        status: "INTAKE",
      };
    }

    updateContextBar();
  }

  // Fetch CBU data using cbu.show verb and update both context and debug panel
  async function fetchAndDisplayCbu(cbuId: string) {
    try {
      // Use cbu.show verb to get full CBU structure
      const dsl = `(cbu.show :cbu-id "${cbuId}")`;
      const resp = await fetch("/api/dsl/execute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ dsl }),
      });

      if (resp.ok) {
        const data = await resp.json();

        if (data.success && data.results && data.results.length > 0) {
          const cbuResult = data.results[0];
          if (cbuResult.result) {
            const cbuData = cbuResult.result;

            // Update context
            context.cbu = {
              id: cbuId,
              name: cbuData.name || "Unknown",
              jurisdiction: cbuData.jurisdiction,
              clientType: cbuData.client_type,
            };

            // Update debug panel with full CBU data
            debugState.cbu = mapCbuShowResult(cbuData, cbuId);
            updateDebugPanels();
          }
        }
      }
    } catch (e) {
      // Fallback - just use the ID
      context.cbu = {
        id: cbuId,
        name: "CBU " + cbuId.substring(0, 8),
      };
    }

    updateContextBar();
  }

  // ==================== UI HELPERS ====================
  function escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  function clearEmptyState() {
    const empty = chatMessages.querySelector(".empty-state");
    if (empty) empty.remove();
  }

  function addUserMessage(text: string) {
    clearEmptyState();
    const msg = document.createElement("div");
    msg.className = "msg msg-user";
    msg.innerHTML = `<div class="msg-bubble">${escapeHtml(text)}</div>`;
    chatMessages.appendChild(msg);
    chatMessages.scrollTop = chatMessages.scrollHeight;
  }

  function addAssistantMessage(options: {
    text: string;
    dsl?: string;
    status?: { type: "valid" | "error" | "warning"; text: string };
    actions?: Array<{ label: string; class?: string; onClick: () => void }>;
    suggestions?: ValidationCorrection[];
    createdEntities?: Array<{ name: string; type: string; binding?: string }>;
    entityChoices?: {
      results: EntitySearchResult[];
      createOption: string;
      onSelect: (choice: EntitySearchResult | "create") => void;
    };
  }) {
    clearEmptyState();
    const msg = document.createElement("div");
    msg.className = "msg msg-assistant";

    let html = `<div class="msg-bubble">${escapeHtml(options.text)}`;

    if (options.dsl) {
      html += `<div class="msg-dsl">${escapeHtml(options.dsl)}</div>`;
    }

    if (options.status) {
      html += `<div class="msg-status ${options.status.type}">${escapeHtml(options.status.text)}</div>`;
    }

    if (options.suggestions && options.suggestions.length > 0) {
      html += `<div class="suggestion-list">`;
      options.suggestions.forEach((s, i) => {
        const pct = Math.round(s.confidence * 100);
        html += `<div class="suggestion-item" data-index="${i}">
                    <span class="num">${i + 1}</span>
                    <span>"${escapeHtml(s.current)}" → "${escapeHtml(s.suggested)}"</span>
                    <span class="confidence">${pct}%</span>
                </div>`;
      });
      html += `</div>`;
    }

    // Entity choices for disambiguation
    if (options.entityChoices && options.entityChoices.results.length > 0) {
      html += `<div class="suggestion-list entity-choices">`;
      options.entityChoices.results.forEach((e, i) => {
        const pct = Math.round(e.similarity * 100);
        const typeDisplay =
          e.entity_type_code?.replace(/_/g, " ").toLowerCase() || e.entity_type;
        const jurisdictionDisplay = e.jurisdiction
          ? ` (${e.jurisdiction})`
          : "";
        html += `<div class="suggestion-item entity-choice" data-entity-idx="${i}">
                    <span class="num">${i + 1}</span>
                    <span class="entity-info">
                        <strong>${escapeHtml(e.name)}</strong>${jurisdictionDisplay}
                        <span class="entity-type">${typeDisplay}</span>
                    </span>
                    <span class="confidence">${pct}%</span>
                </div>`;
      });
      // Add "create new" option
      html += `<div class="suggestion-item entity-choice create-new" data-entity-idx="create">
                <span class="num">+</span>
                <span>${escapeHtml(options.entityChoices.createOption)}</span>
            </div>`;
      html += `</div>`;
    }

    if (options.createdEntities && options.createdEntities.length > 0) {
      html += `<div class="created-entities">`;
      for (const e of options.createdEntities) {
        html += `<div class="created-entity">
                    <span class="type">${escapeHtml(e.type)}</span>
                    <span class="name">${escapeHtml(e.name)}</span>
                    ${e.binding ? `<span class="binding">@${escapeHtml(e.binding)}</span>` : ""}
                </div>`;
      }
      html += `</div>`;
    }

    html += `</div>`; // close msg-bubble

    if (options.actions && options.actions.length > 0) {
      html += `<div class="msg-actions">`;
      options.actions.forEach((a, i) => {
        html += `<button class="${a.class || ""}" data-action="${i}">${escapeHtml(a.label)}</button>`;
      });
      html += `</div>`;
    }

    msg.innerHTML = html;
    chatMessages.appendChild(msg);
    chatMessages.scrollTop = chatMessages.scrollHeight;

    // Attach action handlers
    if (options.actions) {
      msg.querySelectorAll("[data-action]").forEach((btn, i) => {
        btn.addEventListener("click", () => options.actions![i].onClick());
      });
    }

    // Attach suggestion click handlers
    if (options.suggestions) {
      msg
        .querySelectorAll(".suggestion-item:not(.entity-choice)")
        .forEach((item) => {
          item.addEventListener("click", () => {
            const idx = parseInt(item.getAttribute("data-index") || "0");
            handleSuggestionSelect(idx + 1);
          });
        });
    }

    // Attach entity choice handlers
    if (options.entityChoices) {
      msg.querySelectorAll(".entity-choice").forEach((item) => {
        item.addEventListener("click", () => {
          const idxAttr = item.getAttribute("data-entity-idx");
          if (idxAttr === "create") {
            options.entityChoices!.onSelect("create");
          } else {
            const idx = parseInt(idxAttr || "0");
            options.entityChoices!.onSelect(
              options.entityChoices!.results[idx],
            );
          }
        });
      });
    }

    return msg;
  }

  function addExecutionResult(
    success: boolean,
    message: string,
    bindings?: Record<string, string>,
  ) {
    clearEmptyState();
    const msg = document.createElement("div");
    msg.className = "msg msg-assistant";

    let html = `<div class="msg-bubble">`;
    html += `<div class="execution-result ${success ? "success" : "error"}">`;
    html += success ? "✓ " : "✗ ";
    html += escapeHtml(message);

    if (success && bindings && Object.keys(bindings).length > 0) {
      html += `<div class="created-entities" style="margin-top: 0.5rem;">`;
      for (const [binding, id] of Object.entries(bindings)) {
        // Try to get human-readable name from context
        const entity = context.entities.get(binding);
        const displayName = entity ? entity.name : id.substring(0, 8) + "...";
        html += `<div class="created-entity">
                    <span class="binding">@${escapeHtml(binding)}</span>
                    <span class="name">${escapeHtml(displayName)}</span>
                </div>`;
      }
      html += `</div>`;
    }

    html += `</div></div>`;
    msg.innerHTML = html;
    chatMessages.appendChild(msg);
    chatMessages.scrollTop = chatMessages.scrollHeight;
  }

  // ==================== CONVERSATION FLOW ====================
  function handleSuggestionSelect(num: number) {
    if (!pendingCorrections || num < 1 || num > pendingCorrections.length) {
      addAssistantMessage({
        text: `Please select a number between 1 and ${pendingCorrections?.length || 0}.`,
      });
      return;
    }

    const correction = pendingCorrections[num - 1];

    if (pendingDsl) {
      if (correction.type === "lookup") {
        pendingDsl = pendingDsl.replace(
          `"${correction.current}"`,
          `"${correction.suggested}"`,
        );
      } else {
        pendingDsl = pendingDsl.replace(
          `(${correction.current}`,
          `(${correction.suggested}`,
        );
      }
    }

    addUserMessage(String(num));
    validateAndPrompt(pendingDsl!);
  }

  async function handleEntitySelection(choice: EntitySearchResult | "create") {
    if (!pendingEntitySelection) return;

    const { originalMessage } = pendingEntitySelection;
    pendingEntitySelection = null;

    if (choice === "create") {
      // User wants to create new - proceed with original message
      addUserMessage("Create new");
      await generateDslForMessage(originalMessage);
    } else {
      // User selected an existing entity - modify message to reference it
      addUserMessage(`Use existing: ${choice.name}`);

      // Store entity in context with a binding
      const bindingName = choice.name
        .toLowerCase()
        .replace(/\s+/g, "_")
        .replace(/[^a-z0-9_]/g, "");
      context.bindings.set(bindingName, choice.entity_id);
      context.entities.set(bindingName, {
        id: choice.entity_id,
        name: choice.name,
        type: choice.entity_type,
        binding: bindingName,
      });

      // Modify the message to tell agent to use existing entity
      const modifiedMessage = `${originalMessage}\n\n[Use existing entity: "${choice.name}" (ID: ${choice.entity_id}, binding: @${bindingName})]`;
      await generateDslForMessage(modifiedMessage);
    }
  }

  async function handleConfirmExecute(confirmed: boolean) {
    if (!confirmed) {
      pendingDsl = null;
      pendingCorrections = null;
      addAssistantMessage({
        text: "Okay, cancelled. What would you like to do?",
      });
      return;
    }

    if (!pendingDsl) {
      addAssistantMessage({
        text: "Nothing to execute. Describe what you want to onboard.",
      });
      return;
    }

    await executeDsl(pendingDsl);
    pendingDsl = null;
    pendingCorrections = null;
  }

  async function validateAndPrompt(dsl: string) {
    // NORMALIZE BINDINGS FIRST - map @cbu_id -> @cbu etc.
    const normalizedDsl = normalizeDslBindings(dsl);

    // Pre-extract entities for display
    const extractedEntities = extractEntitiesFromDsl(normalizedDsl);

    try {
      const resp = await fetch("/api/dsl/validate-with-fixes", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ dsl: normalizedDsl }),
      });

      if (!resp.ok) throw new Error("Validation failed");

      const data: ValidateWithFixesResponse = await resp.json();

      if (data.status === "valid") {
        pendingDsl = normalizedDsl;
        pendingCorrections = null;
        addAssistantMessage({
          text: "I've prepared this for you:",
          dsl: normalizedDsl,
          status: { type: "valid", text: "✓ Ready to run" },
          createdEntities: extractedEntities,
          actions: [
            {
              label: "Yes, run it",
              class: "success",
              onClick: () => handleConfirmExecute(true),
            },
            {
              label: "No",
              class: "secondary",
              onClick: () => handleConfirmExecute(false),
            },
          ],
        });
      } else if (data.status === "auto_fixed") {
        pendingDsl = data.corrected_dsl;
        pendingCorrections = null;
        addAssistantMessage({
          text: `I auto-corrected: ${data.message}`,
          dsl: data.corrected_dsl!,
          status: { type: "valid", text: "✓ Auto-corrected, ready to run" },
          createdEntities: extractEntitiesFromDsl(data.corrected_dsl!),
          actions: [
            {
              label: "Yes, run it",
              class: "success",
              onClick: () => handleConfirmExecute(true),
            },
            {
              label: "No",
              class: "secondary",
              onClick: () => handleConfirmExecute(false),
            },
          ],
        });
      } else if (data.status === "needs_confirmation") {
        pendingDsl = normalizedDsl;
        pendingCorrections = [];

        for (const lc of data.lookup_corrections) {
          if (lc.action === "needs_confirmation") {
            pendingCorrections.push({
              type: "lookup",
              line: lc.line,
              current: lc.current_value,
              suggested: lc.suggested_value,
              confidence: lc.confidence,
              action: lc.action,
              available: lc.available_values,
              arg_name: lc.arg_name,
            });
          }
        }
        for (const vc of data.verb_corrections) {
          if (vc.action === "needs_confirmation") {
            pendingCorrections.push({
              type: "verb",
              line: vc.line,
              current: vc.current_verb,
              suggested: vc.suggested_verb,
              confidence: vc.confidence,
              action: vc.action,
              available: vc.available_verbs,
            });
          }
        }

        addAssistantMessage({
          text: data.message || "I found some issues. Please confirm:",
          dsl: normalizedDsl,
          status: { type: "warning", text: "Needs your input" },
          suggestions: pendingCorrections,
        });
      } else {
        pendingDsl = null;
        pendingCorrections = null;
        addAssistantMessage({
          text: data.message || "I couldn't generate valid DSL.",
          dsl: normalizedDsl,
          status: {
            type: "error",
            text: data.compile_error || data.parse_error || "Invalid",
          },
        });
      }
    } catch (e) {
      pendingDsl = normalizedDsl;
      addAssistantMessage({
        text: "Here's what I've prepared:",
        dsl: normalizedDsl,
        createdEntities: extractedEntities,
        actions: [
          {
            label: "Yes, run it",
            class: "success",
            onClick: () => handleConfirmExecute(true),
          },
          {
            label: "No",
            class: "secondary",
            onClick: () => handleConfirmExecute(false),
          },
        ],
      });
    }
  }

  async function executeDsl(dsl: string) {
    addAssistantMessage({ text: "Executing..." });

    try {
      const resp = await fetch("/api/dsl/execute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ dsl }),
      });

      const data: ExecuteResponse = await resp.json();

      if (data.success) {
        if (data.bindings) {
          await updateContextFromBindings(data.bindings);
        }
        addExecutionResult(true, "Executed successfully!", data.bindings);
      } else {
        const errorMsg =
          data.errors?.join("; ") || data.error || "Execution failed";
        addExecutionResult(false, errorMsg);
      }
    } catch (e) {
      addExecutionResult(false, `Error: ${(e as Error).message}`);
    }
  }

  // Build context string for agent
  function getContextForAgent(): string {
    const parts: string[] = [];

    if (context.cbu) {
      parts.push(
        `Current CBU: "${context.cbu.name}" - use @cbu to reference it`,
      );
    }
    if (context.case) {
      parts.push(
        `Current KYC Case: ${context.case.type} - use @case to reference it`,
      );
    }
    if (context.bindings.size > 0) {
      const bindings = Array.from(context.bindings.entries())
        .map(([k, _]) => `@${k}`)
        .join(", ");
      parts.push(`Available bindings: ${bindings}`);
    }

    return parts.length > 0 ? parts.join("\n") : "";
  }

  async function generateDslForMessage(message: string) {
    try {
      if (!sessionId) {
        const sessResp = await fetch("/api/agent/session", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
        });
        const sessData = await sessResp.json();
        if (sessData.session_id) {
          sessionId = sessData.session_id;
        }
      }

      // Include context in the message
      const contextStr = getContextForAgent();
      const messageWithContext = contextStr
        ? `[Context]\n${contextStr}\n\n[Request]\n${message}`
        : message;

      const resp = await fetch("/api/agent/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          session_id: sessionId,
          message: messageWithContext,
        }),
      });

      const data = await resp.json();

      // Update debug panels with session state from response
      if (data.dsl_source || data.ast || data.bindings) {
        updateDebugFromResponse(data);
      }

      if (data.error) {
        addAssistantMessage({ text: `Error: ${data.error}` });
      } else {
        const dsl = data.assembled_dsl?.combined || data.dsl;
        if (dsl) {
          await validateAndPrompt(dsl);
        } else {
          addAssistantMessage({
            text: data.message || "I couldn't generate DSL for that.",
          });
        }
      }
    } catch (e) {
      addAssistantMessage({
        text: `Connection error: ${(e as Error).message}`,
      });
    }
  }

  async function sendMessage() {
    const text = chatInput.value.trim();
    if (!text) return;

    chatInput.value = "";

    // Simple confirmations
    const lower = text.toLowerCase();
    if (
      lower === "yes" ||
      lower === "y" ||
      lower === "run" ||
      lower === "execute"
    ) {
      addUserMessage(text);
      handleConfirmExecute(true);
      return;
    }
    if (lower === "no" || lower === "n" || lower === "cancel") {
      addUserMessage(text);
      handleConfirmExecute(false);
      return;
    }

    // Number selection for corrections
    const num = parseInt(text);
    if (
      !isNaN(num) &&
      pendingCorrections &&
      num >= 1 &&
      num <= pendingCorrections.length
    ) {
      handleSuggestionSelect(num);
      return;
    }

    // Number selection for entity choices
    if (!isNaN(num) && pendingEntitySelection) {
      if (num >= 1 && num <= pendingEntitySelection.results.length) {
        handleEntitySelection(pendingEntitySelection.results[num - 1]);
        return;
      } else if (
        num === pendingEntitySelection.results.length + 1 ||
        lower === "create" ||
        lower === "new"
      ) {
        handleEntitySelection("create");
        return;
      }
    }

    // "create" or "new" for entity selection
    if ((lower === "create" || lower === "new") && pendingEntitySelection) {
      handleEntitySelection("create");
      return;
    }

    addUserMessage(text);

    sendBtn.disabled = true;
    sendBtn.innerHTML = `<span class="spinner"></span>`;

    try {
      // Check for potential entity names that might already exist
      const entityNames = detectEntityNames(text);

      if (entityNames.length > 0) {
        // Search for the first entity name found
        const searchName = entityNames[0];
        const jurisdiction = context.cbu?.jurisdiction; // Use CBU jurisdiction if available
        const searchResult = await searchEntities(searchName, jurisdiction);

        // Only show choices if we have good matches (>= 70% similarity)
        const goodMatches = searchResult.results.filter(
          (r) => r.similarity >= 0.7,
        );

        if (goodMatches.length > 0) {
          // Found potential matches - ask user to choose
          pendingEntitySelection = {
            originalMessage: text,
            searchQuery: searchName,
            results: goodMatches,
            createOption: searchResult.create_option,
          };

          addAssistantMessage({
            text: `I found existing entities matching "${searchName}". Would you like to use one of these, or create a new one?`,
            entityChoices: {
              results: goodMatches,
              createOption: searchResult.create_option,
              onSelect: handleEntitySelection,
            },
          });
          return;
        }
      }

      // No entity matches found or no entity names detected - proceed with generation
      await generateDslForMessage(text);
    } catch (e) {
      addAssistantMessage({
        text: `Connection error: ${(e as Error).message}`,
      });
    } finally {
      sendBtn.disabled = false;
      sendBtn.textContent = "Send";
    }
  }

  function newSession() {
    sessionId = null;
    clearContext();
    pendingDsl = null;
    pendingCorrections = null;
    pendingEntitySelection = null;
    // Clear debug panel state
    debugState = { dslSource: "", ast: [], bindings: {}, cbu: null };
    updateDebugPanels();
    chatMessages.innerHTML = `<div class="empty-state">
            <p><strong>Enterprise Onboarding Agent</strong></p>
            <p>Describe what you want to onboard in natural language.</p>
            <p style="font-size: 0.85rem;">e.g., "Create a fund in Luxembourg called Apex Capital"</p>
        </div>`;
  }

  // Event listeners
  sendBtn.addEventListener("click", sendMessage);
  chatInput.addEventListener("keypress", (e) => {
    if (e.key === "Enter") sendMessage();
  });
  newSessionBtn.addEventListener("click", newSession);
  contextClearBtn.addEventListener("click", clearContext);

  // Initialize panels
  initPanels();
});
