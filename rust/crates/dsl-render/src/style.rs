//! Embedded CSS styles for SVG output.

pub const EMBEDDED_CSS: &str = "\n  \
.gateway polygon { filter: drop-shadow(1px 1px 2px rgba(0,0,0,0.15)); }\n  \
.service-task rect, .user-task rect, .subprocess rect, \
.business-rule-task rect, .task rect {\n    \
filter: drop-shadow(1px 1px 2px rgba(0,0,0,0.1));\n  \
}\n  \
text { user-select: none; }\n  \
path { transition: stroke 0.1s; }\n";
