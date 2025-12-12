// AST Panel - Tree view for AST structure
export class AstPanel {
    constructor(callbacks) {
        this.selectedNodeId = null;
        this.treeEl = document.getElementById('ast-tree');
        this.expandAllBtn = document.getElementById('ast-expand-all');
        this.collapseAllBtn = document.getElementById('ast-collapse-all');
        this.callbacks = callbacks;
        this.setupEventListeners();
    }
    setupEventListeners() {
        this.expandAllBtn.addEventListener('click', () => this.expandAll());
        this.collapseAllBtn.addEventListener('click', () => this.collapseAll());
        this.treeEl.addEventListener('click', (e) => {
            const target = e.target;
            // Toggle node expansion
            if (target.classList.contains('ast-toggle')) {
                const nodeEl = target.closest('.ast-node');
                const childrenEl = nodeEl?.querySelector('.ast-children');
                if (childrenEl) {
                    childrenEl.classList.toggle('collapsed');
                    target.textContent = childrenEl.classList.contains('collapsed') ? '▶' : '▼';
                }
                return;
            }
            // Select node
            const nodeEl = target.closest('.ast-node');
            if (nodeEl) {
                this.selectNode(nodeEl.dataset.id || null);
            }
        });
    }
    setAst(statements) {
        if (!statements || statements.length === 0) {
            this.treeEl.innerHTML = '<span style="color: var(--text-muted)">AST will appear here</span>';
            return;
        }
        let html = '';
        let nodeId = 0;
        for (const stmt of statements) {
            if (stmt.VerbCall) {
                html += this.renderVerbCall(stmt.VerbCall, nodeId++);
            }
            else if (stmt.Comment) {
                html += this.renderComment(stmt.Comment, nodeId++);
            }
        }
        this.treeEl.innerHTML = html;
    }
    renderVerbCall(vc, nodeId) {
        const fullVerb = `${vc.domain}.${vc.verb}`;
        const bindingStr = vc.binding ? ` <span class="ast-symbol">@${vc.binding}</span>` : '';
        const argsHtml = vc.arguments.map((arg, idx) => this.renderArgument(arg, `${nodeId}-arg-${idx}`)).join('');
        return `
            <div class="ast-node" data-id="${nodeId}" data-type="VerbCall">
                <span class="ast-toggle">▼</span>
                <span class="ast-type">VerbCall</span>
                <span class="ast-name">${fullVerb}</span>${bindingStr}
                <div class="ast-children">
                    ${argsHtml}
                </div>
            </div>
        `;
    }
    renderArgument(arg, nodeId) {
        const valueHtml = this.renderValue(arg.value, `${nodeId}-val`);
        return `
            <div class="ast-node" data-id="${nodeId}" data-type="Argument">
                <span class="ast-toggle">${this.hasChildren(arg.value) ? '▼' : ' '}</span>
                <span class="ast-keyword">:${arg.key}</span>
                <div class="ast-children">
                    ${valueHtml}
                </div>
            </div>
        `;
    }
    renderValue(value, nodeId) {
        // Literal
        if (this.isLiteral(value)) {
            const lit = value.Literal;
            return this.renderLiteral(lit, nodeId);
        }
        // SymbolRef
        if (this.isSymbolRef(value)) {
            const sym = value.SymbolRef;
            return `
                <div class="ast-node" data-id="${nodeId}" data-type="SymbolRef">
                    <span class="ast-toggle"> </span>
                    <span class="ast-type">Symbol</span>
                    <span class="ast-symbol">@${sym.name}</span>
                </div>
            `;
        }
        // EntityRef
        if (this.isEntityRef(value)) {
            const ref = value.EntityRef;
            const unresolvedClass = ref.resolved_key ? '' : 'unresolved';
            const resolvedStr = ref.resolved_key
                ? `<span class="ast-resolved">→ ${ref.resolved_key.substring(0, 8)}...</span>`
                : '<span class="ast-unresolved">(unresolved)</span>';
            return `
                <div class="ast-node ${unresolvedClass}" data-id="${nodeId}" data-type="EntityRef" data-entity-type="${ref.entity_type}">
                    <span class="ast-toggle"> </span>
                    <span class="ast-type">${ref.entity_type}</span>
                    <span class="ast-value">"${ref.value}"</span>
                    ${resolvedStr}
                </div>
            `;
        }
        // Array
        if (Array.isArray(value)) {
            const items = value.map((v, idx) => this.renderValue(v, `${nodeId}-${idx}`)).join('');
            return `
                <div class="ast-node" data-id="${nodeId}" data-type="List">
                    <span class="ast-toggle">${value.length > 0 ? '▼' : ' '}</span>
                    <span class="ast-type">List</span>
                    <span class="ast-value">[${value.length}]</span>
                    <div class="ast-children">
                        ${items}
                    </div>
                </div>
            `;
        }
        // Object/Map
        if (typeof value === 'object') {
            const entries = Object.entries(value)
                .filter(([k]) => k !== 'Literal' && k !== 'SymbolRef' && k !== 'EntityRef');
            const items = entries.map(([k, v], idx) => `
                <div class="ast-node" data-id="${nodeId}-${idx}">
                    <span class="ast-keyword">:${k}</span>
                    ${this.renderValue(v, `${nodeId}-${idx}-val`)}
                </div>
            `).join('');
            return `
                <div class="ast-node" data-id="${nodeId}" data-type="Map">
                    <span class="ast-toggle">${entries.length > 0 ? '▼' : ' '}</span>
                    <span class="ast-type">Map</span>
                    <div class="ast-children">
                        ${items}
                    </div>
                </div>
            `;
        }
        return `<span class="ast-value">${String(value)}</span>`;
    }
    renderLiteral(lit, nodeId) {
        let typeStr = '';
        let valueStr = '';
        if (lit === 'Null' || lit === null) {
            typeStr = 'Null';
            valueStr = 'nil';
        }
        else if (lit.String !== undefined) {
            typeStr = 'String';
            valueStr = `"${lit.String}"`;
        }
        else if (lit.Integer !== undefined) {
            typeStr = 'Integer';
            valueStr = String(lit.Integer);
        }
        else if (lit.Decimal !== undefined) {
            typeStr = 'Decimal';
            valueStr = String(lit.Decimal);
        }
        else if (lit.Boolean !== undefined) {
            typeStr = 'Boolean';
            valueStr = String(lit.Boolean);
        }
        else {
            typeStr = 'Unknown';
            valueStr = JSON.stringify(lit);
        }
        return `
            <div class="ast-node" data-id="${nodeId}" data-type="Literal">
                <span class="ast-toggle"> </span>
                <span class="ast-type">${typeStr}</span>
                <span class="ast-value">${this.escapeHtml(valueStr)}</span>
            </div>
        `;
    }
    isLiteral(value) {
        return value && typeof value === 'object' && 'Literal' in value;
    }
    isSymbolRef(value) {
        return value && typeof value === 'object' && 'SymbolRef' in value;
    }
    isEntityRef(value) {
        return value && typeof value === 'object' && 'EntityRef' in value;
    }
    hasChildren(value) {
        if (Array.isArray(value))
            return value.length > 0;
        if (this.isLiteral(value) || this.isSymbolRef(value) || this.isEntityRef(value))
            return false;
        if (typeof value === 'object')
            return Object.keys(value).length > 0;
        return false;
    }
    renderComment(comment, nodeId) {
        return `
            <div class="ast-node" data-id="${nodeId}" data-type="Comment">
                <span class="ast-toggle"> </span>
                <span class="ast-type">Comment</span>
                <span class="ast-comment">; ${this.escapeHtml(comment)}</span>
            </div>
        `;
    }
    selectNode(nodeId) {
        // Deselect previous
        this.treeEl.querySelectorAll('.ast-node.selected').forEach(el => {
            el.classList.remove('selected');
        });
        this.selectedNodeId = nodeId;
        if (nodeId) {
            const nodeEl = this.treeEl.querySelector(`[data-id="${nodeId}"]`);
            nodeEl?.classList.add('selected');
            this.callbacks.onNodeSelected(nodeId);
        }
    }
    highlightNode(nodeId) {
        this.selectNode(nodeId);
        // Scroll into view
        const nodeEl = this.treeEl.querySelector(`[data-id="${nodeId}"]`);
        nodeEl?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
    expandAll() {
        this.treeEl.querySelectorAll('.ast-children').forEach(el => {
            el.classList.remove('collapsed');
        });
        this.treeEl.querySelectorAll('.ast-toggle').forEach(el => {
            if (el.textContent === '▶')
                el.textContent = '▼';
        });
    }
    collapseAll() {
        this.treeEl.querySelectorAll('.ast-children').forEach(el => {
            el.classList.add('collapsed');
        });
        this.treeEl.querySelectorAll('.ast-toggle').forEach(el => {
            if (el.textContent === '▼')
                el.textContent = '▶';
        });
    }
    escapeHtml(text) {
        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;');
    }
}
