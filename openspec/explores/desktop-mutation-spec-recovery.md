# 突变测试规格回收：改写反馈边 + 归档回灌

workflow_type: requirement

> **一句话**：`desktop-mutation-filter-observability` 的清单 / 过滤 / 失败保留三条实现面已随
> desktop 0.5.7 落地，但 change 未归档、主 spec 未回灌；delta 的反馈边 Requirement 又已被
> 2026-10-19 的 `test-execution-zero-agent-decision` 淘汰。剩余工作 = 改写 delta Req 4 →
> 归档回灌主 spec →（可选）`change_edit_set` ensure 顺序硬化。
>
> **状态**：方案定稿，待执行（2026-10-22 整理）。
> **目标 change**：`openspec/changes/desktop-mutation-filter-observability/`（仅 `proposal.md` + delta，无 `workflow.json`，未归档）。
> **实现提交**：`505049418dc1`（desktop 0.5.7，2026-10-15）。

## 1. 背景

三条实现面（清单单点 + exclude、归桶过滤、失败段保留）已落地并在代码中可考；它们只存在于 delta，主 spec 仍是旧版表述。同时 delta 的第四条 Requirement「双时点反馈边」与现状双冲突：

- 代码：MutationTest 步 `on_fail = OnFail::Escalate`（`phase_table.rs`），失败即升格交决策、零会话；
- 主 spec：「失败升格交决策（报告指引式信息面）」已定稿；
- delta：仍写立即反馈 executor、`FeedbackLoop` 独立计数、「两反馈边并存」。

**原样归档会把已退役口径写回主 spec**，因此剩余工作是规格回收，不是实现。

| 待处理面 | 现状 | 处置 |
|---|---|---|
| delta Req 4 反馈边 | 与代码 / 主 spec 双冲突 | 改写（§2.1） |
| 主 spec Req 1–3 | delta 已携增量，主 spec 未回灌 | 归档 sync（§2.2） |
| proposal 元数据 | `draft`；版本写 0.5.5 → 0.5.6 | 对齐 0.5.7（§2.3） |
| `change_edit_set` 取数顺序 | 先取数、后 ensure | 可选硬化（§2.4） |

## 2. 方案

### 2.1 改写 delta Req 4（必做）

文件：`openspec/changes/desktop-mutation-filter-observability/specs/desktop-mutation-testing/spec.md`

**标题逐字对齐主 spec**：`### Requirement: 失败升格交决策（报告指引式信息面）`。归档 sync 按 Requirement 名匹配 MODIFIED；沿用旧名「双时点反馈边（报告指引式口径）」会被当作新增，主 spec 会出现两条互相矛盾的失败处置 Requirement。

正文收敛为单一口径：

- 失败一律升格相位 fail（零 WorkerAgent 会话），中断与未达标两时点同式；达标 pass 直接收口（机械 checklist 落账、零会话）。
- 中断 report = 中断错误摘要 + 已产报告路径清单 + 保留的失败段调试产物路径（保留面非空时）。错误摘要沿用 `ProcessOutput` 双段口径（错误行组 + 末 50 行；语义归 `desktop-exec-runtime`）。
- 未达标 report = score / threshold 结论摘要 + 原始报告路径清单。
- 反馈 MUST NOT 内联存活突变体清单——决策 agent 依 prompt 指引自行读段级 / 归并报告。
- 失败步 `on_fail = OnFail::Escalate`，MUST NOT 声明 `FeedbackLoop`；失败条目三会话槽位恒 `None`。
- **删除**：立即反馈 executor、`FeedbackLoop` 计数（label `"mutation-test"`，上限 5）、「两反馈边并存」表述及其全部 Scenario；testexec 同源 Scenario 不随本 Requirement 回灌。

### 2.2 归档并回灌主 spec（必做）

执行 `dev-team:openspec-archive-change desktop-mutation-filter-observability`，sync 提示处选「Sync now」。回灌增量：

| Requirement | 增量 |
|---|---|
| 突变范围清单封装（change_edit_set 单点） | 取数链路内幂等 ensure（两 pattern 写入 `.git/info/exclude`，linked worktree 共享 common-dir）+ 索引面残留撤出（`ls-files --cached` 探测、`reset -q --` 只撤索引）+ 三条 Scenario |
| 突变执行框架矩阵与映射表 | `suite.excludes` 全桶消费；Stryker 桶专属 allowlist（可插桩扩展名）+ 测试文件剔除（tool-scoped）；全桶滤空 `EmptyDiff` skip / 单桶滤空掉出 runnable；假绿防线 + 四条 Scenario |
| sandbox 跨段复用与清理时点 | 失败（Interrupted / Fatal）Stryker 桶临时面保留、反馈附路径；rust 桶零保留；成功照清 + 一条 Scenario |
| 失败升格交决策（报告指引式信息面） | §2.1 改写后的信息面（含保留产物路径） |

> 该 change 无 `workflow.json` / `tasks.md`，`change_list` 的 `workflow_done = false`，archive skill 会告警。确认继续即可；MUST NOT 手写 `workflow.json`。归档目录：`openspec/changes/archive/<执行日>-desktop-mutation-filter-observability/`。

### 2.3 proposal 元数据对齐（必做，归档前）

`openspec/changes/desktop-mutation-filter-observability/proposal.md`：

- 状态：`draft` → `已实现（desktop 0.5.7，2026-10-15）`
- 影响段版本：`0.5.5 → 0.5.6` → `0.5.5 → 0.5.7`

### 2.4 `change_edit_set` ensure 顺序硬化（可选，独立小改）

现状（`packages/desktop/src-tauri/crates/infra/vcs/src/git.rs:441`）：先 `git diff` / `git ls-files --others` 取数，后调 `ensure_mutation_artifact_exclude`。

```
现在：git diff ─┐
      ls-files ─┴─> ensure(exclude) ─> 合并返回   ← 首次调用时 exclude 尚未生效
改后：ensure(exclude) ─> git diff ─┐
                        ls-files ─┴─> 合并返回
```

**缺口**：升级后首次调用时，盘上遗留的 `_stryker-tmp/**` 可插桩源文件（`.ts` / `.js` 等）会进入 untracked 清单，成为 mutate 面。命中条件：workspace 存在 `root: "."` 的 suite，且盘上有旧版本 / 失败运行遗留的 sandbox。现有用例都是先显式 ensure 再取数，未覆盖生产首次调用顺序。

**修法**：把 ensure 提到两次 git 列举之前，并补一条用例——磁盘预置 `_stryker-tmp/legacy.ts` 与 `mutation-stryker-seg-1.conf.json`，首次 `change_edit_set` 即断言二者不入清单、exclude 两 pattern 已在位。

**价值**：让「取数链路内幂等确保」在首次调用也成立；成本低（一行顺序调整 + 一条用例），属内部修正，不 bump desktop 版本。

## 3. 执行顺序

```
1  改写 delta Req 4            （纯文本，先行——决定归档内容）
2  对齐 proposal 元数据        （归档前）
3  归档 + sync 回灌主 spec     （dev-team:openspec-archive-change）
4  （可选）ensure 顺序硬化 + 用例
5  勾掉 todo.md 的 cargo-mutants 条目
```

## 4. 改动清单

| 文件 | 改动 |
|---|---|
| `openspec/changes/desktop-mutation-filter-observability/specs/desktop-mutation-testing/spec.md` | Req 4 标题对齐 + 改写为升格信息面 |
| `openspec/changes/desktop-mutation-filter-observability/proposal.md` | 状态 / 版本 |
| `openspec/specs/desktop-mutation-testing/spec.md` | 归档 sync 回灌（§2.2 四行增量） |
| `openspec/changes/archive/<日期>-desktop-mutation-filter-observability/` | 归档移动（archive skill 执行） |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs`（可选） | ensure 前移 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs`（可选） | 残留 + 首次调用用例 |
| `openspec/todo.md` | 勾掉「rust 支持突变测试，使用 cargo-mutants」 |

## 5. 测试策略

| 面 | 验证 |
|---|---|
| 规格回收 | 无代码测试；按 §6 的 SHALL ↔ 符号逐条核对 |
| ensure 顺序（可选） | `cargo test -p vcs-runtime`（cwd `packages/desktop/src-tauri`）；新增「残留 + 首次调用」用例；既有 `change_edit_set` / ensure 用例不回归 |
| 归档 sync | 合并后主 spec 无重复 Requirement、既有 Scenario 未被误删；同一 delta 重复应用结果一致 |

## 6. 验收标准

1. delta Req 4 标题与主 spec 逐字一致；全文无 `FeedbackLoop` / 「立即反馈」/「两反馈边并存」残留。
2. 归档后主 spec 四条 Requirement 的每条 SHALL 都能在代码找到对应符号：
   - 清单：`change_edit_set` / `ensure_mutation_artifact_exclude` / `reset_staged_mutation_artifacts`
   - 框架：`bucket_suites` 三过滤 + `EmptyDiff` 空桶语义
   - 清理：失败桶 retain + `retained_artifacts_section`
   - 反馈：`OnFail::Escalate` + `step_mutation_test_upgraded_phase_log`
3. proposal 状态 / 版本与实际一致。
4. （若做硬化）残留 + 首次调用用例绿，清单不含 `_stryker-tmp` 内文件。
5. `openspec/todo.md` 条目已勾选。

## 7. 风险

| 风险 | 缓释 |
|---|---|
| sync 按名匹配，Req 4 标题不一致会被当新增 → 主 spec 两条失败处置 Requirement | 改写时标题逐字对齐；归档后复查 Requirement 清单 |
| change 无 `workflow.json`，archive 告警 | 确认继续；不手写 workflow.json |
| ensure 前移改变首次调用行为 | 只影响 untracked / 索引残留面；补用例锚定 |
| 失败段保留产物被归档提交收编 | 由 ensure 的 exclude 语义覆盖（已含在 Req 1 回灌条款） |

## 8. 参考

- 主 spec：`openspec/specs/desktop-mutation-testing/spec.md`
- delta：`openspec/changes/desktop-mutation-filter-observability/specs/desktop-mutation-testing/spec.md`
- 实现提交：`505049418dc1`（desktop 0.5.7）
- 淘汰来源：`openspec/changes/archive/2026-10-19-test-execution-zero-agent-decision/`
- 关键代码：`packages/desktop/src-tauri/crates/infra/vcs/src/git.rs:441`、`packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`
