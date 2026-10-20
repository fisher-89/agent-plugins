# 提案: rust-suite-single-build

> **变更**: rust-suite-single-build
> **日期**: 2026-10-19
> **状态**: 已定稿（2026-10-20 回溯修订：AC-6 证据面改挂 `reports/evidence/`，门禁 summary 侧待自举回验）
> **来源**: `explore.md`（requirement 类型，已实测验证；本篇为该探索的 S1 切片，可独立实施）

---

## 问题

`crates/infra/checks/src/testexec/detect.rs` 的 rust 档注册表模板（现状，`detect.rs:82-83`）把测试与覆盖率拆成两条命令：

```rust
// shell 臂
"cargo test --workspace; _X=$?; cargo llvm-cov --json --output-path \"{coverage_file}\"; exit $_X"
// cmd 臂
"cargo llvm-cov --json --output-path \"{coverage_file}\" & cargo test --workspace"
```

三个彼此独立、均已在真件日志中取证的缺陷，外加一处跨相位资源冲突：

1. **重复构建 + 重复跑测试**：`cargo llvm-cov` 自身就会运行测试，后面的 `cargo test --workspace` 是第二遍；两者还落在**不同 target 目录**（`target/llvm-cov-target` 与 `target/debug`），编译缓存无法复用——一次 test-execution 付两遍全量构建成本。

2. **`&` 吞退出码 → 覆盖率假绿**：cmd 臂的 `&` 是无条件顺序分隔符，退出码取**最后一条**命令。实证：`openspec/changes/archive/2026-10-18-desktop-agent-chat-component/reports/baseline/exec-outputs/exec-1792234957813-1.log` 里 `cargo llvm-cov` 因文件锁失败（`error: failed to build archive … target\llvm-cov-target\…`，`could not compile serde`，cargo 退出 101），随后的 `cargo test --workspace` 跑绿 → 该 run 的 `reports/baseline/summary.json` 结论 **`conclusion=pass`**，而 rust 子报告 `source_files = 0`（覆盖率工件根本没产出），全局覆盖数字仅由 vite-plus 一侧供给（lines 96.21）——**覆盖率门禁整面失效而门禁判定为绿**。冷 / 热 target 的锁失败概率不同（冷 target 先死、热 target 后死），窗口真实存在。

3. **覆盖率只测根包 25 个文件（最严重）**：`cargo llvm-cov` 不带 `--workspace` 时只测根包 `dev-team`（`src-tauri/Cargo.toml` 是「根包混合 workspace」清单，根包 = `src/**`）。而 2180+ 条用例跑在 `cargo test --workspace`（另一份 target、不产覆盖率）。两份口径对照：

   | 口径 | 覆盖文件 | 分布 |
   |---|---|---|
   | 现状（无 `--workspace`） | **25** | 全为 `src-tauri/src/**`；**`crates/**` 零覆盖** |
   | 合并后（带 `--workspace`） | **209** | `crates/core` 104 + `crates/infra` 79 + `crates/tools` 1 + `src-tauri/src` 25 |

   本 change 自身的 `reports/baseline/packages_desktop_src-tauri_rust/report.json` 即现状实证：`total=2198` 用例、`source_files` 恰 25 条；回溯归档的每次 test-execution 亦恒为 25（个别 27）条。**门禁用的覆盖率数字与门禁用的测试集，不是同一批用例产出的**——这解释了历史 summary 里 coverage 长期稳定 96–97%、从不下滑。

4. **跨相位 target 冲突**：一次 run 内 static-check 写 `target/debug`（`cargo clippy` / `cargo run`），test-execution 的 `cargo test --workspace` 随后要清理同一目录。实证：`openspec/changes/archive/2026-10-19-desktop-explore-list-archive/reports/baseline/exec-outputs/exec-1792379250452-2.log:217` — `error: failed to remove …\target\debug\deps\foundation-…rcgu.o: 另一个程序正在使用此文件 (os error 32)`。

## 提案

只改桌面端 rust 档执行模板与其测试（`packages/desktop/src-tauri/crates/infra/checks`），编排 / 聚合 / 前端 / 插件零改动，四面：

1. **rust 档收敛为单条命令**：注册表 shell 臂与 cmd 臂模板 SHALL 逐字同串，改为
   `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`。
   一条命令同时满足三件事：跑全 workspace 测试（`--workspace`）、覆盖率 JSON 落 `--output-path`（不进 stdout）、测试 harness stdout 重定向进 `results.txt`（`parse_cargo_test` 读取面不变）。退出码**直继承**该命令——llvm-cov 失败即 suite 非零退出 → `execution_error` → `conclusion=Error`，假绿路径从根上消失（缺陷 2、3 同批修掉；重复构建亦随之一并消除）。

2. **runner 侧重定向注入退役**：`runner.rs:82-92` 的 `applyResultsRedirect` 分支（对非原生 output file 框架 `replacen("cargo test", "cargo test > …")`）与 `runner.rs:45-48` 的 `native_output_file` 助手删除——重定向已进模板，runner 展开后对命令串**零改写**。连带好处：S1 后命令串不再含 `cargo test` 字样，注入式补丁失去锚点（留着即死代码）。

3. **跨相位 target 冲突消失**：test-execution 不再触碰 `target/debug`（llvm-cov 走 `target/llvm-cov-target`），缺陷 4 的系统性成因消除。残留的偶发文件锁不再被 `&` 掩盖——它会以非零退出显式暴露为 `execution_error`（这是期望行为：显式失败优于假绿）。

4. **覆盖率口径与阈值零调整**：沿用现网口径（不排除 `*_test.rs`，现网 25 文件里同样含 `mod_test.rs`），实测 `--workspace` 后 lines **96.64 %** ≥ 90、functions **90.22 %** ≥ 90，三项全过，**不改阈值、不改聚合语义**。口径收紧（排除 `_test.rs` 后 functions 85.84 < 90）属独立裁决，不在本变更内。

范围裁决（explore 未决项的落地口径）：

- **只改 desktop**：CLI（`plugins/dev-team/bin/src/lib/test-framework.ts` / `test-runner.ts`）保留旧双段链与其用例。插件源已弃用（`AGENT.md` 首段：不再积极开发，功能零变更），本变更**不修 CLI 侧同缺陷**、零版本 bump；若将来重启插件，需另立变更同步模板。
- **与 sibling 篇解耦**：explore 提到的「篇 2（`exec-run-shell`）」改 `runner.rs` 的 spawn 段（99–150），本篇只动 45–48 / 82–92 两段与注册表模板——同文件不同区块，建议先合本篇。
- **真件回归证据面为 `reports/evidence/`，门禁 summary 侧待自举回验**：explore 已实测该命令（393.3 s / 2198 用例行 / 17 个 `running` 块 / coverage 14.8 MB / 209 文件 / lines 96.64 %），留痕为 `reports/evidence/` 五件套（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`）。该实跑不经 `crates/core/checks` 聚合、不产 `summary.json`，故 `conclusion` 面以**等价判定**承接（AC-6 的既有判据恰是 `determine_conclusion` 判 pass 的全部输入，而该判定代码本变更零改动）。本 change 的 `reports/test/` slot 由已安装 dev-team 0.5.17 写出，必为**旧双段链**口径（25 文件 / 97.17 / 91.00，加权 97.48·93.02·93.35），**retry 或重测 test-execution 都不改此事实**，只作「自举前」参照；门禁侧 `conclusion=pass` / `coverage.pass=true` / `coverage.overrides=null` 的回验在 0.5.19 安装 + 重启门禁进程后（见风险 R1）。

## 能力

### 新增的能力

无（本变更在既有桌面能力内新增一条 requirement，不引入新能力）。

### 修改的能力

- **desktop-test-execution** —
  - **ADDED**「rust suite 单构建（一条命令同产结果与覆盖率）」：单条模板两臂同串、退出码直继承、覆盖全 workspace、不触碰 `target/debug`、口径与阈值零调整。
  - **MODIFIED**「框架矩阵与显式报错」：rust 档描述由「cargo test + llvm-cov」改为「单条 `cargo llvm-cov --workspace` 同产文本结果与覆盖」。
  - **MODIFIED**「输出记录为脚本真实输出」：rust 模板串由 `cargo test > "{results_file}"`（runner 注入）改为模板自带重定向的单条 llvm-cov 命令；`2>&1` 禁忌与 record 合并流语义逐字保留。

其余能力零改动，理由：

- **desktop-checks-domain**：`parse_cargo_test`（多 `running` 块、`test … ok/FAILED/ignored` 行）、llvm-cov 覆盖解析、阈值 / override 聚合、复用门 mtime 判定均为纯层且代码零改动——本变更只换「谁产工件」，不换「怎么判」。
- **desktop-orchestration 侧（`desktop-change-orchestration` / `desktop-phase-prompts` 等）**：`ToolStepOutput::TestExecution` 载荷、步词汇、升格与决策链零改动。
- **desktop-mutation-testing**：cargo-mutants 段与 `_stryker-tmp` 清理面零改动（target 冲突缓解是本变更的副作用，非契约变化）。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/checks/src/testexec/detect.rs` | `REGISTRY` rust 条目 `shell_template` / `cmd_template` 改为单条 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`（两臂逐字同串）；`REGISTRY` 头部注释「唯 rust cmd 臂单侧修复偏离：CLI 保留旧 `if errorlevel` 形态」改写为「rust 档单条命令有意偏离 CLI 双段链」并留偏离理由（重复构建 / `&` 吞码 / 只测根包） |
| `crates/infra/checks/src/testexec/runner.rs` | 删 `native_output_file`（45–48）与重定向注入分支（82–92）；`PlanExecution.output`（36–40）与合并流（138–141）注释里的「rust 模板 `cargo test > results_file`」改为新模板串（语义不变：stdout 进工件、不加 `2>&1`） |
| `packages/desktop/package.json` | `version` 0.5.18 → 0.5.19（沿「仅用户可见变更才提升」纪律：门禁结论与报告数字用户可见） |

### 测试文件

| 用例 / 常量 | 位置 | 处置 |
|---|---|---|
| `rust_shell` / `rust_cmd` 模板常量 | `detect_test.rs:157-159` | 改为新单条串 |
| 五框架注册表对拍（含 `assert_eq!(REGISTRY[3].cmd_template, rust_cmd)` 与「不含 `%` / `if `」守卫） | `detect_test.rs:224-262` | 期望值随动；新增两臂同串断言 |
| `rust_suite探测_llvm_cov档` 的模板断言 | `detect_test.rs:369-376` | `starts_with("cargo test --workspace")` 改为：`starts_with("cargo llvm-cov --workspace")`、含 `--workspace`、含 `> "{results_file}"`、**不含** `cargo test`、**不含** ` & `、不含 `; _X=$?` |
| `rust档不加重定向合并_stdout进工件而output承载stderr` | `runner_test.rs:610-645` | 改为「模板自带重定向、runner 零改写」锚：模板串含 `> "{results_file}"` 时命令串原样展开（无 `cargo test >` 注入），results 工件仍落报告树、`output` 仍只承载 stderr |
| `rust档链形态_覆盖段恒执行_尾段退出码透传` | `runner_test.rs:255-327` | 退役（rust 档不再是链形态）；承接到「单条命令：双工件同命令落位 + 退出码直继承」锚（非零退出 → `exit_code` 透传 + error 面记因，不产假用例） |
| `artifact_text` / `rust_plan` 等装置的 doc 注释 | `runner_test.rs:86-98` / `524-540` / `516-522` | 「`cargo test > "{results_file}"` 重定向注入」措辞改为新模板串（行为锚不变） |
| 新增 | `detect_test.rs` / `runner_test.rs` | 单条模板三不变量（无链分隔符、无 `cargo test`、含 `--workspace` 与结果重定向）+ 退出码直继承锚 |
| 真件回归 | 本 change 的 `reports/evidence/`（五件套） | 工作树按目标命令实跑一次（explore 已取得 393.3 s / 2198 用例行 / 209 文件 / lines 96.64 % / functions 90.22 %）并持久化留痕（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`）；门禁 slot `reports/test/` 是 0.5.17 旧双段链产物，**不作证据**，其 summary 侧数字待自举后回验 |

### 不要修改

- **四档 JS / node-test 模板与用例**（jest / vitest / vite-plus / node-test）：本就是单条命令，零改动。
- `crates/core/checks`（parser / aggregate / diagnose / reuse）：`parse_cargo_test` 多块解析、llvm-cov 解析、阈值与 override 聚合、`coverage_meets_thresholds` 的 null 感知全部零改动；**阈值与 suite scope 声明（`openspec/config.json`）本变更不动**。
- `crates/core/orchestration` / `crates/core/workflow` / 前端 `src/**` / `src/types/generated/bindings.ts`：载荷、步词汇、报告 DTO 零改动，无需重生成 bindings。
- `crates/infra/checks/src/testexec/mod.rs` / `report.rs`：报告组装、复用门、exec-outputs 落盘零改动。
- `plugins/**`（含 CLI 侧 rust 双段链模板与非 `--workspace` 口径及其用例）：插件源已弃用，零改动、零版本 bump。
- `openspec/specs/desktop-mutation-testing` / `desktop-checks-domain` / CLI 侧 `cli-unit-test-execute` / `test-detect-frameworks` 等：零改动。
- `openspec/changes/rust-suite-single-build/explore.md`：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | rust 档 shell / cmd 模板逐字同串且为单条 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`；断言不含 `cargo test`、不含 ` & `、不含 `; _X=$?` | `detect_test`（注册表对拍 + 三不变量） |
| AC-2 | runner 对 rust 档命令串零改写：模板含 `> "{results_file}"` 时展开后的命令与模板展开逐字一致（无 `cargo test >` 注入） | `runner_test` 新锚 |
| AC-3 | `native_output_file` 与重定向注入分支删尽（无 dead code 警告） | `cargo clippy`（`pnpm -C packages/desktop run server:check` 链内）+ `grep` 复核 `applyResultsRedirect` / `native_output_file` 零残留 |
| AC-4 | 单条命令双工件同落位：`results.txt`（stdout 重定向）与 `coverage-summary.json`（`--output-path`）由同一条命令产出，`parse_cargo_test` 解析面零改动 | `runner_test`（results 工件 + 覆盖工件双读取面） |
| AC-5 | 退出码直继承：单条命令非零退出 → suite `exit_code` 非零 → summary `problems` 记 `execution_error`、`conclusion=Error`；MUST NOT 出现「覆盖率工件缺失而结论 pass」 | `runner_test` 非零退出用例 + 新增直继承锚 |
| AC-6 | 真件回归（证据面 = `reports/evidence/`）：工作树按目标命令实跑退出码 0；`results.txt` 用例行 ≥ 2180、`coverage-files.txt` 覆盖文件数 ≥ 200（含 `crates/**`）、`coverage-totals.json` lines ≥ 90、functions ≥ 90、加权 `coverage.pass=true`（按 vite-plus baseline 原始计数预演 96.63 / 90.58 / 93.02）；`reports/test/` slot 由已安装 dev-team 0.5.17 旧双段链写出（25 文件 / 97.17 / 91.00），**不作本 change 的「变更后」证据**；门禁侧 `conclusion=pass` / `coverage.pass=true` / `coverage.overrides=null` 待 0.5.19 安装 + 重启门禁进程后的自举 run 回验 | 证据面：本 change `reports/evidence/` 五件套（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`）；自举回验面：0.5.19 生效后任一 test-execution run 的 `reports/test/summary.json` 与 rust 子报告（翻转前旧值既不算回归失败、也不得引作变更后数字） |
| AC-7 | 四档 JS / node-test 模板零改动、其用例全绿 | 既有用例 + `git diff` 复核 `detect.rs` 仅 rust 条目前后差异 |
| AC-8 | `packages/desktop/package.json` 0.5.18 → 0.5.19；`plugins/**` 零改动零 bump | 版本 diff |
| AC-9 | 静态门禁全绿：`pnpm -C packages/desktop run server:check`（`cargo fmt` + `cargo clippy`）、`bindings:check` 零 diff | 静态检查链输出 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 门禁聚合链的 override 组未在 explore 实验中走过**：`--workspace` 后逐文件条目由 25 → 209，`compute_overrides` 对 rust suite（缺省 scope `**/tests/**/*.rs`）可能**新生成** override 组，其 `pass` 会参与全局 `coverage.pass` 的与门 | 真件回归可能反而判红 | 中 | AC-6 的证据面已按 `reports/evidence/` 核定：加权 `coverage.pass` 预演为真、override 结构性判定为 `null`（209 文件零 `tests/**` 命中）；**门禁侧**的 override 组条目只能在自举后（0.5.19 安装 + 重启门禁进程）的 `reports/test/summary.json` 上回验；若翻转，先判定该组条目是否真实（集成测试文件覆盖率本就偏低），裁决面为「给 rust suite 显式声明 `excludes` / `includes`」——**MUST NOT 以调阈值绕过**；该追加属同族口径修正，须在 design / implement 阶段取得用户裁决 |
| **R2 functions 90.22 % 距阈值 90 仅 0.22 个点** | 后续新增未覆盖代码即翻红 | 中 | 本变更零阈值调整（实测达标）；余量不足留痕于此，跌破时按口径变更另立（排除 `_test.rs` 需先给 `export-bindings` 补测试或调 functions 阈值） |
| **R3 覆盖率口径含 `*_test.rs`**（103 个、占 81 % 代码行、自身覆盖 97.48 %），数字虚高、掩盖业务代码缺口 | 门禁宽松（明知接受） | 中 | 沿用现网口径（现网 25 文件同样含 `mod_test.rs`），口径收紧独立变更；本 spec 留痕可考 |
| **R4 `cargo llvm-cov` 未来版本不再转发 test harness stdout** | `results.txt` 无用例行 → `execution_error` 显式失败（非假绿） | 低 | `runner_test` 真件锚（results 工件须含 `test … ok` 行）+ AC-6 用例行下限；失效时回退「两条命令 + `&&`」（仍保退出码直继承语义） |
| **R5 红跑路径 llvm-cov 可能不产 coverage 工件** | 子报告 `coverage=null`，覆盖维度跳过 | 中 | 结论已由 `failed>0` / `execution_error` 定型（error 优先），`coverage_meets_thresholds` 对 null 维度本就跳过——不产假绿；留痕于 spec scenario |
| **R6 环境性文件锁（`os error 32`）不因本变更根除** | llvm-cov 偶发失败 | 中 | 系统性成因（跨相位 `target/debug` × `target/llvm-cov-target` 双面争用）随本品消失；残留锁失败现以非零退出显式暴露（旧 cmd 臂是假绿） |
| **R7 冷 `target/llvm-cov-target` 全量重编** | 相位耗时 | 低 | 现状同样存在（旧流程另加一遍 `target/debug` 全量构建）；实测 393 s 与现状 391 s 持平，不恶化 |
| **R8 `crates/tools`（export-bindings）0 % 覆盖率拉低 functions** | 逼近阈值 | 低 | 已计入实测（functions 90.22 达标）；补测试另立变更 |
| **R9 CLI 侧同缺陷留存**（`plugins/dev-team` 双段链模板） | 若插件复活，同一假绿缺陷仍在 | 低（已弃用） | 本篇显式不修并留痕；插件重启时另立变更同步模板 |
| **R10 与 sibling 篇（`exec-run-shell`）同文件不同区块** | 合入冲突 | 中 | 本篇只动 `runner.rs` 45–48 / 82–92 与注册表模板，spawn 段（99–150）不动；建议先合本篇 |
