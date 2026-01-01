/**
 * VoiceService - Main orchestrator for voice input
 *
 * Coordinates:
 * - VoiceRouter (provider selection + fallback)
 * - EsperCommandMatcher (transcript → command)
 * - VoiceIndicator (mic button UI)
 * - CommandEcho (Blade Runner display)
 *
 * Dispatches commands to egui via CustomEvents following the unified
 * CommandSource pattern.
 */

import { VoiceState } from './types.js';
import { VoiceRouter } from './VoiceRouter.js';
import { EsperCommandMatcher } from './EsperCommandMatcher.js';
import { createVoiceIndicator } from './VoiceIndicator.js';
import { createCommandEcho } from './CommandEcho.js';

/**
 * Create and initialize the voice service
 * @param {Object} options - Configuration options
 * @param {string} options.deepgramApiKey - Deepgram API key (optional, falls back to WebSpeech)
 * @param {HTMLElement} options.container - Container for UI components (default: document.body)
 * @returns {Object} VoiceService controller
 */
export function createVoiceService(options = {}) {
    const {
        deepgramApiKey,
        container = document.body
    } = options;

    // Track if we've warned about missing API key
    let hasWarnedMissingKey = false;

    // Create UI components
    const indicator = createVoiceIndicator({
        container,
        onToggle: (action) => {
            if (action === 'start') {
                start();
            } else {
                stop();
            }
        }
    });

    const echo = createCommandEcho({
        container,
        displayDuration: 3000
    });

    // Create command matcher
    const matcher = new EsperCommandMatcher();

    // Create voice router with fallback
    const router = new VoiceRouter({
        deepgramApiKey,
        onTranscript: handleTranscript,
        onFinalTranscript: handleFinalTranscript,
        onStateChange: handleStateChange,
        onError: handleError
    });

    // Track current context for command matching
    let currentContext = {
        focusedEntityId: null,
        currentCbuId: null,
        viewMode: 'kyc_ubo'
    };

    /**
     * Handle interim transcripts (while speaking)
     */
    function handleTranscript(transcript, confidence, isFinal) {
        if (!isFinal) {
            echo.showTranscript(transcript, false);
        }
    }

    /**
     * Handle final transcripts (speech complete)
     */
    function handleFinalTranscript(transcript, confidence) {
        console.log(`[VoiceService] Final transcript: "${transcript}" (confidence: ${confidence.toFixed(2)})`);

        // Match command
        const result = matcher.match(transcript, currentContext);

        if (result.command) {
            console.log(`[VoiceService] Matched command: ${result.command}`, result.params);

            // Show in echo UI
            echo.showCommand(transcript, result.command);

            // Dispatch to egui via CustomEvent
            dispatchCommand(result);
        } else {
            console.log(`[VoiceService] No command matched`);
            echo.showNoMatch(transcript);

            // Still dispatch as raw transcript for chat
            dispatchRawTranscript(transcript, confidence);
        }
    }

    /**
     * Handle state changes from router
     */
    function handleStateChange(state, provider) {
        indicator.setState(state, provider);
    }

    /**
     * Handle errors from router
     */
    function handleError(error) {
        console.error('[VoiceService] Error:', error);
        indicator.flashError(error.message || 'Voice error');
    }

    /**
     * Dispatch a matched command to egui
     * Uses CustomEvent following the unified CommandSource pattern
     */
    function dispatchCommand(result) {
        const event = new CustomEvent('voice-command', {
            detail: {
                source: 'voice',
                command: result.command,
                params: result.params || {},
                transcript: result.transcript,
                confidence: result.confidence,
                provider: router.getActiveProvider()
            }
        });

        // Dispatch on document for egui to catch
        document.dispatchEvent(event);

        // Also dispatch on canvas if egui is listening there
        const canvas = document.getElementById('egui-canvas');
        if (canvas) {
            canvas.dispatchEvent(event);
        }
    }

    /**
     * Dispatch raw transcript (no command matched) for potential chat input
     */
    function dispatchRawTranscript(transcript, confidence) {
        const event = new CustomEvent('voice-transcript', {
            detail: {
                source: 'voice',
                transcript,
                confidence,
                provider: router.getActiveProvider()
            }
        });

        document.dispatchEvent(event);
    }

    /**
     * Start listening
     */
    async function start() {
        if (!deepgramApiKey && !hasWarnedMissingKey) {
            console.log('[VoiceService] No Deepgram API key provided, will use WebSpeech fallback');
            hasWarnedMissingKey = true;
        }

        try {
            await router.start();
        } catch (error) {
            console.error('[VoiceService] Failed to start:', error);
            indicator.flashError('Could not start voice input');
        }
    }

    /**
     * Stop listening
     */
    function stop() {
        router.stop();
        echo.hide();
    }

    /**
     * Update context for command matching
     */
    function updateContext(ctx) {
        currentContext = { ...currentContext, ...ctx };
    }

    // Listen for context updates from egui
    document.addEventListener('esper-context', (event) => {
        if (event.detail) {
            updateContext(event.detail);
        }
    });

    // Log initialization
    console.log('[VoiceService] Initialized', {
        hasDeepgramKey: !!deepgramApiKey,
        webSpeechSupported: router.isWebSpeechSupported(),
        deepgramSupported: router.isDeepgramSupported()
    });

    return {
        /**
         * Start voice recognition
         */
        start,

        /**
         * Stop voice recognition
         */
        stop,

        /**
         * Toggle voice recognition
         */
        toggle() {
            if (router.isListening()) {
                stop();
            } else {
                start();
            }
        },

        /**
         * Check if currently listening
         */
        isListening() {
            return router.isListening();
        },

        /**
         * Update context for command matching
         */
        updateContext,

        /**
         * Get current state
         */
        getState() {
            return indicator.getState();
        },

        /**
         * Get available providers
         */
        getProviders() {
            return {
                deepgram: router.isDeepgramSupported(),
                webSpeech: router.isWebSpeechSupported()
            };
        },

        /**
         * Destroy the service and clean up
         */
        destroy() {
            stop();
            indicator.destroy();
            echo.destroy();
        }
    };
}

/**
 * Initialize voice service with environment config
 * Called from HTML or module initialization
 */
export async function initVoiceService() {
    // Try to get API key from various sources
    let deepgramApiKey = null;

    // Check window config
    if (window.VOICE_CONFIG?.deepgramApiKey) {
        deepgramApiKey = window.VOICE_CONFIG.deepgramApiKey;
    }
    // Check meta tag
    else {
        const meta = document.querySelector('meta[name="deepgram-api-key"]');
        if (meta) {
            deepgramApiKey = meta.getAttribute('content');
        }
    }

    const service = createVoiceService({
        deepgramApiKey,
        container: document.body
    });

    // Expose globally for debugging
    window.voiceService = service;

    return service;
}
