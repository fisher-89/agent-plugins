# 任务: desktop-change-worktree

> **变更**: desktop-change-worktree
> **依据**: proposal.md + design.md（D1–D16 决策编号见 design）

任务边界：本列表只含实现任务；测试编写（create 建域 / 拒绝面 / 补偿链 / vcs 真实 git 夹具 / 复合键并行 / exec root 驱动 / 读面回退 / 命令面 DTO）、语料 worktree 两态样本与 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工确认留痕（design D16 范围清单）、bindings 复核由 test-design / test-gen / test-execution 阶段承接。任务内引用的文件均在 design.md 变更清单内；工作区在飞的 `infra/agent` SDK loop 重试改动属另一变更，不在本列表（design 隔离注记）。

## 阶段一：crate 图与身份段单点（独立可编译，行为零变化）

- [x] `crates/core/foundation/src/identity.rs`（新）：`workspace_identity_segment(canonical_root: &str) -> String`——sha256 前 16 字节 32 位小写 hex + 可读段清洗（非法字符置换 `_`、char 截断 ≤24、去尾 `.` 与空格、空回退纯哈希），算法与参数自 `store.rs` 逐字平移；`crates/core/foundation/src/lib.rs` 挂 `pub mod identity` 并更新 crate 文档注释（layout 解析 + 身份段派生两能力）；`crates/core/foundation/Cargo.toml` 增 `sha2`
- [x] 新 crate `crates/infra/vcs`（裸名 `vcs-runtime`）：`src/lib.rs` 携 `WORKTREES_DIR_NAME = "worktrees"` 常量与 `worktree_dir(data_root: &Path, workspace_root: &str, change: &str) -> PathBuf`（消费 foundation 身份段；design D6）；`src-tauri/Cargo.toml` `[workspace] members` 增目录、`[workspace.dependencies]` 增 `vcs-runtime`、根包 `[dependencies]` 增 `vcs-runtime`；crate 本阶段依赖仅 `foundation`（`workflow` 边随阶段三 port 类型落位）；零 tokio 零 Tauri

## 阶段二：store v2 与状态缝字段面（编译期一次补齐字面量）

- [x] `crates/infra/store/src/model.rs`：现 `ChangeRecord` 改名 `ChangeRecordV1`（`#[native_model(id = 9, version = 1)]`，`pub(crate)`，不入模型组）；新 `ChangeRecord`（id=9 version=2 `from = ChangeRecordV1`）增 `worktree: Option<String>` / `base_commit: Option<String>`（`#[serde(default)]`）；双向 `From`（升级两字段 None = legacy 语义 / 降级丢弃占位，provider context_length 先例同模式，design D7）；`ChangeRecord::new` 增两 Option 参
- [x] `crates/infra/store/src/store.rs`：`workspace_db_file_name` 改为 `format!("{}.redb", foundation::identity::workspace_identity_segment(canonical_root))`（行为零变化）；删除 `readable_segment` / `is_illegal_name_char` / `READABLE_SEGMENT_MAX_CHARS`（`dir_name` 留守——`WorkspaceRecord` 展示名仍消费，非身份段成分）；`create_change_record` / `change_state` 记录映射增双字段往返；`crates/infra/store/Cargo.toml` 增 `foundation`
- [x] `crates/core/workflow/src/state.rs`：`ChangeStateRecord` 增 `pub worktree: Option<String>` / `pub base_commit: Option<String>`（执行锚引用、不参与库身份派生的注释）；受牵构造点机械补齐——`write/create.rs` 建档字面量暂以 `None`/`None` 占位（建域组合阶段三替换）

## 阶段三：WorktreePort 与写面建域、归档引导

- [x] `crates/core/workflow/src/write/worktree.rs`（新）：`WorktreePort` trait 六方法（`probe` / `branch_exists` / `add_worktree` / `remove_worktree` / `delete_branch` / `run_install`，sync 零 tokio，design D1）+ `RepoProbe { head, dirty }` + `InstallRun { success, summary }`；`write/mod.rs` 挂载并导出三类型
- [x] `write/create.rs` 建域扩展（design D2/D3/D4/D5）：签名改 `create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name, goal)`；两棵 Layout 写面内经 `resolve` 自铸；前置七道（名称 / goal / 主仓 active 目录 / db 同名 active / git probe 三态显式 Err 引导且 MUST NOT 静默回退主 root / branch `change/<name>` 冲突 / worktree 目录冲突——全零副作用）；建档先行（`worktree` / `base_commit` 入记录）→ `add_worktree`（HEAD 基线）→ worktree 内目录树 + explore.md → 脏仓警告 → bootstrap（映射表四行首匹配、Cargo 无 lock 专项注记、未知管理器注记、失败不回滚入警告）；补偿链（add 失败：删建档 + 尽力删分支；树写出失败：remove_worktree --force → 删分支 → 删建档，尽力链失败 Err 呈现残留与 `git worktree list` 手动清理指引）；`CreateOutcome` 增 `worktree: String` / `warnings: Vec<String>`；警告文案按 design D5 单点
- [x] `write/archive.rs`：active / archive 两树未命中的 Err 分支前插入 worktree 前置——`record.worktree.is_some()` → 显式拒绝并引导「先 merge worktree 分支 change/<name> 回主仓再归档」（design D13，非泛化「目录未找到」）；归档不触碰 worktree / branch
- [x] `crates/infra/vcs`：`git.rs` 同步 git 子命令族（`git -C`：`rev-parse --is-inside-work-tree` / `rev-parse HEAD` / `status --porcelain` / `show-ref --verify --quiet refs/heads/<branch>` / `worktree add -b <branch> <path>` / `worktree remove --force` / `branch -D`；拉起失败映射「git 不可用（PATH 未发现 git）」引导文案；空仓 HEAD 失败映射「先提交」引导）；`bootstrap.rs` `run_install`（shell 包装 Windows `cmd /C`、其余 `sh -c`，static_check 先例；cwd = worktree；环境全继承零注入；900 s 超时轮询 try_wait 到点 kill；stdout+stderr 尾部 ≤300 字摘要）；`lib.rs` `ProcessWorktree` 装配 `impl WorktreePort`；`Cargo.toml` 增 `workflow` 依赖（port 类型）

## 阶段四：读面 worktree 感知与 create 命令装配

- [x] `queries/mod.rs`：`locate_change(layout, worktree: Option<&str>, name)`——解析链主仓 active 精确 → archive 精确 → archive 日期前缀 → worktree 回退（`resolve(worktree).changes_root/<name>` is_dir → `ChangeSource::Active`，design D12）；单分量名校验保留
- [x] `queries/detail.rs`：record 先读后定位（worktree 自记录直传 locate）；建档记录恒可达详情（定位全 miss → dir 缺席、产物清单空、状态面在，`source` 自 record.status 映射）；record 与定位双缺 → None（既有语义）；`ChangeDetail` 增 `worktree: Option<String>`（直读透出，None → null）
- [x] `queries/list.rs`：`db_entry` 归组改 `record.status` 权威（active → 进行中组、archived → 归档组）；磁盘目录仅决定目录名取位（active 主仓命中 → 建档名；archived → archive 树精确 / 日期前缀名；未命中 → 建档名）；归档月份逻辑不变（design D11 语义修订留痕）
- [x] `src/commands/changes/mod.rs`：`create_change` 转 async + `create_change_with<R: tauri::Runtime>` 泛型测试缝（沿 `archive_change_with` 先例），装配 `vcs_runtime::worktree_dir(&data_root, &root, &name)`（`State<'_, PathBuf>` 数据根）+ `ProcessWorktree::new()`，经 `tauri::async_runtime::spawn_blocking` 调 sync 写面（design D2，IPC 面不变）；`get_change_detail` / `read_artifact` 读 record.worktree 传 `locate_change` 回退参；组文档注释更新（worktree 维度）

## 阶段五：run 双 root 与复合键

- [x] `crates/infra/agent/src/compose.rs`：`compose_turn(stores, registry, store: Arc<Store>, agent)` 拆参——store 半边命令层预解析注入，`for_root` 调用自 compose 删除、root 参移除（design D8；cwd 半边经既有 `SessionCtx` 通道零改动）
- [x] `src/commands/exec/mod.rs`：`agent_start` 机械适配新签名（`stores.for_root(&root)` 后注入；会话仍主 root，D7 拍板——语义零变化）
- [x] `crates/core/orchestration/src/control.rs`：注册表键改 `(String, String)`（workspace root, change）复合；`begin_run` / `subscribe` / `request_stop` / `current_session` / `publish` / `set_session` / `answer` / `confirm` / `snapshot` 签名增 root 段；`RunGuard` 持复合键；`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动（design D10）
- [x] `src/commands/change_flow/mod.rs`：前置校验后解析 exec root（`record.worktree` → 绝对路径 + is_dir 存在性校验（仅带 worktree 记录生效，缺失显式 Err 引导；legacy 零变化），None → 主 root，design D9）；`compose_turn` 注入 workspace root 实例；`RunRequest.root` / `StoreSnapshot::new` 恒 exec root；`begin_run` / `subscribe` / `ChangeFlowSink` 改复合键；stop / answer / confirm / state / watch 五命令控制面调用复合键（IPC 签名不变）；walker / steps / snapshot 装配形态不动

## 阶段六：壳层注入与前端可见性

- [x] `src/main.rs`：setup 内 `app.manage(data_root.clone())`（PathBuf 状态——worktrees 落位派生的注入面，沿 store 注入式打开纪律）；setup 注释更新
- [x] `packages/desktop/src/views/changes/components/change-create-dialog.tsx`：提交成功后成功面替换表单——名称、worktree 绝对路径（break-all）、警告清单行内逐条、「进入详情」按钮（调 `onCreated`）；toggle 收起重开即重置（design D14）
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`DetailHeader` 增 worktree 信息行（`detail.worktree !== null` 渲染，break-all；legacy null 不渲染）
- [x] `packages/desktop/src/types/generated/bindings.ts`：经 `pnpm -C packages/desktop run bindings:export` 重导出（`CreateOutcome.worktree/warnings` + `ChangeDetail.worktree`）
- [x] `packages/desktop/package.json` `version` 0.4.16 → 0.4.17（design D15：spec 数值 0.4.14 为提案快照，按「升一位」语义以现状为基；`tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）

## 阶段七：守线收口（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零错误；crate 图审查：`vcs-runtime` workspace 内依赖恰 `workflow` + `foundation` 且零 Tauri 零 tokio；`store` workspace 内依赖为 `agent` + `foundation`（新增）+ 既有 `workflow` 中性类型边（db-state 既成事实，design AC-11 注记留痕）；core 各 crate（foundation / config / workflow / agent）依赖无 `store` / `vcs-runtime`
- [x] 双 root 不变量自查：全 desktop 源码 grep `for_root` 调用点——无一处以 worktree 路径为参（store 半边唯一入口 = compose 注入与命令层装配）；grep git `worktree add` / `branch -D` / bootstrap spawn 触点仅 `crates/infra/vcs`；`core/workflow` 零进程 spawn（`WorktreePort` 为唯一缝）且写面零 tokio
- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零错误且 knip 零新增豁免；`pnpm -C packages/desktop run bindings:check`（导出 + `git diff --exit-code -- src/types/generated`）绿
- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏；在飞 SDK loop 重试改动不在本变更面）；`plugins/dev-team` 零改动复核（版本 2.10.44 与三类交付产物未动）；AC-10 版本交付 0.4.17 已执行
