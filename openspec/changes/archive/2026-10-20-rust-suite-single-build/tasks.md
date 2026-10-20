# 任务: rust-suite-single-build

> 本列表只覆盖实现文件（`detect.rs` / `runner.rs` / `package.json`）与 AC-6 证据留痕（阶段五，产物为 `reports/evidence/**`）。`detect_test.rs` / `runner_test.rs` 的断言改写、用例退役与新增锚归 test-design / test-gen / test-execution 相位——**本相位不改测试文件**。
>
> 依赖序要点：阶段一（模板自带重定向）必须先于阶段二（删 runner 注入），否则中间态会丢 `results.txt`；阶段三（版本 bump）与阶段四（自查）殿后。本变更零类型 / 签名演进，故无编译面随动、无 bindings 重生成、无 `plugins/**` 改动。
>
> 已知跨相位事实（非本相位修复）：实现落地后，`runner_test.rs` 的 `rust档链形态_覆盖段恒执行_尾段退出码透传`（:256，rust 已非链形态）与 `rust档不加重定向合并_stdout进工件而output承载stderr`（:610，模板需自带 `> "{results_file}"` 才有工件）在两处语义上失效，由测试相位按 proposal「测试文件」表改写 / 退役；`split_outputs_plan` 装置随之处置。

## 阶段一：rust 档模板单源（`detect.rs`）

- [x] 修改 `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs`：`REGISTRY` rust 条目（现 :82）的 `shell_template` 改为 `"cargo llvm-cov --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\""`
- [x] 修改同文件同条目（现 :83）的 `cmd_template`：改为与上一条**逐字相同**的字面量（不抽 `const`，D1）；删除旧串中的 `cargo test --workspace`、`&`、`; _X=$?`、`exit $_X` 全部痕迹
- [x] 修改同文件 `REGISTRY` 头注释（现 :43-44）：由「模板串与 CLI `test-framework.ts` 逐字对齐——唯 rust cmd 臂单侧修复偏离：CLI 保留旧 `if errorlevel` 形态」改写为「jest / vitest / vite-plus / node-test 与 CLI `test-framework.ts` 逐字对齐；rust 档单条命令**有意偏离** CLI 双段链」，并列三条理由——① `cargo llvm-cov` 自身即跑测试，双段付两遍全量构建且 target 目录不复用；② cmd 臂 `&` 退出码取末段，覆盖率工件缺失仍判绿；③ llvm-cov 不带 `--workspace` 只测根包 25 文件，与 2180+ 用例口径不同源
- [x] 修改同文件 `FrameworkEntry.shell_template` 字段注释（现 :26-27）：把「其余文本与 CLI 逐字对齐」改为「其余框架模板文本与 CLI 逐字对齐（rust 档为有意偏离，见 REGISTRY 头注释）」，避免与新注释/新模板自相矛盾
- [x] 自查 `detect.rs`：rust 条目的 `coverage_format` / `coverage_output` / `results_output` / `default_glob` / `config_flag` / `version_command` 逐字未动；jest / vitest / vite-plus / node-test 四条目零字节改动（`git diff -- detect.rs` 仅 :26-27 字段注释、:43-44 头注释与 :82-83 两行模板）

## 阶段二：runner 零注入（`runner.rs`，依赖阶段一）

- [x] 删除 `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` 的 `native_output_file` 函数及其 doc 注释（现 :43-47）
- [x] 删除结果重定向注入分支及其块注释（现 :82-92，`if !native_output_file(&plan.framework) { command = command.replacen("cargo test", …) }`）；`let mut command = expand_template(…)`（现 :74）改为 `let command = expand_template(…)`
- [x] 修改 `PlanExecution.output` 的 doc 注释（现 :36-39）：把承载串 `cargo test > results_file` 改为 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`；「不加 `2>&1`」「stderr 混入会污染 `parse_cargo_test`」「`mod.rs` 按 plan 序聚合落单 `.log`」三句逐字保留
- [x] 修改合并流注释（现 :138-139）：改引新模板串（仅 stdout 重定向、stderr 留在捕获面），语义表述不变
- [x] 退出码缺码哨兵抽为模块私有常量 `EXIT_CODE_NO_STATUS: i32 = -1`：超时强杀分支与 `status.code().unwrap_or(…)` 两处同源（值零变更），`PlanExecution.exit_code` doc 改引该常量——`status.code()` 的缺码臂在 Windows 恒不可达（`std` windows 实现直取 `DWORD`），该行 `-1` 字面量会被 `--in-diff` 突变面收成等价的 `-1` → `1` 突变（Windows 测试无从击杀），字面量离开函数体后由常量值锚用例（`退出码缺码哨兵锚定_负一与超时同值`）钉死
- [x] 自查 `runner.rs`：`grep -n "native_output_file\|applyResultsRedirect\|command.replacen"` 零命中；`results_file_posix` / `coverage_file_posix` 仍为 `expand_template` 实参（无 unused 变量）；spawn / 超时 / 捕获段（现 :94-149）与 `parse_results` / `parse_coverage_artifact` 段零改动

## 阶段三：版本面

- [x] 修改 `packages/desktop/package.json`：`version` `0.5.18` → `0.5.19`（单行；`tauri.conf.json` 经 `../package.json` 跟随，`src-tauri/Cargo.toml` 与 `plugins/**` 不随动）

## 阶段四：边界与语义自查（grep / diff 核对，非脚本执行）

- [x] 核对 `git status` 仅含三项实现文件（`detect.rs` / `runner.rs` / `package.json`）；`crates/core/**`、`crates/infra/checks/src/testexec/{mod.rs,report.rs}`、`src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 全部零改动
- [x] 确认命令串三不变量：展开后 rust 命令不含 `cargo test`、不含 ` & `、不含 `; _X=$?`（`grep` 非测试源码里的旧串只应剩 `detect_test.rs` 内的待改写常量，属测试相位）
- [x] 确认四档 JS / node-test 条目与 `FrameworkEntry` 结构 / `TestPlan` / `PlanExecution` 字段面零变动 → `bindings:check` 预期零 diff（AC-9）
- [x] 确认 `native_output_file` 删除后无任何引用（含测试模块），`cargo clippy` 无 dead code 面（静态链由桌面端 `pnpm -C packages/desktop run server:check` 执行）
- [x] 记录 R1 判据供 test-execution 相位核对：结构性判据 = `reports/evidence/coverage-files.txt` 的 209 文件零 `tests/**` 条目（override 组不产出）；门禁 `reports/test/` 的 `coverage.overrides` 只能在自举后（0.5.19 安装 + 重启门禁进程）回验。若出现条目且 `pass=false`，**停**——不调阈值，交用户裁决 `openspec/config.json` 是否给 rust suite 追加 `includes` / `excludes`（design D4 / 待决问题 1）
- [x] 复核旧链 slot 的数值漂移不改 D11 判据：`reports/test/` 经多次重测后 `total=2219`、rust `lines=97.15` / `functions=91.00`、加权 `97.46 / 93.35 / 93.02`、`results.txt` 2370 行（早期一轮为 2198 / 97.17 / 91.00 / 97.48·93.02·93.35 / 2217 行），但 `source_files` 恒 **25** → 该 slot 仍是旧双段链口径（未自举翻转），继续只作「自举前」参照；凡引其数字按「量级 + 25 文件判定」读，不按末位读（design「起点核实」门禁 slot 条 + 「槽位语义」）

## 阶段五：AC-6 证据留痕（工作树实跑，非门禁产物）

- [x] 目标命令的工作树实跑产物落独立证据目录 `openspec/changes/rust-suite-single-build/reports/evidence/`：`README.md`（来源 / 命令 / 退出码 / 耗时 / 判据映射 / 局限 / 自举次序）、`run.cmd`（命令原文）、`results.txt`（stdout 全文 2198 用例行 / 17 个 `running` 块）、`coverage-files.txt`（209 文件路径）、`coverage-totals.json`（lines 85784/88763、functions 6713/7441、branches `count=0`、分组计数、阈值判定）
- [x] 明确证据挂靠面（design D11）：`reports/baseline/` = 变更前实测；`reports/test/` = 门禁进程（已安装 dev-team 0.5.17，exe 10-18 15:31 / 进程 10-19 10:45，早于 `detect.rs` 16:56 改动）的旧双段链产物（25 文件 / 97.17 / 91.00 / 加权 97.48·93.02·93.35），**不作变更后证据、retry 与重测都不改此事实**；`reports/evidence/` = 唯一「变更后」数字来源
- [x] 记录自举次序：合入 → 构建安装 desktop 0.5.19 → 重启门禁进程 → 下一次 test-execution run 的 `reports/test/` 才翻转为 209 文件 / 2198 用例行 / 96.64 / 90.22（加权 96.63·93.02·90.58），届时回验 `conclusion=pass` / `coverage.pass=true` / `coverage.overrides=null`

### R1 判据留痕（供 test-execution 相位）

实现已按 D4 零改 `openspec/config.json`（阈值 lines 90 / branches 85 / functions 90 与 rust suite 声明逐字未动）。核对面：

- **结构性判据**（现在可核）：`reports/evidence/coverage-files.txt` 的 209 条路径零 `tests/**` 命中 → rust override 组不产出。
- **门禁侧回验**（自举后）：读自举后 run 的 `reports/test/summary.json` 的 `coverage.overrides`：期望 `null`；若为 `null` → 结论由加权实测 + 阈值决定，核 `coverage.pass=true`、`conclusion=pass`。
- 若非 `null` 且任一条目 `pass=false` → **停**，不改阈值、不改 config；把该组条目（`glob` / `file_count` / `measured`）交用户裁决是否给 rust suite 追加 `includes` / `excludes`。
- **不得**用当前 `reports/test/`（旧链进程产物）判定 R1 翻转与否。

