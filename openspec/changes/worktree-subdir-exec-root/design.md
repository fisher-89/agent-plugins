# 设计: worktree-subdir-exec-root

> **变更**: worktree-subdir-exec-root
> **日期**: 2026-11-27
> **状态**: draft
> **依据**: `proposal.md`（需求 / 变更范围 / AC-1..AC-14）与 `explore.md`（D1–D7 裁定台账）、本 change `specs/**` 9 个能力 delta（模块边界契约）

---

## 提案与规格同步状态

`proposal.md` 的 D1–D10 决策台账与 `specs/**` 的 9 个能力 delta 已定稿，本设计不重复其内容，只在其上定稿**实施面**：端口形态与 git 判据、create 时序落点、记录 → 中性快照 → DTO 三段的取值口径、前端派生单点、测试文件改动面（含三文件四处断言手改与语料 golden）。

五点与提案的**显式偏离 / 勘误**（均已在「待决问题」登记，不静默吸收）：

1. **变更清单补入「测试文件改动面」（落到文件级）**：三个 port DTO 各增一个字段（`ChangeStateRecord` / `ArchivePreflight` / `RunRequest`）+ 三个出线 DTO 各增 `execRoot`，Rust 结构体字面量必须逐点补齐，`ChangeRecord::new` 加参牵动 store 测试，三文件四处**精确字段集 / 键集断言**必须手改（见 3）；实际改动文件集因此**大于**提案「变更范围」两张表（详见「测试文件改动面」与 Q1）。
2. **create 成功面前提不成立**：`change-create-dialog.tsx` 的 CreateSuccess 面已在 `300f65e` 退役（提交成功即 `onCreated(result.id)` 关窗导航），提案所述「成功面路径行」与「两行」在代码里没有落点。本设计**不改该文件**，只在变更清单落一行**零代码复核**消除提案文件的覆盖缺口（Q3）。
3. **bindings 侧没有 golden 重写通道（断言手改，不是环境变量开关）**：提案写作 `src/types/generated/bindings/mod_test.rs`，实际镜像测试驻 `packages/desktop/src-tauri/src/bindings/mod_test.rs`（`src/types/generated/` 只放生成产物 `bindings.ts` / `bindings.zod.ts` / `bindings.zod.check.ts`）。该文件的 `authoritative_snapshot()` 运行时先 `export_bindings()` 再读产物（是运行时重导出，不是入仓文件镜像），`DESKTOP_GOLDEN_REWRITE` 只存在于 `crates/core/{archi,checks,workflow}/tests/` 的三处 corpus golden。故 `CreateOutcome` 增列后 bindings 侧的做法是**手改精确字段集断言**（五 → 六），MUST NOT 写成环境变量重写。同一增列还使另三处**精确键集断言**必红：`crates/core/workflow/src/write/create_test.rs` 的 `create_outcome_serde线形状恰五键且id首字段`（结构体字面量缺字段即编不过 + 键集恰五的序列化断言）与 `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` 的两处五键序列化断言；故四处（三文件）与字段同批手改，全部登记为 C 组。
4. **会话 cwd 的派生单点在详情视图**：`use-change-chat.ts` 的 `cwd` 派生表达式不引入第二个参数（面板按 props 收单值 `cwd`，提案与 `desktop-change-chat` Module Contract 均要求「面板零改动」），本设计把 `execRoot ?? worktree` 收敛到详情视图一处（Q4）。
5. **语料 golden 的连带面（提案未列这些文件）**：`ChangeDetail` 加 `execRoot` 后，`project_detail`（`serde_json::to_value(&ChangeDetail)`，序列化键按字典序）多一键——8 份 detail 语料 golden（`crates/core/workflow/tests/golden/corpus-*.json`，`corpus-list-mixed.json` 是 list 投影、不受影响）各需增 `"execRoot": null`。这才是 `DESKTOP_GOLDEN_REWRITE=1` 的正当落点（`crates/core/workflow/tests/corpus_golden_test.rs` 的 `check_or_rewrite`）；文件按 Q1 扩表登记，MUST NOT 静默自动重写。

**架构模型零改动**：本变更不新增 / 删除元素与关系、不改任何 `.schema.json` 在册导出面（`WorktreePort` 等方法不在任何接口文件里——`Backend.Core.Workflow` 接口文件本就不在盘）；`archi_drift` 现为 `status = clean`（37 条 `docMissing` 属既有文档债，恒 `warn`、不纳管），本变更不改变该读数。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 落位 / 偏移派生（端口形态） | 一次 `rev-parse --show-toplevel --show-prefix` 取「git 根 → 落位父锚」与「workspace root 相对偏移」两值；`rel` 只供执行根派生与准入判据 | `crates/core/workflow/src/write/worktree.rs`（trait） + `crates/infra/vcs/src/git.rs`（原语） | crate 内 std；vcs → workflow（port 类型） | sync trait 两方法；`WorktreePlacement { root, rel }` 驻 core |
| 准入判据（`HEAD:<rel>`） | 子目录是否随 worktree 检出的一次 git 询问；`rel = ""` 零 spawn 直返 `true` | `crates/infra/vcs/src/git.rs` + `src/lib.rs` 委托 | 同上 | `rev-parse --verify --quiet`：退出 0 = 在案 / 1 = 缺席 / 其余 fail-closed |
| create 建域组合 | 前置十道（含新增第十道）→ 双列建档 → `add_worktree`（host）→ 后置 `is_dir` 断言 → 执行根内建 change 目录 + `explore.md` → bootstrap（执行根） | `crates/core/workflow/src/write/create.rs` | `state` / `worktree` / `foundation::layout` | sync 零 tokio；补偿链入参固定 host |
| 记录承载与访问器 | `exec_root` 列语义（`None` = 同 `worktree`）+ 单点访问器（真后代守卫 + `[workflow]` 留痕） | `crates/core/workflow/src/state.rs` | serde / specta | 中性快照加一列 + 固有方法 |
| 模型版本 | `ChangeRecord` v4 → v5（`from = ChangeRecordV4`），新列尾部追加；格式版本零 bump | `crates/infra/store/src/model.rs` | native_model / native_db | `#[serde(default)] Option<String>` + 双向 `From` |
| 映射单点 | 记录 ↔ 中性快照两处补列（`change_state` / `create_change_record`），其余写路径读写原记录自动随行 | `crates/infra/store/src/store.rs` | store → workflow（既有中性类型边） | 纯字段搬运 |
| 运行三根分离 | `RunRequest.root` = 执行根、`main_root` = workspace root、`worktree` = host；Clean 步锚 host；legacy 判据换 `worktree.is_none()` | `crates/core/orchestration/src/walker.rs` | 既有编排缝 | 单字段 + 两处判据替换；git 步命令体零改 |
| 归档前置双列 | `ArchivePreflight.worktree`（host）/ `.execRoot`（原始列）；产物与 delta specs 探测吃执行根 | `crates/core/orchestration/src/{state,steps}.rs` | 同上 | 载荷装配 + 两个探测函数换根参数值 |
| 读面执行根定位 | 详情 / 产物 / exec 读命令组 / archi 改动侧统一经记录访问器定位 | `crates/core/workflow/src/queries/detail.rs`、`src/commands/{changes,archi,change_flow}/mod.rs` | 既有 Layout / locate 单点 | 调用点换取值来源，候选序零改 |
| 前端执行根派生 | 详情视图单点解析 `execRoot ?? worktree`，下传画布根 / 会话 cwd；详情头两行；归档预览根三级回落 | `packages/desktop/src/views/changes/change-detail-view.tsx`、`flow/run-control-panel.tsx` | bindings | React + specta 生成类型 |
| 出线生成物 | 三个 DTO 加 `execRoot` 后经 `bindings:export` 再生成；三文件四处精确键集断言手改（五 → 六：workflow 侧 serde 键集 + bindings 字段集 + commands 两处序列化键集）；8 份 detail 语料 golden 走 workflow corpus 的显式重写开关 | `packages/desktop/src/types/generated/**`、`src-tauri/src/{bindings,commands/changes}/mod_test.rs`、`crates/core/workflow/src/write/create_test.rs`、`crates/core/workflow/tests/{corpus_golden_test.rs,golden/*.json}` | export-bindings 管线 / specta 生成物 / corpus golden | `bindings:export` 重生成 + C 组四处断言手改 + `DESKTOP_GOLDEN_REWRITE=1` 显式重写（**仅** corpus golden 侧） |

**零触点组件（核对结论）**：`write/delete.rs`、`write/promote.rs`、`write/archive.rs`（产品面）、`queries/list.rs`、`queries/mod.rs` 的 `locate_change` 形态、`config` crate 的谓词 / `KNOWN_KEYS` / `TestFramework`、相位提示词模板（`prompts/`）、`change-chat-panel.tsx`、`change-delete-dialog.tsx`、`openspec/specs/**`、`.dev-team/architecture/**`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。

---

## 关键设计决策

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| **D-1** | 端口形态与 `rel` 来源 | `WorktreePort::worktree_root` 退役，改 `placement(main_root) -> Result<WorktreePlacement, String>`（一次性给 `root` = `<.worktrees>/<repo>` 与 `rel`）+ 新增 `tracked_at_head(main_root, rel) -> Result<bool, String>`。`rel` 取 `git rev-parse --show-prefix` 原样输出、`trim_end_matches('/')` 归一（根 workspace → `""`），MUST NOT 由 canonicalize 路径 `strip_prefix` 重建。`WorktreePlacement` 驻 `write/worktree.rs`（port 类型随消费者），vcs 侧新增自由函数 `pub fn placement(workspace_root: &Path)`，`worktree_root_from_toplevel` 保留为私有纯派生（单测直驱） | 一次 `rev-parse` 两个值省掉第二次 spawn；`--show-prefix` 一次绕开 `\\?\` 前缀 / 真实大小写 / 分隔符三坑；`tracked` 不进结构体（提案 §六②）——判据是**第二个 git 询问**，独立方法让「根 workspace 零新增 spawn」可以整段跳过而不靠条件表达式维持 |
| **D-2** | `tracked_at_head` 的错误语义 | `rel.is_empty()` 直返 `Ok(true)`（零 spawn，实现侧兜底）；非空时 `rev-parse --verify --quiet HEAD:<rel>`：退出 `0` → `Ok(true)`、退出 `1` → `Ok(false)`（`--quiet` 的 miss 形态）、其余退出码 / spawn 失败 → `Err`（fail-closed，把「问不出来」与「不在案」区分开）。create 侧另有一层 `rel` 非空判断（根 workspace **零调用**，假件可计数断言） | 判据只回答「HEAD 里有没有这个路径」，不回答「目录里有什么」（R-1 语义边界）；fail-closed 使 git 异常不会伪装成「子目录未跟踪」的拒绝文案。本仓实测：`--show-prefix` 在根 workspace 输出空行、子目录 workspace 输出 `packages/desktop/`；`rev-parse --verify --quiet HEAD:packages/desktop` 退出 0、不存在路径退出 1 |
| **D-3** | create 时序与两层准入落点 | 前置序保持现状编号（① name / ② goal / ③ 主仓 active 目录 / ④ db 同名 active / ⑧ active 上限 / **前置⑨ 磁盘门槛（源改 `placement.root`）** / ⑤ `probe` / ⑥ branch / ⑦ worktree 目录冲突），新增**第十道**紧随 ⑦：`rel` 非空且 `tracked_at_head` 否 → 显式 `Err`（文案含 `rel`、三形态成因与「纳入 git 跟踪并提交 / 改用仓库根」引导），零建档零目录零 worktree 零 branch。执行段：`host = placement.root.join(name)`、`exec_root = host.join(rel)` → 建档双写 → `add_worktree(main_root, &host, &branch)` → **后置断言（`rel` 非空才做）`exec_root.is_dir()`**，miss ⇒ `compensate_tree_failure`（入参 host）+ `Err`（文案含「已补偿回收」与双源残留指引）→ `resolve(&exec_root)` 建目录树 + `explore.md` → `bootstrap(&exec_root, …)` | 前置判据先于任何写 IO 才能保住「拒绝面零副作用」（R-13）；后置断言 MUST 早于 `write_fs_half`——`create_dir_all` 会把缺失的执行根**建出来**，断言若在其后就永远为真（这是本变更最容易被写反的一行）；`rel = ""` 时断言跳过，`CreateOutcome` / 路径 / spawn 序列与今天逐字相同（AC-1） |
| **D-4** | 记录承载、访问器与守卫 | `ChangeStateRecord.exec_root: Option<String>` + 固有方法 `exec_root(&self) -> Option<&str>`：`worktree` 为 `None` → `None`；`exec_root` 为 `None` → `Some(worktree)`；`exec_root` 是 `worktree` 的**真后代**（`Path::new(exec) != Path::new(worktree) && Path::new(exec).starts_with(worktree)`，组件级比较）→ `Some(exec)`；否则 `eprintln!("[workflow] …")` 留痕 + `Some(worktree)`。消费点 MUST NOT 自写 `exec_root.or(worktree)` | 读面零路径拼接、零分隔符考古（B 案被否决的理由）；守卫把陈旧行 / 手改库的失败形态退回「今天的错位」而不产生越界路径（R-11，读面纯 fs 不宜做成 `Err`）；`starts_with` 与 `==` 都是**组件级**比较，分隔符混用与尾分隔符都不误判 |
| **D-5** | 三处取值口径（记录列 / 访问器 / DTO） | **DTO 出线取原始列**（`record.exec_root.clone()`，根 workspace 与 legacy 均 `null`）；**路径解析取访问器**（`record.exec_root()`，根 workspace 回落 `worktree`、legacy `None`）。两者在同一文件内显式书写，不互相替代：`ChangeDetail` / `ArchivePreflight` / `CreateOutcome` 的 `execRoot` = 原始列，`locate_change` / `missing_artifacts` / `detect_delta_specs` / 命令层 exec root 解析 = 访问器值 | D2 / AC-11 / AC-12 要求「根 workspace → `null`」且前端两行「非空才渲染第二行」；若 DTO 也走回落，根 workspace 会渲染两条同值路径（AC-12 明禁）。`desktop-change-queries` 与 `desktop-change-archive` Module Contract 里「`execRoot` = 访问器值」一句按本决策读作「**路径解析**用访问器值」，DTO 面取原始列（Q2） |
| **D-6** | 归档前置双列与探测函数 | `run_archive_preflight` 填 `worktree` = 记录 host（原样）、`exec_root` = 记录原始列；`missing_artifacts` / `detect_delta_specs` 的根参数**改吃访问器值**（参数名 `worktree` → `exec_root`，**签名形态与调用点数量不变**——「加一个根参数」的实质是多了一个根**来源**，不引入冗余参数）。`archive_context()` 仍返回 `(name, Option<host>)`：Commit / Rebase / ResolveConflicts / Merge / Cleanup / 半程 `worktree_snapshot` 全部继续吃 host | 双列各自命名才能同时钉住两侧（R-4）：单列改执行根 → `git worktree remove` 删错目录；单列改 host → 产物探测恒「全缺」；`detect_delta_specs` 的「主仓优先、worktree 回退」两级序不变，只是第二级换成执行根 |
| **D-7** | run 三根分离与 legacy 判据 | `RunRequest` 增 `worktree: Option<String>`（host）；`change_flow` 直供 `record.worktree.clone()`；`step_clean` 的 `restore_baseline` 实参改 `request.worktree`（`None` 即先到的 legacy 跳过分支）；initialize 相位短路与 Clean 双保险两处 `root == main_root` 代理判据换 `request.worktree.is_none()`。另：`change_flow` 前置校验 3 的 Err 文案名词由「worktree 目录」改「执行根目录」（路径值随访问器走） | exec root 不再必然等于 host，「路径相等」代理只在本变更之前巧合成立（子目录 workspace 下恒不等）；`RunRequest.worktree` 的存在正是为了让 git 步**不需要**从执行根反推（R-2）。文案名词随语义改：子目录 workspace 下报 `worktree/<rel>` 缺失时说「worktree 目录」会让用户去看落位根 |
| **D-8** | 前端派生单点 | 详情视图内一次解析 `const execRoot = detail.execRoot ?? detail.worktree`，供三处消费：画布根（`archive` 或 `null` → `root`）、`ChangeChatPanel.cwd`、会话 hook 的执行根来源（面板 props 与 hook 参数面均保持**单值**，文件零改动）。归档预览根独立按 `preflight.execRoot ?? preflight.worktree ?? root`（`ArchitectureChangeBlock` 的局部 prop 由 `worktree` 更名 `previewRoot`，文件内私有，无跨文件契约） | 「派生点收敛为同一表达式」在本仓按**最少派生点**落地：详情视图一处解析、三处消费，比「两处各写一遍 `execRoot ?? worktree`」更难漂（R-5）；面板零改动是提案「不要修改」与 `desktop-change-chat` Module Contract 的共同要求（Q4） |
| **D-9** | 生成物、版本与 archi 资产 | `package.json` `0.7.23 → 0.7.24`；`bindings.ts` / `bindings.zod.ts` 经 `bindings:export` 重生成（生成物，MUST NOT 手改）；**三文件四处精确断言手改**——① `crates/core/workflow/src/write/create_test.rs` 的 `create_outcome_serde线形状恰五键且id首字段`（:2491-2531）改六键（字面量补 `exec_root: None`、键集 vec 与断言消息、函数名 / 两处注释「五 → 六」、补 `execRoot` null 断言，其余九项逐字不动；逐项口径见「测试文件改动面」C 组）；② `src-tauri/src/bindings/mod_test.rs` 的 `create_outcome出线恰五字段_id为首字段`（:1555-1582）改六字段（`field_names` 期望 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings`）；③ `src-tauri/src/commands/changes/mod_test.rs` 的**两处**五键序列化断言（`create接线_建域落位_返回dto增id且按id立即可见可发起` :621-625 与 `create_workflow_type透传test_only且dto五键不变` :2564-2608）同改六键（排序后 `created` / `execRoot` / `id` / `name` / `warnings` / `worktree`）；`bindings` 侧无 `DESKTOP_GOLDEN_REWRITE` 通道（`authoritative_snapshot()` 每次运行时先 `export_bindings()` 再读产物）；8 份 detail 语料 golden 各增 `"execRoot": null`，走 `crates/core/workflow/tests/corpus_golden_test.rs` 的 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工复核 diff；`INDEX.toml` / `.i.*` / `.schema.json` / `*.md` 零改动（无元素 / 关系 / 导出面变化）；`WORKSPACE_STORE_FORMAT_VERSION` 仍为 `3` | 加列属模型版本面、与库格式版本面职责分立（D9 提案）；`bindings` 产物是运行时重导出而非入仓镜像，能挡住字段漂移的只有精确断言，故断言随字段同批手改（`CreateOutcome` 的测试构造点唯一——仅 `create_test.rs` 一处，产品构造点 `write/create.rs:305`——加列后该处既编不过、键集也必红，故同属 C 组）；`api:check` 与本变更无关，但须保持零差异（AC-11） |
| **D-10** | 机械补齐面的处置（文件级） | 字段新增不做「兼容层」逃避（不写 `..Default::default()`、不加无主字段）：port DTO 与 `ChangeRecord::new` 的字面量逐点补齐 `exec_root: None` / `worktree: None`，**只改编译不改变断言**；连同签面迁移（4 个 `WorktreePort` 假件）、公共入口退役的调用点（2）、精确断言手改（3 文件 4 处）、语料 golden（8）、`git_test.rs` 承接登记，全部落到「测试文件改动面」的文件级表，口径是**文件** | 让编译期一次性暴露全部构造点，比藏一个「可选字段」更安全；文件级表同时是 AC-13 改动集对账的凭据（Q1） |

---

## 数据模型

### `ChangeRecord`（native_model id=9，v4 → v5）

| 字段 | 类型 | 语义 | 本次 |
|------|------|------|------|
| `id` / `name` / `title` / `workflow_type` / `created_at` / `status` / `archived_at` / `active_phase` / `worktree` / `base_commit` | 既有十列 | 逐字不变；`worktree` 语义钉为 **host**（git 落位目录） | 零改动 |
| `exec_root` | `Option<String>`（`#[serde(default)]`） | 执行根绝对路径；`None` = 与 `worktree` 同值（legacy 记录与根 workspace 同形）。与 `worktree` 同形态（写面 `Path` 的字符串形态，MUST NOT 额外归一大小写 / 分隔符 / 尾分隔符）；归档后原样保留（历史锚） | **新增（声明序尾部追加）** |

升级链（照 `AgentProviderRecord` v4→v5 先例，零迁移脚本）：

```
ChangeRecordV3 (id=9, version=3)                     ← 原样在案
ChangeRecordV4 (id=9, version=4, from = ChangeRecordV3)  ← 今日 ChangeRecord 整体改名（pub(crate)，双向 From 保留）
ChangeRecord    (id=9, version=5, from = ChangeRecordV4)  ← 现役：+ exec_root
  From<ChangeRecordV4> for ChangeRecord : exec_root = None（旧行升级即此档）
  From<ChangeRecord>   for ChangeRecordV4: exec_root 丢弃（降级形态不作数据承诺）
WORKSPACE_STORE_FORMAT_VERSION: 3 → 3（零 bump）
```

`ChangeRecord::new(id, name, workflow_type, created_at, worktree, base_commit, exec_root)`（`exec_root` 追加在末位）。写路径：仅 `change_state()`（记录 → 中性快照）与 `create_change_record`（中性快照 → 记录）两处搬运；`start_change_phase` / `log_change_phase` / `apply_change_backtrack` / 归档翻转 / 启动标定均为「读记录 → 改一字段 → upsert」，新列原地随行（零补列）。

### `ChangeStateRecord`（core 中性快照）

| 字段 | 类型 | 语义 |
|------|------|------|
| `worktree` | `Option<String>` | 既有；host |
| `exec_root` | `Option<String>` | **新增**；执行根原始列（`None` = 与 `worktree` 同值） |

访问器 `ChangeStateRecord::exec_root(&self) -> Option<&str>`（D-4）：`worktree = None` → `None`（legacy，调用方回落主 workspace root）；`exec_root = None` → `Some(worktree)`；真后代 → `Some(exec_root)`；越界 / 陈旧 → stderr `[workflow]` 留痕 + `Some(worktree)`。

### 端口流量类型

```rust
pub struct WorktreePlacement {
    pub root: PathBuf, // <git 根父目录>/.worktrees/<git 根目录名>
    pub rel: String,   // workspace root 相对 git 根的偏移，`/` 分隔，根 workspace 为 ""
}
```

### DTO 演进（进程内 + IPC 出线）

| DTO | 字段 | 取值来源 | 出线形态 |
|-----|------|----------|----------|
| `CreateOutcome` | `execRoot` | 原始列（`rel = ""` → `None`） | `string \| null`；`worktree: String` 语义钉为 host |
| `ChangeDetail` | `execRoot` | 原始列（legacy / 根 workspace → `None`） | `string \| null`；`worktree` 原样保留 |
| `ArchivePreflight` | `execRoot` | 原始列 | `string \| null`；`worktree` 语义钉为 host |

**字段声明序（三处一致）**：`exec_root` 紧排 `worktree` 之后——`CreateOutcome` 出线序 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings`（`bindings/mod_test.rs` 的 `field_names` 期望按此序）、`ArchivePreflight` 为 `name` / `completed` / `incompletePhases` / `missingArtifacts` / `deltaSpecs` / `worktree` / `execRoot` / `branch` / `mergeTarget`、`ChangeDetail` 为 `worktree` / `execRoot`（末位）。两列相邻是刻意的：它们是同一 change 的两个根，读面与出线面都不该被别的字段隔开。`commands/changes/mod_test.rs` 的序列化键断言按字典序，故 `execRoot` 居 `created` 与 `id` 之间。

`RunRequest.worktree: Option<String>` 为进程内字段（不出线），恒 host、legacy → `None`。

---

## 路由 / API 设计

命令族与参数形态零变化，只有三个返回 / 载荷类型各多一个字段（`execRoot`）。本地 Tauri IPC，无鉴权面。

| 命令 / 载荷 | 参数 | 返回 | 本次变更 |
|-------------|------|------|----------|
| `create_change` / `create_change_with` | `(root, name, goal, workflow_type)`（promote 路径多 `title`） | `CreateOutcome` | 返回体加 `execRoot`（写面 DTO 直通，命令层零代码） |
| `get_change_detail` / 详情统一查询 | `(root, id)` | `ChangeDetail` | 返回体加 `execRoot`；产物定位改执行根 |
| `read_artifact` | `(root, id, kind, source)` | `ArtifactEnvelope \| null` | 签名不变；`locate_change` 回退参改访问器值 |
| `exec_output_path` / `read_exec_command` / exec 读命令组（经 `change_dirs`） | 既有 | 既有 | 签名不变；候选根改执行根（探测序与去重语义不变） |
| `change_flow_start` 的 `RunUpdate::ConfirmWait` / 活动快照 | 既有 | `ArchivePreflight` | 载荷加 `execRoot`（`worktree` 语义钉为 host） |
| `archi_change_summary` | `(root, change_id)` | `Option<ArchiChangeSummary>` | 签名不变；active 改动侧改执行根（archived 仍恒 `None`，与会话 cwd 同口径） |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  公共函数仅列模块级导出函数 / trait 方法与公共类型方法；私有函数与 pub(crate) 不入表。
  测试文件：新用例的设计与编写归 test-design / test-gen，不入本清单；但「字段新增导致的编译面补齐 + 精确断言手改 + 语料 golden 重写」是必须随动的既有代码，单列「测试文件改动面」一节（口径为文件）。
  本清单 + 「测试文件改动面」= 提案「变更范围」的超集（Q1）。
-->

### 新增文件

无。`WorktreePlacement` 落既有 `write/worktree.rs`（port 类型随消费者，不新开文件）；无新 crate、无新 npm 包、无新配置键。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/worktree.rs` | `WorktreePort::worktree_root` → `placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>`；新增 `tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>`；新增 `pub struct WorktreePlacement { pub root: PathBuf, pub rel: String }`；trait 与方法文档口径改执行根 / 落位分离 | D-1 / D-2 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | `pub use worktree::{InstallRun, RepoProbe, WorktreePlacement, WorktreePort}` | 门面再导出 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` | `toplevel()` → `pub(crate) fn placement(main_root: &Path) -> Result<(PathBuf, String), String>`（一次 `rev-parse --show-toplevel --show-prefix`，首行 = git 根、次行 `trim_end_matches('/')` = `rel`；非 git / 裸仓 / 空输出三档引导文案自 `toplevel` 原样迁移）；新增 `pub(crate) fn tracked_at_head(main_root: &Path, rel: &str) -> Result<bool, String>`（空 `rel` 零 spawn）；新增私有退出码通道（`git_raw` 返回 `(code, stdout, stderr)`，`git()` 保持「非零即 Err」现行语义与文案，调用点零改）；`restore_baseline` / `worktree_snapshot` / `remove_worktree` / `commit_all` / `dirty` / `rebase` / `ff_merge` 命令体**逐字不变** | D-1 / D-2 / AC-7 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | 新增 `pub fn placement(workspace_root: &Path) -> Result<WorktreePlacement, String>`（`git::placement` + `worktree_root_from_toplevel` 组合）；`pub fn worktree_root` **删除**（公共单点退役，AC-10 零残留调用点）；`worktree_root_from_toplevel` 保留为私有纯派生；`ProcessWorktree` 增 `placement` / `tracked_at_head` 委托 | D-1 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | `vcs.placement` 取 `placement.root` 与 `placement.rel`（前置⑨ 磁盘门槛的扫描源随之改）；新增第十道 `tracked_at_head`（`rel` 非空才调用）；`host = placement.root.join(name)` / `exec_root = host.join(rel)`；建档双写 `worktree` / `exec_root`（`rel = ""` → `None`；执行根串一次铸出、记录与 DTO 各持一份克隆，避免两处取值漂移）；`add_worktree` 目标恒 host；新增后置 `exec_root.is_dir()` 断言（`rel` 非空才做），miss ⇒ `compensate_tree_failure`（入参 host）；`resolve(&exec_root)` + `write_fs_half` 与 `bootstrap(&exec_root, …)` 改执行根；私有 `bootstrap_worktree_root` → `bootstrap_exec_root`（参数名 `worktree` → `exec_root`）；`bootstrap_project` 警告文案「在 worktree 内不存在」→「在执行根内不存在」；`BootstrapProject.root` 文档改「相对执行根」；`CreateOutcome` 增 `pub exec_root: Option<String>`（声明序紧跟 `worktree`，出线序 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings`）；`worktree_reconcile_hint` 文案逐字保留（落位根口径不变） | D-3 / D-5；`create` 签名不变 |
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `ChangeStateRecord` 增 `pub exec_root: Option<String>`（声明序 `worktree` 之后、`base_commit` 之前）；新增 `impl ChangeStateRecord { pub fn exec_root(&self) -> Option<&str> }`（真后代守卫 + `[workflow]` 留痕，D-4） | 访问器单点 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ArchivePreflight` 增 `pub exec_root: Option<String>`（serde camelCase → `execRoot`；声明序紧跟 `worktree`）；其余七字段与语义不变 | D-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `run_archive_preflight`：`worktree` 填记录 host、`exec_root` 填记录原始列；探测改吃访问器值——私有 `missing_artifacts(layout, exec_root: Option<&str>, name)` / `detect_delta_specs(layout, exec_root: Option<&str>, name)` 参数名与取值改（签名形态与级数不变）；`detect_delta_specs` 的「主仓优先、worktree 回退」两级序保留 | D-6 / AC-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `RunRequest` 增 `pub worktree: Option<String>`（host；字段文档）；`step_clean` 早退判据与 `restore_baseline` 实参改 `request.worktree`（host 全树）；initialize 相位短路（`phase == INITIALIZE_PHASE_ID`）与 Clean 双保险两处 `root == main_root` 换 `worktree.is_none()`；其余步执行体与 `archive_context` 返回形态逐字不变 | D-7 / AC-4 / AC-8 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `change_detail` 的 `locate_change` 回退参改 `record.exec_root()`；`ChangeDetail` 增 `pub exec_root: Option<String>`（原始列；声明序紧跟 `worktree`，即 DTO 末位）；`worktree` 字段与语义不变 | D-5 / AC-6 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 前置校验 3 的 exec root 解析改 `record.exec_root()`（`is_dir` 校验沿用，Err 文案名词改「执行根目录」并携路径）；`RunRequest` 增 `worktree: record.worktree.clone()`；其余装配（compose 注入 workspace root、快照源、复合键）零改 | D-7 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `read_artifact` 的 `locate_change` 回退参改 `record.exec_root()`；`change_dirs` 两处候选供给改 `record.exec_root()`（`push_worktree_candidate` 复用，探测序与去重不变）；`CreateOutcome` 透传（写面 DTO 直通，零代码改动，仅复核字段抵达） | D-5；AC-6 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | `archi_change_summary` 的 active 改动侧改 `record.exec_root().map(str::to_owned)`；`Archived → None` 分支保持（归档恒回落主仓根） | D-5 / AC-6 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 今日 `ChangeRecord` 整体改名 `ChangeRecordV4`（`#[native_model(id = 9, version = 4, from = ChangeRecordV3)]`，双向 `From` 保留）；现役 `ChangeRecord` 升 `version = 5, from = ChangeRecordV4` 并在 `base_commit` 后追加 `#[serde(default)] pub exec_root: Option<String>`；新增 v4↔v5 双向 `From`（升级 `exec_root = None` / 降级丢弃）；`ChangeRecord::new` 增 `exec_root: Option<String>`（末位）；`WORKSPACE_STORE_FORMAT_VERSION` **零改动** | D-9（AC-9） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `change_state()` 与 `create_change_record` 两处映射补 `exec_root` 搬运；其余 change 写路径零改 | D-4 |
| `packages/desktop/src-tauri/crates/core/config/config.schema.json` | `projects.description` 与 `projects.items.properties.root.description` 的「相对 worktree 根」→「相对执行根」；键集 / 类型 / `default` / `minLength` / 其余描述逐字不变（须与实现同 commit） | D5 是 zero-code（AC-13 对配置 schema 的要求只到文案） |
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | `is_valid_project_root` 注释理由「逃出 worktree」→「逃出执行根」；谓词 / `KNOWN_KEYS` / `TestFramework` / `validate` 逐字不变 | 同上 |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | **零代码复核**（不产生 diff）：核对成功面已退役（提交成功即 `onCreated(result.id)` 关窗导航）、组件内无 `worktree` 路径渲染、无 `execRoot` 消费点，确认提案「成功面路径行 / 两行」无落点 | 提案把它列入「实现文件」而设计判零改动——本行是覆盖对账的留痕（Q3）；`CreateOutcome` 加列在编译面只落在其 `*.test.tsx` fixture（见「测试文件改动面」F 段） |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `DetailHeader` 增第二行（`detail.execRoot !== null` 才渲染，`data-testid="detail-exec-root"`，文案 `执行根：<execRoot>`；为空时零占位、零空行）；详情体增一次解析 `const execRoot = detail.execRoot ?? detail.worktree`，供 `archiRoot`（`archive` 或 `null` → `root`）与 `ChangeChatPanel.cwd` | D-8 / AC-12 |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | 参数文档口径「执行根来源（`detail.worktree` …）」→「执行根来源（`detail.execRoot ?? detail.worktree`，由详情视图解析后经 `cwd` prop 送入）」；`cwd = archived ? null : worktree` 行为**逐字不变** | D-8；Q4 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | `ArchitectureChangeBlock` 的预览根 prop 更名 `worktree` → `previewRoot`（文件内私有）并改喂 `preflight?.execRoot ?? preflight?.worktree ?? null`；`ArchiPreview` 文档同步；合入 / 清理告知两处 `preflight.worktree !== null` 判定保持不变（host 口径） | D-8 / AC-12 |
| `packages/desktop/src/types/generated/bindings.ts`、`.../bindings.zod.ts` | 经 `bindings:export` 重生成（三 DTO 的 `execRoot` 入线） | 生成物，MUST NOT 手改；重生成 MUST 先于前端 `execRoot` 消费点（详情视图 / 会话 hook / 预览面板）与 fixture 补齐落地——前端 TS 类型面（`ChangeDetail` 等）依赖它，倒置即 TS 编译失败 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 手改 `create_outcome出线恰五字段_id为首字段`（:1555-1582）为六字段：测试名与文档注释「五 → 六」、`field_names` 期望改为 `["id", "name", "created", "worktree", "execRoot", "warnings"]`、补 `execRoot: string \| null` 形态断言；同文件 `archive_preflight线面字段面保留` 是逐字段 presence 循环（非精确集合），受字段新增影响为零、**不改**（补 `execRoot` 行归 test-gen） | 本文件没有 `DESKTOP_GOLDEN_REWRITE` 通道——`authoritative_snapshot()` 每次运行时先 `export_bindings()` 再读产物（提案路径勘误见「同步状态」3 / D-9） |
| `packages/desktop/package.json` | `version` `0.7.23` → `0.7.24`（+1 patch；`tauri.conf.json` 经 `"../package.json"` 跟随，`src-tauri/Cargo.toml` crate 版本行不随动） | AC-13 |

### 零改动（显式登记）

| 对象 | 理由 |
|------|------|
| `crates/core/workflow/src/write/delete.rs`、`promote.rs`、`archive.rs` | 删除面 / promote / 归档写面本就在 host 上工作；本变更只把「锚 host」写成显式不变量（`DeleteOutcome` / 执行序 / 归档双写零改） |
| `crates/core/workflow/src/queries/mod.rs`（`locate_change`）、`queries/list.rs` | 签名与候选序逐字不变，只是调用点喂的根换了；列表不定位产物 |
| `crates/core/orchestration/src/archive_flow.rs`、`port.rs` | `ArchiveVcsPort` 方法族与签名不变（`restore_baseline` / `worktree_snapshot` / `remove_worktree` 仍吃一个路径，调用方给 host） |
| `crates/infra/vcs/src/git.rs` 的 git 步命令体（`restore_baseline` / `worktree_snapshot` / `remove_worktree` / `commit_all` / `dirty` / `rebase` / `ff_merge`） | 只换锚，不改命令串与语义（AC-7）；同文件内的落位 / 判据原语（`placement` / `tracked_at_head` / 退出码通道）属上方「修改文件」表 |
| `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | 提案「不要修改」+ `desktop-change-chat` Module Contract；单值 `cwd` prop 已足（D-8） |
| `packages/desktop/src/views/changes/components/change-delete-dialog.tsx` | 勾选语义与 `worktreeCapable` 判据均看记录 `worktree`（host），零改 |
| `openspec/specs/**`、`.dev-team/architecture/**`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` | delta-only；零 archi 资产、零依赖、零 crate 版本跟随（AC-13） |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `WorktreePort::placement` | `crates/core/workflow/src/write/worktree.rs` | 修改（取代 `worktree_root`） | `fn placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>` | 一次 git 派生：落位父锚 + workspace 相对偏移；非 git / 裸仓显式 `Err` 引导 |
| `WorktreePort::tracked_at_head` | 同上 | 新增 | `fn tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>` | `HEAD:<rel>` 在案判据；`rel = ""` 恒 `Ok(true)` 且零 spawn |
| `WorktreePort::worktree_root` | 同上 | **删除** | — | 公共单点退役（AC-10 零残留调用点） |
| `vcs_runtime::placement` | `crates/infra/vcs/src/lib.rs` | 新增 | `pub fn placement(workspace_root: &Path) -> Result<WorktreePlacement, String>` | 落位派生单点（`git::placement` + `worktree_root_from_toplevel` 组合）；`ProcessWorktree::placement` 委托之 |
| `vcs_runtime::worktree_root` | 同上 | **删除** | — | 由 `placement` 取代 |
| `ProcessWorktree::tracked_at_head` | 同上 | 新增 | `fn tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>` | 委托 `git::tracked_at_head` |
| `ChangeStateRecord::exec_root` | `crates/core/workflow/src/state.rs` | 新增 | `pub fn exec_root(&self) -> Option<&str>` | 执行根单点：`None` 回落 `worktree`；越界 / 陈旧 → `[workflow]` 留痕 + 回落；`worktree = None` → `None` |
| `ChangeRecord::new` | `crates/infra/store/src/model.rs` | 修改 | `pub fn new(id: &str, name: &str, workflow_type: &str, created_at: i64, worktree: Option<String>, base_commit: Option<String>, exec_root: Option<String>) -> Self` | `exec_root` 末位追加 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `WorktreePlacement` | `crates/core/workflow/src/write/worktree.rs`（经 `write::mod` 再导出） | 新增 struct | `{ root: PathBuf /* <git 根父目录>/.worktrees/<repo> */, rel: String /* `/` 分隔、根为 "" */ }` |
| `ChangeStateRecord.exec_root` | `crates/core/workflow/src/state.rs` | 修改（字段） | `Option<String>`；执行根原始列，`None` = 与 `worktree` 同值 |
| `ChangeRecord.exec_root` | `crates/infra/store/src/model.rs` | 修改（字段） | `Option<String>`，`#[serde(default)]`，v5 尾部追加 |
| `ChangeRecordV4` | 同上 | 新增（历史形态，`pub(crate)`） | 今日 `ChangeRecord` 的形状（`from = ChangeRecordV3`，双向 `From`） |
| `ArchivePreflight.exec_root` | `crates/core/orchestration/src/state.rs` | 修改（字段） | `Option<String>`；camelCase 出线 `execRoot` |
| `CreateOutcome.exec_root` | `crates/core/workflow/src/write/create.rs` | 修改（字段） | `Option<String>`；camelCase 出线 `execRoot`；`worktree: String` 语义钉为 host |
| `ChangeDetail.exec_root` | `crates/core/workflow/src/queries/detail.rs` | 修改（字段） | `Option<String>`；camelCase 出线 `execRoot` |
| `RunRequest.worktree` | `crates/core/orchestration/src/walker.rs` | 修改（字段） | `Option<String>`；host、legacy → `None`（进程内，不出线） |
| `ChangeDetail` / `CreateOutcome` / `ArchivePreflight`（TS 侧） | `packages/desktop/src/types/generated/bindings.ts` | 修改（生成物） | 各增 `execRoot: string \| null` |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `projects.description` | `crates/core/config/config.schema.json` | string（schema 描述文案） | 不变（空数组） | 文案「相对 worktree 根」→「相对执行根」；键集 / 类型 / 断言锚点零改 |
| `projects.items.properties.root.description` | 同上 | string（同） | — | 同上一处文案；`minLength: 1` 与「非绝对 / 无 `..` / 无 `:` / 无通配符」文字逐字保留 |

外无新增 / 删除配置键：active 建档上限与 worktree 磁盘门槛仍为源码常量，`rel` MUST NOT 出现在 `config.json`。

### 测试文件改动面（文件级）

> 字段 / 签面变更的「编译面 + 断言面」配套。口径是**文件**：本表 + 上方「修改文件」表相加即 AC-13 改动集对账的完整依据（Q1）；提案「测试文件」表里的每个文件都能在此找到归属，MUST NOT 出现清单外的测试改动。

| 组 | 文件 | 改动 |
|----|------|------|
| **A** 端口签面迁移 | `crates/core/workflow/src/write/create_test.rs` | `FakeVcs`（:311 起）的 `worktree_root` → `placement`（返回可编程 `WorktreePlacement`）；新增 `tracked_at_head`（可编程布尔 + 调用计数，供 AC-1「零调用」与 AC-3 六形态断言）；**该文件同时是 C 组第四处**——`create_outcome_serde线形状恰五键且id首字段`（:2491-2531）的键集 / 字面量随 `CreateOutcome` 增列手改（逐项见 C 组行）；除此之外既有断言意图零改 |
| **A** | `crates/core/workflow/src/write/delete_test.rs` | 同形签名迁移（`placement` 直返夹具值）；断言意图零改 |
| **A** | `crates/core/workflow/src/write/promote_test.rs` | 同形签名迁移（`placement` 直返夹具值）；此文件同时是 E 组的构造点（含 `ChangeStateRecord { … }`，需补 `exec_root: None`）；断言意图零改 |
| **A** | `crates/core/workflow/tests/corpus_golden_test.rs` | 同形签名迁移；同时是 D 组的重写驱动 |
| **B** 公共入口退役的调用点 | `crates/infra/vcs/src/lib_test.rs` | `use super::{worktree_root, …}` → `placement`；`worktree_root` 两例三断言（git 根 workspace 落位 / 子目录 workspace 仍锚 git 根 / 非 git 目录 Err）改断 `placement(...)` 的 `.root` 与 `.rel` 两值；`worktree_root_from_toplevel` 纯派生四例保留（私有函数仍在本 crate 内可见） |
| **B** | `src-tauri/src/commands/changes/mod_test.rs` | `vcs_runtime::worktree_root(env.ws_root.path())`（落位派生）→ `vcs_runtime::placement(env.ws_root.path()).root` |
| **C** 精确字段集 / 键集断言手改（不手改即必红） | `crates/core/workflow/src/write/create_test.rs` | `create_outcome_serde线形状恰五键且id首字段`（:2491-2531）→ 六键：① 文档注释（:2491-2495）五键清单补 `execRoot`、措辞「恰五键」→「恰六键」；② 函数名（:2497）→ `create_outcome_serde线形状恰六键且id首字段`；③ 结构体字面量（:2498-2504）在 `worktree` 之后补 `exec_root: None`（根 workspace / legacy 形态）；④ 键集断言（:2510-2514）vec → `["created", "execRoot", "id", "name", "warnings", "worktree"]`（排序后 `execRoot` 居第二）、消息「恰五键面（id 为身份锚、worktree 为刻意出线的执行锚）」→「恰六键面（id 为身份锚、worktree 与 execRoot 为刻意出线的两根）」；⑤ `warnings` 断言之后（:2522 后）新增一条 `assert_eq!(object["execRoot"], serde_json::json!(null), "execRoot 键恒在场（None → null，不省键）")`；⑥ Debug 注释（:2524-2525）五字段清单补 `execRoot`。**其余九项逐字不动**：`to_value` / `as_object` / `keys` 收集与排序 / `object["id"]` / `object["name"]` / `object["created"]` / `object["warnings"]` / `format!("{outcome:?}")` / `debug.starts_with("CreateOutcome { id: ")`；该函数是 `CreateOutcome` 的唯一测试构造点（产品构造点仅 `write/create.rs:305`）：加列后既编不过（缺字段），键集断言也必红——「不手改即编不过 / 必红」的随动面，MUST NOT 留给 test-gen。`None` 取值刻意锚「null 仍是键」（AC-11 根 workspace / legacy 出线形态）；`Some` 形态的字符串出线由同一 `Option<String>` + `camelCase` 与 bindings 侧 `execRoot: string \| null` 断言承担 |
| **C** | `src-tauri/src/bindings/mod_test.rs` | `create_outcome出线恰五字段_id为首字段`（:1555-1582）→ 六字段：测试名 / 文档注释「五 → 六」、`field_names` 期望 `["id", "name", "created", "worktree", "execRoot", "warnings"]`、补 `execRoot: string \| null` 形态断言 |
| **C** | `src-tauri/src/commands/changes/mod_test.rs` | `create接线_建域落位_返回dto增id且按id立即可见可发起` 的 `keys` 断言（:621-625，五键）→ `["created", "execRoot", "id", "name", "warnings", "worktree"]`（排序后 `execRoot` 居第二位），注释「恰五键面」→「恰六键面」 |
| **C** | `src-tauri/src/commands/changes/mod_test.rs`（同文件第二处键集） | `create_workflow_type透传test_only且dto五键不变`（:2564-2608）的 `keys` 断言（:2605，五键）→ `["created", "execRoot", "id", "name", "warnings", "worktree"]`；同函数文档注释「返回 DTO 仍恰五键（created / id / name / warnings / worktree）」（:2560-2562）→ 六键清单，断言消息「CreateOutcome 恰五键」（:2606）→「CreateOutcome 恰六键」 |
| **D** 语料 golden（`DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工 diff） | `crates/core/workflow/tests/golden/corpus-{archive-prefix,backtrack-stale,initialize,multi-attempt,run-history,run-interrupted,slots-null,worktree}.json` | 各增 `"execRoot": null`（`project_detail` 是 `to_value(&ChangeDetail)`，键按字典序，新键落 `created` 与 `id` 之间）；`corpus-list-mixed.json` 是 list 投影，零变化；MUST NOT 静默自动重写 |
| **E** 字面量机械补齐（只改编译，不改断言） | 31 个文件 | 见下方明细表（文件清单经 `grep -F "<字面量> {"` 实测，处数由编译器逐点核定） |
| **F** 前端 DTO fixture 补齐（TS 编译面） | 10 个文件 | 见下方明细表（8 × `ChangeDetail` / 1 × `ArchivePreflight` / 1 × `CreateOutcome`） |
| **G** 承接登记（本设计零代码改动） | `crates/infra/vcs/src/git_test.rs` | `toplevel` 无直接用例（落位派生用例驻 `lib_test`），故本文件零代码改动；`placement` / `tracked_at_head` 的真件用例（`rel` 空 / 尾 `/`、`HEAD:<rel>` 四态）归 test-gen 承接 |

**E 组明细**（文件清单已按 `grep -F "<字面量> {" --include=*_test.rs` 实测核对；处数不在此断言——含 `fn … -> T {` 返回类型行与 `matches!` 模式行，实施以编译错误逐点核对为准）：

| 字面量 | 测试文件 | 文件数 |
|--------|----------|--------|
| `ChangeStateRecord { … }` | `core/workflow/src/state_test.rs`、`core/workflow/tests/corpus_golden_test.rs`、`core/workflow/src/queries/{detail,list}_test.rs`、`core/workflow/src/write/{archive,backtrack,create,delete,phase_log,phase_next,phase_start,promote,run}_test.rs`、`core/orchestration/src/{steps,walker,run_history,snapshot}_test.rs`、`infra/store/src/{store,change_port,envelope,model}_test.rs`、`infra/checks/src/mutation/mod_test.rs`、`src/commands/{changes,archi,change_flow}/mod_test.rs` | 25 |
| `ArchivePreflight { … }` | `core/orchestration/src/{state,steps,walker,port,control,notify}_test.rs`、`src/commands/{changes,change_flow}/mod_test.rs` | 8 |
| `RunRequest { … }` | `core/orchestration/src/{walker,run_history}_test.rs`、`core/workflow/tests/corpus_golden_test.rs` | 3 |
| `ChangeRecord::new(…)` | `infra/store/src/{model,describe,query,store}_test.rs` | 4 |

四行并集 **31 个文件**（`corpus_golden_test.rs` / `commands/changes/mod_test.rs` / `write/{create,delete}_test.rs` / `orchestration/src/{steps,walker}_test.rs` / `src/commands/change_flow/mod_test.rs` / `infra/store/src/store_test.rs` 等跨行重复者只计一次）。A 组的四个假件全在这 31 个之内；再加 B / C / G 三组补充的三个文件（`crates/infra/vcs/src/lib_test.rs`、`src-tauri/src/bindings/mod_test.rs`、`crates/infra/vcs/src/git_test.rs`），Rust 测试文件合计 **34 个**。

**F 组明细**（局部对象补 `execRoot: null`，不改断言；`ChangeDetail` / `ArchivePreflight` / `CreateOutcome` 的 `execRoot` 是必填键，缺则 TS 编译失败）：

| 文件 | 落点 |
|------|------|
| `packages/desktop/src/app.test.tsx` | `fakeDetail: ChangeDetail` 字面量 |
| `packages/desktop/src/views/changes/change-detail-view.test.tsx` | `detail()` 工厂 + 局部字面量 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.test.ts` | `fakeDetail(...)` 工厂 |
| `packages/desktop/src/views/changes/flow/attachments.test.ts` | `detail()` / `testOnlyDetail()` 两工厂 |
| `packages/desktop/src/views/changes/flow/graph.test.ts` | `detail()` / `richDetail()` / `testOnlyDetail()` 三工厂 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx` | `detail()` 工厂 |
| `packages/desktop/src/views/changes/flow/layout.test.ts` | `detail()` 工厂 |
| `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` | `detail()` 工厂 |
| `packages/desktop/src/views/changes/flow/run-control-panel.test.tsx` | 三处 `preflight()` / `archivePreflight()` 工厂（`Partial<ArchivePreflight>` 基底仍需全键） |
| `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx` | `OUTCOME`（`CreateOutcome`）fixture 与文档注释「五字段」→「六字段」 |

**F 组零改动复核**（上表之外的邻近校验）：

| 文件 | 结论 |
|------|------|
| `packages/desktop/src/views/changes/change-list-view.test.tsx` | 零改动——`create_change` 应答体是**匿名结构类型**（`CreateAnswer`），不经 `CreateOutcome` 类型检查；补 `execRoot` 可选、非必需 |
| `packages/desktop/src/lib/ipc-drift.test.ts` | 零改动——仅出现命令名字符串，零 DTO 字面量 |

> 说明：上表是「构造点与断言面」，不是新增用例；产品面本身的两处字面量（`store.rs` 的 `change_state`、`write/create.rs` 的建档）属实现改动，已在上方「修改文件」表内。新硬用例（AC-1..AC-12）的设计与编写归 test-design / test-gen 相位，见提案「测试文件」表与本设计「验收锚点」。

---

## 依赖

**运行时依赖**：零新增。`rev-parse --verify --quiet` 与 `--show-toplevel --show-prefix` 复用既有 `crates/infra/vcs` 的 `git()` 通道与 `crates/infra/exec` 的 `SystemCommand`（新增的私有退出码通道只在本文件内）；`WorktreePlacement` 只依赖 std `PathBuf` / `String`；前端零新包（`execRoot` 由 specta 生成）。

**构建 / 测试依赖**：零新增。既有 `native_model` / `native_db` / `tempfile` / `specta` 已足；无新 crate、无 `Cargo.toml` / `package.json` 依赖项变化（`package.json` 只动 `version`）。

---

## 验收锚点（AC → 组件 / 决策对账）

| AC | 锚点 |
|----|------|
| AC-1 根 workspace 逐字等价 | D-3（`rel = ""`：`exec_root` 不写列、`tracked_at_head` 零调用、后置断言跳过）+ D-4（访问器回落同值）+ D-5（`execRoot = null`）+ C 组第一处（`CreateOutcome` 线形状断言随字段手改属机械随动，行为面零改动——「既有测试零改动」按 Q1 口径读作行为断言零改动） |
| AC-2 子目录建域四件套 | D-3（host / exec_root 双写、`add_worktree` 收 host、`resolve(&exec_root)`）+ D-1（`rel` 来源） |
| AC-3 前置判据六形态 | D-2（退出码三档）+ D-3（第十道位置 / 后置断言在 `write_fs_half` 之前 / 补偿入参 host） |
| AC-4 / AC-7 回收与 git 步只认 host | D-6（归档双列）+ D-7（`RunRequest.worktree`、`step_clean` 锚 host）+ 零改动表（delete / archive 写面 / vcs 命令体） |
| AC-5 访问器与守卫 | D-4（单点 + 真后代守卫 + `[workflow]` 留痕） |
| AC-6 读面全覆盖 | D-5（DTO / 路径两口径）+ D-6（preflight 双列与探测改根）+ 修改文件表中的五个命令 / 读面文件 |
| AC-8 legacy 判据显式化 | D-7（两处 `root == main_root` → `worktree.is_none()`） |
| AC-9 native_model v5 | D-9 + 数据模型（v4 → v5 链、格式版本零 bump、尾部追加） |
| AC-10 端口契约同步 | D-1 + 公共函数表（`worktree_root` 删除、两新方法）+ 测试文件改动面 A / B 组（4 个假件 + `lib_test` + `commands/changes/mod_test` 调用点） |
| AC-11 DTO 与绑定 | D-5 + D-9（三 DTO 加 `execRoot`、`bindings:export` 重生成、C 组四处断言手改（三文件：`create_test.rs` + `bindings/mod_test.rs` + `commands/changes/mod_test.rs` 两处）、D 组 8 份语料 golden 显式重写、`api:check` / `INDEX.toml` 零差异） |
| AC-12 前端根派生 | D-8（详情视图单点 + 两行 + 预览三级回落；create 成功面一项见 Q3，文件行落「零代码复核」） |
| AC-13 改动集与版本 | D-9（`0.7.24`）+ 零改动表 + 「测试文件改动面」A–G 组（C 组三文件四处断言手改已在表内）+ **Q1（改动集扩表：字面量补齐与语料 golden 不在提案列表内）** |
| AC-14 spec delta 形态 | 规格面产物（本设计不改 spec 文件）；Q2 / Q3 / Q4 / Q7 为需在 spec 面修正的措辞 |

---

## 待决问题

| # | 问题 | 影响 | 建议处置 |
|---|------|------|----------|
| **Q1** | **AC-13「改动集 ⊆ 提案『变更范围』所列文件」按字面不可满足**：①三个 port DTO 各加一列 + `ChangeRecord::new` 加参 ⇒ Rust 字面量逐点补齐（E 组 31 个文件）；②三 DTO 加列 ⇒ 四处精确断言手改（C 组，三文件：`create_test.rs` 的 serde 键集与字面量、`bindings/mod_test.rs` 一处、`commands/changes/mod_test.rs` 两处）；③`ChangeDetail` 加列 ⇒ 8 份 detail 语料 golden 各增一键（D 组）；④10 个前端测试文件的 DTO 字面量补 `execRoot: null`。提案「测试文件」表只列了其中一部分，且**未列语料 golden 文件与多数前端 fixture 文件**，`create_test.rs` 的键集断言亦未点名 | AC-13 的机械判定（`git status --porcelain` 对账）会红；AC-1 的「全量既有测试零改动通过」同此口径读作**行为断言零改动**（C 组四处属 DTO 出线字段集的机械随动，非行为改动） | AC-13 改为「⊆ 提案『变更范围』+ 本设计『变更清单』『测试文件改动面』所列文件」；或允许「纯 `exec_root: None` / `worktree: None` / `execRoot: null` 增行」与「golden 单键增行」免于对账。**本设计按扩表执行**，不改实现方案 |
| **Q2** | **`execRoot` 出线口径的 spec 措辞冲突**：`desktop-change-queries`「自记录访问器取值——否则回落 `worktree`」与 `desktop-change-archive` Module Contract「`execRoot` = 记录访问器值」两处，同 AC-11 / AC-12 要求的「根 workspace → `null`（只渲染一行）」互斥 | 按 spec 字面实现会让根 workspace 详情头出现两条同值路径（AC-12 明禁） | 设计取 D-5（DTO = 原始列，路径解析 = 访问器）；spec 两处措辞改为「`execRoot` 直接出线记录的 `exec_root` 列（`None` → `null`），回落由读面访问器与前端 `execRoot ?? worktree` 承担」 |
| **Q3** | **create 成功面不存在**：`change-create-dialog.tsx` 的成功面在 `300f65e`（0.4.26）已被有意退役（成功即关窗导航），但基线 spec 与本次 delta 的 `desktop-change-worktree`「worktree 路径 UI 可见性」仍含「create 对话框成功面呈现路径与警告」「两行并存」两条要求；提案据此把该文件列入变更范围 | 若照 spec 实现，等于在本变更里恢复一个被刻意退役的 UI 面（越出「字段语义拆分」主题）；若零改动，两条 spec 条款与 AC-12 第四处无落点 | 本设计取**零改动**（`change-create-dialog.tsx` 保持零 diff、变更清单落「零代码复核」行；其 `*.test.tsx` 仅在 `CreateOutcome` 加字段时机械补 fixture）。spec 面须择一：① 删除该两句 create 成功面条款、AC-12 的「create 成功面路径」一处随之取消；② 另立变更恢复成功面并同时实现两行 + 警告 |
| **Q4** | **`desktop-change-chat` Module Contract 的「hook 参数面增 `execRoot`」与提案「`change-chat-panel.tsx` 零改动」互斥**：面板当前把单值 `cwd` prop 映射到 hook 的 `worktree` 参数，hook 拿不到第二个值 | 二者只能取一 | 设计取「面板零改动」（D-8）：派生单点在详情视图，hook 参数语义改写为「已解析的执行根来源」，行为逐字不变。spec 行改为「`use-change-chat.ts` 的 `cwd` 来源语义改执行根（值由详情视图按 `execRoot ?? worktree` 解析后经面板 `cwd` prop 送入），参数面形态不变」 |
| **Q5** | `change_flow` 前置校验 3 的 Err 文案名词（「worktree 目录不存在」→「执行根目录不存在」）是否属 spec 说的「文案沿用」 | 仅文案；AC 未钉字符串，但 spec 用了「沿用」 | 设计取改名词（路径值随访问器走，子目录 workspace 下名词准确才可排查）；若要求逐字保留，回退为原串即无行为影响 |
| **Q6** | `tracked_at_head` 的 `HEAD:<rel>` 在子模块（submodule）路径 / 大小写不敏感文件系统下的语义边界 | 判据只回答「HEAD 里有没有这个路径」，与「目录里有什么」不同（R-1 已接受） | 保持现状（判据语义写进 spec，非 UTF-8 路径接受 git 自身口径）；若实测出现反直觉形态，纳入后续变更，不预置特判 |
| **Q7** | **spec 的「`CreateOutcome` 字段演进 SHALL 走 golden 显式重写流程」与 bindings 侧现状不符**：`desktop-change-create` 与 `desktop-change-worktree` 两处沿用 detail 线的 boilerplate，但 bindings 侧没有重写开关（`authoritative_snapshot()` 是运行时再导出），能「重写」的只有 `crates/core/workflow/tests/golden/` 的 corpus golden | 照字面执行会让实施者找一个不存在的开关（本 change 已因此走过一次弯路） | 设计取「手改精确断言 + 复核产物 diff」（仍满足「显式、非静默」）：spec 措辞改为「出线 DTO 字段演进 SHALL 以显式方式处置——corpus golden 经 `DESKTOP_GOLDEN_REWRITE=1` 重写并人工复核 diff；bindings 产物经 `bindings:export` 重生成 + 精确字段集断言手改 + diff 复核」 |
