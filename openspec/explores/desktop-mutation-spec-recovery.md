# 突变测试规格回收：change 已删后的纯方案重规划

workflow_type: requirement

> **一句话**：`desktop-mutation-filter-observability` 已整目录删除（commit `a98a1efad5d5`，**未归档**），
> 其三条实现面早已随 desktop 0.5.7 落地，但主 spec 仍是旧版；剩余工作收敛为 **4 项规格补写 +
> 1 项可选代码硬化 + 1 项台账**，原方案里的「proposal 元数据对齐」「归档 sync」两项随 change
> 消失而**作废**。
>
> **状态**：重规划定稿（2026-10-22），待执行。
> **代码基线**：desktop 0.5.7 实现（`505049418dc1`）——本轮**零代码改动**（除可选项）。
> **delta 全文可恢复**：
> `git show a98a1efad5d5^:openspec/changes/desktop-mutation-filter-observability/specs/desktop-mutation-testing/spec.md`（104 行）
> `git show a98a1efad5d5^:openspec/changes/desktop-mutation-filter-observability/proposal.md`（30 行）

## 1. 现状盘点

| 资产 | 状态 |
|---|---|
| change 目录 `desktop-mutation-filter-observability/` | **已删除**（未归档，`openspec/changes/` 现仅剩 `archive/`）；删除提交 `a98a1efad5d5`，尚无同名 archive 目录 |
| 三条实现面（清单 exclude / 归桶过滤 / 失败保留） | **代码全部在位**：`git.rs:441`（`change_edit_set` + `ensure_mutation_artifact_exclude` + `reset_staged_mutation_artifacts`）、`mutation/mod.rs:88-115`（`STRYKER_INSTRUMENTABLE_EXTS` / `is_stryker_instrumentable` / `is_test_file`）、`mod.rs:307-361`（`bucket_suites` 三过滤）、`mod.rs:180-250` + `664`（`retain` + `retained_artifacts_section`） |
| 主 spec `openspec/specs/desktop-mutation-testing/spec.md` | 174 行 / 9 条 Requirement；**Req 1 清单封装、Req 2 框架矩阵、Req 5 sandbox 清理三条仍是 delta 之前旧版**，未见任何增量回灌 |
| 主 spec Req 6「失败升格交决策（报告指引式信息面）」 | 已由 `2026-10-19-test-execution-zero-agent-decision` 回灌，但**缺「保留的失败段调试产物路径」条款**（代码在，spec 无） |
| 主 spec `Purpose` 行 | 仍写「…sandbox 跨段复用、**双时点报告指引式反馈边**、段级复用门…」——反馈边已退役，表述陈旧 |
| 主 spec `Module Contract` 表 | 缺本次三条增量的符号行（`ensure_mutation_artifact_exclude` / `reset_staged_mutation_artifacts` / `bucket_suites` 过滤 / `retained_artifacts_section`） |
| `change_edit_set` ensure 时序 | 仍是**先取数后 ensure**（`git.rs:443-445`）；无「首次调用 + 盘上残留」用例（`git_test.rs` 无 `legacy.ts` 类夹具） |
| `openspec/todo.md:24` | `- [ ] rust支持突变测试，使用cargo-mutants` **未勾**（代码 `cargo_mutants.rs`、`config.json` src-tauri `mutation.score=70`、映射表均已就位） |
| `ProcessOutput` 详细口径 | 代码在（`infra/exec/src/output.rs:49/93/112`：`error_excerpt` 前 10 错误行组 + ≤3 续行、遇空行止；`tail` 末 50 行；`failure_message` 双段装配）；spec 侧 `desktop-exec-runtime` 仅有「语义零变」声明（§109-123），**无口径定义承载**——原先由被删 delta 的 Scenario「非零退出错误面双段装配」承载 |

## 2. 缺口图

```
代码（已落地，可考）                 主 spec（待补）
──────────────────                 ──────────────────────────────────
ensure .git/info/exclude ─┐
reset_staged 残留撤出    ─┼──▶  Req 1 清单封装 ── 缺 2 条款 + 2 Scenario
                                  Req 2 框架矩阵 ── 缺 3 规则 + 4 Scenario
bucket_suites 三过滤     ─┤        ├ excludes 全桶消费
  ├ excludes 全桶        ─┤        ├ Stryker allowlist（12 扩展名）
  ├ 可插桩 allowlist     ─┤        ├ 测试文件剔除（tool-scoped）
  └ 测试文件剔除         ─┤        └ 全桶滤空 EmptyDiff 假绿防线
EmptyDiff 假绿防线       ─┘
                                  Req 5 sandbox 清理 ── 缺失败桶保留条款 + 1 Scenario
失败桶 retain + 保留段   ────▶    Req 6 失败升格 ── 缺「保留产物路径」clause
retained_artifacts_section ─┘
                                  Purpose 行 ── 「双时点反馈边」待删
                                  Module Contract ── 缺 4 处符号
```

## 3. 待办清单（重规划结果）

| ID | 待办 | 性质 | 必要性 |
|---|---|---|---|
| **T1** | 主 spec 回灌三条 Req 增量（8 条 Scenario：ensure 幂等 / 索引残留撤出 / 不可插桩剔除 / 测试文件仅 Stryker / excludes 全桶 / 全桶滤空假绿防线 / 无执行器不受过滤 / 失败段保留供复现） | 规格缺口 | 必做 |
| **T2** | Req 6 补「中断 report 携保留的失败段调试产物路径（保留面非空时）」+ 中断 Scenario 同步 | 规格缺口 | 必做 |
| **T3** | Purpose 行「双时点报告指引式反馈边」→「失败升格交决策」 | 规格陈旧 | 必做 |
| **T4** | Module Contract 补 4 处符号行（ensure / reset_staged / bucket 过滤 / retained 段） | 规格陈旧 | 必做 |
| **T5** | `change_edit_set` 的 ensure 前移到两次 git 列举之前 + 「残留 + 首次调用」用例 | 代码硬化 | 可选（内部修正，不 bump 版本） |
| **T6** | `openspec/todo.md:24` 勾选 cargo-mutants 条目（勾前先实跑一次 rust 突变取证） | 台账 | 必做（确认后） |
| **T7** | `desktop-exec-runtime` 补 `ProcessOutput` 三方法口径定义（或把双段装配 Scenario 收回该 spec） | 规格缺口（弱） | 可选 |
| ~~X1~~ | ~~proposal 状态 / 版本对齐~~ | — | **作废**（文件已随 change 删除） |
| ~~X2~~ | ~~`openspec-archive-change` 归档 + sync~~ | — | **作废**（无 change 可归档；主 spec 直接补写） |

## 4. 方案

### 4.1 载体选择

| 方案 | 做法 | 权衡 |
|---|---|---|
| **A（推荐）** | 新建轻量 change `desktop-mutation-spec-recovery`：`proposal.md` 写「纯规格回收、零代码改动」，delta 携 4 条 MODIFIED（Req 1 / 2 / 5 / 6），执行 `dev-team:openspec-archive-change` 走 sync 回灌；Purpose 行与 Module Contract 在 sync 后随同一次提交补（delta 不承载这两段） | 保留「spec 变更经 change」审计链；成本 = 一份 30 行 proposal + 复用 git 历史 delta 全文 |
| **B** | 直接编辑主 spec 四段，不立 change | 零流程开销；但 104 行实质条款无审计记录，事后无从追「谁把这批条款写进主 spec」 |

**取 A**。理由：新增条款量大（3 条 Requirement 正文 + 8 Scenario），且 Req 6 的保留面与已退役反馈边条款在同一段落，直接改主 spec 容易与 `test-execution-zero-agent-decision` 的口径纠缠不清。

> 归档注意：MODIFIED 按 **Requirement 名逐字匹配**。四条标题必须与主 spec 现状逐字一致：
> `突变范围清单封装（change_edit_set 单点）` / `突变执行框架矩阵与映射表` / `sandbox 跨段复用与清理时点` /
> `失败升格交决策（报告指引式信息面）`。**MUST NOT** 沿用 delta 旧标题「双时点反馈边（报告指引式口径）」——那会被当新增，主 spec 出现两条互相矛盾的失败处置 Requirement（本 change 的 Req 6 已在主 spec 存在，正是本次教训）。

### 4.2 T1：三条增量（正文要点 + Scenario 名）

源文本直接取自 git 历史 delta 的前三条，**逐字沿用**，只需删掉两处已失效引用：

- Req 1 增量：取数链路内幂等 ensure 两 pattern 写入 `.git/info/exclude`（`git rev-parse --git-path` 解析、linked worktree 共享 common-dir、行终止符跟随既有内容、目录缺失补建、失败显式 `Err`）+ 索引面残留撤出（`ls-files --cached` 探测 intent-to-add、`reset -q --` 只撤索引）；Scenario：`取数链路幂等确保忽略规则`、`索引面残留撤出`。
- Req 2 增量：`mutate` 面 = 清单经消费面过滤后的子集——`suite.excludes` 全桶消费（`is_file_excluded`）、Stryker 桶 tool-scoped 两规则（12 扩展名 allowlist 镜像 `@stryker-mutator/instrumenter` `getFormat`、测试文件剔除 `__tests__/` + `.test.` / `.spec.`）、过滤落点 `bucket_suites`、全桶滤空 `EmptyDiff` skip（假绿防线：dry-run 0 mutant → score 100.0 路径 MUST NOT 可达）、单桶滤空掉出 runnable、NoExecutor 面不受过滤约束；Scenario：`不可插桩文件不入 mutate 面`、`测试文件仅 Stryker 桶剔除`、`excludes 全桶消费`、`全桶滤空假绿防线`，并保留既有 `无执行器框架跳过留痕`（口径改为「仍产 NoExecutor 跳过」）。
- Req 5 增量：失败（Interrupted / Fatal）的 Stryker 桶临时面保留不清理（粒度 = 失败桶全临时面，段 index 不上抛 + sandbox 跨段共享 + 清单有界）、rust 桶零保留、保留产物由 ensure 的 exclude 语义保证不入归档提交；Scenario：`失败段临时面保留供复现`；既有 Scenario 改名 `成功清理不触报告工件`（含 Fail 结论的完整收束照清）。

### 4.3 T2：Req 6 保留面条款

在主 spec Req 6 的中断分支补一句（与代码 `mod.rs:242` 对齐）：

> 中断 report = 中断错误摘要 + 已产报告路径清单 + **保留的失败段调试产物路径（保留面非空时）**；未达标 report = score / threshold 结论摘要 + 原始报告路径清单。错误摘要沿用 `ProcessOutput` 双段口径（错误行组 + 末 50 行；见 T7）。

中断 Scenario 的 THEN 同步补「（+ 保留产物路径清单）」，其余三条 Scenario 不动（达标 / 未达标 / 两口径并存红线）。

### 4.4 T3：Purpose 行

`…sandbox 跨段复用、双时点报告指引式反馈边、段级复用门…` → `…sandbox 跨段复用（失败桶临时面保留）、失败升格交决策、段级复用门…`。

### 4.5 T4：Module Contract

补四行：

| 模块 | 关键契约 |
|---|---|
| `crates/infra/vcs/src/git.rs` | `ensure_mutation_artifact_exclude(root)`（两 pattern 幂等写 `.git/info/exclude` + `reset_staged_mutation_artifacts`）、`MUTATION_EXCLUDE_PATTERNS` |
| `crates/infra/checks/src/mutation/mod.rs`（归桶） | `bucket_suites`：`is_file_excluded` 全桶 + `is_stryker_instrumentable` / `is_test_file`（tool-scoped）；全桶滤空 → `skip_outcome(MutationSkipReason::EmptyDiff)` |
| `crates/infra/checks/src/mutation/mod.rs`（保留面） | `retain` 只对 `MutationTool::Stryker` 失败桶置位；`retained_artifacts_section(&retained)` 拼入 `feedback_text` / `Err` |
| `crates/core/checks/src/aggregate.rs` | `is_file_excluded(path, suites)` 由消费面复用（excludes 全桶口径单点） |

### 4.6 T5（可选）：ensure 前移

```
现在：git diff ─┐
      ls-files ┴─> ensure(exclude) ─> 合并返回    ← 首次调用 exclude 尚未生效
改后：ensure(exclude) ─> git diff ─┐
                        ls-files ┴─> 合并返回
```

缺口命中条件：workspace 有 `root: "."` 类 suite（js 桶），且盘上有旧版本 / 早前失败运行遗留的 `_stryker-tmp/**` 可插桩源文件（`.ts` / `.js`）——升级后**首次** `change_edit_set` 会把它当 untracked 纳入 mutate 面。`change_edit_patch`（`git.rs:572`）在 set 之后调用，届时 exclude 已在位，无需改。

修法：把 `ensure_mutation_artifact_exclude(root)?` 提到两个 `git(...)` 之前；补一条用例——磁盘预置 `_stryker-tmp/legacy.ts` 与 `mutation-stryker-seg-1.conf.json`，首次 `change_edit_set` 断言二者不入清单、两 pattern 已在 `.git/info/exclude`。

### 4.7 T7（可选）：`ProcessOutput` 口径

`desktop-exec-runtime` 现只有「语义零变」声明（§109-123）。两条路：把双段装配的**定义**写进该 spec 的既有 Requirement（`error_excerpt` = 前 10 错误行组、每组 ≤3 续行、遇空行止；`tail` = 末 50 行；`failure_message(head)` = `head` + 双段，空输出逐字等于 `head`），或把原 delta 的 Scenario `非零退出错误面双段装配` 收回该 spec。避免「口径只活在被删的 delta 与代码注释里」。

## 5. 执行顺序

```
1  新建 change desktop-mutation-spec-recovery（proposal：纯规格回收，零代码）
2  delta 四条 MODIFIED：Req1/2/5 逐字复用 git 历史 + Req6 补保留面（T1/T2）
3  归档 + sync 回灌主 spec（dev-team:openspec-archive-change）
4  sync 后同提交补 Purpose 行 + Module Contract 四行（T3/T4）
5  （可选）ensure 前移 + 首次调用用例（T5）
6  （可选）desktop-exec-runtime 补 ProcessOutput 口径（T7）
7  实跑一次 rust 突变取证 → 勾 todo.md cargo-mutants 条目（T6）
```

## 6. 验收

1. 主 spec 每条新增 SHALL 都能定位到代码符号（§4.2-4.5 的符号列即清单）；Requirement 数仍为 9，Scenario 增加 8，无重名 Requirement。
2. 全文无 `FeedbackLoop` / 「立即反馈」/「两反馈边并存」残余；Purpose 行无「双时点」。
3. 同一 delta 重复应用结果一致（幂等 sync）。
4. （若做 T5）`cargo test -p vcs-runtime`（cwd `packages/desktop/src-tauri`）绿，首次调用用例断言清单不含 `_stryker-tmp` 内文件。
5. （若做 T7）`desktop-exec-runtime` 中 `error_excerpt` / `tail` / `failure_message` 三方法口径可读且与 `output.rs` 一致。
6. `todo.md:24` 已勾且附实跑证据（一次 rust suite 突变报告路径）。

## 7. 风险

| 风险 | 缓释 |
|---|---|
| MODIFIED 按名匹配，标题不一致 → 主 spec 出双胞胎 Requirement | 四条标题逐字对齐（§4.1 注）；sync 后复查 Requirement 清单 |
| 无 `workflow.json` / tasks 的 change 触发 archive 告警 | 确认继续；MUST NOT 手写 `workflow.json` |
| 方案 B（直改主 spec）或 sync 后手补段失去审计链 | 方案 A 下 Purpose / Module Contract 的补写与 change 同提交，提交信息注明回收批次 |
| ensure 前移改变首次调用行为 | 只影响 untracked / 索引残留面；补用例锚定 |
| 失败段保留产物被归档提交收编 | 由 ensure 的 exclude 语义覆盖（已含在 Req 1 回灌条款）；archive spec 的 `add -A` 尊重 ignore |

## 8. 参考

- 主 spec：`openspec/specs/desktop-mutation-testing/spec.md`
- delta / proposal 全文（历史）：`git show a98a1efad5d5^:openspec/changes/desktop-mutation-filter-observability/specs/desktop-mutation-testing/spec.md`、`…/proposal.md`
- 实现提交：`505049418dc1`（desktop 0.5.7）、删除提交 `a98a1efad5d5`（2026-10-22）
- 淘汰来源：`openspec/changes/archive/2026-10-19-test-execution-zero-agent-decision/`
- 关键代码：`infra/vcs/src/git.rs:441`、`infra/checks/src/mutation/mod.rs:88/180/307/664`、`infra/exec/src/output.rs:49/93/112`、`core/orchestration/src/steps.rs:272`、`core/orchestration/src/write/phase_table.rs`、`core/orchestration/src/walker.rs`
