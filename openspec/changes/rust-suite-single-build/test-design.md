# 测试设计: rust-suite-single-build

> **变更**: rust-suite-single-build
> **日期**: 2026-10-19
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-9）、`design.md`（「变更清单 - 修改文件」三项；「新增文件」与「删除文件」均为空；「公共函数 / API」与「类型定义」子表均为「无」；关键设计决策 D1 / D2 / D3 / D7 / D9 / D10 / D11；修改文件 `runner.rs` ⑤ 的 `EXIT_CODE_NO_STATUS` 常量抽离）、`specs/desktop-test-execution/spec.md`（ADDED「rust suite 单构建」+ MODIFIED「框架矩阵与显式报错」/「输出记录为脚本真实输出」）
> **复核基线（2026-10-20）**: 本设计对 `detect.rs` / `runner.rs` 落地态与兄弟测试模块做了一次逐锚对拍——两臂新串落地、`grep -rn "native_output_file\|applyResultsRedirect\|command.replacen" crates/infra/checks/src/` 零命中、`packages/desktop/package.json` = `0.5.19`；`detect_test.rs`（11 个 rust 面锚 + 1 个 `detect_plans` → `execute_plan` 组合锚）与 `runner_test.rs`（`rust档零改写_模板自带重定向_命令串原样展开` / `单条命令双工件同落位_结果与覆盖同源解析` / `退出码直继承_产双工件后非零` / `llvm_cov工件缺失且非零_error面记因不出假绿` / `退出码缺码哨兵锚定_负一与超时同值` 等）均已按下列设计落位；`reports/evidence/` 实测复核 = 用例行 2198（文件总行 2284）/ `running` 块 17 / 覆盖文件 209 / lines 96.6439 % / functions 90.2164 %。
> **被测模块来源**: `design.md`「修改文件」两项 Rust 实现文件（`testexec/detect.rs`、`testexec/runner.rs`）+ `packages/desktop/package.json`（数据面，无测试路径 → 记入「不可测试项」）

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。测试框架沿用仓库既有约定：Rust 侧 `#[test]` / `#[tokio::test]` 内嵌同目录兄弟测试模块（`#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;`），断言用标准 `assert!` / `assert_eq!` / `assert!(matches!(..))`；断言消息为中文，测试函数名亦为中文标识符。临时目录用 `tempfile::TempDir`（`runner_test`）或既有 `TempWs` fixture 装置（`detect_test`），进程 / 环境变量相关用例经 `crate::TEST_ENV_LOCK` 串行化（`PathWindow` / `EnvWindow` RAII）。

边界取值口径（按 Rust 类型逐项收敛）：`&'static str` / `&str` 无 `None` 可空面，故字符串边界取「空 / 全空白 / 超长（>1000 字符）/ 特殊字符（引号、换行、emoji）」四类并以既有字面量形态为正样本；`Option<&str>`（`config_args`）取 `Some` / `None` 双形态；`&[String]`（`files`）取「空 / 单元素 / 大清单（≥50 项）」三档；`i32`（退出码）取 `0` / 谱内非零值 / `-1` 哨兵；`enum`（`CoverageFormat` / `TestFramework`）取合法值逐个 + 非法值显式 `Err`。`i32` 的极值（`i32::MAX`）与信号终止形态属壳层语义不可稳定构造面，见 `## 不可测试项`。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------|
| AC-1 | rust 档 shell / cmd 模板逐字同串且为单条 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`；不含 `cargo test` / ` & ` / `; _X=$?` | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect_test.rs` |
| AC-2 | runner 对 rust 档命令串零改写：模板含 `> "{results_file}"` 时展开后命令与模板展开逐字一致 | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs`（链路组合锚挂靠 `detect.rs -> detect_test.rs` 的「`detect_plans` → `execute_plan` 组合」行） |
| AC-3 | `native_output_file` 与重定向注入分支删尽（无 dead code 警告） | 无单元测试面（私有函数删除的编译期诊断 + `grep` 复核，见 `## 不可测试项`） |
| AC-4 | 单条命令双工件同落位：`results.txt`（stdout 重定向）与 `coverage-summary.json`（`--output-path`）由同一命令产出，`parse_cargo_test` 解析面零改动 | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs` |
| AC-5 | 退出码直继承：非零退出 → suite `exit_code` 非零 → `execution_error`；不出现「覆盖率工件缺失而结论 pass」 | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs`（suite 级 `PlanExecution.error` 面由本文件用例锚承接；`summary` / `conclusion` 定型属 `mod.rs` / `crates/core/checks` 零改动面，门禁侧回验待 0.5.19 自举后） |
| AC-6 | 真件回归：`conclusion=pass`、`coverage.pass=true`、`results.txt` 用例行 ≥ 2180、覆盖文件数 ≥ 200、lines / functions ≥ 90 | 无单元测试面（判据由**工作树按目标命令实跑**的独立证据目录 `openspec/changes/rust-suite-single-build/reports/evidence/` 承接：用例行 2198（`results.txt` 文件总行 2284、`running` 块 17 个）/ `coverage-files.txt` 209 文件 / `coverage-totals.json` lines 96.6439 %（85784/88763）、functions 90.2164 %（6713/7441）、branches `count=0`；门禁 `reports/test/` 是 0.5.17 旧链产物，不作证据，见 `## 不可测试项`） |
| AC-7 | 四档 JS / node-test 模板零改动、其用例全绿 | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs` -> `.../detect_test.rs`（五框架注册表对拍）+ `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` -> `.../runner_test.rs`（jest / node-test 执行面既有锚） |
| AC-8 | `packages/desktop/package.json` 0.5.18 → 0.5.19；`plugins/**` 零改动零 bump | `packages/desktop/package.json`（数据面，无测试路径，见 `## 不可测试项`） |
| AC-9 | 静态门禁全绿（`cargo fmt` + `cargo clippy`）、`bindings:check` 零 diff | 无单元测试面（桌面端静态链，见 `## 不可测试项`） |

> 被测范围边界：`crates/core/checks`（parser / aggregate / diagnose / reuse）、`crates/infra/checks/src/testexec/{mod.rs,report.rs}`、`crates/core/{orchestration,workflow}`、前端 `src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 均为只读红线（proposal「不要修改」+ design「只读红线零触点」），不纳入被测范围。`framework_name` / `detect_plans` / `to_posix` / `execute_plan` / `expand_template` 签名零演进，`FrameworkEntry` / `TestPlan` / `PlanExecution` 字段面零改，故无编译面随动、无需新增测试模块文件——两个 `*_test.rs` 只在既有用例上改写断言与增删锚点。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect_test.rs

<!-- design.md「公共函数 / API」子表为「无」（`framework_name` / `detect_plans` / `to_posix` 签名全部不变），故本节不推导任何源文件导出面。下列待测功能取自 design.md「修改文件」表的 `detect.rs` ① 修改项（rust 条目模板单源）与其可观测面（`detect_plans` 产出的 `TestPlan`），以及 proposal「变更范围 - 测试文件」表列明的既有断言改写项；`detect.rs` 修改项 ②③ 为注释留痕，无可执行断言面（见 `## 不可测试项`）。 -->

#### 待测功能

- `REGISTRY` rust 条目模板单源（design 修改项 ①）：`shell_template` 与 `cmd_template` 逐字同串的单条 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`，且不含链式残留（AC-1）
- `REGISTRY` rust 条目字段面不变量：`coverage_format = LlvmCov`、`coverage_output` / `results_output` / `default_glob` / `config_flag` / `version_command` 与改造前一致，并作为四档 JS / node-test 条目零改动的对拍基线（AC-7）
- `TestPlan::template()` 平台选臂：Windows 取 `cmd_template`、其余取 `shell_template`；两臂同串使 rust 档平台切换零行为差异
- `detect_plans()` rust 档 plan 产出：`id` / `files` / `cwd` / 新模板占位符原样保留 / `config_args.is_none()`（模板交付面）
- 组合锚（链路方向：`detect_plans` → `execute_plan`）：真实注册表产出的 rust `TestPlan` 交 `execute_plan` 展开执行——单条命令零改写、双工件同落位（AC-2 + AC-4）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `REGISTRY` 五框架对拍（模板 / 工件名 / glob / 旗标 / 版本命令） | 正向 | rust 两臂逐字等于新单条串；jest / vitest / vite-plus / node-test 四档模板（含 jest 双形态同串）与 `coverage_output` / `results_output` / `default_glob` / `config_flag` / `version_command` 逐个等于既有期望值（AC-7 零改动锚） | 表驱动（5 条目） |
| rust 两臂同串与占位符计数 | 正向 | `assert_eq!(REGISTRY[3].shell_template, REGISTRY[3].cmd_template)`；两臂各含 `{results_file}` 与 `{coverage_file}` **恰一次**（`matches(..).count() == 1`，防重复重定向 / 双 `--output-path`） | 单值 |
| rust 模板三不变量 | 正向 | 逐臂断言不含 `cargo test`、不含 ` & `、不含 `; _X=$?`，并断言不含 `exit $_X`（AC-1 / spec scenario「单条命令两臂同串」） | 参数化（2 臂 × 4 不变量） |
| rust 模板正向形态 | 正向 | 两臂均 `starts_with("cargo llvm-cov --workspace")`、含 `--json`、含 `--output-path "{coverage_file}"`、以 `> "{results_file}"` 收尾；不含 `%` / `if ` / `--target-dir`（cmd 单行解析期构造面与 `target/debug` 零钉定面的结构代理） | 参数化（2 臂） |
| rust 模板占位符子集 | 边界 | 新串中 `{files}` 零出现（D9：suite 的 `includes` / `excludes` 不参与 rust 命令构造）、`{randomize}` 零出现（rust 无版本特性分支）、`{report_dir}` 零出现（覆盖工件经 `--output-path` 落位） | 单值 |
| 平台选臂等价 | 边界 | `plan.template()` 在 `cfg!(windows)` 取 `cmd_template`、其余取 `shell_template`；因两臂同串，rust 档选臂结果恒等于新单条串 | 单值 |
| rust 条目字段面不变 | 边界 | `coverage_format = LlvmCov`、`coverage_output = "coverage-summary.json"`、`results_output = "results.txt"`、`default_glob = "**/tests/**/*.rs"`、`config_flag = None`、`version_command = "cargo --version"` 逐字段等值（模板改造未触及字段面） | 表驱动（6 字段） |
| `detect_plans` 的 rust 档产出 | 正向 | tempdir `crates/tests/it.rs` 形态 rust suite → `plan.id == "crates_rust"`、`plan.files == ["tests/it.rs"]`、两臂为新单条串且占位符原样保留、`config_args.is_none()`、字段面同上一行 | 单值 |
| 不支持框架显式 `Err` | 异常 | bun / go / pytest 三值仍显式 `Err` 且记因框架名（模板改造未波及 `registry_entry` 路径；不静默跳过） | 遍历枚举（3 值） |
| rust suite 声明 config → `Err` | 异常 | `config_flag` 仍 `None` → `Err` 含「不支持配置注入」，记因 suite root 与框架名（校验面未因换模板而放宽） | 单值 |
| rust suite 零命中 → 不产 plan | 边界 | includes 全不命中 / 目录下无 `**/tests/**/*.rs` → `plans` 为空且不 panic；混合 suite（rust + vite-plus）时仅命中 suite 产 plan 且 rust 模板仍为新串 | 表驱动（2 形态：零命中 / 混合） |
| `detect_plans` → `execute_plan` 组合（rust plan 单条命令） | 正向 | 真实 `detect_plans` 产出 rust `TestPlan`（真注册表模板）→ PATH 前置 `cargo` 替身 shim（同时写 `--output-path` 目标与 stdout）→ `execute_plan` 展开后的命令串与 `expand_template` 直接展开结果逐字相等（零注入、`> "{results_file}"` 恰一次）→ `results.txt` 解析出用例行、`coverage-summary.json` 解析出实测块（AC-2 + AC-4 链路锚） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（suite 树 / glob 探测面） | 不 mock：`TempWs` 装置真实落盘 fixture 树（命中 / 排除 / dot 剪枝 / 非测试扩展），用例后 `Drop` 清理 | 全部 `detect_plans` 用例 |
| 子进程（`cargo --version` 版本探测） | 不 mock spawn：Unix 前置可执行 shim 脚本、Windows 落 `*.cmd`，经 `PathWindow::prepend` 使裸名真实解析；失败臂用 `PathWindow::clear()` 置空 `PATH`（rust 档无版本特性分支，空版本不影响模板） | rust 档探测、平台选臂、零命中用例 |
| 子进程（`execute_plan` 的被测命令本体） | 不 mock spawn：PATH 前置 `cargo` 替身 shim（脚本解析 `--output-path` 参数写覆盖工件、stdout 写结果工件），真实走 `cmd /C` / `sh -c` 壳 | `detect_plans` → `execute_plan` 组合用例 |
| 全局变量 `PATH` | `TEST_ENV_LOCK` 串行 + `PathWindow` RAII 备份 / 恢复（既有装置），进程内单窗口互斥 | 所有触达版本探测与替身程序解析的用例 |
| 内部模块（`runner::execute_plan` / `checks::parser`） | 不 mock：组合用例真实调用 `execute_plan` 与真实解析器（最小 mock 原则——内部模块必须真实组合） | 组合用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

<!-- design.md「公共函数 / API」子表为「无」（`execute_plan` / `expand_template` 签名不变；被删的 `native_output_file` 为模块私有且仅一处消费、按「私有函数不列」不入表），故本节不推导任何源文件导出面。下列待测功能取自 design.md「修改文件」表的 `runner.rs` ①②③④⑤ 修改项（删注入面 + 注释承载串更新 + 退出码哨兵常量）与其可观测面，以及 proposal「变更范围 - 测试文件」表列明的断言改写 / 用例退役 / 新增锚。 -->

#### 待测功能

- `execute_plan()` 零注入展开（design 修改项 ①②，AC-2）：`native_output_file` 与重定向注入分支删除后，`command` 不可变、展开后的命令串不经任何后置拼装改写
- `execute_plan()` 双工件同落位（AC-4）：`results.txt`（模板 `>`）与 `coverage-summary.json`（`--output-path`）由同一条命令产出，`parse_results` / `parse_coverage_artifact` 两个读取面路由零改动
- `execute_plan()` 退出码直继承（AC-5）：单条命令退出码即 suite `exit_code`，非零 → `error` 面记因，不洗 0、不产假绿
- `PlanExecution.output` 合并流（design 修改项 ③④）：stdout 进工件、stderr 进捕获面，不加 `2>&1` 语义逐字保留，合并序恒「stdout 前缀 → stderr 后缀」
- `expand_template()` 五占位符（零改动回归锚）：`{report_dir}` / `{results_file}` / `{coverage_file}` / `{config_args}` / `{files}` 替换语义不变，含 `Option` / 清单边界
- 退出码哨兵常量 `EXIT_CODE_NO_STATUS`（design 修改项 ⑤）：值 `-1` 由常量锚钉死，超时强杀与「进程无退出码」两处同源；字面量离开函数体后常量值改动即被击杀

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| rust 档零改写（模板自带重定向） | 正向 | rust `TestPlan` + 替身模板 `<系统 cp / type> "<src>" > "{results_file}"` + stderr 打点 → `execute_plan` 展开后命令串与 `expand_template` 直接展开逐字相等；`results.txt` 全量承载 stdout（重定向计数恰 1，无 `cargo test >` 二次注入）；`execution.output` 仅含 stderr 段 | 单值 |
| 单条命令双工件同落位 | 正向 | 一条替身命令同时写 `{results_file}`（stdout 重定向）与 `{coverage_file}`（模拟 `--output-path`）→ `exit_code = 0`、`error = None`、`cases` 非空、`source_files` 非空、`measured.lines = Some(..)`（双读取面均从同一命令产物解析） | 单值 |
| 多 `running` 块结果工件解析 | 正向 | `results.txt` 注入多个 `running` 块（≥2 块）+ `test … ok / FAILED / ignored` 行 → 用例行全量解析、计数与失败归因正确（`parse_cargo_test` 路由零改动，真件 17 块形态的替身锚） | 单值 |
| 退出码直继承（产工件后非零） | 异常 | 替身命令先产双工件再 `exit 3` → `exit_code = 3`、`error` 含「非零退出」与 `3`；已产工件的 `cases` / `measured` 照常解析（不被链式末段洗 0） | 单值 |
| llvm-cov 文件锁失败（覆盖率工件缺失 + 非零） | 异常 | 只写 results、以 `101` 退出（模拟 `error: failed to build archive … target\llvm-cov-target\…` 形态）→ `exit_code = 101`、`error` 记因并携输出尾段、`measured = None`、`source_files` 为空；**断言 `error.is_some()`**（「覆盖率工件缺失而 error 为空」不可达，AC-5 假绿路径封闭） | 单值 |
| 非零退出错误文本装配（既有锚，AC-5 辅证） | 异常 | 非零退出且输出非空 → `error` 含「测试命令非零退出（码）」+ 进程报错文本；输出为空 → 回落裸退出码措辞（不 panic）；`exec::ProcessOutput` 双段拼装逐字（错误面唯一归因线索，改动后承载串换新但不改装配语义） | 表驱动（3 形态：双段 / 空回落 / 携报错文本） |
| 退出码谱边界 | 边界 | 退出码 `0` / `1` / `3` / `7` / `255` 逐一透传（`exit_code` 逐值等值、非零臂 `error` 记因同值），`0` 形态零 error | 参数化（5 值） |
| 结果工件空 / 缺失 | 边界 | 命令成功但 `{results_file}` 为空（含全空白形态）→ `error` 含「结果工件为空」；文件完全缺失 → `error` 含「结果工件缺失」；两者 `cases` 为空且不 panic | 表驱动（2 形态） |
| 退出 0 但覆盖率工件缺失 | 边界 | 命令成功仅产 results（R5 红跑 / 未产覆盖形态）→ `measured = None`、`source_files` 空、`cases` 正常解析、`error = None`（null 感知跳过，非假绿亦非误红） | 单值 |
| 合并流语义保持（rust 档） | 边界 | stdout 重定向进工件后捕获面为空、`output` 只含 stderr（编译错误面）；`error` 面经合并文本派生但不倒灌 `output`（「模板自带重定向、runner 零改写」口径） | 单值 |
| 空输出与大输出 | 边界 | stdout + stderr 皆空 → `output` 为空 `Vec`；stdout 超长（>1000 字符）落工件、stderr 尾段随 `output` 全量承载且序恒「stdout 前缀 → stderr 后缀」（不截断） | 参数化（2 形态：空 / 超长） |
| 模板串边界 | 异常 | 空模板 / 全空白模板 → `execute_plan` 返 `Err` 记因「展开后的命令为空白」（前置检查零改动）；含特殊字符（`"` / `\n` / emoji）的模板经展开后逐字保留（占位符替换不做转义、不改写） | 参数化（3 形态：空 / 空白 / 特殊字符） |
| `expand_template` 五占位符 | 正向 | `{config_args}` 有值形态原样注入、`None` 形态连前导空白剥离；`{report_dir}` / `{results_file}` / `{coverage_file}` 逐字替换；`{files}` 空清单置空串 | 表驱动（4 形态） |
| `expand_template` 清单规模与占位符形态 | 边界 | `files` 取单元素 / 大清单（≥50 项）→ 空格连接逐字；同一占位符多次出现 → 全量替换（`replace` 全量语义）；未知占位符（`{unknown}`）逐字保留且不 panic | 表驱动（4 形态） |
| 裸名程序不可达 | 异常 | `no-such-test-runner-xyz --run {files}` → `Err` 记因裸名 + 「未找到」（前置检查零改动，基础设施失败不落报告级 error） | 单值 |
| 相对 cwd 程序不可达 | 异常 | 带路径分隔符的不存在程序 → `Err` 记因「不可达（相对 suite cwd 不存在）」（前置检查零改动） | 单值 |
| node-test / jest 执行面零改动 | 边界 | jest 档手工 `TestPlan` 走既有 spawn 解析面 → 行为不变（istanbul 族结果不产用例行、计数交退出码）；node-test 文本结果解析路由不变（AC-7 runner 侧锚） | 单值 |
| 退出码缺码哨兵与超时常量锚定 | 边界 | `EXIT_CODE_NO_STATUS == -1`（超时强杀与「进程无退出码」同记 `-1`，非零 → `error` 面显式失败）；`SUITE_COMMAND_TIMEOUT` = 600 s、`VERSION_DETECT_TIMEOUT` = 30 s 值锚定——同时击杀 `-1` 字面量的 UnaryOperator 突变（`-1` → `1`） | 表驱动（3 常量） |

> 退役与承接（proposal「变更范围 - 测试文件」表）：`rust档链形态_覆盖段恒执行_尾段退出码透传`（改动前 `runner_test.rs:255-327`）随 rust 档不再是链形态退役，其「退出码不被前置段洗 0」语义由上表「退出码直继承」「llvm-cov 文件锁失败」「退出码谱边界」三行承接；链形态专用装置 `split_outputs_plan` 同批删除，双工件替身命令装置改为上表前两行形态。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（被测命令本体） | 不 mock spawn：真实 `spawn` 系统原语替身命令（Unix `cp` / `false` / `exit N`，Windows `cmd /C type` / `exit /b N`），经 `exec::SystemCommand::script_raw`（`sh -c` / `cmd /C`）真实执行 | 全部 spawn 用例 |
| 文件系统（报告目录 / 工件落位 / 结果文本） | 不 mock：`tempfile::TempDir` 真实目录 + `std::fs::write` 预置工件，报告目录由 `execute_plan` 自身清空重建 | 全部 spawn 用例 |
| 全局变量 `PATH` | 不 mock：`EnvWindow`（`crate::TEST_ENV_LOCK` + RAII）串行窗口；可达 / 不可达臂靠程序是否存在区分，不改写 PATH | 程序解析前置检查用例 |
| 环境（超时常量 / 退出码哨兵） | 不 mock：`SUITE_COMMAND_TIMEOUT` / `VERSION_DETECT_TIMEOUT` 常量值与 `EXIT_CODE_NO_STATUS` 哨兵值按值锚定；真实超时触发臂与「进程无退出码」回落臂不在此设计内（见 `## 不可测试项`） | 常量锚用例 |
| 内部模块（`expand_template` / `parse_results` / `parse_coverage_artifact` / `checks::parser`） | 不 mock：真实组合调用，端到端断言解析产物（最小 mock 原则） | 全部用例 |

---

## 不可测试项

| 项 | 原因 | 承接面 |
|---|---|---|
| AC-3：`native_output_file` / 重定向注入分支删尽且无 dead code 警告 | 私有函数删除的单元可观测面退化为「源码文本匹配」（脆断言），真实可观测面是编译期诊断；单元测试无从断言「无 dead code」 | 桌面端静态链 `pnpm -C packages/desktop run server:check`（`cargo clippy`）+ `grep -n "native_output_file\|applyResultsRedirect\|command.replacen" runner.rs` 零命中 |
| AC-6：真件回归（2198 用例行 / 209 覆盖文件 / lines 96.64 / functions 90.22 / `conclusion=pass`） | 需本机 `cargo-llvm-cov --workspace` 全量构建（实测 393 s 量级、14.8 MB 覆盖工件），非单元测试可承载 | **工作树按目标命令实跑**的证据目录 `openspec/changes/rust-suite-single-build/reports/evidence/`（`README.md` / `run.cmd` / `results.txt` / `coverage-files.txt` / `coverage-totals.json`）。门禁 `reports/test/` 由已安装 dev-team 0.5.17（exe 10-18 15:31 构建、进程 10-19 10:45 启动，早于 `detect.rs` 16:56 改动）写出，必为旧双段链口径（25 文件 / 97.17 / 91.00 / 加权 97.48·93.02·93.35），**retry 或重测 test-execution 都不改此事实**，其数字不作证据；门禁 summary 侧的自举回验在该目录 README「自举次序」中列明（0.5.19 安装 + 重启门禁进程后翻转） |
| `target/debug` 零写入 / 零清理（spec scenario「不触碰 target/debug」） | 真实落点由 cargo 构建目录选择决定，只能在真件观测；单元测试只能断言命令串不含 `cargo test` / `--target-dir`（已作为 `detect.rs` 用例「rust 模板正向形态」的结构代理） | 工作树实跑证据（`reports/evidence/run.cmd` 无 `--target-dir`、llvm-cov 走 `target/llvm-cov-target`）+ 用例内结构锚 |
| AC-8：`packages/desktop/package.json` 版本 0.5.18 → 0.5.19 | `package.json` 为 JSON 数据面，无兄弟测试模块可挂靠，亦无公共 API 面 | 版本 diff 复核（`git diff -- packages/desktop/package.json`；`plugins/**` 零改动） |
| AC-9：静态门禁（`cargo fmt` / `cargo clippy` / `bindings:check` 零 diff） | 静态检查链与 bindings 漂移检测属脚本执行面，非单元断言面（本变更零类型 / 签名演进，bindings 预期零 diff） | 桌面端静态链输出 |
| `detect.rs` 注释留痕（`REGISTRY` 头注释 ②、`FrameworkEntry.shell_template` 字段注释 ③） | 注释无可执行断言面；语义要求（「rust 档单条命令有意偏离 CLI 双段链」+ 三条偏离理由：重复构建 / `&` 吞退出码致假绿 / 无 `--workspace` 只测根包）无法用 `assert!` 表达 | code review + spec delta（MODIFIED「框架矩阵与显式报错」的 rust 档描述） |
| `SUITE_COMMAND_TIMEOUT` 真实超时触发臂（600 s 挂死 → `exit_code = -1` + 「测试命令超时」） | 单元测试需真实等待 600 s，成本不可接受；超时常量值已由常量锚覆盖 | 既有常量锚 + 真件相位观测，不新增用例 |
| 退出码「进程无退出码」回落臂（`status.code() == None`，Unix 信号终止 → `EXIT_CODE_NO_STATUS`） | Windows `ExitStatus::code()` 恒返回 `Some`（`std` windows 实现直取 `DWORD`），该回落臂在门禁平台不可触达——任何 Windows 用例都无法区分 `-1` 与 `1`（cargo-mutants 对 `-1` 字面量的 UnaryOperator 突变在本平台语义等价，测试无从击杀） | 哨兵值经「退出码缺码哨兵与超时常量锚定」按值锚定（`-1` → `1` 的常量突变由该用例击杀）；回落语义由 `std::process::ExitStatus::code` 的 Unix 契约承接，不新增跨平台不可验证用例 |
| 退出码极值（`i32::MAX`）与负值跨平台透传 | 退出码经 OS 壳层截断/重解释（POSIX `sh -c 'exit 300'` 取低 8 位、Windows `cmd /C exit /b` 互不相同），无法跨平台构造稳定断言；`None` 情形见上一行 | 谱内值（`0` / `1` / `3` / `7` / `255`）透传锚 + `-1` 哨兵常量锚覆盖语义，极值面不设用例 |
