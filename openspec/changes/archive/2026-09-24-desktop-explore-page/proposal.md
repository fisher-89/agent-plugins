# 提案: desktop-explore-page

> **变更**: desktop-explore-page
> **日期**: 2026-09-24
> **状态**: proposed

---

## 问题

desktop 已有变更页（`/changes`）与 Agent 调试页（`/agent`），但 workspace 下第三类内容——探索笔记 `openspec/explores/*.md`——没有任何入口：

- `foundation::layout::resolve` 已预留 `explores_root`（全包唯一目录知识点），但零查询使用；
- `workflow::queries` 只有 changes 域（list / detail / locate 全是 change 语义），explore 笔记无法读取与列出；
- 前端无 `/explores` 路由、无侧栏入口、无任何 chat 类组件（现有 `ui/` 九件套均为静态展示件）；
- 用户无法在 desktop 内发起一次「探索」式 agent 对话并查看产出的 explore 笔记——`/openspec-explore` 的 stance（探索立场：先调查再落笔、不建 change 目录、产出单文件 md）只能在 CLI 会话中由 skill 注入，desktop 无注入通道；
- agent 调试链路是 one-shot：`AgentRunParams` 无 resume，`--resume` 列在 MVP 边界内（信封仅保留 session_id），多轮对话无处落地；`AgentRunRecord` 无来源字段，run 与业务对象（explore / change / 调试）无关联，未来跨源统计与调试页全局视图缺乏锚点。

---

## 提案

新增 desktop Explore 页面，对应 workspace 下 `openspec/explores/`：清单页 + 双栏详情页（左对话区、右 explore.md 实时预览），支持在页面内与 agent 多轮交互探索一个话题。四个支点：

1. **数据三分**：`ExploreRecord`（store，清单/绑定元数据，独立主键与文件名解耦）/ `explore.md`（磁盘，内容唯一真源，懒创建——新话题不预建空档、文件被删记录保留）/ `AgentRunRecord` + 事件（store，对话）。清单 DB 化，绑定是显式动作（导入扫描未绑定文件 / 新话题建档），agent 落盘的新文件不自动入清单。
2. **多轮会话走 resume 扩展**：`agent_start` 增可选 `resumeSessionId`（flags 组装 `--resume <id>`），不传即既有 one-shot 行为（向后兼容）；`AgentRunRecord` 原地演进（native_model v2）加 `source` / `source_ref` / `parent_run_id` 三个字段——统一 run 表不分表，explore 单会话链按 `(source="explore", source_ref)` 定位链头、沿 `parent_run_id` 回溯还原；重开 explore 双恢复：文档从磁盘读、对话重放该链落库事件。
3. **stance 内置前导**：desktop 内置精简 explore stance 模板拼进 prompt（接受与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源漂移，模板归属 desktop 包）；MVP 不接 questionnaire——`AskUserQuestion` 的 ToolUse 渲染为静态可读卡片，答案经 composer 文本输入并以 resume 续话。
4. **watch 通道（第二个被认可的推送语义）**：通用「watch 文件变化」失效信号通道——notify 订阅当前 `explore.md` 单文件，通知只携带「文件被修改」信号（不含内容），前端防抖后经 `read_explore` 显式拉取刷新；数据面维持取数纪律，订阅生命周期即详情页生命周期。

UI 采用 shadcn 官方 registry 件：`message-scroller` + `message`（2026-06 chat primitives，只管滚动行为，消息内容留在应用层）、`resizable`（react-resizable-panels，左右分栏拖分界）。事件呈现复用既有 `AgentEvent.Message.blocks` 四变体映射（Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 成对）与 `useAgentRun` / `useAgentRunHistory` 既有链路。页面导航照 `/changes` 路由模式平移（`/explores` + `/explores/:name`，URL 承载选中，workspace 切换 replace 回清单）。

---

## 能力

### 新增能力

- **desktop-explore-page** — Explore 页面：DB 清单页（按 workspace 过滤）、双栏详情页（对话区 + 预览）、新建流程（导入扫描绑定 / 新话题建档）、会话链（单链续话 + stance 前导 + 双恢复）、composer、workspace 切换回列表。
- **desktop-explore-queries** — workflow crate 查询层 explore 读面：`read_explore`（单文件读取，name 单分量防穿越）+ 导入扫描（列目录 `*.md`），命令层薄包装；查询层保持纯读，应用不落盘 explore.md（文件由 agent 会话流程创建）。
- **desktop-file-watch** — 通用单文件 watch 失效信号通道：notify 订阅、通知仅修改信号不含内容、无后端去抖、Channel 包装在壳层命令、与执行流通道并列的第二个推送语义。

### 修改的能力

- **desktop-workspace-store** — 新增 `ExploreRecord` 模型（native_model 新 id，独立主键、root 归属、绑定与时间戳）；`AgentRunRecord` native_model v1→v2 原地演进加 `source` / `source_ref` / `parent_run_id`；新增按来源单链还原查询与 explore 清单操作面。
- **desktop-agent-execution** — `AgentRunParams` 增 `resume_session_id`（`--resume` flag 组装，解除「无续会话」MVP 边界，`--continue` 仍不实现）；`agent_start` 增可选 resume 与来源参数并透传编排落库；MVP 边界条款相应改写（交互工具限制改为 explore 页静态卡片呈现）。
- **desktop-page-routing** — 路由表增 `/explores` 与 `/explores/:name`；侧栏「页面」组增 [探索] 入口（`nav-explores`）；「既有集成语义保留」中「无文件 watch」条款修订为指向 desktop-file-watch 唯一例外。

---

## 变更范围

### 实现文件

Rust 后端（`packages/desktop/src-tauri/`）：

- `crates/core/workflow/src/queries/explore.rs`（新）+ `queries/mod.rs`（导出）— `read_explore` / 导入扫描
- `crates/core/agent/src/runner.rs` — `AgentRunParams.resume_session_id: Option<String>`
- `crates/infra/agent/src/flags.rs` — `--resume <id>` 组装（None 时无此 flag）
- `crates/infra/store/src/model.rs` — `ExploreRecord`（新模型注册）；`AgentRunRecord` 加三字段（native_model version 1→2）
- `crates/infra/store/src/store.rs` — explore 清单操作（list / create / delete，按 root 过滤）+ 按 `(source, source_ref)` 的单链还原查询
- `crates/infra/watch/`（新 crate）— notify 单文件订阅，std mpsc 修改信号流，零 Tauri；注册进 workspace members
- `src/commands/explores/mod.rs`（新轨道）— `read_explore` / `scan_explores` / explore 记录 CRUD 薄命令（blank root 空结果纪律）
- `src/commands/watch/mod.rs`（新）— `watch_subscribe` / `watch_unsubscribe`，infra/watch 信号流桥接 Tauri Channel
- `src/commands/exec/mod.rs` + `src/commands/exec/agent.rs` — `agent_start` 透传 resume / source 参数；`run_agent()` 编排填充记录来源与链字段
- `src/main.rs` — 注册新命令
- `Cargo.toml`（根）— workspace member 与 notify 依赖

前端（`packages/desktop/src/`）：

- `routes.tsx` — `/explores`、`/explores/:name` 两路由
- `components/AppSidebar.tsx` — 「页面」组增 [探索]（`NavLink`，testid `nav-explores`）
- `views/explores/`（新目录）— `ExploreView`（清单 + 新建流程）、`ExploreDetailView`（双栏）+ `components/`（对话区 / composer / 预览 / 新建对话框）+ `hooks/`（清单、会话链、watch 防抖刷新）
- `lib/exploreStance.ts`（新）— 精简 explore stance 前导模板（注释互链 SKILL.md 双源）
- `types/dto.ts` — `ExploreRecord`、扫描结果等 DTO
- `package.json` / `components.json` — `pnpm dlx shadcn@latest add message-scroller message resizable`（引入 `react-resizable-panels`）

### 测试文件

- `crates/core/workflow/src/queries/explore_test.rs`（新）— 读取 / 防穿越 / 空目录扫描
- `crates/infra/agent/src/flags_test.rs` — resume 组装正反用例
- `crates/infra/store/src/model_test.rs` / `store_test.rs` — ExploreRecord CRUD、v1→v2 演进读取、单链还原
- `crates/infra/watch/src/*_test.rs`（新）— 修改信号、通知无内容、退订
- `src-tauri/src/commands/explores/mod_test.rs`、`src/commands/watch/mod_test.rs`（新）；`src/commands/exec/mod_test.rs` / `agent_test.rs` 扩展
- 前端：`src/__tests__/route_pages.test.tsx`（探索路由扩展）、`src/components/AppSidebar.test.tsx`（nav-explores）、`src/views/explores/**/*.test.tsx`（清单 / 详情 / 新建 / 会话恢复 / 防抖刷新）

### 删除文件

- 无（全新增 + 既有文件原地演进）

### 不要修改

- `crates/core/foundation/` — `explores_root` 已在，零改动
- change 域查询与产物层 — `queries/list.rs` / `queries/detail.rs` / `locate_change` / `artifacts/`（ArtifactEnvelope 不套 explore）
- `views/agent/`、`views/changes/`、`views/db/` — 既有页面结构不变（调试页加 source 徽标为 design 可选项，不构成本变更验收面）
- `plugins/` 与 openspec CLI — 本变更为 desktop 域
- store legacy 迁移层 — 已移除不重新引入，shape 演进由 native_model 版本机制承担

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `read_explore` 查询 + 命令 | 对含 `openspec/explores/foo.md` 的 workspace 调用返回文件文本；未知 stem 返回空；`a/b`、`..` 等穿越分量被拒；blank root 返回空结果 |
| AC-2 | 导入扫描 + 命令 | 扫描列目录内全部 `*.md`（stem 形式），目录缺失返回空；命令层以 store 清单滤除已绑定文件 |
| AC-3 | store `ExploreRecord` | 建档 / 按 root 清单 / 删除可用；独立主键与文件名解耦（文件改名记录不破）；磁盘文件删除记录保留；DB 查看器信封 API 零改动覆盖新模型 |
| AC-4 | `AgentRunRecord` v2 演进 | 含 v1 记录的存量库打开后既有 run 可重放且 `source` 缺省 `debug`；新写入 run 携带 `source` / `source_ref` / `parent_run_id` |
| AC-5 | 单链还原查询 | 同一 explore 的 resume 链（≥2 条 run，`parent_run_id` 相连）可按发起顺序完整还原并重放事件 |
| AC-6 | resume 透传 | `resume_session_id` 非空时 CLI 参数含 `--resume <id>` 且其余 flag 不变；None 时无此 flag；调试页不传新参数行为与现状完全一致 |
| AC-7 | watch 通道 | 修改被订阅文件 → 前端收到仅含修改信号的推送（防抖后）重读刷新；通知不含文件内容；详情页卸载后退订、无后续信号 |
| AC-8 | 清单页与新建流程 | `/explores` 清单来自 store 按当前 workspace 过滤；导入选中未绑定文件即建档；新话题建档后预览呈空态（应用不落盘 explore.md、不预建空档） |
| AC-9 | 双栏详情与会话链 | 详情双栏可拖分界；气泡映射四变体（Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 成对）；打开续最近链重放历史；发送拼 stance 且经 resume 续话；`AskUserQuestion` 呈现静态卡片 |
| AC-10 | 路由与导航 | `/explores`、`/explores/:name` 可达；`nav-explores` active 态由 URL 派生；详情页 workspace 切换 replace 回 `/explores`；未知路径仍回 `/changes` |
| AC-11 | 管线合规 | `vp check` / knip / `vp test` 全绿；`cargo test --workspace` 全绿（含 store / agent / workflow / watch 新测试）；无 knip 豁免新增 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| CLI `--resume` 语义歧义（fork vs 延续）导致会话链错乱 | 对话历史重放与实际不符 | 中 | E2E 实测 resume 续话并以 design 记录结论；`parent_run_id` 显式链指针 + `session_id` 双证；链还原查询收口单点 |
| native_model v1→v2 字段演进在存量库上异常 | 旧库不可读 / 数据丢失 | 低 | 新字段全部可缺省（`source` 缺省 `debug`、其余 `Option`）；旧版本记录读取测试覆盖；无 legacy 迁移层回归 |
| `message-scroller` registry 件形态与预期不符（较新官方件） | 对话区需自实现滚动 | 中 | 探索期已核实官方 chat primitives 家族与职责切分；落地验证失败则退自实现滚动容器并在 design 记录 |
| stance 模板与 `SKILL.md` 双源漂移 | 探索引导口径不一致 | 中 | 模板注释双向互链两源；漂移代价已拍板接受（模板归属 desktop 包） |
| notify 跨平台单文件 watch 陷阱（inotify rename 失效） | 将来跨平台丢事件 | 低 | 当前目标平台 Windows（父目录实现，rename-replace 原子保存不丢事件已核）；平台边界留痕，跨平台时再议 |
| watch 通道动摇「显式取数、无 watch」纪律 | 推送语义滥用蔓延 | 低 | 通道定位失效信号（无内容、无数据面、单文件）；`desktop-page-routing` 语义条款同步修订收口唯一例外地位 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 多轮对话实现方式 | 扩展调试接口：`agent_start` 增可选 `resumeSessionId`，flags 组装 `--resume` | 复用调试链路的自然延伸；向后兼容（不传即 one-shot）；`session_id` 字段本就是续会话预留 | 独立会话管理轨道（重）；`--continue` 隐式续最近（语义含糊不采） |
| questionnaire 是否进 MVP | 不进；`AskUserQuestion` ToolUse 渲染静态卡片，答案经 composer + resume | 首版收敛交互面；回程机制（resume 续话）已确定，二期接线成本可控 | 引入 `questionnaire` 官方件（二期再评估） |
| stance 注入通道 | desktop 内置精简前导模板拼 prompt | 无 skill 上下文注入通道，模板化最直接；接受双源漂移 | 读盘解析 SKILL.md（引入对 plugins 目录的运行时依赖，不采） |
| 会话恢复语义 | 每 explore 单会话链；打开默认续最近链，无链起链；无「新会话」动作 | MVP 最小面；链模型（parent_run_id）已支持未来多链 | 多链 + 新建会话按钮（二期） |
| run 与来源的关联形态 | 统一 `AgentRunRecord` 单表加 `source` / `source_ref` / `parent_run_id`，不按来源分表 | 共同面占绝对主导（13 字段 + 事件管线来源无关）；事件表全局 run_id 锚定；跨源查询（调试页全局视图 / 成本统计）是真实需求；加字段演进不对称成本低 | 按来源分表（fork store API × N、跨源 union，不采） |
| explore 清单来源 | store `ExploreRecord` DB 化 + 显式绑定（导入扫描 / 新话题），非实时扫目录 | 绑定与会话链成对持久；agent 落盘文件不自动入清单，避免噪音条目 | 实时扫目录（无绑定语义，弃） |
| `ExploreRecord` 身份 | 独立主键，与文件名解耦；文件懒创建、被删记录保留 | 改名/重建不破链；文件是记录的可丢弃投影 | 以文件名为主键（改名破链，弃） |
| watch 通道形态 | 通用单文件订阅；通知仅修改信号不含内容；前端防抖；后端无去抖 | 比执行流通道口子更小的推送语义；数据面维持显式取数 | 目录 watch（清单 DB 化后无消费者，弃）；轮询（违反取数纪律，弃） |
| 页面清单查询归属 | workflow 查询层收缩为 `read_explore` + 导入扫描；页面清单移 store | 清单是绑定元数据（store 域），目录扫描仅服务导入 | workflow 提供页面清单查询（跨 store 依赖，弃） |

### 待决问题

- 会话链锚形态：`ExploreRecord` 存 `head_run_id`（O(1) 链头，需与 run 终态双写）vs 由 `(source, source_ref)` 派生（无双写，靠索引扫描）——design 定。
- 记录与磁盘文件的路径关联形态：导入时锚定实际文件名的路径字段 vs 新话题按名派生，两种来源如何统一——design 定。
- 未落盘 / 已删除场景的 watch 处置细节（重挂时机）与前端防抖窗口取值——design 定。
- run 来源二级索引形态（`source` 单字段 vs 复合打包）与调试页是否加 source 徽标——design 定。
- `message-scroller` / `resizable` registry 件落地形态验证（add 成功性与组件 API 匹配度）——dev-design 前核实。

---
