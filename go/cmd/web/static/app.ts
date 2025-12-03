document.addEventListener("DOMContentLoaded", () => {
    // State
    let currentDsl = "";
    let sessionId: string | null = null;

    // DOM elements
    const chatMessages = document.getElementById("chat-messages") as HTMLDivElement;
    const chatInput = document.getElementById("chat-input") as HTMLInputElement;
    const sendBtn = document.getElementById("send-btn") as HTMLButtonElement;
    const newSessionBtn = document.getElementById("new-session-btn") as HTMLButtonElement;
    const dslCode = document.getElementById("dsl-code") as HTMLDivElement;
    const dslStatus = document.getElementById("dsl-status") as HTMLSpanElement;
    const executeBtn = document.getElementById("execute-btn") as HTMLButtonElement;
    const copyBtn = document.getElementById("copy-btn") as HTMLButtonElement;
    const validateBtn = document.getElementById("validate-btn") as HTMLButtonElement;
    const resultsDiv = document.getElementById("results") as HTMLDivElement;
    const settingsBtn = document.getElementById("settings-btn") as HTMLButtonElement;
    const settingsModal = document.getElementById("settings-modal") as HTMLDivElement;
    const settingsSave = document.getElementById("settings-save") as HTMLButtonElement;
    const settingsCancel = document.getElementById("settings-cancel") as HTMLButtonElement;
    const rustUrlInput = document.getElementById("rust-url") as HTMLInputElement;
    const agentUrlInput = document.getElementById("agent-url") as HTMLInputElement;

    // Utils
    function escapeHtml(text: string): string {
        const div = document.createElement("div");
        div.textContent = text;
        return div.innerHTML;
    }

    function addMessage(role: "user" | "assistant", content: string, dsl?: string) {
        const empty = chatMessages.querySelector(".empty-state");
        if (empty) empty.remove();

        const msg = document.createElement("div");
        msg.className = `msg msg-${role}`;
        
        let html = `<div class="msg-bubble">${escapeHtml(content)}</div>`;
        if (dsl) {
            html += `<div class="msg-dsl">${escapeHtml(dsl)}</div>`;
        }
        msg.innerHTML = html;
        chatMessages.appendChild(msg);
        chatMessages.scrollTop = chatMessages.scrollHeight;
    }

    function setDsl(dsl: string) {
        currentDsl = dsl;
        dslCode.textContent = dsl || "; DSL will appear here after generation";
        executeBtn.disabled = !dsl;
        if (dsl) {
            dslStatus.textContent = "Ready to execute";
        }
    }

    function showResult(success: boolean, message: string, bindings?: Record<string, string>) {
        resultsDiv.style.display = "block";
        let html = `<div class="result-box ${success ? "success" : "error"}">
            <div class="result-header">${success ? "[OK]" : "[X]"} ${escapeHtml(message)}</div>`;
        
        if (bindings && Object.keys(bindings).length > 0) {
            html += `<div class="bindings">`;
            for (const [k, v] of Object.entries(bindings)) {
                html += `<span class="binding">@${k} -> ${v.substring(0, 8)}...</span>`;
            }
            html += `</div>`;
        }
        html += `</div>`;
        resultsDiv.innerHTML = html;
    }

    // API calls - using Go proxy endpoints
    async function sendMessage() {
        const text = chatInput.value.trim();
        if (!text) return;

        chatInput.value = "";
        addMessage("user", text);
        
        sendBtn.disabled = true;
        sendBtn.innerHTML = `<span class="spinner"></span>Generating...`;

        try {
            // Create session if needed (via Go proxy)
            if (!sessionId) {
                const sessResp = await fetch("/api/agent/session", { 
                    method: "POST",
                    headers: { "Content-Type": "application/json" }
                });
                const sessData = await sessResp.json();
                if (sessData.error) {
                    addMessage("assistant", "Error: " + sessData.error);
                    return;
                }
                sessionId = sessData.session_id;
            }

            // Send chat message (via Go proxy)
            const resp = await fetch("/api/agent/chat", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ session_id: sessionId, message: text })
            });
            const data = await resp.json();
            console.log("Chat response:", data);

            if (data.error) {
                addMessage("assistant", "Error: " + data.error);
            } else {
                const dsl = data.assembled_dsl?.combined || data.dsl;
                console.log("DSL extracted:", dsl);
                addMessage("assistant", data.message || "DSL generated", dsl);
                if (dsl) {
                    console.log("Calling setDsl with:", dsl);
                    setDsl(dsl);
                } else {
                    console.log("No DSL found in response");
                }
            }
        } catch (e) {
            addMessage("assistant", "Failed to connect: " + (e as Error).message);
        } finally {
            sendBtn.disabled = false;
            sendBtn.textContent = "Send";
        }
    }

    async function executeDsl() {
        if (!currentDsl || !sessionId) return;

        executeBtn.disabled = true;
        executeBtn.innerHTML = `<span class="spinner"></span>Executing...`;
        dslStatus.textContent = "Executing...";

        try {
            const resp = await fetch("/api/agent/execute", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    session_id: sessionId,
                    dsl: currentDsl
                })
            });
            const data = await resp.json();
            console.log("Execute response:", data);

            if (data.error) {
                showResult(false, data.error);
                dslStatus.textContent = "Execution failed";
            } else if (data.success) {
                const bindings: Record<string, string> = {};
                data.results?.forEach((r: any) => {
                    if (r.entity_id) {
                        bindings[r.entity_type || "entity"] = r.entity_id;
                    }
                });
                showResult(true, "Execution successful", bindings);
                dslStatus.textContent = "Executed successfully";
            } else {
                const errors = data.errors?.join(", ") || "Execution failed";
                showResult(false, errors);
                dslStatus.textContent = "Execution failed";
            }
        } catch (e) {
            showResult(false, (e as Error).message);
            dslStatus.textContent = "Execution failed";
        } finally {
            executeBtn.disabled = false;
            executeBtn.textContent = "Execute";
        }
    }

    async function validateDsl() {
        if (!currentDsl) return;

        validateBtn.disabled = true;
        validateBtn.textContent = "Validating...";

        try {
            const resp = await fetch("/api/validate", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ dsl: currentDsl })
            });
            const data = await resp.json();

            if (data.valid) {
                showResult(true, "DSL is valid");
            } else {
                const errors = (data.errors || []).map((e: {message: string}) => e.message).join(", ");
                showResult(false, errors || "Invalid DSL");
            }
        } catch (e) {
            showResult(false, (e as Error).message);
        } finally {
            validateBtn.disabled = false;
            validateBtn.textContent = "Validate";
        }
    }

    function newSession() {
        sessionId = null;
        chatMessages.innerHTML = `<div class="empty-state">
            <p><strong>Describe what you want to create</strong></p>
            <p>e.g., "Create a fund in Luxembourg with John Smith as director"</p>
        </div>`;
        setDsl("");
        resultsDiv.style.display = "none";
        resultsDiv.innerHTML = "";
    }

    function copyDsl() {
        if (currentDsl) {
            navigator.clipboard.writeText(currentDsl);
            copyBtn.textContent = "Copied!";
            setTimeout(() => { copyBtn.textContent = "Copy"; }, 1500);
        }
    }

    // Settings
    function openSettings() { settingsModal.style.display = "block"; }
    function closeSettings() { settingsModal.style.display = "none"; }
    
    async function saveSettings() {
        const rustUrl = rustUrlInput.value;
        const agentUrl = agentUrlInput.value;
        await fetch("/api/config", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ rust_url: rustUrl, agent_url: agentUrl })
        });
        closeSettings();
        location.reload();
    }

    // Event listeners
    sendBtn.addEventListener("click", sendMessage);
    chatInput.addEventListener("keypress", (e) => { if (e.key === "Enter") sendMessage(); });
    newSessionBtn.addEventListener("click", newSession);
    executeBtn.addEventListener("click", executeDsl);
    validateBtn.addEventListener("click", validateDsl);
    copyBtn.addEventListener("click", copyDsl);
    settingsBtn.addEventListener("click", openSettings);
    settingsCancel.addEventListener("click", closeSettings);
    settingsSave.addEventListener("click", saveSettings);
    settingsModal.addEventListener("click", (e) => { if (e.target === settingsModal) closeSettings(); });
});
