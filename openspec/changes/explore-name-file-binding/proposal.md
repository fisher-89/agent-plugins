# 提案: explore-name-file-binding

> **变更**: explore-name-file-binding
> **日期**: 2026-10-10
> **状态**: draft
> **探索**: `openspec/explores/explore-name-file-binding.md`（三目的 + 四决策用户拍板在案）

---

## 问题

desktop 的探索（explore）记录与磁盘笔记文件存在命名双轨：`ExploreRecord.name` 同时承担「清单展示名」与「磁盘寻址键（笔记文件 stem）」两个职责，而 agent 落盘时自拟 topic 文件名，导致 DB 记录名与磁盘文件 stem 分叉——清单展示名、详情预览寻址（`read_explore(root, name)`）与 agent 实际写出的文件对不上，预览读不到内容，探索内容与清单条目无法关联。

用户补充的三个目的收敛到同一个结构性缺口：

1. **清单更好认出标题**——`ExploreRecord` 没有人类可读标题字段，`name` 被寻址职责绑架，放不下中文 / 空格 / 长标题；
2. **探索正确关联结果文件**——读面（`read_explore` / `explore_doc_path` / watch 订阅）硬编码 `explores/<name>.md` 单一目录单一文件名，结果一旦落别处就关联不上；
3. **探索 → 变更打通**——desktop 的 explore 记录与 change 零关联：没有 `linked_change` 字段、没有「从探索启动变更」入口、没有笔记 promote 路径。

当前代码库已具备的关键事实：`ChangeRecord` 已完成 **id 身份锚 + name 裸名属性**分离（desktop-change-db-identity），change 寻址恒以 id；`ExploreRecord.id` 是真正的身份锚（会话 `source_ref` 指向它），`name` 本可自由改名；`rename_explore_record` / 会话链级联删除 / watch 订阅已就绪；`ChangeRecord` 已示范 `native_model` 加字段的 decode-only 升级模式（`from = ChangeRecordV1` + `#[serde(default)]`）。

---

## 提案

把 `ExploreRecord.name` 的「展示名 + 寻址键」双职拆开，并补齐「标题」「promote 关联」两个维度；change 侧同步引入 `title`，使探索 → 变更的 move 语义成立：

1. **`ExploreRecord` v2**：`name` 退化为纯 stem 寻址键（升级为 kebab-case 口径，与 change name 对齐）；新增 `title`（人类可读标题，恒非空、默认 = name）与 `promoted_to`（`Option<String>`，指向已 promote 的 **change id**——身份锚，非 name，见决策 D1）。存量 v1 记录经 `native_model` decode-only 升级（`from = ExploreRecordV1`，`title = name`、`promoted_to = None`），零迁移。
2. **stance 注入 name + 标题行约定**：explore stance 前导模板注入当前记录 `name`（agent 落盘文件名 MUST 等于 `record.name`，MUST NOT 自拟），并明示「笔记首行写 `# <标题>`」。这同时止血「预览读空」的最痛问题（方向 1 最小贯通）。
3. **title 回填**：详情页 watch 重读笔记后解析首行 `# <标题>`，与 `record.title` 不同则调显式写命令 `update_explore_title` 回填（读是纯读、写是显式写命令，不破坏纯读纪律）。
4. **promote（move）命令**：探索详情页提供「启动变更」入口；`promote_explore` 读主仓笔记全文 → `create` 建 change（`name = explore.name`、`goal = 笔记全文`、`title = explore.title`，worktree 内写 `explore.md`）→ 删主仓 `explores/<name>.md`（move 的移走半边）→ `ExploreRecord.promoted_to = change.id`（记录打标，**会话链保留、MUST NOT 删记录**）。promote 后探索详情显示「已转变更」+ 跳转，不自动导航。
5. **`ChangeRecord` v4**：新增 `title`（恒非空，默认 = name）；change 列表 / 详情出线 `title`，前端清单与详情头渲染 `title`。手动新建 change 的 title 默认 = name（不加输入框），promote 路径显式继承 explore.title。
6. **explore 名称口径升级**：`create_explore_record` / `rename_explore_record` 校验由宽松单分量名升级为 kebab-case（与 `write::create` 同口径）；导入扫描命令过滤非 kebab stem（不列入可绑定清单）；存量非 kebab 记录保留可读，promote 前置拒绝并引导改名。

---

## 能力

### 新增能力

- 无。promote 语义沿既有 explore / change 两域能力落地，不新增独立能力面。

### 修改的能力

- **desktop-workspace-store** — `ExploreRecord` v1→v2（新增 `title` / `promoted_to`，decode-only 升级）；explore 建档 / 改名名称校验升级为 kebab-case；新增 `set_explore_title` 与 `mark_explore_promoted` 两个 store 操作面。
- **desktop-explore-page** — 清单条目 / 详情头渲染 `title`；详情页 promoted 态（「已转变更」徽标 + 跳转）；「启动变更」promote 入口；新建话题本地 kebab-case 校验；title 回填（watch 重读后解析首行标题并回填）；stance 注入当前记录 name 与首行标题约定。
- **desktop-explore-queries** — 新增 `promote_explore` 与 `update_explore_title` 两条命令；导入扫描命令层过滤非 kebab stem；workflow 写面新增探索笔记搬运（move）半边。
- **desktop-change-create** — 写面 `create` 新增 `title` 入参（空/空白回退 name，title 恒非空单点）；手动新建 change 的 title 默认 = name；`promote_explore` 复用 `create` 建 change。
- **desktop-change-state-store** — `ChangeStateRecord` / `ChangeRecord` 新增 `title`（`native_model` v3→v4 decode-only 升级）；`change_state()` 映射单点新增 title。
- **desktop-change-queries** — `ChangeSummary` / `ChangeDetail` 出线 `title`（wire contract 演进走 golden 显式重写）。
- **desktop-change-flow-view** — change 清单条目与详情头渲染 `title`（空态回退 name 语义经「title 恒非空」不变量消除，legacy 记录经升级 `title = name`）。
- **desktop-corpus-regression** — db 种子语料与 golden 覆盖 title 两态投影（title=name 缺省样本与显式标题样本各至少一个）。

### 引用沿用（零 delta）

- desktop-data-dimensions — `ExploreRecord` / `ChangeRecord` 仍归 workspace 维度，字段增量不改维度归属。
- desktop-change-worktree — promote 复用 `create` 的 worktree 建域与 bootstrap，无新增 worktree 语义。
- desktop-file-watch — title 回填复用既有 watch 失效信号通道，订阅面零改动。
- desktop-ipc-type-bindings — 新 DTO 字段与新命令经既有 specta 管线出线，管线本身零改动。
- desktop-artifact-plugins — change 的 `explore.md` 经既有 markdown-doc 插件收录，promote 移入后自然可见。
- desktop-change-orchestration — promote 不触及 run 编排；`create_change_with` 仅机械增 title 参数。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `ExploreRecord` v2（新增 `title` / `promoted_to` + `ExploreRecordV1` 与双向 `From`）；`ChangeRecord` v3→v4（新增 `title` + `ChangeRecordV3` 与双向 `From`）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — `change_state()` / `create_change_record` 映射 title；explore 建档 / 改名 kebab-case 校验；新增 `set_explore_title` / `mark_explore_promoted`
- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — `ChangeStateRecord` 新增 `title`
- `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` — `create` 新增 `title` 入参（空/空白回退 name）；`ChangeStateRecord` 建档载荷新增 title
- `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs`（新）— `promote_explore`：调 `create`（goal=笔记全文、title=explore.title）成功后删主仓 `explores/<name>.md`（move 半边）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` — 导出 `promote_explore`；`create` 签名演进
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` — `ChangeSummary` 新增 `title`，`db_entry` 映射
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `ChangeDetail` 新增 `title`，聚合映射
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — `create_change_with` 增 title 参数（`create_change` IPC 保持三参，内部 title=name）
- `packages/desktop/src-tauri/src/commands/explores/mod.rs` — 新增 `promote_explore` / `update_explore_title` 命令；`scan_explores` 命令层过滤非 kebab stem
- `packages/desktop/src/views/explores/explore-view.tsx` — 清单条目渲染 `title`（挂 `data-testid`）
- `packages/desktop/src/views/explores/explore-detail-view.tsx` — 详情头渲染 `title`；promoted 态（「已转变更」+ 跳转）；「启动变更」入口；title 回填 effect
- `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` — 新话题本地 kebab-case 校验
- `packages/desktop/src/views/explores/hooks/use-explore-list.ts` — 新增 `updateTitle` / `promote` 动作（invoke 后 refresh）
- `packages/desktop/src/views/explores/hooks/use-explore-session.ts` — `send` 组装时注入 `record.name` 至 stance
- `packages/desktop/src/lib/explore-stance.ts` — `buildExplorePrompt(userInput, topic)` 注入 name + 首行标题约定
- `packages/desktop/src/lib/explore-title.ts`（新）— `extractMarkdownTitle` 纯函数（首行 `# 标题` 解析）
- `packages/desktop/src/views/changes/change-list-view.tsx` — 条目渲染 `summary.title`
- `packages/desktop/src/views/changes/change-detail-view.tsx` — `DetailHeader` 渲染 `detail.title`
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO / 命令演进重生成
- `packages/desktop/package.json` — `version` 0.4.32 → 0.4.33

### 测试文件

- `crates/infra/store/src/model_test.rs` / `store_test.rs` — ExploreRecord v1→v2 与 ChangeRecord v3→v4 decode-only 升级、kebab 校验、`set_explore_title` / `mark_explore_promoted`
- `crates/core/workflow/src/write/create_test.rs` — `create` title 入参（默认 name / 显式 title）
- `crates/core/workflow/src/write/promote_test.rs`（新）— promote move 语义（create + 笔记删除 + 失败补偿）
- `crates/core/workflow/src/queries/list_test.rs` / `detail_test.rs` — `title` 投影
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/` — title 字段 golden 显式重写
- `src/commands/explores/mod_test.rs` — `promote_explore` / `update_explore_title` 命令、`scan_explores` kebab 过滤
- `src/commands/changes/mod_test.rs` — `create_change_with` title 默认
- `src/views/explores/*.test.tsx` — title 显示、promoted 态、promote 入口、kebab 校验、title 回填
- `src/lib/explore-title.test.ts` / `explore-stance.test.ts` — 标题解析与 stance 注入
- `src/views/changes/change-list-view.test.tsx` / `change-detail-view.test.tsx` — title 显示断言

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（MCP 工具、hooks、CLI 工作流、版本 2.10.44、三类交付产物）
- 相位表与 prompt 模板（`phase_table.rs`）——change 的 explore.md 仍作为 free-form 探索上下文被 proposal 相位既有交接行读取，零新占位符
- workflow.json 领域模型与双向墙（desktop 全链零 workflow.json 触点）
- `ChangeRecord` 身份锚（id 主键不变，name 恒裸名属性不变）
- workflow 查询层 `read_explore` / `explore_doc_path` / `scan_explores` 的宽松单分量名校验（读旧文件语义保留）
- change 归档 / 编排 / worktree 建域既有语义
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | ExploreRecord v2 升级 | 含 v1 记录的存量库 additive 打开，旧记录读出 `title = name`、`promoted_to = None`（decode-only，零手工迁移）；新记录 `title` / `promoted_to` 回环逐字段一致；`list_models` / `scan` 信封 API 零改动覆盖 |
| AC-2 | ChangeRecord v4 升级 | 含 v3 记录的存量库 additive 打开，旧记录读出 `title = name`；`change_state` 映射与建档回环一致；change 清单 / 详情出线 title |
| AC-3 | explore 名称 kebab 口径 | `create_explore_record` / `rename_explore_record` 拒绝大写 / 下划线 / 空格 / 前导数字等非法 kebab；导入扫描不列非 kebab stem；存量非 kebab 记录仍可读、promote 显式拒绝 |
| AC-4 | stance 注入 name | 任一 explore run 的 prompt 原文含当前记录 name（落盘文件名 MUST 等于 name）与「首行写 `# 标题`」约定；调试页 run prompt 不含该模板 |
| AC-5 | title 回填 | 笔记首行 `# 标题` 与 record.title 不同时，watch 重读后经 `update_explore_title` 回填；纯读路径（read_explore）零写入；回填成功后清单与详情头 title 一致 |
| AC-6 | promote move 语义 | 合法 kebab explore（笔记已落盘且非空白）promote 成功：worktree 内 `changes/<name>/explore.md` 内容为笔记全文、ChangeRecord.title = explore.title、主仓 `explores/<name>.md` 已删、ExploreRecord.promoted_to = change.id、名下会话链保留（记录不删） |
| AC-7 | promote 拒绝面 | 笔记未落盘 / 内容空白 / 非 kebab name / 已 promoted / 同名 active change 冲突 / 分支或 worktree 目录冲突——各返回显式 `Err`，explore 记录与笔记零改动（change 侧 create 失败补偿链由既有 create 语义承担） |
| AC-8 | promoted 详情态 | promote 后探索详情显示「已转变更」+ 跳转（路由 `/changes/<change_id>`），预览不再读空；手动新建 change 后 change 详情头 / 清单条目显示 title（默认 = name） |
| AC-9 | change title 出线与 golden | `ChangeSummary` / `ChangeDetail` 增 title 字段经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕；bindings 一致性守卫绿 |
| AC-10 | 版本交付 | `packages/desktop` version 0.4.33；`plugins/dev-team` 零改动；`vp test` / `client:check` / knip 全绿 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 promote 后段失败（笔记已删但 promoted_to 未打标，或打标前崩溃） | explore 记录指向已移走文件，详情空态 | 中 | promote 顺序：create → 删笔记 → 打标；失败显式呈现残留对象与手动恢复指引（笔记全文已在 change explore.md 留底，可回写）；record 不删故会话链不丢 |
| R2 title 回填与 agent 落盘竞态（watch 解析到的首行标题是中间态） | title 短暂非最终值 | 低 | 回填只在内容稳定后的 watch 防抖重读上解析；title 可再次回填覆盖，幂等 |
| R3 kebab 口径升级影响存量 explore（非 kebab 文件不再可导入 / 不可 promote） | 用户存量笔记绑定与 promote 受限 | 中 | 存量记录保留可读；promote 显式引导改名（`rename_explore_record`）；边界留痕 |
| R4 手动新建 change 的 title 默认 = name 可读性弱 | 清单标题仍为 kebab | 中 | V1 接受（决策 D4 默认 = name）；后续变更可加 title 输入或 agent 派生 |
| R5 native_model 双模型升级（ExploreRecord v1→v2、ChangeRecord v3→v4）破坏存量库 | 存量 workspace 库不可读 | 低 | decode-only 先例模式（AgentProviderRecord / ChangeRecord 既有先例）+ 升级回归测试为 AC-1 / AC-2 |
| R6 change title 四层贯穿（state / store / queries / 前端）传播面 | 编译面连锁、wire contract 漂移 | 低 | 字段为纯投影零行为；golden 显式重写 + bindings 守卫（AC-9） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| `promoted_to` 存什么 | 存 change **id**（身份锚），非 name | change 寻址已 id 化（desktop-change-db-identity），跳转与一切后续寻址恒以 id；explore.md 蓝图「存 name」与身份锚约定冲突，按既有约定修正 | 存 name（需 name→id 反查，name 无唯一约束，反查歧义） |
| `title` 非空不变量 | `title` 恒非空：创建 / 升级默认 = name，agent 回填覆盖 | 消除前端空态回退分支；legacy 升级后展示与现状一致 | title 可空 + 前端回退 name（fallback 逻辑散落各渲染点） |
| explore 名称校验 | 建档 / 改名升级 kebab-case（与 change 对齐） | 决策 3「change name 从 explore 文件名继承」的隐藏前提；promote 免转换冲突 | promote 时转换（可能冲突，且回到命名分叉） |
| 导入非 kebab 文件 | 导入扫描命令层过滤非 kebab stem（不列入可绑定） | 建档已拒非 kebab，过滤避免「点击后报错」 | 允许绑定非 kebab（与 kebab 建档口径矛盾，promote 前置仍拒） |
| 手动新建 change title | 默认 = name，不加输入框 | 最小改动；promote 路径显式继承 explore.title；后续可加输入 | 对话框加 title 输入（扩大 UI 面，V1 非必要） |
| promote 全文含标题行 | 保留笔记全文（含首行 `# 标题`） | 「move 全文」语义最纯粹、零内容转换、promote 不解析笔记结构 | 去标题行只留正文（需第二处解析点，且与「全文」决策相悖） |
| stance 注入方式 | `buildExplorePrompt(userInput, topic)` 注入 record.name | 止血预览读空的最小贯通（方向 1），title 回填在其上叠加 | agent 自拟 topic（原问题复现） |

### 待决问题

- `promote_explore` 命令与 workflow 写面 `promote_explore` 的边界细则：change create 复用形态（直接调 `write::create` vs 抽 `promote` 组合）与失败补偿链（create 成功后删笔记失败 / 打标失败）由 design 定稿。
- title 回填的防抖窗口与「内容稳定」判定（watch 防抖 500ms 是否足够、是否需 run 收口定点重读后再次解析）由 design 定稿。
- `set_explore_title` 是否接受空白 title（当前建议拒绝，title 恒非空）；agent 未写标题行时 title 保持 name 的兜底展示是否满足「清单标题」目的，由 design 复核。
- promote 后 explore 详情「已转变更」展示是否解析 change title（当前建议仅以 id 链接跳转，不反查标题）——design 定稿呈现形态。
