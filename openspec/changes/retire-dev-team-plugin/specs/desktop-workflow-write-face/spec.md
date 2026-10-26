# desktop-workflow-write-face Specification (Delta)

## MODIFIED Requirements

### Requirement: backtrack 白名单二次校验与 stale 语义

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态，状态库零变更）；校验通过 SHALL 落回跳记录、对被跳过相位的 PhaseRecord 打 stale 标记并传播（与回跳落账同一事务语义），`reason` 超过 1000 字符 SHALL 被拒绝。`backtrack_to` 与发起相位相同（自环）SHALL 在白名单含该相位时正常受理——这是「重做本相位」的唯一载体（白名单自含规则见「相位定义有序 step 序列与 artifact 清单」）：落账语义为改写最新条目 `backtrack_to` + 目标相位下游 stale 传播，`phase_next` 随后以新 attempt 路由回该相位；白名单不含自身时（零 agent 步相位）自环 SHALL 被拒且状态库零变更。该口径为桌面写面自有裁定：MUST NOT 沿用「目标索引必须小于当前相位」类旧约束——承载该约束的对偶 spec 已随插件退役整目录删除，桌面路径只受本条白名单规则约束，回跳合法性的唯一判据是 `allowed_backtrack_phases`。

#### Scenario: 自环裁定单源自持

- **WHEN** 审查桌面写面 `backtrack` 的合法性判据
- **THEN** 判据只有 `allowed_backtrack_phases` 白名单（含自环三态：白名单内受理 / 不含自身拒绝 / 越权拒绝零落账）
- **AND** 无「与插件侧 spec 各管其面 / 插件零改动」类对偶条款或注释（对偶 spec 已删除）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/` 写面模块 | change 流程状态域权威写面 | `backtrack` 白名单二次校验 + stale 传播语义不变；合法性判据单源（本 spec） |
| `crates/core/orchestration/src/prompt.rs`（不改） | prompt 组装 | 模板取自 core/workflow 相位表单源 |
| 插件 MCP / TS 写路径 | **（删除）** | 随插件整目录退役；Module Contract 原「插件 MCP / TS 写路径（彻底分离，不改）」行整行删除——双向墙的另一方已不存在，「墙」不再是约束而是既成事实 |
