# 语料夹具（desktop-corpus-regression）

历史 workflow.json 目录树语料（v0-a … v3-a 与 corrupt-* 共 13 个）已随 parse
面整体退役删除（desktop-workflow-db-state，design D12 处置定稿 = 删除；旧语
料可考于 git 历史）。desktop 对 workflow.json 双向墙（零读零写）；列表 / 详
情读面为 **db 单源**（零磁盘扫描，磁盘-only 目录零呈现），`workflow.json`
惰性字节样本仅在「零发现反例」语料中按字节零读取保留，不进任何投影。

## db 种子语料矩阵（覆盖面清单）

新语料 = **db 种子构造器**（经 store 真实 change 域操作面：建档 / 开相 / 落
账 / 回跳 / 步骤追加 / run 发起与收口整包 / 启动标定；种子 id 归键——`id`
固定字面量直携，`name` 恒裸名属性）+ **运行时合成磁盘产物树**（沿 layout
fixture 先例）。
覆盖面：

| 维度 | 形态 | 构造路径 |
|------|------|----------|
| 多 attempt | 同 phase 多条 PhaseRecord（fail → retry → pass），attempt 连续递增 | 重复 `log_change_phase`（active_phase 开 / 清交替） |
| backtrack stale | 回跳标记 + 目标最新 pass 置 stale + 下游全条目 stale | `apply_backtrack`（`stale_dependents` 闭包由写面计算） |
| 槽位全缺 | 三会话槽位列恒 None 的 PhaseRecord（缺省落账） | 不携槽位的 `log_change_phase` |
| 文档形态（零发现反例） | 仅磁盘目录 + 产物 markdown，零 db 记录（存量 CLI change）——list 零呈现、`change_detail`（裸名 / 前缀名寻址）恒 None、目录字节零变化；不产出 golden（出局删除） | 只建目录树（含惰性 workflow.json 字节样本），不走建档 |
| 归档前缀 | db archived 裸名 + `archived_at` + 磁盘 `YYYY-MM-DD-` 前缀目录 → detail 全状态面（status=archived、10 站与 runs 全量）与列表归档组（裸名 + id + 月分组） | `create_change_record`（status=Archived 携 archived_at）+ 磁盘前缀目录树 |
| 坏行 | 库内 native_model 解码失败行（坏字节） | db 文件直写字节注入（store_test 裸 redb 注入用例 `建档表坏行直写注入_读侧store_error显式记因不静默` 承载；store 读面 `StoreError` 路径） |
| worktree 两态 | 建档携 `worktree` / `base_commit` 执行锚 + worktree 内磁盘产物树（merge 前主仓两树未命中，`corpus-worktree`）；legacy `worktree=None` 投影 null（既有建档语料全量重写后覆盖） | `create_change_record` 携 Some 两字段 + worktree 树（投影路径经 `<WORKTREE_ROOT>` 占位归一）；detail `worktree` 键恒在场（spec desktop-corpus-regression「各至少一个」） |
| run 全史 | 同 change 四次 run 全史留存（≥2 run，attempt 跨 run 递增不撞号）、二十二值词汇步整包（sessionId / detail 有无两态）、emit seq 稳定序、时间戳 ISO 出线（`corpus-run-history`） | `run_start` / `finish_change_run`（经 store run 域操作面，禁裸表插桩；步序列经 orchestration `finish_command` 全词汇喂入——落库过滤单点同真实写路径，unify-run-state-persistence） |
| 归档相位 run 史（slim-flow-aux-steps） | archive 列空 checklist 落账 pass（report 空串 + 空 checklist）；archive 相位**真实相位形态**九步节点整包（9 归档步词，git 步分类可辨；零 `phase_start` / `phase_log`）+ 8 条串行 forward 边 | `log_change_phase`（archive 空 checklist）+ `run_start` / `finish_change_run`（archive 九步 + 8 forward 边，`corpus-archive-prefix`） |
| run 中断标定 | run 发起后中途死亡残留（running 行 + active_phase 悬挂）→ 标定翻 interrupted（记因附中断语境、finished_at 标定时刻）+ active_phase 清位（`corpus-run-interrupted`） | `run_start` + `start_change_phase`（开相不落账）→ `calibrate_interrupted_runs` 直调（open_workspace 内嵌同路径，D12） |
| initialize 相位 run 史（slim-flow-aux-steps） | initialize 相位**真实相位形态**三节点整包（`clean` / `static_check` / `test_execution`，零 `phase_start` / `phase_log`）+ 2 条串行 forward 边；机械 checklist PhaseRecord 与 legacy skipped 条目照旧 | `run_start` / `finish_change_run`（三节点 + 2 forward 边，`corpus-initialize`） |
| 步级时间与 token（desktop-step-running-snapshot） | run 步节点四字段（started_at / ended_at / token_input / token_output）定值注入两态：时间（正常起止双值步 / 瞬步 `started == ended` 同槽定值 / running 收口在途残留 `ended = None`）+ token（WorkerAgent 步带增量两值 / 工具步 None）；毫秒数口径出线（`corpus-run-history` run-3 stopped 收口样本，run-1 / run-2 既有步为默认派生形态；瞬步同刻形态由 run-4 的存量解码样本 `verdict_gate` 承载——新写面零瞬步生产例） | `step` / `step_at` fixture 默认派生定值（`t(n)` 种子时间族基准槽位，零 wall-clock——直构 `ChangeStepState`，不经 `emit_node` / `now_millis`）+ `step_metrics` overrides 两态样本 → `run_start` / `finish_change_run`（经 store run 域操作面，禁裸表插桩） |

golden 快照纪律与显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留
痕）延续：detail / list golden 全量含 `id` 键（身份键入——desktop-change-db-
identity D13 显式重写范围），列表 golden db 单源（磁盘-only 条目出局）、归档
条目裸名 + `archived_at` 月分组；归 test-design / test-gen / test-execution
阶段承接落地。`golden/layout-*.json` 三份 layout golden 与
layout_queries_test 同期保留。

## 解码专用词汇 vs 真实相位形态（slim-flow-aux-steps）

辅助步（`PhaseStart` / `VerdictGate` / `PhaseLog`）与决策门（`RetryGate` /
`WhitelistGate`）退场后，计划图不再产出这五类节点——开相归 `enter_phase`、
verdict 裁决归 evaluator 步内、落账归 `settle_phase`。**词面仍然冻结**（22 值
MUST NOT 删减重排：旧库行经 bincode / camelCase 出线解码必需），因此语料集内
两类样本**并列存在、互不替换**：

- **真实相位形态样本**：initialize 三节点（`clean` / `static_check` /
  `test_execution`）、archive 九节点（`preflight` … `finalize`）、implement /
  dev-design 的真实步序列。这些样本内**零** `phase_start` / `phase_log` /
  `verdict_gate` / `retry_gate` / `whitelist_gate`，且无同刻（`startedAt ==
  endedAt`）节点。
- **存量解码形态样本**：`corpus-run-history` 的 run-4 attempt 专门承载五词各
  至少一个节点（含 `verdict_gate` 的瞬步同刻形态与 `phase_start` 的工具步 token
  null 形态）。它存在的作用是「旧库行仍可解码」的回归面；MUST NOT 因新写面零
  产出而删除这些样本（删任一侧即破坏双口径判据）。
