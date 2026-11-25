# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This repository is the **WPS AI desktop application** — a Tauri 2 + React app. `packages/desktop` is the only distributable package: `src/` is the React frontend, `src-tauri/` the Rust backend (app shell + layered crates). Change-workflow knowledge (phase role prompts) and phase orchestration are carried natively by the desktop app; the repository ships no plugin layer.

## Project Rules

- **Version discipline**: bump `packages/desktop/package.json` `version` by one patch for every desktop source change. `src-tauri/tauri.conf.json` follows via `"version": "../package.json"`; `src-tauri/Cargo.toml` keeps its own crate version line and MUST NOT follow. Releasing is the workflow's job (`.github/workflows/desktop-release.yml`) — no manual tags, never `git push --tags`.
- **Spec corpus scope**: `openspec/specs/` holds surviving capabilities only. Retiring an asset means deleting its specs in the same change — no empty shells, no `TBD` placeholders, no specs describing deleted code.
- **Config schema is hand-maintained**: `src-tauri/crates/core/config/config.schema.json` is a static JSON mirror (editors consume it through the raw `$schema` URL, it is not bundled). Adding or removing a `KNOWN_KEYS` entry or a `TestFramework` variant requires updating the mirror in the same commit; the consistency tests in `crates/core/config/src/lib_test.rs` enforce both directions.
- **Config isolation**: workspace config is read only through the `config` crate. Product `.rs` files MUST NOT contain the `config.json` filename literal (sole exception: `foundation/src/layout/mod.rs`), and `openspec` path strings go through `foundation::layout`.

## Architecture

- Backend crates are layered: `src-tauri/crates/core/` holds domain crates, `infra/` holds adapters, `tools/` holds generation tooling. Dependency direction is one-way (infra → core, never core → infra). The crate inventory and per-crate responsibilities are owned by the architecture model and the `desktop-*` specs, not here.
- `src-tauri/src/` is the Tauri app shell: IPC commands, error mapping, bindings export. It wires core/infra ports and owns no domain rules.
- Generated artifacts are never hand-edited: architecture `.i.*` / `.schema.json` / `INDEX.toml` and frontend `src/types/generated/` (refresh via `bindings:export`, then review the git diff before commit). Frontend tooling is vite-plus (`vp`) — no bare vite/vitest.

## 模块 API 契约

- **手写真相只有一份**：`.dev-team/architecture/models/<分层目录…>/<FQN>.md`（目录 = 元素 FQN 段链去掉叶段：`Backend.Core.Foundation` → `Backend/Core/`）。散文 + `json api-op` / `json api-types` / `json api-reexport` 三类 fenced json 块；名字清单的唯一持有者也是这三类 fence——散文段 MUST NOT 用表格复述契约名字（重复持有即装载期契约错误），散文只留语义；与代码一致性由机械闭环守，不靠 review 纪律。模型声明（`NN-<域>.toml`）在 `.dev-team/architecture/graph/model/`，契约资产在 `.dev-team/architecture/models/`——两者按目录分家。

## Coding Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. **Think before coding**: State assumptions. If unclear, stop and ask.
2. **Simplicity first**: No speculative features. No abstractions for single-use code.
3. **Surgical changes**: Touch only what's needed. Match existing style.
4. **Goal-Driven Execution**: Define success criteria. Loop until verified.
5. **No test-only exports**: Do not add `export` solely to make a symbol accessible to tests. knip treats exports only imported by test files as unused, which pollutes the dead-code report. Instead, test through the module's public API, or refactor so the logic is testable without extra exports.
6. **Comments are the exception, not the default**: Let naming and structure carry the intent — do not restate in a comment what the code already says. Write a comment only where it carries information the code cannot express: a non-obvious reason, a deliberate tradeoff, a workaround and its cause, or an external constraint. If a comment is needed to explain *what* a block does, fix the code instead.
