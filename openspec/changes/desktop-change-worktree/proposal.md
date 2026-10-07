# 提案: desktop-change-worktree

> **变更**: desktop-change-worktree
> **日期**: 2026-10-06
> **状态**: draft
> **探索**: `openspec/explores/desktop-change-worktree.md`（决策 D1–D7 用户拍板在案）

---

## 问题

desktop 的 change run 今天把所有编辑直接落在主仓工作区。根源是单一 `root: String`（前端每命令显式传入）在四个身份上复用：① store 身份（`compose_turn` 内 `for_root` 派生 workspace 库）；② 会话 cwd（CLI 引擎 `current_dir` 与 SDK 路径沙箱前缀校验的锚）；③ 工件根（`Layout::resolve` 定位 `openspec/changes/<n>/`）；④ 检查器 cwd（static-check / test-execution / git diff 的执行目录）。由此产生四个结构性痛点：

1. **编辑混仓**：change 编辑与主仓未提交改动混在同一工作区——`git diff HEAD` 把无关改动一并喂进 executor / evaluator prompt（信噪比差）；归档后用户仍需从混合工作区手动摘出本 change 的改动才能形成干净提交，与「完成即归档即提交」的流程期望脱节。
2. **并行不可能**：两个 change 同时 run 必然在主仓工作区互相踩踏；且 `begin_run` 只按 change 名做键、不含 root（`core/orchestration/src/control.rs:40`）——两 workspace 同名 change 已会假冲突（先例 bug），真并行解锁后必现。
3. **基线不可追溯**：change 从主仓当前（可能脏的）状态起跑，fork 点无记录，事后无法回答「这个 change 从哪个提交起做」。
4. **无弃置面**：change 做废即主仓工作区残留半成品，与用户其他未提交工作纠缠，清理成本高。

explore 核实的三个有利事实：**相位状态机零 fs**（phase_next / phase_log / backtrack 只吃 `ChangeStateStore`、不知道路径——root 撕裂后原地不动）；**会话 cwd 经 `turn.root → SessionCtx.workspace_root` 通道逐轮传入**（SDK 路径沙箱前缀校验以 cwd 为锚，cwd 换 worktree 后文件编辑自动囚于 worktree；`SessionRecord` 不存 cwd，resume 链零改动）；**walker 的 `RunRequest.root` / `ToolStepRequest.root` / `StoreSnapshot` 的 fs 半边天然指向执行根**——唯一深度重构点是 `compose_turn` 内部 `for_root` 拆参（直接喂 worktree 路径会铸出空库：provider / 会话 / 建档全消失）。

三个硬点：worktree 路径必须永不进 `for_root`（store 身份锚定 workspace root 是核心不变量）；全新 worktree 无 `node_modules` / `target/`，static-check / test-execution 是确定性步骤无 agent 救场、首跑直接失败（需确定性 bootstrap）；`git worktree add` 是硬依赖（git diff 是降级面，不可比照——create 需 git 存在性前置）。

---

## 提案

把单一 root 撕成两个身份，worktree 作为执行锚落地：

1. **双 root 不变量**：workspace root（用户注册的 canonical root）是唯一 store 身份锚——`for_root` 派生、provider / 会话 / change 建档 / 相位状态全部钉在其上；exec root（= 该 change 的 worktree）承担会话 cwd、工件根、git diff 与检查器执行目录。`compose_turn` 拆参：store 半边由命令层预解析的 workspace root 库实例注入（命令层已 `for_root` 一次，缓存命中无害），cwd 半边经既有 turn 通道零改动。db 与 worktree 零耦合（`~/.dev-team/workspaces/` 与 `~/.dev-team/worktrees/` 相邻子树；git 树永不感知 db）。
2. **create 建域扩展**：写面 `create` 由「建档 + 目录 + explore.md」扩为「建档（`ChangeRecord` 增 `worktree` / `base_commit` 字段）→ `git worktree add` + branch `change/<name>`（基线恒主仓 HEAD）→ worktree 内建 `openspec/changes/<n>/` 目录树与 explore.md → 确定性 bootstrap」。前置校验扩展（全部 IO 前）：git 可用性（git 不可发现或主仓非 git 仓 → 显式 `Err` 引导，不静默回退主 root 创建）、branch `change/<name>` 已存在、worktree 目标目录已存在——对齐既有三道校验前置风格。脏主仓警告但不阻止（基线仍 HEAD，文案引导「先提交再开新 change」）。
3. **确定性 bootstrap**：探测 worktree 内 lockfile（pnpm-lock / package-lock / yarn.lock / Cargo.lock 等已知管理器映射）→ 在 worktree 内 spawn 安装命令；未知 / 无已知管理器跳过并在警告面注记；失败不回滚 create、警告立即呈现（后续确定性检查步大声失败兜底）。
4. **run 组合切换**：`change_flow_start` 解析 exec root（db 记录带 worktree → worktree 路径；legacy 记录 `worktree=None` → 主 root），`RunRequest.root` 恒 exec root；会话 cwd / Layout / git diff / static-check / test-execution 全部随之落在 worktree；worktree 内 `git diff HEAD` 恰为本 change 编辑集（prompt 信噪比提升为顺带收益）。run 收口停等语义不变（全相位 pass「Ready for archiving」，不自动归档不自动 merge）。
5. **begin_run 键修正**：并行冲突键改 `(workspace root, change)` 复合——顺手修掉既有两 workspace 同名 change 假冲突 bug；worktree 隔离解锁同 workspace 多 change 真并行（各自 exec root，db 进程内并发既有支撑）。
6. **merge-first 归档**：用户手动 merge worktree 分支回主仓 → 主仓出现 `changes/<n>/` → 既有 archive 双写（status 翻转 + 目录改名）零改动可用；未 merge 即归档时以引导文案显式拒绝（「先 merge worktree 分支回主仓再归档」，优于现有泛化「目录未找到」）。merge 不被强制（弃置 = 手动清 worktree / branch，db 记录保留）。
7. **读面与可见性**：list 归组以 db status 为权威（worktree change 主仓两树未命中不入未知组、不被丢弃，工件经 worktree 路径解析；`locate_change` 增 worktree 回退）；detail 与 `CreateOutcome` 出线 worktree 绝对路径（review / 手动 commit / merge 可达）与警告清单（wire contract 冻结约束下走 golden 显式重写）。
8. **spawn 落位**：git worktree / 脏仓探测 / bootstrap 安装的进程执行新立 **vcs 边界**（第七类）落 `crates/infra/vcs`（裸名 `vcs-runtime`）；port 定义留 core/workflow（消费者归属，checks 先例）；worktree 子目录身份派生单点 `{可读段}-{sha256 前 16 字节 32 位 hex}` 上提 foundation——db 文件名与 worktree 子目录同源消费，恰好触发 foundation「第二消费者出现才下沉」纪律。

---

## 能力

### 新增能力

- **desktop-change-worktree** — change worktree 生命周期与双 root 执行：落位与身份派生单点、create 分配 worktree 与分支基线（HEAD）、双 root 不变量（db 身份锚 workspace root / 执行锚 worktree）、确定性 bootstrap、脏仓警告、merge-first 归档引导、worktree 路径 UI 可见性、V1 边界留痕（手动清理 / legacy 主 root / agent_start 与 explore 留主仓 / config 冻结 / 构建成本接受 / 孤儿对账 / report_dir 失效接受）。

### 修改的能力

- **desktop-change-create** — 写面 `create` 扩为建档（含 worktree / base_commit）+ worktree 建域 + worktree 内目录树与 explore.md + bootstrap + 警告收集；前置校验扩展（git 可用性 / branch 冲突 / worktree 目录冲突）；`CreateOutcome` 增 `worktree` 与 `warnings` 字段（路径不下沉条款修订：worktree 绝对路径为刻意出线的执行锚）。
- **desktop-change-state-store** — `ChangeRecord` 字段面增 `worktree: Option<String>` 与 `base_commit: Option<String>`（native_model v1→v2 decode-only 升级，provider context_length 先例同模式；存量记录读出 None = legacy 主 root 语义）；归档对带 worktree 记录增 merge-first 前置与引导。
- **desktop-change-orchestration** — 新增 run 双 root 组合与 exec root 解析（store 半边恒 workspace root、`compose_turn` 注入化拆参）；运行控制面并行冲突键改 `(workspace root, change)` 复合。
- **desktop-change-queries** — 列表归组以 db status 权威覆盖 worktree 条目（工件经 worktree 路径解析）；详情出线 worktree 路径（None → null）与产物发现经 exec root 解析。
- **desktop-corpus-regression** — db 种子语料覆盖 worktree 维度（带 worktree / base_commit 建档样本与 legacy None 样本两态）。
- **desktop-crate-layout** — 边界分类学扩为七类（新增 vcs 边界：外部进程 + 版本控制工作面；port 留 `core/workflow`、实现落 `crates/infra/vcs` 裸名 `vcs-runtime`、既有 `infra/agent` 内 git_diff 迁入留后续）；未来租户归位表增行（git 工作面 → 已落地行）；依赖规则第二例修订：`store → foundation` 身份段派生纯函数（db 文件名与 worktree 子目录同源单点，store 内不残留第二份派生实现）与 `desktop-app → vcs-runtime` 装配边；foundation 能力面注释随身份段派生更新（「保持极小」scenario 口径同步为 layout + 身份段派生两能力）。

### 引用沿用（零 delta）

- desktop-agent-execution — SDK 路径沙箱以会话 cwd（exec root）为锚的机制零改动背书「编辑囚于 worktree」；`compose_turn` 拆参为签名演进、消费纪律（每轮 compose → begin → open_session）不变。
- desktop-checks-domain — static-check / test-execution 的 cwd 跟随 exec root（spawn 入参既有通道），检查域语义零改动。
- desktop-workspace-store — `workspace_db_file_name` 改为消费 foundation 身份段单点，派生算法与产物 db 文件名零变化（行为零变化；库布局与打开注入契约零改动），`for_root` 身份锚定语义不变（双 root 不变量的 store 半边）。
- desktop-file-watch / desktop-data-dimensions — 零触点（watch 通道唯一消费方是 explore 页；worktrees 子树是 git 管理的执行锚，非 db 维度载体）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/foundation/src/`（新模块）— workspace 身份段派生纯函数单点（`{可读段}-{sha256 前 16 字节 32 位 hex}`；可读段清洗与截断规则自 store 平移）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — `workspace_db_file_name` 改为消费 foundation 单点（行为零变化）；`model.rs` — `ChangeRecord` v2（`worktree` / `base_commit` Option 字段 + native_model v1→v2 decode-only `from`）
- `packages/desktop/src-tauri/crates/infra/vcs/`（新 crate，裸名 `vcs-runtime`）— `git worktree add` / `git status --porcelain` / bootstrap 安装命令 spawn（PATH 探测前置沿 bash.rs / static_check 先例）；零 Tauri
- `packages/desktop/src-tauri/crates/core/workflow/src/write/`（新 port + create 改造）— WorktreePort 缝（add / 脏仓探测 / bootstrap，spawn 不进 core）；`create.rs` 建域组合扩展（校验前置扩展 + 补偿链 + 警告收集 + `CreateOutcome` 演进）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/` — `locate_change` 增 worktree 路径回退；detail 出线 worktree 路径
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` — `begin_run` / `subscribe` 键改 `(root, change)` 复合
- `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` — store 半边注入化拆参（命令层预解析库实例）
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — exec root 解析（db 记录 worktree 字段 → worktree 路径，None → 主 root）+ compose 注入装配
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` — `agent_start` 随 compose 签名机械适配（会话仍主 root，D7）
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — `create_change` DTO 演进（`worktree` / `warnings`）
- `packages/desktop/src/views/changes/components/change-create-dialog.tsx` — 成功面呈现 worktree 路径与警告清单（行内）
- `packages/desktop/src/views/changes/`（详情信息面）— worktree 路径信息行（legacy null 不呈现）
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO 演进重生成
- `packages/desktop/package.json` — `version` 0.4.13 → 0.4.14

### 测试文件

- `crates/infra/store/src/model_test.rs` / `store_test.rs` — v1→v2 decode-only 存量升级、新字段回环、身份段单点平移后 db 文件名同名回归
- `crates/core/workflow/src/write/create_test.rs` — worktree 建域组合、扩展拒绝面（git 缺失 / 非 git 仓 / branch 冲突 / 目录冲突）、脏仓警告、bootstrap 失败不回滚、补偿链
- `crates/infra/vcs/src/*_test.rs`（新）— 真实 git tempdir 夹具：worktree add / branch / status 探测 / bootstrap 命令组装（spawn 以 fake/夹具锚定）
- `crates/core/workflow/src/queries/*_test.rs` — locate worktree 回退、detail worktree 出线（None → null）、legacy 归组零变化
- `crates/core/orchestration/src/control_test.rs` / `walker_test.rs` — 复合键并行（异 workspace 同名不误拒、同 workspace 两 change 并行）、exec root 驱动（假引擎 + 假写面）
- `crates/infra/agent/src/compose_test.rs` — 注入化签名演进回环
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/` — worktree 维度种子样本 + `DESKTOP_GOLDEN_REWRITE=1` 显式重写
- `src/commands/changes/mod_test.rs` / `src/commands/change_flow/mod_test.rs` — exec root 解析、DTO 字段面、归档引导文案
- `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx` 等 — 路径与警告呈现断言

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（MCP 工具、hooks、CLI 工作流、版本 2.10.44、三类交付产物）
- `crates/core/workflow/src/write/` 相位机操作语义（phase_next / phase_start / phase_log / backtrack / archive 双写——状态机零 fs，原地不动；archive 仅增 worktree 前置引导分支）
- 相位表与 prompt 模板（`phase_table.rs`；proposal 交接行文本零改动——executor cwd 在 worktree，经相对通道自然读 worktree 内 explore.md）
- `SessionRecord` / 会话转录 / 轮统计行记录面（零迁移零改形；resume 链零改动）
- SDK 路径沙箱实现（`sdk/sandbox.rs` 机制零改动，锚随 cwd 自然切换）
- `crates/infra/watch/`（零触点）；`agent_start` 命令语义（仅随 compose 签名机械适配）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | ChangeRecord v2 与身份段单点 | 含 v1 记录的存量库 additive 打开，旧记录读出 `worktree=None` / `base_commit=None`（decode-only，无手工迁移）；新建档回环逐字段一致；`list_models` / `scan` 零改动覆盖；身份段派生上提 foundation 后 db 文件名同名回归通过（同根恒同名） |
| AC-2 | create worktree 建域 | 干净 git 仓上 create 成功：worktree 落位 `~/.dev-team/worktrees/{身份段}/<name>`、branch `change/<name>` 存在且基线 = 主仓 HEAD、worktree 内 `openspec/changes/<name>/explore.md` 内容为 goal 原文、db 记录含 `worktree` 路径与 `base_commit`；主仓 active 树不含该目录 |
| AC-3 | 前置校验与拒绝面 | git 不可发现 / 主仓非 git 仓 / branch `change/<name>` 已存在 / worktree 目录已存在 / 同名 active 建档 / 非法名称 / goal 空白——各返回显式 `Err` 且零 worktree、零建档、零目录（拒绝面全 IO 前置） |
| AC-4 | 脏仓警告 | 主仓构造未提交文件后 create 成功且 `warnings` 含「先提交再开新 change」引导；worktree 内容不含该未提交改动（基线恒 HEAD）；干净仓 `warnings` 为空 |
| AC-5 | 确定性 bootstrap | pnpm-lock 夹具下安装命令经 port 被 spawn（cwd = worktree）；未知管理器跳过且注记入 `warnings`；bootstrap 失败不回滚（建档与 worktree 保留）且警告呈现 |
| AC-6 | 双 root run 组合 | worktree change 发起 run：会话 / 槽位 / StepRecord 落 workspace root 库（compose 注入实例，worktree 路径不进 `for_root`）；executor 会话 cwd = worktree；git diff / static-check / test-execution 执行目录 = worktree；diff 面不含主仓无关改动 |
| AC-7 | 复合键并行 | 同 workspace 同 change 二次发起显式 `Err`；异 workspace 同名 change 互不误拒；同 workspace 两 change 并行 run（假引擎）各自 worktree 走通 |
| AC-8 | legacy 回归 | `worktree=None` 存量建档照旧主 root 跑通（假引擎全相位）；list / detail / 归档既有语义零变化 |
| AC-9 | 读面与 UI 可见性 | worktree change 以 db status 归组入进行中组、详情可达、`read_artifact` 命中 worktree 内产物；detail 出线 worktree 路径（legacy null）——经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕；语料种子含 worktree 非 null 与 `None` 两态建档样本（golden 覆盖两投影）；create 对话框行内呈现 worktree 路径与警告清单 |
| AC-10 | 归档引导与版本交付 | 未 merge（主仓 active / archive 两树未命中）archive 显式拒绝并引导 merge；merge 后既有归档双写零改动（status 翻转 + 主仓目录改名 + 日期前缀）；`packages/desktop` version 0.4.14；`plugins/dev-team` 零改动；`vp test` / `client:check` / knip 全绿 |
| AC-11 | crate 图与 vcs 边界落位 | `crates/infra/vcs`（裸名 `vcs-runtime`）在案且 workspace 内依赖仅 `workflow` + `foundation`、零 Tauri（由 desktop-app 装配注入）；`store` 的 workspace 内依赖为 `agent` + `foundation` 且身份段派生实现仅 foundation 一份（db 文件名同名回归见 AC-1）；core 各 crate 依赖无 `store` 不变；七类边界分类学与租户归位表 vcs 行留痕 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 构建成本：fresh worktree 依赖 / 构建产物全量重建（本仓 Windows cargo 全量重编可观） | 首 run 前等待长，用户体验下降 | 高 | V1 显式接受并留痕；bootstrap 收敛等待到 create 时一次；共享 `CARGO_TARGET_DIR` 等逃生口文档化不内建机制 |
| R2 基线纪律：创建基线 = HEAD，主仓未提交状态对新 change 不可见（含 staged 已归档 change） | 用户以为已带入、实际 worktree 缺失 | 中 | create 脏仓警告文案引导「先提交再开新 change」；「完成即归档即提交」升格为文档化纪律 |
| R3 并行 merge 冲突：两 change 改同一文件各自绿、merge 撞 | 收口阶段需手动解冲突 | 中 | 隔离固有代价，走 git 原生冲突处理；边界留痕 |
| R4 孤儿 worktree：create 中途崩溃遗留半成品 worktree / branch | 磁盘残留与后续同名冲突 | 中 | 前置校验已拒（branch / 目录已存在 → 显式报错指引）；`git worktree list` 对账 + 手动清理路径文档化，不内建自动清理 |
| R5 config / AGENT.md 冻结：worktree 内配置停在创建时刻，主仓中途改动不传导 | 长命 change 用旧配置跑检查 | 中 | 边界留痕 + 文档化；用户可手动 merge 主仓进分支或重建 change |
| native_model 升级破坏存量库 | 存量 workspace 库不可读（高危） | 低 | decode-only 先例模式（provider context_length v1→v2）+ 存量库升级回归测试为 AC-1 |
| `compose_turn` 拆参传播面失控（exec / change_flow 双消费方） | 编译面连锁、语义漂移 | 低 | 两消费方均命令层薄包装；注入形态经 design 定稿；AC-6 断言 store 身份 |
| worktree 路径长度（Windows MAX_PATH：`~/.dev-team/worktrees/{段}/{change}/packages/...` 深路径） | 深仓 checkout / 构建路径超限 | 低 | 可读段截断上限既有（`READABLE_SEGMENT_MAX_CHARS`）；`core.longpaths` 逃生口文档化 |
| 跨盘 worktree（home C: / repo D:）构建 IO 略慢 | bootstrap / 首构建变慢 | 低 | 探索已核实 git worktree 跨盘合法；接受（用户 D1 拍板 home 落位） |
| wire contract 冻结被破坏（`CreateOutcome` / detail DTO 字段演进） | golden 静默漂移 | 低 | 冻结契约显式重写流程逐字执行（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕 + AC-9） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| worktree 落位 | `~/.dev-team/worktrees/{身份段}/<change-name>`；身份段与 workspace db 文件名同源（`workspace_db_file_name` 同算法，上提 foundation 单点） | D1 用户拍板 + 探索细化：同根恒同名、跨重启可复现、异根必不同名；多 workspace 并存不撞 | repo 内隐藏目录（污染主仓 git status）；无 workspace 段平铺（跨 workspace 同名 change 相撞） |
| 分支与基线 | branch `change/<name>`，基线恒主仓 HEAD，`base_commit` 落库 | fork 点可追溯（调试 / UI 价值）；脏改动不暗中带入（基线语义纯净） | 基线携带未提交改动（stash/apply，改变基线语义且易炸） |
| `compose_turn` 拆参 | B 案：命令层预解析 workspace root 库实例注入；cwd 经既有 `turn.root → SessionCtx` 通道零改动 | 组合根已在命令层 `for_root` 一次（缓存命中无害）；唯一重构点收敛为 store 半边签名；resume 链零改动 | A 案签名 store_root / cwd_root 双参（传播面更大、cwd 本就不经 compose） |
| 脏仓行为与警告载体 | 警告但不阻止，基线仍 HEAD；警告经 `CreateOutcome.warnings` 持久入 DTO（golden 重写） | D2 拍板 + 遗留题 3 收敛：警告须抵达前端行内呈现且可回看，toast 易失 | 阻止创建（打断流程）；仅 toast 不入 DTO（丢失持久性） |
| bootstrap | create 时确定性探测 lockfile → spawn 安装；失败不回滚、警告立即呈现；未知管理器跳过注记 | D3 拍板 + 遗留题 2 收敛：static-check / test-execution 无 agent 救场，首跑大声失败兜底；回滚补偿链过长且丢现场 | 首次检查步前懒引导（时序复杂）；失败回滚 create |
| 归档时序 | merge-first（A 案）：用户 merge → 主仓出现目录 → 既有 archive 零改动；未 merge 归档显式引导 | D4 用户拍板；桌面不引入 merge 冲突处理面 | 桌面内自动 merge（V1 超范围） |
| 清理与弃置 | V1 手动（`git worktree remove` + `branch -d`）；db 记录保留；桌面 cleanup 按钮留后续 | D5 拍板：与「手动提交」同哲学 | V1 内建清理命令（收益低、补偿语义复杂） |
| 存量 change | `worktree` 字段 nullable；None 记录照旧主 root 执行（legacy 路径）；零迁移 | D6 拍板 | 存量迁移脚本（风险大无收益） |
| 手动会话与探索笔记 | `agent_start` 会话与 `openspec/explores/` 笔记留主仓 root，不混入 change worktree | D7 拍板：exec / explore 轨道与 change 域工作面分离 | 手动会话可选挂 worktree（无真实诉求） |
| list / detail 读面 | db status 为权威归组；主仓两树未命中的 db 条目不入未知组、工件与产物经 worktree 路径解析（`locate_change` 增回退） | 遗留题 4 收敛：worktree change 在 merge 前主仓无目录，磁盘归组会使其从清单消失 | 磁盘位置优先归组（worktree change 不可见，不可接受） |
| spawn 落位 | 新 vcs 边界：port 留 core/workflow（消费者归属），进程执行落 `crates/infra/vcs`（裸名 `vcs-runtime`）；身份段单点上提 foundation，store 依赖白名单随之增 foundation（「禁 core 行为、准 core 纯类型 / 纯函数」第二例修订） | git 面随本变成族（worktree / status / bootstrap），分类学「先声明边界」纪律；foundation「第二消费者出现才下沉」正触发；git_diff 迁入留后续 | 寄居 `infra/agent`（容忍寄居清单加长）；壳层 spawn（边界纯度与可测性劣） |
| `begin_run` 键 | `(workspace root, change)` 复合，随本变更一并修 | 遗留先例 bug（两 workspace 同名假冲突）；worktree 解锁真并行后必现 | 维持 change 单键（已知 bug 留存） |
| report_dir 绝对路径进 db 文本 | 接受失效（worktree 删除后仅展示面失效，属转录历史） | 相对化牵动 checks 报告树语义与历史行解释，收益低 | 相对化（另立变更） |
| git 缺失 / 非 git 仓 | create 显式 `Err` 引导（git worktree 是硬依赖），MUST NOT 静默回退主 root 创建 | 遗留题 6 收敛：静默回退改变编辑落点语义，不可预期 | 回退 legacy 主 root 创建（语义漂移，边界留痕留后续） |

### 待决问题

- WorktreePort 形态与写面 `create` 签名演进：port 方法粒度（add / 脏仓探测 / bootstrap 三方法 vs 建域单方法）、`Layout` 入参在主仓根 / worktree 根双根下的形态（design 定稿）。
- bootstrap 映射表具体值（lockfile → 安装命令、超时上限、环境变量传递面）与 Cargo 无 lock 场景、共享 target 逃生口的文档化位置（design 定稿）。
- create 补偿链细则：worktree add 成功后目录树 / explore.md 写出失败的回收边界（倾向回收：worktree remove + 建档补偿删除；bootstrap 段失败明确不回收）——design 定稿。
- 非 git 仓 workspace 是否提供 legacy 主 root 创建逃生口（V1 显式拒绝；真实诉求出现另立变更）。
- worktree 被用户手动删除后带记录 change 的读面呈现（detail 工件发现 miss → 产物清单空 + 状态面仍在；是否提示 worktree 缺失）——design 定稿呈现形态。
- detail / `CreateOutcome` 字段演进的 golden 重写范围清单与人工确认记录（随实现留痕）。
