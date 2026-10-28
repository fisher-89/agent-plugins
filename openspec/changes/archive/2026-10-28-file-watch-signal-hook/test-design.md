# 测试设计: file-watch-signal-hook

> **变更**: file-watch-signal-hook
> **日期**: 2026-10-27
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（变更范围 / 测试文件表 / 验收标准 AC-1..AC-12 / R-1..R-8）、`design.md`（架构组件 / D1..D9 / 新增与修改文件表 / 公共函数·API 表 / 数据模型状态机 / AC 落点 / 待决问题 1..5）、`specs/desktop-file-watch/spec.md`（delta）、`tasks.md`、工作树源码与既有测试盘点
> **被测范围**: design「变更清单 - 新增文件」与「修改文件」两张子表的全部模块（`packages/desktop/src/hooks/use-file-watch-signal.ts`、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts`、`packages/desktop/package.json`）；删除文件零项（design 明示「不涉及」）
> **本变更性质**: 纯重构 + 两处账面微调（M1 显式刷新不重订 watch / M2 刷新与待发信号可能各拉一次）。零 Rust、零 bindings、零 IPC 命令面 ⇒ 全部用例落在前端 hook 面

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| 前端（本变更唯一面） | `vite-plus` 的 test 面（vitest API，`vp test`）：`environment: jsdom`、`globals: true`、`setupFiles: src/test-setup.ts`、`include: src/**/*.test.ts?(x)` | 与被测源文件同目录的 `<模块名>.test.ts`（`.test.tsx` 仅组件）；本变更涉及的唯一测试文件是既有 `src/views/explores/hooks/use-explore-doc.test.ts` | `import { describe, expect, it, vi } from 'vite-plus/test'` + `@testing-library/react` 的 `act` / `renderHook` / `waitFor`；`vi.hoisted` + `vi.mock('@tauri-apps/api/core')` 打**进程边界**替身（`invokeMock` 按命令名分发、`ChannelMock` 为原型 accessor + `deliver` 的可编程 class）；防抖窗口用 `vi.useFakeTimers()` + `advanceTimersByTime` 精确推进 |
| Rust | 不适用 | — | design / proposal 明示零 Rust 改动，`server:test` 面无非回归断言（回归由门禁承担） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、reject、失败路径、静默面）/ `边界`（空、空串、单元素、超大、`null`、恰好阈值、生命周期交错点）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行：空参四形态、载荷三形态、时间四点等）/ `组合迭代`（两个以上维度交叉：旧臂计时清理 × 身份变更、无订阅臂 × 卸载等）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增用例**——proposal 判据 §1 与 AC-1 要求既有断言行零删改。

**待测功能条目口径**：`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目（本变更两文件各 1 条，故无「未声明公共 API」的占位章节）；design 未列入公共 API 表的模块私有件（`SIGNAL_DEBOUNCE_MS` / `createDebouncer` / `armSubscription`）**不虚构条目**，其覆盖路径记入「不可测试项」。

**Mock 纪律**（贯穿全篇）：只 mock 进程边界（`@tauri-apps/api/core` 的 `invoke` / `Channel`、计时器）与被测 API 显式注入的依赖；`src/lib/ipc.ts`（代理 + 运行期校验）、`src/lib/ipc-drift.ts`、生成物 `bindings` / `bindings.zod`、以及**被测件自身 `use-file-watch-signal.ts`** 一律真实组合（mock 新件等于 mock 被测对象，AC-3 / AC-4 / AC-5 的判据即刻失效）。不为测试新增导出（`no test-only exports`）：新件经 `useExploreDoc` 公开面穿透覆盖。

**测试布局决策（源文件 → 测试文件的映射依据）**：新件 `src/hooks/use-file-watch-signal.ts` **不新增独立测试文件**——proposal「变更范围 - 测试文件」表明示「公共件自身单测文件：不新增」，理由是六臂（`armKey: null` / `resolvePath` null / 订阅 reject / 迟到退订 / 防抖重置 / 清理）在 `useExploreDoc` 公开面上全部可达，而按仓库规则 5 与 AC-6，为测试给新件开第二导出面（或直接测文件私有 helper）是被禁止的路径。故新件的用例**追加**在既有 `use-explore-doc.test.ts` 的独立 `describe` 块中，既有 `describe` / `it` 的断言行零改动（只新增行，AC-1）。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 既有用例**文件零改动**全绿（纯重构判据）。覆盖方式：既有 `it` / `it.each` 站点（24 站点，proposal 记 23 例——口径差异见 design 待决问题 5）逐条零改动通过 + diff 内既有断言行零删改 | `packages/desktop/src/views/explores/hooks/use-explore-doc.test.ts`（既有用例）、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts`、`packages/desktop/src/hooks/use-file-watch-signal.ts` |
| AC-2 | 消费样板单点化（**生产代码面**）：`new Channel<FileWatchEvent>` 与 `ipc.watchSubscribe` / `ipc.watchUnsubscribe` 的命中**文件路径唯一**且为 `src/hooks/use-file-watch-signal.ts`；`use-explore-doc.ts` 零 `FileWatchEvent` Channel / 零 watch 命令 / 零 `createDebouncer` / 零 `armWatch`。覆盖方式：桌面端静态核对（排除 `*.test.ts(x)` 后按命中文件路径去重）+ 用例侧间接证据（「有效 `armKey` 挂载」用例断言 `watch_subscribe` 首参为本件构造的 `ChannelMock` 实例）——静态判据不计入单元测试 | `packages/desktop/src/hooks/use-file-watch-signal.ts`、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts`（两者均需有单测覆盖，AC-1 / AC-3 / AC-4 即其运行期判据） |
| AC-3 | 臂键语义（M1）：显式 `refresh()` 后 `watch_subscribe` 仍 1 次、`watch_unsubscribe` 0 次；`root` / `name` 变更仍退订 + 重订。覆盖方式：新增用例（消费方章节「M1」两行 + 「订阅身份含 `name` / 含 `root`」两行）+ 既有参数变更用例 | `packages/desktop/src/views/explores/hooks/use-explore-doc.ts`（`armKey` = `root`␀`name`，不含 `tick`）、`packages/desktop/src/hooks/use-file-watch-signal.ts`（effect 依赖仅 `[armKey]`） |
| AC-4 | 信号重拉**不置** `loading`、不清 `error`；500ms trailing 合并语义与既有 3 例一致。覆盖方式：新增用例（AC-4 两行：`loading` 恒 false / `error` 保持原串）+ 既有「500ms trailing 防抖」三例零改动 + 新件章节的窗口边界四点 | `packages/desktop/src/views/explores/hooks/use-explore-doc.ts`（`fetchDoc` 经 `readRef`）、`packages/desktop/src/hooks/use-file-watch-signal.ts`（`createDebouncer` 窗口 500ms） |
| AC-5 | 订阅失败不阻断数据面、订阅迟到就绪补发退订、`resolvePath` 为 null 零退订（含新增的 `explore_doc_path` reject 静默臂）语义逐字不变。覆盖方式：既有用例零改动（null 路径 / subscribe reject / 迟到补发 / 卸载零退订）+ 新增 `explore_doc_path` reject 用例 | `packages/desktop/src/hooks/use-file-watch-signal.ts`（`armSubscription` 的 `catch` 与 `disposed` 补发）、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts`（`resolvePath` 的判空与代理调用） |
| AC-6 | 导出面不变：`use-explore-doc.ts` 仍只 `export` `ExploreDocState` / `useExploreDoc`；新件只 `export` `useFileWatchSignal`；`knip` 零新增未使用导出。覆盖方式：类型层 + 死代码扫描（桌面端静态）；运行期 `Object.keys(module)` 只能观测值导出（`ExploreDocState` 为类型不可观测），断言会退化为模块结构快照 ⇒ 记入「不可测试项」 | `packages/desktop/src/hooks/use-file-watch-signal.ts`、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts` |
| AC-7 | 门禁全绿（fmt / lint / typeCheck 含 `max-lines-per-function: 50`、覆盖率 lines 90 / branches 85 / functions 90、`bindings:check` 无 diff）。覆盖方式：桌面端门禁执行（机械步骤，由静态检查与测试执行相位承担）；新件的分支覆盖依赖本文件新件章节用例 | `packages/desktop/src/hooks/use-file-watch-signal.ts`、`packages/desktop/src/views/explores/hooks/use-explore-doc.ts`、`packages/desktop/package.json` |
| AC-8 | 零后端面：`src-tauri/**` 零 diff、`Cargo.toml` crate 版本行未跟随。覆盖方式：diff 审查（无运行期行为可断言） | `packages/desktop/src-tauri/**`（零 diff 面，不在被测范围内） |
| AC-9 | 边界纪律：`use-change-detail.ts` / `use-exec-output.ts` / `knip.json` 零 diff。覆盖方式：diff 审查 | `packages/desktop/src/views/changes/hooks/use-change-detail.ts`、`packages/desktop/src/views/changes/hooks/use-exec-output.ts`、`packages/desktop/knip.json`（均零 diff 面） |
| AC-10 | 版本纪律：`version` 0.6.2 → 0.6.3（一次 patch）。覆盖方式：`package.json` 审查；无运行期行为 ⇒ 记入「不可测试项」 | `packages/desktop/package.json`（`version`） |
| AC-11 | 契约归属：delta 中 `desktop-file-watch` 的 Module Contract 指向 `src/hooks/use-file-watch-signal.ts` 并含 explore 详情页消费方行；`单文件 watch 订阅通道` / `生命周期与平台边界` 原样保留。覆盖方式：delta 审查 + 归档合入演练（文档资产，无运行期行为） | `openspec/changes/file-watch-signal-hook/specs/desktop-file-watch/spec.md`（delta，只读核对） |
| AC-12 | 手工冒烟：保存笔记 → 500ms 内预览自动刷新且**无 `preview-loading` 闪动**；点刷新后预览更新且 watch 不重建；切换 workspace 无残留订阅。覆盖方式：Windows 桌面端手工冒烟（单元测试只能观测 hook 状态面，观感判据归手工）；其中「刷新不重建」「无残留订阅」两条已有运行期用例作为前置置信 | `packages/desktop/src/views/explores/explore-detail-view.tsx`（呈现面观察点，零改动）、`packages/desktop/src/views/explores/components/**`（零改动） |

---

## 单元测试

### `packages/desktop/src/hooks/use-file-watch-signal.ts` -> `packages/desktop/src/views/explores/hooks/use-explore-doc.test.ts`

**不新建测试文件**（依据见篇首「测试布局决策」），新增用例追加为独立 `describe` 块（建议标题 `useFileWatchSignal：经消费方公开面（C1–C5）`），既有 `describe` / `it` 断言行零改动。

本章覆盖新件的五件事：C1 Channel 建立、C2 `resolvePath` → `watch_subscribe`、C3 500ms trailing 防抖、C4 退订（卸载 / 迟到补发 / 无订阅零退订）、C5 订阅链失败静默。驱动面只有 `renderHook(() => useExploreDoc(root, name))` 一条（新件无第二消费方，规则 2 / R-1）。

**既有装置的追加行（只加不改既有断言）**：`mockIpc()` 的 `explore_doc_path` 分支需补可切 reject 的开关（与既有 `docResult` / `subscribeResult` 的「字符串即 reject」同手法），供本章「`resolvePath` 的 Promise reject」用例驱动。

**不在断言面的臂（避免写成恒真或未实现契约）**：① 退订**后**的迟到信号投递到旧 `Channel` 实例——后端退订后不再投递，spec / design 未声明该臂，本章只锁定「卸载后投递零取数」（`readRef` 已置 null 的既有语义）；② `resolvePath` 的**同步抛出**臂（见「不可测试项」）。

#### 待测功能

- `useFileWatchSignal(params: { armKey: string | null; resolvePath: () => Promise<string | null>; onSignal: () => void }): void`: 文件 watch 失效信号消费件。`armKey` 非 null 才建 `Channel<FileWatchEvent>` 并订阅；`resolvePath()` 返回 null 则不订阅（零 `watch_subscribe`）；信号经 500ms trailing 防抖合并为一次 `onSignal`；`armKey` 变化 / 卸载时取消待发计时并 `watch_unsubscribe(subscriptionId)`（订阅迟到就绪则立即补发退订）；订阅链任一环 reject 静默（不抛、不重试、不新增错误通道）；`resolvePath` / `onSignal` 以最新引用生效，effect 依赖仅 `[armKey]`；零取数、零消费方状态写入、零载荷语义；参数面恰三项（不开 `debounceMs` / `enabled` / `retry` / 错误回调）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useFileWatchSignal` | 正向 | 有效 `armKey` 挂载：`resolvePath()` 恰驱动 1 次 `explore_doc_path`（入参为当前 `root` / `name`），`watch_subscribe` 恰 1 次且首参为 `ChannelMock` 实例、次参为派生路径；订阅链完成不产生额外 `read_explore`（**本件零取数**：`read_explore` 仍为挂载时那 1 次） | 单次 |
| `useFileWatchSignal` | 正向 | 订阅就绪后投递 1 条信号 → 未推进窗口时 `onSignal` 零调用（`read_explore` 不变）；推进 `SIGNAL_DEBOUNCE_MS` = 500ms 后 `onSignal` 恰 1 次（`read_explore` 1 → 2） | 单次 |
| `useFileWatchSignal` | 正向 | 窗口内 3 条信号（间隔 200ms）：合并为恰 1 次 `onSignal`（trailing 每次重置计时，最后一拍必达）——既有用例（零改动） | 单次 |
| `useFileWatchSignal` | 正向 | 载荷形态无关（D5「信号无载荷」）：`deliver(undefined)` / `deliver({})` / `deliver({ path, content: '不应被读取的字节' })` 三形态，各窗口内均合并为 1 次 `onSignal`、`doc` 不因载荷变化——`{ path, content }` 形态已有既有两例（零改动，`:193` / `:428`）覆盖；`undefined` 与 `{}` 两形态为**新增** | 参数化迭代（3 种载荷） |
| `useFileWatchSignal` | 正向 | 初始化顺序：挂载即完成 `explore_doc_path` → `watch_subscribe`（两命令各 1 次）且 `watch_subscribe` 的 `onEvent` 与 `ChannelMock.instances[0]` 同一实例（C1 / C2 接线未被 `readRef` 桥接打乱） | 单次 |
| `useFileWatchSignal` | 异常 | `resolvePath()` 解析为 `null`：零 `watch_subscribe`；卸载后零 `watch_unsubscribe`（`subscriptionId` 恒 null）——既有两例（零改动，一处走 `waitFor` 一处走 `flush`） | 单次 |
| `useFileWatchSignal` | 异常 | `watch_subscribe` reject：无错误态冒泡（`error` 为 null）、页面不崩、显式 `refresh()` 仍拉取——既有用例（零改动）；**新增**补充断言「无信号投递时推进 1000ms 不产生重拉」（被拒绝的订阅链自身不触发 `onSignal`，既有用例未断言该臂） | 单次 |
| `useFileWatchSignal` | 异常 | `resolvePath()` 的 Promise **reject**（`explore_doc_path` 失败）：零 `watch_subscribe`、零 `watch_unsubscribe`、静默（`error` 仍 null、`doc` 正常呈现、`loading` 复位）——**新增**（既有套件只有 null 臂，无 reject 臂） | 单次 |
| `useFileWatchSignal` | 异常 | 订阅就绪前卸载（`watch_subscribe` 在飞行中）：返回的 `subscription_id` 被**立即**用于补发 1 次 `watch_unsubscribe`，无残留订阅——既有用例（零改动） | 单次 |
| `useFileWatchSignal` | 边界 | `armKey === null`（`root` / `name` 任一为 `null` 或空串，共 4 形态）：零 `Channel` 构造（`ChannelMock.instances.length === 0`）、零 `watch_subscribe`、零 `watch_unsubscribe`（含卸载后）、零 `read_explore`——其中 `(ROOT, null)` 为既有用例（零改动），其余三形态为**新增** | 参数化迭代（`(null, NAME)` / `(ROOT, null)` / `('', NAME)` / `(ROOT, '')`） |
| `useFileWatchSignal` | 边界 | 窗口阈值四点：信号到达 → 推进 499ms 零触发、再推进 1ms（恰好 500ms）触发恰 1 次；另一臂在窗口内第 400ms 再投 1 条 → 第 899ms 零触发、第 900ms 触发恰 1 次（重置语义的精确边界，既有用例只测「推 500 一次性到位」）——**新增** | 组合迭代（499 / 500 / 899 / 900 四点） |
| `useFileWatchSignal` | 边界 | 超长 `armKey`（`root` 与 `name` 合计 > 1000 字符）：仍恰 1 次订阅、`explore_doc_path` 与 `watch_subscribe` 入参逐字不截断（NUL 拼接无碰撞）——**新增** | 单次 |
| `useFileWatchSignal` | 边界 | 卸载前窗口内有待发信号：cleanup 取消计时 ⇒ 推进 1000ms 后 `onSignal` 零调用、`read_explore` 不增、`doc` 保持——既有用例（零改动） | 单次 |
| `useFileWatchSignal` | 边界 | 订阅 reject 后卸载：`subscriptionId` 从未写入 ⇒ 零 `watch_unsubscribe`（不产生对无效 id 的退订）；与「订阅成功 1 次退订」对照（同一 `unmount` 驱动两臂）——**新增** | 组合迭代（reject × 卸载 / 成功 × 卸载） |
| `useFileWatchSignal` | 边界 | 同 `armKey` 连续重渲染（新件的 `resolvePath` / `onSignal` 每次渲染都是新函数标识）：零重订（`watch_subscribe` 恒 1、`watch_unsubscribe` 恒 0），信号仍由**最新** `onSignal` 引用触发一次重拉——**新增**（spec「MUST NOT 因回调身份变化重订订阅」的运行期判据） | 单次 |
| `useFileWatchSignal` | 异常 | 零副作用面：订阅 → 信号 → 卸载全序列中 `console.warn` / `console.error` 零调用（本件不新增错误通道、不写消费方状态）——**新增** | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `vi.hoisted` 的 `invokeMock`：按命令名（`read_explore` / `explore_doc_path` / `watch_subscribe` / `watch_unsubscribe`）分发，命令返回「字符串即 reject、其余按 fixture resolve」；本章仅追加 `explore_doc_path` 的 reject 开关（`derivedPath` 扩展为 `string \| null \| { reject: string }` 同手法） | 进程边界（Tauri IPC）；`explore_doc_path` reject 臂、订阅 reject 臂、迟到就绪臂 |
| `@tauri-apps/api/core`（`Channel`） | 沿用既有 `ChannelMock`（原型 accessor + `#onmessage` 私有字段 + `deliver(event)` + `instances` 静态表）：`deliver` 是唯一的信号注入口，`instances` 用于断言「零 Channel 构造」与「首参同一实例」 | 进程边界（`Channel` 是 IPC 值类型，只能在边界替身中构造）；C1、`armKey: null` 边界、载荷形态 |
| 计时器 | 既有 `vi.useFakeTimers()` + `vi.advanceTimersByTime`，`afterEach` 复位 `vi.useRealTimers()`；新增用例沿用同一 `flush()` 辅助（`await act(async () => {})`）冲刷微任务 | 500ms trailing 防抖的窗口阈值四点、卸载取消计时、迟到补发（fake timers 下不可用 `waitFor`） |
| `console` | `vi.spyOn(console, 'warn' / 'error')` 只观测零调用（既有 `useExploreDoc：经 ipc 的参数面` 章节已有同款装置） | 「零副作用面」用例 |
| 不 mock：`src/hooks/use-file-watch-signal.ts` | **必须真实组合**——它是本章被测对象；改用它自己的替身等于让全部断言恒真 | 全章节 |
| 不 mock：`src/lib/ipc.ts` / `ipc-drift.ts` / `types/generated/*` | 真实组合（`ipc` 代理面是 spec「只经 `ipc` 代理调用」的判据所在；既有用例的 `driftCount()` 断言依赖真实校验面） | 全章节 |

### `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` -> `packages/desktop/src/views/explores/hooks/use-explore-doc.test.ts`

既有测试文件**零改动**（AC-1）；本变更只在文件尾部追加新的 `describe` 块（建议标题 `useExploreDoc：臂键与重订边界（M1）` / `useExploreDoc：信号重拉与状态面（AC-4）` / `useExploreDoc：有参 ↔ 空参平移`）。既有 `describe` / `it` / `it.each` 的断言行一行不改——这是纯重构判据本身的验证方式。

取数语义（`fetchDoc` / `loading` / `error` / 取消抑制 / `[root, name, tick]` 依赖面）逐字不动，故本章的「正向」主体是**回归登记**；新增用例只针对 design D6 / D7 / D8 与 proposal §5 的两处微调。

#### 待测功能

- `useExploreDoc(root: string | null, name: string | null): ExploreDocState`: explore 详情页消费方。持有取数语义（`read_explore` + 取消抑制 + 失败落 `error`）与 `loading` / `error` 状态，取数 effect 依赖面 `[root, name, tick]` 不变；watch 订阅移交 `useFileWatchSignal`（`armKey` = `root`␀`name`，**不含 `tick`**，空参 / 空串给 `null`）；信号重拉经 `readRef` 桥接**当前**取数闭包，**不置** `loading`、**不清** `error`；导出面仍只有 `ExploreDocState` / `useExploreDoc`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useExploreDoc` | 正向 | 既有站点零改动全绿（挂载 `read_explore` 恰 1 次 / 同步进入 `loading` 后复位 / 500ms 三例合并 / 卸载以 `subscription_id` 退订 1 次 / 迟到就绪补发退订 / `explore_doc_path` null 不订阅 / `watch_subscribe` reject 不阻断 / `read_explore` reject 置 error / 参数变更取消抑制两例 / 四命令调用面 / `ipc` 校验与零漂移四例）——**既有**（零改动） | 参数化迭代（既有站点全集） |
| `useExploreDoc` | 正向 | 信号重拉路径与挂载一致：投递 1 条信号 → 500ms 后 `read_explore` 的入参逐字为 `{ root, name }`、`doc` 更新为最新一次读取结果——**既有**（零改动） | 单次 |
| `useExploreDoc` | 正向 | **M1（AC-3）**：订阅就绪后调用 `refresh()` → `read_explore` 2 次、`watch_subscribe` 仍 1 次、`watch_unsubscribe` **0 次**（`tick` 不进 `armKey`，显式刷新只重取数不重订）——**新增** | 单次 |
| `useExploreDoc` | 正向 | **M1 续**：`refresh()` 之后投递信号 → 500ms 后 `read_explore` 3 次（刷新未杀掉订阅，自动重拉仍存活）——**新增** | 单次 |
| `useExploreDoc` | 正向 | **AC-4**：信号触发的重拉全程 `loading` 恒 `false`（挂载取数 settle 后不再置 true）⇒ 呈现层 `preview-loading` 不因外部保存闪动——**新增** | 单次 |
| `useExploreDoc` | 异常 | **AC-4 续**：`read_explore` 先 reject 置 `error`，随后改为正常返回并投递信号 → 500ms 后 `doc` 更新，而 `error` **保持原串不被清**（信号重拉 ≠ 显式刷新；显式刷新才会 `setError(null)`）——**新增** | 单次 |
| `useExploreDoc` | 边界 | **M2（账面行为，proposal 标「可选」）**：投递信号后在窗口内点 `refresh()`，再推进 500ms → `read_explore` 共 3 次（挂载 1 + 刷新 1 + 待发信号 1）；两次结果幂等（`doc` 为同一内容）、`loading` 不置、`error` 不变——**新增** | 单次 |
| `useExploreDoc` | 正向 | 订阅身份含 `name`：`name` 由 `old-doc` → `new-doc` ⇒ `watch_unsubscribe(旧 id)` 恰 1 次、`watch_subscribe` 新增 1 次（总计 2 次，`explore_doc_path` 以新 `name` 派生）；`doc` 为新文档——**新增**（既有参数变更两例只断言取数侧，未断言退订 / 重订计数） | 单次 |
| `useExploreDoc` | 边界 | 订阅身份含 `root`：切换 workspace 根 ⇒ 旧订阅以旧 id 退订恰 1 次、新根重新 `explore_doc_path` + `watch_subscribe`；随后卸载只以新 id 退订 1 次（无残留订阅）——**新增** | 单次 |
| `useExploreDoc` | 边界 | 臂键变更与待发计时的交叉：信号到达后**未到 500ms** 即切 `name` ⇒ 旧臂计时随旧 `armKey` 清理（推进 500ms 不产生旧臂重拉），重拉只来自新臂的挂载取数（`read_explore` 总计 2 次）——**新增** | 组合迭代（信号在飞 × 身份变更） |
| `useExploreDoc` | 边界 | 有参 → 空参（`root` 变 `null`）：旧订阅退订恰 1 次、零新订阅、`doc` / `error` 复位为 `null`、`loading` 为 `false`——**新增** | 单次 |
| `useExploreDoc` | 边界 | 空串参数（`root = ''` 或 `name = ''`）：`armKey` 为 `null` ⇒ 零 Channel / 零 watch 命令，与取数 effect 的 `!root \|\| !name` 判据同口径——**新增** | 参数化迭代（`('', NAME)` / `(ROOT, '')`） |
| `useExploreDoc` | 异常 | 取数实例与被取消实例的交互：`tick` 变化后旧取数实例 `cancelled` 置真 ⇒ 迟到信号只由**当前**实例发起一次重拉（`read_explore` 增 1）、旧实例结果不覆盖 `doc`（`doc` 恒等于当前实例结果、`error` 为 null）——**新增** | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | 沿用既有 `invokeMock` + `ChannelMock`（`vi.hoisted` 可编程 class、`deliver` / `instances`）；本章只复用既有分发器，不新增替身形态（`explore_doc_path` reject 开关若已在新件章节补入则直接复用） | 进程边界；四命令分发、信号注入 |
| 计时器 | 既有 `vi.useFakeTimers()` + `advanceTimersByTime`；未用 fake timers 的用例走既有 `waitFor` | 500ms 合并、窗口内 `refresh()` 交错（M2）、臂键变更与待发计时交叉 |
| `console` | 复用既有 `useExploreDoc：经 ipc 的参数面` 章节的 `vi.spyOn(console, 'warn')` 静音（该章节 `afterEach` 已 `vi.restoreAllMocks`） | 新增用例若命中 `ipc` 漂移告警面时不污染输出 |
| 不 mock：`src/hooks/use-file-watch-signal.ts` | 真实组合——M1 / M2 / AC-4 的判据全是「消费方 tick × 新件 armKey」的交叉行为，替身任一侧都会让结论变成自证 | 全章节 |
| 不 mock：`src/lib/ipc.ts` / `ipc-drift.ts` / 生成物 | 真实组合（`explore_doc_path` / `watch_*` 参数面与漂移计数是 AC-5 的观测面） | 全章节 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 0.6.2 → 0.6.3） | 配置 / 元数据，无运行期行为可断言；版本纪律的判据是「一次 patch、不连带 `Cargo.toml`」 | AC-10 / AC-8：diff 审查（`package.json` 单行变更、`src-tauri/Cargo.toml` crate 版本行未跟随） |
| `packages/desktop/src/hooks/use-file-watch-signal.ts` 的文件私有件（`SIGNAL_DEBOUNCE_MS` / `createDebouncer` / `armSubscription`） | 非导出，无自己的测试路径；为测试新增导出违反仓库规则 5 与 AC-6（`knip` 判 unused、`client:check` 报错）。design D1 明示拆件留在文件内、不外泄导出，是**有意的**测试面收窄 | 新件章节用例 1–16 全部落在这些私有件的运行期行为上（Channel 建立 / 订阅 / 防抖窗口四点 / 退订外壳 / 静默面），覆盖率分支由同一批用例命中 |
| `resolvePath` 的**同步抛出**臂（新件 `void (async () => …)().catch(() => {})` 的同步面，D3） | 公开面无路径可达：消费方的 `resolvePath` 只有「判空 → `ipc.exploreDocPath(root, name)`」，后者同步抛出仅发生在参数面非法时；而同一组 `root` / `name` 会先被同一 effect 的 `read_explore` 使用并同样同步抛出（抛在 `useEffect` 同步段，测试无法只让 `explore_doc_path` 抛）。构造该臂需 mock 内部 `ipc` 模块或新增测试专用导出——违反最小 mock 与 `no test-only exports` | 代码审查（`async` IIFE 尾部单一 `catch` 是静默面的唯一入口，与既有 `armWatch` 的 `.catch(() => {})` 同构）；**异步** reject 面由新件章节「`resolvePath()` 的 Promise reject」用例覆盖 |
| 新件「迟到信号投递到已 `dispose` 的 `Channel` 实例」 | 不在 spec / design 的契约面（后端退订后不再投递；`Channel` 是 IPC 值类型，前端无法自证后端行为）。若断言「零取数」则与实现的 `onmessage` 直连语义冲突，等于凭空造契约 | 卸载后投递零取数由既有用例（`readRef.current = null`）锁定；退订语义本身由「订阅身份变化即重订」「迟到就绪补发退订」两臂覆盖；`disposed` 只影响退订、不影响通道，属代码审查项 |
| AC-2 的生产代码单点化判据（`new Channel<FileWatchEvent>` / `ipc.watchSubscribe` / `ipc.watchUnsubscribe` 命中**文件路径唯一**） | 属 grep 类静态核对：把文件读进测试做扫描会把用例绑死在工作树路径布局上，且与桌面端静态检查步骤重复；proposal 的 AC-2 口径明确它是静态断言（排除 `*.test.ts(x)`、不按行数计） | 桌面端静态检查（排除测试文件后按命中文件路径去重，期望唯一 `src/hooks/use-file-watch-signal.ts`）；运行期侧证由「有效 `armKey` 挂载」用例给出（`watch_subscribe` 首参必为本件构造的 `ChannelMock` 实例） |
| AC-6 导出面与 `knip` 零新增判据 | 运行期只能观测**值**导出（`ExploreDocState` 是接口、不可观测），`Object.keys(await import(...))` 只能给模块结构快照，属恒真或易漂移断言；真判据在类型层与死代码扫描 | 桌面端静态（导出声明审查 + `client:check` 的 knip；新件被 `use-explore-doc.ts` 生产消费即满足「非 test-only 导出」） |
| AC-7 门禁面（fmt / lint `max-lines-per-function: 50` / typeCheck / 覆盖率阈值 / `bindings:check` 无 diff） | 机械门禁步骤，由桌面端静态检查与测试执行相位承担；在测试设计里复述阈值等于复制运行脚本细节 | 门禁执行结果 + design D1 / R-7 的规模前提（新件三个函数体分别约 12 / 18 / 15 行）+ 本文件新件章节的六臂覆盖（`armKey: null` / `resolvePath` null / 订阅 reject / 迟到退订 / 防抖重置 / 清理，R-8） |
| AC-8 / AC-9 的「零 diff」纪律（`src-tauri/**`、`knip.json`、`use-change-detail.ts`、`use-exec-output.ts`） | 是 diff 断言，无运行期行为可测（这些模块的正确性主张就是「一行不变」） | `git diff` 审查（变更面应仅三文件 + 测试新增行） |
| AC-11 契约归属（delta `spec.md` 的 Module Contract 行） | 文档资产，无运行期行为 | delta 审查 + 归档合入演练（`MODIFIED` / `ADDED` 段与 D1–D4、D7 落点逐条对照） |
| AC-12 手工冒烟（`preview-loading` 不闪、切 workspace 无残留订阅、点刷新 watch 不重建） | 「闪动」是渲染观感，需真实桌面端与真实文件系统写入；jsdom + fake timers 下的 hook 断言只能给状态面等价证据 | 手工冒烟清单；其中两条已有运行期前置置信：AC-4 用例（`loading` 恒 false）与 M1 用例（刷新不重订）+ 切根用例（旧 id 退订） |
