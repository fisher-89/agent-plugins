# 提案: testexec-feedback-embed-findings

> **变更**: testexec-feedback-embed-findings
> **日期**: 2026-10-08
> **状态**: implemented（proposal 与实现同轮落盘；相位推进留待用户裁决）

---

## 问题

两条定向反馈边（static-check / test-execution）失败后的修复 prompt 嵌入面不对称：

- **static-check 已满足**：修复 prompt 嵌入完整 stdout+stderr 诊断（`walker.rs` `static_check_loop`，测试锚「失败诊断注入修复 prompt」在案），本次零改动。
- **test-execution 是缺口**：修复 prompt 只嵌 `findings_brief`（≤10 条聚类措辞、单条 200 字截断——如「「jest」3 项测试失败——聚类失败……明细见子报告 error_cases」）+ `report_dir` 路径指针。真正可动手的修复原料——失败用例名 / 文件 / 行号 / 错误消息 / 堆栈（`SubReport.error_cases`）、suite 执行错误面、覆盖缺口对照——都留在报告 JSON 文件里，修复 agent 得自己找目录、猜 planId、解析 JSON：多烧会话轮次，且 istanbul 族（jest / vitest / vite-plus）V1 不解析用例行时错误面只有命令输出尾部，agent 自读报告也拿不到更多。

用户要求：检查失败后应将结果嵌入 prompt 由 agent 修复——反馈边修复会话的输入面应当自足。

## 提案

test-execution 门禁产出载荷增`findings_detail: String`（修复边明细文本），反馈边修复 prompt 以其替换 `findings_brief` 直嵌：

1. **明细装配**（`infra/checks/testexec`，`findings_brief` 同装配点）：诊断面（findings 全量，缺省回落 problems 消息面）+ suite 执行概览（框架 / root / 退出码 / 计数 / 覆盖三维度对照，null 维度跳过）+ 失败用例明细（名称 + 文件:行号 + 错误类型 + 消息 + 堆栈）。新鲜跑与复用门两路同源装配（复用路子报告自磁盘装载）。
2. **有界执法**：单条消息 500 / 堆栈 800 字符截断（chars 口径 + 省略号）、每 suite 至多 20 条（余量记数）、总预算 12000 字符（超限尾注指向报告目录）——报告文件仍是全量权威。
3. **pass 态恒空串**（反馈边仅 fail / error 消费）。
4. **walker 修复 prompt**：明细直嵌（前导反馈计数 / conclusion 与 `report_dir` 全量兜底行保留）；static-check 反馈边零改动。
5. **载荷边界不变**：`TestExecutionOutcome` 零 Serialize 不进 IPC——`RunUpdate::Step` detail 仍走 `findings_brief` + `diagnose_brief`，明细只在进程内 walker 反馈边消费；bindings 零变化。

## 修改的能力

- **desktop-test-execution** — 「结论反馈边与独立预算」注入物从 findings 诊断摘要改为诊断明细（有界装配文本）；「步骤可观测与最小载荷」载荷面增修复边明细文本（IPC 零全量透传条款保留）。

## 影响

| 触点 | 改动 |
|------|------|
| `crates/core/orchestration/src/port.rs` | `TestExecutionOutcome` + `findings_detail` |
| `crates/infra/checks/src/testexec/mod.rs` | `outcome` 增 sub_reports 参；`findings_detail` / `coverage_brief` / `case_detail` / `clip_chars` 装配 |
| `crates/core/orchestration/src/walker.rs` | `test_execution_loop` 修复 prompt 换嵌明细 |
| 对应 `*_test.rs`（port / steps / walker / testexec mod_test） | 构造点补字段；反馈边 prompt 直嵌回归锚；明细装配口径测试 |

验证：`cargo test --workspace` 1259/1259 pass；`cargo fmt --check` clean；clippy 零新告警。desktop 版本 0.4.20 → 0.4.21。
