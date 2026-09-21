# 提案: backtrack-reason-propagation

> **变更**: backtrack-reason-propagation
> **日期**: 2026-07-06
> **状态**: 草稿

---

## 问题

当前工作流引擎的 backtrack 功能存在信息断层：当评估器（evaluator）调用 `phase_log(backtrack_to: "dev-design")` 时，eval.json 只记录了回溯目标阶段，但没有记录**为什么**回溯。后续 `phase_next` 返回的 planner prompt 不包含任何回溯上下文，导致 planner agent 不知道为什么被重新执行，只能盲猜。

具体问题：

1. **信息丢失**：`phase_log` 的输入参数中 `backtrack_to` 非空时，没有强制要求附带原因
2. **上下文断裂**：`phase_next` 返回的 planner/evaluator prompt 不包含回溯原因，agent 无法基于前一次失败的原因调整输出
3. **调试困难**：查看 eval.json 时只能看到 `backtrack_to: "dev-design"`，无法理解当时为什么做这个决策

---

## 提案

在 `phase_log` 的输入 schema 中增加 `backtrack_reason` 字段，并在以下环节传递该信息：

1. **Schema 层**：在 `phaseLogSchema` 中添加可选字段 `backtrack_reason`（字符串，最长 500 字符）
2. **校验层**：`runPhaseLog()` 中增加校验：当 `backtrack_to` 非空时，`backtrack_reason` 必填且不能为空字符串
3. **存储层**：`buildEntry()` 将 `backtrack_reason` 写入 eval.json 条目
4. **读取层**：`getLatestBacktrackTarget()` 重构为 `getLatestBacktrackInfo()`，同时返回 target 和 reason
5. **Prompt 层**：`buildPhaseDef()` 在检测到回溯时，将原因拼接到 planner 和 evaluator 的 prompt 末尾
6. **向后兼容**：旧 eval.json 条目无 `backtrack_reason` 字段时解析不报错，prompt 不拼接

---

## 能力

### 修改的能力

- `pipeline-backtrack` — 增强回溯功能，支持记录回溯原因并在后续 phase 的 prompt 中传播

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` — 在 `phaseLogSchema` 中添加 `backtrack_reason` 字段
- `plugins/dev-team/bin/src/lib/eval-json.ts` — `BuildEntryParams` 和 `buildEntry()` 支持 `backtrack_reason`
- `plugins/dev-team/bin/src/commands/phase-log.ts` — `runPhaseLog()` 添加 `backtrack_reason` 校验逻辑
- `plugins/dev-team/bin/src/commands/phase-next.ts` — `getLatestBacktrackTarget()` 改为 `getLatestBacktrackInfo()`，将 reason 拼接到 prompt 中

### 测试文件

- `plugins/dev-team/bin/src/commands/phase-next.test.ts` — 更新 backtrack 相关测试，覆盖 reason 传播场景

### 不要修改

- `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` — 输出 schema 无需变更，reason 通过 prompt 字符串传递
- `plugins/dev-team/bin/src/lib/workflow.ts` — 工作流定义和依赖图不涉及
- `plugins/dev-team/bin/src/commands/phase-next.test.ts` 之外的测试文件
- 其他 capabilites 的 spec 文件
- MCP 工具接口（handler 层）

---

## 验收标准

| ID   | 变更实现                                         | 验收条件                                                                                                  |
| ---- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| AC-1 | Schema: `phaseLogSchema` 添加 `backtrack_reason` | `backtrack_reason` 为可选字符串，最长 500 字符，旧条目解析不报错                                          |
| AC-2 | 校验: `runPhaseLog()` 验证必填                   | 当 `backtrack_to` 非空时，`backtrack_reason` 缺失或为空字符串则抛错                                       |
| AC-3 | 存储: `buildEntry()` 写入字段                    | `backtrack_reason` 被正确写入 eval.json 的条目标                                                          |
| AC-4 | 读取: `getLatestBacktrackInfo()`                 | 替代 `getLatestBacktrackTarget()`，同时返回 target 和 reason；无 `backtrack_reason` 字段时 reason 为 null |
| AC-5 | Prompt: 回溯原因拼接                             | 当检测到回溯时，planner 和 evaluator 的 prompt 末尾包含 `⚠️ 回溯原因: <reason>`                           |
| AC-6 | 向后兼容                                         | 旧 eval.json 条目无 `backtrack_reason` 字段时不抛错，`getLatestBacktrackInfo()` 返回 reason 为 null       |

---

## 风险

| 风险                                                    | 影响                  | 概率 | 缓解措施                                                                                    |
| ------------------------------------------------------- | --------------------- | ---- | ------------------------------------------------------------------------------------------- |
| 未升级的 skill 调用 `phase_log` 不传 `backtrack_reason` | 校验通过但无原因记录  | 中   | `backtrack_reason` 仅在 `backtrack_to` 非空时强制校验；不使用 backtrack 的正常流程不受影响  |
| 回溯原因过长截断                                        | 信息丢失              | 低   | Zod 的 `.max(500)` 自动截断校验，与 `report` 字段机制一致                                   |
| 向后兼容遗漏                                            | 旧 eval.json 解析失败 | 低   | Schema 中 `backtrack_reason` 为 `optional().nullable()`，旧条目解析为 undefined/null 不报错 |

---

## 过程

### 决策

| 问题                                                  | 决策                                                      | 理由                                                                                    | 备选方案                                                                                |
| ----------------------------------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `backtrack_reason` 校验放在 Schema 还是业务逻辑层     | 业务逻辑层（`runPhaseLog`）                               | Schema 中的 optional 允许旧数据兼容，业务层在 `backtrack_to` 非空时做强校验，分离关注点 | 在 Zod 中用 `.refine()` 做条件校验 — 但 Zod 的条件校验对 nullable/optional 组合不够直观 |
| `getLatestBacktrackTarget()` 改为返回对象还是新增函数 | 改为 `getLatestBacktrackInfo()` 返回 `{ target, reason }` | 避免重复遍历 entries，一次调用获取完整回溯信息                                          | 保留原函数 + 新增 `getLatestBacktrackReason()` — 但需要两次遍历 entries                 |

### 待决问题

- 无

---
