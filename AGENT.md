# AGENT.md

This file provides guidance to agent (Claude Code / other coding agents) when working with code in this repository.

## Project Overview

This repository is the **WPS AI desktop application** — a Tauri 2 + React app. `packages/desktop` is the only distributable package: `src/` is the React frontend, `src-tauri/` the Rust backend (app shell + layered crates). Change-workflow knowledge (phase role prompts) and phase orchestration are carried natively by the desktop app; the repository ships no plugin layer.

## Project Rules

- **Version discipline**: bump `packages/desktop/package.json` `version` by one patch for every desktop source change. `src-tauri/tauri.conf.json` follows via `"version": "../package.json"`; `src-tauri/Cargo.toml` keeps its own crate version line and MUST NOT follow. Releasing is the workflow's job (`.github/workflows/desktop-release.yml`) — no manual tags, never `git push --tags`.
- **Spec corpus scope**: `openspec/specs/` holds surviving capabilities only. Retiring an asset means deleting its specs in the same change — no empty shells, no `TBD` placeholders, no specs describing deleted code.
- **Config schema is hand-maintained**: `src-tauri/crates/core/config/config.schema.json` is a static JSON mirror (editors consume it through the raw `$schema` URL, it is not bundled). Adding or removing a `KNOWN_KEYS` entry or a `TestFramework` variant requires updating the mirror in the same commit; the consistency tests in `crates/core/config/src/lib_test.rs` enforce both directions.
- **Config isolation**: workspace config is read only through the `config` crate. Product `.rs` files MUST NOT contain the `config.json` filename literal (sole exception: `foundation/src/layout/mod.rs`), and `openspec` path strings go through `foundation::layout`.

## Architecture

- Backend crates live in `src-tauri/crates/` in three groups. `core/`: `foundation` (layout / path resolution), `config` (workspace config read + validation), `workflow` (phase table, write face), `orchestration` (walker / step loop / report-driven routing), `agent` (session execution), `checks` (report semantics). `infra/`: adapters `agent`, `checks`, `exec`, `store`, `vcs`, `watch`. `tools/export-bindings`: TypeScript type generation. Dependency direction is one-way (infra → core, never core → infra).
- `src-tauri/src/` is the Tauri app shell: IPC commands, error mapping, bindings export. It wires core/infra ports and owns no domain rules.
- Phase role prompts are 16 static markdown files under `crates/core/workflow/src/prompts/` (7 executor + 9 evaluator), assembled at compile time through literal `include_str!` paths — fully static, zero placeholders.
- `crates/infra/agent/src/sdk/protect/` is the write-protection face (built-in locked files + user globs from `openspec/config.json`; shell / node / python write detection), mounted as a segment of the agent loop guard chain.
- The frontend under `packages/desktop/src/` is React + Vite (vite-plus tooling: `vp check`, `vp test`, `knip` dead-code scan); generated IPC types live in `src/types/generated/` and are refreshed by `bindings:export` (staleness confirmed by reviewing the git diff before commit).

## Coding Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. **Think before coding**: State assumptions. If unclear, stop and ask.
2. **Simplicity first**: No speculative features. No abstractions for single-use code.
3. **Surgical changes**: Touch only what's needed. Match existing style.
4. **Goal-Driven Execution**: Define success criteria. Loop until verified.
5. **No test-only exports**: Do not add `export` solely to make a symbol accessible to tests. knip treats exports only imported by test files as unused, which pollutes the dead-code report. Instead, test through the module's public API, or refactor so the logic is testable without extra exports.
