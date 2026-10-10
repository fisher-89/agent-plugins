# 提案: desktop-change-db-identity

> **变更**: desktop-change-db-identity
> **日期**: 2026-10-10
> **状态**: draft

---

## 问题

change 在 workspace 库里以 **name 作主键**（`ChangeRecord.name`，物理表名 `9_2_name`），而 `list_changes` 是 **db 记录 ∪ 磁盘目录**的去重并集——归档条目出线的是磁盘目录名（含 `YYYY-MM-DD-` 日期前缀）。前端拿这个前缀名作路由参数回查，链路必然打空：

| 环节 | 现状 | 后果 |
|------|------|------|
| 列表出线（`queries/list.rs:117-137` `db_entry` → `locate_prefixed_archive_name`） | 归档条目 `name` = 磁盘目录名 `2026-02-02-seeded-archived` | 前缀名成为前端眼里的「change 名」 |
| 前端路由（`views/changes/change-detail-view.tsx:347` `useParams<'name'>` + 列表行 `navigate('/changes/${summary.name}')`） | 以该前缀名进详情 | 前缀名回传后端 |
| 详情查询（`queries/detail.rs:185` `store.get_change(name)`） | 主键是裸名 `seeded-archived` → miss | `record = None` |
| 磁盘定位（`queries/mod.rs:52` archive 树精确名命中） | 带前缀目录命中 → 有 location | 落进文档形态分支 |
| 视图分流（`change-detail-view.tsx:87` `detail.status === null`） | `status` 为 null → `flow-empty` 占位 | **已建档 change 的流程图被「文档形态：未建档，仅产物清单」顶掉** |

根因不是某处漏改，而是 **name 一职三任**：既是库身份主键，又是磁盘寻址键，还是前端路由键——归档改名（加日期前缀）一发生，三者就错位；「文档形态」分支（db 缺记录的存量 CLI change）又为这条错位链路提供了不报错的伪装出口，使 bug 静默呈现为「图没了」。

结构性矛盾还有两处：

1. **两个真相源**。清单是 db ∪ 磁盘并集：无 db 记录的 CLI 建 change 以「文档形态」入列（`status` 恒 null），有记录的以状态面入列。同名共存靠 `db_names` 去重，恰恰是身份歧义的温床。
2. **身份键散落全链**。run 注册表复合键 `(root, change)`（`control.rs` 12 处）、`SessionAnchors` 的 `(change, run_id)`、agent 会话 provenance `source_ref = <change>/<phase>/<role>/<attempt>`（`walker.rs:918-927`，归档链两条同式）、`envelope.rs` 的 change 信封 key、13 条 IPC 命令的 `change` 参数、前端路由与行键——全部以 name 为身份。name 一日不可变，则归档前缀之流的外因总能再次顶穿。

## 提案

**change 身份锚换 `id`（uuid 串，建档铸出、终身恒定、不复用）**，name 降为可变属性；列表停扫磁盘（db 为唯一基准数据源）；旧库作废重建。四刀：

1. **主键换锚**：`ChangeRecord` 主键 `id`（String，UUID 形态），`name`（String，恒裸名）为普通可变属性；`PhaseRecord.change` / `StepRecord.change` / `RunRecord.change` 一律改 `change_id` 归属。一切寻址（库查询、命令面、前端路由 `/changes/:id`、run 注册表 `(root, id)`、provenance source_ref、会话反查）以 id 为准；**磁盘与 git 面仍 name 化**（change 目录、worktree 落位 `worktrees/{身份段}/<name>`、branch `change/<name>`、归档目录 `YYYY-MM-DD-<name>` 前缀与 `locate_prefixed_archive_dir` 后缀扫描——判定点 `status` + 后缀匹配不变），由 **id → 记录 → name** 的分辨率单点供给。name 与磁盘目录名不一致成为合法常态，错位类 bug 结构性消失。
2. **唯一基准数据源**：`list_changes` 停扫磁盘（active / archive 两树扫描与文档形态条目整体退役），列表 = db 全量记录；**磁盘-only 的 CLI 建 change 在 desktop 不再可见**（deliberate 能力回退）；详情查询的文档形态分支删除——未知 id 走「未找到」降级，MUST NOT 回退目录解析、MUST NOT 呈文档形态空面。归档前缀不再出线给前端（清单行 `name` 恒裸名，时间语义由月分组承载）。
3. **provenance 对齐**：`source_ref` 身份段换 id（`<id>/<phase>/<role>/<attempt>`，归档链 `<id>/archive/spec-sync` / `<id>/archive/merge-conflict`）——explore 会话以记录 id 作 source_ref 是既有先例，change 会话是最后的可变名异常点；旧库既已作废，无历史会话包袱。
4. **旧库作废重建**：store 格式版本 bump（change 身份锚形态版本）；打开时的版本缺失（旧 name 主键形态库）或低于当前版本 → **旧库整体作废并重建全新空库**，零迁移层、零 name→id 解码链。历史 phase / run / 会话转录数据损失为既定接受面（用户拍板）；全局库（workspace 注册表）不受波及。

出线面：`ChangeSummary` / `ChangeDetail` / `CreateOutcome` 增 `id`（golden 显式重写流程，`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）；通知信封 `RunNotice` 为零载荷 kind-only，零改动。

## 能力

### 新增能力

- 无（本变更修改十个既有能力）。

### 修改的能力

- **desktop-change-state-store** — ChangeRecord 换锚（id 主键 + name 可变属性）；per-change 表（PhaseRecord / StepRecord / RunRecord）`change_id` 归属；port 缝（`ChangeStateStore`）与命令载荷 id 化；v1 解码链退役；归档双写 id 语义修订。
- **desktop-change-queries** — 列表 db 单源（停扫磁盘、文档形态条目退役）；详情 id 寻址（文档形态分支删除）；`ChangeSummary` / `ChangeDetail` 增 id、name 恒裸名；归档月分组 `archived_at` 唯一权威。
- **desktop-change-flow-view** — 两态降级收敛单态（`flow-empty` 文档形态占位分支删除，未找到降级沿用）；运行控制入口与转录联动 id 化（sourceRef 定式身份段换 id）。
- **desktop-change-archive** — 归档入口 / 编排链 id 寻址；run 互斥复合键 `(root, id)`；spec 同步 agent provenance source_ref 身份段 id。
- **desktop-change-orchestration** — 运行控制命令面按 id 发起（并行冲突键 `(workspace root, change id)`）；会话挂靠 provenance `source_ref=<id>/…`。
- **desktop-change-create** — 写面 create 铸出 id 并建档；`CreateOutcome` 增 `id`；清单页新建成功后导航 `/changes/<id>`。
- **desktop-app-shell** — change 域命令面 id 寻址新增（change 域三读一写 + change_flow / archive_flow 全域命令参数 id）。
- **desktop-page-routing** — 路由表 `/changes/:id`；选中态 URL 化与清单行导航 id 化。
- **desktop-corpus-regression** — 语料 id 归键（种子构造携 id）；文档形态样本退役；列表 golden 改 db 单源断言；id 字段入 golden。
- **desktop-workspace-store** — workspace 库格式版本与旧库作废重建（打开探测、整体丢弃、零迁移层、全局库不受波及）。

### 引用沿用（零 delta）

- desktop-change-worktree — 双 root 组合与库身份锚定不变：worktree 落位 / branch 仍 name 化，exec root 自记录（现以 id 读取）解析，`locate_change`（worktree + name 两参由记录供给）回退语义零改动。
- desktop-ipc-type-bindings — DTO 演进经 bindings 再生成惯例（一致性守卫拦截漂移）。
- desktop-db-inspector — 信封 API 只读边界不变（change 信封 key 投影随主键自然改呈 id，查看器零模型代码）。
- desktop-data-dimensions — 两维度划分与落盘位置不变（change 状态仍归 workspace 维度）。
- desktop-agent-chat-infra — 会话查询 / 转录重放机制零改动（仅 source_ref 字符串的组装值更换）。

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `ChangeRecord` 换锚（`id` 主键 + `name` 属性）；`PhaseRecord.change` / `StepRecord.change` / `RunRecord.change` → `change_id`；native_model 版本段演进（表名 `9_2_name` → 新版本 + `id` 键，旧表对新读面不可见）；`ChangeRecordV1` 解码链退役
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — change 域操作面 id 化；create 同名 active 冲突前置改扫描查重（name 无唯一约束）；run_finish / 启动标定按 id join 与清位；打开路径的格式版本探测与旧库作废重建
- `packages/desktop/src-tauri/crates/infra/store/src/change_port.rs` — `ChangeStateStore` 适配全方法 id 化
- `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` — change 信封 key 投影随主键改 `id`
- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — port trait 全签名 id 化；`ChangeStateRecord` 增 `id`、name 降属性；命令载荷（PhaseLog / Backtrack / Step / RunStart / RunFinish）`change_id`
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` — 列表 db 单源（`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` 段删除；签名收敛）；`ChangeSummary` 增 `id`、name 恒裸名；月分组取自 `archived_at`
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — id 寻址（`get_change(id)` → 记录 → name / worktree 定位）；文档形态分支删除；`ChangeDetail` 增 `id`
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs` — `locate_change` 消费面随 id → name 分辨率调整（后缀扫描保留）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/`（create / archive / phase_start / phase_log / backtrack / decision_log / phase_next / run）— id 归键；create 铸 id；archive 经 id → 记录 → name 做目录改名；`SessionAnchors` 键 id
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` — 注册表键 `(root, change)` → `(root, id)`
- `packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs` — `ArchiveControl` 键 id；spec 同步 / 解冲突 agent provenance source_ref 身份段 id
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `RunRequest` / 载荷 change → id；provenance 定式 `<id>/<phase>/<role>/<attempt>`
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` + `steps.rs` + `run_history.rs` + `snapshot.rs` — 相位机步与 run 史载荷 id 化
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — `get_change_detail` / `read_artifact` / `archive_change` 参数 id；`create_change` 返回 id
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` + `archive_flow/mod.rs` — 两命令组全域命令参数 id（装配与互斥校验随动）
- `packages/desktop/src/types/generated/bindings.ts` — 随命令面与 DTO 再生成（一致性守卫拦截漂移）
- `packages/desktop/src/routes.tsx` — `/changes/:name` → `/changes/:id`
- `packages/desktop/src/views/changes/change-list-view.tsx` — 行键 / 导航 id 化；归档行显示裸名
- `packages/desktop/src/views/changes/change-detail-view.tsx` — `useParams<'id'>`；`flow-empty` 文档形态分支删除；页内身份源统一 id
- `packages/desktop/src/views/changes/hooks/`（use-change-list / use-change-detail / use-change-flow-run / use-archive-flow）— id 参数与 DTO id 消费
- `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` + `flow/detail-drawer.tsx` — sourceRef 定式组装身份段 id
- `packages/desktop/src/views/changes/flow/run-control-panel.tsx` + `archive-panel.tsx` — id 透传（展示面仍 name）
- `packages/desktop/package.json` — `version` 0.4.30 → 0.4.31（**归档时执行**；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）

### 测试文件

- `crates/infra/store/src/store_test.rs` / `change_port_test.rs` / `model_test.rs` — id 归键回环；旧形态库作废重建；表名版本段与旧表不可见断言；v1 解码链退役
- `crates/core/workflow/src/queries/list_test.rs` / `detail_test.rs` / `mod_test.rs` — db 单源列表、磁盘目录零发现、id 寻址、归档 change 全状态面详情
- `crates/core/workflow/src/write/*_test.rs` — id 载荷与 create 铸 id
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/golden/*.json` — 语料 id 归键 + 文档形态样本退役 + `DESKTOP_GOLDEN_REWRITE=1` 显式重写
- `crates/core/orchestration/src/control_test.rs` / `archive_flow_test.rs` / `walker_test.rs` — 注册表键 id、provenance 定式、互斥语义
- `src-tauri/src/commands/changes/mod_test.rs`（含 :262 归档条目名断言改写）/ `change_flow/mod_test.rs` / `archive_flow/mod_test.rs` — id 参数与列表 / 详情断言重写
- `packages/desktop/src/views/changes/**.test.ts(x)` + `src/app.test.tsx` — 路由 / 行键 / sourceRef / `flow-empty` 退役断言

### 删除文件

- 无整文件删除（list 磁盘扫描段、detail 文档形态分支、`ChangeRecordV1` 解码链、`flow-empty` 分支为段级删除）。

### 不要修改

- `plugins/dev-team` 全部（版本 2.10.44、三类交付产物）
- 磁盘 / git 的 name 化面：change 目录名、worktree 落位与 branch `change/<name>`、归档目录 `YYYY-MM-DD-<name>` 前缀与 `locate_prefixed_archive_dir` 后缀扫描判定
- 存量 CLI change 的磁盘目录与 workflow.json（零触碰、零迁移、零建档）
- 会话转录落库机制 / StopRegistry / `agent_sessions` 查询语义（仅 source_ref 字符串值更换）
- 相位机语义（phase_next / phase_log / backtrack 的校验与白名单）与 run 双写时序（每 run 两写）
- 通知信封 `RunNotice`（零载荷 kind-only，无 id 迁移面）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 身份锚换 id | 建档 change 铸出 uuid 形态 `id`（主键）；`name` 为无唯一约束的可变属性；相位 / checklist / 步骤 / run 史行均以 `change_id` 归属；重开库回环读写逐字段一致 |
| AC-2 | 归档错配 bug 修复 | 打开归档 change（磁盘目录带 `YYYY-MM-DD-` 前缀）详情（经 id 寻址）：`status=archived`、9 站 pipeline 与 runs 全量出线，**MUST NOT** 落「文档形态」空面；列表归档条目 `name` 为裸名、`id` 恒在案 |
| AC-3 | 列表 db 单源 | `list_changes` 源码零磁盘扫描触点；主仓存在磁盘-only（CLI 建、无 db 记录）目录时清单零呈现该条目（不入列、不报错、目录字节零变化）；归档月分组以 `archived_at` 为权威 |
| AC-4 | 详情文档形态退役 | db 无该 id 记录时详情返回「未找到」降级，MUST NOT 回退磁盘目录解析、MUST NOT 返回空流水线文档形态；`flow-empty` 占位分支删除（前端零 `文档形态` 文案消费） |
| AC-5 | 命令面 id 寻址 | `get_change_detail` / `read_artifact` / `archive_change` / change_flow 五命令 / archive_flow 五命令的定位参数均为 change id；id → 记录 → name 分辨率单点（branch `change/<name>`、worktree、目录定位仍 name 化）；bindings 再生成一致性守卫绿 |
| AC-6 | 注册表与 provenance | run / archive 注册表键为 `(workspace root, change id)`（12 处 name 键清零）；`source_ref` 定式 `<id>/<phase>/<role>/<attempt>` 与 `<id>/archive/*`；前端反查（detail-drawer / use-session-transcript）与服务端写侧逐字一致 |
| AC-7 | 前端 id 全链 | 路由 `/changes/:id`；列表行键 / `navigate` 均为 id；`useChangeDetail` 等 hooks 参数 id；页内单一身份源（URL id），无 `detail.name` 作寻址残留 |
| AC-8 | 旧库作废重建 | 打开含 name 主键形态 ChangeRecord 的存量 workspace 库：打开成功、旧数据零可达（重建为空库）、零迁移代码路径、零报错；全新库版本标记就位且重开不触发作废；全局库 workspace 注册表不受波及 |
| AC-9 | 语料与 golden | 语料种子 id 归键、文档形态样本退役；list / detail golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕（含 id 字段与 db 单源列表）；bindings:check 绿 |
| AC-10 | 全管线回归 | `cargo test --workspace`（src-tauri 根）绿、`pnpm -C packages/desktop run check` / `client:check`（含 knip）/ `vp test` 全绿，无新增豁免条目 |
| AC-11 | 版本交付 | 归档时 `packages/desktop/package.json` version 0.4.30 → 0.4.31（用户可见变更：归档详情修复 + 文档形态回退）；`plugins/dev-team` 零改动 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 数据损失与能力回退：旧库作废（phase / run / 会话转录史全失）；磁盘-only 的 CLI change 在 desktop 不再可见 | 用户可见回退与历史数据丢失 | 高（确定发生） | 用户拍板接受（作废重建 + 唯一基准数据源）；能力回退在 spec 显式留痕（不入列 / 不建档 / 零触碰）；CLI 建 change 仍可全程经 CLI 处置 |
| R2 身份键散落面广（10 capability、约 12 处注册表键、13 条 IPC 命令、provenance、路由、行键、语料） | 漏改某处以 name 寻址 → 静默 miss 复现同类 bug | 中 | AC 逐面机械断言；「name 非寻址键」grep 清单收口（`(root, change)` / `sourceRef` / `change:` 参数）；bindings 与 tsc 全量类型检查拦截漂移 |
| R3 前端两身份源（URL 参数 vs `detail.name`）剩一处漏改 | 页内寻址不一致，归档 / 运行控制面板以错值回传 | 中 | AC-7 断言单一身份源；detail / hook 测试逐条对齐 |
| R4 source_ref 格式变更打破会话反查 | 旧会话不可达 | 低 | 旧库整体作废已令存量会话消失，无历史包袱；新旧格式无并存期（单次切换） |
| R5 id 铸出点与确定性（uuid 随机性 vs 语料 golden 确定性） | 语料不可复现 / core 依赖边界争议 | 中 | 铸出点定稿后以可注入形态支撑语料（固定 id 种子）；uuid 属普通依赖（specta / serde 先例）不破 crate 依赖方向 |
| R6 golden 与测试重写面大（全部语料 id 字段 + 列表 db 单源 + 命令测试断言） | diff 人工确认成本、误改漏改 | 中 | 显式重写流程逐字执行（`DESKTOP_GOLDEN_REWRITE=1` + 预期范围留痕 + 复核）；测试重写与实现同 change 收口 |
| R7 name 无唯一约束后同名并存（active + archived 同名各一行） | 清单可读性（两行同名、无日期前缀） | 低 | 月分组与 status 徽章区分；title 展示演进（explore-name-file-binding 草案）须以本变更 id 主键为前提重新定形，届时偿还 |
| R8 归档链 / 互斥错误文案仍按 name 组织 | 文案与 id 寻址错位，用户难定位 | 低 | 错误面文案随动（设计阶段清单化）；AC 覆盖拒绝面带 id 语境 |

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 身份锚载体 | `id`（uuid 串）主键 + `name` 可变属性 | 用户拍板：一切寻址单一 id，归档前缀 / 磁盘名错配类 bug 结构性消失 | name 继续作主键 + 磁盘名映射层（错配根源仍在）；max+1 计数 id（ExploreRecord 先例——可复用，不适合作跨表长期引用） |
| 列表数据源 | db 单源（停扫磁盘，文档形态分支整体退役） | 用户拍板：唯一基准数据源；文档形态是错位链路的伪装出口，一并拆除 | 维持 db ∪ 磁盘并集（两真相源）；保留文档形态只读（详情分支继续分叉） |
| 旧库处置 | 格式版本 bump + 旧库作废重建（零迁移层） | 用户拍板：历史 phase / run / 转录损失接受；避免一次性 name→id 迁移（高风险、低复用价值） | native_model decode-only 迁移链（name → id 映射，迁移层与解析分支双负担）；手工迁移脚本 |
| provenance source_ref | 身份段换 id（`<id>/…`） | 单一身份锚贯穿；explore 会话以记录 id 作 source_ref 是既有先例；旧库已作废无历史包袱 | 保留 name 段（身份锚二源化，改名 / 同名再出歧义） |
| 磁盘与 git 面 | name 化不变（目录名 / branch `change/<name>` / 归档前缀），经 id → 记录 → name 分辨率供给 | 磁盘可读性与既有 git 约定零扰动；改名能力（后续）只需记录 + 目录两处协同 | 目录 / branch 也 id 化（可读性损失、git 面大改、与既有归档链全冲突） |
| 前端路由 | `/changes/:id` | URL 与身份锚单一来源；归档行不再暴露日期前缀名（时间语义归月分组） | 保留 `:name`（前缀错配 bug 原地复发） |
| 出线 DTO 面 | `ChangeSummary` / `ChangeDetail` / `CreateOutcome` 增 `id`；存档面 DTO（ArchivePreflight / ArchiveSummary / ArchiveOutcome）保持 name / archivedDir 展示语义不增 id；`RunNotice` 零改动 | 寻址面必须有 id（路由 / 导航）；存档面命令已按 id 发起、回包无 id 消费者，展示值 name 语义不变；通知零载荷无需迁移 | 全部 DTO 无差别增 id（golden 面与人工确认成本扩大，无消费者） |
| 版本交付 | AC-11 声明 + 归档时执行 bump（0.4.30 → 0.4.31） | 用户可见变更（归档详情修复 + 文档形态回退）；版本提升不入 spec requirement（既有约定） | 写 spec 版本 requirement（废弃形态，早前惯例已废止） |

### 待决问题（design 相位定稿，不阻塞本 proposal）

- id 铸出点：写面 `create` 直铸（uuid 入 core/workflow 依赖）vs 命令层铸出经入参传入；uuid 版本（v4 / v7）与语料确定性注入形态（固定 id 种子）。
- store 格式版本载体：库级版本标记形态（打开探测 + 文件删除重建 vs 旧文件换名留档后新建）与既有「旧库惰性废弃」条款的衔接措辞。
- 作废重建的执行时序：产物版本探测在 workspace 库首次打开（注册预开）时执行，还是应用启动批量执行（影响错误呈现与启动耗时）；库列名口径已定（per-change 表 `change_id`、IPC 参数 `id`）。
- 归档条目展示面：清单行 name 恒裸名后，归档日期是否以行内辅助信息补充（纯展示，月分组已承载时间语义）。
- 前端页内身份源合一的 props 形态：`change-detail-view` 向 `DetailDrawer` 传 id（现传 `detail.name`）的命名与传参收敛方式。
- name 唯一性回退的边界确认：create 前置「仅拒 main active 目录同名 / 同名 active 记录」在 id 主键下的精确语义（归档同名共存合法化是否需在 create 对话框提示）。
