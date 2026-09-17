# workflow-file-inventory Specification

## Purpose

管理 change 级文件操作清单（`workflow.json` 的 `files` 净状态）：PostToolUse 记录器被动归账工具事件，经净状态折叠后作为变更文件范围的唯一权威来源，供突变测试 scope、测试路径解析、架构交叉核对与 evaluator 对账消费。

## Requirements

### Requirement: workflow.json 持有文件清单净状态

`workflow.json` SHALL 含可选字段 `files`，形状为 `{ written: string[], deleted: string[] }`——两个路径列表，路径为相对项目根的 POSIX 风格路径（条目可选携带来源审计字段 `agent_type`，见「清单条目来源审计」要求；承载形态——flat 字符串数组＋旁挂来源映射或条目内嵌对象——由 design 定，消费方契约仅依赖路径）；目录删除 SHALL 以目录路径本身记录，消费方按路径前缀匹配。`written` 桶内 SHALL NOT 再按 add/modify 细分（该区分由 design 变更清单承载）。

`workflowFileSchema` SHALL 扩展校验该字段；未知键保留策略不变。消费方（突变测试 scope、`test_resolve_paths` 清单模式、evaluator 对账）读到缺失 `files` 字段的 change 时 SHALL 硬报错（文案说明"该 change 创建于文件清单机制之前，请重建"），MUST NOT 回退 git diff。

#### Scenario: 清单字段通过 schema 校验

- **WHEN** 解析含 `files: { written: ["src/a.ts"], deleted: ["src/old.ts"] }` 的 `workflow.json`
- **THEN** `workflowFileSchema` 校验通过，`workflow_type` / `created` / `eval` / 未知键行为不变

#### Scenario: 旧 change 硬报错不回退

- **WHEN** 某消费方读取无 `files` 字段的 `workflow.json`
- **THEN** 返回硬错误，文案含重建指引
- **AND** MUST NOT 回退为 git 工作区 diff

### Requirement: PostToolUse 记录器被动归账文件操作

插件 SHALL 注册 PostToolUse hook 记录器，覆盖工具 `Write`、`Edit`、`StrReplace`（取 `tool_input.file_path`）、`NotebookEdit`（取 `tool_input.notebook_path`）与 `Bash`、`PowerShell`、`Shell`（经 `extractFileOps` 提取）——其中 `StrReplace` 与 `Shell` 为 Cursor 侧工具名，与 `Edit` / `Bash` 同链路归账。Claude 与 Cursor 产物 SHALL 各自注册该记录器（matcher 含各自产物的 `phase_next` MCP 全名）。记录器 SHALL 将每次文件操作折叠为净状态后写入当前 change 的 `workflow.json` 的 `files` 字段，条目来源按「清单条目来源审计」要求随事件 `agent_type` 落盘。

记录器 SHALL 排除自污染路径：`openspec/` 整体（含 `openspec/architecture/**`、reports、proposal/design 等）与 `workflow.json` 自身的写/删操作 MUST NOT 入清单。既有 PreToolUse 写保护行为不受影响。

记录器定位目标 change SHALL 经 session 注册表（见下条要求）；无法解析目标 change 的事件 SHALL 被静默丢弃（不报错、不阻塞工具调用），并留诊断输出通道。

#### Scenario: Write 归账 written

- **WHEN** 当前 session 已绑定 change `my-change`，记录器收到 `tool_name: "Write"`、`tool_input.file_path: "<root>/src/foo.ts"` 的 PostToolUse 事件
- **THEN** `openspec/changes/my-change/workflow.json` 的 `files.written` 含 `src/foo.ts`
- **AND** `files.deleted` 不含 `src/foo.ts`

#### Scenario: openspec 路径被排除

- **WHEN** 记录器收到对 `openspec/changes/my-change/design.md` 或 `openspec/architecture/models/x.c4` 的写事件
- **THEN** 这些路径 MUST NOT 出现在 `files.written`

#### Scenario: 递归删除记录目录前缀

- **WHEN** Bash 事件命令为 `rm -rf src/old-module`
- **THEN** `files.deleted` 含 `src/old-module`（目录路径本身）

#### Scenario: 无绑定事件静默丢弃

- **WHEN** 记录器收到 `session_id` 未注册表命中的写事件
- **THEN** hook 进程以 0 退出且不阻塞该工具调用
- **AND** `workflow.json` 不被改写

#### Scenario: Cursor 工具名事件同链路归账

- **WHEN** 已绑定 session 的 `StrReplace` 事件（`tool_input.file_path`）与 `Shell` 事件（command `echo x > src/a.ts`）被记录器处理
- **THEN** `files.written` 含该 `file_path` 路径与 `src/a.ts`

### Requirement: 清单条目来源审计

PostToolUse 记录器写入清单条目时 SHALL 一并记录事件 stdin 携带的 `agent_type`（subagent 来源标识）：

- subagent 触发的文件操作事件，其条目 SHALL 携带该 subagent 的 `agent_type`（如 `dev-team:implementation-generator`）
- 主会话触发的事件与 `change_files` 工具补录的条目 MUST NOT 携带 subagent 来源（字段省略）；append 已存在路径去重时 SHALL 保留既有来源
- 同路径多次写入时来源 SHALL 取最后写入者（last-writer-wins）；条目被折叠规则移除时其来源随之移除
- 该字段 SHALL 仅作来源审计（排查归账错误、范围超集定位）；突变 scope、`test_resolve_paths`、`archi_check` 与 evaluator 对账 MUST NOT 依赖该字段

`agent_type` 的具体 JSON 承载形态（条目内嵌对象 vs 保持路径数组 flat 的旁挂 provenance 映射）由 design 决议。

#### Scenario: subagent 写入携带来源

- **WHEN** session 已绑定 change，`implementation-generator` subagent 的 Write 事件（stdin 含 `agent_type`）被记录器归账 `src/foo.ts`
- **THEN** `files.written` 中该条目（或其旁挂映射）记录该 `agent_type`

#### Scenario: 主会话写入与手动补录无来源

- **WHEN** 主会话的 Write 事件被归账，或经 `change_files` append `src/manual.ts`
- **THEN** 对应条目不携带 subagent 来源字段

#### Scenario: 同路径重写来源取最后写入者

- **WHEN** `src/a.ts` 先后被 agent_type 为 X 与 Y 的两个 subagent 写入
- **THEN** 该条目来源为 Y

#### Scenario: 消费方不读取来源字段

- **WHEN** 突变 scope / `test_resolve_paths` / `archi_check` / evaluator 对账消费 `files`
- **THEN** 判定结果与条目是否携带 `agent_type` 无关

### Requirement: session 注册表绑定 current-change

系统 SHALL 维护 `session_id → change` 的持久化注册表。主 agent 调用 MCP `phase_next` 时，该调用自身触发的 PostToolUse 事件 SHALL 携带 `stdin.session_id` 与 `tool_input.change`，记录器据此建立/更新绑定；此后同一 `session_id` 下的工具事件（含 subagent 触发的事件）SHALL 查表归账到对应 change。

PostToolUse matcher 对 `phase_next` SHALL 使用 MCP 全名 `mcp__plugin_dev-team_dev-team__phase_next`（裸 server 名匹配不到任何工具）。`phase_next` 的输入协议（含必填 `run_id`）MUST NOT 改变；`run_id` 继续承担 phase-next 的 turn 内 anchor 职责，注册表键控使用 `session_id`，两者仅在同一 MCP 调用事件中交汇。

subagent 事件与主会话共享 `session_id` 是文档强指标但非明文契约：若实现期实测不共享，SHALL 退化为"暂存-认领"模式——记录器按 `session_id` 暂存事件流，由消费命令认领合并。

#### Scenario: phase_next 调用建立绑定

- **WHEN** 主 agent 在 session S 中调用 `phase_next(change: "my-change", run_id: <id>)`
- **THEN** 注册表含 `S → my-change`
- **AND** `phase_next` 的输入 schema 与响应行为不变

#### Scenario: subagent 事件按 session 归账

- **WHEN** session S 已绑定 `my-change`，`implementation-generator` subagent 在同一 session 下 Write `src/bar.ts`
- **THEN** 该操作归账到 `my-change` 的 `files.written`

#### Scenario: 不同 session 不串账

- **WHEN** session S1 绑定 `change-a`，session S2 绑定 `change-b`
- **THEN** S1 的写事件归账 `change-a`，S2 的写事件归账 `change-b`

### Requirement: extractFileOps 三态提取

`plugins/dev-team/bin/src/hooks.ts` SHALL 将 `extractBashWriteTargets` / `extractPowerShellWriteTargets` 泛化为共享提取器 `extractFileOps(command) → Array<{ op: 'write' | 'delete' | 'revert', path }>`，同时供 PreToolUse 写保护与 PostToolUse 记录器消费。

提取分类 SHALL 至少覆盖：

- write：`>`、`>>`、`>|`、`tee`、`Set-Content`、`Out-File`、`Add-Content` 等
- delete：`rm`、`Remove-Item`、`del`、`git rm`
- delete(旧路径) + write(新路径) 双条目：`mv`、`Move-Item`、`ren`
- revert：`git restore`、`git checkout --`；带 `--source=<commit>` 的归 write

python/node 命令豁免与现有 fail-open 语义 SHALL 保持不变。

#### Scenario: 重定向归 write

- **WHEN** `extractFileOps` 收到 `echo x > src/a.ts`
- **THEN** 返回含 `{ op: 'write', path: 'src/a.ts' }`

#### Scenario: mv 产生双条目

- **WHEN** `extractFileOps` 收到 `mv src/a.ts src/b.ts`
- **THEN** 返回含 `{ op: 'delete', path: 'src/a.ts' }` 与 `{ op: 'write', path: 'src/b.ts' }`

#### Scenario: git restore 归 revert 而 --source 归 write

- **WHEN** `extractFileOps` 分别收到 `git restore src/a.ts` 与 `git restore --source=HEAD~1 src/a.ts`
- **THEN** 前者返回 `{ op: 'revert', path: 'src/a.ts' }`
- **AND** 后者返回 `{ op: 'write', path: 'src/a.ts' }`

### Requirement: 净状态折叠规则

记录器 SHALL 按以下对称规则把事件流折叠为净状态（而非去重追加）：

```
write(P)   →  deleted -= P ;  written += P
delete(P)  →  written -= P ;  deleted  += P
revert(P)  →  written -= P ;  deleted -= P
```

折叠语义 SHALL 满足"只多记（overstate）、不漏记危险操作"。系统 SHALL NOT 引入 change 前的 baseline 路径或内容快照；折叠产生的存在性歧义（mv 往返、重写原内容还原等）由外部裁判兜底：delete 净状态裁判为文件系统存在性，write 净状态裁判为内容核对与突变去噪过滤。

hook 认不出的还原路径 SHALL 走 `change_files` 工具的 `set` 语义显式修正；批量还原（`git stash` / `git clean`）由 PreToolUse 拦截（见 protect-files-hook 规格），phase 内增量还原以 `git restore` 为规范动作（见 phase-agents 规格）。

#### Scenario: 新建后放弃折叠为 deleted

- **WHEN** 事件序列为 write(P) 后 delete(P)，且 design 未声明 P
- **THEN** 净状态为 `deleted` 含 P、`written` 不含 P
- **AND** 对账时因文件不存在交 agent 判断（自建自删说明后放行）

#### Scenario: revert 折叠为净 untouched

- **WHEN** 事件序列为 write(P) 后 `git restore P`
- **THEN** `written` 与 `deleted` 均不含 P

#### Scenario: 删除后重建折叠为 written

- **WHEN** 事件序列为 delete(P) 后 write(P)
- **THEN** `written` 含 P、`deleted` 不含 P
- **AND** 若 design 声明删除 P，对账因文件存在判"未删"硬 fail

### Requirement: 消费方对账三态

evaluator 范围核对 SHALL 按 design 变更清单 × actual 清单 × 文件系统三态对账，write 与 delete 两侧对称：

| 对比 | 判定 |
|------|------|
| 计划有、actual 无、文件不存在 | 硬 fail（未实现） |
| 计划有、actual 无、文件存在 | 良性漏记，内容照常核对 |
| actual 有、计划无 | agent 判断；额外删除从严（无关源码被删＝fail） |
| design 声明删、文件系统仍存在 | 硬 fail（未删） |
| design 声明删、文件已消失但不在 `actual.deleted` | hook 漏记，良性 |
| `actual.deleted` 含计划外路径 | agent 判断但从严 |

actual 的职责 SHALL 是圈定突变/审查范围并抓计划外改动，MUST NOT 被当作"实现发生过"的证明（该证明由内容核对权威承担）。

#### Scenario: 计划有文件缺失硬 fail

- **WHEN** design 声明修改 `src/a.ts`，`files.written` 不含它且文件系统无该文件
- **THEN** evaluator 范围核对判硬 fail

#### Scenario: 漏记但文件存在则良性

- **WHEN** design 声明修改 `src/a.ts`，`files.written` 不含它但文件存在且内容已改
- **THEN** 判定良性漏记，内容核对照常进行

#### Scenario: 计划外删除从严

- **WHEN** `files.deleted` 含 `src/util.ts` 且 design 未声明删除
- **THEN** evaluator 判定需 agent 说明；无关源码被删判 fail

### Requirement: archi_check 支持清单文件列表

`plugins/dev-team/bin/src/lib/c4-cross-ref.ts` 的被查文件集解析 SHALL 支持来自 change 文件清单的文件列表：以清单模式调用时，被查文件集取目标 change 的 `files.written`（经既有 test 无关的路径直通，不做 test config 过滤），与 `openspec/architecture` 模型做 import 交叉核对，MUST NOT 执行 `git diff --cached`。既有 `files` 显式入参 SHALL 继续可用并优先于清单；`staged` 语义 SHALL 由清单模式替代或降级为显式入参别名（参数形态 design 定），MUST NOT 成为清单缺失时的隐式 git 回退。目标 change 的 `workflow.json` 缺失 `files` 字段时 SHALL 硬报错（与其它消费方一致的重建指引）。

架构模型文件（`openspec/architecture/**`）SHALL 只作为核对参照系，MUST NOT 进入被查文件集（与清单的 `openspec/` 排除规则一致）。

#### Scenario: 清单驱动被查文件集

- **WHEN** 以清单模式调用 `archi_check`，目标 change 的 `files.written` 为 `["src/a.ts", "src/b.ts"]`
- **THEN** cross-ref 以这两个文件为被查文件集与架构模型比对
- **AND** 进程不执行 `git diff --cached`

#### Scenario: 显式 files 参数优先且不触发清单读取

- **WHEN** `archi_check` 收到非空 `files: ["src/c.ts"]` 与清单模式标志并存
- **THEN** 被查文件集为 `["src/c.ts"]`
- **AND** 不读取 `workflow.json` 的 `files` 字段

#### Scenario: 模型文件不进被查文件集

- **WHEN** 清单模式被查文件集解析完成
- **THEN** `openspec/architecture/**` 下的模型文件不出现在被查文件集
- **AND** 模型仅作为 import 关系的参照系参与核对

#### Scenario: 旧 change 硬报错

- **WHEN** 清单模式调用且目标 change 的 `workflow.json` 无 `files` 字段
- **THEN** 返回硬错误并指引重建
- **AND** MUST NOT 回退为 `git diff --cached`

### Requirement: backtrack 保持清单，遗留由 redo 轮对冲

`backtrack` MCP 工具 MUST NOT 修改该 change 的 `files`（不读取、不清空、不重记），对任何回溯目标（含 `implement` 及更早 phase）保持中立；记录器随后继续在原净状态上折叠。

回退遗留（被丢弃方案写入、新方案未声明的文件）由 redo 轮对冲消除：遗留 HEAD 已有文件 → redo 轮 `git restore <path>`（`revert(P)` 双侧移除，净 untouched 自行消除）；遗留旧方案新建文件 → redo 轮删除该文件（`delete(P)`：`written -= P`、`deleted += P`），对账按「新建后放弃折叠为 deleted」良性放行。未被 redo 轮对冲的路径由「消费方对账三态」的「actual 有、计划无」判定打回清理（处置语义见 `phase-agents`「implementation-evaluator 范围核对改为三态对账」）。

#### Scenario: 回退后遗留 HEAD 已有文件，redo 轮 git restore 净消除

- **WHEN** design v1 修改 `src/a.ts`（`files.written` 含该路径）后回退到 `dev-design` 换方案（v2 变更清单不涉及该文件）
- **AND** redo 轮对遗留执行 `git restore src/a.ts`
- **THEN** 折叠后 `written` 与 `deleted` 均不含 `src/a.ts`
- **AND** 对账不再把它判为计划外改动

#### Scenario: 回退后遗留旧方案新建文件，redo 轮删除良性放行

- **WHEN** design v1 新建 `src/legacy.ts`（`files.written` 含该路径）后回退，v2 未声明该文件
- **AND** redo 轮删除该文件
- **THEN** 折叠为 `written` 不含、`deleted` 含 `src/legacy.ts`
- **AND** evaluator 按「自建自删」说明后放行，不判 fail

#### Scenario: backtrack 自身不触碰清单

- **WHEN** 对 change 调用 `backtrack(phase: "test-execution", backtrack_to: "implement", ...)`
- **THEN** `files` 与回溯前逐项一致（含 `source` 审计映射）
- **AND** 记录器此后继续在原净状态上折叠
