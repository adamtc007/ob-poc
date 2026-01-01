/**
 * Voice Module - ES Module Entry Point
 *
 * Provides voice input with Deepgram cloud + WebSpeech fallback
 * Dispatches commands to egui via CustomEvents
 */

export { VoiceState } from './types.js';
export { WebSpeechProvider } from './WebSpeechProvider.js';
export { DeepgramProvider } from './DeepgramProvider.js';
export { VoiceRouter } from './VoiceRouter.js';
export { EsperCommandMatcher } from './EsperCommandMatcher.js';
export { createVoiceIndicator } from './VoiceIndicator.js';
export { createCommandEcho } from './CommandEcho.js';
export { createVoiceService, initVoiceService } from './VoiceService.js';

// Auto-initialize if script is loaded directly with data-auto-init
if (document.currentScript?.dataset.autoInit !== undefined) {
    import('./VoiceService.js').then(({ initVoiceService }) => {
        document.addEventListener('DOMContentLoaded', () => {
            initVoiceService();
        });
    });
}
