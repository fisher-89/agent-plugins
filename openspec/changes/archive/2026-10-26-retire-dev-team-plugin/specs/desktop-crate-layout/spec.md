# desktop-crate-layout Specification (Delta)

## MODIFIED Requirements

### Requirement: packages/desktop 完全独立包落位

桌面端 SHALL 落位为 `packages/desktop`，作为完全独立的包存在：自带 lockfile 与构建脚本，MUST NOT 引入仓库根 `package.json` 或 `pnpm-workspace.yaml`，MUST NOT 进入 `dist/` 分发链（`dist/` 的三份产物目录已随插件退役整体删除，仓库自此只有这一个包）。仓库根曾有的「改插件源码后 bump 插件 `package.json` version 并 rebuild 产物」规则随插件退役整条消失；`packages/desktop` 的版本 bump SHALL 只由「版本交付纪律」（见 desktop-release-pipeline）承载，MUST NOT 由任何产物重建步骤驱动、MUST NOT 要求重建第二份产物。

#### Scenario: 独立包不污染主仓

- **WHEN** 检查仓库根目录与 `dist/`
- **THEN** 根目录无新增 `package.json` / `pnpm-workspace.yaml`
- **AND** `dist/` 不含 desktop 产物

#### Scenario: 版本 bump 只走桌面发版纪律

- **WHEN** `packages/desktop` 源码变更
- **THEN** 只需 `packages/desktop/package.json` patch 版本 +1（`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 不随动）
- **AND** 无任何第三方产物 bump 或 rebuild 步骤被要求（仓库内不存在第二个可分发包）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop` | 仓库内唯一可分发包 | 自带 lockfile 与构建脚本；不进仓库根 workspace；不进 `dist/` 分发链（`dist/` 三份产物目录已随插件退役删除） |
| `packages/desktop/src-tauri/Cargo.toml` | cargo workspace 三层分组 | core / infra / desktop-app 三层依赖方向不变；`crates/core/config/config.schema.json` 为 crate 内资产（不入 bundle） |
| `packages/desktop/package.json` | 版本权威 | patch +1 纪律（见 desktop-release-pipeline「版本交付纪律」）；MUST NOT 由产物重建驱动 |
