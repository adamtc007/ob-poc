# TODO: Deepgram Voice Integration with Web Speech API Fallback

## Overview

Implement browser-based speech-to-text for Esper-style navigation commands using Deepgram Nova-2 as primary provider with vanilla Web Speech API as fallback. Must work seamlessly for demos while maintaining zero-config fallback for environments without Deepgram credentials.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Browser Voice Input                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌──────────────┐    ┌───────────────┐  │
│  │ Microphone  │───▶│ Voice Router │───▶│ Command Parse │  │
│  └─────────────┘    └──────┬───────┘    └───────┬───────┘  │
│                            │                     │          │
│              ┌─────────────┴─────────────┐      │          │
│              ▼                           ▼      ▼          │
│  ┌───────────────────┐    ┌──────────────────────────────┐ │
│  │ Deepgram WebSocket│    │ Web Speech API (fallback)    │ │
│  │ (if API key set)  │    │ (always available)           │ │
│  └───────────────────┘    └──────────────────────────────┘ │
│              │                           │                  │
│              └─────────────┬─────────────┘                  │
│                            ▼                                │
│                 ┌──────────────────┐                       │
│                 │ Esper Command    │                       │
│                 │ Matcher (RAG)    │                       │
│                 └────────┬─────────┘                       │
│                          ▼                                  │
│                 ┌──────────────────┐                       │
│                 │ UI Action        │                       │
│                 │ Dispatcher       │                       │
│                 └──────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Task 1: Voice Service Abstraction Layer

Create a provider-agnostic voice service interface.

**File:** `rust/src/ui/voice/mod.rs` (or JS equivalent if browser-only)

```rust
// If doing Rust/WASM
pub trait VoiceProvider {
    fn start_listening(&mut self) -> Result<(), VoiceError>;
    fn stop_listening(&mut self);
    fn is_available(&self) -> bool;
    fn provider_name(&self) -> &str;
    fn on_transcript(&mut self, callback: Box<dyn Fn(Transcript)>);
    fn on_error(&mut self, callback: Box<dyn Fn(VoiceError)>);
}

pub struct Transcript {
    pub text: String,
    pub confidence: f32,
    pub is_final: bool,
    pub provider: String,
}

pub enum VoiceError {
    NotSupported,
    PermissionDenied,
    NetworkError(String),
    NoSpeechDetected,
    Aborted,
}
```

**JavaScript equivalent (if pure browser):**

```typescript
// src/voice/VoiceProvider.ts
interface VoiceProvider {
  start(): Promise<void>;
  stop(): void;
  isAvailable(): boolean;
  providerName(): string;
  onTranscript(callback: (transcript: Transcript) => void): void;
  onError(callback: (error: VoiceError) => void): void;
}

interface Transcript {
  text: string;
  confidence: number;
  isFinal: boolean;
  provider: string;
}
```

---

## Task 2: Web Speech API Provider (Fallback)

Implement vanilla browser speech recognition as always-available fallback.

**File:** `src/voice/WebSpeechProvider.ts`

```typescript
export class WebSpeechProvider implements VoiceProvider {
  private recognition: SpeechRecognition | null = null;
  private transcriptCallback: ((t: Transcript) => void) | null = null;
  private errorCallback: ((e: VoiceError) => void) | null = null;

  constructor() {
    const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (SpeechRecognition) {
      this.recognition = new SpeechRecognition();
      this.recognition.continuous = true;
      this.recognition.interimResults = true;
      this.recognition.lang = 'en-GB'; // UK English default
      this.setupHandlers();
    }
  }

  isAvailable(): boolean {
    return this.recognition !== null;
  }

  providerName(): string {
    return 'Web Speech API';
  }

  async start(): Promise<void> {
    if (!this.recognition) {
      throw new Error('Speech recognition not supported');
    }
    this.recognition.start();
  }

  stop(): void {
    this.recognition?.stop();
  }

  onTranscript(callback: (t: Transcript) => void): void {
    this.transcriptCallback = callback;
  }

  onError(callback: (e: VoiceError) => void): void {
    this.errorCallback = callback;
  }

  private setupHandlers(): void {
    if (!this.recognition) return;

    this.recognition.onresult = (event) => {
      const result = event.results[event.results.length - 1];
      const transcript: Transcript = {
        text: result[0].transcript.trim().toLowerCase(),
        confidence: result[0].confidence,
        isFinal: result.isFinal,
        provider: 'webspeech',
      };
      this.transcriptCallback?.(transcript);
    };

    this.recognition.onerror = (event) => {
      const error = this.mapError(event.error);
      this.errorCallback?.(error);
    };

    this.recognition.onend = () => {
      // Auto-restart for continuous listening
      if (this.recognition) {
        this.recognition.start();
      }
    };
  }

  private mapError(error: string): VoiceError {
    switch (error) {
      case 'not-allowed': return { type: 'PermissionDenied' };
      case 'no-speech': return { type: 'NoSpeechDetected' };
      case 'network': return { type: 'NetworkError', message: 'Network error' };
      default: return { type: 'Aborted' };
    }
  }
}
```

---

## Task 3: Deepgram WebSocket Provider

Implement Deepgram streaming for enhanced accuracy.

**File:** `src/voice/DeepgramProvider.ts`

```typescript
export class DeepgramProvider implements VoiceProvider {
  private socket: WebSocket | null = null;
  private mediaRecorder: MediaRecorder | null = null;
  private apiKey: string | null = null;
  private transcriptCallback: ((t: Transcript) => void) | null = null;
  private errorCallback: ((e: VoiceError) => void) | null = null;

  // Esper command keywords with boost
  private static KEYWORDS = [
    // Core Esper commands
    'enhance', 'zoom in', 'zoom out', 'pull back', 'move in',
    'track left', 'track right', 'track up', 'track down',
    'pan left', 'pan right', 'pan up', 'pan down',
    'center', 'stop', 'freeze', 'hold',
    'give me a hard copy', 'hard copy', 'export',
    
    // 3D/Scale navigation
    'drill through', 'drill down', 'drill up', 'drill into',
    'surface', 'peel back', 'peel', 'x-ray', 'x ray',
    'core sample', 'cross section',
    'orbit', 'orbit around', 'rotate', 'flip', 'tilt',
    
    // Scale commands
    'universe', 'galaxy', 'system', 'planet', 'surface view',
    'scale up', 'scale down',
    
    // Temporal
    'rewind', 'play', 'time slice', 'freeze frame',
    'show history', 'time trail',
    
    // Investigation
    'follow the money', 'who controls', 'illuminate',
    'shadow', 'red flag', 'black hole',
    
    // Financial domain terms
    'UBO', 'KYC', 'CBU', 'SSI', 'AML',
    'custody', 'settlement', 'booking rule',
    'subfund', 'sub fund', 'share class',
    'umbrella', 'feeder', 'master',
    'counterparty', 'beneficiary',
    'PSET', 'BUYR', 'SELL',
    
    // View switching
    'show kyc', 'show trading', 'show custody', 'show services',
    'kyc view', 'trading view', 'custody view',
    'load cbu', 'open cbu',
  ];

  constructor(apiKey?: string) {
    this.apiKey = apiKey || this.getApiKeyFromEnv();
  }

  private getApiKeyFromEnv(): string | null {
    // Check for API key in various places
    return (
      (window as any).DEEPGRAM_API_KEY ||
      localStorage.getItem('deepgram_api_key') ||
      null
    );
  }

  isAvailable(): boolean {
    return this.apiKey !== null && this.apiKey.length > 0;
  }

  providerName(): string {
    return 'Deepgram Nova-2';
  }

  async start(): Promise<void> {
    if (!this.apiKey) {
      throw new Error('Deepgram API key not configured');
    }

    // Get microphone access
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

    // Build WebSocket URL with keywords
    const keywordParams = DeepgramProvider.KEYWORDS
      .map(kw => `keywords=${encodeURIComponent(kw)}:2`)
      .join('&');

    const wsUrl = `wss://api.deepgram.com/v1/listen?` +
      `model=nova-2&` +
      `language=en-GB&` +
      `punctuate=true&` +
      `interim_results=true&` +
      `endpointing=300&` +
      `${keywordParams}`;

    this.socket = new WebSocket(wsUrl, ['token', this.apiKey]);

    this.socket.onopen = () => {
      console.log('[Deepgram] Connected');
      this.startStreaming(stream);
    };

    this.socket.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.channel?.alternatives?.[0]) {
        const alt = data.channel.alternatives[0];
        const transcript: Transcript = {
          text: alt.transcript.trim().toLowerCase(),
          confidence: alt.confidence,
          isFinal: data.is_final,
          provider: 'deepgram',
        };
        if (transcript.text) {
          this.transcriptCallback?.(transcript);
        }
      }
    };

    this.socket.onerror = (error) => {
      console.error('[Deepgram] WebSocket error:', error);
      this.errorCallback?.({ type: 'NetworkError', message: 'WebSocket error' });
    };

    this.socket.onclose = () => {
      console.log('[Deepgram] Disconnected');
    };
  }

  private startStreaming(stream: MediaStream): void {
    this.mediaRecorder = new MediaRecorder(stream, {
      mimeType: 'audio/webm;codecs=opus',
    });

    this.mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0 && this.socket?.readyState === WebSocket.OPEN) {
        this.socket.send(event.data);
      }
    };

    // Send audio chunks every 250ms
    this.mediaRecorder.start(250);
  }

  stop(): void {
    this.mediaRecorder?.stop();
    this.socket?.close();
    this.socket = null;
    this.mediaRecorder = null;
  }

  onTranscript(callback: (t: Transcript) => void): void {
    this.transcriptCallback = callback;
  }

  onError(callback: (e: VoiceError) => void): void {
    this.errorCallback = callback;
  }
}
```

---

## Task 4: Voice Router (Provider Selection)

Automatic provider selection with fallback chain.

**File:** `src/voice/VoiceRouter.ts`

```typescript
export class VoiceRouter {
  private providers: VoiceProvider[] = [];
  private activeProvider: VoiceProvider | null = null;
  private transcriptCallback: ((t: Transcript) => void) | null = null;
  private statusCallback: ((status: VoiceStatus) => void) | null = null;

  constructor() {
    // Priority order: Deepgram first, Web Speech fallback
    const deepgram = new DeepgramProvider();
    const webSpeech = new WebSpeechProvider();

    if (deepgram.isAvailable()) {
      this.providers.push(deepgram);
      console.log('[Voice] Deepgram available');
    }
    if (webSpeech.isAvailable()) {
      this.providers.push(webSpeech);
      console.log('[Voice] Web Speech API available');
    }
  }

  getAvailableProviders(): string[] {
    return this.providers.map(p => p.providerName());
  }

  getActiveProvider(): string | null {
    return this.activeProvider?.providerName() || null;
  }

  async start(preferredProvider?: string): Promise<void> {
    // Find provider
    let provider: VoiceProvider | undefined;
    
    if (preferredProvider) {
      provider = this.providers.find(p => 
        p.providerName().toLowerCase().includes(preferredProvider.toLowerCase())
      );
    }
    
    // Fall back to first available
    if (!provider) {
      provider = this.providers[0];
    }

    if (!provider) {
      throw new Error('No speech recognition provider available');
    }

    this.activeProvider = provider;

    // Wire up callbacks
    provider.onTranscript((transcript) => {
      this.transcriptCallback?.(transcript);
    });

    provider.onError((error) => {
      console.warn(`[Voice] ${provider!.providerName()} error:`, error);
      // Try fallback on error
      this.tryFallback();
    });

    this.statusCallback?.({
      listening: true,
      provider: provider.providerName(),
    });

    await provider.start();
  }

  private async tryFallback(): Promise<void> {
    const currentIndex = this.providers.indexOf(this.activeProvider!);
    const nextProvider = this.providers[currentIndex + 1];
    
    if (nextProvider) {
      console.log(`[Voice] Falling back to ${nextProvider.providerName()}`);
      this.activeProvider = nextProvider;
      await nextProvider.start();
      this.statusCallback?.({
        listening: true,
        provider: nextProvider.providerName(),
      });
    }
  }

  stop(): void {
    this.activeProvider?.stop();
    this.activeProvider = null;
    this.statusCallback?.({ listening: false, provider: null });
  }

  onTranscript(callback: (t: Transcript) => void): void {
    this.transcriptCallback = callback;
  }

  onStatus(callback: (status: VoiceStatus) => void): void {
    this.statusCallback = callback;
  }
}

interface VoiceStatus {
  listening: boolean;
  provider: string | null;
}
```

---

## Task 5: Esper Command Matcher Integration

Connect voice transcripts to existing verb RAG patterns.

**File:** `src/voice/EsperCommandMatcher.ts`

```typescript
import { VoiceRouter, Transcript } from './VoiceRouter';
// Import your existing verb matcher
import { findVerbsByIntent } from '../session/verb_rag'; // Adjust path

export class EsperVoiceController {
  private router: VoiceRouter;
  private commandCallback: ((cmd: EsperCommand) => void) | null = null;
  private echoCallback: ((text: string, matched: boolean) => void) | null = null;
  private lastFinalTranscript: string = '';
  private debounceTimer: number | null = null;

  constructor() {
    this.router = new VoiceRouter();
    this.router.onTranscript((t) => this.handleTranscript(t));
  }

  async startListening(): Promise<void> {
    await this.router.start();
  }

  stopListening(): void {
    this.router.stop();
  }

  onCommand(callback: (cmd: EsperCommand) => void): void {
    this.commandCallback = callback;
  }

  // For UI echo display
  onEcho(callback: (text: string, matched: boolean) => void): void {
    this.echoCallback = callback;
  }

  private handleTranscript(transcript: Transcript): void {
    // Show interim results as echo
    this.echoCallback?.(transcript.text, false);

    // Only process final transcripts
    if (!transcript.isFinal) return;
    
    // Dedupe
    if (transcript.text === this.lastFinalTranscript) return;
    this.lastFinalTranscript = transcript.text;

    // Debounce rapid finals
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }

    this.debounceTimer = window.setTimeout(() => {
      this.matchCommand(transcript.text);
    }, 100);
  }

  private matchCommand(text: string): void {
    // Use existing verb RAG matcher
    const matches = findVerbsByIntent(text);
    
    if (matches.length > 0) {
      const bestMatch = matches[0]; // Highest confidence
      
      this.echoCallback?.(text, true);
      
      this.commandCallback?.({
        verb: bestMatch.verb,
        confidence: bestMatch.score,
        rawText: text,
        parameters: this.extractParameters(text, bestMatch.verb),
      });
    } else {
      // No match - show as unrecognized
      this.echoCallback?.(text, false);
      console.log(`[Esper] No command match for: "${text}"`);
    }
  }

  private extractParameters(text: string, verb: string): Record<string, string> {
    const params: Record<string, string> = {};
    
    // Extract entity names after focus/load commands
    if (verb.includes('focus') || verb.includes('load')) {
      const match = text.match(/(?:on|to)\s+(.+)$/i);
      if (match) {
        params.target = match[1];
      }
    }
    
    // Extract temporal references
    if (verb.includes('rewind') || verb.includes('time')) {
      const dateMatch = text.match(/(?:to|at|on)\s+(.+)$/i);
      if (dateMatch) {
        params.date = dateMatch[1];
      }
    }

    return params;
  }
}

interface EsperCommand {
  verb: string;
  confidence: number;
  rawText: string;
  parameters: Record<string, string>;
}
```

---

## Task 6: UI Components

### 6.1 Voice Status Indicator

```typescript
// src/ui/VoiceIndicator.tsx (React) or equivalent

interface VoiceIndicatorProps {
  listening: boolean;
  provider: string | null;
}

export function VoiceIndicator({ listening, provider }: VoiceIndicatorProps) {
  return (
    <div className={`voice-indicator ${listening ? 'active' : ''}`}>
      <div className="pulse-ring" />
      <MicrophoneIcon />
      {provider && <span className="provider-badge">{provider}</span>}
    </div>
  );
}

// CSS
.voice-indicator {
  position: fixed;
  bottom: 20px;
  right: 20px;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: #1a1a2e;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.voice-indicator.active {
  background: #e63946;
}

.voice-indicator.active .pulse-ring {
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% { transform: scale(1); opacity: 1; }
  100% { transform: scale(1.5); opacity: 0; }
}

.provider-badge {
  position: absolute;
  bottom: -8px;
  font-size: 8px;
  background: #333;
  padding: 2px 4px;
  border-radius: 2px;
}
```

### 6.2 Command Echo Display (Blade Runner Style)

```typescript
// src/ui/CommandEcho.tsx

interface CommandEchoProps {
  text: string;
  matched: boolean;
  visible: boolean;
}

export function CommandEcho({ text, matched, visible }: CommandEchoProps) {
  if (!visible || !text) return null;

  return (
    <div className={`command-echo ${matched ? 'matched' : 'pending'}`}>
      <span className="echo-prefix">🎤</span>
      <span className="echo-text">"{text}"</span>
      {matched && <span className="echo-check">✓</span>}
    </div>
  );
}

// CSS - Blade Runner aesthetic
.command-echo {
  position: fixed;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(0, 0, 0, 0.85);
  border: 1px solid #00ff88;
  padding: 12px 24px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 14px;
  color: #00ff88;
  text-transform: uppercase;
  letter-spacing: 2px;
  animation: fadeIn 0.2s ease;
}

.command-echo.matched {
  border-color: #00ffff;
  color: #00ffff;
  animation: flash 0.3s ease;
}

.command-echo.pending {
  opacity: 0.7;
}

@keyframes flash {
  0%, 100% { background: rgba(0, 0, 0, 0.85); }
  50% { background: rgba(0, 255, 255, 0.2); }
}
```

---

## Task 7: Configuration & API Key Management

### 7.1 Environment Configuration

```typescript
// src/config/voice.ts

export const VoiceConfig = {
  // Deepgram settings
  deepgram: {
    apiKey: import.meta.env.VITE_DEEPGRAM_API_KEY || null,
    model: 'nova-2',
    language: 'en-GB',
    punctuate: true,
    interimResults: true,
    endpointing: 300, // ms silence to end utterance
  },
  
  // Fallback settings  
  webSpeech: {
    language: 'en-GB',
    continuous: true,
    interimResults: true,
  },
  
  // UI settings
  ui: {
    echoDisplayMs: 2000, // How long to show command echo
    showProviderBadge: true,
  },
};
```

### 7.2 Runtime API Key Setting (for demos)

```typescript
// Allow setting API key at runtime for demos
export function setDeepgramApiKey(key: string): void {
  localStorage.setItem('deepgram_api_key', key);
  (window as any).DEEPGRAM_API_KEY = key;
}

// Quick setup function for demo
(window as any).setupVoice = (apiKey: string) => {
  setDeepgramApiKey(apiKey);
  console.log('Deepgram API key configured. Reload to activate.');
};
```

### 7.3 .env.local Template

```bash
# .env.local (do not commit)
VITE_DEEPGRAM_API_KEY=your_api_key_here
```

---

## Task 8: Integration with Existing UI

Wire voice controller to your existing Esper UI action dispatcher.

```typescript
// src/main.ts or equivalent entry point

import { EsperVoiceController } from './voice/EsperCommandMatcher';
import { uiActionDispatcher } from './ui/actions'; // Your existing dispatcher

const voiceController = new EsperVoiceController();

// Connect to UI
voiceController.onCommand((cmd) => {
  console.log(`[Esper] Command: ${cmd.verb}`, cmd);
  
  // Dispatch to your existing UI action system
  uiActionDispatcher.dispatch({
    type: cmd.verb,
    payload: cmd.parameters,
  });
});

voiceController.onEcho((text, matched) => {
  // Update echo UI component
  setEchoState({ text, matched, visible: true });
  
  // Auto-hide after delay
  setTimeout(() => {
    setEchoState({ visible: false });
  }, 2000);
});

// Start listening
document.getElementById('voice-toggle')?.addEventListener('click', async () => {
  if (voiceController.isListening()) {
    voiceController.stopListening();
  } else {
    await voiceController.startListening();
  }
});
```

---

## Task 9: Testing

### 9.1 Unit Tests

```typescript
// tests/voice/EsperCommandMatcher.test.ts

describe('EsperCommandMatcher', () => {
  it('matches enhance command', () => {
    const matches = findVerbsByIntent('enhance');
    expect(matches[0].verb).toBe('ui.zoom-in');
  });

  it('matches drill through command', () => {
    const matches = findVerbsByIntent('drill through');
    expect(matches[0].verb).toBe('ui.drill-through');
  });

  it('matches follow the money', () => {
    const matches = findVerbsByIntent('follow the money');
    expect(matches[0].verb).toBe('ui.follow-the-money');
  });

  it('matches give me a hard copy', () => {
    const matches = findVerbsByIntent('give me a hard copy');
    expect(matches[0].verb).toBe('ui.export');
  });

  it('handles UK accent variations', () => {
    // Common misrecognitions
    const variations = ['enhawnce', 'en hance', 'and hance'];
    // Test fuzzy matching handles these
  });
});
```

### 9.2 Manual Test Script

```markdown
## Voice Command Test Checklist

### Setup
- [ ] Deepgram API key configured
- [ ] Microphone permissions granted
- [ ] Voice indicator shows "Deepgram Nova-2"

### Basic Esper Commands
- [ ] "Enhance" → zoom in
- [ ] "Pull back" → zoom out  
- [ ] "Track right" → pan right
- [ ] "Track left" → pan left
- [ ] "Stop" → halt current action
- [ ] "Give me a hard copy" → export

### 3D Navigation
- [ ] "Drill through" → penetrate to terminus
- [ ] "Surface" → return to top
- [ ] "X-ray" → transparent view
- [ ] "Orbit around" → rotate view
- [ ] "Peel back" → remove one layer

### Scale Navigation
- [ ] "Show universe" → full book view
- [ ] "Enter system" → CBU focus
- [ ] "Land on [entity]" → entity focus

### Investigation
- [ ] "Follow the money" → ownership trace
- [ ] "Who controls" → control trace
- [ ] "Show black holes" → data gaps

### Temporal
- [ ] "Rewind to [date]" → as-of view
- [ ] "Time slice" → comparison view

### Domain Terms
- [ ] "Show KYC" → KYC view
- [ ] "Load CBU" → CBU selector
- [ ] "Show UBO structure" → ownership view

### Fallback Test
- [ ] Disconnect internet → falls back to Web Speech API
- [ ] Provider badge updates to "Web Speech API"
- [ ] Basic commands still work
```

---

## Task 10: Demo Setup Script

Quick setup for demos:

```bash
#!/bin/bash
# scripts/demo-voice-setup.sh

echo "=== Esper Voice Demo Setup ==="
echo ""
echo "1. Get your Deepgram API key from: https://console.deepgram.com"
echo "2. Run: export VITE_DEEPGRAM_API_KEY=your_key_here"
echo "3. Or in browser console: setupVoice('your_key_here')"
echo ""
echo "Testing microphone..."
# Check mic access
if command -v ffmpeg &> /dev/null; then
  ffmpeg -f avfoundation -list_devices true -i "" 2>&1 | grep "Microphone"
fi
echo ""
echo "Ready for demo!"
```

---

## Implementation Order

1. **Task 1-2**: Voice abstraction + Web Speech (get basic voice working)
2. **Task 3**: Deepgram provider (enhanced accuracy)
3. **Task 4**: Voice router (automatic fallback)
4. **Task 5**: Connect to existing verb RAG matcher
5. **Task 6**: UI components (indicator + echo)
6. **Task 7**: Configuration management
7. **Task 8**: Integration with main UI
8. **Task 9-10**: Testing and demo prep

---

## Acceptance Criteria

- [ ] Voice commands work with vanilla Web Speech API (no config needed)
- [ ] Deepgram activates automatically when API key present
- [ ] Automatic fallback if Deepgram fails
- [ ] All existing Esper commands recognized
- [ ] Command echo displays with Blade Runner aesthetic
- [ ] Provider indicator shows which engine is active
- [ ] < 500ms latency from speech to UI action
- [ ] Works in Chrome, Edge, Safari
- [ ] Demo can be set up in < 2 minutes

---

## Notes

- Web Speech API sends audio to Google servers (Chrome) - fine for demos, flag for production
- Deepgram $200 free credit = ~430 hours of demo time
- Keywords boost list should sync with verb_rag_metadata.rs patterns
- Consider adding "Computer," wake word for hands-free activation
- Potential future: local Whisper.cpp for fully offline operation
