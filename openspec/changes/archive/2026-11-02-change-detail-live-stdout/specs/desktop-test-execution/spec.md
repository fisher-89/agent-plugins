# desktop-test-execution Specification（delta: change-detail-live-stdout）

## MODIFIED Requirements

### Requirement: 输出记录为脚本真实输出

test-execution 的输出记录 SHALL 由 summary JSON 重序列化改为**脚本真实输出**：runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（不加 `2>&1`——rust 模板 `cargo llvm-cov nextest --workspace --json --output-path "{coverage_file}" --message-format libtest-json --message-format-version 0.1 --no-fail-fast > "{results_file}"` 已把 stdout（libtest-json 事件流）重定向进结果工件，`2>&1` 会让 stderr（人读报告 / 编译错误）混入污染 `parse_libtest_json`）。`PlanExecution` SHALL 增 `output` 字段承载该合并流（当前 stdout_bytes / stderr_bytes 是局部变量、只进 error 尾部）；`mod.rs` 聚合各 suite `output` 按 plan 序（字典序稳定）拼接，经运行中记录链（`LiveExec::begin` → 逐 plan `write_command` → `commit(&merged)`，同 id 收口——见 desktop-checks-domain「输出记录收敛到 infra/exec」与 desktop-exec-runtime「运行中实时输出记录与增量回查」）落单一 `.log` 并回填 `exec_id`；`record_exec_output` 的等价语义（单一 `.log`、零截断零改写、同锚槽位派生）由该链的 `begin + commit` 保持。全量结构化报告（`summary.json` + 逐 suite 子报告）仍留 report_dir 子树，各框架 report 工件各自为政，MUST NOT 强行统一各框架脚本输出格式。

**命令 sidecar（本变更新增，元数据面）**：每个 plan 展开命令后、spawn 之前 SHALL 把 `=== plan {id} · {framework} ===
{展开后的命令}` 追加进同 id 的 `<exec_id>.cmd`（纯文本，与 `.log` 同目录）——命令文本**不进** `.log`（`.log` 的内容口径仍是「脚本真实输出 + plan 分隔头」，零变化），也 MUST NOT 进 `SubReport` / `summary`。写入失败 SHALL 静默降级（面板呈「命令未记录」，不阻断执行）。

**plan 分隔头（既有，元数据面）**：`mod.rs` 聚合时 SHALL 在每个**已执行** plan 的 `output` 之前写入一行
`=== plan {id} ({framework}) · files={n} · argv={len} 字符 ===`（plan 序稳定、以换行起手与收尾）；该 plan 发生 `{files}` 省略降级时 SHALL 紧随一行 `[argv] {降级说明}`（展开长度 / 平台预算 / 文件数 / 已降级为框架自发现）。分隔头与 `[argv]` 行为**记录元数据**，MUST NOT 被视为脚本输出，故不与本条首句「内容为脚本真实输出」冲突：去掉这些行后，记录正文 MUST 与各 suite 脚本输出逐字节一致、按 plan 序保序。零 plan 执行的复用分支记录仍为空。降级说明 MUST NOT 进入 `SubReport.findings` / `summary.findings`（`collect_problems` 会把「`failed == 0` 且 findings 非空」判成 `execution_error`，绿跑降级会因此翻红——信息面单点在本条记录头）。

#### Scenario: rust 套件不加 2>&1

- **WHEN** test-execution 执行 rust 套件（模板为单条 `cargo llvm-cov nextest … --message-format libtest-json > "{results_file}"`）
- **THEN** spawn 不施加 `2>&1`（stdout 仍重定向进结果工件供 `parse_libtest_json`）；record 内容为 stderr（人读 PASS/Summary 行与编译错误面），`results.txt` 在报告树可见，不强行 tee

#### Scenario: 各 suite 输出按 plan 序聚合

- **WHEN** 一次 test-execution 含多个 suite（如 jest 与 rust），各 suite 脚本分别产出 stdout / stderr
- **THEN** record 落单一 `.log`，内容为各 suite 合并输出按 plan 序拼接（每段前有该 plan 的分隔头）；`read_exec_output` 回查内容在去掉分隔头与 `[argv]` 行后与脚本真实输出逐字节一致，MUST NOT 是 summary JSON 重序列化

#### Scenario: 结构化报告仍为全量权威

- **WHEN** record 改为脚本输出后 fail / error 收场
- **THEN** `summary.json` 与逐 suite 子报告仍落 report_dir 子树（全量结构化结论权威），`findings_detail` / 报告目录兜底语义不变，record 只作人工回查的终端还原

#### Scenario: 分隔头消歧与留痕

- **WHEN** 一次执行含 vp / rust 两 plan，其中 vp plan 展开后被降级
- **THEN** record 中两段输出各以自身 plan 头起手（`=== plan … · files=N · argv=M 字符 ===`），降级 plan 头后紧跟 `[argv]` 行；人工回查无需反推模板即可判定「哪段输出属于哪个 plan」「哪个 plan 发生过降级」

#### Scenario: 空输出多 plan 仍有头

- **WHEN** 各 plan 脚本未产出任何 stdout / stderr（如程序未拉起即被拒绝）
- **THEN** record 内容为各 plan 的分隔头序列（非空、plan 序稳定），仍可经 `exec_id` 回查；零 plan 执行的复用分支记录仍为空串

#### Scenario: 逐 plan 命令落 sidecar 而不进 log

- **WHEN** 一次 test-execution 含 rust 与 vite-plus 两 suite，两 plan 均 spawn
- **THEN** `<exec_id>.cmd` 含两段（各以 `=== plan <id> · <framework> ===` 起手并紧随该 plan 展开后的命令）；`.log` 不含命令文本（内容仍是脚本真实输出 + plan 分隔头），去掉头行后与脚本输出逐字节一致的口径不变

#### Scenario: 运行中即可读且同 id 收口

- **WHEN** 多 plan 执行中读取 `<exec_id>.log`，与执行结束后再读并比对 `TestExecutionOutcome.exec_id`
- **THEN** 运行中读到迄今已写字节（连续读单调增长）；终态覆盖为 `merged_output` 且 `exec_id` 与运行中同一 id（不另铸新 id）

#### Scenario: 复用门零 spawn 仍回填引用

- **WHEN** 复用门命中（既有 summary 齐整且输入未变），本次零 spawn
- **THEN** 仍产生一条 `.log` 记录（空内容）并回填 `exec_id`；`<exec_id>.cmd` 可为缺省（无命令可记 ⇒ 面板呈「命令未记录」），零 spawn 语义不变


## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/testexec/mod.rs` | 记录生命周期与命令 sidecar | `execute()` 入口 `LiveExec::begin` → 上抛 `exec_id`（`on_output_ref`）→ 逐 plan spawn 前 `write_command` 追加一段 `=== plan <id> · <framework> ===` + 换行 + 展开命令 → `commit(&merged_output)`（`outcome` / `fault_outcome` / 复用门三分支同 id）；`.log` 内容口径零变（脚本真实输出 + plan 分隔头），命令文本只进 `.cmd` |
| `crates/infra/checks/src/testexec/runner.rs` | 只透传 sink | `execute_plan(plan, report_dir, cancel, live)` 把 `live` 交给 `run_shell`；MUST NOT 自持记录生命周期、MUST NOT 自写 `.cmd` |
| `crates/infra/exec/src/lib.rs` | 记录单点（跨域） | `LiveExec::{begin, write_command, commit}` 与 `read_exec_command_in`（见 desktop-exec-runtime / desktop-checks-domain）；本域零私有落盘分支 |
