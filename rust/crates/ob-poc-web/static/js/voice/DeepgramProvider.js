/**
 * DeepgramProvider - Cloud Speech Recognition via WebSocket
 *
 * High-accuracy provider using Deepgram's Nova-2 model.
 * Supports keyword boosting for domain-specific terms.
 *
 * IMPORTANT: Falls back to WebSpeechProvider if connection fails.
 */

import { VoiceState } from "./types.js";

// Esper navigation keywords to boost recognition
// Thematic: Blade Runner (forensic), Matrix (hidden truth), Sci-Fi (system nav)
const ESPER_KEYWORDS = [
  // Scale navigation (Astronomical)
  "universe",
  "galaxy",
  "system",
  "planet",
  "surface",
  "core",
  "zoom",
  "enhance",
  "pull back",
  "god view",
  "full book",

  // Depth navigation (3D)
  "drill",
  "through",
  "x-ray",
  "peel",
  "cross section",
  "deeper",
  "drill through",
  "drill down",
  "drill into",

  // Orbital
  "orbit",
  "rotate",
  "flip",
  "tilt",
  "spin",

  // Temporal (4D)
  "rewind",
  "play",
  "freeze",
  "time",
  "history",
  "trail",
  "time slice",
  "as of",
  "before",
  "after",

  // Matrix-themed investigation (finding hidden truth)
  "follow the rabbit",
  "follow the white rabbit",
  "white rabbit",
  "rabbit hole",
  "down the rabbit hole",
  "how deep does this go",
  "dive into",
  "dive in",
  "deep dive",
  "go deep",
  "find the humans",
  "trace to terminus",
  "follow the money", // Legacy support

  // Blade Runner-themed (forensic examination)
  "enhance",
  "track left",
  "track right",
  "give me a hard copy",
  "hard copy",
  "stop",
  "freeze frame",

  // Investigation patterns
  "who controls",
  "illuminate",
  "shadow",
  "red flag",
  "black hole",
  "missing",
  "data gaps",

  // Context
  "review",
  "investigation",
  "onboarding",
  "monitoring",
  "remediation",

  // Entities
  "CBU",
  "UBO",
  "entity",
  "director",
  "shareholder",
  "fund",
  "beneficial owner",
  "ownership chain",
  "control chain",

  // Actions
  "show",
  "hide",
  "focus",
  "highlight",
  "filter",
  "clear",
];

export class DeepgramProvider {
  constructor(config = {}) {
    this.config = {
      apiKey: config.apiKey,
      language: config.language || "en-US",
      model: config.model || "nova-2",
      keywords: [...ESPER_KEYWORDS, ...(config.keywords || [])],
      ...config,
    };

    this.socket = null;
    this.mediaRecorder = null;
    this.audioContext = null;
    this.state = VoiceState.IDLE;
    this.onTranscript = null;
    this.onStateChange = null;
    this.onError = null;
    this.onFallback = null; // Called when we need to fall back

    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 3;
  }

  _setState(state) {
    this.state = state;
    if (this.onStateChange) {
      this.onStateChange(state);
    }
  }

  _buildWebSocketUrl() {
    const params = new URLSearchParams({
      model: this.config.model,
      language: this.config.language,
      punctuate: "true",
      interim_results: "true",
      endpointing: "300",
      vad_events: "true",
    });

    // Add keyword boosting
    if (this.config.keywords.length > 0) {
      // Deepgram supports keywords parameter
      params.append("keywords", this.config.keywords.join(":5,") + ":5");
    }

    return `wss://api.deepgram.com/v1/listen?${params.toString()}`;
  }

  isSupported() {
    return !!(
      this.config.apiKey &&
      window.WebSocket &&
      navigator.mediaDevices &&
      navigator.mediaDevices.getUserMedia
    );
  }

  async start() {
    if (!this.config.apiKey) {
      console.warn("DeepgramProvider: No API key, triggering fallback");
      this._triggerFallback("No API key configured");
      return;
    }

    if (this.state === VoiceState.LISTENING) {
      return;
    }

    this._setState(VoiceState.PROCESSING);

    try {
      // Get microphone access
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          sampleRate: 16000,
        },
      });

      // Connect to Deepgram
      await this._connectWebSocket(stream);
    } catch (error) {
      console.error("DeepgramProvider start error:", error);
      this._triggerFallback(error.message);
    }
  }

  async _connectWebSocket(stream) {
    return new Promise((resolve, reject) => {
      const url = this._buildWebSocketUrl();

      this.socket = new WebSocket(url, ["token", this.config.apiKey]);

      const connectionTimeout = setTimeout(() => {
        if (this.socket.readyState !== WebSocket.OPEN) {
          this.socket.close();
          reject(new Error("Connection timeout"));
        }
      }, 5000);

      this.socket.onopen = () => {
        clearTimeout(connectionTimeout);
        console.log("DeepgramProvider: Connected");
        this.reconnectAttempts = 0;
        this._setState(VoiceState.LISTENING);
        this._startStreaming(stream);
        resolve();
      };

      this.socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);

          if (data.type === "Results" && data.channel) {
            const alt = data.channel.alternatives[0];
            if (alt && alt.transcript) {
              if (this.onTranscript) {
                this.onTranscript({
                  text: alt.transcript,
                  isFinal: data.is_final,
                  confidence: alt.confidence || 0.9,
                  provider: "deepgram",
                });
              }
            }
          }
        } catch (e) {
          console.warn("DeepgramProvider: Parse error", e);
        }
      };

      this.socket.onerror = (error) => {
        clearTimeout(connectionTimeout);
        console.error("DeepgramProvider WebSocket error:", error);

        if (this.state !== VoiceState.LISTENING) {
          // Connection failed during setup
          reject(new Error("WebSocket connection failed"));
        }
      };

      this.socket.onclose = (event) => {
        clearTimeout(connectionTimeout);
        console.log("DeepgramProvider: Disconnected", event.code, event.reason);

        this._stopStreaming();

        // If we were listening and got disconnected unexpectedly
        if (this.state === VoiceState.LISTENING) {
          if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            console.log(
              `DeepgramProvider: Reconnecting (${this.reconnectAttempts}/${this.maxReconnectAttempts})`,
            );
            setTimeout(() => this._connectWebSocket(stream), 1000);
          } else {
            this._triggerFallback("Connection lost after max retries");
          }
        } else {
          this._setState(VoiceState.IDLE);
        }
      };
    });
  }

  _startStreaming(stream) {
    // Use MediaRecorder to capture audio
    const mimeType = MediaRecorder.isTypeSupported("audio/webm;codecs=opus")
      ? "audio/webm;codecs=opus"
      : "audio/webm";

    this.mediaRecorder = new MediaRecorder(stream, { mimeType });

    this.mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0 && this.socket?.readyState === WebSocket.OPEN) {
        this.socket.send(event.data);
      }
    };

    // Send audio chunks every 250ms
    this.mediaRecorder.start(250);
  }

  _stopStreaming() {
    if (this.mediaRecorder && this.mediaRecorder.state !== "inactive") {
      this.mediaRecorder.stop();
      this.mediaRecorder.stream.getTracks().forEach((track) => track.stop());
      this.mediaRecorder = null;
    }
  }

  _triggerFallback(reason) {
    console.warn("DeepgramProvider: Falling back to WebSpeech -", reason);
    this._setState(VoiceState.ERROR);

    if (this.onFallback) {
      this.onFallback(reason);
    }

    if (this.onError) {
      this.onError(reason);
    }
  }

  stop() {
    this._stopStreaming();

    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }

    this._setState(VoiceState.IDLE);
  }

  getState() {
    return this.state;
  }

  getName() {
    return "Deepgram Nova-2";
  }
}
