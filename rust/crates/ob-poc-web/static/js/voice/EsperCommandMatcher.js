/**
 * EsperCommandMatcher - Blade Runner Esper Voice Command Recognition
 *
 * Matches voice transcripts to Esper navigation commands.
 * Uses fuzzy matching and synonym expansion.
 *
 * The patterns here mirror those in rust/src/api/agent_service.rs
 */

/**
 * Command patterns organized by category
 * Each pattern has: triggers (words/phrases), command, and optional param extractor
 */
const COMMAND_PATTERNS = {
    // =========================================================================
    // Scale Navigation (Astronomical metaphor)
    // =========================================================================
    scaleUniverse: {
        triggers: ['universe', 'full book', 'god view', 'show all', 'everything', 'all clients', 'whole book'],
        command: 'ScaleUniverse',
        params: null
    },
    scaleGalaxy: {
        triggers: ['galaxy', 'segment', 'sector', 'group'],
        command: 'ScaleGalaxy',
        extractParams: (text) => {
            // Try to extract segment name: "hedge fund galaxy", "show me the fund segment"
            const match = text.match(/(?:show|go to|view)?\s*(?:the\s+)?(\w+)\s+(?:galaxy|segment|sector)/i);
            return match ? { segment: match[1] } : {};
        }
    },
    scaleSystem: {
        triggers: ['system', 'solar system', 'cbu', 'client'],
        command: 'ScaleSystem',
        extractParams: (text) => {
            // Extract CBU name if mentioned
            const match = text.match(/(?:show|go to|focus on)?\s*(?:the\s+)?(.+?)\s+(?:system|cbu|client)/i);
            return match ? { cbu_id: match[1] } : {};
        }
    },
    scalePlanet: {
        triggers: ['planet', 'entity', 'focus on', 'show me', 'zoom to'],
        command: 'ScalePlanet',
        extractParams: (text) => {
            const match = text.match(/(?:planet|entity|focus on|show me|zoom to)\s+(.+)/i);
            return match ? { entity_id: match[1].trim() } : {};
        }
    },
    scaleSurface: {
        triggers: ['surface', 'attributes', 'details', 'properties'],
        command: 'ScaleSurface',
        params: null
    },
    scaleCore: {
        triggers: ['core', 'raw data', 'json', 'source'],
        command: 'ScaleCore',
        params: null
    },

    // =========================================================================
    // Classic Esper Commands (Zoom/Pan)
    // =========================================================================
    enhance: {
        triggers: ['enhance', 'zoom in', 'closer', 'magnify'],
        command: 'ZoomIn',
        params: { factor: 1.5 }
    },
    pullBack: {
        triggers: ['pull back', 'zoom out', 'wider', 'back up', 'further'],
        command: 'ZoomOut',
        params: { factor: 1.5 }
    },
    zoomFit: {
        triggers: ['fit', 'fit all', 'show all', 'reset zoom'],
        command: 'ZoomFit',
        params: null
    },
    panLeft: {
        triggers: ['pan left', 'go left', 'move left', 'left'],
        command: 'Pan',
        params: { direction: 'left', amount: 100 }
    },
    panRight: {
        triggers: ['pan right', 'go right', 'move right', 'right'],
        command: 'Pan',
        params: { direction: 'right', amount: 100 }
    },
    panUp: {
        triggers: ['pan up', 'go up', 'move up', 'up'],
        command: 'Pan',
        params: { direction: 'up', amount: 100 }
    },
    panDown: {
        triggers: ['pan down', 'go down', 'move down', 'down'],
        command: 'Pan',
        params: { direction: 'down', amount: 100 }
    },
    center: {
        triggers: ['center', 'recenter', 're-center'],
        command: 'Center',
        params: null
    },
    stop: {
        triggers: ['stop', 'halt', 'freeze', 'hold'],
        command: 'Stop',
        params: null
    },

    // =========================================================================
    // Depth Navigation (Z-axis)
    // =========================================================================
    drillThrough: {
        triggers: ['drill through', 'drill down', 'go deeper', 'dive in', 'penetrate'],
        command: 'DrillThrough',
        params: null
    },
    surfaceReturn: {
        triggers: ['surface', 'come up', 'back to top', 'ascend', 'rise'],
        command: 'SurfaceReturn',
        params: null
    },
    xray: {
        triggers: ['x-ray', 'xray', 'see through', 'transparent'],
        command: 'XRay',
        params: null
    },
    peel: {
        triggers: ['peel', 'peel back', 'remove layer', 'strip'],
        command: 'Peel',
        params: null
    },
    crossSection: {
        triggers: ['cross section', 'cross-section', 'slice', 'cut through'],
        command: 'CrossSection',
        params: null
    },

    // =========================================================================
    // Orbital Navigation
    // =========================================================================
    orbit: {
        triggers: ['orbit', 'circle', 'rotate around', 'spin around'],
        command: 'Orbit',
        extractParams: (text) => {
            const match = text.match(/orbit\s+(?:around\s+)?(.+)/i);
            return match ? { entity_id: match[1].trim() } : {};
        }
    },
    rotateLayer: {
        triggers: ['rotate layer', 'spin layer', 'turn layer'],
        command: 'RotateLayer',
        extractParams: (text) => {
            const match = text.match(/(?:rotate|spin|turn)\s+(?:the\s+)?(\w+)\s+layer/i);
            return match ? { layer: match[1] } : { layer: 'ownership' };
        }
    },
    flip: {
        triggers: ['flip', 'flip view', 'invert', 'upside down'],
        command: 'Flip',
        params: null
    },
    tilt: {
        triggers: ['tilt', 'angle', 'perspective'],
        command: 'Tilt',
        extractParams: (text) => {
            const match = text.match(/tilt\s+(?:to\s+)?(\w+)/i);
            return match ? { dimension: match[1] } : { dimension: 'ownership' };
        }
    },

    // =========================================================================
    // Temporal Navigation
    // =========================================================================
    timeRewind: {
        triggers: ['rewind', 'go back in time', 'show me last', 'historical', 'as of'],
        command: 'TimeRewind',
        extractParams: (text) => {
            // Try to extract date: "rewind to 2023", "show me last month"
            const yearMatch = text.match(/(\d{4})/);
            if (yearMatch) return { target_date: yearMatch[1] };

            const relativeMatch = text.match(/last\s+(week|month|year|quarter)/i);
            if (relativeMatch) return { target_date: relativeMatch[1] };

            return {};
        }
    },
    timePlay: {
        triggers: ['play', 'animate', 'show changes', 'evolution'],
        command: 'TimePlay',
        params: null
    },
    timeFreeze: {
        triggers: ['freeze', 'pause', 'stop time', 'snapshot'],
        command: 'TimeFreeze',
        params: null
    },
    timeSlice: {
        triggers: ['compare', 'diff', 'difference', 'what changed'],
        command: 'TimeSlice',
        params: null
    },
    timeTrail: {
        triggers: ['trail', 'history', 'timeline', 'track changes'],
        command: 'TimeTrail',
        extractParams: (text) => {
            const match = text.match(/(?:trail|history|timeline)\s+(?:of\s+)?(.+)/i);
            return match ? { entity_id: match[1].trim() } : {};
        }
    },

    // =========================================================================
    // Investigation Patterns
    // =========================================================================
    followMoney: {
        triggers: ['follow the money', 'trace funds', 'money flow', 'cash trail', 'where does the money'],
        command: 'FollowTheMoney',
        extractParams: (text) => {
            const match = text.match(/from\s+(.+)/i);
            return match ? { from_entity: match[1].trim() } : {};
        }
    },
    whoControls: {
        triggers: ['who controls', 'who owns', 'ownership chain', 'control chain', 'who is behind'],
        command: 'WhoControls',
        extractParams: (text) => {
            const match = text.match(/(?:who controls|who owns|who is behind)\s+(.+)/i);
            return match ? { entity_id: match[1].trim() } : {};
        }
    },
    illuminate: {
        triggers: ['illuminate', 'highlight', 'show me the', 'light up'],
        command: 'Illuminate',
        extractParams: (text) => {
            const match = text.match(/(?:illuminate|highlight|show me the|light up)\s+(.+)/i);
            return match ? { aspect: match[1].trim() } : { aspect: 'risks' };
        }
    },
    shadow: {
        triggers: ['shadow', 'dim', 'fade out', 'hide low risk'],
        command: 'Shadow',
        params: null
    },
    redFlagScan: {
        triggers: ['red flag', 'scan for risks', 'show risks', 'risk scan', 'what are the risks'],
        command: 'RedFlagScan',
        params: null
    },
    blackHole: {
        triggers: ['black hole', 'what\'s missing', 'gaps', 'incomplete', 'where does it go dark'],
        command: 'BlackHole',
        params: null
    },

    // =========================================================================
    // Context Intentions
    // =========================================================================
    contextReview: {
        triggers: ['review mode', 'periodic review', 'annual review'],
        command: 'ContextReview',
        params: null
    },
    contextInvestigation: {
        triggers: ['investigation mode', 'forensic', 'deep dive', 'investigate'],
        command: 'ContextInvestigation',
        params: null
    },
    contextOnboarding: {
        triggers: ['onboarding mode', 'new client', 'setup'],
        command: 'ContextOnboarding',
        params: null
    },
    contextMonitoring: {
        triggers: ['monitoring mode', 'watch', 'alert mode'],
        command: 'ContextMonitoring',
        params: null
    },
    contextRemediation: {
        triggers: ['remediation mode', 'fix', 'resolve issues'],
        command: 'ContextRemediation',
        params: null
    },

    // =========================================================================
    // View Mode Commands
    // =========================================================================
    viewKyc: {
        triggers: ['kyc view', 'show kyc', 'compliance view'],
        command: 'SetViewMode',
        params: { view_mode: 'kyc_ubo' }
    },
    viewServices: {
        triggers: ['services view', 'show services', 'delivery view'],
        command: 'SetViewMode',
        params: { view_mode: 'service_delivery' }
    },
    viewTrading: {
        triggers: ['trading view', 'show trading', 'custody view'],
        command: 'SetViewMode',
        params: { view_mode: 'trading' }
    },

    // =========================================================================
    // Filter Commands
    // =========================================================================
    filterType: {
        triggers: ['show only', 'filter to', 'just show'],
        command: 'FilterByType',
        extractParams: (text) => {
            const match = text.match(/(?:show only|filter to|just show)\s+(.+)/i);
            return match ? { type_codes: [match[1].trim().toUpperCase()] } : {};
        }
    },
    clearFilter: {
        triggers: ['clear filter', 'show all', 'remove filter', 'reset filter'],
        command: 'ClearFilter',
        params: null
    },

    // =========================================================================
    // Help
    // =========================================================================
    help: {
        triggers: ['help', 'what can you do', 'commands', 'how do i'],
        command: 'Help',
        params: null
    }
};

export class EsperCommandMatcher {
    constructor() {
        this.patterns = COMMAND_PATTERNS;
        this.lastMatch = null;
    }

    /**
     * Match a transcript to a command
     * @param {string} transcript - The voice transcript
     * @returns {MatchedCommand|null}
     */
    match(transcript) {
        const text = transcript.toLowerCase().trim();

        if (!text || text.length < 2) {
            return null;
        }

        // Try each pattern
        for (const [key, pattern] of Object.entries(this.patterns)) {
            for (const trigger of pattern.triggers) {
                if (text.includes(trigger.toLowerCase())) {
                    // Found a match
                    let params = pattern.params || {};

                    // Extract dynamic params if extractor exists
                    if (pattern.extractParams) {
                        params = { ...params, ...pattern.extractParams(text) };
                    }

                    const match = {
                        verb: pattern.command,
                        transcript: transcript,
                        confidence: this._calculateConfidence(text, trigger),
                        params: params
                    };

                    this.lastMatch = match;
                    return match;
                }
            }
        }

        return null;
    }

    /**
     * Calculate match confidence based on how well the trigger matches
     */
    _calculateConfidence(text, trigger) {
        // Exact match = high confidence
        if (text === trigger) return 1.0;

        // Starts with trigger = good confidence
        if (text.startsWith(trigger)) return 0.9;

        // Contains trigger = medium confidence
        const ratio = trigger.length / text.length;
        return Math.min(0.85, 0.5 + ratio * 0.4);
    }

    /**
     * Get all available commands for help display
     */
    getAvailableCommands() {
        const commands = [];
        for (const [key, pattern] of Object.entries(this.patterns)) {
            commands.push({
                command: pattern.command,
                triggers: pattern.triggers.slice(0, 3), // First 3 examples
                hasParams: !!(pattern.extractParams || pattern.params)
            });
        }
        return commands;
    }
}
