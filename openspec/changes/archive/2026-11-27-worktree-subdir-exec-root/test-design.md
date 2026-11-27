# 测试设计: worktree-subdir-exec-root

> **变更**: worktree-subdir-exec-root
> **日期**: 2026-11-27
> **依据**: `proposal.md`（AC-1..AC-14）、`design.md`（组件 / D-1..D-10 / 公共函数与类型定义 / 修改文件 / 测试文件改动面 A–G 组）、本 change `specs/**` 9 个能力 delta
> **范围**: design「修改文件」表（含「零代码复核」行）+「类型定义」「配置」两张子表所登记的源文件；**不含**删除文件与任何被删除的对象。`bindings.ts` / `bindings.zod.ts` 作为生成物本体 MUST NOT 手改，其字段面经 `bindings/mod_test.rs` 的出线断言覆盖
> **测试框架**: Rust = `#[test]` / `#[tokio::test]` + 标准 `assert!` / `assert_eq!`，测试模块与源文件同级（`*_test.rs`，经 `#[cfg(test)] mod *_test;` 挂载），运行器 `cargo nextest run --workspace`；前端 = `vite-plus/test`（`describe` / `it` / `expect` / `vi`，vitest 同面）+ `@testing-library/react`，测试文件与源同级 `*.test.ts(x)`，运行器 `vp test`
> **口径**: 断言风格沿仓库惯例（中文用例名 + 带语义消息的 `assert!`）；本设计的用例只描述范围与判据，**不含**测试运行命令细节（`cargo nextest` / `vp test` / 静态脚本由桌面端承接）；design「测试文件改动面」A–G 组的字面量机械补齐、四处精确键集断言手改、8 份 detail 语料 golden 重写属编译 / 断言随动，不在本设计的用例面（其期望形态已在下列相关章节的用例中声明）
> **本版修订（回溯修正）**: 修正三处不可满足的用例表述——① v4 → v5 两跳要求保 `exec_root`（`ChangeRecordV4` 无此列，与 AC-9「存量 v4 行升级后 `exec_root = None`」冲突）改为「`None` 可无损穿 v4、`Some` 降级即丢」；② 裸仓 `Err` 断言英文 `must be run in a work tree`（该串只是上游 stderr 的**内部匹配键**，不进产品文案）改为「非 git 仓」中文引导；③ 误传锚夹具用 `host/rel` 混合分隔符（`delete_tree` 的 `\\?\` 前缀 → OS 系统错误 123，「删子目录」不可观察）改为「逐段 `join` 构造 + 平台无关主判据」。另在 `## 单元测试` 开头补「夹具与断言纪律」五条，堵住自扫自匹配 / 无关目录充当 `exec_root` / 记录名与查询名不一致 / 跨项连接词子串断言四类夹具与断言缺陷

## 验收范围

<!-- 逐条映射 proposal.md 的 14 条 AC；末行为 design 台账 D5（非 AC，配置文案口径）。
     「被测文件或模块」列只填 design「修改文件」表（含「零代码复核」行）+「类型定义」「配置」两张子表登记的源文件；
     源文件 → 测试文件的映射对见 `## 单元测试` 各节标题（`### 源文件 -> 测试文件`，两侧均为相对仓库根的完整路径），本节不单列测试文件表。
     路径基准：Rust 侧相对 `packages/desktop/src-tauri/`（`crates/**`、`src/**`），前端侧相对 `packages/desktop/`（`src/views/**`、`src/types/**`）。
     纯静态步骤 / 规格文档面 AC 落「—（见「不可测试项」N）」；下列每个源文件至少在一行出现。 -->

| AC ID | 验收条件 | 被测文件或模块 |
| --- | --- | --- |
| AC-1 | 根 workspace 逐字等价：`exec_root` 不写列、`tracked_at_head` 零调用、路径与 DTO 与今天逐字同、既有行为断言零改 | `crates/core/workflow/src/write/create.rs`、`crates/core/workflow/src/write/worktree.rs`、`crates/core/workflow/src/state.rs` |
| AC-2 | 子目录 workspace 建域四件套：`worktree` = host、`exec_root` = host/rel、`explore.md` 落 `exec_root`、`add_worktree` 收 host | `crates/core/workflow/src/write/create.rs` |
| AC-3 | 前置判据六形态 + D7 同仓同名拒绝：拒绝面零副作用、后置 miss 走补偿链（入参 host） | `crates/core/workflow/src/write/create.rs`、`crates/infra/vcs/src/git.rs` |
| AC-4 | 回收与 git 步只认 host：`remove_worktree` 误传 exec root 必可见、`reset` 全域、`clean` 不缩域、`ls-files` 含域外条目 | `crates/core/orchestration/src/walker.rs`、`crates/infra/vcs/src/git.rs`、`crates/infra/vcs/src/lib.rs` |
| AC-5 | 访问器与守卫：单点存在、陈旧 / 越界回落 host 且留痕、`worktree = None` → `None` | `crates/core/workflow/src/state.rs` |
| AC-6 | 读面全覆盖：detail / `read_artifact` / `change_dirs` / `archi_change_summary` / 归档 preflight 两列 | `crates/core/workflow/src/queries/detail.rs`、`crates/core/orchestration/src/steps.rs`、`src/commands/changes/mod.rs`、`src/commands/archi/mod.rs` |
| AC-7 | git 步锚点单点：`restore_baseline` 实参只来自 `request.worktree`，`remove_worktree` 实参只来自记录 `worktree` 列 | `crates/core/orchestration/src/walker.rs`、`crates/infra/vcs/src/git.rs`、`crates/infra/store/src/store.rs` |
| AC-8 | legacy 判据显式化：两处 `root == main_root` → `worktree.is_none()` | `crates/core/orchestration/src/walker.rs`、`src/commands/change_flow/mod.rs` |
| AC-9 | native_model v5：v4→v5 往返逐字段、V4 存量行升级后 `exec_root = None`、格式版本仍 3 | `crates/infra/store/src/model.rs`、`crates/infra/store/src/store.rs` |
| AC-10 | 端口契约同步：trait 两方法 + 4 个假件 + vcs 真件签名一致、`worktree_root` 零残留调用点 | `crates/core/workflow/src/write/worktree.rs`、`crates/core/workflow/src/write/mod.rs`、`crates/infra/vcs/src/lib.rs`、`crates/infra/vcs/src/git.rs` |
| AC-11 | DTO 与绑定：三个 DTO 各带 `execRoot`（根 workspace / legacy → `null`）、`bindings:export` 重生成、精确字段集断言随字段手改 | `packages/desktop/src/types/generated/bindings.ts`、`crates/core/workflow/src/write/create.rs`、`crates/core/workflow/src/queries/detail.rs`、`crates/core/orchestration/src/state.rs` |
| AC-12 | 前端四处根派生：画布 root / 会话 cwd / 新图预览 root 按 `execRoot ?? worktree`；create 成功面按 design Q3 落「零代码复核」、详情头两行 | `src/views/changes/change-detail-view.tsx`、`src/views/changes/agent/hooks/use-change-chat.ts`、`src/views/changes/flow/run-control-panel.tsx`、`src/views/changes/components/change-create-dialog.tsx` |
| AC-13 | 改动集与版本：`package.json` version 恰 +1 patch；`src-tauri/Cargo.toml` / `WORKSPACE_STORE_FORMAT_VERSION` / `openspec/specs/**` / `.dev-team/architecture/**` 逐字节不变 | `packages/desktop/package.json`（版本号与改动集对账属静态步骤，见「不可测试项」①；`src-tauri/Cargo.toml` / 格式版本常量同见①、`openspec/specs/**` 基线零改动见②、`.dev-team/architecture/**` 零差异见⑤） |
| AC-14 | spec delta 形态：9 个能力目录、MODIFIED 整段拷贝后改、每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:`、每目录含 `## Module Contract` | —（规格文档形态，非代码面，见「不可测试项」②） |
| D5（design 台账，非 AC） | 配置文案口径：`projects[].root` 的相对基准由「worktree 根」改「执行根」，zero-code（谓词 / 键集 / 值域零改） | `crates/core/config/config.schema.json`、`crates/core/config/src/lib.rs` |

## 单元测试

> **夹具与断言纪律（跨节共用；第 1–3 条是本轮回溯的根因修正，第 4–5 条封堵同类夹具缺陷）**
> 1. **路径一律逐段构造**：`rel`（`/` 分隔）拆段后逐段 `PathBuf::join`，MUST NOT 用字符串拼接、`format!("{host}/{rel}")` 或带 `/` 的单次 `join`——Windows 上会产出 `C:\host\packages/desktop` 式混合分隔符，`remove_worktree` 的 OS 兜底 `delete_tree` 对其加 `\\?\` 前缀后报系统错误 123（`ERROR_INVALID_NAME`），「执行根子目录被删」因此不可观察。期望串同样以同一 `Path::join` 派生后 `to_string_lossy`；断言优先做 `Path` 相等（组件级，混合分隔符在 `Path` 层等价），MUST NOT 做跨分隔符形态的字符串逐字比较。
> 2. **`exec_root` 夹具必须是 `worktree` 的真后代**：`worktree` 下逐段 `join` `rel` 构造。用「与 `worktree` 无关的 tempdir」充当 `exec_root` 会违反 `ChangeStateRecord::exec_root()` 的真后代守卫 → 访问器回落 `worktree`，用例测得的是回落面而不是执行根面（`archi` / `change_dirs` / `detail` / `preflight` / `change_flow` 各节同）。
> 3. **`Err` 文案断言钉独立短语**：逐形态钉各自的短语（如「从未纳入 git 跟踪」/「已在 HEAD 中删除」/「被 gitignore 排除」），MUST NOT 钉跨项连接词子串（「或已」这类拼接在文案里不存在）；MUST NOT 断言只存在于上游 stderr 的英文串（`not a git repository` / `must be run in a work tree` 是内部匹配键，产品文案是中文引导）。
> 4. **按 name 定位的夹具同名**：`change_dirs` / 候选序一类夹具，参与比对的记录 `name` 必须与查询参数逐字相同（只有 `id` / `status` / 根列不同）；用 `id` 充当 `name` 会查不到候选，用例静默变空。
> 5. **禁止源码自扫自匹配**：以源码文本为判据的用例，被扫文件 MUST NOT 是承载该断言的测试文件本身（断言里的字面量必然自匹配、恒红）；能改成行为判据的一律改行为判据（`bindings` 的「无 golden 重写开关」即此类，处置见「不可测试项」⑨）。

### packages/desktop/src-tauri/crates/core/workflow/src/write/worktree.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

> 本文件无专属 `worktree_test.rs`：trait 契约经消费者侧的假件实现 + create 消费面断言落地（既有布局，不新开测试文件）。同签面迁移的另三处假件见 `write/delete_test.rs` / `write/promote_test.rs` / `tests/corpus_golden_test.rs`（断言意图零改，不在本节展开）。

#### 待测功能

- `WorktreePort::placement()`: 一次 git 派生 —— 返回落位父锚 `root` 与 workspace 相对偏移 `rel`（design「公共函数 / API」· 修改，取代 `worktree_root`）
- `WorktreePort::tracked_at_head()`: `HEAD:<rel>` 在案判据；`rel = ""` 恒 `Ok(true)` 且零 spawn（design「公共函数 / API」· 新增）
- `WorktreePort::worktree_root()`: 公共单点退役（design「公共函数 / API」· 删除）
- `WorktreePlacement`: `{ root: PathBuf, rel: String }`，`rel` 为 `/` 分隔、根 workspace 为 `""`（design「类型定义」）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `WorktreePort::placement` | 正向 | 假件返 `root = <tmp>/.worktrees/repo`、`rel = "packages/desktop"` → create 侧 host = `root.join(name)`、exec_root = `host.join(rel)`，两处取值与假件值逐字一致 | 单点 |
| `WorktreePort::placement` | 边界 | `rel = ""`（空串）→ 执行根派生抽化为 host（`exec_root` 列不写、`CreateOutcome.execRoot = null`） | 单点 |
| `WorktreePort::placement` | 边界 | `rel` 单段（`"client"`）与多段（`"client/app/pkg"`）两形态 → 执行根派生同解，无尾分隔符残留 | 表驱动（2 形态） |
| `WorktreePort::placement` | 边界 | `rel` 超长（>1000 字符多层偏移）→ 原样透传零截断，执行根串 = host + `/` + rel 逐字 | 单点 |
| `WorktreePort::placement` | 异常 | 假件返 `Err`（非 git 仓 / 裸仓 / 空输出三档引导文案）→ create 取落位即 `Err`，零建档零 `add_worktree` 零 branch | 表驱动（3 形态） |
| `WorktreePort::tracked_at_head` | 正向 | 假件返 `Ok(true)` 且 `rel` 非空 → create 照常建档；调用恰一次且实参 `(main_root, rel)` 逐字（捕获面断言） | 单点 |
| `WorktreePort::tracked_at_head` | 异常 | 假件返 `Ok(false)` → 前置第十道 `Err`，零建档零目录零 `add_worktree` 零 branch | 单点 |
| `WorktreePort::tracked_at_head` | 异常 | 假件返 `Err`（fail-closed：问不出来 ≠ 不在案）→ `Err` 透传，零建档零 `add_worktree` | 单点 |
| `WorktreePort::tracked_at_head` | 边界 | `rel = ""` → 假件调用计数恒 0（create 侧整段跳过；AC-1 的「零新增 spawn」判据） | 单点 |
| `WorktreePort::worktree_root` | 异常 | 同签面迁移后 `calls()` 记录中零 `worktree_root` 项（旧方法不再被任何写路径触达；全仓残留对账归静态步骤） | 单点 |
| `WorktreePlacement` | 正向 | 两字段全公开可构造（结构体字面量 `{ root, rel }` 不经构造函数），`rel` 恒 `/` 分隔（`\` 不得拼入 `rel`） | 单点 |
| `WorktreePlacement` | 边界 | `rel = ""` + `root` 组合 → `root.join("")` 语义不产出尾分隔符（出线串与 host 串逐字相等） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `WorktreePort`（假件 `FakeVcs`） | 手写 `impl`：`placement` 返可编程 `WorktreePlacement`，`tracked_at_head` 返可编程布尔 + 调用计数（被测 API 的显式注入依赖） | 全部 placement / tracked_at_head 契约与 create 消费面 |
| `ChangeStateStore`（假件 `CreateStore`） | 手写 `impl`：内存记录 + `creates` / `deletes` 捕获（被测 API 的显式注入依赖） | 建档 / 补偿 / 零建档断言 |
| 文件系统与 git | 不 mock：`tempfile` 真实目录 + 假件 `mirror_tree` 模拟检出 | 目录树 / `explore.md` / 执行根在场断言 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

#### 待测功能

- `write::WorktreePlacement`: 门面再导出（`pub use worktree::{InstallRun, RepoProbe, WorktreePlacement, WorktreePort}`；design「修改文件」）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `write::WorktreePlacement` 再导出 | 正向 | 经 `workflow::write` 出口取型构造 `WorktreePlacement` 并驱动 create → 编译面与行为面同时成立（既有「出口再导出取型」用例同式） | 单点 |
| `write::WorktreePlacement` 再导出 | 边界 | 再导出零新增字段 / 方法：`root` 与 `rel` 两字段公开可读，`rel` 空串语义可辨 | 单点 |
| `write::WorktreePlacement` 再导出 | 异常 | 门面层零包装：假件 `placement` 返 `Err` 时经 `write` 出口取型的消费点原样透传 `Err`（带上下文但不改写文案），`WorktreePlacement` 类型路径不引入第二份定义 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `WorktreePort` / `ChangeStateStore` | 沿用同文件假件（`FakeVcs` / `CreateStore`），不新增替身 | 再导出取型后的行为驱动 |
| 文件系统 | 真实 `tempfile` | 目录树断言 |

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

<!-- design.md 的「公共函数 / API」子表未登记本文件条目（该表只列模块级导出函数 / trait 方法与公共类型方法，`pub(crate)` 不入表，见 design 表头口径）。本文件的改动声明见 design「修改文件」表、「零改动（显式登记）」表与 D-1 / D-2 / AC-7；下列条目逐条对齐该登记面，MUST NOT 从源码导出声明推导条目。 -->

#### 待测功能

- `placement()`: `toplevel()` 扩为一次 `rev-parse --show-toplevel --show-prefix` 取两值 —— git 根（首行）与 `rel`（次行，`trim_end_matches('/')` 归一、根 workspace 为 `""`）；非 git 与裸仓**归同一「非 git 仓」中文引导**（git stderr 的 `not a git repository` / `must be run in a work tree` 只是映射匹配键，MUST NOT 进入 `Err` 文案），空输出单列「git 工作树根解析为空」——三档文案自 `toplevel` 原样迁移（design「修改文件」/ D-1）
- `tracked_at_head()`: `rev-parse --verify --quiet HEAD:<rel>` 的三档退出码判据 —— 0 → 在案、1 → 缺席、其余 / spawn 失败 → `Err`（fail-closed）；`rel = ""` 零 spawn 直返 `true`（design「修改文件」/ D-2）
- `git_raw()`: 新增私有退出码通道，返回 `(code, stdout, stderr)`；`git()` 保持「非零即 Err」现行语义与文案，调用点零改（design「修改文件」）
- `restore_baseline()` / `worktree_snapshot()` / `remove_worktree()`: git 步命令体**逐字不变**，只换调用点传的锚（host 全树）（design「零改动（显式登记）」表 / AC-7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `git::placement` | 正向 | 真实 git 仓、workspace root = git 根 → 返回 `(git 根, rel = "")`（`--show-prefix` 空行归一） | 单点 |
| `git::placement` | 正向 | workspace root = `<repo>/packages/desktop` → `rel = "packages/desktop"`（`/` 分隔、无前导 / 尾随 `/`） | 单点 |
| `git::placement` | 边界 | 深层子目录（`client/app/web`）→ `rel` 逐段保真、无尾 `/`（`trim_end_matches('/')` 归一形态） | 单点 |
| `git::placement` | 边界 | 一次 spawn 取两值：`rev-parse --show-toplevel --show-prefix` 同调用（`--show-prefix` 不再独立 spawn）——以 git 调用计数或输出两行一致性断言 | 单点 |
| `git::placement` | 边界 | CRLF 归一：输出含 `\r` 时 root 与 rel 均无残留空白字符 | 单点 |
| `git::placement` | 异常 | 非 git 目录 → `Err` 含「非 git 仓」+ `git init` 引导（文案自 `toplevel` 原样迁移） | 单点 |
| `git::placement` | 异常 | 裸仓（`git init --bare`）→ `Err` 走「非 git 仓」中文引导（断言钉「非 git 仓」+「git init」）；MUST NOT 断言 `must be run in a work tree` 英文串——它只是上游 stderr 的内部匹配键，原样迁移的是中文引导（与上方非 git 目录同档、可辨于空输出档） | 单点 |
| `git::placement` | 异常 | `--show-prefix` 输出为空行且 `--show-toplevel` 亦空 → `Err` 含「工作树根解析为空」（原样迁移） | 单点 |
| `git::tracked_at_head` | 正向 | 子目录在 HEAD（`rel = "packages/desktop"`，内含至少一个 tracked 文件）→ `Ok(true)` | 单点 |
| `git::tracked_at_head` | 异常 | 子目录整体未跟踪（本地新建目录）→ `Ok(false)`（退出码 1） | 单点 |
| `git::tracked_at_head` | 异常 | HEAD 已删除该子目录、工作区仍有未跟踪残留 → `Ok(false)` | 单点 |
| `git::tracked_at_head` | 异常 | 子目录被 `.gitignore` 命中 → `Ok(false)` | 单点 |
| `git::tracked_at_head` | 边界 | 子目录被跟踪但仅含被 ignore 的文件 → `Ok(true)`（判据只答「HEAD 有没有这个路径」，不答「目录里有什么」；R-1 语义边界） | 单点 |
| `git::tracked_at_head` | 边界 | `rel = ""` → `Ok(true)` 且零 spawn（`TEST_PATH_LOCK` + 清空 PATH 下仍 `Ok(true)`，可证零 spawn） | 单点 |
| `git::tracked_at_head` | 异常 | 退出码既非 0 也非 1（如 128，路径非法 / 仓库损坏）→ `Err`（fail-closed，与 `Ok(false)` 严格可分） | 单点 |
| `git::tracked_at_head` | 异常 | git 不可发现（PATH 注入）→ `Err` 含「git 不可用」引导，不伪装成「未跟踪」 | 单点 |
| `git::tracked_at_head` | 边界 | `rel` 超长（>1000 字符多层路径）→ 参数原样入线，结果稳定不 panic（`Ok` / `Err` 之一且文案可读） | 单点 |
| `git`（退出码通道） | 正向 | `git()` 的非零退出仍 `Err` 且文案「git `<args>` 失败（退出码 N）: stderr」逐字保留（新增私有退出码通道不改既有语义） | 单点 |
| `git`（退出码通道） | 边界 | 退出码 0 且 stdout 为空 → 各调用点既有空面语义不变（`placement` 空输出仍 `Err`） | 单点 |
| `git::restore_baseline` | 正向 | 子目录 workspace 夹具（host = 整棵检出、`rel = "packages/desktop"`）：传 host → `reset --hard` 覆盖**整个** worktree（域外 tracked 改动被恢复） | 单点 |
| `git::restore_baseline` | 边界 | 同一夹具传 exec root（host/rel 子目录）→ `reset --hard` 仍全域生效但 `clean -fd` **缩域**（域外 untracked 残留留存）——与传 host 的对照面形成可辨差异（AC-4 / R-2） | 单点（误传锚形态） |
| `git::worktree_snapshot` | 正向 | 子目录 workspace 夹具传 host → index 条目含域外条目（`ls-files -s` 不缩域） | 单点 |
| `git::worktree_snapshot` | 边界 | 传 exec root → index 仅含 `rel` 子树条目（缩域形态可见，证明锚点不可替换） | 单点（误传锚反向对照） |
| 归档 / 工作区 git 步命令体 | 正向 | `restore_baseline` / `worktree_snapshot` / `remove_worktree` / `commit_all` / `dirty` / `rebase_branch` / `ff_merge` 的命令串与语义逐字不变——既有用例零改动通过 | 回归集（7 方法） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| git 子进程 | 不 mock：`tempfile` 真实仓 + 真实 `git` 子进程（进程边界依赖保持真实） | `placement` / `tracked_at_head` / git 步全域用例 |
| `PATH` 全局变量 | `TEST_PATH_LOCK` 串行化 + 临时改写 PATH（进程级全局变量边界，仓库既有先例） | git 不可发现面 |

### packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs

#### 待测功能

- `vcs_runtime::placement()`: 落位派生单点（`git::placement` + `worktree_root_from_toplevel` 组合，`rel` 原样透传；`ProcessWorktree::placement` 委托之）（design「公共函数 / API」· 新增）
- `ProcessWorktree::tracked_at_head()`: 委托 `git::tracked_at_head`（design「公共函数 / API」· 新增）
- `vcs_runtime::worktree_root()`: 公共入口退役（design「公共函数 / API」· 删除；`worktree_root_from_toplevel` 保留为私有纯派生）
- `ProcessArchiveVcs::restore_baseline` / `remove_worktree`: 委托单点不变，回收锚由调用方给定（design「修改文件」/ D-3）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `vcs_runtime::placement` | 正向 | git 根 workspace → `.root` = `<父目录>/.worktrees/<仓库目录名>`、`.rel = ""` | 单点 |
| `vcs_runtime::placement` | 正向 | 子目录 workspace（`repo/client/app`）→ `.root` 仍锚 git 根（与根 workspace 同值）、`.rel = "client/app"` | 单点 |
| `vcs_runtime::placement` | 异常 | 非 git 目录 → `Err` 含「非 git 仓」（与 create probe 同口径） | 单点 |
| `vcs_runtime::placement` | 边界 | `WORKTREES_DIR_NAME` 仍为 `".worktrees"`（落位形态零改，常量单点） | 单点 |
| `ProcessWorktree::placement`（trait 委托） | 正向 | 经 `&dyn WorktreePort` 动态分发与自由函数 `placement` 同仓双跑逐字等值（`root` 与 `rel` 两值对拍） | 单点 |
| `ProcessWorktree::tracked_at_head` | 正向 | 委托对拍：trait 面结果 == `git::tracked_at_head` 自由函数结果（在 HEAD / 未跟踪两态） | 表驱动（2 形态） |
| `ProcessWorktree::tracked_at_head` | 边界 | `rel = ""` → `Ok(true)`（零 spawn 语义随实现侧兜底，`PATH` 注入下仍成立） | 单点 |
| `ProcessWorktree::tracked_at_head` | 异常 | 非 git 目录 → trait 面 `Err` 原文透传（不包装不吞，与自由函数同串） | 单点 |
| `vcs_runtime::worktree_root` | 异常 | 公共入口退役：本 crate 内 `use super::{placement, …}` 面成立、旧名不可达（编译面 + 全仓零残留对账归静态步骤） | 单点 |
| `worktree_root_from_toplevel`（私有纯派生） | 边界 | 四例保留：无父目录 `Err` / 可复现与隔离 / change 名零清洗 / 落位形态三路径（既有断言意图零改） | 表驱动（4 例） |
| 误传锚的整树锚（AC-4 行为钉） | 异常 | 夹具：真实主仓 + `git worktree add` 建位 host（含 tracked 文件），并在 host 下**逐段 `join`** `rel` 预置执行根子目录与内容。主判据（平台无关、必断）：把 exec root 传给 `ProcessArchiveVcs::remove_worktree` 后 ① host 根目录与其检出文件仍在场，② `git worktree list` 仍列出 host（回收零生效、登记残留可见）；对照面传 host → 整树回收且登记注销。执行根子目录自身是否被 OS 兜底删除**不作硬断言**（兜底删除受路径形态影响：混合分隔符 + `\\?\` 前缀 → 系统错误 123；生产回收恒传 host，该形态不是实现缺陷） | 单点（误传锚形态） |
| `ProcessArchiveVcs::restore_baseline` | 正向 | 转发对拍与主仓双保险真件锚零改（既有两例断言意图零改，锚点由调用方给定） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| git 子进程与仓 | 不 mock：`init_repo` 真实仓装置 + worktree 分置 tempdir；host 与执行根一律**逐段 `join`** 构造（纪律 1） | 全部正 / 异常用例 |
| `PATH` 全局变量 | `TEST_PATH_LOCK` 串行化（进程级全局变量边界，既有先例） | git 不可发现面 |
| 档案 vcs 端口 | 经 `&dyn ArchiveVcsPort` 与自由函数双入口对拍（不引入替身） | 委托等值锚 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

<!-- design.md 的「公共函数 / API」子表未登记本文件条目（`create` 签名不变）。本文件的改动声明散在 design「类型定义」表（CreateOutcome.exec_root）与「修改文件」表（前置第十道 / 后置断言 / 执行根锚 / bootstrap 口径 / D7 拒绝面）；下列待测功能与用例以这两处声明为对象。 -->

#### 待测功能

- `CreateOutcome.exec_root`: `Option<String>`，camelCase 出线 `execRoot`，声明序紧跟 `worktree`（design「类型定义」/ AC-11）
- `create()`: `vcs.placement` 取 host 与 `rel`（前置⑨ 磁盘门槛扫描源改 `placement.root`）（design「修改文件」· D-3）
- `create()`: 前置第十道 `tracked_at_head`（`rel` 非空才调用；miss → 零副作用 `Err`）（D-3 / AC-3 / R-13）
- `create()`: 建档双写 `worktree`（host）与 `exec_root`（`rel = ""` → `None`）（D-3 / AC-2）
- `create()`: `add_worktree` 目标恒 host（AC-2 / AC-4）
- `create()`: 后置 `exec_root.is_dir()` 断言（`rel` 非空才做、MUST 早于 `write_fs_half`；miss ⇒ `compensate_tree_failure` 入参 host）（D-3 / R-3）
- `create()`: `resolve(&exec_root)` + `write_fs_half` + `bootstrap(&exec_root, …)`（AC-2）
- `create()`: bootstrap 相对基准改执行根（`bootstrap_exec_root` / `bootstrap_project` 警告文案「在执行根内不存在」）（D5）
- `create()`: D7 同仓同名拒绝（落位与 branch 同为全仓命名空间）（AC-3 附加 / R-7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `create()` · AC-1 根 workspace | 正向 | `rel = ""` → 记录 `exec_root = None`、`CreateOutcome.execRoot = null`、`explore.md` 与 change 目录树落 host（worktree） | 单点 |
| `create()` · AC-1 根 workspace | 边界 | `rel = ""` → `tracked_at_head` 调用计数 0、后置 `is_dir` 断言跳过（vcs 调用序无新增项） | 单点 |
| `create()` · AC-1 根 workspace | 正向 | `rel = ""` → 既有九道前置、执行段、补偿链、警告序与文案逐字不变（既有行为断言零改通过） | 回归集 |
| `create()` · AC-2 双列建档 | 正向 | `rel = "packages/desktop"` → 记录 `worktree` = `placement.root.join(name)` 的字符串形态、`exec_root` = `host.join(rel)` 的字符串形态（两值均绝对）。期望值 MUST 以同一 `Path::join` 派生后比对（`rel` 内的 `/` 在 Windows 上原样保留；字符串手拼 `format!("{host}/…")` 会造出另一种形态）；比较维度取 `Path` 相等 + 两串可辨 | 单点 |
| `create()` · AC-2 建域落位 | 正向 | `rel` 非空 → `openspec/changes/<name>/explore.md` 落 `exec_root` 下；主仓 `<repo>/<rel>/openspec/changes/` 与 `worktree/openspec/` 两侧均无该目录 | 单点 |
| `create()` · AC-2 落位目标 | 正向 | `rel` 非空 → `add_worktree` 收到 `(main_root, host, branch)`（`adds` 捕获面断言目标为 host 而非 exec root） | 单点 |
| `create()` · AC-2 出线两串 | 正向 | `CreateOutcome.worktree` 与记录 `worktree` 同串、`execRoot` 与记录 `exec_root` 同串（一次铸出、记录与 DTO 各持克隆；两串可辨、互不替代，期望值同样以 `Path::join` 派生） | 单点 |
| `create()` · 执行根派生 | 边界 | `rel` 单段（`"client"`）与多段（`"client/app"`）→ 执行根派生同解且两列同源 | 表驱动（2 形态） |
| `create()` · 执行根派生 | 边界 | `rel` 超长（>1000 字符）→ 记录与 DTO 的 exec root 串原样透传零截断、不 panic | 单点 |
| `create()` · 前置第十道形态一 | 异常 | 子目录整体未跟踪（`tracked_at_head → Ok(false)`）→ `Err` 逐条钉：`rel` 原样出现、「常见成因三类」、三形态各自的**独立短语**（「从未纳入 git 跟踪」/「已在 HEAD 中删除」/「被 gitignore 排除」）、两条引导（「纳入 git 跟踪并提交」/「改用仓库根」）、尾注「本次未建档、未建 worktree、未铸分支」；MUST NOT 用跨项连接词子串（如「或已」）作断言。零建档零目录零 `add_worktree` 零 branch | 单点 |
| `create()` · 前置第十道形态二三 | 异常 | HEAD 已删除（`Ok(false)`）与 gitignore 命中（`Ok(false)`）两夹具 → 两次 `Err` **逐字相等**（同一判据同形拒绝、不逐形态分叉），且各自短语仍在文案中并列；形态差异只由夹具体现，不进入文案分支 | 表驱动（2 形态） |
| `create()` · 前置第十道形态四 | 正向 | 子目录被跟踪但仅含被 ignore 的文件（`Ok(true)`）→ 建档成功（`exec_root` 列写入） | 单点 |
| `create()` · 前置第十道形态五 | 边界 | `rel = ""` → 判据整段跳过（零调用，见 AC-1 行） | 单点 |
| `create()` · 前置第十道形态六 | 异常 | 前置命中但检出后执行根缺席（假件 `add_worktree` 提供「不镜像 `rel` 子树」的可编程形态）→ `compensate_tree_failure` 入参 host（`remove_worktree` 收到 host 串）、`Err` 含「已补偿回收」与双源残留指引；零 worktree 零 branch 零建档 | 单点 |
| `create()` · 前置第十道 fail-closed | 异常 | `tracked_at_head` 返 `Err` → `Err` 透传、零建档零 `add_worktree`（文案与 `Ok(false)` 的拒绝面可辨） | 单点 |
| `create()` · 后置断言时序 | 边界 | 后置 `is_dir` 断言早于 `write_fs_half`：同一「不镜像 `rel` 子树」假件形态下不得被 `create_dir_all` 建出执行根（以「store 零残留 + host 已回收」而非「建域成功」收口） | 单点 |
| `create()` · D7 同仓同名拒绝 | 异常 | 同仓第二个子目录 workspace 建同名 change → 撞前置⑦（worktree 目录已存在）或前置⑥（branch 已存在），拒绝面零建档零新目录 | 单点 |
| `create()` · 前置⑨ 扫描源 | 边界 | 落位根下已有 8 个直属子目录（`placement.root` 为扫描源）→ 门槛 `Err` 文案含双源对账引导（既有门槛用例随扫描源迁移零改） | 回归集 |
| bootstrap 锚执行根 | 正向 | `projects = [{ root: "." }]` 且 `rel` 非空 → 安装 cwd = `exec_root`（`installs` 捕获断言） | 单点 |
| bootstrap 锚执行根 | 正向 | `projects = [{ root: "packages/desktop" }]` 且 `rel = "packages/desktop"` → 安装 cwd = `exec_root/packages/desktop`（相对执行根同解） | 单点 |
| bootstrap 锚执行根 | 异常 | 项目根在执行根内不存在 → 警告「项目根 "x"：在执行根内不存在，跳过依赖引导」（文案执行根口径） | 单点 |
| bootstrap 锚执行根 | 边界 | `projects` 空 + `rel` 非空 → legacy 扫描锚 = `exec_root`（lockfile 在执行根内命中、在 host 根不命中） | 单点 |
| `CreateOutcome.exec_root` | 正向 | serde 线形状恰六键且 `id` 首字段（声明序即出线序：`id` / `name` / `created` / `worktree` / `execRoot` / `warnings`），键集排序后为 `created` / `execRoot` / `id` / `name` / `warnings` / `worktree` | 单点 |
| `CreateOutcome.exec_root` | 边界 | `exec_root = None` → `execRoot` 键恒在场且值为 `null`（不省键）；`Some("…")` → 字符串出线 | 表驱动（2 形态） |
| `CreateOutcome.exec_root` | 边界 | Debug 形态字段清单含 `execRoot`（五 → 六字段，`starts_with("CreateOutcome { id: ")` 断言零改） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `WorktreePort`（假件 `FakeVcs`） | 手写 `impl`：可编程 `placement` / `tracked_at_head`（布尔 + 调用计数）、`add_worktree` / `remove_worktree` / `delete_branch` / `run_install` 可编程故障与捕获（被测 API 的显式注入依赖） | 六形态判据、补偿链、bootstrap、落位目标断言 |
| `ChangeStateStore`（假件 `CreateStore`） | 手写 `impl`：内存记录 + `creates` / `deletes` 捕获（被测 API 的显式注入依赖） | 建档双列 / 零建档 / 补偿删除 |
| 文件系统 | 不 mock：`tempfile` 真实目录树 | 目录树 / `explore.md` / 后置断言时序 / 双源对账文案 |
| 时钟 / uuid | 不 mock：沿用 `now_millis()` + uuid v7（既有装置），`created` 以「今天」断言 | DTO 出线形态 |

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- `ChangeStateRecord::exec_root()`: 执行根单点 —— `worktree = None` → `None`；`exec_root = None` → `Some(worktree)`；真后代 → `Some(exec_root)`；越界 / 陈旧 → `[workflow]` 留痕 + `Some(worktree)`（design「公共函数 / API」· 新增 / D-4 / AC-5）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `ChangeStateRecord::exec_root()` | 正向 | `worktree = Some(host)`、`exec_root = Some(host/rel)`（真后代）→ 返回 `exec_root` 原值 | 单点 |
| `ChangeStateRecord::exec_root()` | 边界 | `exec_root = None` → 回落 `Some(worktree)`（legacy 与根 workspace 同形） | 单点 |
| `ChangeStateRecord::exec_root()` | 边界 | `worktree = None`（legacy 记录）→ `None`（即使 `exec_root` 亦为 `None`） | 单点 |
| `ChangeStateRecord::exec_root()` | 异常 | `exec_root == worktree`（非真后代，陈旧行形态）→ 回落 `Some(worktree)` | 单点 |
| `ChangeStateRecord::exec_root()` | 异常 | `exec_root` 为 `worktree` 的祖先（越界）→ 回落 `Some(worktree)` | 单点 |
| `ChangeStateRecord::exec_root()` | 异常 | `exec_root` 为无关绝对路径（异盘 / 同级另一目录 / `/` / 相对路径）→ 全部回落 `Some(worktree)` | 表驱动（4 形态） |
| `ChangeStateRecord::exec_root()` | 边界 | `exec_root = Some("")`（空串）→ 非真后代 → 回落 + 留痕 | 单点 |
| `ChangeStateRecord::exec_root()` | 边界 | 分隔符混用：`worktree = "C:\\a\\b"`、`exec_root = "C:\\a\\b/c"` → 组件级比较判为真后代（不误回落） | 单点 |
| `ChangeStateRecord::exec_root()` | 边界 | 尾分隔符：`exec_root = "host/rel/"` → 仍判真后代（组件级比较，不因尾 `/` 误判） | 单点 |
| `ChangeStateRecord::exec_root()` | 边界 | 深层真后代（`host/a/b/c`）与单段真后代（`host/a`）→ 均命中 | 表驱动（2 形态） |
| `ChangeStateRecord.exec_root` 字段面 | 边界 | 声明序 `worktree` 之后、`base_commit` 之前；结构体字面量补 `exec_root: None` 后既有字段面断言零改 | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| —（纯函数面） | 零 mock：直接构造 `ChangeStateRecord` 值，路径比较走真实 `std::path` | 全部访问器用例 |
| stderr | 不 mock / 不捕获：留痕面以返回值回落为可观察判据，`[workflow]` 前缀与文案以源码字面量对账（见「不可测试项」③） | 越界 / 陈旧留痕 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- `ChangeRecord::new()`: 末位追加 `exec_root: Option<String>` 参数（design「公共函数 / API」· 修改）
- `ChangeRecord`（v5）`exec_root` 列: `#[serde(default)] Option<String>`，声明序尾部追加（design「类型定义」/ AC-9）
- `ChangeRecordV4`: 今日形态整体改名（`from = ChangeRecordV3`，双向 `From` 保留），`pub(crate)`（design「类型定义」）

> **断言边界**：`Some(exec_root)` 跨 v4 两跳**不保真**且不得如此断言——升级半边 `From<ChangeRecordV4>` 恒给 `exec_root = None`（与 AC-9「V4 存量行升级后 `exec_root = None`」是同一条语义）；两跳往返只在 `exec_root = None` 时无损。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| v4 → v5 升级链 | 正向 | `exec_root = None` 的 v5 记录经 `ChangeRecordV4::from` → `ChangeRecord::from` 两跳后与自身逐字段相等（`None` 是唯一能无损穿过 v4 的取值；`ChangeRecordV4` 无该列） | 单点 |
| v4 → v5 升级链 | 正向 | V4 存量行升级后 `exec_root = None` 且其余字段原值（逐字段断言） | 单点 |
| v4 → v5 升级链 | 正向 | V4 载荷字节经 native_model 版本机制解码 → 升级为 v5 且 `exec_root` 为空（照 `AgentProviderRecord` v4→v5 先例装置，零手搓字节） | 单点 |
| v5 → v4 降级 | 正向 | `exec_root = Some(...)` 形态降级后该列被丢弃（v4 无此列），其余字段逐字段保真；再升回 v5 归 `None`（降级形态不作数据承诺） | 单点 |
| 版本头锚点 | 边界 | 现代形态 `native_model(id = 9, version = 5)`、`ChangeRecordV4` 为 `version = 4` 且 `from = ChangeRecordV3`（v3 = 3，同 id 无跳级） | 单点 |
| `ChangeRecord::new()` | 正向 | 末位实参 `Some("…")` 与 `None` 两态可构造，`exec_root` 落位正确、其余字段与 `title = name` 语义零改 | 表驱动（2 形态） |
| `exec_root` 列 serde 面 | 边界 | `None` / `Some` 两态往返相等；缺列（JSON 无该键）→ `None`（`#[serde(default)]` 读兼容） | 表驱动（3 形态） |
| `exec_root` 列 serde 面 | 边界 | 超长串（>1000 字符）与分隔符混用（`\` 与 `/` 并存）原样保存往返、零归一 | 表驱动（2 形态） |
| `exec_root` 列 serde 面 | 异常 | 类型违例（`exec_root` 给数字 / 对象 / 数组，手改 payload 形态）→ 反序列化 `Err`（不静默吞、不猜形态） | 表驱动（3 形态） |
| v3 → v4 → v5 三级链 | 边界 | v3 行（缺 `title` 与 `exec_root`）升级 → `title = name`、`exec_root = None` | 单点 |
| `WORKSPACE_STORE_FORMAT_VERSION` | 边界 | 常量仍为 `3`（格式版本零 bump；库按低版本删库重建的红线） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| native_model 编解码 | 不 mock：真实 `native_model::decode` / `encode`（进程边界内库自身） | v4 载荷字节升级用例 |
| 文件系统 | 不 mock：`tempfile` 真实库文件（`inject_old_shape_change_row` 既有装置） | 存量行升级 / 重开库 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- `change_state()`: 记录 → 中性快照补 `exec_root` 搬运（design「修改文件」/ D-4）
- `create_change_record()`: 中性快照 → 记录补 `exec_root` 搬运（同上）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `create_change_record()` | 正向 | 建档携 `exec_root = Some(host/rel)` → `find` / `list` 与中性快照（`change_state`）三面逐字段一致 | 单点 |
| `create_change_record()` | 正向 | 重开库后 `exec_root` 仍一致（落库往返） | 单点 |
| `create_change_record()` | 边界 | `exec_root = None`（根 workspace / legacy）→ 两向映射均 `None`，不臆造 `Some(worktree)` | 单点 |
| `change_state()` | 边界 | `worktree` 与 `exec_root` 双列并存且可辨（互不覆盖、两串各自原样） | 单点 |
| 其余 change 写路径 | 异常 | `start_change_phase` / `log_change_phase` / `apply_change_backtrack` / `set_change_archived` / 启动标定（读记录改一字段 upsert）→ `exec_root` 原地随行不被清空 | 表驱动（5 路径） |
| 其它记录族 | 边界 | `SessionRecord` / `ExploreRecord` 等非 change 写路径零改动（`exec_root` 不渗入他表） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| 库文件 | 不 mock：`tempfile` 真实 redb 库（`Env` 既有装置，含重开库形态） | 全部映射往返用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- `ArchivePreflight.exec_root`: `Option<String>`，camelCase 出线 `execRoot`，声明序紧跟 `worktree`（八 → 九字段）（design「类型定义」/ D-6 / AC-11）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `ArchivePreflight.exec_root` | 正向 | 九字段 camelCase 线面逐字段可辨，serde 往返逐字段相等（`worktree` = host、`execRoot` = host/rel 两值均可辨） | 单点 |
| `ArchivePreflight.exec_root` | 边界 | `exec_root = None` → `execRoot` 键恒在场、值为 `null`（键不省） | 单点 |
| `ArchivePreflight.exec_root` | 边界 | `exec_root = Some` → 字符串原样出线（不在 DTO 面做回落，回落归访问器 / 前端） | 单点 |
| `ArchivePreflight.exec_root` | 边界 | 声明序：`execRoot` 紧邻 `worktree`（TS 出线序 `… / worktree / execRoot / branch / mergeTarget`） | 单点 |
| `ArchivePreflight.exec_root` | 异常 | 类型违例（`execRoot` 给数字 / 对象）→ 反序列化 `Err`（封闭面不静默吞） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| —（纯 DTO 面） | 零 mock：结构体字面量 + serde_json / specta 真实往返 | 全部字段面用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

- `run_archive_preflight()`: 双列填写 —— `worktree` = 记录 host 原样、`exec_root` = 记录原始列；`branch` / `merge_target` 口径零改（design「修改文件」/ D-6）
- `missing_artifacts()` 根参数改吃**执行根访问器值**（参数名 `worktree` → `exec_root`，签名形态与级数不变）
- `detect_delta_specs()` 根参数改吃执行根访问器值；「主仓优先、worktree 回退」两级序保留（第二级换执行根）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `run_archive_preflight()` | 正向 | 子目录 workspace 夹具（`worktree = host`、`exec_root = host` 下逐段 `join` `rel` 的**真后代**）→ `preflight.worktree == host`、`preflight.execRoot` = 该真后代串（两列各自可辨、互不替代） | 单点 |
| `missing_artifacts()` | 正向 | 三件产物写在 `exec_root` 的 change 目录 → `missingArtifacts` 为空（不再出现「产物全缺」假阴性）；host 根下同形目录不存在 | 单点 |
| `missing_artifacts()` | 异常 | 三件全缺 → 清单恰 `proposal.md` / `design.md` / `tasks.md` 三行（定位失败 = 全缺语义零改） | 单点 |
| `detect_delta_specs()` | 正向 | `exec_root/openspec/changes/<name>/specs/<capability>/spec.md` 在场 → `deltaSpecs` 命中且字典序 | 单点 |
| `detect_delta_specs()` | 边界 | 两级序：主仓有同名 change 与 specs、执行根亦有 → 取主仓（第一级）；主仓无 → 回退执行根 | 表驱动（2 形态） |
| `detect_delta_specs()` | 边界 | `specs/` 在场但 capability 目录无 `spec.md` → 不入清单（列表为空而非错误） | 单点 |
| `detect_delta_specs()` | 边界 | 超大目录（>100 个 capability）→ 全量入清单且严格字典序（大列表边界） | 单点 |
| `run_archive_preflight()` | 边界 | 根 workspace（记录 `exec_root = None`）→ `execRoot = null`；探测锚回落 `worktree`（= host），产物命中同今日 | 单点 |
| `run_archive_preflight()` | 边界 | legacy（`worktree = None`）→ `worktree` / `execRoot` / `branch` / `mergeTarget` 全 `null`；探测锚主仓根 | 单点 |
| `run_archive_preflight()` | 异常 | 执行根目录被删（陈旧列）→ 访问器回落 host，产物按 host 全缺、`execRoot` 仍出线原始列；不 panic 不 `Err` | 单点 |
| `run_archive_preflight()` | 异常 | `exec_root` 指向 host 外（手改库）→ 访问器回落 host，探测不读域外；`execRoot` 出线原始越界值（两口径可辨） | 单点 |
| `run_archive_preflight()` | 异常 | db 读失败 / 未建档 / 已归档 → 既有 `None` 兜底语义零改（读面不伪造载荷） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `ChangeStateStore` | 不 mock：真实 `Store`（`TestDb` / `ArchiveEnv` 既有装置） | 记录读取 / 完成度核算 |
| `ArchiveVcsPort`（假件 `FakeVcs`） | 手写 `impl` 记录调用（被测 API 的显式注入依赖） | `merge_target` 探测与「探测面零 git 依赖」断言 |
| 文件系统 | 不 mock：`tempfile` 真实目录树（change 目录 / specs 子树） | 产物与 delta specs 探测 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- `RunRequest.worktree`: `Option<String>`（host；legacy → `None`；进程内字段不出线）（design「类型定义」/ D-7）
- `step_clean()`: 早退判据与 `restore_baseline` 实参改 `request.worktree`（host 全树）（D-7 / AC-4）
- legacy 判据显式化：initialize 相位短路与 Clean 双保险改 `request.worktree.is_none()`（D-7 / AC-8）
- 归档 git 步 / Cleanup 回收锚：`archive_context()` 返回 `(name, host)` 逐字不变，`commit_all` / `rebase_branch` / `ff_merge` / `remove_worktree` / 半程 `worktree_snapshot` 全锚 host（D-6 / AC-4 / AC-7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `step_clean()` | 正向 | `request.worktree = Some(host)`、`request.root = host/rel` → `restore_baseline` 实参 = host（假件捕获面断言，MUST NOT 为 exec root） | 单点 |
| `step_clean()` | 正向 | 子目录 workspace 夹具（host 整树 + `rel` 子树）→ 域外 tracked 改动被恢复（`reset --hard` 全域） | 单点（行为锚，与 git_test 真件用例双向） |
| `step_clean()` | 正向 | 同上夹具 → 域外 untracked 残留被清（`clean -fd` 不缩域） | 单点（行为锚） |
| `worktree_snapshot`（归档半程） | 正向 | 传 host → index 条目含域外条目（`ls-files` 不缩域） | 单点 |
| `step_clean()` | 边界 | `request.worktree = None`（legacy）→ Clean 步 `passed` + 跳过因、零 `restore_baseline` 调用 | 单点 |
| initialize 相位短路 | 边界 | `request.worktree = None` → 整相位 `skipped`（`TXT_SKIP_LEGACY_INITIALIZE`）、零 vcs 调用零检查 spawn（主仓零重置） | 单点 |
| legacy 判据显式化 | 异常 | `request.root != request.main_root` 但 `worktree = Some(..)` → 不短路（判据只看 `worktree.is_none()`；`root` 与 `main_root` 相等性不再是代理） | 单点 |
| legacy 判据显式化 | 异常 | 源码级自证：`walker.rs` 代码行内零 `root == request.main_root` 形态的 legacy 代理（两处判据均为 `worktree.is_none()`） | 单点 |
| `step_clean()` | 异常 | `request.worktree = Some(host)` 但 `restore_baseline` 返 `Err` → 节点 `failed` + terminal 文案含「基线恢复失败」 | 单点 |
| `RunRequest.worktree` | 边界 | `Some("")`（空串形态）→ 仍视作「worktree 在场」（`is_none()` 只看 Option），下游失败显式可辨、不静默按 legacy 跳过 | 单点 |
| 归档 git 步锚点 | 正向 | 子目录 workspace 夹具下全链归档：`commit_all` / `rebase_branch` / `ff_merge` / `remove_worktree` / 半程 `worktree_snapshot` 收到的路径均为 host（调用序 + 实参逐项断言） | 表驱动（5 步） |
| 归档 git 步锚点 | 异常 | `walker.rs` 代码行内 `restore_baseline(` 实参只来自 `request.worktree`、`remove_worktree(` 实参只来自 `archive_context` 的 host；两处均无 `exec_root` 出处（AC-7 的 source 断言） | 单点 |
| `RunRequest` 构造面 | 边界 | 全仓 `RunRequest { }` 构造点补 `worktree: None` 后既有断言意图零改（`walk_run` 行为面回归） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `ArchiveVcsPort`（假件 `FakeVcs`） | 手写 `impl`：`restore_baseline` 载荷捕获、`commit_all` / `rebase_branch` / `ff_merge` / `remove_worktree` / `worktree_snapshot` 调用记录（被测 API 的显式注入依赖） | 全部锚点与调用序断言 |
| `ChangeStateStore` | 不 mock：真实 `Store`（既有 `TestDb` 装置） | 相位落账 / 归档载荷 |
| 文件系统 | 不 mock：`tempfile` 真实 host / exec root 双目录夹具 | 域内外改动与残留的行为锚 |
| 源码文本（AC-7 / AC-8 自证） | 不 mock：`env!("CARGO_MANIFEST_DIR")` + 读 `src/walker.rs` 去注释行（仓库既有 `steps_test` 先例） | 实参单点与 legacy 代理零残留 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- `ChangeDetail.exec_root`: `Option<String>`，取**原始列**（legacy / 根 workspace → `null`），声明序紧跟 `worktree`（即 DTO 末位）（design「类型定义」/ D-5）
- `change_detail()`: `locate_change` 回退参改 `record.exec_root()`（design「修改文件」/ D-5 / AC-6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `change_detail()` | 正向 | 子目录 workspace 夹具（`worktree = host`、`exec_root = host` 下逐段 `join` `rel` 的**真后代**、产物写在 `exec_root` 的 change 目录）→ `artifacts` 命中、`source = Active` | 单点 |
| `ChangeDetail.exec_root` | 正向 | `detail.worktree` = host、`detail.execRoot` = host/rel（两值逐字来自记录、互不替代） | 单点 |
| `ChangeDetail.exec_root` | 边界 | 根 workspace（`worktree = Some(host)`、`exec_root = None`）→ `execRoot = null`、`worktree` 照常出线（前端只渲染一行） | 单点 |
| `ChangeDetail.exec_root` | 边界 | legacy（`worktree = None`）→ `execRoot` 与 `worktree` 均 `null`，键恒在场 | 单点 |
| `ChangeDetail.exec_root` | 边界 | 序列化键序：`execRoot` 与 `worktree` 相邻、居 DTO 末位（语料 golden 的字典序落 `created` 与 `id` 之间） | 单点 |
| `change_detail()` | 异常 | `exec_root` 指向 host 外（手改库）→ 访问器回落 host，产物自 host 定位；`execRoot` 仍出线原始越界值（原始列 vs 访问器两口径可辨） | 单点 |
| `change_detail()` | 异常 | `exec_root` 指向 host/rel 但目录已被删 → 定位全 miss，detail 仍可达（`source` 自 `record.status` 映射）、`execRoot` 原样出线 | 单点 |
| 语料 golden 随动 | 边界 | 8 份 detail 语料 golden 各增 `"execRoot": null`（`DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工复核 diff；`corpus-list-mixed.json` 零变化） | 单点（机械随动，非新用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `ChangeStateStore` | 不 mock：真实 `Store` 台账（`Env` 既有装置） | 记录读取与子表读面 |
| 文件系统 | 不 mock：`tempfile` 真实主仓 / worktree / exec root 三处目录树 | 产物发现与定位 miss 形态 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- `change_flow_start_with()`: 前置校验 3 的 exec root 解析改 `record.exec_root()`（`is_dir` 校验沿用，Err 文案名词改「执行根目录」并携路径）（design「修改文件」/ D-7）
- `RunRequest` 装配: `worktree: record.worktree.clone()`（host 直供，MUST NOT 从执行根反推）（D-7 / AC-7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| exec root 解析 | 正向 | 子目录 workspace（`worktree = host` 在场、`exec_root = host` 下逐段 `join` `rel` 的**真后代**且在场、`openspec` 树在 `exec_root`）→ 发起成功，summary `running` | 单点 |
| exec root 解析 | 正向 | 同夹具 → 相位半边落 **workspace root 库**，数据根 `workspaces/` 子树仅一个库文件（`for_root` 身份不被执行根污染；既有断言零改） | 回归集 |
| `RunRequest` 装配 | 正向 | `RunRequest.root` = `exec_root`、`RunRequest.worktree` = 记录 `worktree`（host）——两值可辨且各自来源正确（组合根注入面观察） | 单点 |
| exec root 解析 | 异常 | `exec_root` 目录缺席（`host/rel` 被删、host 仍在）→ `Err` 含「执行根目录不存在」+ 路径 + 恢复引导；零 run 登记 | 单点 |
| exec root 解析 | 边界 | `worktree = None`（legacy）→ exec root = 主 workspace root，不经存在性校验（既有 legacy 用例零改） | 单点 |
| exec root 解析 | 边界 | `exec_root` 越界（手改库指向 host 外的存在目录）→ 访问器回落 host → 解析为 host（不产生越界 cwd） | 单点 |
| exec root 解析 | 边界 | `exec_root = Some("")`（空串）→ 非真后代 → 回落 host | 单点 |
| 前置校验既有面 | 异常 | 未建档 id / 未知 `workflow_type` / 并行 run 冲突 → 既有文案与零副作用语义零改 | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `WorkspaceStores` / 库 | 不 mock：真实库 + `MockRuntime` app 装置（既有 `Env` / `app_with`） | 记录读取与库身份断言 |
| agent 组合根 | 不 mock：走真实组合根装配（executor 轮 `ConfigMissing` 显式收敛，不 spawn 进程） | 发起链路面 |
| 文件系统 | 不 mock：`tempfile` 真实 host / exec root 目录树 | 执行根在场 / 缺席形态 |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- `read_artifact()`: `locate_change` 回退参改 `record.exec_root()`（design「修改文件」/ AC-6）
- `change_dirs()`: 两处候选供给改 `record.exec_root()`（`push_worktree_candidate` 复用，探测序与去重不变）
- `create_change` / `create_change_with`: `CreateOutcome.execRoot` 透传（写面 DTO 直通，零代码，仅复核抵达）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `read_artifact()` | 正向 | 子目录 workspace：产物写在 `exec_root`（host 的真后代）的 change 目录 → envelope 非 `null` 且内容逐字 | 单点 |
| `change_dirs()` | 正向 | active 记录（`worktree = host`、`exec_root = host` 下逐段 `join` `rel` 的**真后代**且在场）→ 候选首项为该真后代，四条 exec 读命令（`exec_output_path` / `read_exec_command` 等）命中 `exec_root` 内文件 | 单点 |
| `change_dirs()` | 边界 | 同名 active + archived 记录并存：两条记录的 `name` 与查询参数**逐字相同**（如都是 `dupe`，只有 `id` / `status` / 根列不同）；active 的 `exec_root` 在场、archived 的 `worktree` 指向陈旧目录（其 `exec_root = None` 经访问器回落）→ 候选序：非 archived 的 `exec_root` 优先 → archived 的陈旧 host → `None`（主仓）；去重语义不变 | 表驱动（3 候选） |
| `change_dirs()` | 边界 | `exec_root = None`（legacy / 根 workspace）→ 候选仅 `worktree(Some host)` 与 `None` 两档（行为零变化） | 单点 |
| `change_dirs()` | 异常 | `exec_root` 越界（手改库）→ 访问器回落 host，候选不含域外路径 | 单点 |
| `read_artifact()` | 异常 | 未建档 id / kind 未注册 / source 非法 → `None`（既有语义零改） | 回归集 |
| `create_change_with()` | 正向 | 成功面 DTO 序列化键集恰六键（排序 `created` / `execRoot` / `id` / `name` / `warnings` / `worktree`），`execRoot` 值 = host 下逐段 `join` `rel` 的串（期望值以 `Path::join` 派生，非字符串手拼） | 单点 |
| `create_change_with()` | 正向 | test-only 类型同六键（第二处键集手改），`workflow_type` 透传语义零改 | 单点 |
| `create_change_with()` | 边界 | 根 workspace 创建 → `execRoot = null`（键在场），`worktree` 与落位形态与今天逐字一致 | 单点 |
| 落位派生调用点 | 边界 | 测试内 `vcs_runtime::placement(ws_root).root`（原 `worktree_root`）→ 落位断言串与记录 `worktree` 逐字相等（B 组调用点迁移） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `WorkspaceStores` / 库 | 不 mock：真实库 + `MockRuntime` app（既有 `Env` / `app_with`） | 记录读取 / 建档 / 候选序 |
| `WorktreePort` | 不 mock：真实 `ProcessWorktree` + 真实 git tempdir（`as_git_repo` 既有装置） | 建域与落位串断言 |
| 文件系统 | 不 mock：`tempfile` 真实主仓 / worktree / exec root 三处目录树 | 产物命中 / 候选过滤 |

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- `archi_change_summary()`: active 改动侧改 `record.exec_root().map(str::to_owned)`；`Archived → None` 分支保持（归档恒回落主仓根）（design「修改文件」/ D-5 / AC-6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `archi_change_summary()` | 正向 | 夹具：`worktree` = host tempdir、`exec_root` = host 下**逐段 `join`** `rel` 的子目录（**真后代**），架构模型改动写在 `exec_root` 内、主仓根模型为基线 → 摘要以 `exec_root` 为改动侧，新增 / 删除元素与实盘一致。MUST NOT 用与 host 无关的 tempdir 充当 `exec_root`（真后代守卫会回落 host，测得的是回落面） | 单点 |
| `archi_change_summary()` | 边界 | `status = Archived` → 改动侧恒主仓根（两侧同一 → `None` 摘要；既有断言零改） | 单点 |
| `archi_change_summary()` | 边界 | active + `exec_root = None`（根 workspace）→ 改动侧 = `worktree`（host）= 执行根，与今天同解 | 单点 |
| `archi_change_summary()` | 异常 | `exec_root` 越界 / 陈旧（手改库）→ 访问器回落 host（不读域外模型） | 单点 |
| `archi_change_summary()` | 异常 | 执行根目录缺席（`host/rel` 被删）→ 改动侧模型为空 → 元素全为删除（既有「worktree 缺席」同形语义） | 单点 |
| `archi_change_summary()` | 异常 | `change_id` 未建档 → `Err` 附 id；空白 / 不可读 root → `Err`（既有语义零改） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `WorkspaceStores` / 库 | 不 mock：真实库 + 既有 `seed_record` 装置 | 记录读取 |
| 架构模型扫描 | 不 mock：`tempfile` 真实 `.dev-team/architecture` 目录树 | 两侧模型 diff |
| `ArchiveVcsPort` | 既有 panic 式替身（命令面不应触达 vcs；沿用现状） | 归零断言 |

### packages/desktop/src-tauri/crates/core/config/config.schema.json -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

#### 待测功能

- `projects.description` 与 `projects.items.properties.root.description`: 文案「相对 worktree 根」→「相对执行根」；键集 / 类型 / `default` / `minLength` / 其余描述逐字不变（design「配置」表 / D5 zero-code）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| schema 文案 | 正向 | `include_str!` 镜像解析后两处 `description` 含「执行根」且不含「相对 worktree 根」 | 单点 |
| schema 文案 | 异常 | 镜像内除两处 `description` 外零 diff：`projects.type` / `default: []` / `items.properties.root.minLength: 1` / 其它描述逐字不变 | 单点 |
| schema 契约镜像 | 异常 | 镜像损坏形态：副本删 `projects` / 把 `projects` 改成数组 → 既有取值器给出可指认 panic 文案（不静默通过——文案比对断言） | 表驱动（2 形态） |
| schema 契约镜像 | 边界 | 既有 A1 键集 / A2 枚举 / A3 往返三面零改通过（键集与实现 `KNOWN_KEYS` 双向一致） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| schema 文件 | 不 mock：`include_str!("../config.schema.json")` 编译期绑定入仓文件 | 全部镜像断言 |
| 变体断言 | 内存副本就地改写（磁盘文件零改动） | 反向 / 边界行 |

### packages/desktop/src-tauri/crates/core/config/src/lib.rs -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

#### 待测功能

- `is_valid_project_root()`: 注释理由口径改「逃出执行根」，**谓词零改**（`..` / 绝对路径 / `:` / 通配符仍拒绝；`.` 等价根）（design「修改文件」/ D5）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `is_valid_project_root()` | 正向 | 合法 root（`"."` / `"packages/desktop"` / `"./apps/web"`）→ 通过（既有断言零改） | 表驱动（3 形态） |
| `is_valid_project_root()` | 异常 | `".."` / `"a/../b"` / 绝对路径 / `"C:\\x"` / `"a*"` / `"a?"` / `"{a}"` / `"[a]"` / `"a:b"` → 拒绝 + `InvalidValue` 诊断（既有断言零改） | 表驱动（9 形态） |
| `is_valid_project_root()` | 边界 | 空串 / 纯空白 → 拒绝（`minLength` 之外的第二道防线） | 表驱动（2 形态） |
| `is_valid_project_root()` | 边界 | 超长单段 root（>1000 字符）→ 谓词不 panic，按组件判定（不校验存在性） | 单点 |
| `is_valid_project_root()` 注释口径 | 边界 | 源码文本断言：`lib.rs` 不含「逃出 worktree」字面量，含「逃出执行根」（`env!("CARGO_MANIFEST_DIR")` 读源文件，同 `steps_test` 先例） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| 配置文件 | 不 mock：`TempWs` 真实临时 workspace 目录（既有装置） | `load` / `validate` 面 |
| 源码文本 | 不 mock：读 `src/lib.rs` 去注释行 | 注释口径断言 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- design「公共函数 / API」表对本文件零行；下列条目取自 design「类型定义」表本文件行（三 DTO 各增 `execRoot: string | null`）与「修改文件」表 bindings 行 / D-9。生成物本体（bindings.ts / bindings.zod.ts）MUST NOT 手改；其字段面经运行时再导出产物断言落地。同目录 zod_test.rs 只覆盖 zod 渲染机制，不承载 DTO 字段集，故不纳入映射。 -->

#### 待测功能

- `CreateOutcome.execRoot`: 生成物段内 `execRoot: string | null`，出线序 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings`（`worktree` 语义钉为 host）（design「类型定义」/ D-9）
- `ChangeDetail.execRoot`: 段内 `execRoot: string | null`，声明序紧邻 `worktree`（DTO 末位）（design「类型定义」/ D-5）
- `ArchivePreflight.execRoot`: 段内 `execRoot: string | null`，紧跟 `worktree`（`… / worktree / execRoot / branch / mergeTarget`）（design「类型定义」/ D-6）
- `authoritative_snapshot()`: 字段集断言的唯一通道 —— 每次运行时先 `export_bindings()` 再读产物（行为面可观察）；「无 golden 重写开关」属设计约定、不可源码自证（见「不可测试项」⑨）（design C 组第二处 / D-9 / Q7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `CreateOutcome` 出线字段集 | 正向 | `field_names` 恰 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings`（声明序 = 出线序，`id` 首字段）——五 → 六字段 | 单点 |
| `CreateOutcome` 出线形态 | 正向 | 段内含 `execRoot: string \| null`；`id: string` / `worktree: string` / `warnings: string[]` 形态零改 | 单点 |
| `ChangeDetail` / `ArchivePreflight` | 边界 | 两类型段各含 `execRoot: string \| null`，且 `execRoot` 与 `worktree` 相邻（声明序） | 表驱动（2 类型） |
| 断言通道 | 边界 | 「运行时再导出」以行为收口：字段集断言读到的是本次 `export_bindings()` 的产物（新列 `execRoot` 能读到即为证）。MUST NOT 写「扫源文件找 `DESKTOP_GOLDEN_REWRITE` 不存在」的用例——断言字面量与被扫文件同居一处即自匹配、恒红；「无重写开关」属设计面约定，见「不可测试项」⑨ | 单点 |
| `archive_preflight` presence 循环 | 异常 | 既有逐字段 presence 循环受新增列影响为零、断言零改（证明「presence 循环 ≠ 精确集合」两侧职责分离） | 单点 |
| 出线快照取段 | 异常 | 段缺失形态：`CreateOutcome` 被更名 / 段落取不到 → `type_section` 取不到段即断言失败（fail-closed，不静默空过）；产物非 UTF-8 → `expect` 文案可指认 | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| bindings 产物 | 不 mock：真实 `export_bindings()` 运行时再导出（进程边界内的生成管线） | 全部字段集断言 |
| 并发 | 既有 `lock()` 串行化（进程内全局变量边界） | 快照读写 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

- `DetailHeader`: `execRoot` 非空 → 第二行「执行根：<execRoot>」（`data-testid="detail-exec-root"`）；为空 → 零占位、零空行（design「修改文件」/ D-8 / AC-12）
- 详情视图单点派生 `execRoot = detail.execRoot ?? detail.worktree`，供 `archiRoot`（`archive` 或 `null` → `root`）与 `ChangeChatPanel.cwd` 两处消费（D-8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `DetailHeader` 两行 | 正向 | `execRoot = "D:\\host\\rel"`、`worktree = "D:\\host"` → 头部呈现两行（`detail-worktree` + `detail-exec-root`），执行根行文案逐字 | 单点 |
| `DetailHeader` 两行 | 边界 | `execRoot = null`（根 workspace / legacy）→ 仅 `worktree` 一行、`detail-exec-root` 零渲染、header 子元素计数零空行 | 单点 |
| `DetailHeader` 两行 | 边界 | `worktree = null` 且 `execRoot = null` → 两行皆无（零占位） | 单点 |
| `archiRoot` 派生 | 正向 | active + `execRoot` 非空 → 架构页 `ArchitecturePanel.root = execRoot`（非 worktree） | 单点 |
| `archiRoot` 派生 | 边界 | `source === 'archive'` → `archiRoot = root`（归档恒回落主仓根，`execRoot` 在场亦不用） | 单点 |
| `archiRoot` 派生 | 边界 | active + `execRoot = null` + `worktree` 非空 → `archiRoot = worktree` | 单点 |
| `ChangeChatPanel.cwd` | 正向 | active + `execRoot` 非空 → 面板 `cwd = execRoot` | 单点 |
| `ChangeChatPanel.cwd` | 边界 | `execRoot = null` + `worktree = null`（legacy）→ `cwd = null`（后端回落 root） | 单点 |
| 派生单点 | 异常 | 前端不做第二套越界校验：`execRoot` 为任意非空串即原样使用（回落职责在后端访问器，双处校验会漂） | 单点 |
| `DetailHeader` 两行 | 异常 | 旧形状应答（`detail.execRoot` 为 `undefined`，不经类型面）→ 派生回落 `worktree`，不渲染「执行根：undefined」；detail 取数失败 / `null` → 既有降级页零改 | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `@tauri-apps/api/core` 的 `invoke` / `Channel` | `vi.hoisted` mock（IPC 进程边界依赖，既有装置） | 详情取数 / run 活面 |
| 路由 | `MemoryRouter` 真实组件（不 mock） | 深链与根切换 |
| 子面板（画布 / 会话 / 抽屉） | 不 mock：真实组件或既有 `mountMaterials` 装置，按 props 观测 | 三处根派生断言 |

### packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts -> packages/desktop/src/views/changes/agent/hooks/use-change-chat.test.ts

<!-- design「公共函数 / API」表对本文件零行：D-8 / Q4 定稿为「参数面形态不变、`cwd = archived ? null : worktree` 行为逐字不变」，改动只在参数文档口径（值由详情视图按 execRoot ?? worktree 解析后经面板 cwd prop 送入）。下列条目取自 design「修改文件」表本文件行，用例是该口径变更后的回归锚。 -->

#### 待测功能

- `cwd` 派生: 会话 `cwd = archived ? null : worktree`；其中 `worktree` 参数的语义改「已解析的执行根来源（`detail.execRoot ?? detail.worktree`，由详情视图经面板 `cwd` prop 送入）」，行为与归一职责逐字不变（design「修改文件」/ D-8 / Q4）
- `worktree` 入参: 单值参数面形态不变（`change-chat-panel.tsx` 零改动），值语义改执行根；归档只读时 `cwd` 恒 `null` 的既有语义保持（design「修改文件」/ D-8 / Q4）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `cwd` 派生 | 正向 | 入参 `worktree` 已是解析后的执行根（`host/rel`）→ 空态首发 `cwd` = 该值（值语义改口径、形态不变） | 单点 |
| `cwd` 派生 | 边界 | `worktree = null`（legacy）→ `cwd` 传 `null`（后端回落 root，既有断言零改） | 单点 |
| `cwd` 派生 | 边界 | `archived = true` → `cwd` 恒 `null`（归档只读，不传悬空执行根） | 单点 |
| `cwd` 派生 | 边界 | `worktree = ""`（空串）→ 逐字透传为 `cwd`（hook 零归一，归一职责在上游） | 单点 |
| 禁发判据 | 异常 | 归档只读 / 流程运行中 → `disabledReason` 与全禁发语义零改（既有断言零改） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `invoke`（IPC 边界） | 既有 `vi.hoisted` mock（进程边界依赖） | 空态首发 / 会话清单 |
| —（DOM） | 不 mock：`renderHook` 真实 React | 全部 hook 断言 |

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

- `ArchitectureChangeBlock` 的预览根 prop 更名 `worktree` → `previewRoot`（文件内私有），改喂 `preflight?.execRoot ?? preflight?.worktree ?? null`（design「修改文件」/ D-8 / AC-12）
- 合入 / 清理告知行两处 `preflight.worktree !== null` 判定保持不变（host 口径）（D-8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| `previewRoot` 派生 | 正向 | `preflight.execRoot` 非空 → 新图预览根 = `execRoot`（`ArchiPreview` 收到的 root 断言） | 单点 |
| `previewRoot` 派生 | 边界 | `execRoot = null` + `worktree` 非空（根 workspace）→ 预览根 = `worktree` | 单点 |
| `previewRoot` 派生 | 边界 | 两者皆 `null`（legacy / `preflight === null`）→ 预览根 = `null`（`ArchiPreview` 回落 `root`） | 表驱动（2 形态） |
| 合入 / 清理告知行 | 正向 | `preflight.worktree !== null`（host 在场）→ 合入告知行呈现，文案结构零改 | 单点 |
| 合入 / 清理告知行 | 异常 | 异常组合 `execRoot` 非空而 `worktree = null` → 不渲染合入告知行（判定只看 host 列，不被 `execRoot` 带偏） | 单点 |
| `previewRoot` 派生 | 异常 | 旧载荷形状（`preflight.execRoot` 为 `undefined`）→ `??` 链回落 `worktree`，不把 `undefined` 传给 `ArchiPreview`（不渲染空画布） | 单点 |
| 归档确认卡片 | 边界 | `ArchivePreflight` 八 → 九字段后既有断言零改（完成度警告 / 未过相位 / 缺席产物 / delta specs / 合入目标全渲染） | 回归集 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `preflight()` / `archivePreflight()` 工厂 | 局部对象字面量补 `execRoot: null`（F 组 fixture；不改断言） | 载荷供给 |
| `ArchiPreview` | 不 mock：真实子组件，按收到的 `root` prop 观测 | 预览根断言 |
| IPC | 不 mock：组件纯 props 驱动，零命令调用 | 全部用例 |

### packages/desktop/src/views/changes/components/change-create-dialog.tsx -> packages/desktop/src/views/changes/components/change-create-dialog.test.tsx

<!-- design「公共函数 / API」表对本文件零行（design Q3 判「零代码复核（不产生 diff）」）：成功面在 300f65e 已退役（提交成功即 onCreated(result.id) 关窗导航），组件内零 worktree 路径渲染与零 execRoot 消费点。下列条目取自该「零代码复核」行与「测试文件改动面」F 组登记；三条用例是该裁定的回归锚（防日后误恢复被退役的成功面）。 -->

#### 待测功能

- 成功面（已退役）: 提交成功即 `onCreated(result.id)` 关窗导航；组件内零 `worktree` 路径渲染、零 `execRoot` 消费点（design「修改文件」· Q3「零代码复核」行）
- 创建提交链: `create_change` 的调用形态与表单校验语义逐字不变（同一零代码复核行；AC-12 第四处「create 成功面路径」随之取消）
- `OUTCOME`（`CreateOutcome`）fixture: 补 `execRoot: null` 属 F 组编译面补齐，渲染面零变化（design「测试文件改动面」F 组）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
| --- | --- | --- | --- |
| 成功面零渲染 | 异常 | 提交成功 → `onCreated(id)` 被调用且对话框关闭；渲染面零 worktree 路径行、零「执行根」文案（成功面无落点） | 单点 |
| 创建提交链 | 正向 | 合法输入 → `create_change` 以既有参数形被调用恰一次、成功后 `onCreated(id)` 关窗导航；表单校验失败面（非法名 / 空白 goal）零变化 | 表驱动（2 形态） |
| `CreateOutcome` fixture 随动 | 边界 | `OUTCOME` fixture 补 `execRoot: null` 后渲染面零变化（组件不消费该字段，F 组仅编译面补齐） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
| --- | --- | --- |
| `invoke`（IPC 边界） | 既有 mock 装置 | 创建应答供给 |
| `onCreated` 回调 | `vi.fn()` 观测（组件入参回调，非内部模块） | 成功面导航断言 |

## 不可测试项

| # | 对象 | 原因与处置 |
| --- | --- | --- |
| ① | `packages/desktop/package.json` 的 `version` bump（`0.7.23` → `0.7.24`）与 AC-13 的「改动集 ⊆ 清单」对账 | 纯机械版本号与 `git status --porcelain` 对账，无行为面可断言；由桌面端静态步骤（变更清单对账）承接。同理 `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 的「逐字节不变」亦属该步骤 |
| ② | `openspec/changes/worktree-subdir-exec-root/specs/**` 的 delta 形态（AC-14：9 个能力目录 / MODIFIED 整段拷贝 / `### Requirement` 带 SHALL 或 MUST 且至少一个 `#### Scenario:` / 每目录含 `## Module Contract`） | 规格文档形态而非代码行为，无单元测试面；由 spec 评审与静态形状核对承接 |
| ③ | `eprintln!("[workflow] …")` 的执行根留痕在运行时的 stderr 文本 | Rust std 测试无 stderr 捕获通道，仓库内零先例（`change_detail` 的 `eprintln` 同族亦未捕获）。替代判据：访问器**返回值回落**（可观察行为）+ 源码 `[workflow]` 前缀字面量对账（`env!("CARGO_MANIFEST_DIR")` 读源文件，`steps_test` 先例） |
| ④ | 跨 crate / 全仓 greps 守线：AC-5「无散落 `exec_root.or(worktree)`」、AC-7「`restore_baseline` / `remove_worktree` 实参无 `exec_root` 出处」、AC-8「无 `root == main_root` 形态」、AC-10「`worktree_root` 零残留调用点」的全仓面 | 单测只能闭合**本 crate 本文件**的自证（已在 `walker.rs` / `lib.rs` / `config/src/lib.rs` 章节落地为源码文本断言）；跨 crate 全仓扫描属静态守线步骤，由桌面端承接（不复制脚本细节） |
| ⑤ | `.dev-team/architecture/**`、`INDEX.toml`、`.i.*`、`*.schema.json` 的零差异（AC-11 / AC-13） | 生成物与架构模型零 diff 判据；由 `api:check`（零写入比对）与 git 对账静态步骤承接。生成物本体 MUST NOT 手改，故不设用例 |
| ⑥ | `packages/desktop/src/types/generated/bindings.zod.ts` 的 TS 类型面 | 生成物，由 `bindings:zod-check`（`tsc --noEmit`）类型检查承接；其字段面已被 `bindings/mod_test.rs` 的出线断言覆盖（见「单元测试 · bindings.ts」） |
| ⑦ | `crates/core/workflow/tests/corpus_golden_test.rs` 的 8 份 detail 语料 golden 重写与 34 个 Rust 测试文件的字面量机械补齐（design「测试文件改动面」A–G 组） | 编译 / 断言随动而非新行为用例：`exec_root: None` / `worktree: None` / `execRoot: null` 的增行与四处精确键集断言手改由实现阶段承接（tasks 阶段一 / 四 / 七 / 八），其**期望形态**已在本设计各章节用例中声明 |
| ⑧ | `crates/core/workflow/src/write/{delete,promote}_test.rs`、`tests/corpus_golden_test.rs` 的同签面迁移 | 仅签名迁移（断言意图零改），不产生可设计的新断言；其行为面由 `create_test.rs`（trait 契约）与 `walker_test.rs` / `lib_test.rs`（锚点行为）覆盖 |
| ⑨ | `packages/desktop/src-tauri/src/bindings/mod_test.rs` 的「无 `DESKTOP_GOLDEN_REWRITE` 重写开关」源码自证 | 该字面量只能写在承载断言的测试文件内，扫自身即自匹配（恒红），属不可测形态。替代判据：①「字段集断言读到本次导出的产物」（新列 `execRoot` 能读到即证运行时再导出）已在 bindings 章节落地；②「无重写开关」由 design D-9 / Q7 的约定与产物 diff 复核承接，不设单测 |
