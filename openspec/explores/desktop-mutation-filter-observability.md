# 突变过滤与可观测性：落地核对与规格回收

workflow_type: requirement

> **一句话**：`desktop-mutation-filter-observability` 的三条实现面已随 desktop 0.5.7
> 落地，但 change 从未归档、主 spec 未回灌；第四条反馈边又被 2026-10-19 的
> `test-execution-zero-agent-decision` 淘汰。后续应做「改写 Req 4 + 归档回灌」的
> 规格回收，而不是继续实现。
>
> **状态**：核对完成（2026-10-21），结论已定，待执行。
> **实现提交**：`505049418dc1`（desktop 0.5.7，2026-10-15）；核对时版本 0.5.19。
> **淘汰来源**：`openspec/changes/archive/2026-10-19-test-execution-zero-agent-decision/`。

---

## 1. 结论先行

| 问题 | 结论 |
|---|---|
| change 是否与代码脱节 | **半脱节**：清单/过滤/保留三条实现面与代码一致；反馈边 Requirement 与代码、主 spec 双重冲突 |
| 主 spec 是否落后于代码 | **是**：三条实现面的细化条款从未回灌（主 spec 只有旧版表述） |
| 原样归档是否安全 | **否**：会把已退役的 `FeedbackLoop` 口径写回主 spec |
| 是否还有未实现的有价值设计 | change 内没有；唯一仍有代码价值的是 `change_edit_set` 的 ensure 顺序硬化（见 §4.2） |

## 2. 代码事实核对

### 2.1 清单单点 + 归档排除（已实现）

对应 delta 的「突变范围清单封装（change_edit_set 单点）」。

- `crates/infra/vcs/src/git.rs`
  - `change_edit_set`：`git diff <base> --name-only -z` ∪ `git ls-files --others
    --exclude-standard -z`，排序去重、POSIX 归一；
  - `ensure_mutation_artifact_exclude`：幂等写入 `.git/info/exclude`
    （`_stryker-tmp/`、`mutation-stryker-seg-*.conf.json`），跟随既有行终止符、
    目录缺失补建；
  - `reset_staged_mutation_artifacts`：`ls-files --cached` 探测后用 `git reset -q --`
    撤出索引、文件留盘。
- 测试：`ensure幂等_重复调用不重复追加`、`ensure兼容crlf与补目录`、
  `ensure后_调试产物三面全剔`、`ensure撤出intent_to_add残留`、
  `ensure_worktree写主仓共享exclude`（`infra/vcs/src/git_test.rs`）。
- 主 spec 缺口：`desktop-mutation-testing` 的「突变范围清单封装」只写清单单点，
  **没有** ensure/exclude/索引残留撤出条款。

### 2.2 mutate 过滤（已实现）

对应 delta 的「突变执行框架矩阵与映射表」。

- `crates/infra/checks/src/mutation/mod.rs`
  - `bucket_suites`：`checks::aggregate::is_file_excluded` 全桶消费；
  - Stryker 桶专属过滤：`STRYKER_INSTRUMENTABLE_EXTS` allowlist +
    `is_test_file`（`__tests__/`、`.test.`、`.spec.`），rust 桶不受约束；
  - 空桶语义：全桶滤空 → `EmptyDiff` skip（零 spawn、mutation 位 null）；
    单桶滤空 → 掉出 `runnable`；`NoExecutor` 跳过面不受过滤规则影响。
- 测试：`归桶过滤_可插桩扩展名与测试文件仅stryker桶剔除`、
  `全桶滤空_emptydiff零spawn假绿防线`（`mutation/mod_test.rs`）。
- 主 spec 缺口：框架矩阵 Requirement 只有基础分派，**没有**这三条过滤规则。

### 2.3 失败段临时面保留（已实现）

对应 delta 的「sandbox 跨段复用与清理时点」。

- `crates/infra/checks/src/mutation/mod.rs`
  - `execute` 中断 / Fatal 时把失败 Stryker 桶的 `mutation_dir` 记为 `retain`；
  - `cleanup_stryker_dirs` 只清理非保留桶，`retained_artifacts_section` 把保留
    路径追加到反馈文本；
  - rust 桶失败零保留（无临时面），成功收束照常清理。
- 测试：`中断保留失败段产物_反馈附路径`、`成功全段_临时产物照清`
  （`mutation/mod_test.rs`）。
- 主 spec 缺口：清理时点 Requirement 只写成功清理，**没有**失败保留条款。

### 2.4 反馈边（与代码、主 spec 冲突）

对应 delta 的「双时点反馈边（报告指引式口径）」。

delta 仍写：立即反馈 executor、`FeedbackLoop` 独立计数、与 test-execution
反馈边①并存。但当前事实是：

- 代码：MutationTest 步声明 `OnFail::Escalate`
  （`core/workflow/src/write/phase_table.rs`），执行体为
  `step_mutation_test_escalate`（`core/orchestration/src/walker.rs`），
  fail / error 两时点均升格交决策、零会话；
- 主 spec：「失败升格交决策（报告指引式信息面）」明确失败步
  `on_fail = OnFail::Escalate`、`MUST NOT 声明 FeedbackLoop`、三会话槽位恒 `None`；
- 来源：2026-10-19 归档的 `test-execution-zero-agent-decision`。

**原样归档会把主 spec 回退到 FeedbackLoop 旧口径，必须改写。**

### 2.5 ProcessOutput 统一错误面（已实现）

对应提案面 2。

- `crates/infra/exec/src/output.rs`：`ProcessOutput::{error_excerpt, tail,
  failure_message}`——错误行组（前 10 组 / 每组至多 3 行续行 / 遇空行止）+
  末尾 50 行行口径；
- 三消费方已迁移：`mutation/stryker.rs`、`mutation/cargo_mutants.rs`、
  `testexec/runner.rs`；
- 非目标保持未动：static-check（全文工件）、bootstrap（成功摘要）、agent bash。

### 2.6 元数据漂移

- `proposal.md` 状态仍为 `draft`；
- 版本写「0.5.5 → 0.5.6」，实际落地为 0.5.7（`505049418dc1`），核对时为 0.5.19；
- change 目录仍在 `openspec/changes/`，未归档。

## 3. 脱节矩阵

| Delta Requirement | 代码事实 | 主 spec 现状 | 处置 |
|---|---|---|---|
| 突变范围清单封装 | 已实现（单点 + ensure + 残留撤出） | 只有单点 | 归档回灌 |
| 框架矩阵与映射表 | 已实现（三过滤 + 空桶语义） | 只有基础矩阵 | 归档回灌 |
| sandbox 复用与清理时点 | 已实现（失败桶保留） | 只有成功清理 | 归档回灌 |
| 双时点反馈边 | 未实现且已废弃（Escalate） | 已是升格单口径 | **改写后归档** |

## 4. 仍有实现价值的设计

### 4.1 规格回收（最高价值）

1. **改写 Req 4**，只保留信息面：
   - 中断报告 = 中断错误摘要（`ProcessOutput` 双段）+ 已产报告路径 +
     保留的失败段调试产物路径；
   - 未达标报告 = score / threshold 结论摘要 + 原始报告路径；
   - MUST NOT 内联存活突变体清单；
   - 删除：立即反馈 executor、`FeedbackLoop` 计数/上限、「两反馈边并存」及其
     Scenario。
2. **归档 change**：把 Req 1–3 回灌主 spec，补上 §2.1–2.3 的三处缺口。
3. **更新 proposal 元数据**：状态从 `draft` 改为已实现/待归档，版本号对齐
   0.5.7 及后续实际版本。

验收口径：主 spec 三条 Requirement 的每条 SHALL 都能在代码里找到对应符号；
Req 4 与「失败升格交决策」Requirement 单口径一致，无 `FeedbackLoop` 残留。

### 4.2 可选硬化：`change_edit_set` 的 ensure 顺序

当前实现先取 `git diff` / `git ls-files`，再调用
`ensure_mutation_artifact_exclude`（`infra/vcs/src/git.rs`）。升级后首次调用时，
遗留 `_stryker-tmp/**` 中的可插桩源文件（`.ts` / `.js` 等）仍可能进入 mutate
清单——命中条件：workspace 存在 `root: "."` 的 suite，且盘上有旧版本 / 失败运行
遗留的 sandbox。现有测试都是先显式 ensure 再取数，未覆盖生产调用顺序。

修法：把 ensure 提到两次 git 列举之前，并补一条「残留 + 首次调用」测试。
价值：让 delta 场景「取数链路内幂等确保」在首次调用也成立；成本低。

### 4.3 明确不做

- 不实现 `FeedbackLoop` 机制；
- 不重新引入 MutationTest 相位内修复会话；
- 不做「统一两反馈边」的改动。

## 5. 建议执行顺序

1. 改写 change delta 的 Req 4（按 §4.1 信息面）；
2. 更新 proposal 状态 / 版本；
3. 归档 change，回灌主 spec；
4. （可选）提交 `change_edit_set` ensure 顺序硬化 + 测试；
5. 顺带勾掉 `openspec/todo.md` 的「rust 支持突变测试，使用 cargo-mutants」
   （代码早已实现，条目仍空白）。

## 6. 证据索引

- 实现提交：`505049418dc1`（0.5.7）；
- 淘汰来源：`openspec/changes/archive/2026-10-19-test-execution-zero-agent-decision/`；
- 主 spec：`openspec/specs/desktop-mutation-testing/spec.md`；
- Delta：`openspec/changes/desktop-mutation-filter-observability/specs/desktop-mutation-testing/spec.md`；
- 关键代码：`infra/vcs/src/git.rs`、`infra/checks/src/mutation/mod.rs`、
  `infra/exec/src/output.rs`、`core/workflow/src/write/phase_table.rs`、
  `core/orchestration/src/walker.rs`；
- 关键测试：`infra/vcs/src/git_test.rs`、`infra/checks/src/mutation/mod_test.rs`、
  `infra/exec/src/output_test.rs`。
