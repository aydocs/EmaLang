# EMA Language Support (VS Code)

This extension provides a solid developer experience for `.ema` files:

- Syntax highlighting (EMA + UI tags + `{{ interpolation }}` + embedded block starters)
- Snippets (state / onclick DSL / embedded blocks / HTTP route)
- Diagnostics via `ema_compiler --check --json`
- Formatter: safe indentation + trailing whitespace trimming (never touches heredoc bodies)

## Local development

1. Open this folder in VS Code: `ema-vscode/`
2. `npm install`
3. Press `F5` to start an Extension Development Host

## Diagnostics

By default the extension uses:

1. `target/debug/ema_compiler(.exe)` if it exists
2. Otherwise (if enabled) `cargo run --bin ema_compiler -- <file> --check --json`

Settings:
- `ema.diagnostics.enabled`
- `ema.diagnostics.debounceMs`
- `ema.diagnostics.useCargoFallback`
- `ema.compiler.path` (optional override)

Command:
- `EMA: Check current file`

