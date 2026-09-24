# 提案: change-detail-react-flow

> **变更**: change-detail-react-flow
> **日期**: 2026-09-24
> **状态**: draft

---

## 问题

change 详情页 `packages/desktop/src/views/changes/ChangeDetailView.tsx` 目前为纯垂直线性布局：Header → 流水线（9 站 Station 逐站堆叠 attempt）→ 中断留档 → file_log 表格 → 产物区（ArtifactView 平铺）。三重不足：

1. **执行史不可读**：多 attempt 重试、backtrack 回跳、interrupted 留档、active 运行中在垂直堆叠中只是先后文本块，推进 / 重试 / 回跳的结构关系不可见（v2-b 夹具实证：backtrack 可同站也可跨站，`interrupted` 与 `active_phase` 的事件不在 eval 序列内）。
2. **素材归属不可见**：file_log 条目后端已带 `scope`（phase id + attempt）与 `attempt` 字段，但前端只画整张表（scope 未用）；eval-checklist 信封 payload 自带 `phase` + `attempt`；产物文档按文件名约定归属某站——三者与所属站 / attempt 的关联在页面上全部丢失。
3. **信息与结构割裂**：产物区是独立文档列表，与流水线、文件改动互不参照；看一个 change「做到哪、改了什么、验过什么」需自行拼装。

探索结论（`explore.md` 三轮已全部定案）：`ChangeDetail` 现有 DTO 字段已足够在前端推导整张流程图，**无需动后端**。file_log 只记源码改动（CLI `record.ts` 显式排除 `openspec/**`），artifacts 是过程文档，两类素材正交、都应进图。

---

## 提案

以 react-flow（`@xyflow/react` v12，兼容 React 19 + Tailwind 4）把详情页重排为 **attempt 级流程图**：

- **布局即语义**：9 站固定 9 列（x = `PIPELINE_PHASES` 列索引 × COL_W），列内按执行序纵向排布（y = 序 × ROW_H）；手工坐标，无 dagre / elk。边的 Δx 符号免费表达类型：向右=前进、同列=重试、向左=回跳（虚线，`backtrack_reason` 作边标签）——backtrack 重做无需显式边分类即自动呈现为向左。
- **节点三分类**：eval 节点（实心，pass 绿 / fail 红，stale 半透明淡化表示废弃支线）、active 节点（pulse 运行中，无 verdict，接流末端）、interrupted 节点（dashed 灰显留档，独立成节点、不与同号 eval 合并——CLI `phase-state.ts` 语义：interrupted 永不写 eval、无 id 关联）。
- **素材挂载层次**：phase 列容器（react-flow v12 subflow：`parentId` + `extent: 'parent'`）列头带 phase 名与该站过程文档徽章。过程文档挂**列**（前端静态映射表：`proposal.md` / `specs/**` → proposal、`design.md` / `tasks.md` → dev-design、`test-reports/**` → test-execution，每站一份不随 attempt 重复）；eval-checklist（payload 自带 phase+attempt）与 file_log（`scope`=phase+attempt）挂**节点**；`scope='workflow'` 的 file_log 不入图、独立面板展示（存量数据几乎全是它）；`explore.md` 不入图（探索在流程之外）。
- **单一交互入口**：列头与节点点击均打开右侧抽屉，按【本站文档 | eval report+checklist | 文件表】分节，复用既有 `renderers/registry`（新增产物 kind 仍只加 registry 一行）。
- **纯函数转换层**：`detail → { nodes, edges }` 为 `views/changes/` 下独立纯函数模块，vitest 直测（前进 / 重试 / 回跳 / stale / interrupted / attempt 缺号 / 时间戳缺失兜底）；react-flow 仅作渲染薄层。
- **边界保留**：不动后端 DTO 与取数契约（显式 refresh，无 watch / 轮询——live 更新留作后续独立 change）；v0 空图占位 + 沿用现有产物区文档列表，v1 图正常画但节点无文件区。
- **交付收尾**：`packages/desktop/package.json` version `0.3.2` → `0.3.3`（用户追加交付要求）；`tauri.conf.json` 引用 `../package.json` 自动跟随无需改动，`src-tauri/Cargo.toml`（`0.1.0`）历来不随 desktop 版本走（0.3.0 / 0.3.1 两次 bump 先例），不随动。

---

## 能力

### 新增能力

- **desktop-change-flow-view** — change 详情页的 react-flow 流程图视图契约：attempt 级节点三分类与视觉、9 列手工坐标布局（布局即语义）、时间序边推导（前进 / 重试 / 回跳）、过程素材（文档 / eval-checklist / file_log）挂载规则、右侧抽屉单一交互入口、三代际降级（v0 / v1 / v2）与纯函数转换层可测试性。

### 修改的能力

（无 —— 路由契约（desktop-page-routing）、后端查询聚合（desktop-change-queries）、产物发现（desktop-artifact-plugins）均不变；本变更只替换详情页的呈现层。）

---

## 变更范围

### 实现文件

- `packages/desktop/package.json` — 新增依赖 `@xyflow/react`（v12）；version `0.3.2` → `0.3.3`（用户追加交付要求）
- `packages/desktop/src/views/changes/flow/`（新增目录，命名由 design 定夺）— `detail → { nodes, edges }` 纯函数转换、布局常量（COL_W / ROW_H）、节点 / 边类型、文档→列归属静态映射表
- 流程图渲染组件（新增）— ReactFlow 薄层 + phase 列容器（subflow `parentId` / `extent: 'parent'`）+ eval / active / interrupted 自定义节点 + 列头组件
- 右侧抽屉组件（新增）— 【本站文档 | eval report+checklist | 文件表】三分节，复用 `renderers/registry` 的 `ArtifactView`
- `packages/desktop/src/views/changes/ChangeDetailView.tsx` — 线性布局改为 Header + 流程图区 + workflow 独立面板（`scope='workflow'` 的 file_log 表格）+ 产物区；`DetailSection*` 的展示语义迁入抽屉与图节点；`unparsable` 警示与 error / loading / 未找到降级页保留

### 测试文件

- 转换层纯函数直测（新增 `flow/*.test.ts`）— 前进 / 重试 / 回跳 / stale 淡化 / interrupted 插入 / active 接流末端 / attempt 缺号兜底 / 时间戳缺失兜底 / 文档归属映射 / workflow scope 剔除
- 视图集成测试（新增 `*.test.tsx`）— 抽屉三分节内容、三代际降级、空图占位；data-testid 挂钩
- `packages/desktop/src/views/changes/ChangeDetailView.test.tsx` — 按新布局更新既有断言（逐条迁移而非删除）

### 删除文件

（无 —— 旧线性布局在 `ChangeDetailView.tsx` 内就地改造，无独立文件删除。）

### 不要修改

- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `ChangeDetail` DTO 与 `PIPELINE_PHASES` 不动（现有字段足够）
- `packages/desktop/src/views/changes/hooks/useChangeDetail.ts` 的取数语义 — 显式 refresh、无 watch / 无轮询（live 更新留作后续独立 change；如无必要不改 hook 本体）
- `packages/desktop/src/views/changes/renderers/` — registry 机制与既有 renderer 不动（抽屉只消费）
- `plugins/dev-team/bin/src/modules/workflow/files/record.ts` — file_log 写入侧 scope 语义源头不动
- `packages/desktop/src-tauri/tauri.conf.json` — version 引用 `../package.json` 自动跟随，不改动
- `packages/desktop/src-tauri/Cargo.toml` — `[package].version = "0.1.0"` 历来不随 desktop 版本 bump（0.3.0 / 0.3.1 两次升级均只改 package.json），不随动

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 转换层纯函数 `detail → { nodes, edges }` | x = `PIPELINE_PHASES` 列索引 × COL_W，y = 列内执行序 × ROW_H；列数恒为 9，未走的站呈现空列 |
| AC-2 | 节点三分类 | eval 节点实心（pass 绿 / fail 红、stale 半透明）、active 节点 pulse 运行中、interrupted 节点 dashed 灰显且独立成节点不与同号 eval 合并（v2-b 夹具形态可证） |
| AC-3 | 边推导与「布局即语义」 | 每节点入边来自时间上紧邻的前一事件（eval 追加序为主序，interrupted / active 按 start_at 归并插入）；向右=前进、同列=重试、向左=回跳（虚线 + `backtrack_reason` 边标签） |
| AC-4 | 素材挂载 | 过程文档按静态映射表挂列；eval-checklist 按 payload phase+attempt 挂节点；file_log `scope=phase+attempt` 精确挂 attempt 节点；`scope='workflow'` 与 `explore.md` 不入图 |
| AC-5 | 抽屉单一交互入口 | 列头与节点点击打开同一右侧抽屉，分节【本站文档 / eval report+checklist / 文件表】，文档经 renderers/registry 渲染 |
| AC-6 | 三代际降级 | v0：空图占位 + 沿用产物区文档列表（图区不动产物区）；v1：图正常画、节点无文件区（fileLog=null）；v2：完整图 |
| AC-7 | 取数模型不变 | 详情更新仍仅由显式 refresh 触发，无 watch / 轮询 / 事件订阅 |
| AC-8 | 转换层测试覆盖 | 纯函数直测覆盖前进 / 重试 / 回跳 / stale / interrupted / active / attempt 缺号 / 时间戳缺失 / 归属映射边界；react-flow 渲染语义不做逐项断言 |
| AC-9 | 管线合规 | `pnpm -C packages/desktop run client:check`（fmt / lint 含 `max-lines-per-function: 50` / knip）与 `pnpm -C packages/desktop run test` 全绿；无新增豁免条目；测试挂钩均为 data-testid |
| AC-10 | `packages/desktop/package.json` 版本升级 | `version` 字段为 `0.3.3`；`tauri.conf.json` 经 `../package.json` 引用自动跟随（该文件无改动）；`src-tauri/Cargo.toml` 不随动（保持 `0.1.0`） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `@xyflow/react` 与 React 19 + Tailwind 4 集成不兼容（样式 / 类型） | 流程图渲染异常或构建失败 | 低 | 探索已确认 v12.11.x 为兼容组合；锁定该版本线，集成失败仅回退视图层、数据层不受影响 |
| 9 列 ≈ 2000px 宽在最小窗口（900px）下不可读 | 节点文字过小、交互困难 | 中 | fitView + 缩放适配；COL_W / ROW_H 与 fitView 策略由 dev-design 定夺并以夹具调验 |
| 时间序归并在时间戳缺失 / 乱序数据下边推导失真 | 边方向错误（前进画成回跳） | 中 | eval 追加序为主序、interrupted / active 按 start_at 归并，attempt 缺号 `unwrap_or(0)` 兜底（与后端 detail.rs 同款）；纯函数直测 + v2-a / v2-b 夹具回归 |
| 旧线性布局信息在改造中丢失（report 文本、中断留档时间等） | 详情信息回归缺口 | 中 | 抽屉三分节保留全部信息；`ChangeDetailView.test.tsx` 既有断言逐条迁移而非删除 |
| 单 change 事件量大导致节点数量增长 | 渲染性能与可读性下降 | 低 | 9 列恒定上限 + 仅渲染真实事件；截断 / 折叠留作后续优化（决策 #9） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 节点粒度 | attempt 级（phase 列对齐布局） | 还原执行史：失败分叉、backtrack 显式成边，素材可精确挂到 attempt | station 级节点 + 站内展开 |
| 布局引擎 | 手工坐标（x=列索引、y=列内执行序），9 列固定 | 列数恒定 → 转换层逻辑最简；Δx 符号免费表达边类型 | dagre / elk 自动布局；按实际进度截断列 |
| 归属规则位置 | 前端静态映射表，不动后端 DTO | 现有字段（file_log scope、checklist 信封 phase+attempt、固定 PIPELINE_PHASES）已足够 | `ArtifactDescriptor` 加 phase 字段 |
| `scope='workflow'` 的 file_log | 不入图，独立面板展示 | 无归属；存量数据几乎全是它（夹具实证） | 挂图外层 / 省略 |
| 展开交互 | 右侧抽屉（列头与节点同一入口） | 复用 renderers/registry，单一交互入口 | 节点内展开 |
| 旧代际降级 | v0 空图+产物区文档列表；v1 图正常画、无文件区 | 无 workflow.json 无图可画；v1 仅缺 file_log | 整页降级回线性布局 |
| 可测试性 | 纯函数转换层 + 渲染薄层 | vitest 直测边界；符合「不逐项验证库语义」约定 | 仅组件级测试 |
| live 更新 | 本次不做，维持显式 refresh | FileWatchEvent 基建已就绪但属独立语义，避免本变更范围膨胀 | 接入 watch 实时刷图 |
| `explore.md` 归属 | 不入图 | 探索在流程之外（无 change 时独立存在于 `openspec/explores/`），语义上不属于任何站 | 挂 proposal 列 |
| desktop 版本升级时机 | 本变更内升 `0.3.3`，仅改 `packages/desktop/package.json` | 用户追加交付要求；`tauri.conf.json` 引用 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 历来不随动（0.3.0 / 0.3.1 先例）→ 升级 = 改 1 个文件 | 单独 version-bump change；顺手改 Cargo.toml（否决：破坏既有惯例） |

### 待决问题

（无 —— 2026-09-24 探索第三轮全部定案。留给 dev-design 的实现细节：COL_W / ROW_H 常量取值与 fitView 策略、flow 模块目录与组件命名、react-flow 样式引入位置。）

---
