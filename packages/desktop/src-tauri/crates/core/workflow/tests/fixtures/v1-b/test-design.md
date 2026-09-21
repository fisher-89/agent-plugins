# 测试设计: workflow-file-inventory

> **日期**: 2026-09-17

---

## 验收范围

| AC ID | 验收条件                                                                                                                                                                                                                                                              | 测试类型                 | 被测文件或模块                                                                                                                                                                                                                                                                                 |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-1  | 新建 change 的 `workflow.json` 含 `files: { written: [], deleted: [] }`；`workflowFileSchema` 校验通过；既有 `workflow_type`/`created` 行为不变                                                                                                                       | 单元测试                 | `plugins/dev-team/bin/src/commands/change-create.test.ts`（`workflowFileSchema` 断言内嵌于消费方测试；`bin/src/schemas/**` 在测试配置 excludes 中，无独立 schema 测试文件）                                                                                                                    |
| AC-2  | 经 Write/Edit/NotebookEdit 写 `src/foo.ts` 后当前 change 的 `files.written` 含该路径；经 Bash `rm` 删除后 `files.deleted` 含该路径；`openspec/**` 与 `workflow.json` 自身不入清单                                                                                     | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/hooks.test.ts`；`plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts`；`plugins/dev-team/build/__tests__/hooks-post-tool-use-registry/hooks-post-tool-use-registry.test.ts`（hook 注册组装面）                                  |
| AC-3  | hook 观察到 `phase_next` 调用事件后建立 `session_id → change` 映射；同 session 的 subagent 工具事件归账到该 change；不同 session 互不串账                                                                                                                             | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/lib/session-registry.test.ts`；`plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts`；`plugins/dev-team/build/__tests__/hooks-post-tool-use-registry/hooks-post-tool-use-registry.test.ts`（`__MCP:phase_next__` matcher 组装） |
| AC-4  | `>`/`>>`/`tee`/`Set-Content` 等归 write；`rm`/`Remove-Item`/`git rm` 归 delete；`git restore`/`git checkout --` 归 revert；`mv`/`Move-Item` 产生 delete(旧)+write(新) 双条目；write→revert 折叠为净 untouched，delete→write 折叠为 written；`rm -rf dir` 记录目录路径 | 单元测试                 | `plugins/dev-team/bin/src/lib/file-inventory.test.ts`（`foldFileOps` 折叠规则）；`plugins/dev-team/bin/src/hooks.test.ts`（`extractFileOps` 为模块内部符号，经 `runProtectFiles`/`runRecordFiles` 黑盒验证）                                                                                   |
| AC-5  | `git stash` / `git clean` 类命令被 deny；`rm <受保护路径>` 被 deny；`git restore` 对 `openspec/changes/**` 工作流产物的还原被 deny                                                                                                                                    | 单元测试                 | `plugins/dev-team/bin/src/hooks.test.ts`                                                                                                                                                                                                                                                       |
| AC-6  | backtrack 到 `implement` 或更早后，该 change 的 `files` 被清空；backtrack 到 implement 之后的 phase 不清空                                                                                                                                                            | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/commands/backtrack.test.ts`；`plugins/dev-team/bin/__tests__/inventory-backtrack-clear/inventory-backtrack-clear.test.ts`                                                                                                                                            |
| AC-7  | 传入 `--change=<change-name>` 时突变 scope 自动来自该 change 的 `files.written` ∩（内容 ≠ HEAD）；无需 `--mutation-scope`（不引入）、`--mutation-diff-only` 已删除；`--skip-mutation` 仍可跳过；`files` 缺失的旧 change 硬报错；`lib/git.ts` 与 `git.test.ts` 不存在  | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/commands/test-execution.test.ts`；`plugins/dev-team/bin/src/cli.test.ts`；`plugins/dev-team/bin/__tests__/mutation-scope-inventory/mutation-scope-inventory.test.ts`                                                                                                 |
| AC-8  | 清单模式入参返回 `files.written`（经 test config 过滤）中源文件的测试路径推导结果；不再调用 `git diff`                                                                                                                                                                | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`；`plugins/dev-team/bin/__tests__/inventory-test-path-resolution/inventory-test-path-resolution.test.ts`                                                                                                                         |
| AC-9  | `archi_check` 可接收清单文件列表作为被查文件集（staged 模式的 git 依赖移除）                                                                                                                                                                                          | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/lib/c4-cross-ref.test.ts`；`plugins/dev-team/bin/__tests__/archi-check-inventory/archi-check-inventory.test.ts`                                                                                                                                                      |
| AC-10 | MCP 工具列表含 `change_files`；append 追加路径；set 覆写净状态；对受保护文件的非法操作被拒绝                                                                                                                                                                          | 单元测试                 | `plugins/dev-team/bin/src/commands/change-files.test.ts`；`plugins/dev-team/bin/src/mcp.test.ts`（注册断言）                                                                                                                                                                                   |
| AC-11 | implementation-evaluator/test-gen-evaluator 提示词按三态对账；acceptance/code-review 保留 git diff 仅作观察辅助                                                                                                                                                       | 人工核对（见不可测试项） | `plugins/dev-team/agents/implementation-evaluator.md`、`test-gen-evaluator.md`、`test-gen-generator.md`、`acceptance-evaluator.md`、`code-review-evaluator.md`                                                                                                                                 |
| AC-12 | `design.md.template` 含「删除文件」子节；proposal 模板变更范围支持删除；SKILL.md 与 `implementation-generator.md` 文案不再把 git diff 当范围权威                                                                                                                      | 人工核对（见不可测试项） | `plugins/dev-team/templates/artifacts/design.md.template`、`proposal.md.template`、`skills/phase-implement/SKILL.md`、`skills/phase-test-gen/SKILL.md`、`agents/implementation-generator.md`                                                                                                   |
| AC-13 | subagent 条目携带 `agent_type`；主会话事件与 `change_files` 补录无该字段；同路径重写 last-writer-wins；突变 scope / `test_resolve_paths` / `archi_check` / evaluator 对账不读取该字段                                                                                 | 单元测试 + 集成测试      | `plugins/dev-team/bin/src/lib/file-inventory.test.ts`；`plugins/dev-team/bin/src/hooks.test.ts`；`plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts`                                                                                                    |

> 说明：
>
> - 测试配置（`openspec/config.json` tests.includes）范围为 `bin/src/**` 且 excludes `bin/src/schemas/**`，因此 `workflow.schema.ts` / `archi-check.schema.ts` / `test-resolve-paths.schema.ts` / `change-files.schema.ts` / `schemas/index.ts` 无独立测试文件，其断言按既有模式内嵌于消费方测试（`change-create.test.ts` 校验 `workflowFileSchema`、`mcp.test.ts` 校验各 input schema、`change-files.test.ts` 校验 `changeFilesInputSchema`）。
> - `lib/git.ts` 与 `git.test.ts` 随本变更整体删除：`git.test.ts` 上既有用例全部废弃；防回归由「残留 import 即编译失败」与去噪/退场 spy 断言（见 test-execution / test-resolve-paths / c4-cross-ref 章节）结构性承载。

---

## 单元测试

<!-- 集成测试框架识别结果（test_detect_frameworks）：vite-plus（vitest 语义，断言库为 vite-plus/test 的 expect），配套 Stryker 突变测试与 istanbul 覆盖率；既有测试模式为与源文件同目录共置的 `*.test.ts`，以下单元测试文件路径均来自 test_resolve_paths 的 unit_tests 映射。 -->

### plugins/dev-team/bin/src/lib/file-inventory.ts -> plugins/dev-team/bin/src/lib/file-inventory.test.ts

#### 待测功能

- readFileInventory(changeDir: string): FileInventory — 读 `workflow.json` 并校验 `files` 字段；文件缺失 / JSON 非法 / 根非对象 / schema 不合法 / 无 `files` 均抛错（文案含"该 change 创建于文件清单机制之前，请重建"）
- foldFileOps(inventory: FileInventory, ops: FileOp[]): FileInventory — 纯函数：write/delete/revert 三条对称折叠规则 + `source` 旁挂来源维护
- writeFileInventory(changeDir: string, files: FileInventory): void — 写回 `workflow.json.files`；保留 `workflow_type` / `created` / `eval` / 未知键；2 空格缩进 + 末尾换行；文件不存在抛错且绝不创建
- 类型 `FileInventory`（`{ written: string[]; deleted: string[]; source?: Record<string, string> }`）与 `FileOp`（`{ op: 'write' \| 'delete' \| 'revert'; path: string; agentType?: string }`）

#### 用例

| 测试对象           | 路径类型 | 测试条件                                                                                                                                             | 迭代类型 |
| ------------------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| readFileInventory  | 正向     | 合法 `workflow.json`（workflow_type + created + eval 数组 + 未知键 + files 双数组 + source 映射）返回完整 FileInventory，written/deleted/source 透传 | 新增     |
| readFileInventory  | 正向     | `files` 缺 `source` 键时返回 source 为 undefined（不补默认空对象，保证写回不新增键）                                                                 | 新增     |
| readFileInventory  | 异常     | changeDir 下无 `workflow.json` → 抛错含 change_create 指引，且不创建任何文件或目录                                                                   | 新增     |
| readFileInventory  | 异常     | `workflow.json` JSON 非法（截断/含注释）→ 抛解析错误                                                                                                 | 新增     |
| readFileInventory  | 异常     | 根元素为数组 / 字符串 / null → 抛"根元素必须是对象"类错误                                                                                            | 新增     |
| readFileInventory  | 异常     | 合法 workflow.json 但无 `files` 字段（机制前旧 change）→ 抛错文案含"该 change 创建于文件清单机制之前，请重建"                                        | 新增     |
| readFileInventory  | 异常     | `files.written` / `files.deleted` 为字符串 / 对象 / null 等非数组 → schema 校验抛错                                                                  | 新增     |
| readFileInventory  | 异常     | `files` 内出现未知额外字段 → looseObject 语义不报错（与 workflow.schema 未知键保留策略一致）                                                         | 新增     |
| readFileInventory  | 边界     | `files: { written: [], deleted: [] }` 空净状态合法返回                                                                                               | 新增     |
| readFileInventory  | 边界     | written 含空字符串路径 / 超长路径（>1000 chars）/ 含空格、中文、emoji 的路径 → 原样透传不做语义裁剪                                                  | 新增     |
| foldFileOps        | 正向     | 空净状态 + [write(P)] → written=[P]、deleted=[]（AC-4）                                                                                              | 新增     |
| foldFileOps        | 正向     | 空净状态 + [delete(P)] → deleted=[P]、written=[]（AC-4）                                                                                             | 新增     |
| foldFileOps        | 正向     | written=[P] + [revert(P)] → 双桶均无 P（write→revert 净 untouched，AC-4）                                                                            | 新增     |
| foldFileOps        | 正向     | deleted=[P] + [write(P)] → P 移出 deleted 进入 written（delete→write 折叠为 written，单桶不变量，AC-4）                                              | 新增     |
| foldFileOps        | 正向     | written=[P] + [delete(P)] → P 移出 written 进入 deleted                                                                                              | 新增     |
| foldFileOps        | 正向     | [delete(old), write(new)]（mv 双条目）→ old 在 deleted、new 在 written（AC-4）                                                                       | 新增     |
| foldFileOps        | 正向     | op 携带 agentType → source[path]=agentType；同路径再次 write 携带不同 agentType → 覆写（last-writer-wins，AC-13）                                    | 新增     |
| foldFileOps        | 正向     | 已有 source[path] 的路径被不带 agentType 的 op 重写 → source[path] 清除（AC-13）                                                                     | 新增     |
| foldFileOps        | 正向     | 条目被折叠移出双桶（revert / write-后-delete）→ 对应 source 键随之移除（AC-13）                                                                      | 新增     |
| foldFileOps        | 异常     | 未知 op 值（如 'copy'）→ 抛错（枚举防御，schema 层已约束的前提下运行时兜底）                                                                         | 新增     |
| foldFileOps        | 边界     | ops=[] → 返回与输入等价的净状态深拷贝，且不修改入参 inventory（纯函数不变量）                                                                        | 新增     |
| foldFileOps        | 边界     | 长序列折叠（write→delete→write→revert 等 10+ 步）终态与逐步语义一致（顺序确定性）                                                                    | 新增     |
| foldFileOps        | 边界     | 同一 op 重复出现（[write(P), write(P)]）→ written 单条目（去重幂等）                                                                                 | 新增     |
| foldFileOps        | 边界     | 目录路径（`rm -rf src/lib` 记录目录路径本身）与文件路径同规则折叠，不做 glob 展开（AC-4）                                                            | 新增     |
| foldFileOps        | 边界     | 折叠结果中任意路径不同时存在于 written 与 deleted（单桶不变量扫描断言）                                                                              | 新增     |
| writeFileInventory | 正向     | 写回后 `files` 更新；`workflow_type` / `created` / `eval` / 未知键原样保留                                                                           | 新增     |
| writeFileInventory | 正向     | 输出 2 空格缩进 + 末尾换行（与 eval-json 写盘纪律同构）                                                                                              | 新增     |
| writeFileInventory | 正向     | source 非空时写为旁挂映射（仅审计字段）；source 为 undefined 时不写 `source` 键                                                                      | 新增     |
| writeFileInventory | 异常     | `workflow.json` 不存在 → 抛错、不创建文件与目录（绝不创建契约）                                                                                      | 新增     |
| writeFileInventory | 异常     | `workflow.json` JSON 非法 → 抛错且磁盘内容逐字节不变                                                                                                 | 新增     |
| writeFileInventory | 边界     | 清空写回（written=[] deleted=[]）成功（backtrack 清空与记录器空折叠共用通道）                                                                        | 新增     |

#### Mock策略

| Mock主体                  | Mock方案                                                                                                                       | 应用场景                                        |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------- |
| 文件系统（workflow.json） | 不 mock：mkdtempSync 临时目录 + 手工写入 / runChangeCreate 生成 fixture（同 change-create.test.ts 模式），读写与格式断言走真盘 | readFileInventory / writeFileInventory 全部用例 |
| —（foldFileOps 纯函数）   | 无外部依赖，不需要 mock                                                                                                        | foldFileOps 全部用例                            |

---

### plugins/dev-team/bin/src/lib/session-registry.ts -> plugins/dev-team/bin/src/lib/session-registry.test.ts

#### 待测功能

- bindSession(projectRoot: string, sessionId: string, change: string): void — 建立/更新 `session_id → change` 绑定并刷新 `updatedAt`；顺带清理 TTL（24h）过期条目
- lookupChange(projectRoot: string, sessionId: string): string \| null — 查绑定；未命中返回 null（调用方静默丢弃事件）

#### 用例

| 测试对象                   | 路径类型 | 测试条件                                                                                                         | 迭代类型 |
| -------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------- | -------- |
| bindSession + lookupChange | 正向     | bind(s1, change-a) 后 lookup(s1) 返回 'change-a'（AC-3）                                                         | 新增     |
| bindSession                | 正向     | 同 session 先绑 change-a 再绑 change-b → lookup 返回 change-b 且 updatedAt 刷新                                  | 新增     |
| bindSession + lookupChange | 正向     | s1→change-a、s2→change-b 并存，各自命中互不串账（AC-3）                                                          | 新增     |
| bindSession                | 正向     | 注册表落盘于 `os.tmpdir()/dev-team-hooks/<sha256(projectRoot) 前 16 位>.json`；不同 projectRoot 哈希隔离互不可见 | 新增     |
| lookupChange               | 异常     | 未绑定 session → 返回 null 且不抛错                                                                              | 新增     |
| lookupChange               | 异常     | 注册表文件不存在 → 返回 null 不抛错                                                                              | 新增     |
| lookupChange               | 异常     | 注册表文件 JSON 非法 / 根非对象 → 返回 null 不抛错                                                               | 新增     |
| lookupChange               | 边界     | sessionId 为空字符串 → 与未绑定同语义返回 null（键缺失）                                                         | 新增     |
| bindSession                | 边界     | 预置 updatedAt 超 24h 的旧条目后新 bind → 旧条目被清理、未过期条目保留                                           | 新增     |
| bindSession                | 边界     | TTL 阈值边界（恰好 24h / 略小于 24h）→ 清理判定方向与实现常量一致                                                | 新增     |
| bindSession                | 边界     | 损坏注册表（根非对象）后 bind → 自愈重写为合法结构                                                               | 新增     |

#### Mock策略

| Mock主体       | Mock方案                                                                                                                                    | 应用场景 |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| node:os tmpdir | vi.mock `os.tmpdir` 指向 mkdtempSync 临时目录，隔离真实系统临时目录并避免并发测试互扰（不使用 process.chdir，遵循 Stryker worker 线程约定） | 全部用例 |
| 文件系统       | 真实 fs 读写注册表文件（落盘结构直接读盘断言）                                                                                              | 全部用例 |

### plugins/dev-team/bin/src/hooks.ts -> plugins/dev-team/bin/src/hooks.test.ts

#### 待测功能

- runProtectFiles(): PreToolUse 保护子命令（扩展：delete 双分类拦截、批量还原拦截、`extractFileOps` 提取路径精确匹配保护 glob）
- runRecordFiles(): 新增 PostToolUse 记录器子命令（`phase_next` 事件建绑 → 查表归账 → 路径归一化/自污染过滤 → 折叠 → 写清单；异常只走 stderr、exit 0）
- runStaticCheck(): SubagentStop 静态检查子命令（本变更不改动，回归保留）
- captureStderr(): stderr 捕获工具（回归保留）
- main(): 子命令 switch 分发（新增 `'record-files'` 分支）
- 模块内部符号 extractFileOps(command): FileOp[]（不出模块导出，经 `runProtectFiles` / `runRecordFiles` 黑盒覆盖三态提取）

#### 用例

| 测试对象                        | 路径类型 | 测试条件                                                                                                                                                  | 迭代类型 |
| ------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| record-files（phase_next 建绑） | 正向     | stdin 为全名 `mcp__plugin_dev-team_dev-team__phase_next` 事件（tool_input.change=change-a）→ bindSession(session_id, 'change-a') 被调用且不写清单（AC-3） | 新增     |
| record-files（phase_next 建绑） | 正向     | 事件携带 agent_type（subagent 触发）→ 绑定照常建立（绑定不依赖 agent_type）                                                                               | 新增     |
| record-files（phase_next 建绑） | 异常     | phase_next 事件缺 tool_input.change → 静默：stderr 诊断、exit 0、不建立绑定、不写清单                                                                     | 新增     |
| record-files（phase_next 建绑） | 异常     | stdin 非法 JSON / 空输入 → 静默丢弃、exit 0、不产生任何写副作用                                                                                           | 新增     |
| record-files（phase_next 建绑） | 边界     | tool_name 为其他 server 的同名后缀（如 `mcp__other__phase_next`）→ 不建立绑定（前缀+后缀匹配精确性，AC-3）                                                | 新增     |
| record-files（归账写清单）      | 正向     | 已绑定 session 的 Write 事件 file_path=src/foo.ts → 清单 written 含 src/foo.ts（相对项目根 POSIX 风格，AC-2）                                             | 新增     |
| record-files（归账写清单）      | 正向     | Edit 事件 file_path / NotebookEdit 事件 notebook_path → 同归账语义（AC-2）                                                                                | 新增     |
| record-files（归账写清单）      | 正向     | Bash `rm src/old.ts` → deleted 含 src/old.ts；PowerShell `Remove-Item` 同语义（AC-2）                                                                     | 新增     |
| record-files（归账写清单）      | 正向     | Bash `git restore src/foo.ts` → 折叠为净 untouched（written/deleted 均无该路径，AC-4）                                                                    | 新增     |
| record-files（归账写清单）      | 正向     | 同 session 事件序列 write→delete→write → 净状态按折叠规则收敛（与 file-inventory 单元语义一致）                                                           | 新增     |
| record-files（归账写清单）      | 正向     | 事件携带 agent_type → 清单 source[path]=agent_type；同路径随后被无 agent_type 的主会话事件重写 → source 清除（AC-13）                                     | 新增     |
| record-files（归账写清单）      | 异常     | 未绑定 session 的写事件 → 静默丢弃，workflow.json 不变（AC-3 前置语义）                                                                                   | 新增     |
| record-files（归账写清单）      | 异常     | 目标 change 缺 workflow.json / 无 files → 静默（stderr、exit 0），不产生半写状态                                                                          | 新增     |
| record-files（归账写清单）      | 边界     | 绝对路径（含 Windows 反斜杠）相对 getProjectDir() 归一化为 POSIX 相对路径                                                                                 | 新增     |
| record-files（归账写清单）      | 边界     | 路径越出项目根（../outside.ts）→ 丢弃不入清单                                                                                                             | 新增     |
| record-files（归账写清单）      | 边界     | `openspec/**` 路径与 `workflow.json` 自身路径 → 排除不入清单（自污染排除，AC-2）                                                                          | 新增     |
| record-files（归账写清单）      | 边界     | 归账全程任何内部异常 → exit 0 且 stdout 无阻塞决策输出（不阻塞工具调用）                                                                                  | 新增     |
| protect-files（扩拦截）         | 正向     | `git stash` / `git stash pop` / `git clean -fd`（无路径限定）→ deny（AC-5）                                                                               | 新增     |
| protect-files（扩拦截）         | 正向     | `git clean <openspec 下路径>` → deny；`git clean -n` / `--dry-run` 只读形态 → allow（AC-5）                                                               | 新增     |
| protect-files（扩拦截）         | 正向     | `git stash list` / `git stash show` 只读形态 → allow（AC-5）                                                                                              | 新增     |
| protect-files（扩拦截）         | 正向     | `git restore openspec/changes/x/design.md` → deny；`git restore src/foo.ts` → allow（工作流产物拦、源码放行，AC-5）                                       | 新增     |
| protect-files（扩拦截）         | 正向     | `rm <受保护路径>` / `Remove-Item <受保护路径>` → deny（删保护，AC-5）                                                                                     | 新增     |
| protect-files（扩拦截）         | 正向     | `mv <受保护路径> <新路径>` → deny（delete(旧)+write(新) 双条目任一命中即拦）                                                                              | 新增     |
| protect-files（扩拦截）         | 边界     | `git restore --source=<commit> <path>` / `git checkout <commit> -- <paths>` 归 write 分类（还原到历史版本为内容写入语义，AC-4）                           | 新增     |
| protect-files（扩拦截）         | 边界     | 既有 write 语义回归：`>` `>>` `tee` `Set-Content` 命中保护 glob 仍 deny（既有用例保留）                                                                   | 新增     |
| protect-files（扩拦截）         | 边界     | python/node 命令豁免语义不变：`python -c "…rm…"` 类命令不拦截（fail-open 语义回归）                                                                       | 新增     |
| main（子命令分发）              | 正向     | argv[2]='record-files' → 分发执行 runRecordFiles                                                                                                          | 新增     |
| main（子命令分发）              | 异常     | 未知子命令 → stderr 提示 + exit 1（既有语义回归）                                                                                                         | 新增     |

#### Mock策略

| Mock主体                                                | Mock方案                                                                                                                         | 应用场景                           |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| node:fs.readFileSync（stdin fd 0）                      | vi.mock 返回按用例注入的 hook 事件 JSON（既有 hooks.test.ts 注入模式）                                                           | 全部 describe                      |
| ./lib/config readConfig                                 | 固定返回 write_protection patterns（叠加内置 glob 场景）                                                                         | protect-files（扩拦截）describe    |
| ./lib/project-root getProjectDir                        | mock 指向 mkdtempSync 临时项目根（不用 process.chdir，遵循 Stryker worker 约定）                                                 | 全部 describe                      |
| ./lib/session-registry                                  | vi.mock `bindSession` / `lookupChange` 为 spy，断言建绑与查表路由                                                                | record-files 两个 describe         |
| ./lib/file-inventory                                    | vi.mock `readFileInventory` / `writeFileInventory` 为 spy（`foldFileOps` 用真实实现），聚焦 hooks 侧编排；真盘链路由集成测试覆盖 | record-files（归账写清单）describe |
| process.stdout/stderr.write、process.exit、process.argv | spy / 临时替换（既有模式）                                                                                                       | 全部 describe                      |

---

### plugins/dev-team/bin/src/commands/change-files.ts -> plugins/dev-team/bin/src/commands/change-files.test.ts

#### 待测功能

- runChangeFiles(options: ChangeFilesInput): ChangeFilesOutput — `op: 'append'` 按折叠规则逐路径合并（去重、保留既有来源、新路径无来源）；`op: 'set'` 整体覆写指定桶并清理被覆写路径的 source 条目；校验 change 存在且 `workflow.json` 合法且含 `files`（缺失硬报错指引重建）；输出操作后净状态 `{ written, deleted }`

#### 用例

| 测试对象                | 路径类型 | 测试条件                                                                                                                            | 迭代类型 |
| ----------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------- | -------- |
| runChangeFiles — append | 正向     | append written 新路径 → 返回净状态含之且 workflow.json 落盘（AC-10）                                                                | 新增     |
| runChangeFiles — append | 正向     | append 已存在路径 → 去重不重复且既有 source 保留（append 已存在路径保留既有来源，AC-13）                                            | 新增     |
| runChangeFiles — append | 正向     | append deleted 路径 → written 移除 + deleted 加入（折叠语义合并，AC-10）                                                            | 新增     |
| runChangeFiles — append | 边界     | append 新路径 → 不写 source 键（手动补录无来源，AC-13）                                                                             | 新增     |
| runChangeFiles — set    | 正向     | set written 覆写 → 指定桶整体替换、被移除路径的 source 条目清理（AC-10）                                                            | 新增     |
| runChangeFiles — set    | 正向     | set deleted 覆写 → 同语义（AC-10）                                                                                                  | 新增     |
| runChangeFiles — set    | 边界     | set `written: []` → 清空该桶（净状态显式修正通道，兼作 hook 认不出的还原修正）                                                      | 新增     |
| runChangeFiles — 异常   | 异常     | change 不存在 → 抛错                                                                                                                | 新增     |
| runChangeFiles — 异常   | 异常     | workflow.json 非法 / 缺 `files` 字段 → 硬报错含"该 change 创建于文件清单机制之前，请重建"指引                                       | 新增     |
| runChangeFiles — 异常   | 异常     | `written` 与 `deleted` 均缺省 → changeFilesInputSchema（至少其一 refine）拒绝                                                       | 新增     |
| runChangeFiles — 异常   | 异常     | 路径命中受保护范围（`workflow.json` 自身 / `openspec/config.json` / 自污染排除范围）→ 拒绝入清单（AC-10）                           | 新增     |
| runChangeFiles — 边界   | 边界     | op 为非法枚举值（如 'merge'）→ schema 拒绝                                                                                          | 新增     |
| runChangeFiles — 边界   | 边界     | 路径为空字符串 / 绝对路径 / 反斜杠形态 → 与 changeFilesInputSchema（相对项目根 POSIX 风格）校验行为一致（拒绝或归一化，随实现收敛） | 新增     |
| runChangeFiles — 边界   | 边界     | 批量 100+ 路径一次性 append → 全部入净状态且去重稳定（超大列表）                                                                    | 新增     |

#### Mock策略

| Mock主体                                         | Mock方案                                                                                                              | 应用场景        |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- | --------------- |
| 文件系统                                         | 不 mock：临时项目内 runChangeCreate 创建 fixture change 后直接操作 workflow.json（同 change-create.test.ts 真盘模式） | 全部用例        |
| changeFilesInputSchema / changeFilesOutputSchema | 经 schemas/index re-export 直接 safeParse 断言（schema 文件在测试 excludes 中，断言内嵌消费方测试）                   | 异常 / 边界用例 |

---

### plugins/dev-team/bin/src/commands/change-create.ts -> plugins/dev-team/bin/src/commands/change-create.test.ts

#### 待测功能

- runChangeCreate(name, projectRoot, workflowType): { name, path } — 签名不变；写入对象增 `files: { written: [], deleted: [] }` 初始空净状态，键序 `workflow_type` → `created` → `files`

#### 用例

| 测试对象                       | 路径类型 | 测试条件                                                                                                                   | 迭代类型 |
| ------------------------------ | -------- | -------------------------------------------------------------------------------------------------------------------------- | -------- |
| runChangeCreate — files 初始化 | 正向     | 新建 workflow.json 含 `files: { written: [], deleted: [] }`（AC-1）                                                        | 新增     |
| runChangeCreate — files 初始化 | 正向     | 键序为 workflow_type → created → files（JSON.stringify 插入序断言）                                                        | 新增     |
| runChangeCreate — files 初始化 | 正向     | 产出可被 workflowFileSchema 解析，files 字段合法且 source 缺省（AC-1）                                                     | 新增     |
| runChangeCreate — files 初始化 | 边界     | 4 值 workflow_type 枚举（requirement/bug-fix/refactor/test-only）逐一创建均含 files                                        | 新增     |
| runChangeCreate — 键集合断言   | 废弃     | 既有「Object.keys 仅为 ['workflow_type','created'] 两键」断言删除（files 键加入后不再成立）                                | 废弃     |
| runChangeCreate — 键集合断言   | 新增     | 替换断言：Object.keys 为 ['workflow_type','created','files'] 且仍无 `eval` 键、不创建 `eval.json`（AC-1 既有契约不变部分） | 新增     |
| runChangeCreate — 拒绝已存在   | 边界     | 已存在 change 拒绝时不覆写、不修补已有 files（幂等回归扩展）                                                               | 新增     |
| runChangeCreate — 异常回归     | 异常     | 非法 kebab-case 名称 / 超长名称（>128）的既有异常用例保留（files 不影响校验路径）                                          | 新增     |

#### Mock策略

| Mock主体 | Mock方案                                                                              | 应用场景 |
| -------- | ------------------------------------------------------------------------------------- | -------- |
| 文件系统 | 不 mock：mkdtempSync 临时目录真实创建（既有模式），workflow.json 落盘内容直接读盘断言 | 全部用例 |

---

### plugins/dev-team/bin/src/commands/backtrack.ts -> plugins/dev-team/bin/src/commands/backtrack.test.ts

#### 待测功能

- runBacktrack(options: BacktrackOptions): BacktrackResult — 签名不变；新增清空规则：`backtrack_to` 在同一 phase 表中的索引 ≤ `implement` 的索引时，重置 `files` 为空净状态；预检 `files` 存在（缺失即抛"机制前旧 change 请重建"）先行于 eval 读改写回，保证评估条目不部分生效；`implement` 不在 phase 表中的工作流（test-only）不触发清空

#### 用例

| 测试对象                      | 路径类型 | 测试条件                                                                                                                       | 迭代类型 |
| ----------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------ | -------- |
| runBacktrack — files 清空规则 | 正向     | backtrack 到 implement → writeFileInventory 以空净状态被调用（AC-6）                                                           | 新增     |
| runBacktrack — files 清空规则 | 正向     | backtrack 到 implement 之前的 phase（如 proposal / dev-design）→ 同样清空（AC-6）                                              | 新增     |
| runBacktrack — files 清空规则 | 正向     | backtrack 到 implement 之后的 phase（如 test-execution）→ 不调用 writeFileInventory（不清空，AC-6）                            | 新增     |
| runBacktrack — files 清空规则 | 正向     | 清空调用发生在 writeEvalJson 之后（执行序断言：评估条目先落盘再清清单）                                                        | 新增     |
| runBacktrack — files 清空规则 | 边界     | 目标 phase 恰为 implement（索引相等，≤ 判定含等号）→ 清空                                                                      | 新增     |
| runBacktrack — files 清空规则 | 边界     | test-only 工作流（phase 表无 implement）→ 不触发清空（无 writeFileInventory 调用）                                             | 新增     |
| runBacktrack — 预检           | 异常     | readFileInventory 抛"机制前旧 change 请重建" → 整体抛错，且 readEvalJson / writeEvalJson 未被调用（评估条目不部分生效）        | 新增     |
| runBacktrack — 回归           | 边界     | 既有语义回归：backtrack_to / backtrack_reason 写入、markPhaseStale 传播、回溯到未来 phase 抛错、无评估条目抛错（既有用例保留） | 新增     |

#### Mock策略

| Mock主体                            | Mock方案                                                                                                        | 应用场景                       |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| ../lib/eval-json                    | vi.mock `readEvalJson` / `writeEvalJson`（既有模式），保留 not-called 断言能力                                  | 全部 describe                  |
| ../lib/file-inventory               | vi.mock `readFileInventory` / `writeFileInventory` 为 spy（新增）：清空规则聚焦调用编排；真盘语义由集成测试覆盖 | files 清空规则 / 预检 describe |
| ../lib/change、../lib/change-config | vi.mock `getChangeDir` / `getWorkflowType`（既有模式）                                                          | 全部 describe                  |
| ../lib/workflow getPhaseTable       | 不 mock（真实 phase 表驱动 ≤ implement 判定）                                                                   | files 清空规则 describe        |

---

### plugins/dev-team/bin/src/commands/test-execution.ts -> plugins/dev-team/bin/src/commands/test-execution.test.ts

#### 待测功能

- runTestExecution(options: TestExecutionOptions): Promise\<number\> — options 删 `mutationDiffOnly`；`change` 兼作清单突变 scope 入口：读 `files.written` → 同位源文件反推 → HEAD 内容净归零去噪；`files` 缺失硬报错；未传 `change` 突变不限圈（现状默认）
- interface TestExecutionOptions（change / projectRoot / files / framework / noMutation；`mutationDiffOnly` 移除）

#### 用例

| 测试对象                          | 路径类型 | 测试条件                                                                                                                                               | 迭代类型 |
| --------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| runTestExecution — 清单突变 scope | 正向     | 传 change → readFileInventory(changeDir).written 经同位源文件反推后作为 mutationDiffFiles 传入 executePlanEntry（AC-7）                                | 新增     |
| runTestExecution — 清单突变 scope | 正向     | stdout 诊断行改为清单来源前缀（`mutation scope (change inventory): N files` 形态），不再出现 `--mutation-diff-only:` 前缀                              | 新增     |
| runTestExecution — 去噪           | 正向     | written 文件工作区内容 == HEAD 内容 → 从 scope 剔除（净归零去噪，AC-7）                                                                                | 新增     |
| runTestExecution — 去噪           | 边界     | HEAD 无该文件（新建文件）→ 保留（漏斗只向 overstate 方向，AC-7）                                                                                       | 新增     |
| runTestExecution — 去噪           | 边界     | HEAD 内容读取失败 → 保留（overstate 方向，AC-7）                                                                                                       | 新增     |
| runTestExecution — 惰性           | 边界     | noMutation=true 且传 change → 不读清单、不执行 HEAD 内容对比（惰性解析）                                                                               | 新增     |
| runTestExecution — 惰性           | 边界     | 未传 change → 突变不限圈（现状默认语义），mutationDiffFiles 为 undefined 且不读清单                                                                    | 新增     |
| runTestExecution — 异常           | 异常     | change 的 workflow.json 缺 `files` → 硬报错"该 change 创建于文件清单机制之前，请重建"（AC-7）                                                          | 新增     |
| runTestExecution — 异常           | 异常     | change 不存在 → 报错                                                                                                                                   | 新增     |
| runTestExecution — 边界           | 边界     | `files.written` 为空数组 → 空 scope，不回退任何 git diff 发现路径                                                                                      | 新增     |
| runTestExecution — 边界           | 边界     | written 含测试文件路径 → 反推同位源文件并入 scope（expandMutationDiffWithInferredSources 保留语义）                                                    | 新增     |
| runTestExecution — 回归           | 正向     | `--skip-mutation`（noMutation）跳过突变阶段的既有断言保留（AC-7）                                                                                      | 新增     |
| runTestExecution — git 退场       | 废弃     | `vi.mock('../lib/git')` 与 getGitDiffFiles 相关用例/断言全部移除（模块随本变更删除，残留 import 即编译失败；`git diff HEAD` 不再是文件发现来源，AC-7） | 废弃     |
| runTestExecution — 选项面         | 废弃     | `mutationDiffOnly` 透传断言与 `--mutation-diff-only` 诊断行断言移除（选项已删除）                                                                      | 废弃     |

#### Mock策略

| Mock主体                                                         | Mock方案                                                                                       | 应用场景                          |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------- |
| ./test-detect-frameworks、../lib/test-runner、../lib/test-report | vi.mock（既有模式），隔离真实测试执行与报告生成                                                | 全部 describe                     |
| ../lib/file-inventory readFileInventory                          | vi.mock 返回 fixture 净状态（单元级聚焦 scope 编排；真盘链路由集成测试覆盖）                   | 清单 scope / 去噪 / 异常 describe |
| HEAD 内容对比（只读 git 子进程调用）                             | mock 等价的只读 git 调用出口（execSync 层）返回 fixture 内容；工作区文件用真实临时项目承载对比 | 去噪 describe                     |
| 文件系统                                                         | 真实临时项目（createTempProject 既有模式）承载工作区文件内容                                   | 去噪 describe                     |

---

### plugins/dev-team/bin/src/commands/test-resolve-paths.ts -> plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts

#### 待测功能

- runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult — `modules` union 中 `'git-change'` 字面量替换为 `'change'`，新增可选 `change` 参数；清单模式以 `files.written` 作为 effectiveModules 进入既有 test config 过滤管线；清单读取失败进 `errors` 并 earlyReturn（不抛出）
- interface TestResolvePathsInput（`modules: string[] \| 'change'`；`change?: string`）与 ResolveTestPathsResult

#### 用例

| 测试对象                       | 路径类型 | 测试条件                                                                                                                                  | 迭代类型 |
| ------------------------------ | -------- | ----------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| runTestResolvePaths — 清单模式 | 正向     | modules='change' + change 名 → unit_tests 来自 `files.written` 经 test config 过滤 + exclude 过滤的推导结果（AC-8）                       | 新增     |
| runTestResolvePaths — 清单模式 | 正向     | written 含测试文件 → errors "Path is already a test file"；非源文件 → "Not a testable source file"（既有管线语义不变）                    | 新增     |
| runTestResolvePaths — 清单模式 | 边界     | `files.written` 为空数组 → unit_tests=[] 且无致命 errors（不触发 config 全库扫描，design 既定）                                           | 新增     |
| runTestResolvePaths — 清单模式 | 边界     | written 含越出项目根路径 → errors "Path is outside project root"                                                                          | 新增     |
| runTestResolvePaths — 错误收集 | 异常     | change 不存在 / workflow.json 缺失非法 / 无 `files` → errors 增含"请重建"指引条目并 earlyReturn（不抛出，与该工具错误收集语义一致，AC-8） | 新增     |
| runTestResolvePaths — 错误收集 | 异常     | modules='change' 缺 change 参数 → schema refine 拒绝（MCP 层）；命令层以 errors 条目呈现                                                  | 新增     |
| runTestResolvePaths — git 退场 | 废弃     | modules='git-change' 场景用例删除；以 spy 断言 `execSync('git diff HEAD --name-only')` 不再被调用（AC-8）                                 | 废弃     |
| runTestResolvePaths — 回归     | 正向     | 显式 modules 数组与空数组（config 驱动目录扫描）两模式既有用例保留                                                                        | 新增     |

#### Mock策略

| Mock主体                              | Mock方案                                                                               | 应用场景          |
| ------------------------------------- | -------------------------------------------------------------------------------------- | ----------------- |
| 项目根解析                            | vi.mock getProjectDir 指向临时项目，或经入参 `project_root` 直接传入（与既有用例一致） | 全部 describe     |
| openspec/config.json 与 workflow.json | 真实 fixture 文件写入临时项目（清单走真实文件以保证管线集成度）                        | 清单模式 describe |
| node:child_process execSync           | spy 仅作 not-called 断言（git 退场防回归）                                             | git 退场用例      |

---

### plugins/dev-team/bin/src/lib/c4-cross-ref.ts -> plugins/dev-team/bin/src/lib/c4-cross-ref.test.ts

#### 待测功能

- runCrossRefCheck(projectRoot: string, options?: { staged?: boolean; files?: string[]; change?: string }): Promise\<ArchiCheckResult\> — 增 `change` 清单模式（被查文件集优先级：显式 files > `files.written` 直通 > 空）；`staged: true` 改为显式抛错别名，MUST NOT 隐式执行 `git diff --cached`

#### 用例

| 测试对象                       | 路径类型 | 测试条件                                                                                                                              | 迭代类型 |
| ------------------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| runCrossRefCheck — 清单模式    | 正向     | 传 change → 被查文件集 = `files.written` 直通（不做 test config 过滤），violations / matched / unmatched_files 按模型映射产出（AC-9） | 新增     |
| runCrossRefCheck — 清单模式    | 正向     | 显式 files 与 change 同传 → files 优先（优先级契约）                                                                                  | 新增     |
| runCrossRefCheck — staged 废弃 | 异常     | `staged: true` → 直接抛错（"staged 模式已由清单模式替代，请传 change 或 files"），且不产生任何 git 子进程调用（AC-9）                 | 新增     |
| runCrossRefCheck — staged 废弃 | 废弃     | 既有 staged 生效语义用例（`git diff --cached --name-only` 数据源断言）删除                                                            | 废弃     |
| runCrossRefCheck — 异常        | 异常     | change 清单模式 + workflow.json 无 `files` → 硬报错指引重建（非静默降级，AC-9）                                                       | 新增     |
| runCrossRefCheck — 异常        | 异常     | change 不存在 → 报错                                                                                                                  | 新增     |
| runCrossRefCheck — 边界        | 边界     | `files.written` 为空数组 → status 'no_changes'                                                                                        | 新增     |
| runCrossRefCheck — 边界        | 边界     | 清单中文件无对应模型元素 → unmatched_files 含之（直通不过滤的可见后果）；`openspec/architecture/**` 模型文件不进被查文件集            | 新增     |
| runCrossRefCheck — 回归        | 正向     | 显式 files 列表模式既有用例保留；模型缺失时 status 'skipped' 语义保留                                                                 | 新增     |

#### Mock策略

| Mock主体                                | Mock方案                                                       | 应用场景                   |
| --------------------------------------- | -------------------------------------------------------------- | -------------------------- |
| ./c4-parser readAllModels               | vi.mock 返回 fixture DSL（沿用既有 c4-cross-ref.test.ts 模式） | 全部 describe              |
| ../lib/file-inventory readFileInventory | vi.mock 返回 fixture 净状态                                    | 清单模式 / 异常 describe   |
| child_process execSync                  | spy 仅作 not-called 断言（staged 与 git 退场防回归）           | staged 废弃 / git 退场用例 |

---

### plugins/dev-team/bin/src/cli.ts -> plugins/dev-team/bin/src/cli.test.ts

#### 待测功能

- cli（cac 实例）：`test-execution` 命令选项集变更 — `--change` 描述更新为"兼作突变 scope 入口（清单）"、`--mutation-diff-only` 选项与 `mutationDiffOnly` 透传删除、`--skip-mutation` 映射 `noMutation` 不变、不引入 `--mutation-scope`

#### 用例

| 测试对象                    | 路径类型 | 测试条件                                                                                                                                                  | 迭代类型 |
| --------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| cli test-execution — 选项面 | 废弃     | `--mutation-diff-only` 选项注册与 `mutationDiffOnly` 透传断言删除；选项计数边界断言同步缩减（AC-7）                                                       | 废弃     |
| cli test-execution — 选项面 | 新增     | action 收到的 options 不含 mutationDiffOnly 字段（删除后不残留透传，AC-7）                                                                                | 新增     |
| cli test-execution — 选项面 | 正向     | `--change <name>` / `--project-root` / `--files` / `--framework` / `--skip-mutation` 解析与透传既有断言保留（AC-7：`--change` 与 `--skip-mutation` 保留） | 新增     |
| cli test-execution — 选项面 | 边界     | `--change` 传值经 action 映射至 runTestExecution options.change（职责扩大后透传不破坏）                                                                   | 新增     |
| cli test-execution — 选项面 | 边界     | 全部选项集中传参时命令注册与 action 调用正常（多选项组合回归）                                                                                            | 新增     |
| cli 回归                    | 边界     | 未知命令报错 / 缺选项值 CACError / run_static_analysis 注册与退出码既有用例保留                                                                           | 新增     |

#### Mock策略

| Mock主体                                                  | Mock方案                                  | 应用场景      |
| --------------------------------------------------------- | ----------------------------------------- | ------------- |
| ./commands/test-execution、./commands/run-static-analysis | vi.mock（既有模式），断言 action 透传参数 | 全部 describe |
| process.exit / stderr / stdout                            | spy（既有模式）                           | 全部 describe |

---

### plugins/dev-team/bin/src/mcp.ts -> plugins/dev-team/bin/src/mcp.test.ts

#### 待测功能

- MCP server 工具注册面（经 InMemoryTransport + Client 断言 tools/list 与 tools/call，无直接导出函数）：
  - 新增 `change_files`（changeFilesInputSchema / changeFilesOutputSchema，走 withResolvedProjectRoot + jsonContent 既有模式）
  - `archi_check` handler 增 `change` 传参、`staged` 显式报错
  - `test_resolve_paths` 入参面与 description 更新（模式 3 文案改清单）

#### 用例

| 测试对象           | 路径类型 | 测试条件                                                                                                                                                                            | 迭代类型 |
| ------------------ | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| change_files 注册  | 正向     | tools/list 含 `change_files`，inputSchema 与 changeFilesInputSchema 的 JSON Schema 形状一致（change 必填、op 枚举 append/set、written/deleted 至少其一，AC-10）                     | 新增     |
| change_files 注册  | 正向     | tools/call append / set → 委托 runChangeFiles 并返回操作后净状态 `{ written, deleted }`（AC-10）                                                                                    | 新增     |
| change_files 注册  | 异常     | 非法输入（缺 change / op 非法 / written 与 deleted 均缺省）→ 工具层参数错误                                                                                                         | 新增     |
| archi_check        | 正向     | tools/call 传 change → runCrossRefCheck 收到 change 传参（AC-9）                                                                                                                    | 新增     |
| archi_check        | 异常     | `staged: true` → 工具调用返回错误（显式废弃别名，AC-9）                                                                                                                             | 新增     |
| archi_check        | 废弃     | 既有 staged 生效语义断言（如有）替换为报错断言                                                                                                                                      | 废弃     |
| test_resolve_paths | 正向     | tools/call modules='change' + change → runTestResolvePaths 收到新入参形态（AC-8）                                                                                                   | 新增     |
| test_resolve_paths | 异常     | modules='change' 缺 change → schema refine 校验拒绝（AC-8）                                                                                                                         | 新增     |
| test_resolve_paths | 边界     | 工具 description 不再含 git diff 圈定语义（schema description 驱动 MCP 元数据）                                                                                                     | 新增     |
| 工具清单回归       | 边界     | 既有工具（phase_log / phase_next / backtrack / change_create / change_list / spec_list / config_get / archi_* / test_detect_frameworks）注册名称与数量不变（change_files 为纯新增） | 新增     |

#### Mock策略

| Mock主体                                                                      | Mock方案                                                      | 应用场景      |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------- | ------------- |
| 命令与 lib 模块（runChangeFiles / runCrossRefCheck / runTestResolvePaths 等） | vi.mock 命名空间 spy（既有 `import * as` 模式），断言委托传参 | 全部 describe |
| 项目根解析（getProjectDir / project_root resolve）                            | 既有 mockResolve 模式                                         | 全部 describe |
| MCP 传输                                                                      | InMemoryTransport + Client 真实握手（既有模式，非 mock）      | 全部 describe |

---

## 集成测试

<!-- 跨模块交互共识别 6 项，集成测试文件按项目既有约定放置于 `plugins/dev-team/bin/__tests__/<场景名>/<场景名>.test.ts` 与 `plugins/dev-team/build/__tests__/<场景名>/<场景名>.test.ts`（vitest include 覆盖 bin/__tests__/** 与 build/**）。 -->

### phase_next 事件 → session 注册表 → files 清单归账 → `plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts`

**涉及模块**:

| 模块                                                 | 角色                                      |
| ---------------------------------------------------- | ----------------------------------------- |
| `plugins/dev-team/bin/src/commands/change-create.ts` | workflow.json 创建方（fixture）           |
| `plugins/dev-team/bin/src/hooks.ts`                  | PostToolUse 记录器（事件触发方 / 归账方） |
| `plugins/dev-team/bin/src/lib/session-registry.ts`   | session 绑定注册表（归属判定）            |
| `plugins/dev-team/bin/src/lib/file-inventory.ts`     | 清单读取 / 折叠 / 写回（落盘方）          |

**关联AC**: AC-2, AC-3, AC-13

**关系描述**:

记录器子命令把 stdin 事件流折叠进 `workflow.json` 的净状态，链路横跨 hooks 的事件解析与自污染过滤、session-registry 的归属判定、file-inventory 的折叠与保留键写回。单元测试以 mock 替身隔离各环节，只能证明各自编排正确；三个真实模块协作时才暴露的行为——注册表丢失后事件静默丢弃、折叠写回与 eval/未知键保留的相互作用、Windows 路径归一化与 openspec 排除在真盘上的组合——必须真盘集成验证。可能的出错模式：建绑与归账两侧 projectRoot 归一化不一致导致注册表错位漏记；写回覆盖 `eval`；subagent 事件缺 agent_type 时误清除既有来源。

#### 场景: 工作流事件序列回放净状态

前置条件为临时项目根上由 runChangeCreate 创建真实 change。按序注入 stdin 事件（模拟 readFileSync）：phase_next 全名事件建绑 → Write src/foo.ts → Bash 重定向写 src/bar.ts → Bash `rm src/foo.ts` → Bash `git restore src/bar.ts` → Bash `mv src/a.ts src/b.ts`。验证 `workflow.json.files` 的折叠终态及其他键保留。

##### 用例

| 路径类型 | 测试条件                                                                                                                                                                                                         | 迭代类型 |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| 正向     | 上述序列后 files 终态 = written:['src/b.ts']、deleted:['src/foo.ts','src/a.ts']（bar 经 write→revert 净零、a→b 为 mv 双条目、foo 经 write→delete 净 deleted），且 workflow_type/created/未知键保留（AC-2、AC-4） | 新增     |
| 边界     | 序列中穿插 `openspec/changes/x/proposal.md` 写事件与 workflow.json 自身写事件 → 均不入清单（自污染排除，AC-2）                                                                                                   | 新增     |
| 异常     | 注册表文件被外部删除后注入写事件 → 静默丢弃（stderr 诊断、exit 0、workflow.json 不变）                                                                                                                           | 新增     |

#### 场景: session 隔离与换绑

前置条件为同一项目根两个 change；session S1 绑 change-a、session S2 绑 change-b。验证多 session 事件交错注入时归账互不串账，以及同 session 经新的 phase_next 事件换绑后的归属迁移。

##### 用例

| 路径类型 | 测试条件                                                                                           | 迭代类型 |
| -------- | -------------------------------------------------------------------------------------------------- | -------- |
| 正向     | S1 与 S2 事件交错注入 → 两 change 清单各自只含自家路径，互不串账（AC-3）                           | 新增     |
| 正向     | S1 经新 phase_next 事件换绑 change-b 后继续写 → 后续事件归 change-b，change-a 清单不再增长（AC-3） | 新增     |
| 边界     | 首个 phase_next 之前的未绑定事件 → 丢弃且不影响其后建绑与归账                                      | 新增     |

#### 场景: 条目来源审计（agent_type）

前置条件为 change 已绑定，subagent 事件 stdin 携带 agent_type、主会话事件不携带。验证 source 旁挂映射的 last-writer-wins 维护与折叠联动。

##### 用例

| 路径类型 | 测试条件                                                                                                                  | 迭代类型 |
| -------- | ------------------------------------------------------------------------------------------------------------------------- | -------- |
| 正向     | subagent（agent_type=dev-team:implementation-generator）写 src/foo.ts → source['src/foo.ts'] 为该值（AC-13）              | 新增     |
| 正向     | 随后主会话（无 agent_type）重写同路径 → source 键清除（AC-13）                                                            | 新增     |
| 边界     | 条目被 revert 折叠移除后再次 write → source 仅反映最新写入者（条目移除时来源随之消失，AC-13）                             | 新增     |
| 边界     | 消费方不读取契约：本链路产出的清单被 test_resolve_paths / archi_check 消费时，输出不含 source 字段（在 R3/R4 关系中复核） | 新增     |

##### Mock策略

| Mock主体        | Mock方案                                                                               | 应用场景 |
| --------------- | -------------------------------------------------------------------------------------- | -------- |
| hook 事件 stdin | vi.mock node:fs readFileSync（fd 0）按序列返回事件 JSON（既有 hooks.test.ts 注入模式） | 全部场景 |
| 项目根解析      | vi.mock getProjectDir 指向临时项目根（不用 process.chdir）                             | 全部场景 |
| —（其余模块）   | change-create / session-registry / file-inventory 全部真实实现，不 mock                | 全部场景 |

---

### --change CLI 参数 → 清单突变 scope → 净归零去噪 → `plugins/dev-team/bin/__tests__/mutation-scope-inventory/mutation-scope-inventory.test.ts`

**涉及模块**:

| 模块                                                  | 角色                        |
| ----------------------------------------------------- | --------------------------- |
| `plugins/dev-team/bin/src/cli.ts`                     | 触发方（--change 选项入口） |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | scope 解析与去噪编排方      |
| `plugins/dev-team/bin/src/lib/file-inventory.ts`      | 清单读取方                  |
| `plugins/dev-team/bin/src/lib/test-path-naming.ts`    | 测试文件反推同位源文件      |
| 真实 git 仓库（临时 fixture）                         | HEAD 内容裁判（外部依赖）   |

**关联AC**: AC-7

**关系描述**:

突变 scope 从 git 工作区迁移到清单后，正确性取决于三个环节的串联：CLI 传参 → 清单读取（含旧 change 硬报错守卫）→ written 逐文件与 git HEAD 内容对比。去噪读的是"工作区 vs HEAD"这一 hook 体系之外的事实，若 mock 掉 git 则去噪漏斗方向（只向 overstate、不漏删）无法验证，因此本关系要求以 git 初始化的真实临时仓库 fixture。可能的出错模式：路径分隔符或行尾差异导致 HEAD 对比恒不命中（去噪整桶失效）；去噪误删 HEAD 中不存在的文件；`--skip-mutation` 时仍触发清单读取使旧 change 误报硬错误。

#### 场景: 真实仓库去噪漏斗

前置条件为临时目录初始化 git 仓库并完成首次提交（含 src/kept.ts 与 src/noisy.ts）；change_create 创建 change 后写入 `files.written = [src/kept.ts, src/noisy.ts, src/new-file.ts]`；工作区改写 src/kept.ts 使内容 ≠ HEAD，src/noisy.ts 保持与 HEAD 一致。验证 scope 组装结果。

##### 用例

| 路径类型 | 测试条件                                                                                                                                                           | 迭代类型 |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| 正向     | 上述 fixture 下 executePlanEntry 收到的 mutationDiffFiles 恰为 [src/kept.ts, src/new-file.ts]（noisy 被去噪剔除、new-file 因 HEAD 无该文件保留，顺序无关）（AC-7） | 新增     |
| 边界     | written 全部与 HEAD 内容一致 → scope 为空（全去噪），突变阶段按空 scope 处理而不报错                                                                               | 新增     |
| 边界     | CRLF 行尾与反斜杠路径形态 → 归一化后对比命中（不因行尾/分隔符使漏斗失效）                                                                                          | 新增     |

#### 场景: 入口与守卫

前置条件为临时项目与各异常 fixture（机制前旧 change、仅 skip-mutation 的调用、不传 change 的调用）。

##### 用例

| 路径类型 | 测试条件                                                                        | 迭代类型 |
| -------- | ------------------------------------------------------------------------------- | -------- |
| 异常     | 机制前旧 change（无 files）+ 传 change → 硬报错"请重建"且不产出执行报告（AC-7） | 新增     |
| 正向     | --skip-mutation 与 change 同传 → 不触发清单读取与 git 调用（惰性解析，AC-7）    | 新增     |
| 边界     | 不传 change → 突变不限圈（现状默认），清单通道完全不激活                        | 新增     |
| 正向     | written 含测试文件路径 → 反推同位源文件并入 scope（AC-7）                       | 新增     |

##### Mock策略

| Mock主体                                                                     | Mock方案                                                                   | 应用场景 |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------- | -------- |
| 测试执行边界（executePlanEntry / generateSubReport / generateSummaryReport） | vi.mock 隔离真实测试运行器（本关系验证 scope 编排，不真跑 vitest/Stryker） | 全部场景 |
| 框架版本探测                                                                 | bin/**tests**/test-setup.ts 全局 mock（既有 setup）                        | 全部场景 |
| —（git 与清单）                                                              | 真实 git 仓库 fixture + 真实 workflow.json，不 mock                        | 全部场景 |

---

### change 文件清单 → test_resolve_paths 测试路径推导 → `plugins/dev-team/bin/__tests__/inventory-test-path-resolution/inventory-test-path-resolution.test.ts`

**涉及模块**:

| 模块                                                          | 角色                                   |
| ------------------------------------------------------------- | -------------------------------------- |
| `plugins/dev-team/bin/src/commands/change-create.ts`          | workflow.json 创建方（fixture）        |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts`     | 读取方（清单 → effectiveModules 管线） |
| `plugins/dev-team/bin/src/lib/file-inventory.ts`              | 清单来源                               |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | test config 过滤管线（既有）           |

**关联AC**: AC-8

**关系描述**:

清单模式把 `files.written` 直接灌入既有 modules 校验管线，风险集中在两个界面：清单条目形态（POSIX 相对路径、目录路径、测试文件、项目根外路径）与管线校验规则（test config scope、exclude、isSourceFile）的组合行为；以及失败路径——旧 change 必须以 `errors` 条目呈现而非抛出，保持该工具的错误收集契约。可能的出错模式：清单读取失败被上游吞成空 modules，误触发 config 全库扫描导致范围爆炸；git-change 分支残留使结果依赖环境 git 状态。

#### 场景: 清单全景推导

前置条件为临时项目写入 openspec/config.json（tests: vitest）与 change 的 workflow.json，`files.written = [src/a.ts, src/a.test.ts, scripts/notes.txt, ../outside.ts, src/excluded.ts]`（末者命中 exclude 规则）。

##### 用例

| 路径类型 | 测试条件                                                                                                                                           | 迭代类型 |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| 正向     | 上述 fixture → unit_tests 恰含 src/a.ts 的推导条目；errors 分别对应 test file / not testable / outside project root；excluded 条目静默跳过（AC-8） | 新增     |
| 边界     | written 仅含目录路径 → 按管线语义处理（不展开目录，与显式 modules 传入行为一致）                                                                   | 新增     |
| 边界     | written 为空数组 → unit_tests=[]、无致命 errors，不触发全库扫描                                                                                    | 新增     |
| 异常     | 机制前旧 change → errors 含"请重建"指引 + unit_tests=[]（earlyReturn 不抛出，AC-8）                                                                | 新增     |
| 边界     | 全链路以 spy 断言无任何 git 子进程调用（git 退场防回归，AC-8）                                                                                     | 新增     |

##### Mock策略

| Mock主体                    | Mock方案                                                           | 应用场景     |
| --------------------------- | ------------------------------------------------------------------ | ------------ |
| 项目根解析                  | vi.mock getProjectDir 指向临时项目（既有集成模式）                 | 全部场景     |
| config 与清单               | 真实 fixture 文件（openspec/config.json + workflow.json），不 mock | 全部场景     |
| node:child_process execSync | spy 仅作 not-called 断言                                           | git 退场用例 |

---

### change 清单 → archi_check 被查文件集 → `plugins/dev-team/bin/__tests__/archi-check-inventory/archi-check-inventory.test.ts`

**涉及模块**:

| 模块                                             | 角色                               |
| ------------------------------------------------ | ---------------------------------- |
| `plugins/dev-team/bin/src/lib/c4-cross-ref.ts`   | 检查方（被查文件集组装与交叉引用） |
| `plugins/dev-team/bin/src/lib/file-inventory.ts` | 清单来源                           |
| `plugins/dev-team/bin/src/lib/c4-parser.ts`      | 模型参照系（models/*.c4 解析）     |

**关联AC**: AC-9

**关系描述**:

archi_check 的清单直通绕过 test config 过滤，被查文件集的正确性取决于优先级链（显式 files > `files.written` > 空）与失败语义（staged 显式报错、无 files 硬报错而非静默降级）。只有在真实模型 fixture 下才能验证直通文件与 path_to_element 匹配的端到端产出（violations / matched / unmatched 的生成）。可能的出错模式：staged 分支残留导致隐式 git 调用；清单读取失败被降级为空集，静默 no_changes 掩盖机制前旧 change。

#### 场景: 清单直通检查

前置条件为临时项目 `openspec/architecture/models/` 写入声明两元素与关系的 .c4 fixture；change 的 `files.written` 同时含已建模文件与未建模文件。

##### 用例

| 路径类型 | 测试条件                                                                                                                                                  | 迭代类型 |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| 正向     | change 清单模式 → files.written 全量参与被查（含测试文件等显式数组模式之外不会被过滤的条目），matched / unmatched_files / violations 反映直通集合（AC-9） | 新增     |
| 正向     | 显式 files 与 change 同传 → files 优先（AC-9）                                                                                                            | 新增     |
| 边界     | written 为空数组 → status 'no_changes'                                                                                                                    | 新增     |
| 异常     | `staged: true` → 抛错且 spy 确认无 git 调用（AC-9）                                                                                                       | 新增     |
| 异常     | 无 files 的机制前旧 change → 硬报错（非静默 skipped，AC-9）                                                                                               | 新增     |

##### Mock策略

| Mock主体                    | Mock方案                                        | 应用场景        |
| --------------------------- | ----------------------------------------------- | --------------- |
| models/*.c4                 | 真实 fixture 文件写入临时项目（走真实解析管线） | 全部场景        |
| 项目根解析                  | vi.mock getProjectDir 指向临时项目              | 全部场景        |
| node:child_process execSync | spy 仅作 not-called 断言                        | staged 废弃用例 |

---

### backtrack → workflow.json.files 清空重记 → `plugins/dev-team/bin/__tests__/inventory-backtrack-clear/inventory-backtrack-clear.test.ts`

**涉及模块**:

| 模块                                                 | 角色                            |
| ---------------------------------------------------- | ------------------------------- |
| `plugins/dev-team/bin/src/commands/change-create.ts` | workflow.json 创建方（fixture） |
| `plugins/dev-team/bin/src/commands/backtrack.ts`     | 触发方（清空规则与预检）        |
| `plugins/dev-team/bin/src/lib/eval-json.ts`          | 评估通道（eval 条目读写）       |
| `plugins/dev-team/bin/src/lib/file-inventory.ts`     | files 读取与清空写回            |

**关联AC**: AC-6

**关系描述**:

backtrack 在同一次调用中改写两块持久化状态——`workflow.json.eval` 条目与 `files` 净状态。单元测试以 mock 验证调用编排；集成层在真盘上验证"预检先行、不部分生效"的关键承诺：`files` 缺失时 `eval` 绝不能已被改写。可能的出错模式：清空写回破坏 eval 或未知键；预检时序颠倒导致机制前旧 change 的 backtrack 半生效；test-only 工作流误触发清空。

#### 场景: 真盘清空与评估联动

前置条件为临时项目内由 runChangeCreate 创建真实 change 并构造多条 eval 条目与非空 `files`。

##### 用例

| 路径类型 | 测试条件                                                                                                                                     | 迭代类型 |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| 正向     | backtrack 到 implement → eval 条目获得 backtrack_to / backtrack_reason 并向下游传播 stale，同时 files = { written: [], deleted: [] }（AC-6） | 新增     |
| 正向     | backtrack 到 implement 之后的 phase（如 test-execution）→ files 保持原内容（AC-6）                                                           | 新增     |
| 异常     | 无 files 的机制前旧 change backtrack → 整体抛错，磁盘 workflow.json 逐字节不变（含 eval，不部分生效）                                        | 新增     |
| 边界     | test-only 工作流 backtrack（phase 表无 implement）→ files 不被清空                                                                           | 新增     |
| 边界     | 清空后紧接 readFileInventory → 空净状态合法读取（构成 R2 空 scope 的前置通路）                                                               | 新增     |

##### Mock策略

| Mock主体      | Mock方案                                             | 应用场景 |
| ------------- | ---------------------------------------------------- | -------- |
| 项目根解析    | vi.mock getProjectDir 指向临时项目                   | 全部场景 |
| —（其余模块） | eval-json / file-inventory / phase 表全真实，不 mock | 全部场景 |

---

### hooks.canonical.json → build 组装 → 产物 PostToolUse 注册 → `plugins/dev-team/build/__tests__/hooks-post-tool-use-registry/hooks-post-tool-use-registry.test.ts`

**涉及模块**:

| 模块                                          | 角色                                                                         |
| --------------------------------------------- | ---------------------------------------------------------------------------- |
| `plugins/dev-team/hooks/hooks.canonical.json` | 配置源（postToolUse 条目）                                                   |
| `plugins/dev-team/build/hooks-profile.ts`     | 组装方（canonical schema 扩展 + 平台包装，入口 buildHooksFile）              |
| `plugins/dev-team/build/apply-env-tokens.ts`  | token 展开方（`__MCP:phase_next__` / `__BIN:hooks__` / `__DEV_TEAM_ROOT__`） |

**关联AC**: AC-2, AC-3

**关系描述**:

记录器是否真正生效取决于注册链：canonical JSON 新增的 postToolUse 条目必须被扩展后的 canonical schema 接受，经 token 展开后在 claude 产物中生成 `hooks.PostToolUse` 条目（matcher 含 `Write|Edit|NotebookEdit|Bash|PowerShell` 与 phase_next 的 MCP 全名），而 cursor 产物因 matcher 为 null 必须整段省略。schema 与组装任一遗漏都会让记录器静默失效——各模块单元测试都以 fixture 直通，只有组装链集成才能发现。可能的出错模式：canonical schema 未扩 postToolUse 导致 parse 抛错或字段被剥离；cursor 分支未按 null 过滤输出空 matcher 条目；既有 preToolUse / subagentStop 包装行为被破坏。

#### 场景: claude 产物注册

前置条件为以真实 hooks.canonical.json（或等价对象 fixture）与 claude ProductEnv 调用 buildHooksFile。

##### 用例

| 路径类型 | 测试条件                                                                                                                                         | 迭代类型 |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| 正向     | 输出 hooks.PostToolUse 含 record-files 条目，commandTemplate 含 record-files 子命令且 `__BIN:hooks__` / `__DEV_TEAM_ROOT__` token 已展开（AC-2） | 新增     |
| 正向     | `__MCP:phase_next__` 展开为 claude 产物 MCP 全名 `mcp__plugin_dev-team_dev-team__phase_next` 并入 matcher（AC-3）                                | 新增     |
| 边界     | 既有 PreToolUse 三条目与 SubagentStop 条目的包装形态不变（回归）                                                                                 | 新增     |

#### 场景: cursor 产物省略

前置条件为以同一 canonical 与 cursor ProductEnv 调用 buildHooksFile。

##### 用例

| 路径类型 | 测试条件                                                                                                      | 迭代类型                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| 边界     | cursor matcher 为 null 的 postToolUse 条目 → cursor 产物不出现 postToolUse 段（整段省略而非输出空条目，AC-2） | 新增                                                                  |
| 正向     | cursor 产物既有 preToolUse（Write                                                                             | StrReplace、Shell）与 subagentStop（null 过滤后为空）行为不变（回归） | 新增 |

##### Mock策略

| Mock主体       | Mock方案                                                                                                       | 应用场景 |
| -------------- | -------------------------------------------------------------------------------------------------------------- | -------- |
| ProductEnv     | 构造 claude / cursor 两个 env fixture（沿用既有 build/**tests**/hooks-platform-subagent-stop 的 env 构造方式） | 全部场景 |
| canonical JSON | 优先以 fs 读取真实 hooks.canonical.json 断言注册面（覆盖真实配置源）；token 展开走真实 applyEnvTokens          | 全部场景 |

---

## 不可测试项

- AC-11（evaluator 三态对账提示词重写、git diff 降级为纯观察辅助）— **原因**: 行为载体是 LLM 提示词，对账判定质量无法用进程内断言验证；对 agents/*.md 做文本包含断言只能证明字面存在、不能证明语义正确。由 acceptance 阶段按 AC-11 逐条人工核对。
- AC-12（design/proposal 模板「删除文件」子节、SKILL.md 与 implementation-generator 文案对齐）— **原因**: 同上，markdown 模板与提示词的语义对齐不可自动化验证，由 acceptance 阶段人工核对。
- hooks 平台运行时真实触发链（Claude Code / Cursor 对 PostToolUse matcher 的实际分发、subagent 工具事件 session_id 共享性）— **原因**: 依赖宿主平台运行时行为，进程内测试只能以注入事件模拟；提案既定由实现期临时 log hook 实测一轮完整工作流验证，不在自动化测试范围。
- `simple-git` 依赖移除、package.json version bump 与 rebuild 产物刷新（claude-plugins/** / cursor-plugins/**）— **原因**: 构建发布动作，由 CLAUDE.md 项目规则与构建流程保证；自动化仅以"`lib/git.ts` 删除后残留 import 即编译失败"与去噪/退场 spy 断言间接约束。
- 多 session 并发写 session 注册表 / workflow.json 的竞态 — **原因**: 提案风险表既定本期不解决（工作流为单 session 串行，`change_files` set 语义可事后修复），不存在确定性的断言语义。
- `git stash pop` / `git clean` 等批量还原后的清单净状态漂移 — **原因**: 批量还原对 PostToolUse hook 不可见是提案既定边界，防线交 PreToolUse 拦截（已纳入单元测试）；还原后的偏差由外部裁判（文件系统存在性、内容核对、突变去噪）兜底，清单机制本身无可断言目标。
