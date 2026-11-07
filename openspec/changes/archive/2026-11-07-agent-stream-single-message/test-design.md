# 测试设计: agent-stream-single-message

> **变更**: agent-stream-single-message
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（变更范围 / 测试文件表 / 新增用例 ①–⑲ / AC-1..AC-13 / R-1..R-11）、`design.md`（D1–D20 / 架构组件 7 行 / 新增文件「无」+ 修改文件 5 项 / 公共函数·API 表 12 行 / 类型定义 9 行 / 数据模型两层 + 折叠伪码 + 实时路状态机 / AC 落点 / 待决问题 1..8）、两份 delta spec（`desktop-agent-chat-infra` 5 requirement / `desktop-agent-execution` 3 requirement）、`tasks.md`（五阶段与编译闭合批）、工作树源码与既有测试盘点
> **被测范围**: design「变更清单 - 修改文件」子表 **5 项全列**——4 个前端代码面（`src/lib/agent-adapter.ts`、`src/lib/agent-transport.ts`、`src/hooks/use-agent-session.ts`、`src/components/agent/agent-messages.tsx`）+ `packages/desktop/package.json`（版本行，无测试路径，登记于「不可测试项」）；**新增文件零项、删除文件零项**（design 明示「无新增文件」）
> **本变更性质**: 纯前端实时流语义重构——实时发射层「一事件一消息」→「一轮一条 assistant 消息 + step 分段」，折叠层「一事件一消息 + 轮行独立消息」→「按轮折叠 + 回合元数据」；**Rust 后端零改动、生成物零改动、零新增依赖、零新增 IPC 命令**。因此用例集中在四个纯前端面：chunk 帧序（transport）、按轮分组与 id 锚点（adapter）、双起点提问气泡与收口一致（hook）、轮状态可见性（组件）；不为覆盖率造用例（R-10）。

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`，`vp check` 的 typeCheck 为编译闭合判据）；`environment: jsdom`、`include: src/**/*.test.ts?(x)`、覆盖率 lines 90 / branches 85 / functions 90 | 与被测源文件**同目录共置**的 `<模块名>.test.ts`（`.test.tsx` 仅组件/hook 渲染面） | `import { describe, expect, expectTypeOf, it, vi } from 'vite-plus/test'` + `@testing-library/react`（`render` / `renderHook` / `act` / `waitFor` / `screen` / `within`）；进程边界替身一律 `vi.hoisted()` + `vi.mock('@tauri-apps/api/core')`（`invokeMock` 按命令名分发、`ChannelMock` 可编程 class 捕获 `onmessage`），`ai` / `@ai-sdk/react` 的 `useChat` **保留真实实现**（reducer 归约是对拍判据本体）；迭代类型标题中文长句 |
| 组件渲染面 | 同上（jsdom） | `src/components/agent/*.test.tsx` | `data-testid` 定位（`chat-bubble` / `agent-messages` / `history-load-hint` / `turn-status-line` 族）；jsdom 缺 `IntersectionObserver` 时以既有 `FakeIntersectionObserver` 显式驱动触顶（运行环境能力替身，非内部模块替身） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、`null`、极值、恰好阈值、生命周期交错点、类型签名外的脏值）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增断言**；未带前缀的为**新增**或**按新形状改写**（改写只动 id 形状与 chunk 帧序两类断言，语义断言逐条保留——R-7）。
- **参数类型边界口径（贯穿各章节）**：`seq: number` 取 `0` / `-1` / `Number.MAX_SAFE_INTEGER` / `undefined`（越签名脏值）+ 常规 `7`；`turnId: number | null` 取 `null` / `0` / `-1` / 极值；字符串参数取 `''` / 超长（>1000 chars）/ `\n\0 emoji` / `undefined` + 常规；数组参数取 `[]` / 单元素 / 1000 元素 / `undefined`（越签名脏值）+ 常规；对象参数取 `{}` / 缺必填字段 / 多余字段 / `undefined` + 常规；`TurnSummary['status']`（四值枚举）取全 4 值 + 非法值。

**待测功能条目口径**

`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目（adapter 7 行含 3 行退役、transport 2 行、hook 2 行、组件 1 行，合计 12 行逐行落地）。design 的私有实现件（`foldSessionMessages` 内部分组 / 行匹配 / `synthesizeRow`、transport 的 `ensureStart` / `finishRecord` / `eventChunks` / `partAnchors`、`agent-messages.tsx` 的 `hasVisibleParts` / `turnStatusOf` / `TurnStatusLine`）**不列本表**（design 明示），其覆盖经所在文件的公共 API 用例承接——不 grep 源码导出声明、不虚构 API 行。

**测试布局决策（源文件 → 测试文件的映射依据）**

1. **零新建测试文件**：design 明示「本变更零新增源文件」，四个源文件的同目录测试文件全部已在盘上（`agent-adapter.test.ts` 1570 行 / `agent-transport.test.ts` 1826 行 / `use-agent-session.test.ts` 1152 行 / `agent-messages.test.tsx` 604 行），本变更是**改写 + 追加**，不新建文件（proposal「测试文件」表的处理列逐行对应）。
2. **`use-agent-session.ts` 有第二个测试文件** `use-agent-session.throttle.test.tsx`（33 行）：实测该文件只断言「`useChat` 以 100ms 节流窗订阅 messages」，**无任何消息 id 断言**（proposal 表内「随 id 锚点改消息 id 断言」在实况下为空集）。故它在本设计中只作**回归登记行**（挂在 `use-agent-session.ts` 章节的用例表末），本变更零改该文件。
3. **组合用例挂靠按链路发起方**：AC-1（一轮一条气泡，真实 reducer 归约）与 AC-3 / AC-4 / AC-6（帧序与 id 派生）的链路发起方是 **transport**（chunk 流的产出者）⇒ 挂 `agent-transport.ts` 章节；AC-5（两路 id 同源、收口 `isContinuation` 成立）与 AC-2 的「发起流本地提问 + 归一」链路发起方是 **hook**（`sendMessage` 与 `onFinish` 收口）⇒ 挂 `use-agent-session.ts` 章节；AC-9 的「`stopped` 不触发顶部横幅 + 同轮位置有『已停止』行」链路最上层调用方是 `AgentSessionView`，但该源文件**零 diff**（AC-13），故该行挂在 `agent-messages.tsx` 章节并在行内标注**执行文件是 `agent-session-view.test.tsx`**（archive 先例 `2026-11-02-change-detail-live-stdout` 第 3 条的同一手法）。不新建独立集成测试章节。
4. **不 mock 被测件与被测链路内部**：`agent-adapter` / `agent-transport` / `use-agent-session` / `agent-messages` 之间以及与 `ipc` / `ipc-drift` / `agent-blocks` / `tool-cards` / `parse` 全部真实组合——mock 它们等于让断言自证（AGENT.md 规则 5 与 `knip` 判据下也不为测试新增导出）。
5. **真实 reducer 是对拍判据本体**：`agent-adapter.test.ts` 既有 `replayTransport(chunks)` + `useChat` 装置（真实 `ai` v7 reducer）保留并扩展到 step 分段面——design R-1 的两条外部假设（`reset-step` 只清最后一个 `step-start` 之后、`finish` 的 `messageMetadata` 深合并）只能由真实 reducer 归约断言，替身化会同时废掉 AC-1 / AC-3 / AC-6 三条判据。
6. **夹具按轮构造、不按事件构造**：各套件里「一事件一消息」的 fixture（`eventsToUIMessages([...])` 逐事件产一条）改为「一组事件 → `foldSessionMessages(events, turns)` 产一轮三序」，断言改按新形状；`runRecordToUIMessage` 构造面全部退役。
7. **覆盖口径不在本设计调参**：`vite.config.ts` / `knip.json` / `stryker.config.json` / `openspec/config.json` 阈值文件零改动；达标与否是 test-execution 相位读覆盖率报告的机械判定。

---

## 验收范围

<!-- 测试区域：前端 `packages/desktop/src/**/*.test.ts(x)`（与源文件同目录共置；运行器 `vp test`，jsdom）。本变更**零新建测试文件**：四个既有测试文件改写 + 追加，其余套件（`agent-render-memo.test.tsx` / `index.test.tsx` / `session-panel.test.tsx` / `agent-session-view.test.tsx` / `src/views/**` 会话面板套件）只做夹具与 id 形状的机械适配（proposal「测试文件」表），其结构性断言零改动，见「不可测试项」的回归登记段。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|------------------|----------------|
| AC-1 | **一轮一条气泡**：一轮含「提问 + 思考 + 工具调用 + 回答」时，实时流归约后该轮恰一条 `assistant` 消息（部件数不限、按首次到达顺序），MUST NOT 出现同一轮第二个 assistant 气泡 | `packages/desktop/src/lib/agent-transport.ts` → `packages/desktop/src/lib/agent-transport.test.ts`（chunk 序列 + 真实 reducer 归约）；`packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`（折叠侧 assistant 恒一条）；`packages/desktop/src/components/agent/agent-messages.tsx` → `packages/desktop/src/components/agent/agent-messages.test.tsx`（呈现侧气泡计数） |
| AC-2 | **提问角色不再靠 seq 猜**：续问一轮，实时流中提问消息 `role === 'user'` 且排在 assistant 之前（结构保证）；id 为 `turn-<turnId>-user`；transport 对该事件零 chunk、恰一条气泡 | `packages/desktop/src/lib/agent-transport.ts` → `packages/desktop/src/lib/agent-transport.test.ts`（零 chunk）；`packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`（fold 侧提问序与 id）；`packages/desktop/src/hooks/use-agent-session.ts` → `packages/desktop/src/hooks/use-agent-session.test.ts`（发起流本地推 + 归一、订阅流追加） |
| AC-3 | **step 分段**：一次模型响应一次 `start-step`；密封 `Message` 到达先 `reset-step` 再追加整块，前面 step 的密封块不被清除；工具结果只投 `tool-output-available`、不开新 step | `packages/desktop/src/lib/agent-transport.ts` → `packages/desktop/src/lib/agent-transport.test.ts` |
| AC-4 | **part id 确定性**：同一 step 内同一 kind 的 delta 共用「首个 delta 的 seq」派生的 id（`t-<seq>` / `r-<seq>`），跨 step 不撞；订阅流从任意 `since_seq` 起步不产生 `missing part` 错误 | `packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`（`deltaPartId` 直调 + 第三参缺省/注入）；`packages/desktop/src/lib/agent-transport.ts` → `packages/desktop/src/lib/agent-transport.test.ts`（两起点归约一致） |
| AC-5 | **id 锚点一致**：实时路 assistant id 与折叠路同为 `turn-<turnId>`；收口 `isContinuation` 成立（MUST NOT 收口后整段重挂） | `packages/desktop/src/hooks/use-agent-session.ts` → `packages/desktop/src/hooks/use-agent-session.test.ts`；`packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`（fold 侧 id 锚点两分支） |
| AC-6 | **record 归 metadata**：`Record` 信封到达时该轮 assistant 的 `metadata.record` 为该轮轮行；MUST NOT 再产 `data-run-record` 部件或独立 system 消息；`data-run-record` / `runRecordToUIMessage` / `provisionalMessageId` 零残留（`knip` 零未用导出） | `packages/desktop/src/lib/agent-transport.ts` → `packages/desktop/src/lib/agent-transport.test.ts`（`finish` + `messageMetadata.record`）；`packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`（导出键集与部件词表）；`knip` / `tsc` 判据见「不可测试项」 |
| AC-7 | **轮分组不依赖 RunStarted**：无 `TurnDone` 的停止轮 + 其后正常轮的转录折叠，轮行 MUST NOT 错位；零事件轮轮行亦就位 | `packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`；`packages/desktop/src/hooks/use-agent-session.ts` → `packages/desktop/src/hooks/use-agent-session.test.ts`（重放装载三轮含停止轮） |
| AC-8 | **旧转录可读**：`turn_id` 全为 `null` 的存量转录按 `RunStarted` 顺序分组，消息数与本变更前形状等价、不抛错；MUST NOT 第二套分组逻辑 | `packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`；`packages/desktop/src/hooks/use-agent-session.ts` → `packages/desktop/src/hooks/use-agent-session.test.ts`（重放装载旧转录 fixture） |
| AC-9 | **空内容轮可见**：`parts` 空 + `failed` 呈状态行（标题 + 可展开原文）；`parts` 空 + `stopped` 呈「已停止」；`parts` 空 + `completed` 不渲染；`stopped` MUST NOT 触发顶部失败横幅 | `packages/desktop/src/components/agent/agent-messages.tsx` → `packages/desktop/src/components/agent/agent-messages.test.tsx`；横幅面执行落点 `packages/desktop/src/components/agent/agent-session-view.test.tsx`（源文件零 diff，见「测试布局决策」第 3 条） |
| AC-10 | **搜索卡两态回退不变**：`pause_turn` 拆分段累积在同一轮同一条消息内（不产生第二段 assistant 消息），收口后由重放折叠补出来源列表；`tool-web_search` 卡形状零变 | `packages/desktop/src/lib/agent-adapter.ts` → `packages/desktop/src/lib/agent-adapter.test.ts`（`global` 视野跨段收敛单卡、合轮单消息）；`packages/desktop/src/components/agent/agent-messages.tsx` → `packages/desktop/src/components/agent/agent-messages.test.tsx`（既有 web-search 用例在新形状下全绿） |
| AC-11 | **零后端改动**：`packages/desktop/src-tauri/**` 与 `src/types/generated/**` 零 diff；`cargo nextest run --workspace` 全绿 | `packages/desktop/src-tauri/**` / `packages/desktop/src/types/generated/**`（**零 diff 面**，见「不可测试项」） |
| AC-12 | **门禁与版本**：`vp check`（含 knip）零未用导出、`vp test` 覆盖率 lines ≥ 90 / branches ≥ 85 / functions ≥ 90、`bindings:export` 后 `bindings.*` 零 diff；`package.json` 0.6.19 → 0.6.20，`src-tauri/Cargo.toml` crate 版本行不跟 | `packages/desktop/package.json`（**配置面**，见「不可测试项」）；`packages/desktop/src-tauri/Cargo.toml`（版本行零跟）；运行期判据是本表其余各行用例 |
| AC-13 | **边界纪律**：`agent-error.ts` / `agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `src/views/**` 源码零 diff；生成目录零 diff；`explore.md` 零 diff | 零 diff 面清单（见「不可测试项」）；`stopped` 不触发横幅的行为侧证在 AC-9 的 `agent-session-view.test.tsx` 行 |

---

## 单元测试

### `packages/desktop/src/lib/agent-adapter.ts` -> `packages/desktop/src/lib/agent-adapter.test.ts`

既有测试文件**改写 + 追加**：`eventsToUIMessages` 五组（五变体映射 / 消息形状 / 工具同 id 配对 / 无主 toolResult / 空输入 / 未知透传）与 `runRecordToUIMessage` 两组改写为 `foldSessionMessages` 的按轮分组断言（`describe` 标题随入口更名）；`deltaPartId` / `provisionalMessageId` / 「IR 三层私有性」三组的 id 与导出键集断言按新签名改写。**语义断言逐条保留**（工具同 id 配对 / 无主 result 占位 / 引用挂 text 元数据 / 未知事件 `data-raw` 透传 / `global` × `local` 视野差异 / 计划层对拍 / 搜索两态——R-7 的反向约束）。装置沿用既有 `replayTransport(chunks)` + `useChat`（真实 reducer）、`shapeOf` 对拍投影与 `adapterModule` 命名空间导入。

#### 待测功能

- `foldSessionMessages(events: AgentEvent[], turns: TurnSummary[]) => AgentUIMessage[]`: 重放 / 收口折叠唯一入口。只消费密封事件；按信封 `turnId` 分组（旧转录 `null` 按 `RunStarted` 边界）；输出序 = `turns` 行序（零事件轮就位）+ 剩余组按组序追加；每轮三序 = aux system 消息（逐事件，`id = evt-<seq>`）→ 提问消息（可缺席，`id = turn-<turnId>-user` / `evt-<seq>`）→ assistant 消息（恒一条且为轮内末条，`id = turn-<turnId>` / `evt-<组首 seq>`，parts 可空）；`metadata = { seq, parentToolUseId, record }`；`TurnDone` 只兜 status（行缺席时经合成轮行）。
- `deltaPartId(seq: number, part: 'text' | 'thinking') => string`: 返回 `t-<seq>` / `r-<seq>`；`seq` = 该 step 内首个同类 delta 的 `seq`（作用域在 transport 的 `partAnchors`），同一 part 后续 delta 复用、跨 step 不撞、与整块部件 id `t-<seq>-<n>` 无碰撞域。
- `DeltaPartAnchor`（type alias）: `(kind: 'text' | 'thinking', seq: number) => number`——delta part 锚解析回调，由 transport 注入持 `partAnchors` 的实现；`eventToChunk` 缺省实现返回入参 `seq`（缺省 = 逐 delta 独立 part）。
- `eventToChunk(event: AgentEvent, currentMessageToolCalls?: ReadonlySet<string>, deltaAnchor?: DeltaPartAnchor) => AgentUIMessageChunk[]`: **只产块 chunk**（delta 路 + 密封块路 + 工具就地补全）。**不产** `start` / `start-step` / `reset-step` / `finish`（帧归 transport）；`role = 'user'` 的密封事件返回 `[]`；aux 事件返回单条 data 部件 chunk。
- `provisionalMessageId`（**退役**）: 由 `turn-<turnId>` + per-step `deltaPartId(seq, part)` 取代；模块导出面 MUST NOT 再出现该名。
- `eventsToUIMessages`（**退役**）: 由 `foldSessionMessages` 取代；`knip` 零未用导出。
- `runRecordToUIMessage`（**退役**）: 由 `metadata.record` 承载轮行；`data-run-record` 部件与 `AgentDataParts['run-record']` 成员一并退役。

<!-- design.md「类型定义」表另声明 AgentMessageMetadata.record / AgentDataParts 去成员 / 四份派生别名，它们不是「公共函数 / API」行；其覆盖归入上述 foldSessionMessages 与 eventToChunk 用例（metadata 与部件词表断言），不另立条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `foldSessionMessages` | 正向 | 真实运行事件（`turnId != null`）三轮「正常 / 停止（无 `TurnDone`）/ 正常」+ 三行轮行 → 每轮恰一条 assistant（`id = turn-<turnId>`）与提问（`turn-<turnId>-user`），第 2 轮 `metadata.record` 为 `stopped` 行、第 3 轮轮行落在第 3 轮（**MUST NOT 错位一格**）；实现零 `RunStarted` 计数逻辑（AC-7） | 单次 |
| `foldSessionMessages` | 正向 | 一轮密封五变体（`runStarted` / `message(assistant: text+thinking+toolUse)` / `message(tool: 同 id toolResult)` / `systemNotice` / `turnDone` / `raw`）+ 一枚 `messageDelta` → assistant 部件序 text / reasoning / tool(`output-available`) 与 seq 同序，`messageDelta` 被过滤不产消息；aux 各一条独立 system 消息（`data-run-started` / `data-system-notice` / `data-run-result` / `data-raw`）（AC-10 的「部件词表」面，`turnDone → data-run-result` 维持） | 单次 |
| `foldSessionMessages` | 正向 | 一轮含 `role='user'` 密封 → 提问消息 `role='user'`、`id = turn-<turnId>-user`、`metadata.seq` = 该提问事件 seq，且**排在该轮 assistant 之前**；assistant 恒为该轮**末条**（D7 的机械判据） | 单次 |
| `foldSessionMessages` | 正向 | `metadata` 保真：assistant 的 `seq` = 组首事件 seq、`parentToolUseId` = 首个 assistant 密封事件的归因（非 null 形态一组）；aux 的 `seq` = 该事件 seq；提问的 `seq` = 提问事件 seq（spec「保真字段可考」） | 参数化迭代（4 类消息） |
| `foldSessionMessages` | 正向 | 零事件轮（`turns` 有该行、事件侧零事件，如 ask 失败 / 立即停止）→ 该行位置产出 `id = turn-<turnId>`、`parts = []`、`metadata = { seq: null, parentToolUseId: null, record: 该行 }` 的 assistant（AC-9 / spec「空内容轮恒产一条消息」） | 单次 |
| `foldSessionMessages` | 正向 | `pause_turn` 拆段：`webSearch` use 侧块与同 id result 侧块分处两条密封 message → `global` 视野收敛为**单张** `tool-web_search` 部件（`input = { query }`、`output.search.results` 在场），result 侧不重复呈现；两段同属一轮同一条 assistant 消息（AC-10） | 单次 |
| `foldSessionMessages` | 异常 | 行缺失且组内有 `TurnDone` → 合成内存轮行：`status = isError ? 'failed' : 'completed'`、`record.error = null`、`turnId = event.turnId ?? -1`、`finishedAt = TurnDone.timestampMs`；`record` 非 null（`usage` 不入合成行，仍只由 `data-run-result` 承载） | 参数化迭代（`isError` 两值） |
| `foldSessionMessages` | 异常 | 行缺失且组内**无** `TurnDone` → `record = null`（不抛错、不虚构 status）；该轮仍产一条 parts 可空的 assistant | 单次 |
| `foldSessionMessages` | 异常 | `TurnDone` 不得充当轮边界：一轮内两枚 `TurnDone` 只产**一条** assistant，且不因此新增轮；`TurnDone` 出现在轮首 / 轮尾两形态等价 | 组合迭代（位置 2 × 枚数 1） |
| `foldSessionMessages` | 异常 | 运行时未知事件变体（越过 `AgentEvent` 类型签名的脏值）→ `data-raw` 原文透传、不抛错、不丢事件（未知事件不丢） | 单次 |
| `foldSessionMessages` | 边界 | `events = []` 且 `turns = []` → `[]`（空数组，不抛错） | 单次 |
| `foldSessionMessages` | 边界 | `events = []` 且 `turns` 三行 → 恰三条 assistant（`turn-<id>` 各一、`parts` 空、`record` 就位、`seq = null`），不含任何提问或 aux | 单次 |
| `foldSessionMessages` | 边界 | `turns = []` 且 `events` 两组（`turnId` 11 / 12）→ 按组序追加两条 assistant，`record` 分别为合成行或 `null`；MUST NOT 抛错 | 单次 |
| `foldSessionMessages` | 边界 | 旧转录退化面：`turnId` 全 `null`、两组以 `RunStarted` 为界 → 每轮提问 + 一条 assistant，id 为 `evt-<组首 seq>` / `evt-<提问 seq>`，消息数等价既有形状（无缺轮、不抛错、无第二套分组）（AC-8） | 单次 |
| `foldSessionMessages` | 边界 | 旧转录组序 × 轮行数的三形态：行数 == 组数（逐位配）/ 行数 > 组数（多余行各产空内容 assistant）/ 组数 > 行数（`turns[nullIdx++]` 越界走 `synthesizeRow` 或 `null`） | 参数化迭代（3 形态） |
| `foldSessionMessages` | 边界 | 单元素极简：仅一枚 `runStarted` → 一条 aux system 消息 + 一条 `parts` 空 assistant（`record = null` 或合成行） | 单次 |
| `foldSessionMessages` | 边界 | 超大列表：1000 条密封事件（多轮混合）→ 输出保序、每轮 assistant 恰一条、assistant 的 `metadata.seq` 单调不减、不抛错 | 单次 |
| `foldSessionMessages` | 边界 | `TurnSummary['status']` 四枚举值 `running` / `completed` / `failed` / `stopped` × 空 `parts` → `metadata.record.status` 逐字等于轮行（`running` 亦就位，不做终态过滤——过滤归渲染层） | 参数化迭代（4 枚举值） |
| `foldSessionMessages` | 边界 | 非法 `status`（越过类型的未来枚举 / 脏串）→ 原样透传不改写、不抛错（折叠层零业务判别） | 单次 |
| `foldSessionMessages` | 边界 | 超长文本（>1000 chars）与特殊字符（`\n` / `\0` / emoji）在 text 与提问块 → 部件 `text` 逐字保留、不截断不转义 | 单次 |
| `foldSessionMessages` | 边界 | 密封 assistant 事件 `blocks = []` → 该轮 `parts = []` 但消息**仍在**（不因零部件缺轮）、`record` 就位 | 单次 |
| `deltaPartId` | 正向 | `deltaPartId(7, 'text') === 't-7'`、`deltaPartId(7, 'thinking') === 'r-7'`；同 seq 两 kind 互异、同 kind 不同 seq 互异；同参重复调用幂等 | 单次 |
| `deltaPartId` | 异常 | 旧常量语义退役：返回值 MUST NOT 以 `t-delta-` / `r-delta-` 开头（`delta-<key>` 家族的机械反判据）；与整块部件 id `t-<seq>-<n>` 形状不相等 | 单次 |
| `deltaPartId` | 边界 | `seq` 四形态 `0` / `-1` / `Number.MAX_SAFE_INTEGER` / `undefined`（越签名脏值）→ 稳定输出 `t-<值>` / `r-<值>`，不抛错、不夹取（负号与极值不被归一） | 参数化迭代（4 形态） |
| `deltaPartId` | 边界 | `part` 枚举两值 + 非法值（`'reasoning'`）：非法值走非 text 分支得 `r-<seq>`，不抛错（`Enum` 的 N+1 面） | 参数化迭代（3 值） |
| `DeltaPartAnchor`（类型面） | 正向 | 类型契约：`eventToChunk` 第三参接受 `(kind, seq) => number` 回调；缺省不传时逐 delta 用自身 seq（行为面由 `eventToChunk` 缺省行覆盖）；类型错（返回 `string` / 参数错位）由 `vp check` 的 tsc 强制（类型面判据登记见「不可测试项」） | 单次 |
| `eventToChunk` | 正向 | `messageDelta`（text / thinking 各一枚，不传第三参）→ 恰一条 `text-delta` / `reasoning-delta`，`id = t-<seq>` / `r-<seq>`（缺省 = 逐 delta 独立 part）、`delta` 载荷逐字 | 参数化迭代（2 kind） |
| `eventToChunk` | 正向 | 注入 `deltaAnchor = (kind, seq) => firstSeqOf(kind) ?? seq` → 同 step 内三条 text delta 全部投**同一 id**（首个 delta 的 seq），thinking 另开一族（AC-4） | 单次 |
| `eventToChunk` | 正向 | 密封 assistant（text + thinking + toolUse）→ 只含块 chunk（`text-start` / `text-delta` / `text-end` / `reasoning-*` / `tool-input-available`），**零** `start` / `start-step` / `reset-step` / `finish`（帧归 transport） | 单次 |
| `eventToChunk` | 正向 | 密封 `role='tool'` 且同 id 已在 `currentMessageToolCalls` → 只投一条 `tool-output-available`（无消息头、无 input 重投）；reducer 归约后 `t1` 的 input 与 output 同住一条消息、无双卡 | 单次 |
| `eventToChunk` | 正向 | aux 四变体各一枚（`runStarted` / `systemNotice` / `turnDone` / `raw`）→ 单条 data 部件 chunk，载荷与折叠路同事件**逐字段相等**（计划层对拍同形）；`turnDone → data-run-result` 维持（R-9 反向钉住） | 参数化迭代（4 变体） |
| `eventToChunk` | 异常 | `role='user'` 的密封 `Message`（含 text 块与空 blocks 两形态）→ 返回 `[]`（chunk 协议无 role 字段；提问由会话状态侧承载）（AC-2） | 参数化迭代（2 形态） |
| `eventToChunk` | 异常 | 无主 `toolResult`（未命中视野）→ 就地合成占位工具计划 chunk（`tool-input-available` + `tool-output-available`），`toolName = 工具调用`；与折叠路孤儿卡语义一致 | 单次 |
| `eventToChunk` | 异常 | 未知块变体 / 未知事件变体（越过类型的脏值）→ 未知块不产部件不抛错、未知事件产 `data-raw` 兜底（双层收口：编译期 `_exhaustive` + runtime 兜底） | 参数化迭代（2 形态） |
| `eventToChunk` | 边界 | 密封 assistant `blocks = []` → 返回 `[]`（零块 chunk；帧由 transport 补，故「无部件的事件」不产任何 chunk 也不抛错） | 单次 |
| `eventToChunk` | 边界 | `currentMessageToolCalls` 三形态：`undefined`（缺省 `EMPTY_TOOL_CALLS`）/ 空 `Set` / 含该 id 的 Set → 前两者等价走占位卡、后者就地补全；缺省参零漂移 | 参数化迭代（3 形态） |
| `eventToChunk` | 边界 | 同事件混含命中与未命中结果 + 重复 id 结果 → 命中项先投 output（就地补全）、未命中项照常起块、重复 id 只补首个（既有语义保留） | 组合迭代（命中 2 × 重复 2） |
| `eventToChunk` | 边界 | 全部块均命中视野（纯结果事件）→ 返回的 chunk 全部为 `tool-output-available`（零 input、零部件开件） | 单次 |
| `eventToChunk` | 边界 | delta 载荷边界：`text = ''` / 超长（>1000 chars）/ 含 `\n\0 emoji` → `delta` 字段逐字保留、不截断不转义 | 参数化迭代（3 形态） |
| `provisionalMessageId`（退役） | 异常 | 模块运行时导出键集合**恰为** `['deltaPartId', 'eventToChunk', 'foldSessionMessages']`（`provisionalMessageId` / `eventsToUIMessages` / `runRecordToUIMessage` 三者缺席）；私有件 `PairingScope` / `planBlock` / `plansToParts` / `plansToChunks` / `auxEventData` 仍不外露（IR 三层私有性） | 单次 |
| `eventsToUIMessages`（退役） | 异常 | 同上一行的导出键集判据（三项退役合判）；源码残留与 `knip` 零未用导出登记见「不可测试项」 | 单次 |
| `runRecordToUIMessage`（退役） | 异常 | `foldSessionMessages` 产出中**零** `data-run-record` 部件（遍历全部消息与部件断言 `part.type !== 'data-run-record'`）；轮行只出现在 `metadata.record`；`data-run-result`（`turnDone`）仍在场（两层级不互相替代） | 单次 |
| 两路同构（组合，入口 = 本文件的折叠路 × 真实 reducer） | 组合 | 同一事件序列分别经 `foldSessionMessages` 与「`eventToChunk` 产块 + 按 design「实时路状态机」就地拼帧（`start` / `start-step` / `reset-step` / `finish`）」流入真实 `useChat` reducer 归约 → assistant 消息 **id 相等**（`turn-<turnId>`）、过滤 `step-start` 后的**部件序与部件载荷等价**、`metadata.record` 相等（AC-5 / AC-1 的对拍主判据；D7 的「assistant 恒末条」使 `isContinuation` 成立。真实 transport 帧序的主判据在 `agent-transport.test.ts` 章节） | 组合迭代（正常轮 + 停止轮） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：`ai` / `@ai-sdk/react` 的 `useChat` | 真实 reducer 归约（既有 `replayTransport` + `renderHook(useChat)` 装置），`{ type: 'finish' }` 收尾——对拍判据本体（R-1 的两条外部假设只能由此侧证） | 两路同构、step 部件存活、`tool-output-available` 就地补全行 |
| 不 mock：`../types/dto` 类型与 fixture | 事件由 serde camelCase 线格式字面量构造（`turnId` 显式给值 / 显式 `null`），类型面由 tsc 强制 | 全章节 |
| 不 mock：`PairingScope` / `planBlock` / `plansToParts` / `plansToChunks`（本文件私有件） | 真实组合——`global` × `local` 视野差异是既有显式锁定断言的被测对象，替身化会使「两视野可辨」自证 | 视野差异、搜索两态、计划层对拍行 |
| 不 mock：被测函数自身 | `foldSessionMessages` / `eventToChunk` / `deltaPartId` 直调（同模块内不 mock 自己） | 全章节 |
| 不 mock：时钟 | 事件 `timestampMs` 为字面量；`synthesizeRow` 的 `startedAt` / `finishedAt` 由事件时间戳派生，断言只比相等关系不比「当前时间」 | 合成轮行行 |

### `packages/desktop/src/lib/agent-transport.ts` -> `packages/desktop/src/lib/agent-transport.test.ts`

既有测试文件**按 step 语义重写**（proposal 处理列：1826 行 / 68 用例，`reset-step` 命中 73 处）：chunk 逐块断言改按「帧 + 块」两段（`start` 每轮一次、`start-step` 每响应一次、`reset-step` 恒在密封 assistant 前、`finish` 携 `messageMetadata`）；`openedParts` / provisional 开件组 / `data-run-record` 用例删除；**保留**「两路同构」对拍（改写为「发起流 vs 订阅流对同一事件序列归约一致」）与全部会话参数面 / 状态门 / abort / 漂移上报用例（语义零改）。装置沿用 `ChannelMock`（原型 accessor + 私有字段，与真实 `Channel` 同构）+ `invokeMock` 按命令名分发 + 真实 `ReadableStream` + `drain(stream)`。

#### 待测功能

- `TauriAgentTransport.sendMessages(options) => Promise<ReadableStream<AgentUIMessageChunk>>`（体内变化、签名与 body 参数面不变）: 发起流——Channel 先于 invoke 建立；每轮恰一次 `start`（`messageId = turn-<turnId>`，`turnId` 取自该轮首个信封），每响应一次 `start-step`，密封 assistant 前恒一次 `reset-step` + 追加整块（前面 step 的密封块存活），工具结果就地补全不开 step；`role = 'user'` 密封事件零 chunk；`Record` 信封 → `finish` 携 `messageMetadata: { seq, parentToolUseId: null, record }` 后关流（该轮消息尚未存在时先 `start` 建消息）。
- `TauriAgentTransport.reconnectToStream(options) => Promise<ReadableStream<AgentUIMessageChunk> | null>`（体内变化、签名不变）: 订阅流——参数解析与状态门（`detail.turns.some(status === 'running')`）不变；流内 step 分段与 `start` / `finish` 帧与发起流**共用同一 `envelopeStream` 单点**；从任意 `sinceSeq` 起步时补齐起始帧，reducer 归约结果与发起流对同一事件序列一致（无 `missing part`）。

<!-- design.md「公共函数 / API」表对本文件只声明上面两行（体内变化、签名不变）；私有件 ensureStart / finishRecord / eventChunks / partAnchors / currentToolCalls 的覆盖经这两行的 chunk 序列断言承接，不另立条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `sendMessages` | 正向 | 一轮两次响应（`RunStarted` → user `Message` → delta₁ → 密封 A₁ → toolResult → delta₂ → 密封 A₂ → `Record`）drain 全流 → `start` 恰 **1** 次且 `messageId === 'turn-<turnId>'`、`start-step` 恰 **2** 次、`reset-step` 恰 **2** 次且两次都紧跟在该密封事件的块 chunk **之前**；A₁ 段的 `text-start` / `text-delta` 在 A₂ 到达后仍存活（AC-1 / AC-3） | 单次 |
| `sendMessages` | 正向 | 同上序列经真实 `useChat` reducer 归约 → 该轮恰**一条** assistant 消息（`id = turn-<turnId>`），部件序 = A₁ 段口部件 → A₂ 段部件（思考 / 正文 / 工具卡各一），无第二条气泡（AC-1 的归约侧主判据） | 单次 |
| `sendMessages` | 正向 | `Record` 信封 → `finish` chunk 的 `messageMetadata` 同时含 `record`（该轮轮行整行）与 `seq`（= `start` 写入的首事件 seq，显式回填不被 `null` 覆盖）、`parentToolUseId: null`；流随后关闭；全流**无** `data-run-record` 部件、无第二条消息（AC-6） | 单次 |
| `sendMessages` | 正向 | `role = 'user'` 密封事件 → 该事件产**零** chunk（drain 结果中前后 chunk 序列与不含该事件的流逐条相等）；提问气泡不在 transport 侧产生（AC-2） | 单次 |
| `sendMessages` | 正向 | 工具就地补全：密封 assistant（`toolUse` id=`t1`）→ 随后 `role='tool'` 同 id 事件 → 该事件只投一帧 `tool-output-available`（无 `start` / `start-step` / `reset-step`、无 input 重投）；reducer 归约后 `t1` 单卡 `output-available`（AC-3） | 单次 |
| `sendMessages` | 正向 | aux 事件（`runStarted` / `systemNotice` / `turnDone` / `raw`）→ 各自只产单条 data 部件 chunk，**不开新消息**也不开新 step（无 `start` 追加、`start-step` 计数不增）；`turnDone` 仍产 `data-run-result`（D8 / R-9） | 参数化迭代（4 变体） |
| `sendMessages` | 正向 | `currentToolCalls` 本轮累积（跨 step 不清）：第一响应的 `toolUse` 在第二响应提交后仍命中，第二响应后的同 id 结果事件仍就地补全（不产生占位卡） | 单次 |
| `sendMessages` | 正向 | `start` 帧的 `messageMetadata.seq` = 该轮**首个**信封事件 seq（`RunStarted` / `user` / `delta` 三种首事件形态各一）→ 三形态下 `seq` 均取首事件值（D1 / D13） | 参数化迭代（3 形态） |
| `reconnectToStream` | 正向 | 订阅流从任意 `sinceSeq` 起步、首个信封为 delta → 补 `start`（`turn-<turnId>`）+ `start-step` + part-start，随后累积 delta；reducer 归约结果与发起流对同一事件序列**逐条相等**（无 `missing part`）（AC-4） | 单次 |
| `reconnectToStream` | 正向 | 两出口同一事件序列对拍：`sendMessages` 与 `reconnectToStream` 各 drain 后经同一 reducer 归约 → 消息 id / 部件序 / `metadata.record` 全等（两路同构不变量；共用 `envelopeStream` 单点的机械判据） | 组合迭代（发起流 × 订阅流） |
| `reconnectToStream` | 正向 | 状态门与寻址零改：`agent_session_watch` 携 `{ channel, root, sessionId, sinceSeq }`；多轮行含 running（`some` 而非 `every`）放行订阅（既有回归） | 单次 |
| `sendMessages` | 异常 | `agent_start` reject → 流以错误 reject、零 chunk 产出（既有回归；新帧纪律不改变失败面）；非 Error 原因包一层 Error 再透出（既有） | 参数化迭代（2 形态） |
| `sendMessages` | 异常 | 未知信封判别（`ipc` 既非 `event` 也非 `record`）→ 忽略单条、流不断开、后续事件照常产 chunk（既有回归） | 单次 |
| `sendMessages` / `reconnectToStream` | 异常 | `Record` 收口后迟到事件 → 零 chunk、`onEvent` 零调用；第二个 `Record` 不重复收口（`closed` 守卫幂等；既有 + 新 `finish` 单点） | 单次 |
| `reconnectToStream` | 异常 | abort 两形态（调用前已 abort / 订阅中 abort）→ 流关闭、后续迟到信封零 chunk；命令 resolve 迟到不重复关闭（幂等，既有回归） | 参数化迭代（2 形态） |
| `reconnectToStream` | 异常 | `session_detail` 返回 null 或轮行全非 running → 返回 `null` 且零 Channel 零订阅（既有回归） | 参数化迭代（2 形态） |
| `sendMessages` | 异常 | 缺会话参数（`root` / `prompt` / `source` 任一非串）、非法 `permissionMode`、非法 `agent`、非法 `sessionId` / `sourceRef` / `cwd` 类型、`trigger !== 'submit-message'` → 参数校验先行拒绝且零 invoke（既有回归，语义零改） | 参数化迭代（既有站点全集） |
| `sendMessages` | 边界 | `turnId` 为 `null` 的信封（无轮归属构造面）→ `start` 的 `messageId = evt-<该首事件 seq>`；随后 `Record`（`record.turnId` 有值）若与已建消息 id 不同则以 `turn-<record.turnId>` 另发 `start` 再 `finish`（D13 / D6 兜底分支） | 单次 |
| `sendMessages` | 边界 | 零事件轮：Channel 直达 `Record` → 先发 `start`（`turn-<record.turnId>`，`messageMetadata.seq = null`）再发 `finish`（携 `record`），全流恰 2 条 chunk、恰一条消息（AC-9 的「空内容轮有消息可挂」结构前提） | 单次 |
| `sendMessages` | 边界 | 连续两条密封 assistant 之间**无 delta**（D2 分支②）→ 第二次仍先发 `start-step` 再 `reset-step`，第一次的密封块**存活**（无 `step-start` 时 `reset-step` 会误清前段）；drain 断言 + reducer 归约双判据 | 单次 |
| `sendMessages` | 边界 | 空 blocks 密封 assistant（无 delta 无部件）→ `〔必要时 `start-step`〕+ reset-step` 恰一次、零块 chunk、不抛错、前面 step 部件存活 | 单次 |
| `sendMessages` | 边界 | 同一 transport 连续两次 `sendMessages` → 第二流从干净状态起步（`start` 再发一次、`start-step` 重新计、`partAnchors` 空故 part-start 重发），跨流不残留（流闭包局部状态的既有约定） | 单次 |
| `sendMessages` | 边界 | delta 载荷边界：`''` / 超长（>1000 chars）/ 含 `\n\0 emoji` → chunk 的 `delta` 逐字透传、不 trim 不改写；`seq` 三形态 `0` / `1` / `Number.MAX_SAFE_INTEGER` 派生的 part id 稳定 | 组合迭代（3 载荷 × 3 seq） |
| `reconnectToStream` | 边界 | `sinceSeq` 三形态 `0` / 已观测最大 / `Number.MAX_SAFE_INTEGER` → 原样透传到 `agent_session_watch`（不做本地裁剪，既有回归）；`body` 缺字段 / 类型非法 → `null`（既有） | 参数化迭代（3 形态） |
| 消费侧取消（`sendMessages`） | 边界 | 消费侧 cancel 后迟到信封 → `onEvent` / `onRecord` 零调用、流不再产 chunk（既有回归）；新 `finish` 单点不改变取消语义 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` + `Channel`） | `vi.hoisted()` 手写 `ChannelMock`（原型 accessor + 私有字段，与真实 `Channel` 同构，投递读私有字段）+ `invokeMock` 按命令名分发（可编程 resolve / reject / deferred）；**进程边界**，MUST mock | 全章节 |
| 不 mock：`ReadableStream` / chunk 消费 | 真实实现（`drain(stream)` 逐条 `reader.read()`）——chunk 序列是被测输出本体 | 全章节 |
| 不 mock：`./agent-adapter` | `eventToChunk` / `deltaPartId` 真实调用——「帧归 transport、块归 adapter」的分工判据依赖两者真实组合 | 全部 chunk 序列行 |
| 不 mock：`ai` / `@ai-sdk/react` | 真实 reducer 归约（`useChat` + `replayTransport` 手法）——「前段密封块存活」「无 `missing part`」只能用真实 reducer 侧证（R-1 / R-2） | 归约侧判据行（AC-1 / AC-3 / AC-4） |
| 不 mock：`./ipc` 与 `./ipc-drift` | 真实薄包装与真实漂移上报（`resetDrift` / `driftReports`），信封形状与上报计数面零改（既有） | 信封判别 / 漂移上报行 |
| 不 mock：`../types/generated/bindings` | 类型与 schema 真实引用（`AgentRunMessage` 双变体）；本变更零生成物增量，替身化会掩盖「零 diff」事实 | 全章节 |

### `packages/desktop/src/hooks/use-agent-session.ts` -> `packages/desktop/src/hooks/use-agent-session.test.ts`

既有测试文件**改写 id 断言 + 追加两组新契约用例**（proposal 处理列：1152 行 / 45 用例，含 `['evt-0','evt-1','evt-2','turn-11']` 类断言）：`buildMessages` 相关断言改 `foldSessionMessages` 形状（`turn-<turnId>` / `turn-<turnId>-user`）；镜像面（`events` / `chain` / seq 游标 / 无进展闸 / early-resolve 镜像 / sourceRef·cwd 参数面 / 漂移上报）用例**逐条保留零改**；新增「本地提问副本归一」与「订阅流提问追加」两组、以及「收口前后 id 序一致」。装置沿用可编程 `ipc` 状态 + `ChannelMock` 直投信封（`deliverEvent` / `deliverRecord`）+ `mounted()` / `messageIds()` / `messageTexts()`。

#### 待测功能

- `useAgentSession(params: UseAgentSessionParams) => UseAgentSessionState`（体内变化、签名与状态形状不变）: 三处收口（`applyReplay` / `runReplayLoad` 的 catch 回落 / `onFinish`）统一改 `foldSessionMessages(events, turns)`；镜像面（`events` / `chain` / seq 游标 / 无进展闸 / `running` 库态派生）零语义变更；`UseAgentSessionState` 七字段形状不变。
- `UseAgentSessionState.sendMessage(input: AgentSessionSendInput) => void`（行为变化、签名不变）: 以 `{ id: pendingId, parts: [{ type: 'text', text }] }` 形态本地推提问（`pushMessage` 同步先于 `makeRequest`，结构上早于任何 chunk）；该轮提问事件到达时把本地副本 id **归一**为 `turn-<turnId>-user`（消息数不增）；订阅流（无本地副本）在提问事件到达时以 `turn-<turnId>-user` 追加并置于该轮 assistant 之前；`body` 八字段逐字不变。

<!-- design.md「公共函数 / API」表对本文件只声明上面两行。私有件 buildMessages（删除）/ applyUserQuestion / pendingQuestionIdRef / setMessagesRef 的覆盖经这两行承接：删除面的判据在「重放装载」行与「不可测试项」的静态检索行。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentSession` | 正向 | 重放装载：`session_detail`（三轮行：正常 / 停止 / 正常）+ 转录落位 → `messages` 为 fold 输出，id 序列 = `[turn-<11>-user, turn-<11>, turn-<12>, turn-<13>-user, turn-<13>]` 类形状（改写既有 `evt-0/evt-1/turn-11` 断言）；停止轮 `metadata.record.status === 'stopped'` 且后续轮行不错位（AC-5 / AC-7） | 单次 |
| `useAgentSession` | 正向 | 旧转录重放（`turnId` 全 `null`）→ 消息 id 走 `evt-<seq>` 兜底分支，每轮提问 + 一条 assistant，消息数与既有形状等价、不抛错、`error` 面为 null（AC-8） | 单次 |
| `useAgentSession` | 正向 | 收口一致（`isContinuation`）：实时流（发起流 delta + 密封 + `Record`）后的 id 序列与收口后（`onFinish` 的 fold 整表替换 → 随后重放覆盖）的 id 序列**逐元素相同**；MUST NOT 出现整段重挂成新 id（AC-5 / 新增用例 ⑭） | 单次 |
| `useAgentSession` | 正向 | 收口重放形状 vs 直接重开同一会话：两次装配产出的 `messages` 逐条相等（含跨消息工具单卡；既有「收口以重放收敛」用例改写为 fold 入口） | 单次 |
| `UseAgentSessionState.sendMessage` | 正向 | 发起流本地推入：`sendMessage({ prompt })` 后**同步**可见一条 `role='user'`、`parts=[{ type:'text', text: prompt }]` 的本地消息，且此时尚无任何 chunk / 无 assistant 消息（结构上先于任何 chunk，AC-2）；`agent_start` 的 body 八字段逐字不变（既有参数面断言保留） | 单次 |
| `UseAgentSessionState.sendMessage` | 正向 | 提问归一：随后该轮 `role='user'` 密封事件（`turnId = 13`）到达 → 该消息 id 变为 `turn-13-user`、`metadata.seq` = 事件 seq，**消息总数不增**（`messageIds().length` 前后相等）；顺序仍是提问在 assistant 之前（AC-2 / 新增用例 ⑫） | 单次 |
| `useAgentSession`（订阅流路径） | 正向 | 旁观（无本地副本）经订阅流收到该轮 `role='user'` 密封事件 → 以 `turn-<turnId>-user` 追加，并插在该轮 `turn-<turnId>` assistant 消息**之前**；收口后 fold 整表替换，提问仍恰一条、id 不变（AC-2 / spec「旁观路径的提问气泡」/ 新增用例 ⑬） | 单次 |
| `useAgentSession` | 正向 | 订阅流多轮：两轮各自提问事件 → 两条提问气泡各就位（不串轮、不重复）；assistant 仍恒一条/轮 | 单次 |
| `useAgentSession` | 正向 | 零内容轮：轮行有 running / stopped 但事件侧无 assistant 密封 → `messages` 含一条 `parts` 空、`metadata.record` 就位的 assistant（重放侧 AC-9 前提；空态判定在组件侧） | 单次 |
| `useAgentSession` | 异常 | 重放失败回落：`agent_session_transcript` reject → `messages` = `foldSessionMessages(镜像 events, 镜像 turns)`（与镜像一致、不抛错），`error` 携重放失败串（既有用例改 fold 入口） | 单次 |
| `useAgentSession` | 异常 | `session_detail` reject → `error` 置位、`loading` 复位、零订阅（既有回归，零改） | 单次 |
| `useAgentSession` | 异常 | `agent_session_watch` reject → 错误面呈现、不永处 streaming、重订次数有界（既有回归，零改） | 单次 |
| `useAgentSession` | 异常 | 发起流断流（未收 `Record`、提问事件也不可达）→ `onFinish` 触发 fold 兜底，**临时 pending id 不残留**（无 `pending-user-*` 形态 id）；本地副本语义由 fold 覆盖 | 单次 |
| `UseAgentSessionState.sendMessage` | 异常 | `root === null` 或在挂流（`isStreaming`）时 `sendMessage` → 零 `agent_start` invoke 且**零本地消息推入**（守卫先于推入，不产生孤儿提问） | 参数化迭代（2 形态） |
| `UseAgentSessionState.sendMessage` | 边界 | `prompt` 四形态：`''` / 超长（>1000 chars）/ 含 `\n\0 emoji` / `undefined`（越签名脏值）→ 本地 text 部件逐字保留不 trim；空串与脏值形态不抛错（守卫层不在 hook 侧） | 参数化迭代（4 形态） |
| `UseAgentSessionState.sendMessage` | 边界 | 连续两轮发送：pending 计数各自独立，第二轮归一为 `turn-<第二轮 turnId>-user` 且不与首轮串号；两轮提问各自一条 | 单次 |
| `useAgentSession` | 边界 | 同一轮提问事件重投（重订 / 收口竞态）→ 归一后仍**恰一条**气泡（id 替换幂等，不新增第二条） | 单次 |
| `useAgentSession` | 边界 | `root = null` 或 `sessionId = null` → 零取数零订阅、`messages` 为空（fold 零参面）、`loading === false`（既有回归，零改） | 参数化迭代（2 形态） |
| `useAgentSession` | 边界 | `events = []` + `turns = []`（空会话）→ `messages` 为 `[]`；随后注入一条 running 轮行仍不虚构消息（等重放） | 单次 |
| 镜像面回归（`use-agent-session.throttle.test.tsx`） | 正向（既有回归） | `useChat` 以 100ms 节流窗订阅 messages；该文件**无消息 id 断言**（proposal 表述偏保守），本变更**零改**该文件；镜像面 / seq 游标 / 无进展闸 / early-resolve 镜像 / sourceRef·cwd 参数面 / 漂移上报等既有用例全绿即镜像零语义变更的判据 | 参数化迭代（既有站点全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` + `Channel`） | `vi.hoisted()` `ChannelMock`（捕获 `onmessage` 供测试直投信封）+ `invokeMock` 按命令名 + 调用序可编程（`session_detail` / `agent_session_transcript` / `agent_session_watch` / `agent_start` / `agent_stop`，deferred 态供手动落定）；**进程边界**，MUST mock | 全章节（hook 即链路入口） |
| 不 mock：`../lib/agent-transport` | 真实 `TauriAgentTransport`——chunk 帧序与 hook 侧提问推入的时序关系是被测契约（`start` 与本地提问的先后） | 发起流 / 收口一致行 |
| 不 mock：`../lib/agent-adapter`（`foldSessionMessages`） | 真实折叠——收口 id 一致与旧转录回退只能用真实实现侧证；mock 折叠即让 AC-5 自证 | 重放装载 / 收口 / 旧转录行 |
| 不 mock：`@ai-sdk/react` 的 `useChat` / `setMessages` | 真实 `useChat`（`sendMessage({ id, parts })` 的 `pushMessage` 语义、`setMessages` 函数式 updater、`onFinish` 收口）——「本地副本结构上先于任何 chunk」依赖库真实行为 | 全部提问气泡与收口行 |
| 不 mock：`../lib/ipc-drift` | 真实 `resetDrift` / `driftCount` / `driftReports`（信封与结果面漂移上报，既有） | 脏信封 / 脏结果面回归行 |
| 不 mock：时钟 | 既有 `vi.useFakeTimers()` 仅用于「零轮询」类时限断言（保留原样）；本变更新增断言不引入计时 | 全章节 |

### `packages/desktop/src/components/agent/agent-messages.tsx` -> `packages/desktop/src/components/agent/agent-messages.test.tsx`

既有测试文件**改写过滤类用例 + 追加轮内状态行用例组**（proposal 处理列：604 行 / 21 用例）：`eventsToUIMessages` / `runRecordToUIMessage` 构造的夹具改 `foldSessionMessages(events, turns)` 按轮构造；「空部件消息过滤（AC-10）」组改写为「空内容轮按 `metadata.record.status` 判定可见性」；新增 4 条状态行用例 + 1 条「不叠加」用例。装置沿用 `FakeIntersectionObserver`（触顶驱动）+ `data-testid` 定位；`agent-blocks` / `tool-cards` 真实组合。

#### 待测功能

- `AgentMessages(props: AgentMessagesProps) => React.JSX.Element`（体内变化、入参三字段不变）: 可见性判据收为**单一来源**（`role === 'system'` 剔除 → 存在可见部件（`text` / `reasoning` / `tool-*`）→ 否则按 `metadata.record.status`：`failed` / `stopped` 保留、`completed` / 轮行缺席剔除）；分页 `remaining` 基于过滤后列表；assistant 且无可见部件时渲染轮内状态行（`failed` → 通用标题「Agent 运行失败」+ `<details>` 展开原文（`record.error` + 该消息 `api_error` 通知去重拼接）；`stopped` → 「已停止」；`completed` / `null` 不渲染）；`parts` 非空时**不叠加**状态行。

<!-- design.md「公共函数 / API」表对本文件只声明上面一行（入参不变）。私有件 hasVisibleParts / turnStatusOf / TurnStatusLine 的覆盖经该行承接；design 另明令 AgentMessagesProps 三字段与 `agent-session-view.tsx` 零 diff，故状态行只能在本文件内出现（D16）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentMessages` | 正向 | `parts = []` + `metadata.record.status = 'failed'`（`error` 携 provider 原文）→ 渲染 1 个 `data-testid="turn-status-line"`（`data-status="failed"`）+ `turn-status-line-failed` 标题「Agent 运行失败」+ `<details>` 内 `turn-status-line-raw` 含原文；无空白气泡叠加（AC-9 / 新增用例 ⑮） | 单次 |
| `AgentMessages` | 正向 | `parts = []` + `status = 'stopped'`（`error = null`）→ 渲染 `data-status="stopped"` 状态行 + `turn-status-line-stopped` 文案「已停止」；`turn-status-line-raw` 缺席（无原文不挂空 details）（AC-9 / 新增用例 ⑯） | 单次 |
| `AgentMessages` | 正向 | `parts` 非空 + `status = 'stopped'`（停止前已产出部分回答）→ 只呈正常气泡（思考 / 正文 / 工具卡按序），`turn-status-line` **缺席**（不叠加）（AC-9 / 新增用例 ⑱） | 单次 |
| `AgentMessages` | 正向 | `parts` 非空 + `status = 'failed'` → 只呈气泡，`turn-status-line` 缺席（轮失败事实仍可从 `metadata` 与顶部横幅读出） | 单次 |
| `AgentMessages` | 正向 | 原文来源四形态（参数化）：①仅 `record.error` 在场；②仅该消息内 `data-system-notice`（`subtype='api_error'`，`payload.error` 字符串）在场；③两者皆在且**去重后各出现一次**；④两者皆空 → 只显标题、`<details>` 缺席 | 参数化迭代（4 形态） |
| `AgentMessages` | 正向 | 一轮一条气泡（呈现侧）：同一轮 `foldSessionMessages` 产出的 assistant（text + reasoning + tool 三部件）→ 恰 **1** 个 `chat-bubble`（`data-role="assistant"`），部件按序呈现（AC-1） | 单次 |
| `AgentMessages` | 正向 | system 载体过滤：入参含 `data-run-started` / `data-system-notice` / `data-raw` / `data-run-result` 的独立 system 消息 → 整条不进渲染列表（既有用例，载体清单去掉 `run-record`）；空态判定基于过滤后列表 | 参数化迭代（4 载体） |
| `AgentMessages` | 正向 | `pause_turn` 两段同轮同消息：一轮 assistant 携 `tool-web_search` use 侧与 result 侧配对结果 → **同一气泡内**搜索卡呈查询词与来源列表（AC-10 的呈现侧），无第二条气泡 | 单次 |
| `AgentMessages` | 正向（既有回归） | 四变体气泡（user 文本 / assistant 文本 / reasoning 折叠 / tool 对卡）、citations 引用列表（去重、`title ?? url`、缺省不渲染容器）、AskUserQuestion 静态卡：既有断言在新夹具下全绿（AC-10 反向钉住面） | 参数化迭代（既有站点全集） |
| `AgentMessages` | 异常 | `metadata.record.status` 非法值（越过类型的未来枚举 / 脏串）→ 按「非 `failed` / 非 `stopped`」处理：空 parts 消息不渲染、不抛错 | 单次 |
| `AgentMessages` | 异常 | `metadata` 缺席 / `metadata.record` 为 `undefined`（实时流未收口）/ 为 `null` → 不抛错；空 parts 且无可见状态时不渲染 | 参数化迭代（3 形态） |
| `AgentMessages` | 异常 | `api_error` 通知的 `payload.error` 非字符串（对象 / 数字 / 数组）/ 空白串 / `payload` 非对象 → 跳过该来源（不渲染空 `<details>`、不拼出 `[object Object]`） | 参数化迭代（4 形态） |
| `AgentMessages` | 异常 | 可见性判据单一来源：同一「空 parts + failed」消息 → 通过过滤（计入 `remaining`）**且**状态行出现恰一次（不出现「过滤保留但渲染空」或「两处各渲染一次」的分裂） | 单次 |
| `AgentMessages` | 边界 | 只有 `data-*` 部件的 assistant（实时态 aux 载体）与只有 `step-start` 部件的 assistant → 均视为无可见部件：`record` 为 `completed` / `null` 时不渲染（不产生空壳气泡）、为 `failed` / `stopped` 时呈状态行 | 参数化迭代（2 部件形态 × 4 status） |
| `AgentMessages` | 边界 | `messages = []` → `conversation-empty` 空态（既有回归）；全为 system 载体 → 空态基于过滤后列表（既有） | 参数化迭代（2 形态） |
| `AgentMessages` | 边界 | 分页不计不可见空内容轮：11 条正常 + 5 条空 parts + `completed` → 一页 10 个气泡 + `history-load-hint` 在场；触顶后 11 条、提示行撤销（既有用例改写） | 单次 |
| `AgentMessages` | 边界 | 分页**计入**可见的失败 / 停止空内容轮：10 条正常 + 1 条空 parts + `failed` → 一页 10 条 + 提示行在场（与上一行的差异即「状态可读的轮参与计数」） | 单次 |
| `AgentMessages` | 边界 | 大列表：1000 条消息（含若干空内容轮）→ 默认渲染最新一页 10 条、`remaining` 正确、不抛错（`remaining` 基于过滤后列表） | 单次 |
| `AgentMessages` | 边界 | `loading` / `running` 呈现两态（既有回归）：`loading && 可见列表为空` → 「会话还原中…」；`running` → 运行中标记；非运行态无标记（零改） | 参数化迭代（3 形态） |
| 组合：`stopped` 轮与顶部横幅（入口 = `AgentSessionView` × 真实 hook × 真实组件；**执行文件 `packages/desktop/src/components/agent/agent-session-view.test.tsx`**） | 组合 | 会话终止于 `stopped`（轮行 `stopped`、转录零 assistant 密封）→ 顶部 `agent-session-failure` 横幅 **缺席**（`stopped` 不算失败，`agent-error.ts` 零 diff 的既有判据），同一时间线位置出现 `turn-status-line[data-status="stopped"]`「已停止」行（AC-9 / 新增用例 ⑲ / spec「停止轮空内容呈已停止行」） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `IntersectionObserver`（jsdom 缺失的**运行环境能力**） | 既有 `FakeIntersectionObserver`：只记录装配参数（`root` / `rootMargin`），触顶由 `trigger(true)` 显式驱动 | 分页 / `remaining` 行 |
| 不 mock：`./agent-blocks` / `./tool-cards` / `./agent-error` | 真实组合——搜索卡、工具对卡、引用列表与失败判据是被测呈现面的一部分；`agent-error.ts` 本变更零 diff，其真实行为必须在链路上 | 四变体气泡 / 搜索卡 / 组合行 |
| 不 mock：`../lib/agent-adapter`（`foldSessionMessages`） | 夹具由真实折叠产出（`foldSessionMessages(events, turns)`）——「一轮一条气泡」「空内容轮 record 就位」依赖真实产出形状；手写 UIMessage 夹具只用于纯边界形态（`record.status` 脏值 / 未来部件） | 全章节 |
| 不 mock：`@/lib/parse` | 真实 `asRecord` / `textOf` 宽解析（`api_error` 原文取值的边界判据） | `api_error` 四形态行 |
| 不 mock：被测组件自身 | `AgentMessages` 直渲染；轮内状态行是私有件，经公共 API 断言（不新增导出——AGENT.md 规则 5） | 全章节 |
| 进程边界（组合行专用） | `@tauri-apps/api/core` 替身（`invokeMock` + `ChannelMock`），执行文件为 `agent-session-view.test.tsx`（既有装置） | 组合行 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 0.6.19 → 0.6.20） | 配置 / 元数据，无运行期行为可断言；版本纪律的判据是「一次 patch、`src-tauri/Cargo.toml` crate 版本行不跟」 | AC-12：diff 审查（`package.json` 单行变更 + `src-tauri/Cargo.toml` 的 `version = "0.1.0"` 未动 + `tauri.conf.json` 的 `"version": "../package.json"` 引用行零改） |
| AC-3 / AC-6 的**帧纪律静态判据**（`grep -n "'start'" agent-transport.ts` 只命中 `ensureStart` 与 `finishRecord`；`'start-step'` 只命中 `eventChunks` 两分支；`'reset-step'` 只命中密封 assistant 前一处；`grep -rn "'start'\|'start-step'\|'reset-step'\|'finish'" agent-adapter.ts` **零命中**） | 源码位置事实，运行期不可观测（chunk 序列断言只覆盖被驱动的输入路径，覆盖不到「某分支从不发帧」这类**缺席**事实） | 桌面端静态检查相位执行上述 grep（tasks.md 阶段五自查逐条给定命令）；运行期侧证是本设计 transport 章节的「`start` 恰 1 次 / `start-step` 恰 2 次 / 密封前恒 `reset-step` / `eventToChunk` 零帧」四条断言 |
| AC-6 的**退役面零残留**（`provisionalMessageId` / `eventsToUIMessages` / `runRecordToUIMessage` / `data-run-record` / `'run-record'` / `delta-<` 在源码零命中）与 `knip` 零未用导出 | 静态检索与死代码扫描；写成测试会随路径布局漂移且与桌面端静态检查重复 | ① `knip` / `vp check` 门禁（AC-12）；② 运行期侧证：`agent-adapter.test.ts` 的**运行时导出键集合恰为三项**断言（退役三名缺席）+「产出中零 `data-run-record` 部件」断言；③ `grep` 判据由桌面端执行 |
| `AgentDataParts` 去掉 `'run-record'` 成员、`AgentMessageMetadata` 增 `record`、`EventPayload` 派生别名（`Extract` / `Omit`）、`DeltaPartAnchor` 类型契约 | **纯类型面 / 编译期事实**：类型收窄与派生守卫在运行期不存在，`tsc` 通过即断言成立；`DeltaPartAnchor` 的类型错值（返回 `string`、参数错位）只能编译期拒绝 | `vp check` 的 `tsc --noEmit`（AC-12）；运行期侧证是 `foldSessionMessages` 的 `metadata` 行（`record` 可读）、`eventToChunk` 第三参注入行（锚回调生效）与导出键集行 |
| **编译闭合批**（阶段一~四同批落地：删导出 / 改签名 / 改调用点） | 拆开即 `tsc` 报错，不存在可运行的中间态；中间态不可测是 design D19 的裁定 | 整批末的 `vp check` + 本设计四个测试文件全绿；tasks.md 阶段四末的源码级残留自查 |
| AC-11 的**零后端 diff**（`packages/desktop/src-tauri/**` 含 `crates/**` 与 `src/commands/**`）与 `cargo nextest run --workspace` 自然绿 | 「正确性主张」就是一行不变，运行期不可观测；前端单测无法断言 Rust 侧 | `git diff --stat -- packages/desktop/src-tauri` 为空 + `cargo nextest` 全绿（桌面端执行）；前端侧证是本变更零新增 IPC 命令 / 零信封字段（`src/lib/ipc-contract.test.ts` / `ipc.test.ts` 既有用例零改全绿） |
| AC-11 / AC-12 的**生成物零 diff**（`src/types/generated/{bindings.ts,bindings.zod.ts}`、`src/types/dto.ts`） | 「正确性主张」是零 diff；在测试里读产物会把用例绑死在路径布局上 | `git diff --stat` 为空 + `pnpm run bindings:export` 后再生成仍零 diff（AC-12，桌面端执行）；`src/types/dto.ts` 为 `export type *` shim，自动跟随 |
| AC-12 的**全量门禁与覆盖率阈值**（`vp check` / `knip` / `vp test` 的 lines 90 / branches 85 / functions 90） | 机械门禁步骤，由桌面端静态检查与 test-execution 相位执行；在测试设计里复述阈值等于复制运行脚本细节 | 门禁执行结果 + 本设计各章节用例对覆盖率的支撑（新增断言全部落在真实契约面：三态状态行、两起点归约一致、旧转录 fixture、双起点提问——不为覆盖率造句，R-10） |
| AC-13 的**边界零 diff**（`agent-error.ts` / `agent-blocks.tsx` / `agent-input.tsx` / `tool-cards.tsx` / `session-panel.tsx` / `agent-session-view.tsx` / `src/views/**` / `openspec/specs/**` / `openspec/config.json` / `explore.md`） | 「正确性主张」是这些文件一行不变；`AgentSessionView` 的零 diff 与「`stopped` 不触发横幅」是同一裁定的两面（D16 / Q4） | `git diff --stat` 逐项审查（tasks.md 阶段五自查逐字列出）；行为侧证是 AC-9 组合行（`agent-session-view.test.tsx` 里 `stopped` 轮零横幅 + 状态行在场）与既有 `agent-error.test.ts` 全绿（`latestAgentFailure` 判据零改） |
| **`step_index` 第二计数轴**（不得新增） | design D20 ② 明令：`step` 是纯前端概念，无信封字段、不落库；「不存在」的运行期断言只能退化为负向 grep | `grep -rn "stepIndex\|step_index" packages/desktop` 零命中（tasks.md 阶段五自查）+ `git diff --stat -- src-tauri` 为空 |
| **design 待决问题 8 的「订阅流就地续写中途态」**（旁观期间该轮此前由 fold 补出的部件短暂消失，收口恢复） | 与 reducer 的内部时序（`lastMessage: void 0` 起新消息 + 首个 `write` 命中「替换末条」）强耦合；断言它会把手写时序钉进契约，且 proposal 明令 MUST NOT 为消除该中途态另立第二套收口路径 | 终态一致性由 AC-5 的「收口前后 id 序列一致」与 AC-1 的两路对拍举证；中途态属已知接受项（design 登记的「已知、接受」），不作断言 |
| **轮内状态行的「按错误类型细分的友好标题」** | design D16 / Q4 裁定本期只取通用标题（`ERROR_RULES` 为 `agent-error.ts` 模块私有且该文件零 diff，复制规则必漂移）；细分标题在原样状态下**不存在**，写用例会虚构契约 | 通用标题 + 可展开原文已由 `agent-messages.tsx` 章节的两条正向行锁定（`turn-status-line-failed` 文案 + `turn-status-line-raw` 原文）；不做错误类型矩阵 |
| **其他套件的夹具机械适配**（`agent-render-memo.test.tsx` / `index.test.tsx` / `session-panel.test.tsx` / `agent-session-view.test.tsx` 的其余用例 / `src/views/**` 会话面板套件：`agent-debug-view` / `change-chat-panel` / `archi-chat-panel` / `explore-detail-pane` / `session-transcript-panel` / `use-change-chat` / `use-archi-chat`）的**新增覆盖** | proposal「测试文件」表口径：这些套件只改受影响断言（气泡数与 id 形状，即 `eventsToUIMessages` → `foldSessionMessages` 的夹具构造）与「一轮一条气泡」后的气泡计数；proposal 明令其**结构性断言零改动**（面板无自有转录查询 hook / 注入形态渲染）。在其上增补新契约用例会让测试范围越出 design 的四个被测文件 | 回归登记：这些套件全绿即「消费点接线零改动 + 注入形态未破」的判据；语义判据集中在四个被测文件的用例（`foldSessionMessages` 的两路对拍已覆盖 id 与部件形状） |
| `agent-adapter.ts` 的 `PairingScope.local` / `planBlock` / `plansToParts` / `plansToChunks` / `auxEventData` / `citationMetadata` 的**独立测试路径** | 它们是本文件私有件（design 明令「不列公共函数表」），既非导出也无独立测试文件；为其开导出以直测会违反 AGENT.md 规则 5 并污染 `knip` | 经 `foldSessionMessages` / `eventToChunk` 的双视野差异行、搜索两态行、计划层对拍行、`data-raw` 透传行覆盖（既有断言逐条保留）；IR 三层私有性由导出键集合断言守住 |
| `agent-transport.ts` 的 `ensureStart` / `finishRecord` / `eventChunks` / `partAnchors` / `currentToolCalls` 的**独立测试路径** | 模块私有件（design 明令「私有，不列公共函数表」），且是 `sendMessages` / `reconnectToStream` 的体内实现；直测需开导出或直构实例私有状态 | 经两出口的 chunk 序列断言全覆盖（`start` 恰一次 / `start-step` 计数 / `reset-step` 位置 / `finish` 携 metadata / part id 派生 / 工具就地补全 / 帧在订阅起点补齐） |
| `agent-messages.tsx` 的 `hasVisibleParts` / `turnStatusOf` / `TurnStatusLine` 的**独立测试路径** | 同上（design D15 / D16 明令私有件）；状态行是 `ChatBubbleBase` 的分支产物，直测须新增导出 | 经 `AgentMessages` 的四条状态行正向行 + 「不叠加」行 + 判据单一来源行覆盖（`data-testid` 挂钩即为其可定位面） |
| **`ai` v7 reducer 的两条外部假设本身**（`reset-step` 只删「最后一个 `step-start` 之后」的部件；`finish` 的 `messageMetadata` 为深合并非覆盖） | 这是依赖库内部实现，不是本仓代码；为其写断言等于把第三方内部行为钉成自身契约 | 以**真实 reducer 归约**做行为侧证（transport 章节「前段密封块存活」行 + 「`finish` 的 `seq` 显式回填不被覆盖」行）；若库行为变化，这两行会红，即为告警面（R-1 的缓解措施落地形态） |
| `use-agent-session.ts` 的 `buildMessages`（**已删除**） | 删除面的正确性主张是「函数与其调用点零残留」；运行期已无可调用的对象 | `grep -rn "buildMessages" packages/desktop/src` 零命中（桌面端静态检查）+ 三处收口的运行期侧证（重放装载 / 重放失败回落 / `onFinish` 收口三行的 fold 形状断言） |
