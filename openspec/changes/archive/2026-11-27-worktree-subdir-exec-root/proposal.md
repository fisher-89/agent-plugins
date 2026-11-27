# 提案: worktree-subdir-exec-root

> **变更**: worktree-subdir-exec-root（需求原句：新建变更时，若基线 root 属于 git 某个子目录，切换到 worktree 也要将对应子目录作为 root）
> **日期**: 2026-11-27
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（`workflow_type = requirement`；§1–§6 为裁定依据与留痕台账）
> **性质**: **一次字段语义拆分 + 读面纠正**——`ChangeRecord.worktree` 保持「git 落位目录（host）」单义，新增 `exec_root` 承担「change 工作面根」；子目录 workspace 的 change 从此在 worktree 内的对应子目录建档 / 执行 / 检查 / 装依赖，而 git 步与 worktree 回收仍锚整棵检出。
> **不含**: `rel` 纳入落位派生（D7 已否决，见「风险 R-7」）；上游锁面逃逸（放开 `projects[].root` 的 `..` / 绝对路径）；未跟踪资产复制进 worktree（V1 边界第 4 条）；同名 change 的命名隔离机制；rich 档冲突处置；`agent_start` 与 explore 笔记的执行根（边界 3 原样保留）。

---

## 问题

### 一 · 一个字段同时承担两种语义，子目录 workspace 下首次分叉即错位

`ChangeRecord.worktree` 一个字段今天被两处消费，语义不同：

```
① git 落位目录（可 remove / 可在场判定 / 对得上 git worktree list）
② change 工作面根（会话 cwd / Layout / 检查器 / 沙箱 / 画布 / artifact 定位）
```

git 根 workspace（`rel = ""`）下两者恒等，故分裂不可见。workspace root 是仓库子目录时，实测错位：

```
<repo 父>/.worktrees/repo/<name>/        ← exec root = 整棵检出根（错）
├─ openspec/changes/<name>/explore.md    ← create 真写在这（错位）
└─ packages/desktop/openspec/changes/    ← 主仓 layout 认得的位置（空）
```

| # | 破口 | 位置 |
| --- | --- | --- |
| 1 | 产物落在检出根；merge 后出现在仓库根 `openspec/`，主仓 `<subdir>/openspec` 看不到 | `write/create.rs` 的 `resolve(&worktree)` |
| 2 | 相位提示词的 `openspec/changes/…` 相对路径据此写错位（cwd = exec root） | 同上 |
| 3 | archi 双根错位：`<subdir>` 基线 vs 检出根改动侧 | `src/commands/archi/mod.rs:135-146` |
| 4 | bootstrap 装到检出根；`projects[].root` / `tests[].root` / `static_analysis` 相对路径同理错位 | `write/create.rs`、`testexec/mod.rs:669` |
| 5 | SDK 沙箱锚 = 检出根 → 子目录 workspace 的 change 反而能改全仓 | 会话 cwd 通道 |

### 二 · git 在子目录下的作用域实测（决定改造面：哪些步**不能**跟着 exec root 走）

| 命令（`git -C <subdir>`） | 子目录下 | 影响 |
| --- | --- | --- |
| `status --porcelain` | 全树，路径相对 repo 根 | `dirty()` 可留 exec root 也可锚 host，无差 |
| `diff --name-only <base>` | 全树 | 编辑集 / 补丁不变 |
| `add -A` | 全树 | `commit_all` 不变 |
| `ls-files -s -z` | **仅 cwd 子树且路径相对 cwd** | `worktree_snapshot` 的 index 缩域 → 归档冲突后验 A/B 失真 |
| `clean -fd` | **仅 cwd 子树** | `restore_baseline` 半域风险（`reset --hard` 全树 + `clean -fd` 子树） |
| `worktree add/remove <abs>` | 正常 | 落位 / 回收不受影响 |
| `rev-parse --show-prefix` | 直接给偏移（根为空行、尾带 `/`） | 偏移的干净来源 |

结论：**git 步必须锚 host 全树**，exec root 只接 cwd / Layout / 检查器 / bootstrap。半域 clean 与缩域 index 都是「看起来正常、事后失真」的失败形态。

### 三 · 一句话结论

**这不是「让 worktree 落在子目录」，而是把执行根从「落位」里拆出来**：落位恒 `…/<repo>/<name>`（与 workspace 在仓库内的位置无关），执行根 = `落位/<rel>`；`rel` 只参与执行根派生与准入判据。

---

## 提案

### 一 · 总图

```
workspace root（用户注册）      = <repo>/<rel>           ← store 身份锚（for_root）不变
  │
  ├─ host（git 落位目录）        = <.worktrees>/<repo>/<name>       ← ChangeRecord.worktree（原样）
  │     用于：add/remove_worktree / 在场判定 / restore_baseline / worktree_snapshot /
  │           dirty / commit_all / rebase / ff_merge / branch 探测 / 对账
  │
  └─ exec root（执行根）         = host/<rel>（rel = "" 时 = host） ← ChangeRecord.exec_root（新列）
        用于：会话 cwd / Layout::resolve / artifact 定位 / 检查器 spawn cwd /
             bootstrap 扫描锚 / archi 改动侧 / SDK 沙箱前缀
```

核心不变量（写进规格正文）：

```
exec root ≡ workspace root 在 worktree 内的对应物
⇒ 任何「相对 workspace root」的声明，在主仓与 worktree 两侧同解
```

仓内四个既有声明因此**全部自动正确**、无需逐个裁决：`projects[].root`、`tests[].root`、`static_analysis` 相对路径命令、`write_protection.files[].glob`（装配根本就是主仓 workspace root）。

### 二 · 决策台账

| # | 决策 | 内容 |
| --- | --- | --- |
| **D1** | 记录承载 | `worktree` 仍是 host；新增 `exec_root: Option<String>`（绝对路径）。**`None` = 与 `worktree` 同值**（legacy 记录 + 根 workspace 同形） |
| **D2** | DTO 出线 | `ChangeDetail` / `ArchivePreflight` / `CreateOutcome` 各加 `execRoot`（无 worktree / 根 workspace → `null`）；`worktree` 保留。展示面两行：`worktree：<host>` + （非空时）`执行根：<execRoot>` |
| **D3** | git 步锚点 | `restore_baseline` 锚 **host 全树**；`dirty` / `commit_all` / `rebase` / `ff_merge` / `worktree_snapshot` / `remove_worktree` 全锚 host。`RunRequest` 新增 `worktree: Option<String>`（host）——walker 的 git 步不再从 exec root 反推 |
| **D4** | 子目录不在检出 | **显式 Err**，两层：前置 `HEAD:<rel>` 判据（零副作用）+ 后置 `is_dir()` 断言（走既有补偿链）。**不回退**——回退后的 change 根还是错的，只是把失败推迟 |
| **D5** | 「相对 workspace root」的声明 | 基准改为 **exec root**；代码零改，改文档与文案措辞（`config.schema.json` 两处、`is_valid_project_root` 注释、`BootstrapProject.root` 文档与警告文案） |
| **D6** | 展示面 | 见 D2（两行、各自用途：开编辑器看改动 / 对上 `git worktree list`） |
| **D7** | 落位是否纳入 `rel` | **不纳入**（定案）：落位恒 `…/<repo>/<name>`。落位与 workspace 身份彻底解耦；代价见 R-7 |
| **D8** | 访问器与守卫 | `exec_root()` 单点（`workflow::state`）：`Some(e)` 且 `e` 是 `worktree` 的真后代 → `e`；否则 stderr 留痕 + 回落 `worktree`（陈旧行 / 手改库的失败形态退回「今天的错位」，不产生越界路径） |
| **D9** | 迁移 | `ChangeRecord` v4 → **v5**（`from = ChangeRecordV4`，当前形态抄为 `ChangeRecordV4`（`from = ChangeRecordV3`））；新列 `#[serde(default)] Option<String>`。**MUST NOT** bump `WORKSPACE_STORE_FORMAT_VERSION`（加列走 native_model 链；格式版本一动所有 workspace 库作废） |
| **D10** | 端口形态 | `WorktreePort::worktree_root` → `placement(main_root) -> WorktreePlacement { root, rel }`，加 `tracked_at_head(main_root, rel) -> bool`；偏移来源用 `rev-parse --show-prefix`（**不用** `strip_prefix`——canonicalize 的 `\\?\` 前缀 / 真实大小写 / `\` vs `/` 三个坑一次绕开） |

### 三 · 读写分工（最终归属）

| 语义 | 字段 | 消费点 |
| --- | --- | --- |
| **host** | `record.worktree`（原样） | `remove_worktree`（create 补偿 / delete / Cleanup 三处）、在场判定与 merge-first 判定、branch 探测、归档全部 git 步、`worktree_snapshot` |
| **exec root** | `record.exec_root`（回落 `worktree`） | `change_flow` 的 exec root 解析、detail `locate`、`read_artifact`、`change_dirs` 候选、archi 改动侧、归档前置载荷的 artifact / delta specs 探测、bootstrap、检查器 spawn |

`ArchivePreflight` 拆两列：`worktree`（host → walker git 步 + merge-first）+ `execRoot`（artifact 探测 + 前端预览）。

### 四 · create 时序（含新增两处）

```
前置①–⑥ → placement(main_root) → 前置⑨ 磁盘门槛
→ 【新增】tracked_at_head(rel) 判据（rel="" 恒 true，零 spawn）
→ host = placement.root.join(name)；exec_root = host.join(rel)
→ 建档（worktree = host；exec_root = rel 非空 ? Some(exec_root) : None）
→ add_worktree(main_root, &host, &branch)
→ 【新增】exec_root.is_dir() 断言，miss ⇒ 既有补偿链 + Err
→ resolve(&exec_root) 建 change 目录 + explore.md
→ bootstrap(exec_root, projects)
```

### 五 · 数据与接口（要点）

```
ChangeRecord { worktree: Option<String>   // 不变
               exec_root: Option<String>  // 新增；None = 同 worktree
               … }

WorktreePlacement { root: PathBuf /* <.worktrees>/<repo> */, rel: String /* `/` 分隔，根为 "" */ }
fn placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>;
fn tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>;
```

映射单点两处：`store.rs` 的 `change_state()`（记录 → 中性快照）与 `create_change_record`（中性快照 → 记录）；其余写路径（读记录 → 改一字段 → upsert）新列自动随行。

### 六 · 相对 explore 的两处扩充登记

| # | explore | 本提案 | 理由 |
| --- | --- | --- | --- |
| ① | §4.4 规格清单只列 5 个能力（worktree / state-store / archive / delete / orchestration） | **9 个**（另加 create / queries / change-chat / workspace-config） | 六处基线措辞与「worktree 即执行根」耦合，不改即漂移：`create_change` 的 `CreateOutcome` 字段集 scenario（「恰为五字段」）、create 的前置清单、detail DTO 的 `worktree` 定位语义、change-chat 的 `cwd = detail.worktree ?? root`、`projects[].root` 的「相对 worktree 根」、queries 列表段的「工件 / 产物解析经记录的 worktree 路径」。D2 与 D5 的语义本来就落在这些文件里，能力清单漏项会让 delta 只有签发方没有受让方 |
| ② | §3.2 把 `tracked` 放在 `WorktreePlacement.tracked` 或独立方法（二选一） | 取**独立方法** `tracked_at_head` | 判据与派生是两个关注点：`placement` 是纯派生（`rev-parse --show-toplevel --show-prefix` 一次调用），`tracked_at_head` 是第二个 git 询问（`rev-parse --verify --quiet HEAD:<rel>`）；合并进结构体让「根 workspace 零新增 spawn」这条不变量只能靠条件表达式维持，独立方法可以整段跳过 |

---

## 能力

### 新增能力

无。本变更改的是既有能力对「执行根」的定义与承载，拆成新能力会把同一条不变量变成两份持有者。

### 修改的能力

| 能力 | delta | 改什么 |
| --- | --- | --- |
| `desktop-change-worktree` | **ADDED ×1 + MODIFIED ×5** | ADDED「子目录 workspace 的执行根对齐（exec root = host/rel）」：核心不变量、`rel` 的来源与射程、`exec_root` 列语义、两层准入判据（前置 `HEAD:<rel>` + 后置 `is_dir`）、同仓同名拒绝（D7 守卫）、根 workspace 零差异断言。MODIFIED：①「双 root 执行语义与 db 身份不变量」——exec root 供应改为 `record.exec_root`（回落 `worktree`），补 host / exec root 分工与「子目录 workspace 同解」；②「create 分配 worktree 与分支基线」——新增前置判据与后置断言，`resolve` / `bootstrap` 改用 exec root；③「确定性依赖 bootstrap」——`projects[].root` 基准改为执行根；④「worktree 路径 UI 可见性」——两行展示；⑤「V1 范围与边界留痕」——第 4 条扩写为「未跟踪资产不随 worktree 复制」、新增「上游锁面不可表达」一条（八条 → 九条） |
| `desktop-change-state-store` | **ADDED ×1 + MODIFIED ×1** | ADDED「`ChangeRecord.exec_root` 执行根列与 native_model v5 升级链」：字段语义（`None` = 同 `worktree`）、访问器单点与守卫、v5 链（`from = ChangeRecordV4`）、`rel = ""` 不写新列、零格式版本 bump。MODIFIED「change 流程状态 workspace 库单源与双向墙」：`ChangeRecord` 字段清单补 `exec_root` 一条 |
| `desktop-change-orchestration` | **MODIFIED ×2** | ①「run 双 root 组合与 exec root 解析」——exec root 供应改 `record.exec_root`，补 `RunRequest.worktree`（host）与 legacy 判据显式化；②「initialize 基线检查相位执行体」——Clean 步锚 host 全树（`reset --hard` 全树 + `clean -fd` 子树 ⇒ 半域），legacy 短路判据换 `request.worktree.is_none()` |
| `desktop-change-archive` | **MODIFIED ×2** | ①「worktree 提交与主仓合入」——归档链 git 步锚 host、前置载荷双列（`worktree` / `execRoot`）、artifact 与 delta specs 探测走 execRoot；②「Cleanup 步与 worktree 自动清理」——回收锚 host（传 exec root 会「删子目录 + 留登记」） |
| `desktop-change-delete` | **MODIFIED ×1** | 「写面删除与执行序」——「同时清空 worktree」的回收锚 = host（记录 `worktree` 列），MUST NOT 用执行根 |
| `desktop-change-create` | **MODIFIED ×2** | ①「写面 create 操作进程内创建 change」——建档字段补 `exec_root`、前置校验补「子目录被跟踪」判据、目录树与 bootstrap 锚改执行根；②「create_change IPC 命令面」——`CreateOutcome` 五字段 → 六字段（+`execRoot`），字段集 scenario 随动 |
| `desktop-change-queries` | **MODIFIED ×2** | ①「change 详情聚合」——产物定位走执行根、DTO 增 `execRoot`（`worktree` 仍旧出线）；②「change 列表扫描与分组」——「其工件 / 产物解析经记录的 worktree 路径」一句改准为执行根（列表零代码改动，措辞随能力定义） |
| `desktop-change-chat` | **MODIFIED ×1** | 「change 环境边界（worktree 为 cwd）」——cwd 派生改 `detail.execRoot ?? detail.worktree ?? root`，提示面承认两行 |
| `desktop-workspace-config` | **MODIFIED ×1** | 「配置解析 core crate」——`projects[].root` 的相对基准由「worktree 根」改「执行根」（根 workspace 与子目录 workspace 同解） |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
| --- | --- |
| `desktop-workspace-store` | 库文件寻址仍以 workspace root 身份派生（`for_root` 红线），`exec_root` 是记录字段而非身份；`WORKSPACE_STORE_FORMAT_VERSION` 不动（D9）。**注意**：v5 链的语义落在 `desktop-change-state-store`（`ChangeRecord` 归该能力），本能力零改动 |
| `desktop-ipc-type-bindings` | 命令族、生成管线、golden 重写流程逐字不变；三个 DTO 的字段演进走既有「golden 显式重写」通道，不需要新纪律 |
| `desktop-app-shell` | 命令名、参数形态、id 寻址与薄包装纪律逐字不变；「worktree 路径 / name 分辨率单点收口」这句仍成立（执行根解析落在 core 侧的 queries / workflow，命令层不新增字段语义） |
| `desktop-change-flow-view` | 详情头 / 画布 / 预览面板的呈现契约不含路径派生规则（数据由 detail 与 preflight 出线）；`worktree 信息行照常呈现` 的既有 scenario 在追加一行后仍成立 |
| `desktop-artifact-plugins` / `desktop-file-read` | 产物发现只吃 Layout 的 changes_root，根从哪来到本变更无关 |
| `desktop-test-execution` / `desktop-checks-domain` | `tests[].root` / `static_analysis` 的相对基准本就是 run 的 exec root（`root_path.join(plan.root)`、检查器 spawn cwd），代码与契约零改 |
| `desktop-write-protection` | 装配根 = 主仓 workspace root，与执行根无耦合 |
| `desktop-archi-canvas` / `desktop-archi-gate` / `desktop-archi-check`（`desktop-archi-*` 族） | 画布「归档 change 取数以主仓根发起」的既有口径不变；archi 改动侧的根来自命令层解析（`archi_change_summary` 入参），门禁命令串逐字不变 |
| `repo-surface-scope` / `repo-archi-model` / `desktop-crate-layout` | 零新 crate、零依赖变化、零 `.dev-team/architecture/**` 改动 |
| `desktop-phase-prompts` | 提示词模板里的 `openspec/changes/…` 相对路径**无需改**——cwd 修正后同解（破口 2 的修法在根，不在文案） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
| --- | --- |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/worktree.rs` | trait：`worktree_root` → `placement(main_root) -> WorktreePlacement`；新增 `tracked_at_head(main_root, rel)`；`WorktreePlacement { root, rel }` 定义（`rel` 为 `/` 分隔、根为 `""`） |
| `.../crates/infra/vcs/src/lib.rs` | `ProcessWorktree::placement` / `tracked_at_head` 委托；`worktree_root` 公共单点退役或降为内部（`worktree_root_from_toplevel` 纯派生保留） |
| `.../crates/infra/vcs/src/git.rs` | `toplevel()` 扩为 `placement()`（一次 `rev-parse --show-toplevel --show-prefix`，解析两行）；新增 `tracked_at_head()`（`rev-parse --verify --quiet HEAD:<rel>`；`rel = ""` 直返 `true` 不 spawn）；`restore_baseline` / `worktree_snapshot` / `remove_worktree` 命令体**逐字不变**（只换调用点传的锚） |
| `.../crates/core/workflow/src/write/create.rs` | 用 `placement` 取 host 与 `rel`；新增前置判据（D4 第一层）；建档双写（`worktree` / `exec_root`）；`add_worktree` 目标仍 host；新增后置 `is_dir` 断言（miss ⇒ 既有 `compensate_tree_failure` + `Err`）；`resolve(&exec_root)` 与 `bootstrap(&exec_root, …)`；`bootstrap_worktree_root` → `bootstrap_exec_root`；`BootstrapProject.root` 文档 / 警告文案改「相对执行根」；`CreateOutcome` 加 `execRoot` |
| `.../crates/core/workflow/src/state.rs` | `ChangeStateRecord.exec_root` + `exec_root()` 访问器（含 D8 守卫，stderr 前缀 `[workflow]`） |
| `.../crates/core/orchestration/src/state.rs` | `ArchivePreflight.execRoot: Option<String>` |
| `.../crates/core/orchestration/src/steps.rs` | `run_archive_preflight` 填两列（`worktree` = 记录 host、`execRoot` = 记录 exec root）；`missing_artifacts` / `detect_delta_specs` 改吃 execRoot（签名加一个根参数） |
| `.../crates/core/orchestration/src/walker.rs` | `RunRequest.worktree: Option<String>`（host）；`step_clean` 的 `restore_baseline` 锚 `request.worktree`；两处 legacy 代理判据（initialize 短路 / Clean 双保险）换 `request.worktree.is_none()`；`archive_context` 返回 `(name, host)` 逐字不变（git 步零改） |
| `.../crates/core/workflow/src/queries/detail.rs` | `locate_change` 改吃 `record.exec_root()`；`ChangeDetail` 加 `execRoot` |
| `.../src/commands/change_flow/mod.rs` | exec root 解析改 `record.exec_root()`（`is_dir` 校验沿用）；`RunRequest` 补 `worktree`（host） |
| `.../src/commands/changes/mod.rs` | `read_artifact` 的 `locate_change` 回退参改执行根；`change_dirs` 候选改执行根（`push_worktree_candidate` 复用，探测序不变）；`CreateOutcome` 透传 `execRoot` |
| `.../src/commands/archi/mod.rs` | `archi_change_summary` 的改动侧改 `record.exec_root()`（archived 仍恒 `None`） |
| `.../crates/infra/store/src/model.rs` | `ChangeRecord` v4 → v5：当前形态抄为 `ChangeRecordV4`（`from = ChangeRecordV3`，V3 双向 `From` 保留），现役加 `#[serde(default)] exec_root`；`ChangeRecord::new` 签名加 `exec_root` |
| `.../crates/infra/store/src/store.rs` | `change_state()` 与 `create_change_record` 两处映射补新列（其余写路径零改） |
| `.../crates/core/config/config.schema.json` | `projects.description` 与 `projects.items.properties.root.description` 的「相对 worktree 根」改「相对执行根」；键集 / 值域 / 断言锚点零改动 |
| `.../crates/core/config/src/lib.rs` | `is_valid_project_root` 的注释理由改口径（「逃出 worktree」→「逃出执行根」，谓词零改） |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 头部两行（`worktree` + 条件 `执行根`）；画布 root 与 `ChangeChatPanel.cwd` 改 `execRoot ?? worktree` |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | cwd 派生改 `execRoot ?? worktree`（归档恒 `null` 语义不变） |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 新图预览根改 `preflight.execRoot ?? preflight.worktree ?? root` |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | 成功面路径行：`worktree` + 条件「执行根」两行 |
| `packages/desktop/src/types/generated/**` | 经 `bindings:export` 再生成（`execRoot` 字段进三个 DTO） |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` +1 patch（`tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） |

### 测试文件

| 文件 | 改动 |
| --- | --- |
| `.../crates/core/workflow/src/write/create_test.rs` | 假件 `WorktreePort` 同步两方法签名；新增：根 workspace 零差异（记录无 `exec_root`、零 `tracked_at_head` 调用）、子目录建域四件套、前置判据六形态、后置 miss 补偿、同仓同名拒绝 |
| `.../crates/core/workflow/src/write/{delete,promote}_test.rs`、`.../crates/core/workflow/tests/corpus_golden_test.rs` | 假件 `WorktreePort` 同步签名（断言意图零改） |
| `.../crates/infra/vcs/src/lib_test.rs` / `git_test.rs` | `placement()` 真件（根 workspace `rel = ""` / 子目录 `rel` 带 `/` 尾）、`tracked_at_head` 四态（在 HEAD / 未跟踪 / HEAD 已删 / 被 ignore）、`worktree_root` 三例随签名迁移 |
| `.../crates/core/workflow/src/state_test.rs` | `exec_root()` 访问器：`None` 回落、非空命中、越界 / 陈旧行回落 + 留痕 |
| `.../crates/infra/store/src/model_test.rs` | v4 → v5 往返、V4 行升级后 `exec_root = None`、元数据直证（照 `AgentProviderRecord` v4→v5 先例） |
| `.../crates/core/orchestration/src/steps_test.rs` / `walker_test.rs` | preflight 两列断言；`step_clean` 锚 host（子目录 workspace 夹具：`reset` 覆盖域外、`clean` 不缩域）；legacy 判据显式化 |
| `.../crates/core/workflow/src/queries/detail_test.rs` | 子目录 workspace 夹具：detail 命中 `exec_root` 下的产物、`execRoot` 出线、legacy → `null` |
| `.../src/commands/changes/mod_test.rs` / `archi/mod_test.rs` / `change_flow/mod_test.rs` | `change_dirs` / `read_artifact` 走执行根；archi 改动侧锚执行根；`RunRequest.worktree` 透传 |
| 前端 | `change-detail-view.test.tsx`（两行与三处 root）、`use-change-chat.test.ts`（cwd 派生）、`run-control-panel.test.tsx`（预览根）、`change-create-dialog.test.tsx`（成功面两行）、`src/types/generated/bindings/mod_test.rs`（黄金快照显式重写） |

### 契约资产与架构模型

| 对象 | 改动 |
| --- | --- |
| `.dev-team/architecture/graph/**`、`INDEX.toml`、`.i.*` | **零改动**：本变更不新增 / 删除元素与关系，不改任何导出面（`WorktreePort` 等方法不在任何 `.schema.json` 的在册导出里——`Backend.Core.Workflow` 的接口文件本就不在盘） |
| `.dev-team/architecture/models/**` | **零改动**（受影响模块 `Backend.Core.Workflow` / `Backend.Core.Orchestration` / `Backend.Infra.VcsRuntime` / `Backend.Infra.Store` 均无在盘理由文档；新建模块级文档属独立编辑决策，理由留在本提案与 `design.md`） |

### 不要修改

| 对象 | 理由 |
| --- | --- |
| `.../crates/core/workflow/src/write/delete.rs`（产品面） | 第 1 步仍 `record.worktree`（host）；本变更只把「锚 host」写成显式不变量，不改执行序与 `DeleteOutcome` |
| `.../crates/core/orchestration/src/archive_flow.rs`、`port.rs` | `ArchiveVcsPort` 方法族与签名逐字不变（`restore_baseline` / `worktree_snapshot` / `remove_worktree` 仍吃一个路径，调用方给 host） |
| `.../crates/infra/vcs/src/git.rs` 的 `restore_baseline` / `worktree_snapshot` / `remove_worktree` / `commit_all` / `dirty` / `rebase` / `ff_merge` 命令体 | 只换锚，不改命令串与语义（AC-7 逐条钉死） |
| `.../crates/infra/store/src/model.rs` 的 `WORKSPACE_STORE_FORMAT_VERSION` | 格式版本一动所有 workspace 库作废重建（D9）；加列走 native_model 链 |
| `.../crates/core/config/src/lib.rs` 的 `KNOWN_KEYS` / `TestFramework` / `is_valid_project_root` 谓词 | 只改注释与 schema 文案（D5 是 zero-code）；`..` / 绝对路径拒绝不变——放开等于拆 cwd 逃逸护栏 |
| `openspec/specs/**` | delta-only：本 change 只写 `openspec/changes/worktree-subdir-exec-root/specs/**` |
| `.dev-team/architecture/**`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` | 零 archi 资产、零依赖、零 crate 版本跟随 |
| `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | 面板按 props 收 `cwd`，取值改在详情视图；面板内零改动（`执行根` 文案行保持退役态） |

---

## 验收标准

1. **AC-1 · 根 workspace 逐字等价**：`rel = ""` 时建档记录的 `exec_root` 为 `None`；create 时序、`CreateOutcome`（除新增 `execRoot = null`）、detail / preflight 的全部路径与今天逐字相同；`tracked_at_head` 零调用（用假件计数断言）；全量既有测试零改动通过。
2. **AC-2 · 子目录 workspace 建域四件套**：`worktree` = `…/.worktrees/<repo>/<name>`、`exec_root` = `worktree/<rel>`、`openspec/changes/<name>/explore.md` 落在 `exec_root` 下（主仓 `<repo>/<rel>/openspec/changes/` 与 `worktree/openspec/` 两侧均无该目录）、`add_worktree` 收到的是 host。
3. **AC-3 · 前置判据六形态**（explore §6 表逐条）：未跟踪 / HEAD 已删 / 被 ignore → 前置 `Err` 且零建档零 worktree 零 branch；跟踪但仅含被 ignore 文件 → 建档成功；`rel = ""` → 零新增 spawn；前置命中后置 miss → 补偿回收（零 worktree / 零 branch / 零建档）且 `Err` 文案含双源残留指引。
4. **AC-4 · 回收与 git 步只认 host**：把 exec root 传给 `remove_worktree` 的用例必须失败（`remove_worktree` 的入参在调用点固定为记录 `worktree`，用 source 断言 + 行为用例双钉）；`restore_baseline` 的子目录 workspace 夹具下 `reset --hard` 覆盖 **整个** worktree（域外 tracked 改动被恢复）、`clean -fd` **不缩域**（域外 untracked 残留被清）；`worktree_snapshot` 的 `ls-files` 索引含域外条目。
5. **AC-5 · 访问器与守卫**：`exec_root()` 单点存在（`grep` 无散落 `exec_root.or(worktree)`）；陈旧 / 越界 `exec_root`（等于 host、host 祖先、无关绝对路径）→ 返回值回落 host 且 stderr 有留痕；`worktree = None` → 返回 `None`。
6. **AC-6 · 读面全覆盖**：子目录 workspace 下 `get_change_detail` 命中 `exec_root` 内的产物、`read_artifact` 同、`change_dirs` 候选同、`archi_change_summary` 的改动侧同、归档 preflight 的 `missingArtifacts` / `deltaSpecs` 与实盘一致（不再出现「产物全缺」的假阴性）。
7. **AC-7 · git 步锚点单点**：`grep` 断言 `restore_baseline` 的实参只来自 `request.worktree`（host），`remove_worktree` 的实参只来自记录 `worktree` 列；两者均无 `exec_root` 出处。
8. **AC-8 · legacy 判据显式化**：initialize 短路与 Clean 双保险读 `request.worktree.is_none()`（`grep` 无 `root == main_root` 形态的 legacy 代理）；legacy run 全链零变化（既有用例零改动通过）。
9. **AC-9 · native_model v5**：V4 → V5 往返逐字段相等；V4 存量行升级后 `exec_root = None` 且其余字段原值；`WORKSPACE_STORE_FORMAT_VERSION` 仍为 3；旧库打开不触发作废重建。
10. **AC-10 · 端口契约同步**：trait 两方法 + 4 个测试假件 + vcs 真件签名一致（`cargo build --workspace --all-targets` 零错误）；`worktree_root` 公共入口零残留调用点。
11. **AC-11 · DTO 与绑定**：三个 DTO 各带 `execRoot`（根 workspace / legacy → `null`）；`bindings:export` 后 `bindings.ts` / `bindings.zod.ts` / 黄金快照同批重写并复核 diff；`api:check` 零差异、`INDEX.toml` 与 `.i.*` 逐字节不变。
12. **AC-12 · 前端四处根派生**：画布 root / 面板 cwd / 新图预览 root / create 成功面路径，均按 `execRoot ?? worktree`（归档 change 恒 `null` 语义保持）；详情头在 `execRoot` 非空时呈现两行、为空时仅 `worktree` 一行且零空行。
13. **AC-13 · 改动集与版本**：`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（另加本 change 目录全量：`proposal.md` / `explore.md` / `design.md` / `tasks.md` / `test-design.md` / `specs/**` / `reports/**`）；`package.json` version 恰 +1 patch；`src-tauri/Cargo.toml` / `WORKSPACE_STORE_FORMAT_VERSION` / `openspec/specs/**` / `.dev-team/architecture/**` 逐字节不变。
14. **AC-14 · spec delta 形态**：`specs/` 恰 9 个能力目录；MODIFIED 块为基线整段拷贝后改（与基线 diff 只含本提案声明的差异），未丢任何未冲突条目；每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:`（恰 4 个 `#`）；每个能力目录含 `## Module Contract` 章节。

---

## 风险

| # | 风险 | 处置 |
| --- | --- | --- |
| R-1 | **`HEAD:<rel>` 判据的边界**：空目录不被 git 跟踪（`git add` 空目录无对象）⇒「子目录被跟踪」恒意味着至少一个文件；但子模块 / 大小写不敏感文件系统 / 非 UTF-8 路径都可能让 `rev-parse --verify` 表现与直觉不同。 | 判据只回答「HEAD 里有没有这个路径」，不回答「目录里有什么」——语义边界写进 spec；`rel` 由 `--show-prefix` 原样给出（不经 `strip_prefix` 重建），非 UTF-8 面接受 `git` 自身口径（`--show-prefix` 输出即 git 的路径表示）。AC-3 六形态覆盖真相 |
| R-2 | **半域 git 步是本变更最容易犯的错**：把 exec root 交给 `restore_baseline` / `worktree_snapshot` 后行为「看起来正常」（命令都成功），失败只在「域外改动被丢 / 索引缩域」时暴露。 | D3 写成台账 + AC-4 / AC-7 双闸（source 断言 + 子目录 workspace 行为用例）；`RunRequest.worktree` 的存在就是为了让 git 步**不需要**从 exec root 反推 |
| R-3 | **后置断言失败的补偿面**：`compensate_tree_failure` 的回收对象是 host，而不是 exec root——误传会「删子目录 + 留登记」（与 D4 拒绝的回退同族）。 | 补偿链入参固定 host（AC-3 后置 miss 用例断言 `remove_worktree` 收到 host、`git worktree list` 无残留） |
| R-4 | **归档前置载荷双列遗漏**：只把 `ArchivePreflight.worktree` 改成 exec root（或反之）会让 git 步与产物探测**同时**错：前者删错目录，后者恒「产物全缺」。 | 两列各自命名（`worktree` = host / `execRoot`），消费点表列进提案；AC-6 的 preflight 断言与 AC-4 / AC-7 的 git 步断言分别钉两列 |
| R-5 | **前端遗漏处静默落错根**：画布 / cwd / 预览三处任一漏改，子目录 workspace 下表现为「空白架构图 / 会话在整棵检出根跑」，无报错。 | AC-12 逐处断言；`use-change-chat.ts` 与 `change-detail-view.tsx` 的派生点收敛为同一表达式 |
| R-6 | **格式版本误 bump**：`WORKSPACE_STORE_FORMAT_VERSION` 一动，所有 workspace 库按「低版本删库重建」作废——损失全部会话与流程史。 | D9 明写 + AC-9 断言版本号仍为 3；v5 只走 native_model 链（先例 `AgentProviderRecord` v4→v5） |
| R-7 | **D7 的已知代价**：一仓多子目录 workspace 不能各有同名 change（落位与 branch 名都是全仓命名空间：第二个撞前置⑦「worktree 目录已存在」或前置⑥「branch 已存在」）；跨 workspace 命名唯一即可规避。 | 拒绝面显式且零副作用（既有文案已给清理指引），不是静默错用；AC-3 附一条同仓同名用例钉住拒绝形态；若日后确认为真实痛点，纳入 `rel` 是独立变更（改落位形态 + 存量地址迁移 + 两源对账文案） |
| R-8 | **Windows 路径长度**：`host/<rel>/<name>/…` 比今天更长。 | 逃生口仍是 `core.longpaths`（`archive/2026-10-08-desktop-change-worktree/explore.md:102` 已留痕）；本变更不加机制 |
| R-9 | **上游锁面不可表达**：`is_valid_project_root` 拒绝 `..` / 绝对路径 ⇒ 子目录 workspace 无法声明仓库根的 lockfile，legacy 扫描也只看执行根 ⇒ 恒落「未识别依赖管理器」。 | 作为 V1 边界**第 9 条**（新增「上游锁面不可表达」）留痕；逃生口 `projects: [{ root: "." }]`；放开逃逸 = 拆 cwd 逃逸护栏，**不做** |
| R-10 | **未跟踪资产不随 worktree 复制**：前置只保证 `<rel>` 目录被跟踪；其内若 `openspec/` / `.dev-team/` 被 ignore，worktree 的执行根里就没有语料树与架构资产（本仓 `.dev-team/` 已跟踪，不受影响）。 | 并入 V1 边界第 4 条（扩写为「未跟踪资产不随 worktree 复制」）；不在本变更内做复制机制 |
| R-11 | **陈旧 / 手改库的越界 `exec_root`**：库被外部改写后 `exec_root` 可能指向 host 外。 | D8 守卫：非 host 真后代即 stderr 留痕 + 回落 host（失败形态退回「今天的错位」，不产生越界路径）；回退面**不**做成 `Err`——读面纯 fs、零 vcs 的重启代价大于收益 |
| R-12 | **文案层混淆**：`rebase_halfway_message` / 残留对账指引 / 删除面注记都把 worktree 绝对路径给用户；字段分治后若混用两列，用户 `cd` 过去发现不是自己编辑的地方。 | 三处文案 SHALL 区分「落位目录」与「执行根」（spec 的展示面条款 + AC-12 的两行断言）；既有文案逐字保留、仅在需要时补执行根一行 |
| R-13 | **前置判据的时序插位**：新判据若插在 `add_worktree` 之后，`Err` 将带着已铸出的 worktree 与分支离场（补偿链可回收，但拒绝面不再「零副作用」）。 | 判据 MUST 位于 `add_worktree` 之前（D4 第一层，零副作用），后置 `is_dir` 断言只兜竞态 / 磁盘异常；AC-3 的六形态按「是否触及 worktree」分两组断言 |
