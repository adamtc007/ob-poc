/**
 * VoiceRouter - Auto-selecting Voice Provider with Fallback
 *
 * Tries Deepgram first for best accuracy, automatically falls back
 * to Web Speech API if Deepgram fails (no API key, connection error, etc.)
 *
 * This ensures voice always works - just with varying accuracy.
 */

import { VoiceState } from './types.js';
import { WebSpeechProvider } from './WebSpeechProvider.js';
import { DeepgramProvider } from './DeepgramProvider.js';

export class VoiceRouter {
    constructor(config = {}) {
        this.config = config;
        this.activeProvider = null;
        this.deepgramProvider = null;
        this.webSpeechProvider = null;
        this.state = VoiceState.IDLE;

        // Callbacks
        this.onTranscript = null;
        this.onStateChange = null;
        this.onProviderChange = null;
        this.onError = null;

        this._init();
    }

    _init() {
        // Always create WebSpeech as fallback
        this.webSpeechProvider = new WebSpeechProvider({
            language: this.config.language || 'en-US'
        });

        // Try to create Deepgram if API key available
        const apiKey = this.config.apiKey || this._getApiKeyFromEnv();

        if (apiKey) {
            this.deepgramProvider = new DeepgramProvider({
                apiKey: apiKey,
                language: this.config.language || 'en-US',
                keywords: this.config.keywords || []
            });

            // Set up fallback handler
            this.deepgramProvider.onFallback = (reason) => {
                console.log('VoiceRouter: Deepgram failed, switching to WebSpeech');
                this._switchToWebSpeech();
            };
        }

        // Wire up callbacks for both providers
        this._wireProvider(this.webSpeechProvider);
        if (this.deepgramProvider) {
            this._wireProvider(this.deepgramProvider);
        }
    }

    _getApiKeyFromEnv() {
        // Try to get from window (set by HTML script)
        if (window.DEEPGRAM_API_KEY) {
            return window.DEEPGRAM_API_KEY;
        }
        // For Vite-style env
        if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_DEEPGRAM_API_KEY) {
            return import.meta.env.VITE_DEEPGRAM_API_KEY;
        }
        return null;
    }

    _wireProvider(provider) {
        provider.onTranscript = (result) => {
            if (provider === this.activeProvider && this.onTranscript) {
                this.onTranscript(result);
            }
        };

        provider.onStateChange = (state) => {
            if (provider === this.activeProvider) {
                this.state = state;
                if (this.onStateChange) {
                    this.onStateChange(state);
                }
            }
        };

        provider.onError = (error) => {
            if (provider === this.activeProvider && this.onError) {
                this.onError(error);
            }
        };
    }

    _switchToWebSpeech() {
        // Stop Deepgram if running
        if (this.deepgramProvider) {
            this.deepgramProvider.stop();
        }

        // Switch to WebSpeech
        this.activeProvider = this.webSpeechProvider;

        if (this.onProviderChange) {
            this.onProviderChange(this.webSpeechProvider.getName());
        }

        // Start WebSpeech if we were listening
        if (this.state === VoiceState.LISTENING || this.state === VoiceState.PROCESSING) {
            this.webSpeechProvider.start().catch(e => {
                console.error('VoiceRouter: WebSpeech fallback failed:', e);
                this.state = VoiceState.ERROR;
                if (this.onStateChange) {
                    this.onStateChange(VoiceState.ERROR);
                }
            });
        }
    }

    async start() {
        // Try Deepgram first if available
        if (this.deepgramProvider && this.deepgramProvider.isSupported()) {
            this.activeProvider = this.deepgramProvider;

            if (this.onProviderChange) {
                this.onProviderChange(this.deepgramProvider.getName());
            }

            try {
                await this.deepgramProvider.start();
                return;
            } catch (error) {
                console.warn('VoiceRouter: Deepgram start failed, trying WebSpeech');
                // Fall through to WebSpeech
            }
        }

        // Fall back to WebSpeech
        if (this.webSpeechProvider.isSupported()) {
            this.activeProvider = this.webSpeechProvider;

            if (this.onProviderChange) {
                this.onProviderChange(this.webSpeechProvider.getName());
            }

            await this.webSpeechProvider.start();
        } else {
            throw new Error('No voice recognition available in this browser');
        }
    }

    stop() {
        if (this.activeProvider) {
            this.activeProvider.stop();
        }
        this.state = VoiceState.IDLE;
    }

    toggle() {
        if (this.state === VoiceState.LISTENING) {
            this.stop();
        } else {
            this.start();
        }
    }

    getState() {
        return this.state;
    }

    getActiveProviderName() {
        return this.activeProvider?.getName() || 'None';
    }

    isListening() {
        return this.state === VoiceState.LISTENING;
    }
}
