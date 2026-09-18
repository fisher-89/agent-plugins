# 测试设计: move-files-write-into-workflow-module

> **日期**: 2026-09-18

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | gitignore 过滤器实现：`modules/workflow/files/` 下存在基于 `ignore` 包的封装；语法与匹配语义由 `ignore` 包负责、不逐项验证；单测验证自研层（实样装载、层级应用、缺失层视为无规则、读取失败 fail-open） | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/files/gitignore.test.ts` |
| AC-2 | 记录器接入同步过滤：已绑定 session 时 gitignore 命中路径不入 `files`、未忽略路径照常入清单、祖先链全无 `.gitignore` 时行为与现状一致且 hook 不阻塞 | 集成测试 | `plugins/dev-team/bin/src/hooks.test.ts`（hooks.ts → commands/record-files.ts → files/record.ts + files/gitignore.ts + files/file-inventory.ts）；管线内过滤细节另由 `files/record.test.ts` 单元承接 |
| AC-3 | 写路径收敛模块：`workflow/index.ts` 导出归账管线与 append/set 语义；`commands/record-files.ts` / `commands/change-files.ts` 无内联折叠/落盘实现残留；无 re-export shim | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/index.test.ts` |
| AC-4 | 行为回归不变：除新增过滤场景外，`hooks.test.ts` 与 `change-files.test.ts` 既有断言全部保持（折叠规则、source last-writer-wins、保留键纪律、硬报错语义） | 集成测试 | `plugins/dev-team/bin/src/hooks.test.ts`、`plugins/dev-team/bin/src/commands/change-files.test.ts`（回归面）+ `plugins/dev-team/bin/src/modules/workflow/index.test.ts`（导出面） |
| AC-5 | change_files 通道不过滤：append/set 对 gitignore 命中路径仍生效入桶 | 集成测试 | `plugins/dev-team/bin/src/commands/change-files.test.ts`（commands/change-files.ts → modules/workflow/files/file-inventory.ts） |
| AC-6 | 版本与产物：`plugins/dev-team/package.json` `version` 提升，build 成功且 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 刷新 | 不可自动化测试 | —（进程外构建行为，见 ## 不可测试项） |

---

## 单元测试

框架识别结果：`vite-plus`（`vite-plus/test` 的 describe/it/expect，与既有测试文件一致）。测试文件按 `test_resolve_paths` 的同目录映射落位，两处与 proposal 测试清单的差异在此注明：

1. proposal 将「归账管线用例」预列于 `file-inventory.test.ts`；design 阶段把管线拆分至新文件 `files/record.ts`（proposal 授权的拆分），故管线用例随模块落位 `files/record.test.ts`（同目录映射），append/set 用例仍按 proposal 落位 `file-inventory.test.ts`。
2. `commands/record-files.ts` 的同目录默认路径 `record-files.test.ts` 不启用——该命令层的既有场景按 proposal 测试清单宿主于 `hooks.test.ts`（既有结构），不为纯协议适配层新建平行测试文件。

文件系统一律不 mock：`mkdtempSync` 临时目录 + 真实读写（沿用 `file-inventory.test.ts` / `change-files.test.ts` 既有模式）。`ignore` 包的单文件 gitignore 语义（`*` / `**`、负模式求值、目录递归、后匹配覆盖、尾随 `/` 行为）由库自身保障，不逐项验证；以下用例只覆盖自研装载与层级组装层。

### plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts -> plugins/dev-team/bin/src/modules/workflow/files/gitignore.test.ts

新建文件。被测对象为 design 终定的三个导出（`gitignore.test.ts` 为 proposal 测试清单指定的新建测试文件）。

#### 待测功能

- loadGitignoreFilter(projectRoot: string, onWarn?: (message: string) => void): GitignoreFilter — 构建层级过滤器：根层即刻解析；祖先链各层按需惰性装载并缓存；缺失层视为无规则（非告警）；解析异常 fail-open + `onWarn` 诊断
- isGitIgnored(filter: GitignoreFilter, relPath: string): boolean — 层级判定：自浅入深装载祖先链层、目录短路（被排除目录不再下探、其内 `.gitignore` 不生效）、取最后产生匹配的层的结论（深层优先）；空路径与不可判定返回 `false`（fail-open）
- GitignoreFilter — 不透明句柄类型（`projectRoot`、根层、层缓存 `Map`、`onWarn`），消费方不探其内部

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| loadGitignoreFilter / isGitIgnored — 根层实样装载 | 正向 | 项目根 `.gitignore` 为本仓库实样（`.*/` + `!.claude-plugin` + `!.cursor-plugin` + `!.gitignore` + `memory`）时，`.claude/agent-memory/x.md` 判定为 ignored=true、`.claude-plugin/marketplace.json` 判定为 false、`memory/notes.md` 判定为 true（提取准确性代表用例，AC-1） | 新增 |
| loadGitignoreFilter / isGitIgnored — 根层实样装载 | 正向 | 同一实样下未命中路径 `src/a.ts` 判定为 false（无规则路径放行） | 新增 |
| isGitIgnored — 层级应用 | 正向 | 子目录规则覆盖根规则（深层优先）：根 `*.log`、`sub/.gitignore` 含 `!keep.log` → `sub/keep.log` 为 false 且 `sub/other.log` 为 true | 新增 |
| isGitIgnored — 层级应用 | 正向 | 深层层存在时根层规则仍对完整路径生效：根 `*.log`、`a/b/.gitignore` 存在（规则不相关）→ `a/b/c.log` 仍为 true（激活层集合按「相对该层的路径后缀」逐层测试的组装正确性） | 新增 |
| isGitIgnored — 层级应用 | 边界 | 中间目录被排除（根 `ignored/`）、`ignored/.gitignore` 含 `!rescue.txt` → `ignored/rescue.txt` 仍为 true（目录短路：被排除目录内 `.gitignore` 不装载、不参与判定） | 新增 |
| isGitIgnored — 层级应用 | 边界 | `a/b/c.ts` 多层嵌套 → `a/.gitignore` 与 `a/b/.gitignore` 均装载参与判定（祖先链逐层行走） | 新增 |
| loadGitignoreFilter — 缺失与失败层 | 边界 | 某层目录缺 `.gitignore` → 该层视为无规则且不触发 `onWarn`（缺失为正常路径、非告警），判定继续 | 新增 |
| loadGitignoreFilter — 缺失与失败层 | 异常 | 层上 `.gitignore` 为目录（读取失败）→ `onWarn` 收到含该层路径的诊断消息，该层按无规则处理，整体不抛错 | 新增 |
| loadGitignoreFilter — 缺失与失败层 | 异常 | 层 `.gitignore` 含使 `ignore` 解析抛错的规则内容 → 解析异常被吞、`onWarn` 诊断、该层按无规则处理（fail-open） | 新增 |
| loadGitignoreFilter — 缺失与失败层 | 异常 | `isGitIgnored` 判定过程抛错 → 返回 false（fail-open）且 `onWarn` 诊断，不向调用方传播异常 | 新增 |
| isGitIgnored — 输入边界 | 边界 | relPath 为空字符串 → 直接返回 false（fail-open 短路，不触发任何层装载） | 新增 |
| isGitIgnored — 输入边界 | 边界 | relPath 含中文 / 空格 / emoji → 按字面参与匹配，组装层不做语义裁剪 | 新增 |
| isGitIgnored — 输入边界 | 边界 | 同一 filter 重复判定同层路径（`.claude/a` 后 `.claude/b`）→ 层缓存复用、结论一致（单 hook 调用内缓存契约） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 不 mock：`mkdtempSync` 临时项目内真实写入各层 `.gitignore`（含实样内容、子目录规则、目录形态的 `.gitignore`），真实读写判定 | 全部 describe |
| `onWarn` 回调 | 注入 `vi.fn()` 捕获诊断消息，断言消息含层路径 | 缺失与失败层 describe |

### plugins/dev-team/bin/src/modules/workflow/files/record.ts -> plugins/dev-team/bin/src/modules/workflow/files/record.test.ts

新建文件（归账管线自 `commands/record-files.ts` 迁入后的同目录宿主；proposal 原列于 `file-inventory.test.ts` 的管线用例随 design 拆分随迁至此）。`recordFileOps` 经 workflow barrel 导出，属模块公共 API，非 test-only export。

#### 待测功能

- recordFileOps(changeDir: string, ops: FileOp[], context: { projectRoot: string; agentType?: string }): void — 归账管线入口：规范化（项目根相对化、越界丢弃）→ 自污染排除（`openspec/**` 与 `workflow.json`）→ gitignore 过滤 → 读 → 折叠 → 落盘

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| recordFileOps — 管线正序 | 正向 | ops 含绝对路径（含 Windows 反斜杠形态）→ 相对项目根 POSIX 化后入桶（`normalizeRecordedPath` 迁移语义保持） | 新增 |
| recordFileOps — 管线正序 | 正向 | op 携带 `agentType` / context 携带 `agentType` → `source[path]=agent_type` 盖章，沿用 fold 的 last-writer-wins 与无 agentType 清除语义 | 新增 |
| recordFileOps — 管线正序 | 正向 | write / delete / revert 三类 op 各按折叠规则入桶并落盘 | 新增 |
| recordFileOps — 管线正序 | 边界 | ops=[] → 不读不写 `workflow.json`（空批次提前返回） | 新增 |
| recordFileOps — 管线正序 | 边界 | 全部路径越界（`../outside.ts`）→ 无候选路径，不读不写清单 | 新增 |
| recordFileOps — 自污染排除与 gitignore 过滤叠加 | 正向 | `openspec/**` 与 `workflow.json`（含嵌套形态）路径被排除、不入桶不触发读取（迁移语义保持） | 新增 |
| recordFileOps — 自污染排除与 gitignore 过滤叠加 | 正向 | 项目根 `.gitignore` 含 `.claude/` 时，op `.claude/x.md` 被过滤丢弃、同批 `src/a.ts` 照常入桶（AC-2 管线层） | 新增 |
| recordFileOps — 自污染排除与 gitignore 过滤叠加 | 正向 | gitignore 过滤对 write / delete / revert 统一生效：被忽略路径的 delete op 不入 deleted 桶 | 新增 |
| recordFileOps — 自污染排除与 gitignore 过滤叠加 | 边界 | `.claude/`（gitignore 命中）与 `openspec/`（自污染排除）同批出现 → 均不入桶（两类过滤叠加） | 新增 |
| recordFileOps — 自污染排除与 gitignore 过滤叠加 | 边界 | 净状态中已存在的历史 ignored 路径不被回溯清理：过滤只作用于本次增量 op，既有桶内条目原样保留 | 新增 |
| recordFileOps — 自污染排除与 gitignore 过滤叠加 | 边界 | 项目根及祖先链全无 `.gitignore` → 行为与现状一致（候选路径全量入桶），过滤器惰性构建不产生副作用 | 新增 |
| recordFileOps — 持久化与异常 | 正向 | 落盘后 `workflow_type` / `created` / `eval` / 未知键保留、2 空格缩进 + 尾换行（经 `writeFileInventory` 纪律） | 新增 |
| recordFileOps — 持久化与异常 | 异常 | legacy change（`workflow.json` 无 `files`）→ 抛错向上传播（由命令层 catch-all 兜底），文件不被半写 | 新增 |
| recordFileOps — 持久化与异常 | 边界 | `.gitignore` 读取失败（fail-open）→ 路径保留入清单，诊断经 `process.stderr.write` 输出且带 `record-files:` 前缀，不抛错 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 不 mock：临时项目 + `validWorkflowDoc` fixture（真盘读写，同 `file-inventory.test.ts` 模式） | 全部 describe |
| `process.stderr.write` | `vi.spyOn` 捕获输出，断言 `record-files:` 前缀诊断 | 持久化与异常 describe |

### plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts -> plugins/dev-team/bin/src/modules/workflow/files/file-inventory.test.ts

修改既有测试文件。既有原语（`readFileInventory` / `foldFileOps` / `writeFileInventory`）用例全部保持不变；本文件追加迁入的 append/set 读改写语义用例。该文件不接 gitignore 过滤——`appendFileOps` / `setFileBuckets` 签名无 `projectRoot` 入参，结构上无法过滤（AC-5 的结构性保障，无需模块层用例）。

#### 待测功能

- readFileInventory(changeDir: string): FileInventory — 既有：读取并校验清单（签名不变，用例不变）
- foldFileOps(inventory: FileInventory, ops: FileOp[]): FileInventory — 既有：对称折叠纯函数（签名不变，用例不变）
- writeFileInventory(changeDir: string, files: FileInventory): void — 既有：写回纪律（签名不变，用例不变）
- appendFileOps(changeDir: string, paths: { written?: string[]; deleted?: string[] }): FileInventory — 新增迁入：append 读改写，桶内折叠合并去重、保留既有 `source`、新并路径无 source
- setFileBuckets(changeDir: string, paths: { written?: string[]; deleted?: string[] }): FileInventory — 新增迁入：set 读改写，提供桶整桶覆写、覆写条目清 `source`、净状态外 source 条目删除

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| appendFileOps — append 读改写 | 正向 | 空净状态 append written 新路径 → 入桶并落盘，返回净状态与磁盘一致，无 source 键 | 新增 |
| appendFileOps — append 读改写 | 正向 | 已存在路径（带 source）再次 append → 去重不重复且既有 `source` 原样保留（锁定「append 不复用 `foldFileOps`」的 source 保留差异，防实现漂移） | 新增 |
| appendFileOps — append 读改写 | 正向 | append deleted → 该路径移出 written 进入 deleted（折叠合并语义） | 新增 |
| appendFileOps — append 读改写 | 边界 | 同批重复路径去重（first-seen 顺序稳定）；written/deleted 传空数组 → 净状态不变 | 新增 |
| appendFileOps — append 读改写 | 边界 | 100+ 路径批量 append → 全部入桶且去重稳定 | 新增 |
| setFileBuckets — set 读改写 | 正向 | 提供桶整桶覆写、未提供桶保持原样 | 新增 |
| setFileBuckets — set 读改写 | 正向 | 覆写条目清除 `source`；净状态中已不存在路径的 source 条目删除；未提供且仍在净状态的路径 source 保留 | 新增 |
| setFileBuckets — set 读改写 | 边界 | `written: []` → 该桶显式清空、deleted 桶不动 | 新增 |
| setFileBuckets — set 读改写 | 边界 | written 与 deleted 全部提供 → 旧 source 条目全清 | 新增 |
| setFileBuckets — set 读改写 | 异常 | changeDir 无 `workflow.json` / 文件非法 / 缺 `files` → 抛错且磁盘内容不变（复用 `readFileInventory` 前置） | 新增 |

既有 describe 组（`readFileInventory — 正向/异常`、`foldFileOps — 折叠规则 / source 旁挂来源维护 / 异常与纯函数不变量`、`writeFileInventory — 写回纪律`）全部保持、断言语义不变（AC-4），不在表中逐行重复。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 不 mock：`mkdtempSync` 临时项目 + `runChangeCreate` / `validWorkflowDoc` fixture 真盘读写（沿用既有模式） | 全部 describe |

### plugins/dev-team/bin/src/modules/workflow/index.ts -> plugins/dev-team/bin/src/modules/workflow/index.test.ts

修改既有测试文件。barrel 无自有逻辑，测试聚焦导出面与「命令层收敛契约」（AC-3 锚点）。

#### 待测功能

- workflow barrel — 模块统一出口：既有 `readFileInventory` / `foldFileOps` / `writeFileInventory` / `getChangedFiles`，本次扩充 `recordFileOps`（自 `files/record.ts`）与 `appendFileOps` / `setFileBuckets`（自 `files/file-inventory.ts`）；gitignore API 不进 barrel

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| workflow barrel — 函数导出 | 正向 | `recordFileOps` / `appendFileOps` / `setFileBuckets` 为 function 类型，且与源模块直接导入为同一函数引用（toBe，re-export 而非复制） | 新增 |
| workflow barrel — 导出面精确 | 废弃 | 旧断言「运行时导出键恰好等于 4 个函数名」随导出扩充失效，移除 | 废弃 |
| workflow barrel — 导出面精确 | 新增 | 运行时导出键恰好等于 7 个函数名（readFileInventory / foldFileOps / writeFileInventory / getChangedFiles / recordFileOps / appendFileOps / setFileBuckets），且不含 `loadGitignoreFilter` / `isGitIgnored`（gitignore API 不进 barrel 的结构锁定，防 knip dead export） | 新增 |
| workflow barrel — 命令层收敛契约 (AC-3) | 正向 | `commands/record-files.ts` 源码文本不含 `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` / `foldFileOps` / `writeFileInventory`（无实现残留） | 新增 |
| workflow barrel — 命令层收敛契约 (AC-3) | 正向 | `commands/change-files.ts` 源码文本不含 `applyAppend` / `applySet` / `dedupe`（无实现残留）；两文件均无对迁移函数的 re-export shim | 新增 |

既有断言「barrel 导出与源模块直接导入为同一函数引用」保持，随扩充函数追加断言行。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无外部依赖 | 不 mock：纯模块导入 + 引用断言；源码残留扫描经真实 `node:fs` 读取两命令文件文本（同 `hooks.test.ts` 「fixture 路径断言」模式） | 全部 describe |

### plugins/dev-team/bin/src/commands/change-files.ts -> plugins/dev-team/bin/src/commands/change-files.test.ts

修改既有测试文件。命令层瘦身为「校验前置 + 委托」，既有四组用例全部保持；新增 AC-5 场景。

#### 待测功能

- runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput — 校验前置（`resolveChangeDir` + 清单可读）后委托 `appendFileOps` / `setFileBuckets`，透传 `{ written, deleted }` 返回投影（签名与 IO 契约不变）
- ChangeFilesOptions — MCP handler 消费的 options 类型（定义不动，保留）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeFiles — change_files 通道不过滤 (AC-5) | 正向 | 临时项目根 `.gitignore` 含 `.claude/`（与记录器过滤同一实样）时 append `written: ['.claude/memory.md']` → 仍入桶并落盘、返回净状态含之（记录器会过滤的同一路径，人工补录通道放行） | 新增 |
| runChangeFiles — change_files 通道不过滤 (AC-5) | 正向 | 同前置下 set `written` 含 ignored 路径 → 整桶覆写照常生效 | 新增 |

既有 describe 组（`runChangeFiles — append` / `— set` / `— 异常` / `— 边界（schema 校验行为）`）全部保持、断言语义不变（AC-4 回归面），不在表中逐行重复。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 不 mock：临时项目 + `runChangeCreate` fixture + 真实写入根 `.gitignore`（真盘模式） | change_files 通道不过滤 describe |

### plugins/dev-team/bin/src/commands/record-files.ts -> plugins/dev-team/bin/src/hooks.test.ts

修改既有测试文件。命令层瘦身后仅保留 `runRecordFiles` 导出；其场景按 proposal 测试清单宿主于 `hooks.test.ts`（同目录默认路径 `record-files.test.ts` 不启用，见本章节开头说明）。

**关键测试影响**：迁移后「读 → 折叠 → 写」在 `files/record.ts` 内部经 `./file-inventory` 直接导入完成（不得经 barrel 导入，避免 `record.ts ↔ index.ts` 循环依赖），因此 `hooks.test.ts` 对 `./modules/workflow` barrel 的 `readFileInventory` / `writeFileInventory` mock 不再截获管线内部 IO——「归账写清单」组的净状态观察点必须从 mock 调用参数改为读回临时项目内真实 `workflow.json`。

#### 待测功能

- runRecordFiles(): void — stdin 读取 + 全量 catch-all（stderr 诊断、exit 0）；事件流程收敛为：提取 raw ops（空则提前返回）→ `resolveChangeDir` → 委托 `recordFileOps`（签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| record-files — 归账写清单（持久化观察点改造） | 废弃 | 既有「归账写清单」组经 `mockWriteFileInventory` 调用参数观察净状态、经 `mockReadFileInventory` 断言未读的断言形态（管线内移后 barrel mock 不再截获内部 IO） | 废弃 |
| record-files — 归账写清单（持久化观察点改造） | 新增 | 同组语义断言改为读回临时项目内真实 `workflow.json`：折叠规则、source last-writer-wins/清除、绝对路径归一化、越界丢弃、自污染排除、legacy 报错吞并逐条保持（AC-4） | 新增 |
| record-files — gitignore 过滤接入 | 正向 | 已绑定 session，临时项目根 `.gitignore` 含 `.claude/`，Write `.claude/agent-memory/x.md` → 清单 written 不含该路径且其余净状态不变（AC-2） | 新增 |
| record-files — gitignore 过滤接入 | 正向 | 同前置 Bash `rm .claude/x.md` → deleted 桶不含该路径（过滤对 delete/revert 统一生效） | 新增 |
| record-files — gitignore 过滤接入 | 正向 | 同前置 Write `src/a.ts` → 照常入清单（未忽略路径透明通过） | 新增 |
| record-files — gitignore 过滤接入 | 边界 | 临时项目祖先链全无 `.gitignore` → 归账行为与现状一致，hook exit 0、stdout 无阻塞输出（AC-2 现状回归） | 新增 |
| record-files — gitignore 过滤接入 | 边界 | `.gitignore` 读取失败（fail-open）→ 路径保留入清单，hook exit 0 不阻塞 | 新增 |

既有 describe 组（`record-files — phase_next 建绑 (AC-3)`、`main — record-files 子命令分发` 及 protect-files / static-check 各组）全部保持；其中建绑组内「不产生清单 IO」类断言随观察点改造同步调整为真盘形态。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `node:fs.readFileSync`（stdin 与 `.gitignore` 共用 mock 面） | mock 按参数分流：fd 0 返回事件 JSON；字符串路径转发真实 `readFileSync`（`.gitignore` 真盘装载，缺失抛 ENOENT → 该层无规则），避免过滤器读到事件 JSON 文本 | record-files 全部 describe |
| `./lib/session-registry` | `bindSession` / `lookupChange` mock（会话绑定不落真实 tmp 注册表） | 建绑与归账 describe |
| `./lib/config`、`./commands/run-static-analysis` | 保持既有 mock（protect-files / static-check 场景与本次变更无关，仅维持测试文件可加载） | 既有非 record-files describe |
| `process.stderr.write` / `process.exit` | spy 拦截（吞错与 exit-0 断言） | 异常与吞错场景 |

---

## 集成测试

### PostToolUse 事件 → record-files 命令 → 归账管线（gitignore 过滤）→ workflow.json → `plugins/dev-team/bin/src/hooks.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/hooks.ts` | 触发方（子命令分发至 `runRecordFiles`） |
| `plugins/dev-team/bin/src/commands/record-files.ts` | 协议适配（stdin 解析、`phase_next` 绑定识别、raw ops 提取、错误吞并 exit-0） |
| `plugins/dev-team/bin/src/modules/workflow/files/record.ts` | 归账管线（规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘） |
| `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts` | 过滤器（层级装载与判定） |
| `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 持久化原语（读 / 折叠 / 写） |

**关联AC**: AC-2, AC-4

**关系描述**:

一次真实的 PostToolUse 归账横跨五个模块：hooks 分发层把 stdin 事件交给命令层，命令层完成协议解析与会话归属后，把原始 ops 委托给模块层管线；管线内规范化与自污染排除先行，幸候选路径再交由 gitignore 过滤器做层级判定，最终经持久化原语落盘。这是本次变更唯一接入过滤器的事件链路，也是「过滤正确性直接决定真实路径是否漏记」的位置：过滤器误判会漏记真实变更路径，过滤器抛错则会打断 hook（违反 MUST NOT 阻塞契约）。值得集成验证的出错模式有三类——其一，命令层与管线层的责任切分是否完整（规范化/排除不再残留于命令层、过滤只存在于管线内）；其二，过滤器 fail-open 时事件是否仍完整归账且 hook 以 exit 0 收场；其三，祖先链全无 `.gitignore` 的多数真实项目里行为是否与现状逐字节一致。迁移后管线内部 IO 不再经 barrel（避免循环依赖），故本关系的净状态断言以临时项目内真实 `workflow.json` 为准，模块间调用不 mock，仅 stdin、会话注册表与 stderr/exit 保留进程边界 mock。

#### 场景: gitignore 命中路径不入清单

前置条件：临时项目根写入 `.gitignore`（含 `.claude/`），session 已由 `phase_next` 事件绑定 change，change 下存在含 `files` 的合法 `workflow.json`。输入：已绑定 session 的 Write 事件（`file_path` 指向被忽略路径）。预期：落盘净状态不含该路径，hook exit 0、stdout 无输出、stderr 无诊断。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | Write `.claude/agent-memory/x.md`（根 `.gitignore` 命中）→ `workflow.json.files.written` 不含该路径，既有净状态键不变 | 新增 |
| 正向 | Bash `rm .claude/x.md` → deleted 桶不含该路径（过滤对 delete / revert 统一生效） | 新增 |
| 边界 | 净状态中已存在的历史 ignored 路径（预置入桶）在后续无关事件归账后原样保留（过滤不回溯清洗存量） | 新增 |

#### 场景: 未忽略路径照常归账（过滤透明性）

前置条件同上（存在 `.gitignore` 但目标路径未命中）。输入：普通源码路径的写/删事件序列。预期：归账结果与未接入过滤器时一致——净状态、source 盖章、保留键纪律逐条不变（AC-4 回归在新链路上重验）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | Write `src/a.ts` → `files.written` 含 `src/a.ts`（相对项目根 POSIX 化） | 新增 |
| 正向 | 同 session 事件序列 write → delete → write → 净状态按折叠规则收敛 | 新增 |
| 正向 | 事件携带 `agent_type` → `source[path]=agent_type`；随后主会话重写 → source 清除（last-writer-wins 迁移回归） | 新增 |
| 边界 | 越界路径（`../outside.ts`）与 `openspec/**`、`workflow.json` 自污染路径 → 不入桶且不动磁盘清单 | 新增 |

#### 场景: 祖先链全无 `.gitignore` 时行为不变且 hook 不阻塞

前置条件：临时项目（默认无任何 `.gitignore`，多数真实项目形态）。输入：常规归账事件与异常注入。预期：归账行为与现状一致；任何过滤层异常不阻断工具调用。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | 无任何 `.gitignore` 的项目内写事件 → 照常入桶、exit 0、stdout 无阻塞决策输出（AC-2 现状回归） | 新增 |
| 异常 | 根 `.gitignore` 不可读（读取失败 fail-open）→ 路径保留入清单，stderr 出现 `record-files:` 前缀诊断，exit 0 | 新增 |
| 异常 | 目标 change 无 `workflow.json` / 缺 `files`（legacy）→ stderr 诊断、exit 0、磁盘无半写状态（既有吞错语义回归） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `node:fs.readFileSync` | 按参数分流：fd 0 返回事件 JSON，字符串路径（`.gitignore` 装载）转发真实读取 | 全部场景 |
| `./lib/session-registry` | `bindSession` / `lookupChange` mock（替代真实 tmp 注册表读写） | 全部场景 |
| `process.stderr.write` / `process.exit` | spy 拦截 | 异常路径类型 |

### change_files 命令 → append/set 模块语义 → workflow.json → `plugins/dev-team/bin/src/commands/change-files.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/change-files.ts` | 触发方 / 校验层（options 校验、`resolveChangeDir`、委托与返回投影） |
| `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 语义实现方（`appendFileOps` / `setFileBuckets` 与读 / 写原语） |

**关联AC**: AC-4, AC-5

**关系描述**:

`change_files` 是与归账管线并列的第二条写路径：命令层校验通过后把净状态修正整体委托给模块层 `appendFileOps` / `setFileBuckets`，持久化一律经 `readFileInventory` + `writeFileInventory`。本次变更把该链路的语义实现从命令层内联函数改为模块函数，风险集中在两点——委托后语义是否逐条不变（折叠合并、source 保留 / 清理、硬报错），以及该通道是否被误接 gitignore 过滤（它是 hook 漏记与存量噪音的人工修正兜底，过滤会使兜底失效；结构上 `appendFileOps` / `setFileBuckets` 无 `projectRoot` 入参，测试需以「存在 `.gitignore` 时 ignored 路径仍入桶」验证该边界在组合层面成立）。可能的出错模式是实现时顺手在命令层加了过滤调用、或委托时改用了 `foldFileOps`（其「无 agentType 清 source」语义与 append 不同，会静默清掉审计来源）。

#### 场景: ignored 路径经 change_files 仍可补录

前置条件：临时项目根存在 `.gitignore`（含 `.claude/`，与记录器过滤同一实样），change 及合法 `workflow.json` 已建。输入：`op=append` / `op=set` 携带被忽略路径。预期：路径照常入桶并落盘，返回投影含之（AC-5）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | append `written: ['.claude/memory.md']` → 入桶并落盘，返回净状态含之 | 新增 |
| 正向 | set `written` 含被忽略路径 → 整桶覆写照常生效 | 新增 |
| 边界 | 同一被忽略路径先 append 再被记录器事件折叠 → 净状态符合折叠规则，无过滤干预痕迹 | 新增 |

#### 场景: 委托后 append/set 语义回归

前置条件：临时项目 + `runChangeCreate` fixture（既有真盘模式）。验证内容：既有四组用例（append 折叠合并与 source 保留、set 整桶覆写与 source 清理、三类硬报错、schema 形态校验）在语义实现迁至模块后逐条保持，无断言修改（AC-4）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 既有 append 用例（新路径入桶、既有路径去重保留 source、deleted 折叠、同批去重）语义不变 | 新增 |
| 正向 | 既有 set 用例（桶覆写、未提供桶不动、source 清理与保留、`written: []` 清桶）语义不变 | 新增 |
| 异常 | 既有硬报错用例（change 不存在 / JSON 非法 / 缺 `files`）抛错文案与不落盘行为不变 | 新增 |

##### Mock策略

<!-- 无跨进程边界 Mock：临时项目真盘读写，模块间调用不 mock，省略此小节。 -->

### workflow barrel → 命令层消费（导出面与无残留契约） → `plugins/dev-team/bin/src/modules/workflow/index.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/modules/workflow/index.ts` | 出口方（barrel re-export） |
| `plugins/dev-team/bin/src/modules/workflow/files/record.ts` | 实现方（`recordFileOps`） |
| `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 实现方（`appendFileOps` / `setFileBuckets` 与既有原语） |
| `plugins/dev-team/bin/src/commands/record-files.ts`、`plugins/dev-team/bin/src/commands/change-files.ts` | 消费方（无实现残留、无 shim 的扫描对象） |

**关联AC**: AC-3, AC-4

**关系描述**:

barrel 是命令层消费模块语义的唯一合法通道，本次扩充后其导出面与两个命令文件的内联实现删除互为镜像：出口多了三个函数，命令层必须恰好少三份实现且不留转发 shim。该约束用 grep 在 AC-3 中表述，这里将其测试化为源码文本断言（读取两命令文件源码、断言迁移标识符零残留），与导出面的精确断言（恰好 7 个运行时导出、gitignore API 不在列）共同构成结构性防回归：任何一侧回潮（命令层重新内联折叠语义、或 barrel 误导出仅供管线内部消费的 gitignore API 触发 knip dead export）都会在此被拦截。出错模式还有引用复制（barrel 手写包装而非 re-export，导致行为分叉），以「与源模块同引用（toBe）」断言堵住。

#### 场景: barrel 扩充导出与引用同一

前置条件：无（纯模块导入）。验证内容：三个新导出存在、为函数、与源模块导出同引用；导出面精确含 gitignore API 排除。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `recordFileOps` / `appendFileOps` / `setFileBuckets` 与 `files/record.ts`、`files/file-inventory.ts` 直接导入 toBe 同引用 | 新增 |
| 废弃 | 旧「运行时导出键恰好等于 4 个函数名」断言随扩充移除 | 废弃 |
| 边界 | 运行时导出键恰为 7 个、不含 `loadGitignoreFilter` / `isGitIgnored`（不进 barrel 的结构锁定） | 新增 |

#### 场景: 命令层无迁移残留、无 re-export shim

前置条件：无（真实 fs 读取源码文本）。验证内容：AC-3 的 grep 条款测试化。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `commands/record-files.ts` 源码不含 `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` / `foldFileOps` / `writeFileInventory` 标识符 | 新增 |
| 正向 | `commands/change-files.ts` 源码不含 `applyAppend` / `applySet` / `dedupe` 标识符；两文件均无对迁移函数的 re-export shim | 新增 |

##### Mock策略

<!-- 无 Mock：模块导入断言 + 真实 fs 源码文本读取，省略此小节。 -->

---

## 不可测试项

- AC-6（`version` 提升、build 刷新 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/`） — **原因**: 版本号与构建产物是进程外的构建行为，单元/集成测试无法验证；由构建输出与 git status 在验收阶段核验。
- `ignore` 包的单文件 gitignore 语法与匹配语义（`*` / `**` 通配、字符类、负模式求值、目录递归、尾随 `/`、后匹配覆盖等） — **原因**: 由 `ignore` 包自身及其测试保障，遵循「不逐项验证库自带语义」约定；本设计只测自研装载与层级组装层。
- PostToolUse hook 在 Claude / Cursor 宿主中的真实端到端生效（hooks.json matcher、hook 进程 spawn、双平台共用实现） — **原因**: 需真实宿主进程环境；本次 hooks.json 无键变更，进程内的分发与归账语义已由 `hooks.test.ts` 覆盖。
- 每事件按祖先链装载 `.gitignore` 的性能开销（hook 延迟） — **原因**: 非功能属性、无确定性断言；由「惰性构建一次、层缓存复用、缺失层短路」的结构性用例间接约束。
