# desktop-ipc-runtime-guard Specification

## Purpose

定义桌面端 IPC 调用面的运行期校验契约：由后端命令签名生成的 zod schema（args / result / channel 元数据）、前端装配层 `src/lib/ipc.ts` 的分级校验（参数面 strict 本地拒绝、结果面与 `Channel` 信封 soft-report 原值透传）、漂移报告的进程内聚合与 DEV 汇总告警、schema 只表达接受面的生成约束（禁修饰 / 具名降级哨兵），以及前端运行期 `zod` 依赖的落位与单实例约束。schema 产物的生成链与入库守卫归 desktop-ipc-type-bindings；本能力只管「产物到手后如何被消费与观测」。

## Requirements

### Requirement: 参数面 strict 本地校验（IpcArgsError）

前端调用面 SHALL 在发起 `invoke` 之前，以「从后端接受面生成」的 args schema 对每个命令的参数做 `safeParse` 校验：失败 MUST 在本地抛出 `IpcArgsError`（`extends Error`，携带命令名、jsonPointer 风格字段路径、`expected`、`got` 与中文文案），MUST NOT 发起任何 IPC 调用（零往返）。校验通过后 MUST 以原参数对象发起调用（MUST NOT 用解析后的副本替换调用参数——`Channel` 等非 JSON 可表达参数必须原样保留在 payload 内，「跳过校验 ≠ 剔出对象」）。

- **合法输入零误拒**：args schema 由后端命令签名生成，合法调用 MUST NOT 被拒；MUST NOT 在 schema 上叠加 `.refine()` 等二次校验（重复实现保存期规则）。
- **`null` / `undefined` 面**：必填参数 MUST 拒绝 `null` 与键缺失（Tauri 对缺键直接失败，本地拦截把 ms 级往返提前到 0 ms）；`Option<T>` 无默认值时仍为必填键（值可为 `null`），MUST NOT 用 `.optional()` 放宽。
- **`Channel<T>` 参数**：MUST 跳过值校验（非 JSON 可表达），BUT 该键 MUST 保留在 payload 内且由生成物以具名降级哨兵标注（见「schema 只表达接受面」）。

#### Scenario: 参数类型错误本地拒绝且零往返

- **WHEN** 以数字实参调用一个 `string` 形参的命令
- **THEN** 抛出 `IpcArgsError`，消息含命令名与该参数路径；底层 `invoke` 调用次数为 0

#### Scenario: null 与缺键被抓

- **WHEN** 以 `null` 调用必填 `string` 形参，或以缺键对象调用必填形参命令
- **THEN** 两种形态均抛 `IpcArgsError`；错误信息含 `expected` / `got`

#### Scenario: 合法参数直通且零误拒

- **WHEN** 以生成绑定声明的合法参数调用命令（含 `null` 值给 `Option` 参数、`null` 给 `f64` 参数）
- **THEN** 校验通过、无 `IpcArgsError`、零漂移记录；`invoke` 以原参数对象被调用一次

#### Scenario: Channel 参数跳过校验但保留在 payload

- **WHEN** 调用带 `onEvent: Channel<T>` 参数的命令
- **THEN** `onEvent` 不参与 args 值校验，且出现在 `invoke` 的第二实参对象内（同一引用，未被替换或剔除）

### Requirement: 结果面 soft-report（原值返回）

命令结果 SHALL 以对应 result schema `safeParse` 校验，但校验失败 MUST NOT 抛异常、MUST NOT 阻断返回：SHALL 记录一条漂移报告（`{cmd, path, expected, got}`）后**原值透传**给调用方，使既有手写降级渲染照旧生效（「漂移可见 + UI 不崩」二者兼得）。校验通过时 MUST NOT 产生任何报告。

- 校验 MUST NOT 使用会抛异常的 schema 组合：`z.intersection` 的 `mergeValues` 在同键两侧 `.default()` / `.transform()` 并存时是 throw 而非 issue，soft-report 链路 MUST NOT 因此在自身内部崩掉（生成期禁四类修饰，见「schema 只表达接受面」）。
- 结果面 MUST NOT 在高频路径做同步阻塞式深拷贝或重排（原值透传即天然零拷贝）。

#### Scenario: 脏结果原值透传且产生一条漂移

- **WHEN** 后端返回的对象某字段为字符串而契约声明为数字
- **THEN** 调用方收到的值与后端返回对象逐字等价（同一引用/深相等）；漂移记录新增 1 条，含命令名与字段路径；无异常抛出

#### Scenario: 干净结果零记录

- **WHEN** 后端返回完全符合契约的结果（含 `null` 给可空字段、`unknown` 字段给任意 JSON）
- **THEN** 漂移记录为空；返回值原样交付

#### Scenario: 既有降级渲染照旧生效

- **WHEN** 结果面出现契约外数据（漂移已记录）
- **THEN** 前端既有 renderer / hook 的降级分支仍按原逻辑渲染（MUST NOT 因本能力新增二次兜底或改写既有降级路径）

### Requirement: Channel 信封逐条 soft-report 与幂等包裹

流式命令的 `Channel<T>` 消息 SHALL 在交付调用方 handler 之前逐条校验：失败 MUST 记录单条漂移报告，且 MUST 仍以原消息调用原 handler（不丢流、不断流、不改调用次数与顺序）。同一 `Channel` 实例被重复传入（重订阅 / 复用缓存 channel）时 MUST 幂等：MUST NOT 重复包裹、MUST NOT 重复上报同一条消息。

- 包裹 MUST NOT 改变 `Channel` 的其它可观察行为（`toJSON` / 序号 / 取消订阅路径均不介入）。
- 需要知道「哪些参数是 Channel」的事实 SHALL 来自生成物元数据（逐命令 channel 参数键名），MUST NOT 靠运行期值嗅探（值嗅探在测试替身与多实例场景下不可靠）。

#### Scenario: 脏信封单条上报且流不断

- **WHEN** 频道逐条投递「合法消息 → 契约外消息 → 合法消息」
- **THEN** 调用方 handler 三次均被调用且收到原消息；漂移记录恰好新增 1 条（对应契约外消息），流未中断

#### Scenario: 同一 channel 重复传入只包裹一次

- **WHEN** 同一 `Channel` 实例被同一命令（或重订阅路径）连续传入两次
- **THEN** handler 每次投递只被调用一次，同一条脏消息只产生 1 条漂移记录（计数不因重复传入翻倍）

### Requirement: 错误面三分

IPC 错误面 SHALL 明确三分，且既有呈现不退化：

```
invoke reject: string   —— 后端业务失败（语义与文案不变，既有 error 态照旧）
IpcArgsError            —— 前端本地参数拒绝（可定位命令 + 字段路径）
IpcDriftReport          —— 结果面 / 信封漂移（不是 throw，只上报）
```

`IpcArgsError` MUST 是 `Error` 子类，使既有「`String(err)` 进 error 态」链路零改动即可呈现；MUST NOT 把 `IpcArgsError` 包装成 `Promise.reject("字符串")`（那会与后端 reject 面混同、失去形态区分）。漂移 MUST NOT 通过 throw 或 reject 传播。

#### Scenario: 后端 reject 语义不变

- **WHEN** 后端命令返回业务错误（reject 字符串）
- **THEN** 调用方收到的仍是该字符串（形态、文案、时序不变），不产生 `IpcArgsError`、不产生漂移记录

#### Scenario: 参数错误走既有 error 态

- **WHEN** hook 以非法参数调用命令并被本地拒绝
- **THEN** hook 既有 `String(err)` 路径显示该错误信息（含命令名与路径），无未捕获异常、无 toast 重复

#### Scenario: 漂移不抛

- **WHEN** 结果面出现漂移
- **THEN** 调用 Promise 正常 resolve（原值），零 throw / 零 reject

### Requirement: 装配层零手写镜像与调用点收口

前端 IPC 调用面 SHALL 收敛到单一装配模块（`src/lib/ipc.ts`），其命令集合与参数 / 结果类型 MUST 完全由生成物推导（遍历 `bindings.ts` 的 `commands` 键 + `bindings.zod.ts` 的表），MUST NOT 手写命令清单、MUST NOT 手写参数 / 结果类型镜像。业务代码 MUST NOT 再直接调用 `commands.<name>(...)`（生成物内部与类型派生 `Parameters<typeof commands.x>` / `typeof commands` 除外）；`ipc` 必须对新命令零改动即可覆盖（键遍历自动生效）。

#### Scenario: 业务代码零直接调用

- **WHEN** 以 `commands\.` 模式扫描 `packages/desktop/src`（排除 `src/types/generated/**` 与测试内对生成物的类型断言）
- **THEN** 业务代码无 `commands.<name>(...)` 调用点，全部经 `ipc.<name>(...)`

#### Scenario: 新增命令零改装配层

- **WHEN** 在 `all_commands!` 新增一个命令并重导出产物（不改 `ipc.ts`）
- **THEN** `ipc.<新命令>` 自动存在且具备参数面 strict 与结果面 soft-report；`ipc.ts` 零 diff

#### Scenario: 类型派生保留

- **WHEN** 审查 `agent-transport.ts` 等需要从生成绑定派生参数类型的模块
- **THEN** `Parameters<typeof commands.agentStart>` 一类**类型派生**保留（类型仍出自 `bindings.ts`），手写参数类型镜像为零

### Requirement: 漂移上报聚合（进程内、去重、零新面）

漂移报告 SHALL 进入进程内聚合器：按 `(cmd, path)` 去重并累计次数、有条数上限、提供「读取 / 计数 / 清空」三个入口；DEV 构建 SHALL 对每个去重键打印一次 `console.warn`，且打印发生在**装配层**（`src/lib/ipc.ts`）而非 collector 内部——collector 保持纯数据，可被单测直接断言。本能力 MUST NOT 新增 IPC 命令、MUST NOT 写盘、MUST NOT 引入 UI 面（prod 观测面另立变更）。

聚合器的四个导出 MUST 各有至少一处**非测试**消费方（仓库规则「no test-only exports」：仅被测试文件引用的导出会被 knip 判为 dead code）：

| 导出 | 非测试消费方 | 说明 |
|------|--------------|------|
| `reportDrift` | `src/lib/ipc.ts`（参数面 / 结果面 / Channel 信封三处上报点） | 唯一写入口；运行期校验链的汇聚点 |
| `driftCount` | `src/lib/ipc.ts` DEV 汇总告警（新增去重键时输出，附当前去重条数） | 计数供告警文案与噪声上限判断 |
| `driftReports` | `src/lib/ipc.ts` DEV 汇总告警（同一次输出读取聚合快照，列出当前键） | 读入口供 DEV 排查，不做额外轮询 / 定时器 |
| `resetDrift` | `src/hooks/use-workspaces.ts`（当前根**变更**时清空） | 聚合键 `(cmd, path)` 跨 workspace 混计会让「哪个库的数据漂移」失真；根未变的重取数 MUST NOT 清空 |

本模块四个导出 MUST NOT 经 `knip.json` 的 `ignore` / `ignoreExportsUsedInFile` 豁免、MUST NOT 靠「仅供测试的再导出 / 包装」取得「已使用」判定——那是关掉判据而非满足它；前端运行期依赖 `zod` 的同类约束见「前端运行期 zod 依赖」。

#### Scenario: 同键去重累计

- **WHEN** 同一命令同一字段路径的漂移出现 N 次
- **THEN** 聚合器只有 1 条该键记录且计数为 N；不同 `(cmd, path)` 各占一条

#### Scenario: 上限与清空

- **WHEN** 漂移键数超过上限后继续产生新键，或调用清空入口
- **THEN** 记录数不超过上限（不无限增长）；清空后读取为空、计数归零

#### Scenario: 读取与计数入口在装配层被消费

- **WHEN** 审查 `src/lib/ipc.ts` 与 `src/lib/ipc-drift.ts` 的引用关系
- **THEN** `driftCount()` 与 `driftReports()` 各在 `ipc.ts` 的 DEV 汇总告警分支被调用（非测试文件），collector 自身零 `console` 调用

#### Scenario: 清空入口有产品消费方

- **WHEN** 用户切换当前 workspace 根（侧栏选择 / 移除后回落 / 新增后切换）
- **THEN** `resetDrift()` 被调用，切换后首次读取为空、计数归零；调用点位于非测试的 `src/hooks/use-workspaces.ts`，且同一根的重取数不清空聚合

#### Scenario: knip 零 dead code 且未关判据

- **WHEN** 执行 `client:check`（含 knip）
- **THEN** `ipc-drift.ts` 四个导出零 dead-code 报告；`knip.json` 相对既有配置未新增 `ignore` / `ignoreExportsUsedInFile` / `ignoreDependencies` 条目

#### Scenario: 零新 IPC 面

- **WHEN** 对比变更前后的命令登记面与 `bindings.ts` 命令清单
- **THEN** 命令增删为零；漂移上报不经过任何 IPC 往返、不落盘

### Requirement: schema 只表达接受面（禁修饰、命名降级哨兵）

生成的 schema SHALL 只表达「后端接受 / 发出的形态」，MUST NOT 承载领域不变量或二次校验：

- MUST NOT 出现 `.default()` / `.transform()` / `.catch()` / `.pipe()`（会使 `z.intersection` 的 `mergeValues` 可能 throw，见「结果面 soft-report」）；MUST NOT 出现 `.refine()` 复制保存期 / 配置校验规则（领域不变量单点仍在 Rust）。
- struct 面 SHALL 一律为 `z.strictObject`（多余键拒绝写死，MUST NOT 提供 strict / loose 双套产物）。
- 无法结构化表达的类型 SHALL 以**具名降级哨兵** `degraded("原因")`（运行时等价 `z.unknown()`，带 `meta` 可回读）标注；MUST NOT 用裸 `z.unknown()` 作哨兵——合法 `unknown` 字段（`serde_json::Value` 出线面）必须与降级面在产物文本上可区分。

#### Scenario: 产物无修饰与无 refine

- **WHEN** 对生成产物 grep `.default(` / `.transform(` / `.catch(` / `.pipe(` / `.refine(`
- **THEN** 零命中

#### Scenario: 降级面可 grep、合法 unknown 不隐身

- **WHEN** 审查产物中 `Channel<T>` 参数键与 `serde_json::Value` 出线字段
- **THEN** 前者为 `degraded("…")`（命中 `degraded(` 并可 `.meta()` 回读原因），后者仍为 `z.unknown()`；两者形态可区分

#### Scenario: strict 写死

- **WHEN** 审查 struct 面产物
- **THEN** 一律 `z.strictObject(...)`；多余键在校验中被拒（含深层嵌套对象）；产物内不存在 loose 变体

### Requirement: schema 与类型面等价断言

每份 schema MUST 与其类型面语义等价：逐命令的 `z.infer` 结果类型 MUST 与 `bindings.ts` 中该命令的 result 类型相等（Equal 断言，含 `Option` / `f64` / 整数 / literal 枚举 / Intersection / `unknown` / `char` 面），具名出线类型同此。递归类型 MUST NOT 因 `z.infer` 塌陷为 `any`。无法给出等价 schema 的类型 MUST 降级为 `degraded("原因")`（哨兵可见、导出期在 stderr 列清单），MUST NOT 静默产出不等价的 schema。

#### Scenario: 逐面 Equal 通过

- **WHEN** 执行类型面等价断言（`expectTypeOf(...).toEqualTypeOf`，由 `vp check` 的 typeCheck 承担）
- **THEN** 全部通过；`Option` / `f64` / 整数 / 枚举 / Intersection / `unknown` / `char` 各面均 Equal

#### Scenario: 递归类型不塌 any

- **WHEN** 断言递归出线类型（`TreeEntry ↔ DirNode`）的 schema 推导类型
- **THEN** 推导类型与该具名类型 Equal（非 `any`、无 `TS7022` 类自引用错误）

#### Scenario: 不等价降级可见

- **WHEN** 某类型无法给出等价 schema
- **THEN** 该类型位置为 `degraded("原因")`，导出期 stderr 列出降级清单，MUST NOT 静默产出不等价 schema

### Requirement: 前端运行期 zod 依赖：直接声明、精确 pin、单实例

生成的 schema 产物与装配层在前端**运行期**装载 zod，故 `zod` SHALL 在 `packages/desktop/package.json` 的 `dependencies`（MUST NOT 落 `devDependencies`——它随 client bundle 装载、非工具链依赖）以**精确 pin** 声明 `4.6.5`（MUST NOT 用 `^` / `~` / 范围符）。

单实例理由（结构性的，不是风格偏好）：zod v4 的 `$ZodType` 判定、`.meta()` 注册表与 `safeParse` 结果类都活在实例内部；schema 跨实例传递时 `instanceof` 与 `.meta()` 回读会**静默**失效——而 soft-report 的降级哨兵正是靠 `.meta()` 回读，失效即观测链路悄悄退化为「零报告」，与「零漂移」不可区分。今日仓库处于「zod 仅作传递依赖」状态：裸 `import 'zod'` 由 hoisting 偶然解析到 `4.6.5`（未声明的 phantom dependency），树内另有 `3.25.76`（`shadcn` → `@modelcontextprotocol/sdk` 的 CLI 路径）；hoisting 一变就可能解析到 3.x，生成物的 `z.strictObject` / `z.int()` / `.meta()` 全部不可用。

依赖落位表：

| 依赖 | 落位 | 版本形态 | 消费者 | 理由 |
|------|------|----------|--------|------|
| `zod` | `packages/desktop/package.json` → `dependencies` | 精确 `4.6.5` | `src/types/generated/bindings.zod.ts`（生成物在模块加载期构造 schema）+ `src/lib/ipc.ts`（`safeParse` 调用与类型） | 运行期装载并随 bundle 发布 ∴ 属 `dependencies`；与树内 ai-sdk 链路解析到的 4.x 同版 ∴ 单实例 |

- **单实例**：前端解析到的 zod 4.x MUST 为一份（`4.6.5`），MUST NOT 出现两个 4.x 副本。树内既有 `zod@3.25.76` MUST NOT 被 `packages/desktop/src` 任何模块 import、MUST NOT 进入 client bundle；其存在本身不构成本能力违规，但 MUST NOT 作为「放宽 4.x pin」的理由。
- **knip 可见性**：`src/lib/ipc.ts` SHALL 直接 import `zod`（值或类型），使 knip 能判定该依赖被使用；MUST NOT 把 `zod` 加入 `knip.json` 的 `ignoreDependencies`——`src/types/generated/**` 在 knip `ignore` 内，靠生成物 import 不足以支撑依赖判定。
- **版本与产物 API 面一致**：生成物使用的 v4 API（`z.strictObject` / `z.int()` / `.meta()`）MUST 由所声明版本提供；MUST NOT 声明「同时兼容 3.x」的范围来换取 hoisting 宽容（那会把两份产物的语义一致性交给解析运气）。

#### Scenario: 精确 pin 且入运行期依赖

- **WHEN** 审查 `packages/desktop/package.json`
- **THEN** `dependencies.zod` 为 `"4.6.5"`（无 `^` / `~` / 范围符），`devDependencies` 内无 `zod`

#### Scenario: 单实例解析

- **WHEN** 解析依赖图（`pnpm why zod` 或审查 `packages/desktop/pnpm-lock.yaml`）
- **THEN** desktop 的直接依赖解析到 `4.6.5` 一份、无第二个 4.x 副本；`3.25.76` 只出现在 `shadcn` → `@modelcontextprotocol/sdk` 传递路径，`packages/desktop/src` 内对 `zod` 的 import 全部解析到该 `4.6.5`

#### Scenario: 生成物 API 与声明版本一致

- **WHEN** 生成物出现 `z.strictObject(` / `z.int(` / `.meta(`
- **THEN** 声明的 `4.6.5` 支持全部命中（均为 v4 API）；`vp check` 与 `client:build` 零「API 不存在」错误

#### Scenario: knip 判定依赖被使用

- **WHEN** 执行 `client:check`（含 knip）
- **THEN** knip 不报 `zod` 为未声明 / 未使用依赖（由 `src/lib/ipc.ts` 的非生成物 import 支撑），`knip.json` 的 `ignoreDependencies` 相对既有四项零新增

#### Scenario: 双副本显性拦截

- **WHEN** lockfile 解析出两份 zod 4.x（版本分叉）
- **THEN** 依赖审查判不通过、需收回到单实例后才合入；MUST NOT 以「运行期看起来正常」放过

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/lib/ipc.ts` | 装配层：分级校验 | 命令集合由生成物键遍历推导（零手写镜像）；参数面 strict → `IpcArgsError`（零往返）；结果面 / Channel 信封 soft-report（原值透传）；`Channel` 参数不校验但保留在 payload、包裹幂等；DEV 汇总告警在此层消费 `driftCount` / `driftReports`；直接 import `zod`（knip 依赖可见性） |
| `packages/desktop/src/lib/ipc-drift.ts` | 漂移聚合 | `(cmd, path)` 去重 + 计数 + 上限；读 / 计数 / 清空三入口；零 IPC / 零写盘 / 零 UI / 零 `console`（打印在装配层） |
| `packages/desktop/src/hooks/use-workspaces.ts` | 聚合生命周期 | 当前根**变更**时调用 `resetDrift()`（`applyRoot` 写口内判变才清）；同根重取数不清、其余行为零变更 |
| `packages/desktop/package.json` / `knip.json` | 前端运行期依赖与检查边界 | `zod` 精确 pin `4.6.5` 入 `dependencies`（单实例）；MUST NOT 入 `knip.json` `ignoreDependencies`；生成目录 ignore 覆盖两份产物 |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | schema 面 | `argsSchemas` / `resultSchemas` / `channelArgs` + 具名类型 schema + `degraded()` prelude；`z.strictObject` 写死；零 `export type`、零 `.default/.transform/.catch/.pipe/.refine` |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 唯一类型源 | 类型与 typed invoke 包装来源不变；`ipc` 与 Equal 断言只从此处取类型；MUST NOT 转发 zod 类型 |
| `packages/desktop/src/lib/agent-transport.ts` 及业务调用点 | 调用面切换 | `commands.x(...)` → `ipc.x(...)`；`Parameters<typeof commands.agentStart>` 类型派生保留；`readPermissionMode` 等既有运行期校验保留 |

### Function: `packages/desktop/src/lib/ipc.ts`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `ipc` | `{ [K in keyof typeof commands]: typeof commands[K] }`（形态由生成物推导） | 分级校验后的调用面；参数面 strict、结果面 / 信封 soft-report |
| `IpcArgsError` | `class extends Error`，字段 `cmd` / `path` / `expected` / `got` | 本地参数拒绝；中文消息含命令名与字段路径 |
| `IpcDriftReport` | `{ cmd: string; path: string; expected: string; got: unknown }` | 漂移报告载荷（不 throw） |

### Function: `packages/desktop/src/lib/ipc-drift.ts`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `reportDrift` | `(report: IpcDriftReport) => void` | 记录并按 `(cmd, path)` 去重累计；超上限丢弃新键。消费方：`src/lib/ipc.ts` |
| `driftReports` | `() => ReadonlyArray<IpcDriftReport & { count: number }>` | 读取当前聚合。消费方：`src/lib/ipc.ts` DEV 汇总告警（列出当前键） |
| `driftCount` | `() => number` | 去重后条数。消费方：`src/lib/ipc.ts` DEV 汇总告警（附条数） |
| `resetDrift` | `() => void` | 清空。消费方：`src/hooks/use-workspaces.ts`（当前根变更时）——跨 workspace 混计会让漂移归属失真 |

三个读取 / 清空入口的非测试消费方是 `knip` 零 dead code 的判据（见「漂移上报聚合」）；MUST NOT 靠 `knip.json` 豁免或测试专用再导出维持「被使用」的假象。前端运行期 `zod` 依赖的落位 / 版本形态 / 单实例约束见「前端运行期 zod 依赖」的依赖落位表，此处不重复。

### Component

无。本能力 MUST NOT 引入 UI 面（零新增组件、零新增路由、零新增可见状态）。
