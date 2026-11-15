# 测试设计: worktree-remove-orphan-defect

> **变更**: worktree-remove-orphan-defect
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（四现象 / 两段式提案 / D1–D9 裁决 / 变更范围实现文件 5 行 + 测试文件 12 行 / AC-1…AC-10 / R1–R10）、`design.md`（起点核实 / DD1–DD10 / 文案单点面 / 架构组件 4 行 / 新增文件 **0 行** + 修改文件 **5 行** / 公共函数·API **12 行** / 类型定义「无」/ 数据模型 / AC 落点表 / 文案锚点迁移清单）、四份 delta spec（`desktop-change-worktree` 1 ADDED + 1 MODIFIED / `desktop-change-archive` 1 MODIFIED / `desktop-change-delete` 1 MODIFIED / `desktop-change-create` 1 MODIFIED）
> **被测范围**: design「新增文件」零项（全部落点为既有文件内改动）+ design「修改文件」全 5 项。**删除文件零项**（design 无删除面）。
> **本变更性质**: **单点回收自愈化 + 文案双源校正**（零新增 crate / 零依赖变化 / 零 port 签名变更 / 零 wire DTO / 零前端改动）。① `git::remove_worktree` 由「一次 `git worktree remove --force`，失败即 `Err`」改为两段式（护栏 → git 段 → OS 层 `remove_dir_all` → `prune`），`Ok` 语义统一为「磁盘已无该目录」（含缺席幂等），`Err` 面收窄为「两段式皆失败 + 护栏拒绝」并携五段成因；② `ProcessArchiveVcs::remove_worktree` 的自组 wrapper 退役为纯透传（文案单点收口）；③ create 的四处面向用户文案（前置⑨ + 两补偿残留 + 补偿成功确认）经私有 `worktree_reconcile_hint` 改双源；④ `walker::step_cleanup` doc 随动（实现零改动）；⑤ `package.json` 版本行。**测试面的主轴是「同一批断言锚的形态迁移」**：`git_test` 的缺席 / 普通目录两形态由 `Err` 改判 `Ok`，`lib_test` 的「手动命令指引」字面锚退役，`create_test` / `walker_test` 的注入文案改五段双源形态。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**`） | `cargo nextest run --workspace`（`server:test`；本变更定向 `-p vcs-runtime` / `-p workflow` / `-p orchestration` 三包） | **sidecar 同目录同前缀** `*_test.rs`；由 crate 根以 `#[cfg(test)] mod <name>_test;` 挂载（`infra/vcs/src/lib.rs` 内三条 `mod *_test;`）。**本变更零新建测试文件**，全部为既有 sidecar 的追加 / 改造 | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**；装置为**真实 git tempdir 仓**（`init_repo` / `init_empty_repo` / `plain_dir` / `worktree_slot`）+ 手写端口假件（`FakeVcs` / `CreateStore` / `DeleteStore` / `TempRoot`）+ `tempfile` |
| 生成物 / 版本行 | — | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/package.json` **零测试路径**（生成物由 `bindings:export` 零 diff 守住；版本行走 `git diff` 复核） | 登记于「不可测试项」 |

**测试文件面清单（本变更触及 5 个既有 Rust sidecar，零新建）**

| 面 | 新建 | 改造 / 追加 |
|---|---|---|
| Rust | —（零新建） | `crates/infra/vcs/src/git_test.rs`（既有 1 条改造（拆为 2 行：普通目录兜底 + 缺席幂等）+ 13 条新增，其中 2 条 `#[cfg(windows)]` 独占、1 条含 `#[cfg(unix)]` 支、1 条 PATH 注入互斥）；`crates/infra/vcs/src/lib_test.rs`（既有 1 条改造 + 2 条新增）；`crates/core/workflow/src/write/create_test.rs`（既有 3 条改造 + 5 条新增，另 4 条既有用例回归）；`crates/core/workflow/src/write/delete_test.rs`（既有 1 条改造 + 1 条新增 + `FakeVcs` 加一臂，另 8 条既有用例回归）；`crates/core/orchestration/src/walker_test.rs`（既有 1 条改造 + 既有成功用例加一组断言，另 2 条既有用例回归） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、平台分叉、竞态窗口）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」前缀**：该断言由既有用例锁定，本设计**只做回归登记**（不新增断言、不改断言口径）；未带前缀者为新增或按新形状改写。
- **「间接」**：被测目标为**私有**函数（`is_protected_path` / `comparable_path` / `delete_tree` / `prune_worktrees` / `worktree_reconcile_hint` / `compensate_*`），只能经其唯一调用方（`remove_worktree` / `create`）的公共行为观测——**MUST NOT 为其新增 `pub(crate)` / `pub` 出口**（AGENT.md「No test-only exports」；且 `git_test` / `lib_test` 是 crate 根的 sibling 模块，本就看不到 `git` 模块的私有项）。
- **「组合行」**：跨模块链路用例，挂靠到该链路的**发起方 / 最上层调用方**模块章节（见「跨模块组合用例挂靠」），不另立集成测试章节。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `&Path`（`main_root`） | 正常仓根 / 非 git 目录（`prune` 段必失败的静默面）/ 尾部带分隔符 / `\\?\` 前缀形（Windows）/ 大小写变异（Windows）——后三者只为**护栏归一**（`comparable_path`）服务，断言为「同一 `main_root` 的形态差异不改变拒绝结论」 |
| `&Path`（`worktree`） | 正常 worktree（登记与目录一致）/ 已注销孤儿（admin 手删、目录在场）/ 目录缺席（从未存在、已手删、重复调用）/ 非 worktree 普通目录（在场）/ 普通**文件**位点 / 主仓自身 / 主仓祖先 / 主仓子目录 / 文件系统根 / 空路径 `""` / 超长路径（> 260 字符，Windows）/ 路径深度极端（`node_modules` 式深层树） |
| `Option<&Path>`（`worktree.parent()`，`root_hint` 缺父回退） | 「路径无父目录」支被护栏（`target.parent().is_none()` → 拒）**先行排除**，故该回退分支为**不可达防御支**（登记于「不可测试项」，不设计用例） |
| `bool`（`is_protected_path` 返回值，间接） | `true` 四类（自身 / 祖先 / 子目录 / 文件系统根，含空路径）/ `false` 合法 worktree 与合法落位根子目录；**无 `None`**（非 `Option`） |
| `io::Result<()>`（`delete_tree`，间接） | `Ok`（目录在场且可删）/ `Err(NotFound)`（不在场 → 幂等 `Ok`）/ `Err(其他 kind)`（Windows 无 delete 共享的句柄、超长路径以外的 OS 障碍）——三分支各由独立装置触发 |
| `Result<(), String>`（两缝出口） | `Ok` / `Err` 两态 × 文案形态判据四则：五段俱在、**单行**（零 `\n`）、双源短语在场、**祈使短语缺席**（`请手动执行` / `请以 … 对账并手动清理`——DD3：不锚裸命令串，因原文必回显 `git worktree remove --force <path>`） |
| `String`（`add_error` / `fs_error` / 注入的 vcs `Err`） | 注入原文逐字进 `Err`（零改写）；换行 / 超长注入不改写（由消费侧单行渲染面约束，本变更不断言截断） |

**跨模块组合用例挂靠（按链路发起方 / 最上层调用方）**

- **回收单点链**（`git_test` 装置 → `git::remove_worktree` 两段式 → 真实磁盘 + 真实 git 登记）发起方即单点本身 ⇒ 全部两段式行为行写 `crates/infra/vcs/src/git.rs -> git_test.rs` 章节。
- **归档缝透传链**（`ArchiveVcsPort` → `ProcessArchiveVcs::remove_worktree` → `git::remove_worktree`）发起方 = 归档缝实现 ⇒ 「两缝同实现同文案」组合行写 `crates/infra/vcs/src/lib.rs -> lib_test.rs` 章节。
- **补偿链**（`create` → `compensate_tree_failure` / `compensate_add_failure` → `remove_worktree` / `delete_branch` / 建档删除 → 文案出口）最上层调用方 = 写面 `create` ⇒ 组合行写 `crates/core/workflow/src/write/create.rs -> create_test.rs` 章节。
- **删除链**（`delete` → `remove_worktree` → `branch_exists` / `delete_branch` → 产物目录 → 级联删库）最上层调用方 = 写面 `delete` ⇒ 组合行写 `crates/core/workflow/src/write/delete.rs -> delete_test.rs` 章节。
- **归档 Cleanup 链**（run → walker → `step_cleanup` → `ArchiveVcsPort` → 归档收口）最上层调用方 = walker ⇒ 组合行写 `crates/core/orchestration/src/walker.rs -> walker_test.rs` 章节。
- 不新建独立集成测试章节、不新建组合测试区；跨 crate 的「同一文案形态」以**短语在场 / 祈使缺席**二值观测承载（`core/*` 不依赖 `infra/vcs`，无法共享字面）。

---

## 验收范围

<!-- 测试区域：Rust 侧 `packages/desktop/src-tauri/crates/{infra/vcs,core/workflow,core/orchestration}/src/**/*_test.rs`（sidecar，`cargo test`）。本变更零新建测试文件，5 个既有 sidecar 承接全部 per-file 章节。不可测面（版本行 / [vcs] stderr 留痕 / doc 改写 / 零 diff 结构事实 / 不可达防御支）见文末「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **孤儿自愈**：已注销孤儿（git 无登记、目录在场）经 `remove_worktree` → `Ok`、目录从磁盘消失、`git worktree list` 不再含它、`<main>/.git/worktrees/` 无残留 admin；全程一次调用、零重试 | `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` -> `git_test.rs`（孤儿自愈 + 普通目录兜底两条正向）；`crates/core/workflow/src/write/delete.rs` -> `delete_test.rs`（删除面孤儿回收）；`crates/core/orchestration/src/walker.rs` -> `walker_test.rs`（Cleanup 面孤儿 `passed` 断言） |
| AC-2 | **原语义零漂移**：登记与目录一致的 worktree → `Ok`、目录消失、主仓树无损、branch `change/<name>` 留存、同路径可再 `add`；主仓 active 树零触碰 | `crates/infra/vcs/src/git.rs` -> `git_test.rs`（既有四条 remove 族用例零改动通过）；`crates/infra/vcs/src/lib.rs` -> `lib_test.rs`（既有四条归档缝用例零改动通过） |
| AC-3 | **幂等**：对不存在路径调 `remove_worktree` → `Ok`；连续两次调用均 `Ok` | `crates/infra/vcs/src/git.rs` -> `git_test.rs`（缺席 / 二次 / 重复三形态 + `prune` 清残留 admin） |
| AC-4 | **护栏**：path == `main_root` / `main_root` 祖先 / `main_root` 子目录 → `Err` 且磁盘零变化（主仓文件全在） | `crates/infra/vcs/src/git.rs` -> `git_test.rs`（四类 + 空路径 + 归一形态 + 零副作用快照） |
| AC-5 | **`prune` 不误伤**：兜底分支执行后，另一活 worktree 仍在 `git worktree list` 且目录在场 | `crates/infra/vcs/src/git.rs` -> `git_test.rs`（双活 worktree 反向用例） |
| AC-6 | **失败面成因可辨**：两段式皆失败 → `Err` 含绝对路径、原 git 错误、OS 错误与成因清单；且**不含**「`git worktree remove --force <path>`」单源指引；释放占用后重试收敛为 `Ok` | `crates/infra/vcs/src/git.rs` -> `git_test.rs`（Windows 句柄装置 + 释放收敛 + git 不可用两臂）；`crates/infra/vcs/src/lib.rs` -> `lib_test.rs`（wrapper 退役 / 单点透传判据） |
| AC-7 | **文案双源**：create 前置⑨ 的 `Err` 含 worktree 落位根绝对路径与「`git worktree list` + 直接检查落位根子目录」双源措辞；create 两处补偿残留文案同为该形态且不含单源 `git worktree remove --force` 指引 | `crates/core/workflow/src/write/create.rs` -> `create_test.rs`（前置⑨ + 两补偿残留 + 补偿成功面 + 形参面） |
| AC-8 | **尽力步不变**：Cleanup 遇两段式皆失败的 `Err` → 步 `passed`、detail 携成因、Seal / Finalize 照常收口、run `Completed`、branch 零触碰 | `crates/core/orchestration/src/walker.rs` -> `walker_test.rs`（失败 detail 形态随动 + 结构断言零改动 + 单行判据） |
| AC-9 | **删除面收口**：勾选清空对已注销孤儿的 change → `removed_worktree = true`、branch 删除、产物目录与库记录级联；两段式皆失败 → 整体 `Err`、库记录与产物目录原样未动 | `crates/core/workflow/src/write/delete.rs` -> `delete_test.rs`（孤儿回收 + 皆失败中断 + 既有用例回归） |
| AC-10 | **零面漂移**：两 port 签名零 diff、`crates/infra/exec` 零 diff、`bindings:export` 后生成物零 diff、`src-tauri/Cargo.toml` crate 版本行不动、`package.json` version 恰 +1 patch | 结构事实 / 版本行：**零测试路径**（`worktree.rs` / `port.rs` 签名面、`crates/infra/exec/**`、`src-tauri/**`、`packages/desktop/src/**`）→ 登记于「不可测试项」，由 `git diff` 复核承接 |

---

## 单元测试

### `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` -> `packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs`

**既有 sidecar 改造 / 追加**（真实 git tempdir 夹具 `init_repo` / `worktree_slot` / `plain_dir` 原样复用；测试文件以 `use super::git;` 直调 `pub(crate) fn`，四个新助手为模块私有 → 只经 `remove_worktree` 观测，不新增出口）。章节对应架构元素 `App.Backend.Infra.VcsRuntime`（worktree 落位与 git 操作适配）。

#### 待测功能

- `git::remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String>`: 唯一回收单点，体内两段式（护栏 → `git worktree remove --force` → 失败落 OS 层删除 → `prune` → `Ok`）；`Ok` 语义统一为「磁盘已无该目录」（含缺席幂等），`Err` 面收窄为「两段式皆失败 + 护栏拒绝」。签名逐字不变。
- `git::is_protected_path(main_root: &Path, worktree: &Path) -> bool`（私有，间接）: 护栏判据——空路径 / 无父的文件系统根 / `main_root` 自身与其祖先（`main.starts_with(target)`）/ `main_root` 的子目录（`target.starts_with(main)`）四类命中即 `true`，调用点入口即 `Err` 且零副作用；除此黑名单外无形态判据（D4）。
- `git::comparable_path(path: &Path) -> PathBuf`（私有，间接）: 护栏归一（`fs::canonicalize` 优先 → `std::path::absolute` → 原文；Windows 剥 `\\?\` 前缀并小写）——只为让「同一路径的文本形态差异」不改变拒绝结论。
- `git::delete_tree(path: &Path) -> io::Result<()>`（私有，间接）: OS 层兜底删除（Windows 绝对路径且无前缀时包 `\\?\`，POSIX 直删）；零重试、零属性清理；`NotFound` 由调用点读作「不在场」→ 幂等 `Ok`。
- `git::prune_worktrees(main_root: &Path) -> Result<String, String>`（私有，间接）: `git worktree prune`（清「目录已不存在」的残留 admin）；**仅在目标目录不在场之后**调用；失败由调用点 `let _ =` 静默吞（DD5）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `git::remove_worktree` | 正向 | **「既有…」原语义零漂移（AC-2）**：`add_worktree` 建位 → `remove_worktree` → `Ok`、目录消失、主仓 `a.txt` 在场、同路径可再 `add` 成功（既有 `remove_worktree_目录移除主仓无损且同路径可再add` 零改动通过） | 单次 |
| `git::remove_worktree` | 正向 | **「既有…」branch 留存 + 补偿序（AC-2）**：`remove_worktree后分支留存_清理步零触碰` 与 `补偿序组合_add后remove加delete全链主仓回初态` 零改动通过（`git worktree list` 仅主仓一行、branch `change/<name>` 在案、两段式不影响登记清空语义） | 参数化迭代（2 例） |
| `git::remove_worktree` | 正向 | **已注销孤儿自愈（AC-1，新增）**：`add_worktree` 后手动删 `<main>/.git/worktrees/<admin>`（admin 名从该目录 `read_dir` 的唯一子项取，**不写死 basename**）→ 前置断言 `git worktree list` 已不含该位、`git worktree prune` 输出为空 → `remove_worktree` → `Ok`；目录消失、`list` 仍只主仓、`.git/worktrees/` 下该 admin 不在场；`remove_worktree` 恰一次调用（零重试） | 单次 |
| `git::remove_worktree` / `git::prune_worktrees`（间接） | 正向 | **目录缺席但 admin 残留（AC-3 + `prune` 承担面，新增）**：`add_worktree` 后手动删 worktree **目录**（保留 `<main>/.git/worktrees/<admin>`）→ `remove_worktree` → `Ok` 且该 admin 目录被 `prune` 清除（`list` 无该行、`prune` 后无残留） | 单次 |
| `git::remove_worktree` / `git::prune_worktrees`（间接） | 正向 | **`prune` 不误伤活 worktree（AC-5，新增）**：A、B 两个活 worktree 在案 → 只把 A 造成孤儿形态（删 A 的 admin）→ 对 A 走兜底分支 → B 仍在 `git worktree list`、B 目录在场、B 的 admin 子目录仍在（`remove_dir_all` 未被波及） | 单次 |
| `git::delete_tree`（间接） | 正向 | **非 worktree 普通目录兜底删除（AC-1 正面 / 既有用例改造）**：目录在场但 git 报 `not a working tree` → 兜底删除成功 → `Ok`；目录内文件随之消失（原 `remove_worktree缺席路径err原文语义持衡` 形态二由 `Err` 改判 `Ok`） | 单次 |
| `git::remove_worktree` / `git::delete_tree`（间接） | 边界 | **缺席幂等三形态（AC-3）**：① 从未存在路径 → `Ok`（零删除动作、零 panic）② 同路径连续两次 → 两次均 `Ok` ③ 手动删除后再调 → `Ok`（`NotFound` 单判据，DD2——不设 `exists()` 前置） | 参数化迭代（3 形态） |
| `git::is_protected_path` / `git::comparable_path`（间接） | 异常 | **护栏四类（AC-4，新增）**：① `worktree == main_root` ② `main_root.parent()`（祖先）③ `main_root` 内子目录 ④ 文件系统根（POSIX `/` / Windows `C:\`；另加空路径 `Path::new("")`）→ 五形态均 `Err`，文案含该路径与拒绝因（「主仓 … 自身、其祖先、其子目录或文件系统根」） | 参数化迭代（5 形态） |
| `git::is_protected_path` | 正向 | **护栏不削弱自愈面（D4 反向判据）**：合法 worktree（`<落位根>/<name>` 形态，与 `main_root` 同级）与合法落位根子目录 → 不被拒（走两段式并 `Ok`）；即黑名单之外零形态判据 | 参数化迭代（2 形态） |
| `git::is_protected_path` / `git::comparable_path`（间接） | 边界 | **归一形态不改结论（DD1，新增）**：同一 `main_root` 以四形态传入护栏仍全拒——① 原样 ② 尾部带分隔符（`main_root.join("")` 或尾加 `/`）③ `\\?\` 扩展前缀形（`#[cfg(windows)]`）④ Windows 大小写变异（`#[cfg(windows)]`）/ POSIX 经 symlink 指向主仓（`#[cfg(unix)]`，canonicalize 消解） | 参数化迭代（4 形态，平台门控） |
| `git::is_protected_path` | 边界 | **护栏先于副作用（AC-4 的零变化判据）**：护栏命中前后对 `<main>/.git/worktrees/` 的目录项集合取快照 → 逐一相等（证明 git 段未跑过，非仅「目录尚未删」）；主仓 `a.txt` / `.git/` 全在 | 单次 |
| `git::delete_tree`（间接） | 异常 | **两段式皆失败携成因（AC-6，新增，`#[cfg(windows)]`）**：worktree 内文件以 `OpenOptions::new().read(true).share_mode(FILE_SHARE_READ)`（不含 delete 共享，沿 `lockprobe_test` 装置）持有 → `remove_worktree` → `Err`；断言五段俱在：worktree 绝对路径 / `git worktree remove` 失败原文与退出码 / `OS 删除错误` / 五类成因短语（终端 cwd / 编辑器 / 常驻子进程 / 杀毒实时扫描 / 云同步）/ 双源（`① git worktree list` 与 `② 直接检查 … 落位根 … 下的子目录`）；**祈使短语缺席**（无 `请手动执行`、无 `请以 … 对账并手动清理`）；文案**单行**（`!error.contains('\n')`） | 单次 |
| `git::remove_worktree` / `git::delete_tree`（间接） | 正向 | **释放即收敛（AC-6 后半，新增）**：上例 `drop` 句柄后**立即**同一路径再调 → `Ok`、目录消失（零 sleep / 零退避 / 零重试；两次调用间隔无等待窗口） | 单次 |
| `git::delete_tree`（间接） | 异常 | **git 不可用不设分支（DD9，新增）**：PATH 置空（沿 crate `test_path_lock` 互斥窗，`TEST_PATH_LOCK` 已存在装置）→ ① 普通目录在场 → 兜底删除成功 `Ok`（不因 git 缺失拒绝自愈）② 同装置 + Windows 句柄占用 → `Err` 且文案**同时**含 `git 不可用（PATH 未发现 git）` 与 OS 删除错误（原「环境缺失 vs git 语境」判据的落点迁移；真占用臂 `#[cfg(windows)]`） | 参数化迭代（2 形态） |
| `git::delete_tree`（间接） | 边界 | **Windows 超长路径（D9 / `\\?\` 前缀，新增，`#[cfg(windows)]`）**：非 git 的深层目录树（逐层 `create_dir_all` 至绝对路径 > 260 字符，含一个文件）→ `remove_worktree`（git 段必失败）→ 兜底经扩展前缀删除成功 → `Ok`、目录消失 | 单次 |
| `git::delete_tree`（间接） | 边界 | **目标位点是普通文件**：路径处为一个普通文件（非目录）→ `remove_dir_all` 非 `NotFound` 形态 → 结果二值可考（`Err` 携双源 或 `Ok`）；硬判据 = 不 panic、文件内容未被改写、主仓零变化、再次调用结果稳定 | 单次 |
| `git::prune_worktrees`（间接） | 边界 | **`prune` 失败静默（DD5，新增）**：`main_root` 为非 git 目录 + 目标路径缺席 → `prune` 段必失败 → 仍 `Ok`（不升级为 `Err`、不把 git 错误外泄为用户面文案） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（真实 git 子进程 + 真实 tempdir 仓） | 复用既有装置 `init_repo` / `init_empty_repo` / `plain_dir` / `worktree_slot` / `fs_write`；git 子进程是被测面的**真实对象**——替身化会让「git 段失败 → 兜底」这一核心判据空转，且孤儿形态只能由真实 admin 目录手删制造 | 全章节 |
| 不 mock：`std::fs` / 真实磁盘 | 兜底删除、admin 残留、超长路径树、普通文件位点全部由真实文件系统观测（含 `git worktree list` / `.git/worktrees/` 的快照比对） | 全章节 |
| 真实句柄装置（非 mock）：Windows `OpenOptions::share_mode` | `share_mode(FILE_SHARE_READ)` 持 worktree 内文件（沿 `crates/infra/exec/src/lockprobe_test.rs` 的 `hold_with_share` 形态，本文件内自建 helper，不跨模块引用私有装置）——「另一进程持有」的本进程等价面，零新依赖（`std::os::windows::fs`） | 两段式皆失败 / 释放收敛两行 |
| 进程边界注入：PATH 环境变量 | 沿 crate 既有 `TEST_PATH_LOCK`（`lock_path()` 互斥守卫）置空 PATH——「git 不可用」注入窗，既有真件用例同行互斥 | git 不可用两形态 |
| 不 mock：`eprintln!` `[vcs]` 留痕 | stderr 属进程边界，同进程无法断言（`git_impl` 既有留痕同此）；本章节只断言外部可见的 `Ok` / `Err` 与磁盘结果——留痕本身登记于「不可测试项」 | 兜底成功 / 目录缺席两支 |

### `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` -> `packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs`

**既有 sidecar 改造 / 追加**（本文件自带 `init_repo` / `archive_worktree_slot` 装置，不跨测试模块引用 `git_test` 的私有装置）。章节对应架构元素 `App.Backend.Infra.VcsRuntime` 的两个缝实现（`WorktreePort` / `ArchiveVcsPort`）。

#### 待测功能

- `ProcessArchiveVcs::remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>`: 归档缝实现，**修改**为纯透传 `git::remove_worktree`（自组「…请手动执行 `git worktree remove --force {path}` 清理」wrapper 整段退役）——文案单点收口（D8），本缝不再改写 `Ok` / `Err`。
- `ProcessWorktree::remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>`: create / delete 缝，**零改动声明**（已为透传）；两缝同实现同文案的对照面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProcessArchiveVcs::remove_worktree` | 正向 | **「既有…」归档缝四态零漂移（AC-2）**：`process_archive_vcs_remove_worktree目录移除且list不再列出` / `…转发与自由函数效果等值` / `…后分支留存` / `…脏worktree经force仍可移除` 四条零改动通过（目录回收、`&dyn ArchiveVcsPort` 与自由函数效果等值、branch 留存、`--force` 带走脏面） | 参数化迭代（4 例） |
| `ProcessArchiveVcs::remove_worktree` | 正向 | **wrapper 退役 / 单点透传（AC-6，新增）**：`#[cfg(windows)]` 真占用装置（同 `git_test` 形态，本文件自建 helper）→ `Err`；断言含 worktree 绝对路径、`git worktree remove` 失败原文、`OS 删除错误`、五类成因短语、双源短语；**MUST NOT 含**旧 wrapper 前缀 `worktree 移除失败（` 与 `请手动执行`（自组文案整段退役的判据） | 单次 |
| `ProcessArchiveVcs::remove_worktree` | 正向 | **两缝同文案（组合行，D8 反向判据，新增）**：两个同形孤儿目录（各删自身 admin）分别经 `ProcessWorktree::remove_worktree` 与 `ProcessArchiveVcs::remove_worktree` → 两 `Ok`、两目录均消失、`git worktree list` 均无该位；两缝对同一 `Err` 装置（Windows 真占用）产出的文案**除路径外等值**（同一单点，零分歧） | 组合迭代（2 缝 × 2 结果态） |
| `ProcessArchiveVcs::remove_worktree` | 边界 | **缺席 / 普通目录改判 `Ok`（AC-3 + 兜底，既有用例改造）**：`process_archive_vcs_remove_worktree_err携路径与手动指引` 的两形态——① 缺席路径 → `Ok`（幂等，零删除）② 普通目录在场 → `Ok` 且目录消失；`Err` 面迁移至上一条真占用用例 | 参数化迭代（2 形态） |
| `ProcessArchiveVcs::remove_worktree` | 边界 | **归档缝孤儿回收（AC-1 的缝面）**：`add_worktree` → 手删 admin 造成已注销孤儿 → 经 `&dyn ArchiveVcsPort` 调用 → `Ok`、目录消失、`list` 无该位（Cleanup 步的真实受益面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（真实 git tempdir + 真实 `ProcessArchiveVcs`） | 被测对象是无状态薄透传；`git` 子进程与磁盘为真实对象 | 全章节 |
| 不 mock：`&dyn ArchiveVcsPort` 转发 | 经 trait object 调用（既有 `转发与自由函数效果等值` 形态），不替身——「两缝同实现」只能由两个真实入口观测 | 等值 / 两缝同文案两行 |
| 真实句柄装置（非 mock） | Windows `OpenOptions::share_mode(FILE_SHARE_READ)`（本文件内自建 helper，沿 `lockprobe_test` 形态） | wrapper 退役 / 两缝同文案的 `Err` 面 |
| 不 mock：`lib.rs` 其余方法 | 同文件其它 `ArchiveVcsPort` / `WorktreePort` 方法零改动，既有用例零改动通过即回归判据（`git diff` 出现越界 hunk 即失败） | 全章节 |

### `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs`

**既有 sidecar 改造 / 追加**（装置：`Env`（真实 tempdir `root` + 真实 `worktree_root`）+ `FakeVcs`（可编程 `add_worktree` / `remove_worktree` / `delete_branch` 故障）+ `CreateStore` 假件）。章节对应架构元素 `App.Backend.Core.Workflow`。

#### 待测功能

- `workflow::write::create(main_root, store, vcs, project_roots, name, goal, title, workflow_type) -> Result<CreateOutcome, String>`: **零改动声明**（签名与实参序逐字不动）；本变更只改其**面向用户的失败文案**——前置⑨ 保底门槛文案与两处补偿出口文案改双源（AC-7）。
- `worktree_reconcile_hint(worktree_root: &Path) -> String`（私有，间接）: create 侧文案单点（DD4②）——「① `git worktree list` 对账；② 直接检查 worktree 落位根 `<worktree_root>` 下的子目录」双源形态，供前置⑨ / 两处补偿残留 / 补偿成功确认四处共用。
- `compensate_add_failure(store, vcs, main_root, id, branch, worktree_root, add_error) -> String`（私有，间接）: 加形参 `worktree_root`（置于 `branch` 之后）；双故障角落文案改「hint + `branch 侧仍以 git branch -D <branch> 清理`」，`(None, None)` 支文案零改动。
- `compensate_tree_failure(store, vcs, main_root, id, branch, worktree, worktree_root, fs_error) -> String`（私有，间接）: 加形参 `worktree_root`（紧邻既有 `worktree`）；残留面与成功面均改 hint；补偿链次序（`remove_worktree` → `delete_branch` → 删建档）、`worktree_note` 残留行、`residuals.is_empty()` 判据零改动。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workflow::write::create`（前置⑨ 文案，改造既有 `worktree门槛拒绝_8个直属子目录err文案含对账引导`） | 异常 | `worktree_root` 下 8 个直属子目录 → `Err` 含 `worktree 目录数已达保底门槛` / `当前 8` / `门槛 7` / **落位根绝对路径**（`env.worktree_root.display()` 逐字）/ `git worktree list` / `直接检查` / `清理后再创建`；**MUST NOT 含** `请手动执行` 与 `git worktree remove --force`；零副作用断言零改动（`vcs.calls() == ["worktree_root"]`、`create_call_count() == 0`） | 单次 |
| `worktree_reconcile_hint`（间接，前置⑨ 出口） | 异常 | **双源形态横向判据（spec Scenario「残留指引双源」）**：两源措辞同时在场（`git worktree list` ∧ `直接检查`）、且**不以 git 单源收尾**（无 `请以 … 对账并手动清理` 祈使句） | 单次 |
| `workflow::write::create`（前置⑨ 判据与阈值） | 正向 | **「既有…」门槛判据零改动（AC-7 的反向面）**：`worktree门槛恰7个目录照常创建` / `worktree门槛_目录缺席计零通过且普通文件不计入` / `worktree门槛_非notfound读错误显式err` / `db与磁盘漂移_3active加5残留合计8目录被门槛拦` 四条零改动通过（严格大于才拒、`NotFound` 计 0、其余读错误显式 `Err`） | 参数化迭代（4 例） |
| `compensate_tree_failure`（补偿成功面，改造既有 `树写出失败补偿链全下发且残留指引`） | 正向 | worktree 内路径分量占位使 fs 写出失败 → `remove_worktree` / `delete_branch` / 删建档全链成功 → `Err` 含 `已补偿回收` + 双源 hint（落位根绝对路径在场）+ `git worktree list`；**MUST NOT 含** `请手动执行`；既有调用序（`remove` < `delete_branch`）、按 id 删建档、`record_count() == 0` 断言零改动 | 单次 |
| `compensate_tree_failure`（残留面，改造既有 `补偿链再失败呈现残留对象与指引` 的 worktree 支） | 异常 | `remove_worktree` 注入 `Err` → `Err` 含残留行 `worktree <路径>: <注入原文>`（`worktree` 路径与 `worktree_root` **同现且不串位**）+ 双源 hint + `branch 侧仍以 git branch -D change/orphan-name 清理`；**MUST NOT 含** `请手动执行` | 单次 |
| `compensate_add_failure`（残留面，同上既有用例的 add + branch 支） | 异常 | `add_worktree` 注入 `Err` + `delete_branch` 注入 `Err` → `Err` 含 `worktree add 模拟失败` 原文、`branch "change/add-orphan"` 残留与记因、双源 hint、`git branch -D` 保留；**MUST NOT 含** `请手动执行`；建档按 id 补偿删除断言零改动 | 单次 |
| `compensate_add_failure`（`(None, None)` 支） | 正向 | `add_worktree` 注入 `Err` 且无其它故障 → `Err` 为「…（建档与分支已补偿回收）」；**该支零改动判据**：`MUST NOT` 出现双源 hint（文案单点不外溢到本支）、不含残留清单 | 单次 |
| `compensate_tree_failure` / `compensate_add_failure`（新增形参面） | 正向 | **形参正确性**：两处补偿出口的 hint 均含 `env.worktree_root` 的 `display()` 逐字（= `vcs.worktree_root` 派生的落位根），而非 `worktree_root.join(<name>)` 的目标目录路径；`tree` 支文案中「落位根」与「残留行里的 worktree 路径」两个路径同时在场且各自正确 | 单次 |
| `worktree_reconcile_hint`（间接，三出口） | 边界 | **单行与零包转（DD8）**：前置⑨ 与两处补偿三出口的 `Err` 均 `!contains('\n')`；落位根路径按 `display()` 原样插入（Windows 反斜杠不转义、分隔符不归一） | 参数化迭代（3 出口） |
| `compensate_tree_failure`（组合行：补偿链次序） | 边界 | **残留不改变链次序**：`remove_worktree` 注入 `Err` 时 `delete_branch` 与建档删除**仍照常下发**（既有断言零改动）；`residuals` 空与非空两支互斥（同一次调用不可能既报残留又报已补偿） | 单次 |

> **形参随动（零新增用例）**：`compensate_add_failure` / `compensate_tree_failure` 各加一个 `&Path` 形参与两处 call site 补传，属**编译闭合**（`E0061` 由编译器兜住，无运行期中间态）；本表不为其新增断言，实参正确性由「形参面」一行观测。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorktreePort`（`FakeVcs`，被测 API 的显式入参 / 注入依赖） | 既有假件：`worktree_root` 返回预置绝对路径、`add_worktree` / `remove_worktree` / `delete_branch` 按臂注入 `Err`、调用序列可观测 | 前置⑨ 四条 + 两补偿四支 |
| `ChangeStateStore`（`CreateStore`，进程边界＝库） | 既有假件：`seed` / `created_id` / `deletes` / `record_count` / 可编程 `StoreFault` | 补偿链与残留清单断言 |
| 不 mock：`fs`（真实 tempdir） | 真实目录树（`worktree_root` 下 8 / 7 个真实目录、路径分量普通文件占位制造 fs 半边真实失败）——坑位语义只能由真实 `read_dir` / `create_dir_all` 观测 | 前置⑨ 全部 + 树写出失败补偿 |
| 不 mock：`worktree_reconcile_hint` / `compensate_*` | 私有纯字符串装配，真实调用、只观测 `create()` 出口 `Err` 文本（不新增出口、不直调私有函数） | 全章节 |
| 不 mock：`create` 本体 | 章节内全部用例经 `env.create(...)` 公共入口驱动（最上层调用方＝写面） | 全章节 |

### `packages/desktop/src-tauri/crates/core/workflow/src/write/delete.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/delete_test.rs`

**既有 sidecar 改造 / 追加**（装置：`Env`（真实 tempdir 双树）+ `FakeVcs`（可编程 `remove_fault` / 分支在场表）+ `DeleteStore`；`FakeVcs` 加一臂「兜底成功」）。`delete.rs` 实现**逐字不动**（design「公共函数 / API」表「零改动（声明）」行；spec `desktop-change-delete` Module Contract 明确本文件承接覆盖面）——本章节是**消费面验收**（AC-9）：写面不看 git 状态，只看缝的 `Ok` / `Err`。

#### 待测功能

- `workflow::write::delete(main_root: &Path, layout: &Layout, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, id: &str, clean_worktree: bool) -> Result<DeleteOutcome, String>`: 零改动声明（执行序与级联范围逐字不动）；语义随 vcs 缝随动——`Ok` 面新增「已注销孤儿可回收」，`Err` 面收窄为「回收两段式皆失败」且成因由缝提供（写面仅加 `worktree 移除失败（<path>）: ` 前缀）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workflow::write::delete` | 正向 | **孤儿回收（AC-9 / spec Scenario「已注销孤儿经删除面回收」，新增）**：`FakeVcs::with_fallback_recovery()`（内部两步语义：模拟 git 段失败留痕 → OS 段删除目录 → 对外仍 `Ok`）→ 写面 `Ok`；断言 `removed_worktree == true`、branch `change/seed-change` 删除、主仓产物目录消失、`cascade_ids == [id]` 且级联为最后一步、`remove_worktree` **恰一次**（零重试）；**判据要点**：写面不因「git 段曾失败」中断（`Ok` 即终裁） | 单次 |
| `workflow::write::delete` | 异常 | **两段式皆失败中断（AC-9）**：`remove_fault` 注入新形态单行 `Err`（路径 + 原 git 错误 + OS 错误 + 成因清单 + 双源）→ 写面 `Err` = `worktree 移除失败（<path>）: <注入原文>`（`delete.rs` wrapper 逐字不动）；断言含路径与成因片段、`cascade_ids()` 为空、产物目录在场、`deleted_branches()` 为空（branch 零触碰）（改造既有 `worktree移除失败_删除中断且记录与产物目录原样`） | 单次 |
| `workflow::write::delete` | 边界 | **「既有…」目录缺席幂等 + 分支照删**：`删除worktree已缺席_目录跳过但分支照删_幂等重试` 零改动通过（`removed_worktree == false`、`vcs.removed()` 空、branch 照删——`is_dir` 前置判据保留，两段式的缺席 `Ok` 只在其后兜住竞态，DD7） | 单次 |
| `workflow::write::delete` | 正向 | **「既有…」其余八条零改动通过**：勾选清空 / 不勾选保留注记 / legacy 零 vcs 触点 / 归档前缀命中 / 分支不在场零删除 / 分支删除失败中断 / 级联删库失败 `Err` 透出 / 未建档 id 显式拒绝 | 参数化迭代（8 例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorktreePort`（`FakeVcs`，被测 API 的显式入参） | 既有假件 + 新增 `with_fallback_recovery()` 一臂（表达「git 段失败但 OS 段成功 → 对外 `Ok`」）；移除 / 删分支故障注入保持既有形态 | 孤儿回收 / 皆失败两行 |
| `ChangeStateStore`（`DeleteStore`，进程边界＝库） | 既有假件（`with_record` / `cascade_ids` / `record_present_at_cascade`） | 全部行 |
| 不 mock：`fs`（真实 tempdir 双树） | 产物目录 / worktree 目录由真实磁盘判在场（`is_dir` / `exists`）——写面的 `is_dir` 前置判据只能由真实目录观测 | 全部行 |
| 不 mock：`delete` 本体 | 经公共入口驱动，断言 `DeleteOutcome` 三字段与调用序（补偿序 / 级联最后一步） | 全部行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`

**既有 sidecar 改造 + 既有成功用例加断言**（装置：`TempRoot`（真实 tempdir 归档链根）+ `FakeVcs`（`with_real_removal()` / `with_remove_result(Err(…))`）+ `FakeWorker` / `FakeTools` / `StubSnapshot` rig）。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无 walker 行）：本文件只改 step_cleanup 的 doc
  （DD10），执行体 :2770-2807 逐字不动。故本节唯一「待测功能」条目下无签名变更可登记——签名与执行体逐字
  不变，用例全部落在**既有可观测面**（Cleanup 步的 settle 状态 + detail 文本 + run 终态 + vcs 调用序列）
  上，其形态随「注入的 Err 文案」与「回收语义」随动，结构断言零改动。 -->

#### 待测功能

- `step_cleanup(ctx: &StepContext<'_>, trace: &mut EdgeTrace, cursor: &mut PhaseCursor) -> Result<(), Terminal>`（私有，间接）: 归档 Cleanup 步（SpecSync 与 Seal 之间），**doc 随动、实现与签名逐字不变**；语义随 vcs 单点随动——失败面收窄为「两段式皆失败」、detail 逐字转发 vcs `Err` 原文（路径 + 原 git 错误 + OS 错误 + 占用成因清单 + 双源对账）、已注销孤儿经兜底回收后记「已移除 worktree（<path>）」；步恒 `passed`（尽力步，永不 `Terminal`、不回滚归档收口）、branch 全程零触碰、`remove_worktree` 恰一次（零重试）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `step_cleanup`（成功 detail） | 正向 | **「既有…」清理成功收口**：`归档cleanup成功移除_worktree目录消失且链照常收口` 的结构断言零改动通过——Cleanup `passed`、detail「已移除 worktree（<path>）」逐字、Seal「已归档」、Finalize `passed`、零 `phase_log` 节点、run `Completed`、`remove_worktree` 恰一次（`(main_root, worktree)` 双参透传） | 单次 |
| `step_cleanup`（孤儿兜底回收） | 正向 | **已注销孤儿经兜底回收 `passed`（AC-1 / spec Scenario「已注销孤儿经兜底回收成功」，新增断言挂既有成功用例）**：worktree 目录在盘前置（`root.change` 预置）→ 假件 `with_real_removal`（表达「兜底删除成功 → `Ok` 且目录消失」）→ Cleanup `passed` + detail 记「已移除 worktree（<path>）」（**MUST NOT** 因 `not a working tree` 判失败）、Seal / Finalize 照常、run `Completed`、`remove_worktree` 恰一次（零重试） | 单次 |
| `step_cleanup`（失败 detail） | 异常 | **两段式皆失败仅警告不断链（AC-8，改造既有 `归档cleanup失败尽力_警告detail步恒passed且收口不断链`）**：假件注入新形态单行 `Err`（worktree 绝对路径 + `git worktree remove` 失败原文 + `OS 删除错误` + 五类成因短语 + 双源）→ detail 含 `worktree 清理失败` + 路径 + 成因原文片段 + 双源短语；**MUST NOT 含** `请手动执行`；结构断言零改动（步 `passed`、Seal / Finalize 照常、run `Completed`、`call_count("remove_worktree:") == 1`、零 `delete_branch`） | 单次 |
| `step_cleanup`（缺席 / legacy 跳过） | 边界 | **「既有…」两跳过支零改动**：`归档cleanup缺席幂等跳过_零失败面`（detail「worktree 已不在场，跳过清理」、零 remove 调用）与 legacy 支（`worktree=None` → 跳过注记、零 git 触点）零改动通过 | 参数化迭代（2 例） |
| `step_cleanup`（detail 单行渲染） | 边界 | **单行不破行（DD8）**：注入含五段的**单行** `Err` → detail 在「worktree 清理失败（尽力步，不影响归档收口）: 」前缀之后逐字接原文，detail 文本不含 `\n`（walker 不改行、不截断）；成因清单短语不插值进程名 / 文件路径（固定短语判据：含「终端 cwd」「常驻子进程」等词且不含该 worktree 内的文件名） | 单次 |
| `step_cleanup`（组合行：归档全链） | 异常 | **失败不回滚归档（AC-8 收口面）**：失败路径下 run 终态 `Completed`、`ArchiveOutcome` 消费面零变化（Seal detail 与成功形态同形）、branch 全程零触碰（`calls()` 白名单无 `delete_branch`）——与 `remove_worktree` 恰一次共同构成「尽力步语义 + 零重试」判据 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiveVcsPort`（`FakeVcs`，被测链路的显式注入依赖） | 既有假件：`with_real_removal()`（真删目录 + `Ok`）/ `with_remove_result(Err(…))`（注入任意 `Err` 文本）、`calls()` / `call_count(prefix)` 观测 | 成功 / 孤儿 / 失败三行 |
| Worker / Tools / Snapshot 假件 | 既有 rig（`FakeWorker` + `FakeTools::with_archive_preflight` + `StubSnapshot`）——归档确定性相位零 agent 会话 | 全部行 |
| 不 mock：walker / 相位机 / 步收口 | 真实 `spawn_run_with_vcs` 驱动，断言 `history.finish()` 的节点行（`node_rows` / `node_detail`）与 `ChangeRunStatus`——「尽力步」语义只能由真实步收口观测 | 全部行 |
| 不 mock：`fs`（`TempRoot` 真实 tempdir） | worktree 目录 / 归档目录由真实磁盘判在场（假件 `with_real_removal` 真删） | 成功 / 孤儿 / 缺席三行 |
| 不 mock：`git` | 本文件零真实 git 触点（`ArchiveVcsPort` 全替身）；git 登记面不在观测范围（由 `git_test` 承接） | 全部行 |

---

## 不可测试项

| 项 | 原因 | 承接方式 |
|---|---|---|
| `packages/desktop/package.json` 的 `version` 行（design 修改文件第 5 项） | 版本字符串无运行期行为面（零代码分支、零 IO）；「落地时 HEAD 现值 + 1 patch」只能相对比对 | `git diff HEAD -- packages/desktop/package.json` 复核恰 +1 patch（AC-10） |
| `eprintln!("[vcs] …")` 两处降级留痕（兜底成功 / 目录缺席） | stderr 属进程边界：同进程测试无法捕获自身 `eprintln!` 输出；捕获需重定向进程 stderr（装置增量远大于判据价值），既有 `[vcs]` 留痕（保留名跳过 / `commit_all` 幂等收口）同此处理 | 人工核验 / 真机日志观测；外部可见面（`Ok` + 磁盘结果）已在 `git_test` 覆盖 |
| doc 注释改写本身（`git::remove_worktree` doc / `lib.rs` 两缝 doc / `create.rs` 两补偿 doc / `walker.rs::step_cleanup` doc） | 注释不进运行期，零断言面 | `git diff` 复核：doc 有 diff、执行体零 diff（DD10 / ⓪ 逐字不动面） |
| 零 diff 结构事实（AC-10）：`crates/core/workflow/src/write/worktree.rs`（`WorktreePort`）、`crates/core/orchestration/src/port.rs`（`ArchiveVcsPort`）的签名与 `Result` 形状；`crates/infra/exec/**`；`crates/infra/checks/**`；`crates/core/foundation/**`；`src-tauri/src/**`；`packages/desktop/src/**`；`src/types/generated/bindings.ts` | 这些是「不应发生的变化」，不是行为；编译期可见但无运行期判据（签名不变不能靠断言证明） | `git diff` / `bindings:export` 零 diff 复核；`cargo test` 编译通过与既有用例全绿是辅助证据 |
| `root_hint` 的「worktree 路径无父目录」回退支（`（未知：worktree 路径无父目录）`） | **不可达防御支**：`is_protected_path` 的 `target.parent().is_none()`（文件系统根）判据先行拒绝，故该回退分支在任何输入下不可达；为它造用例需绕过护栏（即构造不可达状态） | 代码审查留痕（DD8 已声明「纯防御」） |
| Windows 专属面的 POSIX 对应（两段式皆失败的 `Err` 装置、`\\?\` 超长路径、大小写归一） | POSIX 允许删除被打开的文件 ⇒ 句柄装置不产生 `Err`（R9：平台差异而非逻辑分叉，无 `cfg` 分叉除扩展前缀）；同名 `Err` 面在 POSIX 上无同形装置 | 相关行以 `#[cfg(windows)]` / `#[cfg(unix)]` 门控；POSIX 侧保留平台无关面（孤儿自愈 / 缺席幂等 / `prune` 静默 / 护栏） |
| 真机「杀毒实时扫描 / 云同步 / 编辑器同时在扫或持有 worktree 时归档」 | 非自动化验证面（进程级外部占用者不可在单测内复现） | 判据被 AC-1（孤儿必自愈）与 AC-6（真占用必 `Err` 携成因）双向夹住，见 proposal 验收面口径 |
| 前端 / IPC 面（零命令、零 DTO、零前端文案） | 本变更零改动 ⇒ 无被测面 | 无（零 diff 即判据，见上「零 diff 结构事实」行） |
