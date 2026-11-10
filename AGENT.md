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

## 模块 API 契约

- **手写真相只有一份**：`.dev-team/architecture/api/<Domain>/<FQN>.md`（`<Domain>` = 父元素 FQN 末段）。散文 + `json api-op` / `json api-types` / `json api-reexport` 三类 fenced json 块；与代码一致性由机械闭环守，不靠 review 纪律。
- **四类生成物勿手改**：`.schema.json`（同目录）、`INDEX.toml`（契约目录根）、元素根同级的 `<root>.i.rs` / `<root>.i.ts`。手改已提交即零 diff 门失败，未提交会被下次生成静默覆盖。
- **两个脚本**：`api:export` 生成、`api:check` 读盘比对（零写入，差异即非零退出）。`check` 链为四段：`server:check` → `bindings:export` → `api:check` → `client:check`。
- **`api = none` 是显式留白**（模型元素字段，非「还没写」）：断言本模块无公共 API；写了契约文件反而触发 `apiUnexpected`。
- **契约判据进报告但不进 status**：漂移报告的并列槽位 `apiFindings`（定义见 `desktop-archi-api-check`），首期不影响 `status` / `passed`；索引与计数经 `.dev-team/architecture/api/INDEX.toml` 一眼看全。

## Coding Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. **Think before coding**: State assumptions. If unclear, stop and ask.
2. **Simplicity first**: No speculative features. No abstractions for single-use code.
3. **Surgical changes**: Touch only what's needed. Match existing style.
4. **Goal-Driven Execution**: Define success criteria. Loop until verified.
5. **No test-only exports**: Do not add `export` solely to make a symbol accessible to tests. knip treats exports only imported by test files as unused, which pollutes the dead-code report. Instead, test through the module's public API, or refactor so the logic is testable without extra exports.
6. **Comments are the exception, not the default**: Let naming and structure carry the intent — do not restate in a comment what the code already says. Write a comment only where it carries information the code cannot express: a non-obvious reason, a deliberate tradeoff, a workaround and its cause, or an external constraint. If a comment is needed to explain *what* a block does, fix the code instead.
