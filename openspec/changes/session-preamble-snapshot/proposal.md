# 提案: session-preamble-snapshot

> **变更**: session-preamble-snapshot
> **日期**: 2026-11-23
> **状态**: draft

---

## 问题

agent 会话的系统提示词今天**每轮现算**（`infra/agent/src/sdk/loop.rs` 每轮调 `preamble::load(&turn.cwd)` 重读 `<cwd>/AGENT.md`，并与 `SessionInjections.preamble`（archi / config 知识）合并成一条 System 消息）。这条语义被写进 spec（`desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」：*注入 SHALL 每轮重读，不做起播快照*），其代价在引入前缀缓存后才显形：

1. **会话内任一 `AGENT.md` 改动都会击穿整段缓存前缀**。前缀缓存（anthropic top-level `cache_control` / openai 隐式）只在「前缀逐字相同」时命中；system 消息排在消息序列最前，文件中间被改一个字，本轮之后**全部历史**（tools + 全部轮）都得重新计费。仓库自身就是活样本：`AGENT.md` 随 change 提交频繁改动，而 change 流程的 evaluator→executor 打回是同会话续跑（`walker.rs` 的 `continue_session`）。
2. **知识 preamble 随二进制漂移**。archi / config 知识是 `include_str!` 骨架 + 代码派生词表；应用升级后同一会话续跑，知识文本会变，同样击穿前缀——而这部分变化对「已经跑了一半的会话」没有任何增益。
3. **恢复（Continue）时 100% 现算**。System 消息不进转录、不进库（`resume::rebuild` 只重建密封 Message 事件），恢复后的 prefix 是否复用完全取决于「现算结果与上一轮逐字相同」这个巧合。
4. **系统提示词对前端完全不可见**。`SessionSummary` 只有 id / 统计 / 轮行，用户与调试者都无法回答「这段会话到底被塞了什么系统提示词、是哪一版 `AGENT.md`」。

需求原文三段（存储 / 读回 / 返回前端）恰好对应「一个写点、一个读点 + 语义反转、一个出口」。语义反转与既有 spec 条款正面冲突（不是加字段就能收的事），故本变更必然修订该条款。

---

## 提案

一句话：**会话首轮把「装配知识 + `AGENT.md` + 冻结指引常量」拍成一份快照落库，此后本会话每轮都用这一份逐字不变的系统提示词；规则要更新由人对 agent 说一句「重新读 `AGENT.md`」，走 `read` 工具落在史尾（前缀零 miss）；快照元信息与全文随 `session_detail` 返回前端并只读可见。**

四项拍板（探索轮决策均在案，见「过程 · 决策」）：

1. **快照边界 = 知识 + `AGENT.md` 合并体 + 装配层指引常量**（否「只冻知识」——`AGENT.md` 是主敌手；否「尾部注入 / 显式重载写面」——见决策表）。
2. **写点单点在组合根首轮**（`ComposedTurn::begin`）：`New` 或 `Continue` 且 `is_unstarted_session` → 现算 + 落库 + 本轮用内存体；`Continue` 已开轮 → **读库覆盖注入位**，不读文件、不算知识。存量会话（已开轮无快照）在其下一次 `Continue` 一次性补拍。落库为**写一次**语义（已有行即不改写，快照不可变）。
3. **存储位 = 新表 `SessionPreambleRecord`**（workspace 维度，`native_model` id 取现役最大值 + 1 = 19，version 1）：无行 = 未快照，`SessionRecord` 与既有行零改动、零版本链；清单查询的全表扫描不因 37KB 载荷而膨胀。
4. **出口 = `session_detail` 独携**：`SessionSummary.preamble { savedAt, content }`，**清单路径恒 `null` 且不读快照表**；前端在会话视图元信息行给只读入口（`StandardDrawer` 展示拍存时刻 / 体量 / 全文 + 一行静态指引），零新命令、零新 hook、零写面、零订阅。

`AGENT.md` 的读取口径（根 = 会话执行根、单文件、32KB char 边界截断、无 `CLAUDE.md` / `AGENTS.md` 兜底）**逐条不变**，只是读取时机从「每轮」改为「首轮一次」。

**生效方式的闭环**：快照尾部追加一段装配层常量指引（「本会话系统提示词冻结于会话开始；规则真身为 `AGENT.md`，用户提示更新时用 `read` 重读并按新规则执行」）。它不违反「前端 / 自由字符串 MUST NOT 供给 preamble」（装配层静态常量），却让「人说一句 → agent 去读」从约定变成模型可发现的机制；`read` 结果落在史尾，已缓存前缀逐字不变（零 miss），这正是「零 miss 的生效方式」。

**明确不做**（探索轮已裁决放弃）：UI 重载按钮 / `session_preamble_reload` 写面（P2）、把 `AGENT.md` 挪成尾部 user 消息（P4）、把 `AGENT.md` 纳入写保护（P6）、只冻知识（P5）、切 1h 缓存 TTL、`AGENTS.md` 复数读取源、会话统计扩缓存 token 字段（观测走 `TurnDone.usage` 落库，db 检视 / `db_query` 可查）。

---

## 能力

### 新增能力

- 无（全部为既有能力的增量与修订）

### 修改的能力

- **desktop-agent-execution** — 反转「每轮重读」为「首轮快照 + 续轮读库」（MODIFIED）；新增快照拍存 / 读回 / 补拍 / 写一次 / 知识版本语义（ADDED）；`session_detail` 携快照面、清单路径恒 `null`（MODIFIED）；系统消息合成从引擎二元函数收敛为注入位一元消费
- **desktop-workspace-store** — 新增 workspace 维度模型 `SessionPreambleRecord`（id 19）与其写一次 / 直查操作面、信封登记与自动说明派生（ADDED）
- **desktop-agent-chat-infra** — `AgentSessionView` 元信息行的快照只读入口与抽屉（ADDED）
- **desktop-agent-db-tools** — `db_tables` 表清单计数与名单随新表更新（workspace 维度 11 → 12、合计 15 → 16）（MODIFIED）
- **desktop-change-chat** — 「每轮 `AGENT.md` 读取根」措辞同步为首轮读取（MODIFIED，纯措辞）

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — 新模型 `SessionPreambleRecord`：`#[native_model(id = 19, version = 1)]`（现役最大值 + 1 而非补空洞，沿 `NotificationRecord` 注释先例）+ `#[native_db]`；主键 `session_id`；`content: String`（空串 = 快照成立但无正文）/ `saved_at: i64`（其余元信息不落库——体量按正文长度导出）；表与字段说明走 `///` doc（describe 自动派生）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — `workspace_models()` 注册（doc 计数同步）；`stamp_session_preamble(session_id, content) -> Result<bool, StoreError>`（**写一次**：行已存在返回 `Ok(false)` 不改写；`saved_at` 由 store 侧时钟取，沿既有「时间戳归落库侧」口径）；`find_session_preamble(session_id)`（主键直查）；`find_session_summary` 装配时读快照并填 DTO（清单路径不读）
- `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` — `MODEL_ENTRIES` 增 `session_preamble` 一行（count / scan / key_of / schema / walk 五件，与既有行同型）
- `packages/desktop/src-tauri/crates/core/agent/src/session.rs` — 新 DTO `SessionPreambleInfo { saved_at, content }`（`specta::Type`，出 IPC）；`SessionSummary` 增 `preamble: Option<SessionPreambleInfo>`
- `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` — `SessionInjections.preamble` 的 doc 语义改写为「会话系统提示词快照正文」（**零结构改动**，字段面与词汇不变）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs` — 保持 `load`（读取 + 32KB 截断 + `<agent_md>` 包装，签名与口径零改动）+ 新增 `freeze`（合成单点：知识段 + 空行 + `AGENT.md` 段 + 空行 + 指引常量；空段跳过；全空 → 空串）+ `GUIDANCE` 常量（≤ 512 B、零路径字面量、零占位符）；零新增第三方依赖
- `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` — `ComposedTurn::begin` 增首轮/续轮两分支：首轮（`New` / `is_unstarted`）读 `ctx.workspace_root` 的 `AGENT.md`、合成快照、`kernel.begin_turn` 之后经 `self.query.store()` 落库（沿 `restamp_session_config` 组合根直写先例，**不扩 `SessionSink`**）、落库失败仅日志不中断；续轮读库并覆盖 `injections.preamble`（空正文覆盖为 `None`）；存量补拍同首轮口径
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` — 系统消息一元化：删 `preamble::load(&turn.cwd)` 调用与二元 `system_prompt(knowledge, agent_md)` 形参，改为只读 `turn.injections.preamble`；`AGENT.md` / `preamble::load` 字样零残留（机械可查）
- `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` — 经 `bindings:export` 再生成（`SessionPreambleInfo` 与 `SessionSummary.preamble`）
- `packages/desktop/src/hooks/use-agent-session.ts` — `sessionFromTurn` 的 `SessionSummary` 字面补 `preamble: null`（本地轮行镜像无快照面；重放路径 `applyReplay` 原样落 `session_detail` 的 `preamble`，零其他改动）
- `packages/desktop/src/components/agent/agent-session-view.tsx` — `SessionMeta` 增「系统提示词」入口（`preamble !== null` 才渲染，`data-testid="session-meta-preamble"`）+ 只读抽屉（复用 `StandardDrawer`：拍存时刻 / 体量（按正文字节数就地算）/ 静态指引文案 / 全文，等宽可滚动）；零取数、零新 hook、零新命令
- `.dev-team/architecture/models/Desktop/Components/Agent/Desktop.Components.Agent.AgentSessionView.md` — 职能与不变量增一行（快照只读展示位），公共面表零变化（无新导出）

### 测试文件

- `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` — 新模型注册 / 主键 / 字段面 / 无版本链（与 `SessionRecord` 各自独立）
- `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` — 写一次（二次 stamp 不改写、返回未写入）/ 主键读回 / `find_session_summary` 携 `preamble`（`savedAt` 与 `content`）/ 清单路径 `preamble == None` 且零读快照表
- `packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs` — workspace 维度 11 → 12 行、名字唯一集 15 → 16、`session_preamble` 的 scan / schema / walk 直通
- `packages/desktop/src-tauri/crates/core/agent/src/session_test.rs` — `SessionSummary` 新字段的派生与序列化（`preamble: null` 不省略）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble_test.rs` — 合成次序（知识在前 / `AGENT.md` 段在后 / 空行分隔 / 常量末段）、单侧在场、全空 → 空串、32KB 截断只作用于 `AGENT.md` 段、常量体量与零占位符
- `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs` — 首轮落库（正文逐字）/ 续轮读库覆盖注入位（改文件不影响）/ 存量补拍一次性 / 落库失败不中断 / 空快照后续轮次零 System
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs` — 两轮请求的 System 消息**逐字相等**（快照成立的结构证明）；`preamble = None` → 零 System 消息
- `packages/desktop/src-tauri/src/bindings/mod_test.rs` — 生成类型清单与 `session_detail` 返回形状（含 `SessionPreambleInfo`）
- 前端夹具连锁（`SessionSummary` 为 zod `strictObject`）：`src/components/agent/agent-session-view.test.tsx`、`src/components/agent/session-panel.test.tsx`、`src/components/agent/index.test.tsx`、`src/hooks/use-agent-session.test.ts`、`src/lib/agent-transport.test.ts`、`src/views/agent/agent-debug-view.test.tsx`、`src/views/architecture/agent/archi-chat-panel.test.tsx`、`src/views/changes/agent/change-chat-panel.test.tsx` — 补 `preamble: null`（另：会话视图新增入口 / 抽屉的组件测试）

### 删除文件

- 无（`loop.rs` 的 `system_prompt` 二元形参就地收敛，文件不删）

### 不要修改

- `packages/desktop/src-tauri/crates/core/agent/src/{port.rs,event.rs,kernel.rs}` — 端口契约、事件词汇、内核治理零结构改动（`SessionQuery` 方法面不变，`find_session_detail` 已返回 `SessionSummary`）
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` 与 `commands/mod.rs` — 命令签名与 `all_commands!` 零变更（出口只扩 DTO，不新增命令）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/{context.rs,compact.rs}` — L2/L3 防线零改动（剪裁时刻的前缀重建是已承认代价，不在本变更治理）
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` — 自动前缀缓存装配（`with_automatic_caching`）与 TTL 零改动
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/{tools.rs,policy.rs,sandbox.rs,protect/**}` — 工具面 / 权限 / 沙箱 / 写保护判定零改动（`AGENT.md` 不纳入内置写保护）
- `packages/desktop/src-tauri/crates/infra/agent/src/{archi_knowledge.rs,config_knowledge.rs}` 与两份骨架 md — 知识生产者零改动（本变更只改「何时读回」，不改「生产什么」）
- `packages/desktop/src/components/agent/session-panel.tsx` — 面板骨架零改动（入口落在 `AgentSessionView` 元信息行，`SessionPanelState.note` 优先级不受影响）
- `packages/desktop/src/hooks/use-file-watch-signal.ts` 与 `watch` 命令面 — 变更提示（watch chip / `session_preamble_status`）显式不在本期
- `AGENT.md` 本仓根文件 — 文档面随常规提交演进，本变更不为其新增读取源 / 兜底规则（`AGENTS.md` 复数形态不支持）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 首轮拍存 | `New` 会话与 `is_unstarted_session` 的 `Continue` 首轮：快照正文落 `session_preamble` 行（正文 = 知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量，空段跳过）；`compose_test` 覆盖正文逐字 |
| AC-2 | 续轮读库 | 已开轮会话 `Continue`：注入位取库中快照正文，**不读文件、不算知识**；`loop.rs` 内 `preamble::load` / `AGENT.md` 字样零命中；两轮续跑的 System 消息逐字相等（`loop_test` 断言） |
| AC-3 | 中途改文件不改前缀 | 会话中改写 `AGENT.md` 后续发下一轮：该轮 System 消息与上一轮逐字相同（`compose_test` / `loop_test` 断言） |
| AC-4 | 存量补拍 | 已开轮且无快照行的会话 `Continue`：按首轮口径现算 + 落库 + 使用（一次性），此后冻结；`compose_test` 覆盖 |
| AC-5 | 写一次冻结 | 快照行已存在时二次 `stamp_session_preamble` 返回未写入、库中内容不变（`store_test` + `compose_test`）；无任何重载 / 刷新写面（命令面零新增） |
| AC-6 | 存储面 | `SessionPreambleRecord` 注册进 workspace 模型组与信封注册表；`list_models` / `scan` / `describe` / `describe_table` 自动覆盖（零模型特定代码）；`SessionRecord` 与既有行零改动、零版本链、零迁移 |
| AC-7 | 空快照与缺席 | 无 `AGENT.md` 且无知识的会话：快照正文为空串、零 System 消息（不追加常量）；之后文件出现不改本会话（仍零 System）；`AGENT.md` 缺席而知识在场 → 单条知识 + 常量 |
| AC-8 | 读取口径不变 | 读取根 = `SessionCtx.workspace_root`（worktree 即分支版）；严格单文件（`CLAUDE.md` / `AGENTS.md` 兜底回退零命中）；32KB char 边界截断只作用于 `AGENT.md` 段（`truncate` 单点、零复制） |
| AC-9 | 出口与载荷纪律 | `session_detail` 返回 `preamble { savedAt, content }`；`agent_sessions` / `list_change_chat_sessions` / `list_archi_chat_sessions` / `list_config_chat_sessions` 四项清单的 `preamble` 恒 `null` 且不读快照表（结构断言）；bindings / zod 再生成零手改 |
| AC-10 | 前端只读可见面 | 会话视图元信息行在有快照时渲染「系统提示词」入口，点开抽屉展示拍存时刻 / 体量 / 静态指引 / 全文；`preamble` 缺席不渲染入口；零取数（内容取自 `SessionSummary.preamble`）、零新命令、零订阅、MUST NOT 把正文插入对话流；组件测试覆盖 |
| AC-11 | 语料与文档一致 | `db_tables` 计数与名单更新（workspace 12 / 合计 16）；`desktop-change-chat` 措辞同步；`desktop-agent-execution` Module Contract 的 `sdk/preamble.rs` 行改为快照口径；`AgentSessionView` 契约资产 prose 同步 |
| AC-12 | 全管线回归 | `pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `vp test`、`cargo test --workspace` 全绿；`api:check` 零 diff（无新公共 API 面逃逸） |
| AC-13 | 版本交付（归档时执行） | `packages/desktop/package.json` `version` 0.7.7 → 0.7.8；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **语义反转**：agent 不能再通过改 `AGENT.md` 影响本会话系统提示词（spec 原论证前提消失：bash 引入后 agent 可改自己的提示词） | 既有隐性行为被改写；用户/agent 可能误以为「改了文件没生效是 bug」 | 高（必然发生，是设计意图） | 快照尾部常量指引写明「冻结 + 真身在 `AGENT.md` + 用 `read` 重读」；前端可见面（拍存时刻 + 全文）让「冻结的是什么」可查；spec 明写「新会话生效」与存量补拍语义 |
| 前缀并非绝对恒定：L2 剪裁（`context::prune`）/ L3 compaction 会改写历史，剪裁时刻前缀必然重建 | 「保障持续命中」被过度承诺 | 中 | spec 显式承认边界（剪裁时刻前缀重建不在承诺内）；剪裁水位与缓存纪律挂钩的条款不新增承诺 |
| 缓存 TTL：change 流程 evaluator→executor 打回常超 5min 默认 TTL，隔夜恢复必 miss | 「恢复也命中」只在密集续跑时兑现 | 中 | 本期不切 1h TTL（成本/计费差异另立决策）；spec 把「隔夜恢复」写在边界外；前缀稳定仍带来「同进程续跑零 miss」的确定收益 |
| 存量会话首次 `Continue` 补拍 → 一次性前缀 miss | 老会话切换时一次额外计费 | 高（一次性） | 写入 spec 明确「一次性」；补拍后豁免（写一次） |
| 应用升级后旧会话沿用旧知识（archi / config 骨架与派生词表不更新） | 旧会话可能拿到过期词表（如新增 config 键） | 中 | 「前缀稳定优先」写进 spec；新会话自然取新知识；知识是尽力增强项且 `archi_api` / `propose_config` 等工具仍在场 |
| `AGENT.md` 32KB 截断使「system 里是前缀、`read` 出来是全文」两条通道口径不同（既有缺口，快照化后更显形；32 KB–50 KB 区间最明显） | 大文件仓库里 agent 按 `read` 得到比 system 更长的规则 | 低（本仓约 6.5 KB） | 口径逐条不变（不扩大改动面）；前端可见面呈现拍存时刻与全文，用户可自行对账；`AGENT.md` 上限由使用者约束 |
| 快照落库失败（低概率）→ 该轮用内存体，下轮回落现算（可能换前缀） | 一次 miss，无数据损坏 | 低 | 尽力语义 + 日志留痕（沿 `bind_remote_session` 先例），MUST NOT 中断本轮 |
| DTO 加字段触发前端 zod `strictObject` 夹具连锁（约 8 个测试文件补 `preamble: null`） | 一次性机械改动，漏改即测试红 | 高（必然发生） | AC-9 / AC-12 机械兜底；只扩 `SessionSummary` 一处，不动 `SessionRow` / `SessionStats`（避免二次连锁） |
| 存储体积：每会话至多约 37KB（32KB `AGENT.md` + 知识 + 常量）一行，无清理策略 | workspace 库随时间线性增长（数百会话 < 15MB） | 低 | 会话本身亦无清理（既有口径）；新表独立使清单扫描不读该载荷；若需清理，另立「会话保留策略」变更 |
| 新表 id 19 与未来模型 id 冲突 | 需再取更大 id | 低（id 单调递增约定） | 沿 `NotificationRecord` 注释约定「现役最大值 + 1，不补空洞」 |
| 冻结使首轮 `AGENT.md` 缺失的会话永久无该段（之后文件出现也不补） | 用户可能期望「补上」 | 中 | 写进 spec（冻结即冻结，新会话才有）；常量指引提示真身在文件、可用 `read` 取最新 |
| **无「规则已变更」提示**：用户改完 `AGENT.md` 后旧会话毫无反馈（本期不做 watch chip / `session_preamble_status` 命令 / 快照 sha 列） | 用户困惑「为什么 agent 还用旧规则」 | 中 | 抽屉静态指引文案写明「改 `AGENT.md` 后直接要求 agent 重新读取」；快照正文自带常量指引；后续落地检测面时可加 sha 列（native_model 版本链演进，成本已在案） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|------|
| Q1: 快照边界——只冻知识，还是知识 + `AGENT.md`？ | 知识 + `AGENT.md` + 指引常量 | `AGENT.md` 是前缀稳定性的主敌手（每轮重读、仓库内随 change 提交变动）；只冻知识治不住 | 只冻知识（P5：收益减半）；尾部注入 `AGENT.md`（P4：契约破坏最大、token 双份） |
| Q2: `AGENT.md` 改动后如何生效？ | 人说一句「重新读 `AGENT.md`」→ agent 用 `read` 落史尾 | 零新写面、零 miss（新增内容只在未缓存尾部）；`read` 的截断面（>100 KB 才收窄为前 100 行预览、单结果 50 KB 封顶）对本仓约 6.5 KB / 42 行的 `AGENT.md` 绰绰有余 | 显式重载命令（P2：改写 system = 整段前缀 miss + 新写面 / 权限面）；watch chip 一键填入话术（降级为后续）；常驻 note（与既有 note 优先级冲突） |
| Q3: 快照存哪？ | 新表 `SessionPreambleRecord`（id 19） | 无版本链、`SessionRecord` 零改动；**清单查询全表扫 `SessionRecord` 不会被 37KB 载荷膨胀**；老会话天然无行 = 未快照 | `SessionRecord` v3 加列（需 `SessionRecordV2` 中间形态 + 升级链断言，且清单读面膨胀）；塞进密封转录（污染 core 事件契约） |
| Q4: 写点接缝——扩 `SessionSink` 还是组合根直写 store？ | 组合根直写 `Store`（沿 `restamp_session_config` 同处同型先例） | `SessionSink` 扩面要动 core 端口契约；首轮 stamp 在 `begin_turn` 返回后才知会话 id，组合根已在手 store 句柄 | 扩 `SessionSink::stamp_session_preamble`（core 词汇 +1，得在 kernel 内写） |
| Q5: 存量会话（已开轮无快照）怎么办？ | 下一次 `Continue` 一次性补拍 | 一次 miss 换永久稳定；「永久现算」会让老会话永远落在不稳定态 | 永久现算（老会话永不快照） |
| Q6: 前端出口形状？ | 仅 `session_detail` 携 `preamble`（meta + 全文），**清单路径恒 `null`** | 列表最坏 500×37KB≈18MB 与「详情每开一次 37KB」都不可接受；清单本无消费点 | `SessionRow` 加 `preambleMeta`（清单/详情都带，且需逐会话读快照表）＋独立 `session_preamble(root,id)` 命令（多一条 IPC 面） |
| Q7: 前端展示落点？ | `AgentSessionView` 元信息行入口 + `StandardDrawer` 只读抽屉 | 一处实现覆盖全部五个消费面；数据随 `session_detail` 已到手（零新 hook / 零新命令 / 零订阅）；`SessionPanel` 骨架与 `note` 优先级零触碰 | `SessionPanel` 头区动作（要动全局面板契约与三处 hook）；常驻 note 文案（须与既有 `DRAFT_NOTE` / `REFRESH_NOTE` 争同一槽位） |
| Q8: `AGENT.md` 是否纳入内置写保护？ | 不纳入 | 生效路径已改为 `read`，「agent 改自己提示词」不再是问题前提；纳入要改 `desktop-write-protection` 判定矩阵，收益与改动面不成比例 | P6（内置锁定清单 +1） |
| Q9: 缓存 token 统计是否进 `SessionStats`？ | 本期不做 | `TurnDone.usage` 已含 `cached_input_tokens` 且已落库（db 检视 / `db_query` 可查）；扩 DTO 会再带一轮 core + zod 夹具连锁 | 扩 `cachedInputTokens` / `cacheCreationInputTokens`（作为独立小改另立） |
| Q10: 是否切 anthropic 1h 缓存 TTL？ | 不切（保持 `with_automatic_caching` 默认） | 承诺边界先收敛：「同进程密集续跑零 miss」已可兑现；1h 有写入溢价与成本差异，需独立裁决 | `with_automatic_caching_1h`（隔夜恢复才可能命中） |
| Q11: 快照是否落 sha / 体量等元信息？ | 只落 `saved_at`；sha 与 bytes 不落库不出线 | 三个需求句都用不上：体量按正文字节数就地算，「文件是否已变更」的检测面本期不做（无 watch chip / status 命令），落一个无消费方的字段属投机件；未来要用时按 native_model 版本链加列（成本已量化在案） | 全量 meta（`savedAt` / `bytes` / `sha` / `agentMdSha`，为后续变更检测预埋） |

### 待决问题

- 快照合成（`freeze` 与 `GUIDANCE`）的落点与命名（`sdk/preamble.rs` 内扩位，还是拆新文件）——design 定稿；约束：`truncate` / `load` 口径单点复用，MUST NOT 复制一份截断或读取逻辑
- 指引常量的文案与语言定稿（≤ 512 B、零路径字面量、零占位符；须表达「冻结 / 真身 `AGENT.md` / 用 `read` 重读」三点）——design 定稿
- `SessionPreambleInfo`（`{ savedAt, content }`）的字段名与是否随 `SessionSummary` 直接内联（当前拍板：独立具名 struct，`specta::Type` 出 IPC）——design 定稿
- 组合根 `begin` 内 stamp 的错误处置形态（日志文案与是否上报 `SystemNotice`）——design 定稿（拍板：MUST NOT 中断本轮）
- 前端抽屉的具体排版与静态指引文案（沿 `StandardDrawer` 还是 `StandardDailog`）——design / 实现期定稿
- `desktop-archi-agent` / `desktop-config-agent` 中引用本仓 `AGENT.md` 的措辞是否需同步（当前判为**不需**：两处只引用「合并规则归 desktop-agent-execution」且头部名保留）——design 复核
