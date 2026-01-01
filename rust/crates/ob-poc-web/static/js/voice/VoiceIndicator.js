/**
 * VoiceIndicator - Mic button with visual state feedback
 *
 * Visual states:
 * - IDLE: Gray mic icon, subtle pulse
 * - LISTENING: Green mic with pulse animation (Blade Runner style)
 * - PROCESSING: Yellow spinner
 * - ERROR: Red with shake animation
 *
 * Follows egui pattern: returns actions, doesn't mutate external state directly
 */

import { VoiceState } from './types.js';

/**
 * Create the VoiceIndicator component
 * @param {Object} options - Configuration options
 * @param {HTMLElement} options.container - Parent element to mount into
 * @param {Function} options.onToggle - Callback when mic is clicked (returns action)
 * @returns {Object} VoiceIndicator controller
 */
export function createVoiceIndicator(options = {}) {
    const { container, onToggle } = options;

    if (!container) {
        console.error('VoiceIndicator: container is required');
        return null;
    }

    // Create the indicator element
    const indicator = document.createElement('div');
    indicator.id = 'voice-indicator';
    indicator.className = 'voice-indicator voice-idle';
    indicator.innerHTML = `
        <button class="voice-button" aria-label="Toggle voice input" title="Voice input (V)">
            <svg class="voice-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
                <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
                <line x1="12" y1="19" x2="12" y2="23"/>
                <line x1="8" y1="23" x2="16" y2="23"/>
            </svg>
            <div class="voice-pulse"></div>
            <div class="voice-spinner"></div>
        </button>
        <div class="voice-provider-badge"></div>
    `;

    // Add styles
    const styles = document.createElement('style');
    styles.textContent = `
        .voice-indicator {
            position: fixed;
            bottom: 20px;
            right: 20px;
            z-index: 1000;
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 4px;
        }

        .voice-button {
            width: 56px;
            height: 56px;
            border-radius: 50%;
            border: 2px solid transparent;
            background: rgba(30, 30, 35, 0.9);
            cursor: pointer;
            position: relative;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s ease;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .voice-button:hover {
            transform: scale(1.05);
            box-shadow: 0 6px 16px rgba(0, 0, 0, 0.4);
        }

        .voice-button:active {
            transform: scale(0.95);
        }

        .voice-icon {
            width: 24px;
            height: 24px;
            color: #888;
            transition: color 0.2s ease;
        }

        .voice-pulse {
            position: absolute;
            inset: -4px;
            border-radius: 50%;
            border: 2px solid transparent;
            opacity: 0;
            pointer-events: none;
        }

        .voice-spinner {
            position: absolute;
            inset: -4px;
            border-radius: 50%;
            border: 3px solid transparent;
            border-top-color: #f0a500;
            opacity: 0;
            pointer-events: none;
        }

        .voice-provider-badge {
            font-size: 10px;
            color: #666;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            opacity: 0;
            transition: opacity 0.2s ease;
        }

        /* ============================================= */
        /* IDLE STATE - Subtle presence */
        /* ============================================= */
        .voice-idle .voice-icon {
            color: #666;
        }

        .voice-idle .voice-button {
            border-color: rgba(100, 100, 100, 0.3);
        }

        .voice-idle .voice-pulse {
            opacity: 0.3;
            border-color: rgba(100, 100, 100, 0.5);
            animation: idle-pulse 3s ease-in-out infinite;
        }

        @keyframes idle-pulse {
            0%, 100% { transform: scale(1); opacity: 0.3; }
            50% { transform: scale(1.1); opacity: 0.1; }
        }

        /* ============================================= */
        /* LISTENING STATE - Active, Blade Runner glow */
        /* ============================================= */
        .voice-listening .voice-icon {
            color: #00ff88;
        }

        .voice-listening .voice-button {
            border-color: #00ff88;
            background: rgba(0, 255, 136, 0.1);
            box-shadow: 0 0 20px rgba(0, 255, 136, 0.3), 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .voice-listening .voice-pulse {
            opacity: 1;
            border-color: #00ff88;
            animation: listening-pulse 1.5s ease-out infinite;
        }

        .voice-listening .voice-provider-badge {
            opacity: 1;
            color: #00ff88;
        }

        @keyframes listening-pulse {
            0% { transform: scale(1); opacity: 0.8; }
            100% { transform: scale(1.5); opacity: 0; }
        }

        /* ============================================= */
        /* PROCESSING STATE - Thinking */
        /* ============================================= */
        .voice-processing .voice-icon {
            color: #f0a500;
        }

        .voice-processing .voice-button {
            border-color: #f0a500;
            background: rgba(240, 165, 0, 0.1);
        }

        .voice-processing .voice-spinner {
            opacity: 1;
            animation: spin 1s linear infinite;
        }

        .voice-processing .voice-provider-badge {
            opacity: 1;
            color: #f0a500;
        }

        @keyframes spin {
            from { transform: rotate(0deg); }
            to { transform: rotate(360deg); }
        }

        /* ============================================= */
        /* ERROR STATE - Attention needed */
        /* ============================================= */
        .voice-error .voice-icon {
            color: #ff4444;
        }

        .voice-error .voice-button {
            border-color: #ff4444;
            background: rgba(255, 68, 68, 0.1);
            animation: shake 0.5s ease-out;
        }

        .voice-error .voice-provider-badge {
            opacity: 1;
            color: #ff4444;
        }

        @keyframes shake {
            0%, 100% { transform: translateX(0); }
            20%, 60% { transform: translateX(-4px); }
            40%, 80% { transform: translateX(4px); }
        }

        /* ============================================= */
        /* Keyboard shortcut hint on hover */
        /* ============================================= */
        .voice-button::after {
            content: 'V';
            position: absolute;
            bottom: -24px;
            font-size: 10px;
            color: #666;
            background: rgba(0, 0, 0, 0.8);
            padding: 2px 6px;
            border-radius: 3px;
            opacity: 0;
            transition: opacity 0.2s ease;
            pointer-events: none;
        }

        .voice-button:hover::after {
            opacity: 1;
        }
    `;

    // Mount to container
    document.head.appendChild(styles);
    container.appendChild(indicator);

    // State
    let currentState = VoiceState.IDLE;
    let currentProvider = '';

    const button = indicator.querySelector('.voice-button');
    const badge = indicator.querySelector('.voice-provider-badge');

    // Click handler - returns action via callback
    button.addEventListener('click', () => {
        if (onToggle) {
            // Return action based on current state
            const action = currentState === VoiceState.LISTENING ? 'stop' : 'start';
            onToggle(action);
        }
    });

    // Keyboard shortcut (V key)
    document.addEventListener('keydown', (e) => {
        // Don't trigger if typing in an input
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
            return;
        }
        if (e.key.toLowerCase() === 'v' && !e.ctrlKey && !e.metaKey && !e.altKey) {
            button.click();
        }
    });

    return {
        /**
         * Update the visual state
         * @param {string} state - VoiceState value
         * @param {string} provider - Provider name (optional)
         */
        setState(state, provider = '') {
            currentState = state;
            currentProvider = provider;

            // Remove all state classes
            indicator.classList.remove('voice-idle', 'voice-listening', 'voice-processing', 'voice-error');

            // Add current state class
            switch (state) {
                case VoiceState.LISTENING:
                    indicator.classList.add('voice-listening');
                    break;
                case VoiceState.PROCESSING:
                    indicator.classList.add('voice-processing');
                    break;
                case VoiceState.ERROR:
                    indicator.classList.add('voice-error');
                    break;
                default:
                    indicator.classList.add('voice-idle');
            }

            // Update provider badge
            badge.textContent = provider || '';
        },

        /**
         * Get current state
         * @returns {string} Current VoiceState
         */
        getState() {
            return currentState;
        },

        /**
         * Show a brief error flash
         * @param {string} message - Error message (for accessibility)
         */
        flashError(message = 'Error') {
            this.setState(VoiceState.ERROR);
            button.setAttribute('aria-label', message);

            // Reset after animation
            setTimeout(() => {
                if (currentState === VoiceState.ERROR) {
                    this.setState(VoiceState.IDLE);
                    button.setAttribute('aria-label', 'Toggle voice input');
                }
            }, 2000);
        },

        /**
         * Destroy the component
         */
        destroy() {
            indicator.remove();
            styles.remove();
        }
    };
}
