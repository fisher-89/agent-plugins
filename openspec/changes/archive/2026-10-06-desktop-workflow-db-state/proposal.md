# 提案: desktop-workflow-db-state

> **变更**: desktop-workflow-db-state
> **日期**: 2026-10-06
> **状态**: draft

---

## 问题

desktop 当前以 `openspec/changes/<name>/workflow.json` 为 change 流程状态唯一载体（`workflow_type` / `eval[]` / `active_phase` / `file_log[]`）。该文件是三方共写的共享形状：插件 MCP 工具（CLI 工作流）、插件 hooks（record-files / sweep-phase / protect-files）、desktop Rust 写面（core/workflow 七操作，walker 经 LocalToolSteps 进程内直调）。共享 JSON 文件带来四个结构性痛点：

1. **双实现锁死共享形状**——desktop 已有相位机 Rust 端口（evaluator 明令禁止 agent 调 MCP phase 工具，落账由桌面代写），两套写面靠 JSON 文件形状对齐；`persist.rs` 的 raw Value 保形改写只为不丢插件未知字段。
2. **无事务**——JSON 读-改-写，多窗口 / 插件 hook 并发共写存在竞态面。
3. **schema 与插件版本耦合**——历史痛点两次真实发生：CLI 2.10.39 读不了 file_log 版 workflow.json；desktop 写 2000 字 report 但插件读侧 500 字上限截断读挂。
4. **fs 即数据**——list 全量扫盘逐个 parse（含 archive 树）；inventory 代际（v0/v1/v2）与 unparsable 状态是文件形状的投影而非数据本身。

已确认的关键事实（explore 核实）：

- desktop 门禁不依赖 file_log：walker 给 evaluator 的变更上下文走 DiffContextPort（git diff），file_log 在 desktop 侧只是详情抽屉展示；`persist.rs` 明言「file_log 零触点」。
- workspace db（app-data 下 `workspaces/{name}-{hash}.redb`）已有成熟落点与惯例：SessionRecord / SessionEventRecord / AgentRunRecord / ExploreRecord，native_model 版本治理、i64 unix millis 时间戳、写事务内 max+1 主键、信封 API 零改动可浏览。
- **先例**：ExploreRecord 即「记录在 db（身份/时间）、内容在磁盘笔记」双载体——change 状态搬库与其同构（状态 db、产物磁盘）。

---

## 提案

把 change 流程状态从 workflow.json 整体搬进 workspace 库，desktop 对 workflow.json 转为**不读不写（双向墙）**：

1. **store 新增四模型**（workspace 维度，落所属 workspace 库）：`ChangeRecord`（name 主键 / workflow_type / created_at / status / active_phase）、`PhaseRecord`（相位状态 + eval report ≤2000 内联 + executor/evaluator/decision 三会话槽位）、`ChecklistItemRecord`（相位内产物独立子表，打包键 `(phase_id as u128) << 64 | item_index`）、`StepRecord`（相位机步骤审计行）。
2. **写面重定义到 db**：core/workflow 七操作（phase_next / phase_start / phase_log / backtrack / decision_log / create / archive）落库；phase_log 在单个 redb 写事务内原子完成（PhaseRecord + ChecklistItemRecord + active_phase 更新 + stale 翻转）；写面经 port 缝落库，core/workflow 零 infra 依赖；parse 模块（detect / workflow_file）退役删除。
3. **queries 改 db 读**：状态单源 db；磁盘扫描保留为产物发现；列表 = db 记录 ∪ 磁盘目录（db 缺记录 → 文档形态，同名共存以 db 为准）；时间出线保持 ISO 串 + null（wire contract 冻结，字段演进走 golden 显式重写流程）。
4. **orchestration 接线**：steps 进程内直调面不变（载体在 workflow 写面内换血）；snapshot 改 db 读；发起前置校验从「workflow.json 可解析」改为「db 已建档」（CLI change 无建档不可发起 run）；file_log 相关 AC 退役。
5. **前端退役面**：inventory 代际徽章（V0/V1/V2）、「workflow.json 无法解析」警示、fileLog 展示区（workflow 独立面板 + 文件表节 + file-log-table）退役；bindings 重生成。
6. **归档双写**：写面 archive 操作 = db status 翻转 + active→archive 目录改名（日期前缀）同一提交语义；db 主键 name 不随目录改名变。

收益对号：`persist.rs` raw Value 保形杂技退役（db 自持形状，字段演进走 native_model 版本）；redb 单事务原子落（eval 追加 + stale 翻转 + active_phase 更新一步完成）；与插件 schema 解耦（500 字上限、2.10.39 类版本兼容痛点消失）；PhaseRecord 会话槽位与 SessionRecord 同库 join，详情抽屉 transcript 的 sourceRef 逆查变库内一等查询；步骤历史可查（新增能力）。

**影响面修正**（对 explore 影响面速查的代码核对结论）：

- **desktop-file-watch 零触点**：现状代码中不存在 workflow.json 订阅消费点（watch 通道唯一消费方是 explore 详情页的 `explore.md`，changes 视图明确「无轮询、无文件 watch」）；explore 拍板 5 的「订阅消费点随写面退役」无对象可退役，watch crate 保留不动，本变更不产生该 spec 的 delta。
- **stats 零触点**：`code_stats` 仅遍历源码文件树统计行数，不读 workflow.json 任何维度（仅 mod_test 夹具顺带写过一个 workflow.json 文件）；不入变更范围。

---

## 能力

### 新增能力

- **desktop-change-state-store** — change 流程状态的 workspace 库载体：四模型落库（ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord）、相位机写面单事务原子、归档双写、步骤审计可查不做恢复、双向墙（desktop 对 workflow.json 零读零写）。

### 修改的能力

- **desktop-workspace-store** — workspace 库模型注册组由四模型扩至八模型（新增 change 状态四模型）+ change 域 store 操作面（建档 / 相位落账 / 步骤追加，信封 API 零改动覆盖）。
- **desktop-workflow-write-face** — 写面持久化载体 workflow.json → workspace 库（经 port 缝）；serde/zod 兼容与「schema 权威移交双写并存」命题退役；操作语义（路由 / 白名单 / stale / 槽位 / 回跳重评）保持。
- **desktop-change-parse** — 三代代际探测与 serde 宽松解析随 workflow.json 载体退役（模块删除）。
- **desktop-change-queries** — list / detail 读源改 db + 磁盘产物发现；inventory / fileLog / unparsable 线面字段退役（golden 显式重写）；时间出线口径不变。
- **desktop-change-orchestration** — 节点状态派生源换 db 状态；发起前置校验改建档校验；file_log 边界条款退役为载体退役留痕。
- **desktop-change-create** — create 改为「目录 + explore.md + db 建档」双写，不再产出 workflow.json；新建立即可见可发起。
- **desktop-change-flow-view** — 代际徽章 / unparsable 警示 / fileLog 面板（workflow 独立面板 + 抽屉文件表节）退役；降级语义改为建档状态两态（完整状态面 / 文档形态）。
- **desktop-corpus-regression** — 语料从 workflow.json 目录树夹具改为 db 种子夹具 + 磁盘产物树；golden 快照纪律与显式重写流程延续。
- **desktop-data-dimensions** — 「workflow 过程数据归 user 维度」改判：change 流程状态归 workspace 维度（2026-10-06 拍板，原裁定的首个写入方到场即反转）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` / `store.rs` — ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord 模型与 native_model id 分配、workspace 组注册、change 域操作面（建档 / 相位落账 / checklist 落行 / 步骤追加 / active_phase 更新 / status 翻转）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/` — 七操作持久化载体重定义到 db（落库 port 缝 + 事务编排；`persist.rs` 保形改写退役）；新增 `archive` 操作
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/` — list / detail 读源改 db + 磁盘产物发现；DTO 字段面演进（inventory / fileLog / unparsable 退役）
- `packages/desktop/src-tauri/crates/core/workflow/src/model/` — 磁盘模型（`Workflow` / `Inventory` / eval 条目）退役，查询线面类型保留演进
- `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` / `snapshot.rs` — 写面 / 读面接线核对（进程内直调面不变，fake port 随写面签名演进）
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — 发起前置校验改 db 建档校验
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — list / detail / archive 命令接线（archive 为新增薄命令）
- `packages/desktop/src/views/changes/` — change-detail-view（徽章 / 警示 / WorkflowPanel 退役）、change-list-view（徽章 / unparsable 标注退役）、flow/detail-drawer（文件表节退役）、flow/attachments（file_log 挂载分支退役）
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO 演进重生成

### 测试文件

- `crates/infra/store/src/model_test.rs` / `store_test.rs` — 四模型回环、打包键序、查重、信封覆盖
- `crates/core/workflow/src/write/*_test.rs` — 七操作 db 化重写（单事务原子 / 查重 / 白名单 / stale 传播 / 归档双写）
- `crates/core/workflow/src/queries/*_test.rs` — db 读 + 发现语义 + 文档形态
- `crates/core/orchestration/src/steps_test.rs` 等 — fake 写面随签名演进
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/golden/` + `tests/fixtures/` — db 种子夹具重构 + golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1`）
- `packages/desktop/src/views/changes/*.test.tsx` — 退役面断言改写（徽章 / 警示 / 文件面板消失路径）
- `src/commands/changes/mod_test.rs` / `src/commands/change_flow/mod_test.rs` — 前置校验与命令面夹具改 db 种子

### 删除文件

- `crates/core/workflow/src/parse/`（`detect.rs` / `workflow_file.rs` 及测试）— parse 面整体退役
- `crates/core/workflow/tests/fixtures/` 下 workflow.json 语料与对应 golden 快照 — 处置形态（删除 vs 转标 legacy 目录）由 design 定稿
- `packages/desktop/src/views/changes/flow/file-log-table.tsx`（及其测试）— fileLog 面板退役

### 不要修改

- `plugins/dev-team` 全部（MCP 工具、hooks、CLI 工作流照旧读写其自己的 workflow.json；版本 2.10.44 不动）
- openspec CLI 侧任何代码
- `crates/infra/watch/`（crate 保留，零触点）
- 既有 SessionRecord / SessionEventRecord / AgentRunRecord / ExploreRecord 记录面（零迁移、零改形）
- 存量 CLI change 的磁盘目录与 workflow.json（原样保留，desktop 以文档形态展示其产物）
- `openspec/specs/**`（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | store 四模型与 change 域操作面 | 回环测试：建档 → phase_log（含 checklist 多条）→ 重开 db 读出逐字段一致；打包键同相位内自然序即 evaluator 输出序；(change, phase, attempt) 重复落账返回 StoreError；native_model id 分配不与既有冲突；`list_models` / `scan` 零改动覆盖四新模型 |
| AC-2 | 写面 db 化与 port 缝 | 七操作全部经 workspace 库落库；phase_log 中途构造失败（如 checklist 序列化错误注入）后 db 零残留（无半截 PhaseRecord / ChecklistItem）；审查 `crates/core/workflow/Cargo.toml` 零 infra/store 依赖（落库经 port 缝由壳层装配）；写面保持同步签名 |
| AC-3 | 双向墙与 parse 退役 | `crates/core/workflow/src/parse/` 目录不存在；全 desktop 源码扫描无 workflow.json 读 / 写触点（测试夹具除外）；存量 CLI change（有 workflow.json 无 db 记录）在清单以文档形态可见、详情可读产物 |
| AC-4 | queries db 读与发现语义 | 列表 = db 记录 ∪ 磁盘目录去重并集；同名共存以 db 为准；db 缺记录条目无状态面（空流水线 + 产物清单）；detail 时间出线为 ISO 串 + null（golden 守卫绿）；`change_flow_start` 对无建档 change 显式拒绝 |
| AC-5 | orchestration 全链走通 | 假引擎 + 假写面驱动 run：phase_next / phase_start / phase_log / backtrack 全部落库，节点状态由 db 状态 × provenance 派生；已 pass 相位重启后不重头执行（active_phase 自 db 续走）；StepRecord 按 run 可枚举完整步骤历史 |
| AC-6 | create 建档 | 新建 change 产出目录 + explore.md（goal 原文）+ ChangeRecord，无 workflow.json 产出；成功后立即可见（清单 / 详情）可发起（前置校验通过）；同名 active 冲突拒绝且 db 零建档 |
| AC-7 | 归档双写 | archive 成功后 db status=archived 且目录已改名带日期前缀；ChangeRecord 主键 name 不变；归档条目在列表按月分组可达；无建档目录不可归档（显式拒绝） |
| AC-8 | 前端退役面与全管线 | 清单 / 详情无 V0/V1/V2 徽章与「workflow.json 无法解析」文案；抽屉无文件表节、无 workflow 独立面板；`pnpm -C packages/desktop run client:check` 与 `vp test` 全绿，knip 零新增豁免 |
| AC-9 | golden 显式重写 | detail 线面字段演进（inventory / fileLog / unparsable 删除）经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成，diff 人工确认预期范围并留痕；db 种子语料覆盖多 attempt / backtrack stale / 槽位全缺 / 文档形态 |
| AC-10 | 版本交付 | `packages/desktop/package.json` version 由 0.4.12 升级为 0.4.13（用户可见变更；归档时执行；`src-tauri/Cargo.toml` 版本不随动） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| core/workflow 直依 infra/store 破坏依赖方向红线 | 高（core 层纪律破口） | 中 | 落库 port 缝为硬约束，AC-2 以 Cargo.toml 审查显式验收；port 形态 design 定稿 |
| 双向墙误伤存量用法（CLI 创建的 change 在 desktop 不可发起 run） | 中（用户路径变化） | 中 | 文档形态可见性保底（产物 / 探索照常）；边界在 proposal 与 spec 显式留痕；后续可按需补「导入建档」 |
| 归档双写与外部 openspec CLI 归档（目录改名）竞态 | 中（status 与磁盘不一致） | 中 | 对账策略列入待决问题 design 定稿；列表发现语义以磁盘目录存在性兜底 |
| StepRecord 摘要信息量失当（过小不可审计 / 过大重复落库） | 低 | 低 | 有界截断 + 引用出全量（checks 报告树 / 会话 id 已承载全文），首版审计视图从简 |
| golden 重写范围失控、静默丢投影 | 中（wire contract 冻结被破坏） | 中 | 冻结契约显式重写流程逐字执行：diff 人工确认 + 留痕 + AC-9 验收 |
| detail DTO 字段删除引发前端大面积编译错误 | 低（机械修复） | 高 | tsc 全量拦截 + bindings 前置重导出管线，退役面在视图层一次收口 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| change 状态落哪个库 | workspace 库（推翻 desktop-data-dimensions 原 user 维度裁定） | PhaseRecord 会话槽位 join SessionRecord、级联治理要求同库收敛（store 惯例禁跨库引用）；ExploreRecord「记录 db、内容磁盘」先例同构；原裁定对象（run / phase 记录）从未落地，执行转录本就落 workspace 库 | 全局库（跨库 join 需引库外引用，违 store 惯例） |
| CLI 互操作 | 接受双向墙：desktop 不读不写 workflow.json | 双实现保形对齐是痛点根源；schema 解耦收益最大；CLI 工作流零改动照旧 | 继续双写保形（维持 persist.rs 杂技） |
| file_log 接替 | 不建 FileOpRecord，git diff 承接（DiffContextPort 既有） | desktop 门禁已确认零依赖 file_log；磁盘上 skill 路径 file_log 数据随其 workflow.json 留档不动 | FileOpRecord 落库（无消费方） |
| run 崩溃恢复 | 不做；run 状态仍驻内存 ChangeFlowControl | active_phase 留 db，重启后 phase_next 依 eval 历史重算即可续走；StepRecord 定位审计不承担恢复 | StepRecord 重放恢复（复杂度高、无真实诉求） |
| checklist 存储 | 独立子表 ChecklistItemRecord，不内嵌 PhaseRecord | SessionRecord → SessionEventRecord 父子行 + 打包键先例同构；状态 / 产物分面；evidence 长文本走独立版本链；写时机不变（phase_log 单事务内同落，拆存储不拆原子性） | 内嵌 PhaseRecord JSON 列（版本链耦合） |
| StepRecord 摘要字段面 | 有界摘要（截断上限对齐 report ≤2000 口径，design 定稿具体值）+ 可选引用（checks 报告目录 / 会话 id）；不落全文 | 审计视图需要「发生了什么、结果如何、去哪看全量」；全文在 checks 报告树与转录单表已承载，重复落库徒增体积 | 输出全文落库 / 仅引用无摘要 |
| 发现语义 | 列表 = db 记录 ∪ 磁盘目录；同名共存以 db 为准；db 缺记录 → 文档形态 | 双向墙下产物 markdown 本就是共享载体，磁盘目录存在即 change；与双向墙不冲突 | 仅 db 记录（CLI change 从 desktop 消失，不可接受） |
| 时间口径 | db i64 unix millis（store 惯例）；出线 ISO 串 + null；转换收 queries 层单点 | detail 线面既有形态（golden wire contract 冻结记忆） | 出线毫秒数（破坏冻结契约） |
| unparsable 路径 | 随 parse 退役（警示与标注一并删除） | db 行 native_model 解码失败另有 StoreError 面，警示对象不复存在 | 保留警示（无对应事实） |
| report 载体 | PhaseRecord 内联，≤2000 字（与 desktop 既有 report 写侧一致） | desktop 自有 schema 后与插件 500 字读侧上限彻底解耦 | 外置文件（查询多一跳，无必要） |
| 桌面归档入口 | 写面 archive + IPC 薄命令；前端入口形态 design 定稿 | db status 位需要合法翻转通道；归档语义双写拍板在案 | 仅靠外部 CLI 改名（status 永远 stale） |

### 待决问题

- 外部归档对账：用户经 openspec CLI 归档（目录改名）后，db status 翻转的时机与机制——列表发现时对账投影、读时以磁盘事实归组、或显式重开翻转（design 定稿；AC-7 只约束桌面归档命令路径）。
- 前端归档入口形态：详情页归档按钮 vs 命令先行无 UI 入口（design 定稿）。
- 历史 workflow.json 语料夹具处置：删除 vs 转标 legacy 目录保留（design 定稿，随 parse 退役一并收敛）。
- PhaseRecord 等 native_model id 分配与 SessionAnchors 进程内锚点在 db 化后的存续形态（design 定稿）。
- create 的 db 建档与目录创建双写顺序及失败补偿（非事务载体间的一致性窗口，design 定稿）。

---
