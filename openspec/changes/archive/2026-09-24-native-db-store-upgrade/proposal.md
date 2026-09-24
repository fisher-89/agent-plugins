# 提案: native-db-store-upgrade

> **变更**: native-db-store-upgrade
> **日期**: 2026-09-23
> **状态**: proposed

---

## 问题

三个诉求汇入同一变更：

1. **依赖升级**：`crates/infra/store` 现为 redb 直接驱动（约 354 行手写表定义 / 事务 / JSON 编解码 / `schema_version` 手工轮账）。为承接未来数据（workflow 过程数据，含每次 agent 执行明细：phase 执行、评估结论背后的完整事件流、成本、时长——repo 的 workflow.json 只存 eval 结论与 file_log，执行明细 repo 里没有），手工表定义 + `schema_version` 轮账的扩展方式不可持续。
2. **DB 查看页**：数据库内容目前无查看入口，排查只能靠外部工具。需要新增查看 db 的页面，与 Agent 调试一并收入侧栏「系统工具」组。
3. **store 架构规划**：store 需适应未来扩展更多数据结构，模型注册、二级索引、迁移治理要有机制承载。

现状痛点具体化：

- 事件流以 `serde_json::Value` 不透明进出（`user_agent_run_events` 表），而 `core/agent` 的 `AgentEvent` 已是类型化五变体模型且文档注释明说"serde camelCase 线格式同时是落库形态"——类型已存在，store 存 opaque JSON 只是在回避依赖方向问题；
- 查询形态只有"全表读 + 内存排序"（如 `list_agent_runs`），撑不起「change X 完整执行史」这类按索引扫描的查询；
- `user_meta` 表的 `schema_version` 手工轮账在 redb 无内建迁移的前提下是被迫发明，引擎升级后应整体退役。

---

## 提案

store 从 redb 直接驱动升级为 **native_db**（0.8.x）。关键事实：native_db 构建在 redb 之上，事务 / ACID / 文件格式仍是 redb——升级实质是**加模型层**，不是换引擎。买到的能力：模型宏注册（新数据 = 定义 struct + 注册）、二级索引、内建迁移（`schema_version` 手工轮账退役）、实时订阅（未来 UI 刷新通道，本变更不接）。

```
现在:  dev-team ─▶ store ─▶ redb        (手写表定义/事务/编解码/schema_version)
升级后: dev-team ─▶ store ─▶ native_db ─▶ redb
                             └─ native_model(类型版本/迁移治理)
```

四个动作：

1. **模型层落地**：`WorkspaceRecord`（PK=root）/ `AgentRunRecord`（PK=id）现状平移；`AgentEventRecord` 类型化新建——包装 struct 打 native_db derive，嵌装 core `agent::AgentEvent` 纯类型（core 保持 derive-free，版本治理留 infra 侧），旧数据解不出的行以 `AgentEventKind::Raw` 兜底（零丢失有构造性保证）。复合主键 `(run_id, seq)` 是否可直接做 PK 进 dev-design 前必须 spike（不行退合成键）。
2. **legacy 一次性迁移**：启动时探测旧 redb 手写表格式 → read_only 打开逐表读出 → 事务内写入 native_db 新文件 → 旧文件改名 `.bak` 留档。底层同为 redb，旧读新写不同文件无锁冲突。store 短期同时保留 redb 直依赖（读旧）与 native_db（写新），迁移稳定一个版本后由后续变更收掉 redb。
3. **记录信封 API**：store 暴露 `list_models() -> [{name, count}]` 与 `scan(model, offset, limit) -> Vec<RecordEnvelope>`，native_db 类型不越 crate 公共面。新模型注册即自动可被查看器浏览，零额外代码——依赖升级与查看页最漂亮的咬合点。
4. **DB 查看页 + 系统工具组**：侧栏重组——「页面」组保留 [变更]；新增「系统工具」组收入 [Agent 调试]（平移）与 [DB 查看]（新增）。查看器第一版**只读**：模型清单 + 计数、分页扫描、单条 JSON 查看。欢迎态维持完全独立页面，DB 查看页仅壳态可达。

**范围裁定**：workflow 过程数据模型（`WorkflowRunRecord` / `PhaseExecutionRecord`）为升级后的**首个未来租户**，本变更不注册——尚无生产者（桌面端无 workflow 编排），无生产者的模型是投机特性；native_db 注册成本低，随首个写入方变更落地即可。`AgentRunRecord` 联动字段（`source` / `workflow_run_id` / `phase`）同理随该租户引入（决策：一张表加可选字段，不单独成表）。本变更交付的是让该租户"注册即用"的地基。

---

## 能力

### 新增能力

- **desktop-db-inspector** — DB 查看页：侧栏「系统工具」组新入口，仅壳态可达；只读查看面（模型清单 + 计数、分页扫描、单条记录 JSON 查看）；数据面仅经信封 API（`db_models` / `db_records` 两命令），查看器零模型特定代码，新模型注册即自动可浏览。

### 修改的能力

- **desktop-workspace-store** — 引擎升级 redb 直驱 → native_db 模型层；`AgentEventRecord` 类型化嵌装；legacy redb 库一次性迁移（Raw 兜底 + `.bak` 留档）；记录信封 API；`schema_version` 手工轮账退役（native_model 版本治理接管）；「redb 类型不泄漏」平移为「native_db 类型不泄漏」。
- **desktop-crate-layout** — 依赖规则修订：「store MUST NOT 依赖 core 任何 crate（自含模型）」→「store MUST NOT 依赖 core 行为，SHALL 仅依赖 `agent` 纯类型作嵌装载荷」（infra → core 依赖有先例：`agent-cli` 依赖 `agent`；store 是唯一特例，其成立前提——模型平凡——已被过程数据打破）。
- **desktop-app-shell** — 侧栏重组：新增「系统工具」组（[Agent 调试] 平移 + [DB 查看] 新增），「页面」组收敛为 [变更]；`TopPage` 增 `db` 变体；command 组织增 `commands/db/` 只读查询轨道。
- **desktop-data-dimensions** — 维度语义载体适配：redb 表名 user 前缀 → native_db 模型归属 user db（app data dir）；新增 requirement：workflow 过程数据归 user 维度（不可重建活动历史，记录携带 `workspace_root` + `change_name`，查询靠二级索引不靠分库，三笔账全部不触发）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/`：引擎换 native_db + native_model；`AgentEventRecord` 包装模型；legacy 迁移模块；信封 API；`Cargo.toml`（新增 native_db / native_model / `agent` 依赖，redb 迁移期保留）
- `packages/desktop/src-tauri/Cargo.toml`：workspace 依赖登记（native_db / native_model pin 版本、redb 版本收敛）
- `packages/desktop/src-tauri/src/commands/db/`（新轨道：`db_models` / `db_records` 只读命令）+ `src/commands/mod.rs` + `src/main.rs`（注册）
- `packages/desktop/src/components/AppSidebar.tsx`：「系统工具」组 + `TopPage` 增 `db` 变体
- `packages/desktop/src/App.tsx`：顶层视图切换支持 db 页
- `packages/desktop/src/views/db/DbInspectorView.tsx`（新）：只读查看页
- `packages/desktop/src/hooks/useDbInspector.ts`（新）：信封 API 取数
- `packages/desktop/src/types/dto.ts`：`RecordEnvelope` 等信封 DTO
- `packages/desktop/package.json`：desktop 版本 bump

### 测试文件

- `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` / `model_test.rs`：引擎升级后存量场景全绿
- `packages/desktop/src-tauri/crates/infra/store/src/`（新增迁移测试模块）：旧格式 fixture 库迁移数据完整、损坏行 Raw 兜底、`.bak` 留档
- `packages/desktop/src-tauri/src/commands/db/mod_test.rs`（新）
- `packages/desktop/src/components/AppSidebar.test.tsx`：系统工具组渲染与切换
- `packages/desktop/src/views/db/DbInspectorView.test.tsx`（新）：清单/分页/单条查看/只读边界

### 删除文件

- 无整文件删除。store 内 redb 手写表定义与 `schema_version` 轮账代码随引擎升级移除（属文件内删除）；redb 直依赖在迁移稳定一个版本后由后续变更收掉，本变更保留。

### 不要修改

- `packages/desktop/src-tauri/crates/core/agent/**` — `AgentEvent` 等类型保持 derive-free 原样嵌装，MUST NOT 加 native_db/native_model derive 或任何为落库服务的改动
- `packages/desktop/src-tauri/crates/core/workflow/**` — `PhaseLog` / `ChecklistItem` 等 workflow 领域形状属未来租户引用对象，本变更不动
- `packages/desktop/src-tauri/crates/core/foundation/**`、`packages/desktop/src-tauri/crates/infra/agent/**`
- workflow CLI 侧（`plugins/dev-team` 工作流引擎）— 过程数据写入方属未来变更，本变更不接生产者

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | store 引擎升级 native_db + 模型层 | src-tauri workspace `cargo test` 全绿；workspace 注册表与 agent run 落库/重放的存量 spec 场景行为不变 |
| AC-2 | legacy 一次性迁移 | 旧 redb 格式 fixture 库首启自动迁移：各表记录数一致、解不出的历史事件行落 `Raw`、旧文件改名 `.bak` 存在 |
| AC-3 | `schema_version` 退役 | store 源码无 META 轮账代码；模型 shape 演进治理经 native_model 版本机制 |
| AC-4 | `AgentEventRecord` 嵌装建模 | `crates/core/agent` git diff 为空（零 derive 改动）；store 公共 API / desktop 命令签名 / 前端 DTO 无任何 native_db 类型 |
| AC-5 | 记录信封 API | `list_models` / `scan` 落地并经 `db_models` / `db_records` 命令可达；分页扫描返回 `RecordEnvelope{key, value}`（JSON Value） |
| AC-6 | 系统工具组 + DB 查看页 | 壳态侧栏渲染「系统工具」组（[Agent 调试] [DB 查看]），「页面」组仅 [变更]；欢迎态无该组 DOM；点击切换本地 state 无路由 |
| AC-7 | 查看器只读四件套 | 模型清单 + 计数、分页扫描、单条 JSON 查看可用；查看器与 db 轨道命令无任何写操作入口 |
| AC-8 | 依赖规则修订落档 | store `Cargo.toml` workspace 内依赖仅新增 `agent`；desktop-crate-layout spec 修订可考 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Spike①：复合主键 `(run_id, seq)` 不被 native_db 直接支持 | 事件模型键设计返工 | 中 | dev-design 首项验证；退路为合成键（u128 打包或字符串），二级索引 `run_id` 保查询形态 |
| Spike②：native_db 自带 redb 与仓库 pin 的 `=4.3.0` 无法收敛 | 依赖树出现双 redb 版本 | 中 | 双版本 cargo 可共存且迁移读旧 / 写新为不同文件无锁冲突；收敛失败则评估 redb 退为纯传递依赖，一并看 native_db pre-1.0 的 API 稳定性心智账 |
| Spike③：native_model 不支持按模型选 serde_json 后端 | 查看器需自解 bincode，人可读目标打折 | 中 | bincode 解码收在信封 API 内部，查看页面无感；选型以「人可读」加权 |
| native_db 0.8.x pre-1.0 API 变动 | 后续升级成本 | 中 | workspace 精确 pin 版本；升级按 lockstep 处理 |
| 迁移丢数据 | agent 运行历史丢失（不可重建、非派生缓存，desktop-data-dimensions 裁定） | 低 | `Raw` 变体兜底给出零丢失的构造性保证 + 旧文件 `.bak` 留档 + 迁移集成测试覆盖损坏行 |
| 迁移窗口双开竞态 | 另一进程迁移期间写旧库 | 低 | 既有单进程约束显式声明强化；迁移仅启动时执行一次，成功即置新格式标记 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 事件流建模走类型化还是 raw KV 保留通道 | 类型化建模 | `core/agent` 的 `AgentEvent` 已是类型化模型且自带 `Raw` 逃生舱（永不丢事件、永不炸解析），旧数据零丢失有构造性保证；opaque JSON 只是在回避依赖方向问题 | raw KV 保留通道（放弃类型安全与二级索引红利） |
| store 依赖 core 类型的所有权 | 包装建模：native_db derive 打在 store 的包装 struct 上，core 类型保持 derive-free 作嵌装载荷 | core 保持零 derive 依赖；native_model 版本治理留在 infra 侧 | 镜像类型（双份维护、形状漂移）；抽 contract crate（等第二个消费方出现再谈，现在做是过度设计） |
| workflow 过程数据维度 | user 维度；记录携带 `workspace_root` + `change_name`，查询靠二级索引 | 不可重建的活动历史（workspace 维度硬约束"SHALL 作为派生数据可重建"不满足）；不进 repo（转录进 git = 噪音 + 体积 + 隐私）；三笔账（双开锁/gitignore 分型/克隆重建）全部不触发 | workspace 维度分库（触发三笔账，且查询按 workspace 切库无必要） |
| workflow 过程数据模型本变更是否落地 | 否——为升级后首个未来租户，随首个写入方变更注册 | 桌面端尚无 workflow 编排生产者，无生产者的模型是投机特性；native_db 注册成本低，"新数据 = 定义 struct + 注册"正是本次买到的能力 | 本变更一并注册（空表无生产者） |
| `AgentRunRecord` 与 workflow 的联动设计 | 一张表加可选联动字段（`source`: manual\|workflow / `workflow_run_id` / `phase`），字段随 workflow 租户变更引入 | 事件流、重放、查看器全部复用；手动调试 run 成为 workflow 执行史的特例（`source=manual`），调试页是执行史的免费子集 | workflow 驱动 run 单独成表（重复事件流/重放/查看器三套） |
| 变更拆分 | 一个 change 打包（依赖升级 + 迁移 + 信封 API + 查看页 + 系统工具组） | 查看页吃升级红利（信封 API + 模型注册），分两个 change 会导致信封 API 做两遍 | 拆「依赖升级」「查看页」两个 change |
| 查看器是否提供写操作 | 第一版只读 | 无真实 debug 需求支撑写路径；只读边界缩小风险面 | 提供增删改（ speculative，无需求方） |
| `schema_version` 手工轮账去留 | 退役，`user_meta` 随迁移删除；模型 shape 演进交 native_model 版本治理 | native_db 内建迁移取代其全部职责 | 保留轮账（双轨冗余） |
| 欢迎态是否露系统工具组 | 不露——欢迎屏维持完全独立页面，DB 查看页仅壳态可达 | 欢迎态无 db 上下文可看；壳语义（workspace 已选）才是系统工具的前提 | 欢迎态渲染壳但隐藏入口（壳挂载逻辑复杂化） |

### 待决问题

- Spike ①②③ 结果（进 dev-design 前必须收口，见风险表对应缓解措施）
- 信封 API 命令命名：`db_models` / `db_records` 为提案命名，design 可调
- native_db / native_model 的 workspace pin 版本值（随 Spike② 定）
- 迁移稳定一个版本后收掉 redb 直依赖的时机（后续 change 承担）

---
