# 测试设计: parallel-dev-test-tracks

> **变更**: parallel-dev-test-tracks
> **日期**: 2026-06-03
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | 独立函数：`getPrerequisites()`、`getDependents()`、`markPhaseStale()`、`propagateStale()`、`hasPhasePassed()`、`checkGate()`、`buildEntry()` 等纯函数 | `vitest`（`vite-plus/test`） | 覆盖所有 AC（AC-1 ~ AC-16），含边界条件、向后兼容、错误路径。纯函数无 I/O，直接调用。 |
| 集成测试 | `resolvePhaseNext()`、`runPhaseLog()` 等含 mock eval.json 数据的模块间协作 | `vitest`（`vite-plus/test`） | 验证模块间交互正确性：`phase/log` 写入口触发 `markPhaseStale`、`phase/next` 只读决策过滤 stale、`clearEntriesFromPhase` 已被移除。mock 文件系统 I/O。 |
| 临时验证脚本 | workflow 循环不再调用 `phase/check` —— 检查 skill 文件 | Bash/PowerShell | 手动验证 AC-17 —— skill 文件中无 `phase/check` MCP 调用 |

---

## 2. 覆盖映射

> **注**: 所有测试文件路径均指向 `openspec/changes/parallel-dev-test-tracks/tests/` 目录。

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `tests/workflow.test.ts` | 单元测试 | `getPrerequisites("02-dev-design")` 返回 `["01-proposal"]` |
| AC-2 | `tests/workflow.test.ts` | 单元测试 | `getPrerequisites("03-test-design")` 返回 `["01-proposal", "02-dev-design"]` |
| AC-3 | `tests/workflow.test.ts` | 单元测试 | `getPrerequisites("07-code-review")` 返回 `["04-test-gen", "05-implement"]`（与 06-unit-test 相同前置，可并行） |
| AC-4 | `tests/workflow.test.ts` | 单元测试 | `getDependents("02-dev-design")` 返回 `["03-test-design", "05-implement", "09-acceptance"]` |
| AC-5 | `tests/phase-next.test.ts` | 集成测试 | phase/next: 03-test-design 无有效 pass 且 02-dev-design 未 pass → 返回 02-dev-design |
| AC-6 | `tests/phase-next.test.ts` | 集成测试 | phase/next: 05-implement 无有效 pass 且 02-dev-design pass → 返回 05-implement（不因 03/04 未 pass 跳过） |
| AC-7 | `tests/phase-next.test.ts` | 集成测试 | phase/next: 06-unit-test 无有效 pass 且 04-test-gen stale → 返回 04-test-gen |
| AC-8 | `tests/eval-json.test.ts` | 单元测试 | `markPhaseStale("02-dev-design")` 标记 02 stale 并立即传播到下游 05,06,07,08,09 全部 stale，03/04 stale=false |
| AC-9 | `tests/phase-log.test.ts` | 集成测试 | phase/log 含 `backtrack_to: "02-dev-design"` 时自动调用 `markPhaseStale` |
| AC-10 | `tests/phase-log.test.ts` | 集成测试 | phase/log 含 `backtrack_to: ["02-dev-design", "03-test-design"]` 数组时分别调用 `markPhaseStale`，各自传播 |
| AC-11 | `tests/phase-next.test.ts` | 集成测试 | `hasPhasePassed()` 忽略 `stale: true` 的条目 |
| AC-12 | `tests/eval-json.test.ts` | 单元测试 | 旧条目无 `stale` 字段 → 视为 `stale: false`（向后兼容） |
| AC-13 | `tests/phase-next.test.ts` | 集成测试 | bug-fix 工作流不受影响（phase 序列和依赖图正确） |
| AC-14 | `tests/eval-json.test.ts` | 单元测试 | 失效传播为传递闭包：`markPhaseStale("02")` → 02 stale → 05 stale → 06/07/08/09 stale |
| AC-15 | `tests/phase-next.test.ts` | 单元测试 | `clearEntriesFromPhase()` 已被移除；`resolvePhaseNext()` 不再返回 `updatedEntries` |
| AC-16 | `tests/schema.test.ts` | 单元测试 | `backtrack_to` 字符串格式向后兼容（`"02-dev-design"` 与 `["02-dev-design"]` 行为一致） |
| AC-17 | `tests/workflow-loop-check.ps1` | 临时脚本 | workflow skill 文件中无 `phase/check` 调用；`phase/next` 是唯一决策点 |

---

## 3. 测试策略

### 3.1 方法

采用"纯函数单元测试 + mock I/O 集成测试"的两层策略。核心业务逻辑（依赖图、失效传播、stale 过滤器）均为纯函数，可直接测试。命令层（`resolvePhaseNext`/`runPhaseLog`）用 mock eval.json 条目模拟文件系统。

### 3.2 测试分类

- **单元测试**: 覆盖 workflow.ts 的新增导出（`getPrerequisites`, `getDependents`, `PHASE_PREREQUISITES`）、eval-json.ts 的新增函数（`markPhaseStale`, `propagateStale`）、修改后的 `checkGate()`、以及 schema 校验。不依赖文件系统。
- **集成测试**: 覆盖 phase-next.ts 和 phase-log.ts 的入口函数（`resolvePhaseNext`，`runPhaseLog` 的 `markPhaseStale` 调用验证）。mock eval.json entry 数组，不读写磁盘。`runPhaseLog` 通过 mock/spy 验证对 `markPhaseStale` 的调用。
- **临时验证脚本**: 验证 AC-17（workflow 循环不再调用 phase/check），通过 grep 检查 skill 文件中无 `phase/check` MCP 调用。

#### 测试分类说明

- **单元测试**: 不依赖外部服务/数据库的测试，测试独立函数、方法或组件的单一行为。使用 mock 模拟所有外部依赖。
- **集成测试**: 依赖 mock 外部服务的测试，验证多个模块/组件的协同工作。可以启动轻量级测试容器或使用内存数据库。

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock，不依赖真实外部环境。

### 3.3 模拟策略

| 测试对象 | 模拟对象 | 策略 |
|----------|---------|------|
| `resolvePhaseNext()` | eval.json entries | 纯函数，直接传入 mock entries 数组（无 I/O） |
| `runPhaseLog()` | `readEvalJson()`, `appendEntry()` | mock 文件系统读写；验证对 `markPhaseStale` 的调用次数和参数 |
| `markPhaseStale()` / `propagateStale()` | eval.json entries | 纯函数，直接传入 mock entries 数组 |
| `checkGate()` | eval entries | 纯函数，直接传入 mock entries |
| schema 校验 | Zod schema | 直接调用 `schema.parse()` 验证输入/输出 |

---

## 4. 边界场景

### 4.1 workflow.test.ts 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 根 phase 无前置依赖 | `getPrerequisites("01-proposal")` | 返回 `[]` | `tests/workflow.test.ts` |
| 未知 phase ID 容错 | `getPrerequisites("99-invalid")` | 返回 `[]`（不抛异常） | `tests/workflow.test.ts` |
| 未知 phase ID 的 dependents | `getDependents("99-invalid")` | 返回 `[]`（不抛异常） | `tests/workflow.test.ts` |
| bug-fix 工作流前置依赖 | `getPrerequisites("05-implement", "bug-fix")` | 返回 `["02-dev-design"]` | `tests/workflow.test.ts` |
| bug-fix 工作流 dependents | `getDependents("02-dev-design", "bug-fix")` | 返回 `["05-implement"]`（不含 test 轨道 phase） | `tests/workflow.test.ts` |
| 默认 workflow_type 回退 | `getPrerequisites("06-unit-test")`（不传 workflow_type） | 返回 `["04-test-gen", "05-implement"]`（默认 requirement） | `tests/workflow.test.ts` |
| getDependents 从 prerequisites 推导的一致性 | 遍历所有 phase，每个 phase 的 prerequisites 的并集满足逆关系 | `getDependents` 结果与手动反向推导的 dependents 表完全一致 | `tests/workflow.test.ts` |

### 4.2 eval-json.test.ts 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| markPhaseStale 目标无 pass 条目 | entries 中 02-dev-design 只有 fail 条目 | 无条目被修改，`propagateStale` 不调用 | `tests/eval-json.test.ts` |
| markPhaseStale 目标无任何条目 | entries 中不含 02-dev-design 的条目 | 无操作，不抛异常 | `tests/eval-json.test.ts` |
| markPhaseStale 多 pass 条目时标记最新 | 02-dev-design 有 attempt 1/2/3 三个 pass | attempt 3（最新 timestamp）被标记 `stale: true` | `tests/eval-json.test.ts` |
| propagateStale 中途有 phase 无条目 | entries 无 04-test-gen，但有 06-unit-test | 跳过 04，继续传播到 06 | `tests/eval-json.test.ts` |
| propagateStale 从叶节点 | `propagateStale(entries, "06-unit-test")` | 无条目被标记（06 无 dependents） | `tests/eval-json.test.ts` |
| propagateStale 从最后一个 phase | `propagateStale(entries, "09-acceptance")` | 无条目被标记（09 无 dependents） | `tests/eval-json.test.ts` |
| propagateStale visited-set 防重复遍历 | 同一 downstream phase 被多条路径引用 | 每个 phase 只处理一次 | `tests/eval-json.test.ts` |
| 旧条目无 stale 字段视为 false | `{phase: "02", verdict: "pass"}`（无 stale 字段） | `checkGate` 视此条目标记为有效 pass | `tests/eval-json.test.ts` |
| 新条目 stale:false 与无字段等价 | 一个条目标明 `stale: false`，另一个无该字段 | 两者在 `checkGate` 和 `hasPhasePassed` 中行为一致 | `tests/eval-json.test.ts` |
| checkGate 按 prerequisites 过滤 stale | 02 有 pass 但 `stale: true`，作为 03 的前置 | `checkGate(entries, ["02"])` 返回 `{passed: false, missing: ["02"]}` | `tests/eval-json.test.ts` |
| checkGate 空 prerequisites | `checkGate(entries, [])` | 返回 `{ passed: true, missing: [] }` | `tests/eval-json.test.ts` |
| propagateStale 标记 dependent 的所有条目 | 某 dependent 有 pass 和 fail 两个条目 | 两个条目均被标记 `stale: true` | `tests/eval-json.test.ts` |

### 4.3 phase-next.test.ts 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 依赖图驱动：03 无 pass 且 02 未 pass | entries: 01✓ | phase/next 返回 02（03 依赖 02，故不能跳过） | `tests/phase-next.test.ts` |
| 依赖图驱动：05 无 pass 但有 02 pass 且 03/04 未 pass | entries: 01✓, 02✓ | phase/next 返回 05（05 仅依赖 02，不因 03/04 阻塞） | `tests/phase-next.test.ts` |
| 依赖图驱动：06 的前置 04 或 05 未 pass | entries: 01✓, 02✓, 03✓, 04✓(stale), 05✓ | phase/next 返回 04（06 依赖 04 和 05，04 失效） | `tests/phase-next.test.ts` |
| stale pass 被 hasPhasePassed 忽略 | 02 唯一 pass `stale: true` | `hasPhasePassed(entries, "02")` 返回 false | `tests/phase-next.test.ts` |
| 混合 stale 和非 stale 条目 | 02 有 stale:true 和 stale:false 两个 pass | `hasPhasePassed(entries, "02")` 返回 true（找到非 stale） | `tests/phase-next.test.ts` |
| 向后兼容：无 stale 字段 | 02 pass 条目无 stale 字段 | `hasPhasePassed(entries, "02")` 返回 true | `tests/phase-next.test.ts` |
| `clearEntriesFromPhase` 已被移除 | `resolvePhaseNext` 返回结果 | `ResolvePhaseNextResult` 无 `updatedEntries` 属性 | `tests/phase-next.test.ts` |
| resolvePhaseNext 不再修改 entries | 传入 entries 数组 | 返回后 entries 数组内容不被修改（引用相同对象） | `tests/phase-next.test.ts` |
| bug-fix 工作流正常 | bug-fix: 01✓, 02✓ → 返回 05 | 跳过 test 轨道（03,04），正确返回 05 | `tests/phase-next.test.ts` |
| bug-fix 工作流 backtrack | bug-fix: 06 fail, backtrack_to: "02" | 返回 02（bug-fix 表第二个 phase） | `tests/phase-next.test.ts` |
| 全轨道多轮迭代 | 01-09 全部 pass 后重新回溯 02 | hasPhasePassed(02) 先 false（旧 stale），重做后再 true | `tests/phase-next.test.ts` |

### 4.4 phase-log.test.ts 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| phase/log 写 pass 不触发传播 | 写入 02-dev-design pass | `markPhaseStale` 未被调用，只写条目 | `tests/phase-log.test.ts` |
| phase/log 首条 pass 不触发传播 | 写入 02-dev-design 的首个 pass（之前无条目） | `markPhaseStale` 未被调用 | `tests/phase-log.test.ts` |
| phase/log fail + backtrack_to 触发传播 | backtrack_to: "02-dev-design" | 写入前调用 `markPhaseStale("02-dev-design")` | `tests/phase-log.test.ts` |
| phase/log fail + backtrack_to 数组 | backtrack_to: ["02-dev-design", "03-test-design"] | 分别对每个 target 调用 `markPhaseStale` | `tests/phase-log.test.ts` |
| backtrack_to 数组含无效 phase ID | backtrack_to: ["02-dev-design", "99-invalid"] | 校验失败，拒绝整个操作，不写入任何条目 | `tests/phase-log.test.ts` |
| backtrack_to target 无 pass 条目 | 02-dev-design 无任何条目 | `markPhaseStale` no-op，不抛异常，条目正常写入 | `tests/phase-log.test.ts` |
| phase/log 新条目不含 stale 字段 | 新 pass 或 fail 条目 | 条目不包含 `stale` 字段（视为 false），stale 只标记旧条目 | `tests/phase-log.test.ts` |
| phase/log 不再调用 checkGate | 任意输入 | `checkGate` 不在 `runPhaseLog` 执行路径中 | `tests/phase-log.test.ts` |
| phase/log 写入新 fail 条目不标记 stale | 新 fail 条目含 backtrack_to | 新条目本身无 `stale` 字段；stale 仅标记旧条目 | `tests/phase-log.test.ts` |

### 4.5 schema.test.ts 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| backtrack_to 为字符串 | `"02-dev-design"` | schema 校验通过 | `tests/schema.test.ts` |
| backtrack_to 为字符串数组 | `["02-dev-design", "03-test-design"]` | schema 校验通过 | `tests/schema.test.ts` |
| backtrack_to 为 null | `null` | schema 校验通过（向后兼容） | `tests/schema.test.ts` |
| backtrack_to 为 undefined | 不传该字段 | schema 校验通过，输出中为 null | `tests/schema.test.ts` |
| backtrack_to 为空字符串 | `""` | schema 校验应拒绝（空字符串不是有效 phase ID） | `tests/schema.test.ts` |
| 字符串与单元素数组等效 | phase/log 收到 string 或 string[] | 内部处理一致，传播行为相同 | `tests/schema.test.ts` |

---

## 5. 测试数据

### 5.1 基础 Mock Entry 工厂

沿用现有 `phase-next.test.ts` 中的模式。新增 stale 相关的工厂函数：

```typescript
// stale pass entry: 一个被标记为失效的 pass 条目
function stalePassEntry(phase: string, attempt: number = 1): MockEntry {
  return { phase, verdict: 'pass', attempt, timestamp: nextTs(), stale: true };
}

// stale fail entry: 一个被标记为失效的 fail 条目
function staleFailEntry(phase: string, attempt: number = 1): MockEntry {
  return { phase, verdict: 'fail', attempt, timestamp: nextTs(), stale: true };
}

// 指定 stale 值的 pass 条目
function passEntry(phase: string, attempt: number = 1, overrides: Partial<MockEntry> = {}): MockEntry {
  return { phase, verdict: 'pass', attempt, timestamp: nextTs(), backtrack_to: null, ...overrides };
}
```

### 5.2 完整 9-phase 场景数据集

供集成测试使用的完整数据集：

| 数据集 | 内容 | 用途 |
|--------|------|------|
| `fullPassEntries` | 全部 9 个 phase 各一个 pass 条目 | 正常完成、done 检测 |
| `fullStaleEntries` | 全部 9 个 phase pass，但目标 phase 及下游全部 `stale: true` | markPhaseStale 传播结果验证 |
| `trackSeparatedEntries` | Dev 轨道（01,02,05）pass，Test 轨道（03,04）pass | 轨道隔离验证（06/07/08 同时依赖两者） |
| `backtrackTo02Entries` | 01-05 pass，latest entry 含 `backtrack_to: "02"` | backtrack 场景 |

### 5.3 eval.json 旧格式兼容数据

```typescript
// 旧格式 entry：无 stale 字段
const oldFormatEntry = {
  phase: "02-dev-design",
  verdict: "pass",
  timestamp: "2026-01-01T00:00:00.000Z",
  attempt: 1,
  // 故意不包含 stale 字段
};

// 新格式 entry：明确 stale: false
const newFormatEntry = {
  phase: "02-dev-design",
  verdict: "pass",
  timestamp: "2026-06-01T00:00:00.000Z",
  attempt: 2,
  stale: false,
};
```

---

## 6. 不可测试项

- **Workflow skill 中的 phase/check 移除** — 测试框架无法自动验证 skill 文件内容。**原因**: 由 AC-17 临时验证脚本 `workflow-loop-check.ps1` 通过 grep 手动验证 skill 文件中无 `phase/check` MCP 调用。
- **用户手动 archive 行为** — 用户是否手动运行 `openspec-archive-change` 由用户控制，非系统行为。**原因**: 超出测试范围。
- **所有历史 eval.json 文件的向后兼容性** — 无法用自动化测试覆盖所有已存在的 eval.json 文件。**原因**: 通过 AC-12 的单元测试验证代码逻辑层面的向后兼容（`!e.stale` 对 `undefined` 返回 `true`）。
- **propagateStale 的递归深度** — DAG 最多 9 层，深度极浅。**原因**: 性能无需专项测试。
- **phase/check MCP 工具保留的调试行为** — 保留但弃用，不影响核心工作流。**原因**: 调试工具不产生功能需求。
