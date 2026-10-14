# desktop-mutation-testing Specification (Delta)

## MODIFIED Requirements

### Requirement: 突变范围清单封装（change_edit_set 单点）

桌面突变范围 SHALL 以 change 编辑集为唯一权威：vcs 边界 SHALL 单点封装 `change_edit_set(root, base) -> Vec<String>`——跟踪改动取 `git diff <base> --name-only -z`（worktree 语义下 base=HEAD，恰为本 change 编辑集）、untracked 面经 `git ls-files --others --exclude-standard -z` 并入（新建源文件不 add 也入清单）、排序去重并 POSIX 归一后返回。该封装是**突变范围与文件清单的唯一入口**：`MutationRunner::run(root, name, files)` 收清单入参，清单获取经 vcs 缝注入（实现落 `crates/infra/vcs`，port 归消费者），checks-runtime 依赖面 MUST NOT 扩 vcs；全仓 MUST NOT 存在第二处取 diff / 反推突变文件的触点（CLI 侧清单反推逻辑不移植、不动）。清单取数链路内 SHALL 幂等确保 mutation 调试产物忽略规则（`_stryker-tmp/` 与 `mutation-stryker-seg-*.conf.json` 两 pattern）写入 workspace `.git/info/exclude`（经 `git rev-parse --git-path` 解析——linked worktree 与主仓共享 common-dir 一份；行终止符跟随既有内容、目录缺失补建），并 SHALL 撤出索引面残留（`git ls-files --cached` 探测——intent-to-add 条目不出 `diff --cached` 但 `add -A` / commit 会真正收录；`git reset -q --` 只撤索引、文件留盘）。ensure 失败显式 `Err`（保证优先：排除写不进就别让 mutation 继续跑）。

#### Scenario: 清单恰为编辑集含 untracked

- **WHEN** worktree 内一 change 编辑了 3 个已跟踪文件并新建 1 个未 add 的源文件，调用 `change_edit_set(root, HEAD)`
- **THEN** 返回恰为 4 个文件路径（跟踪 3 + untracked 1），字典序去重、POSIX 分隔符归一；无 net-zero 去噪、无 change 清单反推

#### Scenario: 唯一入口多消费者同源

- **WHEN** MutationRunner 与未来其他消费者（如 diff 摘要）各自需要变更文件清单
- **THEN** 两者均经 `change_edit_set` 取数（同源同口径）；infra/vcs 之外无 `git diff --name-only` / `ls-files --others` 清单拼装逻辑

#### Scenario: 取数链路幂等确保忽略规则

- **WHEN** `change_edit_set` 在调试产物忽略规则未在位（或仅部分在位）的 workspace 上取数
- **THEN** `.git/info/exclude` 被幂等补齐两条 pattern（已全在则零写盘）；后续 `ls-files --others --exclude-standard`、`status --porcelain`、`add -A` 三面均不再呈现调试产物

#### Scenario: 索引面残留撤出

- **WHEN** 修复前遗留的 intent-to-add / staged 调试产物在索引（`ls-files --cached` 可见）
- **THEN** ensure 后该条目被 `git reset -q --` 撤出索引、文件留盘；`add -A` 后 `diff --cached` 不含

### Requirement: 突变执行框架矩阵与映射表

突变执行 SHALL 按框架分派执行器：js 族（jest / vitest / vite-plus）经 Stryker（无 in-diff，`mutate` 面 = `change_edit_set` 清单经消费面过滤后的子集：`suite.excludes` 全桶消费——任何 suite 的排除 glob 命中即不入任何桶；Stryker 桶专属且 tool-scoped 的两规则——可插桩扩展名 allowlist（镜像 `@stryker-mutator/instrumenter` `getFormat` 注册表：`.js/.jsx/.mjs/.cjs/.ts/.mts/.cts/.tsx/.vue/.html/.htm/.svelte`）与测试文件剔除（路径含 `__tests__/` 或 basename 含 `.test.` / `.spec.`——测试文件是判定 oracle 非突变对象，CLI `deriveSourceFiles` 反向口径）；rust 桶不受扩展名 / 测试文件约束（cargo-mutants `--in-diff` 切片自滤））。过滤落点为归桶时刻（`bucket_suites`）：全桶滤空 SHALL 走 `EmptyDiff` skip 面（Pass、mutation 位 null、零 spawn——「mutate 清单非空但匹配 0 文件 → dry-run 0 mutant → score 100.0 静默假 Pass」路径 MUST NOT 可达）；单桶滤空 SHALL 掉出 runnable（该 suite 不执行）。rust 经 cargo-mutants（`--in-diff` 行级精度优先，untracked 面经 `git add -N` 补充；`--json` 输出；`--baseline skip`——TestExecution 绿跑已保证基线）。突变并发 SHALL 为一半 CPU 并配 per-mutant timeout 护栏。cargo-mutants outcome 映射 SHALL 定案为：`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`。无突变执行器的框架（node-test 等）SHALL 跳过该 suite 突变并记 evidence，MUST NOT Err、MUST NOT 静默无痕（NoExecutor 跳过面不受过滤规则约束——tool-scoped）。

#### Scenario: 不可插桩文件不入 mutate 面

- **WHEN** change 编辑集含 js suite 根下的 `package.json` / `*.md` / `*.css` / `*.yaml` / `*.png` 等白名单外扩展名文件
- **THEN** 该文件被归桶过滤剔除，Stryker 段 `mutate` 清单不含（插桩器 `Unable to parse … No parser registered` 崩溃路径不可达）

#### Scenario: 测试文件仅 Stryker 桶剔除

- **WHEN** 编辑集含 `a.test.ts` / `b.spec.tsx` / `__tests__/helper.ts` 落 vitest 桶，及 `tests/x.rs` 落 rust 桶
- **THEN** 前三者不入 mutate 面（判定 oracle 非突变对象）；rust 桶不受约束（`--in-diff` 自滤）

#### Scenario: excludes 全桶消费

- **WHEN** 任意 suite 的 `excludes` glob 命中编辑集内文件（无论该 suite 框架为何）
- **THEN** 该文件不入任何桶（CLI `isFileExcluded` 同口径）

#### Scenario: 全桶滤空假绿防线

- **WHEN** 某 js suite 的编辑集文件全被过滤（只碰了文档 / 配置 / 测试文件）
- **THEN** EmptyDiff skip（conclusion Pass、mutation 位 null、零 spawn）；MUST NOT 进段执行（dry-run 0 mutant → score 100.0 假 Pass 路径不可达）

#### Scenario: 无执行器框架跳过留痕

- **WHEN** tests[] 声明 node-test suite 且其文件入突变范围（含测试文件）
- **THEN** 该 suite 仍产 NoExecutor 跳过（过滤规则 tool-scoped 不作用于无执行器桶）

### Requirement: sandbox 跨段复用与清理时点

分段多轮执行 MUST 复用 sandbox，避免多轮初始化：Stryker 各段 SHALL 共用同一 `tempDirPath`；cargo-mutants 各段 SHALL 共享 workspace `CARGO_TARGET_DIR`。清理 SHALL 移到全部段完成后一次执行，且只清 `_stryker-tmp` 与临时 config 文件——段级原始报告与归并报告等报告工件 MUST NOT 被清理。**失败（Interrupted / Fatal）的 Stryker 桶临时面 SHALL 保留不清理**（保留粒度 = 失败桶全临时面——段 index 不上抛、sandbox 本跨段共享、清单有界且失败段 config 必在其中），反馈面附保留产物路径供单段复现；rust 桶失败零保留（无临时面）；保留产物由 workspace `.git/info/exclude` 保证 MUST NOT 进归档提交（见清单封装 Requirement 的 ensure 语义）。

#### Scenario: 共享 sandbox 只初始化一次

- **WHEN** 3 段 Stryker 顺序执行
- **THEN** `tempDirPath` 建置 / 依赖链接仅发生一次（段 2、3 复用），各段仅 mutate 面与临时 config 不同

#### Scenario: 成功清理不触报告工件

- **WHEN** 全部段完成后（含 Fail 结论的完整收束）执行清理
- **THEN** `_stryker-tmp` 与临时 config 被清、`mutation-seg-<i>.json` 与归并报告原样在位

#### Scenario: 失败段临时面保留供复现

- **WHEN** 某 Stryker 桶段执行中断（非零退出 / 无段报告 / 超时）或 Fatal 收束
- **THEN** 该桶 `mutation-stryker-seg-*.conf.json` 与 `_stryker-tmp/` 在盘不清理，反馈面附保留产物路径清单；其余已跑 Stryker 桶照常清理

### Requirement: 双时点反馈边（报告指引式口径）

MutationTest 失败 SHALL 按双时点反馈 executor 会话修复（补测试杀突变）：突变执行**中断**（段执行 error / harness 崩 / 超时）SHALL 立即反馈（不等汇总），内容 = 中断错误摘要 + 已产报告路径清单 +（保留面非空时）保留产物路径清单；全部段正常跑完 SHALL 最后汇总突变得分，**仅未达标**（score < threshold）才反馈，内容 = score / threshold 结论摘要 + 原始报告路径清单；达标 SHALL pass 直接收口（零反馈）。段失败 / 非零退出的错误摘要 SHALL 为「错误信息 + 输出末尾」双段形态（exec `ProcessOutput` 统一装配：错误信息 = 前 10 个错误行组、每组带至多 3 行续行、遇空行止；输出末尾 = 末尾 50 行行口径——旧「末 500 字符」单尾部口径会把 JS 错误首行截掉，退役）。反馈内容 MUST NOT 内联存活突变体清单——明细由 executor 自行读报告定位。本反馈边（反馈边②）为**报告指引式口径**，与 test-execution 反馈边①「有界内联自足」反向并存，MUST NOT 被「统一」掉。反馈 SHALL 走 `FeedbackLoop` 独立计数（label `"mutation-test"`，上限 5），超限 SHALL 升格相位 fail。

#### Scenario: 中断立即反馈（双段错误摘要 + 保留产物）

- **WHEN** 某 Stryker 段执行中 harness 崩溃（非零退出且未产出段报告）
- **THEN** 立即反馈 executor：错误摘要（错误信息段 + 输出末尾 50 行段——根因行不再被截掉）+ 已产报告路径清单 + 保留的失败段调试产物路径清单

#### Scenario: 非零退出错误面双段装配（testexec 同源）

- **WHEN** 测试命令非零退出且携错误输出
- **THEN** 错误面 = `测试命令非零退出（{code}）` + 错误信息段 + 输出末尾（50 行）段；空输出回落裸退出码措辞（空面无冒号缀语义保持）

#### Scenario: 未达标报告指引式反馈

- **WHEN** 全部段跑完且汇总 score 低于 threshold
- **THEN** 反馈含 score / threshold 结论摘要与原始报告路径清单，MUST NOT 内联存活突变体明细

#### Scenario: 达标零反馈收口

- **WHEN** 全部段跑完且逐 suite 全部达标
- **THEN** MutationTest pass，机械 checklist 落账推进，零反馈会话发起

#### Scenario: 两反馈边口径并存不被统一

- **WHEN** 审阅 test-execution 反馈边①与 MutationTest 反馈边②的 spec 表述
- **THEN** ①为有界内联自足口径、②为报告指引式口径，两口径显式并存留痕；任何「统一」两口径的改动须经用户重新裁决
