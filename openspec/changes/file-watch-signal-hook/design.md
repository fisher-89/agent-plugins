# 设计: file-watch-signal-hook

> **变更**: file-watch-signal-hook
> **日期**: 2026-10-27
> **基于**: `proposal.md`、`explore.md` 与 `specs/desktop-file-watch/spec.md`（delta：`MODIFIED` 失效信号语义 + `ADDED` 共享文件 watch 信号消费件 / 订阅臂键与重订边界 + Module Contract 归属修正）

---

## 提案与规格同步状态

proposal 与 delta spec 已定稿（含 AC-2 的口径回溯修正与 §5 两处微调记账）。本设计不重述其论证，只做三件事：把新件的**内部拆分**落到 `max-lines-per-function: 50` 约束下的可实现形态（C1–C5 分别由哪个私有件承载）；把消费方的**取数 effect 与信号订阅解耦**落成具体结构（`readRef` 桥接点、空参数守卫的类型形态、cleanup 顺序）；把**两处微调（M1/M2）的注释留痕位置**与逐条 AC 落点定死。

**起点核实**（工作树实况：`packages/desktop` 0.6.2；锚点为改动前行号）：

- **消费方现状**：`src/views/explores/hooks/use-explore-doc.ts`（130 行）——`WATCH_DEBOUNCE_MS = 500`（`:7-8`）、`ExploreDocState`（`:10-17`）、`fetchDoc`（`:20-40`：`read_explore` + 取消抑制 + 失败落 `error`，**不置 `loading`**）、`createDebouncer`（`:43-63`）、`armWatch`（`:66-78`：`explore_doc_path` → `path === null` 短路 → `watch_subscribe` → 返回退订闭包）、取数与订阅**合一**的 effect（`:96-127`，依赖 `[root, name, tick]`）、导出面仅 `ExploreDocState`（`:10`）+ `useExploreDoc`（`:88`）。
- **AC-2 基线实测**（本次 grep，排除 `*.test.ts(x)`）：`new Channel<FileWatchEvent>` 仅命中 `use-explore-doc.ts:112`；`ipc.watchSubscribe|ipc.watchUnsubscribe` 命中文件唯一 `use-explore-doc.ts`；`new Channel<` 另有 `src/lib/agent-transport.ts:177,202`（`AgentRunMessage`）与 `src/views/changes/hooks/use-change-flow-run.ts:46`（`RunNotice`）——属其它推送语义，不在判据内。实施后「文件路径唯一 = `src/hooks/use-file-watch-signal.ts`」必须成立。
- **调用面现状**：`ipc` 是 `commands` 的 Proxy（`src/lib/ipc.ts`），签名取自生成物——`watchSubscribe: (onEvent: Channel<FileWatchEvent>, path: string) => Promise<number>`（`bindings.ts:196`）、`watchUnsubscribe: (subscriptionId: number) => Promise<boolean>`（`:201`）、`exploreDocPath: (root: string, name: string) => Promise<string | null>`（`:142`）、`readExplore`（`:131`）；`Channel` 来自 `@tauri-apps/api/core`。本变更零新增命令、零 bindings 重生成。
- **用例面现状**：`src/views/explores/hooks/use-explore-doc.test.ts` 602 行，24 个 `it` / `it.each` 站点（proposal 记 23 例，差异是「站点数」与「表驱动展开数」的计数口径，判据仍取「既有断言行零删改」）；用例只在公开面驱动（`renderHook(... useExploreDoc)`），进程边界 mock 落在 `@tauri-apps/api/core` 的 `invoke` / `Channel` 上 ⇒ 新件经 `ipc` 调用同一 `invoke`，既有用例零改动即可穿透覆盖新件。
- **lint 面现状**（`packages/desktop/vite.config.ts:39-75`）：`max-lines-per-function: 50`（非测试 `.ts`）+ `typescript/no-non-null-assertion: error` + `typescript/no-unsafe-type-assertion: error` + `reportUnusedDisableDirectives: error` ⇒ 消费方的空参数守卫只能用**显式判空**（不可 `root!` / `as string`，也不可加 disable 注释）；超长函数必须**拆文件私有 helper**（R-7）。
- **knip 面现状**（`knip.json`）：`project` 排除 `*.test.ts(x)`，`ignore` 仅 `src/types/generated/**` 与 `src/components/ui/**` ⇒ 新件必须被生产文件 import（`use-explore-doc.ts` 满足）；新件零新增依赖 ⇒ `knip.json` 零 diff。
- **delta spec 现状**：`specs/desktop-file-watch/spec.md` 已把 Module Contract 归属改到 `src/hooks/use-file-watch-signal.ts` 并补 explore 详情页消费方行；`MODIFIED` 段保留「单文件订阅通道」「生命周期与平台边界」等其余 requirement 原样。

**三处校准**（不改变规格意图，但决定实现形态）：

1. **消费方空参数守卫的形态**（D7）：`armKey` 非 null ⇒ `root` / `name` 必非 null，故 `resolvePath` 内的空参臂**结构上不可达**。这是 spec「`armKey: null` 时不订阅、不解析路径」的产物，不是实现缺陷；MUST NOT 为覆盖率让本件在 `armKey === null` 时也调用 `resolvePath`（那会新增无谓 IPC，违「非定址态不订阅」）。
2. **M2 的计时器归属**（D8）：计时器归新件、只随 `armKey` 重建 ⇒ 消费方 `tick` 重跑不再取消待发信号。这是「计时器按臂键存续」的必然结果，已由 proposal §5 记账。
3. **§4 的否决落地**（D6）：公共件零取数、零首拍 ⇒ 消费方「挂载即拉一次」与「同步 `loading`」两条既有语义**逐字不动**，这是 AC-1 与 AC-5 的前提。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 新件内部拆分 | 三个成员：① `createDebouncer(onDue: () => void): { push: () => void; cancel: () => void }`（只注入到点回调，窗口恒为文件私有 `SIGNAL_DEBOUNCE_MS = 500`、**不进参数面**，C3）；② `armSubscription(channel: Channel<FileWatchEvent>, resolvePath: () => Promise<string \| null>): () => void`（C1/C2/C4/C5 的装配与退订外壳）；③ `useFileWatchSignal(params): void`（ref 面 + `useEffect` 主体约 15 行，C1/C4 的接线）。其中常量与两个 helper 全部文件私有，导出面只有 `useFileWatchSignal` | spec 禁把窗口开成参数 ⇒ 常量进 `createDebouncer` 体内而非入参（不开第二取值面）；三个函数体分别约 12 / 18 / 15 行，稳过 `max-lines-per-function: 50`（R-7）；拆件留在文件内，不外泄导出（AC-6 / knip 零新增） |
| D2 | 最新引用 | `resolvePathRef` / `onSignalRef` 两个 `useRef`，**渲染期直接赋值**（`resolvePathRef.current = resolvePath`），effect 依赖仅 `[armKey]` | spec「`resolvePath` / `onSignal` 以最新引用生效，MUST NOT 因回调身份变化重订订阅」；渲染期赋值沿本仓既有先例 `use-change-detail.ts:75-90` `useDebouncedBump` 的 `bumpRef.current = bump`；不给消费方加 `useCallback`（identity 无关，多一层记忆化无收益） |
| D3 | 装配启动时点与「已作废」短路 | `armSubscription` 内 `void (async () => { const path = await resolvePath(); if (path === null \|\| disposed) return; … })().catch(() => {})`：`resolvePath()` 在 async 函数体**首个 `await` 之前**被同步调用 ⇒ 读到的是**本臂**的闭包，不是后续 render 的新闭包 | 与现状 `armWatch` 的 `if (path === null \|\| isCancelled()) return () => {}` 逐字同构（`path === null` 与「已作废」共用一条短路），保住「参数变更后旧路径不再订阅」这条既有语义（`旧文档迟到的 resolve` 用例所在链）；`async` IIFE + 尾部 `catch` 让 `resolvePath` 的**同步抛出**也归入同一静默面（C5） |
| D4 | 退订外壳与迟到补发 | 两个闭包变量：`disposed: boolean`、`subscriptionId: number \| null`。`dispose()` 置 `disposed = true`，并在 `subscriptionId !== null` 时 `void ipc.watchUnsubscribe(subscriptionId)`；订阅落定后若 `disposed` 已真 ⇒ **立即**以新 `id` 补发一次退订、不记入 `subscriptionId`；`path === null` 时 `subscriptionId` 恒 null ⇒ 卸载零退订 | C4 逐条；单订阅只需两个标量，不引入 `Map` / `Set`；`dispose` 天然幂等（重复调用只会在已退订时再判一次 null） |
| D5 | 「信号无载荷」的实现形态 | `channel.onmessage = signals.push`（`push` 是 0 参函数） | 载荷在读面上**无处可达**（`FileWatchEvent` 只作 `Channel` 泛型参数），比「读了但不用」更强；同时规避 lint 的未使用形参。`onSignal` 只由计时器到点触发调用 |
| D6 | 消费方取数与订阅解耦 | 取数 effect 保持 `[root, name, tick]` 与 `fetchDoc` **逐字不动**；effect 体内 `readRef.current = readDoc`（`readDoc` = `() => fetchDoc(root, name, () => cancelled, setDoc, setError, () => setLoading(false))`），cleanup 置 `readRef.current = null`；`onSignal: () => readRef.current?.()` | 「挂载即拉一次」与「同步进入 loading」仍归消费方（proposal §4 否决首拍进公共件）⇒ 既有「挂载恰一次 / 同步 `loading`」用例零影响；`readRef` 指**当前** effect 实例的闭包，其 `cancelled` 由该实例的 cleanup 置真 ⇒ 迟到信号对已作废实例自动 no-op；信号重拉经 `fetchDoc` 不置 `loading`、不清 `error`（AC-4）。effect 声明顺序在 `useFileWatchSignal` 之前 ⇒ 挂载期 ref 先就绪 |
| D7 | 臂键与空参数守卫 | 组件体内 `const armKey = root && name ? \`${root}\u0000${name}\` : null;`；`resolvePath: () => (root === null \|\| name === null ? Promise.resolve(null) : ipc.exploreDocPath(root, name))` | 臂键判据照搬取数 effect 的 `!root \|\| !name`（空串亦不订阅），语义同一处口径；NUL 作分隔符无碰撞（`name` 为 kebab stem，字符集 `[a-z0-9-]`）；守卫只为类型完备，`no-non-null-assertion` 下不能用 `!`（校准 1，残余不可达臂记入待决问题 1） |
| D8 | 两处微调的留痕位置 | **M1** 记于新件头部 doc（「重订只由 `armKey` 变化触发；数据面刷新维度 MUST NOT 进臂键 ⇒ 显式刷新不重订 watch——勿当 bug 修回去」，并在 `useEffect` 依赖 `[armKey]` 处再点一句）；**M2** 记于消费方 `useFileWatchSignal(...)` 调用处（「计时器归本件、按臂键存续 ⇒ 显式刷新与待发信号可能各拉一次 `read_explore`；两次结果幂等、不置 `loading`、无 UI 副作用」） | proposal §5 要求两处代码注释留痕（R-2 / R-3），且必须写明「被误认作 bug 时会被怎么改错」，否则留痕无效 |
| D9 | 版本与零改动边界 | `packages/desktop/package.json` `version` 0.6.2 → **0.6.3**（一次 patch）；`src-tauri/**`、`src/types/generated/**`、`knip.json`、`use-change-detail.ts`、`use-exec-output.ts` 全零 diff | AC-8 / AC-9 / AC-10；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行**不跟**（零 Rust 改动） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 共享信号消费件（**新增**） | 文件失效信号的单点消费：Channel 建立（C1）/ `resolvePath` → `watch_subscribe`（C2）/ trailing 防抖 500ms（C3）/ 卸载与臂键变化退订 + 迟到补发（C4）/ 订阅失败静默（C5）。零取数、零状态、零载荷语义、零目录知识 | `packages/desktop/src/hooks/use-file-watch-signal.ts`（新） | `@tauri-apps/api/core`（`Channel`）、`react`（`useEffect` / `useRef`）、`../lib/ipc`（`watchSubscribe` / `watchUnsubscribe`）、`../types/generated/bindings`（`import type { FileWatchEvent }`） | React hook + TS（模块私有 helper 两件） |
| explore 详情页消费方（**修改**） | 持有**取数**语义（`read_explore` + 失败落 `error` + 取消抑制）与 `loading` / `error` 状态；退化为纯取数 effect（依赖 `[root, name, tick]` 不动）+ 一次 `useFileWatchSignal` 调用（臂键 = `root`␀`name`，`onSignal` 经 `readRef` 走当前取数闭包）；导出面 `ExploreDocState` / `useExploreDoc` 不变 | `packages/desktop/src/views/explores/hooks/use-explore-doc.ts`（改） | `react`（去掉 `Channel` 直接依赖）、`../../../lib/ipc`、`../../../hooks/use-file-watch-signal`、`../../../types/generated/bindings`（仅 `ExploreDoc` 类型） | React hook |
| IPC 代理层（零改动，被依赖） | `ipc` 提供 `watchSubscribe` / `watchUnsubscribe` / `exploreDocPath` / `readExplore` 的一等调用面与运行期校验；新件与消费方**只经此层**调用（MUST NOT 裸 `commands.*`） | `packages/desktop/src/lib/ipc.ts` | 生成物 `bindings` / `bindings.zod`、`zod` | 既有装配层 |
| watch 通道侧（零改动，被依赖） | `crates/infra/watch`（notify 单文件订阅、零内容零去抖）+ `src-tauri/src/commands/watch/`（Channel 桥接薄命令、退订 miss 幂等） | `packages/desktop/src-tauri/crates/infra/watch/`、`packages/desktop/src-tauri/src/commands/watch/` | notify | Rust（本变更零 diff） |
| 版本面（修改） | `version` 0.6.2 → 0.6.3；`tauri.conf.json` 跟随，`Cargo.toml` crate 版本行不跟 | `packages/desktop/package.json` | — | JSON |

---

## 变更清单

> 路径口径：新增文件用仓库根相对全路径；修改文件与 proposal「变更范围 - 实现文件」同口径（`src/...` 省略 `packages/desktop/` 前缀）。测试文件与测试用例归测试相位，不在本清单。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src/hooks/use-file-watch-signal.ts` | 唯一导出 `useFileWatchSignal`；文件私有 `SIGNAL_DEBOUNCE_MS = 500`、`createDebouncer(onDue)`、`armSubscription(channel, resolvePath)`。头部 doc 记：① 五件事边界（C1–C5）与「信号无载荷 / 零取数 / 零状态」纪律；② 划界原则（公共件按**驱动源**切分——文件失效信号属本件，`use-change-detail` 的 run 通知防抖不在本件职责内）；③ M1 记账（重订只由 `armKey` 变化触发，刷新维度 MUST NOT 进臂键 + 参数面三项且不开 `debounceMs` / `enabled` / `retry` / 错误回调）。约 70 行 | D1 / D2 / D3 / D4 / D5 / D8；公共函数表 `useFileWatchSignal`；`specs/desktop-file-watch` 的「共享文件 watch 信号消费件」（C1–C5）与「订阅臂键与重订边界」 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | ① import 区：删 `import { Channel } from '@tauri-apps/api/core';`、删 `FileWatchEvent` 类型导入（保留 `type ExploreDoc`）、`react` 导入增 `useRef`、增 `import { useFileWatchSignal } from '../../../hooks/use-file-watch-signal';`；② 删 `WATCH_DEBOUNCE_MS`（`:7-8`）、`createDebouncer`（`:43-63`）、`armWatch`（`:66-78`）；③ 取数 effect（原 `:96-127`）去掉 watch 装配整段，保留 `!root \|\| !name` 空态分支、`cancelled` / `isCancelled`、`fetchDoc` 调用、`setLoading(true)` / `setError(null)` 与 cleanup 中 `cancelled = true`；新增 `const readRef = useRef<(() => void) \| null>(null);`（在 `useState` 之后声明），effect 体内 `readRef.current = readDoc;`、空态分支与 cleanup 中 `readRef.current = null;`；依赖面 `[root, name, tick]` **逐字不动**；④ 组件体尾部、`return` 之前增一次 `useFileWatchSignal({ armKey, resolvePath, onSignal })`——`armKey = root && name ? \`${root}\u0000${name}\` : null`、`resolvePath` 按 D7 的显式判空、`onSignal: () => readRef.current?.()`；该调用处写 M2 留痕注释；⑤ 模块头 doc 改写为「取数 + 信号订阅消费方」：保留「信号 500ms trailing 防抖合并为一次 `read_explore` 重拉 / 卸载退订 / 订阅失败不阻断数据面」的语义描述，并把防抖与退订的**持有方**指向 `../../../hooks/use-file-watch-signal`；⑥ `ExploreDocState`（`:10-17`）、`fetchDoc`（`:20-40`）、`refresh`、`return { doc, loading, error, refresh }` 逐字不动 | 文件约 130 → 约 85 行；AC-2（零 `FileWatchEvent` Channel / 零 watch 命令 / 零 `createDebouncer` / 零 `armWatch`）、AC-4（信号重拉不置 `loading`、不清 `error`）、AC-5（`resolvePath` 为 null 零退订 / 迟到补发 / 订阅失败不阻断）、AC-6（导出面不变） |
| `packages/desktop/package.json` | `version` 0.6.2 → 0.6.3 | 版本纪律（D9 / AC-10）；`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动 |

### 删除文件

<!-- 不涉及：本变更零文件删除——私有三件（WATCH_DEBOUNCE_MS / createDebouncer / armWatch）在消费方就地退场，其形态搬入新件的文件私有 helper（SIGNAL_DEBOUNCE_MS / createDebouncer / armSubscription）。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `useFileWatchSignal` | `packages/desktop/src/hooks/use-file-watch-signal.ts` | 新增（本变更唯一新增导出） | `(params: { armKey: string \| null; resolvePath: () => Promise<string \| null>; onSignal: () => void }) => void` | 文件 watch 失效信号消费件：`armKey` 非 null 才建 Channel 并订阅；`resolvePath()` 返回 null 则不订阅；信号经 500ms trailing 防抖合并为一次 `onSignal`；`armKey` 变化 / 卸载时取消待发计时并退订（迟到就绪补发退订）；订阅链任一环 reject 静默。`resolvePath` / `onSignal` 以最新引用生效，effect 依赖仅 `[armKey]`。参数面**恰三项**，不开 `debounceMs` / `enabled` / `retry` / 错误回调 |
| `useExploreDoc` | `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | 修改（**仅实现**，导出签名与语义不变） | `(root: string \| null, name: string \| null) => ExploreDocState` | 取数语义（`read_explore` + 取消抑制 + 失败落 `error`）与 `loading` / `error` 状态留在本件；watch 订阅移交 `useFileWatchSignal`（臂键 = `root`␀`name`，不含 `tick`）；信号重拉经 `readRef` 走当前取数闭包，不置 `loading`、不清 `error` |

> 模块私有（不列，均非模块级导出）：新增 `SIGNAL_DEBOUNCE_MS`（常量）、`createDebouncer`、`armSubscription`（新件内）；`createDebouncer` / `armWatch` / `WATCH_DEBOUNCE_MS` 在消费方退场；`fetchDoc`（消费方保留、零改）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `FileWatchEvent` | `packages/desktop/src/types/generated/bindings.ts` | 不变 | 只作 `Channel<FileWatchEvent>` 的泛型参数（`{ path: string }`）；本件**不读**其字段（D5）。零 bindings 重生成 |
| `ExploreDocState` | `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | 不变 | `doc` / `loading` / `error` / `refresh` 四项与语义零改（AC-6）；`explore-detail-view.tsx` 的 `ReturnType<typeof useExploreDoc>`（`:313`）因此零影响 |

> **无新增导出类型**：`useFileWatchSignal` 的参数面以**行内对象字面量**标注（与 spec Module Contract 的写法一致）。不引入具名 `interface FileWatchSignalParams`——它只会被本文件与调用点的字面量使用，导出后必被 knip 判 `Unused exports`（本仓实测过该判据，见 archive `ipc-zod-runtime-schema` 校准 1），而规则 5 禁止为测试造导出。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.3`（原 `0.6.2`） | 版本纪律（D9）；一次 patch |

> 无新增环境变量 / feature flag / 运行期配置键。防抖窗口 `SIGNAL_DEBOUNCE_MS = 500` 是新件内的**编译期常量**（非配置键、非参数、不导出）——单点声明在 `createDebouncer` 所在文件顶部，消费方 MUST NOT 自建防抖窗口。

---

## 数据模型

持久化面**零改动**（无表结构、无 bindings、无配置 schema 变更）。本变更引入的是一层**订阅生命周期状态机**（全部在内存、随组件生命周期生灭）：

```
render(root, name, tick)
├── armKey = root && name ? `${root}\u0000${name}` : null      ← 订阅身份（寻址维度，不含 tick）
│
├── 取数 effect [root, name, tick]                             ← 消费方（use-explore-doc）
│    ├── 空参（!root || !name）: doc/error/loading 复位 + readRef = null
│    └── 非空: cancelled=false; readRef = readDoc; loading=true; error=null; readDoc()
│         cleanup: cancelled = true; readRef = null
│         ⇒ 信号经 readRef 调到的永远是**当前实例**的闭包；旧实例因 cancelled 而 no-op
│
└── useFileWatchSignal effect [armKey]                          ← 新件（use-file-watch-signal）
     ├── armKey === null: 零 Channel 构造、零 watch 命令、零退订（cleanup 不存在）
     └── armKey !== null:
          channel = new Channel<FileWatchEvent>()
          signals = createDebouncer(() => onSignalRef.current())   // timer: null | Timeout，窗口 500ms
          channel.onmessage = signals.push       // 载荷不可达（D5）
          dispose = armSubscription(channel, () => resolvePathRef.current())
                 ├── 同步段: 调用 resolvePath()（本臂闭包）
                 ├── path === null || disposed → 不订阅（subscriptionId 留 null）
                 ├── await watch_subscribe → id
                 │     ├── disposed 已真 → 立即 void watch_unsubscribe(id)（迟到补发）
                 │     └── 否则 subscriptionId = id
                 └── 任一环 reject → 静默（不抛、不重试、不新增错误通道）
          cleanup: signals.cancel(); dispose()   // 取消待发计时 → 若已有 id 则退订一次
```

**状态字段与不变量**：

| 状态 | 归属 | 初值 | 不变量 |
|------|------|------|--------|
| `timer` | `createDebouncer` 闭包 | `null` | `push` 先清旧计时再挂新计时（trailing：窗口内每次信号重置，最后一拍必达）；`cancel` 幂等 |
| `disposed` | `armSubscription` 闭包 | `false` | 一经 `true` 永不复位（每臂一个新闭包）；`true` 后不再记录新 `subscriptionId` |
| `subscriptionId` | `armSubscription` 闭包 | `null` | 至多写入一次（首订阅）；`dispose` 只在非 null 时退订恰一次 ⇒ 卸载零重复退订 |
| `readRef` | 消费方 `useRef` | `null` | 只在取数 effect 体内赋值、在其 cleanup / 空态分支置 null ⇒ 其指向的闭包的 `cancelled` 与当前 effect 实例一致 |
| `doc` / `loading` / `error` | 消费方 `useState` | `null` / `false` / `null` | 只有 `fetchDoc`（取数）与 effect 首拍写这三个；新件零触碰（AC-4 / AC-5） |

**关系**：一条 watch 订阅 ⟷ 一个 `armKey`（1:1，由 effect 依赖保证）；`armKey` 与「`root` + `name`」一一对应（NUL 分隔无碰撞，`name` 为 kebab stem）。`armKey` 变化 ⇒ 旧臂 `dispose`（退订）→ 新臂装配（重新 `resolvePath` + 订阅），顺序由 React cleanup 先于新 effect 保证。

**持久化**：无。订阅 id 只在进程内存在，进程退出由后端通道关闭回收（既有语义）。

---

## 路由 / API 设计

零新增 / 零改动 HTTP 路由与 IPC 命令、零 bindings 重生成。既有面保持，仅**调用方变更**：

| 面 | 变更 | 说明 |
|----|------|------|
| IPC `explore_doc_path` `(root, name) → string \| null` | 调用方由 `use-explore-doc.ts` 的 `armWatch` 移到同一文件的 `resolvePath` 回调（仍经 `ipc` 代理） | 布局派生路径仍由命令层给出（前端零目录知识）；返回 null = 不订阅 |
| IPC `watch_subscribe` `(onEvent: Channel<FileWatchEvent>, path) → number` | 调用方移到 `src/hooks/use-file-watch-signal.ts` 的 `armSubscription`（经 `ipc` 代理） | 生产代码唯一调用点（AC-2）；`Channel` 实例仍在该件内构造 |
| IPC `watch_unsubscribe` `(subscriptionId) → boolean` | 同上（新件 `dispose` 与迟到补发两处调用，同一消费点） | miss 幂等返回 `false`（后端既有语义） |
| IPC `read_explore` `(root, name) → ExploreDoc \| null` | **调用点与入参零改**（消费方 `fetchDoc`） | 信号触发的重拉仍走同一命令、同一取消抑制链 |
| 路由 `/explores/:name` | 零改 | 详情页仍在页面生命周期内保持订阅；切 `name` 经 `armKey` 变化重订 |

---

## 依赖

运行时依赖：**无新增**（新件只用既有 `@tauri-apps/api/core` 的 `Channel`、`react` 的 `useEffect` / `useRef`、既有 `ipc` 代理与生成物类型）。

构建 / 测试依赖：**无新增**（`vp check` / `vp test` / `knip` / `bindings:check` 形态与阈值不动；测试装置仍 mock `@tauri-apps/api/core` 的 `invoke` / `Channel`，新件经 `ipc` 落到同一 mock 面）。

| 依赖 | 变更 | 用途 |
|------|------|------|
| `react`（`useEffect` / `useRef`） | 不变 | 订阅生命周期与最新引用（新件）；既有 `useState` / `useCallback`（消费方） |
| `@tauri-apps/api/core`（`Channel`） | 不变（**消费方不再直接 import**） | `Channel<FileWatchEvent>` 构造与 `onmessage` 挂派发 |
| `../lib/ipc`（`ipc` 代理） | 不变 | `watchSubscribe` / `watchUnsubscribe` / `exploreDocPath` / `readExplore` 的一等调用面（运行期校验 + 漂移上报） |
| `../types/generated/bindings` | 不变（仅 `import type`） | `FileWatchEvent`（新件）/ `ExploreDoc`（消费方） |
| 覆盖率阈值（lines 90 / branches 85 / functions 90） | 不变 | 新件分支由既有 + 新增用例穿透（R-8）；残余不可达臂见待决问题 1 |

---

## 验收标准落点

| ID | 设计落点 |
|----|----------|
| AC-1 | D6：取数 effect 的依赖面、`fetchDoc`、`setLoading(true)` 首拍、`cancelled` 抑制链逐字不动；新件只在消费方 effect **之后**挂接 ⇒ 既有用例（含「挂载恰一次 / 同步 loading / 参数变更竞态 / 迟到 reject」）零改动 |
| AC-2 | 新增文件表 + 修改文件表 ②：`new Channel<FileWatchEvent>` / `ipc.watchSubscribe` / `ipc.watchUnsubscribe` 三处生产命中全落 `src/hooks/use-file-watch-signal.ts`；消费方零 `FileWatchEvent` Channel、零 watch 命令、零 `createDebouncer` / `armWatch`（起点核实的基线对照） |
| AC-3 | D7 + D2：`armKey` 不含 `tick` ⇒ 显式 `refresh()` 只重跑取数 effect、不重跑订阅 effect（`watch_subscribe` 1 次 / `watch_unsubscribe` 0 次）；`armKey` 含 `name` 与 `root` ⇒ 二者变更仍退订 + 重订 |
| AC-4 | D6 + D5：`onSignal` 走 `readRef.current()` → `fetchDoc`（不置 `loading`、不清 `error`）；防抖窗口 500ms trailing 归 `createDebouncer`，窗口内信号重置计时 |
| AC-5 | D3 + D4：`path === null` → 不订阅且 `subscriptionId` 留 null（卸载零退订）；订阅链 reject → `catch` 静默（不冒泡错误态、不阻断取数）；`disposed` 已真时新 id 立即补发退订 |
| AC-6 | 公共函数表：新件仅导出 `useFileWatchSignal`；消费方导出面仍 `ExploreDocState` / `useExploreDoc`；类型定义表「无新增导出类型」+ 依赖表（新件被生产文件 import ⇒ knip 零新增） |
| AC-7 | D1（三个函数体 ≤ 20 行，过 `max-lines-per-function: 50`）+ 修改文件表 ⑤（头 doc 改写不改语义）+ 依赖表（零 Rust / 零 bindings 改动 ⇒ `bindings:check` 无 diff） |
| AC-8 | D9：`src-tauri/**` 零文件改动，`src-tauri/Cargo.toml` crate 版本行不跟 |
| AC-9 | D9 + 架构组件表「watch 通道侧（零改动）」：`use-change-detail.ts` / `use-exec-output.ts` / `knip.json` 零 diff（划界原则见新件头 doc） |
| AC-10 | 配置表：`version` 0.6.2 → 0.6.3 |
| AC-11 | `specs/desktop-file-watch/spec.md`（delta，提案阶段已定稿）：Module Contract 已指向 `src/hooks/use-file-watch-signal.ts` 并含 explore 详情页消费方行；`MODIFIED` / `ADDED` 段与 D1–D4、D7 的落点一一对应（实施阶段只核对，不重写） |
| AC-12 | 修改文件表 ④：信号重拉不置 `loading` ⇒ `ExplorePreview` 的 `preview-loading` 不因外部保存闪动；`armKey` 不含 `tick` ⇒ 点刷新后 watch 不重建；`armKey` 含 `root` ⇒ 切 workspace 触发 `dispose`（零残留订阅） |

---

## 待决问题

1. **`resolvePath` 空参数臂的不可达性（校准 1）**：`armKey` 非 null ⇒ `root` / `name` 必非 null，故消费方的 `Promise.resolve(null)` 臂无公开路径可达。本设计接受这一残余臂（branch 覆盖 93.78% 基线有余量），若覆盖率 / 突变门禁因此不足，由测试相位按其判据处置（补用例或按门禁结论处理）；MUST NOT 通过「让本件在 `armKey === null` 时也调 `resolvePath`」换覆盖率，也 MUST NOT 加 knip / coverage 豁免。
2. **参数面具名类型的取舍**：本期不导出 `FileWatchSignalParams`（knip 会判 unused）。若后续第二消费方需要显式标注参数类型（例如把 `useFileWatchSignal` 的调用包进一个工厂），届时再提具名 `interface` 并同步 spec；那时的前提是它真有 ≥1 处生产消费。
3. **第二消费方形态校验（R-1 / R-6）**：本件参数面只按「explore 详情页 + 未来 exec 输出跟随」共同形态设计（`armKey: null` 覆盖非运行态）。若后续 change 接入时发现需要 `debounceMs` / `enabled` / 错误回调，返工面固定在本件（约 70 行）+ 单一消费点，MUST NOT 提前开空参数。
4. **`desktop-explore-page` Module Contract 的归属漂移（proposal R-5）**：`use-explore-session` 行把「watch 信号防抖 → `read_explore`」挂在会话 hook 上属存量漂移，本变更不修（改它会与「只做抽取」的范围纪律冲突），留给触及该 hook 的后续 change。
5. **既有用例计数口径**：proposal 记 23 例，文件实况 24 个 `it` / `it.each` 站点（表驱动展开更多）。判据取「既有断言行零删改」，精确条数由测试相位核对，不影响本设计落点。
