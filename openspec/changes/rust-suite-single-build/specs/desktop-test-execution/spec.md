# desktop-test-execution Specification (Delta)

## ADDED Requirements

### Requirement: rust suite 单构建（一条命令同产结果与覆盖率）

Test-execution 的 rust 档 SHALL 以**单条命令**执行测试并采集覆盖率：注册表 shell 臂与 cmd 臂模板 SHALL 逐字同串，形如 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`。runner MUST NOT 对展开后的命令串做任何拼装改写——rust 档 MUST NOT 残留 `; _X=$?; … ; exit $_X` 形态的退出码搬运、MUST NOT 残留 `&`（cmd 无条件顺序分隔符）链式分隔、MUST NOT 由 runner 侧插入结果重定向。suite 退出码 SHALL 直继承该单条命令的退出码（MUST NOT 被链中后段命令洗 0）：`cargo llvm-cov` 失败（测试红 / 覆盖率工件未产出 / 环境文件锁）即 suite 非零退出，经 `execution_error` 定型为 `conclusion=Error`——「覆盖率工件缺失而结论 pass」的假绿路径 MUST NOT 可达。stdout SHALL 全量重定向进 `{results_file}`（`parse_cargo_test` 单一读取面保持；`--output-path` 使 coverage JSON 不进 stdout），覆盖率采集范围 SHALL 为 cargo workspace 全成员（`--workspace`）——结果工件与覆盖率工件 SHALL 由同一批用例产出，MUST NOT 出现「门禁判定的覆盖率数字来自与门禁测试集不同的一批用例」。test-execution 的 rust 档 MUST NOT 写 / 清 `target/debug`（llvm-cov 走 `target/llvm-cov-target`），跨相位（static-check 的 `cargo clippy` / `cargo run`）的 target 清理冲突 MUST NOT 由本相位引入。覆盖率口径与阈值 SHALL 零调整：沿现网形态（不排除 `*_test.rs`）、阈值保持 lines 90 / branches 85 / functions 90，`--workspace` 带来的分母扩大由真件回归证据承接；口径收紧（排除 `*_test.rs`）MUST NOT 在本变更内发生。

#### Scenario: 单条命令两臂同串

- **WHEN** 审查 rust 档注册表条目与 runner 展开结果
- **THEN** shell / cmd 模板逐字同串且为单条 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`；命令串不含 `cargo test`、不含 ` & `、不含 `; _X=$?`；runner 展开后的命令与模板展开逐字一致（零追加改写）

#### Scenario: 退出码直继承不假绿

- **WHEN** `cargo llvm-cov` 因环境文件锁 / 覆盖率工件未产出以非零退出收场，而同批测试用例本身全绿
- **THEN** suite `exit_code` 非零 → summary `problems` 记 `execution_error`（携命令输出尾段）、`conclusion=Error`；MUST NOT 判 `pass`、MUST NOT 静默降级为「覆盖面无数据但通过」

#### Scenario: 覆盖率与结果同批用例产出

- **WHEN** rust 档真件执行完毕（工作区含 `crates/**` 多成员加根包）
- **THEN** `results.txt` 的用例行覆盖全 workspace 成员、`coverage-summary.json` 覆盖文件数 ≥ 200（含 `crates/core` / `crates/infra` / `crates/tools` / `src-tauri/src`），二者由同一次命令产出；子报告 `source_files` 不再局限于根包 25 个文件

#### Scenario: 不触碰 target/debug

- **WHEN** 同一 run 内 static-check 相位与本相位先后执行（前者 `cargo clippy` / `cargo run` 写 `target/debug`）
- **THEN** 本相位对 `target/debug` 零写入零清理（覆盖率构建落 `target/llvm-cov-target`），跨相位 `failed to remove …\target\debug\deps\*.rcgu.o (os error 32)` 冲突形态不可达

#### Scenario: 阈值零调整达标与红跑覆盖缺失

- **WHEN** 真件回归聚合 summary，及测试红跑时 llvm-cov 不产覆盖率工件两种情形
- **THEN** 前者 lines / functions 均达阈值（实测 96.64 % / 90.22 %）、conclusion=pass；后者子报告 `coverage=null`、覆盖维度按 null 感知跳过，结论仍由 `failed>0` / `execution_error` 定型为 fail / error——两情形均不产生假绿

## MODIFIED Requirements

### Requirement: 框架矩阵与显式报错

Test-execution SHALL 支持 jest / vitest / vite-plus（istanbul 族，`coverage-summary.json`）/ rust（单条 `cargo llvm-cov --workspace` 同产文本结果与覆盖，见「rust suite 单构建（一条命令同产结果与覆盖率）」）/ node-test（`--test-reporter=spec` 文本结果）五框架的探测与执行；text 解析器 SHALL 以 node-test / rust 文本分支进移植范围。tests[] 声明 bun / go / pytest suite 时 runner SHALL 显式 `Err`（框架不被支持），MUST NOT 静默跳过该 suite；go-parser / coverage-py 不移植。

#### Scenario: 五框架探测产出 plan

- **WHEN** tests[] 声明五框架 suite（含 includes / excludes glob）
- **THEN** detect 逐 suite 产出 plan entry（root / cwd / id / files），排除过滤生效

#### Scenario: 不支持框架显式报错

- **WHEN** tests[] 声明 `framework: bun`（或 go / pytest）
- **THEN** runner 显式 `Err` 指明框架不被支持，不产出部分结论、不静默跳过

### Requirement: 输出记录为脚本真实输出

test-execution 的输出记录 SHALL 由 summary JSON 重序列化改为**脚本真实输出**：runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（不加 `2>&1`——rust 模板 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会让 stderr（编译错误）混入污染 `parse_cargo_test`）。`PlanExecution` SHALL 增 `output` 字段承载该合并流（当前 stdout_bytes / stderr_bytes 是局部变量、只进 error 尾部）；`mod.rs` 聚合各 suite `output` 按 plan 序（字典序稳定）拼接，经 `record_exec_output(root, change, merged)` 落单一 `.log` 并回填 `exec_id`。全量结构化报告（`summary.json` + 逐 suite 子报告）仍留 report_dir 子树，各框架 report 工件各自为政，MUST NOT 强行统一各框架脚本输出格式。

#### Scenario: rust 套件不加 2>&1

- **WHEN** test-execution 执行 rust 套件（模板为单条 `cargo llvm-cov --workspace … > "{results_file}"`）
- **THEN** spawn 不施加 `2>&1`（stdout 仍重定向进结果工件供 `parse_cargo_test`）；record 内容主要为 stderr（编译错误面），`results.txt` 在报告树可见，不强行 tee

#### Scenario: 各 suite 输出按 plan 序聚合

- **WHEN** 一次 test-execution 含多个 suite（如 jest 与 rust），各 suite 脚本分别产出 stdout / stderr
- **THEN** record 落单一 `.log`，内容为各 suite 合并输出按 plan 序拼接；`read_exec_output` 回查内容与脚本真实输出一致，MUST NOT 是 summary JSON 重序列化

#### Scenario: 结构化报告仍为全量权威

- **WHEN** record 改为脚本输出后 fail / error 收场
- **THEN** `summary.json` 与逐 suite 子报告仍落 report_dir 子树（全量结构化结论权威），`findings_detail` / 报告目录兜底语义不变，record 只作人工回查的终端还原

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/testexec/detect.rs` | rust 档注册表模板单源 | `shell_template` / `cmd_template` 逐字同串 = `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`；`coverage_format = LlvmCov`、工件名 `coverage-summary.json` / `results.txt` 不变；注释留痕「rust 档单条命令有意偏离 CLI 双段链」 |
| `crates/infra/checks/src/testexec/runner.rs` | 进程执行（零注入） | 删 `applyResultsRedirect` 分支与 `native_output_file` 助手——命令串展开后零改写；spawn / 超时 / 输出捕获 / `PlanExecution.output` 合并流 / 工件解析面不变 |
| `crates/infra/checks/src/testexec/`（detect / runner / report） | 五框架注册表与执行链 | 其余四档模板（jest / vitest / vite-plus / node-test）零改动；`{report_dir}` / `{results_file}` / `{coverage_file}` / `{config_args}` / `{files}` 占位符语义零改动；`mod.rs` 报告组装与复用门零改动 |
| `crates/core/checks`（parser / aggregate） | 纯层判定（零改动） | `parse_cargo_test` 多 `running` 块解析、`parse_llvm_cov` 逐文件条目、`coverage_meets_thresholds` 的 null 维度感知、`compute_overrides` 语义均不变——本变更只换工件产出方，不换判定口径 |
