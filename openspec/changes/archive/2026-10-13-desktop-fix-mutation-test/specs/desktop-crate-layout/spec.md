# desktop-crate-layout Specification (Delta)

## MODIFIED Requirements

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力扩展为五类，再经检查域扩展为六类，再经 change worktree 扩展为七类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-runtime`）；允许进程 spawn，禁 Tauri；
- **checks 边界**：外部进程 + pass/fail 结论门禁的检查域（现有成员：static-check、test-execution、mutation-test）——纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；允许进程 spawn，禁 Tauri；runner port 留 `core/orchestration`；
- **vcs 边界**：外部进程 + 版本控制工作面（git worktree 建域 / 脏仓探测 / 依赖 bootstrap 安装 / archive 相位 git 步（commit / rebase / ff_merge / commit_paths）/ 变更文件清单（`change_edit_set`，突变范围唯一入口）的进程执行）——建域 port（`WorktreePort`）落位 `crates/core/workflow`（port 属于消费者，spawn 不进 core），归档 git 步 port（`ArchiveVcsPort`）落位 `crates/core/orchestration`（walker 第三执行缝），实现落位 `crates/infra/vcs`（crate 裸名 `vcs-runtime`）；允许进程 spawn，禁 Tauri。

领域核心与应用编排位于各边界之内：core 各 crate 纯依赖；`store` 不依赖 core 行为（仅依赖 `agent` 纯类型作嵌装载荷与 `foundation` 身份段派生纯函数），`agent-runtime` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证。`infra/` 目录表达各边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent / checks / vcs 七类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: 检查步归类 checks 边界

- **WHEN** 评审 static-check、test-execution 与 mutation-test 执行步的落位
- **THEN** 三者归 checks 边界：core/checks 纯层零 spawn 零 Tauri，infra/checks 执行层允许 spawn；检查域成员不出现在 infra/agent

#### Scenario: git 工作面归类 vcs 边界

- **WHEN** 评审 git worktree 建域 / 脏仓探测 / bootstrap 安装 / archive 相位 git 步 / 变更文件清单的落位
- **THEN** 归 vcs 边界：core 侧 port（`WorktreePort` 在 core/workflow、`ArchiveVcsPort` 在 core/orchestration）零 spawn 零 Tauri，infra/vcs 执行层允许 spawn；vcs 成员不出现在 infra/agent 或 desktop-app 内联 spawn

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate（含 agent-runtime 与 checks、vcs 各 crate）的依赖声明
- **THEN** 无任何 Tauri 系依赖，shell 依赖仅存在于 desktop-app

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| 七类边界分类学（checks / vcs 注记扩展） | 落位先声明边界 | checks 成员注记增 mutation-test（`infra/checks/src/mutation/`）；vcs 职责注记增 `change_edit_set`（突变范围唯一入口）；其余六类注记不变 |
| `crates/infra/checks`（Component） | 检查域第三成员 | mutation-test 归 checks 边界（core/checks 纯层 + infra/checks 执行）；零 Tauri 不变 |
| `crates/infra/vcs`（Function） | 清单封装落位 | `change_edit_set(root, base)` 落 `git.rs`；spawn 留 vcs 执行层 |
