# 验收留痕: archi-file-tree-lazy-load

> **变更**: archi-file-tree-lazy-load（架构页 · 命令不冻结 + 文件树不参与首屏）
> **日期**: 2026-11-20
> **性质**: 手写验收留痕，机器不消费——不进编译 / 测试 / 生成物 / 契约资产 / 门禁输入
> **承接**: proposal AC-11（R-13 改写后的形态）与 explore §4.F 六项观测
> **口径**: 「不冻结」的机械判据是 AC-2 的首次 poll 必为 `Pending`（测试相位落 `assert_offloaded`），**不是本文件的六项真机观测**；时长不在承诺面（R-10），真机观测只作证据补强

---

## 1 · 验收清单

环境基准：16 逻辑核 / 63,023 个 git 跟踪文件的工作区（explore §2 实测口径），前端命令面 `packages/desktop`。

| # | 观测项 | 观测手段 | 预期 | 判据来源 |
|---|---|---|---|---|
| ① | 进页不冻结（release） | `pnpm build:prod` 起应用，进入 `/architecture`；观察期（图 → 漂移 → 元素树三条取数全程）按住标题栏拖动窗口、切页、滚动左栏 | 窗口始终可拖动 / 可切页 / 不白屏；左栏在取数期显示「解析中…」态而非空白 | proposal AC-11 第 1 条 ①；explore §4.F 第 1 项；机制面 = `desktop-app-shell`「命令执行线程纪律（批量子系统 IO）」（四条取数命令 `pub async fn` + `spawn_blocking`，命令体不在 IPC 线程内联） |
| ② | 切「文件」同样不冻结 | 同一 release 构建下，从缺省「元素」视图点「文件」；约 3 s 全仓 walk 期间重复拖动 / 切页 | 窗口可交互；文件树落位后条目投影与标记五值照常 | proposal AC-11 第 1 条 ②；explore §4.F 第 2 项；`desktop-archi-file-tree`「缺省元素树 + 点了才取」 |
| ③ | 三命令起止时刻 | 在命令层 / 前端 invoke 包装处临时计时（`archi_graph` / `archi_drift` / `archi_file_tree`），记每次进入页面的起止时刻 | `archi_drift` 仍为最大项（约 16 s IO + 42k 文件解析），`archi_file_tree` 次之（约 3.1 s / 59,771 节点 / 约 10.4 MB），`archi_graph` 毫秒级；且首屏 `archi_file_tree` 调用数为 0 | proposal AC-11 第 1 条 ③；explore §2 实测表；AC-5（进页 `archi_file_tree` 调用数 = 0，点「文件」后恰 1 次） |
| ④ | debug 对照 | `pnpm build`（debug）重复 ①②③ 一次 | debug 数字显著劣于 release，仅作「debug 数字不可当基线」的实证，不参与任何通过判定 | proposal AC-11 第 1 条 ④；explore §4.F 第 4 项 |
| ⑤ | 连点刷新基线 | release 下连点刷新钮 N 次（建议 N = 5），记现象（窗口是否仍可拖动）与总耗时 / 内存 / IO 抖动 | 窗口仍不冻结，但每次重挂各起一条全仓扫描（`refreshTick` 变 ⇒ `<ArchitectureCanvas key>` 整块重挂 ⇒ 三 hook 全重取），page cache 被反复冲刷 ⇒ 总耗时劣于单次 | proposal R-1（零代码护栏：无取消 / 无去重 / 无节流）；explore §5 R1 与「R1 的连带影响」；`desktop-archi-drift`「并发扫描（已知限制）」 |
| ⑥ | 冷 / 热缓存各一次 | 重启应用 / 清 page cache 后测一次（冷），随即再测一次（热），两次数值并列记录 | 冷启动走约 16 s IO + 3.1 s walk；热缓存下 IO 段降一个量级（page cache 命中），量级差即预期现象 | proposal AC-11 第 1 条 ⑥；explore §2 / §4.F 末段 |

**读法约定（避免误判）**：⑤ 见内存 / IO 抖动时先查是否连点了刷新，MUST NOT 误判成 async 化回归（explore §5 R1）；① ② 的「不冻结」与 ③ 的时长是两个正交面——时长未改善属 R-10 的范围外项，不构成本变更的失败。

---

## 2 · 环境不具说明

本 change 的验收面**不具备**履行上表六项的观测条件，故六项真机观测在本工作流相位不可履行：

| 缺口 | 事实 |
|---|---|
| 无 release 产物 | 机上有 63k 文件工作区（`/d/Projects/wpsweb/client/app`），但无 `pnpm build:prod` 产出的 release 构建 ⇒ ①②④⑤⑥ 的载体不存在 |
| 无 GUI 交互读取面 | 「窗口可拖动 / 可切页 / 不白屏」「按住标题栏拖动」属人工 GUI 交互，不落任何 agent 可读产物 |
| 无时刻读数面 | ③ 的三命令起止时刻与 ⑤ 的连点耗时需在运行中的应用内读；当前无计时埋点、无运行期日志产物，时刻读数无 agent 可读面 |

结论：六项观测写成 agent 相位门禁必败，且重做同因再败（不是偶发失败，是判据不可履行）。故本文件只做「清单落盘 + 环境不具说明 + 承接安排」，真机观测本身**不作门禁判据 / 不作归档阻塞项 / 不在 `tasks.md` 登记为待办**。先例：`desktop-agent-execution`「SDK 租户 MVP 边界与已知限制」第 16 条与场景「相邻 assistant 消息不设实现期核验门」（`openspec/specs/desktop-agent-execution/spec.md:1015`、`:1040`）。

「不冻结」的可判半边已由机械判据承接：命令 future 首次 poll 必为 `Pending`（AC-2，`assert_offloaded`，四命令各一条），同步实现该用例即红；缺省视图面由 AC-5 / AC-6 的调用计数与 `role="tab"` 声明序断言承接。时长本非承诺面（R-10）。

---

## 3 · 归档后人工验收承接

上表六项的观测与结论 **SHALL 并入归档后人工验收**：真实大仓库 + release 构建，人工执行 ①–⑥，结论逐项记入本文件末尾（新增「观测记录」小节），回填日期与 release 版本（`packages/desktop/package.json` 的 `version`）。

| 承接事项 | 约定 |
|---|---|
| 执行时机 | 本 change 归档之后（不阻塞归档） |
| 执行主体 | 人工（具备 GUI 与大仓库工作区的一方） |
| 判据 | ① ② 看「窗口可交互」；③ 看「`archi_drift` 最大、`archi_file_tree` 次之、首屏 `archi_file_tree` = 0」；④ 只作对照；⑤ 看「不冻结但并行扫描」；⑥ 看冷 / 热量级差 |
| 不符预期的处置 | **MUST NOT 回退本 change**；按 R-1（并发扫描：取消 / 去重 / 节流）或 R-10（漂移总时长 / 增量触发）的口径**另立变更** |
| 与机械判据的关系 | 人工验收只补强证据；改变「是否不冻结」的机械判据仍是 AC-2 的首次 poll 断言，人工观测不改写该判据 |

<!-- 观测记录（归档后人工验收回填）：待填 -->
