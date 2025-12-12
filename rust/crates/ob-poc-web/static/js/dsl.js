// DSL Panel - Syntax highlighting for DSL source
export class DslPanel {
    constructor() {
        this.currentSource = '';
        this.codeEl = document.getElementById('dsl-source');
        this.statusEl = document.getElementById('dsl-status');
    }
    setSource(dsl) {
        this.currentSource = dsl;
        if (!dsl || dsl.trim() === '') {
            this.codeEl.innerHTML = '<span class="dsl-comment">; DSL will appear here after chat</span>';
            this.updateStatus('empty');
            return;
        }
        // Apply syntax highlighting
        this.codeEl.innerHTML = this.highlight(dsl);
        this.updateStatus('pending');
    }
    markExecuted() {
        this.updateStatus('executed');
    }
    markError() {
        this.updateStatus('error');
    }
    getSource() {
        return this.currentSource;
    }
    highlightLine(lineNumber) {
        // Remove existing highlights
        this.codeEl.querySelectorAll('.line-highlight').forEach(el => {
            el.classList.remove('line-highlight');
        });
        // Add highlight to specific line
        const lines = this.codeEl.querySelectorAll('.dsl-line');
        if (lines[lineNumber]) {
            lines[lineNumber].classList.add('line-highlight');
        }
    }
    highlight(dsl) {
        const lines = dsl.split('\n');
        return lines.map((line, idx) => {
            const highlighted = this.highlightLine_(line);
            return `<span class="dsl-line" data-line="${idx}">${highlighted}</span>`;
        }).join('\n');
    }
    highlightLine_(line) {
        // Comment
        if (line.trim().startsWith(';')) {
            return `<span class="dsl-comment">${this.escapeHtml(line)}</span>`;
        }
        let result = line;
        // Escape HTML first
        result = this.escapeHtml(result);
        // Keywords (:keyword)
        result = result.replace(/(:[\w-]+)/g, '<span class="dsl-keyword">$1</span>');
        // Verb calls (domain.verb)
        result = result.replace(/\b([\w-]+)\.([\w-]+)\b/g, '<span class="dsl-verb">$1.$2</span>');
        // Strings ("...")
        result = result.replace(/"([^"\\]|\\.)*"/g, '<span class="dsl-string">$&</span>');
        // Symbols (@name)
        result = result.replace(/@[\w_]+/g, '<span class="dsl-symbol">$&</span>');
        // Brackets
        result = result.replace(/[()[\]{}]/g, '<span class="dsl-bracket">$&</span>');
        return result;
    }
    escapeHtml(text) {
        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;');
    }
    updateStatus(status) {
        this.statusEl.textContent = status;
        this.statusEl.className = 'status-badge';
        switch (status) {
            case 'pending':
                this.statusEl.classList.add('pending');
                break;
            case 'executed':
                this.statusEl.classList.add('executed');
                break;
            case 'error':
                this.statusEl.classList.add('error');
                break;
        }
    }
}
