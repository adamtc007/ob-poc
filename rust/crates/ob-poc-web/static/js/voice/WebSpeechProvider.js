/**
 * WebSpeechProvider - Browser-native Speech Recognition
 *
 * Fallback provider using the Web Speech API.
 * Works on Chrome, Safari, Edge without any API keys.
 */

import { VoiceState } from './types.js';

export class WebSpeechProvider {
    constructor(config = {}) {
        this.config = {
            language: config.language || 'en-US',
            continuous: true,
            interimResults: true,
            ...config
        };

        this.recognition = null;
        this.state = VoiceState.IDLE;
        this.onTranscript = null;
        this.onStateChange = null;
        this.onError = null;

        this._init();
    }

    _init() {
        const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;

        if (!SpeechRecognition) {
            console.warn('WebSpeechProvider: Speech Recognition not supported in this browser');
            return;
        }

        this.recognition = new SpeechRecognition();
        this.recognition.lang = this.config.language;
        this.recognition.continuous = this.config.continuous;
        this.recognition.interimResults = this.config.interimResults;

        this.recognition.onresult = (event) => {
            for (let i = event.resultIndex; i < event.results.length; i++) {
                const result = event.results[i];
                const transcript = result[0].transcript.trim();
                const confidence = result[0].confidence || 0.8;

                if (this.onTranscript) {
                    this.onTranscript({
                        text: transcript,
                        isFinal: result.isFinal,
                        confidence: confidence,
                        provider: 'webspeech'
                    });
                }
            }
        };

        this.recognition.onstart = () => {
            this._setState(VoiceState.LISTENING);
        };

        this.recognition.onend = () => {
            // Auto-restart if we're supposed to be listening
            if (this.state === VoiceState.LISTENING) {
                try {
                    this.recognition.start();
                } catch (e) {
                    this._setState(VoiceState.IDLE);
                }
            } else {
                this._setState(VoiceState.IDLE);
            }
        };

        this.recognition.onerror = (event) => {
            console.error('WebSpeechProvider error:', event.error);

            // Don't treat 'no-speech' as fatal
            if (event.error === 'no-speech') {
                return;
            }

            if (this.onError) {
                this.onError(event.error);
            }

            if (event.error === 'not-allowed') {
                this._setState(VoiceState.ERROR);
            }
        };
    }

    _setState(state) {
        this.state = state;
        if (this.onStateChange) {
            this.onStateChange(state);
        }
    }

    isSupported() {
        return !!(window.SpeechRecognition || window.webkitSpeechRecognition);
    }

    async start() {
        if (!this.recognition) {
            throw new Error('Speech Recognition not supported');
        }

        if (this.state === VoiceState.LISTENING) {
            return; // Already listening
        }

        try {
            this.recognition.start();
        } catch (e) {
            if (e.name === 'InvalidStateError') {
                // Already started, ignore
            } else {
                throw e;
            }
        }
    }

    stop() {
        if (this.recognition && this.state === VoiceState.LISTENING) {
            this._setState(VoiceState.IDLE);
            this.recognition.stop();
        }
    }

    getState() {
        return this.state;
    }

    getName() {
        return 'Web Speech API';
    }
}
