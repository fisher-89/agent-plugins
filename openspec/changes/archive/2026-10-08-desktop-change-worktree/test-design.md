# 测试设计: desktop-change-worktree

> **变更**: desktop-change-worktree
> **日期**: 2026-10-06
> **依据**: proposal.md（AC-1..AC-11）+ design.md（D1–D16）；实现任务边界与阶段划分见 tasks.md（测试编写与 golden 显式重写由本文件承接）

---

## 测试边界与框架识别

<!-- 逐文件推导自 design.md 变更清单与公共函数 / API 表；实现阶段零清单外测试文件。
     隔离注记沿 design 基准勘误 2：infra/agent SDK loop 重试护栏改动属另一在飞
     变更，本测试设计零触点（不为 loop.rs / runner.rs 增写或改写断言）。 -->

- **Rust 面**：src-tauri cargo workspace 套件，测试文件为共置 `*_test.rs` 模块（沿 workflow / store / orchestration / checks / commands 先例）与 `crates/core/workflow/tests/corpus_golden_test.rs` 语料黄金集成目标；新 crate `vcs-runtime`（裸名）同样共置 `*_test.rs`，`[dev-dependencies]` 增 tempfile。
- **前端面**：packages/desktop vite-plus 套件，共置 `.test.tsx`（vite-plus/test describe/it/expect）。
- **进程边界策略**：git 子命令族与 bootstrap 安装 spawn 以**真实进程 + 真实 git tempdir 仓**锚定（vcs-runtime 真件测试，design D4 / tasks 阶段三「真实 git 夹具」）；PATH 隔离窗口沿 checks / worker / commands 先例以共享互斥锁串行化、测毕恢复。core/workflow 写面与查询面测试以进程内假件实现 `WorktreePort` / `ChangeStateStore`（design D1 细粒度 port 的可测性拍板——编排断言留 core，进程执行断言落 vcs）。db 为真实 tempfile redb 组合；workflow 的 store dev-dep 自环在 lib-test 与普通 lib 双工件下类型不统一（既有限制），真实 db 组合行收 corpus 集成目标与 store 域测试。
- **迭代类型词表**：**新增**（新用例）/ **重写**（既有用例因 D2 签名演进或 D11 语义修订改写，断言面更新）/ **适配**（机械传参补齐——如 control 复合键 root 段，既有断言零改动）/ **迁移**（断言整体迁往另一文件，原处删除）。本变更无测试面退役（零删除），故无「废弃」行。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 11 条 AC。被测文件或模块为承载用例的测试文件（单值）；
     跨半边 AC 以「（同上——…半边）」行展开到各自测试文件；纯静态 / 流程性半边落
     「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | ChangeRecord v2 与身份段单点：v1→v2 decode-only 存量升级、新建档回环逐字段一致、`list_models` / `scan` 零改动覆盖、身份段上提后 db 文件名同名回归（同根恒同名） | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs（v1→v2 decode-only / v2 回环 / 双向 From / `new` 增参半边） |
| AC-1 | （同上——store 真件半边：db 文件名委托 foundation 同名回归、`create_change_record` / `change_state` 映射双字段往返、存量 v1 行库 additive 打开读出 None） | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-1 | （同上——身份段清洗算法行为平移半边：置换 / char 截断 / 尾点空格 / 空回退 / 确定性） | packages/desktop/src-tauri/crates/core/foundation/src/identity_test.rs |
| AC-2 | create worktree 建域：干净 git 仓 create 成功——落位 `worktrees/{身份段}/<name>`、branch `change/<name>` 基线 = 主仓 HEAD、worktree 内 explore.md = goal 原文、db 记录携 worktree / base_commit、主仓 active 树不含该目录 | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs（core 假件半边：四段编排 + 建档字段 + 主仓树零目录） |
| AC-2 | （同上——vcs 真件半边：`add_worktree` 建目录 + 铸 branch 指向 HEAD、`remove_worktree` / `delete_branch` 补偿面） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-2 | （同上——落位派生半边：`worktree_dir` 消费身份段单点、同根可复现 / 异根不撞） | packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs |
| AC-2 | （同上——命令装配半边：`create_change` async + `create_change_with` 泛型缝，data_root 注入 + ProcessWorktree + spawn_blocking，worktree 实落 data_root/worktrees/ 下） | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-3 | 前置校验与拒绝面：git 不可发现 / 非 git 仓 / branch 冲突 / worktree 目录冲突 / 同名 active 建档 / 非法名称 / goal 空白——各显式 `Err` 且零 worktree、零建档、零目录（全 IO 前置） | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs（probe 三态真件对照见 AC-3 同上行 → git_test.rs） |
| AC-3 | （同上——probe 三态真件半边：git 不可发现 / 非 git 仓 / 空仓无 HEAD 的 Err 引导面） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-4 | 脏仓警告：warnings 含「先提交再开新 change」引导、worktree 不含未提交改动（基线恒 HEAD）、干净仓空清单 | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs（警告文案锚半边） |
| AC-4 | （同上——基线恒 HEAD 真件半边：脏主仓 `add_worktree` 检出内容不含未提交文件、base_commit 记录该 HEAD） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-5 | 确定性 bootstrap：lockfile 命中 spawn（cwd = worktree）、未知管理器跳过注记、失败不回滚且警告呈现 | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs（映射表 / 注记 / 不回滚——假件捕获半边） |
| AC-5 | （同上——`run_install` 执行体真件半边：cwd / 环境全继承 / 退出态 / 摘要尾部 ≤300 字 / 拉起失败 Err） | packages/desktop/src-tauri/crates/infra/vcs/src/bootstrap_test.rs |
| AC-6 | 双 root run 组合：store / 会话 / StepRecord 落 workspace root 库（注入实例，worktree 路径不进 `for_root`）、executor cwd = worktree、diff / 检查器执行目录 = worktree | packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs（store 注入半边） |
| AC-6 | （同上——exec root 透传锚半边：`RunRequest.root` → 假引擎 / 假工具 / 快照源收到的 root 恒 exec root） | packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs |
| AC-6 | （同上——发起装配半边：exec root 解析三态 + worktree 存在性校验 + db 半边落 workspace root 库零 worktree 库） | packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs |
| AC-7 | 复合键并行：同 workspace 同 change 二次发起 Err、异 workspace 同名不误拒、同 workspace 两 change 并行 | packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs（命令面机械传参行见 AC-7 同上行 → change_flow mod_test.rs） |
| AC-7 | （同上——命令面半边：五控制命令复合键传参、异 workspace 同名并行发起互不误拒） | packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs |
| AC-8 | legacy 回归：`worktree=None` 存量建档照旧主 root 跑通、list / detail / 归档既有语义零变化 | packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs（发起零变化半边；locate / detail / list None 持衡半边经 queries mod_test / detail_test / list_test 各节承载，archive 持衡半边经 archive_test 节） |
| AC-9 | 读面与 UI 可见性：db status 权威归组、详情恒可达、`read_artifact` 命中 worktree 产物、detail 出线 worktree（legacy null）经 golden 显式重写、语料两态、create 对话框行内呈现 | packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs（归组半边） |
| AC-9 | （同上——`locate_change` worktree 回退半边） | packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs |
| AC-9 | （同上——detail 出线与建档记录恒可达半边） | packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs |
| AC-9 | （同上——命令面 detail / read_artifact worktree 感知半边） | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-9 | （同上——golden 显式重写与语料 worktree 两态半边） | packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs |
| AC-9 | （同上——bindings DTO 出线半边：`CreateOutcome` 四字段 + `ChangeDetail.worktree`） | packages/desktop/src-tauri/src/bindings/mod_test.rs |
| AC-9 | （同上——create 对话框成功面半边） | packages/desktop/src/views/changes/components/change-create-dialog.test.tsx |
| AC-9 | （同上——详情 worktree 信息行半边） | packages/desktop/src/views/changes/change-detail-view.test.tsx |
| AC-10 | 归档引导与版本交付：未 merge（两树未命中）archive 显式拒绝引导 merge、merge 后既有双写零改动 | packages/desktop/src-tauri/crates/core/workflow/src/write/archive_test.rs（命令面引导文案行见 AC-10 同上行；版本交付静态半边 → 不可测试项 5） |
| AC-10 | （同上——`archive_change` IPC 薄命令引导文案半边） | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-11 | crate 图与 vcs 边界落位：`vcs-runtime` 依赖白名单、身份段仅 foundation 一份、core 各 crate 无 store / vcs | —（见不可测试项 1 / 2） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] / #[tokio::test]、前端 vite-plus）。
     门面 / 纯类型 / 生成物模块（write/worktree.rs trait 词汇面、state.rs 字段面、
     write/mod.rs、foundation lib.rs、bindings.ts、fixtures/README.md、Cargo.toml /
     package.json / main.rs 装配行）不建独立测试文件，统一落「不可测试项」声明
     （含行为去向）。既有 *_test.rs 扩展 / 重写节仅声明受本变更牵动的用例行，
     未列出者零改动持衡。 -->

### packages/desktop/src-tauri/crates/core/foundation/src/identity.rs -> packages/desktop/src-tauri/crates/core/foundation/src/identity_test.rs

<!-- 新增文件节。挂 AC-1（身份段清洗算法行为平移半边——断言自 store_test
     「派生单点纯函数可读段清洗_截断_非法字符_尾点空格与空回退」整体迁移）、
     AC-2（worktree 落位派生的身份段前提）。design D6：算法与参数自 store.rs
     逐字平移，纯函数零 fs。 -->

#### 待测功能

- workspace_identity_segment(canonical_root: &str) -> String: `{可读段}-{sha256 前 16 字节 32 位小写 hex}`；可读段清洗（非法字符置换 `_` → char 截断 ≤24 → 去尾 `.` 与空格）后为空回退纯哈希

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| identity_test · 基本形态 | 正向 | 常规路径 → `{末段目录名}-{32 位小写 hex}`；hash 与 SHA-256(canonical_root UTF-8) 前 16 字节逐字一致（测试内以 sha2 独立对拍 + 一组固定期望向量钉死跨实现稳定） | 迁移（新增文件承载） |
| identity_test · 非法字符清洗 | 边界 | `/ \ : * ? " < > |` 与控制符逐字置换 `_`；产物不含任何 OS 非法字符 | 迁移 |
| identity_test · char 截断 | 边界 | 超 24 字符按 char 截断恰 24（多字节 emoji 不悬挂不 panic） | 迁移 |
| identity_test · 尾部清洗与空回退 | 边界 | 尾部 `.` 与空格去除；清洗后为空 → 纯 32 位 hex 无 `-` 连接 | 迁移 |
| identity_test · 确定性 | 正向 | 同根两次调用逐字相等（跨重启可复现）；异根产物必不同 | 迁移 |
| identity_test · 同源锚 | 边界 | 同一 root 的 segment 与 `workspace_db_file_name(root)` 去 `.redb` 后缀逐字一致（同源消费的单点证明；db 文件名半边经 store_test 同名回归行随动） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱（字符串字面量 + sha2 独立摘要对拍，零 fs 零 IO） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs

<!-- 新增文件节。挂 AC-2（落位派生半边）、AC-11（目录名单点——`WORKTREES_DIR_NAME`
     驻 vcs，desktop-app 不自拼）。design D6。 -->

#### 待测功能

- worktree_dir(data_root: &Path, workspace_root: &str, change: &str) -> PathBuf: `data_root/worktrees/{身份段}/{change}` 落位派生单点
- WORKTREES_DIR_NAME: 常量 `"worktrees"`
- ProcessWorktree: `new()` / `Default`（无状态装配入口，`impl WorktreePort` 行为面在 git / bootstrap 两节）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| lib_test · 落位形态 | 正向 | `worktree_dir(data_root, ws_root, change)` = `data_root/worktrees/{workspace_identity_segment(ws_root)}/{change}`——第三级与 foundation 单点同源（直调对拍）；`WORKTREES_DIR_NAME == "worktrees"` | 新增 |
| lib_test · 可复现与隔离 | 边界 | 同 (data_root, ws_root, change) 两次派生等值；异 ws_root 同 change → 不同身份段父目录（互不冲突互不可见） | 新增 |
| lib_test · change 名原样挂尾 | 边界 | change 名不做任何清洗 / 截断（清洗权威在 create 前置，本函数零校验——负向锚定不越权） | 新增 |
| lib_test · ProcessWorktree 构造 | 边界 | `new()` 与 `Default::default()` 等值（无状态可重复构造——组合根按需铸的编译锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯路径推导内存直驱（tempfile 仅取路径字符串，零 git 零 spawn） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

<!-- 新增文件节。挂 AC-2（add/remove/delete 真件半边）、AC-3（probe 三态真件）、
     AC-4（基线恒 HEAD 真件半边）。tasks 阶段三「真实 git tempdir 夹具」；
     git 可用性以测试环境 PATH 可达 git 为前提（与产品硬依赖同口径）。 -->

#### 待测功能

- ProcessWorktree::probe(&self, main_root: &Path) -> Result<RepoProbe, String>: `rev-parse --is-inside-work-tree` / `rev-parse HEAD` / `status --porcelain` 三连（git 可发现 + git 仓 + HEAD + 脏仓单点）
- ProcessWorktree::branch_exists(&self, main_root: &Path, branch: &str) -> Result<bool, String>: `show-ref --verify`
- ProcessWorktree::add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String>: `worktree add -b`（HEAD 基线 + 铸分支）
- ProcessWorktree::remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>: `worktree remove --force`（补偿面）
- ProcessWorktree::delete_branch(&self, main_root: &Path, branch: &str) -> Result<(), String>: `branch -D`（补偿面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| git_test · probe 干净仓 | 正向 | 真实 git tempdir 仓（初始提交在案）→ `RepoProbe { head: <HEAD sha>, dirty: false }` | 新增 |
| git_test · probe 脏仓 | 正向 | 追加未提交文件（工作区与 staged 各一）→ `dirty: true` 且 head 不变（基线仍 HEAD） | 新增 |
| git_test · probe 空仓 | 异常 | `git init` 后零提交（无 HEAD）→ `Err` 含「先提交」引导（design：空仓 HEAD 失败映射） | 新增 |
| git_test · probe 非 git 仓 | 异常 | 普通目录 → `Err` 显式（非 git 仓引导），不静默 | 新增 |
| git_test · git 不可发现 | 异常 | PATH 隔离窗口（共享锁串行化、测毕恢复）→ `Err` 含「git 不可用」引导文案 | 新增 |
| git_test · branch_exists 两态 | 边界 | 既有分支 → true；不存在 → false（非 Err） | 新增 |
| git_test · add_worktree 建域 | 正向 | `add_worktree(main, wt, "change/x")` → worktree 目录在场、branch `change/x` 在案且指向主仓 HEAD（`rev-parse` 对拍） | 新增 |
| git_test · 基线恒 HEAD | 正向 | 主仓置未提交改动后 add → worktree 内**不含**该未提交文件、内容 = HEAD 检出（AC-4 真件半边；base_commit 语义前提） | 新增 |
| git_test · add 目标已存在 | 异常 | worktree 目标目录预置在场 → `Err`（不覆盖既有目录） | 新增 |
| git_test · remove_worktree | 正向 | add 后 `remove_worktree`（--force）→ worktree 目录移除且主仓树无损；再 add 同路径可成功（补偿面幂等可重试） | 新增 |
| git_test · delete_branch | 正向 | add 铸出的分支经 `delete_branch` → `show-ref` miss；分支删除不触碰主仓工作区 | 新增 |
| git_test · 补偿序组合 | 边界 | add → remove → delete 全链后：主仓回到初态（无 worktree 登记、无分支残留——create 补偿链的 vcs 半边证据） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 git 子进程 + tempfile 仓（产品硬依赖同口径；git 经 `git -C <root>` 显式寻址）；「git 不可发现」行以 PATH 隔离窗口注入（进程全局变量边界，共享互斥锁串行化、测毕恢复——沿 checks TEST_PATH_LOCK 先例，本 crate 自持同型锁） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/vcs/src/bootstrap.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/bootstrap_test.rs

<!-- 新增文件节。挂 AC-5（run_install 执行体真件半边）。design D4：shell 包装
     （Windows cmd /C、其余 sh -c，static_check 先例）、cwd = worktree、环境全继承
     零注入、超时 900 s 常量轮询 kill、摘要尾部 ≤300 字。900 s 到点 kill 路径见
     不可测试项 3。 -->

#### 待测功能

- ProcessWorktree::run_install(&self, worktree: &Path, command: &str) -> Result<InstallRun, String>: shell 包装 spawn + cwd = worktree + 输出尾部摘要（`InstallRun { success, summary }`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bootstrap_test · 成功退出 | 正向 | 退出 0 的真实快命令（输出已知文本）→ `success: true`、summary 含输出尾部 | 新增 |
| bootstrap_test · 非零退出 | 异常 | 退出非 0（stdout+stderr 各有输出）→ `success: false`、summary 含 stderr / stdout 尾部信息 | 新增 |
| bootstrap_test · cwd = worktree | 正向 | 命令以相对路径在 cwd 落文件（或回显 cwd）→ 产物落在 worktree 内、回显值 = worktree 绝对路径（AC-5「cwd = worktree」字面） | 新增 |
| bootstrap_test · 环境全继承 | 边界 | 测试注入唯一命名临时环境变量 → 子进程回显可见该值（零注入 = 全继承证明；变量测毕恢复） | 新增 |
| bootstrap_test · 摘要尾部截断 | 边界 | 长输出命令（>300 字）→ summary `chars().count() ≤ 300` 且取自尾部；短输出全量保留 | 新增 |
| bootstrap_test · shell 包装语义 | 边界 | 命令串含 shell 语法（连接符 / 重定向）可执行成功（cmd /C 与 sh -c 双平台口径，static_check 先例） | 新增 |
| bootstrap_test · 拉起失败 | 异常 | 命令本体不可达（PATH 隔离或不存在名）→ `Err` 显式（拉起 / 超时统一 Err 面——D5「依赖引导未执行成功」的 error 来源） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实快命令子进程（echo / cat / printf 等 PATH 可达程序 + tempfile worktree 目录）；环境变量注入以唯一命名键避免串扰、测毕恢复；PATH 隔离窗口沿 git_test 节同型锁 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

<!-- 既有文件重写 + 扩展节（本变更最大断言面）。挂 AC-2（四段编排）、AC-3（前置
     七道拒绝面）、AC-4（警告文案锚）、AC-5（映射表 / 注记 / 不回滚——假件半边）。
     D2 签名演进：create(layout, store, name, goal) → create(main_root, worktree_root,
     store, vcs, name, goal)——既有用例的 Env 装置整体改形（双 tempdir 根 + 假件
     WorktreePort 注入），正向断言从「主仓目录三合一」重写为「worktree 内三件套 +
     主仓 active 树零目录」。 -->

#### 待测功能

- create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str) -> Result<CreateOutcome, String>: 建域四段组合（D2 / D3——前置七道全零副作用 → 建档先行 → worktree add → worktree 内目录树 + explore.md → bootstrap / 警告；补偿链）
- CreateOutcome { name: String, created: String, worktree: String, warnings: Vec<String> }: DTO 演进（D2 / D5——恰四字段面）
- 警告文案单点（D5）：脏仓引导 / 未识别依赖管理器 / Cargo 无 lock 专项注记 / 依赖引导失败 / 依赖引导未执行成功

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create_test · 建域四件套成功 | 正向 | 干净仓假件（probe head 固定 / dirty=false）→ worktree 目录 = `worktree_root/<name>`、`add_worktree` 收到 `(main_root, 该目录, "change/<name>")`、worktree 内 `changes_root/<name>/explore.md` = goal 原文（UTF-8 零包装零 trim）、db 记录 `worktree=Some(该绝对路径)` / `base_commit=Some(probe.head)`；主仓 active 树**不含**该目录；worktree 树零 workflow.json（重写既有「三合一成功」行——落点自主仓迁 worktree） | 重写 |
| create_test · 空白树深层建树 | 正向 | worktree 内 changes_root 全链不存在 → 建全树（场景字面沿既有行，落点改 worktree） | 重写 |
| create_test · goal 原文保真 | 正向 | 多行 + emoji + 超长 + 首尾空白 goal → worktree 内 explore.md 字节保真（沿既有行迁移） | 重写 |
| create_test · 前置①② 名称与 goal | 异常 | kebab-case 违例全族 / 超 128 / 空白 goal → `Err` 且零目录零建档零 vcs 调用（假件全方法调用计数 0——全 IO 前置加强面）；非法名 + 空白 goal 同投归因名称（既有行迁移） | 适配 |
| create_test · 前置③④ 目录与建档冲突 | 异常 | 主仓 active 目录已存在 / db 同名 active → `Err` 且 vcs 零调用（含 probe——冲突检查先于 git 探测的顺序锚）、既有目录零触碰 | 重写 |
| create_test · 前置⑤ git 三态 | 异常 | probe 注入 Err 三态（git 不可发现 / 非 git 仓 / 空仓无 HEAD）→ 各自显式 `Err`（含引导文案）且零建档零目录零 add（MUST NOT 静默回退主 root 创建——断言 Err 非 Ok、无任何目录产生） | 新增 |
| create_test · 前置⑥ branch 冲突 | 异常 | `branch_exists` 返回 true → `Err` 含 branch 名（冲突对象）且零建档零 add | 新增 |
| create_test · 前置⑦ worktree 目录冲突 | 异常 | `worktree_root/<name>` 预置在场 → `Err` 含该路径且零建档 | 新增 |
| create_test · 脏仓警告 | 正向 | probe dirty=true → create 成功、`warnings` 含「先提交再开新 change」引导文案（D5 逐字锚）；基线段断言 `base_commit` 仍 = probe.head | 新增 |
| create_test · 干净仓零警告 | 边界 | 干净 + 无 lockfile（worktree 空检出）→ 依 D4「未识别管理器」注记入 warnings；干净 + 已知管理器顺利安装 → warnings 为空清单（两行分立：空清单的条件面 = 干净仓且顺利） | 新增 |
| create_test · bootstrap 映射表 | 正向 | worktree 检出（假件镜像主仓树）分别预置 `pnpm-lock.yaml` / `package-lock.json` / `yarn.lock` / `Cargo.lock` → `run_install` 恰一次且命令串逐字为 `pnpm install --frozen-lockfile` / `npm ci` / `yarn install --frozen-lockfile` / `cargo fetch --locked`（假件捕获——D4 映射表四行） | 新增 |
| create_test · 首匹配序 | 边界 | 多 lockfile 并存 → 恰命中映射表首匹配项、`run_install` 恰一次 | 新增 |
| create_test · 未知管理器注记 | 边界 | 无任何已知 lockfile → 零 `run_install` 调用 + warnings 含「未识别依赖管理器，跳过依赖引导」（D5 逐字锚） | 新增 |
| create_test · Cargo 无 lock 专项注记 | 边界 | `Cargo.toml` 在场而 `Cargo.lock` 缺席 → 跳过 + warnings 含专项注记文案（D5 逐字锚——先于「未识别」判定） | 新增 |
| create_test · 安装非零退出 | 异常 | `run_install` 返回 `success=false` → create 以 Ok 收口（建档与 worktree 保留——AC-5 不回滚）且 warnings 含「依赖引导失败（{command}）: {summary}」 | 新增 |
| create_test · 安装拉起失败 | 异常 | `run_install` 返回 Err → create 以 Ok 收口且 warnings 含「依赖引导未执行成功（{command}）: {error}」（bootstrap 段失败不回收——D3） | 新增 |
| create_test · 补偿：add 失败 | 异常 | `add_worktree` 注入 Err → `delete_change_record` 删本次建档（恰一次）+ 尽力 `delete_branch`（捕获调用）；`Err` 呈现失败与补偿事实 | 新增 |
| create_test · 补偿：树写出失败 | 异常 | worktree 内路径分量预置文件占位（真实 fs 注入）→ `remove_worktree` → `delete_branch` → 删建档 全链下发（捕获调用序）；`Err` 呈现补偿完成与残留清理指引（`git worktree list` 文案锚）（重写既有「fs失败补偿」行——链自单步删建档扩为三步） | 重写 |
| create_test · 补偿链再失败 | 异常 | 链中 `remove_worktree` 或 `delete_branch` 注入 Err → `Err` 呈现残留对象（worktree / branch 名）与手动清理指引，不静默自愈（沿既有「补偿再失败」行的双故障角落语义，扩至新链） | 新增 |
| create_test · created 出线与立即可见 | 正向 | `created` 取建档 `created_at` UTC 日期（调用前后日界并集——沿既有）；create 成功后 `list_changes` 立即可见：active 组恰一条、状态面完整（worktree 记录不因主仓目录缺席被丢弃——AC-9 半边随行） | 重写 |
| create_test · CreateOutcome 线形状 | 边界 | serde 序列化恰 `name` / `created` / `worktree` / `warnings` 四键（camelCase）——重写既有两键断言；`warnings` 空清单出线 `[]` 非 null | 重写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorktreePort（注入依赖入参） | 进程内脚本化假件：`probe` 可编程（head / dirty / Err 注入）；`branch_exists` 可编程；`add_worktree` 成功即真实 `create_dir_all(worktree)` 并镜像主仓树内容（模拟 HEAD 检出——lockfile / 目录树用例的检出半边）；`remove_worktree` / `delete_branch` / `run_install` 调用与参数捕获 + 按方法 Err 注入 | 本节全部用例（编排 / 拒绝面 / 补偿链 / 警告面） |
| ChangeStateStore（注入依赖入参） | 既有假件沿用（建档 / 补偿删除捕获 + 故障注入），构造字面量补 `worktree` / `base_commit` 两 Option 字段 | 本节全部用例 |
| 文件系统 | 真实 tempdir × 2（主仓根 + worktree 根）；fs 失败经路径分量文件占位注入（既有技法迁移） | 树写出失败 / 目录冲突各用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/archive_test.rs

<!-- 既有文件扩展节。挂 AC-10（merge-first 引导半边）。design D13：未命中 Err
     分支前插入 worktree 前置；既有双写零改动。 -->

#### 待测功能

- archive(layout: &Layout, store: &dyn ChangeStateStore, change: &str) -> Result<ArchiveOutcome, String>: 既有双写 + worktree merge-first 前置引导分支（D13）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_test · 未 merge 引导拒绝 | 异常 | 记录携 `worktree=Some` 且主仓 active / archive 两树均未命中 → `Err` 引导「先 merge worktree 分支 change/{name} 回主仓再归档」（含 change 名与 branch 名文案锚；**非**泛化「目录未找到」——负断言不含该旧文案）；db 与磁盘零变化 | 新增 |
| archive_test · merge 后零特判 | 正向 | 记录携 worktree + 主仓 active 目录在场（模拟 merge 后）→ 既有双写成功（status 翻转 + 日期前缀改名），路径无 worktree 特判行为（与无 worktree 记录的成功行输出等形） | 新增 |
| archive_test · archive 树命中续半边 | 边界 | 记录携 worktree + archive 树前缀目录在场 → 续半边仅补翻转（既有语义对 worktree 记录同样成立） | 新增 |
| archive_test · legacy 未命中持衡 | 异常 | `worktree=None` + 两树未命中 → 既有泛化「目录未找到」Err 原样（legacy 语义零变化——与 worktree 引导行文案互斥的对拍锚） | 新增 |
| archive_test · 归档不触碰 worktree | 边界 | 归档成功行中 worktree 目录与 branch 原样未动（`archive` 签名零 vcs 参为编译期锚——归档面无任何 git 触点） | 新增 |
| archive_test · 既有双写全族 | 正向 | 常规双写 / 续半边 / 幂等 / 无建档拒绝 / 目标冲突 / 半完成呈现各既有行——装置构造字面量补两 Option 字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 既有假件沿用（可编程记录 + 故障注入），种子支持 `worktree=Some` 形态 | 本节全部用例 |
| 文件系统 | 真实 tempdir 主仓树（active / archive 两树按用例布置） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs

<!-- 既有文件扩展节。挂 AC-9（locate 回退半边）、AC-8（None 持衡）。design D12：
     解析链主仓 active 精确 → archive 精确 → archive 日期前缀 → worktree 回退。 -->

#### 待测功能

- locate_change(layout: &Layout, worktree: Option<&str>, name: &str) -> Option<ChangeLocation>: 第四级 worktree 回退（`resolve(worktree).changes_root/<name>` is_dir → `ChangeSource::Active`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · worktree 回退命中 | 正向 | 主仓两树未命中 + worktree 目录树在场（`<worktree>/openspec/changes/<name>` is_dir）→ `Some` 且 `source == Active`、dir = worktree 内该目录 | 新增 |
| mod_test · 优先级链 | 边界 | 主仓 active 精确 / archive 精确 / archive 日期前缀三态各自在场时 worktree 回退不参与（仅第四级）；三级全 miss 才落 worktree（四态一行覆盖优先序） | 新增 |
| mod_test · worktree 缺席 | 异常 | `worktree=Some` 但该目录被删（回退 miss）→ `None`（手动删 worktree 后的定位语义——detail 恒可达半边的反面输入） | 新增 |
| mod_test · None 持衡 | 边界 | `worktree=None` → 既有三级行为零变化（既有六用例经签名适配后断言零改动持衡——补第四参 `None`） | 适配 |
| mod_test · 穿越校验保留 | 异常 | 多分量 / 穿越 / 空名 → `None`（worktree 在场亦不豁免单分量名校验） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir 布置主仓两树 + worktree 树（纯路径推导，零 db） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

<!-- 既有文件扩展节。挂 AC-9（detail 出线 + 建档记录恒可达）、AC-8（null 出线与
     既有语义持衡）。design D12：record 先读后定位、worktree 直读透出 None → null、
     定位 miss → 产物空 + 状态面在（source 自 record.status 映射）。 -->

#### 待测功能

- change_detail(layout: &Layout, store: &dyn ChangeStateStore, name: &str) -> Option<ChangeDetail>: 签名不变；内部改 record 先读后定位（record.worktree 直传 locate_change）
- ChangeDetail: 增 `worktree: Option<String>`（自 ChangeStateRecord.worktree 直读透出——纯 derive 零改写）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail_test · worktree 路径出线 | 正向 | 记录 `worktree=Some(绝对路径)` + worktree 目录树在场（主仓两树未命中）→ detail Some、`worktree` 与库内记录值**逐字**一致、artifacts 自 worktree 目录发现（产物清单命中 worktree 内文件） | 新增 |
| detail_test · worktree 手动删恒可达 | 异常 | 记录在场（worktree=Some）+ 定位全 miss（worktree 目录被删、未 merge）→ 仍 `Some`：`pipeline` 照常（9 站空 attempts 或既有落账）、`artifacts` 空、`status` 状态面在、`source` 自 record.status 映射（active → Active）——「建档记录恒可达详情」字面 | 新增 |
| detail_test · source 映射 | 边界 | 记录 archived + 定位 miss → `source == Archive`（映射双态；archived 记录主仓 miss 的呈现面） | 新增 |
| detail_test · legacy null 出线 | 边界 | 记录 `worktree=None` → detail `worktree` 出线 null、产物解析走主仓两树既有语义（既有用例持衡 + worktree 键断言随行）；serde 线面 `worktree` 键**恒在场**（null 不省键——冻结契约） | 新增（既有行随动适配） |
| detail_test · 双缺 None 持衡 | 异常 | record 缺 + 定位缺 → `None`（既有「未找到 none」语义零变化——适配持衡） | 适配 |
| detail_test · 既有聚合全族 | 正向 | 流水线重组 / ISO 时间 / 槽位 null / 文档形态 / created 回退各既有行——装置种子补两 Option 字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 既有假件沿用（可编程记录 + 相位行），种子支持 worktree 两态 | 本节全部用例 |
| 文件系统 | 真实 tempdir（主仓树 + worktree 树按用例布置） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs

<!-- 既有文件重写 + 扩展节。挂 AC-9（db status 权威归组）、AC-8。design D11：
     对 desktop-workflow-db-state design D7「读时以磁盘事实归组」的显式修订——
     既有 d7 对账用例语义反转重写；磁盘目录仅决定目录名取位。 -->

#### 待测功能

- db_entry(layout: &Layout, record: &ChangeStateRecord) -> ChangeSummary: 归组改 `record.status` 权威（active → 进行中组、archived → 归档组）；磁盘目录仅决定目录名取位

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| list_test · db-active 目录外移归 active 组 | 正向 | db `status=active` + 目录被外部移入 archive 树 → 条目归**进行中组**、`source=Active`、状态面完整、目录名取位 archive 前缀名（语义修订反转行——原 d7 用例断言归 archive 组，重写为留 active 组并留修订注记） | 重写 |
| list_test · db-archived 目录留守归 archive 组 | 正向 | db `status=archived` + 目录仍在 active 树 → 归**归档组**（月组取 `archived_at`）、目录名取位建档名（原 d8 反向行重写） | 重写 |
| list_test · worktree 条目入进行中组 | 正向 | db active（worktree=Some）+ 主仓两树均未命中 → 进行中组、状态面完整（status / created / active_phase）、目录名 = 建档名——不报错、不丢弃、不误归未知时间组（AC-9 scenario 字面） | 新增 |
| list_test · 目录名取位规则 | 边界 | active：主仓命中 → 建档名；archived：archive 树精确 / 日期前缀名；未命中 → 建档名（三态取位一行） | 新增 |
| list_test · 纯读纪律保持 | 边界 | 查询后假件记录序列逐字段不变（不回写 db——沿既有断言面） | 持衡（沿用） |
| list_test · 磁盘-only 与分组全族 | 正向 | 文档形态入列 / 按月分组（archived_at 优先、磁盘回退前缀）/ 未知时间组置尾 / active 扫描守卫各既有行——种子补两 Option 字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 既有假件沿用（可编程记录序列 + snapshot 观察面），种子支持 worktree 两态 | 本节全部用例 |
| 文件系统 | 真实 tempdir 主仓两树（既有装置） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

<!-- 既有文件重写 + 扩展节。挂 AC-7（复合键并行）。design D10：注册表键
     (String, String) = (workspace root, change)；九方法签名增 root 段；RunGuard 持
     复合键；RunUpdate / ChangeRunSnapshot / ChangeRunStatus DTO 零改动。 -->

#### 待测功能

- ChangeFlowControl::begin_run(self: &Arc<Self>, root: &str, change: &str, run_id: String) -> Result<RunGuard, String>: 复合键并行冲突检测
- subscribe / request_stop / current_session / publish / set_session / answer / confirm / snapshot: 签名均增 root 段（复合键寻址）
- RunGuard: 持复合键（finish 除名 / emit / set_session / cancelled 既有语义随键演进）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| control_test · 复合键登记与冲突 | 正向 | (rootA, x) begin 成功；(rootA, x) 二次 begin → `Err`（同 workspace 同 change 并行冲突——文案含 change 名）；(rootB, x) begin 成功（异 workspace 同名**不**误拒——先例 bug 修正锚） | 重写 |
| control_test · 同 workspace 两 change 并行 | 正向 | (rootA, x) 与 (rootA, y) 同时登记互不冲突（真并行解锁） | 新增 |
| control_test · 订阅隔离 | 边界 | (rootA, x) 与 (rootB, x) 各自 run 运行中：rootA 订阅者收到 (rootA, x) 的 publish 而**收不到** (rootB, x) 的任何信封（broadcast 按复合键寻址不串台） | 新增 |
| control_test · 控制面寻址全签名 | 正向 | snapshot / request_stop / current_session / set_session / answer / confirm 以 (root, change) 寻址：命中自身键生效、他键（同名异 root）零影响（六方法一行覆盖机械传参 + 隔离） | 重写 |
| control_test · RunGuard 复合键除名 | 边界 | (rootA, x) guard finish → 仅该键除名（snapshot None）；(rootB, x) 同名键存活（snapshot 仍在）——guard 不误伤他键 | 新增 |
| control_test · DTO 零改动 | 边界 | `RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` 线面 JSON 形状与既有 wire 断言一致——复合键 root 段不出线（键集无 root 渗出负断言） | 新增 |
| control_test · 既有全族 | 正向 | 停止幂等 / ask 应答 / confirm 单次通道 / 快照重组各既有行——begin_run 等调用补 root 段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | `#[tokio::test]` 进程内 tokio 原语真实组合（既有装置沿用；多任务握手沿 `until_ok` 轮询先例） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- 零触点文件的轻量扩展节 + 机械适配注记。挂 AC-6（exec root 透传锚半边）、
     AC-7。walker / steps / snapshot 代码零改动（design 不变组件核对结论），本节
     仅钉「RunRequest.root 语义换 exec root 后全链透传」的回归锚；既有假件随
     control 复合键签名机械适配（见不可测试项 9）。 -->

#### 待测功能

- RunRequest.root: 恒 exec root（worktree 绝对路径或 legacy 主 root）——walker 对 root 透明（WorkerTurnRequest.root / ToolStepRequest.root / 快照 detail(&request.root) 天然随 exec root 落位）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walker_test · exec root 透传锚 | 正向 | `RunRequest.root` 置 worktree 形路径驱动一相位（假引擎 + 假写面）→ 假 worker 捕获的 turn root、假工具捕获的 step root、快照源收到的 detail root 三者恒等于该 exec root（root 透明性防漂移钉——零改动组件的语义演进锚） | 新增 |
| walker_test · 既有全链全族 | 正向 | 路由 / 停等 / 重试 / backtrack / 收口各既有行——假件随 control 复合键与既有签名机械适配，行为断言零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorkerAgentPort / ToolStepPort / DiffContextPort / WorkflowSnapshotPort（注入依赖入参） | 既有 FakeWorker / FakeTools / FakeDiff 装置沿用（调用与 root 捕获面既有） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

<!-- 既有文件适配 + 扩展节。挂 AC-6（store 注入半边）。design D8：B 案拆参——
     compose_turn(stores, registry, store, agent)，store 半边命令层预解析注入，
     for_root 调用自 compose 删除、root 参移除；全局半边解析（agent / provider）
     仍经 stores。 -->

#### 待测功能

- compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>: store 注入化拆参（cwd 半边经既有 SessionCtx 通道不经 compose）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| compose_test · 注入实例消费 | 正向 | 以 `for_root(rootA)` 预解析实例注入 → `composed.begin(...)` 落会话行 / 轮行于**注入实例**（注入 store 的 list_sessions 可查，逐字段一致）——store 半边唯一入口是注入参（worktree 路径在类型上不可能进入库解析） | 新增 |
| compose_test · 全局半边不变 | 正向 | provider / 默认实例解析仍经 `stores`（global 库 fixture 驱动既有解析断言持衡——sdk provider / cli 实例 / 无默认 Err 各形态） | 适配 |
| compose_test · 既有解析全族 | 正向 | 空库缺省 Err / 显式 id 不存在 / continue 不存在各既有行——调用点机械适配新签名（root 参删除），断言零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile WorkspaceStores（既有装置沿用：global 库 provider / 实例种子 + workspace 库实例）；无进程 mock | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- 既有文件扩展节。挂 AC-1（v1→v2 decode-only / 回环 / 双向 From / new 增参）。
     design D7：ChangeRecordV1（id=9 version=1，pub(crate)，不入模型组）+ ChangeRecord
     v2（from = ChangeRecordV1）；沿 AgentRunRecord v3→v4 先例同模式。 -->

#### 待测功能

- ChangeRecord（native_model id=9, version=2, from=ChangeRecordV1）: 增 `worktree: Option<String>` / `base_commit: Option<String>`（`#[serde(default)]`）
- ChangeRecordV1: v1 历史形态（decode-only 链目标）
- 双向 From: 升级两字段 None（legacy 语义）/ 降级丢弃占位
- ChangeRecord::new(name, workflow_type, created_at, worktree, base_commit): 增两 Option 参

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| model_test · v1→v2 decode-only | 正向 | `ChangeRecordV1` 构造 → native_model 编码字节 → `decode::<ChangeRecord>` → 既有六字段逐字一致且 `worktree` / `base_commit` 均为 None（存量记录自动升级读出——AC-1 字面）；decoded version 断言 = 2（沿 AgentRunRecord v3→v4 用例形态） | 新增 |
| model_test · v2 回环 | 正向 | 带 Some(worktree) / Some(base_commit) 构造 → encode / decode 往返逐字段相等（含 None / Some 两态） | 新增 |
| model_test · 双向 From | 边界 | `From<ChangeRecordV1>`：两字段 None；`From<ChangeRecord> for ChangeRecordV1`：两字段丢弃（降级形态不作数据承诺——字段缺席即空） | 新增 |
| model_test · new 增参 | 边界 | `ChangeRecord::new` 四参 / 五参调用面（两 Option 显式传入构造字段一致；默认建档传 None = legacy 形态） | 新增 |
| model_test · 信封零改动覆盖 | 边界 | `list_models` 零改动覆盖 v2 形态（workspace 组注册 id=9 无冲突——既有信封行随动，null 出线经 serde 自动覆盖） | 适配 |
| model_test · 既有构造器全族 | 正向 | 既有各记录模型构造 / 注册断言——ChangeRecord 构造字面量补两参后持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯内存 native_model 编解码 + tempfile 真实 workspace db 注册（既有 Env 装置沿用） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- 既有文件重写 + 扩展节。挂 AC-1（db 文件名同名回归 + 记录映射双字段往返 +
     存量 v1 行库 additive 打开）。design D6：workspace_db_file_name 改委托 foundation
     单点（行为零变化）；readable_segment / is_illegal_name_char /
     READABLE_SEGMENT_MAX_CHARS 删除（清洗算法断言迁 identity_test）。 -->

#### 待测功能

- workspace_db_file_name(canonical_root: &str) -> String: 改为 `format!("{}.redb", foundation::identity::workspace_identity_segment(canonical_root))`（行为零变化——同名回归）
- create_change_record / find_change_record / list_change_records: ChangeStateRecord 双字段（worktree / base_commit）映射往返
- 存量 v1 行库 additive 打开：v1 表行经 native_model 版本机制自动升级可读

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| store_test · db 文件名同名回归 | 正向 | `workspace_db_file_name(root)` == `format!("{}.redb", foundation segment(root))`（委托等值）；真实 `WorkspaceStores` 打开两 root → `workspaces/` 子树文件名与单点派生逐字一致、异根互异（原「db 文件名派生」用例重写为委托等值断言，文件面断言保留） | 重写 |
| store_test · 清洗算法迁移 | 迁移 | 既有「派生单点纯函数可读段清洗_截断_非法字符_尾点空格与空回退」用例整体迁 foundation identity_test（行为平移零丢失），本文件删除原用例 | 迁移 |
| store_test · 双字段映射往返 | 正向 | `create_change_record`（ChangeStateRecord 携 Some 两字段）→ find / list 读出逐字段一致；重开同一 db 文件再读仍一致（真件回环——AC-1 新建档半边）；None 两态同往返 | 新增 |
| store_test · 存量 v1 行库打开 | 正向 | 裸 redb 直写 v1 表行（表名公式断言沿坏行注入先例 `9_1_name`，v1 编码字节注入）→ `open_workspace`（v2 模型组）additive 打开 → find / list 读出既有六字段一致且 worktree / base_commit = None（无手工迁移层——AC-1 存量库半边） | 新增 |
| store_test · dir_name 留守 | 边界 | `WorkspaceRecord.name` 展示名仍取目录末段（`dir_name` 留守 store 的既有语义断言持衡——非身份段成分） | 持衡（沿用） |
| store_test · 既有 change 域全族 | 正向 | 建档查重 / 开相 / 落账 / 回跳 / 归档翻转 / 步骤各既有行——ChangeRecord 构造字面量补两参后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 redb workspace db（既有 Env / StoresEnv 装置）；v1 行注入经裸 redb 表直写（沿 inject_corrupt_change_row 先例的表名公式与锁归还纪律） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/change_port.rs -> packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs

<!-- 既有文件扩展节。挂 AC-1（中性映射半边的 trait 面）。 -->

#### 待测功能

- impl ChangeStateStore for Store: get_change / list_change_records / create_change_record 的记录 ↔ 中性类型映射（双字段随映射单点透传）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_port_test · 双字段 trait 往返 | 正向 | 经 `&dyn ChangeStateStore` 类型擦除建档（携 Some 两字段）→ get / list 中性快照逐字段一致（映射单点无加工——worktree / base_commit 不在 port 面丢失） | 新增 |
| change_port_test · 既有映射全族 | 正向 | trait 全链映射 / 写命令翻译 / StoreFault 三分支 / Send + Sync 边界各既有行——构造字面量补两字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 workspace db + 真实 Store（既有装置） | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

<!-- 既有文件扩展节。挂 AC-2（命令装配半边）、AC-9（detail / read_artifact
     worktree 感知）、AC-10（归档引导命令面）。design D2：create_change 转 async +
     create_change_with<R> 泛型缝（沿 archive_change_with 先例）；D6 数据根注入面
     （State<PathBuf>）；IPC 入参与返回类型面不变（DTO 演进除外）。 -->

#### 待测功能

- create_change(app: AppHandle, root: String, name: String, goal: String) -> Result<CreateOutcome, String>: async + spawn_blocking（IPC 面不变）；装配 `vcs_runtime::worktree_dir(&data_root, &root, &name)` + `ProcessWorktree::new()`
- create_change_with<R: tauri::Runtime>: 泛型测试缝（生产 Wry / 测试 MockRuntime）
- get_change_detail / read_artifact: 读 record.worktree 传 locate_change 回退参

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · create 命令装配组合 | 正向 | MockRuntime app manage `PathBuf` 数据根 + 真实 git tempdir workspace（初始提交在案）→ `create_change_with` 成功：worktree 实落 `data_root/worktrees/{身份段}/<name>`（数据根注入面观察）、branch `change/<name>` 在案、DTO 四字段（worktree 绝对路径 + warnings 清单）、db 记录落 workspace 库（for_root 实例可查携 worktree / base_commit） | 新增 |
| mod_test · create 命令拒绝映射 | 异常 | 非 git 仓 root → 写面 Err 透传前端（引导文案面）；blank root 显式 Err 不进入写面链路（既有行持衡） | 适配 |
| mod_test · detail worktree 感知 | 正向 | 记录携 worktree + 主仓两树未命中 + worktree 内产物树 → `get_change_detail` Some 且 `worktree` 出线、artifacts 命中 worktree 内文件（record 先读后定位的命令面证据） | 新增 |
| mod_test · read_artifact worktree 命中 | 正向 | 同上夹具 → `read_artifact` 经 worktree 回退读取产物信封成功（主仓 miss 不致落空）；legacy 记录 → 主仓两树既有解析持衡 | 新增 |
| mod_test · list 归组随动 | 正向 | worktree 条目（active + 两树未命中）经 `list_changes` 命令入 active 组且状态面完整（读命令透传持衡——与 core list 行对拍） | 新增 |
| mod_test · 归档引导命令面 | 异常 | 记录携 worktree + 两树未命中 → `archive_change_with` Err 引导 merge 文案透传（AC-10 命令半边；与 core archive_test 文案同锚） | 新增 |
| mod_test · 既有全族 | 正向 | 读命令透传 / 信封出线 / blank 双口径 / archive 双写接线各既有行——种子构造补两 Option 字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / Tauri（进程边界） | MockRuntime mock app：manage 真实 WorkspaceStores + `PathBuf` 数据根（与 main.rs 注入面同型）+ `Arc<ChangeFlowControl>` / `Arc<StopRegistry>`（既有装置扩展）；命令真实走 `create_change_with` / `archive_change_with` 泛型缝 | 本节全部用例 |
| git / 安装进程 | 真实 git tempdir 仓 + 真实 ProcessWorktree（命令装配组合行的核心面——与 core 假件行互补）；bootstrap 命中面以无 lockfile 空仓规避分钟级安装（未知管理器注记路径） | create 组合 / 拒绝映射各行 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- 既有文件扩展节。挂 AC-6（发起装配半边）、AC-7（命令面复合键）、AC-8（legacy
     发起零变化）。design D9：exec root 解析三态 + is_dir 存在性前置校验；D10 控制
     面复合键机械传参（IPC 签名不变）；D8 compose 注入。 -->

#### 待测功能

- change_flow_start_with<R: tauri::Runtime>(app, on_event, root, change, auto_next_phase) -> Result<ChangeRunSummary, String>: 前置校验后解析 exec root（record.worktree → 绝对路径 + is_dir 校验；None → 主 root）；compose 注入 workspace root 实例；`RunRequest.root` / `StoreSnapshot::new` 恒 exec root；begin_run / subscribe / sink 桥复合键
- change_flow_stop_with / answer_with / confirm_with / state_with / watch_with: 控制面调用复合键（IPC 签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · worktree 目录缺失拒绝 | 异常 | 记录携 `worktree=Some(不存在路径)` → 发起 `Err` 含「worktree 目录不存在（可能已被手动删除）」引导且零 run 登记（control snapshot None）；仅带 worktree 记录生效——legacy 记录不经此校验 | 新增 |
| mod_test · exec root 解析成功 | 正向 | 记录携 worktree + 目录在场（tempdir 预置 openspec 树）→ 发起成功（提前 resolve summary running）；发起后 StepRecord / 相位半边落 **workspace root 库**（for_root(root) 实例可查），且数据根 `workspaces/` 子树无以 worktree 路径派生的第二库文件（store 身份恒 workspace root——AC-6 db 半边；PATH 隔离下 CLI 引擎合成收敛既有装置驱动） | 新增 |
| mod_test · legacy 发起零变化 | 正向 | `worktree=None` 存量建档 → 发起路径与既有用例逐字一致（exec root = 主 root——AC-8；既有正向行持衡即证） | 持衡（沿用） |
| mod_test · 复合键命令面 | 正向 | 同 app 双 root（WorkspaceStores 多 workspace）：rootA 同名 change run 运行中 → rootB 同名 change 发起成功（互不误拒）；同 root 同 change 二次发起 Err（并行冲突——既有行随复合键适配）；stop(rootA, x) 不影响 rootB 的 state 快照 | 重写 |
| mod_test · 控制面五命令传参 | 边界 | stop / answer / confirm / state / watch 各既有用例——控制调用补 root 段机械适配，断言面零改动（幂等 / Err 透传 / 补订语义持衡） | 适配 |
| mod_test · 前置校验既有全族 | 异常 | 未建档 / 相位表缺失 / blank 参数各既有行——seed 构造补两 Option 字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / Tauri（进程边界） | MockRuntime mock app 三态托管（既有装置）+ `PathBuf` 数据根扩展；Channel 捕获 / wait_for 轮询既有装置沿用 | 本节全部用例 |
| CLI 引擎（进程边界） | PATH 隔离窗口（CliMissing 合成收敛——既有 isolate_path 装置）：发起链路真实驱动但不 spawn 真实 CLI；db 半边落库断言不依赖引擎收敛形态 | exec root 解析成功 / 复合键各行 |

### packages/desktop/src-tauri/src/bindings/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 既有文件扩展节。挂 AC-9（bindings DTO 出线半边）。specta 生成物一致性守卫
     既有结构沿用（重导幂等 + golden diff 守卫）；`bindings:check` 工具链门见
     不可测试项 8。 -->

#### 待测功能

- CreateOutcome: `name` / `created` / `worktree: string` / `warnings: string[]`（恰四字段出线）
- ChangeDetail: 增 `worktree: string | null`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings_test · CreateOutcome 四字段 | 边界 | 生成 bindings 的 `CreateOutcome` 类型段恰 `name` / `created` / `worktree` / `warnings` 四字段（`worktree: string`、`warnings: string[]`）——无主仓 openspec 目录树路径字段（D2 恰四字段面） | 新增 |
| bindings_test · ChangeDetail.worktree | 边界 | `ChangeDetail` 类型段含 `worktree: string | null`（None → null 出线——类型面与 golden 面双锚） | 新增 |
| bindings_test · 签面不变 | 边界 | `create_change` 命令包装参数面不变（root / name / goal——async 化零 bindings 漂移；命令清单零新增零删除，既有清单断言数字不变随动） | 适配 |
| bindings_test · 既有守卫全族 | 边界 | 重导幂等 / 篡改恢复 / 父目录缺失导出各既有行——持衡 | 持衡（沿用） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 export-bindings 管线产物文件读写断言（既有 file_lock 串行化装置沿用） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs（扩展）+ tests/golden/ + tests/fixtures/README.md

<!-- 既有集成目标扩展节。挂 AC-9（golden 显式重写与语料两态半边）、AC-1（投影
     覆盖）。design D16 范围清单逐项承接：① 既有 detail golden worktree 键全量新增
     （null）；② worktree 建档语料新 golden；③ corpus-list-mixed 可能位移（D11）
     ——重写时人工确认留痕；④ CreateOutcome 断言面归 create_test serde 行与
     bindings 节；⑤ bindings 重导出归 bindings 节与工具链门。 -->

#### 待测功能

- db 种子语料构造器: worktree 维度两态样本（带 `worktree` / `base_commit` 建档样本 + legacy `None` 样本各至少一——spec desktop-corpus-regression 字面）
- golden harness: `DESKTOP_GOLDEN_REWRITE=1` 显式重写开关 + 对拍断言 + diff 范围键集断言（既有机制沿用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| corpus · worktree 建档语料 | 正向 | 新语料：建档携 `worktree=Some(固定绝对路径串)` / `base_commit=Some(固定 sha)` + worktree 内磁盘产物树（主仓两树未命中）→ detail 全读链投影：`worktree` 出线与库内记录逐字一致、artifacts 命中 worktree 树 → **新 golden 文件**（D16 ②） | 新增 |
| corpus · legacy None 投影 | 正向 | 既有建档语料（worktree=None）投影 `worktree` 出线 null——两态投影齐备（spec「各至少一个」对账；None 态由既有语料重写后覆盖） | 重写 |
| corpus · 既有 detail golden 重写 | 边界 | 既有四份 detail 语料 golden 全量新增 `worktree: null` 键（投影序列化含该键）——重写后重跑绿（D16 ①） | 重写 |
| corpus · list-mixed 归组位移 | 正向 | `corpus-list-mixed` 扩 worktree 条目行（db-active + 两树未命中）→ 该条目入进行中组；既有「db-active + 目录外移 archive 树」条目自归档组移回进行中组（D11 语义修订）——重写 diff 中该位移属预期，人工确认留痕（D16 ③） | 重写 |
| corpus · diff 范围键集断言 | 边界 | 既有「golden重写后_diff范围键集断言」扩展：detail golden `worktree` 键**恒在场**（建档样本 null / 非 null 两投影、文档形态 null 留位）——防静默漂移的进程内半边 | 重写 |
| corpus · 语料完整性 | 边界 | golden 目录文件集与语料集合一致（新 golden 文件入列）；fixtures/README.md 覆盖面矩阵增「worktree 两态」行且语料完整性断言随动（矩阵与语料集合对账） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile workspace db（种子经 store change 域操作面真件）+ 真实磁盘产物树（主仓树 + worktree 树分置）+ golden 快照文件真实读写；`DESKTOP_GOLDEN_REWRITE` 环境开关仅测试装置；worktree 路径样本为固定字面量（不依赖真实 git——投影只消费记录字段与目录树） | 本节全部用例 |

### packages/desktop/src/views/changes/components/change-create-dialog.tsx -> packages/desktop/src/views/changes/components/change-create-dialog.test.tsx

<!-- 既有文件重写 + 扩展节。挂 AC-9（create 对话框成功面半边）。design D14：提交
     成功后成功面替换表单（名称 / worktree 路径 break-all / 警告清单行内逐条 /
     「进入详情」按钮调 onCreated）；toggle 收起重开即重置；成功不自动导航。 -->

#### 待测功能

- 成功面（新）: `CreateOutcome` 返回后替换表单——名称、worktree 绝对路径（break-all）、warnings 清单（有则行内逐条）、「进入详情」按钮（onCreated(name)）
- 表单既有面: 本地 kebab-case / 长度 / 必填校验、trim 提交、错误行内呈现（持衡）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| dialog · 成功面替换表单 | 正向 | invoke resolve（fixture 携 worktree 路径 + 警告两条）→ 表单输入区消失、成功面呈现名称与 worktree 路径（break-all 类锚）+ 警告逐条行内 + 「进入详情」按钮；onCreated 此时**零调用**（不自动导航——重写既有成功流转行：成功即导航改为按钮驱动） | 重写 |
| dialog · 进入详情回调 | 正向 | 点击「进入详情」→ onCreated(name) 恰一次（携带 trim 后名称）；按钮可重复渲染但回调恰一次（或点击后语义稳定——以实现定稿，断言恰一次口径） | 新增 |
| dialog · 空警告清单 | 边界 | fixture warnings = [] → 无警告清单区块渲染（空清单零占位）；worktree 路径仍呈现 | 新增 |
| dialog · toggle 收起重开重置 | 边界 | 成功面在场时收起再展开 → 空白表单（CreateForm 卸载重建——成功面与输入态零残留）、再次提交可发起第二次 invoke | 新增 |
| dialog · DTO fixture 类型面 | 边界 | OUTCOME fixture 增 `worktree` / `warnings` 字段（bindings 演进后的类型编译锚——旧两字段 fixture 不再编译） | 重写 |
| dialog · 校验与错误面全族 | 正向 | trim 透传 / free-form goal / 必填禁提交 / 非法名全族 / 恰 128 上界 / 错误行内呈现各既有行——持衡（fixture 增字段随动） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke（既有 vi.hoisted 装置沿用，应答 resolve（CreateOutcome 四字段 fixture）/ reject 可切换）；校验与流转逻辑真实组合渲染不 mock；onCreated 以 vi.fn spy 经 props 注入 | 本节全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 既有文件扩展节。挂 AC-9（详情 worktree 信息行半边）。design D14：DetailHeader
     增 worktree 信息行（detail.worktree !== null 渲染，break-all；legacy null 不渲染）。 -->

#### 待测功能

- DetailHeader（组件内部函数）: worktree 信息行两态渲染（非 null 渲染路径 / null 不渲染）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail-view · worktree 信息行渲染 | 正向 | detail fixture `worktree = '/home/u/.dev-team/worktrees/…/fix-bug'` → 头部呈现该路径（break-all 类锚；data-testid 随实现定稿） | 新增 |
| detail-view · legacy 不渲染 | 边界 | detail fixture `worktree = null` → 该信息行不出现（legacy 零占位——负断言） | 新增 |
| detail-view · 既有头部与全页面 | 正向 | 状态徽章 / 日期 / 运行中徽章 / 两态分流 / 降级页各既有行——detail fixture 增 worktree 字段后断言零改动持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke + Channel 返回 detail DTO 夹具（既有装置沿用，fixture 增 `worktree` 字段）；内部 hooks 真实组合 | 本节全部用例 |

---

## 不可测试项

1. AC-11 静态半边：crate 图依赖白名单（`vcs-runtime` workspace 内依赖恰 `workflow` + `foundation` 且零 tokio 零 Tauri；`store` 增 `foundation` 边；core 各 crate 无 `store` / `vcs-runtime`；desktop-app 装配边） — **原因**: Cargo 依赖图是编译期约束（越线即不可编译 / 不可达），无进程内行为断言面；由 tasks 阶段七 crate 图审查（`server:check` + 人工对账）承载。身份段「仅 foundation 一份」的行为半边经 identity_test 同源锚 + store_test 委托等值行承载。
2. AC-6 / AC-11 静态半边：全 desktop 源码 `for_root` 调用点无一以 worktree 路径为参、git `worktree add` / `branch -D` / bootstrap spawn 触点仅 `crates/infra/vcs`、`core/workflow` 零进程 spawn 零 tokio — **原因**: 全源码静态守线（design 阶段七明定 grep 对账），测试夹具豁免使其无法全量自动化；进程内行为面经 compose_test 注入行（store 半边唯一入口 = 注入参）、create_test 假件行（port 为唯一缝）与 change_flow mod_test 数据根无第二库文件行承载。
3. AC-5 超时半边：bootstrap 900 s 到点 kill 路径 — **原因**: 超时为 vcs-runtime 常量、设计不设注入缝，900 s 墙钟超出单测窗口且不可确定性触达；退出态 / cwd / 环境继承 / 摘要截断 / 拉起失败各面已真实快命令锚定，超时分支以常量在场 + kill 代码形态归阶段七静态审查（若实现期引入可测缝再回补本表）。
4. AC-9 流程半边：golden 重写 diff 的人工确认与留痕（D16 ①–③） — **原因**: 流程性动作（人工审阅 diff 范围属预期语义演进并留痕），不可自动化；其测试半边（重写后重跑绿 + diff 范围键集断言 + 语料完整性）已落 corpus_golden_test.rs 节。
5. AC-10 版本交付半边：`packages/desktop/package.json` version 升一位（`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动、`plugins/dev-team` 保持 2.10.44） — **原因**: 静态版本声明无行为面。**基线勘误（test-design 时点）**：design D15 定稿的 0.4.16 基线已被 design 落档 commit 携带的版本 bump 先行消费（工作区现状 0.4.17）；实施 commit 按 D15「用户可见新功能升一位、以实施时现状为基、不降版」语义执行 **0.4.17 → 0.4.18**，数值以实施 commit 为准，非测试断言面。
6. AC-10 工具链门半边：`server:check` / `client:check`（含 knip 零新增豁免）/ `bindings:check` / `vp test` 全绿 — **原因**: 工具链级套件门非单一测试文件可承载，归 test-execution 阶段承接验证（tasks 阶段七同口径）；bindings 出线行为面经 bindings/mod_test.rs 节承载。
7. 门面 / 纯类型 / 生成物 / 装配模块不建独立测试文件：`core/workflow/src/write/worktree.rs`（WorktreePort trait + RepoProbe / InstallRun 流量词汇面）、`core/workflow/src/state.rs` 字段面、`core/workflow/src/write/mod.rs`、`core/foundation/src/lib.rs` 门面、`packages/desktop/src/types/generated/bindings.ts` 生成物、`src/main.rs` 的 `app.manage(data_root)` 注入行、各 `Cargo.toml` / `package.json` / `tauri.conf.json` — **原因**: 纯 trait 声明 / 类型迁移 / re-export / 生成物 / 单行装配无自有行为，空框架章节自相矛盾；行为去向：WorktreePort 经 create_test 假件与 vcs 真件两节消费承载；state.rs 两 Option 字段经 create / detail / list / store 各节构造与断言承载；bindings.ts 经 bindings/mod_test.rs + `bindings:check` 守卫；data_root 注入行经 commands/changes mod_test 组合行（MockRuntime manage PathBuf 同型）承载；清单文件经阶段七对账承载。
8. 语料文档：`tests/fixtures/README.md` — **原因**: 语料矩阵说明文档无独立行为，其矩阵语义经 corpus_golden_test.rs 语料完整性断言行对账承载（本变更随 worktree 两态行扩展）。
9. 纯测试代码机械适配（零新增行为断言）：`walker_test.rs` / `steps_test.rs` / `snapshot_test.rs` 既有假件随 control 复合键签名与 compose 拆参的调用点改写、`commands/exec/mod_test.rs` 既有 agent_start 用例随 compose 新签名适配（会话仍主 root 语义既有断言持衡）、`commands/change_flow/mod_test.rs` 既有控制面用例 root 段补齐 — **原因**: design 不变组件核对结论明定的机械适配，行为覆盖面由 control_test（复合键全语义）、walker_test exec-root 锚定行、compose_test 注入行与 exec 既有落库断言承载；适配行在各扩展节以「适配」迭代类型就地声明。
10. 环境依赖面：真实 git 在测试环境的可用性（vcs 真件各节前提）、Windows `cmd /C` 与 Unix `sh -c` 的平台分支互斥覆盖、跨盘 worktree 与深路径（MAX_PATH）行为 — **原因**: 与产品硬依赖同口径的环境前提（git 缺失路径本身有注入用例覆盖 Err 面）；平台分支经 CI 矩阵承载（单平台单测只触达本机分支）；跨盘 / 深路径属设计显式接受的环境风险（proposal 风险表 R5 / MAX_PATH 行），无确定性单测面。
