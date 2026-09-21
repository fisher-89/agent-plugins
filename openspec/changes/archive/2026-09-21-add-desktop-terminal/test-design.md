# 测试设计: add-desktop-terminal

> **日期**: 2026-09-20

---

## 验收范围

<!-- 用模板表格逐条映射 proposal.md 的每个 AC -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | App 提供文件夹选择器；选定后所有取数以该目录为 workspace 根 | 集成测试 | `packages/desktop/src/App.tsx` + `src/hooks/*` + `crates/desktop-app/src/commands/queries/mod.rs`（关系：前端 hooks → Tauri command → core queries DTO） |
| AC-2 | `resolve(root)` 返回 `Layout { changes_root, archive_root, explores_root }`；除该函数外 desktop 源码无硬编码 `openspec` 路径字符串；crate/模块/类型命名无 `openspec` 字样 | 单元测试 | `packages/desktop/src-tauri/crates/core/foundation/src/layout_test.rs` |
| AC-3 | `Inventory` 按规则判定：有 workflow.json 且有 `file_log` → v2；有 workflow.json 无 `file_log` → v1；无 workflow.json → v0；fixtures 三代代表样本判定正确 | 单元测试 | `packages/desktop/src-tauri/crates/core/workflow/src/parse/detect_test.rs` |
| AC-4 | 未知字段忽略；单条 eval / file_log 条目损坏时降级跳过该条，其余数据正常返回，不炸整份记录 | 单元测试 | `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file_test.rs` |
| AC-5 | `list_changes` 返回 active 与 archive 全量，逐条带代际标注；archive 按月分组，日期取目录名前缀，缺失归入"未知时间"组 | 单元测试 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs` |
| AC-6 | `get_change_detail` 返回 9 站流水线，每站含 attempt 序列、verdict、report、checklist，并暴露 active_phase 与 backtrack 字段 | 单元测试 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs` |
| AC-7 | 所有中间产物以信封传递（kind / version / title / payload / fallback_text）；前端按 kind 路由到 renderer，未注册 kind 走 Fallback | 集成测试 | `crates/core/workflow/src/artifacts/registry.rs` + `packages/desktop/src/renderers/ArtifactView.tsx`（关系：detail → artifacts 注册表；前端取数渲染链路） |
| AC-8 | `markdown-doc`、`eval-checklist`、`tasks-progress` 三 kind 均有 matcher + parser（Rust）与 renderer（React）注册；`markdown-doc` 自带按代际的文件名匹配 | 单元测试 | `artifacts/markdown_doc_test.rs` + `artifacts/eval_checklist_test.rs` + `artifacts/tasks_progress_test.rs` + `src/renderers/*.test.tsx` |
| AC-9 | `tasks-progress` 插件统计 tasks.md 中 `- [ ]` / `- [x]` 数量并渲染进度 | 单元测试 | `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress_test.rs` + `src/renderers/TasksProgressRenderer.test.tsx` |
| AC-10 | 未注册 kind → Fallback 渲染 `fallback_text` + kind 徽标；v0 change 纯文档形态；v1 缺 `file_log` 等字段时对应区块降级留空，不报错、不白屏 | 集成测试 | `src/renderers/Fallback.tsx` + `src/views/ChangeDetailView.tsx`（关系：前端取数渲染链路降级兜底） |
| AC-11 | `commands/queries/` 实现三个命令且均为无状态薄包装（参数 → core → DTO）；`commands/exec/` 轨道存在但为空；workspace 状态只经 core 函数访问 | 单元测试 | `packages/desktop/src-tauri/crates/desktop-app/src/commands/queries/mod_test.rs`（exec 空轨为结构性验收，见不可测试项） |
| AC-12 | 前端经 `useChangeList` / `useChangeDetail` 显式刷新触发 invoke；无文件 watch、无后台轮询 | 单元测试 | `packages/desktop/src/hooks/useChangeList.test.ts` + `src/hooks/useChangeDetail.test.ts` |
| AC-13 | `packages/desktop` 自带 lockfile 与构建脚本；根目录无新增 workspace 配置；`dist/` 不含 desktop 产物 | 不可测试项 | 结构性/仓库卫生验收，见「不可测试项」第 1 条 |
| AC-14 | 存在全量解析 fixtures 语料并与 golden 对比的测试；fixtures 覆盖三代代表样本（从 archive 挑选入仓） | 集成测试 | `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/`（关系：fixtures 全量语料 → golden 快照） |

**测试基础设施决策（proposal / design 留待本阶段定，结论如下）**：

- Rust 三 crate：cargo 内建 `#[test]` 框架。单元测试文件按 `test_resolve_paths` 输出为源文件同目录的 `<module>_test.rs` 兄弟文件，经父模块 `#[cfg(test)] mod <module>_test;` 挂载并 `use super::` / `use crate::` 导入 crate 公共 API；不采用 dist 模板的 `#[path]` 引入方式（跨模块 `crate::` 引用下会重复编译并破坏内部路径解析）。集成测试放 cargo 规范测试目录 `crates/core/workflow/tests/`（proposal 已指定该测试区域，替代 JS 惯例的 `__tests__/`）。
- 前端：`packages/desktop` 为独立包，根工具链（vite-plus）不适用；在包内引入 `vitest` + `@testing-library/react` + `jsdom` 作为 devDependencies，测试文件与源文件同目录（`test_resolve_paths` 输出），集成测试放 `packages/desktop/src/__tests__/`；需在 `openspec/config.json` 的 `tests[]` 新增 `packages/desktop`（framework: vitest）套件项供 test-execution 执行。
- 覆盖纪律：不复测库自带语义（serde 反序列化机制本身、react-markdown 的 markdown 解析、正则语法），只测自研提取 / 组装 / 降级层。类型定义文件（`model/`、`artifacts/envelope.rs`）无独立行为逻辑，其字段合法性经 parse / queries / artifacts 各测试间接覆盖，不设独立用例章节。

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test]、前端 Vitest 纯函数/组件 mock 测试） -->

### packages/desktop/src-tauri/crates/core/foundation/src/layout.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout_test.rs

#### 待测功能

- resolve(root: &Path) -> Layout: 纯路径推导出 changes_root / archive_root / explores_root 三棵目录树，无 IO、对不存在的 root 正常返回；全包唯一磁盘目录名触点
- Layout: 三字段路径结构（changes_root / archive_root / explores_root）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| layout::resolve | 正向 | 对存在的 root 返回三个子路径，均以 root 为前缀且指向 changes 子树 / archive 子树 / explores 子树 | 新增 |
| layout::resolve | 正向 | 对不存在的 root 正常返回（纯推导不校验磁盘、不报错） | 新增 |
| layout::resolve | 异常 | root 指向一个文件而非目录时仍正常返回拼接路径（无目录类型校验） | 新增 |
| layout::resolve | 边界 | root 为空路径（`Path::new("")`）返回相对形式的三路径，不 panic | 新增 |
| layout::resolve | 边界 | root 带尾部分隔符 / 重复分隔符时，结果不产生双分隔符 | 新增 |
| layout::resolve | 正向 | 源码命名隔离扫描：遍历 `packages/desktop/src` 与 `crates/*/src` 全部源文件（排除 `foundation/src/layout.rs` 与 `tests/fixtures/**` 测试数据），断言不含 `openspec` 子串 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：resolve 为纯函数直接传 `Path`；命名隔离扫描经 `CARGO_MANIFEST_DIR` 定位包内真实源码文件读取 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/parse/detect.rs -> packages/desktop/src-tauri/crates/core/workflow/src/parse/detect_test.rs

#### 待测功能

- detect_inventory(change_dir: &Path) -> Inventory: 基于磁盘事实的代际判定（workflow.json 存在性 + `file_log` 键），不依赖目录命名约定

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parse::detect_inventory | 正向 | 有 workflow.json 且含 `file_log` 键 → V2 | 新增 |
| parse::detect_inventory | 正向 | 有 workflow.json 无 `file_log` 键 → V1 | 新增 |
| parse::detect_inventory | 正向 | 无 workflow.json → V0 | 新增 |
| parse::detect_inventory | 边界 | workflow.json 存在但 `file_log` 为空数组 → 仍 V2（键存在即 v2，与 v1 以键而非非空区分） | 新增 |
| parse::detect_inventory | 边界 | workflow.json 内容为非法 JSON（键不可读）→ 按 V1 处理 | 新增 |
| parse::detect_inventory | 边界 | 目录含遗留 `eval.json` 而无 workflow.json → 仍 V0（不受 legacy 文件干扰，fixture v0-a 验证） | 新增 |
| parse::detect_inventory | 边界 | 目录名不含 `YYYY-MM-DD-` 前缀 → 判定结果不受影响（不依赖命名约定） | 新增 |
| parse::detect_inventory | 异常 | change_dir 不存在 → V0 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：`std::env::temp_dir` 下构造临时 change 目录写入探测用文件，测试结束清理；真实代际样本复用 `tests/fixtures/` 语料 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs -> packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file_test.rs

#### 待测功能

- parse_workflow_file(path: &Path) -> WorkflowFileParse: 两段式宽松解析（先 Value 后逐条 `from_value`），单条 eval / file_log 损坏跳过，核心字段非法整体 Unparsable，ISO 8601 时间戳失败降级 None
- WorkflowFileParse: `Parsed(Workflow) | Unparsable { reason }` 整体降级标记

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parse::parse_workflow_file | 正向 | 合法 v2 全字段 workflow.json → `Parsed`，eval / active_phase / file_log / interrupted 全部还原为强类型 | 新增 |
| parse::parse_workflow_file | 正向 | 含未知顶层键（legacy `files{}` 桶、`source` 旁挂）→ 键被忽略，其余字段正常解析（fixture v1-c 验证） | 新增 |
| parse::parse_workflow_file | 异常 | 文件整体非法 JSON → `Unparsable { reason }` | 新增 |
| parse::parse_workflow_file | 异常 | 核心 workflow_type 字段缺失或类型非法 → 整体 `Unparsable`，不返回半份数据 | 新增 |
| parse::parse_workflow_file | 异常 | 单条 eval 条目损坏（checklist 非数组）→ 该条跳过，其余条目完整保留 | 新增 |
| parse::parse_workflow_file | 异常 | 单条 eval 的 verdict 为非法枚举值 → 该条降级跳过，不炸整份 | 新增 |
| parse::parse_workflow_file | 异常 | 单条 file_log 条目 op 为非法枚举值 → 该条跳过，其余 file_log 条目保留 | 新增 |
| parse::parse_workflow_file | 边界 | eval 为空数组 → `Parsed` 且 eval 为空 Vec | 新增 |
| parse::parse_workflow_file | 边界 | file_log 为空数组 → `Some(空 Vec)`，与 v1 的 `None` 严格区分（v1/v2 判定的数据基础） | 新增 |
| parse::parse_workflow_file | 边界 | 条目时间戳为非法 ISO 8601 字符串 → 该字段降级 None，条目本身保留 | 新增 |
| parse::parse_workflow_file | 边界 | attempt 缺失 → None；attempt 为负数 → 该条按损坏降级跳过 | 新增 |
| parse::parse_workflow_file | 边界 | report 为超长字符串（>1000 字符）与含 `\n` / emoji 的特殊字符 → 原样保留 | 新增 |
| parse::parse_workflow_file | 边界 | checklist 条目缺失 evidence 字段 → 按空串兜底（Option 宽松），条目保留 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：临时目录写入构造用 workflow.json + 复用 `tests/fixtures/` 静态语料；测试结束清理临时文件 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs

#### 待测功能

- list_changes(layout: &Layout) -> ChangeList: 扫描 changes_root + archive_root 全量，代际标注，archive 按月分组（未知时间组置尾）；目录缺失返回空而非报错
- ChangeSummary: name / source / inventory / created / unparsable
- ChangeSource: Active / Archive
- ArchiveGroup: month（None 即"未知时间"组，固定排序列尾）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| queries::list_changes | 正向 | active 与 archive 全量返回，逐条 inventory 标注与磁盘事实一致 | 新增 |
| queries::list_changes | 正向 | archive 目录按 `YYYY-MM-DD-` 前缀折叠到月组（如 2025-06、2026-09），组内与组间排序稳定 | 新增 |
| queries::list_changes | 正向 | created 优先取 workflow.json 的 created，archive 无 created 时回退目录名日期前缀（fixture v1-a：workflow.json 无 created → 取 2026-07） | 新增 |
| queries::list_changes | 异常 | changes_root / archive_root 目录缺失 → 返回空结果而非报错 | 新增 |
| queries::list_changes | 异常 | workflow.json Unparsable 的 change → `unparsable=true` 标注且仍入列，不中断扫描 | 新增 |
| queries::list_changes | 边界 | 无日期前缀的 archive 目录名 → 归入 month=None"未知时间"组，固定排组序列尾 | 新增 |
| queries::list_changes | 边界 | changes_root 存在但为空 → 空 active + 空 archive_groups | 新增 |
| queries::list_changes | 边界 | 目录树下混入普通文件（非目录）→ 被忽略不报错 | 新增 |
| queries::list_changes | 边界 | 单月单元素分组（最小分组形态）正确成组 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：`resolve(临时 workspace 根)` 供给 Layout，目录树在临时目录搭建；真实样本复用 `tests/fixtures/` | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- change_detail(layout: &Layout, name: &str) -> Option<ChangeDetail>: 固定 9 站流水线聚合 eval attempts（attempt 升序），暴露 active_phase / interrupted / file_log 区块与产物清单；纯读
- PhaseEntry: phase / attempts（固定 9 站全量输出，顺序固定不依赖 eval 排列）
- AttemptRecord: attempt / verdict / report / checklist / skipped / stale / start_at / timestamp / backtrack_to / backtrack_reason

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| queries::change_detail | 正向 | 同 phase 多 attempt 的 eval 条目折叠为单站，attempts 按 attempt 升序排列 | 新增 |
| queries::change_detail | 正向 | 每站 AttemptRecord 携带 verdict / report / checklist（item / pass / evidence）全量字段 | 新增 |
| queries::change_detail | 正向 | active_phase 字段与 eval 条目的 backtrack_to / backtrack_reason 随条目暴露（fixture v1-b 验证） | 新增 |
| queries::change_detail | 正向 | v2 change 的 file_log 区块返回条目列表（op / scope / path / at） | 新增 |
| queries::change_detail | 异常 | 未知 change 名 → `None` | 新增 |
| queries::change_detail | 边界 | v0 change（无 workflow.json）→ 9 站全空序列 + inventory=V0，不报错 | 新增 |
| queries::change_detail | 边界 | v1 change 缺 file_log 键 → file_log 区块为 None（留空），其余区块正常 | 新增 |
| queries::change_detail | 边界 | eval 未覆盖的 phase → 该站仍在 9 站列表中且 attempts 为空序列（顺序固定） | 新增 |
| queries::change_detail | 边界 | 无 attempt 字段的 eval 条目 → 归入对应站且 attempt=None，不影响其他条目排序 | 新增 |
| queries::change_detail | 边界 | skipped / stale 标记位为 true 的条目 → 标记随 AttemptRecord 透出 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：临时 workspace + `tests/fixtures/` 语料供给真实 change 目录 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry_test.rs

#### 待测功能

- ArtifactPlugin（trait）: kind() / matches(&ArtifactInput) / parse(&ArtifactInput) -> Option<ArtifactEnvelope>，编译期静态注册表自注册
- discover_artifacts(change_dir, inventory, workflow) -> Vec<ArtifactDescriptor>: 枚举候选（文件树 + eval 条目）喂注册表，收集全部命中
- read_artifact(change_dir, inventory, workflow, kind, source) -> Option<ArtifactEnvelope>: 按 kind 定位插件、按 source 重建候选后解析信封
- ArtifactCandidate: File { relative_path } / EvalEntry { index }

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| artifacts::registry::discover_artifacts | 正向 | 文件树候选与 eval 候选喂入后，收集全部命中为 Descriptor 清单（kind / source / title 齐全） | 新增 |
| artifacts::registry::discover_artifacts | 正向 | 同一候选多 kind 命中并存：tasks.md 同时产出 `markdown-doc` 与 `tasks-progress` 两个 Descriptor（无排他） | 新增 |
| artifacts::registry::discover_artifacts | 异常 | 无任何插件命中 → 空 Descriptor 清单 | 新增 |
| artifacts::registry::discover_artifacts | 边界 | change 目录为空或不存在 → 空清单不报错 | 新增 |
| artifacts::registry::read_artifact | 正向 | 已注册 kind + 有效 source → 返回信封，kind / version / title / payload / fallback_text 五字段齐全 | 新增 |
| artifacts::registry::read_artifact | 异常 | 未注册的 kind → `None` | 新增 |
| artifacts::registry::read_artifact | 异常 | source 指向不存在的文件 / 越界 eval 序号 → `None` | 新增 |
| artifacts::registry::read_artifact | 边界 | version 从 1 起；EvalEntry 候选的 source 为条目序号串（与 Descriptor 寻址一致，read 可按 discover 的 source 回放） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：临时目录搭建含 proposal.md / tasks.md / reports 子树的 change 目录 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/markdown_doc.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/markdown_doc_test.rs

#### 待测功能

- markdown-doc 插件: kind 常量 `markdown-doc`；目录树内 .md 收录 matches；代际名单决定 title 标注与排序的 parse

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| artifacts::markdown_doc | 正向 | proposal.md 等目录树内 .md 命中，payload.markdown 为文件原文，fallback_text 同原文 | 新增 |
| artifacts::markdown_doc | 正向 | 代际名单命中 legacy 文件名（如 gan-design.md、test-reports/**）时 title 标注与排序生效，无名单外并行 Docs 探测路径 | 新增 |
| artifacts::markdown_doc | 异常 | 非 .md 文件（json / ts）不命中 matches | 新增 |
| artifacts::markdown_doc | 边界 | 空 .md 文件 → 命中且 markdown 为空串 | 新增 |
| artifacts::markdown_doc | 边界 | 超长 markdown（>1000 行）内容完整保留不截断 | 新增 |
| artifacts::markdown_doc | 边界 | reports/ 等子目录内 .md 一并收录（目录树全量而非仅顶层） | 新增 |
| artifacts::markdown_doc | 边界 | 文件名含中文 / 空格 / 特殊字符 → 命中且 source 为 change 内相对 POSIX 路径 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：临时目录写入构造用 markdown 文件 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist_test.rs

#### 待测功能

- eval-checklist 插件: 逐 eval 条目（含非空 checklist）产出 checklist 信封，source 为条目序号

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| artifacts::eval_checklist | 正向 | 每个含非空 checklist 的 eval 条目产出一个信封，payload 含 phase / attempt / verdict / items 全量 | 新增 |
| artifacts::eval_checklist | 正向 | fallback_text 为 checklist 文本化清单（降级保底非空） | 新增 |
| artifacts::eval_checklist | 异常 | checklist 为空的 eval 条目不产出信封 | 新增 |
| artifacts::eval_checklist | 边界 | 无 workflow（v0，workflow=None）→ 不产出任何信封 | 新增 |
| artifacts::eval_checklist | 边界 | 大量条目（12 条 eval 全含 checklist）逐条产出，source 序号与条目一一对应 | 新增 |
| artifacts::eval_checklist | 边界 | evidence 长文本与含特殊字符的 item 名原样保留 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：以内存构造的 Workflow 模型经 ArtifactInput 注入，无需磁盘（eval 候选不读文件） | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress_test.rs

#### 待测功能

- tasks-progress 插件: tasks.md 中 `- [ ]` / `- [x]` 行计数，payload 为 total / done / pending

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| artifacts::tasks_progress | 正向 | 混合勾选 tasks.md → total / done / pending 计数正确且三者自洽（done + pending = total） | 新增 |
| artifacts::tasks_progress | 正向 | fallback_text 为统计结果文本（如 "3/5"）非空 | 新增 |
| artifacts::tasks_progress | 异常 | tasks.md 不存在 → matches 不命中，不产出信封 | 新增 |
| artifacts::tasks_progress | 边界 | 空 tasks.md → total=0 / done=0 / pending=0 仍产出信封 | 新增 |
| artifacts::tasks_progress | 边界 | 全部勾选与全部未勾两个极端 → pending=0 / done=0 | 新增 |
| artifacts::tasks_progress | 边界 | 缩进的嵌套复选框行（`  - [x]`）同样计数；普通 `- item` 行与正文中的 `[x]` 字样不计入 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：临时目录写入构造用 tasks.md | 全部用例 |

### packages/desktop/src-tauri/crates/desktop-app/src/commands/queries/mod.rs -> packages/desktop/src-tauri/crates/desktop-app/src/commands/queries/mod_test.rs

#### 待测功能

- list_changes(root: String) -> ChangeList（#[tauri::command]）: 薄包装 root → resolve → core list_changes → DTO
- get_change_detail(root: String, change: String) -> Option<ChangeDetail>（#[tauri::command]）: 未知 change 名返回 None
- read_artifact(root: String, change: String, kind: String, source: String) -> Option<ArtifactEnvelope>（#[tauri::command]）: 单产物信封读取

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| commands::queries::list_changes | 正向 | 传入临时 workspace 根，返回结果与直接调用 core `list_changes(resolve(root))` 一致（纯透传无加工） | 新增 |
| commands::queries::get_change_detail | 正向 | 已知 change 名返回完整详情 DTO | 新增 |
| commands::queries::get_change_detail | 异常 | 未知 change 名返回 None（非错误） | 新增 |
| commands::queries::read_artifact | 异常 | 未注册 kind / 不存在 source 返回 None | 新增 |
| commands::queries::list_changes | 边界 | root 为空字符串 → 经 resolve 空路径得空列表，不 panic | 新增 |
| commands::queries::list_changes | 边界 | 命令函数无 State 参数、无文件系统直访（经 core 函数访问，workspace 状态只走 core） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 无 Mock 框架：临时 workspace 目录真实调用命令函数（#[tauri::command] 保留原函数可直调，不启动 Tauri runtime） | 全部用例 |

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

#### 待测功能

- App: 应用壳组件 — workspace 选择（plugin-dialog `open({ directory: true })`）、列表 / 详情视图切换、刷新动作下发、以 root 驱动子组件

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App | 正向 | 未选 workspace 时显示选择入口；dialog 返回路径后以该 root 进入列表视图 | 新增 |
| App | 正向 | 列表项点击后切换到详情视图，root 与 change 名下发正确 | 新增 |
| App | 正向 | 刷新动作触发 useChangeList 的 refresh 回调 | 新增 |
| App | 边界 | dialog 取消（返回 null）→ 保持未选 workspace 状态，不发起取数 | 新增 |
| App | 异常 | dialog 调用 reject → 界面保持可选状态不白屏 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/plugin-dialog | vi.mock 模块，`open` 按用例返回目录路径 / null / reject | App 正向与 dialog 相关用例 |
| @tauri-apps/api/core | vi.mock `invoke` 返回 fixture ChangeList DTO | App 内视图取数用例 |

### packages/desktop/src/hooks/useChangeList.ts -> packages/desktop/src/hooks/useChangeList.test.ts

#### 待测功能

- useChangeList(root: string | null): ChangeListState — data（ChangeList 或 null）/ loading / error / refresh 回调；显式 refresh 触发 invoke("list_changes")

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useChangeList | 正向 | refresh 调用触发 invoke("list_changes", { root })，成功后 data 更新为 DTO | 新增 |
| useChangeList | 正向 | root 变更后再 refresh，invoke 参数携带新 root | 新增 |
| useChangeList | 异常 | invoke reject → error 置位、data 保持原值、不抛未捕获异常 | 新增 |
| useChangeList | 边界 | root 为 null → 不发起 invoke，data 为 null | 新增 |
| useChangeList | 边界 | 连续两次 refresh → 最终状态以后一次结果为准；断言除显式刷新外无任何定时器 / 轮询调用（AC-12） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core | vi.mock `invoke`，按命令名分发返回 fixture ChangeList 或 reject | 全部用例 |

### packages/desktop/src/hooks/useChangeDetail.ts -> packages/desktop/src/hooks/useChangeDetail.test.ts

#### 待测功能

- useChangeDetail(root: string | null, change: string | null): ChangeDetailState — detail / artifacts（ArtifactEnvelope[]）/ loading / error / refresh；refresh 同周期内先取详情再逐产物 invoke("read_artifact") 组装

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useChangeDetail | 正向 | refresh 触发 invoke("get_change_detail", { root, change })，detail 更新 | 新增 |
| useChangeDetail | 正向 | 详情返回产物清单后，同一刷新周期内逐个 invoke("read_artifact") 并把信封组装进 artifacts（调用次数与 Descriptor 数一致，source 一致） | 新增 |
| useChangeDetail | 异常 | 单个 read_artifact reject → 该产物以 Fallback 形态信封呈现，其余产物正常组装，不阻断 | 新增 |
| useChangeDetail | 异常 | get_change_detail 返回 null → detail 为 null 且不发起任何 read_artifact | 新增 |
| useChangeDetail | 边界 | change 为 null → 不发起任何 invoke | 新增 |
| useChangeDetail | 边界 | 产物清单为空数组 → artifacts 为空数组，无额外 invoke | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core | vi.mock `invoke`，按命令名（get_change_detail / read_artifact）分发返回 fixture DTO / null / reject | 全部用例 |

### packages/desktop/src/renderers/registry.ts -> packages/desktop/src/renderers/registry.test.ts

#### 待测功能

- resolveRenderer(kind: string): RendererComponent — kind 到组件映射，未注册返回 Fallback
- RendererComponent: 信封渲染组件类型（入参单个 ArtifactEnvelope）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| renderers::resolveRenderer | 正向 | `markdown-doc` / `eval-checklist` / `tasks-progress` 三 kind 分别返回对应 renderer 组件 | 新增 |
| renderers::resolveRenderer | 异常 | 未注册 kind（如 `file-log`）返回 Fallback 组件 | 新增 |
| renderers::resolveRenderer | 边界 | 空字符串 kind 与大小写变体（如 `Markdown-Doc`）→ 精确匹配语义下返回 Fallback | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯映射函数，无外部依赖，无需 Mock | — |

### packages/desktop/src/renderers/ArtifactView.tsx -> packages/desktop/src/renderers/ArtifactView.test.tsx

#### 待测功能

- ArtifactView: 信封路由组件 — 按 resolveRenderer(kind) 渲染单个 ArtifactEnvelope

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ArtifactView | 正向 | 已注册 kind 信封路由到专属 renderer 并渲染 payload 内容 | 新增 |
| ArtifactView | 正向 | 未注册 kind 信封走 Fallback（fallback_text + kind 徽标可见） | 新增 |
| ArtifactView | 边界 | fallback_text 为 null 的未注册 kind 信封 → Fallback 仍渲染 kind 徽标，不空白 | 新增 |
| ArtifactView | 异常 | payload 形状与 renderer 期望不符（字段缺失 / 类型漂移）→ 不抛错，呈现降级形态（永不白屏） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 组件树内自足（registry 为真实实现），仅以内存构造信封对象 | 全部用例 |

### packages/desktop/src/renderers/Fallback.tsx -> packages/desktop/src/renderers/Fallback.test.tsx

#### 待测功能

- Fallback: 未注册 kind 兜底组件 — fallback_text 渲染 + kind 徽标（硬要求，永不白屏）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| Fallback | 正向 | 渲染 fallback_text 文本与 kind 徽标 | 新增 |
| Fallback | 边界 | fallback_text 为 null → 无文本区域但仍渲染 kind 徽标 | 新增 |
| Fallback | 边界 | 超长 fallback_text（>1000 字符）完整渲染不崩 | 新增 |
| Fallback | 异常 | envelope 缺 kind 字段 → 不抛错，渲染兜底形态 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯展示组件，内存构造 props | 全部用例 |

### packages/desktop/src/renderers/MarkdownDocRenderer.tsx -> packages/desktop/src/renderers/MarkdownDocRenderer.test.tsx

#### 待测功能

- MarkdownDocRenderer: payload.markdown 经 react-markdown 渲染（覆盖自研 payload 收窄与组装，不复测 react-markdown 的 markdown 解析语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| MarkdownDocRenderer | 正向 | payload.markdown 为字符串时传给 react-markdown 并渲染出内容节点 | 新增 |
| MarkdownDocRenderer | 异常 | payload 缺 markdown 字段 / 类型漂移 → 不抛错，渲染空态或 fallback 形态 | 新增 |
| MarkdownDocRenderer | 边界 | markdown 为空串 → 渲染空容器不崩 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | react-markdown 为真实依赖（其渲染语义不属被测对象），内存构造信封 | 全部用例 |

### packages/desktop/src/renderers/EvalChecklistRenderer.tsx -> packages/desktop/src/renderers/EvalChecklistRenderer.test.tsx

#### 待测功能

- EvalChecklistRenderer: items（item / pass / evidence）清单渲染

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| EvalChecklistRenderer | 正向 | items 渲染为清单，每条含 item 名与 evidence | 新增 |
| EvalChecklistRenderer | 正向 | pass true / false 条目有可区分的视觉标识 | 新增 |
| EvalChecklistRenderer | 边界 | items 为空数组 → 渲染空清单不崩 | 新增 |
| EvalChecklistRenderer | 边界 | attempt 为 null → 显示空缺占位而非 NaN / undefined 字样 | 新增 |
| EvalChecklistRenderer | 异常 | payload 缺 items 字段 → 不抛错 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯展示组件，内存构造信封 payload | 全部用例 |

### packages/desktop/src/renderers/TasksProgressRenderer.tsx -> packages/desktop/src/renderers/TasksProgressRenderer.test.tsx

#### 待测功能

- TasksProgressRenderer: total / done / pending 进度渲染

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| TasksProgressRenderer | 正向 | total / done / pending 渲染为进度（比例与计数一致） | 新增 |
| TasksProgressRenderer | 边界 | total=0 → 不产生除零 NaN，显示空进度 | 新增 |
| TasksProgressRenderer | 边界 | done=total 全勾 → 100% 形态 | 新增 |
| TasksProgressRenderer | 异常 | payload 缺字段（undefined 计数）→ 不抛错 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯展示组件，内存构造信封 payload | 全部用例 |

### packages/desktop/src/views/ChangeListView.tsx -> packages/desktop/src/views/ChangeListView.test.tsx

#### 待测功能

- ChangeListView: change 列表视图 — active 列表 + archive 按月分组（"未知时间"组置尾）、代际徽标（v2/v1/v0）、点击进入详情

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeListView | 正向 | active 列表逐条渲染并带正确代际徽标（v2 / v1 / v0） | 新增 |
| ChangeListView | 正向 | archive 按月分组渲染，month=None 的"未知时间"组渲染在序列尾 | 新增 |
| ChangeListView | 正向 | 点击 change 条目触发进入详情回调并携带 change 名 | 新增 |
| ChangeListView | 异常 | error 状态渲染错误提示，不白屏 | 新增 |
| ChangeListView | 边界 | loading 态与空数据（active 空 + 分组空）渲染空态 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯展示组件：以 fixture ChangeList DTO 构造 props；取数已在 hooks 层被测，此处注入状态 | 全部用例 |

### packages/desktop/src/views/ChangeDetailView.tsx -> packages/desktop/src/views/ChangeDetailView.test.tsx

#### 待测功能

- ChangeDetailView: change 详情视图 — 9 站流水线（attempt 序列 / verdict / checklist 展开）、active_phase 运行中标示、v0 纯文档形态、v1 区块留空降级、产物区（ArtifactView 列表）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeDetailView | 正向 | 9 站流水线渲染 attempt 序列 / verdict / checklist 展开内容 | 新增 |
| ChangeDetailView | 正向 | active_phase 存在时渲染运行中标示；backtrack_to / backtrack_reason 随条目展示 | 新增 |
| ChangeDetailView | 正向 | v0 change（空流水线 + 纯文档产物清单）以纯文档形态呈现 | 新增 |
| ChangeDetailView | 边界 | v1 change file_log 区块为 None → 对应区块留空降级，不报错不白屏 | 新增 |
| ChangeDetailView | 边界 | 产物区按信封顺序渲染 ArtifactView 列表 | 新增 |
| ChangeDetailView | 异常 | detail 为 null / error 态 → 空态与错误提示渲染，不白屏 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯展示组件：以 fixture ChangeDetail DTO + ArtifactEnvelope[] 构造 props | 全部用例 |

---

## 集成测试

<!-- 集成测试验证跨模块交互。Rust 侧集成测试文件位于 cargo 测试区域 crates/core/workflow/tests/（proposal 指定），前端侧位于 packages/desktop/src/__tests__/ -->

### foundation::layout::resolve → workflow::queries（Layout 驱动的目录扫描） → `packages/desktop/src-tauri/crates/core/workflow/tests/layout_queries_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs` | 路径供给方（resolve 产出 Layout） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | 消费方（扫描 changes_root / archive_root） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 消费方（按 Layout 定位单 change 目录） |

**关联AC**: AC-2, AC-5, AC-6

**关系描述**:

queries 的全部扫描都以 foundation 产出的 `Layout` 为唯一路径来源，二者之间的契约是"resolve 推导的目录名与磁盘真实布局一致"。这条缝一旦错位（例如 archive 子目录归属变化、目录名拼错），所有查询会静默返回空或指向错误目录，且单元测试各自 mock 掉 Layout 时无法暴露。集成测试在真实形状的 workspace 目录树上跑通 resolve → list_changes / change_detail 全链路，并覆盖路径输入异常时的组合行为，是 AC-2 与 AC-5 / AC-6 之间的缝合验证。

#### 场景: 真实形状 workspace 的 resolve 到查询全链路

前置条件：临时目录搭出 `<root>/<changes>/`、`<root>/<changes>/archive/`（含带日期前缀的 archive 子目录）、`<root>/<explores>/`。输入为该 root。预期：resolve 三路径正确；list_changes 返回 active 与 archive 全量且月分组正确；change_detail 对 active 与 archive 中的 change 均能定位并返回详情。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | resolve(临时 workspace) 后 list_changes 全量返回，active/archive 归属与月分组和磁盘一致 | 新增 |
| 正向 | change_detail 命中 active change 与 archive change 各一（archive 经 archive_root 定位） | 新增 |
| 边界 | workspace 缺 explores 子目录 → resolve 正常，查询不受影响 | 新增 |

#### 场景: 路径输入异常的组合行为

前置条件：构造 root 指向文件、change 名含路径分隔符或 `..` 等异常输入。预期：查询层返回空 / None 而非报错或越出 changes 目录（纯读不逃逸）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | root 指向一个文件（非目录）→ list_changes 返回空，不 panic | 新增 |
| 边界 | change_detail 的 name 含 `/` 或 `..` → 返回 None，不逃逸出 changes_root | 新增 |
| 边界 | changes 子目录整体缺失 → list_changes 空结果 | 新增 |

### 磁盘语料 → detect_inventory → parse_workflow_file → Workflow（代际判定与宽松解析协同） → `packages/desktop/src-tauri/crates/core/workflow/tests/generation_parse_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/detect.rs` | 判定方（磁盘事实 → Inventory） |
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs` | 解析方（workflow.json → Workflow / Unparsable） |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | 数据承载方（宽松字段模型） |

**关联AC**: AC-3, AC-4

**关系描述**:

代际判定与解析是两个独立函数，但对外语义必须协同：detect 说"这是 v2"，解析就必须还原出 file_log；detect 说 v1，file_log 必须是 None。二者对"file_log 键存在但为空数组"这类临界事实的口径不一致时，会出现 v2 标注配 v1 数据的组合错误，且两个单元各自为政时测不出来。集成测试以 fixtures 三代真实样本与合成损坏样本贯通判定→解析→模型，把临界口径钉死。

#### 场景: 三代 fixture 的判定与解析贯通

前置条件：`tests/fixtures/` 语料入仓。输入为逐个 fixture change 目录。预期：v2 样本判定 V2 且解析出 file_log；v1 样本判定 V1 且 file_log=None；v0 样本判定 V0 且无 workflow 可解析；未知键（legacy files{} / source）被忽略。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | v2-a / v2-b fixture：detect=V2 且 parse_workflow_file 还原 file_log / active_phase / eval | 新增 |
| 正向 | v1-a / v1-b / v1-c fixture：detect=V1 且 file_log=None、eval 还原、未知键忽略 | 新增 |
| 正向 | v0-a / v0-b fixture：detect=V0（v0-a 含遗留 eval.json 仍判 V0） | 新增 |
| 边界 | file_log 为空数组的 v2 样本：detect=V2 且 parse 出 `Some(空)`，与 v1 的 None 严格区分 | 新增 |

#### 场景: 损坏条目降级的组合行为

前置条件：合成损坏样本 fixture（单条损坏 / 整体非法）。预期：整体非法 JSON → Unparsable；单条损坏 → 降级跳过且其余数据完整；降级不改变 Inventory 判定。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 整体非法 JSON 的 fixture → Unparsable 且 detect 仍按文件存在性判定代际 | 新增 |
| 异常 | 单条 eval / file_log 损坏的 fixture → 该条跳过、其余条目与区块完整返回 | 新增 |
| 边界 | 非法时间戳与非法 verdict 同时出现的条目 → 降级路径叠加后条目仍被正确跳过一次（不重复计数） | 新增 |

### workflow::queries::detail → artifacts::registry（产物清单与信封读取） → `packages/desktop/src-tauri/crates/core/workflow/tests/artifacts_envelope_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 触发方（详情附产物 Descriptor 清单） |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs` | 中间件（候选枚举 + 注册表分发） |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/markdown_doc.rs` | 插件方（markdown-doc 匹配与解析） |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist.rs` | 插件方（eval-checklist 匹配与解析） |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress.rs` | 插件方（tasks-progress 匹配与解析） |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/envelope.rs` | 契约承载方（ArtifactEnvelope / Descriptor / Candidate） |

**关联AC**: AC-6, AC-7, AC-8, AC-9, AC-10

**关系描述**:

ChangeDetail.artifacts 是前端产物区的寻址依据：detail 给出的 Descriptor (kind, source) 必须能被 read_artifact 按 (kind, source) 精确回放成信封，二者经注册表与三个插件实例缝合。这条缝的典型错法是 discover 时的候选构造与 read 时的候选重建不一致（source 编码漂移）、或同候选多 kind 并存时清单与读取数量对不上。集成测试以真实 fixture change 目录跑通"详情清单 → 逐信封读取"，同时验证信封契约五字段与前端 Fallback 所依赖的"读取失败返回 None"上游语义。

#### 场景: v2 change 的产物清单与逐信封读取

前置条件：v2-b fixture（含 proposal.md / tasks.md / eval 条目）。预期：change_detail.artifacts 与 discover_artifacts 一致；逐 Descriptor 调 read_artifact 均返回五字段齐全的信封；tasks.md 同时产出 markdown-doc 与 tasks-progress 两个 Descriptor 且都能读出。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | detail.artifacts 与 discover_artifacts 输出一致（kind / source / title 逐项相等） | 新增 |
| 正向 | 三个第一波 kind 的信封均产出，payload 契约形状符合设计表（version=1） | 新增 |
| 正向 | tasks.md 双 kind 并存：两个 Descriptor 各自 read 出正确信封（tasks-progress 计数与文件一致） | 新增 |

#### 场景: v0 纯文档形态的产物链路

前置条件：v0-a fixture（无 workflow.json，含 markdown 与 reports/ 子目录）。预期：产物清单即文档清单，仅 markdown-doc 命中，eval-checklist 无产出。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | v0 change 的 detail.artifacts 等于目录树内全部 .md 的 markdown-doc Descriptor | 新增 |
| 边界 | v0 下 read_artifact 请求 eval-checklist kind → None（无 workflow 数据源） | 新增 |

#### 场景: 读取失败路径与兜底上游契约

前置条件：对清单外 / 不存在的 (kind, source) 发起读取。预期：未注册 kind 与无效 source 均返回 None——这是前端"读取失败以 Fallback 呈现、不阻断其余产物"（AC-10）所依赖的上游语义。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 未注册 kind（如 `file-log`）→ read_artifact 返回 None | 新增 |
| 异常 | source 指向已被删除的文件 → 返回 None 而非报错 | 新增 |

### fixtures 全量语料 → 三代解析投影 → golden 快照（防 TS schema 漂移防线） → `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/` | 语料供给方（三代代表样本入仓快照） |
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/` | 解析方（detect + workflow_file） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/` | 投影方（list / detail 聚合） |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/` | 投影方（产物清单与信封） |

**关联AC**: AC-14, AC-3, AC-4, AC-5, AC-6

**关系描述**:

TS 侧 zod schema 是唯一真理源，Rust 解析靠 fixture 语料快照回归兜底漂移：把三代代表 change 目录整体入仓为 fixtures，对每个样本跑"判定 → 解析 → 列表 / 详情投影 → 产物清单"的规范化 JSON 投影，与入仓 golden 文件全量对比。golden 机制决策（design 留待本阶段）：采用手写 golden JSON 文件（`tests/golden/`，每 fixture 一份投影）加显式重写开关（环境变量，test-gen 阶段落地定名）的方式，不引入 insta 等快照 crate（design 依赖节已排除）。投影需键排序规范化、时间戳原样保留，使漂移表现为可读 diff 而非噪声。

#### fixtures 语料清单（本阶段决策，从 74 个 archive 圈定并入仓快照）

实盘盘点：archive 74 个样本中 v0=41、v1=33、v2=0（v2 仅存在于 active change，archive 无 v2）。因此 v2 样本按"active 快照 + 合成全量"补充，其余从 archive 挑选；fixture 目录为入仓只读快照，与实盘 `openspec/` 解耦。

| 代际 | fixture 标识 | 样本来源 | 覆盖点 |
|------|--------------|----------|--------|
| v0 | v0-a | archive 快照 `2025-06-18-use-fast-glob` | 无 workflow.json + 遗留 eval.json 干扰 + markdown 四件套 + reports/ |
| v0 | v0-b | archive 快照 `2026-05-18-pge-workflow-architecture` | proposal/design/tasks/specs 纯文档形态 |
| v1 | v1-a | archive 快照 `2026-07-06-backtrack-reason-propagation` | 最小 workflow.json（仅 workflow_type，无 created / eval） |
| v1 | v1-b | archive 快照 `2026-09-17-workflow-file-inventory` | eval 含 backtrack_to / backtrack_reason、stale、skipped；无 files 桶 |
| v1 | v1-c | archive 快照 `2026-09-18-move-files-write-into-workflow-module` | eval + legacy `files{}` 桶与 `source` 未知键 |
| v2 | v2-a | active change `add-desktop-terminal/workflow.json` 快照 | file_log 键存在（空数组）的临界形态 |
| v2 | v2-b | 合成（非 archive 样本，注明合成） | 全量 v2：file_log 三种 op、active_phase、interrupted、eval 含 backtrack |
| 损坏 | corrupt-* | 合成（5 个小样本） | 整体非法 JSON / 单条 eval 损坏 / 单条 file_log 损坏 / 非法 verdict / 非法时间戳 |
| 布局 | layout-* | 合成 workspace 树 | 无日期前缀 archive 目录、空目录树、目录树混入文件（供 list / 集成用） |

#### 场景: 全语料 golden 对比

前置条件：fixtures 与 golden 均入仓。输入为逐 fixture 的完整投影。预期：每份投影与 golden 逐字节一致；语料覆盖三代 + 损坏 + 布局样本全集。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 全部 fixture 的投影（detect 结果 + list/detail 聚合 + 产物清单）与 golden 一致 | 新增 |
| 正向 | 语料完整性：fixtures 目录与清单表逐项对得上（防样本被静默删减） | 新增 |
| 边界 | 重写开关开启时重写流程可用且产出与现 golden 等价（不误写入仓 golden 的路径回退） | 新增 |

#### 场景: 降级路径进快照（防降级被静默吞掉）

损坏样本的投影必须记录 Unparsable 标记与跳过统计，否则"宽松解析悄悄吞数据"在 golden 上不可见。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | corrupt-* 样本投影含 unparsable / 跳过计数并与 golden 一致 | 新增 |
| 边界 | v1-c 的未知键忽略在投影中表现为"字段缺失"而非报错标记 | 新增 |

### useChangeList / useChangeDetail → ChangeList / ChangeDetailView → ArtifactView / Fallback（前端取数渲染链路） → `packages/desktop/src/__tests__/ipc_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/App.tsx` | 触发方（workspace 选择与视图切换） |
| `packages/desktop/src/hooks/useChangeList.ts` | 取数方（list_changes invoke） |
| `packages/desktop/src/hooks/useChangeDetail.ts` | 取数方（detail + 逐产物 read_artifact） |
| `packages/desktop/src/views/ChangeListView.tsx` | 渲染方（分组列表与代际徽标） |
| `packages/desktop/src/views/ChangeDetailView.tsx` | 渲染方（流水线与降级区块） |
| `packages/desktop/src/renderers/registry.ts` | 中间件（kind → renderer 路由） |
| `packages/desktop/src/renderers/Fallback.tsx` | 兜底方（未注册 kind 与读取失败） |

**关联AC**: AC-1, AC-5, AC-6, AC-7, AC-10, AC-11, AC-12

**关系描述**:

前端的取数纪律（组件不直接 invoke、显式刷新、产物信封同周期组装）与渲染纪律（按 kind 路由、永不白屏）分散在 hooks、views、registry、Fallback 四处，单测各自通过；它们组合起来才暴露的行为是：一次用户交互引发的完整 invoke 序列是否正确、DTO 字段漂移时视图是否按设计降级而非崩溃。Tauri IPC 是跨进程边界，测试在 `@tauri-apps/api/core` 的 invoke 处 mock（以 fixture DTO 模拟 Rust 侧返回），dialog 插件同理；链路内模块间调用一律真实实现。

#### 场景: 选定 workspace 到列表渲染

前置条件：mock invoke 返回 fixture ChangeList（含 active + archive 月分组 + 未知时间组）。输入：dialog 选定目录 → 刷新。预期：invoke("list_changes", { root }) 被以正确参数调用一次，列表视图渲染分组与代际徽标。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 选择目录后刷新 → invoke 参数携带选定 root，列表渲染月分组且"未知时间"组置尾 | 新增 |
| 边界 | 取消选择（dialog 返回 null）→ 无 invoke 调用 | 新增 |

#### 场景: 列表到详情与产物渲染

前置条件：mock invoke 按命令分发返回 detail DTO 与各 kind 信封。输入：点击列表项进入详情。预期：先 invoke("get_change_detail")，再按 artifacts 清单逐个 invoke("read_artifact")，信封按 kind 路由到三个 renderer 正确渲染。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 进入详情 → get_change_detail + N 次 read_artifact 的调用序列与产物清单一致 | 新增 |
| 正向 | 三 kind 信封分别路由到 MarkdownDoc / EvalChecklist / TasksProgress renderer 并渲染 payload | 新增 |
| 边界 | 产物清单为空 → 无 read_artifact 调用，详情其余区块正常 | 新增 |

#### 场景: DTO 漂移与读取失败的降级兜底

前置条件：mock 返回缺字段 DTO / 未注册 kind / 部分 read_artifact reject。预期：对应区块留空或 Fallback 兜底，页面整体不白屏（AC-10 前端侧缝合验证）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 未注册 kind 信封 → Fallback 渲染 fallback_text + kind 徽标，其余产物不受影响 | 新增 |
| 异常 | 单个 read_artifact reject → 该产物 Fallback 形态，详情主体正常渲染 | 新增 |
| 边界 | DTO 缺 file_log / backtrack 字段（模拟 schema 演进）→ 对应区块留空，不抛错 | 新增 |

#### 场景: 显式刷新纪律

前置条件：完成一次完整取数后静置。预期：除显式刷新触发的调用外无任何额外 invoke，无定时器注册（AC-12 缝合断言）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | 交互序列结束后推进虚拟计时 → invoke 调用次数不增长（无轮询 / watch） | 新增 |
| 边界 | 连续两次刷新 → invoke 序列为两组完整调用，最终渲染以后一次为准 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| @tauri-apps/api/core | vi.mock `invoke`，按命令名分发 fixture DTO / null / reject，并记录调用序列供断言 | 全部场景 |
| @tauri-apps/plugin-dialog | vi.mock `open`，按用例返回目录路径或 null | 选定 workspace 场景 |

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因 -->

- AC-13 独立包边界（lockfile 自带、根目录无新增 workspace 配置、`dist/` 不含 desktop 产物） — **原因**: 属仓库结构与构建卫生的结构性验收；自动化测试若断言仓库根全局形态（根 workspace 文件、构建产物目录内容）会与仓库其他工作线耦合而脆弱失效，由收口阶段评审与构建脚本确认。
- AC-11 中 `commands/exec/` 预留空轨道（无命令、无空壳 trait） — **原因**: "不存在"属性无法用运行时用例有意义地断言（无法枚举"未注册的命令"）；属静态结构约束，由代码评审确认。
- Tauri 运行时真实行为（main.rs Builder 命令注册生效、真实窗口创建、原生文件夹选择器对话框本体、真实 IPC 往返） — **原因**: 需桌面 GUI 运行时与 OS 交互；前端集成测试已以 mock 进程边界覆盖协议语义，真实链路留待手工验收（`tauri dev` 冒烟）。
- Tauri 2 + Windows 的 `tauri dev` / `tauri build` 构建链可用性 — **原因**: 属构建与环境验证（MVP 不涉及打包签名），非测试范畴；构建失败会在实现阶段即时暴露。
- 对实盘 `openspec/` 全部 74 个 change 的持续兼容扫描 — **原因**: 实盘数据只读且随仓库演进，自动化仅覆盖入仓 fixtures 代表样本（AC-14 的验收边界即"fixtures 覆盖三代代表样本"）；实盘全量兼容由"代表性挑选 + 宽松解析降级 + 永不白屏"策略保证，不做实盘扫描测试。
