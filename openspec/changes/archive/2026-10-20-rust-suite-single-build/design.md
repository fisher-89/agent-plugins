# 设计: rust-suite-single-build

> **变更**: rust-suite-single-build
> **日期**: 2026-10-19
> **基于**: `proposal.md`、`explore.md` 与 `specs/desktop-test-execution/spec.md`（ADDED「rust suite 单构建」+ MODIFIED「框架矩阵与显式报错」/「输出记录为脚本真实输出」）

---

## 提案与规格同步状态

`proposal.md`「变更范围 - 实现文件」三文件与 capability delta 已定稿，本设计不重复其论证，只做五件事：逐文件落定实现形态（含删改的精确区块）、用真件证据收掉 proposal 风险 R1（override 组）、补齐注释留痕与自查口径、按 AC-1..AC-9 逐条对齐、**把 AC-6 的真件证据面从门禁产物改挂工作树实跑（D11）**——本 change 的 `reports/test/` 由已安装 dev-team 0.5.17 旧模板进程产出，不能作新模板证据；真件证据落 `reports/evidence/`，门禁侧数字按自举次序待 0.5.19 安装后才翻转（见「AC-6 证据路径与自举次序」）。

起点核实（工作树实况，锚点为改动前文件行号；实现已按 D1 / D2 / D3 / D7 与「修改文件」`runner.rs` ⑤ 落地并经复核，三文件无需回改。落地后核对：`grep -rn "native_output_file\|applyResultsRedirect\|command.replacen" crates/infra/checks/src/` 零命中（AC-3）；`packages/desktop/package.json` = `0.5.19`（AC-8）；`crates/core/**`、`testexec/{mod.rs,report.rs}`、`src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 零改动（只读红线）；`reports/evidence/` 五件套在册（AC-6 证据面）。）：

- **rust 档是注册表里唯一的双段链**（`detect.rs:79-89`）：shell 臂 `cargo test --workspace; _X=$?; cargo llvm-cov --json --output-path "{coverage_file}"; exit $_X`（:82），cmd 臂 `cargo llvm-cov --json --output-path "{coverage_file}" & cargo test --workspace`（:83）；另四档（jest / vitest / vite-plus / node-test）本就是单条命令，且 jest 双形态已同串（:49-50）。
- **注册表头注释仅记录了 cmd 单侧修复**（`detect.rs:43-44`）：「模板串与 CLI `test-framework.ts` 逐字对齐——唯 rust cmd 臂单侧修复偏离：CLI 保留旧 `if errorlevel` 形态」。S1 后偏离面从「cmd 臂单侧」扩为「rust 整档单条命令」，注释必须改写，否则注释与事实相反。
- **runner 的注入面只有 rust 一个消费者**（`runner.rs:43-47` / `82-92`）：`native_output_file` 对 jest / vitest / vite-plus / node-test 四档返回 `true`，实际被注入的仅 rust（唯一非原生 output file 框架）。删除后**不存在**其它框架的行为变化（AC-7）。
- **spawn 壳形态**：`exec::SystemCommand::script_raw` 在 Windows 为 `cmd /C`、其余 `sh -c`（`crates/infra/exec/src/lib.rs:63-70`），故同一串必须同时满足 cmd / sh 两方言语义；`>` 重定向 + 双引号 POSIX 路径的 cmd 落位已有既有回归锚（`runner_test.rs:222`「windows引号占位模板_重定向工件落位」）。
- **基线实测（旧模板）**：本 change `reports/baseline/packages_desktop_src-tauri_rust/report.json` = `total=2198` / `exit_code=0` / `source_files=25`（全部 `src-tauri/src/**`）/ `lines=97.19` / `functions=91.00`；逐条路径皆绝对 Windows 路径。
- **门禁侧 `reports/test/` 是旧模板产物（不作新模板证据）**：该 slot 由**已安装 dev-team 0.5.17** 的门禁进程写出（exe 于 2026-10-18 15:31 构建、进程于 2026-10-19 10:45 启动，均早于 `detect.rs` 的 2026-10-19 16:56 改动），实测 = `source_files=25` / `lines=97.17` / `functions=91.00` / `summary.coverage.measured` 加权 `97.48 / 93.02 / 93.35` / `coverage.overrides=null`——即**旧双段链**口径。retry 与重测 test-execution 都不改此事实（进程内跑的是旧 exe），故本 change 绝不引用该 slot 的任何数字作为「变更后」证据（D11）。**末位数字随重测漂移、结论不变**：重测每轮用例数微增（最近一轮 `total=2219`、rust `lines=97.15` / `functions=91.00`、加权 `97.46 / 93.35 / 93.02`、`results.txt` 2370 行），`source_files` 恒 25（全 `src-tauri/src/**`）——「25 文件 / 旧链口径」这一判定面不受漂移影响；下引数字取早期一轮，凡涉旧 slot 数值均按「量级与 25 文件判定」读，不按末位读。
- **目标命令实测（工作树实跑，非门禁进程）**：2026-10-19 全量 run（`/tmp/llvmcov-s1-20261019-121051/`，工作树 `desktop-explore-list-archive` 的 `packages/desktop/src-tauri` —— 命令不依赖哪个 worktree，只依赖 workspace 成员与 target 目录，退出码 0 / 393.3 s）→ `results.txt` 2198 条用例行 / 17 个 `running` 块、`coverage-summary.json` 209 文件（`crates/core` 104 + `crates/infra` 79 + `crates/tools` 1 + `src-tauri/src` 25，`tests/**` 命中 0）、`lines=96.64 %`（85784/88763）、`functions=90.22 %`（6713/7441）、`branches` 计数 0（`None` 语义）。该 run 的派生件已落本 change 独立证据目录 `reports/evidence/`（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`）。同批另两次实验（`115744` / `120225`）带 `-- --list`（不跑用例 → 覆盖 0 %），仅作「stdout 未被覆盖 JSON 污染」面参照，**不**作覆盖数字证据。
- **R1 关键证据（新增于本设计）**：① 上述 209 文件清单**零条** `tests/**` 路径（三份实验产物均为 0）；② 桌面时代全部 `summary.json` 的 `coverage.overrides` 恒 `null`（`2026-10-18` / `2026-10-19` 三份归档 + 本 change baseline）——覆盖条目是**绝对路径**，而 `aggregate.rs::in_suite_scope` 的 includes 匹配是**锚定**匹配（`match_glob` 以 `packages/desktop/src-tauri/<pattern>` 起手全匹配绝对串，必然不中），故 override 组在桌面时代结构性不产出。两条独立证据都指向同一结论：`--workspace` 后 rust suite **不会**新生成 override 组。
- **全局加权算术（预演 AC-6）**：`weighted_measured` 跨子报告按原始计数加权。rust 取工作树实跑证据（`reports/evidence/coverage-totals.json`：lines 85784/88763、functions 6713/7441、branches 0）+ vite-plus baseline（`reports/baseline/packages_desktop_vite-plus`：lines 2258/2345、branches 1547/1663、functions 926/992）→ 全局 `lines 96.63 %` / `functions 90.58 %` / `branches 93.02 %`，三项均 ≥ 现网阈值 90 / 90 / 85。此三值是**算术预演**，不是门禁 `reports/test/summary.json` 的实测值（后者仍是旧链 97.48 / 93.02 / 93.35，见上条）。

**只读红线零触点**：`crates/core/checks`（parser / aggregate / diagnose / reuse）、`crates/core/orchestration`、`crates/core/workflow`、`crates/infra/checks/src/testexec/{mod.rs,report.rs}`、前端 `src/**`、`src/types/generated/bindings.ts`、`openspec/config.json` 阈值与 suite 声明、`plugins/**` 与全部插件交付产物、`openspec/specs/**` 基线。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 两臂同串的代码形态 | 新串 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"` 在 `shell_template` 与 `cmd_template` **各写一份字面量**，不抽 `const RUST_TEMPLATE` | 沿既有注册表形态（jest 双形态亦两份字面量，`detect.rs:49-50`）——注册表是「CLI 表逐字移植」的数据面，抽常量会把「哪档偏离 CLI」从表内可读性推进一层间接；「逐字同串」由 `detect_test` 对拍 + 新增两臂相等断言钉死（AC-1） |
| D2 | runner 侧注入面的退役方式 | **整块删除**：`native_output_file`（`runner.rs:43-47`）+ 注入分支（`runner.rs:82-92`）；`let mut command = expand_template(…)` 收为 `let command = expand_template(…)`；保留 `results_file_posix` / `coverage_file_posix` 两个原料变量（`expand_template` 实参仍在用） | AC-2 / AC-3：新串不含 `cargo test`，注入锚点消失，保留即死代码（`native_output_file` 私有且仅此一处消费）；`mut` 去掉后 clippy 无 `unused_mut`。**不保留**「capability 开关」类旁路（单次变更不留双形态） |
| D3 | 注释留痕的落点与内容 | ① `detect.rs:43-44` 改写为「jest / vitest / vite-plus / node-test 与 CLI `test-framework.ts` 逐字对齐；**rust 档单条命令有意偏离** CLI 双段链」，并列三条偏离理由（重复构建 + 重复跑测试 / cmd `&` 吞退出码致覆盖率假绿 / 无 `--workspace` 只测根包 25 文件）；② `runner.rs:36-39`（`PlanExecution.output` doc）与 `runner.rs:138-139`（合并流注释）把 `cargo test > results_file` 改为新模板串，语义表述（stdout 进工件、不加 `2>&1`）逐字保留 | proposal「变更范围」明列；注释是「为什么偏离 CLI」的唯一可考处（CLI 侧缺同源修，D6），且 `2>&1` 禁忌的理由换了承载串后必须同步，否则注释指向不存在的命令 |
| D4 | R1（override 组翻转）的裁决 | **本变更零改 `openspec/config.json`**：证据见上（零 `tests/**` 条目 + 桌面时代 `overrides` 恒 `null`），rust suite 的缺省 scope 组不会产出，全局 `coverage.pass` 仍只由加权实测 + 阈值决定，且预演 96.63 / 90.58 达标。**若**自举后门禁 run（0.5.19）出现 override 组且 `pass=false`：MUST NOT 调阈值；允许的修订面是在 `openspec/config.json` 的 rust suite 显式声明 `includes`（业务源 glob）或 `excludes: ["**/tests/**"]`，属口径变更，**须用户裁决**后才动，且另记 acceptance 变更 | proposal R1 的「须在 design / implement 阶段取得用户裁决」；本条给出可判定的前置证据与唯一后备路径，避免实现期临场改阈值或静默改 config |
| D5 | 覆盖率口径与阈值 | 零调整：不排除 `*_test.rs`（现网 25 文件同样含 `mod_test.rs`）、suite 声明不动、阈值保持 lines 90 / branches 85 / functions 90；`branches` 在 rust 侧为 `None`（llvm-cov 未采分支）由 `coverage_meets_thresholds` 的 null 感知跳过——既有语义，不改 | proposal 第 4 条 + explore §3；收紧口径（排除 `_test.rs` 后 functions 85.84 < 90）需先给 `export-bindings` 补测试或调阈值，属独立变更 |
| D6 | CLI 侧同缺陷 | `plugins/dev-team/bin/src/lib/test-framework.ts:118` / `:122` 的双段链与 `test-framework.test.ts` 的对拍**零改动、零版本 bump**；偏离事实写入 `detect.rs` 头注释（D3）与 spec | `AGENT.md` 首段：插件源已弃用、功能零变更；proposal「不要修改」显式列 `plugins/**`。留痕保证插件复活时能定位到同步点 |
| D7 | 版本 bump | `packages/desktop/package.json` `version` 0.5.18 → 0.5.19（单行）；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml`（`version = "0.1.0"`）不动 | AC-8 + 既有 desktop 变更惯例（`0.5.17→0.5.18` 同形）；门禁结论与报告数字用户可见，属「用户可见变更」 |
| D8 | 与 sibling 篇（`exec-run-shell`）的区块划分 | 本篇只动 `runner.rs` 的 43-47 / 82-92 两区块与 `detect.rs` rust 条目 + 注释；spawn / 超时 / 输出捕获段（`runner.rs:94-149`）逐字不动，合并序建议「先合本篇」 | proposal R10；两篇同文件不同区块，删注入块后 spawn 段行号上移 ~10 行是唯一「冲突面」，语义零交叠 |
| D9 | rust 模板不使用 `{files}` | 维持现状：`cargo llvm-cov --workspace` 覆盖全 workspace 成员，suite 的 `includes` / `excludes` 只决定「是否产 plan」与 mutation 归桶，**不参与命令构造**；`{files}` 在 rust 条目中不出现（旧模板同样不出现） | 与现状等价（旧双段链也无 `{files}`）；若未来要按文件过滤覆盖率需 `--ignore-filename-regex` 或口径声明，属独立裁决（列待决问题 2） |
| D10 | `results.txt` 仍为唯一结果读取面 | 不引入 tee、不加 `2>&1`、不改 `parse_results` 路由（`runner.rs:227-263`）：stdout 由模板 `>` 落 `results.txt`，`parse_cargo_test` 读该文件；`PlanExecution.output`（record 用）在 rust 档主要承载 stderr（编译 / `info:` 面） | spec MODIFIED「输出记录为脚本真实输出」scenario「rust 套件不加 2>&1」；tee 会二次改造 stdout 面且无消费者 |
| D11 | AC-6 真件证据的挂靠面 | AC-6 证据**不挂门禁 `reports/test/`**，改挂工作树按目标命令实跑的独立证据目录 `openspec/changes/rust-suite-single-build/reports/evidence/`（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`）；`reports/test/` 由已安装 dev-team 0.5.17 旧模板进程写出（25 文件 / 97.17 / 91.00 / 97.48·93.02·93.35），标注为**旧链产物、不作变更后证据**；门禁侧数字按自举次序（合入 → 构建安装 0.5.19 → 重启门禁进程 → 下一次 run）才翻转，属安装后回验，不作本变更验收前置 | 门禁进程早于 `detect.rs` 改动（exe 10-18 15:31 / 进程 10-19 10:45 vs 改动 10-19 16:56），retry / 重测 test-execution 都不改产物口径；工作树实跑是唯一能真实观测新模板双工件的面。**不**把旧 slot 数字当新模板证据，也**不**为凑绿改阈值（D4 / D5 不变） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| rust 档注册表条目（模板单源） | `shell_template` / `cmd_template` 逐字同串的单条 llvm-cov 命令；`coverage_format=LlvmCov`、`coverage_output=coverage-summary.json`、`results_output=results.txt`、`default_glob` 均不变；表头注释记录偏离 CLI 的理由与三条缺陷 | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs`（`FrameworkEntry` / `REGISTRY`） | `checks::parser::CoverageFormat`、`config::TestFramework` | 常量表（`&'static str`），占位符原样保留由 runner 展开 |
| runner 命令展开（零注入） | `expand_template` 展开五占位符 → 原样交 `ensure_program_resolvable` → `script_raw` spawn；**无任何后置拼装** | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs`（`execute_plan` / `expand_template`） | `exec::SystemCommand::script_raw`（Win `cmd /C`、unix `sh -c`） | 纯字符串替换 + OS 壳包装（既有） |
| spawn / 超时 / 输出捕获（零改动） | 600 s 超时、stdout/stderr 并行读、`PlanExecution.output` = stdout 前缀 + stderr 后缀、非零退出 → `non_zero_exit_error` | `runner.rs:94-188` | `tokio`、`exec` | 既有；本变更零字节改动 |
| 工件读取面（零改动） | `parse_results`（rust → `parse_cargo_test` 多 `running` 块）与 `parse_coverage_artifact`（LlvmCov → `parse_coverage` / `parse_coverage_measured`） | `runner.rs:220-278` + `crates/core/checks/src/parser/**` | `checks::parser` | 既有单一读取面；本变更只换「谁产工件」 |
| 报告组装与聚合（零改动） | 子报告 `source_files` / `coverage` / `summary` → `mod.rs` 落 `report.json` / `results.txt` / record → `build_summary_report` 加权实测 + 阈值 + override 组 + 结论定型 | `crates/infra/checks/src/testexec/{mod.rs,report.rs}`、`crates/core/checks/src/aggregate.rs` | 既有 | 既有；`source_files` 25→209 与 `measured` 数值变化由数据承接，代码零改 |
| 版本面 | 用户可见变更提升补丁号 | `packages/desktop/package.json` | — | JSON 单行 |

---

## 变更清单

<!-- 以文件为入口逐层展开。清单 = proposal「变更范围 - 实现文件」三项；测试文件（`detect_test.rs` / `runner_test.rs`）的断言改写、用例退役与新增归 test-design / test-gen / test-execution 相位，不入本清单——本变更不引签名 / 类型 / 变体演进，测试模块**零编译面随动**（唯一私有函数删除无外部引用）。 -->

### 新增文件

<!-- 本变更不新增代码文件：单条模板落既有 REGISTRY 条目、零注入落既有 execute_plan、版本落既有 package.json。`reports/evidence/**` 是 AC-6 的验收证据产物（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`），非代码、无导出面，故不入本清单，说明见「AC-6 证据路径与自举次序」。 -->

无。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs` | ① rust 条目（:79-89）`shell_template`（:82）与 `cmd_template`（:83）改为同一字面量 `"cargo llvm-cov --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\""`（两臂逐字同串，无 `cargo test` / `&` / `; _X=$?` / `exit $_X`）；② `REGISTRY` 头注释（:43-44）改写为「jest / vitest / vite-plus / node-test 与 CLI `test-framework.ts` 逐字对齐；rust 档单条命令**有意偏离** CLI 双段链」，并列三条理由（llvm-cov 自身跑测试 → 双段付两遍全量构建 / cmd `&` 退出码取末段 → 覆盖率工件缺失仍判绿 / 无 `--workspace` 只测根包 25 文件与 2180+ 用例不同源）；③ `FrameworkEntry.shell_template` 字段注释（:26-27）中「其余文本与 CLI 逐字对齐」改为「其余框架模板文本与 CLI 逐字对齐（rust 档为有意偏离，见 REGISTRY 头注释）」——仅此半句，字段类型与结构零改 | AC-1 / AC-7；`coverage_format`、`coverage_output`、`results_output`、`default_glob="**/tests/**/*.rs"`、`config_flag=None`、`version_command` 逐字不动；jest / vitest / vite-plus / node-test 四条目零字节改动 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` | ① 删 `native_output_file`（:43-47，含 doc 注释）；② 删注入分支（:82-92，含 4 行块注释），`let mut command`（:74）→ `let command`；③ `PlanExecution.output` doc（:36-39）把承载串改为新模板 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`，`2>&1` 禁忌与 record 聚合语义逐字保留；④ 合并流注释（:138-139）同批改引新模板串（语义不变：仅 stdout 重定向，stderr 留在捕获面）；⑤ 退出码缺码哨兵抽为模块私有常量 `EXIT_CODE_NO_STATUS: i32 = -1`（超时强杀分支与 `status.code().unwrap_or(…)` 两处同源，`PlanExecution.exit_code` doc 改引该常量）——值零变更、公共签名零演进 | AC-2 / AC-3；`expand_template`、`ensure_program_resolvable`、spawn / 超时 / 捕获 / 解析各段逐字不动；`results_file_posix` / `coverage_file_posix` 保留（`expand_template` 实参）；spawn 段零触碰（D8）。⑤ 的理由：`status.code()` 的缺码臂在 Windows 恒不可达（`std` windows 实现直取 `DWORD` 返回 `Some`），该行任何 `-1` 字面量都会被 `--in-diff` 突变面收进等价的 `-1` → `1` 突变（不可击杀）；把哨兵收成常量后字面量离开函数体，改由常量值锚用例钉死 |
| `packages/desktop/package.json` | `version` `"0.5.18"` → `"0.5.19"` | AC-8（D7）；`tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 与 `plugins/**` 不动 |

### 删除文件

<!-- 本变更不删除文件；唯一删除项为私有函数 `native_output_file`（见修改文件 runner.rs 行），不涉及文件级删除。 -->

无。

### 公共函数 / API

<!-- 无模块级导出函数 / CLI 子命令 / HTTP 端点变更：`detect_plans`（pub(crate)）、`framework_name`（pub(crate)）、`to_posix`（pub(crate)）、`execute_plan`（pub(crate)）、`expand_template`（私有）签名全部不变。被删的 `native_output_file` 为模块私有函数且仅一处消费，按「私有函数不列」不入表；其职责注销已在修改文件行留痕。 -->

无。

### 类型定义

<!-- 无 interface / type alias / enum / 公共 API class 变更：`FrameworkEntry` 字段集与类型不变（仅两字段取值改写）、`TestPlan` / `PlanExecution` / `SubReport` / `CoverageBlock` 字段面零改。 -->

无。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.19`（原 `0.5.18`） | 用户可见变更提升补丁号（AC-8 / D7）；`tauri.conf.json` 经 `../package.json` 跟随 |
| `tests[]`（rust suite：`root` / `framework` / `coverage` / `mutation`） | `openspec/config.json` | 否（本变更零改动） | — | 既有声明不变 | D4 / D5：不加 `includes` / `excludes`，不改阈值（lines 90 / branches 85 / functions 90）；仅当 R1 真件翻转且用户裁决后在该处追加声明 |

---

## 数据模型

持久化面**零结构改动**（`report.json` / `summary.json` / 子报告字段集不变），变的是**工件产出方与口径**：

```
report_dir = openspec/changes/<change>/reports/<slot>/<plan_id>/
├── results.txt            ← 唯一命令的 stdout（模板 `> "{results_file}"` 重定向）
└── coverage-summary.json  ← 同一条命令的 `--output-path`（llvm-cov JSON）
        │
        ├─ parse_results → parse_cargo_test(results.txt)         → summary.total/passed/failed/skipped + error_cases
        └─ parse_coverage_artifact → parse_coverage(LlvmCov)      → source_files[]（逐文件原始计数）
                                   → parse_coverage_measured      → coverage.measured（lines / functions；branches=None）
```

变更前后对照（同一字段的来源切换；末列标明每个数字的挂靠面——`reports/test/` 是 0.5.17 旧链产物，不作「变更后」证据，见 D11）：

| 数据面 | 变更前 | 变更后 | 产出方 | 证据挂靠面 |
|--------|--------|--------|--------|------------|
| `results.txt` | 链中第二段 `cargo test --workspace` 的 stdout（由 runner `replacen` 注入重定向） | 唯一命令 stdout（模板自带重定向，runner 零改写） | 模板 `>` | `reports/evidence/results.txt`：2198 用例行 / 17 个 `running` 块 / 首行非 JSON（`reports/test/` 旧链为注入式重定向产物、行数随重测漂移（本轮 2370 行），不引用） |
| `coverage-summary.json` | 链中第一段 `cargo llvm-cov --json`（**无 `--workspace`**，25 文件，全部 `src-tauri/src/**`） | 同一命令 `--output-path`（`--workspace`，209 文件：`crates/core` 104 / `crates/infra` 79 / `crates/tools` 1 / `src-tauri/src` 25） | `cargo llvm-cov` | `reports/evidence/coverage-files.txt`（209 条，`tests/**` 命中 0）+ `coverage-totals.json`；`reports/test/` 旧链 25 文件属变更前口径 |
| `report.exit_code` | shell 臂 `exit $_X`（测试段码）；cmd 臂取末段 → `cargo test` 可把 llvm-cov 失败洗成 0 | 唯一命令退出码**直继承**（非零 → `execution_error` → `conclusion=Error`） | spawn `status.code()` | 工作树实跑退出码 0（`reports/evidence/README.md`）；非零直继承面由 test 相位用例锚承接；门禁侧实测待自举后回验 |
| 子报告 `coverage.measured` | 根包口径（baseline 实测 lines 97.19 / functions 91.00） | workspace 口径（实测 lines 96.64 / functions 90.22，branches `None`） | `parse_coverage_measured` | `reports/evidence/coverage-totals.json`：lines 85784/88763（96.6439 %）、functions 6713/7441（90.2164 %）、branches `count=0` |
| `summary.coverage.measured` | 加权 97.5 / 93.02 / 93.35（`reports/baseline/summary.json`，旧模板 + vite-plus） | 加权 96.63 / 93.02 / **90.58**——**算术预演**（evidence 原始计数 × vite-plus baseline 原始计数），非门禁实测 | `weighted_measured`（零改） | 计算见「起点核实」与 `reports/evidence/README.md` 判据映射；真实 `summary.json` 待自举后回验 |
| `summary.coverage.overrides` | `null` | `null`（结构性判定：209 文件零 `tests/**` + 绝对路径 × 锚定 includes；D4） | `compute_overrides`（零改） | 结构性证据 = `reports/evidence/coverage-files.txt`；`reports/test/` 的 `null` 是旧链弱佐证，门禁侧 `overrides` 待自举后回验 |
| `PlanExecution.output`（record 合并流） | rust 档主要承载 stderr（stdout 进工件） | 语义不变（stdout 进工件、stderr 进捕获面）；仅注释承载串更新 | spawn 捕获（零改） | 无独立真件面；由注释留痕（D3）+ test 相位「合并流语义保持」锚承接 |

- **槽位语义（防误引，D11）**：`reports/baseline/` = 旧模板（变更前）实测；`reports/test/` = 门禁进程（已安装 dev-team 0.5.17）产物，**同为旧双段链口径**（25 文件 / 97.17 / 91.00 / 加权 97.48·93.02·93.35），只作「变更前 / 自举前」参照（该 slot 的末位数值随重测漂移，`source_files=25` 与旧链口径不变）；`reports/evidence/` = 工作树按目标命令实跑，是**唯一**「变更后」数字来源。
- **关系**：`results.txt` 与 `coverage-summary.json` 由**同一条命令、同一批用例**产出（spec scenario「覆盖率与结果同批用例产出」）；`source_files` 与 `summary` 不再来自两个不同 target 目录（`target/llvm-cov-target` vs `target/debug`）。
- **不变量**：① 两臂模板逐字同串；② 展开后命令串与模板展开逐字一致（零追加改写，AC-2）；③ 命令串不含 `cargo test` / ` & ` / `; _X=$?`（AC-1）；④ test-execution 对 `target/debug` 零写入零清理（llvm-cov 走 `target/llvm-cov-target`）。
- **红跑路径**：测试红 / 覆盖率工件未产出 / 环境文件锁 → 命令非零退出 → `error` 面记因 → `problems` 记 `execution_error` → `conclusion=Error`；`coverage` 维度为 `null` 时按 null 感知跳过，**不产假绿**（AC-5）。
- **持久化**：报告树落盘路径、复用门（mtime）、record 单 `.log`、exec-outputs 落盘语义零改动；本变更不引新表、新文件格式、新迁移。证据目录 `reports/evidence/` 是人工留痕产物（非门禁产物），不参与复用门 / mtime 判定。

---

## AC-6 证据路径与自举次序

### 事实：门禁 slot 的产物口径

本 change 的 `reports/test/` 由**已安装 dev-team 0.5.17** 的门禁进程写出：

| 事实 | 值 | 影响 |
|---|---|---|
| 门禁 exe 构建时间 | 2026-10-18 15:31 | 早于 `detect.rs` 改动（2026-10-19 16:56） |
| 门禁进程启动时间 | 2026-10-19 10:45 | 进程内加载的是 0.5.17 的旧双段链模板 |
| 该 slot 的 rust 子报告 | `source_files=25` / `lines=97.17` / `functions=91.00` | 旧链口径（无 `--workspace`，只测根包） |
| 该 slot 的 `summary.json` | `coverage.measured` 97.48 / 93.02 / 93.35、`conclusion=pass` | 旧链加权，非新口径 |

结论：**retry 或重测 test-execution 都不改该 slot 的口径**（进程内跑的是旧 exe），因此它不构成本 change 的「变更后」证据，只作「自举前」参照。上表数值取早期一轮；末位随重测漂移（最近一轮 `total=2219`、rust `lines=97.15` / `functions=91.00`、加权 `97.46 / 93.35 / 93.02`、`results.txt` 2370 行），但 `source_files` 恒 25 ——判定面（旧链、未 `--workspace`、根包口径）不受漂移影响。

### AC-6 的验收证据面（改挂）

AC-6 的判据由**工作树按目标命令实跑**产出，落独立证据目录 `openspec/changes/rust-suite-single-build/reports/evidence/`（不覆盖门禁 slot）：

| 文件 | 内容 |
|---|---|
| `README.md` | 采集来源（工作树 / cwd / 命令 / 退出码 0 / 393.3 s / 工具链 0.9.1）、判据映射、局限、自举次序 |
| `run.cmd` | 目标命令原文（`--output-path` + `> "{results_file}"` 的 shell 侧实例） |
| `results.txt` | stdout 全文：2198 条用例行 / 17 个 `running` 块 / 首行非 JSON |
| `coverage-files.txt` | 209 条覆盖文件路径（`tests/**` 命中 0） |
| `coverage-totals.json` | totals 原始计数 + 百分比 + 分组计数 + 阈值判定 |

AC-6 逐项判据 → 证据字段：

| AC-6 判据 | 证据 | 实测 | 判定 |
|---|---|---|---|
| `results.txt` 用例行 ≥ 2180 | `evidence/results.txt` | 2198 | 通过 |
| 覆盖文件数 ≥ 200（含 `crates/**`） | `evidence/coverage-files.txt` / `coverage-totals.json` | 209（core 104 / infra 79 / tools 1 / src-tauri 25） | 通过 |
| lines ≥ 90 | `evidence/coverage-totals.json` | 96.6439 % | 通过 |
| functions ≥ 90 | 同上 | 90.2164 % | 通过 |
| 全局 `coverage.pass=true` | 同上 × vite-plus baseline 原始计数 | lines 96.63 / functions 90.58 / branches 93.02 | 通过 |
| `conclusion=pass` | 退出码 0 + 双工件齐全 + 三维度达阈值（`aggregate` / `determine_conclusion` 零改动的等价判定） | 满足 | 通过（门禁 summary 侧待自举回验） |

`conclusion=pass` 的说明：工作树实跑不经 `crates/core/checks` 聚合，不产 `summary.json`，故该面以**等价判定**承接——AC-6 的既有判据（双工件同批产出 + 退出码 0 + 三维度 ≥ 阈值）恰是 `determine_conclusion` 判 pass 的全部输入，而该判定代码本变更零改动；真实 `summary.json` 的 `conclusion` 由自举后门禁 run 回验。

### 自举次序（门禁产物待 0.5.19 安装后才翻转）

1. 合入本 change（`detect.rs` / `runner.rs` / `package.json` 0.5.19）；
2. 构建并安装 desktop 0.5.19；
3. 重启门禁进程（新 exe 生效）；
4. 此后任一 test-execution run 的 `reports/test/` 才呈现新口径（209 文件 / 2198 用例行 / lines 96.64 / functions 90.22 / 加权 96.63·93.02·90.58），届时回验 `conclusion=pass`、`coverage.pass=true`、`coverage.overrides=null`。

在此之前，`reports/test/` 的旧值既不算回归失败，也不得被引作「变更后」数字（D11）。

---

<!-- 路由 / API 设计：本变更零新增 / 零改动 HTTP 路由、零新增 / 零改动 Tauri IPC 命令、零 bindings 重生成（`ToolStepOutput::TestExecution` 载荷与步词汇不变；`FrameworkEntry` / `TestPlan` / `PlanExecution` 均不上 IPC），故整节省略。 -->

## 依赖

运行时依赖：**无新增**。`cargo-llvm-cov`（本机 0.9.1）已是 rust 档现状的前置工具（旧模板同样调用），本变更只调整其参数与重定向位置。

构建 / 测试依赖：**无新增**（不引 crate、不引 npm 包、不改 `Cargo.toml` / `package.json` 依赖段）。

| 依赖 | 变更 | 用途 |
|------|------|------|
| `cargo-llvm-cov`（工具链，0.9.1） | 不变 | 单条命令同产测试 stdout 与 llvm-cov JSON（`--workspace` 全成员） |
| `exec`（`SystemCommand::script_raw`） | 不变 | 平台壳包装（Win `cmd /C` / unix `sh -c`）承接模板内的 `>` 重定向 |
| `checks::parser`（`parse_cargo_test` / `parse_coverage*`） | 不变 | 双工件解析（多 `running` 块 / LlvmCov 逐文件条目） |
| `checks::aggregate`（`weighted_measured` / `compute_overrides` / `coverage_meets_thresholds`） | 不变 | 加权实测、override 组、null 感知阈值判定 |
| `tempfile` / `tokio`（测试面，测试相位承接） | 不变 | 既有 `runner_test` / `detect_test` 装置 |

---

## 验收标准落点

| ID | 设计落点 |
|----|----------|
| AC-1 | D1 + 修改文件 `detect.rs` ①②：两臂同串字面量；三不变量（无 `cargo test` / 无 ` & ` / 无 `; _X=$?`）由测试相位在注册表对拍上断言 |
| AC-2 | D2 + 修改文件 `runner.rs` ①②：注入块与 `native_output_file` 删尽，`command` 不可变 → 展开后零改写 |
| AC-3 | D2（唯一消费点删除）+ 自查任务（grep `applyResultsRedirect` / `native_output_file` / `replacen(` 零残留；`server:check` 由桌面端执行） |
| AC-4 | 数据模型表前两行：`results.txt`（模板 `>`）与 `coverage-summary.json`（`--output-path`）同命令落位；`parse_results` / `parse_coverage_artifact` 路由零改 |
| AC-5 | 数据模型「红跑路径」+ `report.exit_code` 行：退出码直继承 → `execution_error` → `conclusion=Error`；`crates/core/checks` 的 `determine_conclusion` / `coverage_meets_thresholds` 零改即保证「工件缺失而 pass」不可达 |
| AC-6 | D11 + 「AC-6 证据路径与自举次序」：判据由**工作树按目标命令实跑**的证据目录 `reports/evidence/`（`results.txt` 2198 行 / `coverage-files.txt` 209 文件 / `coverage-totals.json` lines 96.6439 %、functions 90.2164 %）承接，全局加权（96.63 / 90.58 / 93.02）为算术预演；`reports/test/` 是 0.5.17 旧链产物（25 文件 / 97.17 / 91.00 / 97.48·93.02·93.35），**不作证据**，门禁侧数字按自举次序待 0.5.19 安装后翻转 |
| AC-7 | 修改文件 `detect.rs` 范围限定（仅 rust 条目前后差异）+ 自查任务（四档模板与用例零改动，`git diff` 范围核对） |
| AC-8 | 配置表 `version` 行（0.5.19）+ D6（`plugins/**` 零改动零 bump） |
| AC-9 | 公共函数 / 类型定义两节「无变更」→ 零 bindings 漂移（`bindings:check` 零 diff）；删除块后无死代码（`native_output_file` 唯一消费点同批删除）；`cargo fmt` / `cargo clippy` 由桌面端 `server:check` 执行 |

---

## 待决问题

1. **R1 残留监测点（已有判据，非阻塞；监测面待自举）**：本设计以两条独立证据（`reports/evidence/coverage-files.txt` 209 文件零 `tests/**` 条目；桌面时代全部 `summary.json` 的 `overrides` 恒 `null`）判定 rust override 组不产出，故零改 `openspec/config.json`。**门禁侧**的 `overrides` 判据只能在自举后（0.5.19 安装 + 重启门禁进程）的 `reports/test/summary.json` 上读；当前 slot 的 `null` 来自旧链进程，仅作弱佐证，不构成回验。若自举后出现 override 组：判据 = `coverage.overrides` 非 `null` 且任一条目 `pass=false`；处置 = **停**，不得调阈值，将组条目（`glob` / `file_count` / `measured`）交用户裁决是否在 `openspec/config.json` 为 rust suite 追加 `includes`（业务源 glob，组按 workspace 口径聚合）或 `excludes: ["**/tests/**"]`（组回退为空）。该追加与 AC-6 的「零阈值调整」不冲突，但属口径变更，需另记。
2. **rust 覆盖率仍无法按文件过滤**（D9）：`--workspace` 是成员级而非文件级；suite 的 `includes` / `excludes` 不参与命令构造，故无法把某个 crate 或某类文件排除在覆盖分母外。若后续要求 `crates/tools`（export-bindings 0 %）不计入，落点是 `--ignore-filename-regex` 或 config 口径声明，均属独立裁决。
3. **functions 余量薄（proposal R2）**：workspace 口径 90.22 % vs 阈值 90，余量 0.22 点；本变更零调整，跌破时按 D5 走口径变更（先给 `export-bindings` 补测试，或另裁 functions 阈值）。
4. **`results.txt` 依赖 llvm-cov 转发 harness stdout（proposal R4）**：已实测（2198 条用例行、17 个 `running` 块）。若上游版本改成不转发，表现为「结果工件无用例行 + 退出码 0」→ 现有 `parse_results` 对空文件记「结果工件为空」error，不产假绿；回退方案为「两条命令 + `&&`」（仍保退出码直继承），另立变更。
5. **超时预算未变**：`SUITE_COMMAND_TIMEOUT` 600 s 不变；实测 393 s（旧流程 428 s 级），冷 `target/llvm-cov-target` 全量重编是既有成本（R7）。若冷构建超时，行为是显式超时 error（非假绿），不属本变更调整面。
6. **与 sibling 篇的合并序（D8，proposal R10）**：建议先合本篇；若先合 `exec-run-shell`，本篇删除块的上下文行号偏移需手工对齐（语义无冲突）。
