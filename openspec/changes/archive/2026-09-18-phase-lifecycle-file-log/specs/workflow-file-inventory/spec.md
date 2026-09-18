# workflow-file-inventory Specification (delta)

## ADDED Requirements

### Requirement: 归账门控与 scope 路由

PostToolUse 记录器 SHALL 对每个文件写事件按序判定,入账当且仅当全部满足:

1. 事件 `session_id` 在注册表绑定 change(未绑定 → 丢弃 + stderr 诊断,现状保留);
2. 目标 change 的 `workflow.json` 存在 `active_phase`(缺失或 null → 丢弃 + stderr 诊断,exit 0,不阻塞);
3. 事件 `agent_type` 与 `active_phase` 对应 phase 的 `executor.agent_type`(经平台名解析)一致。

条件 3 满足 → 记录 scope 为该 phase id 并携带其 `attempt`;不满足(主 agent 无 `agent_type`,或非 executor 子代理)→ **不丢弃**,记录 scope 为 `'workflow'`(无 attempt)——即主 agent 擅自写为"记录 + 标记",MUST NOT 硬丢弃。错误终态(round_limit_exceeded / max_retries_exceeded)与 phase_log 清场后的窗口经门 2 自然闭合;工作流完成(done)后的 Verdict `phase_next` 重绑因门 2 已关 MUST NOT 复活归账;backtrack 后新 turn 的 `phase_start` 重新开门,归账自愈恢复。scope 解析 SHALL 在 hook 命令层完成并作为上下文传入模块归账管线。

#### Scenario: executor 子代理写记为 phase scope

- **WHEN** session 已绑定 change 且 `active_phase` 为 `{ phase: "implement", attempt: 1, ... }`,`implementation-generator` 子代理 Write `src/foo.ts`(事件 `agent_type` 与 executor 一致)
- **THEN** `file_log` 新增 `{ op: "write", scope: "implement", attempt: 1, path: "src/foo.ts", at: <iso> }`

#### Scenario: 主 agent 写记为 workflow scope 不丢弃

- **WHEN** 门 1、2 通过,主会话(无 `agent_type`)Write `src/manual.ts`
- **THEN** `file_log` 新增 `{ op: "write", scope: "workflow", path: "src/manual.ts", at: <iso> }`(无 attempt)

#### Scenario: 无 active_phase 时丢弃

- **WHEN** session 已绑定 change 但该 change 无 `active_phase`(已完成或已被 phase_log 清场),记录器收到写事件
- **THEN** 事件被丢弃,stderr 留诊断,hook 进程以 0 退出,`workflow.json` 不被改写

#### Scenario: 完成后 Verdict 重绑不复活归账

- **WHEN** 最终 `phase_log` pass 落盘(active_phase 清空)后,同 session 再发 `phase_next` 事件(Verdict,返回 done:true),随后同 session 又发生 Write
- **THEN** 绑定被刷新,但该 Write 因门 2 不满足被丢弃,归账不复活

#### Scenario: backtrack 后经 phase_start 自愈

- **WHEN** backtrack 后新 turn 按 `phase_next` → `phase_start` 重开 `active_phase`,executor 写入
- **THEN** 归账恢复为 phase scope 记录,无需任何手工修复

## MODIFIED Requirements

### Requirement: workflow.json 持有文件清单净状态

`workflow.json` SHALL 含可选字段 `file_log`,为日志式记录数组,每条 `{ op: 'write' | 'delete', scope: string, attempt?: number, path: string, at: string }`——`scope` 为产生该操作的 phase id(executor 子代理写)或 `'workflow'`(主 agent 及非 executor 子代理写);`attempt` 仅 phase scope 携带;`path` 为相对项目根的 POSIX 风格路径(目录删除记录目录路径本身,消费方按路径前缀匹配);`at` 为 ISO 8601 时间戳。revert 类操作的日志表达方式由 design 决议,约束见「净状态折叠规则」。存储 SHALL NOT 再保留 `files: { written, deleted }` 净状态字段与 `source` 旁挂来源映射(进行中 change 沿用"legacy 须重建 change"惯例,不做迁移)。`workflowFileSchema` SHALL 扩展校验该字段;未知键保留策略不变。消费方 SHALL 经「净状态折叠规则」定义的派生视图读取 written/deleted 净状态;读到缺失 `file_log` 字段的 change 时 SHALL 硬报错(文案说明"该 change 创建于文件清单机制之前,请重建"),MUST NOT 回退 git diff。本规格下文所称 written/deleted 净状态,除特别说明外均指 `file_log` 的派生视图。

#### Scenario: file_log 通过 schema 校验

- **WHEN** 解析含 `file_log: [{ op: "write", scope: "implement", attempt: 1, path: "src/a.ts", at: "<iso>" }]` 的 `workflow.json`
- **THEN** `workflowFileSchema` 校验通过,`workflow_type` / `created` / `eval` / 未知键行为不变

#### Scenario: 旧 change 硬报错不回退

- **WHEN** 某消费方读取仅有旧 `files` 字段、无 `file_log` 字段的 `workflow.json`
- **THEN** 返回硬错误,文案含重建指引
- **AND** MUST NOT 回退为 git 工作区 diff

### Requirement: PostToolUse 记录器被动归账文件操作

插件 SHALL 注册 PostToolUse hook 记录器,覆盖工具 `Write`、`Edit`、`StrReplace`(取 `tool_input.file_path`)、`NotebookEdit`(取 `tool_input.notebook_path`)与 `Bash`、`PowerShell`、`Shell`(经 `extractFileOps` 提取)——其中 `StrReplace` 与 `Shell` 为 Cursor 侧工具名,与 `Edit` / `Bash` 同链路归账;matcher SHALL 同时含 `phase_next` 与 `phase_start` 的 MCP 全名 token(`__MCP:phase_next__` / `__MCP:phase_start__`)。记录器 SHALL 在「归账门控与 scope 路由」通过后将每次文件操作按 scope 写入当前 change 的 `workflow.json` 的 `file_log` 字段;条目不再携带独立 `agent_type` 审计截(来源由 scope 标记承载)。

记录器 SHALL 排除自污染路径:`openspec/` 整体(含 `openspec/architecture/**`、reports、proposal/design 等)与 `workflow.json` 自身的写/删操作 MUST NOT 入清单。既有 PreToolUse 写保护行为不受影响。

记录器定位目标 change SHALL 经 session 注册表(见下条要求);无法解析目标 change 的事件 SHALL 被丢弃(不报错、不阻塞工具调用),并留诊断输出通道。

#### Scenario: Write 归账 file_log

- **WHEN** 门控通过(executor 子代理、active_phase 为 implement attempt 1),记录器收到 `tool_name: "Write"`、`tool_input.file_path: "<root>/src/foo.ts"` 的 PostToolUse 事件
- **THEN** `openspec/changes/<change>/workflow.json` 的 `file_log` 含 `{ op: "write", scope: "implement", attempt: 1, path: "src/foo.ts" }` 记录

#### Scenario: openspec 路径被排除

- **WHEN** 记录器收到对 `openspec/changes/my-change/design.md` 或 `openspec/architecture/models/x.c4` 的写事件
- **THEN** 这些路径 MUST NOT 出现在 `file_log`

#### Scenario: 递归删除记录目录前缀

- **WHEN** Bash 事件命令为 `rm -rf src/old-module`
- **THEN** `file_log` 含 `{ op: "delete", path: "src/old-module" }` 记录(目录路径本身)

#### Scenario: 无绑定事件静默丢弃

- **WHEN** 记录器收到 `session_id` 未注册表命中的写事件
- **THEN** hook 进程以 0 退出且不阻塞该工具调用
- **AND** `workflow.json` 不被改写

#### Scenario: Cursor 工具名事件同链路归账

- **WHEN** 门控通过的 `StrReplace` 事件(`tool_input.file_path`)与 `Shell` 事件(command `echo x > src/a.ts`)被记录器处理
- **THEN** `file_log` 含这两个路径的 write 记录

### Requirement: session 注册表绑定 current-change

系统 SHALL 维护 `session_id → change` 的持久化注册表。主 agent 调用 MCP `phase_next` 或 `phase_start` 时,该调用自身触发的 PostToolUse 事件 SHALL 携带 `stdin.session_id` 与 `tool_input.change`,记录器据此建立/更新绑定(`phase_start` 的 matcher 为 MCP 全名 token `__MCP:phase_start__`);此后同一 `session_id` 下的工具事件(含 subagent 触发的事件)SHALL 查表归账到对应 change(归账是否入账另经「归账门控与 scope 路由」判定)。

PostToolUse matcher 对 MCP 工具 SHALL 使用 MCP 全名(裸 server 名匹配不到任何工具)。`phase_next` 的输入协议(含必填 `run_id`)MUST NOT 改变;`run_id` 继续承担 phase-next 的 turn 内 anchor 职责,注册表键控使用 `session_id`,两者仅在同一 MCP 调用事件中交汇。

subagent 事件与主会话共享 `session_id` 是文档强指标但非明文契约:若实现期实测不共享,SHALL 退化为"暂存-认领"模式——记录器按 `session_id` 暂存事件流,由消费命令认领合并。

#### Scenario: phase_next 调用建立绑定

- **WHEN** 主 agent 在 session S 中调用 `phase_next(change: "my-change", run_id: <id>)`
- **THEN** 注册表含 `S → my-change`
- **AND** `phase_next` 的输入 schema 与响应行为不变

#### Scenario: phase_start 调用刷新绑定

- **WHEN** session S 的注册表绑定丢失后,主 agent 调用 `phase_start(change: "my-change", phase: <p>)`
- **THEN** 注册表恢复 `S → my-change`

#### Scenario: subagent 事件按 session 归账

- **WHEN** session S 已绑定 `my-change` 且门控通过,`implementation-generator` subagent 在同一 session 下 Write `src/bar.ts`
- **THEN** 该操作以 phase scope 记录入 `my-change` 的 `file_log`

#### Scenario: 不同 session 不串账

- **WHEN** session S1 绑定 `change-a`,session S2 绑定 `change-b`
- **THEN** S1 的写事件归账 `change-a`,S2 的写事件归账 `change-b`

### Requirement: 净状态折叠规则

记录器 SHALL 将事件以日志条目写入 `file_log`,覆盖/追加规则为:dedupe key = `(attempt, path)`(workflow scope 条目无 attempt,按 path 在 workflow scope 内键控);同 key SHALL 原位覆盖 `op` / `scope` / `at`(后写者胜,含 scope 翻转——如同一路径先 phase 记录后 workflow 记录);跨 attempt / 跨 phase SHALL 追加新记录。gitignore 过滤与自污染排除仍在归账管线内、落盘前生效。

净状态 SHALL 按 log 顺序派生:同 `path` 后条胜——末条为 `write` 则 written 含该 path、deleted 不含;末条为 `delete` 则 deleted 含、written 不含。派生结果 SHALL 与现行 `foldFileOps` 的 written/deleted 语义等价(`write(P)` 后 `delete(P)` → deleted 含 P;`delete(P)` 后 `write(P)` → written 含 P),4 个读方契约不变。revert 类操作(`git restore`)的表达方式——追加 `op:'revert'` 记录 vs 移除该 path 的既有记录——由 design 决议,约束为 revert 后 written 与 deleted 均不含该 path(与现行 foldFileOps 的净 untouched 等价)。系统 SHALL NOT 引入 change 前的 baseline 路径或内容快照;派生产生的存在性歧义由外部裁判兜底(与现行一致:delete 净状态裁判为文件系统存在性,write 净状态裁判为内容核对与突变去噪过滤)。hook 认不出的还原路径 SHALL 走 `change_files` 的 `set` 语义显式修正;批量还原由 PreToolUse 拦截,phase 内增量还原以 `git restore` 为规范动作。

#### Scenario: 同 attempt 同路径覆盖

- **WHEN** implement attempt 1 内先 write(P) 后 delete(P)(如建后删)
- **THEN** `file_log` 中 P 的记录为单条 `{ op: "delete", scope: "implement", attempt: 1 }`(原位覆盖,不追加)
- **AND** 派生净状态 deleted 含 P、written 不含 P

#### Scenario: 跨 attempt 追加新记录

- **WHEN** attempt 1 写 P(fail 落盘)后 retry attempt 2 又写 P
- **THEN** `file_log` 含 P 的两条记录(attempt 1 与 attempt 2 各一条)

#### Scenario: scope 翻转原位覆盖

- **WHEN** executor 在 attempt 1 写 P 后,主 agent 在同一 active_phase 期间又写 P
- **THEN** P 的记录被覆盖为 `{ op: "write", scope: "workflow" }`(后写者胜)

#### Scenario: 派生与现行折叠等价

- **WHEN** 事件序列为 write(P) 后 `rm P` 后 write(P)
- **THEN** 派生净状态 written 含 P、deleted 不含 P(与现行折叠一致)

#### Scenario: 派生净状态供读方消费

- **WHEN** 任一读方(突变 scope / `test_resolve_paths` / `archi_check` / evaluator 对账 / `workflow_files`)读取清单
- **THEN** 得到的是派生 written/deleted 路径列表,`file_log` 的 scope/attempt/at 审计明细不改变判定结果

### Requirement: 消费方对账三态

evaluator 范围核对 SHALL 按 design 变更清单 × actual 清单(派生净状态)× 文件系统三态对账,write 与 delete 两侧对称:

| 对比 | 判定 |
|------|------|
| 计划有、actual 无、文件不存在 | 硬 fail(未实现) |
| 计划有、actual 无、文件存在 | 良性漏记,内容照常核对 |
| actual 有、计划无 | agent 判断;额外删除从严(无关源码被删＝fail) |
| design 声明删、文件系统仍存在 | 硬 fail(未删) |
| design 声明删、文件已消失但不在 `actual.deleted` | hook 漏记,良性 |
| `actual.deleted` 含计划外路径 | agent 判断但从严 |

actual 的职责 SHALL 是圈定突变/审查范围并抓计划外改动,MUST NOT 被当作"实现发生过"的证明(该证明由内容核对权威承担)。actual 取自 `file_log` 的派生净状态,scope/attempt 审计明细不参与判定。

#### Scenario: 计划有文件缺失硬 fail

- **WHEN** design 声明修改 `src/a.ts`,派生净状态 written 不含它且文件系统无该文件
- **THEN** evaluator 范围核对判硬 fail

#### Scenario: 漏记但文件存在则良性

- **WHEN** design 声明修改 `src/a.ts`,派生净状态 written 不含它但文件存在且内容已改
- **THEN** 判定良性漏记,内容核对照常进行

#### Scenario: 计划外删除从严

- **WHEN** 派生净状态 deleted 含 `src/util.ts` 且 design 未声明删除
- **THEN** evaluator 判定需 agent 说明;无关源码被删判 fail

### Requirement: archi_check 支持清单文件列表

`plugins/dev-team/bin/src/lib/c4-cross-ref.ts` 的被查文件集解析 SHALL 支持来自 change 文件清单的文件列表:以清单模式调用时,被查文件集取目标 change 清单的派生净状态 written(经既有 test 无关的路径直通,不做 test config 过滤),与 `openspec/architecture` 模型做 import 交叉核对,MUST NOT 执行 `git diff --cached`。既有 `files` 显式入参 SHALL 继续可用并优先于清单;`staged` 语义 SHALL 由清单模式替代或降级为显式入参别名(参数形态 design 定),MUST NOT 成为清单缺失时的隐式 git 回退。目标 change 的 `workflow.json` 缺失 `file_log` 字段时 SHALL 硬报错(与其它消费方一致的重建指引)。

架构模型文件(`openspec/architecture/**`)SHALL 只作为核对参照系,MUST NOT 进入被查文件集(与清单的 `openspec/` 排除规则一致)。

#### Scenario: 清单驱动被查文件集

- **WHEN** 以清单模式调用 `archi_check`,目标 change 派生净状态 written 为 `["src/a.ts", "src/b.ts"]`
- **THEN** cross-ref 以这两个文件为被查文件集与架构模型比对
- **AND** 进程不执行 `git diff --cached`

#### Scenario: 显式 files 参数优先且不触发清单读取

- **WHEN** `archi_check` 收到非空 `files: ["src/c.ts"]` 与清单模式标志并存
- **THEN** 被查文件集为 `["src/c.ts"]`
- **AND** 不读取 `workflow.json` 的 `file_log` 字段

#### Scenario: 模型文件不进被查文件集

- **WHEN** 清单模式被查文件集解析完成
- **THEN** `openspec/architecture/**` 下的模型文件不出现在被查文件集
- **AND** 模型仅作为 import 关系的参照系参与核对

#### Scenario: 旧 change 硬报错

- **WHEN** 清单模式调用且目标 change 的 `workflow.json` 无 `file_log` 字段
- **THEN** 返回硬错误并指引重建
- **AND** MUST NOT 回退为 `git diff --cached`

### Requirement: backtrack 保持清单,遗留由 redo 轮对冲

`backtrack` MCP 工具 MUST NOT 修改该 change 的 `file_log` 与 `active_phase`(不读取、不清空、不重记),对任何回溯目标(含 `implement` 及更早 phase)保持中立;记录器在新 turn `phase_start` 重开门后继续在原 log 上覆盖/追加。

回退遗留(被丢弃方案写入、新方案未声明的文件)由 redo 轮对冲消除:遗留 HEAD 已有文件 → redo 轮 `git restore <path>`(revert 表达按「净状态折叠规则」的 design 决议落 log,派生净 untouched 自行消除);遗留旧方案新建文件 → redo 轮删除该文件(delete(P) 记录,派生 deleted 含 P、written 不含),对账按「新建后放弃折叠为 deleted」良性放行。未被 redo 轮对冲的路径由「消费方对账三态」的「actual 有、计划无」判定打回清理(处置语义见 `phase-agents` 规格)。

#### Scenario: 回退后遗留 HEAD 已有文件,redo 轮 git restore 净消除

- **WHEN** design v1 修改 `src/a.ts`(派生 written 含该路径)后回退到 `dev-design` 换方案(v2 变更清单不涉及该文件)
- **AND** redo 轮对遗留执行 `git restore src/a.ts`
- **THEN** 派生净状态 written 与 deleted 均不含 `src/a.ts`
- **AND** 对账不再把它判为计划外改动

#### Scenario: 回退后遗留旧方案新建文件,redo 轮删除良性放行

- **WHEN** design v1 新建 `src/legacy.ts`(派生 written 含该路径)后回退,v2 未声明该文件
- **AND** redo 轮删除该文件
- **THEN** 派生净状态 written 不含、deleted 含 `src/legacy.ts`
- **AND** evaluator 按「自建自删」说明后放行,不判 fail

#### Scenario: backtrack 自身不触碰清单

- **WHEN** 对 change 调用 `backtrack(phase: "test-execution", backtrack_to: "implement", ...)`
- **THEN** `file_log`(含全部 scope/attempt 审计记录)与 `active_phase` 和回溯前一致
- **AND** 记录器在新 turn 开门后继续在原 log 上覆盖/追加

### Requirement: 记录器同步 gitignore 过滤

PostToolUse 记录器 SHALL 在归账时同步应用 gitignore 过滤:在同一 hook 调用内、落盘之前,以**层级 `.gitignore`** 规则——项目根 `.gitignore` 与目标路径祖先链上各目录的 `.gitignore`——判定规范化后的目标路径,被忽略的路径 MUST NOT 进入清单(不产生 `file_log` 记录)。判定 SHALL 遵循 gitignore 语义:注释(`#`)与空行忽略;`*` / `**` 通配;尾随 `/` 的模式仅匹配目录;`!` 负模式取消忽略;后匹配覆盖先匹配;被忽略目录下的子路径一并忽略。层级间:深层文件的规则 SHALL 优先于浅层文件;目录在任一层被排除后,其下路径一律忽略,更深层 `.gitignore` SHALL NOT 参与判定(git 对被排除目录不下降)。

过滤器不可判定时 SHALL fail-open(保留路径入清单):无 `.gitignore` 的层视为该层无规则;任一层读取失败或包含无法判定的构造时,记录器 SHALL 按未过滤继续归账并向 stderr 留诊断,MUST NOT 抛错阻塞被观察的工具调用(与记录器既有的 exit-0 吞错策略一致)。

gitignore 过滤 SHALL 与既有自污染排除(`openspec/` 整体、`workflow.json` 自身)叠加而非替代。`change_files` 的 append / set 为人工补录与显式修正通道,MUST NOT 应用 gitignore 过滤。过滤仅在归账时生效:MUST NOT 回溯清洗既有 log 记录,MUST NOT 改变 `workflow_files` 查询忠实返回派生净状态的语义。

#### Scenario: gitignore 命中的写事件不入清单

- **WHEN** 项目根 `.gitignore` 含 `.*/`,门控通过,记录器收到对 `<root>/.claude/agent-memory/x.md` 的 Write 事件
- **THEN** `file_log` 不含 `.claude/agent-memory/x.md` 的记录
- **AND** `workflow.json` 其余字段不变

#### Scenario: 负模式取消忽略后照常归账

- **WHEN** 项目根 `.gitignore` 含 `.*/` 与 `!.claude-plugin`,记录器收到对 `<root>/.claude-plugin/marketplace.json` 的 Write 事件
- **THEN** `file_log` 含 `.claude-plugin/marketplace.json` 的 write 记录

#### Scenario: 子目录 .gitignore 规则覆盖根规则

- **WHEN** 项目根 `.gitignore` 含 `*.log`,`apps/x/.gitignore` 含 `!keep.log`,记录器收到对 `apps/x/keep.log` 的 Write 事件
- **THEN** `file_log` 含 `apps/x/keep.log`(深层负模式覆盖根忽略)

#### Scenario: 被排除目录内的 .gitignore 不生效

- **WHEN** 项目根 `.gitignore` 含 `build/`,`build/.gitignore` 含 `!out.js`,记录器收到对 `build/out.js` 的 Write 事件
- **THEN** `file_log` 不含 `build/out.js`(目录短路,`build/` 内规则不参与判定)

#### Scenario: 未忽略路径不受影响

- **WHEN** 记录器收到对 `src/a.ts` 的 Write 事件(门控通过)
- **THEN** 归账行为与引入过滤前一致(write 记录入 log)

#### Scenario: 过滤器缺失或失败 fail-open

- **WHEN** 项目根与目标路径祖先链均不存在 `.gitignore`(或任一层存在但内容无法解析)
- **THEN** 记录行为与无过滤一致,路径照常入清单
- **AND** hook 进程以 0 退出,MUST NOT 阻塞该工具调用

#### Scenario: change_files 补录不受过滤

- **WHEN** 经 `change_files` append 一条 gitignore 命中的路径(如 `node_modules/pkg/index.js`)
- **THEN** 该路径以 workflow scope 记录入 log,append/set 语义不变

## REMOVED Requirements

### Requirement: 清单条目来源审计

**Reason**: 来源审计由 `file_log` 的 `scope` 字段取代(executor 写 → phase id + attempt;主 agent / 非 executor 写 → `'workflow'`),executor 类型可由 phase 表推导,独立的 `agent_type` 旁挂映射成为冗余;且旧形态与 `files` 净状态字段一并废除。

**Migration**: 审计诉求(排查归账错误、定位范围超集来源)经 `file_log` 的 scope/attempt 记录与 phase 表推导满足;原"消费方 MUST NOT 依赖来源字段"的禁令由"消费方仅读派生 written/deleted 净状态"承接(见「消费方对账三态」与「净状态折叠规则」的派生场景)。

## Module Contract

### Module: `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts`(file_log 存储层)

| 函数 | 参数 | 返回 | 描述 |
|------|------|------|------|
| `readFileLog` | `changeDir: string` | `FileLogEntry[]` | 读取并校验 `workflow.json` 的 `file_log`;缺失 `file_log` 时硬报错(重建指引) |
| `appendLogEntries` | `changeDir: string, entries: FileLogEntry[]` | `FileLogEntry[]` | 按 (attempt, path) 键控覆盖/追加后落盘(workflow scope 按 path 键控);写纪律与现行 `writeFileInventory` 一致(文件必须已存在、未知键保留、2 空格缩进) |
| `deriveNetState` | `log: FileLogEntry[]` | `{ written: string[], deleted: string[] }` | 纯函数:按 log 顺序同 path 后条胜,派生净状态;与现行 `foldFileOps` 等价 |
| `recordFileOps` | `changeDir: string, ops: FileOp[], context: { projectRoot: string, scope: { kind: 'phase', phase: string, attempt: number } \| { kind: 'workflow' } }` | `void` | 归账管线:规范化 → 自污染排除 → gitignore 过滤 → 以门控解析出的 scope 写 log(确切签名 design 定) |
| `appendWorkflowFiles` | `changeDir: string, paths: { written?: string[], deleted?: string[] }` | 派生净状态 | change_files append 语义:workflow scope 按 path upsert,phase 审计记录保留 |
| `setWorkflowFiles` | `changeDir: string, paths: { written?: string[], deleted?: string[] }` | 派生净状态 | change_files set 语义:删除涉及 path 的全部记录 + 追加 workflow 记录(实现形态 design 定) |
| 类型 | `FileLogEntry` / `FileOp` | — | `{ op, scope, attempt?, path, at }` 与 hook 提取的原始操作 |

移除:`readFileInventory` / `foldFileOps` / `writeFileInventory` 的 `files` 桶形态 API 与 `source` 映射维护(由 log API 与派生视图取代)。

### 组件: `plugins/dev-team/bin/src/commands/record-files.ts`(消费方)

| 项 | 值 |
|----|-----|
| 事件识别 | `phase_next` 与 `phase_start` MCP 调用事件 → 绑定刷新;其余事件 → 门控判定 |
| 门控 | 绑定? → active_phase? → agent_type 与 executor 一致? → scope 解析(phase / workflow)或丢弃 |
| 失败语义 | 任何错误 stderr 诊断 + exit 0;门 2 不满足的丢弃留 stderr 诊断 |
| gitignore | 归账管线内、落盘前同步应用(不适用 change_files / workflow_files) |

### 读方(契约不变,改走派生视图)

| 文件 | 消费方式 |
|------|---------|
| `modules/workflow/files/files-query.ts`(workflow_files) | 派生净状态,输出 `{ written, deleted }` 形状不变 |
| `lib/c4-cross-ref.ts`(archi_check) | 派生 written 为被查文件集 |
| `commands/test-execution.ts` | 突变 scope 取派生净状态 |
| `commands/test-resolve-paths.ts` | 清单模式取派生净状态 |

---
