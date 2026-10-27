# 测试设计: ipc-zod-runtime-schema

> **变更**: ipc-zod-runtime-schema
> **日期**: 2026-10-26
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（范围 / 验收标准 AC-1..AC-13）、`design.md`（组件 / 决策 D1..D16 / 公共 API 表 / 新增与修改文件表 / 「回溯修正记录」R-1..R-5）、`specs/{desktop-ipc-runtime-guard,desktop-ipc-type-bindings}/spec.md`、工作树源码与既有测试盘点
> **被测范围**: design「变更清单 - 新增文件」与「修改文件」两张子表的全部模块（删除文件不纳入；本变更无删除文件）
> **口径基准**: design §回溯修正记录 明示「后续相位以现值为准」——本文件按现值落笔：`ZodFormat` 复用 `specta_serde::Format` + `builder().disable_serde_phases()`（R-2）、`char` 在产品出线面零实例（R-3）、形参名在类型层不可得（R-4）、被隐藏 `Result` 声明的悬空引用只由 AC-8 的 tsc 桩拦截（R-1）、规模为 46 命令 / 5 个 `Channel` 信封键 / 82 个具名 schema（R-5）

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| 前端 | `vite-plus` 的 test 面（vitest API，`vp test`），`environment: jsdom`、`globals: true`、`setupFiles: src/test-setup.ts`，`include: src/**/*.test.ts?(x)` | `<模块名>.test.ts` / `.test.tsx`，与被测源文件同目录 | `import { describe, expect, it, vi } from 'vite-plus/test'`；`vi.hoisted` + `vi.mock` 打进程边界替身（`@tauri-apps/api/core` 的 `invoke` / `Channel`），`vi.spyOn` 观测 console |
| Rust | `cargo test --workspace`（`server:test`），同一 crate 内 `#[cfg(test)] mod <模块>_test;` | 兄弟文件 `<模块>_test.rs`（既有 121 个，如 `src/bindings/mod_test.rs`、`src/env_test.rs`） | 子模块可直接 `use super::{…}` 引用父模块私有项（既有先例）；`tempfile::TempDir` 作真实文件系统边界；文件产物用 `Mutex` 串行锁 + `Drop` 守卫 |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 输出）/ `异常`（契约外输入、错误面、失败路径）/ `边界`（空、单元素、超大、`null`/`undefined`、上限值、枚举全量等取值边界）。
- **迭代类型**：`单次`（一次调用即判） / `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行，如命令键联合、类型面集合、边界值集合） / `组合迭代`（两个以上维度交叉，如消息序列 × 命中判定、跨命令双出口 × 包裹幂等）。

**Mock 纪律**（贯穿全篇）：只 mock 进程边界（Tauri IPC / 文件系统 / console / 运行环境门）与被测 API 显式注入的依赖；生成物三表、`ipc-drift.ts`、`ipc.ts` 等内部模块一律真实组合。不为测试新增导出（`no test-only exports`），Rust 侧经 `super::` 访问私有项、TS 侧只经模块已声明的导出行为断言。

**待测功能条目口径**：`#### 待测功能` 只列 design「公共函数 / API」与「类型定义」两表声明的条目；design 未声明公共 API 变更的源文件（19 个 `commands.` → `ipc.` 机械替换点，以及 `bindings.ts` / `use-workspaces.ts` / `agent-transport.ts` 三处行为增补）保留 HTML 注释说明，并一律以**无序列表**承载行为面——有 design 声明的条目写 `- funcName(): 描述`，无 design 声明的章节写 `- <回归对象>（无 design 声明的公共 API 变更条目）: 描述`，不为凑数虚构条目、不 grep 源文件导出声明。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 参数面非法输入本地拒绝：抛 `IpcArgsError`（含命令名 + jsonPointer 路径 + expected/got），`invoke` 零调用；合法输入零误拒。覆盖方式：`src/lib/ipc.test.ts` 参数面章节（错类型 / `null` / 缺键 / 嵌套多余键 / 零形参 / 直通多臂） | `packages/desktop/src/lib/ipc.ts`（参数面）、`packages/desktop/src/types/generated/bindings.zod.ts`（`argsSchemas`） |
| AC-2 | 结果面漂移 soft-report：脏结果记录 1 条并原值返回；干净结果零记录；既有降级渲染零改动。覆盖方式：`src/lib/ipc.test.ts` 结果面章节 + 各 `views/**` 既有渲染 / hook 用例全绿回归 | `packages/desktop/src/lib/ipc.ts`、`packages/desktop/src/lib/ipc-drift.ts`、`bindings.zod.ts`（`resultSchemas`） |
| AC-3 | Channel 信封逐条上报、不丢流、包裹幂等（同一 channel 重复传入只报一次）。覆盖方式：`src/lib/ipc.test.ts` 信封面章节 + `src/lib/agent-transport.test.ts` 双出口组合用例 + 两处 Channel 消费 hook 的既有流式回归 | `packages/desktop/src/lib/ipc.ts`（信封面）、`bindings.zod.ts`（`channelArgs`）、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/views/changes/hooks/use-change-flow-run.ts`、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts` |
| AC-4 | 错误面三分：后端 reject 串语义不变、`IpcArgsError` 走既有 `String(err)`、漂移不 throw。覆盖方式：`src/lib/ipc.test.ts` 错误面章节 + `src/hooks/use-workspaces.test.ts` + 各调用点既有 error 态用例全绿 | `packages/desktop/src/lib/ipc.ts`、`packages/desktop/src/hooks/use-workspaces.ts` 与 18 个 `commands.` 调用点 hook / 组件 |
| AC-5 | 装配层零手写镜像（新增命令零改 `ipc.ts`）；业务代码零 `commands.*` 直接调用。覆盖方式：`src/lib/ipc.test.ts` 键遍历用例（键集 == `commands` 键集、逐键可调用、按名缓存）+ `commands\.` grep 扫描（桌面端静态步骤）+ 下列 21 文件既有 mock `invoke` 用例全绿 | `packages/desktop/src/lib/ipc.ts`；`src/hooks/use-workspaces.ts`、`src/hooks/use-app-environment.ts`、`src/hooks/use-agent-session.ts`、`src/lib/agent-transport.ts`、`src/views/agent/hooks/use-agent-options.ts`、`src/views/agent/hooks/use-agent-run-history.ts`、`src/views/agents/hooks/use-agent-instances.ts`、`src/views/agents/hooks/use-agent-providers.ts`、`src/views/changes/agent/hooks/use-change-chat.ts`、`src/views/changes/components/change-create-dialog.tsx`、`src/views/changes/hooks/use-change-detail.ts`、`src/views/changes/hooks/use-change-flow-run.ts`、`src/views/changes/hooks/use-change-list.ts`、`src/views/changes/hooks/use-exec-output.ts`、`src/views/config/hooks/use-workspace-config.ts`、`src/views/db/hooks/use-db-inspector.ts`、`src/views/explores/components/explore-create-dialog.tsx`、`src/views/explores/hooks/use-explore-doc.ts`、`src/views/explores/hooks/use-explore-list.ts`、`src/views/explores/hooks/use-explore-session.ts`、`src/views/info/hooks/use-code-stats.ts` |
| AC-6 | 两份产物语义等价：逐命令 `z.infer` 与 `bindings.ts` 类型 Equal（`Option` / `f64` / 整数 / literal 枚举 / Intersection / `unknown`），递归类型不塌 `any`。`char` 在**产品出线面零实例**（design R-3 / 校准 6）——其等价性判据落在 wrapper fixture（`ZodFormat::map_types` / `map_type` 的 `char → z.string()` 臂），产品面的替代判据是「产物零 `.refine(`」（AC-7）。覆盖方式：`src/lib/ipc-contract.test.ts`（`expectTypeOf` Equal，由 `vp check` 的 typeCheck 执行）+ `src-tauri/src/bindings/zod_test.rs` 的 `char` 臂 | `packages/desktop/src/types/generated/bindings.zod.ts`、`packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src-tauri/src/bindings/zod.rs` |
| AC-7 | 生成期硬约束：产物内九类禁用片段零命中（`.default(` / `.transform(` / `.catch(` / `.pipe(` / `.refine(` / `export type ` / `z.infer` / `z.ZodType<` / `z.object(`）、`z.strictObject` 写死、`degraded(` 只落 `Channel` 形参位且合法 `unknown` 仍 `z.unknown()`。`.refine(` 的真实触发面是**数值型 map key**（design R-4）：命中臂由 `HashMap<u8, _>` / `HashMap<f64, _>` 形状的 fixture 经真实 `export` 构造，判据为 `Err` 且目标路径不产生文件，MUST NOT 用向产物注入文本的方式伪造命中。覆盖方式：`src-tauri/src/bindings/zod_test.rs`（`guard_forbidden` 自检臂 + 三表渲染臂 + map-key fixture 臂）+ 产物 grep（桌面端静态核对；vendored crate 本身的用例归属见「不可测试项」） | `packages/desktop/src-tauri/src/bindings/zod.rs`（`guard_forbidden` 自检 / 三表渲染）、`packages/desktop/src-tauri/crates/tools/specta-zod-patched/src/primitives.rs`（P1–P3 补丁，经真实 exporter 断言）、生成物 `bindings.zod.ts` |
| AC-8 | 生成期自检：只 import 产物的桩文件 `tsc --noEmit` 通过（strict 化 / getter 化 / 删别名三面同看）。覆盖方式：桌面端静态自检（`bindings:zod-check` 经 `bindings:export` 串联），记入「不可测试项」 | `packages/desktop/src/types/generated/bindings.zod.check.ts`、`packages/desktop/tsconfig.zod.json`、`packages/desktop/package.json`（`scripts.bindings:zod-check`） |
| AC-9 | 守卫覆盖两份产物且幂等：连导两次零 diff；漏重导 / 手改产物被 `git diff` 抓住。覆盖方式：`src-tauri/src/bindings/mod_test.rs`（双产物存在 / 幂等 / 篡改恢复 / 移除后重建）+ `bindings:check`（桌面端） | `packages/desktop/src-tauri/src/bindings/mod.rs`、两份生成物 |
| AC-10 | 版本 lockstep 与落位：四件套同窗口 + vendored `specta-zod 0.0.4`；`tauri-specta` / `specta-typescript` 不出壳层，core / infra 仍只有 `specta`。覆盖方式：桌面端静态（各 `Cargo.toml` 审查 + `cargo tree` + `PATCH(` 标记清点），记入「不可测试项」 | `packages/desktop/src-tauri/Cargo.toml`、`packages/desktop/src-tauri/crates/tools/specta-zod-patched/{Cargo.toml,README.md,src/**}` |
| AC-11 | 前端 zod 依赖落位合规：`dependencies` 精确 pin `4.6.5`、单实例、`src/lib/ipc.ts` 直接 import、`knip.json` 零新增豁免。覆盖方式：`ipc.test.ts` 真实装载生成物三表即间接验证运行期 zod 可用（单实例 `.meta()` 回读）+ 桌面端静态（`pnpm why zod` / `client:check`），配置面记入「不可测试项」 | `packages/desktop/package.json`、`packages/desktop/src/lib/ipc.ts`（`import … from 'zod'`）、`packages/desktop/knip.json` |
| AC-12 | 门禁全绿、覆盖率 ≥ lines 90 / branches 85 / functions 90、`version` 0.5.31 → 0.5.33、`openspec/config.json` 零改动。覆盖方式：桌面端门禁（`pnpm -C packages/desktop run check`）+ diff 审查，记入「不可测试项」 | `packages/desktop/package.json`（`version`）、`openspec/config.json` |
| AC-13 | 漂移聚合四导出各有非测试消费方：`reportDrift` / `driftReports` / `driftCount` ← `ipc.ts`，`resetDrift` ← `use-workspaces.ts`；knip 零 dead code 且未关判据。覆盖方式：`src/lib/ipc-drift.test.ts` + `src/lib/ipc.test.ts`（DEV 告警）+ `src/hooks/use-workspaces.test.ts`（清空时机）；knip 由 `client:check` 承担 | `packages/desktop/src/lib/ipc-drift.ts`、`packages/desktop/src/lib/ipc.ts`（DEV 汇总告警）、`packages/desktop/src/hooks/use-workspaces.ts`（根变更清空） |

---

## 单元测试

### `packages/desktop/src/lib/ipc-drift.ts` -> `packages/desktop/src/lib/ipc-drift.test.ts`

新建测试文件；纯内存模块，零依赖，四个导出全部直接断言。

#### 待测功能

- `reportDrift(report: IpcDriftReport): void`: 唯一写入口，按 `(cmd, path)` 去重累计 `count`，键数达上限（`MAX_DRIFT_KEYS = 200`）后丢弃**新键**、已有键继续累计。
- `driftReports(): ReadonlyArray<DriftEntry>`: 当前聚合快照（每项含 `count`），插入序。
- `driftCount(): number`: 去重后键数（非总上报次数）。
- `resetDrift(): void`: 清空聚合。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `reportDrift` | 正向 | 首次上报一条 `{cmd, path, expected, got}` → `driftCount()` 为 1，`driftReports()[0]` 的四个字段与入参逐字一致且 `count` 为 1 | 单次 |
| `reportDrift` | 正向 | 同 `(cmd, path)` 连续上报 3 次 → 仍 1 条记录、该条 `count === 3`、`driftCount() === 1` | 参数化迭代（N = 1 / 2 / 3） |
| `reportDrift` | 正向 | 同 `cmd` 不同 `path`、不同 `cmd` 同 `path`、两者皆不同 → 三组各占独立条目，互不合并 | 参数化迭代 |
| `reportDrift` | 边界 | 键分隔防混：`{cmd:"a/b", path:"#/c"}` 与 `{cmd:"a", path:"#/b/0/c"}` 必须为两条（`\u0000` 分隔语义不被内容穿透） | 单次 |
| `reportDrift` | 边界 | `path` 取根 `"#"`、取深层 `"#/a/0/b"`、取超长（>1000 字符）→ 均正常记入且互不冲突 | 参数化迭代 |
| `reportDrift` | 边界 | `got` 为 `undefined` / `null` / `NaN` / 超大对象引用 → 记录保留原值与原引用（不深拷贝、不序列化） | 参数化迭代 |
| `reportDrift` | 边界 | 键数达 200（恰好上限）仍接受；第 201 个新键被丢弃（`driftCount() === 200`）且既有键 `count` 继续累加 | 参数化迭代（200 / 201 双点） |
| `reportDrift` | 异常 | 写入路径零副作用：调用前后 `console`（warn / error / log）零调用、无 IPC、无存储写入 | 单次 |
| `driftReports` | 正向 | 多条上报后快照按上报顺序给出，条目数为去重键数，每项 `count` 与累计次数一致 | 单次 |
| `driftReports` | 边界 | 空聚合 → `[]`（`length === 0`，非 `undefined`） | 单次 |
| `driftReports` | 边界 | 快照语义：修改返回数组（`push` / `length = 0`）不改变后续 `driftReports()` 与 `driftCount()` | 单次 |
| `driftReports` | 异常 | 上限封顶后仍完整返回 200 条（无空洞、无截断，索引连续） | 单次 |
| `driftCount` | 正向 | 去重语义：同键上报 5 次 → 1；再加两处不同键 → 3 | 参数化迭代 |
| `driftCount` | 边界 | 空聚合 → 0；`resetDrift()` 后 → 0；封顶后恒为 200（继续上报新键不变） | 组合迭代（生命周期 × 容量） |
| `driftCount` | 异常 | 幂等读：连续两次调用返回值相同，且不改变聚合（`driftReports()` 长度不变） | 单次 |
| `resetDrift` | 正向 | 有数据时清空 → `driftReports()` 为 `[]`、`driftCount()` 为 0 | 单次 |
| `resetDrift` | 边界 | 已空时再次清空 → 不抛、保持空 | 单次 |
| `resetDrift` | 边界 | 清空后同键再次上报 → `count` 从 1 **重新**累计（不续接旧计数）；不同键 → 无历史残留 | 参数化迭代 |
| `resetDrift` | 异常 | 清空零副作用：不触发 `console`、不重置其它模块状态、不抛 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 不引入任何 mock；`beforeEach` 以真实 `resetDrift()` 复位模块级单例（用例隔离靠清空入口，不靠 `vi.resetModules()`） | 全章节 |
| `console` | `vi.spyOn(console, 'warn' / 'error' / 'log')` 仅观测零调用（不改行为） | collector 保持纯数据（`零 console`）的两条异常臂 |

### `packages/desktop/src/lib/ipc.ts` -> `packages/desktop/src/lib/ipc.test.ts`

新建测试文件。真实组合 `bindings.ts`（typed invoke 包装）、`bindings.zod.ts`（三张表）与 `ipc-drift.ts`；仅 `@tauri-apps/api/core` 一个进程边界替身（沿 `agent-transport.test.ts` 的 `invokeMock` + `ChannelMock` 先例）。5 个 `Channel` 命令为 `agentStart` / `agentSessionWatch` / `changeFlowStart` / `changeFlowWatch` / `watchSubscribe`。

#### 待测功能

- `ipc`: `typeof commands` 形态的分级校验调用面——`Proxy` 按 `commands` 自有键派发（非命令键原样透传、命令键按名缓存包裹函数）；参数面 strict（失败抛**模块私有** `IpcArgsError`，其 `name` / `cmd` / `path` / `expected` / `got` 五面即 AC-1 的可观测面，零 IPC 往返）；结果面与 `Channel` 信封面 soft-report（原值 / 原消息透传，零 throw）；`Channel` 参数按 `channelArgs` 元数据包裹（幂等，`WeakSet` 短路）；DEV 汇总告警消费 `driftCount()` / `driftReports()`；直接 `import 'zod'`（knip 依赖可见性）。`IpcArgsError` 不导出（产品零消费方；`no test-only exports`），只经抛出的错误对象断言。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ipc` 键遍历面 | 正向 | `Object.keys(ipc)` 键集与 `Object.keys(commands)` 完全一致（长度与逐项相等），零手写命令清单 | 单次 |
| `ipc` 键遍历面 | 正向 | 对 `commands` 每个命令键取 `typeof ipc[key] === 'function'`（键遍历自动覆盖新命令，零改 `ipc.ts`） | 参数化迭代（全命令键） |
| `ipc` 键遍历面 | 边界 | 非自有 / 非函数键（`then`、`toString`、`hasOwnProperty`、`Symbol.toStringTag`、`Symbol.iterator`）原样透传：不包裹、不抛、`ipc.then` 为 `undefined`（await 语义不被破坏） | 参数化迭代 |
| `ipc` 键遍历面 | 边界 | 按名缓存：`ipc.addWorkspace === ipc.addWorkspace`（同一引用，React 依赖不抖动）；不同命令引用不同 | 单次 |
| `ipc` 键遍历面 | 异常 | 访问不存在的命令键 → `undefined`，不抛 | 单次 |
| `ipc` 参数面 | 正向 | 合法参数（含 `null` 给 `Option` 形参、`null` 给可空数值形参如 `contextLength` / `maxTokens`）→ 校验通过、`invoke` 恰 1 次、第二实参为**原参数对象**（同一引用，非解析副本） | 参数化迭代（`listChanges` / `agentSessions` / `saveAgentProvider` 三面） |
| `ipc` 参数面 | 正向 | 合法调用零漂移：`driftCount()` 保持 0，无 `IpcArgsError`、无 `console.warn` | 单次 |
| `ipc` 参数面 | 异常 | 数字给 `string` 形参（`listChanges(42 as unknown as string)`）→ 抛 `IpcArgsError`：`instanceof Error`、`name === "IpcArgsError"`、消息含命令名与 `#/…` 路径、`expected` / `got` 非空；`invoke` 调用次数为 0 | 单次 |
| `ipc` 参数面 | 异常 | `null` 给必填 `string` 形参 → `IpcArgsError`，`got === null`、`expected` 描述非空 | 单次 |
| `ipc` 参数面 | 异常 | 少传实参（缺键，如 `agentSessions(root)` 只给首参）→ `IpcArgsError`，`path` 指向首个缺失形参 | 单次 |
| `ipc` 参数面 | 异常 | 嵌套对象多余键（`saveAgentProvider` 的 `models` 传多余子键）→ `IpcArgsError`（`z.strictObject` 的 `unrecognized_keys`，深层同样被拒） | 单次 |
| `ipc` 参数面 | 边界 | 显式 `undefined` 给 `string \| null` 形参 → `IpcArgsError`（只 `null` 可过，`Option` 未用 `.optional()` 放宽） | 单次 |
| `ipc` 参数面 | 边界 | 多传超出形参个数的实参 → 本地不拒（校验输入按 `shape` 键序取位），原参数对象含多余实参进 payload，交由后端判定 | 单次 |
| `ipc` 参数面 | 边界 | 零形参命令（`listWorkspaces` / `appEnvironment`）→ `z.strictObject({})` 放行、`invoke` 恰 1 次 | 参数化迭代 |
| `ipc` 参数面 | 异常 | 参数面失败不产生漂移记录（错误面不混入 `IpcDriftReport`）：`driftCount()` 仍为 0 | 单次 |
| `ipc` 结果面 | 正向 | 干净结果 → 漂移零、返回值与后端返回对象**同一引用** | 单次 |
| `ipc` 结果面 | 正向 | 结果含合法 `unknown`（`serde_json::Value` 面任意 JSON：对象 / 数组 / 字符串 / `null`）与可空字段 `null` → 零漂移、原值返回 | 参数化迭代 |
| `ipc` 结果面 | 异常 | 脏结果（`addedAt` 声明为 number 实为字符串）→ 漂移恰 1 条（`cmd` 正确、`path` 形如 `#/0/addedAt`）+ Promise 正常 resolve + 返回值与脏对象同一引用（零 throw / 零 reject） | 单次 |
| `ipc` 结果面 | 异常 | 多层嵌套脏字段 → 漂移 `path` 逐段命中（`#/a/0/b` 形态） | 单次 |
| `ipc` 结果面 | 边界 | 同 `(cmd, path)` 连续两次脏结果 → `driftCount() === 1` 且该条 `count === 2`（与 collector 去重组合） | 单次 |
| `ipc` 结果面 | 边界 | 同一命令两处不同字段脏 → 两条独立漂移条目 | 单次 |
| `ipc` 结果面 | 边界 | 大结果（千级节点的 `codeStats` / `dbRecords` 面）→ 零漂移且原引用透传（无深拷贝、无重排） | 单次 |
| `ipc` Channel 信封 | 正向 | 合法信封消息投递 → handler 收到原消息（同一引用）、漂移零 | 单次 |
| `ipc` Channel 信封 | 异常 | 脏信封消息 → 漂移恰 1 条 + handler 仍被调用且原消息原样（不丢流、不改内容） | 单次 |
| `ipc` Channel 信封 | 边界 | 序列「合法 → 脏 → 合法」→ handler 三次、调用顺序不变、漂移恰 1 条 | 组合迭代（序列 × 命中判定） |
| `ipc` Channel 信封 | 边界 | 同一 channel 实例对同一命令连续传入两次 → 只包裹一次：每条消息 handler 调用 1 次、同一条脏消息只 1 条漂移（`count` 不翻倍） | 组合迭代（重复传入 × 上报计数） |
| `ipc` Channel 信封 | 边界 | 同一 channel 实例传入两条不同命令（`agentStart` + `agentSessionWatch` 双出口）→ `WeakSet` 短路，不重复包裹、不重复上报 | 组合迭代（跨命令 × 幂等） |
| `ipc` Channel 信封 | 边界 | `onEvent` 键保留在 `invoke` 第二实参内（同一引用），且不参与值校验（传任意替身对象不被本地拒绝） | 单次 |
| `ipc` Channel 信封 | 边界 | handler 未设置时投递 → 不抛；随后 `onmessage = handler` 再投递 → 正常交付且校验生效（`get` 返回调用方 handler 的兼容面） | 单次 |
| `ipc` Channel 信封 | 边界 | 包裹不改变其它可观察行为：`toJSON()` 结果、`id` 与包裹前逐字一致；原型带 accessor 的替身下私有字段仍经原型 setter 写入（`get`/`set` descriptor 两条路径） | 组合迭代（替身形态 × 观测面） |
| `ipc` Channel 信封 | 边界 | Channel 形参位传 `null`（非 Channel 实例）→ 本地不抛（`degraded` 哨兵放行、跳过包裹），错误由后端 reject 面呈现 | 单次 |
| `ipc` Channel 信封 | 异常 | handler 自身抛异常 → 原样向投递方传播（装配层不吞、不转成漂移） | 单次 |
| `ipc` DEV 汇总告警 | 正向 | 首次出现新漂移键 → `console.warn` 恰 1 次，实参含命令名、`path`、`driftCount()` 值与 `driftReports()` 键清单 | 单次 |
| `ipc` DEV 汇总告警 | 边界 | 同一键再次漂移（计数增、键未新增）→ 不重复 warn（每去重键一次，防刷屏） | 单次 |
| `ipc` DEV 汇总告警 | 异常 | `vi.stubEnv('DEV', false)`（实现若为模块级常量则配合 `vi.resetModules()` 后动态 import）→ 漂移照旧记录但零 warn（门只决定诊断输出可见性） | 单次 |
| `ipc` 错误面三分 | 正向 | 后端 reject 字符串 → 调用方收到同一字符串（形态 / 文案 / 时序不变）、零漂移、非 `IpcArgsError` | 单次 |
| `ipc` 错误面三分 | 异常 | 后端 reject 为 `Error` 实例 / 普通对象 → 原样向上传递（不改造、不包装成字符串） | 参数化迭代 |
| `ipc` 错误面三分 | 正向 | `IpcArgsError` 经 `String(err)` 呈现含命令名与 `#/…` 路径（既有 error 态路径零改动） | 单次 |
| `ipc` 错误面三分 | 边界 | 结果面漂移时 Promise 正常 resolve（零 reject）、不抛 `IpcArgsError` | 单次 |
| `ipc` × `ipc-drift` 组合 | 正向 | 脏结果调用 → 装配层上报与 collector 累计一致：`driftReports()` 1 条、其 `count` 等于该键上报次数（跨模块不重复计数） | 单次 |
| `ipc` × `ipc-drift` 组合 | 异常 | collector 达键数上限（200）后继续产生新键 → 装配层不抛、不重试、`driftCount()` 恒为 200（噪声封顶不掩盖既有漂移） | 组合迭代（容量 × 上报路径） |
| `ipc` × `ipc-drift` 组合 | 边界 | `resetDrift()` 后同一脏结果再次调用 → 重新累计 1 条（生命周期与 collector 组合，不续接） | 组合迭代（清空 × 重放） |
| `ipc` × `bindings.zod` 组合 | 正向 | 5 个 `Channel` 命令全部经 `ipc` 调用 → 参数面放行、`channelArgs` 命中、`onEvent` 原样在 payload | 参数化迭代（5 命令） |
| `ipc` × `bindings.zod` 组合 | 异常 | 某命令在 `channelArgs` 中无键（无 Channel 形参的命令）→ 装配层跳过包裹、不抛；信封面零上报（`degraded` 哨兵只落 Channel 形参位） | 参数化迭代（无 Channel 命令） |
| `ipc` × `bindings.zod` 组合 | 正向 | 运行期名称来源（design R-4）：代表性命令的 `invokeMock` 第二实参键集 == `Object.keys(argsSchemas[cmd].shape)`（`listChanges` → `['root']`、`saveAgentProvider` → 9 键），线格式键名与生成表逐字一致（类型层取不到形参名，故这是名称面的运行期判据） | 参数化迭代（两条代表命令） |
| `ipc` × `bindings.zod` 组合 | 边界 | `argsSchemas` 表项为 getter（每次访问重建）→ 连续两次 `ipc.<cmd>()` 与一次 `ipc.<cmd>()` 的参数面判定一致（零状态残留，`driftCount()` 不变） | 组合迭代（getter 重建 × 判定稳定） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`、`Channel`） | `vi.mock` + `vi.hoisted`：`invokeMock` 按命令名分发并记录 `mock.calls`（可切换 resolve / reject / 同步抛）；`ChannelMock` 为可编程 class（`onmessage` 读写 + `id` + `toJSON` 最小面），另备一份**原型带 accessor** 的替身覆盖私有字段写入路径 | 进程边界（Tauri IPC）；`Channel` 为 IPC 值类型，只能在边界替身中构造 |
| `console.warn` | `vi.spyOn(console, 'warn')`，只观测调用次数与实参，不改行为 | DEV 汇总告警（诊断输出可见性） |
| `import.meta.env.DEV` | `vi.stubEnv('DEV', false)`；若实现读模块级常量则先 `vi.resetModules()` 再动态 `await import` | 告警门（运行环境） |
| 不 mock：`bindings.ts`、`bindings.zod.ts`、`ipc-drift.ts` | 真实组合（生成物三表是事实来源，mock 它们等于 mock 契约本身） | 全章节（违反即失去 AC-1/2/3 的判据效力） |

### `packages/desktop/src/types/generated/bindings.zod.ts` -> `packages/desktop/src/lib/ipc-contract.test.ts`

新建测试文件。design「公共函数 / API」表未为该生成物列行（它是生成物，不是手写模块）；本节的 `待测功能` 逐条取自 design「类型定义」表的 `argsSchemas` / `resultSchemas` / `channelArgs` 三行（生成物唯一对外面），等价断言的另一端类型只从 `bindings.ts` 取（类型主权不转移），**不 grep 源文件导出声明**。

两条来自 design 回溯修正的口径（MUST NOT 写成恒真断言）：
- **`char` 产品面零实例**（R-3）：本文件不设 `char` 等价用例；`char → z.string()` 的等价性由 `zod_test.rs` fixture 承担，产品面替代判据是「产物零 `.refine(`」（AC-7 静态）。
- **形参名在类型层不可得**（R-4）：`Parameters<typeof commands[k]>` 只给位置元组，取不到名字。故名称判据只有两处——生成期 `Casing::CamelCase`（`zod_test.rs` fixture：`since_seq → sinceSeq` + 键序）与运行期 payload 键集（`ipc.test.ts`：`invokeMock` 第二实参键集 == `Object.keys(argsSchemas[cmd].shape)`）；本文件只做**名字无关**的位面断言（键数 == `commands[k].length`、代表性命令的键名键序快照、值类型 Equal）。

#### 待测功能

- `argsSchemas`: 逐命令参数 schema（`as const` + 逐项 getter；键名 lowerCamelCase、键序 = Rust 形参序）。
- `resultSchemas`: 逐命令结果 schema（`Result` 取 Ok 臂、`Option` 为 nullable）。
- `channelArgs`: 逐命令 `Channel` 形参键 → 信封 schema。
- 具名类型 schema 段: 全部出线类型的 schema（无注解、无 `export type`、`z.strictObject` 写死）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `resultSchemas` | 正向 | 逐命令 `z.infer<typeof resultSchemas[k]>` 与 `bindings.ts` 的 `Awaited<ReturnType<typeof commands[k]>>` `toEqualTypeOf`（命令键联合全覆盖） | 参数化迭代（全命令键） |
| `resultSchemas` | 边界 | `Option` 面推导含 `\| null`（`sessionDetail` 的「未找到」）；`f64` 面推导为 `number \| null`（两侧同口径，见 proposal「已否决」表）；整数面（`i64` / `u64` / `sinceSeq`）推导为 `number`（非 `bigint`）；`u32` / `f64` 混排的具名 DTO（`CodeTotals`）逐字段 Equal | 参数化迭代（类型面集合） |
| `resultSchemas` | 边界 | literal 枚举面（`AgentEngineKind` / `AgentPermissionMode` / `DiagnosticKind` 等 `z.literal` 联合）与具名 DTO 的联合类型 Equal | 参数化迭代（每个枚举面） |
| `resultSchemas` | 边界 | Intersection 面（`AgentEvent` 的 `flatten` 变体联合）Equal | 单次 |
| `resultSchemas` | 边界 | `unknown` 面（`ArtifactEnvelope.payload` 等 `serde_json::Value` 出线字段）推导为 `unknown` 且 `.not.toBeAny()` | 参数化迭代（12 处白洞代表面） |
| `resultSchemas` | 异常 | 反例校准：推导类型与「去掉 nullable 的窄类型」必须 `not.toEqualTypeOf`（证明 Equal 断言恒真性被排除，`any` 不会静默通过） | 单次 |
| `resultSchemas` | 边界 | `char` **不入产品面**：出线面零 `char` 派生位（回归判据是产物零 `.refine(`，AC-7 静态）；若日后出现 `char` 出线位，该位 `z.infer` 应为 `string`（等价性先由 `zod_test.rs` 的 `map_types` / `map_type` 臂守住） | 单次 |
| `argsSchemas` | 正向 | 逐命令 `Object.keys(argsSchemas[k].shape).length === commands[k].length`（形参个数来自 `bindings.ts`，名字无关面；新增命令零改断言） | 参数化迭代（全命令键） |
| `argsSchemas` | 边界 | 代表性命令的键名与键序快照：`agentSessionWatch` 为 `['onEvent','root','sessionId','sinceSeq']`、`saveAgentProvider` 为 9 键（`id` / `name` / `protocol` / `baseUrl` / `apiKey` / `models` / `contextLength` / `maxTokens` / `webSearch`）；权威名称来源仍是生成期 `Casing::CamelCase`（`zod_test.rs`）与运行期 payload 键集（`ipc.test.ts`） | 参数化迭代（两条代表命令） |
| `argsSchemas` | 边界 | 零形参命令（`listWorkspaces` / `appEnvironment`）推导为空对象类型 `{}`，且 `safeParse({})` 通过、`safeParse(undefined)` 被拒 | 参数化迭代 |
| `argsSchemas` | 异常 | 形参值与类型面不符的形状（如把 `Option` 形参推导成 `.optional()`）→ Equal 断言报错（守住「必填键 + `null` 可过」不被放宽） | 单次 |
| `channelArgs` | 正向 | 5 个 `Channel` 命令的键名集合为 `{onEvent}`，值 schema 的 `z.infer` 与 `bindings.ts` 中该命令首参 `Channel<T>` 的 `T` Equal | 参数化迭代（5 命令 × 信封类型） |
| `channelArgs` | 边界 | 非 `Channel` 命令在 `channelArgs` 中为空对象（表只含 Channel 形参） | 参数化迭代 |
| `channelArgs` | 异常 | 缺键 / 形态非法显性拦截：若某 Channel 命令缺键或值为非 `z.ZodType`（`null` / 普通对象 / 裸 schema 文本），`ipc.ts` 的 `instanceof ZodType` 收窄失败即跳过包裹（不抛、不 panic），该命令脏信封**零上报**；故契约以「5 命令逐键命中 + 值 `instanceof ZodType`」反向守住「缺键即观测失效」的失败模式 | 参数化迭代（5 命令 × 缺失 / 非法两形态） |
| 具名类型 schema 段 | 正向 | 递归类型 `TreeEntry ↔ DirNode` 的 `z.infer` 与该具名类型 `toEqualTypeOf` 且 `.not.toBeAny()`（getter 化后不塌 `any`、无 `TS7022`） | 单次 |
| 具名类型 schema 段 | 边界 | 降级面哨兵可见：若某位置为 `degraded("…")`，其 `z.infer` 为 `unknown`（断言与 `unknown` Equal，MUST NOT 静默产出不等价 schema 且不冒充具体类型） | 单次 |
| 具名类型 schema 段 | 异常 | 递归自引用若回退为 `z.lazy` 初值 → 该断言在 `vp check` typeCheck 报 `TS7022/TS7023`（自检与类型断言双重拦截） | 单次 |
| 三表 runtime 面 | 正向 | `Object.keys(argsSchemas)` / `Object.keys(resultSchemas)` 与 `Object.keys(commands)` 完全一致（逐命令覆盖、新增命令零改装配层） | 单次 |
| 三表 runtime 面 | 边界 | 表项为 getter：`Object.getOwnPropertyDescriptor(argsSchemas, k).get` 非 `undefined`（表构造零成本、跨表引用无初始化期 TDZ） | 参数化迭代（全命令键） |
| 三表 runtime 面 | 异常 | 版本 / 实例错配显性拦截：产物模块加载即调用 `z.strictObject` / `.meta()`——若解析到 zod `3.x` 或第二份 4.x 副本，加载或 `.meta()` 回读会失败（断言加载成功且 `degraded` 哨兵的 `.meta()` 可回读，即 AC-11 的运行期判据） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 全部真实 import：`bindings.zod.ts`（产物）、`bindings.ts`（类型面）、`zod`（真实实例，单实例是 `.meta()` / `instanceof` 可观测的前提） | 全章节 |
| 不 mock `zod` | MUST NOT 用替身替代 zod（跨实例会让 `.meta()` 回读静默失效，观测链路失去意义） | 契约等价断言的判据前提 |

### `packages/desktop/src/types/generated/bindings.ts` -> `packages/desktop/src/lib/ipc-contract.test.ts`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的 API 变更（「类型定义」表标注「零改动（唯一类型源）」）；该产物在类型等价断言中只作对位参照（`Awaited<ReturnType<typeof commands[k]>>`），产物文本断言由既有 `src-tauri/src/bindings/mod_test.rs` 承担回归。 -->

- `commands` / `Bindings`（既有导出，零改动）: 唯一类型源——出线 DTO / provider 类型与逐命令形参面；本相位不新增 / 不修改导出，等价断言只从此处取对位类型。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `commands`（类型面参照） | 正向 | rc.26 重导出后逐命令返回类型与 `resultSchemas` 推导 Equal（既有语义零变更，见 `bindings.zod.ts` 章节首行用例） | 参数化迭代（全命令键） |
| `commands`（类型面参照） | 正向 | 逐命令形参元组 `Parameters<typeof commands[k]>` 与 `ipc` 侧同名命令的形参元组 Equal（机械替换零形态漂移） | 参数化迭代（全命令键） |
| `commands`（类型面参照） | 边界 | 既有 `mod_test.rs` 的命令名清单 / DTO 名单 / 逐字段行断言全绿（产物文本面未被 zod 产物污染） | 单次 |
| `commands`（类型面参照） | 异常 | 若 rc.26 引入语义差异 → 上述 Equal 断言报错 + `bindings:check` 报 `git diff`（升级噪声显式呈现，不静默入库） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯类型引用（`import type`），零运行期替身 | 全章节 |

### `packages/desktop/src/hooks/use-workspaces.ts` -> `packages/desktop/src/hooks/use-workspaces.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发 + `toastErrorMock`，沿本文件既有装置）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（既有导出 `useWorkspaces` 与 `WorkspaceState` 形态不变）；行为增补取自 design D12 与「修改文件」表：`applyRoot` 写口内「根变更才 `resetDrift()`」，是 `resetDrift` 的非测试消费方（AC-13 / spec「清空入口有产品消费方」）。 -->

- `useWorkspaces`（既有导出）: 调用面 `commands.` → `ipc.`（`listWorkspaces` / `addWorkspace` / `removeWorkspace`）；`applyRoot` 判变时 `resetDrift()`——同根重取数 MUST NOT 清空。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useWorkspaces`（根变更清空） | 正向 | `select` 切到不同根 → 清空生效：切换前先 `reportDrift` 造 1 条，切换后 `driftCount() === 0` | 单次 |
| `useWorkspaces`（根变更清空） | 正向 | `add` 成功且返回记录的 canonical root ≠ 当前根（新记录未必居首）→ 清空生效（`driftCount()` 归零） | 单次 |
| `useWorkspaces`（根变更清空） | 正向 | `remove` 当前根后取数回落至另一根 → 根变更 → 清空生效 | 单次 |
| `useWorkspaces`（根变更清空） | 边界 | 同一根重取数（`list_workspaces` 仍返回当前根）→ **不清空**（`driftCount()` 保持 1，spec 明令「同根重取数 MUST NOT 清空」） | 单次 |
| `useWorkspaces`（根变更清空） | 边界 | 挂载首取：原根 `null` → 新根 X（判变）→ 清空 1 次；清单空回落 `null`（原根非 `null`）→ 判变 → 清空；原根 `null` → `null` → 零清空 | 参数化迭代（三种回落形态） |
| `useWorkspaces`（根变更清空） | 异常 | `list_workspaces` reject → 根不变 → 零清空、`error` 态照旧（既有双轨语义不退化）、零漂移 | 单次 |
| `useWorkspaces`（调用面：参数面 / 结果面） | 正向 | `addWorkspace(root)` / `removeWorkspace(root)` 经 `ipc` 调用：`root` 字符串放行、`invoke` 命令名与 payload 键逐字不变 | 参数化迭代（两命令） |
| `useWorkspaces`（调用面：参数面 / 结果面） | 正向 | 清单取数 / add / remove / select 既有断言全绿（仅 `commands.` → `ipc.` 机械替换，命令名不变） | 单次 |
| `useWorkspaces`（调用面：参数面 / 结果面） | 异常 | `addWorkspace` 传非字符串（`42 as unknown as string`）→ 本地 `IpcArgsError`、`invoke` 零调用、既有 `String(err)` 呈现不变、漂移零记录 | 单次 |
| `useWorkspaces`（调用面：参数面 / 结果面） | 边界 | 清单空 `[]` → `listWorkspaces` 结果面放行、`root` 回落 `null`、零漂移 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` 按命令名分发（本变更不新增替身面） | 进程边界 |
| `sonner`（`toast`） | 沿用既有 `toastErrorMock` | UI 边界（既有先例） |
| 不 mock `./ipc-drift` | 用**真实 collector**：先用 `reportDrift` 造数、再断言 `driftCount()`，以行为差判清空时机（避免断言替身调用次数这类「测 mock」形态） | 清空时机验证 |
| 不 mock `../lib/ipc` / 生成物三表 | 真实组合（参数面 / 结果面校验是本次被测行为，替身化即绕开 AC-1 / AC-5） | 参数面与回归臂 |

### `packages/desktop/src/lib/agent-transport.ts` -> `packages/desktop/src/lib/agent-transport.test.ts`

既有测试文件扩展（`invokeMock` + `ChannelMock` 沿既有装置）。本文件是「业务调用点 → ipc → channelArgs / ipc-drift」链路的**最上层发起方**（`agentStart` / `agentSessionWatch` 双出口 + `sessionDetail`），跨模块组合用例挂靠于此。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`TauriAgentTransport` / `TauriAgentTransportOptions` 形态与返回域不变）；变更取自「修改文件」表：`commands.` → `ipc.` 机械替换（`sessionDetail` / `agentSessionWatch` / `agentStart`），类型派生源改 `Parameters<typeof ipc.agentStart>`（类型仍出自 `bindings.ts`）。 -->

- `TauriAgentTransport`（既有导出）: 三条命令经 `ipc` 调用；`agentStart` / `agentSessionWatch` 的 `Channel<AgentRunMessage>` 首参原样在 payload、信封逐条 soft-report。
- `AgentStartArgs`（类型派生，非导出）: 形参元组派生自 `ipc.agentStart`，与 `commands.agentStart` 逐位 Equal。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `agentStart` ↔ `agentSessionWatch` 双出口（组合，链路：transport → ipc → channelArgs → ipc-drift） | 正向 | 同一 `Channel` 实例先后传给两条命令、各投递合法信封 → handler 每次收到原消息（同一引用）、`driftCount()` 为 0 | 单次 |
| `agentStart` ↔ `agentSessionWatch` 双出口（组合，链路：transport → ipc → channelArgs → ipc-drift） | 异常 | 同一脏信封经第二条命令再次投递 → 仍只 1 条漂移（`count` 不翻倍）+ handler 每次均被调用（不丢流） | 组合迭代（双出口 × 上报计数） |
| `agentStart` ↔ `agentSessionWatch` 双出口（组合，链路：transport → ipc → channelArgs → ipc-drift） | 边界 | 幂等包裹：同一 `Channel` 实例被重复传入 → 只包裹一次：同一消息 handler 调用 1 次、同一条脏消息只 1 条漂移 | 组合迭代（双出口 × 幂等 × 上报计数） |
| `AgentStartArgs` 类型派生（组合，链路：transport → ipc → bindings.ts 类型面） | 正向 | `Parameters<typeof ipc.agentStart>` 与 `Parameters<typeof commands.agentStart>` 类型 Equal（位置面零漂移，类型仍出自 `bindings.ts`） | 单次 |
| `AgentStartArgs` 类型派生（组合，链路：transport → ipc → bindings.ts 类型面） | 异常 | 反例校准：把派生源换成手写窄类型（如 `agent: number` 而非 `number \| null`）→ Equal 断言报错（守住「零手写镜像」不被放宽） | 单次 |
| `AgentStartArgs` 类型派生（组合，链路：transport → ipc → bindings.ts 类型面） | 边界 | 9 个位置逐位 Equal（含尾部可空位 `cwd`），元组长度与 `commands.agentStart` 一致 | 单次 |
| `agentStart` 参数面（链路：transport → ipc → argsSchemas） | 正向 | 合法 `agentStart(onEvent, root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd)` 调用（含 `null` 给四个可空位）→ 校验通过、`invoke("agent_start", …)` 恰 1 次、`onEvent` 在 payload 内同一引用 | 单次 |
| `agentStart` 参数面（链路：transport → ipc → argsSchemas） | 异常 | `prompt` 传非字符串（`null`）→ `IpcArgsError`、`invoke` 零调用（零往返）；transport 不把内部异常改写成 reject 串 | 单次 |
| `agentStart` 参数面（链路：transport → ipc → argsSchemas） | 边界 | 四个可空位全给 `null`（`sessionId` / `source` / `sourceRef` / `agent` / `cwd`）→ 放行；`permissionMode` 取三枚举值全量 → 放行（enum 边界） | 参数化迭代（枚举全值 × 可空位） |
| `sessionDetail` + 信封（组合，链路：transport → ipc → resultSchemas / channelArgs） | 正向 | `sessionDetail` 干净结果 → 零漂移、返回值与后端返回对象同一引用（既有解析分支不变） | 单次 |
| `sessionDetail` + 信封（组合，链路：transport → ipc → resultSchemas / channelArgs） | 异常 | `sessionDetail` reject 串经既有错误路径呈现（`String(err)`），不被漂移上报吞掉、不产生 `IpcArgsError` | 单次 |
| `sessionDetail` + 信封（组合，链路：transport → ipc → resultSchemas / channelArgs） | 边界 | `sessionDetail` 结果 `null`（未找到）→ 零漂移、既有 transport 分支语义不变 | 单次 |
| 既有流式路径（组合，链路：transport → ipc → 结果面） | 正向 | 干净信封序列投递 → handler 调用次数与顺序不变、既有 chunk 序列逐项相同、零漂移 | 组合迭代（序列 × 命中判定） |
| 既有流式路径（组合，链路：transport → ipc → 结果面） | 异常 | 脏信封消息（缺 `kind`）→ 1 条漂移、handler 仍收到原消息、流不断（既有 chunk 序列不变） | 单次 |
| 既有流式路径（组合，链路：transport → ipc → 结果面） | 边界 | 恒等空序列 / handler 未设置时投递 → 不抛、零漂移（兼容面不因包裹而收紧） | 参数化迭代（空序列 / 无 handler） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | 沿用既有 `invokeMock` + `ChannelMock`（`vi.hoisted` 可编程 class） | 进程边界 |
| 不 mock `./ipc` / `./ipc-drift` / 生成物 | 真实组合（`Channel` 幂等标记活在 `ipc.ts` 的模块级 `WeakSet`，替身化即绕开被测对象） | 组合用例 |

### `packages/desktop/src/hooks/use-app-environment.ts` -> `packages/desktop/src/hooks/use-app-environment.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发 + `vi.spyOn(console, 'error')`，沿本文件既有装置）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useAppEnvironment` 导出形态与「未就绪 / 失败一律 null」返回域不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`appEnvironment`），命令名与结果形态零变更。 -->

- `useAppEnvironment`（无 design 声明的公共 API 变更条目）: `appEnvironment`（零形参命令）经 `ipc` 调用后的参数面放行与结果面 soft-report。
- 既有「失败静默为 `null`」语义: reject 与契约外结果均不向上抛、不新增二次兜底（本次零改动面）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAppEnvironment` | 正向 | 挂载 → `invoke("app_environment")` 恰 1 次、载荷合法 → 返回环境对象；`driftCount() === 0` | 单次 |
| `useAppEnvironment` | 正向 | 零形参命令参数面放行（`argsSchemas.appEnvironment` 为 `z.strictObject({})`）→ 无 `IpcArgsError`、payload 为空对象（非 `undefined`） | 单次 |
| `useAppEnvironment` | 异常 | reject 串 → 返回 `null`、`console.error` 零调用（既有静默语义）、零漂移、无未捕获异常 | 单次 |
| `useAppEnvironment` | 异常 | 结果面契约外（`types` 字段给数字）→ 漂移记 1 条 + 既有 `toEnvironment` 收窄仍返回 `null`（不新增二次兜底） | 单次 |
| `useAppEnvironment` | 边界 | 结果面多余键（strict 的 `unrecognized_keys`）→ 漂移 1 条、载荷仍按既有字段收窄（软失败不阻断 UI） | 单次 |
| `useAppEnvironment` | 边界 | 卸载前 resolve（`cancelled` 分支）→ 零状态写入、零漂移 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock`（`vi.hoisted` + 按命令名分发，可切 resolve / reject） | 进程边界 |
| `console.error` | 既有 `vi.spyOn` 观测零调用，不改行为 | 失败静默语义 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合（参数面 / 结果面校验即被测行为） | 全章节 |

### `packages/desktop/src/hooks/use-agent-session.ts` -> `packages/desktop/src/hooks/use-agent-session.test.ts`

既有测试文件扩展（`invokeMock` + 既有 `ChannelMock` 装置保留不变；同目录 `use-agent-session.throttle.test.tsx` 一并回归）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useAgentSession` 与 `AgentSessionState` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`sessionDetail` / `agentSessionTranscript` / `agentStop`）。 -->

- `useAgentSession`（无 design 声明的公共 API 变更条目）: `sessionDetail` / `agentSessionTranscript` / `agentStop` 三条命令经 `ipc` 调用后的参数面放行与结果面 soft-report。
- 既有 error 态文案: reject 串经 `String(err)` 呈现，逐字不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentSession` | 正向 | 恢复会话 → `sessionDetail` 与 `agentSessionTranscript` 各 1 次、参数面放行、结果干净零漂移 | 单次 |
| `useAgentSession` | 正向 | `agentStop(root, sessionId)` 参数面放行、payload 键名 `root` / `sessionId` 逐字不变；结果 `null` 零漂移 | 单次 |
| `useAgentSession` | 异常 | `sessionDetail` reject 串 → 既有 error 态文案（`String(err)`）逐字不变、零漂移、零 `IpcArgsError` | 单次 |
| `useAgentSession` | 异常 | 转录结果面脏（事件 `seq` 给字符串）→ 漂移恰 1 条 + 既有重放逻辑按原值继续（零 throw、零 reject） | 单次 |
| `useAgentSession` | 边界 | `root` / `sessionId` 为 `null` → 零 invoke、零漂移（既有前置检查与取消语义不变） | 参数化迭代（两个 null 位置） |
| `useAgentSession` | 边界 | 转录空数组 `[]` → 结果面放行、零漂移、既有空态不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | 沿用既有 `invokeMock`（按命令名分发）+ 既有 `ChannelMock`（本变更零新增替身面） | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/agent/hooks/use-agent-options.ts` -> `packages/desktop/src/views/agent/hooks/use-agent-options.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useAgentOptions` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`listAgentInstances`）。 -->

- `useAgentOptions`（无 design 声明的公共 API 变更条目）: `listAgentInstances`（零形参命令）经 `ipc` 调用的参数面 / 结果面行为。
- 既有选项派生: 结果条目 `providerId: null` 与空清单下的选项生成逻辑不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentOptions` | 正向 | 挂载 → `invoke("list_agent_instances")` 恰 1 次、零形参放行、结果干净零漂移 | 单次 |
| `useAgentOptions` | 正向 | 结果条目 `providerId: null`（`Option` 面）→ 零漂移、选项照旧生成 | 单次 |
| `useAgentOptions` | 异常 | reject 串 → 既有空选项 / error 语义逐字不变、零漂移 | 单次 |
| `useAgentOptions` | 异常 | 结果面契约外（`engine` 给非 literal 枚举值）→ 漂移恰 1 条 + 既有过滤逻辑按原值处理（不 crash） | 单次 |
| `useAgentOptions` | 边界 | 空清单 `[]` → 放行、零漂移、空态不变 | 单次 |
| `useAgentOptions` | 边界 | 结果面多余键（strict）→ 漂移 1 条、选项渲染不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` -> `packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useAgentRunHistory` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`agentSessions`，5 形参含两个可空位）。 -->

- `useAgentRunHistory`（无 design 声明的公共 API 变更条目）: `agentSessions(root, source, sourceRef, offset, limit)` 经 `ipc` 调用的参数面放行（`null` 可过、整数位）。
- 结果面 soft-report: 分页结果契约外字段只上报并原值透传，列表渲染不崩。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentRunHistory` | 正向 | `agentSessions(root, null, null, 0, limit)` → 两个 `null` 可空位放行、payload 键名 `sourceRef` 等 lowerCamelCase 逐字命中、结果干净零漂移 | 单次 |
| `useAgentRunHistory` | 正向 | 分页位为整数（`Number.isInteger(offset)` 与 `Number.isInteger(limit)`）→ 无 `IpcArgsError`、`invoke` 恰 1 次 | 单次 |
| `useAgentRunHistory` | 异常 | reject 串（分页失败）→ 既有 error / 空态语义逐字不变、零漂移 | 单次 |
| `useAgentRunHistory` | 异常 | 结果面脏（`total` 给字符串）→ 漂移恰 1 条 + 列表渲染不崩（原值透传） | 单次 |
| `useAgentRunHistory` | 边界 | `offset = 0` 与 `limit` 取上限值 → 均放行、零漂移 | 参数化迭代（0 / 上限） |
| `useAgentRunHistory` | 边界 | 空页（`runs: []`、`total: 0`）→ 零漂移、既有空态判据不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/agents/hooks/use-agent-instances.ts` -> `packages/desktop/src/views/agents/hooks/use-agent-instances.test.ts`

既有测试文件扩展（`invokeMock` + `toastErrorMock` 沿既有装置；本文件 4 处调用点）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useAgentInstances` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`listAgentInstances` / `saveAgentInstance` / `deleteAgentInstance` / `setDefaultAgentInstance`）。 -->

- `useAgentInstances`（无 design 声明的公共 API 变更条目）: `listAgentInstances` / `saveAgentInstance` / `deleteAgentInstance` / `setDefaultAgentInstance` 四命令经 `ipc` 调用的参数面放行（`id` / `providerId` 的 `null` 可过）。
- 后端拒面呈现: reject 串经既有 `toast.error`（固定前缀 + `String(err)`）呈现，语义不变。
- 结果面 soft-report: 清单条目契约外字段只上报、不阻断既有清单更新。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentInstances` | 正向 | 新建 `saveAgentInstance(null, name, 'sdk', null)` → 两个 `null` 可空位放行、`invoke("save_agent_instance", …)` 恰 1 次 | 单次 |
| `useAgentInstances` | 正向 | 更新（`id` 为数字）+ `setDefaultAgentInstance` / `deleteAgentInstance` → 参数面放行、结果干净零漂移 | 参数化迭代（3 命令） |
| `useAgentInstances` | 异常 | 后端 reject（store 校验串：悬空 provider / cli 引擎不可改 / 默认标记仅 sdk）→ 既有 `toast.error` 固定前缀 + `String(err)` 文案不变、零漂移、零 `IpcArgsError` | 参数化迭代（三条 reject 文案） |
| `useAgentInstances` | 异常 | 结果面契约外（record 缺 `engine`）→ 漂移恰 1 条 + 既有清单更新逻辑不崩（原值透传） | 单次 |
| `useAgentInstances` | 边界 | 空清单 `[]` → 放行、零漂移、既有空态不变 | 单次 |
| `useAgentInstances` | 边界 | `engine: 'cli'` 保存 → 前端参数面**放行**（拒面单点在 store，MUST NOT 本地预拒）、后端 reject 经既有 toast 呈现 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock`（按命令名分发，可切 resolve / reject） | 进程边界 |
| `sonner`（`toast`） | 沿用既有 `toastErrorMock` | UI 边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` -> `packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts`

既有测试文件扩展（`invokeMock` + `toastErrorMock` 沿既有装置；本文件 3 处调用点）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useAgentProviders` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`listAgentProviders` / `saveAgentProvider` / `deleteAgentProvider`），其中 `saveAgentProvider` 是 9 形参 + 嵌套 `models` 的最宽参数面。 -->

- `useAgentProviders`（无 design 声明的公共 API 变更条目）: `saveAgentProvider` 的 9 形参与嵌套 `models` strict 面经 `ipc` 校验（本地零往返）。
- 三条命令的结果面 soft-report: `listAgentProviders` / `saveAgentProvider` / `deleteAgentProvider` 均只上报、不阻断既有流程。
- 后端保存期校验 reject 的既有文案: 模型名非法 / `maxTokens` 低于下限 / api_key 结构外串三条路径文案不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentProviders` | 正向 | 9 形参齐全、`id` / `contextLength` / `maxTokens` 为 `null` → 放行；payload 键名 `baseUrl` / `apiKey` / `models` / `webSearch` 逐字命中、`invoke` 恰 1 次 | 单次 |
| `useAgentProviders` | 正向 | 嵌套 `models` 三档（`high` / `medium` / `low`）齐全 → strict 深层放行、结果干净零漂移 | 单次 |
| `useAgentProviders` | 异常 | 后端 reject（模型名非法 / `maxTokens` 低于下限 / api_key 结构外串）→ 既有 toast 文案不变、零漂移、零 `IpcArgsError` | 参数化迭代（三条 reject 文案） |
| `useAgentProviders` | 异常 | `models` 多传未知子键 → 本地 `IpcArgsError`、`invoke` 零调用（strict 深层，零往返） | 单次 |
| `useAgentProviders` | 边界 | `webSearch` 取 `true` / `false` → 双值放行（bool 全枚举）；`contextLength` / `maxTokens` 取 `0` → 前端放行（下限校验单点在 Rust） | 组合迭代（bool 全值 × 数值下界） |
| `useAgentProviders` | 边界 | 空 provider 清单 `[]` → 放行、零漂移、既有空态不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| `sonner`（`toast`） | 沿用既有 `toastErrorMock` | UI 边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合（嵌套 strict 面即被测行为） | 全章节 |

### `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` -> `packages/desktop/src/views/changes/agent/hooks/use-change-chat.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useChangeChat` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`listChangeChatSessions`，4 形参分页面）。 -->

- `useChangeChat`（无 design 声明的公共 API 变更条目）: `listChangeChatSessions(root, changeId, offset, limit)` 的参数面放行与结果面 soft-report。
- 既有分页累积语义: 增量取数与 `hasMore` 判定不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useChangeChat` | 正向 | 首取 `listChangeChatSessions(root, changeId, 0, PAGE_SIZE)` → 放行、payload 键名 `changeId` 逐字命中、结果干净零漂移 | 单次 |
| `useChangeChat` | 正向 | 结果含 `null` 可空字段 → 零漂移、既有消息列表映射不变 | 单次 |
| `useChangeChat` | 异常 | reject 串 → 既有 error 态文案逐字不变、零漂移、无未捕获异常 | 单次 |
| `useChangeChat` | 异常 | 结果面脏（`sessions` 元素缺字段）→ 漂移恰 1 条 + 既有分页合并不崩 | 单次 |
| `useChangeChat` | 边界 | 空页（`sessions: []`）→ 零漂移、既有 `hasMore` 语义不变 | 单次 |
| `useChangeChat` | 边界 | `offset` 递增连续两次取数（0 → PAGE_SIZE）→ 两页均零误拒、漂移保持 0 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/changes/components/change-create-dialog.tsx` -> `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx`

既有组件测试扩展（`invokeMock` 按命令名分发；沿用既有 `render` + 用户交互装置）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该组件的公共 API 变更（`ChangeCreateDialog` props 与回调形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`createChange`，4 形参）。 -->

- `ChangeCreateDialog`（无 design 声明的公共 API 变更条目）: 提交路径 `createChange(root, name, goal, workflowType)` 的参数面放行与结果面 soft-report。
- 既有表单前置校验与成功路径: 空白名 / 空 goal 的本地拦截、成功后关闭与回调行为不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeCreateDialog` | 正向 | 合法表单提交 → `invoke("create_change", …)` 恰 1 次、参数面放行、既有成功后关闭与回调行为不变、零漂移 | 单次 |
| `ChangeCreateDialog` | 正向 | `workflowType` 取全部枚举值提交 → 逐值放行、零 `IpcArgsError` | 参数化迭代（枚举全值） |
| `ChangeCreateDialog` | 异常 | 后端 reject（重名 / 非法名）→ 既有错误展示（`String(err)`）文案不变、弹窗不关闭、零漂移 | 单次 |
| `ChangeCreateDialog` | 异常 | 结果面契约外 → 漂移恰 1 条、既有成功回调照旧（漂移不阻断流程） | 单次 |
| `ChangeCreateDialog` | 边界 | `name` 为空白 / `goal` 为空串 → 既有表单前置校验拦截、零 `invoke`、零 `IpcArgsError` | 参数化迭代（两字段） |
| `ChangeCreateDialog` | 边界 | `goal` 超长（>1000 字符）→ 参数面放行（schema 只表达接受面、零 `.refine()`）、`invoke` 恰 1 次 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock`（按命令名分发，可切 resolve / reject） | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/changes/hooks/use-change-detail.ts` -> `packages/desktop/src/views/changes/hooks/use-change-detail.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useChangeDetail` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`getChangeDetail` / `readArtifact`），其中产物信封承载 `payload: unknown` 白洞面。 -->

- `useChangeDetail`（无 design 声明的公共 API 变更条目）: `getChangeDetail` / `readArtifact` 两命令的参数面放行与结果面 soft-report（合法 `unknown` 面必须零漂移）。
- 契约内 `null` 结果与既有降级渲染分支: 「未找到」空态判据不变、漂移不阻断 UI。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useChangeDetail` | 正向 | `getChangeDetail(root, id)` + `readArtifact(root, id, kind, source)` 各 1 次、参数面放行、结果干净零漂移 | 单次 |
| `useChangeDetail` | 正向 | `payload: unknown` 给任意 JSON（对象 / 数组 / `null` / 字符串）→ 零漂移（合法 `unknown` 不隐身、不误报） | 参数化迭代（4 形态） |
| `useChangeDetail` | 异常 | reject 串（store 读失败）→ 既有 error 态文案逐字不变、零漂移、零 `IpcArgsError` | 单次 |
| `useChangeDetail` | 异常 | 结果面脏（`detail` 内字段类型漂移）→ 漂移恰 1 条 + 既有 renderer 降级分支照旧（零 throw） | 单次 |
| `useChangeDetail` | 边界 | 结果为 `null`（契约内「未找到」）→ 零漂移、既有空态判据不变 | 单次 |
| `useChangeDetail` | 边界 | `fallbackText: null` → 零漂移（`Option` 面 `null` 可过）；`version` 取 `0` → 零漂移 | 参数化迭代（两字段） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合（`unknown` 面判定即被测行为） | 全章节 |

### `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` -> `packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts`

既有测试文件扩展（`invokeMock` + `ChannelMock` 沿既有装置）。本文件缓存 channel 复用（`ensureChannel`）是 AC-3 幂等面的实际来源之一。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useChangeFlowRun` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`changeFlowWatch` / `changeFlowStart` / `changeFlowStop` / `changeFlowConfirm` / `changeFlowAnswer`，前两条首参为 `Channel<RunNotice>`）。 -->

- `useChangeFlowRun`（无 design 声明的公共 API 变更条目）: `changeFlowWatch` / `changeFlowStart`（首参 `Channel<RunNotice>`）、`changeFlowStop` / `changeFlowConfirm` / `changeFlowAnswer` 五命令经 `ipc` 调用的参数面放行。
- 信封逐条 soft-report: 脏信封只上报一条、`onNotice` 仍被调用（不丢流、不改调用序）。
- 幂等包裹: 缓存 channel 重复传入只包裹一次、同一条脏消息只上报一条（AC-3）。
- 既有 `error` 态文案: reject 串经 `readError` 呈现，逐字不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useChangeFlowRun` | 正向 | `changeFlowStart(channel, root, id, autoNextPhase)` / `changeFlowWatch(channel, root, id)` → `onEvent` 留在 payload（同一引用）、不参与值校验、合法信封零漂移 | 参数化迭代（两命令） |
| `useChangeFlowRun` | 正向 | `changeFlowConfirm(root, id, proceed, syncSpecs)` 两 bool 全组合 + `changeFlowAnswer(root, id, text)` → 放行、各 `invoke` 1 次、零漂移 | 参数化迭代（2×2 + 1） |
| `useChangeFlowRun` | 异常 | 脏信封（`ipc` 载荷缺字段）→ 漂移恰 1 条 + `onNotice` 仍被调用（不丢流、不改调用序） | 单次 |
| `useChangeFlowRun` | 异常 | reject 串 → 既有 `error` 态（`readError`）文案逐字不变、零漂移、无未捕获异常 | 参数化迭代（四动作命令） |
| `useChangeFlowRun` | 边界 | 缓存 channel 重复传入（重挂补订路径）→ `WeakSet` 幂等：只包裹一次、同一条脏消息只 1 条漂移（`count` 不翻倍） | 组合迭代（重复传入 × 上报计数） |
| `useChangeFlowRun` | 边界 | `root` / `id` 为 `null` → 零 invoke、零漂移（既有前置检查与 `Promise.resolve()` 语义不变） | 参数化迭代（两个 null 位置） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | 沿用既有 `invokeMock` + `ChannelMock`（`vi.hoisted` 可编程 class，可投递任意信封） | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合（幂等标记活在 `ipc.ts` 的 `WeakSet`，替身化即绕开被测对象） | 信封面与幂等臂 |

### `packages/desktop/src/views/changes/hooks/use-change-list.ts` -> `packages/desktop/src/views/changes/hooks/use-change-list.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useChangeList` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`listChanges`）。 -->

- `useChangeList`（无 design 声明的公共 API 变更条目）: `listChanges(root)` 经 `ipc` 调用的参数面放行与结果面 soft-report。
- 既有按月分组渲染: 空清单与契约外字段下的分组逻辑不崩、空态判据不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useChangeList` | 正向 | `listChanges(root)` → 放行、结果干净零漂移、既有分组渲染不变 | 单次 |
| `useChangeList` | 正向 | 空清单（`active: []` + `archive: []`）→ 零漂移、既有空态不变 | 单次 |
| `useChangeList` | 异常 | reject 串 → 既有 error 态文案逐字不变、零漂移 | 单次 |
| `useChangeList` | 异常 | 结果面脏（`createdAt` 非 number）→ 漂移恰 1 条 + 分组逻辑不崩（原值透传） | 单次 |
| `useChangeList` | 边界 | 结果面多余键（strict）→ 漂移 1 条、渲染不变（R-3 已知噪音面） | 单次 |
| `useChangeList` | 边界 | `root` 连续切换两次取数 → 各自零误拒、漂移保持 0 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/changes/hooks/use-exec-output.ts` -> `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useExecOutput` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`readExecOutput`，结果面为 `string | null`）。 -->

- `useExecOutput`（无 design 声明的公共 API 变更条目）: `readExecOutput(root, changeName, execId)` 的参数面放行与 `string | null` 结果面的零误报。
- 既有空态与 error 态: `null` / 空串 / reject 三条既有分支的文案与判据不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useExecOutput` | 正向 | `readExecOutput(root, changeName, execId)` → 放行、字符串结果零漂移、原值透传 | 单次 |
| `useExecOutput` | 正向 | 结果 `null`（无记录）→ 零漂移、既有空态不变 | 单次 |
| `useExecOutput` | 异常 | reject 串 → 既有 error 态文案逐字不变、零漂移 | 单次 |
| `useExecOutput` | 异常 | 结果面类型不符（返回对象而非 `string \| null`）→ 漂移恰 1 条、既有展示不崩 | 单次 |
| `useExecOutput` | 边界 | 超长输出（>1000 字符，含 `\n` 与 emoji）→ 零漂移、原值透传（无截断） | 参数化迭代（换行 / emoji 两形态） |
| `useExecOutput` | 边界 | 空串输出 `""` → 零漂移、既有空态判据不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/config/hooks/use-workspace-config.ts` -> `packages/desktop/src/views/config/hooks/use-workspace-config.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useWorkspaceConfig` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`workspaceConfig`），配置报告面含 `extra` 动态键与阈值整数。 -->

- `useWorkspaceConfig`（无 design 声明的公共 API 变更条目）: `workspaceConfig(root)` 的参数面放行与结果面 soft-report。
- `extra` 动态键面: 既有未知键载荷必须真实放行（strict 不得误拒动态键面）。
- 配置 schema 零改动前提下的零误报: 阈值整数与可空字段的边界取值不产生漂移。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useWorkspaceConfig` | 正向 | `workspaceConfig(root)` → 放行、结果干净零漂移、既有表单回填不变 | 单次 |
| `useWorkspaceConfig` | 正向 | 结果含 `extra` 动态键（既有未知键载荷）与 `null` 可空字段 → 零漂移（动态键面不被 strict 误拒） | 参数化迭代（`extra` 空 / 多键两形态） |
| `useWorkspaceConfig` | 异常 | reject 串 → 既有 error 态文案逐字不变、零漂移 | 单次 |
| `useWorkspaceConfig` | 异常 | 结果面脏（`tests` 字段类型漂移）→ 漂移恰 1 条、配置表单不崩 | 单次 |
| `useWorkspaceConfig` | 边界 | 阈值字段取 `0` 与上限值 → 零漂移（int 边界两侧） | 参数化迭代（下界 / 上界） |
| `useWorkspaceConfig` | 边界 | 结果面顶层多余键 → 漂移 1 条（strict 生效，正是 R-3 的已知噪音面）、渲染不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合（`extra` 动态键面判定即被测行为） | 全章节 |

### `packages/desktop/src/views/db/hooks/use-db-inspector.ts` -> `packages/desktop/src/views/db/hooks/use-db-inspector.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发；本文件 2 处调用点）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useDbInspector` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`dbModels` / `dbRecords`）。 -->

- `useDbInspector`（无 design 声明的公共 API 变更条目）: `dbModels(scope, root)` / `dbRecords(scope, root, selected, offset, limit)` 两命令参数面放行（`selected` 可空、分页整数）。
- 大页结果面的零漂移与原引用透传: 无深拷贝、无重排，既有网格渲染不崩。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useDbInspector` | 正向 | `dbModels(scope, root)` / `dbRecords(scope, root, selected, 0, PAGE_SIZE)` → 放行、各 `invoke` 1 次、结果干净零漂移 | 参数化迭代（两命令） |
| `useDbInspector` | 正向 | `selected` 取 `null` 与具体模型名 → 双形态放行（`Option` 面） | 参数化迭代（2 值） |
| `useDbInspector` | 异常 | reject 串（开库失败 / 未知表）→ 既有 error 态文案逐字不变、零漂移 | 单次 |
| `useDbInspector` | 异常 | 结果面脏（行值为契约外类型）→ 漂移恰 1 条 + 既有网格渲染不崩 | 单次 |
| `useDbInspector` | 边界 | 空结果（`rows: []`、`total: 0`）→ 零漂移、既有空态不变 | 单次 |
| `useDbInspector` | 边界 | 大页（`PAGE_SIZE` 上限、千级行）→ 零漂移 + 原引用透传（无深拷贝、无重排） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` -> `packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx`

既有组件测试扩展（`invokeMock` 按命令名分发；本组件 3 处调用点）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该组件的公共 API 变更（`ExploreCreateDialog` props 与回调形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`scanExplores` / `createExploreRecord`）。 -->

- `ExploreCreateDialog`（无 design 声明的公共 API 变更条目）: `scanExplores(root)` 与 `createExploreRecord(root, name | null)` 的参数面放行（第二形参可空）与结果面 soft-report。
- 既有错误回显: 已存在 / 非法名的 reject 串经既有 `onError` 呈现，文案不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ExploreCreateDialog` | 正向 | 打开扫描 → `invoke("scan_explores", …)` 1 次、参数面放行、候选渲染不变、零漂移 | 单次 |
| `ExploreCreateDialog` | 正向 | `createExploreRecord(root, null)`（既定项）与 `(root, name)`（手填名）双形态 → 均放行、`null` 可过 | 参数化迭代（2 形态） |
| `ExploreCreateDialog` | 异常 | reject 串（已存在 / 非法名）→ 既有 `onError` / error 文案逐字不变、零漂移 | 单次 |
| `ExploreCreateDialog` | 异常 | 结果面契约外 → 漂移恰 1 条、对话框状态不崩 | 单次 |
| `ExploreCreateDialog` | 边界 | 空扫描结果 `[]` → 零漂移、既有空态不变 | 单次 |
| `ExploreCreateDialog` | 边界 | 名称超长（>1000 字符）/ 含 `\n` 与 emoji → 参数面放行（无 `.refine()`），后端判定 | 参数化迭代（超长 / 特殊字符） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock`（按命令名分发，可切 resolve / reject） | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` -> `packages/desktop/src/views/explores/hooks/use-explore-doc.test.ts`

既有测试文件扩展（`invokeMock` + `ChannelMock` 沿既有装置）。本文件的 `watchSubscribe(channel, path)` 是 `Channel<FileWatchEvent>` 信封面在业务侧的第二个消费点。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useExploreDoc` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`readExplore` / `exploreDocPath` / `watchSubscribe` / `watchUnsubscribe`），其中 `Channel<FileWatchEvent>` 为首参。 -->

- `useExploreDoc`（无 design 声明的公共 API 变更条目）: `readExplore` / `exploreDocPath` / `watchSubscribe`（首参 `Channel<FileWatchEvent>`）/ `watchUnsubscribe` 四命令的参数面放行（`string | null` 路径面）。
- 失效信号信封的逐条 soft-report: 脏信封只上报一条、既有 500ms 防抖重拉仍触发（不丢流、不断流）。
- 既有退订幂等与 error 态文案: 卸载 / 参数变更路径与 reject 呈现不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useExploreDoc` | 正向 | `readExplore(root, name)` → 放行、文档结果干净零漂移、既有防抖重拉不变 | 单次 |
| `useExploreDoc` | 正向 | `exploreDocPath` 返回 `null` → 不调 `watchSubscribe`、零漂移（既有短路分支不变）；返回字符串 → `watchSubscribe(channel, path)` 放行 | 参数化迭代（null / 字符串） |
| `useExploreDoc` | 正向 | 合法失效信号信封投递 → handler 收到原消息（同一引用）、零漂移 | 单次 |
| `useExploreDoc` | 异常 | 脏信封（失效信号缺 `kind`）→ 漂移恰 1 条 + 既有 500ms 防抖重拉仍触发（不丢流、不断流） | 单次 |
| `useExploreDoc` | 异常 | reject 串（`readExplore` 失败）→ 既有 error 态文案逐字不变、零漂移；订阅失败分支不阻断页面 | 单次 |
| `useExploreDoc` | 边界 | 卸载 / 参数变更 → `watchUnsubscribe(subscriptionId)` 1 次、零漂移；`exploreDocPath` 返回 `null` 时零退订调用 | 组合迭代（卸载 × 参数变更） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | 沿用既有 `invokeMock` + `ChannelMock`（`vi.hoisted` 可编程 class） | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 信封面与退订臂 |

### `packages/desktop/src/views/explores/hooks/use-explore-list.ts` -> `packages/desktop/src/views/explores/hooks/use-explore-list.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发；本文件 7 处调用点，是 `ipc.` 替换面最宽的文件）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useExploreList` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`listExploreRecords` / `createExploreRecord` / `renameExploreRecord` / `deleteExploreRecord` / `updateExploreTitle` / `updateExploreWorkflowType` / `promoteExplore`）。 -->

- `useExploreList`（无 design 声明的公共 API 变更条目）: 7 命令（`listExploreRecords` / `createExploreRecord` / `renameExploreRecord` / `deleteExploreRecord` / `updateExploreTitle` / `updateExploreWorkflowType` / `promoteExplore`）的参数面放行与结果面 soft-report。
- 既有 `onError` 回显: 重名 / 不存在的 reject 串文案不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useExploreList` | 正向 | `listExploreRecords(root)` → 放行、结果干净零漂移、清单渲染不变 | 单次 |
| `useExploreList` | 正向 | 改名 / 删除 / 标题更新 / workflowType 更新 / promote 五动作命令 → 参数面放行、各 `invoke` 1 次、零漂移 | 参数化迭代（5 命令） |
| `useExploreList` | 异常 | reject 串（重名 / 不存在）→ 既有 `onError` 文案逐字不变、零漂移、无未捕获异常 | 参数化迭代（两条 reject 文案） |
| `useExploreList` | 异常 | 结果面脏（record 缺 `name`）→ 漂移恰 1 条 + 清单不崩（原值透传） | 单次 |
| `useExploreList` | 边界 | 空清单 `[]` → 零漂移、既有空态不变 | 单次 |
| `useExploreList` | 边界 | `renameExploreRecord` 的新名超长（>1000 字符）/ 含 emoji → 参数面放行（无 `.refine()`） | 参数化迭代（超长 / 特殊字符） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/explores/hooks/use-explore-session.ts` -> `packages/desktop/src/views/explores/hooks/use-explore-session.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useExploreSession` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`ensureExploreSession`）。 -->

- `useExploreSession`（无 design 声明的公共 API 变更条目）: `ensureExploreSession(root, recordName)` 的参数面放行与结果面 soft-report。
- 既有会话幂等语义: 已有会话命中时的 `invoke` 次数与状态映射不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useExploreSession` | 正向 | `ensureExploreSession(root, recordName)` → 放行、更新后记录干净零漂移、既有会话绑定不变 | 单次 |
| `useExploreSession` | 正向 | 结果含 `null` 可空字段 → 零漂移、既有状态映射不变 | 单次 |
| `useExploreSession` | 异常 | reject 串 → 既有 error 态文案逐字不变、零漂移 | 单次 |
| `useExploreSession` | 异常 | 结果面契约外（记录缺 `name`）→ 漂移恰 1 条、会话状态不崩 | 单次 |
| `useExploreSession` | 边界 | 已有会话（幂等命中）重复进入 → 放行、零漂移、`invoke` 次数与既有语义一致 | 单次 |
| `useExploreSession` | 边界 | `recordName` 超长（>1000 字符）→ 参数面放行（schema 只表达接受面），后端判定 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合 | 全章节 |

### `packages/desktop/src/views/info/hooks/use-code-stats.ts` -> `packages/desktop/src/views/info/hooks/use-code-stats.test.ts`

既有测试文件扩展（`invokeMock` 按命令名分发）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`useCodeStats` 形态不变）；行为面取自 design「修改文件」表：`commands.` → `ipc.` 机械替换（`codeStats`，结果面为千级节点的整树统计）。 -->

- `useCodeStats`（无 design 声明的公共 API 变更条目）: `codeStats(root, depth)` 的参数面放行与结果面 soft-report。
- 大结果面的零漂移与原引用透传: 千级节点整树统计无深拷贝、无重排（R-5 的性能敏感面）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useCodeStats` | 正向 | `codeStats(root, depth)` → 放行、干净结果零漂移、既有汇总渲染不变 | 单次 |
| `useCodeStats` | 正向 | `depth` 取 `0` 与上限整数值 → 均放行（int 边界两侧） | 参数化迭代（2 值） |
| `useCodeStats` | 异常 | reject 串 → 既有 error 态文案逐字不变、零漂移 | 单次 |
| `useCodeStats` | 异常 | 结果面脏（`files` 元素字段类型漂移）→ 漂移恰 1 条 + 既有渲染降级不崩 | 单次 |
| `useCodeStats` | 边界 | 千级节点大结果（含空文件条目 `[]`）→ 零漂移、原引用透传（无深拷贝、无重排） | 单次 |
| `useCodeStats` | 边界 | 空统计（`files: []`、总计为 `0`）→ 零漂移、既有空态不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` | 进程边界 |
| 不 mock `ipc.ts` / `ipc-drift.ts` / 生成物三表 | 真实组合（大结果面的零拷贝透传即被测行为） | 全章节 |

### `packages/desktop/src-tauri/src/bindings/zod.rs` -> `packages/desktop/src-tauri/src/bindings/zod_test.rs`

新建测试文件（`#[cfg(test)] mod zod_test;`）。fixture 走**真实** `specta` 面：`#[derive(Type)]` 具名类型 + `collect_commands!` 产出的 `Function` 清单与 `Types`（`Function` / `BuilderConfiguration` 均 `#[non_exhaustive]`，只能由宏与 `Default` 取得，不为测试伪造），写盘目标用 `tempfile::TempDir`（真实文件系统边界，不 mock）；design 声明的私有辅助（`ZodFormat::{new, remap_dt, remap_fields}` / `hidden_references` / `is_hidden_ndt` / `is_value_ndt` / `is_result_ndt` / `render_command_tables` / `render_phase_dt` / `result_dt` / `channel_envelope_dt` / `guard_forbidden` / `report_degraded` + 直通格式 `AlreadyMapped` + 常量 `FILE_BANNER` / `FILE_PRELUDE` / `CHANNEL_REASON` / `FORBIDDEN_SNIPPETS`）经 `use super::{…}` 直接断言，不新增导出。

#### 待测功能

- `ZodLanguage::export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), specta_zod::Error>`: 格式映射 → 三表渲染 → `Zod` 组装（header / prelude / raw）→ `export(&cfg.types, AlreadyMapped)` 取完整字符串 → `guard_forbidden` 生成期自检（九类禁用片段）→ `report_degraded` 降级清单（stderr）→ `fs::write` 落盘（自检命中即 `Err`，不写盘）。
- `ZodFormat::map_types(&self, types: &Types) -> Result<Cow<'_, Types>, specta::FormatError>`: `specta_serde::Format.map_types`（统一 serde 形态，相位由 `builder().disable_serde_phases()` 关闭，不是 `PhasesFormat`）+ 大整数 / `char` / `serde_json::Value` 引用改写 + `Value` 与 `Result` 的 `ty` 置 `None`。
- `ZodFormat::map_type(&self, types: &Types, dt: &DataType) -> Result<Cow<'_, DataType>, specta::FormatError>`: 命令参数 / 结果顶层 datatype 的同一套改写。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ZodFormat::map_types` | 正向 | `u64` / `i64` / `u128` / `i128` / `usize` / `isize` → 映射为 `define("z.number()")`（`cast_bigints = true`，与 TS 侧 `dangerously_cast_bigints_to_number` 同口径） | 参数化迭代（6 个整数类型） |
| `ZodFormat::map_types` | 正向 | `char` → `define("z.string()")`（不出现上游的 `.refine(…)`）：产品出线面零 `char`（design R-3），故该臂即 `char` 等价性的**唯一**判据，同时守住 AC-7 的 `.refine(` 禁令 | 单次 |
| `ZodFormat::map_types` | 正向 | `serde_json::Value` 的全部引用（含 `NamedReferenceType::Inline`）→ `specta_zod::Unknown`，且 `Value` 具名类型的 `ty` 置 `None` | 参数化迭代（Inline / NamedReference 两种引用形态） |
| `ZodFormat::map_types` | 正向 | `Result` 具名类型 `ty` 置 `None`（不渲染泛型 schema），其余具名类型不受影响 | 单次 |
| `ZodFormat::map_types` | 边界 | `cast_bigints = false` → 大整数保持原样（开关与 TS 侧同源，不做单侧改写） | 参数化迭代（两个开关值） |
| `ZodFormat::map_types` | 边界 | 空 `Types` 集合 / 无 `Value` 与 `Result` 引用的集合 → 返回原样 `Cow`（零改写、零新增条目） | 单次 |
| `ZodFormat::map_types` | 边界 | 递归类型（`TreeEntry ↔ DirNode` 等价 fixture）→ 结构保留、引用不被展开（getter 化属渲染期，映射期零副作用） | 单次 |
| `ZodFormat::map_types` | 异常 | 相位统一面（design R-2）：带 `#[serde(skip_serializing_if)]` / `#[serde(default)]` 的 fixture 经**统一** `specta_serde::Format` 映射后不产生 `_Serialize` / `_Deserialize` 伴生类型（`builder().disable_serde_phases()` 的等价形态），映射不 panic 且结果仍可被渲染 | 单次 |
| `ZodFormat::map_types` | 边界 | 相位选择在当前形态下恒等：对同一 datatype 取 `select_phase_datatype(…, Phase::Deserialize)` 与 `Phase::Serialize` 得到等价 `DataType`（日后重开相位时该臂提示需重新对齐，R-11 的可验证形态） | 参数化迭代（两个 Phase 值） |
| `ZodFormat::map_type` | 正向 | 顶层 `u64` → `define("z.number()")`；顶层 `char` → `define("z.string()")`（与 `map_types` 同口径，命令顶层不走 `map_types`） | 参数化迭代（两类顶层类型） |
| `ZodFormat::map_type` | 边界 | 顶层 `Option<u64>` → 内层重映射且外层可空语义保留（`null` 可过）；顶层已是 `Unknown` → 幂等不二次包装 | 组合迭代（外层容器 × 内层类型） |
| `ZodFormat::map_type` | 异常 | 顶层 `serde_json::Value` 引用（`NamedReferenceType::Inline` 与 `NamedReferenceType::Reference` 两形态）→ 同样改写为 `z.unknown()`（命令顶层参数面不经 `map_types`，两条路径必须同口径，否则参数面会漏出 enum union） | 参数化迭代（两种引用形态） |
| `ZodLanguage::export` | 正向 | fixture（多命令 + 具名类型）导出到临时路径 → 产物含 `import { z } from "zod"`、`degraded(` prelude、`argsSchemas` / `resultSchemas` / `channelArgs` 三表、具名 schema 段 | 单次 |
| `ZodLanguage::export` | 正向 | 三表表项一律 getter（`get "<accessorKey>"() { return …; }`），`accessorKey` = `cfg.function_casing.apply(cfg.command_accessor_name(command))`；含 `#[tauri::command(rename=…)]` 形态时取重命名后的 accessor | 组合迭代（命名形态 × 大小写） |
| `ZodLanguage::export` | 正向 | 参数键名 lowerCamelCase（`since_seq` → `sinceSeq`）且键序与 Rust 形参序一致；`Channel<T>` 形参位出 `degraded("…")` 且键保留在 `argsSchemas` | 参数化迭代（多形参 / 单形参 / Channel 形参） |
| `ZodLanguage::export` | 正向 | 结果面取 `Result` 的 Ok 臂：产物零 `Result` 具名 schema、结果表引用 Ok 臂类型；零形参命令出 `z.strictObject({})` | 组合迭代（结果形态 × 形参个数） |
| `ZodLanguage::export` | 正向 | struct 面一律 `z.strictObject`（含空 shape 与非空 shape 两处），产物无 loose 变体 | 参数化迭代（空 / 非空 shape） |
| `ZodLanguage::export` | 边界 | 降级清单：每个 `Channel` 形参对应一条 `degraded("<CHANNEL_REASON>")` 表项——断言渲染输出里 `degraded(` 命中数 == `channel_envelope_dt` 命中的形参个数、且 reason 与 `CHANNEL_REASON` 逐字一致；`report_degraded` 的 `eprintln!` 形态归手工导出核对（见「不可测试项」） | 参数化迭代（5 个 Channel 形参形态） |
| `ZodLanguage::export` | 边界 | 空命令集合 → 三表为空对象、产物仍含 banner + prelude（形态可编译） | 单次 |
| `ZodLanguage::export` | 边界 | 同一 fixture 导出两次到两个路径 → 字节完全相同（无时间戳 / 无机器路径 / 表序 = `commands` 序，AC-9 单测侧） | 单次 |
| `ZodLanguage::export` | 异常 | `guard_forbidden` 逐片段命中（`FORBIDDEN_SNIPPETS` 九类：`.default(` / `.transform(` / `.catch(` / `.pipe(` / `.refine(` / `export type ` / `z.infer` / `z.ZodType<` / `z.object(`）→ 返回 `Err` 且不写盘、既有旧文件不被半成品覆盖（fail-closed）。该臂以**私有辅助直接入参**方式逐片段断言（`use super::guard_forbidden`），不伪造产物文本；真实产物的「零命中」由 AC-7 的产物 grep（桌面端静态）承担 | 参数化迭代（九类禁用片段） |
| `ZodLanguage::export` | 异常 | `.refine(` 的真实命中臂（design R-4）：命令参数或具名类型含**数值型 map key**（`HashMap<u8, _>` / `HashMap<f64, _>` 形状）→ 上游 `map_key_datatype` 出 `.regex(…).refine(…)` → `guard_forbidden` 返回 `Err` 且目标路径**不产生文件**；MUST NOT 用向产物注入文本的方式伪造命中 | 参数化迭代（`u8` / `f64` 两种 key） |
| `ZodLanguage::export` | 异常 | 保留 `Result` 引用的 fixture（具名类型字段为 `Result<T, E>`、结果面未解包 Ok 臂）→ 导出**成功**且产物零 `ResultSchema` 声明（`ty: None` 只隐藏声明、不改写引用）；该形态留下的未声明标识符只能由 AC-8 的 tsc 桩（`TS2304`）拦截，MUST NOT 期望 `dangling_named_reference`（design R-1 / D16 现值） | 单次 |
| `ZodLanguage::export` | 异常 | 引用 `types` 中**不存在**的具名类型（命令三表渲染期）→ 上游 `primitives::inline` 报 dangling / missing 类错误，导出失败且不写盘（真正的悬挂错误面只在此处） | 单次 |
| `ZodLanguage::export` | 异常 | 写盘目标父目录不存在 → 返回 `Err`（`io::Error` 经 `LanguageExt::Error` 的 `From` 映射），不 panic、不产生部分文件 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（进程边界用真实临时目录） | 写盘目标为 `tempfile::Builder::new().tempdir()` 下的路径，沿 `src/env_test.rs` 的 `tmp()` 先例；失败路径用「不存在的父目录」造 `io::Error`，不用替身 | 导出 / 写盘 / 失败臂 |
| `specta` 类型面 | 真实 fixture（`#[derive(Type)]` / `collect_commands!` 产出的 `Types` + `Default` 起步的 `BuilderConfiguration`），不 mock 类型系统（`Function` / `BuilderConfiguration` 为 `#[non_exhaustive]`，不为测试伪造） | 全部映射与渲染用例 |
| `stderr` | 不捕获、不替身：降级面的**内容与条数**由渲染输出断言（`degraded(` 命中数与 reason 文案），`eprintln!` 的输出形态归静态核对 | 降级清单 |

### `packages/desktop/src-tauri/src/bindings/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

既有测试文件扩展（沿既有 `file_lock()` 串行 + `RestoreOnDrop` + `authoritative_snapshot()` 装置，产物路径经 `CARGO_MANIFEST_DIR` 定位）。

#### 待测功能

- `export_bindings() -> Result<(), String>`: 唯一导出入口——`create_dir_all` 后同一 builder 两次 `export`（先 `bindings.ts`、后 `bindings.zod.ts`），任一失败即非零退出。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `export_bindings` | 正向 | 调用一次 → 两份产物同目录同时存在（`bindings.ts` + `bindings.zod.ts`），`create_dir_all` 单点仍生效 | 单次 |
| `export_bindings` | 正向 | `bindings.zod.ts` 含三张表与 `degraded(` 5 处（5 个 `Channel` 形参位），且不含 `commands` 包装、不 import `bindings.ts`（职责切分） | 单次 |
| `export_bindings` | 正向 | TS 产物既有断言面（命令包装名 / invoke 命令名 / DTO 名单 / 逐字段行）全绿——类型面语义零变更 | 参数化迭代（既有清单） |
| `export_bindings` | 正向 | 相位收敛（design R-2 / D14）：`builder()` 上的 `disable_serde_phases()` 使两份产物同源统一形态——`bindings.ts` 零 `_Serialize` / `_Deserialize` 伴生类型、`bindings.zod.ts` 同样零伴生 schema（`#[serde(default)]` 字段不分裂） | 单次 |
| `export_bindings` | 边界 | 连续两次调用 → 两份产物各自零 diff（幂等；导出顺序不影响任一份字节） | 单次 |
| `export_bindings` | 边界 | 删除 `bindings.zod.ts` 后再次调用 → 重新生成且字节等于权威内容（第二产物同受守卫） | 单次 |
| `export_bindings` | 边界 | 产物父目录缺失时先 `create_dir_all` 再导出（既有用例在双产物形态下仍成立） | 单次 |
| `export_bindings` | 异常 | 手工篡改 `bindings.zod.ts`（删掉一张表 / 注入 `export type`）后重导出 → 字节恢复权威内容（篡改不留存） | 参数化迭代（两类篡改） |
| `export_bindings` | 异常 | 手工篡改 `bindings.ts` 后重导出 → 恢复权威内容（既有用例在双产物形态下仍成立） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 真实文件系统（产物路径由 `CARGO_MANIFEST_DIR` 固定，与 Rust 产物同源），`cargo test` 多线程并行以既有 `file_lock()` 串行、`RestoreOnDrop` 兜底恢复 | 全部用例 |
| `specta` / `specta-typescript` / vendored exporter | 真实调用（`builder()` 零改动；mock 生成器等于绕过被测链路） | 全部用例 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/src/{lib,error,map_keys,opaque,references,reserved_names,types,zod,primitives}.rs` | design「公共函数 / API」表明确标注该 crate「属 vendor 内部，不入表」，故无一行为其声明的公共 API 变更条目；不 grep 导出声明虚构条目。补丁 P1–P3 的语义在 `src-tauri/src/bindings/zod_test.rs` 中经**真实 vendored exporter** 断言（`z.strictObject` 恒出 / getter 化 / 零别名） | `bindings/zod_test.rs` 的 strict、getter、零别名三臂 + AC-7 产物 grep + AC-8 自检桩 |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/{Cargo.toml,README.md}` | 配置与文档类资产，无运行期行为可断言（README 的「补丁索引 / 跟版核对清单 / 删除判据」是判据文本，不是被测逻辑） | AC-10：`cargo tree` 单一 specta RC + `grep "PATCH("` 命中数与 README 记录一致（桌面端静态） |
| `packages/desktop/src/types/generated/bindings.zod.check.ts` | 手写桩文件（**非生成物**、零导出消费方）：其价值是「被 `tsc --noEmit -p tsconfig.zod.json` 编译」，行为面为空；为它写单测等于测「文件能否被 import」，与 AC-8 重复 | AC-8：`bindings:zod-check` 经 `bindings:export` 串联，失败非零退出（桌面端静态自检） |
| 「被隐藏声明的悬空引用」这一失败面（design R-1 / D16 现值） | 上游 `reference_dt` 不检查目标 `ty`：若被隐藏的 `Result` 声明仍被引用，`ZodLanguage::export` 会**成功**并留下未声明标识符——Rust 单测里没有可断言的 `Err`（真正的 dangling / missing 错误只出现在「引用 `types` 中不存在的具名类型」路径，该路径已在 `zod_test.rs` 覆盖）。MUST NOT 写成 `dangling_named_reference` 断言 | AC-8：只 import 产物的 `tsc --noEmit` 桩以 `TS2304` 拦截；`zod_test.rs` 的等价判据是「导出成功 + 产物零 `ResultSchema` 声明」 |
| `packages/desktop/tsconfig.zod.json` | 纯配置（最小 program：零 DOM / 零 node types），无可断言的运行期行为 | AC-8 同上（配置错即自检不过） |
| `packages/desktop/src-tauri/Cargo.toml` | 清单配置（workspace members / lockstep pin / 依赖落位），无运行期行为 | AC-10：各 `Cargo.toml` 审查 + `cargo tree`（单一 specta RC、`tauri-specta` / `specta-typescript` 不出壳层、core / infra 仍只有 `specta`） |
| `packages/desktop/package.json`（`dependencies.zod` / `scripts.*` / `version`）与 `packages/desktop/knip.json` | 配置与脚本：`zod` 落位形态、自检脚本串联、版本号、knip 边界均无单元测试路径（自检脚本本身是静态步骤） | AC-8 / AC-11 / AC-12：`pnpm why zod` + lockfile 审查 + `pnpm -C packages/desktop run check`（含 knip）+ diff 审查 |
| `openspec/config.json` | 零改动要求（不许扩展 schema / 阈值 / 测试框架登记），无可测行为 | AC-12：diff 审查 |
| `export_bindings()` 的两条错误分支（「导出 TS bindings 失败」/「导出 zod schema 失败」） | 导出目标路径由 `CARGO_MANIFEST_DIR` 固定，design 明确「`create_dir_all` 与错误映射零改动」，未提供可注入目录或故障点；在固定路径上造写失败会污染工作树与并行用例 | 代码审查（错误串分别带产物名，形态可逐字核对）；成功 / 幂等 / 恢复三面仍由 `mod_test.rs` 覆盖 |
| `packages/desktop/src/lib/ipc.ts` 的 `safeParse` 内部异常收敛路径（schema 自身抛异常 → 收敛为 `IpcArgsError`） | 触发需一份会抛异常的 schema；真实生成物（零 `.transform` / `.default` / `.pipe`）在 `4.6.5` 下不可达。构造缺陷 schema 需 mock 内部生成物模块或新增测试专用导出，两者均违反「最小 mock」与「no test-only exports」 | 代码审查（`try/catch` 包裹点单一）；可观测的等价面（参数面失败必抛 `IpcArgsError`、绝不以内部异常逃逸）由 `ipc.test.ts` 参数面异常臂覆盖 |
| Rust 侧 `eprintln!` 的 stderr 降级清单输出形态 | `LanguageExt::export` 内直接写 stderr，单元测试不重定向进程级 stderr（重定向会与 `cargo test` 并行输出互扰） | `zod_test.rs` 断言清单**内容与条数**（渲染辅助返回），输出形态由手工导出核对（stderr 5 条） |
| 前端「产物文本 grep 类判据」（九类禁用片段零命中 / 零 `export type` / `degraded(` 命中 5 处 / `z.strictObject` 覆盖数 / 合法 `unknown` 仍 `z.unknown()`） | 属产物静态核对，不是运行期行为断言；在测试文件里读文件做 grep 会把测试绑死在工作树布局上，且与 `bindings:check` / 静态脚本重复 | AC-7：桌面端产物 grep + 生成期 `guard_forbidden`（`zod_test.rs` 自检臂同时守住失败即不写盘；`.refine(` 的真实命中臂由数值型 map key fixture 覆盖，R-4） |
| `packages/desktop/src/lib/ipc.ts` 的表项缺失 fail-open 分支（`NO_ARGS_SCHEMA` / `NO_RESULT_SCHEMA`） | 生成物逐命令都有 `argsSchemas` / `resultSchemas` / `channelArgs` 表项（`channelArgs` 无 Channel 时为 `{}`），故「`instanceof` 收窄失败 → 退化为空形参 / 不观测」在真实产物下不可达；触发需 mock 内部生成物模块或新增测试专用导出，违反「最小 mock」与「no test-only exports」 | 代码审查（D10 明示该放行面是「恒成功替身」，不是可观测行为）；可观测面「5 个 Channel 命令逐键命中 + 非 Channel 命令空表」由 `ipc.test.ts` / `ipc-contract.test.ts` 覆盖 |
