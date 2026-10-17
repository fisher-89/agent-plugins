# desktop-explore-page Specification

## Purpose

定义 desktop 探索（explore）页面的前端契约：DB 化清单页与新建流程、双栏详情与实时预览、对话区呈现（message-scroller 气泡映射）、会话链与 stance 前导，以及 workspace 切换回列表语义。

## Requirements

### Requirement: 探索清单页与新建流程

前端 SHALL 新增探索清单页（`/explores`）：清单条目 SHALL 来自 store 的 `ExploreRecord` 并按当前 workspace root 过滤，MUST NOT 实时扫目录生成清单。清单条目 SHALL 渲染 `title`（人类可读标题，`title` 恒非空——显式名建档默认 = name、占位建档 = 临时标题「<时间戳>-分析中」，agent 产出标题后经回填覆盖）而非 `name`。新建 SHALL 提供两个入口：

- **从已有文档创建**：经导入扫描列出 explores 目录中未被绑定且 **stem 为 kebab-case** 的 `*.md`（命令层以 store 清单滤除已绑定、再滤除非 kebab stem），选中即创建 `ExploreRecord` 绑定该文件（`name` = 文件 stem、`title` 初始 = name，invoke 携显式名）；
- **新话题**：输入**需求自然语言**（多行文本）建档——前端以 `name = null` 调 `create_explore_record`（后端同事务生成占位名 `draft-*` 与临时标题，见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」），MUST NOT 要求用户输入 kebab 名、MUST NOT 由应用预建空文件或落盘 `explore.md`（内容唯一真源在磁盘，笔记由 agent 会话经专用工具定名落盘，见 desktop-agent-execution「会话级工具面注入与 explore 笔记工具」；未落盘期间预览呈空态）。kebab-case 本地校验 SHALL 随新话题入口退役（空白输入禁用提交；kebab 过滤仅存导入路径）。建档成功后 SHALL 携路由 `state.autoPrompt = 需求` 导航进详情（auto-send 见「会话链与 stance 前导」）。

agent 会话中**经专用工具 `create_explore_note` 定名落盘**的笔记 SHALL 随记录 in-place 改名即时进入清单（文件与记录同名绑定）；**绕过专用工具**（如经 bash 重定向）新建的 explore 文件 MUST NOT 自动进入清单（绑定是显式动作，未导入前于清单隐形）。清单条目 SHALL 提供删除入口（删记录不动磁盘文件；该记录名下的会话 runs 与事件随记录在 store 层同事务级联删除——对话随身份消亡，磁盘笔记文件是唯一保留物）。

#### Scenario: 清单按 workspace 过滤且来自 store

- **WHEN** 用户在 workspace A 下打开 `/explores`，store 中存在归属 A 与归属 B 的 `ExploreRecord` 各若干
- **THEN** 清单仅呈现归属 A 的记录，不发起目录扫描类查询

#### Scenario: 导入绑定（kebab 过滤）

- **WHEN** explores 目录存在 `alpha.md`（未绑定，kebab）、`MyNote.md`（未绑定，非 kebab）与 `beta.md`（已绑定），用户打开新建并选择「从已有文档创建」
- **THEN** 扫描结果仅含 `alpha`（非 kebab 的 `MyNote` 不列出）；选中后清单出现该条目且再扫描不再列出 `alpha`

#### Scenario: 新话题免命名建档与 autoPrompt 导航

- **WHEN** 用户以需求文本「调研 API 重试策略」新建话题
- **THEN** 前端以 `name = null` invoke 建档，store 出现 `ExploreRecord`（`name` 匹配 `draft-*`、`title` 为「<时间戳>-分析中」），`openspec/explores/` 目录内 MUST NOT 出现新文件，详情页预览呈空态；导航至 `/explores/<占位名>` 且路由 `state.autoPrompt` 携需求原文
- **AND** 输入空白时提交按钮禁用且不发起 invoke（kebab 校验分支不再存在）

#### Scenario: 专用工具定名即时入清单

- **WHEN** agent 在会话中经 `create_explore_note(name="agent-topic", …)` 定名成功
- **THEN** 记录名变为 `agent-topic`，清单条目随刷新以新名呈现；绕过专用工具新建的 `rogue.md` 不进入清单（经导入扫描选中后才出现）

#### Scenario: 清单渲染 title

- **WHEN** 某 `ExploreRecord` 的 `title` 为「API 重试调研」而 `name` 为 `api-retry`
- **THEN** 清单条目显示「API 重试调研」，不显示 `api-retry`

### Requirement: 双栏详情与实时预览

探索详情页（`/explores/:name`）SHALL 为左右双栏布局（resizable，可拖分界）：左对话区、右文档预览。详情头 SHALL 渲染 `title`。预览 SHALL 经 `read_explore` 读取磁盘 `explore.md` 并以既有 `MarkdownDocRenderer` 渲染；页面挂载时 SHALL 对当前 explore.md 发起 watch 订阅，收到修改信号后前端防抖再经 `read_explore` 显式拉取刷新（数据面无轮询）。目标文件未落盘或已被删除时预览 SHALL 呈空态（记录保留、不报错）。记录 `promoted_to` 非空时（已 promote），详情 SHALL 显示「已转变更」徽标与「查看变更」跳转（路由 `/changes/<promoted_to>`），预览区 SHALL 显示已 promote 提示而非继续读 `explores/<name>.md`（笔记已移走）。

#### Scenario: 预览随磁盘刷新

- **WHEN** explore.md 被 agent（或外部编辑器）修改且 watch 信号到达
- **THEN** 前端防抖后发起一次 `read_explore` 并更新预览，期间无定时轮询

#### Scenario: 未落盘与已删除空态

- **WHEN** 新话题尚未落盘，或磁盘 explore.md 被删除
- **THEN** 预览呈空态，页面不报错；文件（重）出现后经 watch 信号恢复内容

#### Scenario: promoted 态呈现

- **WHEN** 记录 `promoted_to = Some(change_id)` 时打开该 explore 详情
- **THEN** 详情头显示「已转变更」徽标与「查看变更」跳转（目标 `/changes/<change_id>`），预览区显示已 promote 提示、不再读 `explores/<name>.md`

### Requirement: 对话区呈现

对话区 SHALL 以共享 agent 会话组件族（`components/agent/`，见 desktop-agent-chat-infra）呈现，MUST NOT 保留视图层私有的事件渲染实现：状态面经统一会话基建（`use-agent-chat` + `TauriAgentTransport`），实时流经 transport 走 `agent_start` 的 Channel（执行流通道），历史经 `session_detail(root, sessionId)` 单查 + `agent_session_transcript` 重放装载进 UIMessage 状态（实时与重放同一状态形状）。呈现面 SHALL 以 `AgentMessages` 对话透镜渲染：user / assistant 消息各成气泡；文本块渲染 markdown、思考块折叠呈现、工具调用渲染为可读卡片并与同 id 工具结果成对呈现（配对收口于适配层）；`AskUserQuestion` 的 ToolUse SHALL 呈现为静态卡片（问题与选项原样可读），答案经 composer 文本输入（questionnaire 接线为二期）。composer SHALL 采用 `AgentInput`：prompt 输入与 permission-mode 档位选择（沿用调试页档位语义与默认值，env 不设参数——运行恒为完整环境），运行中忽略重复发送并提供停止入口。滚动 SHALL 沿用 shadcn `message-scroller` 组合（滚动行为由组件负责，消息状态留在应用层）。

#### Scenario: 四变体气泡映射

- **WHEN** 一次含文本、思考、工具调用与工具结果的运行完成
- **THEN** 对话区按序呈现 markdown 气泡、可展开思考块、工具卡片与成对结果，AskUserQuestion 卡片问题/选项可读

#### Scenario: composer 发起运行

- **WHEN** 用户在 composer 输入内容并以默认 bypassPermissions 档发送
- **THEN** 以当前 workspace root 为 cwd 发起 `agent_start`，事件实时流入对话区，result 汇总（num_turns / cost / duration / session_id）可读

#### Scenario: 运行中停止

- **WHEN** 会话运行中用户点击 composer 的停止入口
- **THEN** 触发 `agent_stop`，事件流以终止终态收尾且呈现与停止前实时流一致，随后恢复可发送

#### Scenario: 重开重放形状一致

- **WHEN** 用户离开后重新打开该 explore 详情页
- **THEN** 经重放装载重建的对话区呈现与离开前实时呈现形状一致（同一 `AgentMessages` 透镜渲染）

### Requirement: 会话链与 stance 前导

每个 explore SHALL 对应一条会话链（同一 session 的轮序列即链，按 `started_at` 有序）：打开详情页 SHALL 以记录字段 `ExploreRecord.session_id`（归属 agent 会话 id，建档同事务预建空会话并回写；存量记录经 `ensure_explore_session` 归位补齐）**直查** `session_detail(root, sessionId)` 并重放落库事件还原全部历史；会话尚无轮（建档预建的空会话）则下次发送即其首轮（Continue 开轮，不新铸会话）。按 `source="explore"` + `source_ref` 反查定位链头并沿 `parent_run_id` 回溯的路径 SHALL 退役（前端不再经来源反查会话）。MVP MUST NOT 提供「新会话」动作。发送时 SHALL 将 desktop 内置的精简 explore stance 前导模板拼接进首轮 prompt 头部（模板归属 desktop 包，注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链）。stance 前导 SHALL 按记录定名状态分两形（判别 = `name` 是否 `draft-` 前缀，前端侧）：

- **命名指令形（占位记录）**：指引 agent 生成 kebab 最终名并经 `create_explore_note(name, content)` 原子定名落盘（引擎侧契约见 desktop-agent-execution「会话级工具面注入与 explore 笔记工具」）；MUST NOT 插值笔记文件路径（路径知识归工具描述与返回值）；含「优先用专用工具而非 bash 造文件」软约束；
- **笔记定位形（已定名记录）**：插值当前 `record.name` 与笔记路径，指引用 `edit` 修改笔记、文件缺失时以 `create_explore_note` 同名补建。

两形 SHALL 共同明示「笔记首行写 `# <标题>`」与「推算变更类型并写约定标记行」两条约定（标记行与首行标题同构，具体格式由 design 定稿）。**auto-send**：建档导航携带的 `state.autoPrompt` SHALL 在「重放装载完成（`!loading`）&& `chain.length === 0` && `state.autoPrompt` 在场」三条件齐备时自动发起首轮发送（stance 前导 + 需求，配一次性消费 ref）；HashRouter 下 back/forward 复现 state 时 `chain.length === 0` 闸门拒发（发出即有轮行），首轮发送失败 `chain` 仍 0 时复现构成一次免费重试（留痕接受）。续话 SHALL 以记录回写的 `session_id` 作为会话引用（`Continue`）发起。双恢复 SHALL 为：文档从磁盘读 + 对话重放该会话落库事件，两者互不依赖对方存在。

#### Scenario: 重开续链双恢复

- **WHEN** 用户完成两轮对话后离开并重新打开该 explore（记录 `session_id` 在案，建档即回写）
- **THEN** 预览从磁盘恢复当前 explore.md，对话区以 `session_id` 直查 `session_detail` 并按序重放该会话全部落库事件；再次发送时以该 `session_id` 续话而非新会话

#### Scenario: 建档后自动发出首轮

- **WHEN** 用户以需求建档并被导航至详情（`state.autoPrompt` 在场），重放装载完成时会话链 `chain.length === 0`
- **THEN** 自动发起首轮发送，prompt = stance 前导（命名指令形）+ 需求原文；已发出（`chain > 0`）后 back/forward 复现 state 不再重发

#### Scenario: stance 按定名状态分形

- **WHEN** 分别查看占位记录（`draft-…`）与已定名记录（`api-retry`）发起 run 的 prompt 原文
- **THEN** 占位形含命名指令（生成 kebab 名并经 `create_explore_note` 定名）且不含任何插值笔记路径；定位形含 `record.name` 与笔记路径及 Edit 指引；调试页发起的 run prompt 不含任一形态的该模板

#### Scenario: stance 注入类型推算约定

- **WHEN** 查看任一 explore 发起 run 的 prompt 原文
- **THEN** stance 前导含「推算变更类型并写约定标记行」约定与判断标准（补测试 / 生成测试用例 → test-only，否则 requirement）；调试页发起的 run prompt 不含该约定

### Requirement: 免命名定名与改名感知

占位名 SHALL 仅存在于 store 记录名（从未落盘——世上不存在占位名文件路径）；定名 SHALL 唯一经 agent 会话内的 `create_explore_note(name, content)` 原子完成（文件落盘 + 记录 in-place 改名，引擎侧契约见 desktop-agent-execution「会话级工具面注入与 explore 笔记工具」），前端 MUST NOT 提供定名 / 改名入口（`rename_explore_record` 维持不接线）。**改名感知（id 桥）**：详情页 SHALL 在 run 终态（`running` true→false 既有 effect）时刷新清单并**按记录 `id`**（in-place 改名不变）重寻记录——名变则 `navigate replace(/explores/<新名>)`，`useExploreDoc` 以 name 为依赖自动重订阅新路径（既有机制零改）；刷新与导航之间按旧名寻址 miss SHALL 以既有记录快照兜底，MUST NOT 闪现 MissingRecord 占位。**中间窗口**：定名发生到 run 终态之间页面停留占位名（URL / watch 旧路径、预览空态、标题「分析中」），run 终态后一次收敛（与「分析中」临时标题同窗，留痕接受）。**promote 门禁**：记录名为占位前缀时「启动变更」 SHALL 得到后端显式 `Err`（引导先完成会话定名，见 desktop-explore-queries「promote_explore 命令与笔记搬运」）并照既有行内错误呈现；定名后 promote 入口语义不变。

#### Scenario: 原子定名收敛

- **WHEN** agent 首轮经 `create_explore_note(name="api-retry", …)` 定名成功且 run 转入终态
- **THEN** 记录 `name="api-retry"`（`id` / `session_id` 不变）、磁盘出现 `openspec/explores/api-retry.md`、占位名文件不存在；URL replace 至 `/explores/api-retry`、清单刷新、预览读新名文件，title 与 workflow_type 回填照常触发

#### Scenario: 终态前中间窗口

- **WHEN** `create_explore_note` 已执行成功而 run 仍在进行
- **THEN** 详情页 URL / watch / 预览仍停留占位名（预览空态、标题「分析中」），无报错；run 终态后经 id 桥一次收敛到新名

#### Scenario: 刷新导航无 MissingRecord 闪现

- **WHEN** run 终态触发的清单刷新完成后、URL 替换完成前，详情页按旧名（占位名）从清单寻址记录
- **THEN** 以既有记录快照兜底呈现（不闪现 MissingRecord 占位），随 `navigate replace` 完成后按新名寻址

#### Scenario: promote 占位拦截

- **WHEN** 记录名仍为 `draft-*` 时用户点击「启动变更」
- **THEN** 后端显式 `Err`（引导先在会话中完成定名）行内呈现，记录与磁盘零改动；定名后同入口语义与既有 promote 契约一致

### Requirement: workspace 切换回列表

详情页在 workspace 切换（select / 移除当前根 / 添加新根）时 SHALL replace 导航回 `/explores`，MUST NOT 以旧 workspace 的选中名发起新根查询。watch 订阅生命周期 SHALL 即详情页生命周期（卸载即退订）。

#### Scenario: 根切换落清单

- **WHEN** 用户在 `/explores/<name>` 详情页切换到另一 workspace
- **THEN** URL replace 为 `/explores`，清单以新根重取，旧详情与 watch 订阅不再存活

### Requirement: title 回填

详情页 SHALL 在 watch 重读笔记内容后解析首行 `# <标题>`（纯函数 `extractMarkdownTitle`，返回非空白标题或 `None`）：解析结果非空且与 `record.title` 不同时 SHALL 调显式写命令 `update_explore_title(root, name, title)` 回填 store `title`，成功后刷新清单（清单与详情头 title 一致）。回填 SHALL 仅经显式写命令完成，MUST NOT 在读取路径（`read_explore`）写入或修改任何文件；解析失败（无标题行）SHALL 保持 `record.title` 不变，不报错。

#### Scenario: 标题解析回填

- **WHEN** 笔记首行为 `# API 重试调研` 而 `record.title` 为 `api-retry`，watch 重读后
- **THEN** 前端调 `update_explore_title` 回填「API 重试调研」，成功后清单与详情头显示新 title；`read_explore` 路径零写入

#### Scenario: 无标题行不报错

- **WHEN** 笔记内容无首行 `# 标题`（或标题行空白）
- **THEN** `record.title` 保持原值，无 invoke 调用、无错误呈现

### Requirement: promote 启动变更入口

探索详情页 SHALL 在记录 `promoted_to` 为空时提供「启动变更」入口，并在入口旁提供工作流类型选择器（普通 select，内联实现，MUST NOT 抽共享组件）：默认值 = `record.workflow_type`（推算值），可改 `requirement` / `test-only`。点击后 SHALL 调 `promote_explore(root, name, workflow_type)`，成功后刷新清单并停留在详情页（promoted 态随即呈现「已转变更」+ 跳转，MUST NOT 自动导航离开）。`promoted_to` 非空时 SHALL 不呈现「启动变更」入口。后端错误 SHALL 行内呈现（break-all 错误块），用户可改后重试。测试挂钩 SHALL 使用 data-testid，MUST NOT 以样式类名作查询挂钩。

#### Scenario: promote 成功流转

- **WHEN** 用户在草稿态 explore 详情以默认（推算）类型点击「启动变更」且 promote 成功
- **THEN** 清单刷新、记录 `promoted_to` 打标，详情呈 promoted 态（「已转变更」+ 跳转），URL 不离开 `/explores/<name>`；铸出 change 的 `workflow_type` = 用户选择值

#### Scenario: 改选类型后 promote

- **WHEN** 用户把类型选择器从推算值改为 `test-only` 后点击「启动变更」
- **THEN** `promote_explore(root, name, "test-only")` 被调用，铸出 change 的 `workflow_type = "test-only"`

#### Scenario: promote 失败行内呈现

- **WHEN** promote 因笔记未落盘 / 非 kebab name / change 冲突等原因失败
- **THEN** 详情页行内呈现后端错误，记录与笔记零改动，用户可改后重试

### Requirement: workflow_type 回填与类型推算解析

详情页 SHALL 在 watch 重读笔记内容后解析约定标记行（纯函数 `extractWorkflowType`，返回 `"requirement"` / `"test-only"` / `null`）：解析结果合法且与 `record.workflow_type` 不同时 SHALL 调显式写命令 `update_explore_workflow_type(root, name, workflow_type)` 回填 store `workflow_type`，成功后刷新清单。回填 SHALL 仅经显式写命令完成，MUST NOT 在读取路径（`read_explore`）写入或修改任何文件；解析失败（无约定标记行 / 非法值）SHALL 保持 `record.workflow_type` 不变，不报错。类型选择器的人工改选 SHALL 同样经 `update_explore_workflow_type` 回填落库（沿 title 改即写模式）。

#### Scenario: 标记解析回填

- **WHEN** 笔记含约定标记行 `test-only` 而 `record.workflow_type` 为 `"requirement"`，watch 重读后
- **THEN** 前端调 `update_explore_workflow_type` 回填 `"test-only"`，成功后清单刷新、promote 选择器默认值随动；`read_explore` 路径零写入

#### Scenario: 无标记或非法值不报错

- **WHEN** 笔记内容无约定标记行（或标记值非法）
- **THEN** `record.workflow_type` 保持原值，无 invoke 调用、无错误呈现

#### Scenario: 人工改选落库

- **WHEN** 用户在 promote 入口把类型从 `"requirement"` 改为 `"test-only"`
- **THEN** 前端调 `update_explore_workflow_type` 回填 `"test-only"`（改即写），下次进详情仍是改后值

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/explores/explore-view.tsx`（新） | 清单页 + 新建流程 | store 清单按 root 过滤；导入扫描绑定 / 新话题免命名建档（`name = null` invoke）；删除入口（不动文件，对话随记录级联清）；onCreated 导航携 `state.autoPrompt`；详情记录派生支持 id 桥重寻（刷新后按 `record.id` 对齐，旧名 miss 以既有快照兜底） |
| `packages/desktop/src/views/explores/explore-detail-view.tsx`（新） | 双栏详情 | resizable 分栏；左对话区右预览；对话区 / composer 换共享组件族（`AgentMessages` / `AgentInput`）；watch 生命周期 = 页面生命周期；run 终态 id 桥（refresh → 按 id 重寻 → 名变 `navigate replace`）；promote 错误行内呈现照旧 |
| `views/explores/components/`（新：composer / 预览 / 新建对话框；`explore-conversation.tsx` 已删） | 呈现件 | 对话区由 `components/agent/AgentMessages` 承载（私有渲染实现收编，用例随迁组件族测试）；composer 改用 `AgentInput`；AskUserQuestion 静态卡片；预览复用 `MarkdownDocRenderer` |
| `views/explores/components/explore-create-dialog.tsx`（改） | 新建对话框 | TopicEntry 改需求 textarea（`name = null` invoke；空白禁提交；`isKebabCase` 校验退役）；ImportEntry 显式名 invoke；onCreated 携占位名 + 需求回调 |
| `views/explores/hooks/use-explore-session.ts`（改） | 会话链与刷新编排 | 改走 `use-agent-chat`（重放装载 + 链参数经 body 穿透）；stance 文本组装留本 hook（来源侧）；auto-send effect（三条件闸门 + 一次性消费 ref）；首轮 stance 分形调用（占位 → 命名指令形 / 已定名 → 定位形）；续话语义不变；watch 信号防抖 → `read_explore` 显式刷新 |
| `packages/desktop/src/lib/explore-stance.ts`（新） | stance 前导模板 | `buildExplorePrompt` 两形（占位命名指令形 / 已定名笔记定位形）拼接进 explore run prompt 头部；与 SKILL.md 双源注释互链 |
| `plugins/dev-team/skills/openspec-explore/SKILL.md` | 笔记落盘约定文档 | 约定随工具化改写（`create_explore_note` 定名 + Edit 维护；双源同步；插件源变更走版本 bump + 重建） |
| `routes.tsx` + `components/app-sidebar.tsx` | 路由与导航 | `/explores`、`/explores/:name`；`nav-explores` 入口（active 由 URL 派生） |
| `types/dto.ts` | 前端 DTO | `ExploreRecord` / 导入扫描结果，与 store 自有类型一一对应 |
