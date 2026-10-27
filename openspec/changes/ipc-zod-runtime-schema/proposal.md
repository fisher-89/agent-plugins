# 提案: ipc-zod-runtime-schema

> **变更**: ipc-zod-runtime-schema
> **日期**: 2026-10-26
> **状态**: draft
> **来源**: `explore.md`（requirement 类型，终态梳理：现状取证 / 上游能力实测 / 版本窗口 / 五项否决方案 / 已定决策 9 条 / 未定项 5 条）
> **前置依赖**: 无（`desktop-ipc-type-bindings` 的 tauri-specta 接入、生成物入库、`bindings:check` 守卫、前端 typed 调用面切换均已在位——本篇只在其上补「运行期校验」这一层，不改类型主权）
> **性质**: 观测漂移 + 防脏数据。静态类型零运行期成本 = 零运行期约束（`3.5` 给 `i32`、`300` 给 `u8`、多余键、`'Raw'` 给 `literal('raw')` 静态全部接受），本篇补的正是这段空白。

---

## 问题

### 现状：类型面强、运行期零约束、漂移不可观测

```
Rust DTO (specta::Type)
   ├─ main.rs  generate_handler!      ← all_commands! 单一登记面
   └─ bindings/mod.rs builder()       ← 同一登记面 + ErrorHandlingMode::Throw
            │  .semantic_types(serde_json::Value → unknown)
            │  .dangerously_cast_bigints_to_number()
            ▼
   src/types/generated/bindings.ts    ← 唯一 IPC 类型源（typed invoke 包装 + 全部出线 DTO 类型）
            ▼
      业务代码 commands.xxx()        ← 运行期零校验
      bindings:check = 导出 + git diff --exit-code   ← 只管「产物是否过期」
```

三道缺口：

| # | 缺口 | 后果 |
|---|---|---|
| G1 | 参数面（前端 → Rust）零本地校验 | 参数形状错误（`null` / `undefined` 混用、数字给字符串、多余键）要经一次 IPC 往返才由后端反序列化失败暴露；错误信息丢失命令 + 字段路径的定位 |
| G2 | 结果面（Rust → 前端）零校验 | 「后端先行、前端后跟」的窗口期内，线格式变更后的脏数据静默流进 UI；现有手写降级渲染（约 15 处）是最后一道防线，但漂移本身不可见 |
| G3 | 流式 Channel 信封零校验 | 事件 schema 演进的漂移最易发生处（`bindings.ts` 已注明旧客户端读到含新块类型的事件会整条解码失败），且逐条无观测 |

`bindings:check` 守的是「前端产物与 Rust 类型是否同步」，守不住「运行期实际收到的字节是否符合契约」——两者互补，不可互替。

### 上游缺口（`specta-zod` 0.0.4 实测，均为硬缺口）

| # | 缺口 | 影响 | 解 |
|---|---|---|---|
| U1 | `u64` / `i64` / 大整数直接 `Err(bigint_forbidden)`，且不认 TS 侧的 number 重映射 | 时间戳 / id 出线面无法导出 | 自己的 `Format` 把大整数重映射为 `define("z.number()")`（与既有 `dangerously_cast_bigints_to_number` 同口径） |
| U2 | `serde_json::Value` 出成 serde_json 自己的 enum union，并引用未定义符号 | 产物不可编译 | `Format` 把该具名类型换成 `specta_zod::Unknown` → `z.unknown()` |
| U3 | 普通 struct 无法 strict（`specta:strict_object` 只被枚举变体路径读） | 「多余键拒绝」在 struct 面失效 | fork 补丁 |
| U4 | 产物总附带 `export type X = z.infer<…>`，无可配开关 | `z.infer` 在递归类型上退化成 `any` 并静默传播 | fork 删除别名输出 |

另有一处**未在上游被覆盖的塌陷**：递归类型（本仓出线面确有 `TreeEntry ↔ DirNode`）用 `z.lazy(() => X)` 触发 `TS7022`（自引用初值无类型注解），连带 `z.infer` 全变 `any`；实测 **getter 属性**（`get children() { return … }`）无注解、Equal 全通过。这是必做补丁，不是可选优化。

### 「防脏数据」的真实缺口（必须说清，本次不入）

`unknown` 白洞字段（`ArtifactEnvelope.payload` / `AgentBlock.toolUse.input` / `RecordEnvelope.key|value` / `SessionRow.configSnapshot` / `AgentEventKind` 的 payload 与 usage 等，约 12 处）全部来自 `serde_json::Value`，其 zod 校验力为 0，而手写降级渲染的大头正好落在这些字段上。根治路径是 **payload 契约化**（Rust 侧把 `Value` 换成 tagged enum → 前端 tsc 直接收窄），属另一个 change 的量级（改动 core/workflow 的 artifacts 契约与版本兼容语义），故分层顺序为：本篇只做运行期校验，白洞收益记为已知缺口。

### explore 未决项的本文裁决

| explore 未定项 | 本文裁决 |
|---|---|
| 结果面 prod 全量上报还是采样 | 全量 soft-report + 按 `(cmd, jsonPointerPath)` 去重聚合（否则刷屏）；性能实测千级节点 < 1 ms，不采样 |
| DEV 是否把结果面升 strict | **不升**。「strict / loose 双套」与已定决策「strict 写死、不做双套」同源否决——双套就是双套漂移面；DEV 观测靠哨兵 + 聚合上报 |
| 补丁 4（struct 内 Intersection 兄弟键）是否现在做 | 不做。出线面唯一 `flatten` 在枚举路径（`AgentEvent`），struct 路径无实例；记入风险与遗留 |
| vendor 跟版策略 | 先 vendor + 向上游提 PR（补丁 1 / 3 对所有人有用）；上游合入即删 vendor 改回依赖 |
| `unknown` 白洞是否契约化 | 不入本篇（见上节），另立变更 |
| 上报落点（explore 未定） | 进程内 collector（去重 + 计数 + 上限）+ DEV 汇总告警（打印在装配层，collector 保持纯数据）；四导出各有非测试消费方（见下文范围裁决）；**零新 IPC 命令 / 零写盘 / 零新 UI**（prod 观测面另立变更） |

## 提案

在既有 builder 上多挂一个导出器，产出纯 schema 文件；前端加一层薄装配，按方向分级校验。**唯一类型源不变，命令面不变，Rust DTO 语义不变。**

1. **产物三份职责切分**（三方不可互换）
   - `src/types/generated/bindings.ts`：类型 + typed invoke 包装，**保持唯一类型源**（既稳定 exporter、递归精确、JSDoc 保留）。
   - `src/types/generated/bindings.zod.ts`：纯 zod schema（具名类型 schema + 逐命令 `argsSchemas` / `resultSchemas` + `channelArgs` 元数据 + `degraded()` 哨兵），**零 `export type`**、零 JSDoc 义务。
   - `src/lib/ipc.ts` 装配层 + `src/lib/ipc-drift.ts` 聚合层：手写，从生成物推导，零命令 / 参数类型镜像。

   | 替代方案 | 否决理由 |
   |---|---|
   | zod 产物替代 `bindings.ts` | 结构性不可能：`specta_zod::Zod::export` 只吃 `&Types`，永远看不到 `BuilderConfiguration`——typed invoke 包装 / `Channel<…>` 参数 / 错误面变换全部活在 `cfg.commands` 里，只有 tauri-specta 能渲染 |
   | `bindings.ts` 转发 zod 类型 | ① 丢全部 JSDoc ② 类型主权交给 partial 上游（zod 侧 = Partial、TS 侧 = Stable）③ 相位分裂时前端会拿到 `X_Deserialize \| X_Serialize` 联合而非线格式类型 |
   | zod 产物保留 `export type` | `z.infer` 在递归类型上退化成 `any` 并静默传播，而这些类型没有一个是 specta 给不了的 |

2. **按方向分级校验（核心策略）**

   | 面 | 策略 | 行为 |
   |---|---|---|
   | 参数面（前端 → Rust） | **strict** | `safeParse` 失败 → 本地 throw `IpcArgsError`（含命令名 + jsonPointer 路径 + expected/got），**零 IPC 往返**。schema 就是从「后端接受面」生成的，合法输入不可能被拒 |
   | 结果面（Rust → 前端） | **soft-report** | 失败 → 上报 `{cmd, path, expected, got}` + **原值返回**；现有降级渲染照旧，UI 不崩 |
   | 流式面（Channel 信封） | **逐条 soft-report** | 逐条上报，原消息原样交付 handler（不丢流、不断流） |

   错误面必须三分，否则分级无从落地：`invoke reject: string`（后端业务失败，语义不变）/ `IpcArgsError`（前端本地拒绝）/ `IpcDriftReport`（漂移观测，不是 throw）。

3. **vendor `specta-zod` 0.0.4 + 5 处补丁**：落位 `src-tauri/crates/tools/specta-zod-patched`（workspace 成员，改动点带 grep 标记，crate README 记录基线版本与补丁索引）。补丁 = U3 struct strict（功能）/ U4 删 `export type`（功能）/ 递归改 getter 属性（**必做**）/ struct 内 Intersection 兄弟键（延后）/ 递归类型注解兜底（视需要）。

4. **自家 wrapper（~150–200 行，不含测试）**：
   - `ZodLanguage` newtype（外部类型不能实现外部 trait，newtype 是本地的）→ `impl tauri_specta::LanguageExt`；
   - `ZodFormat`：`impl specta::Format`，= `specta_serde::PhasesFormat`（公开，可复用；**上游 `SpectaFormat` 是 private，不可复用**）+ 大整数重映射 + `serde_json::Value → Unknown`；
   - 遍历 `cfg.commands` 逐参数 / 逐结果渲染，经 `Zod::with_raw` 把 `argsSchemas` / `resultSchemas` / `channelArgs` 表注入产物；
   - 参数键名沿用 tauri-specta 的 lowerCamelCase（与 invoke payload 逐字一致）；
   - 结果面取 `Result` 的 Ok 臂（与现状 `Promise<T>` 同口径）。

5. **单 builder 两次 export**：`export_bindings()` 保持唯一入口，`builder().export(Typescript, …).export(ZodLanguage, …)`（`Builder::export` 是 `&self`）。

6. **生成期硬约束与自检**：
   - MUST NOT 产物出现 `.default()` / `.transform()` / `.catch()` / `.pipe()`——`z.intersection` 的 `mergeValues` 在「同键两侧成功但值不同」时是 **throw**（不是 issue），soft-report 的 `safeParse` 抛异常 = 上报链路自己先炸；
   - strict 写死（`z.strictObject`），不做 strict/loose 双套；
   - 降级哨兵命名化：`degraded("原因")`（运行时等价 `z.unknown()`，但可 grep、可 `.meta()` 回读、可计数），MUST NOT 用裸 `z.unknown()` 作哨兵（与合法 unknown 同形，降级会永久隐身）；
   - 导出后对「只 import zod 产物的桩文件」跑一次 `tsc --noEmit`：一条自检同时看住 struct strict 化、getter 化、删别名三件事。

7. **版本 lockstep 上行一档**：`specta =2.0.0-rc.26` / `specta-typescript =0.0.13` / `tauri-specta =2.0.0-rc.26` / `specta-serde =0.0.13` + vendored `specta-zod 0.0.4`（0.0.4 pin 的正是 rc.26 / 0.0.13，是唯一全绿窗口；0.0.3 无 getter 化能力、0.0.5 需 rc.27 而 tauri-specta 最高 rc.26）。

8. **前端直接依赖 `zod`（`dependencies` 内精确 pin `4.6.5`，单实例）**：生成的 schema 与装配层在前端**运行期**装载 zod，故 SHALL 声明为直接依赖（`dependencies`，MUST NOT 落 `devDependencies`——随 client bundle 发布）并精确 pin `4.6.5`（MUST NOT 用 `^` / `~` / 范围符）。四条依据与边界：

   - **今日「能 import 到」是巧合**：`zod` 未在 `package.json` 声明，裸 `import 'zod'` 由 hoisting 偶然解析到 `4.6.5`（phantom dependency）；树内另有 `zod@3.25.76`（`shadcn@4.21.3` → `@modelcontextprotocol/sdk` 的 CLI 路径）。hoisting 一变就可能解析到 3.x，而生成物用的是 v4 API（`z.strictObject` / `z.int()` / `.meta()`）——3.x 下整份产物不可用。
   - **单实例是「可观测性」的前提，不是洁癖**：zod v4 的 `$ZodType` 判定、`.meta()` 注册表与 `safeParse` 结果类都在实例内部；schema 跨实例传递时哨兵 `.meta()` 回读会**静默**失效，漂移观测就此退化成「零报告」，与「零漂移」不可区分。树内既有 `3.25.76` 不违规（CLI 路径、不进 bundle），但 MUST NOT 被 `src` 任何模块 import，也 MUST NOT 作为放宽 4.x pin 的理由。
   - **knip 可见性**：`src/lib/ipc.ts` SHALL 直接 import `zod`（生成目录在 knip `ignore` 内，靠生成物 import 不足以支撑依赖判定）；MUST NOT 把 `zod` 加进 `knip.json` 的 `ignoreDependencies`（现有四项之外零新增）。
   - **约束归属**：本条由 `desktop-ipc-runtime-guard` 的「前端运行期 zod 依赖」requirement 正式承载（依赖落位表 + 5 条 scenario）；`desktop-ipc-type-bindings` 只约束 Rust 侧 specta 系与 vendored exporter 的落位并显式指向本条——两处约束即两处漂移面。

### 已否决（存档）

| 方案 | 否决理由 |
|---|---|
| 另一份 JSON Schema 产物 + 前端 `z.fromJSONSchema` | 用户决定前端不依赖 JSON Schema；且 JSON Schema 无「类型交汇」原语，`allOf` + 各段 `additionalProperties:false` 互判对方字段多余（AJV 实测合法数据被拒），要正确只能分配律展开，而展开破坏类型等价断言 |
| 自写 zod 渲染器（400–600 行） | 上游已覆盖 10 个 `DataType` 变体 + serde 相位 + 保留字 + 整数范围 + intersection 兄弟键补丁，只需 ~150–200 行 wrapper |
| 从 `bindings.ts` 反推 zod（ts-to-zod 风格） | 产物推产物、双重失真，且要写 TS AST 脚本 |
| 分配律展开 `Intersection` | 报错面与性能无实质优势（差 3×，相对 IPC 的 ms 是噪声），却破坏类型等价 |
| `z.json()` 作 `unknown` 过渡档 | 递归 union 慢 100–1000×（754 B / 7.7 KB / 40 KB 实测 605 µs / 1673 µs / 1997 µs vs `z.unknown()` 5 / 10 / 2 µs） |
| 「修 bindings.ts 的 f64 谎」 | 伪问题：`specta-typescript` 的 `f64` 本就出 `number \| null`，与 zod 侧一致 |
| Event 信封改显式 `kind`（消掉 flatten） | 线格式变更（旧客户端读新块整条解码失败），属另一个 change 的量级 |

### 范围裁决（explore 未定项的落地口径）

- **`degraded()` 落生成物 prelude**：explore 写「手写 3 行，非生成物」，本文改为**由 exporter 注入生成物 prelude**——避免新增「生成物 → 手写文件」import 边，也避免该 helper 因只被 `knip` 忽略目录引用而误报 dead code。语义（运行时等价 `z.unknown()`、带 `meta`）不变。
- **`IpcArgsError` 消息面**：中文文案 + 命令名 + jsonPointer 路径；hook 层既有「`String(err)` 进 error 态」的呈现零改动（`IpcArgsError extends Error` 天然走既有路径）。
- **漂移上报零新面**：不加 IPC 命令、不写盘、不加 UI；collector 提供读 / 计数 / 清空三入口（连同 `reportDrift` 共四导出），且四者各有**非测试**消费方——`driftReports` / `driftCount` 由 `ipc.ts` 的 DEV 汇总告警消费（打印在装配层，collector 保持纯数据），`resetDrift` 由 `use-workspaces` 在当前根变更时消费。这是 knip 零 dead code 的判据（仓库规则「no test-only exports」），MUST NOT 用 `knip.json` 豁免替代。
- **`MIN_OUTPUT_TOKENS` 出线（explore §8.1 的配套项）不入本篇**：与本篇收益正交（需把 `agent_runtime::MIN_OUTPUT_TOKENS` 经 `builder.constant` 出线并改 `provider-panel` 文案与测试），另立变更更清晰。本篇只落实其前提纪律：**schema MUST NOT 承载领域不变量**（不写 `.refine()`，保存期校验单点仍在 Rust）。
- **一个受控类型断言**：`ipc` 面由 `commands` 键遍历构造，必然需要一处类型收敛（`as const` 之外的断言）。lint（`typescript/no-unsafe-type-assertion`）可能报错，处置方式（单点带理由的 disable / 结构化构造函数签名）由 dev-design 定夺，MUST NOT 退化为手写命令清单。
- **`TypeScript` 侧零语义变更**：rc.26 升级后 `bindings.ts` 以重导出为准；若产物有格式级差异，由守卫捕获后重导出收口。

## 能力

### 新增的能力

- **desktop-ipc-runtime-guard** — IPC 边界的**运行期**契约校验面：参数面 strict 本地拒绝（`IpcArgsError`）、结果面与 Channel 信封 soft-report、漂移聚合去重（读 / 计数 / 清空三入口各有非测试消费方，knip 判据）、装配层零手写镜像、schema 只表达接受面且与类型面等价、前端运行期 `zod` 依赖的精确 pin 与单实例。单独立能力而不塞进 `desktop-ipc-type-bindings` 的理由：类型绑定能力管的是「类型产物与其守卫」（编译期 + 产物过期），本能力管的是「字节在运行期是否符合契约」（分级策略 + 上报口径 + 运行期依赖形态）——两者的验收对象、失败模式、退役条件完全不同，合并会让「产物过期」与「数据漂移」共用一条 requirement。

### 修改的能力

- **desktop-ipc-type-bindings** — **MODIFIED**「生成物入库与一致性守卫」（「生成物」概念由单文件升为生成目录：两份产物同目录同次导出、`bindings:check` 覆盖面扩大、命令不变）；**MODIFIED**「依赖落位与 knip 边界」（四件套精确 pin 上移到 rc.26 / 0.0.13 lockstep + vendored `specta-zod 0.0.4` 落位 `crates/tools`；并显式把前端 `zod` 依赖的约束指向 `desktop-ipc-runtime-guard`，避免同一规则两处漂移）；**ADDED**「第二产物职责切分（纯 schema，零 `export type`）」「生成期 tsc 自检与降级清单」「vendored exporter 落位与跟版」。「tauri-specta 接入与命令绑定生成」「前端一次性切换 typed bindings」「dto.ts shim 过渡与判据驱动退役」三条 requirement **零改动**（命令面、类型主权、shim 退役判据均不动）。

### 零改动的能力及理由

- **desktop-crate-layout**：新成员 crate 落 `crates/tools/`（`export-bindings` 同层），不引入 core / infra 任何新边，依赖方向规则逐条不变；vendor crate 的落位与跟版纪律写进 `desktop-ipc-type-bindings`（属产物生成面，不属分层面）。
- **desktop-workspace-config / desktop-app-shell / desktop-page-routing / desktop-standard-components**：零配置 schema 扩展、零命令注册变更、零路由 / 组件改动（本变更无 UI 面）。
- **desktop-agent-execution / desktop-change-orchestration / desktop-explores（查询面）等**：Rust DTO 零字段变更、零 serde 属性变更、事件词汇零扩展——出线字节形态逐字不变，故这些能力零改动。
- **desktop-test-execution / desktop-corpus-regression / desktop-release-pipeline**：检查链脚本与发版纪律不变（`bindings:check` 命令名与形态不变，仅覆盖面随目录扩大）。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/Cargo.toml` | workspace members 增 `crates/tools/specta-zod-patched`；`specta` `=2.0.0-rc.26`、`specta-typescript` `=0.0.13`、`tauri-specta` `=2.0.0-rc.26`、新增 `specta-serde` `=0.0.13`；`specta-zod` 以 path 指向 vendored crate；根包依赖补 `specta-zod` / `specta-serde`（Tauri 系边界不破，仅壳层） |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/**`（新 crate） | vendored `specta-zod` 0.0.4：① 普通 struct 读容器 `specta:strict_object` → `z.strictObject`；② 删除 `export type` / 相位伴生别名输出；③ 具名 / 递归引用改 getter 属性（去 `z.lazy`）；④（延后）struct 路径 Intersection 兄弟键补丁；⑤（视需要）递归类型注解兜底。改动点带 grep 标记 + crate README（基线版本 / 补丁索引 / 跟版记录） |
| `packages/desktop/src-tauri/src/bindings/mod.rs` | 保持单 builder；`export_bindings()` 增第二次 `.export(ZodLanguage::default(), dir.join("bindings.zod.ts"))`；`semantic_types` / `bigint` 既有 TS 侧配置零改动 |
| `packages/desktop/src-tauri/src/bindings/zod.rs`（新，~150–200 行） | `ZodLanguage` newtype + `impl LanguageExt`；`ZodFormat`（`PhasesFormat` + 大整数 → `z.number()` + `serde_json::Value` → `z.unknown()`）；遍历 `cfg.commands` 产出 `argsSchemas` / `resultSchemas` / `channelArgs`（`Channel<T>` 参数以 `degraded(...)` 占位、键保留）；产物头（import zod、`degraded()` prelude、无 `export type`） |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 具名类型 schema + 三张命令表 + `degraded()` prelude；`z.strictObject`；零 `export type` |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 仅因 rc.26 重导出可能有的格式级差异；类型面语义零变更（唯一类型源不变） |
| `packages/desktop/src/lib/ipc.ts`（新） | 装配层：`ipc.<command>`（键遍历 `commands` + 生成表推导，零手写镜像）；参数面 strict → `IpcArgsError`；结果面 + Channel 信封 soft-report；`Channel` 参数跳过校验但**原样留在 payload**、包裹幂等（同一 channel 重复传入不重复包裹）；DEV 汇总告警在此层消费 `driftCount` / `driftReports`（`ipc-drift.ts` 的非测试消费方）；直接 import `zod`（knip 依赖可见性） |
| `packages/desktop/src/lib/ipc-drift.ts`（新） | 漂移聚合：`(cmd, path)` 去重 + 计数 + 条数上限；读 / 计数 / 清空三入口；零 IPC / 零写盘 / 零 UI / 零 `console`（打印在装配层，使 collector 保持纯数据） |
| `packages/desktop/src/hooks/use-workspaces.ts` | 当前根**变更**时 `resetDrift()`（`applyRoot` 写口内判变才清，同根重取数不清）——`resetDrift` 的非测试消费方，兼作「跨 workspace 混计会让漂移归属失真」的收口 |
| `packages/desktop/src/lib/agent-transport.ts` 及业务 `commands.*` 全部调用点 | `commands.x(...)` → `ipc.x(...)`（机械替换）；`Parameters<typeof commands.agentStart>` 等**类型派生**保留（类型仍出自 `bindings.ts`） |
| `packages/desktop/package.json` | 直接依赖 `zod`（`dependencies` 内精确 pin `4.6.5`，与树内 ai-sdk 解析版本同版，单实例）；`version` 0.5.31 → 0.5.33 |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 参数面 strict：错类型 / `null` 给必填 / 多余键 → `IpcArgsError`（含命令名 + 路径）且 `invoke` 零调用；合法参数直通 | `src/lib/ipc.test.ts` | 新增（mock `@tauri-apps/api/core`，沿既有 `invokeMock` 先例） |
| 参数面 Channel：`onEvent` 不参与 args 校验但留在 payload（`invoke` 第二参含该键）；合法调用零漂移记录 | 同上 | 新增 |
| 结果面 soft-report：脏结果 → 记录 1 条漂移且返回值与输入 `deep equal`（原值透传）；干净结果零记录；`safeParse` 不抛（无 `.transform` / `.default` 组合） | 同上 | 新增 |
| Channel 信封：脏消息 → 单条漂移 + handler 仍被调用（不丢流）；同一 channel 重复传入只包裹一次、只上报一次（幂等） | 同上 | 新增 |
| 错误面三分：后端 reject 串照旧向上抛（不被吞成漂移）；args 错 → `IpcArgsError`（`instanceof Error`，走既有 `String(err)` 路径） | 同上 | 新增 |
| 漂移聚合：同 `(cmd, path)` 重复 N 次 → 1 条记录 + `count == N`；跨键分隔；上限封顶；清空生效 | `src/lib/ipc-drift.test.ts` | 新增 |
| 漂移聚合入口的非测试消费方：`ipc.ts` DEV 汇总告警消费 `driftReports` / `driftCount`；当前根**变更**触发 `resetDrift`（同根重取数不清空） | `src/lib/ipc.test.ts` / `src/hooks/use-workspaces.test.ts` | 新增（`use-workspaces.test.ts` 以 `vi.mock` 断言清空时机）；knip 零 dead code 由 `client:check` 承担 |
| 类型面等价：逐命令 `z.infer<resultSchemas[k]>` 与 `bindings.ts` 对应类型 Equal；递归类型（`TreeEntry ↔ DirNode`）不塌 `any` | `src/lib/ipc-contract.test.ts` | 新增（`expectTypeOf(...).toEqualTypeOf`；`vp check` 的 typeCheck 承担断言执行） |
| 产物自检：只 import `bindings.zod.ts` 的桩文件 `tsc --noEmit` 通过（strict 化 / getter 化 / 无别名） | 导出链内置（`bindings:export` 之后或独立脚本，形态 design 定） | 新增 |
| 生成幂等：连续两次导出零 diff（两份产物） | `bindings:check` 主体（无新增测试） | 复用既有守卫 |
| Rust 侧 wrapper 单测：args 表键名为 lowerCamelCase、`Channel<T>` 参数出 `degraded(...)`、有 `export type` 零出现、`Value` 出 `z.unknown()`、大整数出 `z.number()` | `packages/desktop/src-tauri/src/bindings/zod_test.rs` | 新增 |
| 既有 39 个 mock `invoke` 的 hook / 组件测试 | 各调用点测试文件 | 仅随 `commands.` → `ipc.` 机械改引入；mock 按命令名分发，命令名不变 |

### 不要修改

- **命令面**：`all_commands!` 登记面零增删；零新增 / 零删除 IPC 命令；`ErrorHandlingMode::Throw` 与既有错误面语义不变。
- **Rust DTO 与线格式**：零 `#[serde]` 属性变更、零字段增删、零 `specta::Type` 语义变更；`core/agent/src/event.rs` 刻意避开 `skip_serializing_if` 的相位纪律不变（当前 0 相位分裂，本变更 MUST NOT 引入）。
- **类型主权**：`bindings.ts` 内容形态（JSDoc 保留）、MUST NOT 转发 zod 类型、MUST NOT 用 `z.infer` 作业务类型。
- **领域校验单点**：保存期 / 配置校验规则留在 Rust（schema MUST NOT 写 `.refine()` 复制领域不变量）；`config.schema.json` 与 `KNOWN_KEYS` / `TestFramework` 镜像零改动。
- **`openspec/config.json`**：零 schema 扩展（覆盖阈值、测试框架登记不动）。
- **`crates/core/**` 与 `crates/infra/**` 全部业务 crate**：零改动（本篇只新增 `crates/tools/specta-zod-patched` 与壳层 `bindings` 模块）。
- **`knip.json` 既有豁免**：`ignore` 仍只含生成目录与 `components/ui`、`ignoreDependencies` 保持现有四项；MUST NOT 为本变更新增任何豁免条目（dead-code 判据 MUST NOT 通过关掉判据来满足）。
- **zod 版本形态**：MUST NOT 把 `zod` 放宽为范围依赖、MUST NOT 落 `devDependencies`、MUST NOT 让 `src` 任何模块 import 树内 `3.25.76`。
- **`openspec/changes/ipc-zod-runtime-schema/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 参数面非法输入本地拒绝：抛 `IpcArgsError`（含命令名 + jsonPointer 路径 + expected/got），`invoke` 零调用；合法输入零误拒 | `src/lib/ipc.test.ts` 三臂（错类型 / `null` / 多余键 + 直通） |
| AC-2 | 结果面漂移 soft-report：脏结果记录 1 条并原值返回，UI 侧既有降级渲染用例全绿；干净结果零记录 | `ipc.test.ts` + 既有渲染用例 |
| AC-3 | Channel 信封逐条上报、不丢流、包裹幂等（同一 channel 重复传入只报一次） | `ipc.test.ts` 信封两臂 |
| AC-4 | 错误面三分：后端 reject 串语义与既有 error 态文案不变；`IpcArgsError` 走既有 `String(err)` 呈现；漂移不 throw | `ipc.test.ts` + 既有 hook 测试全绿 |
| AC-5 | 装配层零手写镜像：`ipc` 面由 `commands` 键遍历 + 生成表推导（新增命令零改 `ipc.ts`）；业务代码零 `commands.*` 直接调用（生成物内部除外） | grep 扫描 + 键遍历结构审查 |
| AC-6 | 两份产物语义等价：逐命令 `z.infer` 与 `bindings.ts` 类型 Equal（含 `Option` / `f64` / 整数 / literal 枚举 / Intersection / `unknown` / `char`），递归类型不塌 `any` | `src/lib/ipc-contract.test.ts`（`vp check` typeCheck） |
| AC-7 | 生成期硬约束：产物 grep 零 `.default(` / `.transform(` / `.catch(` / `.pipe(` / `.refine(`，零 `export type`；`degraded(` 命中 Channel 参数面，合法 `unknown` 仍为 `z.unknown()` | 产物 grep + 生成期自检脚本 |
| AC-8 | 生成期自检：只 import zod 产物的桩文件 `tsc --noEmit` 通过（同时看住 strict 化 / getter 化 / 删别名） | `bindings:export` 后自检步骤 |
| AC-9 | 守卫覆盖两份产物且幂等：连导两次零 diff；改 Rust DTO 后漏重导 → `git diff --exit-code -- src/types/generated` 非空报错 | `pnpm run bindings:check` |
| AC-10 | 版本 lockstep 与落位：`specta` / `tauri-specta` = rc.26、`specta-typescript` / `specta-serde` = 0.0.13、`specta-zod` = 0.0.4（vendored path）；`tauri-specta` / `specta-typescript` 仍只在壳层；core / infra 仍只有 `specta` | 各 `Cargo.toml` 审查 + `cargo tree` |
| AC-11 | 前端 zod 依赖落位合规：`dependencies` 内精确 pin `4.6.5`（无 `^`/`~`/范围符、不在 `devDependencies`）、单实例（解析出唯一一份 4.x；`3.25.76` 只经 `shadcn` → `@modelcontextprotocol/sdk` CLI 路径、`src` 零 import）、`src/lib/ipc.ts` 直接 import 使 knip 判定依赖被使用、`knip.json` `ignoreDependencies` 零新增、生成目录仍在 ignore 内 | `package.json` 审查 + `pnpm why zod` / lockfile 审查 + `pnpm run client:check` |
| AC-12 | 门禁：`pnpm -C packages/desktop run check` 全绿、覆盖率 ≥ lines 90 / branches 85 / functions 90、`version` 0.5.31 → 0.5.33、`openspec/config.json` 零改动 | desktop 门禁 + diff 审查 |
| AC-13 | 漂移聚合四导出各有**非测试**消费方：`reportDrift` / `driftReports` / `driftCount` ← `src/lib/ipc.ts`（DEV 汇总告警），`resetDrift` ← `src/hooks/use-workspaces.ts`（当前根变更时）；`knip` 零 dead code 且未靠 `ignore` / `ignoreExportsUsedInFile` / `ignoreDependencies` 豁免 | import 关系 grep + `use-workspaces.test.ts`（清空时机）+ `client:check`（knip） |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **vendor 跟版成本**：`specta-zod` 是 0.0.x partial，每个 specta rc 都改 API | 上游升版时补丁失配，产物导出中断 | 高 | 补丁全部落在固定函数（`primitives.rs` 的 struct / export / reference 三处）+ 每处 grep 标记；crate README 记录基线与补丁索引；补丁 1 / 3 提上游 PR，合入即删 vendor |
| R-2 | **递归 getter 化兜底不成立**：更深自引用仍触发 `TS7022` | 需要 Rust 侧再生成一遍形状类型 = 重写 TS 渲染器 | 低 | 实测 `TreeEntry ↔ DirNode` 形态通过；兜底方案为「该类型降级为 `degraded("recursive")`」（牺牲该类型校验力、哨兵可见），不做重写 |
| R-3 | **strict 化噪音**：「后端先行、前端后跟」窗口期内结果面报 `unrecognized_keys` | 上报刷屏 | 中 | soft-report 不阻断（只是噪音）+ `(cmd, path)` 去重聚合 + DEV 每键一次 warn；不做 strict/loose 双套（双套即双漂移面） |
| R-4 | **`z.intersection` `mergeValues` throw**：两侧同键各带 `.default()` / `.transform()` 时抛异常而非产 issue | soft-report 的 `safeParse` 自己先炸 = 观测链路失效 | 低（上游天然满足） | 生成期硬约束（禁四类修饰）+ 产物 grep 门禁 + 自检桩 `tsc`；不再加 `.default()` 走线 |
| R-5 | **结果面全量校验的性能**：超大响应（代码量整树、db 大页） | 交互卡顿 | 低 | 千级节点 < 1 ms（实测 0.16–0.63 ms）；不采样、不设体积上限；若后续出现 > 10 ms 面，按 `(cmd)` 白名单降级为 `degraded(...)`（哨兵可见） |
| R-6 | **装配层类型收敛与 lint 冲突**：`ipc` 面必须有一处受控断言（`no-unsafe-type-assertion`） | 检查红 / 退化为手写镜像 | 中 | 单点带理由断言或结构化构造函数签名（dev-design 定）；`no-unsafe-type-assertion` 若未触发则不得保留 disable（`reportUnusedDisableDirectives: error`） |
| R-7 | **zod 双实例 / 版本错配**：今日 zod 是未声明的传递依赖——裸 `import 'zod'` 靠 hoisting 偶然解析到 `4.6.5`，树内另有 `3.25.76`（`shadcn` → `@modelcontextprotocol/sdk` CLI 路径）；hoisting 变化或与 ai-sdk 链路的 4.x 分叉即可能解析到别的副本 | 生成物的 v4 API 直接不可用；即使同为 4.x，跨实例 `instanceof` / `.meta()` 回读静默失效 → 漂移观测退化为「零报告」，与「零漂移」不可区分 | 中（当前靠 hoisting 侥幸成立） | `dependencies` 内精确 pin `4.6.5`（无范围符）+ `src/lib/ipc.ts` 直接 import（knip 可见）+ `pnpm why zod` / lockfile 审查单实例；`3.25.76` 仅 CLI 路径、`src` 零 import；MUST NOT 用 `ignoreDependencies` 消报告 |
| R-8 | **`unknown` 白洞（约 12 处）校验力为 0** | 「防脏数据」在 payload / usage / key-value 面不成立，仍靠手写降级渲染 | 高（已知） | 明确记为已知缺口（问题节）；payload 契约化另立变更；本篇用 `degraded()` 把「表达不了的降级」与「合法 unknown」在产物里区分开，避免缺口隐身 |
| R-9 | **Channel 包裹幂等**：`use-change-flow-run` 复用缓存 channel、`agent-transport` 双出口重复传入 | 重复包裹 → 同一消息多次上报、报告噪声翻倍 | 中 | 包裹幂等标记（非枚举 symbol / 已包裹短路）+ 单测覆盖「重复传入只报一次」；包裹 MUST NOT 改变 handler 的调用序与调用次数 |
| R-10 | **产物体积与启动开销**：`bindings.zod.ts` 与 `bindings.ts` 同量级，schema 对象在模块加载期构造 | bundle 体积 + 启动解析 | 低 | 生成物已被 `bindings.ts` 引入同一模块图；zod 运行时本就在树内（ai-sdk）；体积由构建产物 diff 观察，必要时把 schema 构造延后到首个调用（懒初始化，design 定） |
| R-11 | **相位分裂未来引入**：若后续有人给出线类型加 `skip_serializing_if`，`PhasesFormat` 会分裂出 `X_Serialize` / `X_Deserialize` | 参数面 / 结果面取哪一相位变得不明 | 低 | 记入能力 spec：参数面取 Deserialize 相位、结果面取 Serialize 相位；当前分裂数为 0，新增分裂需在本能力内重新对齐并向 `desktop-ipc-type-bindings` 报备 |
| R-12 | **rc.26 升级引入 TS 产物差异** | `bindings.ts` diff 混入「升级噪声」 | 中 | 升级与功能改动分两次提交；产物差异全部由 `bindings:check` 显式呈现，逐行审阅后再入库 |

## 遗留观察项（不在本篇范围）

1. **payload 契约化**：`ArtifactEnvelope.payload` 等约 12 处 `serde_json::Value` 换成 tagged enum，前端 tsc 直接收窄、手写收窄整体退役——是本篇收益完整化的前置，另立变更。
2. **`MIN_OUTPUT_TOKENS` 类常量出线**：经 `builder.constant` 收成单点（`provider-panel.tsx` 的手抄字面量），另立变更。
3. **prod 漂移观测面**：当前只有进程内 collector + 装配层 DEV 汇总告警（console）；若需持久化 / 用户可见，需新增 IPC 命令或复用引擎日志，另立变更。
4. **补丁 4（struct 路径 Intersection 兄弟键）**：出线面唯一 `flatten` 在枚举路径，struct 路径无实例；出现实例时补。
5. **两处同「谎」面**：`u64` > 2^53 静默失真、`NaN` / `Infinity` 出 `number | null` 不排除 NaN——两侧一致，由 JSON 表达能力决定，不做额外处理。
