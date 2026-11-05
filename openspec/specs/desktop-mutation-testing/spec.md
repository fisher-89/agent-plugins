# desktop-mutation-testing Specification

## Purpose

定义桌面突变测试（mutation testing）契约：以 change 编辑集（`change_edit_set`）为突变范围唯一单源，js 族 Stryker / rust cargo-mutants 分框架执行与 outcome 映射、score 口径与逐 suite 阈值判定、整体执行与耗时估算、sandbox 落位与清理（失败桶临时面保留）、失败升格交决策、桶级复用门与中断回读。

## Requirements

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

突变执行 SHALL 按框架分派执行器（每 suite 桶一次执行，不拆段）：js 族（jest / vitest / vite-plus）经 Stryker（无 in-diff，`mutate` 面 = `change_edit_set` 清单经消费面过滤后的子集：`suite.excludes` 全桶消费——任何 suite 的排除 glob 命中即不入任何桶；Stryker 桶专属且 tool-scoped 的两规则——可插桩扩展名 allowlist（镜像 `@stryker-mutator/instrumenter` `getFormat` 注册表：`.js/.jsx/.mjs/.cjs/.ts/.mts/.cts/.tsx/.vue/.html/.htm/.svelte`）与测试文件剔除（路径含 `__tests__/` 或 basename 含 `.test.` / `.spec.`——测试文件是判定 oracle 非突变对象，CLI `deriveSourceFiles` 反向口径）；rust 桶不受扩展名 / 测试文件约束（cargo-mutants `--in-diff` 切片自滤））。过滤落点为归桶时刻（`bucket_suites`）：全桶滤空 SHALL 走 `EmptyDiff` skip 面（Pass、mutation 位 null、零 spawn——「mutate 清单非空但匹配 0 文件 → dry-run 0 mutant → score 100.0 静默假 Pass」路径 MUST NOT 可达）；单桶滤空 SHALL 掉出 runnable（该 suite 不执行）。rust 经 cargo-mutants（`--in-diff` 行级精度优先，untracked 面经 `git add -N` 或 `--file` 补充，形态 design 定稿；`--json` 输出；`--baseline skip`——TestExecution 绿跑已保证基线）。突变并发 SHALL 分执行器折算并配 per-mutant timeout 护栏（防单突变体挂死，与执行超时编排正交）：rust 桶 cargo-mutants `--jobs = max(1, 可用并行度 / 4)`（16 线程 → 4），且每 worker 内测试线程数 SHALL 钉为 4（经 `--cargo-test-arg` 透传给 nextest 的 `-j`，该参数只进 Test 阶段、不污染 `--check`）——worker 数即「各持副本并冷编整包」的份数（实测单份 ~110 s），两层乘积须钉在核数附近；编译侧并发由 cargo-mutants 的 GNU jobserver 在所有子进程间全局封顶（默认 NCPUS），不随 worker 数叠乘。worker 副本（scratch）目录 SHALL 经 `TEMP` / `TMP` / `TMPDIR` 三键重定向到 `<suite 执行目录>/target/mutants-scratch`：系统盘临时区的进程启动成本是同源盘的 2~3 倍，nextest 逐用例起进程会把该差距放大成整包测试的数倍墙钟（实测同一份 archi 测试：系统临时目录 78 s、同源盘 29 s）；落在 `target/` 内是因为顶层 `target` 已被 cargo-mutants 排除复制（放别处会把 scratch 自身复制进每个 worker 副本）。目录建不成时 SHALL 静默不注入（回退系统临时目录，行为与未加重定向一致）。spawn 前 SHALL 连同 `mutants.out` 一并清场 `mutants.out.old`（cargo-mutants 起跑时若发现 `mutants.out` 已在位，会把整个旧目录改名到此而非删除，逐次运行无限堆积）。worker 副本 SHALL 经 `--gitignore true` 按 `.gitignore` 剪枝（cargo-mutants 默认不看 gitignore，会把未跟踪的构建产物一并复制：本仓库 `crates/target` 不剪枝时单份副本 246 MB / 962 文件，剪枝后 9 MB / 493 文件）；剪掉的内容是未跟踪 / 已忽略文件，构建所需全部在跟踪集内。任一层放开都会让测试进程互相饿死（实测纯函数突变体撞满 per-mutant 超时 → 假 timeout）。Stryker 桶 `concurrency` SHALL 独立取半 CPU——它只有一个进程、一份 sandbox，`concurrency` 是进程内运行器槽位，不存在副本冷编的乘法放大。cargo-mutants outcome 映射 SHALL 定案为：`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`。无突变执行器的框架（node-test 等）SHALL 跳过该 suite 突变并记 evidence，MUST NOT Err、MUST NOT 静默无痕（NoExecutor 跳过面不受过滤规则约束——tool-scoped）。

#### Scenario: cargo-mutants 映射表口径

- **WHEN** cargo-mutants 报告含 caught / missed / timeout / unviable / not-mutated 五类 outcome
- **THEN** 逐类映射为 `MutationMeasured` 的 killed / survived / timeout / compileError / ignored（unviable MUST NOT 映射 ignored——分母排除口径与 Stryker compileError 同语义）

#### Scenario: 不可插桩文件不入 mutate 面

- **WHEN** change 编辑集含 js suite 根下的 `package.json` / `*.md` / `*.css` / `*.yaml` / `*.png` 等白名单外扩展名文件
- **THEN** 该文件被归桶过滤剔除，Stryker 桶 `mutate` 清单不含（插桩器 `Unable to parse … No parser registered` 崩溃路径不可达）

#### Scenario: 测试文件仅 Stryker 桶剔除

- **WHEN** 编辑集含 `a.test.ts` / `b.spec.tsx` / `__tests__/helper.ts` 落 vitest 桶，及 `tests/x.rs` 落 rust 桶
- **THEN** 前三者不入 mutate 面（判定 oracle 非突变对象）；rust 桶不受约束（`--in-diff` 自滤）

#### Scenario: excludes 全桶消费

- **WHEN** 任意 suite 的 `excludes` glob 命中编辑集内文件（无论该 suite 框架为何）
- **THEN** 该文件不入任何桶（CLI `isFileExcluded` 同口径）

#### Scenario: 全桶滤空假绿防线

- **WHEN** 某 js suite 的编辑集文件全被过滤（只碰了文档 / 配置 / 测试文件）
- **THEN** EmptyDiff skip（conclusion Pass、mutation 位 null、零 spawn）；MUST NOT 进入执行（dry-run 0 mutant → score 100.0 假 Pass 路径不可达）

#### Scenario: 无执行器框架跳过留痕

- **WHEN** tests[] 声明 node-test suite 且其文件入突变范围（含测试文件）
- **THEN** 该 suite 仍产 NoExecutor 跳过（过滤规则 tool-scoped 不作用于无执行器桶）并记 evidence，runner 不 Err、不静默；其余 suite 突变照常执行

### Requirement: score 口径与逐 suite 阈值判定

突变得分 SHALL 沿既有公式：`score = (killed + timeout) / (total - ignored - compileError - runtimeError)`（分母排除 ignored + compileError + runtimeError；分母为 0 → 100.0），MUST NOT 引入百分比平均等新口径。阈值 SHALL 取 suite 配置 `mutation.score`（`openspec/config.json` 既有字段：desktop 80 / src-tauri 70）。门禁结论 SHALL 为逐 suite 判定、全过即 pass（任一 suite `score < threshold` 即 fail）。多 suite 计数归并 SHALL 沿 `aggregate_mutation` 既有计数求和公式（桶报告当子报告，MUST NOT 新增归并公式）；归并 MutationBlock SHALL 回填 `summary.json` 的 `mutation` 位（在位时），未跑 / diff 空跳过时位保持 null（null 合法观察期语义不变，见 desktop-test-execution）。

#### Scenario: 公式与分母排除

- **WHEN** 某 suite 突变计数为 killed=80 / survived=10 / timeout=5 / ignored=3 / compileError=2 / runtimeError=0 / total=100
- **THEN** score = (80+5)/(100-3-2-0) = 89.47（两位舍入），与既有 `aggregate_mutation` 公式逐字一致

#### Scenario: 逐 suite 判定全过即 pass

- **WHEN** 一次 MutationTest 覆盖 desktop（阈值 80）与 src-tauri（阈值 70）两 suite，得分 82 / 68
- **THEN** 门禁结论 fail（src-tauri 未达标），反馈携未达标 suite 的 score / threshold 与报告路径清单

### Requirement: 整体执行与耗时估算

MutationTest 对每个 suite 桶 SHALL 以**单次 spawn 整体执行**该桶的全部突变面，MUST NOT 再按「段预算」拆段顺序执行。理由是可量化的：每次 spawn 有约 210 s 的固定开销（源码树复制 + 首个编译单元冷编），与突变体数无关；实测 25 段形态下该开销占总墙钟 45%，2 段形态下仍有 6%。整体执行 + 中断回读（见「执行中断与部分成果回读」）以更小的总墙钟获得同等的失败爆炸半径。

执行超时 SHALL 按估算耗时折算，MUST NOT 使用固定常数：`超时 = max(SEGMENT_SPAWN_TIMEOUT_MS, 估算耗时 × TIMEOUT_SAFETY_FACTOR)`，其中估算耗时 = 突变体数 × `SEC_PER_MUTANT`、安全倍数取 2。固定值 `SEGMENT_SPAWN_TIMEOUT_MS`（3600 s）降为**下限**并同时充当小规模执行的冷启动余量——小规模下估算低于下限，任何项目跑一小时内不超时，故估算式 MUST NOT 含按仓库校准的固定开销项。

突变体数 SHALL 优先取 `cargo mutants --list` 的精确枚举值（`--workspace --in-diff <本次补丁> --json`，实测 1.5 s、零编译）；不可用时按「文件数 × `EST_MUTANTS_PER_FILE`」兜底。分母口径 MUST 是该桶内文件数，MUST NOT 是「实际产出突变体的文件数」（后者只数有产出的文件，会把密度虚高数倍）。

Stryker 桶的突变数 SHALL 按「文件总行数 × `MUTANTS_PER_LINE`」估算（Stryker 无 `--list` 等价物）；其超时 SHALL 按「单轮测试耗时 × (1 + 突变体数 × `PER_MUTANT_TEST_RATIO`) × 2」折算，单轮测试耗时取同 plan 的 test-execution 报告 `duration_ms`，取不到时退回下限。Stryker **没有编译阶段**，故不存在「每突变体撞满 per-mutant 超时」那种可折算的最坏上界。

rust 桶 SHALL 以 `--profile mutants` 构建（根 `Cargo.toml` 的 `[profile.mutants]`：`inherits = "test"` + `debug = "none"`——突变体预期「编译通过、测试失败」，符号价值远不及编译速度；cargo 不会因定义 profile 就自动采用，必须显式选中；实测全 workspace 冷编 -25%、target -36%，且测试提速后原先被 per-mutant 超时误伤的长用例回归正常判定）。

每次执行 SHALL 产出原始报告 `mutation-seg-0.json`（原始 Stryker / cargo-mutants 报告全程留存 `reports/test/<plan>/mutation/` 子树，MUST NOT 清理），归并出总块（沿 `aggregate_mutation`）。文件名保留 `seg-0` 前缀而不改为无语义名，是因为既有报告树与归档产物的读取面按该形态定位；单桶恒只有一个文件，`<i>` 恒为 0。

#### Scenario: 单次 spawn 跑完整桶

- **WHEN** 某 suite 桶含 14 个源文件、估算 300 个突变体
- **THEN** 该桶只 spawn 一次 cargo-mutants；`mutation-seg-0.json` 与归并块各一份，无 seg-1 及以后

#### Scenario: 耗时估算只含线性项

- **WHEN** 审查耗时估算式
- **THEN** 估算 = 突变体数 × `SEC_PER_MUTANT`，不含任何按仓库校准的固定开销常数；小规模执行由 `SEGMENT_SPAWN_TIMEOUT_MS` 下限兜住冷启动

#### Scenario: 超时按估算折算

- **WHEN** 突变体数取精确值 739、`SEC_PER_MUTANT` = 10
- **THEN** 生效超时 = 739 × 10 × 2 = 14780 s，MUST NOT 是固定 3600 s

#### Scenario: 突变数取精确枚举

- **WHEN** `cargo mutants --list` 可用
- **THEN** 突变体数取该命令在该桶补丁上的枚举条数，MUST NOT 用密度估算；命令不可用时才回落「文件数 × `EST_MUTANTS_PER_FILE`」

### Requirement: sandbox 落位与清理时点

Stryker 桶 SHALL 为一次执行建一份 sandbox（`tempDirName`），执行结束后统一清理；cargo-mutants 桶 SHALL 以**相对路径** `target` 注入 `CARGO_TARGET_DIR`——相对值按各子进程自己的 cwd 解析，而 `--jobs > 1` 时各 worker 各持一份独立临时副本，故天然各写各的 target；相对值同时覆盖父进程可能全局导出的绝对值。共享同一绝对 target dir 是错误的：rustc 的 `-C metadata` 不含源码路径与内容，各副本会撞同一 fingerprint / 产物名，实测表现为 `Fresh` 复用别的副本的二进制（源码已变却不重编 → 假 caught / 假 missed）与并发链接争用（`LNK1104` → 假 unviable）。

每次 spawn 前 SHALL 清场 `mutants.out` 与 `mutants.out.old` 两个目录（陈旧报告不得被误读；轮换目录不堆积）。清理 SHALL 在执行完成后一次执行，且只清 `_stryker-tmp` 与临时 config 文件——原始报告与归并报告等报告工件 MUST NOT 被清理。**失败（Interrupted / Fatal / Cancelled）的 Stryker 桶临时面 SHALL 保留不清理**，失败面（升格 report / Err）附保留产物路径供复现；rust 桶失败零保留（无临时面）；保留产物由 workspace `.git/info/exclude` 保证 MUST NOT 进归档提交（见清单封装 Requirement 的 ensure 语义）。

#### Scenario: rust 桶 scratch 落同源盘

- **WHEN** rust 桶 spawn cargo-mutants（系统临时目录可能在另一块盘）
- **THEN** 子进程看到的 `TEMP` / `TMP` / `TMPDIR` 均为 `<suite 执行目录>/target/mutants-scratch`（worker 副本与源码同盘，且该目录不在被复制的源树范围内）；目录不可建时三键不注入，行为回退到系统临时目录

#### Scenario: rust 桶不共享 target dir

- **WHEN** rust 桶以 `--jobs > 1` 执行（父进程环境可能已全局设绝对 `CARGO_TARGET_DIR`）
- **THEN** 子进程看到的是相对 `target`（绝对值被覆盖），按各自副本 cwd 解析为各 worker 各自的 target；不同副本不互相复用编译产物，不出现因产物互串产生的假 caught / 假 missed 或 `LNK1104` 假 unviable

#### Scenario: scratch 副本内 Tauri 版本可解析

- **WHEN** suite 根为 `src-tauri` 且 `tauri.conf.json` 的 `version` 为相对路径（`../package.json`），cargo-mutants 把源码复制到临时目录后构建 `dev-team` 包
- **THEN** 执行经 `TAURI_CONFIG` 注入从源树解析出的字面版本，`tauri_build` 不再因副本缺上级 `package.json` 而 panic；该包的突变体正常进入分母（不因整包 unviable 变成 score 恒 100 假绿）

#### Scenario: 成功清理不触报告工件

- **WHEN** 一次执行完成（含 Fail 结论的完整收束）后执行清理
- **THEN** `_stryker-tmp` 与临时 config 被清、`mutation-seg-0.json` 与归并报告原样在位

#### Scenario: 失败临时面保留供复现

- **WHEN** 某 Stryker 桶执行中断（非零退出 / 无报告 / 超时）或 Fatal 收束
- **THEN** 该桶 `mutation-stryker-seg-0.conf.json` 与 `_stryker-tmp/` 在盘不清理，失败面（升格 report / Err）附保留产物路径清单；其余已跑 Stryker 桶照常清理

### Requirement: 失败升格交决策（报告指引式信息面）

MutationTest 失败 SHALL 一律升格相位 fail（零 WorkerAgent 会话，不注入修复、不续注会话），两时点同式：突变执行**中断**（执行 error / harness 崩 / 超时）与执行正常跑完但**未达标**（score < threshold）均以固定三步收场——代写一条 fail checklist → `phase_log` 落账 → 截断本相位余下步（`StepFlow::Upgraded`）回 `phase_next` 进决策分叉；达标 SHALL pass 直接收口（机械 checklist 落账、零会话）。升格落账的 `report` SHALL 携完整反馈文本（`feedback_text`，上限 `MAX_REPORT_CHARS = 2000` 字符）：中断时为中断错误摘要 + 已产报告路径清单 + 保留的失败调试产物路径（保留面非空时），未达标时为 score / threshold 结论摘要 + 原始报告路径清单；错误摘要沿用 `ProcessOutput` 双段落口径（错误行组 + 末 50 行；见 desktop-exec-runtime）；evidence SHALL 携诊断摘要（`diagnose_brief` 口径）+ 报告目录。反馈内容 MUST NOT 内联存活突变体清单——决策 agent SHALL 依决策 prompt 的指引自行 `read` 原始 / 归并报告定位（报告指引式取向由决策信息面承继）。失败步 `on_fail` SHALL 为 `OnFail::Escalate`（与 test-execution 两步同），MUST NOT 声明 `FeedbackLoop`；失败条目三会话槽位恒 `None`。`OnFail::Escalate` 的归属守卫 SHALL 保证其只出现在 TestExecution / MutationTest 步上。

#### Scenario: 中断立即升格

- **WHEN** 执行中 harness 崩溃（此前已有桶产出报告）
- **THEN** 立即升格相位 fail（不等其余桶跑完）：零 WorkerAgent 会话、一条 fail PhaseRecord（report 携中断错误摘要 + 已产报告路径清单 + 保留的失败调试产物路径清单（保留面非空时）），回 `phase_next` 进决策分叉

#### Scenario: 未达标升格

- **WHEN** 全部桶跑完且汇总 score 低于 threshold
- **THEN** 升格相位 fail：report 携 score / threshold 结论摘要与原始报告路径清单，MUST NOT 内联存活突变体明细；零会话请求

#### Scenario: 达标零会话收口

- **WHEN** 全部桶跑完且逐 suite 全部达标（或突变面为空跳过）
- **THEN** MutationTest pass，机械 checklist 落账进入 PhaseLog，推进下一相位，零 WorkerAgent 会话发起

#### Scenario: 两口径并存红线随反馈边退役

- **WHEN** 审阅 test-execution 与 MutationTest 的失败处置表述
- **THEN** 两处均为「升格交决策」单一口径，无「有界内联自足 vs 报告指引式反向并存」红线（两反馈边均已不存在）；如未来重引相位内修复会话，须经用户重新裁决并显式提出 spec 变更

### Requirement: 整体复用与执行中断的部分成果回读

MutationTest SHALL 以**桶级**复用门保证幂等：原始报告 `mutation-seg-0.json` 在位且输入（突变面源文件 + 测试文件）mtime 未变 SHALL 复用该桶结果（零 spawn）；任一输入 mtime 变化 SHALL 使该桶失效重跑。整体执行没有中途断点，故复用粒度只能到桶——这是对「重跑必复现」的正当取舍，半成品由下一条的回读承担。复用判定 MUST NOT 演变为反复全量跑。

执行被超时 / 取消 / 崩溃终止时，已判完的突变体 MUST NOT 随结果一并丢弃：cargo-mutants 边跑边往 `mutants.out/` 下的 `caught.txt` / `missed.txt` / `timeout.txt` / `unviable.txt` 追加「一行一突变体」，而 `outcomes.json` 只在正常退出时落盘。故终止时 SHALL 回读这四个增量文件、以行数合成计数面，并把该结果标记为**部分成果**（进程输出留痕「中断回读」）。**四个文件全缺或全空时 MUST NOT 合成零计数载荷**——那是「一件都没判完」，与「全被 caught」语义相反，必须走原中断面。

Stryker 无相应的增量成果文件（报告只在正常退出时由 jsonReporter 写出），故其终止面只有中断一条路径。

#### Scenario: 输入未变零 spawn 复用

- **WHEN** 重入同一 change 的 MutationTest 且输入文件 mtime 未变
- **THEN** 该桶复用既有 `mutation-seg-0.json`（零 spawn），归并块由该报告派生

#### Scenario: 输入变化桶失效

- **WHEN** 上次突变后某源文件或测试文件 mtime 更新
- **THEN** 该桶失效重跑（MUST NOT 复用陈旧报告）

#### Scenario: 超时后回读部分成果

- **WHEN** cargo-mutants 执行撞硬超时被强杀，此时 `caught.txt` 有 2 行、`missed.txt` 有 1 行
- **THEN** 回读合成计数面 `{caught: 2, missed: 1, total_mutants: 3}`，结果为**部分成果**（非中断面），归并块与阈值判定照常进行

#### Scenario: 零成果不伪装为跑完

- **WHEN** 执行在第一个突变体判定前就被终止（四个增量文件全缺或全空）
- **THEN** MUST NOT 合成零计数载荷（那会被 score 公式算成 100 分假绿），一律走原中断面（`Interrupted` / `Cancelled`）

### Requirement: 突变常量参数与 config schema 不扩展

突变执行参数（jobs / per-mutant timeout / 单突变体墙钟速率 / 单文件突变密度 / 超时安全倍数 / 执行超时下限）SHALL 以代码常量观察落地，MUST NOT 进 `suite.mutation` 字段、MUST NOT 扩展 config schema；`suite.mutation` SHALL 维持既有 `{ cwd?, score }` 形态。参数调整需求经常量变更走代码评审，观察期后若需用户可配再另立 schema 变更。

#### Scenario: schema 零扩展

- **WHEN** 审查 `openspec/config.json` schema 与 `MutationConfig` 结构
- **THEN** `suite.mutation` 仍为 `{ cwd?, score }`，无 jobs / timeout / 估算速率字段；对应常量在 mutation 执行模块内可考

#### Scenario: 可迁移常数与需校准常数分列

- **WHEN** 审查 mutation 执行模块的估算常数
- **THEN** 需按仓库校准的只有 `SEC_PER_MUTANT`（注释给出重标方法：真实 run 后取 `outcomes.json` 的 `Σ(build + test) / 突变体数`）与兜底用的 `EST_MUTANTS_PER_FILE`；`MUTANTS_PER_LINE` / `PER_MUTANT_TEST_RATIO` 标注为业界通用值，MUST NOT 要求按仓库重标；耗时估算式 MUST NOT 含按仓库校准的固定开销秒数

### Requirement: 执行取消与进程树击杀

突变执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并在既有 `SegmentFailure` 二分（`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 → runner `Err` 停给用户；`Interrupted` = 执行中断 → `conclusion = Error` 即时反馈）之上保留第三变体 `Cancelled`：

- `SegmentFailure::Cancelled` SHALL 由 `RunOutcome::Cancelled` 映射产生，并沿 `execute → run_suite → execute_suite` 三层贯穿；`Display` 形态与另两变体同型（单串）。
- 取消 SHALL NOT 走 `Fatal`（那会把用户停止呈现为「环境坏了停给用户」）；取消的收敛口径 SHALL 为**执行面归因**（本变更改判，取代既有「中断 + `conclusion = Error`」过渡形态）：runner SHALL 返回 `MutationTestOutcome { fault: Some(RunFault::Stopped), … }`，MUST NOT 以 `conclusion = Error` + 取消文案的升格面收场，MUST NOT 代写 fail checklist——由 walker 以 `Terminal::stopped` 收敛 run `stopped`（见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」）。
- **突变面不接 retry**：每次 spawn SHALL 以 `RetryPolicy::none()` 语义执行（突变长跑是产品语义，重试会放大墙钟；幂等已由桶级复用门承担）。瞬态签名命中 MAY 记入诊断文本，MUST NOT 触发重试。
- **执行超时不进 fault**：超时预算 SHALL 按「整体执行与耗时估算」折算（`SEGMENT_SPAWN_TIMEOUT_MS` 为下限），超时臂 SHALL 为 `RunOutcome::TimedOut` + `exec::kill_tree` 整树回收（此前只杀壳，`cargo mutants` 的 rustc 子孙继续写各自的 target 目录——与两检查域同一棵树）；回读不到部分成果时 `TimedOut` 映射 `SegmentFailure::Interrupted`、`conclusion = Error`、`fault = None`；超时文案 SHALL 携**实际生效预算值**（MUST NOT 写死某个秒数）——突变超时属配置问题而非外部环境占用，MUST NOT 与两检查域共用环境错误口径。
- **超时 / 取消时先试回读**：超时或取消臂 SHALL 先按「整体复用与执行中断的部分成果回读」合成部分成果；回读成功则以部分成果正常收束（不产 `Interrupted` / `Cancelled`），回读不到才走原变体。
- 失败桶临时面保留纪律（失败 / 中断的 Stryker 桶临时 config 与 `_stryker-tmp` 不清理、成功照清）SHALL 对取消面同样适用。

#### Scenario: 取消映射为 fault=Stopped

- **WHEN** 对正在执行的 mutation 置位 cancel（用户停止 run），且无部分成果可回读
- **THEN** 执行体返回 `Cancelled`，runner 以 `fault = Some(Stopped)` 正常返回（MUST NOT 返回 `Err`、MUST NOT 产 `conclusion = Error` 的升格面）；walker 把该步节点收敛 `stopped`、run 收敛 `stopped`

#### Scenario: 执行超时进程树消失

- **WHEN** 某桶超过生效预算未退出（rustc 子孙仍在写 target），且无部分成果可回读
- **THEN** 执行体 `kill_tree` 整树回收（Windows 上壳进程与 rustc 子孙的 pid 均不可再观测），以 `Interrupted` 收场、`fault == None`、文案携实际生效预算值

#### Scenario: 突变面零重试

- **WHEN** 执行命令输出中含瞬态签名文本（如 `os error 32`）并退出非零
- **THEN** 该次执行 `attempts == 1`（零重试），`SegmentFailure` 分流按既有面（`Interrupted`）收场；MUST NOT 因签名命中而追加尝试

#### Scenario: 既有分流不变

- **WHEN** 工具不可达 / 临时 config 写盘失败 / 报告缺失等既有失败面被触发
- **THEN** 仍分别映射 `Fatal` / `Interrupted`（文案骨架与呈现口径零变），`Cancelled` 变体不参与既有面

#### Scenario: 取消后临时面保留

- **WHEN** Stryker 桶因取消（`Cancelled`）中断
- **THEN** 该桶临时 config 与 `_stryker-tmp` 按失败面保留（反馈面附路径供复现），成功桶照常清理

#### Scenario: 复用门不受影响

- **WHEN** 取消后重入同一 change 的 MutationTest（输入未变）
- **THEN** 已完成的桶仍走桶级复用门（零 spawn），未完成的桶重跑；报告与归并块口径零变

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/workflow/src/write/phase_table.rs` | 步声明 | `TEST_EXECUTION_STEPS` 的 MutationTest 步 `on_fail = Some(OnFail::Escalate)` |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test`） | 门禁步执行体 | 删反馈循环与 `fix_session`；`Escalate` 前置分支（fail / error 均升格）；保留 runner `Err → Terminal` |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_upgraded_phase_log`） | 升格落账 | report = `feedback_text`（≤2000）、evidence = `diagnose_brief` + 报告目录、三会话槽位 None |
| `crates/core/orchestration/src/port.rs` | 载荷语义（字段面不变） | `MutationTestOutcome.feedback_text` 消费者由「反馈边修复 prompt」改为「升格 report」 |
| `crates/core/workflow/src/write/phase_table_test.rs` | 声明守卫 | `Escalate` 仅允许出现在 TestExecution / MutationTest 步的断言 |
| `crates/infra/vcs/src/git.rs` | 清单取数与合入排除面 | `ensure_mutation_artifact_exclude(root)`（两 pattern 幂等写 `.git/info/exclude`）+ `reset_staged_mutation_artifacts`（索引残留撤出）、`MUTATION_EXCLUDE_PATTERNS` |
| `crates/infra/checks/src/mutation/mod.rs` | 整体执行与失败面 | `enum SegmentFailure { Fatal(String), Interrupted(String), Cancelled(String) }`（`Display` 同型）；`execute(root, change, files, patch, cancel)` / `run_suite(..., cancel)` / `execute_suite(..., cancel)` 贯穿 cancel；`execute_suite` 返回 `(MutationSegReport, bool)`，布尔位标记「中断回读的部分成果」；`Cancelled` → `fault = Some(Stopped)`（不产 `conclusion = Error` 升格面）、`Interrupted` → `conclusion = Error` + `fault = None`、`Fatal` → `Err` |
| `crates/infra/checks/src/mutation/segments.rs` | 耗时估算单点 | `SEC_PER_MUTANT`（需按仓库校准）/ `TIMEOUT_SAFETY_FACTOR`（2.0）/ `EST_MUTANTS_PER_FILE`（兜底密度）；`estimate_duration_sec(mutants) = mutants × SEC_PER_MUTANT`（**不含固定开销项**）、`estimate_mutants(file_count)`、`run_spawn_timeout_ms(mutants)` |
| `crates/infra/checks/src/mutation/mod.rs`（归桶） | mutate 面过滤 | `bucket_suites`：`is_file_excluded` 全桶 + `is_stryker_instrumentable` / `is_test_file`（tool-scoped）；全桶滤空 → `skip_outcome(MutationSkipReason::EmptyDiff)` |
| `crates/infra/checks/src/mutation/mod.rs`（保留面） | 失败桶临时面保留 | `retain` 只对 `MutationTool::Stryker` 失败桶置位；`retained_artifacts_section(&retained)` 拼入 `feedback_text` / `Err` |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 整体执行（rust） | `run_suite(patch_path, exec_dir, mutant_count, cancel) -> (String, bool)` 经 `run_shell`（`CARGO_TARGET_DIR=target` 相对注入 + `TEMP`/`TMP`/`TMPDIR` 重定向到 `target/mutants-scratch` + `--profile mutants` + `--gitignore true` + `TAURI_CONFIG` 版本覆盖、超时按 `segments::run_spawn_timeout_ms`、`RetryPolicy::none()`）；`recover_partial(exec_dir)` 从 `caught.txt`/`missed.txt`/`timeout.txt`/`unviable.txt` 行数合成计数面（全空 → `None`）；`TimedOut` / `Cancelled` / 崩溃臂先试回读，回读不到才映射 `Interrupted` / `Cancelled` |
| `crates/infra/checks/src/mutation/stryker.rs` | 整体执行（js 族） | `run_suite(segment, round_duration_ms, cancel) -> (String, bool)` 经 `run_shell`（`cwd = suite_cwd`、超时按 `estimate_timeout_ms(突变数, 单轮测试耗时)`、`RetryPolicy::none()`）；突变数 = 文件总行数 × `MUTANTS_PER_LINE`（业界通用值）；`PER_MUTANT_TEST_RATIO` = 0.1（业界通用值）；无增量成果文件，终止面只有中断一条路径（布尔位恒 `false`） |
| `crates/core/orchestration/src/port.rs` | 载荷 | `MutationTestOutcome { conclusion, skip, score, threshold, counts, report_paths, feedback_text, fault: Option<RunFault> }`（仍零 Serialize，不出线） |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_escalate`） | 步分派 | `fault = Stopped` 前置分支（`Terminal::stopped`）先于 pass / 升格；`fault == None` 时既有 pass / 升格链逐字不变 |
| `crates/core/orchestration/src/port.rs` | port 契约 | `MutationRunner::run(&self, root: &str, name: &str, files: &[String], patch: &str, cancel: CancelFlag) -> BoxToolFuture` |
| `crates/infra/checks/src/mutation/mod.rs`（port 实现） | runner 实现 | `impl MutationRunner for ProcessMutation`：签名加 `cancel` 并透传 `execute` |
| `crates/core/checks` | 纯层 | 逻辑零改动（`parse_*_seg` / score / 归并 / 阈值判定不变） |
| `crates/core/checks/src/aggregate.rs` | excludes 口径单点 | `is_file_excluded(path, suites)` 由消费面复用（excludes 全桶口径单点） |
