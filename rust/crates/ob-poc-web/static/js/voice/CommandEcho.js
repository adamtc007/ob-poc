/**
 * CommandEcho - Blade Runner style command display
 *
 * Shows transcripts and recognized commands with visual feedback:
 * - Transcript appears with typing effect
 * - Recognized command highlights in cyan
 * - Unrecognized text fades gray
 * - Auto-dismiss after delay
 *
 * Style inspired by Blade Runner's "ENHANCE" scene
 */

/**
 * Create the CommandEcho component
 * @param {Object} options - Configuration options
 * @param {HTMLElement} options.container - Parent element to mount into
 * @param {number} options.displayDuration - How long to show echo (ms, default 3000)
 * @returns {Object} CommandEcho controller
 */
export function createCommandEcho(options = {}) {
    const { container, displayDuration = 3000 } = options;

    if (!container) {
        console.error('CommandEcho: container is required');
        return null;
    }

    // Create the echo element
    const echo = document.createElement('div');
    echo.id = 'command-echo';
    echo.className = 'command-echo';
    echo.innerHTML = `
        <div class="echo-mic-icon">🎤</div>
        <div class="echo-content">
            <div class="echo-transcript"></div>
            <div class="echo-command"></div>
        </div>
    `;

    // Add styles
    const styles = document.createElement('style');
    styles.textContent = `
        .command-echo {
            position: fixed;
            bottom: 90px;
            right: 20px;
            z-index: 999;
            display: flex;
            align-items: flex-start;
            gap: 12px;
            padding: 12px 16px;
            background: rgba(10, 12, 15, 0.95);
            border: 1px solid rgba(0, 255, 136, 0.3);
            border-radius: 8px;
            box-shadow:
                0 0 20px rgba(0, 255, 136, 0.1),
                0 8px 32px rgba(0, 0, 0, 0.5);
            max-width: 400px;
            opacity: 0;
            transform: translateY(10px) scale(0.95);
            transition: all 0.3s ease;
            pointer-events: none;
            font-family: 'SF Mono', 'Consolas', 'Monaco', monospace;
        }

        .command-echo.visible {
            opacity: 1;
            transform: translateY(0) scale(1);
        }

        .command-echo.success {
            border-color: rgba(0, 255, 136, 0.6);
            box-shadow:
                0 0 30px rgba(0, 255, 136, 0.2),
                0 8px 32px rgba(0, 0, 0, 0.5);
        }

        .command-echo.no-match {
            border-color: rgba(100, 100, 100, 0.4);
        }

        .echo-mic-icon {
            font-size: 18px;
            opacity: 0.8;
            animation: mic-pulse 1s ease-in-out infinite;
        }

        @keyframes mic-pulse {
            0%, 100% { opacity: 0.6; }
            50% { opacity: 1; }
        }

        .command-echo.success .echo-mic-icon {
            animation: none;
            opacity: 1;
        }

        .echo-content {
            display: flex;
            flex-direction: column;
            gap: 4px;
            min-width: 0;
        }

        .echo-transcript {
            font-size: 14px;
            color: #888;
            line-height: 1.4;
            word-break: break-word;
        }

        .echo-transcript .word {
            display: inline;
            opacity: 0;
            animation: word-appear 0.1s ease forwards;
        }

        @keyframes word-appear {
            from { opacity: 0; }
            to { opacity: 1; }
        }

        .echo-command {
            font-size: 16px;
            font-weight: 600;
            color: #00ff88;
            text-transform: uppercase;
            letter-spacing: 1px;
            opacity: 0;
            transform: translateX(-10px);
            transition: all 0.3s ease 0.2s;
        }

        .command-echo.success .echo-command {
            opacity: 1;
            transform: translateX(0);
        }

        /* Scanline effect for extra Blade Runner feel */
        .command-echo::before {
            content: '';
            position: absolute;
            inset: 0;
            background: repeating-linear-gradient(
                0deg,
                transparent,
                transparent 2px,
                rgba(0, 0, 0, 0.1) 2px,
                rgba(0, 0, 0, 0.1) 4px
            );
            pointer-events: none;
            border-radius: 8px;
        }

        /* Glow effect on success */
        .command-echo.success::after {
            content: '';
            position: absolute;
            inset: -2px;
            background: linear-gradient(
                45deg,
                transparent,
                rgba(0, 255, 136, 0.1),
                transparent
            );
            border-radius: 10px;
            animation: glow-sweep 2s ease-out;
        }

        @keyframes glow-sweep {
            from {
                opacity: 1;
                transform: translateX(-100%);
            }
            to {
                opacity: 0;
                transform: translateX(100%);
            }
        }
    `;

    // Mount to container
    document.head.appendChild(styles);
    container.appendChild(echo);

    const transcriptEl = echo.querySelector('.echo-transcript');
    const commandEl = echo.querySelector('.echo-command');

    let hideTimeout = null;
    let wordTimeouts = [];

    /**
     * Clear all pending timeouts
     */
    function clearTimeouts() {
        if (hideTimeout) {
            clearTimeout(hideTimeout);
            hideTimeout = null;
        }
        wordTimeouts.forEach(t => clearTimeout(t));
        wordTimeouts = [];
    }

    /**
     * Show transcript with word-by-word typing effect
     * @param {string} text - The transcript text
     * @param {number} wordDelay - Delay between words (ms)
     */
    function showTranscriptAnimated(text, wordDelay = 50) {
        const words = text.split(/\s+/);
        transcriptEl.innerHTML = '';

        words.forEach((word, i) => {
            const span = document.createElement('span');
            span.className = 'word';
            span.textContent = word + ' ';
            span.style.animationDelay = `${i * wordDelay}ms`;
            transcriptEl.appendChild(span);
        });
    }

    return {
        /**
         * Show a transcript (while listening)
         * @param {string} transcript - The spoken text
         * @param {boolean} isFinal - Is this a final transcript?
         */
        showTranscript(transcript, isFinal = false) {
            clearTimeouts();

            showTranscriptAnimated(transcript);
            commandEl.textContent = '';
            echo.classList.remove('success', 'no-match');
            echo.classList.add('visible');

            if (isFinal) {
                // Will be followed by showCommand or showNoMatch
            }
        },

        /**
         * Show a recognized command (success state)
         * @param {string} transcript - The spoken text
         * @param {string} command - The recognized command name
         */
        showCommand(transcript, command) {
            clearTimeouts();

            showTranscriptAnimated(transcript);
            commandEl.textContent = `→ ${command}`;
            echo.classList.remove('no-match');
            echo.classList.add('visible', 'success');

            // Auto-hide after duration
            hideTimeout = setTimeout(() => {
                this.hide();
            }, displayDuration);
        },

        /**
         * Show unrecognized text (no command matched)
         * @param {string} transcript - The spoken text
         */
        showNoMatch(transcript) {
            clearTimeouts();

            showTranscriptAnimated(transcript);
            commandEl.textContent = '';
            echo.classList.remove('success');
            echo.classList.add('visible', 'no-match');

            // Auto-hide after shorter duration
            hideTimeout = setTimeout(() => {
                this.hide();
            }, displayDuration / 2);
        },

        /**
         * Hide the echo
         */
        hide() {
            clearTimeouts();
            echo.classList.remove('visible', 'success', 'no-match');
        },

        /**
         * Destroy the component
         */
        destroy() {
            clearTimeouts();
            echo.remove();
            styles.remove();
        }
    };
}
