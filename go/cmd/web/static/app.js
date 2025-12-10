"use strict";
// Enterprise Onboarding Agent - Chat-based interface with context tracking
// Maintains CBU/Case context across conversation for incremental building
document.addEventListener("DOMContentLoaded", () => {
    // ==================== STATE ====================
    let sessionId = null;
    let entityFinderState = {
        visible: false,
        refId: null,
        entityType: "",
        searchValue: "",
        results: [],
        loading: false,
    };
    let debugState = {
        dslSource: "",
        ast: [],
        bindings: {},
        cbu: null,
    };
    let context = {
        entities: new Map(),
        bindings: new Map(),
    };
    // Pending state for confirmation flow
    let pendingDsl = null;
    let pendingCorrections = null;
    let pendingEntitySelection = null;
    // ==================== DOM ELEMENTS ====================
    const chatMessages = document.getElementById("chat-messages");
    const chatInput = document.getElementById("chat-input");
    const sendBtn = document.getElementById("send-btn");
    const newSessionBtn = document.getElementById("new-session-btn");
    const contextBar = document.getElementById("context-bar");
    const contextCbu = document.getElementById("context-cbu");
    const contextCbuName = document.getElementById("context-cbu-name");
    const contextCbuId = document.getElementById("context-cbu-id");
    const contextCase = document.getElementById("context-case");
    const contextCaseType = document.getElementById("context-case-type");
    const contextCaseId = document.getElementById("context-case-id");
    const contextClearBtn = document.getElementById("context-clear-btn");
    // Panel elements
    const dslCode = document.getElementById("dsl-code");
    const dslEmpty = document.getElementById("dsl-empty");
    const astCode = document.getElementById("ast-code");
    const astEmpty = document.getElementById("ast-empty");
    const cbuTree = document.getElementById("cbu-tree");
    const cbuEmpty = document.getElementById("cbu-empty");
    const copyDslBtn = document.getElementById("copy-dsl-btn");
    // ==================== BINDING NORMALIZATION ====================
    // Map common agent-generated variants to canonical binding names
    const bindingAliases = {
        cbu: ["cbu_id", "cbu-id", "client", "client_id", "client-id"],
        case: ["case_id", "case-id", "kyc_case", "kyc-case", "kyc_case_id"],
        entity: ["entity_id", "entity-id"],
        company: ["company_id", "company-id", "corp", "corp_id"],
        person: ["person_id", "person-id", "ubo", "ubo_id"],
    };
    function normalizeDslBindings(dsl) {
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
    function detectEntityNames(message) {
        const names = [];
        // Pattern: "add/assign/create [Name] as [role]"
        const addPattern = /(?:add|assign|make|appoint)\s+([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)\s+(?:as|to be)/gi;
        let match;
        while ((match = addPattern.exec(message)) !== null) {
            names.push(match[1]);
        }
        // Pattern: "[Name] as director/UBO/etc"
        const rolePattern = /([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)\s+as\s+(?:director|ubo|beneficial owner|shareholder|officer|manager)/gi;
        while ((match = rolePattern.exec(message)) !== null) {
            if (!names.includes(match[1])) {
                names.push(match[1]);
            }
        }
        // Pattern: "person/individual named [Name]" or "company called [Name]"
        const namedPattern = /(?:person|individual|company|entity)\s+(?:named|called)\s+([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)/gi;
        while ((match = namedPattern.exec(message)) !== null) {
            if (!names.includes(match[1])) {
                names.push(match[1]);
            }
        }
        return names;
    }
    async function searchEntities(query, jurisdiction) {
        try {
            const body = { query, limit: 5 };
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
        }
        catch (e) {
            console.error("Entity search failed:", e);
            return { results: [], create_option: `Create new entity "${query}"` };
        }
    }
    /**
     * Get completions for a given entity type and query.
     * Uses EntityGateway for fast fuzzy matching.
     *
     * @param entityType - Type of entity: "cbu", "entity", "product", "role", "jurisdiction", etc.
     * @param query - Partial text to match
     * @param limit - Max results (default 10)
     */
    async function getCompletions(entityType, query, limit = 10) {
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
            const data = await resp.json();
            return data.items;
        }
        catch (e) {
            console.error("Completion error:", e);
            return [];
        }
    }
    // Export for potential use in other modules or debugging
    window.getCompletions =
        getCompletions;
    // ==================== CONTEXT MANAGEMENT ====================
    function updateContextBar() {
        const hasCbu = !!context.cbu;
        const hasCase = !!context.case;
        if (hasCbu || hasCase) {
            contextBar.classList.add("active");
        }
        else {
            contextBar.classList.remove("active");
        }
        if (hasCbu) {
            contextCbu.style.display = "flex";
            contextCbuName.textContent = context.cbu.name;
            if (context.cbu.jurisdiction) {
                contextCbuName.textContent += ` (${context.cbu.jurisdiction})`;
            }
            contextCbuId.textContent = context.cbu.id.substring(0, 8) + "...";
        }
        else {
            contextCbu.style.display = "none";
        }
        if (hasCase) {
            contextCase.style.display = "flex";
            contextCaseType.textContent =
                context.case.type +
                    (context.case.status ? ` - ${context.case.status}` : "");
            contextCaseId.textContent = context.case.id.substring(0, 8) + "...";
        }
        else {
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
        }
        else {
            dslCode.style.display = "none";
            dslEmpty.style.display = "block";
            copyDslBtn.disabled = true;
        }
        // Update AST panel with interactive EntityRefs
        if (debugState.ast && debugState.ast.length > 0) {
            astCode.innerHTML = renderInteractiveAst(debugState.ast);
            astCode.style.display = "block";
            astEmpty.style.display = "none";
            // Attach click handlers to unresolved EntityRefs
            attachEntityRefHandlers();
        }
        else {
            astCode.style.display = "none";
            astEmpty.style.display = "block";
        }
        // Update CBU panel
        if (debugState.cbu) {
            cbuTree.innerHTML = renderCbuTree(debugState.cbu);
            cbuTree.style.display = "block";
            cbuEmpty.style.display = "none";
        }
        else {
            cbuTree.style.display = "none";
            cbuEmpty.style.display = "block";
        }
    }
    // ==================== INTERACTIVE AST RENDERING ====================
    /**
     * Render AST with clickable unresolved EntityRefs
     * Unresolved refs (resolved_key: null) become clickable to open entity finder
     */
    function renderInteractiveAst(ast) {
        const lines = [];
        ast.forEach((stmt, stmtIdx) => {
            if ("VerbCall" in stmt) {
                const vc = stmt.VerbCall;
                lines.push(`<span class="ast-verb">(${vc.domain}.${vc.verb}</span>`);
                vc.arguments.forEach((arg) => {
                    const argHtml = renderAstValue(arg.value, stmtIdx, arg.key);
                    lines.push(`  <span class="ast-key">:${arg.key}</span> ${argHtml}`);
                });
                if (vc.binding) {
                    lines.push(`  <span class="ast-key">:as</span> <span class="ast-symbol">@${vc.binding}</span>`);
                }
                lines.push(`<span class="ast-verb">)</span>`);
                lines.push(""); // blank line between statements
            }
            else if ("Comment" in stmt) {
                lines.push(`<span class="ast-comment">;; ${escapeHtml(stmt.Comment)}</span>`);
            }
        });
        return lines.join("\n");
    }
    /**
     * Render an AST value node, making EntityRefs clickable if unresolved
     */
    function renderAstValue(value, stmtIdx, argKey) {
        if (value === null || value === undefined) {
            return `<span class="ast-null">null</span>`;
        }
        if (typeof value === "string") {
            return `<span class="ast-string">"${escapeHtml(value)}"</span>`;
        }
        if (typeof value === "number" || typeof value === "boolean") {
            return `<span class="ast-literal">${value}</span>`;
        }
        if (typeof value === "object") {
            // Check for EntityRef
            if ("EntityRef" in value) {
                const ref = value.EntityRef;
                return renderEntityRef(ref, stmtIdx, argKey);
            }
            // Check for SymbolRef
            if ("SymbolRef" in value) {
                const sym = value.SymbolRef;
                return `<span class="ast-symbol">@${escapeHtml(sym.name)}</span>`;
            }
            // Check for Literal wrapper
            if ("Literal" in value) {
                const lit = value.Literal;
                return renderAstValue(lit, stmtIdx, argKey);
            }
            // Check for String wrapper
            if ("String" in value) {
                const str = value.String;
                return `<span class="ast-string">"${escapeHtml(str)}"</span>`;
            }
            // Check for Number wrapper
            if ("Number" in value) {
                const num = value.Number;
                return `<span class="ast-literal">${num}</span>`;
            }
            // Check for Boolean wrapper
            if ("Boolean" in value) {
                const bool = value.Boolean;
                return `<span class="ast-literal">${bool}</span>`;
            }
            // Fallback: JSON stringify
            return `<span class="ast-object">${escapeHtml(JSON.stringify(value))}</span>`;
        }
        return `<span class="ast-unknown">${escapeHtml(String(value))}</span>`;
    }
    /**
     * Render an EntityRef - clickable if unresolved
     */
    function renderEntityRef(ref, stmtIdx, argKey) {
        const entityType = escapeHtml(ref.entity_type);
        const value = escapeHtml(ref.value);
        const resolved = ref.resolved_key;
        if (resolved) {
            // Resolved - show as green triplet
            return (`<span class="ast-entity-ref resolved" title="Resolved to ${escapeHtml(resolved)}">` +
                `<span class="ref-type">${entityType}</span> ` +
                `<span class="ref-value">"${value}"</span> ` +
                `<span class="ref-key">${escapeHtml(resolved.substring(0, 8))}...</span>` +
                `</span>`);
        }
        else {
            // Unresolved - clickable
            return (`<span class="ast-entity-ref unresolved" ` +
                `data-stmt-idx="${stmtIdx}" ` +
                `data-arg-key="${escapeHtml(argKey)}" ` +
                `data-entity-type="${entityType}" ` +
                `data-search-value="${value}" ` +
                `title="Click to resolve: ${entityType} '${value}'">` +
                `<span class="ref-type">${entityType}</span> ` +
                `<span class="ref-value">"${value}"</span> ` +
                `<span class="ref-key unresolved-marker">⚠ unresolved</span>` +
                `</span>`);
        }
    }
    /**
     * Attach click handlers to unresolved EntityRefs in the AST panel
     */
    function attachEntityRefHandlers() {
        const unresolvedRefs = astCode.querySelectorAll(".ast-entity-ref.unresolved");
        unresolvedRefs.forEach((el) => {
            el.addEventListener("click", (e) => {
                e.preventDefault();
                const target = e.currentTarget;
                const stmtIdx = parseInt(target.dataset.stmtIdx || "0", 10);
                const argKey = target.dataset.argKey || "";
                const entityType = target.dataset.entityType || "";
                const searchValue = target.dataset.searchValue || "";
                openEntityFinder(stmtIdx, argKey, entityType, searchValue);
            });
        });
    }
    // ==================== ENTITY FINDER MODAL ====================
    /**
     * Open the entity finder modal for a specific EntityRef
     */
    async function openEntityFinder(stmtIdx, argKey, entityType, searchValue) {
        entityFinderState = {
            visible: true,
            refId: { statement_index: stmtIdx, arg_key: argKey },
            entityType,
            searchValue,
            results: [],
            loading: true,
        };
        renderEntityFinderModal();
        // Search for entities
        try {
            const results = await searchEntitiesForFinder(entityType, searchValue);
            entityFinderState.results = results;
            entityFinderState.loading = false;
            renderEntityFinderModal();
        }
        catch (e) {
            console.error("Entity search failed:", e);
            entityFinderState.loading = false;
            entityFinderState.results = [];
            renderEntityFinderModal();
        }
    }
    /**
     * Search entities using the /api/entity/search endpoint
     */
    async function searchEntitiesForFinder(entityType, query) {
        try {
            const resp = await fetch("/api/entity/search", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    entity_type: entityType,
                    query: query,
                    limit: 10,
                }),
            });
            if (!resp.ok) {
                console.error("Entity search failed:", resp.status);
                return [];
            }
            const data = await resp.json();
            return data.results || [];
        }
        catch (e) {
            console.error("Entity search error:", e);
            return [];
        }
    }
    /**
     * Close the entity finder modal
     */
    function closeEntityFinder() {
        entityFinderState.visible = false;
        entityFinderState.refId = null;
        const modal = document.getElementById("entity-finder-modal");
        if (modal) {
            modal.remove();
        }
    }
    /**
     * Handle entity selection from the finder modal
     */
    async function selectEntityFromFinder(result) {
        if (!entityFinderState.refId || !sessionId) {
            closeEntityFinder();
            return;
        }
        // Call resolve-ref API
        try {
            const resp = await fetch("/api/dsl/resolve-ref", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    session_id: sessionId,
                    ref_id: entityFinderState.refId,
                    resolved_key: result.entity_id,
                }),
            });
            if (!resp.ok) {
                console.error("Resolve-ref failed:", resp.status);
                addAssistantMessage({
                    text: `Failed to resolve entity reference: HTTP ${resp.status}`,
                });
                closeEntityFinder();
                return;
            }
            const data = await resp.json();
            if (data.success) {
                // Update AST from response
                if (data.ast) {
                    debugState.ast = data.ast;
                    updateDebugPanels();
                }
                // Show success message with resolution stats
                const stats = data.resolution_stats;
                const remaining = stats.unresolved_count;
                if (data.can_execute) {
                    addAssistantMessage({
                        text: `Resolved "${entityFinderState.searchValue}" → ${result.name}. All references resolved - ready to execute!`,
                        status: { type: "valid", text: "✓ Ready to execute" },
                    });
                }
                else {
                    addAssistantMessage({
                        text: `Resolved "${entityFinderState.searchValue}" → ${result.name}. ${remaining} unresolved reference(s) remaining.`,
                        status: { type: "warning", text: `${remaining} unresolved` },
                    });
                }
            }
            else {
                addAssistantMessage({
                    text: `Failed to resolve: ${data.error || "Unknown error"}`,
                    status: { type: "error", text: data.code || "Error" },
                });
            }
        }
        catch (e) {
            console.error("Resolve-ref error:", e);
            addAssistantMessage({
                text: `Error resolving entity: ${e.message}`,
            });
        }
        closeEntityFinder();
    }
    /**
     * Render the entity finder modal
     */
    function renderEntityFinderModal() {
        // Remove existing modal
        let modal = document.getElementById("entity-finder-modal");
        if (modal) {
            modal.remove();
        }
        if (!entityFinderState.visible) {
            return;
        }
        modal = document.createElement("div");
        modal.id = "entity-finder-modal";
        modal.className = "entity-finder-overlay";
        const { entityType, searchValue, results, loading } = entityFinderState;
        let resultsHtml = "";
        if (loading) {
            resultsHtml = `<div class="finder-loading"><span class="spinner"></span> Searching...</div>`;
        }
        else if (results.length === 0) {
            resultsHtml = `<div class="finder-empty">No matching entities found for "${escapeHtml(searchValue)}"</div>`;
        }
        else {
            resultsHtml = `<div class="finder-results">`;
            results.forEach((r, idx) => {
                const pct = Math.round(r.similarity * 100);
                const typeDisplay = r.entity_type_code?.replace(/_/g, " ") || r.entity_type;
                const jurisdiction = r.jurisdiction ? ` (${r.jurisdiction})` : "";
                resultsHtml += `
          <div class="finder-result" data-result-idx="${idx}">
            <div class="finder-result-main">
              <span class="finder-result-name">${escapeHtml(r.name)}</span>
              <span class="finder-result-jurisdiction">${escapeHtml(jurisdiction)}</span>
            </div>
            <div class="finder-result-meta">
              <span class="finder-result-type">${escapeHtml(typeDisplay)}</span>
              <span class="finder-result-score">${pct}%</span>
            </div>
          </div>
        `;
            });
            resultsHtml += `</div>`;
        }
        modal.innerHTML = `
      <div class="entity-finder-modal">
        <div class="finder-header">
          <h3>Resolve Entity Reference</h3>
          <button class="finder-close" title="Close">&times;</button>
        </div>
        <div class="finder-info">
          <span class="finder-type">${escapeHtml(entityType)}</span>
          <span class="finder-search">"${escapeHtml(searchValue)}"</span>
        </div>
        <div class="finder-search-box">
          <input type="text" id="finder-search-input" placeholder="Refine search..." value="${escapeHtml(searchValue)}">
          <button id="finder-search-btn">Search</button>
        </div>
        ${resultsHtml}
      </div>
    `;
        document.body.appendChild(modal);
        // Attach event handlers
        modal
            .querySelector(".finder-close")
            ?.addEventListener("click", closeEntityFinder);
        modal
            .querySelector(".entity-finder-overlay")
            ?.addEventListener("click", (e) => {
            if (e.target === modal) {
                closeEntityFinder();
            }
        });
        // Search button handler
        modal
            .querySelector("#finder-search-btn")
            ?.addEventListener("click", async () => {
            const input = document.getElementById("finder-search-input");
            if (input && input.value.trim()) {
                entityFinderState.searchValue = input.value.trim();
                entityFinderState.loading = true;
                renderEntityFinderModal();
                const results = await searchEntitiesForFinder(entityFinderState.entityType, entityFinderState.searchValue);
                entityFinderState.results = results;
                entityFinderState.loading = false;
                renderEntityFinderModal();
            }
        });
        // Enter key in search input
        modal
            .querySelector("#finder-search-input")
            ?.addEventListener("keypress", (e) => {
            if (e.key === "Enter") {
                modal
                    ?.querySelector("#finder-search-btn")
                    ?.dispatchEvent(new Event("click"));
            }
        });
        // Result click handlers
        modal.querySelectorAll(".finder-result").forEach((el) => {
            el.addEventListener("click", () => {
                const idx = parseInt(el.dataset.resultIdx || "0", 10);
                const result = entityFinderState.results[idx];
                if (result) {
                    selectEntityFromFinder(result);
                }
            });
        });
        // Focus search input
        document.getElementById("finder-search-input")?.focus();
        // Escape key to close modal
        const handleEscape = (e) => {
            if (e.key === "Escape") {
                closeEntityFinder();
                document.removeEventListener("keydown", handleEscape);
            }
        };
        document.addEventListener("keydown", handleEscape);
    }
    function updateDebugFromResponse(data) {
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
    async function fetchCbuData(cbuId) {
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
            }
            else {
                console.error("cbu.show failed:", data.error || data.errors);
            }
        }
        catch (e) {
            console.error("Error fetching CBU:", e);
        }
    }
    // Map cbu.show result to CbuData interface
    function mapCbuShowResult(cbuData, cbuId) {
        return {
            cbu_id: cbuData.cbu_id || cbuId,
            name: cbuData.name || "Unknown",
            jurisdiction: cbuData.jurisdiction,
            client_type: cbuData.client_type,
            entities: (cbuData.entities || []).map((e) => ({
                entity_id: e.entity_id,
                name: e.name,
                entity_type: e.entity_type,
                roles: e.roles || [],
            })),
            services: (cbuData.services || []).map((s) => ({
                instance_id: s.delivery_id || "",
                service_name: s.service,
                resource_type: s.product,
                status: s.status,
            })),
            documents: cbuData.documents,
            screenings: cbuData.screenings,
            kyc_cases: cbuData.kyc_cases,
            summary: cbuData.summary,
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
    function getRoleCategory(role) {
        const upper = role.toUpperCase();
        if (UBO_ROLES.has(upper))
            return "ubo";
        if (BUSINESS_ROLES.has(upper))
            return "business";
        return "other";
    }
    // Render CBU as horizontal tree: UBO roles at top, business roles at bottom
    function renderCbuTree(cbu) {
        const esc = escapeHtml;
        const lines = [];
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
                lines.push(`<span class="t-br">│</span> <span class="t-id">${counts.join(" · ")}</span>`);
            }
        }
        // Products/Services section (at top, directly under CBU)
        if (cbu.services && cbu.services.length > 0) {
            lines.push(`<span class="t-br">│</span>`);
            lines.push(`<span class="t-br">├─</span> <span class="t-node">Products</span>`);
            cbu.services.forEach((svc, idx) => {
                const isLast = idx === cbu.services.length - 1;
                const branch = isLast ? "└" : "├";
                const statusClass = svc.status === "ACTIVE" ? "t-status-active" : "t-status-pending";
                lines.push(`<span class="t-br">│  ${branch}─</span> <span class="t-val">${esc(svc.service_name || "")}</span> <span class="t-type">(${esc(svc.resource_type || "")})</span> <span class="${statusClass}">${esc(svc.status)}</span>`);
            });
        }
        // Group entities by role (only entities with roles)
        const byRole = {};
        for (const entity of cbu.entities || []) {
            const roles = entity.roles && entity.roles.length > 0 ? entity.roles : null;
            if (roles) {
                for (const role of roles) {
                    if (!byRole[role])
                        byRole[role] = [];
                    byRole[role].push(entity);
                }
            }
        }
        // Sort: UBO/ownership roles first (top), then other, then business (bottom)
        const roles = Object.keys(byRole).sort((a, b) => {
            const catA = getRoleCategory(a);
            const catB = getRoleCategory(b);
            const order = { ubo: 0, other: 1, business: 2 };
            if (order[catA] !== order[catB])
                return order[catA] - order[catB];
            return a.localeCompare(b);
        });
        if (roles.length > 0) {
            lines.push(`<span class="t-br">│</span>`);
            roles.forEach((role, ri) => {
                const isLast = ri === roles.length - 1;
                const branch = isLast ? "└" : "├";
                const category = getRoleCategory(role);
                const roleClass = category === "ubo"
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
                lines.push(`<span class="t-br">${branch}─</span> <span class="${roleClass}">${esc(role)}</span> → ${entityStrs.join(", ")}`);
            });
        }
        else {
            lines.push(`<span class="t-br">└─</span> <span class="t-id">(no entities with roles)</span>`);
        }
        return lines.join("\n");
    }
    function extractEntitiesFromDsl(dsl) {
        const entities = [];
        // Match entity.create-* patterns
        const entityPattern = /\(entity\.create-(\w+)[^)]*:(?:name|first-name)\s+"([^"]+)"[^)]*(?::as\s+@(\w+))?/g;
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
    async function updateContextFromBindings(bindings) {
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
    async function fetchAndDisplayCbu(cbuId) {
        console.log("[DEBUG] fetchAndDisplayCbu called with:", cbuId);
        try {
            // Use graph endpoint for full CBU structure with sorted entities
            const resp = await fetch(`/api/cbu/${cbuId}/graph`);
            console.log("[DEBUG] graph response status:", resp.status);
            if (resp.ok) {
                const graph = await resp.json();
                console.log("[DEBUG] graph response:", graph);
                // Find CBU node
                const cbuNode = graph.nodes?.find((n) => n.node_type === "cbu");
                if (cbuNode) {
                    // Update context
                    context.cbu = {
                        id: cbuId,
                        name: cbuNode.label || "Unknown",
                        jurisdiction: cbuNode.jurisdiction,
                        clientType: cbuNode.data?.client_type,
                    };
                    // Map graph to CbuData for debug panel
                    debugState.cbu = mapGraphToCbuData(graph, cbuId);
                    console.log("[DEBUG] debugState.cbu set to:", debugState.cbu);
                    updateDebugPanels();
                }
                else {
                    console.log("[DEBUG] No CBU node found in graph");
                }
            }
            else {
                console.log("[DEBUG] graph request failed:", resp.status);
            }
        }
        catch (e) {
            console.error("[DEBUG] fetchAndDisplayCbu error:", e);
            // Fallback - just use the ID
            context.cbu = {
                id: cbuId,
                name: "CBU " + cbuId.substring(0, 8),
            };
        }
        updateContextBar();
    }
    // Map graph response to CbuData interface
    function mapGraphToCbuData(graph, cbuId) {
        const cbuNode = graph.nodes.find((n) => n.node_type === "cbu");
        const entityNodes = graph.nodes
            .filter((n) => n.node_type === "entity")
            .sort((a, b) => (b.role_priority || 0) - (a.role_priority || 0)); // Sort by priority desc (ownership first)
        // Get products, services, and resources from the graph
        const productNodes = graph.nodes.filter((n) => n.node_type === "product");
        const serviceNodes = graph.nodes.filter((n) => n.node_type === "service");
        const resourceNodes = graph.nodes.filter((n) => n.node_type === "resource");
        // Combine all service-layer nodes for display
        const allServiceNodes = [
            ...productNodes,
            ...serviceNodes,
            ...resourceNodes,
        ];
        return {
            cbu_id: cbuId,
            name: cbuNode?.label || "Unknown",
            jurisdiction: cbuNode?.jurisdiction,
            client_type: cbuNode?.data?.client_type,
            entities: entityNodes.map((e) => ({
                entity_id: e.id,
                name: e.label,
                entity_type: e.sublabel || "",
                roles: e.roles || [],
            })),
            services: allServiceNodes.map((s) => ({
                instance_id: s.id,
                service_name: s.label,
                resource_type: s.sublabel || s.node_type, // Use node_type as fallback
                status: s.data?.status || "ACTIVE",
            })),
            summary: {
                entity_count: entityNodes.length,
                document_count: 0,
                service_count: allServiceNodes.length,
                screening_count: 0,
                case_count: 0,
            },
        };
    }
    // ==================== UI HELPERS ====================
    function escapeHtml(text) {
        const div = document.createElement("div");
        div.textContent = text;
        return div.innerHTML;
    }
    function clearEmptyState() {
        const empty = chatMessages.querySelector(".empty-state");
        if (empty)
            empty.remove();
    }
    function addUserMessage(text) {
        clearEmptyState();
        const msg = document.createElement("div");
        msg.className = "msg msg-user";
        msg.innerHTML = `<div class="msg-bubble">${escapeHtml(text)}</div>`;
        chatMessages.appendChild(msg);
        chatMessages.scrollTop = chatMessages.scrollHeight;
    }
    function addAssistantMessage(options) {
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
                const typeDisplay = e.entity_type_code?.replace(/_/g, " ").toLowerCase() || e.entity_type;
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
                btn.addEventListener("click", () => options.actions[i].onClick());
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
                        options.entityChoices.onSelect("create");
                    }
                    else {
                        const idx = parseInt(idxAttr || "0");
                        options.entityChoices.onSelect(options.entityChoices.results[idx]);
                    }
                });
            });
        }
        return msg;
    }
    function addExecutionResult(success, message, bindings) {
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
    function handleSuggestionSelect(num) {
        if (!pendingCorrections || num < 1 || num > pendingCorrections.length) {
            addAssistantMessage({
                text: `Please select a number between 1 and ${pendingCorrections?.length || 0}.`,
            });
            return;
        }
        const correction = pendingCorrections[num - 1];
        if (pendingDsl) {
            if (correction.type === "lookup") {
                pendingDsl = pendingDsl.replace(`"${correction.current}"`, `"${correction.suggested}"`);
            }
            else {
                pendingDsl = pendingDsl.replace(`(${correction.current}`, `(${correction.suggested}`);
            }
        }
        addUserMessage(String(num));
        validateAndPrompt(pendingDsl);
    }
    async function handleEntitySelection(choice) {
        if (!pendingEntitySelection)
            return;
        const { originalMessage } = pendingEntitySelection;
        pendingEntitySelection = null;
        if (choice === "create") {
            // User wants to create new - proceed with original message
            addUserMessage("Create new");
            await generateDslForMessage(originalMessage);
        }
        else {
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
    async function handleConfirmExecute(confirmed) {
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
    async function validateAndPrompt(dsl) {
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
            if (!resp.ok)
                throw new Error("Validation failed");
            const data = await resp.json();
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
            }
            else if (data.status === "auto_fixed") {
                pendingDsl = data.corrected_dsl;
                pendingCorrections = null;
                addAssistantMessage({
                    text: `I auto-corrected: ${data.message}`,
                    dsl: data.corrected_dsl,
                    status: { type: "valid", text: "✓ Auto-corrected, ready to run" },
                    createdEntities: extractEntitiesFromDsl(data.corrected_dsl),
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
            else if (data.status === "needs_confirmation") {
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
            }
            else {
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
        }
        catch (e) {
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
    async function executeDsl(dsl) {
        addAssistantMessage({ text: "Executing..." });
        try {
            // Use session execute endpoint if we have a session (preserves bindings)
            // Otherwise fall back to stateless execute
            const url = sessionId ? `/api/agent/execute` : `/api/dsl/execute`;
            const body = sessionId ? { session_id: sessionId, dsl } : { dsl };
            const resp = await fetch(url, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(body),
            });
            const data = await resp.json();
            if (data.success) {
                if (data.bindings) {
                    await updateContextFromBindings(data.bindings);
                }
                // Update debug panels - use input dsl as source, response for AST
                debugState.dslSource = dsl;
                if (data.ast) {
                    debugState.ast = data.ast;
                }
                updateDebugPanels();
                // Refresh CBU data if we have one in context (to show updated services, etc.)
                if (context.cbu?.id) {
                    await fetchAndDisplayCbu(context.cbu.id);
                }
                addExecutionResult(true, "Executed successfully!", data.bindings);
            }
            else {
                const errorMsg = data.errors?.join("; ") || data.error || "Execution failed";
                addExecutionResult(false, errorMsg);
            }
        }
        catch (e) {
            addExecutionResult(false, `Error: ${e.message}`);
        }
    }
    // Build context string for agent
    function getContextForAgent() {
        const parts = [];
        if (context.cbu) {
            parts.push(`Current CBU: "${context.cbu.name}" - use @cbu to reference it`);
        }
        if (context.case) {
            parts.push(`Current KYC Case: ${context.case.type} - use @case to reference it`);
        }
        if (context.bindings.size > 0) {
            const bindings = Array.from(context.bindings.entries())
                .map(([k, _]) => `@${k}`)
                .join(", ");
            parts.push(`Available bindings: ${bindings}`);
        }
        return parts.length > 0 ? parts.join("\n") : "";
    }
    async function generateDslForMessage(message) {
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
            }
            else {
                const dsl = data.assembled_dsl?.combined || data.dsl;
                if (dsl) {
                    await validateAndPrompt(dsl);
                }
                else {
                    addAssistantMessage({
                        text: data.message || "I couldn't generate DSL for that.",
                    });
                }
            }
        }
        catch (e) {
            addAssistantMessage({
                text: `Connection error: ${e.message}`,
            });
        }
    }
    async function sendMessage() {
        const text = chatInput.value.trim();
        if (!text)
            return;
        chatInput.value = "";
        // Simple confirmations
        const lower = text.toLowerCase();
        if (lower === "yes" ||
            lower === "y" ||
            lower === "run" ||
            lower === "execute") {
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
        if (!isNaN(num) &&
            pendingCorrections &&
            num >= 1 &&
            num <= pendingCorrections.length) {
            handleSuggestionSelect(num);
            return;
        }
        // Number selection for entity choices
        if (!isNaN(num) && pendingEntitySelection) {
            if (num >= 1 && num <= pendingEntitySelection.results.length) {
                handleEntitySelection(pendingEntitySelection.results[num - 1]);
                return;
            }
            else if (num === pendingEntitySelection.results.length + 1 ||
                lower === "create" ||
                lower === "new") {
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
                const goodMatches = searchResult.results.filter((r) => r.similarity >= 0.7);
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
        }
        catch (e) {
            addAssistantMessage({
                text: `Connection error: ${e.message}`,
            });
        }
        finally {
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
        if (e.key === "Enter")
            sendMessage();
    });
    newSessionBtn.addEventListener("click", newSession);
    contextClearBtn.addEventListener("click", clearContext);
    // Initialize panels
    initPanels();
    // ==================== CBU PICKER (Search Modal) ====================
    const selectCbuBtn = document.getElementById("select-cbu-btn");
    let cbuFinderState = {
        visible: false,
        searchValue: "",
        results: [],
        loading: false,
    };
    /**
     * Search CBUs using the completions API (fuzzy search via EntityGateway)
     */
    async function searchCbus(query) {
        try {
            const resp = await fetch("/api/agent/complete", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ entity_type: "cbu", query, limit: 10 }),
            });
            if (!resp.ok) {
                console.error("CBU search failed:", resp.status);
                return [];
            }
            const data = await resp.json();
            // Map completion items to CBU format
            return data.items.map((item) => ({
                cbu_id: item.value,
                name: item.label,
                jurisdiction: item.detail,
            }));
        }
        catch (e) {
            console.error("CBU search error:", e);
            return [];
        }
    }
    /**
     * Open the CBU finder modal
     */
    async function openCbuFinder() {
        cbuFinderState = {
            visible: true,
            searchValue: "",
            results: [],
            loading: true,
        };
        renderCbuFinderModal();
        // Load initial list (empty search returns all)
        try {
            const results = await searchCbus("");
            cbuFinderState.results = results;
            cbuFinderState.loading = false;
            renderCbuFinderModal();
        }
        catch (e) {
            console.error("Failed to load CBUs:", e);
            cbuFinderState.loading = false;
            renderCbuFinderModal();
        }
    }
    /**
     * Close the CBU finder modal
     */
    function closeCbuFinder() {
        cbuFinderState.visible = false;
        const modal = document.getElementById("cbu-finder-modal");
        if (modal) {
            modal.remove();
        }
    }
    /**
     * Handle CBU selection from the finder modal
     */
    async function selectCbuFromFinder(cbu) {
        closeCbuFinder();
        // Ensure we have a session first
        if (!sessionId) {
            try {
                const sessResp = await fetch("/api/agent/session", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                });
                const sessData = await sessResp.json();
                if (sessData.session_id) {
                    sessionId = sessData.session_id;
                }
            }
            catch (e) {
                console.error("Failed to create session:", e);
            }
        }
        // Register the binding in the Rust session
        if (sessionId) {
            console.log("[DEBUG] Binding CBU to session:", sessionId, "cbu_id:", cbu.cbu_id);
            try {
                const bindPayload = {
                    session_id: sessionId,
                    name: "cbu",
                    id: cbu.cbu_id,
                    entity_type: "cbu",
                    display_name: cbu.name,
                };
                console.log("[DEBUG] Bind payload:", JSON.stringify(bindPayload));
                const bindResp = await fetch("/api/agent/bind", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify(bindPayload),
                });
                console.log("[DEBUG] Bind response status:", bindResp.status);
                if (!bindResp.ok) {
                    const errText = await bindResp.text();
                    console.error("Failed to bind CBU:", bindResp.status, errText);
                }
                else {
                    const bindResult = await bindResp.json();
                    console.log("[DEBUG] Bind result:", bindResult);
                }
            }
            catch (e) {
                console.error("Failed to bind CBU:", e);
            }
        }
        else {
            console.warn("[DEBUG] No sessionId available for bind!");
        }
        // Fetch full CBU details and set as active context
        await fetchAndDisplayCbu(cbu.cbu_id);
        // Set up bindings for the selected CBU (local context)
        context.bindings.set("cbu", cbu.cbu_id);
        context.bindings.set("cbu_id", cbu.cbu_id);
        // Update the button to show selected CBU
        if (selectCbuBtn) {
            selectCbuBtn.textContent = context.cbu?.name || cbu.name;
            selectCbuBtn.classList.add("active-cbu");
        }
        // Show confirmation message in chat
        addAssistantMessage({
            text: `Selected CBU: "${context.cbu?.name || cbu.name}". You can now reference it as @cbu or @cbu_id in your commands.`,
            status: { type: "valid", text: "CBU context set" },
        });
    }
    /**
     * Render the CBU finder modal
     */
    function renderCbuFinderModal() {
        // Remove existing modal
        let modal = document.getElementById("cbu-finder-modal");
        if (modal) {
            modal.remove();
        }
        if (!cbuFinderState.visible) {
            return;
        }
        modal = document.createElement("div");
        modal.id = "cbu-finder-modal";
        modal.className = "entity-finder-overlay";
        const { searchValue, results, loading } = cbuFinderState;
        let resultsHtml = "";
        if (loading) {
            resultsHtml = `<div class="finder-loading"><span class="spinner"></span> Loading CBUs...</div>`;
        }
        else if (results.length === 0) {
            resultsHtml = `<div class="finder-empty">No CBUs found${searchValue ? ` matching "${escapeHtml(searchValue)}"` : ""}</div>`;
        }
        else {
            resultsHtml = `<div class="finder-results">`;
            results.forEach((cbu, idx) => {
                const jurisdiction = cbu.jurisdiction ? ` (${cbu.jurisdiction})` : "";
                const clientType = cbu.client_type ? ` [${cbu.client_type}]` : "";
                resultsHtml += `
          <div class="finder-result" data-result-idx="${idx}">
            <div class="finder-result-main">
              <span class="finder-result-name">${escapeHtml(cbu.name)}</span>
              <span class="finder-result-jurisdiction">${escapeHtml(jurisdiction)}</span>
            </div>
            <div class="finder-result-meta">
              <span class="finder-result-type">${escapeHtml(clientType)}</span>
            </div>
          </div>
        `;
            });
            resultsHtml += `</div>`;
        }
        modal.innerHTML = `
      <div class="entity-finder-modal">
        <div class="finder-header">
          <h3>Select CBU</h3>
          <button class="finder-close" title="Close">&times;</button>
        </div>
        <div class="finder-search-box">
          <input type="text" id="cbu-finder-search" placeholder="Search CBUs..." value="${escapeHtml(searchValue)}">
          <button id="cbu-finder-search-btn">Search</button>
        </div>
        ${resultsHtml}
      </div>
    `;
        document.body.appendChild(modal);
        // Attach event handlers
        modal
            .querySelector(".finder-close")
            ?.addEventListener("click", closeCbuFinder);
        // Click outside to close
        modal.addEventListener("click", (e) => {
            if (e.target === modal) {
                closeCbuFinder();
            }
        });
        // Search button handler
        modal
            .querySelector("#cbu-finder-search-btn")
            ?.addEventListener("click", async () => {
            const input = document.getElementById("cbu-finder-search");
            if (input) {
                cbuFinderState.searchValue = input.value.trim();
                cbuFinderState.loading = true;
                renderCbuFinderModal();
                const results = await searchCbus(cbuFinderState.searchValue);
                cbuFinderState.results = results;
                cbuFinderState.loading = false;
                renderCbuFinderModal();
            }
        });
        // Enter key in search input
        modal
            .querySelector("#cbu-finder-search")
            ?.addEventListener("keypress", (e) => {
            if (e.key === "Enter") {
                modal
                    ?.querySelector("#cbu-finder-search-btn")
                    ?.dispatchEvent(new Event("click"));
            }
        });
        // Result click handlers
        modal.querySelectorAll(".finder-result").forEach((el) => {
            el.addEventListener("click", () => {
                const idx = parseInt(el.dataset.resultIdx || "0", 10);
                const cbu = cbuFinderState.results[idx];
                if (cbu) {
                    selectCbuFromFinder(cbu);
                }
            });
        });
        // Focus search input
        document.getElementById("cbu-finder-search")?.focus();
        // Escape key to close modal
        const handleEscape = (e) => {
            if (e.key === "Escape") {
                closeCbuFinder();
                document.removeEventListener("keydown", handleEscape);
            }
        };
        document.addEventListener("keydown", handleEscape);
    }
    // Initialize CBU picker button
    if (selectCbuBtn) {
        selectCbuBtn.addEventListener("click", openCbuFinder);
    }
});
//# sourceMappingURL=app.js.map