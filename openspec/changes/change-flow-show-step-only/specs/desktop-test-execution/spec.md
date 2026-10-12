# desktop-test-execution Specification (Delta)

## ADDED Requirements

### Requirement: 输出记录为脚本真实输出

test-execution 的输出记录 SHALL 由 summary JSON 重序列化改为**脚本真实输出**：runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（不加 `2>&1`——rust 模板 `cargo test > "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会让 stderr（编译错误）混入污染 `parse_cargo_test`）。`PlanExecution` SHALL 增 `output` 字段承载该合并流（当前 stdout_bytes / stderr_bytes 是局部变量、只进 error 尾部）；`mod.rs` 聚合各 suite `output` 按 plan 序（字典序稳定）拼接，经 `record_exec_output(root, change, merged)` 落单一 `.log` 并回填 `exec_id`。全量结构化报告（`summary.json` + 逐 suite 子报告）仍留 report_dir 子树，各框架 report 工件各自为政，MUST NOT 强行统一各框架脚本输出格式。

#### Scenario: rust 套件不加 2>&1

- **WHEN** test-execution 执行 rust 套件（模板为 `cargo test > "{results_file}"`）
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
| `crates/infra/checks/src/testexec/runner.rs` | 单 suite 执行 | `PlanExecution` 增 `output: Vec<u8>`（stdout+stderr 合并流，不加 2>&1）；exit_code / cases / source_files / measured / error 语义不变 |
| `crates/infra/checks/src/testexec/mod.rs` | 聚合与落盘 | `record_summary` 退役；聚合各 suite `output` 按 plan 序拼接 → `record_exec_output(root, change, merged)` 单一 `.log` → 回填 `exec_id`；summary/sub_reports 仍经 `report::write_reports` 落 report_dir |
