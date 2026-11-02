# desktop-change-flow-view Specification（delta: change-detail-live-stdout）

## MODIFIED Requirements

### Requirement: 节点点击判据与 stdout 输出面板

runtime step 节点点击 SHALL 按信息源分派判据：`role !== null` → 单列会话转录（复用 `SessionTranscriptPanel`）；`outputRef !== null` → 单列 stdout 面板（新组件）；两者皆 null → 不可点击（不 `onSelect`）。该判据 SHALL 为转换 / 交互层纯函数可测（不依赖 react 宿主）。

判据本身 SHALL **零改动**（`role === null && outputRef === null` 才拦）：脚本步节点在运行中即携 `outputRef`（后端前置上抛，见 desktop-change-orchestration「脚本步运行中输出引用前置上抛」），故运行中的 static-check / test-execution 节点自然可点击，前端 MUST NOT 为「运行中」另开分支或另设判据。

stdout 面板 SHALL 经 `hooks/use-exec-output.ts` 取数，入参 SHALL 为 `(root, changeName, execId, status)`（`status` 取节点 `status`，由 `DetailDrawer` 下传；`changeName` 取 `detail.name`，非 id）。取数 SHALL **全部经 `src/lib/ipc.ts` 代理面**（MUST NOT 绕回裸 `commands.*`），命令面为三件：

- `ipc.readExecCommand(root, changeName, execId) -> string | null`：执行命令文本（sidecar）；老记录无 sidecar ⇒ `null`（面板呈「命令未记录」占位，MUST NOT 视为错误、MUST NOT 阻断内容面）。
- `ipc.readExecOutputDelta(root, changeName, execId, fromOffset) -> ExecOutputChunk | null`：增量读，返回 `{ text, nextOffset }`；`fromOffset = 0` 即全量（整读亦经本命令，MUST NOT 并存第二条整读路径）。**offset 的唯一产出方是后端**：前端 MUST NOT 自行按字符串长度累加游标（「JS 字符数 vs UTF-8 字节数」的口径分裂因此不可表达）。
- `ipc.execOutputPath(root, changeName, execId) -> string | null`：布局派生的 `.log` 绝对路径，仅供 watch 订阅寻址（目录名知识 MUST NOT 下沉前端）。

跟随语义 SHALL 由**一条状态规则**承载：`status === 'running'` → 订阅 watch 增量跟随（`useFileWatchSignal`，`armKey = root` 与 `execId` 以 NUL 拼接、非 running 时 `null`；`SIGNAL_DEBOUNCE_MS = 500` 沿用共享件，MUST NOT 自建 `Channel` / 直呼 watch 命令 / 自建防抖）；`status` 转为非 running → 退订并把 `fromOffset` 归零后整读一次。该规则同时吸收两件事：收口时权威内容可能重排（test-execution 的「stdout 前缀 + stderr 后缀」覆盖 live 交错流）、文件被覆盖后旧 offset 越界（后端夹到 0，前端零特判）。

面板呈现 SHALL 为三段：命令头行（最上方）、输出文本区、状态提示。口径：

- **命令头行**：多行原样呈现（`whitespace-pre-wrap`，MUST NOT 单行截断把区分信息藏进 hover）+ 复制按钮（复用既有 `CodeBlockCopyButton`）；`null` ⇒「（命令未记录）」。
- **三态可分辨**：`status === 'running'` 且文本为空 ⇒「运行中，等待输出…」；非 running 且文本为 `null` / 空 ⇒「（无输出记录）」；invoke reject ⇒ 错误态（MUST NOT 炸图、MUST NOT 阻断抽屉其余内容）。MUST NOT 把「运行中 0 字节」与「无输出记录」合并为同一文案。
- **`\r` 归一**：渲染前对文本做归一（仅保留最后一个 `\r` 之后的内容），使 cargo / 进度行不在 `<pre>` 中堆成残行；归一笔为纯函数（可直测）。
- **粘底跟随**：输出增长时若用户停留在底部则自动滚动到底；用户上滑即停止粘底，回到底部恢复。判据为纯函数（可直测），MUST NOT 引入新依赖。
- miss（返回 `null`）非错误，仍为空态；`readExecOutput`（整读）命令保留在原位，MUST NOT 删除。

#### Scenario: 有 role 节点分派到转录

- **WHEN** 点击 `role !== null` 的 runtime step 节点
- **THEN** 抽屉分派到会话转录单列，不发起 `read_exec_output` / `readExecOutputDelta`

#### Scenario: 有 outputRef 节点分派到 stdout

- **WHEN** 点击 `outputRef !== null` 的 runtime step 节点（static-check / test-execution）
- **THEN** 抽屉分派到 stdout 面板，经 `readExecOutputDelta(root, name, outputRef, 0)` 与 `readExecCommand(root, name, outputRef)` 渲染命令头行与脚本真实输出

#### Scenario: 运行中节点可点击且实时跟随

- **WHEN** 点击一个 `status === 'running'` 且已携 `outputRef` 的 static-check 节点（进程尚未结束）
- **THEN** 抽屉打开并渲染迄今输出；面板订阅该 `.log` 的 watch 信号，信号到达（500ms 防抖后）再取一次增量、游标推进、文本追加；点击判据源码零改动

#### Scenario: 收口后整读收敛

- **WHEN** 被查看的步从 `running` 转为 `passed` / `failed` / `stopped`
- **THEN** 面板退订（`watch_unsubscribe` 恰一次）、把 `fromOffset` 归零并整读一次；重排后的权威内容与整读结果逐字一致，不残留 live 交错序

#### Scenario: 双 null 节点不可点击

- **WHEN** 点击 phaseStart / phaseLog / verdictGate / retryGate / whitelistGate / commit / rebase / merge / finalize / preflight / seal 节点（`role` 与 `outputRef` 皆 null）
- **THEN** 不触发 `onSelect`、抽屉不打开，节点本身仍照常渲染

#### Scenario: stdout 面板空态与错误态

- **WHEN** `readExecOutput` 返回 `None`（exec_id 未知 / 非法）、`readExecOutputDelta` 返回 `None`，或任一 invoke 被拒绝
- **THEN** stdout 面板呈空态 / 错误态提示，流程图与抽屉不报错、不崩溃；运行中零字节呈「运行中，等待输出…」，非运行无记录呈「（无输出记录）」，两者不混同（既有「空态 / 错误态」不炸图语义不变，文案按 `status` 细分）

#### Scenario: 命令头行与复制

- **WHEN** 打开某 test-execution 步的 stdout 面板（sidecar 含两个 `=== plan … ===` 段）
- **THEN** 命令文本按多行原样显示在面板最上方（不被截断成单行），复制按钮可取其全文；对无 sidecar 的老记录呈「（命令未记录）」且内容面照常渲染

#### Scenario: CR 进度行不堆残行

- **WHEN** 输出含以 `\r` 刷新同一行的进度序列（如多行 `Compiling … \r`）
- **THEN** `<pre>` 中只呈现每行最后一次 `\r` 之后的内容，无残行堆叠；归一笔为纯函数且有直测用例

#### Scenario: 粘底跟随与上滑停止

- **WHEN** 运行中新输出到达，用户先停留底部、随后向上滚动查看历史
- **THEN** 前者自动滚动至最新行；后者停止自动跟随（新输出不拽回底部）；用户回到底部后恢复跟随；判据纯函数有直测用例

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/hooks/use-exec-output.ts` | 运行中 / 收口后输出取数（重写） | 入参 `{ root, changeName, execId, status }`；**全部经 `ipc`**（`readExecCommand` 一次 / `readExecOutputDelta(fromOffset)` 多次 / `execOutputPath` 供 watch）；`fromOffset` 仅取自后端 `nextOffset`（前端零字节计数）；`status === 'running'` ⇒ `useFileWatchSignal({ armKey: root\0execId, resolvePath, onSignal })`，否则 `armKey = null` 且整读一次 + offset 归零；导出 `{ command, text, error }`（`command` 失败不阻断 `text`）；`\r` 归一为文件内私有纯函数（供面板复用同一口径） |
| `packages/desktop/src/views/changes/flow/stdout-panel.tsx` | 单列 stdout 面板（扩） | 新增 `status` prop；三段呈现（命令头行 `<pre>` 多行原样 + `CodeBlockCopyButton` / 输出文本区 / 状态提示）；三态文案（「运行中，等待输出…」/「（无输出记录）」/「（命令未记录）」）可分辨；粘底跟随判据为纯函数（`data-testid` 挂钩在案）；错误态不炸图 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 分派与 `status` 下传 | stdout 分派判据（`outputRef !== null`）零改动；给 `StdoutPanel` 增传 `status={node.status}`；会话转录与列头分派零改动 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | 点击判据（零改动） | `role === null && outputRef === null` 拦截逻辑一字不动——运行中节点因后端前置上抛而自然可点 |
| `packages/desktop/src/hooks/use-file-watch-signal.ts` | watch 消费（零改动） | 第二消费方接入不开新参数（`armKey` / `resolvePath` / `onSignal` 三项照用）；`SIGNAL_DEBOUNCE_MS = 500` 单点沿用 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 通知重查（零改动） | `useDebouncedBump` / 有界重试 / 保 last-good 语义不涉及本变更 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 零改动 | 选中态与抽屉装配不变；运行中刷新仍由既有通知链驱动（本变更不引入前端刷新键） |
