# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **wps-claude-plugin**, a custom Claude Code plugin designed to enhance development workflow with OpenSpec integration.

## Project Rules

- **Upgrade plugin version** after changing plugin source: bump `plugins/<name>/package.json` `version`, then rebuild dual products with `pnpm -C plugins/<name> run build` (refreshes `claude-plugins/<name>/` and `cursor-plugins/<name>/`).

## Architecture: Slim Plugin + Report-Driven Gates

The plugin follows a "slim" architecture:
- **OpenSpec CLI** is the source of truth for skills (explore, propose, apply-change, archive-change)
- **Plugin provides hooks** for report-driven workflow gates
- **Plugin provides agents** for specialized, context-intensive work (code review)
- **Plugin provides Python utilities** for deterministic, scriptable tasks (architecture validation, model management, ADRs)
- **No embedded skills** - skills are invoked via OpenSpec CLI directly; plugin skills are thin routing wrappers

## Coding Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. **Think before coding**: State assumptions. If unclear, stop and ask.
2. **Simplicity first**: No speculative features. No abstractions for single-use code.
3. **Surgical changes**: Touch only what's needed. Match existing style.
4. **Goal-Driven Execution**: Define success criteria. Loop until verified.
5. **No test-only exports**: Do not add `export` solely to make a symbol accessible to tests. knip treats exports only imported by test files as unused, which pollutes the dead-code report. Instead, test through the module's public API, or refactor so the logic is testable without extra exports.
