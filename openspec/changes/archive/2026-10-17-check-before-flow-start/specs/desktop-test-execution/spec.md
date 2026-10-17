# desktop-test-execution Specification (Delta)

## ADDED Requirements

### Requirement: baseline 报告槽位与树隔离

initialize 相位（desktop-change-orchestration「initialize 基线检查相位执行体」）的 test-execution 基线检查 SHALL 与 run 内 test-execution 相位的报告**分树隔离**：基线检查的 summary 与逐 suite 子报告 SHALL 落该 change 目录的 `reports/baseline` 子树（路径经 foundation layout 的 baseline 变体单点推导，见 desktop-checks-domain「baseline 报告槽位参数化」），run 内 test-execution 相位报告仍恒落 `reports/test`（零改动）——两树零混居，MUST NOT 以 exec_id / 时间戳在同目录内区分。基线检查 MUST NOT 执行 mutation（initialize 步序列无 MutationTest 步，summary 的 `mutation` 位不适用）。

复用门 SHALL 按报告树各自判定：`reports/baseline` 树自身的 summary 可在输入未变时复用（clean 后输入 mtime 推进自动失效、语义正确）；run 内 test-execution 相位步 MUST NOT 复用 baseline 树的 summary（决策：不做跨树复用红利），`reports/test` 树的复用门语义零改动。static-check 基线执行的输出记录（exec-outputs）SHALL 同锚落 baseline 树；`read_exec_output` 回查面 SHALL 覆盖 baseline 树（exec_id 跨树唯一，查找策略由 design 定稿——槽位参数或跨树查找，回查 MUST 可达）。基线检查结论 fail / error 时 SHALL NOT 走「结论反馈边与独立预算」的反馈边（initialize 语义：Terminal 停止 + Error / Fail 分文案，见 desktop-change-orchestration）。

#### Scenario: 基线报告落 baseline 树

- **WHEN** initialize 相位的 test-execution 基线检查执行完毕（绿或红）
- **THEN** summary.json 与逐 suite 子报告、exec-outputs 落 `openspec/changes/<name>/reports/baseline/` 子树（layout 单点推导）；run 内 test-execution 相位后续执行时其报告仍落 `reports/test`，两树互不串扰

#### Scenario: 跨树零复用

- **WHEN** initialize 基线检查全绿（baseline 树 summary=pass）后 run 推进至 test-execution 相位
- **THEN** 相位步照常全量执行（或依 `reports/test` 树自身复用门判定），MUST NOT 复用 baseline 树的 summary 零 spawn 跳过

#### Scenario: baseline 树自身复用与失效

- **WHEN** initialize 基线检查 Terminal 失败后再发起（Clean 步恢复基线）重跑基线检查，与 worktree 输入未变即重跑两种情形
- **THEN** 前者（clean 触碰输入 mtime）复用门失效全量重跑；后者输入未变时复用 baseline 树上次 summary（零 spawn、结论一致）

#### Scenario: 基线回查可达

- **WHEN** 前端打开 initialize 相位 test-execution 节点的 stdout 面板（`outputRef` 回查）
- **THEN** `read_exec_output` 命中 baseline 树的 exec-outputs 并返回脚本真实输出，miss 仍为 `None` 非错误

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/testexec/mod.rs` | 报告槽位参数化消费 | 报告目录经槽位（test 缺省 / baseline）派生；复用门按各自报告树判定（baseline 树内 mtime 判定复用，跨树零复用）；聚合与诊断链零改动 |
| `crates/core/foundation/src/layout/mod.rs` | baseline 目录变体单点 | `reports/baseline` 后缀与 `reports/test` 同源派生（root / change 目录两变体）；零磁盘路径字面量扩散 |
| `crates/infra/exec/src/lib.rs` | 输出记录同锚 | `record_exec_output` / `read_exec_output` 按槽位锚定 exec-outputs；exec_id 跨树唯一，回查覆盖两树 |
