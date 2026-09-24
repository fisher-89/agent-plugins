# desktop-explore-page Specification

## Purpose

定义 desktop 探索（explore）页面的前端契约：DB 化清单页与新建流程、双栏详情与实时预览、对话区呈现（message-scroller 气泡映射）、会话链与 stance 前导，以及 workspace 切换回列表语义。

## Requirements

### Requirement: 探索清单页与新建流程

前端 SHALL 新增探索清单页（`/explores`）：清单条目 SHALL 来自 store 的 `ExploreRecord` 并按当前 workspace root 过滤，MUST NOT 实时扫目录生成清单。新建 SHALL 提供两个入口：

- **从已有文档创建**：经导入扫描列出 explores 目录中未被绑定的 `*.md`（命令层以 store 清单滤除已绑定），选中即创建 `ExploreRecord` 绑定该文件（展示名 = 文件名 stem）；
- **新话题**：输入主题名建档——仅创建记录，MUST NOT 由应用预建空文件或落盘 `explore.md`（内容唯一真源在磁盘，文件由 agent 会话流程按 skill 指引创建；未落盘期间预览呈空态）。

agent 会话中落盘的新 explore 文件 MUST NOT 自动进入清单（绑定是显式动作，未导入前于清单隐形）。清单条目 SHALL 提供删除入口（删记录不动磁盘文件）。

#### Scenario: 清单按 workspace 过滤且来自 store

- **WHEN** 用户在 workspace A 下打开 `/explores`，store 中存在归属 A 与归属 B 的 `ExploreRecord` 各若干
- **THEN** 清单仅呈现归属 A 的记录，不发起目录扫描类查询

#### Scenario: 导入绑定

- **WHEN** explores 目录存在 `alpha.md`（未绑定）与 `beta.md`（已绑定），用户打开新建并选择「从已有文档创建」
- **THEN** 扫描结果仅含 `alpha`；选中后清单出现该条目且再扫描不再列出 `alpha`

#### Scenario: 新话题不预建文件

- **WHEN** 用户以主题名 `perf-review` 新建话题
- **THEN** store 出现对应 `ExploreRecord`，`openspec/explores/` 目录内 MUST NOT 出现 `perf-review.md`，详情页预览呈空态

#### Scenario: agent 落盘不自动入清单

- **WHEN** agent 在会话中新建 `openspec/explores/agent-topic.md`
- **THEN** 清单不出现该文件；经导入扫描选中后才出现在清单

### Requirement: 双栏详情与实时预览

探索详情页（`/explores/:name`）SHALL 为左右双栏布局（resizable，可拖分界）：左对话区、右文档预览。预览 SHALL 经 `read_explore` 读取磁盘 `explore.md` 并以既有 `MarkdownDocRenderer` 渲染；页面挂载时 SHALL 对当前 explore.md 发起 watch 订阅，收到修改信号后前端防抖再经 `read_explore` 显式拉取刷新（数据面无轮询）。目标文件未落盘或已被删除时预览 SHALL 呈空态（记录保留、不报错）。

#### Scenario: 预览随磁盘刷新

- **WHEN** explore.md 被 agent（或外部编辑器）修改且 watch 信号到达
- **THEN** 前端防抖后发起一次 `read_explore` 并更新预览，期间无定时轮询

#### Scenario: 未落盘与已删除空态

- **WHEN** 新话题尚未落盘，或磁盘 explore.md 被删除
- **THEN** 预览呈空态，页面不报错；文件（重）出现后经 watch 信号恢复内容

### Requirement: 对话区呈现

对话区 SHALL 以 shadcn 官方 `message-scroller` + `message` 组件承载滚动与气泡（滚动行为由组件负责，消息内容/状态留在应用层）。事件到气泡的映射 SHALL 为：user / assistant 消息各成气泡；`Block::Text` 渲染 markdown、`Block::Thinking` 折叠呈现、`Block::ToolUse` 渲染为可读卡片、`Block::ToolResult` 与同 id `ToolUse` 成对呈现；`AskUserQuestion` 的 ToolUse SHALL 呈现为静态卡片（问题与选项原样可读），答案经 composer 文本输入（questionnaire 接线为二期）。composer SHALL 提供 prompt 输入与 env / permission-mode 档位选择（沿用调试页档位语义与默认值）。实时流 SHALL 走 `agent_start` 的 Channel（执行流通道），历史 SHALL 经 store 重放（`agent_run_events`）。

#### Scenario: 四变体气泡映射

- **WHEN** 一次含文本、思考、工具调用与工具结果的运行完成
- **THEN** 对话区按序呈现 markdown 气泡、可展开思考块、工具卡片与成对结果，AskUserQuestion 卡片问题/选项可读

#### Scenario: composer 发起运行

- **WHEN** 用户在 composer 输入内容并选择 default 档 + bypassPermissions 发送
- **THEN** 以当前 workspace root 为 cwd 发起 `agent_start`，事件实时流入对话区，result 汇总（num_turns / cost / duration / session_id）可读

### Requirement: 会话链与 stance 前导

每个 explore SHALL 对应一条会话链：打开详情页 SHALL 默认续最近链（按 `source="explore"` + 该 explore 的 `source_ref` 定位链头，沿 `parent_run_id` 回溯还原全部历史 run 并重放落库事件），无链则下次发送起链（首条 run 创建链）。MVP MUST NOT 提供「新会话」动作。发送时 SHALL 将 desktop 内置的精简 explore stance 前导模板拼接进 prompt 头部（模板归属 desktop 包，注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链）；续话 SHALL 以链尾 run 的 `session_id` 作为 `resume_session_id` 发起。双恢复 SHALL 为：文档从磁盘读 + 对话重放该链落库事件，两者互不依赖对方存在。

#### Scenario: 重开续链双恢复

- **WHEN** 用户完成两轮对话后离开并重新打开该 explore
- **THEN** 预览从磁盘恢复当前 explore.md，对话区按序重放两条 run 的全部落库事件；再次发送时以链尾 `session_id` 续话而非新会话

#### Scenario: stance 拼接

- **WHEN** 查看任一 explore 发起 run 的 prompt 原文
- **THEN** 内容以 stance 前导模板开头、用户输入随后；调试页发起的 run prompt 不含该模板

### Requirement: workspace 切换回列表

详情页在 workspace 切换（select / 移除当前根 / 添加新根）时 SHALL replace 导航回 `/explores`（照 `ChangeView` 既有模式：过渡轮抑制误发查询），MUST NOT 以旧 workspace 的选中名发起新根查询。watch 订阅生命周期 SHALL 即详情页生命周期（卸载即退订）。

#### Scenario: 根切换落清单

- **WHEN** 用户在 `/explores/<name>` 详情页切换到另一 workspace
- **THEN** URL replace 为 `/explores`，清单以新根重取，旧详情与 watch 订阅不再存活

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/explores/ExploreView.tsx`（新） | 清单页 + 新建流程 | store 清单按 root 过滤；导入扫描绑定 / 新话题建档；删除入口（不动文件） |
| `packages/desktop/src/views/explores/ExploreDetailView.tsx`（新） | 双栏详情 | resizable 分栏；左对话区右预览；watch 生命周期 = 页面生命周期 |
| `views/explores/components/`（新：对话区 / composer / 预览 / 新建对话框） | 呈现件 | `message-scroller` + `message` 气泡映射；AskUserQuestion 静态卡片；预览复用 `MarkdownDocRenderer` |
| `views/explores/hooks/`（新） | 会话链与刷新编排 | 链还原重放 + Channel 实时流；watch 信号防抖 → `read_explore` 显式刷新 |
| `packages/desktop/src/lib/exploreStance.ts`（新） | stance 前导模板 | 拼接进 explore run prompt 头部；与 SKILL.md 双源注释互链 |
| `routes.tsx` + `components/AppSidebar.tsx` | 路由与导航 | `/explores`、`/explores/:name`；`nav-explores` 入口（active 由 URL 派生） |
| `types/dto.ts` | 前端 DTO | `ExploreRecord` / 导入扫描结果，与 store 自有类型一一对应 |
