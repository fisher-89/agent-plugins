# 设计: desktop-change-worktree

> **变更**: desktop-change-worktree
> **日期**: 2026-10-06

---

## 提案与规格同步状态

`proposal.md` 与 `openspec/changes/desktop-change-worktree/specs/**`（8 个 spec delta）已由提案阶段定稿，explore 决策 D1–D7 用户拍板在案；本设计不重复其内容、不将其列为待办，只在其六项「待决问题」之上定稿（见「关键设计决策」）。

两点基准勘误与隔离注记：

1. **版本交付基准**：spec「版本交付」行的 `0.4.13 → 0.4.14` 是提案定稿时的快照；其后主仓已随其他变更升至 `0.4.16`。实施按该 requirement 的语义（用户可见新功能升一位）执行 **0.4.16 → 0.4.17**，不降版（D15）。
2. **在飞改动隔离**：工作区当前未提交的 `infra/agent` SDK loop 重试护栏改动（`sdk/loop.rs` / `runner.rs` 及测试）属另一在飞变更，本变更零触点——变更清单不含这些文件，守线对账排除。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| foundation 身份段单点（新） | workspace 身份段纯函数（db 文件名与 worktree 子目录同源消费） | `crates/core/foundation/src/identity.rs`（新） | sha2 | 纯函数，零 fs |
| vcs-runtime（新 crate，裸名，vcs 边界首成员） | git 工作面进程执行 + worktree 落位派生 | `crates/infra/vcs/`（新：`Cargo.toml` / `src/lib.rs` / `src/git.rs` / `src/bootstrap.rs`） | workflow（port 类型）+ foundation | std::process 同步 spawn（零 tokio 零 Tauri）；超时轮询 kill |
| WorktreePort 缝（新） | 写面 → vcs 执行的进程内缝（port 流量词汇驻 core） | `crates/core/workflow/src/write/worktree.rs`（新） | crate 内 std | sync trait，六方法（D1） |
| create 建域组合 | 建域四段编排：前置校验 → 建档 → worktree add → worktree 内目录树 + explore.md → bootstrap / 警告 | `crates/core/workflow/src/write/create.rs` | `state`、`worktree`、foundation::layout | sync 零 tokio；补偿链（D3） |
| archive merge-first 前置 | 带 worktree 记录且主仓两树未命中 → 引导 merge 的显式拒绝 | `crates/core/workflow/src/write/archive.rs` | `state`、foundation::layout | 既有双写零改动，仅前置分支 |
| 状态缝字段面 | `ChangeStateRecord` 增 worktree / base_commit（执行锚引用） | `crates/core/workflow/src/state.rs` | serde / specta | 中性快照扩两 Option 字段 |
| ChangeRecord v2 | 持久化形状升级（native_model v1→v2 decode-only） | `crates/infra/store/src/model.rs` | native_model | id=9 version=2 `from = ChangeRecordV1`（D7） |
| store 映射与 db 文件名委托 | `workspace_db_file_name` 改消费 foundation 单点（行为零变化）；change 域记录 ↔ 中性类型映射扩双字段 | `crates/infra/store/src/store.rs` | foundation | 派生算法迁出后 store 内零第二份实现 |
| change 目录解析单点 | `locate_change` 增 worktree 路径回退（第四级） | `crates/core/workflow/src/queries/mod.rs` | foundation::layout | 纯路径推导（D12） |
| 详情聚合 | `ChangeDetail` 增 `worktree`（None → null）；建档记录恒可达（定位 miss → 产物空 + 状态面在） | `crates/core/workflow/src/queries/detail.rs` | `state`、artifacts | 纯 derive DTO（golden 显式重写） |
| 列表归组 | `db_entry` 归组改 db status 权威（D11 语义修订） | `crates/core/workflow/src/queries/list.rs` | `state` | 磁盘目录仅决定目录名取位 |
| compose 拆参 | store 半边改命令层注入（B 案）；cwd 半边经既有 turn 通道 | `crates/infra/agent/src/compose.rs` | store、agent | `for_root` 调用自 compose 删除（D8） |
| run 控制复合键 | 注册表键 `(workspace root, change)`；DTO 零改动 | `crates/core/orchestration/src/control.rs` | tokio sync 原语 | HashMap<(String, String), RunEntry>（D10） |
| 发起装配（exec root） | 前置校验后解析 exec root；compose 注入 workspace root 实例；`RunRequest.root` 恒 exec root | `src/commands/change_flow/mod.rs` | agent-runtime、vcs 不涉 | 三件事纪律（D9） |
| change 域命令装配 | `create_change` 装配（data_root + worktree_dir + ProcessWorktree + spawn_blocking）；detail / read_artifact worktree 感知 | `src/commands/changes/mod.rs` | vcs-runtime、workflow | IPC 签名不变，DTO 演进（D2） |
| 数据根注入 | `app.manage(data_root)`（worktrees 落位派生的注入面，沿 store 注入式打开纪律） | `src/main.rs` | — | Tauri State |
| create 对话框成功面 | worktree 路径 + 警告清单行内呈现（D14） | `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | bindings | React |
| 详情 worktree 信息行 | 非 null 渲染路径（legacy 不渲染） | `packages/desktop/src/views/changes/change-detail-view.tsx` | bindings | React |
| 类型跟随 | DTO 演进重导出 | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | specta 生成物 |

**不变组件（零触点核对结论）**：`crates/core/orchestration/src/walker.rs` / `steps.rs` / `snapshot.rs`（`RunRequest.root` 语义换 exec root，但载体、装配形态与代码零改动——walker 对 root 透明：`WorkerTurnRequest.root` / `ToolStepRequest.root` / 快照 `detail(&request.root, ..)` 天然随 exec root 落位）；相位表与 prompt 模板（`phase_table.rs`，交接行经相对通道自然读 worktree 内 explore.md）；相位机其余写操作（phase_next / phase_start / phase_log / backtrack / decision_log——状态机零 fs 原地不动）；SDK 路径沙箱（`sdk/sandbox.rs` 机制零改动，锚随 cwd 自然切换）；`SessionRecord` / 会话转录 / resume 链（记录不存 cwd）；`crates/infra/watch/`；`agent_start` 命令语义（仅机械适配 compose 签名）；`plugins/dev-team` 全部；`openspec/specs/**` 基线。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | WorktreePort 形态与方法粒度 | trait `WorktreePort` 驻 `core/workflow/src/write/worktree.rs`（port 属消费者），**六方法细粒度**：`probe(main_root) -> RepoProbe{head, dirty}`（git 可用 + git 仓 + HEAD + 脏仓单点三连）、`branch_exists(main_root, branch)`、`add_worktree(main_root, worktree, branch)`、`remove_worktree(main_root, worktree)`（补偿面）、`delete_branch(main_root, branch)`（补偿面）、`run_install(worktree, command) -> InstallRun{success, summary}`；port 流量类型（`RepoProbe` / `InstallRun`）驻 core；sync 签名零 tokio（写面 sync 纪律） | 细粒度使 create 编排（顺序 / 补偿 / 警告收口）留在 core 单点、vcs 只做进程执行——与 checks 先例（`StaticCheckRunner` / `TestExecutionRunner` 留 orchestration、spawn 落 infra/checks）同构；建域单方法会把补偿编排推进 infra，port 失去可测性 |
| D2 | 写面 `create` 与命令面签名演进 | 写面：`create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name, goal)`——双根注入，两棵 `Layout` 在写面内经 `foundation::layout::resolve` 自铸（Layout 纪律不变：layout 单参数不足以表达双根）。命令面：`create_change` 转 **async** + `create_change_with<R: Runtime>` 泛型测试缝（沿 `archive_change_with` 先例），经 `tauri::async_runtime::spawn_blocking` 调 sync 写面；IPC 入参与返回类型面不变（DTO 演进除外） | 同步命令在 Tauri 主线程执行，bootstrap 是分钟级 spawn——async + spawn_blocking 使 UI 不冻结；写面本体保持 sync 零 tokio 纪律 |
| D3 | create 四段顺序、拒绝面与补偿链 | **前置七道（全零副作用）**：① 名称 kebab-case + ≤128 ② goal 非空白 ③ 主仓 active 目录已存在（`resolve(main_root).changes_root/<name>`）④ db 同名 active 记录 ⑤ `vcs.probe`——git 不可发现 / 主仓非 git 仓 / 空仓无 HEAD 均显式 `Err` 引导（MUST NOT 静默回退主 root 创建）⑥ branch `change/<name>` 已存在 ⑦ worktree 目标目录已存在。**执行段**：建档先行（`ChangeStateRecord` 携 `worktree=Some(绝对路径串)` / `base_commit=Some(probe.head)`）→ `add_worktree`（HEAD 基线）→ worktree 内目录树 + explore.md → 脏仓警告 → bootstrap。**补偿**：add 失败 → 删本次建档 + 尽力 `delete_branch`；目录树 / explore.md 失败 → `remove_worktree --force` → `delete_branch` → 删本次建档（尽力链，任一失败 `Err` 呈现残留对象与 `git worktree list` 手动清理指引）；**bootstrap 段失败不回收**（spec 拍板） | 「目录在而记录缺」被禁破口保持建档先行；worktree add 会铸 branch，补偿链必须连分支一并尽力回收，否则下次同名创建撞 branch 前置（R4 兜底仍为显式报错指引） |
| D4 | bootstrap 映射表 / 超时 / 环境面 / Cargo 无 lock | 映射表（驻 create.rs 私有常量，首匹配序，worktree 根探测）：`pnpm-lock.yaml → pnpm install --frozen-lockfile`、`package-lock.json → npm ci`、`yarn.lock → yarn install --frozen-lockfile`、`Cargo.lock → cargo fetch --locked`。无已知 lockfile → 跳过 + 注记「未识别依赖管理器」；**Cargo.toml 在场而 Cargo.lock 缺席 → 跳过 + 专项注记**（无 --locked 的 fetch 会铸出未跟踪 Cargo.lock，污染本 change 的 diff 面）。`run_install`：shell 包装（Windows `cmd /C`、其余 `sh -c`，static_check 先例）、cwd = worktree、**环境全继承零注入**（PATH 为 pnpm/npm/cargo 可达前提）、**超时 900 s**（vcs-runtime 常量，轮询 try_wait 到点 kill → `Err`）、输出摘要 = stdout+stderr 尾部 ≤300 字。共享 `CARGO_TARGET_DIR` 等逃生口不内建机制——文档化位置即 spec「V1 范围与边界留痕」第 5 条与 explore R1（本设计引用，不另立文档） | cargo fetch --locked 只暖 registry 缓存不改工作区文件，构建成本留给检查步（R1 接受）；冻结锁面命令保证 bootstrap 本身零 diff 污染 |
| D5 | 警告词汇表（文案单点） | `CreateOutcome.warnings: Vec<String>`，文案铸造收 create.rs 单点（测试断言锚）：脏仓 `主仓有未提交改动（worktree 基线仍取 HEAD，未提交内容不会进入本 change）：建议先提交再开新 change`；未知管理器 `未识别依赖管理器，跳过依赖引导`；Cargo 无 lock `检测到 Cargo.toml 但无 Cargo.lock，跳过依赖引导（避免生成未跟踪 lockfile 混入变更上下文）`；安装非零退出 `依赖引导失败（{command}）: {summary}`；拉起 / 超时 `Err` 面 `依赖引导未执行成功（{command}）: {error}`。干净仓且顺利 → 空清单 | 警告须持久入 DTO 抵达前端行内呈现（D2 拍板）；文案单点使 golden / 断言不漂移 |
| D6 | 身份段单点与 worktree 落位派生 | `foundation::identity::workspace_identity_segment(canonical_root: &str) -> String`（`{可读段}-{sha256 前 16 字节 32 位小写 hex}`；可读段清洗 / ≤24 截断 / 去尾 `.` 与空格 / 空回退纯哈希——算法与参数自 store.rs 逐字平移）；`workspace_db_file_name` 改为 `format!("{}.redb", segment)`（行为零变化，store 内派生实现删除；`dir_name` 留守 store——`WorkspaceRecord` 展示名仍消费，非身份段派生成分）。worktree 落位派生单点 = vcs-runtime `worktree_dir(data_root, workspace_root, change)` = `data_root/worktrees/{segment}/<change>`（`WORKTREES_DIR_NAME` 常量驻 vcs）。**canonical 锚口径**：segment 以命令入参 root（前端清单回传的 canonical root 契约）为锚，与 `for_root` 的 canonical key 同信任级别，不重复 canonicalize（控制面 / 派生面纯字符串，无路径 IO） | spec 拍板「同源单点 + 第二消费者出现才下沉」正触发；落位派生收 vcs 使 desktop-app 不自拼 `worktrees` 目录名（目录名单点哲学与 layout 常量组一致） |
| D7 | ChangeRecord v1→v2 | 现 `ChangeRecord` 整体改名 `ChangeRecordV1`（`#[native_model(id = 9, version = 1)]`，`pub(crate)`，不入模型组）；新 `ChangeRecord` = `#[native_model(id = 9, version = 2, from = ChangeRecordV1)]` 增 `worktree: Option<String>` / `base_commit: Option<String>`（`#[serde(default)]`）；升级 `From`：两字段 `None`（legacy 主 root 语义）；降级 `From`：丢弃占位（降级形态不作数据承诺，provider `context_length` 先例同模式）；`ChangeRecord::new` 增两 Option 参 | decode-only 零迁移；envelope API（`list_models` / `scan`）经 serde 自动覆盖新字段（null 出线） |
| D8 | compose_turn 拆参 | B 案落地签名：`compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, store: Arc<Store>, agent: Option<i64>)`——store 半边 = 命令层预解析注入的 workspace root 库实例（`stores` 仍传入，仅剩 global 半边解析消费）；`for_root` 调用自 compose 删除；**compose 不再收 root**（cwd 半边本就经 `SessionCtx.workspace_root` 通道，不经 compose）。消费方两处：`change_flow_start`（store = workspace root 实例；cwd = exec root，由 turn 通道携带）与 `agent_start`（store = `for_root(&root)`；cwd 仍主 root，D7 拍板）——均机械适配 | 组合根已在命令层 `for_root` 一次（缓存命中无害）；worktree 路径自此在类型上不可能进 `for_root`（store 半边唯一入口是注入） |
| D9 | exec root 解析与发起前置 | `change_flow_start` 在建档 / workflow_type 校验后解析：`record.worktree` 非 None → 该绝对路径为 exec root，并做 **is_dir 存在性前置校验**（缺失 → 显式 `Err`「worktree 目录不存在（可能已被手动删除）」引导；仅对带 worktree 记录生效，legacy 路径零变化）；None → 主 workspace root。`RunRequest.root` / `StoreSnapshot::new` 恒 exec root；`compose_turn` 注入 workspace root 实例；`LocalToolSteps` / `GitDiffSource` 装配不变（root 透明） | worktree 被手动删除后，深埋在引擎 cwd / 检查步的失败不如发起点一行显式 Err 可排查；spec 发起前置校验清单为最低覆盖（SHALL 覆盖），非穷尽 |
| D10 | begin_run 复合键形态 | `ChangeFlowControl` 注册表键改 `(String, String)` = (workspace root, change)；`begin_run(root, change, run_id)` 及 `subscribe` / `request_stop` / `current_session` / `publish` / `set_session` / `answer` / `confirm` / `snapshot` 签名均增 root 段；`RunGuard` 持复合键；`ChangeFlowSink` 增 root 字段；`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动。root 段口径同 D6（命令入参 canonical root 契约，进程内无 IO） | 修掉两 workspace 同名假冲突先例 bug；五条控制命令（stop / answer / confirm / state / watch）IPC 签名已有 root，控制面调用机械传参 |
| D11 | 列表归组语义修订 | `db_entry` 归组一律以 `record.status` 权威：active → 进行中组、archived → 归档组；磁盘目录存在性仅决定**目录名取位**（active 主仓命中 → 建档名；archived → archive 树精确 / 日期前缀名；未命中 → 建档名）与产物解析定位。这是对 desktop-workflow-db-state design D7「读时以磁盘事实归组」的**显式修订**：此前「db-active + 目录被外部移入 archive 树」条目随磁盘归 archive 组，现随 db 留 active 组 | spec 授权（desktop-change-queries：db status 权威归组，worktree 条目不因主仓目录缺席被丢弃 / 误归未知组）；归组单口径消歧——db status 是桌面写面唯一事实，磁盘只承载名字与产物 |
| D12 | locate_change 回退与 detail 可达性（worktree 删除后呈现形态） | `locate_change(layout, worktree: Option<&str>, name)` 解析链：主仓 active 精确 → archive 精确 → archive 日期前缀后缀匹配 → **worktree 回退**（`resolve(worktree).changes_root/<name>` is_dir → `ChangeSource::Active`）。`change_detail` 改 record 先读后定位（worktree 自记录直传）；**建档记录恒可达详情**：定位全 miss（worktree 被删、未 merge）→ dir 缺席、产物清单空、状态面在（`source` 自 record.status 映射）；record 与定位双缺 → `None`（文档形态未知名，既有语义）。**无新增 worktree 缺失提示字段**——呈现形态即「状态面在 + 空产物占位」，发起时 D9 存在性校验承担显式引导 | merge-first 语义下 worktree change 在 merge 前主仓必然两树未命中，detail / read_artifact 必须经记录回退可达；wire contract 已因 `worktree` 字段重写一次，不为 V1 低频角落（手动删 worktree）再扩提示字段 |
| D13 | 归档引导分支 | `archive` 写面在既有「active 命中常规双写 / archive 命中续半边」之后的未命中 `Err` 分支前插入：`record.worktree.is_some()` → `Err("change \"{name}\" 的目录未在主仓出现（active / archive 两树未命中）：请先将 worktree 分支 change/{name} merge 回主仓，再发起归档")`；merge 后主仓目录在场，既有双写零特判。归档不触碰 worktree / branch（清理为手动边界） | spec 拍板 merge-first；优于泛化「目录未找到」的可排查性 |
| D14 | 前端可见性形态 | create 对话框：提交成功后**成功面替换表单**——名称、worktree 绝对路径（break-all）、警告清单（有则行内逐条）、「进入详情」按钮（调 `onCreated`）；toggle 收起重开即重置（CreateForm 卸载重建）。详情页：`DetailHeader` 增 worktree 信息行（`detail.worktree !== null` 渲染，break-all；legacy null 不渲染） | 路径藏于 home 数据目录下的可见性补偿；不自动导航（成功面须可读）；无新组件、无新路由 |
| D15 | 版本交付基准 | `packages/desktop/package.json` 0.4.16 → **0.4.17**（spec 数值 0.4.14 为提案快照；按「用户可见新功能升一位」语义以实施时现状为基递增，不降版；`tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动） | 版本单调性优先；spec requirement 的语义（升一位）逐字保留 |
| D16 | golden 显式重写范围清单 | ① 全部既有 detail golden：`worktree` 键全量新增（存量建档样本 → `null`）；② 新增 worktree 两态建档语料样本（带 `worktree` / `base_commit` 与 `None` 各一）→ 新 golden 文件；③ `corpus-list-mixed` 快照可能因 D11 归组修订位移（db-active + 磁盘 archive 条目改归进行中组）——重写时人工确认该差异属预期语义修订；④ `CreateOutcome` 断言面（测试侧随写）；⑤ `bindings.ts` 重导出。执行：`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕（test-design / test-gen 阶段承接，本设计留范围清单） | wire contract 冻结约束的显式重写流程逐字执行（AC-9） |

---

## 数据模型

### 身份段（foundation 单点，db 文件名与 worktree 子目录同源）

```
workspace_identity_segment(canonical_root) =
  "{可读段}-{hash}"  或（可读段清洗后为空）"{hash}"
  hash   = SHA-256(canonical_root UTF-8 字节) 前 16 字节的 32 位小写 hex
  可读段 = canonical_root 末段目录名 → 非法字符（/ \ : * ? " < > | 与控制符）置换 `_`
           → 按 char 截断 ≤24 → 去尾部 `.` 与空格；空则回退纯哈希

workspace_db_file_name(canonical_root) = segment + ".redb"   （store 消费，行为零变化）
worktree_dir(data_root, workspace_root, change) =
  data_root / "worktrees" / segment(workspace_root) / change  （vcs-runtime 消费）
```

### ChangeRecord v2（native_model id=9，version 2，decode-only）

| 字段 | 类型 | 语义 |
|------|------|------|
| `name`（主键）… `active_phase` | 既有六字段 | 与 v1 逐字一致 |
| `worktree` | `Option<String>` | 该 change 分配的 worktree 绝对路径；`None` = legacy 主 root change（存量记录自动升级读出） |
| `base_commit` | `Option<String>` | 创建基线 fork 点（主仓 HEAD），调试 / UI 价值 |

`ChangeStateRecord`（core 中性快照）同步增两 Option 字段（serde camelCase：`worktree` / `baseCommit`）；两字段为**执行锚引用**，MUST NOT 反向参与库身份派生（`for_root` 恒以 workspace root 为锚——双 root 不变量）。

### DTO 演进（wire contract 冻结 → golden 显式重写）

| DTO | 字段 | 出线 |
|-----|------|------|
| `CreateOutcome` | `name` / `created`（既有）+ `worktree: String` + `warnings: Vec<String>` | 恰四字段面；worktree 绝对路径为刻意出线的执行锚（路径不下沉条款修订） |
| `ChangeDetail` | 既有字段 + `worktree: Option<String>` | 自 `ChangeStateRecord.worktree` 直读透出，None → null，纯 derive 零改写 |

### WorktreePort 流量类型（core/workflow）

```rust
pub struct RepoProbe  { pub head: String, pub dirty: bool }      // probe 产出：基线 + 脏仓
pub struct InstallRun { pub success: bool, pub summary: String } // run_install 产出：退出态 + 尾部 ≤300 字摘要
```

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  公共函数仅列模块级导出函数 / trait 方法 / Tauri 命令；私有函数不列入。
  测试文件由 test-design / test-gen 阶段承接，不入本清单。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/foundation/src/identity.rs` | `workspace_identity_segment` 纯函数（算法自 store.rs 逐字平移；sha2 摘要段） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/worktree.rs` | `WorktreePort` trait（六方法）+ `RepoProbe` / `InstallRun` port 流量类型 |
| `packages/desktop/src-tauri/crates/infra/vcs/Cargo.toml` | 新 crate（裸名 `vcs-runtime`）：依赖 `workflow` + `foundation`，零 tokio 零 Tauri |
| `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | `ProcessWorktree`（`impl WorktreePort` 装配入口）、`WORKTREES_DIR_NAME` 常量、`worktree_dir` 落位派生单点 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` | git 子命令族同步执行（`git -C`：rev-parse × 2 / status --porcelain / show-ref --verify / worktree add -b / worktree remove --force / branch -D）；git 拉起失败 → 「git 不可用」引导文案 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/bootstrap.rs` | `run_install` 执行体：shell 包装（cmd /C \| sh -c）、cwd = worktree、环境全继承、900 s 超时轮询 kill、输出尾部 ≤300 字摘要 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs` | 挂 `pub mod identity`；crate 文档注释更新（layout 解析 + workspace 身份段派生两能力，「刻意极小」口径） | crate 门面 |
| `packages/desktop/src-tauri/crates/core/foundation/Cargo.toml` | `[dependencies]` 增 `sha2` | 身份段哈希成分 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `ChangeRecordV1`（v1 历史形态，decode-only）+ `ChangeRecord` v2（worktree / base_commit + 双向 From）；`ChangeRecord::new` 增两参 | D7 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `workspace_db_file_name` 改委托 foundation 单点；`readable_segment` / `is_illegal_name_char` / `READABLE_SEGMENT_MAX_CHARS` 删除（`dir_name` 留守展示名消费）；`create_change_record` / `change_state` 映射扩双字段 | 行为零变化（同名回归） |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | `[dependencies]` 增 `foundation` | 依赖规则第二例修订落痕 |
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `ChangeStateRecord` 增 `worktree` / `base_commit` 两 Option 字段（执行锚引用注释） | 中性快照字段面 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 挂 `mod worktree` 并导出 `WorktreePort` / `RepoProbe` / `InstallRun` | 写面门面 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 建域扩展（D2 签名 / D3 顺序与补偿 / D4 映射表 / D5 警告单点）；`CreateOutcome` 增 `worktree: String` / `warnings: Vec<String>` | sync 纪律与 Layout 路径纪律保持 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs` | 未命中分支前插入 worktree merge-first 引导拒绝（D13） | 既有双写零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs` | `locate_change` 增 `worktree: Option<&str>` 参与第四级回退（D12） | 目录解析单点收口 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | record 先读后定位；建档记录恒可达（定位 miss → 产物空 + 状态面在）；`ChangeDetail` 增 `worktree` | golden 显式重写 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | `db_entry` 归组改 `record.status` 权威；磁盘目录仅决定目录名取位（D11） | 语义修订留痕 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` | 注册表键 `(String, String)` 复合；九方法签名增 root 段；`RunGuard` 持复合键；DTO 零改动 | D10 |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | `compose_turn` 拆参（store 注入，root 参删除，`for_root` 调用移除） | D8 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | exec root 解析 + worktree 存在性校验（D9）；compose 注入 workspace root 实例；`begin_run` / `subscribe` / sink 桥复合键；`RunRequest.root` / `StoreSnapshot::new` 恒 exec root；stop / answer / confirm / state / watch 控制面调用复合键（IPC 签名不变） | 发起装配 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_start` 机械适配 compose 新签名（store = `for_root(&root)`，会话仍主 root） | D7 拍板 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `create_change` 转 async + `create_change_with` 泛型缝（D2）：装配 `worktree_dir`（`State<PathBuf>` 数据根）+ `ProcessWorktree` + `spawn_blocking` 调写面；`get_change_detail` / `read_artifact` 读 record.worktree 传 locate 回退 | IPC 签面不变 |
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace] members` 增 `crates/infra/vcs`；`[workspace.dependencies]` 增 `vcs-runtime`；根包 `[dependencies]` 增 `vcs-runtime` | crate 图 |
| `packages/desktop/src-tauri/src/main.rs` | setup 增 `app.manage(data_root.clone())`（PathBuf 状态）+ 注释 | 数据根注入面 |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | 成功面（worktree 路径 + 警告清单 + 进入详情按钮，D14） | 行内呈现 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `DetailHeader` 增 worktree 信息行（非 null 渲染） | legacy 不渲染 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `bindings:export` 重导出（`CreateOutcome` 两字段 + `ChangeDetail.worktree`） | 一致性守卫 |
| `packages/desktop/package.json` | `version` 0.4.16 → 0.4.17（D15） | `tauri.conf.json` 自动跟随 |

### 删除文件

无。

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `workspace_identity_segment` | `crates/core/foundation/src/identity.rs` | 新增 | `pub fn workspace_identity_segment(canonical_root: &str) -> String` | 身份段单点（db 文件名与 worktree 子目录同源） |
| `WorktreePort`（trait） | `crates/core/workflow/src/write/worktree.rs` | 新增 | `pub trait WorktreePort: Send + Sync` | 六方法见下 |
| `WorktreePort::probe` | 同上 | 新增 | `fn probe(&self, main_root: &Path) -> Result<RepoProbe, String>` | git 可用 + git 仓 + HEAD + 脏仓三连探测 |
| `WorktreePort::branch_exists` | 同上 | 新增 | `fn branch_exists(&self, main_root: &Path, branch: &str) -> Result<bool, String>` | branch 冲突前置 |
| `WorktreePort::add_worktree` | 同上 | 新增 | `fn add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String>` | HEAD 基线建域 + 铸分支 |
| `WorktreePort::remove_worktree` | 同上 | 新增 | `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` | 补偿面（--force） |
| `WorktreePort::delete_branch` | 同上 | 新增 | `fn delete_branch(&self, main_root: &Path, branch: &str) -> Result<(), String>` | 补偿面（-D） |
| `WorktreePort::run_install` | 同上 | 新增 | `fn run_install(&self, worktree: &Path, command: &str) -> Result<InstallRun, String>` | bootstrap spawn（cwd = worktree） |
| `create` | `crates/core/workflow/src/write/create.rs` | 修改 | `pub fn create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str) -> Result<CreateOutcome, String>` | 建域四段组合（D2/D3） |
| `locate_change` | `crates/core/workflow/src/queries/mod.rs` | 修改 | `pub fn locate_change(layout: &Layout, worktree: Option<&str>, name: &str) -> Option<ChangeLocation>` | 第四级 worktree 回退 |
| `compose_turn` | `crates/infra/agent/src/compose.rs` | 修改 | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>` | store 注入化拆参（D8） |
| `ChangeFlowControl::begin_run` | `crates/core/orchestration/src/control.rs` | 修改 | `pub fn begin_run(self: &Arc<Self>, root: &str, change: &str, run_id: String) -> Result<RunGuard, String>` | 复合键；`subscribe` 等八方法同增 root 段 |
| `ProcessWorktree` | `crates/infra/vcs/src/lib.rs` | 新增 | `pub struct ProcessWorktree`（`impl WorktreePort`；`new()` / `Default`） | vcs 边界执行体（无状态，组合根按需构造） |
| `worktree_dir` | `crates/infra/vcs/src/lib.rs` | 新增 | `pub fn worktree_dir(data_root: &Path, workspace_root: &str, change: &str) -> PathBuf` | worktree 落位派生单点（D6） |
| `create_change` | `src/commands/changes/mod.rs` | 修改 | `pub async fn create_change(app: AppHandle, root: String, name: String, goal: String) -> Result<CreateOutcome, String>` | async + spawn_blocking（IPC 面不变；`create_change_with<R>` 泛型缝） |

---

## 验收锚点（AC → 组件 / 决策对账）

| AC | 锚点 |
|----|------|
| AC-1 | D7（v1→v2 decode-only）+ store.rs 委托（D6，db 文件名同名回归） |
| AC-2 | D2 / D3（建域四段）+ D6（落位派生） |
| AC-3 | D3 前置七道（git 可用性 MUST NOT 静默回退） |
| AC-4 | D3 脏仓警告 + D5 文案 |
| AC-5 | D4（映射表 / 超时 / 环境面）+ D5（警告词汇） |
| AC-6 | D8（compose 注入）+ D9（exec root；store 身份恒 workspace root） |
| AC-7 | D10（复合键） |
| AC-8 | D9（None → 主 root，legacy 零变化）+ D11 / D12 |
| AC-9 | D12（detail 出线 + 产物回退）+ D14（UI）+ D16（golden 重写范围） |
| AC-10 | D13（归档引导）+ D15（0.4.17）+ 守线（plugins 零改动 / 全绿） |
| AC-11 | 阶段一 crate 图（vcs-runtime 依赖白名单；身份段仅 foundation 一份）。注：store 的 `workflow` 依赖为 desktop-workflow-db-state 既成中性类型边（port 词汇），spec「`agent` + `foundation`」按本变更新增面读——守线留痕 |

依赖白名单注记（AC-11 口径）：本变更新增的 workspace 内边恰两条——`store → foundation`（身份段纯函数）与 `vcs-runtime → workflow + foundation`、`desktop-app → vcs-runtime`（装配注入）；既有 `store → workflow` 边不在本变更接触面内。
