# desktop-change-create Specification (Delta)

## MODIFIED Requirements

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **工作流类型入参暴露 requirement / test-only**：`workflow_type` 入参仅接受 `phase_table` 命中的两值；bug-fix 等类型随其相位表变更一起开放；
2. **名称用户输入**：不由 agent 从 goal 派生名称（异步、成本高）；agent 派生留作后续增强；
3. **仅拒 active 冲突**：只拒同名 active，归档树同名不拒绝（清单以代际徽标区分）——判据即本 spec 的 `create` 前置规则，MUST NOT 以对偶实现（旧 `createChange`）的 parity 作为依据；
4. **创建后不自动发起 run**：run 由用户在详情 / flow 视图显式触发；
5. **goal 无桌面编辑入口**：详情只读渲染；goal 修正走既有文件手改或后续变更；

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.0` 升级为 `0.4.1`（用户可见新功能；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.1，`src-tauri/Cargo.toml` 版本不随动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | change 创建域操作 | 唯一创建实现；前置冲突只拒同名 active（无 parity 对偶参照） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（建档半边） | id 主键写入 | 契约不变 |
| `packages/desktop/package.json` | 版本交付 | 0.4.0 → 0.4.1；`src-tauri/Cargo.toml` 不随动（插件半句随插件退役摘除） |
| 插件期 `change-create.ts` / MCP 工具面 / 三类交付产物 | **（删除）** | 随插件整目录退役；原边界「插件侧零改动」整条删除、原「与 `createChange` parity」从句改为直述本 spec 判据 |
