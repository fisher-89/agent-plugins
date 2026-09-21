# 设计: add-desktop-terminal

> **变更**: add-desktop-terminal
> **日期**: 2026-09-20

---

## 范围与同步说明

- `proposal.md` 与 `specs/`（7 个能力规格）已由提案阶段写入并由评审通过（见 `workflow.json` eval[0]），**不在本变更清单与任务列表内**。
- proposal「变更范围 - 实现文件」以目录组表述（`crates/foundation/`、`crates/workflow/`），本设计按 spec `desktop-crate-layout` 的三层分组落位为 `crates/core/foundation/`、`crates/core/workflow/`、desktop-app——与 proposal 决策表「三层分组：core/foundation、core/workflow、desktop-app」一致，仅路径前缀差异。
- **实现偏差（已核实根因）**：desktop-app 包落位由 `crates/desktop-app/` 调整为**根包混合 workspace**——`src-tauri/Cargo.toml` 兼作 desktop-app 的 `[package]` 与 workspace 根，应用源码位于 `src-tauri/src/`。原因：tauri-cli（`RustAppSettings::new`，interface/rust.rs）强制要求 `tauri.conf.json` 所在目录（tauri_dir = src-tauri/）的 Cargo.toml 含 `[package]` 段，虚拟 workspace 会使 `tauri dev/build` 直接报 "No package info in the config file"（实测复现）。三层分组与依赖方向（desktop-app → workflow → foundation）不变，仅 desktop-app 的物理落位变化；`tauri.conf.json` / `build.rs` / `capabilities/` / `icons/` 保持在 src-tauri 根原位。
- proposal「变更范围 - 测试文件」（`tests/fixtures/` 语料与单测 / 快照回归）属 test-design / test-gen 阶段产物，不进本变更清单；设计仅在 AC 对齐中预留其挂接点。
- `packages/desktop/dist/` 是 Vite 前端构建输出目录，与仓库根 `dist/`（插件分发链）无关；仓库根 `dist/` 不含任何 desktop 产物（AC-13）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| foundation crate | 磁盘布局解析：`resolve(root) -> Layout`，纯路径推导无 IO；全包唯一知晓 `openspec/` 磁盘目录名的位置 | `packages/desktop/src-tauri/crates/core/foundation/` | 无（零 workspace 内依赖、零第三方依赖） | Rust（裸 crate 名 `foundation`） |
| workflow crate | change 域纯读库：`model`（领域类型）/ `parse`（代际探测 + serde 宽松解析）/ `queries`（列表 + 详情聚合）/ `artifacts`（信封 + 插件注册表 + 第一波三插件） | `packages/desktop/src-tauri/crates/core/workflow/` | foundation；serde / serde_json / time；**零 Tauri 依赖、无指令概念** | Rust（裸 crate 名 `workflow`） |
| desktop-app crate | Tauri 壳：`commands/queries/` 三个无状态薄包装命令、`commands/exec/` 预留空轨道、入口注册 | `packages/desktop/src-tauri/`（根 Cargo.toml `[package]` + `src/`，见偏差注释） | workflow + foundation；tauri 2、tauri-plugin-dialog | Rust + Tauri 2 |
| 前端应用 | workspace 选择（文件夹选择器）、列表 / 详情视图、取数收口 hooks、renderer 注册表与 Fallback | `packages/desktop/src/` | desktop-app 经 Tauri IPC；react、@tauri-apps/api、@tauri-apps/plugin-dialog、react-markdown | React + TypeScript + Vite |
| 包与工程配置 | 独立包边界：自带 lockfile、Tauri / Vite / TS / capability 配置 | `packages/desktop/`（package.json、index.html、vite.config.ts、tsconfig.json、src-tauri/tauri.conf.json 等） | — | pnpm + vite-plus（vp）+ @tauri-apps/cli |

**依赖图**（编译器强制）：`desktop-app -> workflow -> foundation`；foundation 不依赖任何人；workflow 无 Tauri 依赖（`core/` 仅为目录名，内部 crate 用裸名，不存在名为 `core` 的 crate）。

**数据流**（无 watch）：

```
[文件夹选择器 plugin-dialog] -> workspace root（前端 state）
[刷新按钮] -> useChangeList / useChangeDetail
  -> invoke("list_changes" | "get_change_detail")
  -> command 薄包装 -> foundation::layout::resolve(root) -> workflow::queries
  -> DTO（serde camelCase）-> 前端渲染
详情内产物：ChangeDetail.artifacts（Descriptor 清单）
  -> useChangeDetail 内逐个 invoke("read_artifact") -> ArtifactEnvelope
  -> ArtifactView -> resolveRenderer(kind) -> 专属 renderer / Fallback
```

取数纪律：组件不直接 invoke，取数统一经两个 hooks；`useChangeDetail` 取得详情后在同一显式刷新周期内按产物清单逐个请求信封并组装进自身状态（读取失败的产物以 Fallback 形态呈现，不阻断其余产物）——未来换推送只改 hooks 内部实现。

---

## 变更清单

以文件为入口逐层展开，实现阶段以此清单为边界。全部为新增，无修改、无删除。

### 新增文件

**包根与工程配置**

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/package.json` | 前端包描述：name / private / scripts（dev、build、tauri）/ dependencies / devDependencies；独立包不进根 workspace |
| `packages/desktop/pnpm-lock.yaml` | 独立 lockfile，由 `pnpm install` 在 `packages/desktop` 内生成 |
| `packages/desktop/index.html` | Vite 入口 HTML（挂载 `src/main.tsx`） |
| `packages/desktop/vite.config.ts` | vite-plus 配置（defineConfig 取自 vite-plus）：React 插件、dev server 端口与 `tauri.conf.json` 的 devUrl 对齐；`vp dev` / `vp build` / `vp test` 共用 |
| `packages/desktop/tsconfig.json` | TypeScript 严格编译配置（React JSX、bundler 解析） |
| `packages/desktop/src-tauri/Cargo.toml` | 根包混合 workspace：`[package]`（desktop-app，依赖见 desktop-app 清单行）+ `[workspace]`（`members = ["crates/core/foundation", "crates/core/workflow"]`，根包隐式成员），resolver = 2，workspace 级依赖版本收敛 |
| `packages/desktop/src-tauri/build.rs` | Tauri 构建脚本（`tauri_build::build()`） |
| `packages/desktop/src-tauri/tauri.conf.json` | Tauri 工程配置：productName / identifier / build 前后端命令与目录 / 主窗口参数 |
| `packages/desktop/src-tauri/capabilities/default.json` | Tauri 2 capability：`core:default` + `dialog:allow-open`（文件夹选择器） |

**前端 `src/`**

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/main.tsx` | React 入口挂载 |
| `packages/desktop/src/App.tsx` | 应用壳：workspace 选择（plugin-dialog `open({ directory: true })`）+ 列表 / 详情视图切换 + 刷新动作下发 |
| `packages/desktop/src/types/dto.ts` | 查询 DTO 与 `ArtifactEnvelope` 的 TS 镜像类型（对齐 serde camelCase） |
| `packages/desktop/src/views/ChangeListView.tsx` | change 列表视图：active 列表 + archive 按月分组（"未知时间"组置尾）、代际徽标（v2/v1/v0）、点击进入详情 |
| `packages/desktop/src/views/ChangeDetailView.tsx` | change 详情视图：9 站流水线（attempt 序列 / verdict / checklist 展开）、active_phase 运行中标示、v0 纯文档形态、v1 区块留空降级、产物区 |
| `packages/desktop/src/hooks/useChangeList.ts` | 列表取数 hook：显式 refresh 触发 `invoke("list_changes")`；无轮询、无 watch |
| `packages/desktop/src/hooks/useChangeDetail.ts` | 详情取数 hook：显式 refresh 触发 `invoke("get_change_detail")`，并在同周期内逐个 `invoke("read_artifact")` 组装产物信封 |
| `packages/desktop/src/renderers/registry.ts` | renderer 注册表：kind 到组件的映射 + `resolveRenderer(kind)`（未注册返回 Fallback） |
| `packages/desktop/src/renderers/ArtifactView.tsx` | 信封路由组件：按 `resolveRenderer(kind)` 渲染单个 `ArtifactEnvelope` |
| `packages/desktop/src/renderers/Fallback.tsx` | 未注册 kind 兜底组件：`fallback_text` 渲染 + kind 徽标（硬要求，永不白屏） |
| `packages/desktop/src/renderers/MarkdownDocRenderer.tsx` | `markdown-doc` renderer：`payload.markdown` 经 react-markdown 渲染 |
| `packages/desktop/src/renderers/EvalChecklistRenderer.tsx` | `eval-checklist` renderer：items（item / pass / evidence）清单渲染 |
| `packages/desktop/src/renderers/TasksProgressRenderer.tsx` | `tasks-progress` renderer：total / done / pending 进度渲染 |

**foundation crate（`crates/core/foundation/`）**

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/foundation/Cargo.toml` | crate 清单：零依赖（刻意极小，不预铺通用工具） |
| `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs` | crate 入口，导出 `layout` 模块 |
| `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs` | `Layout` 结构与 `resolve(root) -> Layout` 纯路径推导；磁盘目录名唯一触点 |

**workflow crate（`crates/core/workflow/`）**

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/Cargo.toml` | crate 清单：foundation + serde（derive）+ serde_json + time；禁 Tauri |
| `packages/desktop/src-tauri/crates/core/workflow/src/lib.rs` | crate 入口，导出 model / parse / queries / artifacts |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/mod.rs` | model 模块根与 re-export |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/inventory.rs` | `Inventory` 枚举（V2 / V1 / V0） |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | `Workflow` / `PhaseLog` / `ChecklistItem` / `Verdict` / `FileLogOp` / `FileLogEntry` / `ActivePhase` / `InterruptedEntry`（宽松 Option 字段 + camelCase） |
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/mod.rs` | parse 模块根与 re-export |
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/detect.rs` | `detect_inventory`：基于磁盘事实的代际判定 |
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs` | 两段式宽松解析 `workflow.json`：单条降级、整体 Unparsable、ISO 8601 失败降级空 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs` | queries 模块根与 re-export |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | `list_changes`：全量扫描 + 代际标注 + archive 按月分组（未知时间组） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `change_detail`：固定 9 站流水线聚合 + active_phase + backtrack + file_log 区块 + 产物清单 |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/mod.rs` | artifacts 模块根与 re-export |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/envelope.rs` | `ArtifactEnvelope` / `ArtifactDescriptor` / `ArtifactCandidate` |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs` | `ArtifactPlugin` trait + 编译期静态注册表 + 候选枚举 + `discover_artifacts` / `read_artifact` |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/markdown_doc.rs` | `markdown-doc` 插件：目录树内 .md 收录，代际名单决定 title 标注与排序 |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist.rs` | `eval-checklist` 插件：逐 eval 条目产出 checklist 信封 |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress.rs` | `tasks-progress` 插件：tasks.md 勾选计数（total / done / pending） |

**desktop-app crate（根包混合 workspace：`src-tauri/Cargo.toml` `[package]` + `src/`）**

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/Cargo.toml` | desktop-app 包清单兼 workspace 根：`[package]` 依赖 foundation + workflow + tauri + tauri-plugin-dialog（均 workspace 继承）；`[features]` custom-protocol；members 仅列 crates/core 两 crate（根包隐式成员） |
| `packages/desktop/src-tauri/build.rs` | Tauri 构建脚本（包根即 src-tauri，`tauri.conf.json` / `capabilities/` / `icons/` 相对解析直接可用） |
| `packages/desktop/src-tauri/src/main.rs` | Tauri Builder 入口：注册 dialog 插件与三个查询命令 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | commands 模块根：挂载 queries / exec 双轨 |
| `packages/desktop/src-tauri/src/commands/queries/mod.rs` | 三个 `#[tauri::command]` 薄包装：list_changes / get_change_detail / read_artifact |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 预留空轨道：仅模块注释，无任何命令、无空壳 trait |
| `packages/desktop/src-tauri/icons/icon.ico` | Windows 资源编译必需图标（tauri-build 在 Windows 硬要求） |

<!-- 如无修改文件，省略此子节：本变更不修改任何既有文件（仓库根、plugins/**、openspec/** 现有数据均不动） -->

<!-- 如无删除文件，省略此子节：无删除 -->

### 公共函数 / API

仅列模块级导出函数、Tauri command、前端导出 hook；插件模块内部的私有实现不列入。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `resolve` | `crates/core/foundation/src/layout.rs` | 新增 | `pub fn resolve(root: &Path) -> Layout` | 纯路径推导出三棵目录树；全包唯一磁盘目录名触点；对不存在的 root 正常返回 |
| `detect_inventory` | `crates/core/workflow/src/parse/detect.rs` | 新增 | `pub fn detect_inventory(change_dir: &Path) -> Inventory` | 有 workflow.json 且含 `file_log` 键为 v2；有 workflow.json 无 `file_log` 为 v1；无 workflow.json 为 v0；不依赖目录命名约定 |
| `parse_workflow_file` | `crates/core/workflow/src/parse/workflow_file.rs` | 新增 | `pub fn parse_workflow_file(path: &Path) -> WorkflowFileParse` | 两段式宽松解析：Value 逐条 `from_value`，单条 eval / file_log 损坏跳过，核心字段非法整体 Unparsable |
| `list_changes` | `crates/core/workflow/src/queries/list.rs` | 新增 | `pub fn list_changes(layout: &Layout) -> ChangeList` | 扫描 changes_root + archive_root 全量；目录缺失返回空而非报错；archive 按目录名日期前缀按月分组 |
| `change_detail` | `crates/core/workflow/src/queries/detail.rs` | 新增 | `pub fn change_detail(layout: &Layout, name: &str) -> Option<ChangeDetail>` | 固定 9 站聚合 eval attempts；暴露 active_phase / backtrack / file_log 区块；附 discover_artifacts 产物清单；纯读 |
| `discover_artifacts` | `crates/core/workflow/src/artifacts/registry.rs` | 新增 | `pub fn discover_artifacts(change_dir: &Path, inventory: Inventory, workflow: Option<&Workflow>) -> Vec<ArtifactDescriptor>` | 枚举候选（文件树 + eval 条目）喂给注册表，收集全部命中为 Descriptor 清单 |
| `read_artifact` | `crates/core/workflow/src/artifacts/registry.rs` | 新增 | `pub fn read_artifact(change_dir: &Path, inventory: Inventory, workflow: Option<&Workflow>, kind: &str, source: &str) -> Option<ArtifactEnvelope>` | 按 kind 定位插件、按 source 重建候选后解析出信封 |
| `ArtifactPlugin`（trait） | `crates/core/workflow/src/artifacts/registry.rs` | 新增 | `pub trait ArtifactPlugin: Send + Sync { fn kind(&self) -> &'static str; fn matches(&self, input: &ArtifactInput) -> bool; fn parse(&self, input: &ArtifactInput) -> Option<ArtifactEnvelope> }` | 每类产物一个自包含实现并自注册进静态注册表；编译期注册、无 dylib；同候选多 kind 命中并存（tasks.md 同时产出 markdown-doc 与 tasks-progress），无排他 |
| `list_changes`（command） | `src-tauri/src/commands/queries/mod.rs` | 新增 | `#[tauri::command] pub fn list_changes(root: String) -> ChangeList` | 无状态薄包装：root -> `resolve` -> core `list_changes` -> DTO；无 State、无文件系统直访 |
| `get_change_detail`（command） | `src-tauri/src/commands/queries/mod.rs` | 新增 | `#[tauri::command] pub fn get_change_detail(root: String, change: String) -> Option<ChangeDetail>` | 同上，未知 change 名返回 None |
| `read_artifact`（command） | `src-tauri/src/commands/queries/mod.rs` | 新增 | `#[tauri::command] pub fn read_artifact(root: String, change: String, kind: String, source: String) -> Option<ArtifactEnvelope>` | 同上，单产物信封读取 |
| `useChangeList` | `packages/desktop/src/hooks/useChangeList.ts` | 新增 | `function useChangeList(root: string | null): ChangeListState` | 显式 refresh 触发 invoke；返回 `ChangeListState`（见类型定义） |
| `useChangeDetail` | `packages/desktop/src/hooks/useChangeDetail.ts` | 新增 | `function useChangeDetail(root: string | null, change: string | null): ChangeDetailState` | 显式 refresh 触发详情 + 产物信封取数；返回 `ChangeDetailState`（见类型定义） |
| `resolveRenderer` | `packages/desktop/src/renderers/registry.ts` | 新增 | `function resolveRenderer(kind: string): RendererComponent` | 未注册 kind 返回 Fallback 组件 |

### 类型定义

**Rust（foundation / workflow）**

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `Layout` | `crates/core/foundation/src/layout.rs` | 新增 | `{ changes_root: PathBuf, archive_root: PathBuf, explores_root: PathBuf }` |
| `Inventory` | `crates/core/workflow/src/model/inventory.rs` | 新增 | `enum Inventory { V2, V1, V0 }`，代际标注 |
| `Workflow` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `{ workflow_type: String, created: Option<String>, eval: Vec<PhaseLog>, file_log: Option<Vec<FileLogEntry>>, active_phase: Option<ActivePhase>, interrupted: Vec<InterruptedEntry> }`；未知键（含 legacy `files` 桶、`source` 旁挂）忽略；serde camelCase |
| `PhaseLog` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `{ phase: String, attempt: Option<u32>, verdict: Verdict, report: String, checklist: Vec<ChecklistItem>, skipped: bool, stale: bool, start_at: Option<OffsetDateTime>, timestamp: Option<OffsetDateTime>, backtrack_to: Option<String>, backtrack_reason: Option<String> }` |
| `Verdict` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `enum Verdict { Pass, Fail }`；条目级严格，非法值触发该条降级跳过 |
| `ChecklistItem` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `{ item: String, pass: bool, evidence: String }` |
| `FileLogOp` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `enum FileLogOp { Write, Delete, Revert }` |
| `FileLogEntry` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `{ op: FileLogOp, scope: String, attempt: Option<u32>, path: String, at: Option<OffsetDateTime> }` |
| `ActivePhase` | `crates/core/workflow/src/model/workflow.rs` | 新增 | `{ phase: String, attempt: u32, start_at: Option<OffsetDateTime> }`；`InterruptedEntry` 为其加 `end_at` 的扩展 |
| `WorkflowFileParse` | `crates/core/workflow/src/parse/workflow_file.rs` | 新增 | `enum WorkflowFileParse { Parsed(Workflow), Unparsable { reason: String } }`：整体降级标记 |
| `ChangeSummary` | `crates/core/workflow/src/queries/list.rs` | 新增 | `{ name: String, source: ChangeSource, inventory: Inventory, created: Option<String>, unparsable: bool }`；created 优先取 workflow.json `created`，archive 回退目录名日期前缀 |
| `ChangeSource` | `crates/core/workflow/src/queries/list.rs` | 新增 | `enum ChangeSource { Active, Archive }` |
| `ChangeList` | `crates/core/workflow/src/queries/list.rs` | 新增 | `{ active: Vec<ChangeSummary>, archive_groups: Vec<ArchiveGroup> }` |
| `ArchiveGroup` | `crates/core/workflow/src/queries/list.rs` | 新增 | `{ month: Option<String>, changes: Vec<ChangeSummary> }`；month 形如 "2026-05"，None 即"未知时间"组，固定排组序列尾 |
| `ChangeDetail` | `crates/core/workflow/src/queries/detail.rs` | 新增 | `{ name, source, inventory, created, unparsable, pipeline: Vec<PhaseEntry>, active_phase: Option<ActivePhase>, interrupted: Vec<InterruptedEntry>, file_log: Option<Vec<FileLogEntry>>, artifacts: Vec<ArtifactDescriptor> }` |
| `PhaseEntry` | `crates/core/workflow/src/queries/detail.rs` | 新增 | `{ phase: String, attempts: Vec<AttemptRecord> }`；固定 9 站全量输出（无 attempt 的站为空序列），顺序固定不依赖 eval 排列 |
| `AttemptRecord` | `crates/core/workflow/src/queries/detail.rs` | 新增 | `{ attempt: Option<u32>, verdict: Verdict, report: String, checklist: Vec<ChecklistItem>, skipped: bool, stale: bool, start_at, timestamp, backtrack_to, backtrack_reason }`：backtrack 目标与原因随条目可查 |
| `ArtifactEnvelope` | `crates/core/workflow/src/artifacts/envelope.rs` | 新增 | `{ kind: String, version: u32, title: String, payload: serde_json::Value, fallback_text: Option<String> }`：两侧唯一共享契约 |
| `ArtifactDescriptor` | `crates/core/workflow/src/artifacts/envelope.rs` | 新增 | `{ kind: String, source: String, title: String }`：产物寻址（source 为 change 内相对 POSIX 路径或 eval 条目序号串） |
| `ArtifactCandidate` | `crates/core/workflow/src/artifacts/envelope.rs` | 新增 | `enum ArtifactCandidate { File { relative_path: PathBuf }, EvalEntry { index: usize } }` |
| `ArtifactInput` | `crates/core/workflow/src/artifacts/registry.rs` | 新增 | `{ change_dir: &Path, inventory: Inventory, workflow: Option<&Workflow>, candidate: &ArtifactCandidate }`：matcher / parser 入参 |

**TypeScript（前端镜像，`src/types/dto.ts`）**

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ChangeList` / `ChangeSummary` / `ArchiveGroup` | `packages/desktop/src/types/dto.ts` | 新增 | 镜像同名 Rust DTO（camelCase） |
| `ChangeDetail` / `PhaseEntry` / `AttemptRecord` | `packages/desktop/src/types/dto.ts` | 新增 | 镜像同名 Rust DTO |
| `ArtifactDescriptor` / `ArtifactEnvelope` | `packages/desktop/src/types/dto.ts` | 新增 | 信封契约前端侧；payload 为 `unknown`，由各 renderer 自行收窄 |
| `ChangeListState` | `packages/desktop/src/hooks/useChangeList.ts` | 新增 | 含 data（`ChangeList` 或 null）、loading、error 与 refresh 回调 |
| `ChangeDetailState` | `packages/desktop/src/hooks/useChangeDetail.ts` | 新增 | 含 detail（`ChangeDetail` 或 null）、artifacts（`ArtifactEnvelope[]`）、loading、error 与 refresh 回调 |
| `RendererComponent` | `packages/desktop/src/renderers/registry.ts` | 新增 | 信封渲染组件类型：入参单个 `ArtifactEnvelope` |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `name` / `private` | `packages/desktop/package.json` | 新增 | string / boolean | `"desktop"` / `true` | 独立包名，不进根 workspace |
| `scripts.dev` / `scripts.build` / `scripts.tauri` | `packages/desktop/package.json` | 新增 | string | `vp dev` / `tsc && vp build` / `tauri` | 前端开发、构建与 Tauri 驱动入口（vp build 不含类型检查，tsc 保留类型门槛） |
| `dependencies` / `devDependencies` | `packages/desktop/package.json` | 新增 | object | 见「依赖」节 | react 系为运行时；vite / tsc / tauri-cli 为构建 |
| `productName` / `identifier` | `packages/desktop/src-tauri/tauri.conf.json` | 新增 | string | `"desktop-terminal"` / `"dev.wps.desktop-terminal"` | 均不含 openspec 字样 |
| `build.beforeDevCommand` / `build.devUrl` / `build.beforeBuildCommand` / `build.frontendDist` | `packages/desktop/src-tauri/tauri.conf.json` | 新增 | string | `"pnpm dev"` / `"http://localhost:5173"` / `"pnpm build"` / `"../dist"` | 指向 packages/desktop 本地 Vite 产物，非仓库根 dist/ |
| `app.windows[]` | `packages/desktop/src-tauri/tauri.conf.json` | 新增 | object | 单窗口，标题 "Desktop Terminal"，1200x800 | MVP 不涉及打包签名 |
| `permissions` | `packages/desktop/src-tauri/capabilities/default.json` | 新增 | array | `["core:default", "dialog:allow-open"]` | 仅查询 IPC + 文件夹选择器，无 fs / shell 权限 |
| `members` / `workspace.dependencies` | `packages/desktop/src-tauri/Cargo.toml` | 新增 | array / table | 三 crate 成员；tauri、serde 等版本收敛于此 | cargo workspace 根 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `Workflow`（workflow.json v2 形状） | 见类型定义 | 1 个 change 目录至多 1 份；1 Workflow : N PhaseLog、N FileLogEntry；active_phase / interrupted 为运行状态旁挂 | workspace 端 `<changes_root>/<name>/workflow.json`，desktop 只读，不新增持久化 |
| legacy v1 形状 | eval + 旧 `files{}` 桶 | `files{}` 与 `source` 旁挂按未知键忽略；file_log 缺失 → `Workflow.file_log = None` → 详情对应区块留空 | 同上（只读） |
| legacy v0 形状 | 仅 markdown 产物文件 | 无 workflow.json → Inventory=V0、空流水线、产物清单即文档清单 | 磁盘 markdown 文件（只读） |
| `ChangeList` / `ChangeSummary` / `ArchiveGroup` | 见类型定义 | queries 对目录树扫描的内存投影；ArchiveGroup 按 archive 目录名 `YYYY-MM-DD-` 前缀折叠到月，无前缀入"未知时间"组 | 无（内存 DTO，随刷新重建） |
| `ChangeDetail` | 见类型定义 | 由 `Workflow` 聚合派生：eval 按 phase 折叠为固定 9 站 pipeline（attempt 升序），并携带 active_phase、interrupted、file_log 区块与产物 Descriptor 清单 | 无 |
| `ArtifactEnvelope` | kind / version / title / payload / fallback_text | 由 `ArtifactPlugin::parse` 产出，经 IPC 传递；kind 为契约 ID（三个第一波：`markdown-doc`、`eval-checklist`、`tasks-progress`） | 无 |
| 三 kind payload 契约 | 见下表 | kind 自描述负载，version 均从 1 起 | 无 |
| 前端状态 | workspace root（组件 state） | 选定后驱动全部取数；recent list 延后第二刀（需持久化，MVP 无） | 无 |

**payload 契约（第一波三插件，version = 1）**

| kind | payload 形状 | 来源 | fallback_text |
|------|--------------|------|---------------|
| `markdown-doc` | `{ markdown: string }` | change 目录树内 .md 文件原文 | 文件原文即保底 |
| `eval-checklist` | `{ phase: string, attempt: number|null, verdict: string, items: { item: string, pass: boolean, evidence: string }[] }` | workflow.json eval 条目（每条含非空 checklist 的条目一个实例，source 为条目序号） | checklist 文本化清单 |
| `tasks-progress` | `{ total: number, done: number, pending: number }` | tasks.md 中 `- [ ]` / `- [x]` 行计数 | 统计结果文本 |

---

## 路由 / API 设计

本变更无 HTTP API。对外接口为 Tauri IPC invoke（均本地调用、无认证）：

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| invoke | `list_changes` | change 列表（active + archive 分组） | `{ root: string }` | `ChangeList` | 无（本地 IPC） |
| invoke | `get_change_detail` | 单 change 详情聚合 | `{ root: string, change: string }` | `ChangeDetail | null` | 无（本地 IPC） |
| invoke | `read_artifact` | 按信封读取单个产物 | `{ root: string, change: string, kind: string, source: string }` | `ArtifactEnvelope | null` | 无（本地 IPC） |
| dialog | plugin-dialog `open({ directory: true })` | workspace 文件夹选择器 | 前端直调 | 选定根路径（取消返回 null） | 无（本地） |

`commands/exec/` 轨道存在但为空：无命令、无 Executor trait 空壳（trait 定形等第一条真实执行命令落地，见 proposal 决策表）。

---

## 依赖

### 运行时依赖

Rust（`src-tauri` workspace）：

- `tauri`（2.x） — 桌面壳、窗口与 IPC command
- `tauri-plugin-dialog`（2.x） — workspace 文件夹选择器（AC-1）
- `serde`（derive）+ `serde_json` — 宽松解析（Value 两段式）与 DTO / 信封序列化（camelCase）
- `time`（`parsing` / `formatting` / `serde` features） — ISO 8601 时间戳解析；失败降级为 None 而非报错

前端（`packages/desktop`）：

- `react` / `react-dom`（19.x） — 视图层
- `@tauri-apps/api` — `invoke` IPC
- `@tauri-apps/plugin-dialog` — 文件夹选择器前端 API
- `react-markdown` — `markdown-doc` renderer 的 markdown 渲染（markdown 密集型查看器，生态零件最全）
- `remark-gfm`（4.x，与 react-markdown v10 配套） — GFM 扩展解析（表格 / 任务列表 / 删除线等）；OpenSpec 文档（proposal / design / test-design）大量使用表格，react-markdown 默认仅 CommonMark，不挂此插件表格会被渲染成普通段落

### 构建/测试依赖

- `vite-plus`（vp，0.2.x） — TS 工具链统一入口：`vp dev`（dev server）/ `vp build`（生产构建，选项转发 Vite）/ `vp test`（测试）；dev/build/test 全线走 vp
- `vite`（7.x） — 保留于 devDependencies：`@vitejs/plugin-react` 的 peerDependency 要求（vite-plus 自身经 `@voidzero-dev/vite-plus-core` 内联 vite 内核，不发布独立 `vite` 包；移除声明会被 pnpm 自动补装 peer）
- `@vitejs/plugin-react` — React 插件（无 vite 运行时导入，仅类型）
- `typescript`（5.x）+ `@types/react` + `@types/react-dom` — 类型检查
- `@tauri-apps/cli`（2.x） — `tauri dev` / `tauri build` 驱动
- Rust stable toolchain（cargo） — 三 crate 构建；`cargo test`（fixtures 快照回归）属测试阶段，不引入额外 crate

明确不引入：文件监听（notify 等）、后台轮询、dylib 加载、根 workspace、insta 等快照库（golden 机制留待 test-design 决策）。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 workspace 选择 | App.tsx 经 plugin-dialog `open({ directory: true })` 选定根；root 作为参数驱动全部 hooks 与 invoke（路由/API 节） |
| AC-2 layout 解析与命名隔离 | `foundation::layout::resolve` 返回 `Layout{changes_root, archive_root, explores_root}`；磁盘目录名仅此一处；crate / 模块 / 类型 / kind 命名无 openspec 字样（收口任务含标识符扫描） |
| AC-3 代际探测 | `workflow::parse::detect_inventory` 按 workflow.json 存在性 + `file_log` 键判定 V2/V1/V0；fixtures 判定正确性由测试阶段验证 |
| AC-4 serde 宽松解析 | `parse_workflow_file` 两段式解析：未知键忽略、单条 eval / file_log 损坏跳过、核心字段非法整体 Unparsable、时间戳失败降级空 |
| AC-5 change 列表查询 | `workflow::queries::list::list_changes`：active + archive 全量带代际标注；archive 按目录名日期前缀按月分组，无前缀入"未知时间"组；缺失目录树返回空 |
| AC-6 change 详情查询 | `workflow::queries::detail::change_detail`：固定 9 站 PhaseEntry、attempt 升序、verdict / report / checklist 全量、active_phase、backtrack_to / backtrack_reason 随条目可查 |
| AC-7 ArtifactEnvelope 契约 | `artifacts::envelope::ArtifactEnvelope`（kind / version / title / payload / fallback_text）；前端 `resolveRenderer` 按 kind 路由，未注册走 Fallback |
| AC-8 第一波三个插件 | `artifacts/{markdown_doc,eval_checklist,tasks_progress}.rs` + `renderers/{MarkdownDoc,EvalChecklist,TasksProgress}Renderer.tsx` 两侧注册；markdown-doc 自带按代际名单的文件匹配，无并行 Docs 探测路径 |
| AC-9 tasks 勾选进度 | `tasks_progress.rs` 统计 `- [ ]` / `- [x]` 为 total / done / pending；TasksProgressRenderer 渲染进度 |
| AC-10 永不白屏 | Fallback.tsx（fallback_text + kind 徽标）；v0 纯文档形态（空流水线 + 文档清单）、v1 区块留空（file_log = None）、产物读取失败以 Fallback 呈现 |
| AC-11 Tauri command 双轨 | `commands/queries/mod.rs` 三个无状态薄包装（参数 -> resolve -> core -> DTO，无 State、无 fs 直访）；`commands/exec/mod.rs` 空轨道 |
| AC-12 刷新取数 | `useChangeList` / `useChangeDetail` 仅显式 refresh 触发 invoke；无 watch、无轮询、无定时器；组件不直接 invoke |
| AC-13 独立包边界 | `packages/desktop` 自带 lockfile 与构建脚本；根目录无新增 `package.json` / `pnpm-workspace.yaml`；仓库根 `dist/` 无 desktop 产物（收口任务复查） |
| AC-14 快照回归 | 挂接点为 `crates/core/workflow/tests/`（proposal 测试文件范围）；fixtures 挑选与 golden 机制由 test-design 阶段定，实现阶段不建 |

---

## 待决问题

- fixtures 三代代表样本的具体挑选清单 —— proposal 已留待 test-design 阶段从 74 个 archive 圈定（本设计不变更该决策）。
- golden 快照机制选型（手写 golden 文件 + 环境变量显式重写 vs 引入 insta）—— 由 test-design 阶段定；实现阶段不预置。
- markdown-doc 的 v0/v1 legacy 文件名名单（`gan-design.md`、`test-reports/**` 等）—— 按收录目录树内全部 .md、代际名单仅定 title 标注与排序的方案实现，名单细节以 fixtures 实测校准。
- 渲染器与 Rust 解析器同版本发布策略 —— proposal 待决问题沿用，暂按推荐执行（proposal 阶段可复议）。
- 第二刀范围与排期（recent list、`file-log` / `test-report-summary` / `html-report-ref` 插件、耗时统计、多 workspace 常驻、exec 轨道接入）—— 沿用 proposal，不在本变更。
- Tauri 2 在 Windows 的打包与签名发布细节 —— MVP 不涉及，暂缓。

