# agent 会话 preamble 快照：从「每轮重算」到「一段会话一条前缀」+ 前端可见

workflow_type: requirement

需求原话（三段，逐个拆）：

1. **agent 会话开始时，存储 preamble**
2. **多轮会话和 session 恢复时读取记录拼接 preamble，以保障一段会话可以持续命中缓存**
3. **将 preamble 随前端接口返回**

三段其实指向三个不同的落点：① 一个**写点**（会话首轮落库）、② 一个**读点 + 语义反转**（把「每轮现算」改成「读库复用」）、③ 一个**出口**（IPC DTO 扩面）。下面先把现状摸清，再铺三条线的岔口。

---

## 1. 现状：preamble 从哪来、到哪去

### 1.1 生产者与装配（每轮一次，不是会话一次）

```
命令层 agent_start（src/commands/exec/mod.rs:96）
  └─ compose_turn（infra/agent/src/compose.rs:~290）         ← 每轮发起都跑一遍
       └─ ComposedTurn::begin（compose.rs:~360）
            ├─ assemble_injections(&provenance)              ← ★ preamble 现算点
            │     ├─ source=archi   → archi_knowledge::knowledge()   （include_str! 骨架 2143B + 代码派生词表）
            │     ├─ source=config  → config_knowledge::knowledge()  （骨架 4563B + 派生段）
            │     ├─ source=explore/change/debug → preamble = None
            │     └─ 产物 SessionInjections（五键 preamble/tools/finish/ask/protect）
            └─ kernel.begin_turn(TurnRequest{ injections, … })  （core/agent/src/kernel.rs:157）
                 └─ runner.open_session → sdk 泵
```

### 1.2 消费点：loop 每轮**重算 system 消息**

```
sdk/loop.rs run()（L296 起）
  每轮 continuation 循环里：
    system_prompt(turn.injections.preamble, preamble::load(&turn.cwd).await)   ← L319-325
      └─ 会话 preamble（知识，前） + AGENT.md 逐字（后） → 单空行拼接 → 恰一条 Message::System
  chat_history = [System] + history[...]
  → CompletionRequest → model.stream(request)
```

关键点：`preamble::load`（`sdk/preamble.rs:17`）**每轮重读 `<cwd>/AGENT.md`**，32KB char 边界截断。spec 把它写成了明条（`openspec/specs/desktop-agent-execution/spec.md:331`）：

> 注入 SHALL **每轮重读**：会话中途修改 `AGENT.md`，改动自下一轮请求生效（**不做起播快照**——bash 引入后 agent 可改自己的系统提示词，该语义行为最简且可预期）。

也就是说：**需求 ② 的语义反转与这条 spec 条款正面冲突**，不是加个字段就能收的事。

### 1.3 恢复（Continue）时发生什么

```
Continue{id} → compose.begin → find_session(id)（校验引擎同类）
            → prior_handle = engine_session_id（sdk 引擎不消费）
            → kernel.begin_turn → SdkRunner::open_session → resolve_resume（runner.rs:198）
                 └─ StoreQuery::transcript(session_id) → resume::rebuild(events) → rig 史
            → context::prune（L2 先行剪裁）→ session_pump → loop
```

`resume::rebuild`（`sdk/resume.rs:36`）只从**密封 Message 事件**重建，跳过 RunStarted / SystemNotice / Raw ——**System 消息不在转录里，也不在库里**。所以恢复后 system 消息 100% 是当轮现算结果，历史前缀能否复用完全靠「现算结果与上一轮逐字相同」这个巧合。

---

## 2. 缓存的地基事实（决定需求②能做到什么程度）

### 2.1 机制

| 协议 | 缓存怎么开 | 命中条件 |
|---|---|---|
| anthropic | `with_automatic_caching()`（`sdk/runner.rs:362`）→ 请求体 top-level `cache_control`，rig 在 **tools / system / 消息尾**放断点 | 前缀逐字相同（system 在 message 序列最前） |
| openai | 不追加任何参数（spec 明条） | 服务端隐式前缀缓存，同样要求前缀逐字稳定 |

rig 0.43 的断点放置：`apply_prompt_cache_control`（rig-core `providers/anthropic/completion.rs:1877`）——`apply_tool_cache_control` → `apply_system_cache_control` → `apply_message_cache_control`（尾部）。断点预算 4 个。

### 2.2 缓存账本已经在线（可观测，但没被消费）

rig `Usage`（`rig-core/completion/request.rs:404`）字段：`input_tokens` / `output_tokens` / `total_tokens` / **`cached_input_tokens`** / **`cache_creation_input_tokens`** / `tool_use_prompt_tokens` / `reasoning_tokens`。

`loop.rs:1202 finish()` 把 `usage` 原值塞进 `TurnDone.usage`（鸭子型 JSON）；`sum_usage_tokens`（`core/agent/src/usage.rs:9`）现在只求和 `input_tokens` / `output_tokens`（`SessionStats` 也只有这两个）。**缓存命中率今天在前端完全不可见** —— 需求②若要「保障」，缺一个验证面。

### 2.3 时间尺度（对「session 恢复也命中」的硬约束）

```
会话内连续多轮（间隔 < 5min）        ──▶ 前缀稳定即可命中（现状大多已命中）
应用重启 / 隔夜恢复（> 5min）         ──▶ 缓存条目已过期，命中必 miss
                                      （除非改 with_automatic_caching_1h，1h TTL，且恢复落在窗内）
```

这条决定了需求②的**真实收益边界**：`snapshot 化`对「进程内多轮」是保护（防 AGENT.md 被中途改坏前缀），对「隔夜恢复」只在换成 1h TTL 之后才有意义。值得先跟用户对齐「恢复」指的是哪一种。

---

## 3. 现状里谁在破坏前缀稳定（缓存敌手清单）

| # | 破坏因素 | 现状行为 | 快照化能否治 |
|---|---|---|---|
| A | `AGENT.md` 每轮重读 | 会话中文件一动 → 下一轮 system 变 → 全前缀 miss | ✅ 快照即治（但反转 spec:331 语义） |
| B | 知识 preamble 随代码版本变 | `include_str!` 骨架 + 代码派生词表；应用升级后同一会话续跑会变 | ✅ 快照即治 |
| C | 装配面按 `source_ref` 分组 | 不同来源/相位 tool 清单不同（tools 也在缓存前缀里） | ❌ 不该治（跨相位本就该换前缀） |
| D | effort 换档 | spec:1571 已定「档位只在轮边界确定」，同一会话内换档属未支持能力 | 已由结构约束 |
| E | `context::prune` L2 剪裁（`sdk/context.rs:132`） | 超大上下文丢最老整轮对 → 前缀中段改写 → miss | ❌ 不能治，需在方案里显式承认（剪裁时刻前缀必然重建） |
| F | 模型/端点切换 | Continue 只校验引擎同类，不校验 provider/model | 现状外，不在本次范围 |

**结论**：①②真正的抓手是 A + B，且 A 是主矛盾。

---

## 4. 「存储 preamble」落在哪 —— 三个存储位候选

### 方案 S1：`SessionRecord` 加列（`config_snapshot` 内或并列字段）

```
store/model.rs:414  #[native_model(id = 7, version = 2, from = SessionRecordV1)]
```

- 沿 v1→v2 先例加 v3 + `SessionRecordV2` 解码目标（bincode 嵌套结构变布局必须整结构换型，`SessionConfigSnapshot` 已有 V1 形态前例 `model.rs:459`）。
- 若塞进 `config_snapshot`：`SessionConfigSnapshot` 也得再来一版 V2 快照形态；好处是「续会话引擎路由比对」与 preamble 同处，坏处是把引擎配置快照语义污染成「提示词快照」。
- 并列独立字段（`preamble: Option<String>`，`#[serde(default)]` 不够救 bincode → 仍需 V2 中间形态）：语义干净。
- 成本：**一次 v3 升级链 + model_test 升级链断言 + describe（表文档）更新**。

### 方案 S2：新表 `SessionPreambleRecord`（新 native_model id，如 18）

```
#[native_model(id = 18, version = 1)]
#[native_db] pub struct SessionPreambleRecord {
    #[primary_key] pub session_id: String,
    pub preamble: String,       // 或拆 knowledge / agent_md 两列
    pub captured_at: i64,
}
```
- 老会话天然无行（`None` = 未快照 → 回落到现算，兼容存量）。
- 会话行不变 → `SessionRow` 不动 → IPC 面只按需扩。
- 成本：模型注册（`store.rs:341 workspace_models`）+ 读写函数 + describe 表文档 + 信封/db 检视（`envelope.rs` 的模型清单可能自动带出，需查）。

### 方案 S3：塞进密封转录（`SessionEventRecord`）

- 做法：新增一个 `AgentEventKind` 变体，或复用 `SystemNotice` / `Raw`。
- 代价：**污染 core 事件协议面**（事件是密封契约、跨版本重放对象），`resume::rebuild` 还得显式跳过。不推荐，但记录在此以便讨论时排除。

> 注意：preamble 体量 ~2–5KB（知识），若含 AGENT.md 上限 32KB。会话清单 `MAX_SESSION_PAGE_LIMIT=500`、前端页宽 20 → 若随列表返回全文，单页最坏 20×37KB≈740KB。**S1 若直接进 `SessionRow` 会顺带灌进 `list_sessions`**（`assemble_summary`→`session_row`，`store.rs:126`），这是需求③的核心岔口（见 §5）。

---

## 5. 「随前端接口返回」的形状岔口

现状出口（`src/commands/exec/mod.rs`，bindings 已在 `src/types/generated/bindings.ts`）：

```
agent_sessions(root, source, sourceRef, offset, limit) → SessionPage{sessions[SessionSummary]}
session_detail(root, sessionId)                        → SessionSummary | null
agent_session_transcript(root, sessionId)              → AgentEvent[]
```

`SessionSummary = { row: SessionRow, stats, turns }`，`SessionRow = { id, remoteSessionId, configSnapshot, provenance, createdAt, updatedAt }`。

三个形状：

```
形状 A（最省事）：SessionRow 直接加 preamble            → 列表与详情都带全文 ⚠ 载荷膨胀 20×
形状 B（分层）：  SessionRow 加 preambleMeta{savedAt, bytes, sha}，另开
                  session_preamble(root, id) → 全文    → 列表轻、详情/调试页按需拉
形状 C（详情专属）：SessionSummary 加 preamble（row 不动）
                  但 agent_sessions 也返回 SessionSummary → 依旧全量带出 ⚠
```

→ 真正能减载的只有 B（或给 `session_detail` 一个「轻/重」开关）。另外「随前端返回」的产品目的也需要确认：是**调试可见**（我想看这段会话的系统提示词是什么）、还是**UI 展示**（对话流里插一条「系统提示词」卡）、还是**给用户编辑**（编辑会引入新的写面与权限，与 spec「前端 MUST NOT 直接供给 preamble」正面冲突，见 spec:340）。

---

## 6. 层次与依赖约束（这块决定了改动的「形状」而不是「工作量」）

现存三条硬约束把方案夹住了：

1. **`core/agent` 不解释装配**：`SessionInjections.preamble` 是「装配层给的字符串」，core 只透传（spec:381）。若快照的**读**发生在 core（比如 kernel 在 Continue 时塞回 injections），要新增 core 字段/契约。
2. **`AGENT.md` 是 sdk 内部特性、core 零触**（spec:333）：它的读取根是 `SessionCtx.workspace_root`（= exec root，worktree 场景 ≠ 主仓 root），而 `compose_turn` 手上的 `workspace_root` 是**写入保护装配根**（主仓 root）。所以「在 compose 层读 AGENT.md 做合并快照」**会读错根**——要么 `ComposedTurn::begin` 改用 `ctx.workspace_root` 读（infra 层可行，但把 sdk 特性上提），要么由 **runner/sdk 自己回写 store**（runner 已有 `explore_store` / `db_stores` 句柄先例，但回写会话记录在语义上属 sink 半边，绕开 `SessionSink` 会破坏「持久面单口」）。
3. **持久面只有一条口**：`SessionSink`（`core/agent/src/port.rs:42`）/ `SessionQuery`（:78）。写 preamble 要么扩 Sink（`stamp_session_preamble`），要么在命令层/compose 层直接碰 `Store`（`restamp_session_config` 已有先例 `store.rs:1219`——注意它只对「未开轮会话」生效）。

三种可行的接缝（示意图）：

```
(i) 装配层快照（preamble = 知识，AGENT.md 仍每轮读）
    compose.begin ──注入 preamble──▶ kernel.begin_turn ──NewSessionRow(+preamble)──▶ sink
    loop 仍 system_prompt(preamble_记录值, AGENT.md_当轮值)
    ✔ core 改动小（NewSessionRow 加一字段）  ✘ A 敌手未治（AGENT.md 一动前缀仍崩）

(ii) 装配层快照（含 AGENT.md，AGENT.md 上提到 compose）
    compose.begin ──读 ctx.workspace_root/AGENT.md + knowledge──▶ 合并体 ──▶ 落库
    loop system_prompt = 记录值（不再调 preamble::load）
    ✔ 前缀彻底稳定  ✘ AGENT.md 读取点从 sdk 上移（反转 spec:331/333 两条）

(iii) sdk 起播快照 + 回写
    runner.open_session 读一次 AGENT.md → 合并 injections.preamble → 经新 Sink 面回写
    ✔ 语义最贴「会话开始时」  ✘ infra→core 多一条写契约；open_session 无 session_id 写权？实际有（SessionOpen 带 session_id）
```

---

## 7. 待裁决问题（按重要性排）

1. **preamble 的边界**：只含「装配知识」（方案 i），还是「知识 + AGENT.md 合并体」（方案 ii/iii）？只有后者才真正治住缓存主敌手，但会反转「agent 可改自己系统提示词、下轮生效」这条已写进 spec 的行为。
2. **「session 恢复也命中缓存」的期望**：是指进程内续话（多轮），还是含应用重启后的恢复？后者受 5min TTL 限制，需要顺带决定是否切 1h TTL（并承担成本/计费差异）。
3. **前端返回的目的与形状**：调试可见 / UI 展示 / 可否编辑；列表带全文 vs 详情按需拉（§5 的 A/B/C）。
4. **AGENT.md 改动后的显式刷新面**：快照化之后，用户改 AGENT.md 如何生效？候选——①「下个新会话生效」（最简，行为可预期）；② 提供「重载系统提示词」显式动作（新 IPC + 新写面）；③ 只在会话首轮读、后续忽略并给出系统提示词卡提示。
5. **存量会话兼容**：老会话无快照 → 回落现算（会一次性换前缀、miss 一次）还是沿用空值（无 preamble）？

---

## 8. 与既有 spec 的冲突点（改 spec 是必然，不是可选）

| spec 位置 | 现条款 | 本需求会怎么动 |
|---|---|---|
| `desktop-agent-execution/spec.md:331` | AGENT.md **每轮重读、不做起播快照** | 若走方案 ii/iii → 整条反转，需给 migration 语义（存量会话首次继续即快照） |
| `:333` | 本机制 **core 零改动** | 若 Sink/NewSessionRow 扩面 → 措辞要改成「装配/持久面扩面，语义判定仍在 sdk」 |
| `:340` | 前端 **MUST NOT 直接供给** preamble | 若未来支持编辑 → 与「来源约束」条冲突；只读返回则仅需补一条「可见性」 |
| `:1107` / `:1571` | 前缀缓存显式开启；会话内 effort 恒定 | 补一条「会话内 system 前缀恒定（快照）」的缓存纪律条款，把「AGENT.md 改动生效点」写清 |
| `:294` | `SessionInjections.preamble` 按来源装配 | 快照读回后「注入面来自记录」需另立条款，避免与「前端不可供给」冲突 |
| `desktop-agent-management` / 会话查询能力 | `SessionSummary` / `SessionRow` 字段面 | 新增字段要写进会话查询能力的 DTO 条款，并同步 bindings/zod |

---

## 9. 可验证面（怎么证明「命中」了）

```
TurnDone.usage（已有） ──cached_input_tokens / cache_creation_input_tokens──▶ 逐轮可读
    │
    ├─ A 级：loop_test 断言两轮请求的 System 消息**逐字相等**（快照成立的结构证明）
    ├─ B 级：SessionStats 扩 cachedInputTokens（sum_usage_tokens 鸭子键直接可用）
    │        → 前端会话统计/轮行展示「缓存读 / 缓存写」
    └─ C 级：真机联调观察 usage.cached_input_tokens > 0（anthropic 臂）
```
B 级是「保障持续命中缓存」这条需求唯一能变成可回归断言的落点，建议无论选哪个方案都带上。

---

## 10. 若走「方案 ii + 形状 B」，触及面清单（供后续 proposal 估量）

```
core/agent/src/session.rs      NewSessionRow/SessionRow 加 preamble 位（读回用）
core/agent/src/port.rs         SessionSink 加 stamp 面 / SessionQuery 读面（若走 Sink）
infra/agent/src/compose.rs     begin 处合并读 AGENT.md（改用 ctx.workspace_root）+ 落库
infra/agent/src/sdk/loop.rs    system_prompt 改为不再调 preamble::load（或仅首轮）
infra/agent/src/sdk/preamble.rs 读取函数上移或双用（含 32KB 截断口径归属）
infra/agent/src/store_port.rs  映射扩面
infra/store/src/model.rs       SessionRecord v3（或新表 18）+ 升级链
infra/store/src/store.rs       upsert / find / assemble_summary·session_row / describe 表文档
src/commands/exec/mod.rs       session_detail 新字段 或 session_preamble 新命令
src/bindings（自动）           specta 导出
packages/desktop/src/**        生成的 bindings + hooks（use-agent-session）+ 视图
openspec/specs/**              上述 6 处条款
```

---

## 11. 还没查清的（下一步可挖）

- `envelope.rs` 的模型清单（db 检视页）会不会因新表/新列自动暴露、要不要写 `TableDoc`。
- 前端 `agent-session-view` 目前是否已有「系统提示词」位（若有，形状 B 的 UI 落点免费）。
- 压缩/摘要路径（`sdk/compact.rs`）与 system 前缀的关系（spec 已说摘要请求不共享缓存条目）。

---

# 转向：AGENT.md 改动后如何生效（产品化处理谱系）

用户裁决：先看这条，再定快照边界。以下是第二轮调研结果。

## 12. 五条新增关键事实（直接决定谱系形状）

### F1. `AGENT.md` 有**两个身份**，互不替代

```
身份① 系统提示词：sdk loop 每轮读 → 进 System 消息            （自动、隐式、前缀最前）
身份② 项目文档：  相位 prompt 明写「读项目 CLAUDE.md / AGENT.md」 
                  （proposal-planner.md:11、dev-design-planner.md:13、code-analyze-planner.md:13）
                  → agent 用 read 工具自己读                   （显式、按需、不占前缀）
```

**推论**：冻结身份①不等于 agent 失去最新内容 —— 它仍能 read 到最新版。这大幅降低了「冻结 system」的产品风险，也让 P1/P3 组合成为可能。

### F2. `AGENT.md` 不在写保护锁定清单里

`protect/rules.rs:95 builtin_rules()` 只锁「`**/` + 配置文件名」（`.dev-team/config.json`）；`user_rules` 才读 `write_protection.files[]`。所以**当前 agent 可以用 write / edit / bash 改自己的系统提示词** —— 这正是 spec:331 拒绝快照的原始理由。

### F3. 这个仓库的 `AGENT.md` 真的会被 change 改

`git ls-files` 确认它被跟踪；`git log -- AGENT.md` 显示随 change 提交（如 `archive: archi-doc-api-surface-unification`、`refactor(desktop): move archi model declarations`）。即「会话中文件变化」不是假想场景。

### F4. 变更检测基建已就位（零新建）

- `infra/watch`：通用单文件 watch，**信号不含内容字节**，父目录挂 notify、rename-replace 免疫（`watch/src/lib.rs`）。
- 壳层命令 `watch_subscribe(channel, path)` / `watch_unsubscribe`（`src/commands/watch/mod.rs:72/109`），canonical path 幂等。
- 前端 hook `useFileWatchSignal`（`hooks/use-file-watch-signal.ts`），已被 `use-explore-doc` / `use-exec-output` 消费。
- 前端已知会话 cwd（`use-agent-session` 的 `cwd` 参数），订阅 `<cwd>/AGENT.md` 无需新后端能力。

### F5. `SystemNotice` 能落库进转录，但前端目前只消费 `api_error`

- 后端：subtype 是开放词典（`context_pruned` / `api_error` / `output_truncated` / …），事件密封落库。
- 前端：`agent-adapter.ts:219` 把 `systemNotice` 统一映射成 `data-system-notice` 部件，但真正渲染的只有 `agent-error.ts:109` 的 `api_error` 分支；其余 subtype 在 UI 上**静默丢弃**。
- 推论：若要「系统提示词已变更」提示走 notice 通道，前端需加一条渲染分支（小工作量），或走会话头/横幅而不进流。

## 13. 六种生效方案（= 边界 × 生效时机 × 通知方式）

```
              前缀稳定？  改动何时生效？        实现成本   风险
P0 现状        ✘          下一轮                —        前缀反复失效
P1 冻结+新会话  ✔✔        新会话               低        「改了没反应」无提示
P2 冻结+显式重载 ✔✔       用户点重载（下一轮）  中        多一条写 IPC + 权限面
P3 冻结+变更提示 ✔✔       用户点重载/忽略       中        前后端各一小段（watch 现成）
P4 冻结+尾部增量 ✔✔       下一轮（作为消息）    高        契约破坏最大、token 双份
P5 只冻知识     ~          AGENT.md 仍每轮     低        主敌手未治
P6 冻结+锁文件   ✔✔        须新会话 / 人改后重载 低+      改 write_protection 判定矩阵
```

**P1 冻结 + 新会话生效**（最小闭环）

```
首轮：读 knowledge + AGENT.md → 合并体 → 存库（+sha/时间） → 进 System
续轮：读库 → 逐字进 System（不读文件、不算知识）
新会话：重新拍快照（自然拿到最新文件）
```
- 副作用面**很小**：change 流程每个 step 是独立会话（`walker.rs:3474` 按 `{changeId}/{phase}/{role}/{attempt}` 现拼 source_ref），跨 phase 天然新快照；archi / config / debug 常驻会话才是"改了没反应"的暴露面。
- 缺一块：用户改了文件，旧会话毫无提示 → 需要 P3 的提示或 P1 的"冻结说明"补丁。

**P2 冻结 + 显式重载**

```
session_preamble_reload(root, sessionId) → 重读文件 → upsert 记录 → 下一轮生效（必然 miss 一次）
约束：只在会话空闲时允许（运行中换前缀会让后续轮次与已发前缀不一致，agent 视角突变）
```
- 需要新 IPC（写面）+ 前端入口 + 权限与错误映射；spec 侧要写「显式重载是新会话外的唯一变更入口」。

**P3 冻结 + 变更提示**（与 P1/P2 组合最自然）

```
前端 useFileWatchSignal(<cwd>/AGENT.md) ──变更信号──▶ 比对会话头展示的 preambleMeta.sha
  一致 → 不提示；不一致 → 会话头 chip「AGENT.md 已变更（点此重载 / 新会话生效）」
```
- 依赖需求③的返回形状（形状 B 的 `preambleMeta{sha,bytes,savedAt}` 正好够用）——**需求②③在这里合流**。

**P4 冻结 system + 易变段挪到尾部**

把 AGENT.md 从 System 挪成「紧跟最新 user 的一条 user 消息」：
- 优点：既"改动即生效"又"前缀稳定"（变化只落在未缓存尾部）。
- 代价：`AGENT.md` 的指令优先级从 system 掉到 user；每轮重复进史（未缓存 token ×轮数）；被 compact 摘要污染；spec:331/342 整条重写。**除非 P1/P3 被否，不建议**。
- 变体 P4'：system 冻结，变更时把 **diff** 作为一条 notice/user 消息注入尾部（前缀仍稳、改动仍生效）——最巧但需要 diff 与版本管理，复杂度最高。

**P5 只冻知识**：即原方案 i —— 治住了 B 敌手，A 敌手（AGENT.md）仍在。

**P6 冻结 + 把 AGENT.md 纳入写保护**：`builtin_rules()` 加一条（agent 禁写），人仍可改。与 P1/P2 组合可把"agent 改自己提示词"这个理由从根上摘掉（spec:331 的论证前提消失）。

## 14. 与缓存 TTL 的交叉（P1–P3 都要面对）

change 流程里真正"同一会话多轮"的形态是 **evaluator 打回后 executor 在同一会话续跑**（`walker.rs` 的 `continue_session` = `cursor.executor_session`，fix 反馈边同会话续注）。这类续跑两轮之间隔着「evaluator 一整个会话」的时长 —— 常常超过默认 5 分钟 TTL。

```
首轮请求 ──5min TTL──▶ [evaluator 会话: 数分钟~数十分钟] ──▶ executor 续跑轮
                                                              ▲
                        缓存条目多半已过期 ────────────────────┘
                        → 想真命中：需 with_automatic_caching_1h（1h TTL）
```

所以「保障一段会话持续命中缓存」若要走到底，得连 TTL 一起决策：`1h`（成本/计费差异、缓存写入溢价）还是默认 5min（只在密集续跑时命中）。

## 15. 建议的组合（供裁决）

```
推荐：P1 + P3 + P6 + 形状 B + 缓存统计
  P1  冻结合并体，新会话生效              ← 前缀严格稳定（治 A/B 两个敌手）
  P6  AGENT.md 纳入写保护                 ← 摘掉「agent 改自己提示词」的论证前提
  P3  watch 检测 + 会话头提示「已变更」    ← 补上「改了没反应」的可见性缺口
  P2  提示里的「重载」按钮 = 显式写面      ← 需要再裁决要不要（P1+P3 可先不带上）
  形状B  preambleMeta + 按需取全文        ← 列表不膨胀，且给 P3 提供 sha 比对
  统计  SessionStats + cachedInputTokens   ← 把「命中」变成可回归断言

放弃：P4 / P4'（契约破坏大）、P5（治不住主敌手）、P0（现状）
```

## 16. 追加待裁决

1. 是否接受 **P1「新会话生效」** 作为默认语义（旧会话冻结、无提示）？还是必须同时带 P3 提示（甚至 P2 重载按钮）才能接受？
2. 是否接受 **P6 把 `AGENT.md` 加入内置写保护**（agent 禁写、人可写）？这会改 `desktop-write-protection` 的锁定清单与判定矩阵。
3. TTL 选 **默认 5min** 还是 **1h**？（影响"恢复也命中"这条承诺能不能兑现）
4. 快照体量：含 AGENT.md 时上限 32KB → 是否照存（每会话一行 ≤37KB），还是超阈值只存元信息 + 每轮回读文件（混合策略，语义变复杂）。

---

# 定向：P1 + P3 的落地骨架（用户已裁决）

## 17. 时序图（目标态）

```
新建会话首轮                          续会话多轮 / 恢复
─────────────────────────             ─────────────────────────
compose.begin(New)                    compose.begin(Continue{id})
  ├ assemble_injections(provenance)     ├ assemble_injections(provenance)   ← tools/finish/ask/protect 仍重算
  │    └ preamble = knowledge()         │    └ preamble = knowledge()        （计算出来但随后丢弃）
  ├ 读 ctx.workspace_root/AGENT.md      ├ 读库：find_session(id).preamble
  │    （32KB 截断，sdk::preamble 口径）│    └ 命中 → 覆盖 injections.preamble
  ├ 合并 = knowledge + "\n\n" + agentMd ├ 缺失（存量会话）→ 回落现算 + 补拍
  └ 写库 stamp_session_preamble(id,…)   └ （不读文件、不算知识）
                │                                     │
                ▼                                     ▼
        loop: system_prompt = injections.preamble  同左（**不再调 preamble::load**）
                │
                ▼
        Message::System（逐字稳定 → 前缀命中）
```

## 18. 关键设计点（都有现成先例/判据，零新建概念）

| 设计点 | 现成锚点 |
|---|---|
| **「会话首轮」判定** | `Store::is_unstarted_session`（`store.rs:1198`，无句柄+无轮行+无转录）；`restamp_session_config` 已按它做「首轮定型」（compose.rs:419）——快照时机与之对齐 |
| **写库接缝** | 组合根直写 `Arc<Store>` 已有先例（`restamp_session_config` 就在 `compose.begin` 里调）→ 新增 `Store::stamp_session_preamble(id, pencil)` 即可；**不必**扩 `SessionSink`（但若要求「持久面单口」，也可给 Sink 加面，二选一） |
| **读回接缝** | `compose.begin` 已在 Continue 分支 `query.store().find_session(id)`，读同一条记录的新字段零成本 |
| **32KB 截断归属** | `sdk/preamble.rs::truncate` 保持唯一口径；读取点从 loop 上移到 compose（`preamble::load` 改 `pub` 或经 compose 复用），**截断逻辑零复制** |
| **快照后的知识版本语义** | 旧会话沿用旧知识（应用升级后骨架变了也不换）——这是「前缀稳定」的**必然代价**，必须写进 spec 并接受；新会话自然拿新知识 |
| **存量会话迁移** | 无快照的会话在下次 Continue 时补拍（一次性 miss，之后稳定）；或选「永久现算」二选一 |
| **`AGENT.md` 截断上游变化** | 若文件超 32KB，快照存的是截断体——`sha` 必须对**截断后的内容**算，否则前端 watch 比对永远不等 |

## 19. P3：变更提示的最小闭环（全部有现成件）

```
① 判据后端给（推荐）：
   session_detail 的 preamble meta 带 agentMdSha（快照时的 AGENT.md 段哈希）
   + 新增轻命令 session_preamble_status(root, sessionId)
        → { agentMdSha: String, changed: bool }   // 后端读当前文件算哈希后比对
   前端 watch 信号到达 → 调 status → changed 决定 ②
   （备选：meta 里直接带 currentAgentMdSha，但每次 session_detail 都要读文件，污染读路径）

② 提示落点：SessionPanelState.note（`session-panel.tsx` 现成插槽）
   同族先例：DRAFT_NOTE「发送首条消息即开启新会话」
             REFRESH_NOTE「模型改动不会自动刷新，改完点「刷新」」
   文案候选：`AGENT.md 已变更，本会话系统提示词已冻结（新会话生效）`
   注意 note 的生成规则在来源侧 hook（use-archi-chat / use-config-chat / use-change-chat），
   若要做成通用能力，需要把它下沉到 use-agent-session 或加一个并列的 chip 位

③ 订阅路径：`<cwd>/AGENT.md`，其中 exec root = `cwd ?? root`（后端 `resolve_exec_root` 同口径，
   前端用同一式子算，无需新字段）；worktree 场景 cwd = worktree → 天然按会话隔离
```

## 20. 需求③的返回形状（建议）

```
SessionRow.preambleMeta?: { savedAt, bytes, sha }        ← 轻（几十字节），随列表/详情都带
SessionSummary.preamble?: { …preambleMeta, content }     ← 全文，列表路径 content = null、详情路径 = 全文
   （好处：类型自解释「列表轻、详情重」；坏处：同一类型两态，需要有测试钉住）
备选：session_detail 单独返回 preamble 全文，SessionSummary 不动。
```

三个形状都比"把全文塞进 SessionRow"稳，理由见 §5。

## 21. 缓存统计（建议与 P1/P3 一起进）

```
core/agent/src/usage.rs   sum_usage_tokens 鸭子键已通用 → 加 "cached_input_tokens" 与
                          "cache_creation_input_tokens" 两次求和
core/agent/src/session.rs SessionStats 加 cachedInputTokens / cacheCreationInputTokens
infra/store/src/store.rs  aggregate_stats 补两路求和
前端                      会话统计展示（若已有 tokens 展示位则一行；否则可只进类型不进 UI）
```
这是「保障持续命中缓存」唯一能落成回归断言的验收面（§9 的 B 级）。

## 22. spec 条款清单（P1+P3 落地的必然改动）

| 能力 | 条款 | 改法 |
|---|---|---|
| desktop-agent-execution | `:331` 每轮重读、不做起播快照 | 整条反转：首轮快照 + 续轮读库 + 存量会话补拍 + 升级后旧知识沿用 |
| desktop-agent-execution | `:333` core 零改动 | 若快照走 `NewSessionRow`/Sink 则放宽措辞；若组合根直写则维持「core 零改动」 |
| desktop-agent-execution | `:342`/`:344` AGENT.md 在场逐字注入等 5 个 Scenario | 语义平移：首轮注入 = 快照内容；补「续轮不重读」Scenario |
| desktop-agent-execution | `:1107`/`:1571` 前缀缓存 / effort 恒定 | 补「会话内 system 前缀恒定（快照保证）」条款；TTL 若切换需另立 |
| desktop-agent-management | 会话查询 DTO（SessionRow / SessionSummary / SessionStats） | 新增 `preambleMeta` / `preamble` / 缓存统计三处字段 |
| desktop-file-watch | 复用既有能力 | 若前端只是新增一个订阅者，无需改 spec（能力已覆盖） |
| 新能力 or 既有能力扩条 | 「系统提示词快照与可见性」 | 也可在 desktop-agent-execution 内扩 requirement，避免新能力空壳 |

## 23. 剩余开放问题（小颗粒，可在 proposal 阶段定）

1. **P3 的提示是纯告知还是带动作**：纯告知（"新会话生效"）→ 不动写面；带「重载」按钮 → 需要 P2 的写面（`session_preamble_reload`）与运行中禁用的判据。
2. **TTL**：默认 5min 还是 `with_automatic_caching_1h`（决定「恢复也命中」能否兑现）。
3. **存量会话**：补拍（一次性 miss）vs 永久现算（老会话永不快照）。
4. **快照字段布局**：合并单列（`preamble: String`）vs 分列（`knowledge` / `agent_md` 两列 + 合并时机）。分列的好处：未来知识升级可单独刷新；坏处：多一列 + sha 计算面变多。
5. **`AGENT.md` 缺失时**：快照只含知识（`agentMd = None`），之后文件出现能否补入？建议**不能**（冻结即冻结，新会话才有）——要在 spec 写明。

---

# 再转向：靠对话要求重读（用户提出的生效方式）

用户裁决：**不做 UI 重载按钮，也不做写面** —— 生效方式是人在对话里说一句
「我刚刚更新了 AGENTS.md，请重新读取它并严格按新规则执行」。

这一招的妙处是它把 **F1 的两个身份打通**：身份①（system）冻结不动 → 前缀稳定；
身份②（read 工具）由人一句话触发 → 最新规则以 **tool 消息**进入史尾。

## 24. 为什么这条路在缓存上比 P2 更好

```
P2 重载：改写 system → 下一轮 system 与上一轮不同 → 整段前缀 miss（含 tools + 全部史）
P_read ：追加一条 read 结果到史尾 → 已缓存前缀（system + tools + 旧史）仍逐字相同 → 命中
                                    只有新增的那几 KB 未命中
```

```
前缀比较（Anthropic / OpenAI 同构）：
轮 N   : [system][tools][msg1][msg2]…[msgN]              ← 全段已缓存
轮 N+1 : [system][tools][msg1][msg2]…[msgN][user:重读][tool:AGENT.md 全文]
         └──────────── 逐字相同 → 命中 ────────────┘   └── 新增未缓存 ──┘
```

所以 P_read 是「零 miss 的生效方式」，恰好也是 §3 里 A 敌手的正解——只要**没有人替 agent 把 AGENT.md 重读进 system**。

## 25. 这条路的三个技术前提（已核实）

| 前提 | 事实 | 结论 |
|---|---|---|
| read 能读到完整规则 | `read` 默认 `MAX_READ_LINES=2000`；仅当文件 > `LARGE_FILE_BYTES=100KB` 才收窄为前 100 行 + 分页提示（`sdk/tools.rs:22-36`）。本仓 `AGENT.md` = **6503 B / 42 行** | ✅ 本仓语义没问题；大文件仓库需 agent 主动分页（提示语里已给 offset 用法） |
| 读取根 = AGENT.md 所在根 | `read` 的 base = exec root（= `cwd ?? root`），与 system 注入同根 | ✅ worktree 场景读的是分支版 |
| 两条通道口径可能不一致 | system 注入上限 **32KB 截断**；read 可读全（≤100KB 默认全读） | ⚠ 文件 32KB–100KB 时「system 里的规则」是前缀、「read 出来的规则」是全文 —— **既有缺口**，快照化后反而能被前端展示/提示出来 |

## 26. 需要补的一小块设计（关键，且零成本）

冻结 system 之后，agent 并不知道「你看到的这份规则是会话开始时的快照、真身在 AGENT.md 且可能已更新」。建议在快照内容尾部加一行**装配层常量指引**（进快照 → 逐字稳定 → 不破缓存）：

```
……（会话 preamble 快照正文）……

（本会话系统提示词冻结于会话开始时；项目规则真身为 AGENT.md，
  若用户提示规则已更新，请用 read 重新读取 AGENT.md 后按新规则执行。）
```

- 不违反 spec:340（不是前端/自由字符串供给，是装配层常量）。
- 让「用户说一句」这条路径从**约定**变成**agent 可发现的机制**（否则模型未必会去读）。
- 附带收益：这条指引会随快照落库 → 需求③把它返回前端时，用户也能看到"规则真身在文件里"。

## 27. P3 降级后的形态：从「提示」变成「一句话按钮」（推荐）

既然生效靠人说话，那 watch 提示的最佳形态不是通知，而是**把这句话准备好**：

```
watch(<cwd>/AGENT.md) 变更信号 + sha 比对（后端 session_preamble_status）
   → 会话头 chip：「AGENT.md 已变更」
   → 点一下：把「我刚刚更新了 AGENT.md，请重新读取它并严格按新规则执行」填进输入框
              （只填不发 —— 与既有 composer 行为一致，用户仍可编辑）
```
- 零新写面、零新后端能力（复用 `useFileWatchSignal` + 一个只读 status 命令）。
- 需要的入参：当前快照的 `agentMdSha`（来自需求③的 `preambleMeta` 或 status 命令）。
- 若连这个也不想做：退化成 **常驻一句 `note`**（`SessionPanelState.note` 现成插槽）：
  `'系统提示词在会话开始时冻结；改 AGENT.md 后直接对我说「重新读 AGENT.md」即可'`。
  零订阅、零后端、零新命令 —— 需求②③的最小可用形态。

## 28. 一个必须澄清的字面事实：`AGENT.md` vs `AGENTS.md`

**已确认：用户原文为笔误，产品只读 `AGENT.md`（单数），读取源一切照旧。**

```
sdk/preamble.rs:16     root.join("AGENT.md")
spec:331               「读取范围 SHALL 严格限于 AGENT.md，MUST NOT 做 CLAUDE.md 等兜底回退」
repo 事实              git ls-files：AGENT.md + CLAUDE.md（无 AGENTS.md）
```

（留档：若未来要支持生态通用的 `AGENTS.md`，属新增读取源的独立决策——优先级、是否级联多目录、与「MUST NOT 兜底回退」条款的关系都要另立；本需求不涉及。）

## 29. 更新后的推荐组合

```
① 快照（P1 冻结 + 首轮拍 + 续轮读库）          ← 不变
② 快照尾部加「规则真身在 AGENT.md，可用 read 重读」指引常量   ← 新增，零成本
③ 生效方式 = 人说一句 → agent read               ← 用户裁决，零写面
④ 发现性 = note 常驻文案（最小） / chip 填入一句话（推荐进阶）  ← P3 降级形态
⑤ 形状 B：SessionRow 带 preambleMeta + 详情带全文  ← 不变（也是 ④ 的判据来源）
⑥ 缓存统计 cachedInputTokens                    ← 不变（验收面）
放弃：P2 重载写面、P4 尾部注入、P5、P6
```

---

## 30. 最终方案速览（可直接作为 proposal 输入）

```
【一句话】会话首轮把「装配知识 + AGENT.md」拍成一份快照存库，此后本会话每次请求都用
          这一份逐字不变的 System；规则要更新由人对 agent 说一句「重读 AGENT.md」，
          走 read 工具落在史尾 —— 前缀零 miss；快照元信息随会话接口返回前端。

【写】compose.begin 首轮（is_unstarted_session 判据）
        knowledge() + "\n\n" + preamble::load(ctx.workspace_root) + 指引常量
        → Store::stamp_session_preamble(id, 合并体, savedAt, agentMdSha, bytes)

【读】compose.begin 续轮：find_session(id).preamble 命中 → 覆盖 injections.preamble
        缺失（存量会话）→ 现算 + 补拍 → 之后稳定
        loop：system_prompt 改为一元（不再调 preamble::load）

【存】候选：SessionRecord v3 加两列（preamble: String / preamble_meta 嵌入）
        或新表 SessionPreambleRecord(id 18)，老会话无行 → 回落现算

【出】SessionRow.preambleMeta{savedAt, bytes, sha, agentMdSha}   ← 列表/详情都带（轻）
        SessionSummary.preamble{…meta, content}                  ← content 仅详情路径填
        + 只读命令 session_preamble_status(root, id) → {agentMdSha, changed}

【发现性】note 常驻文案（最小）或 chip「AGENT.md 已变更」+ 一键填入重读话术（推荐进阶）

【验收】SessionStats + cachedInputTokens（TurnDone.usage 鸭子键）；loop_test 断言两轮
        System 消息逐字相等；两次续跑 usage.cached_input_tokens > 0

【代价，必须写进 spec】
  ① 应用升级后旧会话仍用旧知识（前缀稳定优先）
  ② 存量会话首次 Continue 补拍 → 一次性 miss
  ③ AGENT.md 缺席时快照永久无该段（后来出现不补）
  ④ 32KB 截断口径不变；文件 32–100KB 时 system 前缀 ≠ read 全文（展示面应提示）
```

## 31. 余下未查（若继续挖）

- `envelope.rs` / `describe.rs` 的表清单与新表/新列的联动（db 检视页要不要写 `TableDoc`）。
- 前端测试面：生成的 bindings + zod schema 变化会让哪些 mock/fixture 需要补字段（`ipc-drift.test.ts` / `agent-session-view.test.tsx`）。
- `total` 计数与 `preambleMeta` 是否进 `agent_sessions` 的 "会话清单轻量化" 既有约束文档（未见专门 spec）。
- `archi` / `config` 常驻会话的 `cwd` 恒 null → 其 AGENT.md 根恒为主仓 root，是否需要各自的快照独立性确认（目前按会话独立，天然 OK）。
