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

- Backend crates are layered: `src-tauri/crates/core/` holds domain crates, `infra/` holds adapters, `tools/` holds generation tooling. Dependency direction is one-way (infra → core, never core → infra). The crate inventory and per-crate responsibilities are owned by the architecture model and the `desktop-*` specs, not here.
- `src-tauri/src/` is the Tauri app shell: IPC commands, error mapping, bindings export. It wires core/infra ports and owns no domain rules.
- Generated artifacts are never hand-edited: architecture `.i.*` / `.schema.json` / `INDEX.toml` and frontend `src/types/generated/` (refresh via `bindings:export`, then review the git diff before commit). Frontend tooling is vite-plus (`vp`) — no bare vite/vitest.

## 模块 API 契约

- **三资产分工与落位**：`<archi_models_dir>/<分层目录…>/<FQN>.md` 写理由（文档，无结构；产品面：架构页直接渲染给用户读）；同目录 `<FQN>.schema.json` 写接口（**唯一凭据**，手写真相，写前过校验）。文档 MUST NOT 枚举导出名（名字清单的唯一持有者是 schema）。历史 `api-*` fence 是遗留内容（读面不读，清理中），MUST NOT 新增。
- **三类生成物勿手改**：`INDEX.toml`（`<archi_graph_dir>/`）、实现入口同级的 `<入口主干>.i.rs` / `<入口主干>.i.ts`。手改已提交即零 diff 门失败，未提交会被下次生成静默覆盖。
- **两个脚本**：`api:export` 生成、`api:check` 读盘比对（零写入，差异即非零退出）。`check` 链为四段：`server:check` → `bindings:export` → `api:check` → `client:check`。
- **`api = none` 是显式留白**（模型元素字段，非「还没写」）：断言本模块无公共 API，写了接口文件反而触发 `apiUnexpected`；留白的正规形态是空 schema（`$defs {}` + `properties {}`），与「接口还没写（文件缺）」严格可分。
- **纳管口径**：漂移报告并列槽位 `apiFindings` 只把「`api = declared` ∧ schema 在盘」元素的判据折算进 `status`（`error` 档还进 `passed`），文档债 `docMissing` 恒 `warn`、永不纳管；判据定义见 `desktop-archi-api-check`。索引与计数经 `<archi_graph_dir>/INDEX.toml`，两轴（生成物新鲜度 / 契约 vs 代码）经架构页「契约」视图一眼看全。

## Coding Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. **Think before coding**: State assumptions. If unclear, stop and ask.
2. **Simplicity first**: No speculative features. No abstractions for single-use code.
3. **Surgical changes**: Touch only what's needed. Match existing style.
4. **Goal-Driven Execution**: Define success criteria. Loop until verified.
5. **No test-only exports**: Do not add `export` solely to make a symbol accessible to tests. knip treats exports only imported by test files as unused, which pollutes the dead-code report. Instead, test through the module's public API, or refactor so the logic is testable without extra exports.
6. **Comments are the exception, not the default**: Let naming and structure carry the intent — do not restate in a comment what the code already says. Write a comment only where it carries information the code cannot express: a non-obvious reason, a deliberate tradeoff, a workaround and its cause, or an external constraint. If a comment is needed to explain *what* a block does, fix the code instead.
