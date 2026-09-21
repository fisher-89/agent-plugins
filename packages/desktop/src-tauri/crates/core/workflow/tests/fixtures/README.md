# fixtures 语料说明

本目录是 desktop 解析器的入仓只读快照语料（golden 回归的输入），与实盘
`openspec/` 数据解耦。清单与来源：

| fixture     | 来源                                                                                      | 覆盖点                                                                          |
| ----------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `v0-a`      | archive 快照 `2025-06-18-use-fast-glob`（全量拷贝）                                       | 无 workflow.json + 遗留 eval.json 干扰 + markdown 四件套 + reports/             |
| `v0-b`      | archive 快照 `2026-05-18-pge-workflow-architecture`（全量拷贝）                           | proposal/design/tasks/specs 纯文档形态                                          |
| `v1-a`      | archive 快照 `2026-07-06-backtrack-reason-propagation`（全量拷贝）                        | 最小 workflow.json（仅 workflow_type，无 created / eval）+ 遗留 eval.json       |
| `v1-b`      | archive 快照 `2026-09-17-workflow-file-inventory`（剔除 reports/ 构建产物）               | eval 含 backtrack_to / backtrack_reason、stale、skipped                         |
| `v1-c`      | archive 快照 `2026-09-18-move-files-write-into-workflow-module`（剔除 reports/ 构建产物） | eval + legacy `files{}` 桶与 `source` 未知键                                    |
| `v2-a`      | active change 快照（workflow.json + 顶层 markdown + specs/）                              | file_log 键存在的临界形态                                                       |
| `v2-b`      | 合成样本（非 archive 快照）                                                               | 全量 v2：file_log 三种 op、active_phase、interrupted、eval 含 backtrack         |
| `corrupt-*` | 合成样本 × 5                                                                              | 整体非法 JSON / 单条 eval 损坏 / 单条 file_log 损坏 / 非法 verdict / 非法时间戳 |

`v1-b` / `v1-c` 的 `reports/` 目录仅含测试报告构建产物（JSON / HTML，合计约 9MB），
对解析与投影无信息量，入仓时剔除以控制仓库体积；其余文件为逐字节快照。

`layout-*` 合成 workspace 树（无日期前缀 archive 目录、空目录树、目录树混入普通文件）
不落盘于本目录——其内部结构会再现 `openspec/changes/**/workflow.json` 形态，
与仓库写入保护冲突；改由 `corpus_golden_test.rs` 在临时目录中确定性搭建，
投影同样进入 golden 对比。
