//! HTML page generation using DSL infrastructure
//!
//! Server-rendered pages that directly use parse_program, CsgLinter, etc.

use crate::dsl_v2::verb_registry::registry;

/// Main agent UI page
pub fn index_page(session_id: Option<&str>) -> String {
    let reg = registry();
    let domain_list = reg.domains();
    let total_verbs = reg.len();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DSL Agent</title>
    <style>
{css}
    </style>
</head>
<body>
    <header>
        <h1>DSL Agent</h1>
        <div class="session-info">
            <span id="session-id">{session}</span>
            <span class="domains">{domains} domains | {verbs} verbs</span>
        </div>
    </header>

    <main>
        <section class="chat-panel">
            <h2>Chat</h2>
            <div id="chat-history" class="chat-history"></div>
            <form id="chat-form" class="chat-form">
                <textarea id="chat-input" placeholder="Describe what you want to create..." rows="3"></textarea>
                <button type="submit">Send</button>
            </form>
        </section>

        <section class="dsl-panel">
            <h2>DSL</h2>
            <textarea id="dsl-editor" class="dsl-editor" placeholder="; DSL will appear here&#10;; or type directly..."></textarea>
            <div class="dsl-actions">
                <button id="validate-btn">Validate</button>
                <button id="execute-btn" class="primary">Execute</button>
            </div>
            <div id="validation-result" class="validation-result"></div>
        </section>

        <section class="results-panel">
            <h2>Results</h2>
            <div id="results" class="results"></div>
        </section>
    </main>

    <script>
{js}
    </script>
</body>
</html>"#,
        css = CSS,
        js = JS,
        session = session_id.unwrap_or("No session"),
        domains = domain_list.len(),
        verbs = total_verbs,
    )
}

/// Render verb reference page
pub fn verbs_page() -> String {
    let reg = registry();
    let domain_list = reg.domains();
    let mut verb_html = String::new();

    for domain in domain_list {
        verb_html.push_str(&format!(
            r#"<div class="domain">
            <h3>{}</h3>
            <ul>"#,
            domain
        ));

        for verb in reg.verbs_for_domain(domain) {
            verb_html.push_str(&format!(
                r#"<li>
                    <code>{}.{}</code>
                    <span class="desc">{}</span>
                    <div class="args">
                        <span class="required">Required: {}</span>
                        <span class="optional">Optional: {}</span>
                    </div>
                </li>"#,
                verb.domain,
                verb.verb,
                verb.description,
                verb.required_arg_names().join(", "),
                verb.optional_arg_names().join(", "),
            ));
        }

        verb_html.push_str("</ul></div>");
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>DSL Verb Reference</title>
    <style>
        body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; }}
        h1 {{ border-bottom: 2px solid #333; padding-bottom: 10px; }}
        .domain {{ margin: 20px 0; }}
        .domain h3 {{ background: #f0f0f0; padding: 10px; margin: 0; }}
        .domain ul {{ list-style: none; padding: 0; margin: 0; }}
        .domain li {{ padding: 10px; border-bottom: 1px solid #eee; }}
        .domain code {{ font-weight: bold; color: #0066cc; }}
        .desc {{ display: block; margin: 5px 0; color: #666; }}
        .args {{ font-size: 0.85em; color: #888; }}
        .required {{ margin-right: 20px; }}
    </style>
</head>
<body>
    <h1>DSL Verb Reference</h1>
    <p><a href="/">← Back to Agent</a></p>
    {}
</body>
</html>"#,
        verb_html
    )
}

const CSS: &str = r#"
* { box-sizing: border-box; margin: 0; padding: 0; }

body {
    font-family: system-ui, -apple-system, sans-serif;
    background: #1a1a2e;
    color: #eee;
    height: 100vh;
    display: flex;
    flex-direction: column;
}

header {
    background: #16213e;
    padding: 12px 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid #0f3460;
}

header h1 {
    font-size: 1.4em;
    color: #e94560;
}

.session-info {
    font-size: 0.85em;
    color: #888;
}

.session-info .domains {
    margin-left: 20px;
    color: #666;
}

main {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 1.5fr 1fr;
    gap: 1px;
    background: #0f3460;
    overflow: hidden;
}

section {
    background: #1a1a2e;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

section h2 {
    padding: 10px 15px;
    font-size: 0.9em;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #888;
    border-bottom: 1px solid #0f3460;
}

/* Chat Panel */
.chat-history {
    flex: 1;
    overflow-y: auto;
    padding: 15px;
}

.chat-msg {
    margin-bottom: 15px;
    padding: 10px 12px;
    border-radius: 8px;
    max-width: 90%;
}

.chat-msg.user {
    background: #0f3460;
    margin-left: auto;
}

.chat-msg.agent {
    background: #16213e;
    border-left: 3px solid #e94560;
}

.chat-form {
    padding: 15px;
    border-top: 1px solid #0f3460;
}

.chat-form textarea {
    width: 100%;
    background: #16213e;
    border: 1px solid #0f3460;
    color: #eee;
    padding: 10px;
    border-radius: 6px;
    resize: none;
    font-family: inherit;
}

.chat-form button {
    margin-top: 10px;
    width: 100%;
    padding: 10px;
    background: #e94560;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 600;
}

.chat-form button:hover {
    background: #ff6b6b;
}

/* DSL Panel */
.dsl-editor {
    flex: 1;
    background: #0d1117;
    border: none;
    color: #c9d1d9;
    padding: 15px;
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 14px;
    line-height: 1.5;
    resize: none;
}

.dsl-actions {
    padding: 10px 15px;
    display: flex;
    gap: 10px;
    border-top: 1px solid #0f3460;
}

.dsl-actions button {
    flex: 1;
    padding: 10px;
    border: 1px solid #0f3460;
    background: #16213e;
    color: #eee;
    border-radius: 6px;
    cursor: pointer;
}

.dsl-actions button.primary {
    background: #238636;
    border-color: #238636;
}

.dsl-actions button:hover {
    opacity: 0.9;
}

.validation-result {
    padding: 10px 15px;
    font-size: 0.85em;
}

.validation-result.valid {
    color: #3fb950;
}

.validation-result.invalid {
    color: #f85149;
}

/* Results Panel */
.results {
    flex: 1;
    overflow-y: auto;
    padding: 15px;
    font-family: 'SF Mono', monospace;
    font-size: 13px;
}

.result-item {
    margin-bottom: 15px;
    padding: 10px;
    background: #16213e;
    border-radius: 6px;
}

.result-item.success {
    border-left: 3px solid #3fb950;
}

.result-item.error {
    border-left: 3px solid #f85149;
}

.result-item .label {
    font-size: 0.8em;
    color: #888;
    margin-bottom: 5px;
}
"#;

const JS: &str = r#"
(function() {
    let sessionId = null;

    const chatHistory = document.getElementById('chat-history');
    const chatForm = document.getElementById('chat-form');
    const chatInput = document.getElementById('chat-input');
    const dslEditor = document.getElementById('dsl-editor');
    const validateBtn = document.getElementById('validate-btn');
    const executeBtn = document.getElementById('execute-btn');
    const validationResult = document.getElementById('validation-result');
    const results = document.getElementById('results');

    // Create session on load
    async function init() {
        try {
            const res = await fetch('/api/session', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: '{}' });
            const data = await res.json();
            sessionId = data.session_id;
            document.getElementById('session-id').textContent = 'Session: ' + sessionId.slice(0, 8) + '...';
            addChat('agent', 'Session created. Describe what you want to create, or type DSL directly.');
        } catch (e) {
            addChat('agent', 'Failed to create session: ' + e.message);
        }
    }

    function addChat(role, text) {
        const div = document.createElement('div');
        div.className = 'chat-msg ' + role;
        div.textContent = text;
        chatHistory.appendChild(div);
        chatHistory.scrollTop = chatHistory.scrollHeight;
    }

    function addResult(label, content, isError) {
        const div = document.createElement('div');
        div.className = 'result-item ' + (isError ? 'error' : 'success');
        div.innerHTML = '<div class="label">' + label + '</div><pre>' + escapeHtml(content) + '</pre>';
        results.insertBefore(div, results.firstChild);
    }

    function escapeHtml(str) {
        return String(str).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    // Chat submit - generate DSL from natural language
    chatForm.addEventListener('submit', async (e) => {
        e.preventDefault();
        const msg = chatInput.value.trim();
        if (!msg) return;

        addChat('user', msg);
        chatInput.value = '';

        // Check if it looks like DSL already
        if (msg.startsWith('(')) {
            dslEditor.value = msg;
            addChat('agent', 'DSL detected. Click Validate or Execute.');
            return;
        }

        // Try to generate DSL
        try {
            addChat('agent', 'Generating DSL...');
            const res = await fetch('/api/agent/generate', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ instruction: msg })
            });
            const data = await res.json();

            if (data.dsl) {
                dslEditor.value = data.dsl;
                addChat('agent', data.explanation || 'DSL generated. Review and execute.');
            } else if (data.error) {
                addChat('agent', 'Error: ' + data.error);
            } else {
                addChat('agent', 'Could not generate DSL. Try being more specific or type DSL directly.');
            }
        } catch (e) {
            addChat('agent', 'Generation failed: ' + e.message);
        }
    });

    // Validate DSL
    validateBtn.addEventListener('click', async () => {
        const dsl = dslEditor.value.trim();
        if (!dsl) return;

        try {
            const res = await fetch('/api/agent/validate', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ dsl })
            });
            const data = await res.json();

            if (data.valid) {
                validationResult.textContent = '✓ Valid DSL';
                validationResult.className = 'validation-result valid';
            } else {
                validationResult.textContent = '✗ ' + (data.errors || []).map(e => e.message).join('; ');
                validationResult.className = 'validation-result invalid';
            }
        } catch (e) {
            validationResult.textContent = '✗ Validation error: ' + e.message;
            validationResult.className = 'validation-result invalid';
        }
    });

    // Execute DSL
    executeBtn.addEventListener('click', async () => {
        const dsl = dslEditor.value.trim();
        if (!dsl || !sessionId) return;

        try {
            const res = await fetch('/api/session/' + sessionId + '/execute', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ dsl })
            });
            const data = await res.json();

            if (data.success) {
                addResult('Executed', JSON.stringify(data.results, null, 2), false);
                addChat('agent', 'Execution successful.');
            } else {
                addResult('Error', data.error || 'Execution failed', true);
                addChat('agent', 'Execution failed: ' + (data.error || 'Unknown error'));
            }
        } catch (e) {
            addResult('Error', e.message, true);
            addChat('agent', 'Execution error: ' + e.message);
        }
    });

    init();
})();
"#;
