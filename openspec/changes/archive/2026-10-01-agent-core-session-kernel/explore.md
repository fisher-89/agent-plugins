# agent core 重设计:会话域内核(纯类型库 → 有状态内核)

> 状态:探索收敛——开放问题 1-4 已拍板(见「裁定」节),下一步 phase-requirements
> 日期:2026-09-30
> 关联:`openspec/explores/rig-package-selection.md`(原则修正二轮:适配目标恒为 core 协议)
> 触发:debug 页 SDK 一轮回复碎成 1-2 字多行的 bug;由 bug 升维重审 core 形状

## 动机:两个 bug 同根

### bug 1:碎事件(已实锤,源码证据)

- **rig-core 0.42 流面不对称**(`streaming/mod.rs:3019` 枚举定义):`Text(Text)` 注释原话「Text **delta** emitted by the assistant」——text 只有增量、无完整块变体;`Reasoning` / `ToolCall` 有「增量变体 + 完整块变体」(完整块 supersede 增量),text 的完整聚合不在流内、在流末 `response.choice`(rig 测试锁定「text 被工具调用切分成 first/second 段」的聚合行为)
- **normalize 失配**(`infra/agent/src/sdk/normalize.rs:40-47`):把每个 `Text` 增量当完整块直接出 `Message` 事件;`ReasoningDelta` / `ToolCallDelta` 被正确跳过(等完整块),唯独 text 等不到——流内根本没有完整 text 块
- **传播链**:SSE chunk(1-2 字)→ 每个 delta 一个 `Message{blocks:[Text]}` 事件 → loop 盖 seq → store N 条碎事件入库 → adapter 1 事件 = 1 条 `evt-<seq>` UIMessage → timeline 1 UIMessage = 1 行(divide-y)→ 一轮回复显示为几十条碎行
- **追加证据(拍板 Q3 时验证)**:碎事件还**毒化 resume 重建**——`resume.rs:50-55` rebuild 把每个 Message 事件 → 一条独立 rig Message(无折叠),续会话时 provider 收到几十条 1-2 字连续微消息;与 bug 2 复合(链式丢上下文 + 重建史碎裂)
- CLI 租户无此问题:claude stream-json 的 assistant 事件本就是完整消息——**core 协议的 `Message` 语义事实上的确是「完整消息」,SDK 引擎把 provider 传输粒度(SSE delta)泄漏进了协议**

### bug 2:SDK 链式续会话逐轮丢上下文(探索中发现,潜伏)

- `commands/exec/agent.rs:134-151` `find_events_by_session`:`list_agent_runs()`(**降序**,`store.rs:329-337` 按 started_at desc)上 `.find()` 命中**最新** run,取其事件作 resume 转录
- SDK 每 run 只落**本轮**事件(resume 重建的旧史不重发,`sdk/loop.rs` 只发本轮流事件);链上各 run 同 session_id
- ⇒ 第三轮续会话的重建史 = 第二轮事件,**第一轮静默丢失**;链越长丢越多(只保最后一轮)
- CLI 免疫:`--resume` 由 claude 自持全量会话状态,转录重建是 SDK 独有路径

### 同根诊断

| bug | 根因归属 |
|---|---|
| 碎事件 | 协议无增量语义,且**传输粒度 = 落库粒度绑死**(tee 同一流既流出又落库) |
| 链式丢上下文 | **session 非一等公民**:转录 = 单 run 事件的拼凑,非会话全史 |

两者都是 core 形状的欠账,不是实现失误——重设计把两笔一起还。

## 目标架构

```
现状:core = 纯类型库(信封 + AgentRunner trait + 状态机,零 IO)
      编排(tee/落库/收敛)住 commands/exec
      session = run 记录上的衍生字段(engine 铸的 session_id)
      装配 = resolve_agent_engine(commands 层,临时拼装)

目标:core = 会话域内核(有状态,经 port 出 IO)
┌───────────────────────────────────────────────────────┐
│  core: AgentInstance(装配产物)                          │
│                                                       │
│  公共 API:                                             │
│  ├─ run_session(New | Continue{id}, prompt) → 流 + 句柄 │
│  │    流:token 级增量(text/thinking 可辨) + 密封消息     │
│  │    句柄:可终止(RunHandle 语义保留)                    │
│  └─ query_sessions(...) → 会话记录(+ 聚合统计)           │
│                                                       │
│  内部功能(经 port):                                     │
│  ├─ SessionStore port:会话/run/事件落库、转录重建        │
│  ├─ ToolProvider port:平台工具注入(store 读写/通知)      │
│  └─ 会话状态机 + seq/时间戳治理(现 commands 编排下沉)     │
└──────┬──────────────────┬──────────────────┬──────────┘
       │ infra 实现         │ infra 实现         │ infra 实现
   redb store 适配      平台工具执行体       引擎适配(loop/memory/LLM)
                                          ├─ sdk(rig-core 手搓 loop)
                                          └─ cli(claude 租户)
```

原话留痕(用户愿景):公共 API = ①运行会话(新建,继续),会话过程支持流式 token 级传输、能区分思考和回复、可终止;②查询会话记录。内部功能 = ①用 store 记录会话信息(agent 配置、提问轮数、累计墙钟、token 消耗等,MVP 少量、可扩展);②agent 上下文注入平台工具(store 读写、通知)。**loop、memory、LLM 协议由 infra 层适配。**

### 与现有资产的映射

| 资产 | 在新架构中的位置 |
|---|---|
| `AgentInstanceRecord`(store 管理域,引擎 + provider 引用 + 默认标记) | 装配的**配置源** |
| `resolve_agent_engine`(commands/exec/agent.rs:165) | 装配单点雏形——住错层,应下沉进内核组合根 |
| `RunHandle` / `RunStateMachine` / seq 盖戳治理 | 语义保留,收进内核 |
| 前端 `eventToChunk` 增量路(agent-adapter.ts) | 已就绪——`text-delta` chunk 是 ai-sdk 原生词汇,只差 delta 事件进来 |
| P1 原则(rig explore:core 持标记,infra 实现循环) | **加强而非违背**:core 从持「标记」升格为持「记录 + 查询」,loop 仍在 infra |
| P2 原则(memory 是引擎特性) | 不变 |

## 核心协议决策:传输粒度与落库粒度解耦

```
              引擎产出
  ┌─────────────┴──────────────┐
  │ MessageDelta(增量,ephemeral)│  ← token 级;TextDelta / ThinkingDelta
  │                             │     只上传输面,永不落库
  ├────────────────────────────┤
  │ Message(密封,durable)       │  ← 完整消息;唯一落库与重放单位
  ├────────────────────────────┤
  │ RunStarted/SystemNotice/    │
  │ RunResult/Raw(不变)          │
  └────────────────────────────┘
```

- **SDK 引擎**:流内 `Text` / `ReasoningDelta` → `MessageDelta`;轮末 `choice` → 密封 `Message`。碎事件 bug 自然消解——流式显示与干净记录兼得;「方案 A 轮末收口」(bug 的当前架构修法)退化为本协议的退化情形(丢增量、只留密封)
- **CLI 引擎**:现状零变化;将来可选 `--include-partial-messages` 翻译 `stream_event` → `MessageDelta`——delta 语义对两租户都有增值空间
- **落库**:store sink 只落密封类,delta 不占库(redb 不吃每 token 一行);seq 语义需定稿(delta 占 seq 产生重放空洞——排序键语义可容忍;或 delta 独立编号空间,设计阶段定)
- **前端**:适配层 delta → provisional 消息(correlator 键),密封到达以密封为准替换
- **原则连接**(rig explore 二轮):delta 是 core 协议原生词汇,不是对任何 provider 行为的模仿——词典自己铸

## Session 数据模型(草案)

```
SessionRecord(workspace 维度)
  id                 ← core 铸(链式丢上下文的正解:转录 = 会话全史)
  engine_session_id  ← engine 侧句柄(cli: claude session;sdk: 可弃)
  config_snapshot    ← 装配配置快照(engine/model/权限档)
  created_at / updated_at
  └─ runs[]          ← 每轮一个 run(现 AgentRunRecord 挂 session 外键)
       └─ events[](密封)
```

- **跨库维度张力**:`AgentInstanceRecord` 在全局库、session 在 workspace 库,store 惯例禁跨库引用 ⇒ session 存**配置快照**而非引用(实例改名/删除不伤历史会话;「记录 agent 配置」的愿望恰好是对的形状)
- **聚合统计**(轮数/累计墙钟/累计 token):MVP 从 runs 现算,不维护累计列(避免写放大);查询 API 是天然聚合点,将来不够再加

## 平台工具注入

```
ToolProvider port(core 定义)
  ├─ definitions() → 工具面(store 读/写、通知……)
  └─ invoke(name, input) → 执行体回调
```

- **SDK 引擎先行**:loop 工具执行改双轨——引擎原生工具(六件套,沙箱内)+ 注入工具(回调宿主)
- **CLI 引擎受限**:claude CLI 工具面是它自己的,注入走 MCP——即 rig explore 留的逃生门(facade `rmcp` feature),天然排后

## 开放问题(待拍板)

1. **内核纯度**:core 经 port 出 IO 后,spec「core MUST NOT 认识 claude/引擎/不依赖 Tauri」全部保留,但「纯类型无 IO」改写为「无直接 IO、经 port」——spec 层面原则修订,是否接受?
2. **密封消息归属**:事件挂 run(现状,session 查询 fold 链)还是直接挂 session(转录单表,run 退化为统计行)?前者迁移小,后者查询/重建简单——倾向后者,待拍板
3. **切片**:①协议 delta 语义 + 修碎事件(独立可交付,不等重构);②session 一等公民 + 查询 API + 修链式 bug;③instance 装配 + 平台工具注入。三刀各自成 change 依次走,还是一把梭?
4. **链式丢上下文 bug**:先在当前架构热修(转录 fold 全链)再谈重构,还是直接作为 ② 的动机?

## 裁定(2026-10-01)

1. **内核纯度:接受**——core 经 port 出 IO,spec 原则修订为「无直接 IO、经 port」;「MUST NOT 认识 claude/引擎/不依赖 Tauri」全部保留。
2. **密封消息归属:后者**——事件直接挂 session(转录单表),run 退化为统计行;查询/重建简单优先于迁移省事。
3. **切片:②吸收①,③仅预留延后**——
   - ②是否天然修复①:**是,分两半**:落库半边天然且被迫(转录定为会话全史 + 重建源后,密封完整消息是唯一可行 durable 单元——碎事件毒化 resume 重建的追加证据使 store sink 只落密封成为②的结构性必然);传输半边非自动但本就是②的核心交付物(run_session 的「流式 token 级传输、区分思考和回复」愿景原文)。
   - ①不独立成 change,方案 A 热修作废(与裁定 4 立场一致:两 bug 都等②)。
   - ③(instance 装配升维 + 平台工具注入)仅预留接口位,延后实现;②内装配走最小组合根(resolve 下沉,不做 instance 驱动的完整装配 UX)。
4. **链式丢上下文 bug:直接进②**,不做当前架构热修。

实际切片收敛为**一个 change**:会话内核(协议 delta/sealed 双层 + session 一等公民 + 查询 API + 编排自 commands 下沉),③素材留本文件供将来引用。

## 协议终形(三轮细化收敛,2026-10-01 追加)

> 本节为最终结论(用户裁定:仅追加终形,中间轮次不落盘)。上文「目标架构」中 SessionStore port / ToolProvider port 的形状以本节为准。

### 终形架构

```
core 会话内核
├─ 运行面:P1 引擎协议(参考 Actor,不照搬)
│    ├─ 会话建立:injections(prompt + tools)+ ctx(workspace root、
│    │            部分 config、可扩展)+ session 引用
│    ├─ 轮驱动:question;终止:stop
│    └─ 观察回流:Delta / Sealed / Notice / Raw / TurnDone
├─ 治理面:盖戳(seq/时间戳)、收敛状态机、停止注册(内存态)
├─ 持久面:write-through——收到事件/返回值即经 store 原子操作持久化;
│          查询 api 重导保留为对账纠偏(误差校正,非主路径)
└─ 查询面:core 定契约,infra 实现(历史视图/转录重放)

infra
├─ 引擎(sdk/cli):loop / LLM / memory / 会话还原 / 注入整合,全自治;
│    ├─ 唯一标识经协议上报(spawn 响应或事件;现 RunStarted.session_id 即此形态)
│    └─ 还原归 infra:CLI 用 engine_handle 原生 --resume;SDK 自读会话转录
└─ store:转录唯一家 + 「core 要求的查询 api」实现
```

### P1 协议保证(不变量清单;机制不规定)

| 保证 | 锁死 | 自由 |
|---|---|---|
| 流式可实现 | 观察词汇含 delta,通道有界背压 | 通道形状(mpsc / actor mailbox)引擎自选 |
| 统计口径唯一 | TurnDone 字段面(num_turns / duration_ms / usage / cost)= 唯一口径,core 累计计算;引擎不得另立 | 引擎内部采集方式 |
| 唯一标识上报 | engine_handle 在 spawn 响应或事件中告知,core 记双 id 映射 | 上报时机二选一 |
| 注入/轮分离 | injections(会话级)+ question/ctx(轮级) | 整合方式(preamble / --append-system-prompt / 忽略) |

### 词汇与持久化纪律

- 观察词汇密封纪律不变:`Message` = 唯一 durable 单元,一轮 assistant 产出恰一条;delta 只上传输面,store 永不见
- 持久化 write-through:core 收到返回值或事件即 store 原子操作落盘;查询 api 对账重导(从转录重建统计/索引)保留为纠偏机制
- 双 id 映射(core session id ↔ engine_handle)由 core 记录,engine_handle 经协议到达

### 缺省与降级设计

统计事件字段与查询 api 附带信息**全部缺省化**(缺席合法):某些 infra 无法实现时,放弃该统计口径或查询时现算补齐——**降级不违约**,协议永不因缺省拒绝或报错(与 Raw 兜底同哲学)。

### 落点待 design 的细节

- core ↔ store 的 crate 落点:core 直依 store crate,或 core 定要求接口、infra 实现——与 crate-layout 依赖方向的关系 design 阶段定
- Spawn / 会话 actor 语义统一(CLI 每轮进程、SDK 常驻,actor 均为逻辑层);Stop 消息 vs 保留 RunHandle 薄包装(已验证竞态语义)
- delta 配对键(parent_tool_use_id vs 显式 correlator);ctx 首版字段面;未知字段忽略的演进策略

## 对话纪要

1. 触发:debug 页碎事件 bug 诊断(rig-core 源码实锤 `Text` 为增量、normalize 失配)→ 用户升维:包含 bug 重新思考 agent core 设计(会话为中心、instance 装配、token 级流式、store 记录、平台工具注入;loop/memory/LLM 归 infra)。
2. 探索中确认:碎事件根因(协议无增量语义 + 传输/落库粒度绑死)与探索新发现的链式丢上下文 bug(resume 转录取最新单 run 事件)同根——session 非一等公民;两 bug 共同构成本重设计的动机证据链。落盘待拍板:内核纯度 / 密封归属 / 切片 / 链式 bug 处置。
3. 拍板(2026-10-01):纯度接受(port 出 IO)/ 密封挂 session(转录单表)/ ②吸收①(验证 resume.rs 无折叠,碎事件毒化重建,密封纪律成为②的结构性必然;delta 本就是 run_session 核心交付)/ 链式 bug 直接进②不热修 / ③预留延后。切片收敛为单 change:会话内核。
4. 协议细化三轮(初案 P0-P4 → 五点修订:Actor 参考/转录出 P1/ctx/session 注入协议/P4 取消 → 终形收敛):P1 锁四条不变量(流式可实现/统计口径唯一/唯一标识上报/注入轮分离),机制自由;持久化定 write-through(收到即 store 原子落盘),查询 api 对账纠偏保留(纠偏设计获认可);统计字段与查询附带信息全缺省化,infra 实现不了即降级不违约。按用户裁定仅落盘终形结论。
