# desktop-release-pipeline Specification (Delta)

## MODIFIED Requirements

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.29` 升级为 `0.4.30`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.30，`src-tauri/Cargo.toml` 版本不随动

## ADDED Requirements

### Requirement: 版本交付纪律

仓库只有 `packages/desktop` 一个可分发包，版本交付 SHALL 按单包纪律执行，MUST NOT 存在第二个包的 bump / rebuild 步骤：

- 每次桌面源码变更 SHALL 在同一变更内把 `packages/desktop/package.json` 的 patch 版本 +1；
- `src-tauri/tauri.conf.json` 的 `version` SHALL 维持对 `../package.json` 的引用（自动跟随），MUST NOT 手写固定版本；
- `src-tauri/Cargo.toml` 的版本 MUST NOT 随包版本 bump 而动（crate 版本线独立）；
- 发版 SHALL 经 `.github/workflows/desktop-release.yml` 的 detect（版本比较 → `should` → tag）→ build matrix → finalize 链闭合；不 bump 版本时 `should=false` 自动跳过，MUST NOT 手工造 tag。

#### Scenario: 单包 bump 一步到位

- **WHEN** 某变更改动了 `packages/desktop` 的源码或资产
- **THEN** 该变更内 `packages/desktop/package.json` patch 版本 +1，`tauri.conf.json` 仍引用 `../package.json`，`src-tauri/Cargo.toml` 版本不变
- **AND** 仓库内无第二份需同步 bump 或重建的产物

#### Scenario: 本变更的版本实例

- **WHEN** 本变更（退役仓库内插件层）实现完成
- **THEN** `packages/desktop/package.json` 由 `0.5.30` 升级为 `0.5.31`，`src-tauri/Cargo.toml` 版本不随动

#### Scenario: 不 bump 即不触发发版

- **WHEN** 变更只动文档 / 语料等不影响包版本的内容且未 bump 版本
- **THEN** `desktop-release.yml` 的 detect 判定 `should=false` 并跳过 build / finalize（无 tag、无发布）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/package.json` | 唯一版本权威 | patch +1 纪律；本变更实例 `0.5.30 → 0.5.31` |
| `packages/desktop/src-tauri/tauri.conf.json` | 打包版本跟随 | `version` 恒为 `../package.json` 引用，MUST NOT 手写固定值 |
| `packages/desktop/src-tauri/Cargo.toml` | crate 版本线 | 不随包版本 bump 而动 |
| `.github/workflows/desktop-release.yml` | 发版流水线 | detect → build matrix → finalize 链不变；不 bump 版本自动跳过；`packages/desktop/**` 内新增非源码资产（如 `crates/core/config/config.schema.json`）只影响 detect 输入面，不改流水线语义 |
| 插件期三类交付产物 | **（删除）** | `dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team` 与 marketplace 登记随插件退役整体删除；「本变更 SHALL NOT 变更插件版本与交付产物」类条款自此消失（无第二个包可动） |
