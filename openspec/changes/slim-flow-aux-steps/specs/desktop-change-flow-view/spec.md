# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的 step 节点 SHALL 呈现运行态（pulse 视觉），ToolStep 节点、Git 节点与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（分组徽章呈 Agent / Tool / Git / Gate，四类可区分）。运行态呈现 SHALL 与既有节点模型并存：step 节点 running / failed / stopped / passed 四态如实；`PIPELINE_PHASES` 10 列布局、边表读取规则与空列恒定 MUST NOT 因运行态呈现改变。eval 节点与 active 节点 SHALL 不再上图（相位节点整体退役，见「流程图节点模型与两分类」）。

运行收口后，图 SHALL 保持**步节点常驻渲染**：已落库执行图节点（`runs[].nodes` 二十二值全词汇）自统一视图派生常驻上图，与运行中实时面走**同一条派生路径**；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的节点全部保留），archive 相位节点随当前 run 常驻。节点身份键 SHALL 为 `(phase, attempt, step, instanceNo)`（自后端直读），前端 `runStepNodes` 的「槽位配对 + `:seq` 后缀堆叠」机制 SHALL 退役。归档步词 SHALL 落到 `RunStepGroup` 的 git / exec / agent 分类（`GROUP_LABEL` 同步），`archive-panel` / `StageList` 六段清单 SHALL 退役（归档步作为 archive 列 step 节点呈现，不设独立面板）。步词汇归一（库读史 snake 词 → 活面 camel 词，`normalizeKind` / `stepWord` 单点）SHALL 保持二十二值逐词对应：`mutation_test` ↔ `mutationTest`、`archi_check` ↔ `archiCheck`、`cleanup` ↔ `cleanup` 与 `clean` ↔ `clean` 在案、`mutationTest` 与 `archiCheck` 归 exec 分类、`cleanup` 与 `clean` 归 git 分类（现有步骤渲染直接吃，零渲染改动）。Cleanup 步的 detail 警告（清理失败 / 跳过注记）SHALL 随节点 detail 面呈现。

**词汇冻结与存量解码面 SHALL 零改动**：后端的相位表辅助步（`phase_start` / `phase_log` / `verdict_gate`）退场后，新 run 不再产出这三类节点，但**旧库行仍会解码出这三词**——`STEP_LABEL` 的穷尽 Record、`normalizeKind` / `stepWord` 的 snake ↔ camel 映射、`runStepGroup` 的 `gate` 分类（`verdictGate` / `retryGate` / `whitelistGate` 三词归 `gate`）SHALL 逐键保留，MUST NOT 因「不再产出」而删除键、MUST NOT 收缩分组。前端 MUST NOT 为「辅助步退场」新增任何分支或占位：不存在的节点天然不上图，既有渲染零改动即可。

#### Scenario: 运行态与四类并存

- **WHEN** implement#2 相位运行中且历史上有 fail 与 active_phase 残留，archive 相位随后进入
- **THEN** 图上同时呈现：历史 step 节点（终态）、当前节点（pulse 运行态）及同相位的 ToolStep / Git / Agent 节点（类型可辨、状态如实），无 eval 节点、无 active 节点；历史 run 若含 gate 三门节点，其 gate 徽章照常呈现

#### Scenario: archive 步节点上图

- **WHEN** archive 相位运行中（或已收口）
- **THEN** archive 列呈现 9 步节点：Preflight / Seal 为 exec、Commit / Rebase / Merge / Cleanup / Finalize 为 git、SpecSync / ResolveConflicts 为 agent；git 步分类徽章可辨，无独立 ArchivePanel / StageList；无 `phaseStart` / `phaseLog` 节点

#### Scenario: 收口后常驻不回落

- **WHEN** run 以 archive 相位完成收口（或 fail / stopped 收口）后刷新 / 重开详情
- **THEN** 已落库执行图节点常驻上图（含 archive 列，终态样式、无 pulse），派生自统一视图库读史、与运行中同一转换函数；无「回落后步节点消失」的形态跳变

#### Scenario: 稳定身份键渲染与 feedback 多实例

- **WHEN** 某 attempt 内 static-check 失败触发 executor 修复（feedback 重跑）后刷新详情
- **THEN** executor 首次与修复各为 `instanceNo` 递增的独立节点，feedback 边自边表呈现；无「槽位配对 + `:seq` 堆叠」逻辑残留，同键多实例不串扰

#### Scenario: 全史叠加与在飞叠加

- **WHEN** 某 change 历经两次 run 后第三次 run 运行中（含 archive 相位）
- **THEN** 图上前两 run 的节点全史常驻，第三 run 活节点（pulse）叠加其上；archive 列节点随 run 归属分层可溯

#### Scenario: mutationTest 步词汇归一

- **WHEN** test-execution 相位 MutationTest 步上图（活面或库读史）
- **THEN** `mutation_test` ↔ `mutationTest` 归一在 `normalizeKind` / `stepWord` 单点可考，节点落 exec 分类徽章；现有步骤渲染零改动消费

#### Scenario: cleanup 步词汇归一与警告呈现

- **WHEN** archive 相位 Cleanup 步上图（清理成功 / 失败警告 / 幂等跳过三态任一）
- **THEN** `cleanup` ↔ `cleanup` 归一在 `normalizeKind` / `stepWord` 单点可考，节点落 git 分类徽章（标签呈现）；失败警告或跳过注记随节点 detail 面可读，节点状态恒 `passed`

#### Scenario: initialize clean 步词汇归一

- **WHEN** initialize 相位 Clean 步上图（活面或库读史）
- **THEN** `clean` ↔ `clean` 归一在 `normalizeKind` / `stepWord` 单点可考，节点落 git 分类徽章（人读标签在案）；现有步骤渲染零改动消费

#### Scenario: 存量辅助步节点仍可解码

- **WHEN** 打开一个在辅助步退场前落库的 run 详情（节点含 `phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate`）
- **THEN** 五词仍能经 `normalizeKind` / `stepWord` 归一、`STEP_LABEL` 取到非空人读文案、group 分别落 exec / gate；节点照常渲染为静默节点（`role` 与 `outputRef` 皆 null），点击不触发抽屉

### Requirement: 节点点击判据与 stdout 输出面板

runtime step 节点点击 SHALL 按信息源分派判据：`role !== null` → 单列会话转录（复用 `SessionTranscriptPanel`）；`outputRef !== null` → 单列 stdout 面板（新组件）；两者皆 null → 不可点击（不 `onSelect`）。该判据 SHALL 为转换 / 交互层纯函数可测（不依赖 react 宿主）。

判据本身 SHALL **零改动**（`role === null && outputRef === null` 才拦）：脚本步节点在运行中即携 `outputRef`（后端前置上抛，见 desktop-change-orchestration「脚本步运行中输出引用前置上抛」），故运行中的 static-check / test-execution / mutation-test 节点自然可点击，前端 MUST NOT 为「运行中」另开分支或另设判据。该判据对存量辅助步节点（`phaseStart` / `phaseLog` / 三门 gate）自然成立——它们 `role` 与 `outputRef` 皆 null，落「不可点击」臂，MUST NOT 为它们新增特例。

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
- **THEN** 抽屉分派到会话转录单列，不发起 `read_file` / `readExecOutputDelta`

#### Scenario: 有 outputRef 节点分派到 stdout

- **WHEN** 点击 `outputRef !== null` 的 runtime step 节点（static-check / test-execution / mutation-test）
- **THEN** 抽屉分派到 stdout 面板，经 `readExecOutputDelta(root, name, outputRef, 0)` 与 `readExecCommand(root, name, outputRef)` 渲染命令头行与脚本真实输出

#### Scenario: 运行中节点可点击且实时跟随

- **WHEN** 点击一个 `status === 'running'` 且已携 `outputRef` 的 static-check 节点（进程尚未结束）
- **THEN** 抽屉打开并渲染迄今输出；面板订阅该 `.log` 的 watch 信号，信号到达（500ms 防抖后）再取一次增量、游标推进、文本追加；点击判据源码零改动

#### Scenario: 收口后整读收敛

- **WHEN** 被查看的步从 `running` 转为 `passed` / `failed` / `stopped`
- **THEN** 面板退订（`watch_unsubscribe` 恰一次）、把 `fromOffset` 归零并整读一次；重排后的权威内容与整读结果逐字一致，不残留 live 交错序

#### Scenario: 双 null 节点不可点击

- **WHEN** 点击存量 run 的 `phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate` 节点，或任一新 run 的 `commit` / `rebase` / `merge` / `finalize` / `preflight` / `seal` 节点（`role` 与 `outputRef` 皆 null）
- **THEN** 不触发 `onSelect`、抽屉不打开，节点本身仍照常渲染

#### Scenario: stdout 面板空态与错误态

- **WHEN** `readExecOutput` 返回 `None`（exec_id 未知 / 非法）、`readExecOutputDelta` 返回 `None`，或任一 invoke 被拒绝
- **THEN** stdout 面板呈空态 / 错误态提示，流程图与抽屉不报错、不崩溃；运行中零字节呈「运行中，等待输出…」，非运行无记录呈「（无输出记录）」，两者不混同

#### Scenario: 命令头行与复制

- **WHEN** 打开某 test-execution 步的 stdout 面板（sidecar 含两个 `=== plan … ===` 段）
- **THEN** 命令文本按多行原样显示在面板最上方（不被截断成单行），复制按钮可取其全文；对无 sidecar 的老记录呈「（命令未记录）」且内容面照常渲染

#### Scenario: CR 进度行不堆残行

- **WHEN** 输出含以 `\r` 刷新同一行的进度序列（如多行 `Compiling … \r`）
- **THEN** `<pre>` 中只呈现每行最后一次 `\r` 之后的内容，无残行堆叠；归一笔为纯函数且有直测用例

#### Scenario: 粘底跟随与上滑停止

- **WHEN** 运行中新输出到达，用户先停留底部、随后向上滚动查看历史
- **THEN** 前者自动滚动至最新行；后者停止自动跟随（新输出不拽回底部）；用户回到底部后恢复跟随；判据纯函数有直测用例

### Requirement: 步级耗时与 token 节点呈现

runtime step 节点（`RunStepNode`）SHALL 在标题行下新增**常驻行**呈现步级耗时与 token 消耗（非 HoverCard，决策拍板）：耗时 = `endedAt - startedAt` 前端计算（后端不落 duration）；token 行仅 `group === 'workerAgent'` 节点渲染（`tokenInput` / `tokenOutput` 两值缩写呈现，缺席 "—"）。`RuntimeFlowNode` SHALL 透传 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` 四字段（`runStepNodes` / `stepFromRecord` 直读透传，词汇归一单点口径不变）。

呈现语义 SHALL 为：

- **静态耗时**：已收口步（`endedAt !== null` 且 `startedAt > 0`）显格式化耗时；`startedAt === endedAt`（**存量**单 emit 辅助步节点——新写面已无生产例）按 0ms 呈现，MUST NOT 显「0ms」耗时缀；
- **running 走秒**：**仅活面（在飞 run）节点**且 `status === 'running'` 时以 `Date.now() - startedAt` 每秒刷新呈现（节点组件自持 interval 局部重渲染——前端全库首例，xyflow 节点独立渲染不惊动画布；节点离开 running 态或卸载即清 timer，收口后回落静态耗时）；库读史 `status=running` 残留节点（run 已死）MUST NOT 走秒，恒静态呈现；
- **缺席兜底**：`startedAt <= 0`（旧库行）或 `endedAt == null` 的非 running 态显 "-"（决策拍板：旧数据不从 turn 行回填）；token 缺席显 "—"。

毫秒 → 人读格式化 SHALL 为独立纯函数模块（新写，`1.2s` / `3m20s` 类分级；无既有先例可复用），vitest 直测边界；判定「缺席 → "-"」逻辑 SHALL 收格式化侧单点（MUST NOT 散落 `0` 魔数比较）。布局（10 列手工坐标 / subflow）、边表读取、节点点击判据与抽屉交互 MUST NOT 因常驻行改变；测试挂钩 SHALL 使用 data-testid。

#### Scenario: 常驻行静态耗时与 token

- **WHEN** 打开含已收口 executor 步（`startedAt=T0` / `endedAt=T1` / `tokenInput=12000` / `tokenOutput=3000`）的 change 详情
- **THEN** 该节点标题下常驻行呈现格式化耗时 `T1 - T0` 与 tokens in / out 缩写；无 HoverCard 依赖、点击判据不变

#### Scenario: token 行仅 WorkerAgent 组

- **WHEN** 图上同时呈现 executor 节点与 static-check 工具步节点（后者四字段中 token 恒 null）
- **THEN** 工具步 / Gate / Git 节点不渲染 token 行（仅耗时）；workerAgent 组节点渲染 tokens in / out 两值

#### Scenario: running 走秒与收口回落

- **WHEN** run 运行中某 WorkerAgent 节点 running（活面，`startedAt` 在案、`endedAt=null`）
- **THEN** 该节点耗时以每秒 tick 的走秒值呈现；该步收口（状态离开 running）后 timer 卸载、回落静态耗时；同一时刻其余节点零重渲染

#### Scenario: 库读史残留与旧数据兜底

- **WHEN** 打开详情呈现本变更前落库的 run 节点（`startedAt=0` / `endedAt=null`）或库读史 status=running 残留节点
- **THEN** 耗时显 "-"（不走秒、不报错）；token 显 "—"；其余节点呈现不受影响

#### Scenario: 格式化纯函数直测

- **WHEN** 以边界值（0ms / 999ms / 1s 级 / 分秒级 / 时分级）调用格式化函数
- **THEN** 输出符合分级规则（0ms 由调用侧省显、缺席入参出 "-"），无需挂载 react 组件即可断言

#### Scenario: 存量单 emit 节点 0ms 呈现

- **WHEN** 打开含**存量**辅助步节点（`startedAt == endedAt`，如旧 run 的 `phaseStart`；新写面已无此生产例）的详情
- **THEN** 该节点耗时按 0ms 处理、不显耗时缀；token 行不渲染（非 workerAgent 组）；渲染零特判分支新增

### Requirement: initialize 相位列与 clean 步词呈现

initialize 相位（表头一次性基线检查相位，见 desktop-change-orchestration「initialize 基线检查相位执行体」）SHALL 以**零结构改动**方式呈现：initialize 相位列随 `detail.pipeline` 出线自动成为首列（列头相位名文案补 initialize 人读标签），Clean / StaticCheck / TestExecution 步节点以既有 runtime step 节点渲染（四类分组徽章可辨、running / failed / stopped / passed 状态如实）；基线检查输出经节点 `outputRef` 的既有 stdout 面板回查（`exec_output_path` 命中 baseline 树 + `read_file` / `read_exec_output_delta` 整读）；initialize 失败的 run 终态 reason（Error / Fail 分文案 + 报告目录指引）经既有终态呈现面直达。前端步词呈现 SHALL 补 `clean` 与 `archi_check` 两线词：`STEP_LABEL` 穷尽 Record 补 `clean` 键（人读文案）与 `archiCheck` 键（人读文案「架构检查」）、`normalizeKind` / `stepWord` 单点补 `clean` ↔ `clean` 与 `archi_check` ↔ `archiCheck` 两组映射、分类徽章覆盖（`RunStepGroup` / `GROUP_LABEL`：`clean` 归 git、`archiCheck` 归 toolStep），现有步骤渲染零改动直接吃。MUST NOT 为 initialize 引入新节点类型、新面板或独立交互入口。辅助步退场后 initialize 相位的新 run 恰为 Clean / StaticCheck / TestExecution 三节点（无 `phaseStart` / `phaseLog`），MUST NOT 为缺失的两节点补占位或补空态。

#### Scenario: initialize 列与步节点零结构呈现

- **WHEN** 打开一个已跑过 initialize 相位（绿或红）的 change 详情
- **THEN** initialize 列为首列（相位名人读文案在案），Clean（git 分类徽章）/ StaticCheck / TestExecution 步节点按序呈现、状态如实；无 `phaseStart` / `phaseLog` 占位、无新节点类型、无独立面板

#### Scenario: 基线失败终态与回查

- **WHEN** initialize 相位以 Terminal 失败（static-check 红或 test-execution Error / Fail）收口的 run 详情被打开
- **THEN** run 终态 reason 呈现对应文案（Error / Fail 可辨、含 baseline 报告目录指引）；点击 StaticCheck / TestExecution 节点经既有 stdout 面板回查 baseline 树输出

#### Scenario: clean 步词穷尽补齐

- **WHEN** bindings 再生成后审查 `STEP_LABEL` / `normalizeKind` / `stepWord` / `GROUP_LABEL`
- **THEN** `clean` 与 `archiCheck` 两键在案（人读文案非空）、snake ↔ camel 归一在单点可考、分别落 git / toolStep 分类；穷尽 Record 键集与二十二词全词汇一致（编译期兜底），其中三辅助步与三门 gate 五键为存量解码词不删

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/run-state.ts` | 词汇归一与分组（**零改动**） | `STEP_LABEL` 穷尽 Record 保留 `phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate` 五键；`normalizeKind` / `stepWord` 五组映射保留；`runStepGroup` 的 gate 分支保留三词归属——旧库行解码必需，MUST NOT 因「新 run 零产出」删键 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | 静默节点判据（**零改动**） | `role === null && outputRef === null` 判据逐字不变；不存在的节点天然不上图，零新增分支 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 步节点渲染（**零改动**） | 耗时 / token 常驻行与 running 走秒逐字不变；存量 `startedAt == endedAt` 节点的 0ms 呈现走既有格式化判据（`0ms` 不显耗时缀） |
| `packages/desktop/src/views/changes/flow/{run-state,run-step-node,change-flow-graph}.test.tsx` | 用例面（**零改动**） | 既有渲染用例期望值零改写（`phaseStart` / `phaseLog` / `verdictGate` 样本节点仍须可解码 + 静默）；本变更 MUST NOT 新增前端用例——「新 run 三节点 initialize 列」由既有渲染路径天然承载（渲染层零源码改动），用例面随源码一并零 diff |
