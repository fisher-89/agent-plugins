# desktop-change-worktree Specification (Delta)

## MODIFIED Requirements

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.13` 升级为 `0.4.14`（用户可见新功能；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.14，`src-tauri/Cargo.toml` 版本不随动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/package.json` | 版本交付 | 0.4.13 → 0.4.14；`src-tauri/Cargo.toml` 不随动（原「不 bump 插件、不动三类交付产物」半句随插件退役摘除） |
