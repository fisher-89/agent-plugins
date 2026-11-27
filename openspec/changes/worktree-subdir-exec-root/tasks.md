# 任务: worktree-subdir-exec-root

> **变更**: worktree-subdir-exec-root
> **依据**: `proposal.md` + `design.md`（D-1..D-10 决策编号见 design；AC-1..AC-14 锚点见 design「验收锚点」）
> **状态**: draft

任务边界：本列表只含实现任务，按依赖排序（阶段一解锁其后全部）。两条排除：① 测试**用例的设计与编写**（AC-1..AC-12 的新硬用例）归 test-design / test-gen；② `api:check` / 静态检查 / 测试执行等机械步骤由桌面端承接。

本列表**包含**因本次字段与签面变更而必须随动的既有代码——结构体字面量补齐（`exec_root: None` / `worktree: None`）、端口假件签名迁移、公共入口退役的调用点迁移、三文件四处精确键集断言手改、语料 golden 的显式重写。它们不是新用例，而是「不写编不过 / 不写必红」的配套。任务内引用的文件均在 design 变更清单（「修改文件」表 + 「测试文件改动面」A–G 组）内。

## 阶段一：端口形态与 vcs 派生（解锁后续全部）

- [x] `crates/core/workflow/src/write/worktree.rs`：`WorktreePort::worktree_root` → `placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>`；新增 `tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>`；新增 `pub struct WorktreePlacement { pub root: PathBuf, pub rel: String }`（`root` = `<.worktrees>/<repo>`，`rel` = `/` 分隔、根 workspace 为 `""`）；trait 与方法文档改写为「落位 / 执行根分离」（design D-1）；`write/mod.rs` 再导出 `WorktreePlacement`
- [x] `crates/infra/vcs/src/git.rs`：`toplevel()` → `pub(crate) fn placement(main_root: &Path) -> Result<(PathBuf, String), String>`——一次 `rev-parse --show-toplevel --show-prefix`，首行为 git 根、次行 `trim_end_matches('/')` 为 `rel`（根 workspace 空行 → `""`），非 git / 裸仓 / 空输出三档引导文案自 `toplevel` 原样迁移；新增 `pub(crate) fn tracked_at_head(main_root: &Path, rel: &str) -> Result<bool, String>`——`rel.is_empty()` 直返 `Ok(true)` 零 spawn，否则 `rev-parse --verify --quiet HEAD:<rel>`：退出 `0` → `true`、`1` → `false`、其余 → `Err`（fail-closed）；新增私有退出码通道（`git_raw` 返回 `(code, stdout, stderr)`，`git()` 的「非零即 Err」语义与文案逐字保留）；`restore_baseline` / `worktree_snapshot` / `remove_worktree` / `commit_all` / `dirty` / `rebase` / `ff_merge` 命令体**一字不动**（design D-1 / D-2 / AC-7）
- [x] `crates/infra/vcs/src/lib.rs`：新增 `pub fn placement(workspace_root: &Path) -> Result<WorktreePlacement, String>`（`git::placement` + `worktree_root_from_toplevel` 组合，`rel` 原样透传）；删除 `pub fn worktree_root`（公共单点退役）；`worktree_root_from_toplevel` 保留为私有纯派生；`impl WorktreePort for ProcessWorktree` 增 `placement` / `tracked_at_head` 委托；`WORKTREES_DIR_NAME` 与 `remove_worktree` / `restore_baseline` 等归档缝逐字不变（design D-1）
- [x] 假件签名迁移（design「测试文件改动面」A 组，编译面，零断言意图改动——唯一例外：`create_test.rs` 内 `create_outcome_serde线形状恰五键且id首字段` 的键集 / 字面量随 `CreateOutcome` 增列手改，见阶段八）：`crates/core/workflow/src/write/create_test.rs`、`delete_test.rs`、`promote_test.rs`、`crates/core/workflow/tests/corpus_golden_test.rs` 四个 `impl WorktreePort` 改两方法（`placement` 返回可编程 `WorktreePlacement`、`tracked_at_head` 返回可编程布尔；`create_test.rs` 另带调用计数，供 AC-1「零调用」与 AC-3 六形态断言）
- [x] 公共入口退役的调用点迁移（design「测试文件改动面」B 组，AC-10 零残留）：`crates/infra/vcs/src/lib_test.rs` 的 `worktree_root` 两例三断言（根 workspace 落位 / 子目录 workspace 仍锚 git 根 / 非 git 目录 Err）迁到 `placement`（`.root` / `.rel` 两值断言；`worktree_root_from_toplevel` 纯派生四例保留）；`packages/desktop/src-tauri/src/commands/changes/mod_test.rs` 的 `vcs_runtime::worktree_root(env.ws_root.path())` → `vcs_runtime::placement(env.ws_root.path()).root`

## 阶段二：记录承载与模型版本

- [x] `crates/core/workflow/src/state.rs`：`ChangeStateRecord` 增 `pub exec_root: Option<String>`（声明序 `worktree` 之后、`base_commit` 之前，字段文档写「`None` = 与 `worktree` 同值」）；新增 `impl ChangeStateRecord { pub fn exec_root(&self) -> Option<&str> }`——`worktree = None` → `None`；`exec_root = None` → `Some(worktree)`；真后代（`Path::new(exec) != Path::new(worktree) && Path::new(exec).starts_with(worktree)`，组件级比较）→ `Some(exec)`；否则 `eprintln!("[workflow] …")` 留痕 + `Some(worktree)`（design D-4 / AC-5）
- [x] `crates/infra/store/src/model.rs`：今日 `ChangeRecord` 整体改名 `ChangeRecordV4`（`#[native_model(id = 9, version = 4, from = ChangeRecordV3)]`，`pub(crate)`，双向 `From` 保留）；现役 `ChangeRecord` 升 `version = 5, from = ChangeRecordV4`，在 `base_commit` 之后追加 `#[serde(default)] pub exec_root: Option<String>`；新增 v4 → v5 升级 `From`（`exec_root = None`）与 v5 → v4 降级 `From`（丢弃该列）；`ChangeRecord::new` 末位增 `exec_root: Option<String>`；`WORKSPACE_STORE_FORMAT_VERSION` **零改动**（design D-9 / AC-9）
- [x] `crates/infra/store/src/store.rs`：`change_state()` 与 `create_change_record` 两处映射补 `exec_root` 搬运；其余 change 写路径（开相 / 落账 / 回溯 / 归档翻转 / 启动标定）零改动（design D-4）
- [x] 机械补齐（编译面）：`ChangeStateRecord { … }` 与 `ChangeRecord::new(…)` 的全部既有构造点补 `exec_root: None`（前者 25 个文件、后者 4 个文件，清单见 design「测试文件改动面」E 组明细——含 `write/promote_test.rs`，它是被前稿漏掉的构造点），不改任何断言

## 阶段三：create 建域（host / exec root 分工）

- [x] `crates/core/workflow/src/write/create.rs`：`vcs.worktree_root` → `vcs.placement`（`placement.root` 供前置⑨ 磁盘门槛扫描、`placement.root.join(name)` 供 host）；新增**前置第十道**（紧接前置⑦ 之后、建档与 `add_worktree` 之前）：`rel` 非空且 `!vcs.tracked_at_head(main_root, &placement.rel)?` → 显式 `Err`（文案含 `rel`、三形态成因「未跟踪 / HEAD 已删除 / 被 gitignore」与「纳入 git 跟踪并提交，或改用仓库根作为 workspace root」引导），零建档零目录零 worktree 零 branch（design D-3 / AC-3）
- [x] `crates/core/workflow/src/write/create.rs`（执行段）：`exec_root = host.join(&placement.rel)`；建档 `ChangeStateRecord` 双写 `worktree`（host）与 `exec_root`（`rel = ""` → `None`）；`add_worktree` 目标恒 host；新增**后置断言**（`rel` 非空才做，且 MUST 早于 `write_fs_half`——`create_dir_all` 会把缺失目录建出来）：`!exec_root.is_dir()` ⇒ `compensate_tree_failure`（入参 host）并 `Err`，文案与前置拒绝可辨（含「已补偿回收」）；`resolve(&exec_root)` + `write_fs_half` 与 `bootstrap(&exec_root, …)` 改执行根；私有 `bootstrap_worktree_root` → `bootstrap_exec_root`（文档 / 参数名改执行根）；`bootstrap_project` 警告文案「在 worktree 内不存在」→「在执行根内不存在」；`BootstrapProject.root` 文档改「相对执行根」（design D-3 / AC-2）
- [x] `crates/core/workflow/src/write/create.rs`（出线）：`CreateOutcome` 增 `pub exec_root: Option<String>`（`serde(rename_all = "camelCase")` → `execRoot`；声明序紧跟 `worktree`，出线序 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings`；`rel = ""` → `None`）；`worktree: String` 字段文档钉为 host（design D-5 / AC-11）。注：本步使三处既有精确键集断言必红（`write/create_test.rs` 的 serde 键集、`commands/changes/mod_test.rs` 两处序列化键集），其手改统一在阶段八（C 组第一 / 三 / 四处）；阶段三至七间测试目标不绿属预期

## 阶段四：run 三根分离与归档双列

- [x] `crates/core/orchestration/src/state.rs`：`ArchivePreflight` 增 `pub exec_root: Option<String>`（camelCase → `execRoot`；声明序紧跟 `worktree`，字段文档写「原始列；`None` = 与 `worktree` 同值」）；其余七字段逐字不变（design D-6）
- [x] `crates/core/orchestration/src/steps.rs`：`run_archive_preflight` 填两列（`worktree` = 记录 `worktree`、`exec_root` = 记录原始列）；`missing_artifacts` / `detect_delta_specs` 的根参数改吃访问器值（参数名 `worktree` → `exec_root`，签名形态与调用点数不变，`detect_delta_specs` 的「主仓优先、worktree 回退」两级序保留）（design D-6 / AC-6）
- [x] `crates/core/orchestration/src/walker.rs`：`RunRequest` 增 `pub worktree: Option<String>`（host；legacy → `None`，字段文档写「git 步与回收的唯一锚，MUST NOT 从 `root` 反推」）；`step_clean` 早退判据与 `restore_baseline` 实参改 `request.worktree`（host 全树；`None` 走 legacy 跳过分支）；initialize 相位短路与 Clean 双保险两处 `root == main_root` 换 `worktree.is_none()`；其余步执行体与 `archive_context` 返回形态逐字不变（design D-7 / AC-4 / AC-8）
- [x] `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：前置校验 3 的 exec root 解析改 `record.exec_root()`（`is_dir` 校验沿用，Err 文案名词「worktree 目录」→「执行根目录」并携路径）；`RunRequest` 增 `worktree: record.worktree.clone()`；compose 注入 / 快照源 / 复合键装配零改（design D-7）
- [x] 机械补齐（编译面）：`RunRequest { … }`（`orchestration/src/{walker,run_history}_test.rs` / `corpus_golden_test.rs`）与 `ArchivePreflight { … }`（`orchestration/src/{state,steps,walker,port,control,notify}_test.rs` / `src/commands/{changes,change_flow}/mod_test.rs`）补 `worktree: None` / `exec_root: None`，不改断言（design「测试文件改动面」E 组明细）

## 阶段五：读面与命令面（执行根定位）

- [x] `crates/core/workflow/src/queries/detail.rs`：`change_detail` 的 `locate_change` 回退参改 `record.exec_root()`；`ChangeDetail` 增 `pub exec_root: Option<String>`（取原始列，legacy / 根 workspace → `null`；声明序紧跟 `worktree`，即 DTO 末位）；`worktree` 字段与语义不变（design D-5 / AC-6）
- [x] `packages/desktop/src-tauri/src/commands/changes/mod.rs`：`read_artifact` 的 `locate_change` 回退参改 `record.exec_root()`；`change_dirs` 两处候选供给改 `record.exec_root()`（`push_worktree_candidate` 复用，探测序与去重不变）；复核 `CreateOutcome` 新字段经写面 DTO 直通抵达命令返回（零代码）（AC-6）
- [x] `packages/desktop/src-tauri/src/commands/archi/mod.rs`：`archi_change_summary` 的 active 改动侧改 `record.exec_root().map(str::to_owned)`；`Archived → None` 分支保持（归档仍回落主仓根）（AC-6）

## 阶段六：配置文案（zero-code）

- [x] `crates/core/config/config.schema.json`：`projects.description` 与 `projects.items.properties.root.description` 的「相对 worktree 根」→「相对执行根」；键集 / 类型 / `default` / `minLength` / 其余描述逐字不变
- [x] `crates/core/config/src/lib.rs`：`is_valid_project_root` 注释理由「逃出 worktree」→「逃出执行根」；谓词 / `KNOWN_KEYS` / `TestFramework` / `validate` 逐字不变

## 阶段七：出线生成物与前端可见性 / 派生

- [x] `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts`：经 `pnpm -C packages/desktop run bindings:export` 重生成（三 DTO 的 `execRoot` 入线；生成物 MUST NOT 手改），复核 diff。**本任务 MUST 先于其后全部前端消费点与 fixture 补齐**——`ChangeDetail` / `ArchivePreflight` / `CreateOutcome` 的 TS 类型面由它产出，倒置即编译失败（design D-9 / 变更清单 bindings 行）
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`DetailHeader` 增第二行（`detail.execRoot !== null` 才渲染，`data-testid="detail-exec-root"`，文案 `执行根：<execRoot>`；为空时零占位、零空行）；详情体增一处解析 `const execRoot = detail.execRoot ?? detail.worktree`，供 `archiRoot`（`archive` 或 `null` → `root`）与 `ChangeChatPanel.cwd`（design D-8 / AC-12）
- [x] `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts`：参数文档口径改「执行根来源（`detail.execRoot ?? detail.worktree`，由详情视图解析后经面板 `cwd` prop 送入）」；`cwd = archived ? null : worktree` 行为**逐字不变**（`change-chat-panel.tsx` 零改动）（design D-8 / Q4）
- [x] `packages/desktop/src/views/changes/flow/run-control-panel.tsx`：`ArchitectureChangeBlock` 预览根 prop 更名 `worktree` → `previewRoot`（文件内私有，文档同步）并改喂 `preflight?.execRoot ?? preflight?.worktree ?? null`；`ArchiPreview` 文档口径同步；合入 / 清理告知两处 `preflight.worktree !== null` 判定与文案结构保持不变（host 口径）（design D-8 / AC-12）
- [x] 零代码复核（Q3 / AC-13 覆盖对账）：`packages/desktop/src/views/changes/components/change-create-dialog.tsx` 核对成功面已退役（提交成功即 `onCreated(result.id)` 关窗导航）、组件内无 `worktree` 路径渲染与零 `execRoot` 消费点，确认提案「成功面路径行 / 两行」无落点——本文件 MUST 保持零 diff
- [x] 前端 DTO fixture 机械补齐（编译面，design「测试文件改动面」F 组）：`ChangeDetail` 字面量八处——`app.test.tsx`（`fakeDetail`）、`change-detail-view.test.tsx`（`detail()` 工厂）、`hooks/use-change-detail.test.ts`（`fakeDetail()`）、`flow/attachments.test.ts`、`flow/graph.test.ts`、`flow/change-flow-graph.test.tsx`、`flow/layout.test.ts`、`flow/detail-drawer.test.tsx`；`ArchivePreflight` 一处——`flow/run-control-panel.test.tsx`（三处 `preflight()` / `archivePreflight()` 工厂）；`CreateOutcome` 一处——`components/change-create-dialog.test.tsx`（`OUTCOME` fixture 与「五字段」注释 → 「六字段」）。一律补 `execRoot: null`，不改断言。零改动复核：`change-list-view.test.tsx`（`create_change` 应答体是匿名结构类型，不经 `CreateOutcome`）、`lib/ipc-drift.test.ts`（仅命令名字符串）

## 阶段八：精确断言与语料 golden

- [x] 手改 workflow 侧 serde 键集断言（design「测试文件改动面」C 组第一处 / D-9）：`packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs` 的 `create_outcome_serde线形状恰五键且id首字段`（:2491-2531）→ 六键：① 文档注释（:2491-2495）五键清单补 `execRoot`、措辞「恰五键」→「恰六键」；② 函数名（:2497）→ `create_outcome_serde线形状恰六键且id首字段`；③ 结构体字面量（:2498-2504）在 `worktree` 之后补 `exec_root: None`（根 workspace / legacy 形态）；④ 键集 vec（:2512）→ `["created", "execRoot", "id", "name", "warnings", "worktree"]`、断言消息（:2513）→「恰六键面（id 为身份锚、worktree 与 execRoot 为刻意出线的两根）」；⑤ `warnings` 断言之后（:2522 后）新增 `assert_eq!(object["execRoot"], serde_json::json!(null), "execRoot 键恒在场（None → null，不省键）")`；⑥ Debug 注释（:2524-2525）五字段清单补 `execRoot`。**逐项核对其余九项 pass 内容逐字不动**：`to_value` / `as_object` / `keys` 收集与排序 / `object["id"]` / `object["name"]` / `object["created"]` / `object["warnings"]` / `format!("{outcome:?}")` / `debug.starts_with("CreateOutcome { id: ")`（该函数是 `CreateOutcome` 的唯一测试构造点，不手改即编不过 / 必红）
- [x] 手改 bindings 侧出线字段集断言（C 组第二处；design「测试文件改动面」C 组 / D-9；bindings 侧**无** `DESKTOP_GOLDEN_REWRITE` 通道——`authoritative_snapshot()` 每次运行时先 `export_bindings()` 再读产物）：`packages/desktop/src-tauri/src/bindings/mod_test.rs` 的 `create_outcome出线恰五字段_id为首字段`（:1555-1582）→ 六字段（测试名 / 文档注释「五 → 六」、`field_names` 期望 `["id", "name", "created", "worktree", "execRoot", "warnings"]`、补 `execRoot: string | null` 形态断言）；同文件 `archive_preflight线面字段面保留` 是逐字段 presence 循环，零改动
- [x] 手改 commands 侧第一处序列化键集断言（C 组第三处）：`packages/desktop/src-tauri/src/commands/changes/mod_test.rs` 的 `create接线_建域落位_返回dto增id且按id立即可见可发起` 五键断言（:621-625，字面量 :623）→ `["created", "execRoot", "id", "name", "warnings", "worktree"]`，注释「恰五键面」→「恰六键面」
- [x] 手改 commands 侧第二处序列化键集断言（C 组第四处，同文件）：`packages/desktop/src-tauri/src/commands/changes/mod_test.rs` 的 `create_workflow_type透传test_only且dto五键不变`（:2564-2608）五键断言（:2605）→ `["created", "execRoot", "id", "name", "warnings", "worktree"]`，同函数文档注释「返回 DTO 仍恰五键（created / id / name / warnings / worktree）」（:2560-2562）→ 六键清单，断言消息「CreateOutcome 恰五键」（:2606）→「CreateOutcome 恰六键」
- [x] 语料 golden 显式重写（design「测试文件改动面」D 组 / D-9）：以 `DESKTOP_GOLDEN_REWRITE=1` 跑 `crates/core/workflow/tests/corpus_golden_test.rs`，覆写 8 份 detail golden（`corpus-{archive-prefix,backtrack-stale,initialize,multi-attempt,run-history,run-interrupted,slots-null,worktree}.json`）各增 `"execRoot": null`；随即关闭开关重跑复核零 diff，并人工逐份确认 diff 仅此一键（`corpus-list-mixed.json` 零变化）；MUST NOT 静默自动重写

## 阶段九：交付与守线收口

- [x] `packages/desktop/package.json` `version` `0.7.23` → `0.7.24`（+1 patch；`tauri.conf.json` 经 `"../package.json"` 跟随，`src-tauri/Cargo.toml` crate 版本行不随动）（AC-13）
- [x] 守线对账（源码级，非测试执行）：① 全仓 grep `worktree_root` 零残留调用点（含测试内 `vcs_runtime::worktree_root`）、`exec_root.or(` / `exec_root().or(` 零散落（访问器单点）；② grep `restore_baseline` / `remove_worktree` 实参只来自 `request.worktree` / 记录 `worktree` 列，无 `exec_root` 出处（AC-4 / AC-7）；③ grep 无 `root == main_root` 形态 legacy 代理（AC-8）；④ 零改动核对：`write/{delete,promote,archive}.rs`（产品面，同目录 `*_test.rs` 另计）、`queries/{mod,list}.rs`、`change-chat-panel.tsx`、`change-create-dialog.tsx`、`openspec/specs/**`、`.dev-team/architecture/**`、`src-tauri/{Cargo.toml,tauri.conf.json}`、`WORKSPACE_STORE_FORMAT_VERSION` 逐字节不变；⑤ `INDEX.toml` / `.i.*` / `*.schema.json` / `*.md` 零差异（AC-11 / AC-13）
- [x] 变更清单对账：实现文件与 design「修改文件」表 + 「测试文件改动面」A–G 组逐项对齐（无清单外改动、无清单内遗漏）；按 Q1 扩表口径核对 `git status --porcelain`（含 8 份 golden 与 10 个前端 fixture 文件）（AC-13）

## 实施备注（实现相位回填，非新任务）

1. **清单外但必须随动的既有断言（design「测试文件改动面」未列，属 D-9 版本落地的直接后果）**：`crates/infra/store/src/model_test.rs`（`ChangeRecord` v4 回环用例改写为 v5 回环、`change_record_v4到v3降级半边` 与 `title恒非空不变量` 的 `From` 链经 `ChangeRecordV4` 中转、模型 id/版本元数据 `(9, 4)` → `(9, 5)` 与物表名 `9_4_id` → `9_5_id`）、`crates/infra/store/src/store_test.rs`（`CHANGE_RECORD_TABLE` = `9_5_id` 与其两处文案断言）。不随动则这些断言对 v5 恒假。
2. **文案断言随动（design 变更清单已改文案，断言未列）**：`crates/core/workflow/src/write/create_test.rs` 三处「在 worktree 内不存在」→「在执行根内不存在」；`src/commands/change_flow/mod_test.rs` 一处「worktree 目录不存在」→「执行根目录不存在」；`crates/core/orchestration/src/walker_test.rs` 一条 `root == main_root` 判据注释改 `request.worktree.is_none()`。
3. **`tests/` 目录写入授权**：`crates/core/workflow/tests/corpus_golden_test.rs`（A 组假件签名迁移 + 7 处 `ChangeStateRecord` 补 `exec_root` + 3 处 `RunRequest` 补 `worktree`）与 8 份 detail 语料 golden 的重写（`DESKTOP_GOLDEN_REWRITE=1`，逐份 diff 仅 +1 行 `"execRoot": null`，取消开关重跑零漂移）经用户当轮显式授权后执行；相位黑名单默认口径仍为只读。
4. **测试假件的未用注入面**：`create_test.rs` 的 `FakeVcs::{set_rel, set_untracked, set_tracked_error, tracked_calls}` 供 AC-1「零调用」与 AC-3 六形态用例注入，标 `#[allow(dead_code)]`（新增硬用例归 test-gen，落地后移除该豁免）。
5. **`crates/core/orchestration/src/state_test.rs` 的 `archive_preflight八字段线面与往返可辨`** 是逐字段 presence 断言（非精确键集），受 `execRoot` 增列影响为零，未改；`execRoot` 行的断言归 test-gen（同 design 对 bindings `archive_preflight线面字段面保留` 的处置口径）。
