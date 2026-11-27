# worktree 执行根的子目录对齐（定稿）

workflow_type: requirement

> 需求原句：新建变更时，若基线 root 属于 git 某个子目录，切换到 worktree 也要将对应子目录作为 root。
> 解读（已确认）：**基线 root = 建档时的 workspace root**（`create_change` / `promote_explore` 的 `root` 入参、`for_root` 的身份锚），它可以是 git 仓库内的子目录。需求即：`exec root = worktree_host / <rel>`，`rel` = workspace root 相对 git 根的偏移；git 根 workspace（`rel = ""`）与今天逐字等价。

---

## 1. 问题：今天子目录 workspace 的 change 是错的

`ChangeRecord.worktree` 一个字段同时承担两个语义（`change_flow/mod.rs:145` 直接拿它当 exec root）：

```
① git 落位目录（可 remove / 可在场判定 / 对得上 git worktree list）
② change 工作面根（cwd / Layout / 检查器 / 沙箱 / 画布）
```

子目录 workspace 下两者首次分叉，现状实际错位：

```
<repo 父>/.worktrees/repo/<name>/        ← exec root = 整棵检出根
├─ openspec/changes/<name>/explore.md    ← create 真写在这（错位）
└─ packages/desktop/openspec/changes/    ← 主仓 layout 认得的位置（空）
```

| # | 破口 | 位置 |
| --- | --- | --- |
| 1 | 产物落在检出根；merge 后产物出现在仓库根 openspec，主仓 `<subdir>/openspec` 里看不到 | `write/create.rs` 的 `resolve(&worktree)` |
| 2 | 相位提示词的 `openspec/changes/…` 相对路径据此写错位 | 同上（cwd = exec root） |
| 3 | archi 双根错位：`<subdir>` 基线 vs 检出根改动侧 | `commands/archi/mod.rs:135-146` |
| 4 | bootstrap 装到检出根；`tests[].root` / `static_analysis` 相对路径同理错位 | `write/create.rs`、`testexec/mod.rs:669` |
| 5 | SDK 沙箱锚 = 检出根 → 子目录 workspace 的 change 反而能改全仓 | 会话 cwd 通道 |

### 1.1 实测：git 在子目录下的作用域（决定改造面）

| 命令（`git -C <subdir>`） | 子目录下 | 影响 |
| --- | --- | --- |
| `status --porcelain` | 全树，路径相对 repo 根 | `dirty()` 不变 |
| `diff --name-only <base>` | 全树 | 编辑集 / 补丁不变 |
| `add -A` | 全树 | `commit_all` 不变 |
| `ls-files -s -z` | **仅 cwd 子树且路径相对 cwd** | `worktree_snapshot` 的 index 缩域 → 归档冲突后验 A/B 失真 |
| `clean -fd` | **仅 cwd 子树** | `restore_baseline` 半域风险 |
| `worktree add/remove <abs>` | 正常 | 落位 / 回收不受影响 |
| `rev-parse --show-prefix` | 直接给偏移（根为空行、尾带 `/`） | 偏移的干净来源 |

---

## 2. 行为契约（裁决台账）

| 编号 | 裁决 | 一句话 |
| --- | --- | --- |
| **D1** 记录承载 | `worktree` 仍是 **host**（检出根）；新增列 **`exec_root`（绝对路径）**；`None` = 与 `worktree` 同值（legacy 记录 + 根 workspace 同形） | 读面零路径拼接、零分隔符/大小写考古 |
| **D2** DTO 出线 | `ChangeDetail` / `ArchivePreflight` / `CreateOutcome` 各加 `execRoot`；`worktree` 保留。展示面两行：`worktree：<host>` + （非空时）`执行根：<execRoot>` | 两条路径各有正当用途：开编辑器看改动 / 对上 `git worktree list` |
| **D3** git 步锚点 | `restore_baseline` **锚 host 全树**；`dirty` / `commit_all` / `rebase` / `ff_merge` / `snapshot` / `remove_worktree` 全锚 host。`RunRequest` 新增 `worktree: Option<String>`（host） | exec root 只管 cwd / Layout / 检查器 / bootstrap |
| **D4** 子目录不在检出 | **显式 Err**，两层：前置 `HEAD:<rel>` 判据（零副作用）+ 后置 `is_dir()` 断言（走既有补偿链）。**不回退** | 回退后的 change 根还是错的，只是把失败推迟 |
| **D5** 「相对 workspace root」的声明 | 基准 = **exec root**；代码零改，改文档八处 | 见 §4.3 |
| **D6** UI 文案 | 见 D2 | — |
| **D7** 落位是否纳入 rel | **不纳入**（定案）：落位恒 `…/<repo>/<name>`，与 workspace 在仓库内的位置无关 | 落位与 workspace 的身份彻底解耦；代价见 §6.3 |

### 2.1 核心不变量（建议写进规格正文）

```
exec root ≡ workspace root 在 worktree 内的对应物
⇒ 任何"相对 workspace root"的声明，在主仓与 worktree 两侧同解
```

仓内四个既有声明因此**全部自动正确**，无需逐个裁决：`projects[].root`（`create.rs` 安装 cwd）、`tests[].root`（`testexec/mod.rs:669` `root_path.join(plan.root)`，root_path = run 的 exec root）、`static_analysis` 相对路径命令（检查器 spawn cwd）、`write_protection.files[].glob`（装配根 = 主仓 workspace root，本就正确）。

---

## 3. 数据与接口

### 3.1 记录与访问器

```
ChangeRecord {
  worktree:  Option<String>   // 不变：<.worktrees>/<repo>/<name>
  exec_root: Option<String>   // 新增：执行根绝对路径；None = 与 worktree 同值
}
```

- **`rel = ""` 时不写新列** → 不变量：*git 根 workspace 的建档记录与今天逐字相同*（写成断言）。
- 访问器单点（放 `workflow::state`，禁止散落 `unwrap_or`）：`exec_root()` = `self.exec_root.or(self.worktree)`。
- 防漂移守卫：`exec_root` 不是 `worktree` 的后代（陈旧行 / 手改库）→ stderr 留痕 + 回落 `worktree`（失败形态退回「今天的错位」，不产生越界路径）。
- 双列不会漂：唯一从零构造点是 `create_change_record`（`store.rs:1569`）；其余写路径（`store.rs:1846/2166/2220`）是「读出记录 → 改一字段 → upsert」，新列自动随行。需补的映射单点：`change_state()`（`store.rs:97`）+ `create_change_record`。

### 3.2 端口：一次 `rev-parse` 取两个值

实测 `git rev-parse --show-toplevel --show-prefix` 一次调用给两行（根 workspace 前缀为空行）。

```
WorktreePlacement { root: PathBuf /* <.worktrees>/<repo> */, rel: String /* `/` 分隔，根为 "" */ }
fn placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>；   // 取代 worktree_root
fn tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>; // D4 前置判据（rel 空恒 true）
```

`rel` **只**参与 exec root 派生与 D4 判据，MUST NOT 参与落位派生（D7 定案：`host = placement.root.join(name)`，与 `rel` 无关）。

- 偏移来源用 `--show-prefix` **而非** `strip_prefix`：canonicalize 的 `\\?\` 前缀、真实大小写、`\` vs `/` 三个坑一次绕开。
- 波及：trait（`write/worktree.rs:26`）+ 真件（`infra/vcs/src/lib.rs:82`）+ 4 个测试假件（`create_test.rs:312`、`delete_test.rs:241`、`promote_test.rs:192`、`corpus_golden_test.rs:393`）+ `vcs-runtime::worktree_root` 与 `lib_test` 三例。

### 3.3 create 写入时序

```
前置①-⑥ → placement(main_root) → 前置⑨ 磁盘门槛 → 【新增】tracked_at_head(rel) 判据
→ host = placement.root.join(name)；exec_root = host.join(rel)
→ 建档（worktree = host；exec_root = rel 非空 ? Some(exec_root) : None）
→ add_worktree(main_root, &host, &branch)
→ 【新增】exec_root.is_dir() 断言，miss ⇒ compensate_tree_failure + Err
→ resolve(&exec_root) 建 change 目录 + explore.md        ← 改用 exec root
→ bootstrap(exec_root, projects)                          ← 改用 exec root
```

### 3.4 读写分工（最终归属）

| 语义 | 字段 | 消费点 |
| --- | --- | --- |
| **host** | `record.worktree`（原样） | `remove_worktree` ×3（`create.rs:507`、`delete.rs:65`、`walker.rs:2916`）、在场判定（`delete.rs:64,77`、`walker.rs:2146,2193,2279,2910`）、merge-first 判定（`archive.rs:96`）、branch 探测 |
| **exec root** | `record.exec_root`（回落） | `change_flow/mod.rs:145`、`queries/detail.rs:243`（locate）/`:342`（DTO）、`commands/changes/mod.rs:146`（read_artifact）/`:182,185`（change_dirs）、`commands/archi/mod.rs:135`、`steps.rs:413-426`（artifact / delta specs） |

`ArchivePreflight`（`state.rs:234-251`）拆两列：`worktree`（host，walker git 步 + merge-first）+ `execRoot`（artifact 探测 + 前端预览）；`archive_context()` 一次返回二元组。

### 3.5 迁移（native_model）

- `ChangeRecord` v4 → **v5**（`from = ChangeRecordV4`），当前形态抄为 `ChangeRecordV4`（`from = ChangeRecordV3`，V3 双向 `From` 保留）。先例：`AgentProviderRecord` v4→v5 加 `efforts`（`model.rs:1431-1500`）——缺列读兼容、零迁移脚本。
- 新列 `#[serde(default)] Option<String>`。
- **切勿 bump `WORKSPACE_STORE_FORMAT_VERSION`（现 3，`model.rs:765`）**：`store.rs:458-470` 的门是「低版本 → 删库重建」，加列走 native_model 链，格式版本一动所有 workspace 库作废。

---

## 4. 改动清单

### 4.1 后端

| 位置 | 改动 |
| --- | --- |
| `core/workflow/src/write/worktree.rs` | trait：`worktree_root` → `placement`；加 `tracked_at_head`（或 `WorktreePlacement.tracked`） |
| `infra/vcs/src/lib.rs` / `git.rs` | `placement()`（一次 `rev-parse --show-toplevel --show-prefix`）+ `tracked_at_head`（`rev-parse --verify --quiet HEAD:<rel>`） |
| `core/workflow/src/write/create.rs` | 前置判据、双写记录、`resolve(exec_root)`、`bootstrap(exec_root)`、后置断言 + 补偿；`bootstrap_worktree_root` → `bootstrap_exec_root`；`BootstrapProject.root` 文档改口径 |
| `core/workflow/src/state.rs` | `ChangeStateRecord.exec_root` + 访问器 + 防漂移守卫 |
| `core/orchestration/src/state.rs` | `ArchivePreflight.execRoot` |
| `core/orchestration/src/steps.rs` | preflight 填两列；artifact / delta 用 exec root |
| `core/orchestration/src/walker.rs` | `RunRequest.worktree`（host）；`step_clean` 锚 host；两处 legacy 代理判据（`:339`、`:711`）换 `request.worktree.is_none()` |
| `core/workflow/src/queries/detail.rs` | locate 用 exec root；DTO 加 `execRoot` |
| `src/commands/{changes,change_flow,archi}/mod.rs` | exec root 解析、read_artifact、change_dirs、archi 改动侧 |
| `infra/store/src/{model,store}.rs` | v5 链 + 两处映射 + `ChangeRecord::new` 签名 |

### 4.2 DTO / 前端

`ChangeDetail` / `ArchivePreflight` / `CreateOutcome` 各加 `execRoot`（无 worktree / 根 workspace 时 `null`）。前端 5 处切 `execRoot ?? worktree`：`change-detail-view.tsx:250`（架构画布）、`:392`（会话 cwd）、`use-change-chat.ts:221-223`、`run-control-panel.tsx:376`（预览，`:340-344/:440` 同步）；展示面两行（`:66-68`；create 成功面同源）。生成物：`bindings:export` → 复核 `bindings.ts` / `bindings.zod.ts` / `bindings/mod_test.rs` 黄金快照。

### 4.3 配置（zero-code，八处措辞）

`config.schema.json:44`、`:53`；`config/src/lib.rs:736`（`..` 拒绝理由：逃出 worktree → 逃出执行根，谓词不变）；`create.rs` 的 `BootstrapProject.root` 文档 / legacy 扫描 / 警告文案；`desktop-workspace-config` 第 17 条；`desktop-change-worktree` bootstrap 段 + 两处 Scenario。schema 镜像是编辑器直读 raw URL，须与实现同 commit。

### 4.4 规格

- `desktop-change-worktree`：落位、双 root 执行语义、bootstrap、merge-first、UI 可见性五段加子目录分支；新增「根 workspace 与子目录 workspace 同解」Scenario；V1 边界第 4 条扩写，新增「上游锁面不可表达」一条。
- `desktop-change-state-store`：`ChangeRecord.exec_root` 语义与升级链。
- `desktop-change-archive`（Cleanup / Clean 锚 host）、`desktop-change-delete`（清空 worktree 用 host）、`desktop-change-orchestration:672,705-708`（Clean 锚点措辞）。
- 版本：`packages/desktop/package.json` patch +1。

---

## 5. 测试清单（新增硬用例）

1. **根 workspace 零差异**：记录不含 `exec_root`，全部路径 / 行为与今天逐字相同。
2. **子目录 workspace 建域四件套**：`worktree` = host、`exec_root` = `host/rel`；`openspec/changes/<name>` 落在 `host/rel` 下；`add_worktree` 目标仍是 host。
3. **前置判据六形态**（§6 表）：未跟踪 / HEAD 已删 / 被 ignore → 前置 Err，零建档零 worktree；跟踪但只含 ignore 文件 → 建档成功；`rel=""` → 零新 spawn；前置命中后置 miss → 补偿回收 + 双源残留指引。
4. **回收只认 host**：`remove_worktree` 收到 exec root 会「删子目录 + 留登记」——用例把这条钉死。
5. **访问器守卫**：陈旧 / 越界 `exec_root` → 回落 host + stderr 留痕。
6. **native_model v5**：V4→V5 往返 + 「V4 行升级后 `exec_root = None`」+ 元数据直证（照 `model_test.rs:1798-1910,2068` 先例）。
7. **端口假件**：4 个 `impl WorktreePort` 同步新签名 + 新方法的 Ok / Err 面。
8. **快照缩域反证**：`worktree_snapshot` 必须锚 host（子目录锚会让 `ls-files` 缩域）。
9. **同仓同名拒绝（D7 定案的守卫）**：两个子目录 workspace（`rel` 不同）各自 create 同名 change ⇒ 第二个在前置⑦ / 前置⑥ 显式 Err，零建档零 worktree；并断言落位路径与 `rel` 无关（同仓异 workspace 的同名落位指向同一路径）。

---

## 6. 留痕与边界

1. **未跟踪资产不随 worktree 复制**：前置只保证 `<rel>` 目录被跟踪；其内若 `.dev-team/` / `openspec/` 被 ignore，worktree 的 exec root 里没有架构资产与语料树（本仓 `.dev-team/` 已跟踪，不受影响）。并入 V1 边界第 4 条（原「config / AGENT.md 冻结」）。
2. **上游锁面不可表达**：`is_valid_project_root` 拒绝 `..` / 绝对路径，子目录 workspace 无法声明仓库根的 lockfile；legacy 扫描也只看 exec root ⇒ 恒落「未识别依赖管理器」。逃生口：`projects: [{ root: "." }]`。放开逃逸 = 拆 cwd 逃逸护栏，不做。
3. **一仓多子目录 workspace 同名 change（D7 定案：落位不纳入 rel）**：落位恒 `…/<repo>/<name>`，与 workspace 在仓库内的位置无关。后果三条，均已知且接受：
   - **同仓两个 workspace 不能各有同名 change**：第二个撞前置⑦（`worktree 目录已存在`）或前置⑥（`branch change/<name> 已存在`）——分支名同样是全仓命名空间。拒绝面显式且零副作用（既有文案已给清理指引），不是静默错用。
   - **落位与身份解耦的正收益**：`worktree` 路径只由「git 根 + change 名」决定，与 workspace 位置无关 ⇒ 换 workspace root（改注册、挪子目录）不改变存量落位，也不产生「同一 change 两个候选目录」的歧义；两源对账（`git worktree list` / `.worktrees/<repo>/`）与 `remove_worktree` 两段式仍是单锚。
   - **规避方式**：跨 workspace 命名唯一即可（change 名本就是用户选的）。若日后确认这是真实痛点，纳入 rel 是独立变更（会改落位形态、存量地址迁移策略与两源对账文案）。
4. **Windows 路径长度**：`host/rel/<name>/…` 更长，逃生口仍是 `core.longpaths`（`archive/2026-10-08-desktop-change-worktree/explore.md:102` 已留痕）。
5. **文案区分**：`rebase_halfway_message` / 残留对账指引一律把 worktree 绝对路径给用户——字段分治后须区分「落位目录」与「执行根」，否则用户 `cd` 过去发现不是自己编辑的地方。

### 触发场景（D4 用例依据）

| 形态 | 是否 miss | 期望 |
| --- | --- | --- |
| 子目录整体未跟踪（本地新建目录即当 workspace） | miss | 前置 Err，零建档零 worktree |
| 子目录在 HEAD 已被删除、工作区仍有未跟踪残留 | miss | 同上 |
| 子目录被 gitignore（如构建产物目录当 workspace） | miss | 同上 |
| 子目录被跟踪但仅含被 ignore 的文件 | 命中 | 建档成功（内部资产缺席属边界 1） |
| `rel = ""`（根 workspace） | 恒命中 | 与今天逐字等价，零新增 spawn |
| 前置命中、检出后目录缺席（竞态 / 磁盘异常） | 后置 miss | 补偿回收 + Err（双源残留指引照旧） |

---

## 附：已否决方案（一行理由）

| 方案 | 否决理由 |
| --- | --- |
| A：`worktree` 改指 exec root + 新列 host | 忘区分时把子目录当 host 交给 `remove_worktree`：git 段失败 → OS 段删子目录 → `Ok`，登记还在、整树留下，**静默**；且 `worktree` 字段不再是 git worktree |
| A3：exec 主字段 + git 注册表反查 host | 两段式回收存在的理由就是「git 已注销的孤儿」——恰在该场景反查不到 host；还要 read-path spawn |
| B：`worktree` 为 host + 新列 `worktree_rel`（相对偏移） | 被「新列包含 root」取代：相对值要在读面拼路径，重新引入分隔符 / 大小写 / 前缀考古 |
| C：不落库，用时 `rev-parse --show-prefix` 重算 | `queries/*` 今天纯 fs 零 vcs；workspace root 挪位即失真 |
| D3-b：`restore_baseline` 锚 exec root | 半域：`reset --hard` 全树 + `clean -fd` 子树 → 域外 tracked 改动被丢、域外 untracked 垃圾留下 |
| D3-c：子根观（Clean + Commit 都圈 `rel`） | 破坏「worktree 全域提交」既有语义，并引入「域外手工编辑被静默漏提交」 |
