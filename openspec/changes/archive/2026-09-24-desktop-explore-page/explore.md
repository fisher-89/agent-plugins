# Desktop Explore 页面探索笔记

日期：2026-09-24（/dev-team:openspec-explore 会话）

## 目标

desktop 增加 Explore 页面，对应 workspace 下的 `openspec/explores/`；支持查询 explore 笔记与 agent 交互。agent 复用现有调试接口（agent_start 执行链路），上下文参考 `plugins/dev-team/skills/openspec-explore` 的 stance。UI 使用 shadcn 官方 message-scroller、questionnaire 等组件。

## 数据模型（已确认）

- explores 是**扁平的 markdown 文件集合**：`openspec/explores/*.md`，文件名（去 `.md` 扩展名）即 explore 名称。不像 changes 是目录树。
- `foundation::layout::resolve` 已预留 `explores_root`（全包唯一目录知识点），但目前零查询使用。

## 现状盘点

已就位：

- `Layout.explores_root` 已解析 `openspec/explores`
- agent 执行链路完整：`agent_start`（Channel 实时流）+ `agent_runs` / `agent_run_events`（历史重放）；前端 `useAgentRun` / `useAgentRunHistory` 可复用
- `AgentEvent.Message.blocks` 四变体（Text / Thinking / ToolUse / ToolResult）天然映射 chat bubble
- `MarkdownDocRenderer`（react-markdown + remark-gfm）可复用渲染 explore 正文
- `components.json` 在位（new-york / Tailwind v4），可直接 `pnpm dlx shadcn@latest add`
- 路由模式 `/changes` + `/changes/:name`（URL 承载选中，无本地 state 双轨）可平移为 `/explores` + `/explores/:name`

空白：

- `workflow::queries` 只有 changes 域（list / detail / locate 全是 change 语义），explores 无查询
- 前端无 /explores 路由、无侧栏入口
- 无 chat 类组件（现有 ui/ 九件套均为静态展示件）
- explore 模式 skill 上下文（stance）无注入通道

## 页面形态草图（布局已拍板：左聊天右预览）

```
/explores                         /explores/:name（双栏）
┌──────────────┐                 ┌────────────────────────┬────────────────┐
│ 探索笔记清单  │                 │ 对话区                  │ explore.md     │
│  (DB 清单)   │    click ─────▶ │ MessageScroller         │ 实时预览       │
│              │                 │  ├ user bubble          │ (MarkdownDoc-  │
│              │                 │  ├ agent bubble         │  Renderer)     │
│              │                 │  │  Text→markdown       │      ▲         │
│              │                 │  │  Thinking→折叠       │      │ watch   │
│              │                 │  │  ToolUse→卡片        │        通道监听 │
│              │                 │  └ composer             │        文件变化 │
│              │                 │    (输入+env/perm档位)  │                │
└──────────────┘                 └────────────────────────┴────────────────┘
```

- 侧栏「页面」组：[变更] [探索]（探索与变更同属 workspace 域内容页）
- 双栏可拖分界（resizable）；右侧预览由 watch 通道驱动实时刷新

## 查询侧设计（第三轮修订：清单 DB 化）

- 页面清单来自 store 的 `ExploreRecord`（非实时扫目录）；内容读取仍读盘（`read_explore`，name 单分量防穿越即可，无需 active/archive 两棵树定位）
- workflow crate 查询层角色收缩：`read_explore`（读单文件内容）+ 新建导入扫描（列目录中未绑定的 `*.md`）；不再提供页面清单查询
- 命令层薄包装，照 queries/mod.rs 既有纪律（无状态、blank root 空结果）；纯读，不硬塞 change 域的 ArtifactEnvelope 信封

## 组件来源（已核实为官方件）

- **message-scroller**：2026-06 官方 chat primitives 家族（配套 `message` / `bubble`）。只管滚动行为（turn 锚定、流式跟随、历史前置不跳），消息内容/状态/传输留在应用层——匹配「事件流已有，只缺呈现」。`pnpm dlx shadcn@latest add message-scroller message`
- **questionnaire**：2026-08 官方件，文档定位即 "agent clarification prompts"，与 `AskUserQuestion` 工具入参结构（questions / options / multiSelect）几乎一一对应。`pnpm dlx shadcn@latest add questionnaire`

## 决策记录

### A. 多轮对话（已拍板：多轮，走 resume 扩展）

- 选定多轮对话路线：**扩展调试接口加 resume**——`agent_start` 增可选 `resumeSessionId`，flags 组装 `--resume <id>`，会话延续靠 `AgentRunRecord.session_id`（该字段本就是「续会话未来账的信封预留」）。
- 定位为「复用调试接口的自然延伸而非破坏」：向后兼容（不传 resume 即现有 one-shot 行为）。
- 连带影响：张力 C（Questionnaire 回程）机制随之确定为「提交答案 → resume 续话」，不再需要答案文本化拼 prompt。

### 连带确定：Questionnaire 回程机制

提交 Questionnaire = 以 resume 续话发起新 run，答案作为该轮 user 消息内容。（后续 C 拍板 MVP 不做 questionnaire，此机制留作二期接线方式。）

### B. stance 注入 + 会话保持（已拍板：内置前导模板 + store 成对持久）

- desktop 内置精简 explore stance 前导模板，发送时拼接进 prompt（接受与 SKILL.md 双源漂移的代价，模板归属 desktop 包）。
- 会话落 store 既有链路（AgentRunRecord + 事件落库），新增「explore ↔ 会话链」关联：**文档（磁盘 md）与会话（store 事件）成对持久出现**——重开 explore 时双恢复：文档从磁盘读，对话重放该会话链的落库事件（agent_run_events 已支持），续话走 A 的 resume。

### C. questionnaire 不进 MVP（已拍板）；UI 引入清单

- 首版不做 questionnaire：`AskUserQuestion` 的 ToolUse 先渲染为可读卡片（问题/选项静态呈现），答案经 composer 文本输入、走 resume 续话。二期再评估接 questionnaire。
- 需引入的 UI 件（官方 registry，`pnpm dlx shadcn@latest add`）：
  - `message-scroller` + `message`（必需：对话区滚动与气泡；`bubble` 按需）
  - `resizable`（推荐：左右分栏拖分界，基于 react-resizable-panels）
- 不引入：`questionnaire`（二期）、textarea 组件（沿用 AgentRunForm 的原生 styled textarea 模式）、工具卡片（现有 badge / button / details 组合足够）。

### D. 文件 watch 通道（已拍板：通用抽象，订阅单文件、只推修改信号、前端防抖）

- 抽象一个通用「watch 文件变化」推送通道（Tauri Channel），与执行流通道并列，成为第二个被认可的推送语义（此前全应用纪律是显式取数、无 watch 无轮询）。
- **订阅单文件**（当前 explore.md），**通知只携带「文件被修改」信号、不含内容**，前端防抖后经 `read_explore` 显式拉取刷新——通道是**失效信号通道而非数据通道**，数据面维持既有取数纪律（比执行流通道的口子更小）。
- 防抖在前端做，后端只透传修改事件（无去抖逻辑下沉）。
- 边界：订阅目标可能尚未存在（新话题未落盘）或已被删除（孤儿记录）——预览走空态 + run 终态定点重读，文件出现/重建后（重）挂 watch（design 细化）。
- 平台备注：本应用目标平台 Windows 上 notify 对文件路径底层经父目录实现，编辑器 rename-replace 原子保存不丢事件；类 Linux/inotify 平台单文件 watch 有 rename 失效陷阱，将来跨平台时再议。
- 分层纪律不变（core 禁 Tauri）：Channel 包装留在壳层命令。

### 会话恢复语义（已拍板：单链）

- 每个 explore 单会话链，MVP 不提供「新会话」动作；打开 explore 默认续最近链，无链即起链（首条 run 创建链）。

### store 关联形态（已拍板：统一 run 表 + 联动字段，不按来源分表）

背景：未来 agent 会话记录来源主要是 explore / change / 调试 / 架构，还有其他扩展，量会很大。

选型：**增加字段路线**——`AgentRunRecord` 保持单表，加判别与联动字段；不按来源分表。理由：

1. 共同面占绝对主导：13 个字段 + 整条事件落库/重放管线全部来源无关；分表将 fork store API / 命令层 / 前端 hooks ×N
2. 事件表锚定统一 run 身份：`AgentEventRecord` 二级索引是全局 run_id，分表要么 id 撞名空间要么仍靠全局 id（分表只剩装饰）
3. 跨源查询是真实需求：调试页本就是全局视图（应能看到 explore 发起的 run），未来成本/轮数统计天然跨源；分表后跨源视图 = N 路 union
4. 量级大头在事件记录而非 run 行，分表不改善量级；本机桌面规模下二级索引范围扫描绰绰有余（现状 list_agent_runs 即全表读 + 内存排序）
5. 演进不对称：加字段 = native_model 版本原地演进（机制现成，store 无 legacy 包袱）；分表后合并近乎不可逆。未来某来源长出独特字段面时再拆专属表仍是保留选项

字段草图（进 design 细化）：

- `source`：受控字符串（`debug` | `explore` | `change` | `archi` | …），即 model.rs 预留注释的 `source` 联动字段
- `source_ref: Option<String>`：来源内定位（explore 指向 `ExploreRecord` 主键——已拍板不再依赖文件名；change 名 / workflow phase 等未来再加）
- `parent_run_id: Option<i64>`：resume 链显式指针（每次 resume 产生新 run 行；显式指针比依赖 session_id 相等更稳，规避 CLI resume 的 fork 语义歧义）
- 单链还原：按 `(source="explore", source_ref=name)` 取最新为链头，沿 parent_run_id 回溯历史
- 二级索引形态（单字段 source vs 复合打包）与调试页是否加 source 徽标，design 定

### explore 清单 DB 化（已拍板：ExploreRecord + 绑定 agent + 新建导入/新话题）

- 清单从「实时扫目录」改为 store 记录：新增 `ExploreRecord`（native_model 新 id）——清单条目、workspace 归属（root 维度，同 `WorkspaceRecord`）、与 agent 会话链的绑定、时间戳。
- **数据三分**：`ExploreRecord`（DB，清单/绑定）/ `explore.md`（磁盘，内容唯一真源）/ `AgentRunRecord` + 事件（DB，对话）。
- 新建流程（打开新建时扫描 explores 目录）：
  - **从已有文档创建**：扫描列出未绑定的 `*.md`，选中即建 `ExploreRecord` 绑定该文件（名 = 文件名 stem）；
  - **新话题**：输入主题名建档，**文件落盘时再创建**（不预建空档）。
- 后果：
  - agent 落盘新 explore 文件**不自动入清单**——绑定是显式动作，经新建导入（skill 流程中 agent 自建的话题文件在导入前于清单隐形）；
  - D 的目录 watch 用途消失，watch 通道仅剩「当前 explore.md 预览刷新」一个消费者。
- 身份与文件（已拍板）：
  - `ExploreRecord` 用**独立主键**，不依赖文件名（身份与文件名解耦，`source_ref` 锚主键，改名/文件名变化不破链）；
  - 文件**落盘时再创建**（新话题不预建空档；导入来源的文件本就存在）；
  - 磁盘文件被删**保持记录**（不自动清理；预览空态，下次落盘重建——与懒创建同构，文件是记录的可丢弃投影）。
- 设计细节（进 design）：
  - 链锚形态：`ExploreRecord` 存 `head_run_id`（O(1) 取链头，需与 run 终态双写）vs 由 (source, source_ref) 派生（无双写，靠索引扫描）；
  - 记录↔文件路径关联形态：路径字段（导入时锚定实际文件名）vs 名字派生（新话题落盘按名生成），两种来源如何统一；
  - 未落盘 / 已删除场景的预览与 watch 处置（空态 + run 终态定点重读，文件出现后重挂 watch）。

### 页面导航（已拍板：切换 workspace 返回列表页）

- 详情页 `/explores/:name` 在 workspace 切换时 replace 回列表页 `/explores`——照 `ChangeView` 既有模式（根切换带旧选中 → 抑制误发查询 → replace 回清单）。连带含义：watch 订阅生命周期即详情页生命周期（卸载即退订）。

## 剩余开放问题

- 「explore 清单 DB 化」的剩余设计细节（链锚形态、记录↔文件路径关联形态、未落盘/已删除的预览与 watch 处置）。
- watch 前端防抖窗口的具体取值（design 定）。

## 涉及层（预估改动面）

| 层 | 改动 |
|----|------|
| `crates/core/foundation` | 零改动（explores_root 已在） |
| `crates/core/workflow` | queries：`read_explore` 读内容 + 新建导入扫描（页面清单移 store） |
| `crates/core/agent` + `crates/infra/agent` | `AgentRunParams` 加 resume 字段，flags 组装 `--resume` |
| `crates/infra/store` | 新增 `ExploreRecord`（独立主键、清单/绑定、文件懒创建、孤儿保留）；`AgentRunRecord` 加 `source` / `source_ref` / `parent_run_id`（native_model 版本演进）+ 单链查询 |
| watch 通道（新） | notify 单文件订阅，通知仅修改信号（不含内容）、无后端去抖；Channel 包装在壳层命令；首个消费者为当前 explore.md 预览刷新 |
| `src-tauri/commands` | `read_explore` / 导入扫描薄命令；explore 清单 CRUD（store）；`agent_start` 透传 resume；watch 订阅/退订命令 |
| `packages/desktop/src` | `/explores` 路由 + DB 清单/双栏详情视图（resizable）+ 新建流程（扫描导入 / 新话题）+ workspace 切换回列表；`message-scroller` + `message`（shadcn add）；stance 前导模板；双恢复（文档 + 会话重放）；composer + 前端防抖刷新；侧栏入口 |
