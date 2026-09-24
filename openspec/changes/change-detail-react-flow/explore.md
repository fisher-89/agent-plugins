# change 详情页 react-flow 流程图

> 主题：`ChangeDetailView` 从线性垂直布局改为 react-flow 流程图展示；过程文件（产物文档、file_log 源码改动、eval checklist）关联到对应 phase 节点内展开显示。
> 来源：`openspec/todo.md` L90。探索：2026-09-24（三轮；第三轮按倾向定案，问题已全部决策）。

## 现状快照

### 前端现状

- 详情页 `packages/desktop/src/views/changes/ChangeDetailView.tsx`：纯垂直线性布局——
  Header → 流水线（Station 列表，每站 Attempt 逐条堆叠）→ 中断留档 → file_log 表格 → 产物区（ArtifactView 列表）。
- 取数 `hooks/useChangeDetail.ts`：`invoke("get_change_detail")` + 逐个 `invoke("read_artifact")`；显式 refresh、无 watch。
- 产物渲染 `renderers/registry.ts`：按 kind 路由（markdown-doc / eval-checklist / tasks-progress / Fallback），新增 kind = 加一行。
- 依赖：**尚无 react-flow**（`@xyflow/react` v12.11.x 支持 React 19 + Tailwind 4，与 desktop 技术栈完全匹配）。

### 数据模型（ChangeDetail DTO）

```
ChangeDetail
├── pipeline: PhaseEntry[9]        # 固定 9 站，顺序不依赖 eval 排列
│   └── attempts: AttemptRecord[]  # verdict/report/checklist/skipped/stale/
│                                  #   startAt/timestamp/backtrackTo/backtrackReason
├── activePhase                    # 运行中站（phase+attempt+startAt）
├── interrupted[]                  # 中断留档
├── fileLog: FileLogEntry[] | null # v1 及更早为 null
│   └── entry: { op, scope, attempt, path, at }
└── artifacts: ArtifactDescriptor[]  # { kind, source, title }
```

9 站固定序列（`queries/detail.rs` `PIPELINE_PHASES`）：
`proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze`

### 素材归属线索（核心发现）

| 素材 | 归属依据 | 状态 |
|---|---|---|
| file_log 条目 | `scope` = phase id（带 attempt）或保留字 `'workflow'`（无归属） | **后端已给**，前端未用（现在只画整张表） |
| eval-checklist 信封 | payload 自带 `phase` + `attempt` | 天然可挂节点 |
| markdown-doc 信封 | source = 相对路径：`proposal.md`→proposal 站；`design.md`/`tasks.md`→dev-design；`specs/**`→proposal；`test-reports/**`→test-execution；`explore.md`→不入图（已决策）；`phases/*.md`→旧代际 | **只有文件名约定，无 phase 字段** —— 归属靠前端静态映射表 |
| tasks-progress 信封 | source=tasks.md | 同上，按约定挂 dev-design |

关键点：file_log 只记**源码改动**（CLI `record.ts` 显式排除 `openspec/**` 与 workflow.json 本身），artifacts 是**过程文档** —— 两类素材正交，都进图。

### 真实样例形态（workflow crate fixtures v2-a / v2-b）

- 多 attempt 常态：`implement attempt1 fail → attempt2 pass`。
- backtrack 真实存在：`dev-design attempt2 pass (BACKTRACK->dev-design 缺产物区组件回跳补齐)` —— 回跳可同站也可跨站，是图的非线性边来源。
- fixture 中 file_log 全部为 `scope='workflow'`；phase scope 语法 CLI 已支持但存量数据少见。

## 图模型决策（2026-09-24）

### 决策历程

第一轮摆出两个候选：

- **方案 A：station 级节点 + 站内展开**（保守）——节点徽章=素材计数，点击节点展开 attempts+文档+文件。
- **方案 B：attempt 级节点**（还原执行史）——失败 attempt 分叉、backtrack 边显式画出。

**第二轮定案：方案 B 系（attempt 级），但布局不自动排布——
x 坐标 = 所属 phase 的列索引，同 phase 的全部事件纵向排成一列。**

### 节点三分类（探索发现：执行史 ≠ eval 序列）

`workflow.json` 里有三类"执行事件"，eval 序列只是其一（v2-b 夹具实证：
`interrupted` 的 test-gen#1 与 `active_phase` 的 implement#2 都不在 eval 里）：

| 节点类 | 数据源 | 视觉 |
|---|---|---|
| eval 节点 | `eval[]`（verdict/checklist/report/skipped/stale） | 实心，pass 绿 / fail 红 |
| active 节点 | `active_phase` | pulse 运行中，无 verdict，接流末端 |
| interrupted 节点 | `interrupted[]` | dashed 边框灰显，start~end 留档 |

`interrupted` 语义（CLI `phase-state.ts`）：`active_phase` 的归档追加，
**永不写 eval、不烧重试配额**；与后续同号 eval 无 id 关联 → 图上独立成节点，
不与 eval 节点合并。

`stale` 语义（CLI `backtrack` 命令）：回跳时把**目标 phase 的最新 pass 标记
stale: true**（作废重做）→ 图上该节点淡化（半透明），表示废弃支线。

### 布局公式与"布局即语义"

```
x = PIPELINE_PHASES.indexOf(phase) × COL_W     （9 列，phase 序固定）
y = 列内执行序 × ROW_H

        col:proposal  col:dev-design      col:test-gen   col:test-execution
 y0:    proposal#1──▶dev-design#1 ✗──▶ ──▶test-gen#1 ✓(stale)──▶test-execution#1 ✗
 y1:                  dev-design#2 ✓                                          │
 y2:                                          test-gen#2 ✓◀────────回跳(虚线)──┘
                                              （重做，Δx<0 自动呈现为向左）

边的 Δx 符号免费表达类型：向右=前进 · 同列=重试 · 向左=回跳
无需显式边分类树；backtrack_reason 作回跳边标签。
```

边推导：O(n) 扫描 eval 追加序（即时间序），interrupted/active 按 start_at
归并插入；每条事件的入边 ← 时间上紧邻的前一事件。attempt 缺号按 unwrap_or(0)
（后端 detail.rs 已同款兜底）。

### 素材挂载层次（attempt 级的独有优势）

```
phase 列容器（react-flow v12 subflow：parentId + extent: 'parent'）
├── 列头：phase 名 + 该站过程文档徽章（proposal.md / design.md / specs/** …）
└── attempt 节点
    ├── file_log：scope=phase+attempt → 精确挂到具体 attempt 节点 ✨
    ├── eval-checklist：payload 自带 phase+attempt → 挂对应节点
    └── 节点点击 → 抽屉：【本站文档 | eval report+checklist | 文件表】分节（复用 renderers）
```

- 文档挂**列**（每站一份，不随 attempt 重复）；file_log/checklist 挂**节点**。
- `scope='workflow'` 的 file_log 不入图（独立面板，存量数据全是它——fixtures 实证）。

### 架构结论：无需动后端 DTO

`ChangeDetail` 现有字段已足够推导整张图（file_log 有 scope+attempt、
checklist 信封带 phase+attempt、phase 列索引来自固定 PIPELINE_PHASES）。
文档→列的归属是前端一张静态映射表（proposal.md→proposal 列、
specs/**→proposal、design.md+tasks.md→dev-design、test-reports/**→test-execution）。
第一轮"后端 ArtifactDescriptor 加 phase 字段"的方案在此粒度下**不再必要**。

### 可测试性

`detail → { nodes, edges }` 设计为纯函数（前端 `views/changes/` 下独立模块），
vitest 直测：前进/重试/回跳/stale 淡化/interrupted 插入/attempt 缺号/时间戳
缺失兜底。react-flow 只当渲染薄层（符合"不逐项验证库语义"约定）。

## 已决策一览

| # | 问题 | 结论 |
|---|---|---|
| 1 | 节点粒度：station / attempt / 混合 | **attempt 级**，phase 列对齐布局（x=列索引，y=列内执行序） |
| 2 | 归属规则位置：前端映射表 vs 后端 DTO 加 phase 字段 | **前端静态映射表**；不动后端 DTO（现有字段足够） |
| 3 | `scope='workflow'` 的 file_log 挂哪 | **不入图**，独立面板展示 |
| 4 | 展开交互：节点内展开 vs 侧栏抽屉 | **右侧抽屉**，复用 renderers/registry；抽屉按【本站文档 / eval / 文件表】分节，列头与节点点击进同一抽屉（单一交互入口） |
| 5 | 布局引擎与方向 | **手工坐标**（无需 dagre/elk）；横向 9 列、列内纵向 |
| 6 | 旧代际降级 | **v0**（无 workflow.json）：空图占位 + 沿用现有产物区文档列表，图区不动产物区；**v1**（无 file_log）：图正常画，节点无文件区 |
| 7 | 可测试性 | **纯函数转换层** + 渲染薄层，vitest 直测 |
| 8 | live 更新 | **本次不做**，维持显式 refresh；FileWatchEvent 基建已就绪，watch 留作后续独立 change |
| 9 | 列范围：9 列固定 vs 截断（第二轮提出） | **9 列固定**，未走的站空列；列数恒定 → 转换层逻辑最简，截断留作后续优化 |
| 10 | explore.md 归属（第二轮提出） | **不入图**：探索在流程之外，无 change 时独立存在于 `openspec/explores/`，语义上不属于任何站 |

## 待决策问题

（无 —— 2026-09-24 第三轮按倾向全部定案，结论见上表 #4 / #6 / #8 / #9 / #10。）

### 留给 dev-design 的实现细节

- COL_W / ROW_H 常量与 fitView 策略（9 列 ≈ 2000px 宽，靠缩放适配）。

## 第四轮追加（2026-09-24，workflow 启动时补入）

用户在启动 requirement 工作流时追加交付要求：

- **版本号升级到 0.3.3**：desktop 当前 `packages/desktop/package.json` version=`0.3.2` → `0.3.3`。
  核查：`tauri.conf.json` 的 version 引用 `../package.json`（自动跟随无需改）；
  `src-tauri/Cargo.toml` 的 `[package].version=0.1.0` 历来不随 desktop 版本走
  （0.3.0/0.3.1 两次 bump 均只改 package.json）。
  → 版本升级 = 改 1 个文件，应并入 proposal 实现文件清单与验收。
- 提交并推送属会话级收尾动作（工作流全部通过后执行），不进入 change 文档。

## 相关文件

- 前端：`packages/desktop/src/views/changes/ChangeDetailView.tsx`、`hooks/useChangeDetail.ts`、`renderers/*`
- 后端：`src-tauri/crates/core/workflow/src/queries/detail.rs`（PIPELINE_PHASES、ChangeDetail）
- 产物注册表：`src-tauri/crates/core/workflow/src/artifacts/registry.rs`（discover_artifacts 插件机制）
- file_log 写入侧：`plugins/dev-team/bin/src/modules/workflow/files/record.ts`（scope 语义源头）
- 夹具：`src-tauri/crates/core/workflow/tests/fixtures/v2-{a,b}/workflow.json`
