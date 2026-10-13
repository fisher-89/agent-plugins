# desktop-checks-domain Specification (Delta)

## ADDED Requirements

### Requirement: mutation 执行面与 MutationRunner port 归属

突变执行 SHALL 落 checks 边界：进程执行落位 `crates/infra/checks/src/mutation/`（testexec/ 旁新模块：分段调度、Stryker / cargo-mutants 模板展开 spawn、outcome 映射、段级与归并报告写盘、sandbox 复用与全段后一次清理），纯层判定（映射表口径、score 公式、段计数归并、阈值判定）落位 `crates/core/checks`（纯函数 fixtures 直驱可测，零 spawn 零 Tauri 红线不变）。`MutationRunner` port SHALL 留 `orchestration::port` 原地（port 归属消费者先例同式，与 `TestExecutionRunner` 并列；`ToolCommand::MutationTest` / `ToolStepOutput::MutationTest` 入封闭集）；`checks-runtime` 依赖面 MUST NOT 扩 vcs——变更文件清单（`change_edit_set`）由 walker 经 vcs 缝取数后以 `MutationRunner::run(root, name, files)` 入参注入，清单获取实现在 `infra/vcs`。突变报告 SHALL 落 change 报告目录 `reports/test/<plan>/mutation/` 子树（段级 `mutation-seg-<i>.json` + 归并块，全程留存），路径经 foundation layout 推导（命名隔离双禁令唯一触点纪律不变）。

#### Scenario: 双层落位与零 spawn 红线

- **WHEN** 审查 mutation 纯层（映射 / score / 归并）与执行模块（spawn / 分段 / 清理）的落位
- **THEN** 纯层在 core/checks（fixtures 直驱、零 spawn 零 Tauri），进程执行在 infra/checks/src/mutation/；core/checks 无 tokio process 依赖

#### Scenario: 清单经入参注入不扩依赖

- **WHEN** MutationRunner 执行需要变更文件清单
- **THEN** 清单由 `change_edit_set`（infra/vcs）产出、walker 经 vcs 缝取数后传入 `run(root, name, files)`；checks-runtime 的 Cargo.toml 无 vcs 依赖

#### Scenario: 报告落 change 报告树

- **WHEN** 突变分段执行完毕
- **THEN** 段级原始报告与归并块落 `reports/test/<plan>/mutation/` 子树（经 layout 推导零路径字面量），全程留存不清理

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/mutation/`（Component，新） | 突变进程执行 | 分段调度（静态初值 + 自适应）/ Stryker / cargo-mutants 模板展开 spawn + 超时 + 捕获 / outcome 映射 / 段级与归并报告写盘 / sandbox 复用（共享 tempDirPath / CARGO_TARGET_DIR）+ 全段后一次清理 |
| `crates/core/checks`（Function） | 纯层判定 | 映射表口径 / score 公式 `(killed+timeout)/(total-ignored-compileError-runtimeError)` / `aggregate_mutation` 段计数求和归并 / 逐 suite 阈值判定；fixtures 直驱 |
| `orchestration::port`（API） | MutationRunner port | `MutationRunner`（`run(root, name, files)`）与 `TestExecutionRunner` 并列留原地；`ToolCommand::MutationTest` / `ToolStepOutput::MutationTest` 入封闭集；core/orchestration 零依赖 checks |
| `crates/infra/vcs/src/git.rs`（Function） | 清单单点 | `change_edit_set(root, base)`（diff name-only + untracked 并入）；清单获取唯一入口 |
