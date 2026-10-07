# desktop change worktree 探索（2026-10-06）

诉求原文：desktop 创建变更时分配同名 worktree，变更所有修改在 worktree 内执行，
完成后等待手动归档和提交。

核心论断：这个方案的真实主题不是「加 worktree」，而是把一个字符串 root 撕成
两个身份——**workspace root（store 身份锚）** 与 **exec root（文件系统执行锚）**。
worktree 只是 exec root 的具体形态。

## 现状实证（2026-10-06 代码态）

单一 `root: String`（前端每命令显式传入，无「当前 workspace」概念）承担四个身份：

| 身份 | 消费点 | 引证 |
| --- | --- | --- |
| ① store 身份 | `compose_turn` 内部 `stores.for_root(root)` → canonical 路径 hash → **每 root 一个独立 redb** | `infra/agent/src/compose.rs:52-53`、`infra/store/src/store.rs:1494-1508` |
| ② 会话 cwd | CLI 引擎 `command.current_dir(ctx.workspace_root)`；SDK 引擎 `LoopTurn{cwd}` → AGENT.md 前言 / 工具执行 / **沙箱前缀校验（编辑囚于 root 内）** | `infra/agent/src/cli/runner.rs:190-195`、`sdk/runner.rs:184-192`、`sdk/sandbox.rs:62-81` |
| ③ 工件根 | `Layout::resolve(root)` → `openspec/changes/<n>/`；`change_test_reports(root, change)` | `core/foundation/src/layout/mod.rs:54-80` |
| ④ 检查器 cwd | static_check `.current_dir(root)` + `config::load(root)`；test-exec `<root>/<suite.root>/<suite.cwd>` spawn；git_diff `git status --porcelain + diff HEAD` @ root | `infra/checks/src/static_check.rs:23-54`、`testexec/detect.rs:211-231`、`infra/agent/src/git_diff.rs:40-66` |

对方案非常有利的三个发现：

1. **相位状态机零 fs**（desktop-workflow-db-state 遗产）：phase_next/start/log/
   backtrack/decision_log 只吃 `ChangeStateStore`、不知道路径
   （`core/workflow/src/write/phase_next.rs:78-90`、`core/orchestration/src/steps.rs:62-150`）。
   root 撕裂后状态机原地不动。
2. **SDK 沙箱 = 免费的 worktree 围栏**：`sandbox.rs` 前缀校验以 `LoopTurn.cwd`
   为锚，cwd 换 worktree 后 agent 文件编辑自动囚在 worktree 内——「所有修改在
   worktree 内执行」由既有机制背书，非纯约定。
3. **git_diff 在 worktree 内语义变好**：今天 `git diff HEAD` 混入主仓一切无关
   改动；worktree 内 diff 恰为本 change 编辑集，executor/evaluator prompt 上下文
   信噪比提升。

三个硬点：

1. `compose_turn(stores, registry, &root, None)` 内部做 `for_root(root)`——直接
   喂 worktree 路径会铸出**空库**（provider/会话/建档全消失）。撕裂第一刀在此：
   拆成 store 用 workspace root 解析、cwd 用 exec root。
2. `begin_run` 只按 change 名做键、不含 root
   （`core/orchestration/src/control.rs:40,52-58`）：两 workspace 同名 change 假
   冲突（先例 bug）；worktree 解锁并行后会浮出。
3. 全新 worktree 无 `node_modules` / `target/`：static-check / test-exec 是确定性
   步骤、无 agent 救场，首跑直接失败 → 需确定性 bootstrap。

## 目标形态：双 root

```
workspace root（身份锚，用户注册的）        exec root（执行锚 = worktree）
┌──────────────────────────────────┐      ┌────────────────────────────────┐
│ workspace redb                    │      │ git worktree  branch:          │
│  ├ change records（+worktree 字段）│◀──记录│  change/<name>  base=HEAD      │
│  ├ 相位状态机（零改动）             │      │  ├ openspec/changes/<name>/    │
│  ├ sessions / providers           │      │  │  explore/proposal/reports   │
│  └ 探索笔记 explores/（不进 wt）    │      │  ├ 代码编辑（沙箱囚于内）        │
└──────────────────────────────────┘      │  └ openspec/config.json@创建时  │
                                          └────────────────────────────────┘
run 组合：store = for_root(workspace)；cwd / Layout / diff / checks = worktree
```

改动收敛度：walker 的 `RunRequest.root`、`ToolStepRequest.root`、`StoreSnapshot`
的 fs 半边天然指向 exec root；`LocalToolSteps` 早已分离「fs root 每请求」与
「store 构造时注入」（`steps.rs:12-37`）——唯一深度重构点是 `compose_turn` 拆参。

会话 resume 的 cwd 一律由当次 turn 的 root 重推导、记录不存 cwd
（`infra/agent/src/compose.rs:93-112`、`store_port.rs:144-157`）——命令层解析
exec root 即可，resume 链零改动。

## 生命周期

```
create ─── run（多相位）─── 收口 ───────── 用户手动段 ─────── archive
  │           │               │                                 │
worktree    全部编辑落      "All phases passed.        merge 后主仓出现   rename+db 翻转
add +       worktree；      Ready for archiving."     changes/<n>/       （现有逻辑零改）
branch      diff/检查       （停等，现状语义不动）     → 桌面 archive 可用  → wt/branch 手动清
change/<n>  都在 worktree
```

## 决策记录（2026-10-06 用户拍板）

| # | 决策 | 结论 |
| --- | --- | --- |
| D1 | worktree 位置 | **`%HOME%/.dev-team/worktrees`**（= 既有 data root `~/.dev-team` 下 `worktrees/` 子树，用户定） |
| D2 | 脏主仓时创建行为 | 警告但不阻止，基线仍取 HEAD；「先提交再开新 change」写进文案（倾向定案） |
| D3 | 依赖引导 | create 时确定性 bootstrap（探测 lockfile → `pnpm install` 等），spawn 归 infra 合规（倾向定案） |
| D4 | 归档时序 | **A 案 merge-first**：用户 merge → 主仓出现目录 → 现有 archive 零改动（用户定） |
| D5 | worktree/branch 清理 | V1 手动（`git worktree remove` + `branch -d`），与「手动提交」同哲学；后续可加提示按钮（倾向定案） |
| D6 | 存量无 worktree 的 change | db 记录 worktree 字段 nullable → 旧 change 走主 root legacy 路径，不做迁移（倾向定案） |
| D7 | `agent_start` 手动会话 | 默认仍主 root，不混入 change worktree（倾向定案） |

### D1 细化（探索补充，提案阶段确认）

- **per-workspace 子目录**：多 workspace 并存，worktrees 子树需按 workspace 分目录。
  镜像既有 db 文件名身份派生单点 `{可读段}-{sha256 前 16 字节 32 位 hex}`
  （`store.rs:278-293` `workspace_db_file_name`）：worktree 子目录取
  `~/.dev-team/worktrees/{可读段}-{hash}/<change-name>`（去 `.redb` 后缀）。
  同根恒同名、跨重启可复现、异根必不同名，身份派生与 db 文件同源。
- **跨盘注记**：home（C:）与 repo（D:）跨盘对 git worktree 合法（目录 + gitdir
  指针），仅构建 IO 略慢——接受。
- **可见性补偿**：路径藏于 home 下，review/commit 需可达——create 产出与
  change detail 应向 UI 暴露 worktree 绝对路径。
- **路径长度**：`~/.dev-team/worktrees/{readable}-{hash}/<change>/packages/...`
  前缀可控（readable 段截断于 `READABLE_SEGMENT_MAX_CHARS`），深路径场景
  `core.longpaths` 逃生口。

### D4 细化（探索补充，提案阶段确认）

- archive 对 worktree change 的引导：主仓 active/archive 两树均未命中且记录带
  worktree 字段 → 错误文案引导「先 merge worktree 分支回主仓再归档」，优于
  现有泛化「目录未找到」。
- merge 不被强制：用户可不 merge 直接弃置 change（V1 弃置 = 手动清
  worktree/branch/db 记录，文档化；桌面 cleanup 按钮留后）。

## 风险与约束

- **R1 构建成本**：fresh worktree cargo 全量重编（本仓体量在 Windows 可观）；
  共享 `CARGO_TARGET_DIR` 引入并行 build 串行锁。V1 先接受成本、留配置逃生口。
- **R2 基线纪律**：创建基线 = HEAD，主仓未提交状态对新 change 不可见（含
  staged 已归档 change）。实例：desktop-workflow-db-state 曾 staged 未提交半日
  ——「完成即归档即提交」需从习惯升格为纪律（2026-10-06 已归档提交，主仓干净）。
- **R3 并行 merge 冲突**：两 change 改同一文件各自绿、merge 撞——隔离固有代价，
  手动解。
- **R4 孤儿 worktree**：create 中途崩溃 → `git worktree list` 对账恢复（幂等
  采纳或清理）。
- **R5 config/AGENT.md 冻结**：worktree 内配置停在创建时刻，主仓中途改动不
  传导（文档化）。
- **golden wire contract 冻结约束**（2026-10-03 起字段演进走 golden 显式重写）：
  `CreateOutcome` 增 worktree 路径（及 base commit，若采纳）属字段演进——golden
  fixtures 显式重写流程，形态仍冻结。
- **begin_run 键修正**：`(root, change)` 复合键，随本 change 一并修（worktree
  解锁并行后必现）。
- **ChangeStateRecord 升级**：`worktree: Option<String>`（nullable，D6 legacy
  语义），走 native_model 版本升级先例（provider context_length v1→v2 decode-only
  同模式）；可连带记录 `base_commit`（fork 点，调试/UI 价值）。

## 补充问答：db 是否受 worktree 影响（2026-10-06）

结论：**不受影响——这是双 root 设计要守住的核心不变量**。机制四条：

1. db 身份锚定主 workspace root（`for_root` 派生不动；worktree 路径只作
   exec root 消费、从不进 `for_root`，`compose_turn` 拆参即为此）。
2. 相位状态机零 fs，worktree 存在与否状态机不知道。
3. `SessionRecord` 无 cwd 字段，resume 的 cwd 从当次 turn 重推导——会话在
   worktree 跑、记录在主 db 存，互不牵挂。
4. file_log 相对路径在 merge 后语义自洽。
5. db 在 `~/.dev-team/workspaces/`、worktree 在 `~/.dev-team/worktrees/`，
   同 data root 下相邻子树、零耦合；git 树永不感知 db。

接触点四项：① schema 升级一次（ChangeStateRecord + worktree 字段，
native_model v1→v2 decode-only 先例）；② 并行压力（`Arc<Store>` + native_db
进程内并发本就支持，begin_run 复合键修正已列）；③ **report_dir 绝对路径进
db 文本**：`walker.rs:1136-1145` 把 `change_test_reports(worktree_root, change)`
的绝对路径拼进 phase_log report 文本落库，worktree 删除后路径失效——提案时
定夺相对化或接受失效（仅展示面非结构字段；test-execution 修复 prompt 内的
report_dir 同理，属转录历史可接受）；④ create 补偿链变长（db → worktree →
目录树三段补偿，遗留题 5 已含）。

## 提案阶段遗留设计题

1. `compose_turn` 拆参两案：签名加 store_root/cwd_root 双参 vs 命令层预解析
   `Arc<Store>` 注入（命令层已 for_root 一次，缓存命中无害）。
2. bootstrap 失败语义：不回滚 create、警告面立即呈现（static-check 后续仍会
  大声失败）；探测面 = pnpm-lock / package-lock / yarn.lock / Cargo.toml 已知
  管理器，未知跳过并注记。
3. 脏仓警告载体：CreateOutcome 警告字段（golden 重写）vs 仅 UI toast 不入 DTO。
4. list 读面 source 推导：`db_entry` 按磁盘位置归组，worktree change 两树均
   未命中的回退行为待设计确认（预期：db status=Active 即 Active，工件读取经
   worktree 路径解析）。
5. worktree 冲突前置校验：branch `change/<name>` 已存在 / worktree 目录已存在
   的拒绝面（对齐 create 现有三道校验前置风格）。
6. git 可用性前置：git_diff 是降级面，worktree add 是硬依赖——create 需 git
   存在性检查 + 明确报错（PATH 探测已有 bash.rs 先例）。