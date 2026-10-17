# desktop-explore-page Specification (Delta)

## MODIFIED Requirements

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

## ADDED Requirements

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

## Module Contract

| 模块 | 类别 | 关键契约 |
|------|------|----------|
| `views/explores/components/explore-create-dialog.tsx` | Component | TopicEntry 改需求 textarea（`name = null` invoke；空白禁提交；isKebabCase 校验退役）；ImportEntry 显式名 invoke；onCreated 携占位名 + 需求回调 |
| `views/explores/explore-view.tsx` | Component | onCreated 导航携 `state.autoPrompt`；详情记录派生支持 id 桥重寻（刷新后按 `record.id` 对齐，旧名 miss 以快照兜底） |
| `views/explores/hooks/use-explore-session.ts` | Function（hook） | auto-send effect（三条件闸门 + 一次性消费 ref）；首轮 stance 分形调用（占位 → 命名指令形 / 已定名 → 定位形）；续话语义不变 |
| `views/explores/explore-detail-view.tsx` | Component | run 终态 id 桥（refresh → 按 id 重寻 → 名变 navigate replace）；watch 经 name 依赖自动重订阅；promote 错误行内呈现照旧 |
| `lib/explore-stance.ts` | Function | `buildExplorePrompt` 两形（占位命名指令形 / 已定名笔记定位形）；与 SKILL.md 双源注释互链照旧 |
| `plugins/dev-team/skills/openspec-explore/SKILL.md` | Doc | 笔记落盘约定随工具化改写（双源同步；插件源变更走版本 bump + 重建） |
