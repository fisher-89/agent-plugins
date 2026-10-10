# 语料夹具（desktop-corpus-regression）

历史 workflow.json 目录树语料（v0-a … v3-a 与 corrupt-* 共 13 个）已随 parse
面整体退役删除（desktop-workflow-db-state，design D12 处置定稿 = 删除；旧语
料可考于 git 历史）。desktop 对 workflow.json 双向墙（零读零写），语料失去解
析主体；`workflow.json` 惰性字节样本仅在 db 种子构造时按「目录存在即 change、
字节零读取」的发现语义少量保留，不进任何投影。

## db 种子语料矩阵（覆盖面清单）

新语料 = **db 种子构造器**（经 store 真实 change 域操作面：建档 / 开相 / 落
账 / 回跳 / 步骤追加 / run 发起与收口整包 / 启动标定）+ **运行时合成磁盘产
物树**（沿 layout fixture 先例）。
覆盖面：

| 维度 | 形态 | 构造路径 |
|------|------|----------|
| 多 attempt | 同 phase 多条 PhaseRecord（fail → retry → pass），attempt 连续递增 | 重复 `log_change_phase`（active_phase 开 / 清交替） |
| backtrack stale | 回跳标记 + 目标最新 pass 置 stale + 下游全条目 stale | `apply_backtrack`（`stale_dependents` 闭包由写面计算） |
| 槽位全缺 | 三会话槽位列恒 None 的 PhaseRecord（缺省落账） | 不携槽位的 `log_change_phase` |
| 文档形态 | 仅磁盘目录 + 产物 markdown，零 db 记录（存量 CLI change） | 只建目录树（含惰性 workflow.json 字节样本），不走建档 |
| 坏行 | 库内 native_model 解码失败行（坏字节） | db 文件直写字节注入（store_test 裸 redb 注入用例 `建档表坏行直写注入_读侧store_error显式记因不静默` 承载；store 读面 `StoreError` 路径） |
| worktree 两态 | 建档携 `worktree` / `base_commit` 执行锚 + worktree 内磁盘产物树（merge 前主仓两树未命中，`corpus-worktree`）；legacy `worktree=None` 投影 null（既有建档语料全量重写后覆盖） | `create_change_record` 携 Some 两字段 + worktree 树（投影路径经 `<WORKTREE_ROOT>` 占位归一）；detail `worktree` 键恒在场（spec desktop-corpus-regression「各至少一个」） |
| run 全史 | 同 change 两次 run 全史留存（≥2 run，attempt 跨 run 递增不撞号）、五落词汇步整包（sessionId / detail 有无两态）、emit seq 稳定序、时间戳 ISO 出线（`corpus-run-history`） | `run_start` / `finish_change_run`（经 store run 域操作面，禁裸表插桩；步序列经 orchestration `finish_command` 全词汇喂入——落库过滤单点同真实写路径，unify-run-state-persistence） |
| run 中断标定 | run 发起后中途死亡残留（running 行 + active_phase 悬挂）→ 标定翻 interrupted（记因附中断语境、finished_at 标定时刻）+ active_phase 清位（`corpus-run-interrupted`） | `run_start` + `start_change_phase`（开相不落账）→ `calibrate_interrupted_runs` 直调（open_workspace 内嵌同路径，D12） |

golden 快照纪律与显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留
痕）延续，归 test-design / test-gen 阶段承接落地；`golden/layout-*.json` 三
份 layout golden 与 layout_queries_test 同期保留。
