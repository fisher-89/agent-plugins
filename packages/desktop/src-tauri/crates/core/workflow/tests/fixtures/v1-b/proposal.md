# 提案: workflow-file-inventory

> **变更**: workflow-file-inventory
> **日期**: 2026-09-16
> **状态**: draft（proposal 阶段产出，基于 3 轮 explore 结论；2026-09-17 回溯补充：突变测试 CLI 参数面收窄——`--change` 兼作清单入口，不引入 `--mutation-scope`，`--mutation-diff-only` 删除，`--skip-mutation` 保留；同日拍板：清单条目记录来源 `agent_type` 作审计）

---

## 问题

「本 change 改了哪些文件」目前由 **git 工作区状态**充当事实来源，带来一串结构性问题：

1. **index 污染**：`getGitDiffFiles`（`plugins/dev-team/bin/src/lib/git.ts`）用 `git add -N` + `git reset` 临时改写用户 index，让 `git diff HEAD` 能看到 untracked 文件——与用户自己的 staging 操作互相干扰。
2. **范围超集**：git status 是整个工作区共享的。用户手头的无关改动、并行 change 的改动、中途 commit 过的文件，全部被扫进突变测试与 evaluator 核对范围；每个 change 本有独立的 `workflow.json` 天然隔离，却没被利用。
3. **build 产物污染**：untracked 的构建产物（`pnpm build` 刷新的 `claude-plugins/**`、`cursor-plugins/**`）混入变更范围。
4. **计划侧无结构化清单**：proposal.md「变更范围」与 design.md「变更清单」只是非结构化 markdown，且**均无「删除文件」概念**，无法被机器消费与对账。

git 依赖现状全景（探索期扫描，8 处）：

| #   | 位置                                                                                   | 依赖方式                                                       |
| --- | -------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| 1   | `bin/src/lib/git.ts:25` `getGitDiffFiles`                                              | `git add -N` → `git diff HEAD` → `git reset`                   |
| 2   | `bin/src/commands/test-execution.ts:80` 突变测试 scope                                 | 调 #1                                                          |
| 3   | `bin/src/commands/test-resolve-paths.ts:165` `modules:"git-change"` 模式               | `git diff HEAD --name-only`（无任何 agent 文档引用的孤儿路径） |
| 4   | `bin/src/lib/c4-cross-ref.ts:111` `archi_check` staged 模式                            | `git diff --cached`                                            |
| 5   | `agents/implementation-evaluator.md` I1/I2/I6（I6＝范围核对本体）                      | 提示词要求跑 `git diff`                                        |
| 6   | `agents/test-gen-evaluator.md` / `test-gen-generator.md`（"git diff IS the artifact"） | 同上                                                           |
| 7   | `agents/acceptance-evaluator.md` / `code-review-evaluator.md`                          | `git diff` 看全量产出                                          |
| 8   | `skills/phase-implement` / `phase-test-gen` SKILL.md                                   | 文案                                                           |

---

## 提案

**结论架构：计划声明（design 变更清单）＋ 被动观察（hook 记录），两边对账。**

```
   计划侧（主动声明）                    实际侧（被动观察，不可遗忘）
─────────────────────────            ─────────────────────────────
design.md 变更清单                    PostToolUse hook（新增）
  ├── 新增文件                          ├── Write/Edit/NotebookEdit → file_path
  ├── 修改文件                          ├── Bash / PowerShell
  ├── 删除文件 ◄── 模板补此节            │     → extractFileOps 泛化现有
  └── （函数/类型/配置子表不变）          │       detectBashWrite/detectPowerShellWrite
                                       └── 折叠净状态 → 过滤 → 记录
                                                    │
                                                    ▼
                                       workflow.json
                                         files: { written: [...], deleted: [...] }
                                                    │
        ┌──────────────────────────────────────────┤
        ▼                                            ▼
  evaluator 核对（提示词重写）              突变测试 / test_resolve_paths
  （design × actual × 文件系统三态对账）      / archi_check 直接读 actual，git 退场
```

核心机制：

1. **清单载体**：`workflow.json` 新增 `files: { written: string[], deleted: string[] }`。`change_create` 初始化为空；消费方读到缺失字段 → 硬报错（"该 change 创建于文件清单机制之前，请重建"），不做 git 回退。
2. **被动记录**：新增 PostToolUse hook 记录器。hook 观察到主 agent 调用 `mcp__plugin_dev-team_dev-team__phase_next`（matcher 必须用全名）的事件时，用 `stdin.session_id` + `tool_input.change` 建立 **session 注册表**（`session_id → change`）；此后同 session 的 Write/Edit/NotebookEdit/Bash/PowerShell 事件查表归账到对应 change 的 `files`。`run_id` 与 `phase_next` 既有契约不变，两者在同一 MCP 调用事件中交汇、各司其职。
3. **提取器共用**：泛化 `detectBashWrite` / `detectPowerShellWrite` 为 `extractFileOps(command) → [{ op: 'write'|'delete'|'revert', path }]`，同时供 PreToolUse 写保护（提取路径 ∩ 保护 glob → deny）与 PostToolUse 记录器（提取路径 → 过滤 → 归账）消费。附带收益：写保护从包含式判断升级为按提取路径精确匹配，并自动获得**删保护**（拦截 `rm workflow.json` 类操作）。
4. **净状态折叠**（还原语义，不引入 baseline 快照）：
   ```
   write(P)   →  deleted -= P ;  written += P
   delete(P)  →  written -= P ;  deleted  += P
   revert(P)  →  written -= P ;  deleted -= P    ← 仅命令识别触发
                 （git restore / git checkout -- ；带 --source= 的归 write）
   ```
   折叠只多记（overstate）、不漏记危险操作；多记方向由外部裁判兜底（delete 净状态裁判＝文件系统存在性；write 净状态裁判＝内容核对与突变去噪）。`rm -rf <dir>` 记录目录路径本身，消费方按前缀匹配，与 design 侧声明同构。
5. **自污染排除**：`openspec/` 整体排除（design/proposal/reports/architecture 是工作流产物，`archi_write`/`archi_decide` 因此**不需要**任何记录）＋ `workflow.json` 自身；既有 PreToolUse 写保护不变。
6. **对账三态**（write 与 delete 对称）：
   - 计划有、actual 无、文件不存在 → 硬 fail（未实现）
   - 计划有、actual 无、文件存在 → 良性漏记，内容照常核对（覆盖 `archi_write`、`node -e` 等 hook 不可见写路径；actual 的职责是圈定突变/审查范围，不是证明实现发生过）
   - actual 有、计划无 → agent 判断（**额外删除从严**：无关源码被删＝fail）
7. **git 退场与残留角色**：范围判定全部改读清单（突变测试由既有 `--change=<change-name>` 参数直接触发：传入即自动以该 change 的 `files` 为突变 scope，叠加净归零去噪过滤 `written ∩ (content ≠ HEAD)`，`--mutation-diff-only` 删除且不新增 `--mutation-scope`，`--skip-mutation` 保留；`test_resolve_paths` 的 `git-change` 模式改读清单；`archi_check` 支持传入清单文件列表）；`getGitDiffFiles` 连同 `add -N`/`reset` hack **整个删除**。code-review / acceptance 等 evaluator 可把 `git diff` 留作**纯观察辅助**（看修改内容以 review 质量），不参与范围判定；`git stash`/`git clean` 类批量还原对 hook 不可见，交 PreToolUse 拦截——它们同时对用户数据最危险。
8. **backtrack 清空**：backtrack 到 `implement` 或更早 phase → 清空该 change 的 `files` 重记，避免废弃方案残留污染突变范围。批量还原（backtrack）与 phase 内增量还原（折叠规则）分工。
9. **手动补录通道**：新增 MCP 工具 `change_files`（`append`/`set` 两种语义）。Cursor 侧 hooks 支持不全（canonical 描述：static-check "主要作用于 Claude"），该工具作非 Claude 环境或漏记的人工灌入口；`set` 覆写净状态，兼作 hook 认不出的还原的显式修正通道。
10. **提示词与模板对齐**：evaluator 范围核对重写为三态对账；`implementation-generator` 注明"撤销文件修改用 `git restore <path>`（可被记录），勿重写内容"；`design.md.template` 补「删除文件」子节；proposal 模板「变更范围」支持删除。
11. **条目来源审计**：subagent 触发的事件 stdin 额外携带 `agent_type`，记录器将其随清单条目一并落盘（主会话事件与 `change_files` 手动补录省略该字段；同路径重写取最后写入者，条目被折叠移除时来源随之消失）。该字段仅作审计——排查归账错误/范围超集时免翻 transcript；突变 scope、`test_resolve_paths`、`archi_check`、evaluator 对账均不依赖它。具体 JSON 形态（条目内嵌 vs 保持路径数组 flat 的旁挂映射）由 design 定。

---

## 能力

### 新增能力

- **workflow-file-inventory** — `workflow.json` 文件清单机制：`files.written/deleted` schema、PostToolUse 被动记录器、session 注册表绑定、write/delete/revert 三态提取与净状态折叠、条目来源审计（subagent `agent_type`，last-writer-wins）、自污染排除、backtrack 清空、消费方对账三态与突变去噪、`archi_check` 清单消费（`c4-cross-ref.ts` 的 git staged 依赖退场）。
- **change-files** — MCP `change_files` 补录/修正工具（append 追加、set 覆写净状态），作非 Claude 环境兜底与显式还原修正通道。

### 修改的能力

- **protect-files-hook** — `extractFileOps` 共享提取器（write/delete/revert 三态）；写保护升级为提取路径精确匹配并扩为写/删保护；PreToolUse 扩拦截 `git stash` / `git clean` 等批量还原。
- **change-create** — 创建 `workflow.json` 时初始化 `files: { written: [], deleted: [] }`。
- **pipeline-backtrack** — backtrack 到 `implement` 或更早 phase 时清空该 change 的 `files` 重记。
- **unit-test-executor** — 突变测试范围从 git diff 改读清单：传入 `--change=<change-name>` 即自动以该 change 的 `files` 为突变 scope（叠加净归零去噪过滤），无需也不存在 `--mutation-scope` / `--mutation-diff-only`（后者删除），`--skip-mutation` 保留。
- **test-path-resolver** — `modules: "git-change"` 模式改为读取 change 文件清单。
- **phase-agents** — implementation-evaluator I1/I2/I6 等范围核对重写为三态对账；test-gen-generator "git diff IS the artifact" 语义改清单；implementation-generator 规范还原动作。
- **phase-skills** — `phase-implement` / `phase-test-gen` SKILL.md 范围核对文案对齐清单机制。
- **json-design-schemas** — `design.md.template` 补「删除文件」子节；`proposal.md.template` 变更范围支持删除声明。

---

## 变更范围

### 实现文件

代码层（`plugins/dev-team/`）：

- `bin/src/lib/git.ts` — **整个删除** `getGitDiffFiles`（连同 `simple-git` 依赖，若无其他使用）
- `bin/src/hooks.ts` — 泛化 `extractBashWriteTargets` / `extractPowerShellWriteTargets` 为 `extractFileOps`（write/delete/revert 三态）；新增 PostToolUse 记录器子命令（净状态折叠 → 过滤 → 写入 `workflow.json`）；PreToolUse 扩拦截 git stash/clean 与 openspec 工作流产物还原保护
- `hooks/hooks.canonical.json` + build 组装（hooks profile）— 注册 PostToolUse 记录器（matcher：`Write|Edit|NotebookEdit|Bash|PowerShell` 及全名 `mcp__plugin_dev-team_dev-team__phase_next`）
- 新增 session 注册表模块（如 `bin/src/lib/session-registry.ts`，`session_id → change` 映射持久化）
- `bin/src/schemas/workflow.schema.ts` — `workflowFileSchema` 扩展 `files` 字段（`written`/`deleted` 两个路径列表，条目携带可选来源 `agent_type`；具体 JSON 形态由 design 定）
- `bin/src/commands/change-create.ts` — 初始化 `files`
- `bin/src/commands/backtrack.ts` — 回溯清空规则
- `bin/src/commands/test-execution.ts` — `resolveMutationDiffFiles` 改读清单（由 `--change` 触发）+ 删除 `--mutation-diff-only` + 净归零去噪
- `bin/src/commands/test-resolve-paths.ts` — `git-change` 模式改读清单
- `bin/src/lib/c4-cross-ref.ts` — `archi_check` 支持传入清单文件列表
- `bin/src/mcp.ts` + `bin/src/schemas/` — 注册 `change_files` 工具及 input/output schema

提示词/模板层：

- `agents/implementation-evaluator.md` — I1/I2/I6 重写为 design × actual × 文件系统三态对账
- `agents/test-gen-evaluator.md` / `agents/test-gen-generator.md` — "git diff IS the artifact" 语义改为清单核对
- `agents/acceptance-evaluator.md` / `agents/code-review-evaluator.md` — `git diff` 降级为纯观察辅助，范围判定读清单 + 文件系统
- `agents/implementation-generator.md` — 规范还原动作（`git restore <path>`，可被记录；勿重写内容）
- `skills/phase-implement/SKILL.md` / `skills/phase-test-gen/SKILL.md` — 范围核对文案对齐
- `templates/artifacts/design.md.template` — 变更清单补「删除文件」子节
- `templates/artifacts/proposal.md.template` — 「变更范围」支持删除声明

### 测试文件

- `bin/src/hooks.test.ts` — `extractFileOps` 三态提取、折叠规则性质（只多记不漏记）、记录器过滤与归账、条目来源 `agent_type`（subagent 携带、主会话省略、重写 last-writer-wins）、PreToolUse 扩拦截
- 新增 session 注册表测试（绑定、查表归账、过期/清理行为）
- `bin/src/commands/change-create.test.ts` — `files` 初始化断言
- `bin/src/commands/backtrack.test.ts` — 清空规则断言
- `bin/src/commands/test-execution.test.ts` — 移除 `getGitDiffFiles` mock，改清单读取（`--change` 触发、无 scope 选项）与去噪过滤断言；`--skip-mutation` 保留断言
- `bin/src/cli.test.ts` — 移除 `--mutation-diff-only` 选项解析断言；`--change` / `--skip-mutation` 解析断言保留
- `bin/src/commands/test-resolve-paths.test.ts` — 清单模式断言（替换 git-change 场景）
- `bin/src/lib/c4-cross-ref.test.ts` — 清单文件列表入参断言
- `bin/src/mcp.test.ts` — `change_files` 注册断言
- `bin/src/lib/git.test.ts` — **删除**（随 `lib/git.ts`）

### 不要修改

- `phase_next` 输入协议（`run_id` 必填契约、`(change, run_id)` anchor 机制不变）
- 既有内置写保护 glob 集合与 fail-open 语义（仅扩展拦截范围与提取精度）
- `openspec/config.json` 的 `write_protection` 配置结构
- 评估历史机制：`workflow.json.eval`、`phase_log`、`backtrack` 的评估条目语义
- 与清单无关的 evaluator / generator（proposal-evaluator、dev-design-_、test-design-_、code-analyze-* 等）
- `archi_write` / `archi_decide` 的写入行为（产出在 `openspec/` 排除范围内，不记录是既定过滤规则的默认行为，非新增特判）

---

## 验收标准

| ID    | 变更实现                          | 验收条件                                                                                                                                                                                                                                                                            |
| ----- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-1  | `change-create.ts` 初始化 `files` | 新建 change 的 `workflow.json` 含 `files: { written: [], deleted: [] }`；`workflowFileSchema` 校验通过；既有 `workflow_type`/`created` 行为不变                                                                                                                                     |
| AC-2  | PostToolUse 记录器                | Claude 环境下经 Write/Edit/NotebookEdit 写 `src/foo.ts` 后，当前 change 的 `files.written` 含 `src/foo.ts`；经 Bash `rm` 删除后 `files.deleted` 含该路径；`openspec/**` 与 `workflow.json` 自身的操作不入清单                                                                       |
| AC-3  | session 注册表绑定                | hook 观察到 `phase_next` 调用事件后建立 `session_id → change` 映射；同 session 的 subagent 工具事件归账到该 change；不同 session 互不串账                                                                                                                                           |
| AC-4  | `extractFileOps` 三态与折叠       | `>`/`>>`/`tee`/`Set-Content` 等归 write；`rm`/`Remove-Item`/`git rm` 归 delete；`git restore`/`git checkout --` 归 revert；`mv`/`Move-Item` 产生 delete(旧)+write(新) 双条目；write→revert 折叠为净 untouched，delete→write 折叠为 written；`rm -rf dir` 记录目录路径               |
| AC-5  | PreToolUse 扩拦截                 | `git stash` / `git clean` 类命令被 deny；`rm <受保护路径>` 被 deny；`git restore` 对 `openspec/changes/**` 工作流产物的还原被 deny（或按 design 决议的保护范围拦截）                                                                                                                |
| AC-6  | backtrack 清空                    | backtrack 到 `implement` 或更早后，该 change 的 `files` 被清空；backtrack 到 implement 之后的 phase 不清空                                                                                                                                                                          |
| AC-7  | 突变测试读清单                    | 传入 `--change=<change-name>` 时突变 scope 自动来自该 change 的 `files.written` ∩（内容 ≠ HEAD），无需 `--mutation-scope` 或 `--mutation-diff-only`（前者不引入、后者已删除）；`--skip-mutation` 仍可跳过突变；`files` 缺失的旧 change 硬报错；`lib/git.ts` 与 `git.test.ts` 不存在 |
| AC-8  | `test_resolve_paths` 清单模式     | 清单模式入参返回 `files.written`（经 test config 过滤）中源文件的测试路径推导结果；不再调用 `git diff`                                                                                                                                                                              |
| AC-9  | `archi_check` 清单支持            | `archi_check` 可接收清单文件列表作为被查文件集（staged 模式的 git 依赖移除或降级）                                                                                                                                                                                                  |
| AC-10 | `change_files` 工具               | MCP 工具列表含 `change_files`；append 追加路径；set 覆写净状态；对受保护文件的非法操作被拒绝                                                                                                                                                                                        |
| AC-11 | evaluator 三态对账                | implementation-evaluator/test-gen-evaluator 提示词按三态对账（计划有 actual 无文件不存在 → 硬 fail；文件存在 → 良性漏记照常核对；额外删除从严）；acceptance/code-review 保留 git diff 仅作观察辅助                                                                                  |
| AC-12 | 模板与 skill 文案                 | `design.md.template` 含「删除文件」子节；proposal 模板变更范围支持删除；`phase-implement`/`phase-test-gen` SKILL.md 与 `implementation-generator.md` 文案不再把 git diff 当范围权威，generator 注明规范还原动作                                                                     |
| AC-13 | 条目来源审计                      | subagent 触发的写/删事件，其清单条目携带该 subagent 的 `agent_type`；主会话事件与 `change_files` 补录条目无该字段（append 已存在路径保留既有来源）；同路径重写后来源为最后写入者；突变 scope / `test_resolve_paths` / `archi_check` / evaluator 对账不读取该字段                    |

---

## 风险

| 风险                                                                      | 影响                                  | 概率       | 缓解措施                                                                                                                                                   |
| ------------------------------------------------------------------------- | ------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| mid-subagent 工具事件的 `session_id` 共享仅是强指标、文档未明文           | 记录器归账错 change 或漏归账          | 中         | 实现期先用临时 log hook 实测一轮工作流；若不共享，退化为"暂存-认领"模式（hook 按 session_id 暂存事件流，消费命令认领合并）                                 |
| shell 写命令解析长尾漏记（python/node 豁免、node 进程内部写、MCP 写文件） | 突变范围静默缩圈；actual 缺计划内文件 | 中         | write 侧三态对账把"计划有 actual 无 + 文件存在"判为良性漏记；`change_files` 手动补录；`implementation-generator` 把 `git restore` 定为规范还原动作压缩长尾 |
| 折叠语义对 mv 往返、重写还原等场景只保证存在性近似（错但安全）            | 清单与真值有偏差                      | 中         | 偏差方向全落在 overstate，由内容核对 + 突变去噪 + 文件系统裁判兜底；提示词规范还原形态                                                                     |
| 多 session 并发读-改-写同一 `workflow.json`                               | 清单条目丢失                          | 低         | 本期标注不解决；工作流为单 session 串行，`change_files` set 语义可修复                                                                                     |
| 旧 change（无 `files` 字段）不兼容                                        | 消费方报错                            | 高（必然） | 硬报错并指引重建，不做 git 回退；上线时点由用户控制                                                                                                        |
| Cursor 侧 PostToolUse hook 支持不全                                       | 非 Claude 环境无 actual               | 高         | `change_files` 手动补录为既定兜底；canonical 文案如实标注作用范围                                                                                          |
| 突变去噪需逐文件对比 HEAD 内容                                            | test-execution 耗时增加               | 低         | 仅对 `written` 列表对比，量级为 change 内文件数；实现可用 `git show HEAD:<path>` 缓存                                                                      |

---

## 过程

### 决策

| 问题                               | 决策                                                                        | 理由                                                                                                     | 备选方案                                                 |
| ---------------------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| 清单记录方式                       | hook 被动记录                                                               | 基础设施不可遗忘；node 进程内部写对 shell 解析不可见 → 天然过滤 build 产物                               | agent 主动调 MCP 注册（可忘、多一次调用）                |
| current-change 标记                | session 注册表，与 `phase_next` 既有 `run_id` 机制合并                      | `phase_next` MCP 调用本身触发 PostToolUse，事件内同时可见 `session_id` 与 `change`；协议零改动           | workflow.json 存 current-change 键（跨 session 污染）    |
| git diff 去留                      | 范围判定全走清单；evaluator 可留 git diff 作纯观察辅助                      | 观察内容 ≠ 判定范围；review 被删内容仍需 diff                                                            | 全禁 git（丢失 review 视角）                             |
| 旧 change 兼容                     | 硬报错，指引重建                                                            | 双轨回退（git 兜底）永久化复杂度                                                                         | 读缺失字段时回退 git diff                                |
| 还原/净归零语义                    | 三条对称折叠规则；不引入 baseline 快照                                      | 路径快照区分不了内容级还原；折叠只多记不漏记，多记方向有既有兜底通道                                     | checkpoint/内容快照（超出清单职责）                      |
| MCP 工具写文件（`archi_write` 等） | 不做任何记录                                                                | 产出全在 `openspec/` 排除范围内且无消费方需要；白名单特判成本实收益零                                    | recorder 解析特定 MCP 工具的 tool_input                  |
| actual 分桶                        | `written`/`deleted` 两个路径列表                                            | delete 是另一种事实必须分桶；桶内不按 add/modify 细分（条目可携带可选来源 `agent_type`，形态 design 定） | 单列表带 op 标签 / 按 add-modify 三桶                    |
| delete 漏记的定位                  | 优雅降级：design × 文件系统才是权威核对                                     | `actual.deleted` 只用于发现声明外的删除，不影响判定                                                      | 要求 hook 捕获全量删除（不可行：PostToolUse 时目录已删） |
| 条目来源审计                       | 记录：条目携带 subagent `agent_type`，主会话/手动补录省略，重写取最后写入者 | 事件 stdin 天然携带，零探测成本；排查归账错误/范围超集免翻 transcript；仅审计，消费方不依赖              | 不记录，取证走 transcript 兜底                           |

### 待决问题（留给 design）

- session 注册表的存储位置、格式与清理策略（change 归档/会话结束时的 TTL）
- `extractWriteTargets` / `extractFileOps` 的具体正则覆盖面清单与 revert 识别精度（`git restore --source=<commit>` 归 write 的判定）
- 突变去噪的 HEAD 内容对比实现（`git show` vs 缓存）；未传 `--change` 时突变不限圈（现状默认）的语义由 design 确认
- `test_resolve_paths` 清单模式的入参形态（`modules` 字面量命名与 change 参数如何传入）
- `archi_check` 接收清单的参数形态（复用 `files` 参数 vs 新增参数；staged 模式去留）
- NotebookEdit 事件的 `tool_input` 形状确认（`notebook_path`）

---
