/**
 * Voice Provider Types
 *
 * Blade Runner Esper-style voice recognition for graph navigation
 */

/**
 * @typedef {Object} TranscriptResult
 * @property {string} text - The transcribed text
 * @property {boolean} isFinal - Whether this is a final result
 * @property {number} confidence - Confidence score 0-1
 * @property {string} provider - Which provider produced this ('deepgram' | 'webspeech')
 */

/**
 * @typedef {Object} VoiceProviderConfig
 * @property {string} [apiKey] - API key for cloud providers
 * @property {string} [language] - Language code (default: 'en-US')
 * @property {string[]} [keywords] - Domain keywords to boost
 */

/**
 * @typedef {'idle' | 'listening' | 'processing' | 'error'} VoiceState
 */

/**
 * @typedef {Object} MatchedCommand
 * @property {string} verb - The matched command verb (e.g., 'scale-universe')
 * @property {string} transcript - Original transcript text
 * @property {number} confidence - Match confidence 0-1
 * @property {Object} [params] - Extracted parameters
 */

export const VoiceState = {
    IDLE: 'idle',
    LISTENING: 'listening',
    PROCESSING: 'processing',
    ERROR: 'error'
};
