# 测试设计: backtrack-reason-propagation

> **日期**: 2026-07-06

---

## 验收范围

| AC ID | 验收条件                                                                                                  | 测试类型 | 测试文件                                                                                 | 测试对象/测试场景                                 |
| ----- | --------------------------------------------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------- |
| AC-1  | `backtrack_reason` 为可选字符串，最长 500 字符，旧条目解析不报错                                          | 单元测试 | `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts`                              | `phaseLogSchema`                                  |
| AC-2  | 当 `backtrack_to` 非空时，`backtrack_reason` 缺失或为空字符串则抛错                                       | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts`                                    | `runPhaseLog — backtrack_reason validation`       |
| AC-3  | `backtrack_reason` 被正确写入 eval.json 的条目标                                                          | 单元测试 | `plugins/dev-team/bin/src/lib/eval-json.test.ts`                                         | `buildEntry`                                      |
| AC-4  | 替代 `getLatestBacktrackTarget()`，同时返回 target 和 reason；无 `backtrack_reason` 字段时 reason 为 null | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`                                   | `getLatestBacktrackInfo`                          |
| AC-5  | 当检测到回溯时，planner 和 evaluator 的 prompt 末尾包含 `⚠️ 回溯原因: <reason>`                           | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`                                   | `Backtrack Prompt — reason propagation`           |
| AC-5  | 当检测到回溯时，planner 和 evaluator 的 prompt 末尾包含 `⚠️ 回溯原因: <reason>`                           | 集成测试 | `plugins/dev-team/bin/__tests__/backtrack-reason-flow/backtrack-reason-flow.test.ts`     | `backtrack-reason-flow`                           |
| AC-6  | 旧 eval.json 条目无 `backtrack_reason` 字段时不抛错，`getLatestBacktrackInfo()` 返回 reason 为 null       | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`                                   | `getLatestBacktrackInfo — backward compatibility` |
| AC-6  | 旧 eval.json 条目无 `backtrack_reason` 字段时不抛错，`getLatestBacktrackInfo()` 返回 reason 为 null       | 集成测试 | `plugins/dev-team/bin/__tests__/backtrack-reason-compat/backtrack-reason-compat.test.ts` | `backtrack-reason-compat`                         |

---

## 单元测试

### 用例

| 测试文件                                                    | 测试对象                                          | 路径类型 | 测试条件                                                                 | 迭代类型 |
| ----------------------------------------------------------- | ------------------------------------------------- | -------- | ------------------------------------------------------------------------ | -------- |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 正向     | 有效 entry 包含 `backtrack_reason` 时通过 parse                          | 新增     |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 正向     | `backtrack_reason` 不传（undefined）时通过 parse                         | 新增     |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 正向     | `backtrack_reason` 为 null 时通过 parse                                  | 新增     |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 边界     | `backtrack_reason` 长度为 500 字符时通过 parse                           | 新增     |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 边界     | `backtrack_reason` 长度 > 500 字符时 parse 失败                          | 新增     |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 边界     | `backtrack_reason` 含有 emoji、换行符等特殊字符时通过 parse              | 新增     |
| `plugins/dev-team/bin/src/schemas/phase-log.schema.test.ts` | `phaseLogSchema`                                  | 异常     | `backtrack_reason` 为非字符串类型（如数字）时 parse 失败                 | 新增     |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`       | `runPhaseLog — backtrack_reason validation`       | 正向     | `backtrack_to` 非空且 `backtrack_reason` 为有效字符串时通过              | 新增     |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`       | `runPhaseLog — backtrack_reason validation`       | 异常     | `backtrack_to` 非空但 `backtrack_reason` 缺失时抛错                      | 新增     |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`       | `runPhaseLog — backtrack_reason validation`       | 异常     | `backtrack_to` 非空但 `backtrack_reason` 为空字符串时抛错                | 新增     |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`       | `runPhaseLog — backtrack_reason validation`       | 正向     | `backtrack_to` 为 null 时不校验 `backtrack_reason`，不抛错               | 新增     |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`       | `runPhaseLog — backtrack_reason validation`       | 边界     | `backtrack_reason` 长度为 500 字符时通过校验                             | 新增     |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`       | `runPhaseLog — backtrack_reason validation`       | 正向     | `backtrack_to` 为数组且 `backtrack_reason` 提供时通过                    | 新增     |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts`            | `buildEntry`                                      | 正向     | `backtrack_reason` 被正确写入 EvalEntry                                  | 新增     |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts`            | `buildEntry`                                      | 正向     | `backtrack_reason` 为 undefined 时 entry 中该字段为 null                 | 新增     |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts`            | `buildEntry`                                      | 边界     | `backtrack_reason` 为 null 时 entry 中该字段为 null                      | 新增     |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts`            | `buildEntry`                                      | 边界     | `backtrack_reason` 长度为 500 字符时被正确写入                           | 新增     |
| `plugins/dev-team/bin/src/lib/eval-json.test.ts`            | `buildEntry`                                      | 边界     | `backtrack_reason` 含有特殊字符时被正确序列化                            | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo`                          | 正向     | 返回对象包含 `target` 和 `reason` 两个字段                               | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo`                          | 正向     | 最新的 backtrack 条目包含 `reason` 时正确返回                            | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo`                          | 异常     | 回溯条目无 `backtrack_reason` 字段时 `reason` 返回 null                  | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo`                          | 异常     | 无任何回溯条目时 `target` 和 `reason` 均为 null                          | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo`                          | 边界     | 多个回溯条目中只返回最新的 `backtrack_reason`                            | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 正向     | planner prompt 末尾包含 `⚠️ 回溯原因: <reason>`                          | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 正向     | evaluator prompt 末尾包含 `⚠️ 回溯原因: <reason>`                        | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 异常     | 无 `backtrack_reason` 时 prompt 不拼接回溯原因                           | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 边界     | `backtrack_reason` 长度为 500 字符时 prompt 包含完整内容                 | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 正向     | `backtrack_to` 为数组时 prompt 拼接原因                                  | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 异常     | 正常前进（非回溯）时 prompt 不含回溯原因                                 | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `Backtrack Prompt — reason propagation`           | 异常     | 回溯原因是空格字符串时 prompt 不拼接                                     | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo — backward compatibility` | 正向     | 旧 eval.json 条目（无 `backtrack_reason`）解析不抛错                     | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo — backward compatibility` | 正向     | 混合新旧格式条目时，旧条目的 reason 为 null                              | 新增     |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts`      | `getLatestBacktrackInfo — backward compatibility` | 正向     | `getLatestBacktrackInfo()` 在旧格式条目上正确返回 target 和 reason: null | 新增     |

### Mock 策略

| 测试文件                                               | Mock 主体        | Mock 方案                                                                                           | 应用场景                                                                                                             |
| ------------------------------------------------------ | ---------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`  | 文件系统（fs）   | `vi.mock('fs')` 模拟 `existsSync`、`readFileSync`、`writeFileSync`                                  | `runPhaseLog — backtrack_reason validation`                                                                          |
| `plugins/dev-team/bin/src/commands/phase-log.test.ts`  | `eval-json` 模块 | `vi.mock('../lib/eval-json')` 模拟 `readEvalJson`、`markPhaseStale`、`appendEntry`、`writeEvalJson` | `runPhaseLog — backtrack_reason validation`                                                                          |
| `plugins/dev-team/bin/src/commands/phase-next.test.ts` | 文件系统（fs）   | `vi.mock('fs')` 模拟 `existsSync`、`readFileSync`                                                   | `getLatestBacktrackInfo`, `Backtrack Prompt — reason propagation`, `getLatestBacktrackInfo — backward compatibility` |

---

## 集成测试

### 用例

| AC ID | 测试文件                                                                                 | 测试场景                  | 测试条件                                                                                   | 迭代类型 |
| ----- | ---------------------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------ | -------- |
| AC-5  | `plugins/dev-team/bin/__tests__/backtrack-reason-flow/backtrack-reason-flow.test.ts`     | `backtrack-reason-flow`   | phase_log 写入 `backtrack_to` + `backtrack_reason` 后，phase_next 返回的 prompt 含回溯原因 | 新增     |
| AC-5  | `plugins/dev-team/bin/__tests__/backtrack-reason-flow/backtrack-reason-flow.test.ts`     | `backtrack-reason-flow`   | 多阶段回溯：回溯到不同阶段时 prompt 均包含正确的原因                                       | 新增     |
| AC-5  | `plugins/dev-team/bin/__tests__/backtrack-reason-flow/backtrack-reason-flow.test.ts`     | `backtrack-reason-flow`   | planner 和 evaluator 的 prompt 均包含 `⚠️ 回溯原因:` 前缀                                  | 新增     |
| AC-6  | `plugins/dev-team/bin/__tests__/backtrack-reason-compat/backtrack-reason-compat.test.ts` | `backtrack-reason-compat` | 读取不含 `backtrack_reason` 字段的旧 eval.json，phase_next 正常返回不抛错                  | 新增     |
| AC-6  | `plugins/dev-team/bin/__tests__/backtrack-reason-compat/backtrack-reason-compat.test.ts` | `backtrack-reason-compat` | 混合新旧格式条目，回溯时 prompt 不拼接 `⚠️ 回溯原因:`                                      | 新增     |
| AC-6  | `plugins/dev-team/bin/__tests__/backtrack-reason-compat/backtrack-reason-compat.test.ts` | `backtrack-reason-compat` | 旧条目中无 `backtrack_reason`，`backtrack_to` 有效时仍正常回溯                             | 新增     |

### Mock 策略

| 测试文件                                                                                 | Mock 主体 | Mock 方案                                                                                                      | 应用场景                  |
| ---------------------------------------------------------------------------------------- | --------- | -------------------------------------------------------------------------------------------------------------- | ------------------------- |
| `plugins/dev-team/bin/__tests__/backtrack-reason-flow/backtrack-reason-flow.test.ts`     | 文件系统  | 写入临时目录的 eval.json，依次调用 `runPhaseLog` 和 `runPhaseNext`，验证 prompt 内容                           | `backtrack-reason-flow`   |
| `plugins/dev-team/bin/__tests__/backtrack-reason-compat/backtrack-reason-compat.test.ts` | 文件系统  | 准备不含 `backtrack_reason` 字段的旧格式 eval.json 文件，调用 `runPhaseNext`，验证不抛错且 prompt 不含回溯原因 | `backtrack-reason-compat` |

---

## 参数类型边界覆盖: `backtrack_reason`

`backtrack_reason` 类型为 `string | null | undefined`，最大 500 字符。

| 边界场景                                | 输入                                        | 预期行为                                               |
| --------------------------------------- | ------------------------------------------- | ------------------------------------------------------ |
| 正常值                                  | `"设计文档缺少API签名部分"`                 | 通过校验，写入 entry，prompt 拼接                      |
| 空字符串 ("") — backtrack_to 非空时     | `""`                                        | runPhaseLog 抛错                                       |
| 空字符串 ("") — backtrack_to 为 null 时 | `""`                                        | 不校验，通过（无回溯时该字段无意义）                   |
| 超长字符串 (>500 字符)                  | `"a".repeat(501)`                           | Zod parse 失败，runPhaseLog 抛错                       |
| 恰好最大长度 (500 字符)                 | `"a".repeat(500)`                           | 通过校验，正确写入                                     |
| null                                    | `null`                                      | Zod parse 通过；backtrack_to 非空时 runPhaseLog 抛错   |
| undefined（不传）                       | `undefined`                                 | Zod parse 通过；backtrack_to 非空时 runPhaseLog 抛错   |
| 特殊字符                                | `"原因: 包含emoji 😊 和换行\n以及制表符\t"` | 通过校验，正确写入和拼接                               |
| 纯空格                                  | `"   "`                                     | Zod parse 通过（空格为有效字符）；runPhaseLog 不视为空 |

---

## 不可测试项

- 无。所有验收标准均可通过单元测试或集成测试覆盖。
