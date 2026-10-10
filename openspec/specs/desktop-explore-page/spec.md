# desktop-explore-page Specification

## Purpose

定义 desktop 探索（explore）页面的前端契约：DB 化清单页与新建流程、双栏详情与实时预览、对话区呈现（message-scroller 气泡映射）、会话链与 stance 前导，以及 workspace 切换回列表语义。

## Requirements

### Requirement: 探索清单页与新建流程

前端 SHALL 新增探索清单页（`/explores`）：清单条目 SHALL 来自 store 的 `ExploreRecord` 并按当前 workspace root 过滤，MUST NOT 实时扫目录生成清单。清单条目 SHALL 渲染 `title`（人类可读标题，`title` 恒非空——创建 / 升级默认 = name，agent 产出标题后经回填覆盖）而非 `name`。新建 SHALL 提供两个入口：

- **从已有文档创建**：经导入扫描列出 explores 目录中未被绑定且 **stem 为 kebab-case** 的 `*.md`（命令层以 store 清单滤除已绑定、再滤除非 kebab stem），选中即创建 `ExploreRecord` 绑定该文件（`name` = 文件 stem、`title` 初始 = name）；
- **新话题**：输入 kebab-case 主题名建档——仅创建记录，MUST NOT 由应用预建空文件或落盘 `explore.md`（内容唯一真源在磁盘，文件由 agent 会话流程按 skill 指引创建；未落盘期间预览呈空态）。主题名 SHALL 本地做 kebab-case 校验（与后端同口径，非法时禁用提交）。

agent 会话中落盘的新 explore 文件 MUST NOT 自动进入清单（绑定是显式动作，未导入前于清单隐形）。清单条目 SHALL 提供删除入口（删记录不动磁盘文件；该记录名下的会话 runs 与事件随记录在 store 层同事务级联删除——对话随身份消亡，磁盘笔记文件是唯一保留物）。

#### Scenario: 清单按 workspace 过滤且来自 store

- **WHEN** 用户在 workspace A 下打开 `/explores`，store 中存在归属 A 与归属 B 的 `ExploreRecord` 各若干
- **THEN** 清单仅呈现归属 A 的记录，不发起目录扫描类查询

#### Scenario: 导入绑定（kebab 过滤）

- **WHEN** explores 目录存在 `alpha.md`（未绑定，kebab）、`MyNote.md`（未绑定，非 kebab）与 `beta.md`（已绑定），用户打开新建并选择「从已有文档创建」
- **THEN** 扫描结果仅含 `alpha`（非 kebab 的 `MyNote` 不列出）；选中后清单出现该条目且再扫描不再列出 `alpha`

#### Scenario: 新话题不预建文件且 kebab 校验

- **WHEN** 用户以主题名 `perf-review` 新建话题
- **THEN** store 出现对应 `ExploreRecord`（`name=perf-review`、`title=perf-review`），`openspec/explores/` 目录内 MUST NOT 出现 `perf-review.md`，详情页预览呈空态
- **AND** 输入 `My Topic`（含空格 / 大写）时提交按钮禁用且不发起 invoke

#### Scenario: agent 落盘不自动入清单

- **WHEN** agent 在会话中新建 `openspec/explores/agent-topic.md`
- **THEN** 清单不出现该文件；经导入扫描选中后才出现在清单

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

对话区 SHALL 以共享 agent 会话组件族（`components/agent/`，见 desktop-agent-chat-infra）呈现，MUST NOT 保留视图层私有的事件渲染实现：状态面经统一会话基建（`use-agent-chat` + `TauriAgentTransport`），实时流经 transport 走 `agent_start` 的 Channel（执行流通道），历史经 `agent_run_chain` + `agent_run_events` 重放装载进 UIMessage 状态（实时与重放同一状态形状）。呈现面 SHALL 以 `AgentMessages` 对话透镜渲染：user / assistant 消息各成气泡；文本块渲染 markdown、思考块折叠呈现、工具调用渲染为可读卡片并与同 id 工具结果成对呈现（配对收口于适配层）；`AskUserQuestion` 的 ToolUse SHALL 呈现为静态卡片（问题与选项原样可读），答案经 composer 文本输入（questionnaire 接线为二期）。composer SHALL 采用 `AgentInput`：prompt 输入与 permission-mode 档位选择（沿用调试页档位语义与默认值，env 不设参数——运行恒为完整环境），运行中忽略重复发送并提供停止入口。滚动 SHALL 沿用 shadcn `message-scroller` 组合（滚动行为由组件负责，消息状态留在应用层）。

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

每个 explore SHALL 对应一条会话链：打开详情页 SHALL 默认续最近链（按 `source="explore"` + 该 explore 的 `source_ref` 定位链头，沿 `parent_run_id` 回溯还原全部历史 run 并重放落库事件），无链则下次发送起链（首条 run 创建链）。MVP MUST NOT 提供「新会话」动作。发送时 SHALL 将 desktop 内置的精简 explore stance 前导模板拼接进 prompt 头部（模板归属 desktop 包，注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链）；stance 前导 SHALL 注入当前记录 `name`（agent 落盘文件名 MUST 等于 `record.name`，MUST NOT 自拟）并明示「笔记首行写 `# <标题>`」约定。续话 SHALL 以链尾 run 的 `session_id` 作为 `resume_session_id` 发起。双恢复 SHALL 为：文档从磁盘读 + 对话重放该链落库事件，两者互不依赖对方存在。

#### Scenario: 重开续链双恢复

- **WHEN** 用户完成两轮对话后离开并重新打开该 explore
- **THEN** 预览从磁盘恢复当前 explore.md，对话区按序重放两条 run 的全部落库事件；再次发送时以链尾 `session_id` 续话而非新会话

#### Scenario: stance 拼接注入 name

- **WHEN** 查看任一 explore 发起 run 的 prompt 原文
- **THEN** 内容以 stance 前导模板开头、含当前记录 name（落盘文件名 MUST 等于 name）与「首行写 `# 标题`」约定、用户输入随后；调试页发起的 run prompt 不含该模板

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

探索详情页 SHALL 在记录 `promoted_to` 为空时提供「启动变更」入口；点击后 SHALL 调 `promote_explore(root, name)`，成功后刷新清单并停留在详情页（promoted 态随即呈现「已转变更」+ 跳转，MUST NOT 自动导航离开）。`promoted_to` 非空时 SHALL 不呈现「启动变更」入口。后端错误 SHALL 行内呈现（break-all 错误块），用户可改后重试。测试挂钩 SHALL 使用 data-testid，MUST NOT 以样式类名作查询挂钩。

#### Scenario: promote 成功流转

- **WHEN** 用户在草稿态 explore 详情点击「启动变更」且 promote 成功
- **THEN** 清单刷新、记录 `promoted_to` 打标，详情呈 promoted 态（「已转变更」+ 跳转），URL 不离开 `/explores/<name>`

#### Scenario: promote 失败行内呈现

- **WHEN** promote 因笔记未落盘 / 非 kebab name / change 冲突等原因失败
- **THEN** 详情页行内呈现后端错误，记录与笔记零改动，用户可改后重试

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/explores/explore-view.tsx`（新） | 清单页 + 新建流程 | store 清单按 root 过滤；导入扫描绑定 / 新话题建档；删除入口（不动文件，对话随记录级联清） |
| `packages/desktop/src/views/explores/explore-detail-view.tsx`（新） | 双栏详情 | resizable 分栏；左对话区右预览；对话区 / composer 换共享组件族（`AgentMessages` / `AgentInput`）；watch 生命周期 = 页面生命周期 |
| `views/explores/components/`（新：composer / 预览 / 新建对话框；`explore-conversation.tsx` 已删） | 呈现件 | 对话区由 `components/agent/AgentMessages` 承载（私有渲染实现收编，用例随迁组件族测试）；composer 改用 `AgentInput`；AskUserQuestion 静态卡片；预览复用 `MarkdownDocRenderer` |
| `views/explores/hooks/use-explore-session.ts`（改） | 会话链与刷新编排 | 改走 `use-agent-chat`（重放装载 + 链参数经 body 穿透）；stance 文本组装留本 hook（来源侧）；对外状态/行为语义不变；watch 信号防抖 → `read_explore` 显式刷新 |
| `packages/desktop/src/lib/explore-stance.ts`（新） | stance 前导模板 | 拼接进 explore run prompt 头部；与 SKILL.md 双源注释互链 |
| `routes.tsx` + `components/app-sidebar.tsx` | 路由与导航 | `/explores`、`/explores/:name`；`nav-explores` 入口（active 由 URL 派生） |
| `types/dto.ts` | 前端 DTO | `ExploreRecord` / 导入扫描结果，与 store 自有类型一一对应 |
