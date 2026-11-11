# 测试设计: agent-messages-virtual-list-pinning

> **变更**: agent-messages-virtual-list-pinning
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（变更范围 / 测试文件表 / 新增用例 ①–⑪ / AC-1..AC-13 / R-1..R-14）、`design.md`（D1–D19 / 七则事实偏差 / 架构组件 5 行 / 新增文件 1 项 + 修改文件 4 项 + 删除文件 1 项 / 公共函数·API 表 13 行 / 类型定义 9 行 / 数据模型几何账目 + 五条不变量 + 贴底复位判据通道 / AC 落点 / 待决问题 1..7 / 实现期落定 8–11）、两份 delta spec（`desktop-agent-chat-infra`：3 MODIFIED + 1 ADDED 11 scenario；`desktop-explore-page`：1 MODIFIED）、`tasks.md`（七阶段 + 编译闭合批）、工作树源码与既有测试盘点
> **被测范围**: design「变更清单」的**新增文件 1 项 + 修改文件 4 项全列**——`packages/desktop/src/lib/agent-virtual.ts`（新增）、`packages/desktop/src/components/ui/message-scroller.tsx`、`packages/desktop/src/components/agent/agent-messages.tsx`、`packages/desktop/src/test-setup.ts`、`packages/desktop/package.json`；**删除文件 1 项**（`packages/desktop/src/components/ui/questionnaire.tsx`）按规程**不纳入被测范围**——其正确性主张是「文件不存在且依赖无引用」，判据归 `grep` / `git diff` / `knip`（见「不可测试项」）。
> **契约基准**: 用例一律按 `design.md` 的**定稿契约**设计（含 D10 / 实现期落定 11 的 `resetKey` 复位通道、落定 8 的吸附层与虚拟切片按索引交织、落定 9 的 `z-10`、`Viewport` 的 `role`/`tabIndex` 与 `Button` 的可达性对齐），不按工作树当前形态回填用例。
> **契约与工作树的已知差异（用例按契约侧写、差异由实现相位补齐）**: ①`agent-virtual.ts` 现有三函数，`conversationIdentity` 尚无（AC-13 的 `conversationIdentity` 全族用例在其落地后才有可测对象）；②shim 的贴底复位当前仍是「`count > 0` 且尚未贴过」的一次性闩、`MessageScrollerProviderProps` 无 `resetKey`（⑥′⑥″ 与相关边界 / 反向用例依赖该入参与复位判据落地）；③其余契约（交织挂载、`z-10`、`role`/`tabIndex`、`measureWithFallback`、`useStickyMeasure`）工作树已一致。测试相位不因差异删用例，也不把用例改写成一次性闩的形态。
> **本变更性质**: 纯前端呈现层重构——对话透镜从「自研分页窗口 + 全量 DOM」改为「全量虚拟窗口 + 用户提问 sticky 常驻吸附」。Rust 后端 / 适配层 / transport / 会话基建 hook / 生成物**全部零改动**；零新增 IPC 命令、零新增 crate、零新增配置键；依赖净变化 = 增 `@tanstack/react-virtual@3.14.26`、删 `@shadcn/react`。用例集中在三个前端代码面：过滤与吸附几何纯函数、滚动承载 shim 的结构与复位契约、透镜的虚拟窗口 / 吸附 / 复位呈现；**不为覆盖率造用例**。

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`；`vp check` 的 `typeCheck` 为编译闭合判据）；`vite.config.ts` 的 `test` 节：`environment: 'jsdom'`、`include: ['src/**/*.test.ts?(x)']`、`setupFiles: ['src/test-setup.ts']`、`testTimeout: 20000` | 与被测源文件**同目录共置**的 `<模块名>.test.ts`（`.test.tsx` 仅组件渲染面） | `import { describe, expect, it, vi, beforeEach, afterEach } from 'vite-plus/test'` + `@testing-library/react`（`render` / `rerender` / `screen` / `within` / `act` / `fireEvent` / `waitFor`）；组件断言以 `data-testid` 与 `data-slot` 为锚点（既有 `agent-messages.test.tsx` / `agent-debug-view.test.tsx` 定式）；迭代类型标题为中文长句 |
| 测试文件豁免与门禁面 | `vite.config.ts` 的 `lint.overrides`：`*.test.ts(x)` 关 `max-lines-per-function` / `no-non-null-assertion` / `no-unsafe-type-assertion` / `no-unnecessary-type-assertion`；`NO_STATIC_CHECK` 含 `src/components/ui/**`（fmt / lint 双忽略） | `src/components/ui/message-scroller.test.tsx` 为该目录首个测试文件，沿 `src/components/agent/*.test.tsx` 的渲染面定式 | `knip.json` 的 `ignore` 含 `src/components/ui/**` ⇒ shim 的导出面不受死代码门禁约束；`src/lib/**` **不在** `ignore` 面 ⇒ `agent-virtual.ts` 四个导出必须各有**生产**消费（由 `agent-messages.tsx` 承接，非测试引用） |
| 纯函数面 | 同上（jsdom 环境不影响纯函数） | `src/lib/*.test.ts`（先例 `explore-groups.test.ts` / `agent-adapter.test.ts`） | 零 mock 直调；fixture 构造器 + 「不变量断言」辅助函数（先例 `explore-groups.test.ts` 的 `ids()` / `expectSorted()`）；导出键集合断言先例 `agent-adapter.test.ts` 的 `Object.keys(module).sort()` |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、`null`、极值、恰好阈值、生命周期交错点、类型签名外的脏值）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增断言**；未带前缀的为**新增**或**按新形状改写**（改写只动条数断言的分查方式与 `data-slot` 锚点，语义断言逐条保留——R-3）。
- **`测试条件` 内的「⑥′ / ⑥″ / ⑦…⑪」**：proposal「新增用例」表的编号，用于 AC-13 等新契约面的对齐。
- **参数类型边界口径（贯穿各章节）**：`AgentUIMessage[]` 取 `[]` / 单元素 / 1000 元素 / `undefined`（越签名脏值）+ 常规；`start` / `totalSize`（number）取 `0` / `-1` / `Number.MAX_SAFE_INTEGER` / `undefined` + 常规；`count` 取 `0` / `1` / `3` / `100` / `1000`；`resetKey`（`string | null | undefined` 三态）取 `undefined`（调用点不提供）/ `null`（会话身份未知）/ 相等等长串 / 不等串；`span` 与 `sessionId` 类字符串取 `''` / 超长（>1000 chars）/ 含 `\n\0 emoji` + 常规；`TurnSummary['status']`（四值枚举）取全 4 值 + 缺失轮行 + 非法值；`parts` 部件类型（联合类型枚举）取 `text` / `reasoning` / `tool-*` / `dynamic-tool` / `data-*` / `step-start` / 非法值。

**待测功能条目口径**

`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目（`agent-virtual.ts` 4 行；`message-scroller.tsx` 8 行含 1 行退役；`agent-messages.tsx` 1 行；`test-setup.ts` 与 `package.json` 在该表零行 ⇒ 无 `#### 待测功能` 条目，登记于「不可测试项」）。design 明示的私有实现件（shim 的 `measureWithFallback` / `useStickyMeasure` / `useScrollerContext` / `MessageScrollerSlice` / 复位 effect，`agent-messages.tsx` 的 `renderItem` / `renderSticky` 闭包与 `ToolPartView` / `PartView` / `TurnStatusLine`）**不列本表**；其覆盖经所在文件的公共 API 用例承接——不 grep 源码导出声明、不虚构 API 行。design「类型定义」表声明的 interface（`UserTurnSpan` / `MessageScrollerProviderProps` 等）不是「公共函数 / API」行，形状覆盖并入对应函数的入参断言，类型错由 `vp check` 的 tsc 强制（见「不可测试项」）。

**测试布局决策（源文件 → 测试文件的映射依据）**

1. **新建 2 个测试文件**：`src/lib/agent-virtual.test.ts`（proposal 测试文件表第 1 行）与 `src/components/ui/message-scroller.test.tsx`（第 2 行），均与被测源文件同目录共置，命名随仓库定式。
2. **复用 1 个既有测试文件**：`src/components/agent/agent-messages.test.tsx` 按 proposal 处理列**删除**「历史分页」5 例 + 「大列表分页」1 例 + `FakeIntersectionObserver` / `triggerReachTop` 装置（该装置在数段 `describe` 的 `beforeEach` 里复用，删除后相应 `beforeEach` / `afterEach` 一并清空），**条数断言改按 `data-role` 分查**，语义断言逐条保留（R-3），并新增虚拟化 / 吸附 / 换会话复位用例。
3. **`test-setup.ts` 与 `package.json` 无测试路径**：前者是测试基座（design 明示「本文件是测试基座而非测试用例」），后者是配置 / 元数据；两者记入「不可测试项」并给替代验证。
4. **组合用例挂靠按链路发起方**：虚拟窗口 + 吸附 + 贴底复位的端到端呈现链路发起方是 `AgentMessages`（它派生 `count` / `estimateSize` / `renderItem` / `renderSticky` / `resetKey` 并消费几何纯函数）⇒ 「1000 条只渲切片」「user 常驻且虚拟层零节点」「吸附层几何与账目一致」「换会话复位」等组合行挂 `agent-messages.tsx` 章节；shim 自身的结构契约（`data-slot` 钩子、`top` 定位、`transform` 缺席、零测量回落、`resetKey` 复位时机）挂 `message-scroller.tsx` 章节。不新建独立集成测试章节。
5. **渲染 memo 语义的补测落在 `agent-render-memo.test.tsx`**：proposal 明示该文件「补一条『虚拟窗口重渲不重建历史 assistant 项』」；该行挂 `agent-messages.tsx` 章节的组合类，行内标注**执行文件是 `agent-render-memo.test.tsx`**（archive 先例 `2026-11-07-agent-stream-single-message` 的同一手法）。
6. **不 mock 被测件与被测链路内部**：`agent-virtual` / `message-scroller` / `agent-messages` 之间，以及与 `agent-blocks` / `tool-cards` / `parse` / `bubble` / `message` / `trace` / `cn` / `button` 全部真实组合——mock 它们等于让断言自证（AGENT.md「No test-only exports」）；`@tanstack/react-virtual` **不许**替身化（账目 / range / 测量缓存是被测输出本体）。
7. **AC-13 的滚动写入可观测（本版新增前提，取代旧口径「滚动位置类断言一律不写」）**：jsdom 的 `Element` 无 `scrollTo`（虚拟器写入侧是 `scrollElement?.scrollTo?.({...})`），故首帧贴底与换会话复位默认静默无效。用例在**实例级**给视口装最小 `scrollTo` 桩（按参数写 `scrollTop` / `scrollLeft`，**不派发 `scroll` 事件**），并同时装实例级 `scrollTop` 读写访问器（jsdom 原生 `scrollTop` 不写回）与抬高实例级 `scrollHeight`，使「写入发起 + 目标 offset」可机械断言。桩**不进** `test-setup.ts` 的原型（否则改变其余套件「写入静默无效」的既有语义）。仍然不写断言的部分：真实浏览器的滚动效果、`followOnAppend` 的手感、sticky 的真实吸顶 / 顶走——归手工验证（见「不可测试项」）。

---

## 验收范围

<!-- 测试区域：前端 `packages/desktop/src/**/*.test.ts(x)`（与源文件同目录共置；运行器 `vp test`，jsdom，`setupFiles: src/test-setup.ts`）。本变更新建 2 个测试文件、改写 1 个既有测试文件、补 1 条既有套件用例；其余套件（`agent-session-view.test.tsx` / `session-panel.test.tsx` / `index.test.tsx` / `src/views/**` 会话面板套件）只做「打桩后仍可见」的回归确认，其结构性断言零改动，见「不可测试项」的回归登记段。 -->
<!-- 删除文件 `packages/desktop/src/components/ui/questionnaire.tsx` 不纳入被测范围：无源文件即无测试映射对，其判据为「无测试引用 + `grep` 零 import + `knip` 零 unused dependency」（AC-10，见「不可测试项」）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|------------------|----------------|
| AC-1 | **全量虚拟窗口**：1000 条过滤后消息挂载时 DOM 中的 assistant 气泡数 ≪ 1000（只渲可见切片 + overscan），content 高度 = `getTotalSize()`；滚动条比例由全量账目导出（一次装完、无「加载更早」） | `packages/desktop/src/components/agent/agent-messages.tsx` → `agent-messages.test.tsx`（⑦）；`packages/desktop/src/components/ui/message-scroller.tsx` → `message-scroller.test.tsx`（④） |
| AC-2 | **分页面退役**：`useHistoryWindow` / `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `remaining` / `history-load-hint` / `history-load-complete` / `topSentinelRef` 在 `src/` 零残留；shim 侧 `useMessageScroller*` / `scrollToMessage` / `@shadcn/react` 零残留；测试侧 `FakeIntersectionObserver` / `triggerReachTop` 零残留 | 静态检索判据见「不可测试项」；运行期侧证：`agent-messages.test.tsx`（`history-load-*` 零命中、零 `IntersectionObserver` 装配）+ `message-scroller.test.tsx`（导出键集合恰七项） |
| AC-3 | **user 消息常驻吸附**：全部 user 角色消息恒在 DOM 的吸附层（`data-slot="message-scroller-sticky"`，各恰一个节点），虚拟层对 user 索引零节点；滚动使某轮提问完全出顶时该气泡停于视口顶端（布局位置不变），DOM 仍只一份 | `agent-messages.tsx` → `agent-messages.test.tsx`（⑧⑩）；`message-scroller.tsx` → `message-scroller.test.tsx`（⑤） |
| AC-4 | **顶走与边界**：包含块高度 = 该轮跨度（`span`），下一轮提问到达其底边即把上一条顶走（零缓冲）；滚回会话顶部时无气泡吸顶、无第二份气泡；无 user 消息时不渲吸附层 | `packages/desktop/src/lib/agent-virtual.ts` → `agent-virtual.test.ts`（②③）；`message-scroller.test.tsx`（吸附层 `height = span` 内联样式行）；真实吸顶 / 顶走视觉见「不可测试项」（手工滚动验证） |
| AC-5 | **吸附零 scroll 状态**：吸附层渲染零 range 依赖、零自建 `scroll` 监听、零 `pinned` state（判定纯 CSS）；吸附层以 `top` 定位且其祖先链无 `overflow` 裁剪类 | `message-scroller.test.tsx`（`transform` 缺席、`top` 定位、`overflow-*` 反向判据、`pointer-events` 隔离）；`addEventListener('scroll'` / `rangeExtractor` / `onChange` 静态检索见「不可测试项」 |
| AC-6 | **高度回填与账目一致**：吸附层每条气泡 `ResizeObserver` 实测后 `resizeItem(index, h)`，`totalSize` = Σ 实测（含 user 高度），虚拟项无重叠、末项底边与 content 底边对齐；实测 ≤ 0 时回落 `estimateSize`（项 MUST NOT 零高） | `message-scroller.tsx` → `message-scroller.test.tsx`（⑥ 零测量回落 + 正常测量路径）；`agent-messages.tsx` → `agent-messages.test.tsx`（几何账目、末项对齐、吸附层回填） |
| AC-7 | **视觉零装饰且既有 UI 保留**：吸附气泡与普通气泡同组件同 tinted 变体（仅多 `bg-card` 底衬），无渐变 / 阴影 / 提示层；Viewport 保留 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content` 与 `data-slot` / `aria-label="对话消息"`；`data-autoscrolling` 随滚动置位、到底按钮 `data-active` 由距底距离判定 | `message-scroller.tsx` → `message-scroller.test.tsx`（类名与钩子逐字、HUD 条件输出、按钮判据）；`agent-messages.tsx` → `agent-messages.test.tsx`（既有「容器组合」用例改写为虚拟化形态、吸附气泡同渲染件） |
| AC-8 | **过滤判据语义零漂移**：`filterConversation` 与改造前 `useHistoryWindow` 的过滤结果逐族相等（system 过滤 / 空内容 `failed` / `stopped` 可见 / `completed` 与无轮行剔除 / 只含被消费孤儿搜索 result 的 0 部件轮不进透镜）；既有空内容轮用例的语义断言保留 | `agent-virtual.ts` → `agent-virtual.test.ts`（①）；`agent-messages.tsx` → `agent-messages.test.tsx`（空内容轮组按 `data-role` 分查改写） |
| AC-9 | **测试可渲染**：jsdom 下虚拟窗口产出可见 range（`test-setup.ts` 打桩）、滚动写入可观测（用例内实例级 `scrollTo` 桩），`agent-messages` / `agent-session-view` / `agent-render-memo` / `agent-debug-view` / `explore-detail-pane` / `change-chat-panel` / `session-transcript-panel` / `detail-drawer` / `archi-chat-panel` 全套绿；条数断言一律按 `data-role` 分查，文本 / 存在类断言逐字保留 | `packages/desktop/src/test-setup.ts`（测试基座，无独立断言路径 ⇒ 「不可测试项」）；运行期侧证：`message-scroller.test.tsx` 的「视口桩生效 ⇒ range 非空」行 + 全部受影响套件的回归登记 |
| AC-10 | **依赖与版本**：`@tanstack/react-virtual` 入 `dependencies`（`3.14.26`）；`@shadcn/react` 从 `dependencies` 移除、`components/ui/questionnaire.tsx` 删除（primitives 退役后仓内零**可达**消费者，`knip` 零 unused dependency，MUST NOT 新增 `ignoreDependencies` / `ignore` 豁免）；`package.json` `0.6.31 → 0.6.32`（一次 patch）；`src-tauri/Cargo.toml` crate 版本行不变 | `packages/desktop/package.json`（**配置 / 元数据面** ⇒ 「不可测试项」，判据为 diff 审查）；`components/ui/questionnaire.tsx`（删除面 ⇒ 「不可测试项」，判据为 `grep` / `git diff` / `knip`） |
| AC-11 | **门禁**：`pnpm -C packages/desktop run check` 绿（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check` 含 `vp check` + `knip`）；`cargo nextest run --workspace` 全绿 | 机械门禁面 ⇒ 「不可测试项」；支撑面是本表其余各行用例与新增套件的全绿 |
| AC-12 | **边界零 diff**：`src-tauri/**`、`src/types/generated/**`、`src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/hooks/**`、`components/agent/{agent-blocks,agent-input,tool-cards,session-panel,agent-session-view,agent-error}.{tsx,ts}`、`components/ui/**`（除 `message-scroller.tsx` 与 `questionnaire.tsx` 的删除）、`src/views/**`（源码）、`.dev-team/architecture/**`、`knip.json`、`explore.md` 全部零 diff | 零 diff 面清单 ⇒ 「不可测试项」；行为侧证是 `src/views/**` 与 `agent-session-view.test.tsx` 的既有断言在打桩后仍绿（回归登记） |
| AC-13 | **首帧与换会话贴底**（复位机制限在 `agent-messages.tsx` 与 `components/ui/message-scroller.tsx` 之内）：消息集合首次非空或由空变 N（N > 0）时视口在 layout 阶段贴底一次；同一实例换会话（两侧 `conversationIdentity` 均非 `null` 且不等，count 可相同）再贴底一次；同一会话内追加消息、以及复位键单侧为 `null`（首轮 `record` 落库 / 收口重放）都不触发复位（上滑用户不被拽走）。复位判据经 `agent-virtual.ts` 的 `conversationIdentity` + shim 的 `resetKey` 通道 ⇒ `AgentMessagesProps` 形状不变，`use-agent-session*` / `agent-session-view.tsx` / `src/views/**` 零 diff | `agent-virtual.ts` → `agent-virtual.test.ts`（`conversationIdentity` 语义）；`message-scroller.tsx` → `message-scroller.test.tsx`（⑥′⑥″ 复位时机 / 反向不复位，实例级 `scrollTo` 桩）；`agent-messages.tsx` → `agent-messages.test.tsx`（⑪ `rerender` 换会话复位 + 同会话追加不复位）；零 diff 判据见「不可测试项」 |

---

## 单元测试

### `packages/desktop/src/lib/agent-virtual.ts` -> `packages/desktop/src/lib/agent-virtual.test.ts`

**新建测试文件**（纯函数套件，零 mock、零 jsdom 布局依赖）。口径沿 `src/lib/explore-groups.test.ts` 的定式：`describe` 按契约族切分（可见部件判据 / 过滤判据 / 吸附几何 / 会话身份），fixture 用自建构造器覆盖判别字段，几何不变量抽成断言辅助函数（`ids()` / `expectMonotonicStarts()` 一类）。`AgentUIMessage` fixture **优先由 `foldSessionMessages` 真实折叠产出**（与 `agent-messages.test.tsx` 同一装置），需要越过类型签名的脏值（非法 `part.type` / 非法 `status` / `undefined` 入参）时才用字面量构造 + `as` 断言（测试文件豁免 `no-unsafe-type-assertion`）。

#### 待测功能

- `hasVisibleParts(message: AgentUIMessage) => boolean`: 存在 `text` / `reasoning` / `tool-<name>` 部件即为真；`data-*` 与 `step-start` 不计（实时态的空壳消息由此不产空气泡）。`tool-` 前缀判据在本文件内**复写**（`isToolPartType`，不 import `agent-blocks` 的 `isToolPart`——`src/lib/**` 不得反向依赖 `components/`）。两个生产消费点：`filterConversation` 与 `ChatBubbleBase` 的空部件状态行判据。
- `filterConversation(all: AgentUIMessage[]) => AgentUIMessage[]`: 对话透镜可见性单点（自 `useHistoryWindow` 抽出，判据语义逐字等价、顺序保持、不 in-place）：剔除 `role === 'system'`；空内容 assistant 消息仅 `metadata.record.status` 为 `failed` / `stopped` 时保留，`completed` 或轮行缺席时剔除。
- `userTurnSpans(messages: AgentUIMessage[], measurements: readonly { start: number }[], totalSize: number) => UserTurnSpan[]`: 吸附层每条占位块的几何——仅为 `role === 'user'` 的索引产条、按索引升序；`start = measurements[index].start`；`span = 下一 user 的 start − 本条 start`，末轮 `span = totalSize − start`；`measurements[index]` 缺席时该条跳过、末轮退化到 `totalSize`；`span` 下限钳制为 0；无 user 消息返回空数组。
- `conversationIdentity(messages: AgentUIMessage[]) => string | null`: 贴底复位键——取**首条**携 `metadata.record.sessionId` 的消息的会话 id；全体缺席返回 `null` = 未知。同一会话的追加与流式增长不改变返回值，换会话必变（AC-13 的纯函数半边：`null` 不参与换会话比较，故首轮 `record` 落库不拽回上滑用户）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `hasVisibleParts` | 正向 | 单部件三形态：`text` / `reasoning` / `tool-Bash` 各一 → 均为 `true`（三类计入可见部件的正面判据） | 参数化迭代（3 部件类型） |
| `hasVisibleParts` | 正向 | 混合部件序（`text` 与 `tool-*` 与 `reasoning` 交错）→ `true`；文本内容为空串的 `text` 部件**仍为 `true`**（「存在」而非「非空」） | 单次 |
| `hasVisibleParts` | 异常 | 只含 `data-system-notice` / `data-run-started` / `data-raw` / `step-start` 的部件集 → `false`（`data-*` 与 `step-start` 不计；实时态空壳消息不产空气泡） | 参数化迭代（4 部件类型 + 1 混合集） |
| `hasVisibleParts` | 异常 | 前缀伪装与保留字四形态：`dynamic-tool` / `tool`（无连字符）/ `tool-`（空名）/ `data-tool-x` → 均 `false`（判据是 `tool-` 前缀且排除 `dynamic-tool`，不误吞相邻命名） | 参数化迭代（4 形态） |
| `hasVisibleParts` | 异常 | 越过联合类型的脏 `part.type`（如 `'mystery-part'`）→ `false` 且不抛错（判据为白名单式包含，不枚举黑名单） | 单次 |
| `hasVisibleParts` | 边界 | `parts = []` → `false`（空数组不短路抛错） | 单次 |
| `hasVisibleParts` | 边界 | 超长文本部件（>1000 chars）与含 `\n\0 emoji` 的部件 → `true`（判据不看内容长度与字符集） | 参数化迭代（2 载荷形态） |
| `filterConversation` | 正向 | 常规会话（`system` 载体 + `user` + 有内容 `assistant`，多轮）→ 输出剔除全部 `system`、保序（逐元素 `id` 与输入非 system 子序列相等），且输出数组与入参数组**非同一引用**（不 in-place） | 单次 |
| `filterConversation` | 正向 | 空内容 assistant（`parts = []`）× 有轮行 `failed` / `stopped` → 两条**均保留**（状态行载体不得被吞）；保留项 `metadata.record.status` 原样可读 | 参数化迭代（2 状态） |
| `filterConversation` | 正向 | 有内容 assistant + 轮行 `failed` / `stopped` → 保留且**不额外标记**（判据只看「有可见部件」即放行）；与「空内容 + failed 保留」构成同族等价 | 参数化迭代（2 状态） |
| `filterConversation` | 异常 | 空内容 assistant + 轮行 `completed` → 剔除（无空气泡、无状态行） | 单次 |
| `filterConversation` | 异常 | 空内容 assistant + `metadata.record = null`（轮行缺席）→ 剔除（不因状态缺失而误放行） | 单次 |
| `filterConversation` | 异常 | 空内容 assistant + 轮行 `running` → 剔除（判据只放行 `failed` / `stopped`；轮状态读取零改写） | 单次 |
| `filterConversation` | 异常 | 越过类型的非法 `record.status`（如 `'paused'`）→ 同 `completed` 处理（剔除）、不抛错（判据为白名单） | 单次 |
| `filterConversation` | 异常 | 全部为 `system` 载体消息（runStarted / systemNotice / raw 折叠产物）→ `[]`（空态判定基于过滤后列表） | 单次 |
| `filterConversation` | 异常 | `user` 角色消息 `parts = []`（越签名构造）→ 判据不特判角色：无可见部件且无 `failed` / `stopped` 轮行即剔除；用例内钉死该行为（唯一的角色条件是 `system` 剔除） | 单次 |
| `filterConversation` | 边界 | `all = []` → `[]`（空数组、不抛错） | 单次 |
| `filterConversation` | 边界 | 单元素三形态：单 `system` → `[]`；单有内容 `assistant` → 原样一条；单 `user` → 原样一条 | 参数化迭代（3 形态） |
| `filterConversation` | 边界 | 超大列表 1000 条（`system` / 有内容 / 空内容 `completed` / 空内容 `failed` 混合）→ 输出条数 = 期望条数、保序、`id` 序列与手工推导相等、不抛错 | 单次 |
| `filterConversation` | 边界 | 「只含被消费孤儿搜索 result」轮（折叠后 `parts` 为 0 部件、`record = null`）→ 剔除（既有语义逐字保留：`parts.length > 0` 不是唯一判据，但 0 部件且无轮行仍出局） | 单次 |
| `filterConversation` | 边界 | `all = undefined`（越签名脏值）→ 抛错或返回空数组二者择一，但 MUST NOT 静默返回非空（不虚构消息；实际行为以实现为准并在用例内钉死一种，避免行为漂移） | 单次 |
| `userTurnSpans` | 正向 | 多轮 fixture（4 条 user 与交替 assistant，`measurements` 按估算值给出）→ 输出 4 条，逐条 `index` / `start` / `span` 等于手工推导值；前三条 `span` = 下一 user 的 `start` 差，第四条 `span = totalSize − start`（AC-4 的首尾闭合） | 单次 |
| `userTurnSpans` | 正向 | 首条 user 在索引 0、`measurements[0].start = paddingStart` → 输出首条 `start` 等于该值（与虚拟项 `translateY` 同基准，不变量 1） | 单次 |
| `userTurnSpans` | 正向 | 全部消息皆为 `user`（无 assistant）→ 每条 `span` = 下一条 `start` 差（末条 = `totalSize − start`），不跳过任何索引 | 单次 |
| `userTurnSpans` | 异常 | 无 `role === 'user'` 消息（全 assistant / 全 system）→ `[]`（不渲吸附层；AC-4 的「无 user 消息」分支） | 参数化迭代（2 形态） |
| `userTurnSpans` | 异常 | `measurements[index]` 缺席（`measurements` 长度小于 `messages` 长度）→ 该条**跳过**、末轮退化到 `totalSize`、其余条目照常产出、不抛错（越界防护） | 单次 |
| `userTurnSpans` | 异常 | `totalSize < start`（末轮倒挂的脏几何）→ 该条 `span` 钳制为 `0`（MUST NOT 产出负高度），不抛错 | 单次 |
| `userTurnSpans` | 边界 | `messages = []` → `[]`；`measurements = []` 且 `messages` 含 user → 全部跳过（`[]`）；两形态均不抛错 | 参数化迭代（2 形态） |
| `userTurnSpans` | 边界 | `totalSize = 0` 且单条 user 的 `start = 0` → `span = 0`（钳制后仍产一条，`index` / `start` 正确）；`span` MUST NOT 为负 | 单次 |
| `userTurnSpans` | 边界 | `start` 极值三形态 `0` / `Number.MAX_SAFE_INTEGER` / `-1` → 差值计算不抛错、`span` 仍非负（`Math.max(0, …)` 只作用于 `span`，不改写 `start`） | 参数化迭代（3 极值） |
| `userTurnSpans` | 边界 | `start` 单调不变量：任意 fixture 下输出的 `index` 严格递增、`start` 单调不减（吸附层与虚拟项同基准的机械判据） | 单次 |
| `userTurnSpans` | 边界 | 超大 fixture（1000 条中 100 条 user 与 900 条 assistant，`measurements` 稠密）→ 输出 100 条、末条 `span` 闭合到 `totalSize`、不抛错 | 单次 |
| `conversationIdentity` | 正向 | 多轮折叠列表、各条 `metadata.record.sessionId` 同值 `S` → 返回 `S`（同会话的稳定身份，复位键不随条数变化） | 单次 |
| `conversationIdentity` | 正向 | 首条无 `record`（流式首轮未收口）、第 2 条起有 `record` → 返回**首条携 `record` 的那条**的 `sessionId`（不是「取首条消息」的退化解） | 单次 |
| `conversationIdentity` | 正向 | 同会话追加（尾部追加一条携同一 `sessionId` 的消息）→ 返回值不变（AC-13「同会话追加不复位」的纯函数半边） | 单次 |
| `conversationIdentity` | 正向 | 流式增长（同一条消息 `parts` 由 1 个变 3 个、`record` 不变）→ 返回值不变 | 单次 |
| `conversationIdentity` | 异常 | 换会话：消息集合整体换成另一会话（`sessionId` 由 `A` 变 `B`，条数可相同）→ 返回值变化且等于 `B`（AC-13 的复位触发前提） | 单次 |
| `conversationIdentity` | 异常 | 全体消息 `metadata.record = null` / `metadata` 缺席 / `record` 为 `undefined` 三形态 → 均返回 `null`（键未知，不参与换会话比较） | 参数化迭代（3 形态） |
| `conversationIdentity` | 边界 | `messages = []` → `null`（首帧前的空集合不产生身份，不抛错） | 单次 |
| `conversationIdentity` | 边界 | `sessionId` 为空串（越签名脏值）→ 返回空串或 `null` 二者之一，用例内钉死一种且 MUST NOT 抛错（`null` 专表「未知」，空串是否算已知须固定） | 单次 |
| `conversationIdentity` | 边界 | 超大列表 1000 条 → 取首条命中即返回、不扫全表也不抛错 | 单次 |
| `conversationIdentity` | 边界 | `messages = undefined`（越签名脏值）→ 抛错或返回 `null` 二者之一，MUST NOT 静默返回非 `null`（不虚构会话身份） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：`./agent-adapter`（`foldSessionMessages` / `AgentUIMessage`） | 真实折叠产出 fixture——`metadata.record` / `parts` / `role` / `sessionId` 的形状是被测判据的输入本体，替身化会让「语义零漂移」自证（AC-8 / AC-13） | 过滤判据族、可见部件判据族、会话身份族 |
| 不 mock、不 import：本文件内的 `isToolPartType` | 同文件私有判据（`tool-` 前缀且非 `dynamic-tool`），与 `agent-blocks.tsx` 的 `isToolPart` **同口径但无 import 依赖**（`src/lib/**` 不得反向依赖 `components/`）；用例直接断言前缀伪装四形态 | `hasVisibleParts` 正向 / 异常行 |
| 不 mock：被测四函数自身 | 同模块内直调（不起测试专用包装器、不加测试专用导出——AGENT.md 规则 5） | 全章节 |
| 不 mock：时钟 / 随机 | 本文件零时间与随机依赖（`measurements` / `totalSize` / `sessionId` 全为字面量） | 全章节 |
| 只作类型：`TurnSummary` / `AgentUIMessage` | 仅 `import type`；需要越签名脏值时用字面量构造 + `as`（测试文件豁免 `no-unsafe-type-assertion`），不引入运行时替身 | 脏值边界行 |

### `packages/desktop/src/components/ui/message-scroller.tsx` -> `packages/desktop/src/components/ui/message-scroller.test.tsx`

**新建测试文件**（本仓 `src/components/ui/` 目录的测试文件）——沿 `src/components/agent/agent-messages.test.tsx` 的渲染面定式：`data-slot` / `data-*` 属性锚点定位、类名清单逐字断言、无 `data-testid` 依赖（shim 不新增 `data-testid`）。被测面是 **shim 的结构与复位契约**：Provider 的虚拟窗口入参通道、`resetKey` 复位时机、context 契约、五个既有 `data-slot` 钩子 + 一个新增吸附层钩子、Viewport 的类名与 HUD 属性、Content 的几何与索引交织、Item 的绝对定位与零测量回落、Sticky 的 `top` 定位与回填、Button 的距底判据与导出面收窄。全局前提是 `test-setup.ts` 的视口尺寸桩（`clientHeight = scrollHeight = 600`）已生效——`600 / 600` ⇒ `maxScroll = 0` ⇒ 全局语义为「内容不足一屏且贴底」；需要「可滚动」形态的用例在**实例级**抬高 `scrollHeight`。渲染 fixture 由测试文件内自建的「纯数据驱动」`renderItem` / `renderSticky` 提供（不引 `AgentUIMessage`，shim 与领域类型无关）。

#### 待测功能

- `MessageScrollerProvider(props: MessageScrollerProviderProps) => React.JSX.Element`: 持有 `useVirtualizer` 与 `viewportRef`（`getScrollElement` 恒读 `viewportRef`）；向子层供 context（`virtualizer` / `viewportRef` / `pendingScroll` / `scrollToEnd` / `renderItem` / `renderSticky`）；据 `resetKey` 判两条复位条件并在 layout 阶段调一次 `scrollToIndex(count − 1, { align: 'end' })`（条件①「首次非空 / 由空变非空」、②「两侧 `resetKey` 均非 `null` 且不等」；`prevKeyRef` 以 `undefined` 为初值区分「尚未有过键」与 `null`）；`resetKey` 不进 `useVirtualizer` 选项；产出 `pendingScroll` 与 `scrollToEnd`。
- `MessageScroller(props: React.ComponentProps<'div'>) => React.JSX.Element`: 纯 `div`；类名 `group/message-scroller relative flex size-full min-h-0 flex-col overflow-hidden` 与 `data-slot="message-scroller"` 逐字保留。
- `MessageScrollerViewport(props: React.ComponentProps<'div'>) => React.JSX.Element`: 纯 `div` + context 的 `viewportRef`；类名清单逐字保留（含 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overscroll-contain` / `contain-content`）；`role` 缺省 `region`（可被调用点覆盖）、`tabIndex={0}`；`data-autoscrolling` / `data-pending-scroll` **条件输出**（真时 `"true"`、假时不落属性）。
- `MessageScrollerContent(props: { children?: React.ReactNode; className?: string }) => React.JSX.Element`: `position: relative` + 内联 `height: getTotalSize()`；渲 `children` → 可见切片与 `renderSticky?.(geometry)` 的条目**按索引升序交织**（`renderItem(index) === null` 的索引不出节点）；类名**零** `flex` / `h-max` / `min-h-full` / `gap-6` / `p-3`。
- `MessageScrollerItem(props: MessageScrollerItemProps) => React.JSX.Element`: `data-slot="message-scroller-item"` + `data-index={index}`；`position:absolute; top:0; left:0; width:100%` + 内联 `transform: translateY(start)`；保留 `[content-visibility:auto]` / `[contain-intrinsic-size:auto_10rem]` / `min-w-0` 并带 `px-3`；`ref` 挂零测量回落的测量函数（实测 > 0 走原生 `measureElement`，≤ 0 走 `resizeItem(index, estimateSize(index))`）。
- `MessageScrollerSticky(props: MessageScrollerStickyProps) => React.JSX.Element`: 外层 `data-slot="message-scroller-sticky"` + `z-10` + `position:absolute` + **`top: start`** + `left:0` + `width:100%` + `height: span` + `pointer-events:none`（**禁 `transform`**）；内层 `position:sticky; top:0` + `bg-card px-3 pointer-events-auto`；`ResizeObserver` 观察内层 `borderBoxSize` → `resizeItem(index, h > 0 ? h : estimateSize(index))`。
- `MessageScrollerButton(props: MessageScrollerButtonProps & Pick<…, 'variant' \| 'size' \| 'render'>) => React.JSX.Element`: `data-active = getDistanceFromEnd() > scrollEndThreshold`（缺省 24）；`onClick = scrollToEnd()`（`count === 0` 时短路）；`data-slot="message-scroller-button"` / `data-direction="end"` 恒在；`type="button"`、未激活时 `tabIndex={-1}`、激活时 `tabIndex={0}`；`direction` 入参退役。
- `useMessageScroller` / `useMessageScrollerScrollable` / `useMessageScrollerVisibility`（**退役**）: 模块运行时导出面 MUST NOT 再出现三项；跳转能力由 `scrollToIndex` 承担。

<!-- design.md「类型定义」表另声明 MessageScrollerProviderProps / MessageScrollerGeometry / MessageScrollerStickyEntry / MessageScrollerItemProps / MessageScrollerStickyProps 五个 interface 与 MessageScrollerButtonProps 的修改。它们不是「公共函数 / API」行；形状覆盖并入对应组件的入参断言（缺省值 / 必填 / 可选缺席三面），类型错由 vp check 的 tsc 强制（见「不可测试项」）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `MessageScrollerProvider` | 正向 | `count = 100`（另一次 `count = 1000`）→ DOM 中 `[data-slot="message-scroller-item"]` 项数 ≪ count 且 > 0（只渲窗口切片 + overscan），项数随 `count` 增大不线性增长（AC-1 ④） | 参数化迭代（2 形态） |
| `MessageScrollerProvider` | 正向 | `count = 0` → `MessageScrollerContent` 内零 item、`renderSticky` 收到 `totalSize = 0`、`children` 照常渲出（空态块可挂），不抛错 | 单次 |
| `MessageScrollerProvider` | 正向 | `renderItem` 对部分索引返回 `null`（模拟 user 索引）→ 这些索引在 DOM **零节点**（无空壳 `message-scroller-item`），其余索引照常成节点；`data-index` 序列不含被 `null` 的索引（AC-3 的 shim 半边） | 单次 |
| `MessageScrollerProvider` | 正向 | `renderSticky(geometry)` → 被调用且 `geometry.measurements` 长度等于 `count`（稠密可读）、`geometry.totalSize` 等于 `MessageScrollerContent` 的内联 `height`（同一基准的账目判据）；`renderSticky` 缺席时不抛错 | 参数化迭代（2 形态：给 / 不给） |
| `MessageScrollerProvider` | 正向 | context 契约：`MessageScroller` / `Viewport` / `Content` / `Item` / `Sticky` / `Button` 在 Provider 内全部可渲染并读到实例（软证：Button 的 `data-active` 与 Content 的高度均由实例派生） | 单次 |
| `MessageScrollerProvider` | 异常 | 子件在 Provider **外**使用（`MessageScrollerContent` / `MessageScrollerButton` 无 context）→ 抛明确错误，不静默渲出零高容器 | 参数化迭代（2 子件） |
| `MessageScrollerProvider` | 边界 | 缺省可选入参：不传 `overscan` / `gap` / `paddingStart` / `paddingEnd` / `scrollEndThreshold` / `resetKey` → shim 以缺省值（8 / 0 / 0 / 0 / 24 / `undefined`）装配、不抛错 | 单次 |
| `MessageScrollerProvider` | 边界 | 显式 `gap = 24` / `paddingStart = 12` / `paddingEnd = 12` → `totalSize` 比同 fixture 缺省形态大 `24 × (count − 1) + 24`（纵向几何单一来源的运行期侧证，R-13） | 单次 |
| `MessageScrollerProvider` | 正向 | **⑥′ 首帧贴底（初始挂载）**：`count = 3`、实例级抬高 `scrollHeight`（= 内容高度 + 视口高）并装实例级 `scrollTo` 桩 ⇒ 挂载后 `scrollTop` 等于**末端目标值**（用例内按 `clamp(scrollHeight − clientHeight − paddingEnd, 0, scrollHeight − clientHeight)` 推算；`align:'end'` 的末项分支会扣 `paddingEnd`，`scrollPaddingEnd` 缺省 0），零抛错 | 单次 |
| `MessageScrollerProvider` | 正向 | **⑥′ 由空变非空**：`count` 0 → 3 的 `rerender`（同一 Provider 实例）⇒ `scrollTop` 落到上述同一个末端目标值（条件①，MUST NOT 依赖 `anchorTo: 'end'` 的选项时机） | 单次 |
| `MessageScrollerProvider` | 正向 | **⑥″ 换会话复位**：`resetKey` 由 `A` 变 `B`（两侧非 `null`、`count` 不变），先人为把 `scrollTop` 移到 `0` ⇒ 复位回末端目标值（AC-13 的换会话面） | 单次 |
| `MessageScrollerProvider` | 异常 | **⑥″ 反向：同会话追加不复位**：`resetKey` 恒为 `A`、`count` 3 → 5，且 `scrollTop` 已离开末端 ⇒ `scrollTop` MUST NOT 落到末端目标值（虚拟器的端锚定只会为保持原可见项做补偿，不会把视口拽到底；断言取「≠ 末端目标值」，上滑用户不被拽走） | 单次 |
| `MessageScrollerProvider` | 异常 | **⑥″ 反向：键单侧为 `null`** 两形态——`null → A`（首轮 `record` 落库，`count` 不变）与 `A → null`（收口重放，`count` 不变）⇒ 两种变化下 `scrollTop` 均不变（条件②要求两侧均非 `null`；`count` 不变故连端锚定补偿都不发生，可断严格相等） | 参数化迭代（2 形态） |
| `MessageScrollerProvider` | 边界 | `resetKey` 缺省（`undefined`）⇒ 首帧仍贴底（条件①负责）；随后 `count` 增长且 `scrollTop` 已离开末端 ⇒ MUST NOT 由复位机制拽到末端（`undefined` 不参与条件②——`undefined` = 尚未有过键、与 `null` = 键未知的语义分离在此可判） | 单次 |
| `MessageScrollerProvider` | 边界 | 始终空：`count` 0 → 0（`rerender`）⇒ `scrollTo` 桩零调用、不抛错（`count − 1` 的下标不得为负） | 单次 |
| `MessageScrollerProvider` | 边界 | 三次渲染序列 `resetKey` = `A` → `A` → `B`（`count` 不变，每次渲染前把 `scrollTop` 移离末端）⇒ 第二次 `scrollTop` 保持人为值（键不变，零复位）、第三次落到末端目标值（`prevKeyRef` 逐轮推进：无重复复位、无漏复位） | 单次 |
| `MessageScrollerProvider` | 异常 | `resetKey` **不进虚拟器选项**：`count` / `estimateSize` / `getItemKey` 全不变（键集合固定，端锚定不参与），仅 `resetKey` 由 `A` 变 `B` ⇒ 各项 `start`（`translateY`）序列逐项不变、`measurements` 逐项不变（`resetKey` 未触发 `setOptions` 的端锚定重算——D10 的机械判据；断几何不变而非写入次数，避开 reconcile 的追加写入） | 单次 |
| `MessageScroller` | 正向 | 渲出 `div[data-slot="message-scroller"]`，类名逐字含 `group/message-scroller` / `relative` / `flex` / `size-full` / `min-h-0` / `flex-col` / `overflow-hidden`；`className` 入参叠加在后 | 单次 |
| `MessageScroller` | 异常 | 结构前提 ③ 的**落点判据**：`overflow-hidden` 留在 `-scroller` 上（承担 `rounded-lg` 裁剪；它在 `-viewport` 之上、不参与 sticky 夹取基准——事实偏差 6 的口径）；反向判据是该类名 MUST NOT 出现在 `-content` / 吸附层 | 单次 |
| `MessageScroller` | 边界 | 不入 Provider 直接渲染（纯 `div`、零 context 依赖）→ 正常渲出、不抛 context 缺失错误（与 Content / Button 的异常行构成对照） | 单次 |
| `MessageScroller` | 边界 | `className` 缺席 / 空串 / 超长类名串 → 缺省类名清单不受损、不抛错（类名合成为 `cn` 的既有定式） | 参数化迭代（3 形态） |
| `MessageScrollerViewport` | 正向 | 类名逐字含 `scroll-fade-b` / `scrollbar-thin` / `scrollbar-gutter-stable` / `overflow-y-auto` / `overscroll-contain` / `contain-content` / `data-autoscrolling:scrollbar-thumb-transparent` / `data-pending-scroll:invisible`；`data-slot="message-scroller-viewport"` 在场（AC-7 的机械判据） | 单次 |
| `MessageScrollerViewport` | 正向 | 调用点透传 `aria-label="对话消息"` → 落在视口元素上；`ref` 由 shim 独占（视口尺寸桩命中即证 ref 已挂上、虚拟窗口 range 非空） | 单次 |
| `MessageScrollerViewport` | 正向 | 可达性对齐：`role` 缺省为 `"region"`、`tabIndex = 0`；调用点显式传 `role` ⇒ 覆盖生效（不吞调用点入参） | 参数化迭代（2 形态） |
| `MessageScrollerViewport` | 正向 | `data-pending-scroll` 生命周期：挂载后首次 `useLayoutEffect` 前（flush 前）属性为 `"true"`，effect 完成后属性**不落**（`undefined`，不是 `"false"`——事实偏差 4 的存在性变体语义） | 单次 |
| `MessageScrollerViewport` | 正向 | `data-autoscrolling`：`fireEvent.scroll(viewport)` 驱动虚拟器 `isScrolling` → 属性变 `"true"`；滚动收尾后再不落属性（条件输出、不恒亮） | 单次 |
| `MessageScrollerViewport` | 异常 | 反向判据：`data-pending-scroll` / `data-autoscrolling` **不出现**取值 `"false"`（存在性变体会让 `scrollbar-thumb-transparent` 常亮 / 首帧恒隐形） | 单次 |
| `MessageScrollerViewport` | 边界 | `count = 0`（内容不足一屏）→ 挂载后 `data-pending-scroll` 不落、`data-autoscrolling` 不落（无滚动可发生） | 单次 |
| `MessageScrollerContent` | 正向 | 内联 `height` 等于 `getTotalSize()`（`count = 3` × `estimateSize = 100` + 缺省 `gap = 0` + `padding = 0/0` ⇒ `300px`）；类名为 `relative` 且**不含** `flex` / `h-max` / `min-h-full` / `gap-6` / `p-3`（`min-h-full` 与 CSS padding 会让末项越出容器、账目对不上——事实偏差 2 / R-13 的反向判据） | 单次 |
| `MessageScrollerContent` | 正向 | 渲染次序：`children` 先出现、虚拟切片居中、索引大于全部可见项的吸附条目落在末位（DOM 子节点序断言） | 单次 |
| `MessageScrollerContent` | 正向 | 吸附条目按索引与可见切片**交织**：`renderSticky` 返回 index=1 的条目时，DOM 顺序为 `item:0 → sticky → item:1 → item:2`（DOM 顺序即会话顺序，落定 8 的机械判据） | 单次 |
| `MessageScrollerContent` | 异常 | `renderItem` 全返回 `null`（全 user 索引的极端形态）→ Content 内零 item 节点、`height` 仍为全量 `getTotalSize()`（账目照占位、渲染集合为空——不变量 5） | 单次 |
| `MessageScrollerContent` | 边界 | `count = 0` + 传 `children`（空态块 `conversation-empty` 的宿主面）→ `height = 0`、children 仍在 DOM、不抛错 | 单次 |
| `MessageScrollerItem` | 正向 | 内联样式含 `position: absolute` / `top: 0` / `left: 0` / `width: 100%` 与 `transform: translateY(<start>px)`；`data-slot="message-scroller-item"` + `data-index={index}` 在场（`measureElement` 依赖 `data-index`） | 单次 |
| `MessageScrollerItem` | 正向 | 保留类名逐字含 `[content-visibility:auto]` / `[contain-intrinsic-size:auto_10rem]` / `min-w-0` 与新增 `px-3`（横向 gutter 落行级——事实偏差 3） | 单次 |
| `MessageScrollerItem` | 正向 | 正常测量路径：该用例内装**作用面收窄的**项级尺寸桩——`getBoundingClientRect` 命中 `data-slot="message-scroller-item"` 时返 `height = 88`（过零测量护栏）、`offsetHeight` 命中同条件时返 `88`（虚拟器 `measureElement` 的原生读取面），其余回落原生描述符，`afterEach` 复原 ⇒ 该项按实测 88 计入账目，后续项的 `start` 与 `totalSize` 随之改变（AC-6 的虚拟项实测面；与 `test-setup.ts` 的 viewport 尺寸桩同一收窄手法） | 单次 |
| `MessageScrollerItem` | 异常 | 零测量回落：`getBoundingClientRect().height = 0`（jsdom 原生值，未装上一行的项级桩）→ 该 item 的高度回落为 `estimateSize(index)`，后续 item 的 `transform` 值仍连续递增、MUST NOT 出现零高折叠（AC-6 ⑥ / 不变量 4） | 单次 |
| `MessageScrollerItem` | 边界 | `start` 三形态 `0` / `12`（paddingStart）/ 极大值 → `translateY` 逐字输出、不四舍五入不夹取 | 参数化迭代（3 形态） |
| `MessageScrollerSticky` | 正向 | 外层 `div[data-slot="message-scroller-sticky"]` 的内联样式含 `position: absolute` / `top: <start>px` / `left: 0` / `width: 100%` / `height: <span>px` / `pointer-events: none`；内层 div 含 `position: sticky` / `top: 0`（AC-3 ⑤ 的结构面） | 单次 |
| `MessageScrollerSticky` | 正向 | 内层类名含 `bg-card` / `px-3` / `pointer-events-auto`（底衬 + 可交互，D8 / D9）；外层类名含 `z-10`（同层遮挡，落定 9）且**无**渐变 / 阴影 / 「上方还有内容」提示类名（零装饰的反向判据） | 单次 |
| `MessageScrollerSticky` | 异常 | **结构前提 ①**：外层 `style.transform` 缺席、该元素 `style` 属性串小写后不含 `transform`（transform 会创建新包含块 ⇒ 气泡吸住后永不顶走，D7）；内层亦不含 | 单次 |
| `MessageScrollerSticky` | 异常 | **结构前提 ②**：吸附层自身与 `MessageScrollerContent` 的类名**均不含** `overflow-hidden` / `overflow-auto` / `overflow-y-auto`（另立滚动容器 ⇒ 完全不吸附）；`MessageScrollerViewport` 是链上唯一携带 `overflow-y-auto` 的元素 | 单次 |
| `MessageScrollerSticky` | 异常 | `span < 内容高度`（吸附被顶走的触发条件）→ 外层 `height` 逐字等于 `span`（零缓冲：不做 gutter 扣减），不抛错 | 单次 |
| `MessageScrollerSticky` | 边界 | ResizeObserver 回报 `borderBoxSize[0].blockSize = 0`（jsdom 零布局的常态）→ 调 `resizeItem(index, estimateSize(index))`：`totalSize` 不塌陷（等于纯估算账目）、不抛错（AC-6 的吸附层回落路径；以用例内可编程 `ResizeObserver` 替身手动触发回调） | 单次 |
| `MessageScrollerSticky` | 边界 | ResizeObserver 回报正常值（如 `blockSize = 88`）→ `resizeItem(index, 88)`：该项 `start` 后的项整体位移 88 而非 `estimateSize`，`totalSize` 相应变化（D5 的回填生效面） | 单次 |
| `MessageScrollerSticky` | 边界 | `span = 0`（末轮倒挂钳制后）→ 外层 `height: 0px`、内层照常挂载（`position: sticky` 不因零高容器报错）、不抛错 | 单次 |
| `MessageScrollerButton` | 正向 | 贴底形态（`scrollHeight = clientHeight = 600` ⇒ `getDistanceFromEnd() = 0 < 24`）→ `data-active="false"`；`data-slot="message-scroller-button"` / `data-direction="end"` / `data-variant` / `data-size` 恒在；类名清单逐字含 `data-[active=false]:*` / `data-[direction=end]:bottom-4`（AC-7 / D13） | 单次 |
| `MessageScrollerButton` | 正向 | 非贴底形态（实例级抬高 `scrollHeight` 到远超一屏，`getDistanceFromEnd() > 24`）→ `data-active="true"`（可见性与贴底状态互斥） | 单次 |
| `MessageScrollerButton` | 正向 | 可达性对齐：`type="button"` 恒在；未激活 `tabIndex = -1`、激活 `tabIndex = 0`（可聚焦性与可见性同源） | 参数化迭代（2 形态） |
| `MessageScrollerButton` | 正向 | `onClick` → 调 context 的 `scrollToEnd()`：`count > 0` 时经实例级 `scrollTo` 桩观察到写入、且**最后一次写入**的 offset 等于末端目标值（与 ⑥′ 同一口径——按钮与复位共用同一路径；虚拟器的 reconcile 可能追加同值写入，故只断末值与「至少一次」），不抛错 | 单次 |
| `MessageScrollerButton` | 异常 | `count = 0` 时点击 → 短路无操作、零 `scrollTo` 调用、不抛错 | 单次 |
| `MessageScrollerButton` | 异常 | 反向判据：`direction` 入参退役——传入 `direction="start"` 不改变 `data-direction`（恒 `"end"`），也不渲染「Scroll to start」的 `sr-only` 文案（D13） | 单次 |
| `MessageScrollerButton` | 边界 | `data-active` 取值为 `"true"` / `"false"` 两显式字符串（与 Viewport 的存在性变体不同：按钮走 `data-[active=…]` 显式值形态，可恒输出——事实偏差 4 的对照组） | 参数化迭代（2 形态） |
| `useMessageScroller` / `…Scrollable` / `…Visibility`（退役） | 正向 | 保留面可用：七个组件全部可从模块导入并各自渲染成功（`MessageScrollerSticky` 为新增项的在场判据；三 hook 的删除不得带走任一保留件——D2 的「保留 UI + 接 headless」正向面） | 参数化迭代（7 导出） |
| `useMessageScroller` / `…Scrollable` / `…Visibility`（退役） | 异常 | 运行时导出键集合**恰为**七个组件（`MessageScrollerProvider` / `MessageScroller` / `MessageScrollerViewport` / `MessageScrollerContent` / `MessageScrollerItem` / `MessageScrollerSticky` / `MessageScrollerButton`），三个 hook 与 `scrollToMessage` **缺席**（`Object.keys(module).sort()` 断言；D15 无兼容层） | 单次 |
| `useMessageScroller` / `…Scrollable` / `…Visibility`（退役） | 边界 | 导出键集合的稳定性：重复取值后集合逐项相等（无惰性副作用导出），且集合内每项 `typeof === 'function'`（无 `@deprecated` 转发对象、无默认导出垫片——D15「无兼容层」的判据） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ResizeObserver`（运行环境能力） | 用例内 `vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver)`：捕获回调与观测目标，由用例手动投 `{ borderBoxSize: [{ blockSize }] }`；`afterEach` 复原（`test-setup.ts` 的零回调桩作为缺省值保留，不改动它） | Sticky 高度回填两行、`agent-messages` 的吸附层回填行 |
| 视口 `clientHeight` / `scrollHeight`（布局能力） | **不打桩**——直接消费 `test-setup.ts` 的原型 getter（`600 / 600`）；需要「可滚动」形态的用例在**实例级** `Object.defineProperty(viewport, 'scrollHeight', …)` 局部抬高（实例级优先于原型，互不干扰；仓库既有先例 `architecture-view.test.tsx` / `stdout-panel.test.tsx`） | Button 的 `data-active` 两形态、Provider 复位 / 不复位各行的视口形态 |
| `Element.scrollTo`（jsdom 缺失的写入侧） | **实例级最小桩**：由用例自带的子件在自身 `useLayoutEffect` 内按 `data-slot="message-scroller-viewport"` 取视口并 `Object.defineProperty` 装 `scrollTo`（按参数写 `scrollTop` / `scrollLeft`，**不派发 `scroll` 事件**）与 `scrollTop` 读写访问器（jsdom 原生 `scrollTop` 不写回）；子 effect 先于 Provider 的复位 effect 生效，桩随渲染实例消亡。**MUST NOT** 装进 `test-setup.ts` 的 `Element.prototype`（会把其余套件的「写入静默无效」改成「真写位置」，改变既有语义） | Provider ⑥′⑥″ 各行、Button 点击行 |
| 不 mock：`@tanstack/react-virtual` | **真实实现**——`getTotalSize()` / `getVirtualItems()` / `measurementsCache` / `getDistanceFromEnd()` / `isScrolling` / `scrollToIndex` 是被测输出本体；替身化会让 AC-1 / AC-6 / AC-13 的判据自证 | 全章节 |
| 不 mock：`cn` / `@/components/ui/button` / `lucide-react` | 真实组合——类名合成结果与按钮形态是既有锚点断言的一部分 | 类名清单行、Button 行 |
| 不 mock：`react` / `react-dom` 的渲染与 effect 时序 | 真实 `render` / `act`——`data-pending-scroll` 的生命周期与复位 effect 的「子先于父」时序只有真实时序能侧证 | Viewport HUD 行、Provider 复位行 |
| 不 mock：被测组件自身与 `useScrollerContext` | 同模块内真实调用；context 缺席的异常路径经「把子件渲在 Provider 外」触发（不为测试开导出品——`knip` 与 AGENT.md 规则 5） | context 异常行 |
| 项高度（布局能力） | **只在需要非零实测的用例内**装作用面收窄的项级桩：`Element.prototype.getBoundingClientRect`（护栏读取面）与 `Element.prototype.offsetHeight` 的 getter（虚拟器 `measureElement` 的原生读取面）各一，均仅对 `data-slot="message-scroller-item"` 命中返 `88`、其余回落原生描述符，`afterEach` 复原。`test-setup.ts` **不动**——不打桩即 jsdom 原生 0 ⇒ 走零测量回落路径（两条路径各有用例，互不覆盖） | Item 正常测量路径行、Item 零测量回落行 |

### `packages/desktop/src/components/agent/agent-messages.tsx` -> `packages/desktop/src/components/agent/agent-messages.test.tsx`

既有测试文件 **删分页用例 + 改写条数断言 + 追加虚拟化 / 吸附 / 复位用例**。删：「历史分页」5 例、「大列表分页」1 例、`FakeIntersectionObserver`（class 与其各段 `beforeEach` 的 `vi.stubGlobal` / `afterEach` 的 `vi.unstubAllGlobals`）与 `triggerReachTop`（proposal 处理列明示；该装置在多段 `describe` 被复用，删除后这些段的 `beforeEach` / `afterEach` 一并清空）。改写：`chat-bubble` 断言中凡断**条数**者改为按 `data-role` 分查（`getAllByTestId('chat-bubble')` → 过滤 `data-role="assistant"` / `"user"` 的子集计数；user 恒全量为吸附层常驻量）。保留：气泡变体 / 思考折叠 / 工具卡 / AskUserQuestion / 空内容轮状态行 / citations / 三态 / 一轮一条气泡 / 搜索两段合轮等**语义断言逐字保留**（R-3 的反向约束）。夹具沿用既有 `fold(events, turns)` 薄包装（真实适配层折叠）与 `turnConversation` / `emptyTurnMessage` 构造器（去掉分页语义后仍复用为「N 条多轮会话」与「零内容轮形态」构造器）。

#### 待测功能

- `AgentMessages(props: AgentMessagesProps) => React.JSX.Element`（体内变化、入参不变）: `AgentMessagesProps`（`messages` / `loading?` / `running?`）零改动；过滤走 `filterConversation`、虚拟窗口接线（`count` / `estimateSize` 按角色分型 / `getItemKey` = 消息 `id` / `renderItem` 对 user 返回 `null`）、吸附层接线（`renderSticky` → `userTurnSpans` 全部条目 → `MessageScrollerSticky` + `ChatBubble`）与复位键派生（`resetKey` = `conversationIdentity(过滤后列表)`，`useMemo`，经私有 `MessageContainer` 透传给 shim）在此收口；`loading` / `conversation-empty` / `running` 三态与全部块渲染件逐字不变。

<!-- design.md「公共函数 / API」表对本文件只声明上面一行（体内变化、签名不变）；其「修改文件」表列出的退役面（useHistoryWindow / useTopReachObserver / HistoryLoadStatus / MESSAGE_PAGE_SIZE / HISTORY_PRELOAD_MARGIN_PX / topSentinelRef / remaining / history-load-*）与私有件（renderItem / renderSticky 闭包、ToolPartView / PartView / TurnStatusLine / ChatBubbleBase、MessageContainer）不另立待测功能条目：退役面的运行期判据是「history-load-* 节点与触顶观察器零命中」，私有件的覆盖经本行的呈现断言与复位断言承接（design 明令「按纪律不列本表」）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentMessages` | 正向 | 1000 条过滤后消息（含 50 条空内容 `completed` 轮）→ DOM 中 `data-role="assistant"` 的气泡数 **≪ 1000**（`> 0` 且远小于总数，窗口切片 + overscan）、`data-role="user"` 的气泡数 = 该会话的 user 消息条数（全量常驻）；零 `history-load-*` 节点、零 `IntersectionObserver` 装配（AC-1 ⑦ / AC-2 的运行期侧证） | 单次 |
| `AgentMessages` | 正向 | 同一 1000 条 fixture → `[data-slot="message-scroller-content"]` 的内联 `height` 等于全量账目推算值（`12 + 72 × n_user + 240 × n_assistant + 24 × (n − 1) + 12`，jsdom 零测量 ⇒ 估值即几何，AC-6）；末项 `translateY(start) + estimateSize(last) + 12 === height`（末项底边与容器底边对齐——不变量 3） | 单次 |
| `AgentMessages` | 正向 | 12 条 user + 若干 assistant → `[data-slot="message-scroller-sticky"]` 节点数恰 12（各一条 user 气泡，各恰一处）；吸附层内 `data-role="user"` 气泡数 = 12；**虚拟层**（`[data-slot="message-scroller-item"]` 内）`data-role="user"` 节点数 = 0（AC-3 ⑧⑩） | 单次 |
| `AgentMessages` | 正向 | 吸附层几何与虚拟项同基准：对每个 sticky 节点取其 `style.top`，断言其值等于同一 `index` 的 user 消息在估值起点序列中的 `start`；sticky 外层 `style.transform` 缺席（AC-4 / AC-5 的呈现侧证） | 单次 |
| `AgentMessages` | 正向 | 吸附层 `span` 闭合：第 k 条 sticky 的 `style.height` = 第 k+1 条 sticky 的 `style.top` − 第 k 条 `style.top`；末条 `style.height` = 内容高度 − 末条 `style.top`（首尾闭合，AC-4） | 单次 |
| `AgentMessages` | 正向 | 气泡同渲染件：吸附层的 user 气泡与（滚动到可见时的）普通 user 气泡同为 `Bubble variant="tinted"` 结构、同一 `data-testid="chat-bubble"` + `data-role="user"`；吸附层多一层 `bg-card` 底衬容器、零渐变 / 零阴影类名（AC-7 / D9） | 单次 |
| `AgentMessages` | 正向 | 既有「容器组合」用例改写为虚拟化形态：`agent-messages` section 类名逐字含 `flex` / `min-h-0` / `flex-1` / `flex-col`；`[data-slot="message-scroller"]` / `-viewport`（`aria-label="对话消息"`）/ `-button` 在场；**既有断言「`message-scroller-item` 恰 2 个」改为按窗口切片断言（`> 0` 且不含 user 索引）**（AC-7） | 单次 |
| `AgentMessages` | 正向 | 空内容轮可见性（既有语义改写）：1 条空 parts + `completed` 轮 + 2 条正常消息 → `data-role="assistant"` 气泡恰 2、`data-role="user"` 恰 1、无空气泡；文本「第一问」「第一答」在场（逐字保留，AC-8） | 单次 |
| `AgentMessages` | 正向 | 空内容 `failed` / `stopped` 轮（既有语义改写）：`turn-status-line` 在场、`data-status` 分别为 `failed` / `stopped`、可展开原文 `turn-status-line-raw` 在场；该轮计入虚拟列表项数（`count` 与吸附层几何含它）（AC-8） | 参数化迭代（2 状态） |
| `AgentMessages` | 正向 | 既有三态回归：`messages = []` → `conversation-empty` 在场且零气泡；全 `system` 载体（过滤后为空）→ `conversation-empty`；`loading = true` 且过滤后为空 → `agent-messages-loading`；`loading = true` 但已有可见气泡 → 不呈还原中态、气泡照常；`running = true` → `agent-messages-running` + 文案「运行中…」；`running = false` → 无该节点（判据与 `data-testid` 逐字保留） | 组合迭代（4 形态 × 2 标志） |
| `AgentMessages` | 正向 | 既有部件族回归（逐字保留）：四变体气泡 / 思考折叠 / 工具对卡特化卡 / AskUserQuestion 静态卡 / 一轮一条 assistant 气泡 / 搜索两段合轮 / citations 列表 —— 各用例在新形状下全绿（仅条数断言改分查） | 参数化迭代（既有用例组） |
| `AgentMessages` | 正向 | **⑪ 换会话复位**：`resetKey` 由会话 A 的 `sessionId` 变 B（`metadata.record.sessionId` 改值、条数不变，实例级抬高 `scrollHeight` + 装实例级 `scrollTo` 桩），`rerender` 前先把 `scrollTop` 移离末端 ⇒ `scrollTop` 回到末端目标值（证明复位键经透镜重渲染通道成立，AC-13） | 单次 |
| `AgentMessages` | 异常 | **⑪ 反向：同会话追加不复位**：`sessionId` 不变、消息尾部追加/流式增长（`count` 变），`scrollTop` 已离开末端 ⇒ `scrollTop` MUST NOT 落到末端目标值（端锚定可作保持补偿，但不得拽回底部；上滑用户不被拽走） | 单次 |
| `AgentMessages` | 异常 | **⑪ 反向：复位键单侧为 `null`**：消息集合首轮无 `record`（`conversationIdentity` 返回 `null`）→ 收口后 `metadata.record` 落库（键 `null → A`、条数不变）⇒ 不复位（AC-13 的「首轮 record 落库不拽回」面） | 单次 |
| `AgentMessages` | 异常 | 无 user 消息（全 assistant）→ `[data-slot="message-scroller-sticky"]` 节点数 = 0（不渲吸附层；AC-4 的「无 user ⇒ 不吸附、不炸」） | 单次 |
| `AgentMessages` | 异常 | 过滤后为空（`messages = []` 或全 system）→ 零 sticky、零 item、Content `height = 24`（`paddingStart + paddingEnd`）、空态块仍在（吸附层不因零 user 而虚构节点） | 参数化迭代（2 形态） |
| `AgentMessages` | 异常 | 反向判据：`AgentMessages` 子树内零 `history-load-hint` / `history-load-complete`、零 `h-2 w-full` sentinel 产物、零 `IntersectionObserver` 装配；`[data-slot="message-scroller-content"]` 的 `className` 不含 `p-3`（纵向几何改走虚拟器，R-13） | 单次 |
| `AgentMessages` | 异常 | 吸附层高度回填：以用例内可编程 `ResizeObserver` 替身向某条 user sticky 先投 `blockSize = 0`（回落 `estimateSize`，账目不变）再投 `88`（账目与 sticky `span` 随之变化），两臂均不抛错、`totalSize` 不塌陷（AC-6 的吸附层实例） | 参数化迭代（2 投值） |
| `AgentMessages` | 边界 | 单条 user → 恰 1 条 sticky，`style.top = 12`（`paddingStart`）、`style.height = height − 12`；无 assistant 气泡（`data-role="assistant"` 计数为 0） | 单次 |
| `AgentMessages` | 边界 | 单条 assistant（无 user）→ 零 sticky、恰 1 条 assistant 气泡（若在窗口内）；两形态均不抛错 | 参数化迭代（2 形态） |
| `AgentMessages` | 边界 | 空内容轮批（50 条 `completed`）混入 1000 条正常对话 → 项数 = 1000（空内容轮不计入）、气泡分查与账目高度均按 1000 条推算（AC-8 的「不计入 `count`」） | 单次 |
| `AgentMessages` | 边界 | `estimateSize` 角色分型：全 user fixture 与全 assistant fixture（同条数）→ 两者 Content `height` 之差 = `(240 − 72) × n`（分型常量的机械侧证，D14） | 单次 |
| `AgentMessages` | 边界 | `resetKey` 派生不改变入参契约：`AgentMessages` 只接受 `messages` / `loading` / `running` 三字段即可驱动上述全部复位行为（用例全部以这三字段渲染，不额外传 `sessionId`——`AgentMessagesProps` 形状不变的运行期侧证，AC-13） | 单次 |
| `AgentMessages`（组合链路：虚拟窗口重渲 × 渲染 memo；**执行文件 `agent-render-memo.test.tsx`**） | 正向 | 历史 assistant 项在虚拟窗口内渲染后，`rerender` 只改末条流式文本 → `markdownSpy` 对历史文本零调用、对末条新文本恰 1 次；同时历史 assistant 项仍在窗口内（虚拟化未把它们从 DOM 踢出）——「虚拟窗口重渲不重建历史 assistant 项」（proposal 处理列「补一条」；`memo(ChatBubble)` 语义的机械判据） | 单次 |
| `AgentMessages`（组合链路：只读消费点 × 虚拟化；**执行文件 `src/views/**` 与 `agent-session-view.test.tsx`**） | 正向（既有回归） | `agent-session-view` / `session-panel` / `index` / `agent-debug-view` / `explore-detail-pane` / `change-chat-panel` / `session-transcript-panel` / `detail-drawer` / `archi-chat-panel` 各套件在打桩后仍绿：`block-text` 文本序断言逐字不变（吸附层与切片按索引交织保住会话顺序）、`agent-debug-view.test.tsx` 三处 `chat-bubble` 断言逐一复核为「按 `data-role` 子集 / 序列」形态（条数断言只数 assistant 或只数无 user 的 fixture ⇒ 吸附层常驻不改变其语义）、空态断言 `chat-bubble` 数为 0（AC-9 / R-3） | 参数化迭代（9 套件） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ResizeObserver`（运行环境能力） | 用例内 `vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver)` 手动投 `borderBoxSize`，`afterEach` 复原（`test-setup.ts` 零回调桩为缺省值，不改动） | 吸附层回填两投值行 |
| `Element.scrollTo` / `scrollTop`（jsdom 缺失的写入侧与不可写属性） | **实例级最小桩**（与 shim 章节同法）：用例自带的子件在自身 `useLayoutEffect` 内按 `data-slot` 取视口，装 `scrollTo`（只写 `scrollTop` / `scrollLeft`，不派发 `scroll` 事件）与 `scrollTop` 读写访问器；同处实例级抬高 `scrollHeight` 使 `maxScroll > 0`。**MUST NOT** 进 `test-setup.ts` 原型 | ⑥″ / ⑪ 三行（换会话复位、同会话追加不复位、单侧 `null` 不复位） |
| 不 mock：`@/components/ui/message-scroller`（shim） | **真实组合**——虚拟窗口入参通道（`count` / `estimateSize` / `getItemKey` / `renderItem` / `renderSticky` / `resetKey`）与几何回填链（`MessageScrollerSticky` → `resizeItem`）是被测链路本体；替身化会让 AC-1 / AC-3 / AC-6 / AC-13 的呈现判据自证 | 全章节 |
| 不 mock：`@/lib/agent-virtual`（四纯函数） | 真实调用——过滤、吸附几何、会话身份是本文件的接线对象；其独立判据在 `agent-virtual.test.ts`，此处只消费（不重复断言内部分支） | 全章节 |
| 不 mock：`../lib/agent-adapter` 的 `foldSessionMessages` | 真实折叠产出 fixture（既有装置 `fold(events, turns)`）——`parts` / `metadata.record` / `sessionId` 形状是空内容轮、部件族与复位键判据的输入本体 | 全章节 |
| 不 mock：`./agent-blocks` / `./tool-cards` / `@/lib/parse` / `@/components/ui/{bubble,message,trace,spinner}` | 真实组合——块渲染件与 `memo` 语义是 AC-7 / AC-8 的判据面（`ChatBubble` 复用同一渲染件、气泡变体与 citations 逐字保留） | 部件族回归行、吸附气泡同渲染件行 |
| 不 mock：`IntersectionObserver` | **删除**既有 `FakeIntersectionObserver` 替身（触顶观察器退役，AC-2）；改为装一个只计装配次数的探针并以「零实例」为反向判据——不引入任何替代品 | 反向判据行 |
| 不 mock：`test-setup.ts` 的视口尺寸原型 getter | 真实生效（AC-9 前提）；用例只消费、不改写原型（需要「可滚动」形态时走实例级 `defineProperty`） | 全章节 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/src/test-setup.ts`（测试渲染基座） | design「修改文件」表明示「本文件是测试基座而非测试用例」：它只加两个原型级 getter（`clientHeight` / `scrollHeight`，按 `data-slot="message-scroller-viewport"` 收窄、值 600），自身无可断言的运行期契约；为它写断言会把测试基座绑死在桩的实现形态上（R-1 的缓解手段一变就红）。其「不桩 `scrollTo`」的边界同样只能靠用例侧证 | AC-9：运行期侧证是 `message-scroller.test.tsx` 的「视口桩生效 ⇒ range 非空（`count = 100` 有可见 item）」行与 `agent-messages.test.tsx` 的千条用例；「`Element.prototype.scrollTo` 未被改写」由「未装实例桩的套件里滚动写入仍静默无效（不抛错、位置不变）」间接侧证；全部受影响套件全绿即桩生效的判据 |
| `packages/desktop/package.json`（`version` `0.6.31 → 0.6.32`、`dependencies` 增 `@tanstack/react-virtual: 3.14.26` / 删 `@shadcn/react`） | 配置 / 元数据，无运行期行为可断言 | AC-10：`git diff packages/desktop/package.json` 审查（一次 patch + 精确锁版 + `devDependencies` 零变化 + `ignoreDependencies` 未新增）；`src-tauri/Cargo.toml` 的 crate 版本行未动、`src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"` 零手改；运行期侧证是 `@tanstack/react-virtual` 的 API 面被 shim 真实调用（`message-scroller.test.tsx` 全绿）与 `knip` 零 unused dependency |
| `packages/desktop/src/components/ui/questionnaire.tsx`（**删除**） | 「文件不存在」是无运行期观测面的事实；为删除面写测试无从驱动（无模块可导入） | AC-10：`git status` / `git diff --stat` 显示该文件删除；`grep -rn "questionnaire" packages/desktop/src` 零命中（`@shadcn/react` 全域零引用）；`knip` 零 unused file / unused dependency；同步验收 `knip.json` **零 diff**（未新增 `ignoreDependencies` / `ignore` 豁免） |
| AC-2 的**退役面零残留**（`useHistoryWindow` / `useTopReachObserver` / `HistoryLoadStatus` / `MESSAGE_PAGE_SIZE` / `HISTORY_PRELOAD_MARGIN_PX` / `topSentinelRef` / `history-load-hint` / `history-load-complete` / `remaining`；shim 侧 `useMessageScroller*` / `scrollToMessage` / `@shadcn/react` / `autoScroll` / `defaultScrollPosition` / `scrollPreviousItemPeek` / `preserveScrollOnPrepend` / `Item` 的 `messageId`·`scrollAnchor`；测试侧 `FakeIntersectionObserver` / `triggerReachTop`） | 「某标识符从不出现」是**源码位置事实**，运行期不可观测（断言只覆盖被驱动的输入路径，覆盖不到缺失事实）；写成测试会随路径布局漂移且与桌面端静态检查重复 | `grep` 判据由桌面端静态检查相位执行（`tasks.md` 阶段四 ①②、阶段七逐条给定命令）；运行期侧证：`message-scroller.test.tsx` 的导出键集合恰七项（三 hook 与 `scrollToMessage` 缺席）+ `agent-messages.test.tsx` 的 `history-load-*` 零命中、零 `IntersectionObserver` 装配、Content 无 `p-3` |
| AC-5 的「零自建 `scroll` 监听 / 零 `pinned` state / 零 range 依赖」 | 同上（缺席事实）；`addEventListener('scroll'` / `currentAnchor` / `pinnedIndex` / `rangeExtractor` / `onChange` 在 `components/agent/` 与 shim 内零命中属静态检索 | `grep -rn "addEventListener('scroll'" packages/desktop/src/components/agent` 零命中 + 上述四词零命中（桌面端执行）；运行期侧证是 `message-scroller.test.tsx` 的「吸附层 `position: absolute` + `top`、`transform` 缺席」两行与「吸附层 `height = span` 逐字」行（无状态可改写的结构面） |
| **sticky 的真实吸顶 / 顶走行为**（「完全出顶即吸顶」「下一轮顶走上一条」「滚回顶部无气泡吸顶」的视觉结果） | `position: sticky` 的吸附与顶走完全由**布局引擎**给出；jsdom 无布局（所有矩形为 0、无滚动几何）⇒ 单测**测不出来**（design R-2 明示） | 结构前提的机械判据落在 `message-scroller.test.tsx`（`top` 定位、`transform` 缺席、祖先链无 `overflow-*`、内层 `position: sticky; top: 0`、`height = span`、`z-10`）；几何闭合判据落在 `agent-virtual.test.ts`（`span` 差分与末轮闭合）与 `agent-messages.test.tsx`（sticky `top` / `height` 与账目一致）；AC-4 的视觉结果由**合并前手工滚动验证**（`tasks.md` 阶段六六项）兜底 |
| **吸附层 `ResizeObserver` 在吸顶状态下量到的仍是布局盒高度**（`borderBoxSize` 语义） | 浏览器布局盒语义，jsdom 恒回报 0；把它钉成断言等于把浏览器行为写成自身契约 | 行为侧证：`message-scroller.test.tsx` 的回填两行（`0` 回落 / 正常值 88 回填）证「回填通道与回落护栏成立」；盒模型语义属实现前提（D5），由手工验证过一次 |
| **真实浏览器里的滚动效果与跟随手感**（`scrollTop` 写入后视口是否真的移动、`anchorTo: 'end'` + `followOnAppend` 是否把用户黏在底部、上滑后再回底能否恢复跟随） | 依赖真实滚动引擎、真实几何与真实 `isAtEnd` 判定；jsdom 无滚动引擎（`maxScroll` 由桩造出），只能观察「shim 是否发起写入、写到哪个 offset」，观察不到「视口真的移动」 | **写入侧已可测**（本版修订）：AC-13 三处在实例级 `scrollTo` 桩下断言 `scrollTop` 落到末端目标值（`maxScroll`）与「不该复位时不动」；**视觉效果**由 `tasks.md` 阶段五 / 阶段六的手工滚动验证兜底（长会话打开即贴底、切会话落到末端、上滑不被拽回、回底恢复跟随） |
| **`scroll-fade-b` 的 `animation-timeline: scroll(self y)` 实际生效** | CSS scroll-driven animation 需要真实滚动容器与布局，jsdom 不实现（类名在场 ≠ 动画生效） | 机械判据：`message-scroller.test.tsx` 断言 `scroll-fade-b` 类名**留在 Viewport 元素上**（落点判据，挪走即红）；视觉生效由手工验证核对（AC-7） |
| **shim 与透镜的私有实现件独立测试路径**（`measureWithFallback` / `useStickyMeasure` / `useScrollerContext` / `MessageScrollerSlice` / 复位 effect；`renderItem` / `renderSticky` 闭包 / `MessageContainer` / `ToolPartView` / `PartView` / `TurnStatusLine` / `ChatBubbleBase`） | design「公共函数 / API」表明示按纪律不列（私有件）；直测须为测试新增导出，违反 AGENT.md「No test-only exports」并污染 `knip` | 经所在文件的公共 API 用例全覆盖：虚拟项与吸附层两条零测量回落路径、正常测量路径、`resetKey` 四条复位时机（⑥′⑥″ 与其反向）、`pendingScroll` 生命周期、context 缺席异常行、`renderItem === null` 不产节点行、吸附条目按索引交织行；状态行与部件分派由 `agent-messages.test.tsx` 的既有 `data-testid` 断言承接 |
| **`MessageScrollerProviderProps` / `MessageScrollerGeometry` / `MessageScrollerStickyEntry` / `MessageScrollerItemProps` / `MessageScrollerStickyProps` / `MessageScrollerButtonProps` / `UserTurnSpan` 的类型契约** | **纯类型面 / 编译期事实**：字段名与入参形状在运行期不存在（`strict` / `noUnusedLocals` / `isolatedModules` 下由 `tsc` 强制）；类型错值（`renderItem` 返回 `string`、`getItemKey` 返回对象、`renderSticky` 少传 `geometry`、`resetKey` 传 `number`）只能编译期拒绝 | `vp check` 的 `tsc --noEmit`（`client:build` / `client:check`）；运行期侧证是各组件入参行（缺省值 / 必填 / 可选缺席三面）、`renderSticky` 缺席不抛错行与 `resetKey` 三态行 |
| **`AgentMessagesProps` / `AgentUIMessage` / `TurnSummary` 的形状零漂移** | 类型面；`AgentMessagesProps` 三字段零改动是 AC-12「消费点零 diff」与 AC-13「复位机制不上溢」的结构前提 | `tsc --noEmit`（三字段形状与 `AgentUIMessage` 转发不变）+ `git diff --stat -- src/lib/agent-adapter.ts src/types/generated` 为空；运行期侧证是复位三行全部只用 `messages` / `loading` / `running` 驱动，以及全部消费点套件的既有断言零改全绿（`agent-session-view.test.tsx` / `src/views/**`） |
| **AC-9 的「全套件绿」与 AC-11 的机械门禁**（`vp check` / `knip` / `vp test --sequence.shuffle` / `server:check` / `bindings:export` 零 diff / `api:check` 零 diff / `cargo nextest`） | 机械门禁步骤，由桌面端静态检查与 test-execution 相位执行；在测试设计里复述命令与阈值等于复制运行脚本细节 | 门禁执行结果 + 本设计三章用例对覆盖率的支撑（新增断言全部落在真实契约面：过滤判据族、会话身份族、虚拟层零 user 节点、吸附层几何闭合、零测量回落、HUD 条件输出、复位时机正反两面、导出面收窄——不为覆盖率造句） |
| **AC-12 的边界零 diff 面**（`src-tauri/**`、`src/types/generated/**`、`src/types/dto.ts`、`src/lib/{agent-adapter,agent-transport,parse}.ts`、`src/hooks/**`、`src/components/agent/{agent-blocks,agent-input,tool-cards,session-panel,agent-session-view}.tsx` 与 `agent-error.ts`、`src/components/ui/**` 除 `message-scroller.tsx` 的改动与 `questionnaire.tsx` 的删除、`src/views/**`、`.dev-team/architecture/**`、`openspec/specs/**`、`openspec/config.json`、`packages/desktop/knip.json`、本 change 的 `explore.md`） | 「正确性主张」就是这些文件一行不变，运行期不可观测；在测试里读产物会把用例绑死在路径布局上 | `git diff --stat` 逐项审查（`tasks.md` 阶段七自查逐字列出）；行为侧证是这些文件的既有套件全绿（`src/lib/*.test.ts`、`agent-blocks` 相关、`src/views/**` 九套件回归登记）；架构模型零漂移由「纯函数落 `src/lib/**` 封闭排除面」的设计选点（D4）保证 |
| **其他套件的机械适配与回归登记**（`agent-session-view.test.tsx` / `session-panel.test.tsx` / `index.test.tsx` / `agent-render-memo.test.tsx` 的既有两例 / `src/views/**` 九套件）的**新增覆盖** | proposal「测试文件」表口径：这些套件的源文件零 diff（AC-12），本变更只做「打桩后仍可见」的确认与条数断言分查；在其上增补新契约用例会让测试范围越出 design 的五个实现文件 | 回归登记：全绿即「消费点接线零改动 + 注入形态未破 + 虚拟化未使既有断言失明」的判据；新契约判据集中在三个被测文件的用例（`agent-render-memo.test.tsx` 唯一新增项已折进 `agent-messages.tsx` 章节的组合行并标注执行文件） |
| **`@shadcn/react` 依赖移除后「仓内零可达消费者」的完整性** | 依赖图可达性是构建期 / 打包期事实，运行期无可断言对象（`pnpm-lock.yaml` 是否收缩属包管理器行为，生成物不手改） | `grep -rn "@shadcn/react" packages/desktop/src packages/desktop/knip.json` 零命中（桌面端执行）+ `knip` 零 unused dependency + `git diff packages/desktop/knip.json` 为空（未走豁免捷径）；残留 import 会立即编译错（`vp check` 的 tsc） |
