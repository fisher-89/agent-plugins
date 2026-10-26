# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.36` 升级为 `0.4.37`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop` version 为 0.4.37 且 `src-tauri/Cargo.toml` 版本不随动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/package.json` | 版本交付 | 0.4.36 → 0.4.37；`src-tauri/Cargo.toml` 不随动 |
| 插件期 CLI skill 语料（`skills/**`） | **（删除）** | 随插件整目录退役；原「CLI skill 的下线处置另行变更承载」从句随之消失（下线即本变更），归档链语义自此完全自持 |
