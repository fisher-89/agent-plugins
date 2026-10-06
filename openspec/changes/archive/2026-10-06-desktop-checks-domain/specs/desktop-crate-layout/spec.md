# desktop-crate-layout Delta

## MODIFIED Requirements

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力（desktop-agent-execution）扩展为五类，再经检查域（desktop-checks-domain）扩展为六类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app（通用 shell 执行仅 desktop-app；runner 内部的子进程管理属 agent 边界实现细节，不属于 shell 边界）；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-runtime`）；允许进程 spawn，禁 Tauri；
- **checks 边界**：外部进程 + pass/fail 结论门禁的检查域（现有成员：static-check、test-execution）——纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；允许进程 spawn，禁 Tauri；runner port 留 `core/orchestration`（port 属于消费者，spawn 不进 core）。

领域核心（change 域读模型、agent 域契约、检查域纯层）与应用编排（现为 command）位于各边界之内：core 各 crate 纯依赖；`store` 不依赖 core 行为（仅依赖 `agent` 纯类型作嵌装载荷，规则修订见「crate 三层分组与依赖规则」），`agent-runtime` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证（既有 requirement 不变）。`infra/` 目录表达 db / api / agent / checks 等边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent / checks 六类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: 检查步归类 checks 边界

- **WHEN** 评审 static-check 与 test-execution 执行步的落位
- **THEN** 归 checks 边界：core/checks 纯层零 spawn 零 Tauri，infra/checks 执行层允许 spawn；检查域成员不出现在 infra/agent

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate（含 agent-runtime 与 checks 两 crate）的依赖声明
- **THEN** 无任何 Tauri 系依赖（tauri、tauri-plugin-* 等），shell 依赖仅存在于 desktop-app

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| 六类边界分类学 | 落位先声明边界 | shell（仅 desktop-app，runner 进程管理除外）/ db（infra 成员）/ 文件（workspace 读写）/ api（占位留痕）/ agent（core 契约 + infra 实现）/ checks（core/checks 纯层 + infra/checks 执行）；core 与 infra 禁 Tauri |
| `crates/core/checks` + `crates/infra/checks`（新，checks 边界首批成员） | 检查域双层落位 | 依赖：`checks` → config + foundation；`checks-runtime` → orchestration + checks + config + foundation；纯层零 spawn、执行层允许 spawn；两 crate 均零 Tauri（详见 desktop-checks-domain） |
| `crates/infra/agent`（裸名 `agent-runtime`，收窄） | agent 边界实现 | 仅承载会话租户身份（cli / sdk / worker / compose / store_port / git_diff）；static_check 移出 checks 边界；允许进程 spawn；禁 Tauri |
