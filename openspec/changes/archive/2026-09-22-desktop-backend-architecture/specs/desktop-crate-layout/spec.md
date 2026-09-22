# desktop-crate-layout Specification

## ADDED Requirements

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按四类边界组织，任何新特性落位时 SHALL 先声明所属边界：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app；
- **db 边界**：redb（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）。

领域核心（change 域读模型与解释规则）与应用编排（现为 command）位于四类边界之内：core 两 crate 纯依赖，infra 不依赖 core 也不依赖 Tauri，依赖方向由 crate 图机械保证（既有 requirement 不变）。`infra/` 目录表达 db / api 等边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api 四类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate 的依赖声明
- **THEN** 无任何 Tauri 系依赖（tauri、tauri-plugin-* 等），shell 依赖仅存在于 desktop-app

### Requirement: 未来租户归位规则

已规划特性 SHALL 按下表归位，本变更 MUST NOT 据此预建任何目录或 crate：

| 特性 | 边界 | 归位 | 规则 |
|---|---|---|---|
| workspace 写文件 | 文件 | `commands/exec/` 轨道 | trait 形状等第一条真实执行命令落地时定形（既有决策不变）；写用户 repo 属对 repo 的变更，落地时 SHALL 有 write protection / 路径包含性检查 / 审计 |
| workspace 读写 db | db | `infra/store` 的 workspace 维度 | 落盘策略见 desktop-data-dimensions 能力 |
| 图数据库 | db | `infra/graph`（暂不建） | 暂不考虑；关系查询真实痛点出现再启，届时按 db 边界新成员落位 |
| 后端 api | api | `infra/api`（不建） | 先占位留痕：用途未定，MUST NOT 建目录、MUST NOT 动代码 |

`infra/fs` SHALL 在文件边界长出独立适配（区别于 workflow 内纯读逻辑的、可复用的 fs 基础设施）时再立，不预建。

#### Scenario: 无预建目录

- **WHEN** 检查 `crates/infra/` 目录树
- **THEN** 仅 `store` 一个 crate 成员，不存在 `infra/graph` / `infra/api` / `infra/fs` 空目录

#### Scenario: 图数据库与 api 决策留痕

- **WHEN** 查阅本能力 spec
- **THEN** 图数据库"暂不考虑、痛点驱动再启"与 api "先占位留痕"两条裁决可考，后续变更无需重新辩论是否引入

#### Scenario: exec 轨道承接文件写

- **WHEN** 检查 `commands/exec/`
- **THEN** 仍为空轨道（无实现、无空壳 trait）；workspace 写文件将落此轨道而非 queries / workspaces 轨道

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| 四类边界分类学 | 落位先声明边界 | shell（仅 desktop-app）/ db（infra 成员）/ 文件（workspace 读写）/ api（占位留痕）；core 与 infra 禁 Tauri |
| 未来租户归位表 | 特性 → 归位映射 | 写文件 → exec 轨道；读写 db → store workspace 维度；图数据库 → 暂不考虑；api → 不建目录 |
| `crates/infra/` | 目录表达计划 | 仅 store 一个成员；graph / api / fs 均不预建，成员出现才立 |
