# 任务: project-config-protocol-extension

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 11 项（含两份生成物）+ 设计补充的编译闭合面 4 个测试文件（`create_test.rs` 夹具形参与 8 处 roots 传参、前端 3 个测试文件的 4 处 fixture 补键）——逐条登记在 design「变更清单 - 测试文件（编译闭合）」，校准依据见 design「事实校准」1 / 3。
>
> **本相位不改测试用例与断言口径，也不执行任何静态检查 / 测试回归**：新用例（config 矩阵 / 拓扑序五例 / 呈现用例）归 test-design / test-gen / test-execution 相位。本相位对测试文件的唯一动作是**让工作区随字段面 / 形参面变化编译通过**（补字段、补实参），MUST NOT 新增用例、MUST NOT 改断言。
>
> **依赖序（七阶段）**：
>
> - **阶段一**（`core/config` 协议字段）是镜像、写面映射、生成物、呈现面的共同前提。
> - **阶段二**（schema 镜像）依赖阶段一（字段集确定后才能逐项对照），二者同 commit 语义。
> - **阶段三**（写面 `BootstrapProject` + 拓扑序）独立于阶段一/二（不引 `config` 依赖边），是命令层与既有测试夹具编译通过的前提。
> - **阶段四**（两处命令层映射）依赖阶段一（两字段可读）与阶段三（`BootstrapProject` 出口）。
> - **阶段五**（`bindings:export` 再生成）依赖阶段一（`ProjectConfig` 出线面已定）。
> - **阶段六**（配置页呈现）依赖阶段五（生成物有 `language` / `dependsOn`）。
> - **阶段七**（版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ① **协议零智能**：`assemble_project` 的两字段处置 MUST NOT 出现 trim / 大小写归一 / 别名映射 / 白名单 / 引用校验 / 去重（复核 `grep -n "language\|depends_on" crates/core/config/src/lib.rs` 只命中字段定义、两行组装与 doc）。
> ② **消费面零智能**：`src/commands/**` 只做 `Option→Vec` 的 `unwrap_or_default()` 映射，`language` 一次读取都没有；`create.rs` 的 `install_order` 只排序，四类脏数据零诊断（MUST NOT 新增警告文案）。
> ③ **逐字不动面**：`KNOWN_KEYS` 八项、`TestFramework` 八值、`opt_string` / `opt_string_list` / `assemble` 单遍结构、`BOOTSTRAP_LOCKFILE_COMMANDS` 映射表、`bootstrap_project` / `bootstrap_worktree_root` 的三段警告文案与 `WARN_*` 常量、`create` 的九道前置校验与三处补偿、`crates/core/workflow/Cargo.toml`、`.dev-team/architecture/**`、store / db 面、`packages/desktop/src/views/config` 的只读边界、`.dev-team/config.json`、`openspec/todo.md`。
> ④ **生成物不手改**：`packages/desktop/src/types/generated/**` 一律只经 `bindings:export` 产出（diff 只应来自生成）。
> ⑤ **门禁分工**：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与测试回归由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行；自查只收源码级静态判据（`grep` 零命中、读码复核、`git diff` 边界比对）。

## 阶段一：config 协议字段（`core/config`）

- [x] `packages/desktop/src-tauri/crates/core/config/src/lib.rs`：`ProjectConfig` 追加两字段（design DD1）
  ```rust
  /// 编码语言标注（开放字符串：零白名单 / 零归一 / 不与 `tests[].framework` 交叉校验；
  /// 未设为 `None`）——声明型字段，消费侧归一单点见 desktop-workspace-config
  pub language: Option<String>,
  /// 依赖的项目根（保序、不去重、零引用校验；未设为 `None`）——唯一消费者为
  /// bootstrap 拓扑序安装（见 desktop-change-worktree「确定性依赖 bootstrap」）
  pub depends_on: Option<Vec<String>>,
  ```
  结构体 doc 同步为「项目配置（依赖引导项目锚点）：`root` 为目录锚点，`language` / `depends_on` 为软规则字段」；derive 面与 `#[serde(rename_all = "camelCase")]` 零改动。
- [x] 同文件 `assemble_project`：在 `root` 判定成功之后、构造返回值之前追加两行（顺序固定，design DD2）
  ```rust
  let language = opt_string(map, "language", &format!("{project_path}.language"), diagnostics);
  let depends_on =
      opt_string_list(map, "depends_on", &format!("{project_path}.depends_on"), diagnostics);
  ```
  返回值改为 `Some(ProjectConfig { root, language, depends_on })`；`root` 的两个剔除分支（缺失 / 非法）位置与文案逐字不动。
- [x] 同文件 `WorkspaceConfig.projects` 的 doc 改为「依赖引导项目锚点列表（元素携可选 `language` / `depends_on`；未设 / 显式 null → 空列表且无诊断）」；`KNOWN_KEYS`（八项）、`opt_string` / `opt_string_list`、`assemble` 的结构零改动。

## 阶段二：schema 镜像同步（`core/config/config.schema.json`）

- [x] `packages/desktop/src-tauri/crates/core/config/config.schema.json`：`projects.items.properties` 在 `root` 之后追加两条（形状与 description 文案按 design DD3；`required` 保持 `["root"]`，`projects.description` 若提及承载面则一并写「项目锚点（root）与可选 language / depends_on」，其余字节零改）
  ```json
  "language": {
    "type": "string",
    "description": "编码语言标注（自由文本，不做归一 / 校验；仅作声明，后续按语言分派时由消费侧归一）"
  },
  "depends_on": {
    "type": "array",
    "items": { "type": "string" },
    "description": "依赖的项目根（值宜为某个 projects[].root；不做引用校验，安装时依赖先于依赖方）"
  }
  ```
- [x] 复核两断言锚点零改动且仍成立：`KNOWN_KEYS` 八项键集 == `schema.properties` 键集（未加顶层键）、`TestFramework` 八值 == `framework.enum`（未动枚举）；MUST NOT 为两软规则字段新增第三条断言。
- [x] 逐项对照：`ProjectConfig` 字段集（`root` / `language` / `depends_on`）与 `projects.items.properties` 键集三项一致、无多余、无缺项（design 校准 6 / R8）。

## 阶段三：写面拓扑序安装（`core/workflow`）

- [x] `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`：新增 `pub struct BootstrapProject`（design DD4）
  ```rust
  /// 依赖引导的单个项目锚点（写面内部纯数据：不进 IPC、不落盘、不区分「未设」与「空」）。
  #[derive(Debug, Clone)]
  pub struct BootstrapProject {
      /// 项目目录（相对 worktree 根；`.` 等价 worktree 根）
      pub root: String,
      /// 依赖的项目根（零引用校验；仅用于推导安装序）
      pub depends_on: Vec<String>,
  }
  ```
- [x] 同文件新增 `use std::collections::HashMap;` 与私有 `install_order` / 递归 `visit_order`（design DD5）：同 root 取**首次**下标（`entry().or_insert`）；外层 `iter().enumerate()` 并跳过非规范下标；`visit` 先置 `visited` 再递归依赖（悬空查不到即跳过）→ 后序 `push(root)`；返回 `Vec<&str>`。零 `Err`、零 IO、零诊断。
- [x] 同文件 `bootstrap`：第 2 参改 `projects: &[BootstrapProject]`；`projects.is_empty()` → `bootstrap_worktree_root`（逐字保留）；否则 `for root in install_order(projects) { bootstrap_project(worktree, root, vcs, warnings); }`；doc 注释由「按配置序逐个项目根扫描」改为「按 `depends_on` 拓扑序逐个安装（依赖先于依赖方，同层保持声明序；脏依赖零阻断）」。
- [x] 同文件 `create`：第 4 参 `project_roots: &[String]` → `projects: &[BootstrapProject]`，段四调用改 `bootstrap(&worktree, projects, vcs, &mut warnings)`；函数 doc 与 `#[allow(clippy::too_many_arguments)]`、参数顺序、九道前置校验、四段执行、三处补偿、警告清单语义逐字不动。
- [x] `bootstrap_project` / `bootstrap_worktree_root` / `BOOTSTRAP_LOCKFILE_COMMANDS` / `WARN_*` 常量：**零改动**（`git diff` 出现即越界）。
- [x] `crates/core/workflow/src/write/promote.rs`：`promote_explore` 第 4 参改 `projects: &[BootstrapProject]`，doc 与 `super::create(...)` 透传实参随动；搬运失败文案与 `PromoteOutcome` 零改动。
- [x] `crates/core/workflow/src/write/mod.rs`：出口面增 `pub use create::BootstrapProject;`。
- [x] 编译闭合（不新增用例、不改断言）：`crates/core/workflow/src/write/create_test.rs` 的夹具 `create_with_projects` 第 3 参改收 `&[BootstrapProject]`，并在测试模块内加私有助手（例如 `fn proj(root: &str) -> BootstrapProject { BootstrapProject { root: root.to_owned(), depends_on: Vec::new() } }`），8 处项目 roots 传参（`web`/`service`/`api`/`missing`/`plain`/`rs`/`.`）改成对应 `BootstrapProject` 构造——只做形参适配，断言结构与既有文案逐字保留。
- [x] 复核 `promote_test.rs:85` 的 `&[]` 无需改动（空切片类型推断成立），`create_full` / `create_with_title` / `env.create` 传 `&[]` 处零改动。

## 阶段四：命令层映射（`src-tauri/src/commands`）

- [x] `src-tauri/src/commands/changes/mod.rs`（`create_change_with` 闭包内）：`project_roots: Vec<String>` 改为 `projects: Vec<write::BootstrapProject>`，逐元素 `BootstrapProject { root: project.root, depends_on: project.depends_on.unwrap_or_default() }`；`write::create(...)` 实参由 `&project_roots` 改为 `&projects`；注释更新为「项目锚点（root + depends_on）经 config crate 读取；language 在安装路径不消费」。
- [x] `src-tauri/src/commands/explores/mod.rs`（`spawn_blocking` 闭包内）：同上映射，`write::promote_explore(...)` 实参随动，注释同口径。
- [x] 复核：`grep -rn "language" packages/desktop/src-tauri/src/commands` 只应命中注释，无字段读取；两处映射 MUST NOT 对 `depends_on` 做 trim / 去重 / 过滤（协议与消费面均零智能）。

## 阶段五：绑定类型再生成（生成物）

- [x] 运行 `pnpm -C packages/desktop run bindings:export`，确认生成物 `src/types/generated/bindings.ts` / `bindings.zod.ts` 的 `ProjectConfig` 变为 `root: string; language: string | null; dependsOn: string[] | null`（zod 侧 `z.string().nullable()` / `z.array(z.string()).nullable()`）。
- [x] `git diff packages/desktop/src/types/generated/` 复核：diff 只来自生成（仅 `ProjectConfig` 两字段 + doc 注释），MUST NOT 出现手工编辑痕迹；`WorkspaceConfig.projects` 等其余类型零变化。

## 阶段六：配置页呈现（前端）

- [x] `packages/desktop/src/views/config/components/basic-config-section.tsx`：`ProjectsGroup` 按 design DD9 改造——每个 `<li key={index} className="break-all">` 内三个同层 `<span>`：`{project.root}`、`` `language：${project.language ?? '未设'}` ``、`` `depends_on：${depsText(project.dependsOn)}` ``（后两者 `ml-3 text-xs text-muted-foreground`）；新增模块私有 `depsText(deps: string[] | null): string`（`null → '未设'`、`[] → '（空列表）'`、否则 `join('、')`）；容器 `data-testid="config-projects"`、空态 `（空列表）`、`index` 作 key 与只读形态保持（无输入控件、无 onChange、无写回）。
- [x] 同文件 `ProjectsGroup` 的 doc 注释更新为「逐项目呈现 root + 已设的 language / depends_on；未设标注『未设』（呈现面不隐藏已配置值）」；`BasicConfigSection` 头部 doc 的 `projects` 描述同步。
- [x] 前端 fixture 编译闭合（只补字段、不改断言）：`basic-config-section.test.tsx:54 / :97`、`app.test.tsx:131`、`config-view.test.tsx:37` 的 `ProjectConfig` 字面量补 `language: null, dependsOn: null`（`projects: []` 处无需改）；`use-workspace-config.test.ts:28` 为 `[]` 零改动。

## 阶段七：版本、边界自查与交接

- [x] `packages/desktop/package.json`：`version` `0.6.59` → `0.6.60`；确认 `src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零改动）、`src-tauri/Cargo.toml` 的 crate 版本行**不跟**。
- [x] 边界自查（源码级判据，不跑门禁）：
  - `grep -rn "config.json" packages/desktop/src-tauri/crates/core/config packages/desktop/src-tauri/crates/core/workflow` 零命中（`foundation::layout` 例外不变）；
  - `packages/desktop/src-tauri/crates/core/workflow/Cargo.toml` 无 `config` 依赖行（`git diff` 空）；
  - `grep -rn "trim\|to_ascii_lowercase\|to_lowercase" crates/core/config/src/lib.rs` 在 `assemble_project` 区域零命中；
  - `git diff --stat` 未触及 design 约束③的逐字不动面（`KNOWN_KEYS`、`TestFramework`、`bootstrap_project` 文案、`.dev-team/architecture/**`、store / db、`.dev-team/config.json`、`openspec/todo.md`）。
- [x] 契约面复核：`.dev-team/architecture/models/Backend/` 下仍只有 `App.Backend.Core.Foundation.md` / `App.Backend.Infra.ArchiRuntime.md`（本变更新增零契约文档）；若 `api:check` 报 finding，按 `desktop-archi-api-check` 十三值判据在 static-check 相位处置，不回退本设计的协议裁决。
- [x] 交接（由桌面端相位执行，本相位不运行）：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `pnpm -C packages/desktop run test`；并把「`src/bindings/mod_test.rs:841` 的『项目根单字段』注释语义过期」与「新用例清单（config 矩阵 / 拓扑序五例 / 呈现用例）」交接给 test-design / test-gen。
