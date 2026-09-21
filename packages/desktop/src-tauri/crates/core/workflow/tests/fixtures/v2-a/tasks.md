# 任务: add-desktop-terminal

> **变更**: add-desktop-terminal
> 依据: `proposal.md` + `design.md`。测试文件（fixtures / 单测 / 快照回归）由 test-design / test-gen 阶段产出，本列表不包含。

## 阶段 1：独立包与构建骨架

- [x] 创建 `packages/desktop/package.json`（name=`desktop`、private、scripts.dev/build/tauri、dependencies 与 devDependencies 见 design 依赖节）、`index.html`、`vite.config.ts`、`tsconfig.json`
- [x] 在 `packages/desktop` 内运行 `pnpm install` 生成 `pnpm-lock.yaml`，确认仓库根未出现 `package.json` / `pnpm-workspace.yaml`
- [x] 创建 `packages/desktop/src-tauri/Cargo.toml`（cargo workspace，members 三个 crate、resolver=2、workspace 依赖版本收敛）与 `build.rs`、`tauri.conf.json`（productName=`desktop-terminal`、identifier=`dev.wps.desktop-terminal`、build 命令与 `../dist` 指向）、`capabilities/default.json`（`core:default` + `dialog:allow-open`）
- [x] 创建三个 crate 骨架：`crates/core/foundation/Cargo.toml` + 空 `lib.rs`、`crates/core/workflow/Cargo.toml` + 空 `lib.rs`、desktop-app 包（根 `src-tauri/Cargo.toml` `[package]` 段 + `src/main.rs`，见 design 偏差注释：tauri-cli 要求 tauri_dir Cargo.toml 含 [package]），`cargo check` 通过
- [x] 创建前端入口骨架 `src/main.tsx` 与最小 `src/App.tsx`，`pnpm build` 通过

## 阶段 2：foundation crate（layout 解析）

- [x] 实现 `crates/core/foundation/src/lib.rs` 导出 layout 模块；`src/layout.rs` 定义 `Layout { changes_root, archive_root, explores_root }` 与 `pub fn resolve(root: &Path) -> Layout`（纯路径推导、无 IO、对不存在 root 正常返回）；磁盘目录名字符串仅出现在此函数
- [x] 确认 foundation `Cargo.toml` 零依赖（无 workspace 内依赖、无第三方依赖，不预铺通用工具）

## 阶段 3：workflow crate — model 与 parse

- [x] 编写 `crates/core/workflow/Cargo.toml`：依赖 foundation、serde(derive)、serde_json、time(parsing/formatting/serde)；不含任何 Tauri 依赖；`src/lib.rs` 导出 model / parse / queries / artifacts
- [x] 实现 `src/model/mod.rs`、`src/model/inventory.rs`（`enum Inventory { V2, V1, V0 }`）
- [x] 实现 `src/model/workflow.rs`：`Workflow` / `PhaseLog` / `Verdict` / `ChecklistItem` / `FileLogOp` / `FileLogEntry` / `ActivePhase` / `InterruptedEntry`，serde camelCase、未知键忽略、时间戳与可选字段宽松 Option
- [x] 实现 `src/parse/mod.rs` 与 `src/parse/detect.rs`：`detect_inventory` 按 workflow.json 存在性 + `file_log` 键判定代际，不依赖目录命名约定；JSON 整体损坏时按现役结构假设（V2）处理并配合 unparsable 标记
- [x] 实现 `src/parse/workflow_file.rs`：`parse_workflow_file` 两段式宽松解析（serde_json::Value → 逐条 from_value）——单条 eval / file_log 损坏跳过、未知字段忽略、`workflow_type` 缺失或非法时返回 `WorkflowFileParse::Unparsable`、ISO 8601 解析失败降级为 None

## 阶段 4：workflow crate — artifacts 信封与第一波三插件

- [x] 实现 `src/artifacts/mod.rs` 与 `src/artifacts/envelope.rs`：`ArtifactEnvelope { kind, version, title, payload, fallback_text }`、`ArtifactDescriptor { kind, source, title }`、`enum ArtifactCandidate { File, EvalEntry }`
- [x] 实现 `src/artifacts/registry.rs`：`ArtifactPlugin` trait（kind / matches / parse）+ 编译期静态注册表（无 dylib）+ 候选枚举（文件树遍历跳过点前缀项 + eval 条目）+ `discover_artifacts` / `read_artifact`（source 编解码：文件为相对 POSIX 路径、eval 条目为序号串）；同候选多 kind 命中并存、不排他
- [x] 实现 `src/artifacts/tasks_progress.rs`：tasks.md 匹配与 `- [ ]` / `- [x]` 行计数（total / done / pending），version=1
- [x] 实现 `src/artifacts/eval_checklist.rs`：逐条含非空 checklist 的 eval 条目产出信封（phase / attempt / verdict / items），source 为条目序号
- [x] 实现 `src/artifacts/markdown_doc.rs`：收录 change 目录树内全部 .md 文件，v2 现役文件名名单优先 title 标注与排序、legacy 名单其次（无注册表之外的文档探测路径）
- [x] 在 `src/artifacts/mod.rs` 装配注册表：三个插件全部自注册，输出顺序特化 kind（tasks-progress / eval-checklist）先于 markdown-doc，kind 命名均不含 openspec 字样

## 阶段 5：workflow crate — queries

- [x] 实现 `src/queries/mod.rs` 与 `src/queries/list.rs`：`list_changes(layout)` 扫描 changes_root 与 archive_root 全量，逐条 `ChangeSummary`（name / source / inventory / created / unparsable），archive 按 `YYYY-MM-DD-` 目录名前缀按月分组、无前缀入"未知时间"组（排组序列尾）；目录缺失返回空结果不报错
- [x] 实现 `src/queries/detail.rs`：`change_detail(layout, name)` 输出固定 9 站 pipeline（proposal/dev-design/test-design/implement/test-gen/test-execution/code-review/acceptance/code-analyze，attempt 升序）、active_phase、interrupted、file_log 区块（v1 为 None）、`discover_artifacts` 产物清单；未知 change 返回 None
- [x] 自查 queries 纯读：无写入 / 移动 / 修改文件的路径调用，模块无指令（exec）概念

## 阶段 6：desktop-app — Tauri command 双轨

- [x] 实现 `src-tauri/src/commands/mod.rs` 与 `src-tauri/src/commands/queries/mod.rs`：三个 `#[tauri::command]`（`list_changes(root)` / `get_change_detail(root, change)` / `read_artifact(root, change, kind, source)`），无状态薄包装——参数 -> `resolve` -> core 函数 -> DTO 返回，无 State、无直接文件系统访问
- [x] 实现 `src-tauri/src/commands/exec/mod.rs` 预留空轨道：仅模块级注释说明，无任何命令、无 Executor trait 空壳
- [x] 完成 `src-tauri/src/main.rs`：`tauri::Builder` 注册 tauri-plugin-dialog 与三个查询命令（`tauri dev` 冒烟已验证可编译并拉起进程，GUI 视觉走查留待人工完成）

## 阶段 7：前端取数与视图

- [x] 实现 `src/types/dto.ts`：ChangeList / ChangeSummary / ArchiveGroup / ChangeDetail / PhaseEntry / AttemptRecord / ArtifactDescriptor / ArtifactEnvelope（camelCase 对齐 serde DTO；payload 为 unknown 由 renderer 收窄）
- [x] 实现 `src/hooks/useChangeList.ts`：`useChangeList(root)` 返回 ChangeListState（data / loading / error / refresh），仅显式 refresh 触发 `invoke("list_changes")`，无轮询、无 watch
- [x] 实现 `src/hooks/useChangeDetail.ts`：`useChangeDetail(root, change)` 显式 refresh 触发 `invoke("get_change_detail")`，并在同周期内按产物清单逐个 `invoke("read_artifact")` 组装信封数组；单个产物读取失败以 Fallback 形态保留、不阻断其余
- [x] 实现 `src/App.tsx`：workspace 选择（plugin-dialog `open({ directory: true })`）后以选定根驱动两个 hooks；列表 / 详情视图切换；刷新动作覆盖 workspace 级与 change 级
- [x] 实现 `src/views/ChangeListView.tsx`：active 列表 + archive 月份分组（未知时间组置尾）、v2/v1/v0 代际徽标、created 展示、点击进入详情
- [x] 实现 `src/views/ChangeDetailView.tsx`：9 站流水线（每站 attempt 序列、verdict、report、checklist 的 item/pass/evidence 展开、backtrack 标示）、active_phase 运行中标示（attempt + start_at）、v0 纯文档形态、v1 区块留空降级、产物区按信封渲染

## 阶段 8：前端 renderers 与 Fallback

- [x] 实现 `src/renderers/registry.ts`：kind -> 组件注册表与 `resolveRenderer(kind)`（未注册返回 Fallback）；`RendererComponent` 类型
- [x] 实现 `src/renderers/Fallback.tsx`：`fallback_text` 渲染 + kind 徽标，任何未注册 kind / 读取失败均不报错不白屏
- [x] 实现 `src/renderers/ArtifactView.tsx`：单个 `ArtifactEnvelope` 经 `resolveRenderer` 路由渲染
- [x] 实现 `src/renderers/MarkdownDocRenderer.tsx`：payload.markdown 经 react-markdown 渲染
- [x] 实现 `src/renderers/EvalChecklistRenderer.tsx`：items（item / pass / evidence）清单渲染，pass/fail 视觉区分
- [x] 实现 `src/renderers/TasksProgressRenderer.tsx`：total / done / pending 进度渲染
- [x] 在 registry 中注册三个 renderer（kind: `markdown-doc` / `eval-checklist` / `tasks-progress`）

## 阶段 9：边界一致性收口

- [x] 标识符扫描：`packages/desktop` 内 crate 名 / 模块名 / 类型名 / 函数名 / kind 命名无 openspec 子串；磁盘目录名字符串仅存在于 `foundation::layout::resolve` 一处
- [x] 独立包边界复查：仓库根无新增 `package.json` / `pnpm-workspace.yaml`；仓库根 `dist/` 无 desktop 产物；未改动 `plugins/**` 与 `openspec/changes/**` 现有数据
- [x] 纪律复查：`commands/exec/` 为空轨道；前端组件无直接 invoke（仅 hooks 内）；desktop 依赖与代码无文件监听（notify）、无后台轮询、无定时器
- [x] 全链路验证：`cargo check`（src-tauri workspace）与 `pnpm build` 通过；`tauri dev` 手工走查 workspace 选择 -> 列表（代际/分组）-> 详情（流水线/checklist/active_phase）-> 产物渲染（markdown / checklist / tasks 进度）-> 未注册 kind 走 Fallback
  - 完成注记：cargo check/build、pnpm build、`tauri dev` 冒烟（可编译并拉起进程）由 executor 验证通过；GUI 手工走查由用户于 2026-09-20 确认功能验收通过（覆盖列表分区/月度分组、详情流水线、三渲染器、手动刷新、v0 纯文档形态），走查中发现的两个缺陷（GFM 表格渲染、archive 混入进行中列表）已修复并经用户第二轮复核通过
