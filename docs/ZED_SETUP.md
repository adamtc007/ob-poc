# Zed Editor Setup for DSL

This guide explains how to set up Zed editor for first-class DSL support with syntax highlighting, LSP features, and custom tasks.

## Prerequisites

- [Zed Editor](https://zed.dev/) installed
- Rust toolchain (for building the LSP)
- Node.js (for tree-sitter CLI)

## Installation

### 1. Build the DSL Language Server

```bash
cd rust/
cargo build --release -p dsl-lsp
```

The LSP binary will be at `target/release/dsl-lsp`.

### 2. Install the Zed Extension

Copy the extension to Zed's extensions directory:

```bash
# macOS
cp -r rust/crates/dsl-lsp/zed-extension ~/.config/zed/extensions/dsl

# Linux
cp -r rust/crates/dsl-lsp/zed-extension ~/.config/zed/extensions/dsl
```

### 3. Configure Zed Settings

Add to your `~/.config/zed/settings.json`:

```json
{
  "languages": {
    "DSL": {
      "tab_size": 2,
      "format_on_save": "on"
    }
  },
  "lsp": {
    "dsl-lsp": {
      "binary": {
        "path": "/path/to/ob-poc/rust/target/release/dsl-lsp"
      }
    }
  }
}
```

### 4. Regenerate Tree-sitter Grammar (if needed)

If you modify the grammar:

```bash
cd rust/crates/dsl-lsp/tree-sitter-dsl
npx tree-sitter generate
npx tree-sitter test
```

## Features

### Syntax Highlighting

The extension provides semantic highlighting for:

| Element | Color Category |
|---------|----------------|
| Verb names | `@function` |
| Keywords (`:name`, `:id`) | `@property` |
| Strings | `@string` |
| Numbers | `@number` |
| Booleans | `@constant.builtin` |
| Comments | `@comment` |
| Bindings (`:as`) | `@keyword.special` |
| Symbol refs (`@name`) | `@variable.special` |
| Entity refs (`<Name>`) | `@variable` |

### Rainbow Brackets

Nested parentheses, brackets, and braces are colorized for easy matching.

### Document Outline

Press `Cmd+Shift+O` (macOS) or `Ctrl+Shift+O` (Linux) to see:

- All verb calls in the file
- Binding names (e.g., `cbu.create → @fund`)
- Intent comments as annotations

### Code Completion

The LSP provides completions for:

- **Verb names**: Type `cbu.` to see all CBU verbs
- **Keywords**: After a verb, get argument suggestions
- **Symbol refs**: Type `@` to see available bindings
- **Entity refs**: Type `<` to search entities

### Diagnostics

Real-time error checking for:

- Syntax errors (missing parens, invalid tokens)
- Undefined symbol references
- Type mismatches
- Verb validation errors

### Rename Symbol

Select a binding (`@fund`) and press `F2` to rename it across the file.

### Text Objects

Use Vim-style text objects:

| Command | Selects |
|---------|---------|
| `vaf` | Around function (verb call) |
| `vif` | Inside function |
| `vac` | Around class (map) |
| `vic` | Inside class |

### Snippets

Type these prefixes and press `Tab`:

| Prefix | Expands To |
|--------|------------|
| `cbu` | CBU create template |
| `cbue` | CBU ensure template |
| `person` | Entity person template |
| `company` | Entity company template |
| `role` | Role assignment template |
| `kyc` | KYC case template |
| `intent` | Intent comment block |
| `load` | Session load template |

## Tasks

The project includes Zed tasks in `.zed/tasks.json`:

| Task | Description |
|------|-------------|
| DSL: Validate Form | Validate current DSL file |
| DSL: Format File | Auto-format current file |
| DSL: Dump Tree-sitter AST | Show parse tree |
| DSL: Run LSP Tests | Run LSP test suite |
| DSL: Run Parser Tests | Run dsl-core tests |
| DSL: Run Corpus Tests | Run tree-sitter tests |
| DSL: Regenerate Grammar | Regenerate tree-sitter |

Access tasks with `Cmd+Shift+P` → "task: spawn".

### Run Buttons

When cursor is on a verb call, a run button appears in the gutter. Click to run the "DSL: Validate Form" task for that expression.

## Troubleshooting

### Extension Not Loading

1. Check extension is in correct directory:
   ```bash
   ls ~/.config/zed/extensions/dsl/
   # Should show: extension.toml, languages/, snippets/
   ```

2. Restart Zed completely

3. Check Zed logs:
   ```bash
   tail -f ~/.config/zed/logs/Zed.log
   ```

### LSP Not Starting

1. Verify binary exists and is executable:
   ```bash
   ls -la /path/to/dsl-lsp
   ./path/to/dsl-lsp --version
   ```

2. Check LSP configuration in settings.json

3. Look for LSP errors in Zed's output panel (`View → Output`)

### Syntax Highlighting Broken

1. Ensure grammar is up to date:
   ```bash
   cd rust/crates/dsl-lsp/tree-sitter-dsl
   npx tree-sitter generate
   ```

2. Check `config.toml` has `grammar = "dsl"` (not `clojure`)

3. Verify highlights.scm uses DSL node names (`verb_name`, `binding`, etc.)

### Completions Not Working

1. Check LSP is running (status bar should show "DSL")

2. Verify verb registry is loaded:
   ```bash
   curl http://localhost:8080/api/verbs | jq '.total'
   ```

3. Ensure DATABASE_URL is set if using semantic search

## Development

### Modifying the Grammar

1. Edit `tree-sitter-dsl/grammar.js`
2. Regenerate: `npx tree-sitter generate`
3. Test: `npx tree-sitter test`
4. Update corpus tests in `test/corpus/`

### Adding Snippets

Edit `zed-extension/snippets/dsl.json`:

```json
{
  "Snippet Name": {
    "prefix": "trigger",
    "body": [
      ";; intent: ${1:description}",
      "(verb.name",
      "  :arg \"${2:value}\"",
      "  :as @${3:binding})"
    ]
  }
}
```

### Adding Highlighting Rules

Edit `languages/dsl/highlights.scm`:

```scheme
;; New node type
(my_node) @my.scope
```

### Adding Tasks

Edit `.zed/tasks.json`:

```json
{
  "label": "My Task",
  "command": "cargo",
  "args": ["run", "-p", "my-package"],
  "reveal": "always"
}
```

## File Associations

The extension recognizes these file patterns:

- `*.dsl` - DSL source files
- `*.playbook.yaml` - Playbook definitions (uses YAML highlighting + DSL LSP)

## Resources

- [DSL Style Guide](./DSL_STYLE_GUIDE.md) - Formatting conventions
- [Golden Examples](./dsl/golden/) - Reference DSL files
- [Verb Definition Spec](./verb-definition-spec.md) - YAML verb authoring
- [Tree-sitter Docs](https://tree-sitter.github.io/tree-sitter/) - Grammar reference
