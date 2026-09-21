# 提案: add-desktop-terminal

> **变更**: add-desktop-terminal
> **日期**: 2026-09-20
> **状态**: draft

---

## 问题

仓库的变更过程数据（`openspec/changes/` 下的 workflow.json、eval 历史、中间产物 markdown）目前只能在终端里逐文件翻看，缺乏一个可视化入口来回答"每个 change 走到哪一步了、评审说了什么、产物长什么样"。

更深一层的动机是战略性的：插件体系未来将转型多 agent 架构、逐步抛弃 plugin 形态，最终的读写运行时会落在 Rust。本变更是未来 Rust 运行时的第一块砖——不只做一个看板，而是把"读取 change 域数据"这件事沉淀为独立的纯 Rust 核心库，为后续引擎化预留地基。

数据面还有一个现实约束：`openspec/changes/`（含 archive 共 74 个历史 change）横跨三代结构——

| 代际 | 特征 | 判定 |
|---|---|---|
| v2（现在） | workflow.json 含 `file_log[]`、`active_phase`、`interrupted[]` | 有 workflow.json 且有 file_log |
| v1（中期） | workflow.json 只有 `eval[]` + 旧 `files{}` 桶；有 `phases/` 快照 | 有 workflow.json 无 file_log |
| v0（早期） | 仅 markdown 产物（proposal/design/tasks/specs） | 无 workflow.json |

任何读取端都必须消化这三代差异，否则只能看到一半历史。

---

## 提案

新建完全独立包 `packages/desktop`：一个 Tauri 2 桌面应用（React + TS 前端 / Rust 后端），读取**任意 workspace**（用户挑选的项目根目录）下的 changes 记录，展示 change 列表、phase 流水线、eval checklist、markdown 产物与 tasks 勾选进度。取数模型刻意简单：**不做文件 watch，提供刷新按钮**；未来可平滑替换为推送。

核心落点：

1. **Rust 解析层**（不复用 TS zod）：`serde` 宽松解析 + 代际探测（v2/v1/v0），未知字段忽略、单条损坏降级不炸整份记录。TS 侧 schema（`plugins/dev-team/bin/src/schemas/workflow.schema.ts`）仍是唯一真理源，Rust 侧靠 fixture 语料快照回归兜底漂移。
2. **crate 三层分组**：`core/foundation`（layout 路径解析，刻意极小）、`core/workflow`（change 域 model/parse/queries/artifacts）、`desktop-app`（Tauri 壳）。编译器依赖图强制边界：`desktop-app → workflow → foundation`。代码命名一律不含 `openspec` 字样（该名称未来要改），磁盘路径隔离收进 `resolve(root) → Layout`，目录改名只动这一个函数。
3. **中间产物插件化**：产物类型是第一变化轴（v1 `phases/` 快照消失、`files{}` → `file_log[]`、reports 格式演进），故将"产物"隔离为两侧注册表（Rust matcher/parser / React renderer），中间以 `ArtifactEnvelope`（kind / version / title / payload / fallback_text）稳定信封缝合。新增产物 = 两侧各加一个自包含模块，核心零改动。MVP 实现第一波三个插件实例（`markdown-doc`、`eval-checklist`、`tasks-progress`，均服务第一刀视图）；`file-log`、`test-report-summary`、`html-report-ref` 随第二刀对应视图（file_log 时间线 / reports 渲染 / mutation 内嵌）落地——插件化"后加便宜"正是延后的依据。
4. **永不白屏**：代际降级（v2 完整 / v1 部分 / v0 仅文档）与未注册 kind 的 Fallback 组件是硬要求，与宽松解析同哲学。
5. **指令预留缝**：Tauri command 双轨——`commands/queries/`（list_changes / get_change_detail / read_artifact，MVP 实现）与 `commands/exec/`（预留空轨道，不实现任何真实命令）。所有 workspace 状态只经 core 函数访问，Tauri command 是无状态薄包装。

**与项目规则的边界（显式声明）**：CLAUDE.md「改插件源码后 bump 版本 + rebuild 产物」针对 `plugins/` 及其 dist 分发链。`packages/desktop` 是独立 app，自带 lockfile、不引入根 workspace、不进 `dist/` 分发链，**不受该规则约束**。

---

## 能力

### 新增能力

- `desktop-crate-layout` — `packages/desktop` 独立包落位与 crate 三层分组（core/foundation + core/workflow + desktop-app）、依赖规则、独立构建边界
- `workspace-layout-resolution` — foundation crate 的 layout 解析器：`resolve(root) → Layout`，磁盘路径不写死，`openspec/` 目录改名唯一触点；代码命名隔离
- `desktop-change-parse` — workflow crate 对三代 workflow.json 的 serde 宽松解析与代际探测、损坏降级
- `desktop-change-queries` — workflow crate 的扫描/聚合查询：change 列表（active + archive、代际标注、按月分组）与 change 详情（流水线 / attempt / verdict / checklist / 产物清单）
- `desktop-artifact-plugins` — ArtifactEnvelope 信封契约、Rust matcher/parser 注册表、React renderer 注册表、Fallback 兜底、第一波三个插件实例
- `desktop-app-shell` — Tauri 壳与前端：workspace 选择（文件夹选择器）、queries 双轨 command、exec 预留轨道、React hooks 刷新取数与渲染
- `desktop-corpus-regression` — 三代代表性 fixture 语料与全量解析快照回归，防 TS schema 演进导致 Rust 解析静默劣化

### 修改的能力

无。本变更不触碰任何现有能力。

---

## 变更范围

### 实现文件

- `packages/desktop/package.json`、`packages/desktop/pnpm-lock.yaml`（独立包，自带 lockfile）
- `packages/desktop/index.html`、`packages/desktop/src/`（React + TS 前端：App、change 列表视图、change 详情视图、`hooks/useChangeList`、`hooks/useChangeDetail`、`renderers/` 注册表与各 renderer 组件、Fallback 组件）
- `packages/desktop/src-tauri/tauri.conf.json` 及 Tauri 工程配置
- `packages/desktop/src-tauri/Cargo.toml`（cargo workspace）与 `packages/desktop/src-tauri/crates/foundation/`（`layout` 模块：`resolve(root) → Layout`）
- `packages/desktop/src-tauri/crates/workflow/src/`（`model/`：Change / Workflow / Inventory(v2|v1|v0)；`parse/`：serde 宽松解析 + 代际探测；`queries/`：扫描与聚合；`artifacts/`：envelope + matcher/parser trait + 静态注册表 + 三个第一波插件）
- `packages/desktop/src-tauri/crates/desktop-app/src/`（`commands/queries/`：list_changes / get_change_detail / read_artifact；`commands/exec/`：预留空轨道；main 入口）

### 测试文件

- `packages/desktop/src-tauri/crates/workflow/tests/fixtures/`（从 74 个 archive 挑选的三代代表样本语料）
- `packages/desktop/src-tauri/crates/workflow/tests/`（代际探测、宽松解析降级、layout 解析、queries 聚合的单测；全量解析 fixtures → golden 快照回归）

### 删除文件

无。

### 不要修改

- `plugins/**` 与 `dist/**` —— desktop 不进插件分发链，不改任何插件源码
- 仓库根目录 —— MUST NOT 新增 `package.json` / `pnpm-workspace.yaml`
- `openspec/changes/**` 现有数据 —— 只读，desktop 端对任意 workspace 均只读
- `plugins/dev-team/bin/src/schemas/workflow.schema.ts` —— TS schema 是唯一真理源，不因 Rust 侧实现而改动
- `commands/exec/` —— 预留空轨道，不实现任何真实命令、不引入空壳 trait

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | desktop-app workspace 选择 | App 提供文件夹选择器；选定后所有取数以该目录为 workspace 根 |
| AC-2 | foundation layout 解析 | `resolve(root)` 返回 `Layout { changes_root, archive_root, explores_root }`；除该函数外，仓库中 desktop 源码无任何硬编码 `openspec` 路径字符串；crate 名 / 模块名 / 类型名均不含 `openspec` 字样 |
| AC-3 | 代际探测 | `Inventory` 枚举按规则判定：有 workflow.json 且有 `file_log` → v2；有 workflow.json 无 `file_log` → v1；无 workflow.json → v0；对 fixtures 中三代代表样本判定正确 |
| AC-4 | serde 宽松解析 | 未知字段忽略；单条 eval / file_log 条目损坏时降级跳过该条，其余数据正常返回，不炸整份记录 |
| AC-5 | change 列表查询 | `list_changes` 返回 active 与 archive 全量，逐条带代际标注；archive 按月分组，日期取目录名前缀，缺失时归入"未知时间"组 |
| AC-6 | change 详情查询 | `get_change_detail` 返回 9 站流水线，每站含 attempt 序列、verdict、report、checklist（item / pass / evidence），并暴露 active_phase 与 backtrack 字段（backtrack_to / backtrack_reason） |
| AC-7 | ArtifactEnvelope 契约 | 所有中间产物以信封形式传递（kind / version / title / payload / fallback_text），kind 命名不含 `openspec` 字样；前端按 kind 路由到 renderer，未注册 kind 走 Fallback |
| AC-8 | 第一波三个插件 | `markdown-doc`、`eval-checklist`、`tasks-progress` 三个 kind 均有 matcher + parser（Rust）与 renderer（React）注册；`markdown-doc` 自带按代际的文件名匹配，Docs 探测不另设并行机制 |
| AC-9 | tasks 勾选进度 | `tasks-progress` 插件统计 tasks.md 中 `- [ ]` / `- [x]` 数量并渲染进度 |
| AC-10 | 永不白屏 | 未注册 kind → Fallback 渲染 `fallback_text` + kind 徽标；v0 change 以纯文档形态呈现；v1 change 缺 `file_log` 等字段时对应区块降级留空，不报错、不白屏 |
| AC-11 | Tauri command 双轨 | `commands/queries/` 实现三个命令且均为无状态薄包装（参数 → core → DTO）；`commands/exec/` 轨道存在但为空；workspace 状态只经 core 函数访问 |
| AC-12 | 刷新取数 | 前端经 `useChangeList` / `useChangeDetail` 显式刷新触发 invoke；无文件 watch、无后台轮询 |
| AC-13 | 独立包边界 | `packages/desktop` 自带 lockfile 与构建脚本；根目录无新增 workspace 配置；`dist/` 不含 desktop 产物 |
| AC-14 | 快照回归 | 存在全量解析 fixtures 语料并与 golden 对比的测试；fixtures 覆盖三代代表样本（从 archive 挑选入仓） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| TS 侧 schema 演进导致 Rust 解析静默劣化 | 详情数据缺失或错乱且无告警 | 中 | 宽松解析 + 74 archive 语料快照回归；TS zod 仍为唯一真理源 |
| 三代老数据结构怪异（字段缺失、命名漂移） | 解析崩溃或渲染白屏 | 中 | 代际降级展示 + Fallback 组件硬要求；单条损坏降级不炸整份 |
| MVP 范围膨胀（前后端双注册表一步到位） | 拖长交付、抽象被过早定形 | 中 | 插件各自自包含、加法不改核心；第一波收敛为三个（服务第一刀视图），其余 kind 随第二刀视图落地；exec 轨道坚持空置 |
| Tauri 2 + Windows 环境搭建摩擦 | 前期阻塞 | 低 | 独立包独立工具链，失败不影响主仓；MVP 不涉及打包签名 |
| evaluator 误套「bump 版本 + rebuild」规则到 desktop | 误判验收失败 | 低 | 本提案显式声明 desktop 不进 dist 分发链（见提案节末） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 解析层用 TS 还是 Rust | Rust serde 解析，不复用 TS zod | 未来写入也在 Rust；桌面端是第二实现，schema 漂移靠宽松解析 + 语料回归兜底 | 复用 TS zod（与 Rust 运行时战略相悖） |
| 实时性方案 | 不做 watch，刷新按钮 | 砍掉 notify/事件/去抖；取数收在 hook 后，未来可换推送 | 文件 watch + 事件推送 |
| 老数据兼容 | 三代降级展示（v2 完整 / v1 部分 / v0 仅文档） | Inventory enum 收编代际差异，永不白屏 | 仅支持 v2（丢一半历史） |
| 包落位 | `packages/desktop` 完全独立包，自带 lockfile | 不引入根 workspace，不进 dist 分发链 | 纳入根 workspace / 插件分发链 |
| 前端栈 | React + TS | markdown 密集型查看器，生态零件最全 | Vue / Svelte / 纯原生 |
| workspace 语义 | 通用：用户挑任意项目根目录 | 读任意项目的 changes 记录（最近打开列表延至第二刀） | 绑定本仓库路径 |
| 命名 | 代码中不出现 `openspec`；磁盘路径收进 layout 解析器 | 该名称未来要改；两层隔离（命名 + 路径） | 硬编码 `openspec/` 路径 |
| 执行能力 | 预留 command 双轨空轨道，不建空壳 trait | 今天只读；trait 定形等第一条真实命令落地 | 预定义 Executor trait 空壳 |
| crate 组织 | 三层分组：core/foundation、core/workflow、desktop-app；依赖 desktop-app → workflow → foundation | crate 缝即未来按域抽取缝，编译器依赖图强制边界 | 单 crate + 模块（边界弱）；扁平多 crate（无分组） |
| foundation 范围 | 今天只装 layout；第二个消费者出现再下沉 | 避免预铺 fs 助手 / 错误类型等投机代码 | 预铺通用工具集 |
| 中间产物组织 | 抽象 + 插件：ArtifactEnvelope 信封 + 两侧静态注册表 + Fallback 兜底 | 产物类型是第一变化轴；加法而非改法；静态注册已满足诉求，不做 dylib | 硬编码产物分支；dylib 动态加载 |
| 产物抽象放置 | 住 `workflow` crate `artifacts/` 模块，archi 域落地时再提升 foundation | 沿用"第二个消费者出现再下沉"纪律 | 直接放 foundation |

### 待决问题

- 渲染器不独立于 Rust 更新（与解析器同版本发）——explore 标注"proposal 阶段可复议"，暂按推荐执行
- fixtures 三代代表样本的具体挑选清单——留待 test-design 阶段从 74 个 archive 圈定
- 第二刀范围与排期：最近打开列表、`file-log` / `test-report-summary` / `html-report-ref` 插件（分别随 file_log 时间线视图、reports 渲染、mutation.html 内嵌落地）、阶段耗时统计、多 workspace 常驻、exec 轨道接入
- Tauri 2 在 Windows 的打包与签名发布细节——MVP 不涉及，暂缓

---
