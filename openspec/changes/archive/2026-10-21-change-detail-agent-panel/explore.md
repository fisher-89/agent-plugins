# change 详情右侧通用 agent 面板（可折叠 · change 环境 · 新会话/历史会话）

workflow_type: requirement

## 1. 需求坐标

原始条目就在 `openspec/todo.md`：

> 【Desktop】change详情增加通用agent，在change环境下运行；通用agent可以开启新会话，恢复历史会话；

三条硬需求拆开看是三个独立问题域：

| 需求词 | 真正的不确定点 |
| --- | --- |
| 右侧 + 可展开收起 | 布局落点与折叠机制（前端为主） |
| 在 change 环境下运行 | "环境"到底是 cwd / 提示词上下文 / 工具面 / 三者叠加？ |
| 新会话 + 恢复历史会话 | change 维度下的会话归属模型（1:N）+ 当前会话指针存哪 |

前两个可以纯前端做完，第二个的真身几乎必然要碰后端签名。这个分界线是本需求最大的结构性判断点。

## 2. 现有积木地图（已经有的比想象中多）

```
复用面（零改动）
  useAgentSession            hooks/use-agent-session.ts     重放装载/挂流/发送/停止（会话基建单点）
  AgentSessionView           components/agent/...            双形态渲染（自持 | 注入）+ composer 插槽
  AgentInput                 components/agent/agent-input.tsx  输入 + permission 档 + 停发
  agentSessions(root, source, sourceRef, offset, limit)      会话清单（**已支持 sourceRef 精确过滤**）
  sessionDetail / agentSessionTranscript                     按 sessionId 直查重放
  ResizablePanelGroup/Panel  components/ui/resizable.tsx     react-resizable-panels v4，Panel 有 collapsible/collapse()/expand()/isCollapsed()
  ExploreDetailView 双栏范式  views/explores/explore-detail-view.tsx  左对话右预览 + ResizableHandle

缺口（要动的地方）
  agent_start 单 root 双用         cwd 与 store 寻址绑成同一个参数 → 双 root 拆分缺失
  前端 sourceRef 恒 null           use-agent-session.ts:459 / agent-transport.ts:325 两处硬编码
  ChangeRecord 无会话指针字段       ExploreRecord.session_id 的先例不能直接搬（explore 是 1:1）
  右侧面板无先例                   详情页现在是纵向单列
```

一个关键发现：**列表读取面已经免费**。`agentSessions` 的 `sourceRef` 参数在前端一直传 `null`（`views/agent/hooks/use-agent-run-history.ts:113`），后端 `query_sessions` 是按 `source_ref` **精确相等**过滤（`crates/infra/store/src/store.rs:770-778`）。只要发送侧把 `sourceRef` 写成 change id，列表就能按 change 圈出会话，后端零改动。

## 3. 线索一：「在 change 环境下运行」的三个层

### 3.1 cwd 层（真正的环境）——这里有硬缺口

change 流程的会话 cwd 是 **worktree**，而 store 身份锚恒 workspace root，这个"双 root 拆分"是既有设计（`src/commands/change_flow/mod.rs:111-118`，design D9）：

```
change flow：  exec_root = record.worktree ?? workspace_root   → SessionCtx.workspace_root（= agent cwd）
               store_root = workspace_root                    → stores.for_root(...)
               （infra/agent/src/worker.rs:47-49 只把 turn.root 塞进 SessionCtx）

agent_start：  root ──┬──→ stores.for_root(&root)    ← store 身份
                      └──→ SessionCtx.workspace_root ← cwd
               （src/commands/exec/mod.rs:79-96）
```

`for_root` 对任意路径都会**新建一个 workspace db**（`store.rs:2235-2249`，无注册校验）。所以「把 worktree 路径当 root 传给 agent_start」不会报错，而是静默分裂出第二个库——chat 会话落在 worktree 库里，从 workspace 侧永远查不到。这是必须显式决策的坑，不能靠"传 worktree 试试"。

⇒ 真环境方案：`agent_start` 增一个可选 `cwd`（或 `exec_root`）参数，store 寻址仍用 `root`；或新增 `change_agent_start(root, changeId, ...)` 薄命令在命令层做同样的双 root 解析（与 change_flow 同构，命令层已有一份 `is_dir` 探测 + 显式 Err 文案可抄）。

### 3.2 prompt 上下文层（最便宜、收益明确）

先例是 explore 的 stance 前导：`lib/explore-stance.ts:61` 在首轮拼 `stance + user input`，续话裸发（`use-explore-session.ts:129-131`）。change 版可拼：

```
你正在 change「<name>」（id <id>，<workflow_type> 工作流）的环境下工作。
- change 目录：<worktree 或 root>/openspec/changes/<name>/（proposal / design / tasks / specs …）
- 当前相位：<activePhase.phase>（attempt <n>）  · 或"未在运行"
- 纪律：不得调用 phase_log / phase_next / backtrack 等编排工具；落账由桌面代写
```

进料全部已在手：`ChangeDetail{name,title,workflow_type?,pipeline,activePhase,worktree}` + `refreshTick` 下的实时 `detail`。前导模板放 `lib/change-stance.ts`，与 explore-stance 对称。

### 3.3 工具面层

`assemble_injections` 只特判 `source == "explore"`（六工具收窄 + `SessionToolset::Explore`），其余来源恒 default 七工具（`infra/agent/src/compose.rs:14-40`）。change chat 大概率就要七工具（含 write，因为在 worktree 里干活），也就是**零后端改动**——除非产品要求"只读 change 环境"。

## 4. 线索二：会话归属模型（新会话 / 恢复历史）

explore 是 **1:1**：建档同事务预建空会话行 + 回写 `ExploreRecord.session_id`（`model.rs:216-218`、`store.rs:271-292`、`ensure_explore_session`）。change chat 是 **1:N**（每次都"新会话"，旧的都要能翻回来），所以 explore 那套指针只能承载"当前"，不能承载"集合"。

四种方案对比：

```
A. 纯前端，无 store 改动
   source='change', sourceRef=<change id>   ← 写侧需穿 sourceRef（transport 3 处）
   列表 = agentSessions(root,'change',changeId,...)   默认恢复 = 列表首条（updated_at desc）
   新会话 = 本地 reset + sessionId=null（首条消息才真正落库建会话）
   代价：刷新页面后"当前会话"回到最近一条；空新会话重载即丢失（无害）

B. A + ChangeRecord.chat_session_id 指针
   新会话 = 新命令（建/取会话 id 并回写指针），恢复 = 指针直查
   好处：与 explore "来源侧持会话 id 并直接给出" 的纪律完全一致
   代价：store 模型 additive 字段 + 写面 + 升级默认值 + 命令面各一处

C. 新 source 词（'change-chat' / 'chat'）
   语义最干净（debug 页 'change' 筛选不被聊天污染）
   代价：debug 页来源筛选三态要扩成四态；spec 里 source 词表要扩；
        若还要不同工具面，compose 再多一个分支

D. 右侧面板不落库，历史完全走调试页
   等于不做"恢复历史会话"——不符合需求
```

**spec 张力**：`openspec/specs/desktop-agent-execution/spec.md:46` 明文「前端寻址与重放 SHALL 以 sessionId 直查，**MUST NOT 经 source / source_ref 反查会话**（sourceRef 反查取最近一条的路径退役）」。

- 用 `agentSessions(source, sourceRef)` 拿**列表**（用户显式选历史）是探测/清单语义，与调试页的来源筛选同款（spec:644 明确允许）；
- 但"打开面板即自动恢复最近一条"就是被退役的那条路径。所以自动恢复若要做，方案 B 的库内指针才是合规解，方案 A 的"列表首条"是擦边球。

这一点建议直接找用户拍板，因为它决定要不要动 store。

## 5. 线索三：前端落点

### 5.1 布局

现在详情页是纵向单列，`header` + 告警 + 动作面板 + `flex-1` 流程区（`change-detail-view.tsx:280-302`）。加右栏要把流程区挪进横向 split：

```
┌─ DetailHeader（返回 / 刷新 / 标题 / 运行徽标 / worktree）─────────────┐
├─ RefreshStaleNote（失败窗）──────────────────────────────────────────┤
├─ RunControlPanel（发起/停止/确认/ask）───────────────────────────────┤
├─ ResizablePanelGroup(horizontal) ────────────────────────────────────┤
│  ┌ Panel(流程区 flex) ───────────────┐ │ ┌ Panel(agent, collapsible) ┐│
│  │  FlowSection（react-flow 画布）    │◀┼▶│ 会话元信息 / 对话 / composer ││
│  │                                    │ │ 新会话 · 历史会话 · 停止     ││
│  └────────────────────────────────────┘ │ └────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────┘
```

- 折叠：`<Panel collapsible collapsedSize="0" minSize="20" panelRef={...}>` + `collapse()/expand()/isCollapsed()` 已在依赖里（`node_modules/react-resizable-panels/dist/react-resizable-panels.d.ts:223-309`）；收起态建议留一条竖窄条（图标 + 竖排文字）当展开入口，或在 header 放一个开关按钮。
- 注意外壳：`app.tsx` 把路由装进 `<ScrollArea>`，explore 双栏在同样外壳里工作正常（先例可用）。
- 文件纪律：`change-detail-view.tsx` 已 388 行，`max-lines-per-function` 50/80（tsx）⇒ 面板必须独立组件 + 独立 hook 文件（`views/changes/flow/` 或新 `views/changes/agent/`）。

### 5.2 一个必须避开的性能坑：refreshKey 重放风暴

`useAgentSession` 的重放 effect 依赖 `refreshKey`（`hooks/use-agent-session.ts:340-369`），每当它变化且**当前没在挂流**就整轮重放（`sessionDetail` + 整份转录）。而详情页的 `refreshTick` 在 run 期间由 `SessionEvent` 通知驱动（`use-change-detail.ts` 300ms 去抖，`orchestration/state.rs:330` 每密封事件一条 notice）。把 `refreshTick` 直接当 `refreshKey` 传给 chat 会话 = 一轮 agent 跑下来重放几十次。

现有只读转录面板（`SessionTranscriptPanel`）收 `refreshKey` 是合理的（它没有自己的流）；**交互式 chat 会话不该收 detail tick**——它自己有订阅（`useAgentSession` 的 `running → resumeStream`），explore 也是完全不传 `refreshKey` 的注入形态。

### 5.3 与右侧抽屉的并存冲突

流程节点/列头的右侧抽屉是 960px 右贴边 Sheet + 遮罩（`standard/drawer.tsx`、spec `desktop-change-flow-view:124` "右侧抽屉单一交互入口"，且明令"MUST NOT 出现两套并存的展开交互"）。现在右侧再加一个常驻面板：

```
抽屉打开时 → 面板怎么办？
  (a) 自动收起（面板状态与抽屉联动）
  (b) 面板保持，抽屉叠在其上（960px + 85vw 上限，窄窗口会完全盖住面板）
  (c) 抽屉与面板共用同一右侧槽位（抽屉退化为面板内的 tab）← 改动最大、最"一体"
```

需要拍板。我倾向 (a)：一行 `if (selection !== null) panel.collapse()` 的成本，语义也不冲突。

## 6. 线索四：边界与并发

1. **归档 change**：归档链的 Cleanup 步会 `remove_worktree`（`walker.rs:2856-2892`），但 `ChangeRecord.worktree` 不会被清空（`store.rs:1509-1521` 建档写入、归档只翻 status）⇒ 归档后 worktree 是**悬空路径**。面板要么 `is_dir` 探测后回落 workspace root（change_flow 同款一句探测），要么归档 change 禁用发送只留历史浏览。
2. **与 flow 并发写同一 worktree**：flow 的 executor 正在 worktree 改代码，chat agent 也在同一目录改 —— 无锁、无互斥。是否"run 期间禁发 chat"是产品决策（现有 `RunControlPanel` 的 `activeRun` 就在手，禁发判据一行可取）。
3. **停止语义**：`agentStop(root, sessionId)` 已有，chat 自己的 stop 与 flow 的 stop 互不相干（不同 session id、不同注册表条目）✓。
4. **多 change 切换**：面板状态必须挂在 URL id 上（`RootSwitchSuppress` 已经把根切换处理干净），面板本地态（当前会话 id / 折叠态）在 id 变化时应重置或按 change 记忆。

## 7. 待拍板问题（建议按这个顺序问）

1. 「change 环境」是否等于 **worktree 为 cwd**？是 → 动 `agent_start` 签名（双 root）；否 → 纯前端 + 提示词前导即可交付。
2. 自动恢复"上次会话"要不要做？要 → 需要 `ChangeRecord.chat_session_id`（store additive）。不要 → 面板默认空白新会话，历史靠列表点选。
3. 会话来源词：复用 `'change'`（sourceRef 精确到 change id）还是新词 `'change-chat'`？（决定调试页筛选与 spec 措辞）
4. run 期间是否禁发？归档 change 是否只读？
5. 抽屉打开时面板收起 or 叠放？

## 8. 建议的下一步探查

- 细读 `openspec/specs/desktop-change-flow-view/spec.md` 与 `desktop-agent-execution` 的模块契约段，确认"新增面板"要写的 spec delta 边界（可能新建 capability 或扩 flow-view）。
- 验一次 `agentSessions(root,'change',<changeId>)` 在真实库上的返回（现有 change 流程会话的 sourceRef 是四段 `{id}/{phase}/{role}/{attempt}`，与 chat 的 `{id}` 精确匹配天然不撞）。
- 若走双 root：对照 `WalkerRequest.root/main_root` 的双 root 不变量，设计 `agent_start` 的 `cwd` 参数边界（worktree 必须 `is_dir`、store 恒 root）。

---

## 9. 决策回填（5 问已答 → 设计收敛）

用户拍板：①开放参数 ②不自动恢复 ③会话走 sessionId 直查 ④运行中禁发 + 归档只读 ⑤不用抽屉、agent 与面板分栏放左侧。

### 9.1 Q1｜流程怎么指定 worktree，参数能否开到通用 agent 接口

**完整链路**（4 跳，每跳都有代码位置）：

```
① 建域（create_change 命令层）
   worktree_dir(data_root, root, name)
     = {DATA_DIR}/worktrees/{workspace身份段}/{name}       infra/vcs/src/lib.rs:38-45
   write::create(main_root, worktree_root, …)              core/workflow/src/write/create.rs
     ├ git worktree add + 分支 change/<name>（HEAD 基线）
     └ ChangeRecord.worktree = Some(<绝对路径>)            ← 执行锚在此落库（DB 先写，fs 后建）
   DATA_DIR = ~/.dev-team/                                 src-tauri/src/main.rs:18

② 发起 run（change_flow_start 命令层）                     src/commands/change_flow/mod.rs:111-118
   exec_root = record.worktree（且 is_dir）；worktree 非空但缺失 → 显式 Err
               record.worktree === null（legacy）→ root
   store     = stores.for_root(root)   ← 身份锚恒 workspace root（双 root 不变量）

③ RunRequest { root: exec_root, main_root: root }          core/orchestration/src/walker.rs:93-107

④ walker → WorkerTurnRequest.root（= exec_root）
     → SessionCtx.workspace_root                           infra/agent/src/worker.rs:47-49
        这一个字段同时决定三件事：
          · 工具路径沙箱根          sdk/sandbox.rs:62  check(root, raw)
          · bash 的 current_dir     sdk/bash.rs:30
          · 每轮重读 <cwd>/AGENT.md sdk/loop.rs:170 + sdk/preamble.rs
```

结论：**worktree 在流程里的唯一身份就是 exec root（cwd）**，除 cwd 外不承担任何库身份 ⇒ 把它开成 agent 接口参数与流程语义完全同构。

**参数形态**：`agent_start` 尾部追加 `cwd: Option<String>`（None → root）。放在最后，前端既有位置索引 1–7 全不动，只多 `[8]`：

```
agent_start(on_event, root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd)
                                                                                        ↑ 新增（index 8）
root   双职：store 身份 + 默认 cwd   （不动，store 半边恒 workspace root）
cwd    仅进 SessionCtx.workspace_root（新）
```

三个必须一起处理的坑：

1. **store 身份分裂（最危险）**：`for_root` 对任意路径都会新建 db（`store.rs:2235-2249`，无注册校验）。若把 worktree 当 `root` 传，会静默铸出 `~/.dev-team/workspaces/{worktree身份段}.redb`，chat 会话落进去后从 workspace 侧永远查不到。⇒ `cwd` 只喂 SessionCtx，`root` 恒 workspace root。
2. **沙箱根随之收缩**：cwd = worktree 时全部路径工具被限在 worktree 内（`sandbox::check` 前缀校验）。worktree 是完整 checkout（含 `openspec/` 全套：changes 目录、specs 基线、AGENT.md），够用；但**看不到主工作区的未提交改动**——这是"change 环境"应有的边界，建议在面板文案里点明。
3. **cwd 缺失校验**：cwd 非空时做 `is_dir` 探测 + 显式 Err（复用 change_flow 文案"worktree 目录不存在（可能已被手动删除）"），否则错误深埋进引擎工具调用。

**续话一致性缺口（需知会）**：cwd 没有落 `SessionRecord.config_snapshot`，从调试页 'change' 筛选打开同一 chat 会话续话时不会带 cwd → 工具跑在主 root。收口方式：
- (i) 来源侧恒带 cwd（change 详情页恒取 `detail.worktree`）——调试页续话仍是缺口；
- (ii) 命令层按 `SessionRecord.provenance.source_ref` 反查 change 记录 worktree 兜底（隐式，但与"会话归属决定环境"自洽）。
建议先 (i)，(ii) 留作后续统一。

### 9.2 Q2 + Q3｜不自动恢复；会话全走 sessionId 直查 —— change 侧会话 id 盘点

**先答"change 中记录通用会话 id 有哪些"：ChangeRecord 里一个都没有。** change 域的会话 id 全在旁支表：

| 落点 | 字段 | 语义 | 位置 |
| --- | --- | --- | --- |
| `ChangeRecord` | — | **零会话字段** | model.rs:599-630 |
| `RunNodeRecord` | `session_id` | 步级会话（仅 WorkerAgent 步，工具步 None） | model.rs:963 |
| `PhaseRecord` | `executor_session_id` / `evaluator_session_id` / `decision_session_id` | 相位三槽位（出线为 `AttemptRecord` 同名三字段） | model.rs:787-791 |
| `SessionRecord` | `source` / `source_ref` | 反向归属：流程会话 `source="change"`、`source_ref="{changeId}/{phase}/{role}/{attempt}"` | walker.rs:3399-3405 |
| `ExploreRecord` | `session_id` | explore 侧单指针（非 change；1:1 先例） | model.rs:218 |

⇒「通用会话的历史集合」要么新增显式字段，要么用既有的 source/sourceRef 清单查询。**推荐后者**：

- 零库模型变更（`ChangeRecord` 现为 `native_model(id=9, version=4)`，加字段要 v4→v5 升级链 + 双向 From + 中性快照 + DTO + 组装，五处联动）；
- store 自己就有先例：`ensure_explore_session` 第 ② 步按 source+source_ref 扫会话兜底（`store.rs:1171-1180`）；
- 与 spec 不冲突：禁令针对「前端寻址与重放」，而"列一张清单让用户挑"是调试页来源筛选同款语义（`desktop-agent-execution:644` 明文允许）。选中后一律 `sessionDetail` / `agentSessionTranscript` 以 **sessionId 直查** —— 正是 Q3 口径。

**9.2.1 推荐落法（零 store 变更）**

```
写侧  agent_start 的 source='change'、source_ref=<change id>（两段 vs 流程会话四段，天然不撞）
      ↑ 打开前端硬编码的 sourceRef: null（use-agent-session.ts:459 / agent-transport.ts:325）
      transport 的 readSessionParams 增读 body.sourceRef（现为常量 null 直传）

读侧  新增薄命令 list_change_chat_sessions(root, id, offset, limit) -> SessionPage
      = session_query(source="change", source_ref=id)   ← source 词常量收在命令层，前端零词汇

新会话  本地 sessionId=null（空态）→ 首条消息走 agent_start New（后端铸 id）
        → early-resolve 轮行到达后刷新清单，新会话自然出现

恢复历史 清单点一条 → setSessionId → useAgentSession 重放（sessionDetail + transcript）
```

打开面板即空态 = Q2「不要自动恢复」自然满足；一次 New 才落一条会话行，零空会话堆积。

**9.2.2 备选落法（change 记录显式持 id 列表）**

`ChangeRecord.chat_session_ids: Vec<String>`（最近在前）：
model version 4→5 + `from = ChangeRecordV4` + 反向 From（`ExploreRecord` v3→v4 是同款先例，model.rs:340-405）
→ `ChangeStateRecord` 加字段（core/workflow/src/state.rs:71-94）+ `change_state()` 映射（store.rs:100）
→ `ChangeDetail` 加字段（core/workflow/src/queries/detail.rs:191-211）+ 组装
→ 写命令（新会话预建 + append，形如 `ensure_explore_session`）
→ 前端多一个 detail 字段 + 一次 refresh

**成本**：推荐落法 ≈ 1 新命令 + 2 处前端透传；备选 ≈ 1 次库版本升级 + 4 层字段穿透 + 1 写命令。

**两者共同要改一行 spec**：`desktop-agent-execution:46 / 552` 的「`source_ref` 前端恒传 null」需改为「可携带来源侧引用（change id），仅用于归属记录与清单筛选，MUST NOT 用于寻址/重放」。

### 9.3 Q4｜禁发 + 只读的状态矩阵

```
detail.source   activeRun   本会话 running   可发  可开新会话  可切历史  理由
'active'        非 null      —               ✗     ✗           ✓        流程在跑：避免同 worktree 并发写
'active'        null         true            ✗     ✗           ✓        AgentInput 既有 running 禁发
'active'        null         false           ✓     ✓           ✓
'archive'       —            —               ✗     ✗           ✓        只读：worktree 已被 Cleanup 移除
```

实现口径：
- 「运行中禁发」= `activeRun !== null` 与 `session.running` 取或（`activeRun` 在 DetailLoaded 已在手）；
- 「只读」= `detail.source === 'archive'`；
- `AgentInput` 需新增 `disabled`（或 `readOnly`）态：textarea 禁用 + 发送禁用 + 禁用理由文案（"流程运行中" / "已归档 change 只读"）。现有 `running` 只管发送按钮，不够用；
- 面板空态（sessionId === null）只给「开始新会话」入口、输入框禁用——避免"打了字发不出去"。

### 9.4 Q5｜布局：左 agent + 右详情（与探索页同构）

```
ChangeDetailView
┌ DetailHeader（返回 / 刷新 / 标题 / 状态 / worktree / 「隐藏通用 Agent」开关）─────────┐
├ RefreshStaleNote（失败窗，整宽）──────────────────────────────────────────────────────┤
└ ResizablePanelGroup orientation="horizontal"（flex-1 min-h-0，与 explore 同款）────────┤
   ┌ Panel 左：通用 Agent（defaultSize=30 minSize=22 collapsible 48px 窄条）┐│┌ Panel 右──┐
   │ 头：通用 Agent · [新会话] [历史 ▾] · 只读/禁发提示行                    │││ RunControl │
   │ AgentSessionView（注入形态，composer = ChangeChatComposer）             │││ FlowSection│
   │                                                    〈ResizableHandle〉 │││ …          │
   └────────────────────────────────────────────────────────────────────────┘│└────────────┘
```

收起能力现成：`Panel collapsible` + `panelRef.collapse()/expand()/isCollapsed()`（react-resizable-panels v4，d.ts:223-309），`collapsedSize` 支持 px（给 48px 窄条）或直接 0（开关放 DetailHeader，更简单可测）。

**硬冲突（必须先定）**：现有抽屉是**模态浮层** —— `StandardDrawer` → `DrawerOverlay` 是 `fixed inset-0`、`modal=true`（components/ui/drawer.tsx:40-70），打开即遮蔽整页，连左侧 chat 输入一起挡住。所以「不使用抽屉」有两种读法：

```
读法 A（保守）：抽屉保留，只在左侧加 agent
   零 spec 改动（保留「右侧抽屉单一交互入口」requirement）
   代价：点节点看转录时左侧 chat 被遮罩挡住 —— 两个 agent 会话无法并用

读法 B（激进）：抽屉退役，右区改为「上流程图 / 下选中详情」两段
   ┌ 右区 ──────────────────────────┐
   │ FlowSection（上，flex-1）       │
   ├────────────────────────────────┤
   │ 选中详情面板（下，可折叠）      │ ← 原抽屉内容（列头素材 / 会话转录 / stdout）原样搬入
   └────────────────────────────────┘
   优点：与探索页完全同构（无浮层），chat 可与任意节点会话转录并读
   代价：改 desktop-change-flow-view 的「右侧抽屉单一交互入口」requirement；
        change-detail-view.test.tsx（2277 行）抽屉相关断言（如 446 行挂载/卸载）需重写
```

倾向读法 B —— 它才是"与探索页体验一致"的完整形态，并顺带消解"两个 agent 会话不能并读"的尴尬；代价是一次明确的 spec 修订。

**已定：读法 A（保留抽屉）**。但"遮罩遮蔽左侧 chat"有零成本的化解手段 —— 抽屉库本身支持非模态：

```
components/ui/drawer.tsx:99   {modal === true && <DrawerOverlay />}          ← 遮罩仅模态时渲染
components/ui/drawer.tsx:102  data-modal={modal}
components/ui/drawer.tsx:103  className="pointer-events-none fixed inset-0 … data-[modal=true]:pointer-events-auto"
                                        ↑ 非模态时 Viewport 不吃指针事件，Popup 自身 pointer-events-auto
```

即 `modal={false}` + `swipeDirection="right"` ⇒ **无遮罩、左侧 chat 全程可交互**，抽屉仍停在右侧浮层、仍由 `selection !== null` 驱动、仍是唯一交互入口。改动面极小：

```
StandardDrawer 现签名（components/standard/drawer.tsx:14-23）不透传 modal
  → 增 modal?: boolean（缺省 true，既有调用零回归）
  → DetailDrawer 传 modal={false}（views/changes/flow/detail-drawer.tsx:144-160）
```

两个随之而来的小账：
- **无遮罩即无「点遮罩关闭」**：关闭走页脚「关闭」按钮（`StandardDrawer` 的 DrawerFooter 已有）+ Esc；spec `desktop-change-flow-view:124` 的「遮罩点击关闭」条款需改为「非模态形态：无遮罩，关闭经关闭按钮 / Esc」。
- **窄窗口遮挡**：960px + `max-w-[85vw]` 仍可能盖住左侧 chat（这在读法 A 下是接受的取舍 —— 需要并读时收起抽屉即可）。

优先验一条：base-ui Drawer `modal={false}` 下 Esc 是否仍能关闭（预期由 dismiss 逻辑支持，若非则只剩关闭按钮，可接受）。

## 10. 收敛后的改动清单（定稿：读法 A + 9.2.1）

**后端（Rust）**
1. `src/commands/exec/mod.rs`：`agent_start` / `agent_start_with` 增 `cwd: Option<String>`，`is_dir` 校验，`SessionCtx.workspace_root = cwd.unwrap_or(root)`；STORE 半边恒 root。
2. `src/commands/exec/mod.rs`：新增 `list_change_chat_sessions(root, id, offset, limit)`（薄包装 `session_query(source="change", source_ref=id)`）+ `all_commands!` 登记。
3. 生成绑定：`pnpm run bindings:export`；`bindings:check` 会盯前端同步。

**前端**
4. `lib/agent-transport.ts`：`readSessionParams` 增读 `cwd` / `sourceRef`；`invokeStart` 追加 `cwd`；`AgentStartArgs` 索引魔法（1/2/3/4/5/7）改为具名解构，避免加参数时错位。
5. `hooks/use-agent-session.ts`：`UseAgentSessionParams` 增 `cwd?` / `sourceRef?`，`sendMessage` body 透传。
6. `views/changes/agent/`（新目录）：`change-chat-panel.tsx`（会话头 / 新会话 / 历史清单 / 只读与禁发提示）+ `change-chat-composer.tsx`（薄适配 AgentInput，`idPrefix="change-chat"`）+ `hooks/use-change-chat.ts`（当前 sessionId 本地态 + 清单取数 + ensure 语义）。
7. `components/agent/agent-input.tsx`：增 `disabled` 态与禁用理由文案。
8. `views/changes/change-detail-view.tsx`：`DetailLoaded` 改 `ResizablePanelGroup` 两栏（左 agent / 右 stale note + 动作面板 + 流程图）；`components/standard/drawer.tsx` 增 `modal?: boolean`（缺省 true），`detail-drawer.tsx` 传 `modal={false}` 保左侧 chat 可交互；`max-lines-per-function` 要求面板独立成文件。
9. 测试：新增面板交互测试（空态 / 新会话 / 历史切换 / 只读 / 禁发）；`detail-drawer` 增 `data-modal="false"` 断言；`change-detail-view.test.tsx` 既有抽屉断言（446 行挂载 / 卸载）保持不变。

**spec（openspec delta）**
10. `desktop-agent-execution`：`agent_start` 参数面 + `source_ref` 恒 null 条款修订 + cwd 语义。
11. `desktop-change-flow-view`：左侧通用 Agent 面板（新 requirement）；「右侧抽屉单一交互入口」补非模态形态（无遮罩、关闭经按钮 / Esc、上游选择与内容分派不变）。
12. 新 capability（建议 `desktop-change-chat`）：面板状态矩阵（禁发 / 只读）、会话清单命令、新会话与历史切换语义、cwd 语义与 worktree 边界。

**风险点（复述）**
- 忘改 `root` 语义 → 静默分裂 workspace 库（最贵的一次事故）；
- 把 detail 的 `refreshTick` 当 `refreshKey` 传给 chat 会话 → run 期间重放风暴；
- 抽屉虽非模态，960px 宽在窄窗口仍会盖住左侧 chat（读法 A 的既定取舍）；
- `native_model` 本次不动（9.2.1 零模型变更），故无非自描述字段演进风险。

## 11. 定稿（5 + 2 问全数闭环）

| # | 问题 | 定案 |
| --- | --- | --- |
| 1 | 「change 环境」是否 = worktree 为 cwd | **是** → `agent_start` 尾部增 `cwd: Option<String>`（index 8），store 半边恒 root |
| 2 | 自动恢复上次会话 | **不要** → 打开即空态（sessionId=null），一次 New 才落会话行 |
| 3 | 会话记录 / 查找 | **ChangeRecord 零会话字段**；走 `source='change'` + `source_ref=<change id>` 归属 + 清单命令；选中后 `sessionId` 直查重放 |
| 3b | 归属落法 | **9.2.1 零 store 变更**（sourceRef 归属 + `list_change_chat_sessions` 薄命令） |
| 4 | 运行中 / 归档 | **运行中禁发**（`activeRun !== null` 或 `session.running`）；**归档只读**（`source === 'archive'`）；禁发态连「新会话」一起禁，只留历史切换 |
| 5 | 布局 / 抽屉 | **不用抽屉承载 agent**；左 agent + 右详情两栏（`ResizablePanelGroup`，与探索页同构，agent 可 collapsible 收起）；**右侧抽屉保留（读法 A）**，改 `modal={false}` 使左侧 chat 全程可交互 |

定稿后的形状：

```
ChangeDetailView
┌ DetailHeader（返回 / 刷新 / 标题 / 状态 / worktree / 「通用 Agent」显隐开关）─────────┐
├ ResizablePanelGroup(horizontal) ────────────────────────────────────────────────────┤
│ ┌ 左：通用 Agent（defaultSize 30 / minSize 22 / collapsible）┐│┌ 右：详情 ─────────────┐
│ │ 头：[新会话] [历史 ▾] · 禁发/只读提示行                    │││ RefreshStaleNote      │
│ │ AgentSessionView（注入形态 · composer=ChangeChatComposer） │││ RunControlPanel       │
│ │                                                            │││ FlowSection           │
│ └────────────────────────────────────────────────────────────┘│└───────────────────────┘
└──────────────────────────────────────────────────────────────────────────────────────┘
        ↑ 点节点 / 列头 → 右侧非模态抽屉浮层（无遮罩，左侧 chat 仍可输入）
```

### 还剩两处实现前要验的事实（非决策，落地时顺手验）

1. base-ui Drawer `modal={false}` 下 Esc 是否仍关闭（否则仅剩页脚按钮，可接受）。
2. `agentSessions(root,'change',<changeId>)` 在真实库上确实只命中 chat 会话（流程会话 sourceRef 是四段、精确匹配天然不撞）——建议写一条 store 侧断言固化这个不撞性。

### 下一步（若继续）

按既有 OpenSpec 流程产出 change（`openspec/changes/<name>/`）：proposal + design（DS1 双 root / DS2 会话归属 / DS3 面板布局与抽屉非模态）+ spec delta（`desktop-agent-execution`、`desktop-change-flow-view` 或新 capability `desktop-change-chat`）+ tasks。需要我起草就直说。
