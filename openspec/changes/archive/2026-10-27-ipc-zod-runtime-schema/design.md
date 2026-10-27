# 设计: ipc-zod-runtime-schema

> **变更**: ipc-zod-runtime-schema
> **日期**: 2026-10-26
> **基于**: `proposal.md`、`explore.md` 与 `specs/{desktop-ipc-runtime-guard,desktop-ipc-type-bindings}/spec.md`

---

## 提案与规格同步状态

proposal 的八面与两份 delta spec 已定稿，本设计不重复其论证，只做四件事：把「vendor 补丁怎么打」落成可实现的逐点改动（含三处补丁相互依赖的因果链）、把 wrapper 的渲染管线与降级口径定死（哪一处取哪个相位、哪些类型被隐藏）、把前端装配层的零断言形态与 Channel 包裹机制落成可验证的结构、按 AC-1..AC-13 逐条对齐。

**起点核实**（工作树实况；路径前缀 `packages/desktop/`，锚点为改动前行号）：

- **导出入口现状**：`src-tauri/src/bindings/mod.rs`——`builder()` `:14-31`（`all_commands!(collect_commands)` + `ErrorHandlingMode::Throw` + `semantic_types(serde_json::Value → "unknown")` + `dangerously_cast_bigints_to_number()`）；`export_bindings()` `:36-48`（`CARGO_MANIFEST_DIR` 父目录 `create_dir_all` 单点 + 单次 `.export(Typescript::default(), bindings.ts)`）。`Builder::export` 签名是 `fn export<L: LanguageExt>(&self, language: L, path: impl AsRef<Path>)`（`tauri-specta rc.26` `builder.rs:473`）——`&self`，故二次导出无需重建 builder。实施后 `builder()` 仅多一行 `.disable_serde_phases()`（命令注册面零改动，见校准 5 / D1）。
- **导出驱动现状**：`src-tauri/crates/tools/export-bindings/src/main.rs` 只调 `dev_team::bindings::export_bindings()`（失败 `eprintln` + `exit(1)`）；该 crate 是唯一驱动方，`bindings:export` = `cargo run --quiet -p export-bindings`。
- **版本面现状**：`src-tauri/Cargo.toml`——`specta = "=2.0.0-rc.25"` `:94`、`specta-typescript = "=0.0.12"` `:95`、`tauri-specta = "=2.0.0-rc.25"` `:96`；`members` `:51-65`（`crates/tools/export-bindings` 为 `tools` 组唯一成员）；根包 `[dependencies]` `:19-41` 已含 `specta` / `specta-typescript` / `tauri-specta`。`tauri 2.12.5` 自身依赖 `specta = "^2.0.0-rc.16"`（`tauri-2.12.5/Cargo.toml:217`）——`rc.26` 落在该 caret 的预发布窗口内，故全树只会有一份 specta RC（AC-10 的 `cargo tree` 判据可满足）。
- **上游 exporter 现状（registry 内 `specta-zod 0.0.4`，pin `specta =rc.26` / `specta-typescript =0.0.13`）**：`zod.rs:52` `pub struct Zod`（`header` / `framework_prelude` / `with_raw` / `export` / `export_to` 均 pub）；`zod.rs:397` `format_types` 只调 `format.map_types`（**不调 `map_type`**），故 `map_type` 只在我们自己渲染命令参数 / 结果时显式调用。`primitives.rs`：`export_single_internal` 别名渲染 `:99-190`、schema 注解 `:228`（`export const XSchema: z.ZodType<X>`）、泛型函数注解 `:343`、`datatype` `:904`、`datatype_with_inline_attr` `:882`、`struct_dt` `:1520`（无 strict 参数，走 `Fields::Named` 时恒 `z.object`）、`enum_variant_dt` `:1821`（strict 由 `:1762` 读 `specta:strict_object` 决定）、`reference_dt` `:1956`（`:2116` 恒出 `z.lazy(() => X)`）、`primitive_dt` `:1066`（`char` 出 `z.string().refine(...)`；`usize/isize/i64/u64/i128/u128/f128` 直接 `Err(bigint_forbidden)`）、`map_key_datatype` `:1223`。`references.rs` `track_nr` 负责引用收集。上游 `specta:strict_object` 标记**全仓无人写入**（`specta` / `specta-serde` / `tauri-specta` 三方 grep 零命中）——即变体 strict 分支实际是死路径。
- **前端调用面现状**：非测试文件 import `commands` 的共 **21 个**（`src/hooks/` 3、`src/lib/` 1、`src/views/` 17），`commands.<name>(` 调用点 24 处 + 多行链式 8 处；`commands` 的**类型派生**仅 `src/lib/agent-transport.ts:19-20`（`Parameters<typeof commands.agentStart>`）。5 处 `Channel<...>` 参数（`bindings.ts:106/115/144/148/221`）**全部是首参 `onEvent`**。
- **装配层前置条件现状**：`src/hooks/use-workspaces.ts:48-51` `applyRoot` 是当前根唯一写口（`rootRef` + `setRoot`），天然是 `resetDrift()` 的唯一挂点（`select` / `add` / 取数回落全经它）。`vite.config.ts:44-51` 已开 `typescript/no-unsafe-type-assertion: error` + `reportUnusedDisableDirectives: error`（R-6 的两条硬约束）。
- **依赖面现状**：`package.json` 未声明 `zod`；`pnpm why zod` 实测 `4.6.5`（ai-sdk 链）与 `3.25.76`（`shadcn → @modelcontextprotocol/sdk`）并存——`4.6.5` 今日靠 hoisting 偶然可 import（phantom dependency）。`knip.json`：`ignore` 含 `src/types/generated/**`，`ignoreDependencies` 四项。
- **knip 判据实测**（本次实测，非推测）：向 `src/lib/parse.ts` 追加一个仅在文件内使用的导出后运行 `knip`，输出 `Unused exports (2)`。故「导出但只有文件内 / 测试内消费」必被判 dead code——`IpcArgsError` 的形态据此定夺（D9）。
- **zod v4.6.5 行为实测**（`/tmp` 内 `tsc --noEmit` + node 运行，同版 zod 与同一 tsconfig 口径，结论直接决定 P3 的形态）：
  | 写法 | 结论 |
  |---|---|
  | 具名 schema 去掉 `: z.ZodType<X>` 注解 + 字段仍 `z.lazy(() => YSchema)` | ✗ `TS7023`（accessor 返回类型循环）+ `TS2615` + `TS2345` |
  | 同上但 getter 体内**裸引用** `YSchema` | ✓ 零报错；`z.infer` 与手写类型 Equal 全通过 |
  | getter + 非循环引用仍用 `z.lazy`、顶层 `A.and(z.lazy(() => BSchema))` | ✓ 零报错、Equal 通过 |
  | 运行时（node）：`z.strictObject({ get x() { … } })` | ✓ 声明序无关（shape 首次读取时冻结）、深层多余键被拒、`z.unknown().meta({degraded})` 可回读 |
  ⇒ 补丁 3 只能是「getter 化 + **在 getter 体内**裸引用」；不能全局去 `z.lazy`（顶层 `.and(...)` 裸引用会在模块初始化期 TDZ `ReferenceError`），也不能保留 `z.lazy`（`TS7023`）。

**六处与规格 / 提案措辞的校准（不改变规格意图，但决定实现形态）**：

1. **`IpcArgsError` 不导出**：spec「Function: `src/lib/ipc.ts`」把它列为 ipc 面的一项，但产品侧无消费方（错误统一经既有 `String(err)` 呈现），导出即被 knip 判 `Unused exports`（上文实测），而规格又明令 MUST NOT 靠 knip 豁免满足判据。故落为**模块私有类**：`extends Error`、`name = "IpcArgsError"`、`cmd` / `path` / `expected` / `got` 四个只读字段齐备——AC-1 / AC-4 的可观测面（`instanceof Error`、名称、命令名、路径、expected/got、`String(err)` 呈现）逐条不变。
2. **`degraded()` 落生成物 prelude**（proposal 已裁决，此处定死落点）：放 `framework_prelude`（`zod.rs:712` `render_file_header` + `:726` `render_runtime` + `:741` `render_types` 的序列：header → prelude → runtime → raw → types），故 `degraded` 在三张命令表（raw 段）与具名 schema（types 段）之前定义；标识符**不导出**（产品零多余导出，且 `degraded(` 文本仍可 grep）。
3. **`Result` 具名类型隐藏（含两个不同的失败面）**〔回溯修正 R-1〕：`specta-typescript` 侧有 `hide_unused_std_result_type`，`specta-zod` 无对应逻辑，`Result` 会作为一份泛型 schema 落进产物（噪声 + 泛型渲染面）。本设计在 `ZodFormat::map_types` 里把 `Result` 的 `ty` 置 `None`（`export_single_internal` 对 `ty: None` 早返回 `Ok`，该具名 schema 因此不渲染）。前置条件是「无任何保留类型引用它」——本仓 46 命令的 `Result` 全部在结果面被解包 Ok 臂，故恒成立。前置**不**成立时的行为必须分清（上游实测，两者拦在不同环节，都属 fail-closed）：
   - **引用 `types` 中不存在的具名类型**（命令三表渲染期）：`primitives::inline` 解析引用失败，`specta_zod` 以 dangling / missing 类错误返回 → 导出**失败且不写盘**（`zod_test.rs`「引用缺失具名类型时显式失败」臂）。
   - **被隐藏类型（`ty: None`）仍被其它具名类型引用**：`reference_dt` **不检查目标 `ty`**，渲染期照旧输出 `ResultSchema` 引用，而该声明永不生成 → 导出**成功**、产物带一个未声明标识符；此形态**只能由 AC-8 的 tsc 自检桩拦截**（`TS2304` → `bindings:zod-check` 非零退出 → `bindings:export` 非零），MUST NOT 期望导出器给出 `dangling_named_reference`（上游 Reference 分支无该判据，见「回溯修正记录」）。
4. **生成期自检桩形态**：`bindings:export` 之内的独立 `tsc --noEmit -p tsconfig.zod.json`（单文件桩 + 只含 zod 产物的最小 program）。桩文件只 `import` 产物（声明期错误——`TS7022/TS7023`、删别名后悬空名、`z.lazy` 误用、被隐藏类型的悬空引用——在模块进入 program 时即报），零运行期依赖、零测试框架、零 DOM。
5. **相位形态统一：`specta_serde::Format` + `builder().disable_serde_phases()`**〔回溯修正 R-2〕：proposal §4 与 delta spec 的 Module Contract 写「`ZodFormat` = `specta_serde::PhasesFormat`」「`builder()` 零改动」，实施落地为**统一 serde 形态**，理由与范围如下：
   - `specta-serde 0.0.13`（rc.26 窗口内唯一可用版）把 `#[serde(default)]` 字段判为相位差，会把每份 DTO 裂成 `_Serialize` / `_Deserialize` 伴生类型。这既与 R-11「线格式单一形态」纪律（`core/agent` 刻意避开 `skip_serializing_if`）同向冲突，也会让 `bindings.ts` 类型面相对升级前发生语义变化（违 proposal「TypeScript 侧零语义变更」）。
   - 收口方式：在 `builder()` 上一次性 `.disable_serde_phases()`，两份产物同源统一形态；`ZodFormat` 复用公开的 `specta_serde::Format`（上游 `SpectaFormat` 仍 private），不再走 `PhasesFormat`。
   - 三表仍按方向标注 `Phase::Deserialize`（参数面 / Channel 信封）与 `Phase::Serialize`（结果面）并经 `select_phase_datatype` 取值——当前是恒等变换，日后重开相位时各自自动对齐（D14）。
   - 规格的「`builder()` 零改动」据此读作「**命令注册面**零改动（`all_commands!` / `ErrorHandlingMode::Throw` / `semantic_types` / bigint 配置均不动）＋ 相位开关收敛为统一形态」，见 D1 / D14 与待决问题 5。
6. **AC-6 三类面的可观测口径**〔回溯修正 R-3 / R-4〕：AC-6 的类型面里有三处**在产品面不可直接对位**，判据必须落在有源可依的位置，MUST NOT 写成恒真断言：
   - **`char`**：格式层的 `char → z.string()` 是**防御性改写**（挡掉上游的 `.refine(...)`，AC-7 硬约束），当前出线面**零 `char`**（`src/types/generated/bindings.ts` grep 零命中），故产品契约层没有可对位的 `char` 类型。`char` 的等价性由 wrapper 单测 fixture（`ZodFormat::map_types` / `map_type` 的 `char → z.string()` 臂）承担；产品面的替代判据是「产物零 `.refine(`」（AC-7）。
   - **逐命令形参名集合**：前端**类型层拿不到形参名**（TS 函数参数名与元组元素标签不参与可查询类型面，`Parameters<typeof commands[k]>` 只给位置元组），故「`argsSchemas` 键集与 `bindings.ts` 形参名集合 Equal」没有名称来源。可行的名称来源只有两处：① **生成期**——`Casing::CamelCase.apply(rust_param_name)`，与 tauri-specta 渲染 invoke payload 用的是同一映射，由 wrapper 单测 fixture 断言 `since_seq → sinceSeq` 与「键序 = Rust 形参序」；② **运行期**——生成物包装体的 payload 是 shorthand 对象字面量，`invokeMock` 第二实参对象的键集就是线格式键名，代表性命令上断言「payload 键集 == `Object.keys(argsSchemas[cmd])`」。MUST NOT 从类型层派生形参名。
   - **`.refine(` 的生成期真实触发面**：上游 `map_key_datatype` 对数值型 map key（`i8` / `i16` / `i32` / `u8` / `u16` / `u32` / `f16` / `f32` / `f64`）出 `z.string().regex(...).refine(...)`；本仓出线面零 map 类型（`bindings.ts` 零 `Record<`、产物零 `z.record(`），`char` 已被格式层改写成 `z.string()`（不进该分支）。故守卫的命中臂只能在 wrapper fixture 构造（含 `HashMap<u8, _>` / `HashMap<f64, _>` 的命令参数或具名类型），判据为「`ZodLanguage::export` 返回 `Err` 且目标路径不产生文件（不覆盖既有旧文件）」；MUST NOT 用向产物注入文本的方式伪造命中。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 导出入口形态 | `export_bindings()` 保持唯一入口，内部 `let builder = builder();` 后**同一 builder 两次** `.export(...)`（`Typescript` → `bindings.ts`、`ZodLanguage` → `bindings.zod.ts`），`create_dir_all` 与错误映射零改动；`builder()` 的**命令注册面**零改动，仅追加 `.disable_serde_phases()`（相位收敛为统一形态，见校准 5 / D14） | `Builder::export` 是 `&self`；`export-bindings` crate 与 `bindings:export` 命令形态零改动（spec 要求命令名不变）；相位开关必须落在 builder（两份产物共用同一配置），落在 wrapper 只会让两份产物分叉 |
| D2 | 产物形态 | `bindings.zod.ts` 三段结构：① banner + `import { z } from "zod";` + `degraded()`（prelude 段）② `argsSchemas` / `resultSchemas` / `channelArgs` 三张表（raw 段）③ 具名类型 schema 段；外加三条硬约束：零 `export type` / 零 `z.ZodType` 注解 / 零 `.default·.transform·.catch·.pipe·.refine`。三张表的**每一项都是 getter**（`get agentStart() { return … }`），`as const` | getter 表项 = 表构造零成本（R-10 的启动开销归零）+ 跨表引用（`resultSchemas` 引用具名 schema）不会在模块初始化期 TDZ；`as const` 让 `Object.keys` / `Object.entries` 的类型收窄到命令联合 |
| D3 | wrapper 骨架 | `pub(crate) struct ZodLanguage;`（单位结构体）+ `impl tauri_specta::LanguageExt`，`type Error = specta_zod::Error`；`ZodFormat { cast_bigints: bool }` + `impl specta::Format`。内部渲染用 `Zod::new().header(..).framework_prelude(..).with_raw(..).export(&cfg.types, AlreadyMapped)` 取**完整字符串**，自检 + 写盘各一步 | 单位结构体足够承载本地 trait impl（新 type 的实质目的是「本地类型实现外部 trait」）；取字符串而非 `export_to` 是为了在落盘前做完「禁用修饰 + 零 `export type`」的生成期自检（fail-closed 不落半成品） |
| D4 | 格式映射链 | `ZodFormat::map_types` = **`specta_serde::Format.map_types`（统一 serde 形态；不是 `PhasesFormat`，见校准 5）** → 四步改写：① 大整数 `u64/i64/u128/i128/usize/isize` → `define("z.number()")`（受 `cfg.dangerously_cast_bigints_to_number` 开关支配，与 TS 侧逐字同口径）② `char` → `define("z.string()")`（挡掉上游的 `.refine(...)`，AC-7 硬约束；当前出线面零 `char`，纯防御，见校准 6）③ `serde_json::Value` 的所有引用（含 `NamedReferenceType::Inline`）→ `specta_zod::Unknown`，并把 `Value` 的 `ty` 置 `None` ④ `Result` 的 `ty` 置 `None`。渲染时不复用 `Zod::export` 内部的 `map_types`（传 `AlreadyMapped` 直通格式），避免 serde 格式二次施加 | `specta_serde::Format` 是公开可复用层（上游 `SpectaFormat` 是 private，不可复用），且与 builder 关闭相位后的形态自洽；`char` 的 refine 与 `Value` 的 enum union（U2）都会让产物不可用或违 AC-7，必须在格式层而**不是**在 vendor 补丁里解决（补丁只留 P1–P3 三处，跟版面最小） |
| D5 | vendor 落位与形态 | `src-tauri/crates/tools/specta-zod-patched/`（workspace 成员，`crates/tools` 组）；包名保留 `specta-zod`、版本保留 `0.0.4`，依赖改 `{ workspace = true }`（specta / specta-typescript / specta-serde dev-dep）；剥离 `Cargo.lock` / `.cargo_vcs_info.json` / `.cargo-ok`；crate README 记基线版本 / 补丁索引 / 跟版核对清单 / 删除判据。删除判据 = 上游版本覆盖 P1–P3 语义 | 包名与版本不变 ⇒ `specta_zod::` 路径零改、回退只需删 path 依赖换 registry；`crates/tools` 与 `export-bindings` 同层（`tauri-specta`/`specta-typescript` 仍不出壳层侧的工具组，core / infra 零新边）；workspace 继承 pin 让 lockstep 机械成立（AC-10） |
| D6 | PATCH(1) struct strict | `struct_dt` 的 `Fields::Named` 两处（空 shape `:1590` 与非空 `:1600`）恒出 `z.strictObject`；`enum_variant_dt` 的 `strict_object` 形参删掉、恒 strict（`:1762` 的 marker 读取整段删除，`:1868` 的兄弟键补齐保留） | 上游 marker 无人写入（事实核实），「strict 写死、不做双套」（已定决策）意味着连读取分支都不该留——留着就是可被误用的第二形态 |
| D7 | PATCH(2) 删别名 | 删 `:99-190` 别名渲染整段（`typescript_alias_datatype` / `render_type_alias` / `replace_typescript_code` 调用与 `alias_prefix` 处理）+ 删 `:228` 的 `: z.ZodType<{base_name}>` 注解 + 删 `:343`/`:354` 泛型重载里的 `z.ZodType<{base_name}…>` 形参（改 `z.ZodType`）。随之清理因此未使用的 import | 别名是 `z.infer` 的载体，删别名必须同时删引用它的注解（否则悬空名）；当前出线面零泛型类型（`grep '^export type .*<'` 零命中），重载分支仅作形态对齐 |
| D8 | PATCH(3) getter 化 + 引用降级 | `struct_dt` 与 `enum_variant_dt` 的**命名字段**一律出 `get <key>() { return <expr>; }`；`datatype` / `datatype_with_inline_attr` 增 `deferred: bool` 透传（`struct_dt`/`enum_variant_dt` 传 `true`，`export_single_internal` / `inline` / `reference` / 命令渲染入口传 `false`），`reference_dt` 在 `deferred == true` 时出**裸引用** `XSchema`、否则保持 `z.lazy(() => XSchema)` | 见「起点核实」的 tsc 实测表：getter 体内 `z.lazy` 触发 `TS7023`，裸引用通过；而 getter 体外的裸引用会在模块初始化期 TDZ（`AgentEvent` 顶层 `.and(...)` 引用字典序在后的 `AgentEventKind`）。`deferred` 旗标给出「lexically inside a getter」的精确语义，是唯一同时满足「递归不塌 `any`」与「声明序无关」的形态。补丁 4（struct 路径 Intersection 兄弟键）与补丁 5（递归注解兜底）**不做**，README 记录触发条件与处置（该类型位置降级 `degraded("recursive")`，不改渲染器） |
| D9 | 前端零断言装配 | `ipc` = `new Proxy(commands, { get })`，类型即 `typeof commands`。`get` 陷阱：非自有键 / 非函数 → 原样透传；命令键 → 返回**按名字缓存**的包裹函数（`Map<string, unknown>`）。生成物表项取值走 `Reflect.get(table, cmd)` + `instanceof ZodType`（`ZodType` 为值导入的类）收窄，零 `as`、零手写清单、零 `Object.fromEntries` | `no-unsafe-type-assertion: error` 下 `fromEntries(...) as typeof commands` 必红；而带理由的 `eslint-disable` 会被 `reportUnusedDisableDirectives: error` 反噬（若该行实际未触发）。Proxy 让「零手写镜像」在类型层也成立（AC-5），新增命令零改动自动生效，且 `ipc.x === ipc.x`（缓存）不制造 React 依赖抖动。`Reflect.get` 只触发该命令的 getter（不构造其余 45 项），`instanceof` 是运行期收窄而非类型断言 |
| D10 | 参数面实现 | 逐命令取 `Reflect.get(argsSchemas, cmd)` 并以 `instanceof ZodObject` 收窄（失败即「该命令无参数面 schema」→ 跳过校验，fail-open）；`Object.keys(schema.shape)` 得**形参序**（与 Rust 形参序一致，实测 zod 保留声明序）；以 `Object.fromEntries(keys.map((k, i) => [k, args[i]]))` 构造校验输入；`safeParse` 失败 → 抛 `IpcArgsError`（消息 = 命令名 + `z.prettifyError` 全量 issues；字段取首条 issue 的 `path` / `expected` / 路径处原值作 `got`）；**`invoke` 以原参数对象调用**（`Reflect.apply(command, undefined, args)`，不用解析副本）。`Channel` 键在 schema 里是 `degraded(…)`（接受任意值）+ 逐条包裹（D11），故「跳过校验 ≠ 剔出 payload」自动成立 | 规格「MUST NOT 用解析后的副本替换调用参数」；`shape` 取键顺是无镜像前提；`safeParse` 全包 `try/catch`（schema 自身抛异常时降级为 `IpcArgsError`，绝不把内部异常当业务 reject）；**issue 面一律用 zod 类型化的 `ZodIssue`**：`issues[0].path` 直接 `map(String)` 得路径段、`code === 'invalid_type' ? expected : message` 取 `expected`，MUST NOT 对 issue 做 `unknown` 收窄（`'path' in issue` / `Array.isArray` / `typeof expected === 'string'` 一类守卫在真实产物下恒为真，只会在校验语义外再叠一层等价判断）；同一原则适用于表项读取——生成物逐命令都有 `argsSchemas` / `resultSchemas` / `channelArgs` 表项，`instanceof` 收窄之后的 `null` 回退是死分支，放行面改用「空形参 schema / `z.unknown()`」这类**恒成功**替身，不引入 `null` 管道 |
| D11 | Channel 包裹与幂等 | 事实来源 = 生成物 `channelArgs[cmd]`（键名 → 信封 schema）。包裹方式：在**原实例**上 `Object.defineProperty(channel, "onmessage", { get, set })`——`set` 记住调用方 handler，并把一个稳定 forwarder（`validate → reportDrift → handler(msg)`）通过**原型链上的原 setter** 装进 `Channel` 的私有字段；`get` 返回调用方 handler（若原型无 accessor，`get` 返回 forwarder，兼容测试替身）。幂等：模块级 `WeakSet` 已包裹即短路 | `Channel` 的私有字段只能经原型 setter 写入（`core.js` 的 `_Channel_deliver` 直读私有字段，不读 `this.onmessage`），故必须拿原型 descriptor；在**原实例**上打补丁使 `id` / `toJSON` / `cleanupCallback` / 序号全部不变（规格「其它可观察行为零介入」）；`WeakSet` 让「同一 channel 重复传入只报一次」成立（R-9） |
| D12 | 漂移聚合与 DEV 告警 | `ipc-drift.ts`：`Map<string, DriftEntry>`，键 `${cmd}\u0000${path}`，上限 `200` 个键（超限丢弃**新键**，已有键继续累计）；零 `console`、零 IO、零 IPC。告警在 `ipc.ts`：`report(report)` 先取 `driftCount()` → `reportDrift` → 计数变化即 `console.warn`（附命令、路径、`driftCount()` 与新键清单 `driftReports()`）。告警门 = `import.meta.env.DEV === true` | 上限防无界增长；去重防刷屏（R-3）；`console` 在装配层使 collector 可纯数据单测；`import.meta.env.DEV` 只作为**诊断输出可见性**门，MUST NOT 用作任何行为 / UI 判据（构建模式单源仍是后端 `app_environment`，`use-app-environment.ts` 的纪律不破——该门只决定「少打几行日志」，误判方向安全） |
| D13 | 错误面三分落地 | ① 后端 reject：`command(...)` 的 reject 原样向调用方抛（不 catch 成漂移）② 参数面：本地 `IpcArgsError`（`extends Error`，`name` 固定），走既有 `String(err)` → error 态 ③ 漂移：`reportDrift` 只记录，`safeParse` 失败后**返回原值**（结果面）/ **原消息交付 handler**（信封面），零 throw | spec「错误面三分」逐条；`IpcArgsError` 是 `Error` 子类 ⇒ 既有 `String(err)` 呈现零改动（AC-4） |
| D14 | 相位口径 | `builder()` 上 `.disable_serde_phases()` 把 serde 相位**收敛为统一形态**（`specta-serde 0.0.13` 起 `#[serde(default)]` 会判相位差、把 DTO 裂成 `_Serialize` / `_Deserialize` 伴生类型，与「线格式单一形态」纪律冲突）；格式层因此复用 `specta_serde::Format`。三表仍按方向标注：参数面 / Channel 信封取 `Phase::Deserialize`、结果面取 `Phase::Serialize`，经 `specta_serde::select_phase_datatype` 取值（当前恒等变换） | 相位开关单点落在 builder ⇒ 两份产物同源（`bindings.ts` 类型面语义零变更）；方向标注保留 ⇒ 日后重开相位时两份产物各自自动对齐，`ipc.ts` 不需要分支；当前零成本 |
| D15 | 版本与产物纪律 | 三件套 `rc.25 → rc.26` / `specta-typescript 0.0.12 → 0.0.13` + 新增 `specta-serde = 0.0.13` + vendored `specta-zod 0.0.4`；`bindings.ts` 以 rc.26 重导出为准（格式差异逐行审阅后入库，类型面语义零变更）；`packages/desktop/package.json` `version` `0.5.31 → 0.5.33`；`src-tauri/Cargo.toml` 的 `[package] version`（`0.1.0`）**不跟随** | AC-10 / AC-12；AGENT.md 版本纪律（`tauri.conf.json` 经 `../package.json` 跟随，`Cargo.toml` crate 版本线不动） |
| D16 | 隐藏具名类型（`Value` / `Result`）的引用面与兜底 | `Value` 的**一切引用**改写为 `specta_zod::Unknown`（`hidden_references` 收集其全部 `Reference` 后按引用等值改写），故隐藏 `Value` 不会留悬空名；`Result` 只隐藏其**声明**（`ty` 置 `None`），**不改写其引用**（命令结果面由 Ok 臂解包，不产生 `Result` 引用）。两条生成期失败面因此不同：① 三表渲染期引用 `types` 中不存在的具名类型 → `primitives::inline` 报 dangling / missing 类错误、导出失败不写盘；② 被隐藏声明仍被引用（`ty: None` 的目标）→ 上游 `reference_dt` 不检查目标 `ty`，导出**成功**并留下未声明标识符，只由 AC-8 的 tsc 桩（`TS2304`）拦截 | 上游 Reference 分支无「目标是否可渲染」判据（实测），把「隐藏声明」当作可自证安全是错的；本仓 46 命令的 `Result` 全在结果面被解包 Ok 臂 ⇒ ②当前不可达，但失败面必须在设计里写对，否则测试会写出一条**不可生成**的断言（`dangling_named_reference` 实际不出现）。`Value` 走 ① 类改写而非隐藏声明，故 `z.unknown()` 面不悬空 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| vendored exporter | schema 渲染内核（上游 0.0.4 全量）+ 三处补丁（struct strict / 删别名与注解 / getter 化与 deferred 引用） | `src-tauri/crates/tools/specta-zod-patched/**`（新） | `specta`、`specta-typescript`（均为 workspace 继承 pin）；dev-dep `specta-serde` | Rust 2024 edition（上游原样），`PATCH(1..3)` grep 标记 |
| zod 侧 wrapper | `LanguageExt` 实现（取字符串 + 生成期自检 + 落盘）、`Format` 实现（相位 + 大整数 / `char` / `Value` 改写 + `Result` 隐藏）、命令三表渲染、降级清单 stderr | `src-tauri/src/bindings/zod.rs`（新） | `tauri_specta::{LanguageExt, BuilderConfiguration, Casing}`、`specta_zod::{Zod, Error, Unknown, define, primitives}`、`specta_serde::{Format, Phase, select_phase_datatype}`、`specta` | Rust（壳层模块，无 Tauri 依赖新增） |
| 导出入口 | 单 builder 两次导出；`builder()` 追加 `.disable_serde_phases()` 把 serde 相位收敛为统一形态（注册面零改动） | `src-tauri/src/bindings/mod.rs`（改） | 上述 wrapper | 既有形态 + 一行相位开关 |
| 第二产物 | 纯 schema（三表 + 具名 schema + `degraded()`），入库 | `src/types/generated/bindings.zod.ts`（新，生成） | `zod` | 生成物 |
| 生成期自检 | 只 import 产物的桩 + 最小 program 的 `tsc --noEmit` | `src/types/generated/bindings.zod.check.ts`（新）、`tsconfig.zod.json`（新） | `tsc`、`zod` 类型 | 静态检查（非测试） |
| 前端装配层 | 分级校验：参数面 strict / 结果面与信封面 soft-report；`Proxy` 派发；DEV 汇总告警 | `src/lib/ipc.ts`（新） | `bindings`、`bindings.zod`、`zod`、`ipc-drift` | React 无关 TS 模块 |
| 漂移聚合 | `(cmd, path)` 去重 + 计数 + 上限 + 读 / 计数 / 清空 | `src/lib/ipc-drift.ts`（新） | 零依赖 | 进程内 `Map` |
| 聚合生命周期 | 当前根**变更**时清空聚合 | `src/hooks/use-workspaces.ts`（改） | `ipc-drift` | 既有 hook |
| 调用面收口 | 21 个业务模块 `commands.x` → `ipc.x`（类型派生改从 `ipc` 取） | `src/hooks/**`、`src/lib/**`、`src/views/**`（改） | `ipc`、`bindings`（仅类型） | 机械替换 |

---

## 变更清单

> 路径口径：新增文件用仓库根相对全路径；修改文件与 proposal「变更范围 - 实现文件」同口径（`src-tauri/...` 与 `src/...` 省略 `packages/desktop/` 前缀）。测试文件不在本清单（归测试相位）。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/Cargo.toml` | vendored crate 清单：`name = "specta-zod"`、`version = "0.0.4"`、`edition = "2024"`；依赖 `specta = { workspace = true }`、`specta-typescript = { workspace = true }`（保留：`reserved_names` / map key 的 `define` 仍用），dev-dep `specta = { workspace = true, features = ["derive","collect"] }` + `specta-serde = { workspace = true }`（doc test 需要，`cargo test --workspace` 会编译 doctest） | 配置表 `specta-zod` 行；D5 |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/README.md` | 基线版本（0.0.4 / specta rc.26 / specta-typescript 0.0.13）、三处补丁的位置与理由、`PATCH(n)` 标记索引、跟版核对清单（`cargo tree` 单 RC、产物 grep、自检桩）、**删除 vendor 判据**（上游覆盖 P1–P3 语义即改回 registry，不以日历时间为准）、延后补丁（4：struct 路径 Intersection 兄弟键；5：递归注解兜底）与触发后的处置（该类型位置降级 `degraded("recursive")`） | D5 / D8；spec「vendored exporter 落位与跟版」 |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/src/lib.rs` | 上游 0.0.4 原样（`pub use` 面不变：`Error` / `define` / `Unknown` / `Any` / `Never` / `Zod` / `Layout` / `primitives` / `collect_references`） | D3 / D5 |
| `…/specta-zod-patched/src/{error,map_keys,opaque,references,reserved_names,types}.rs` | 上游 0.0.4 原样（零改动，逐字保留以便上游 diff 审阅） | D5 |
| `…/specta-zod-patched/src/primitives.rs` | 上游 0.0.4 + **PATCH(1)**（`struct_dt` 命名分支恒 `z.strictObject`；`enum_variant_dt` 删 `strict_object` 形参、恒 strict、保留兄弟键补齐）+ **PATCH(2)**（删 `:99-190` 别名渲染、`:228` 注解、`:343` 泛型重载注解）+ **PATCH(3)**（命名字段出 getter；`datatype` / `datatype_with_inline_attr` 增 `deferred: bool`；`reference_dt` 在 deferred 下出裸引用） | 公共函数/类型表（属 vendor 内部，不入表）；D6 / D7 / D8 |
| `…/specta-zod-patched/src/zod.rs` | 上游 0.0.4 原样（`Zod` 构造 / prelude / `with_raw` / `export` 全部复用，无补丁） | D3 |
| `packages/desktop/src-tauri/src/bindings/zod.rs` | wrapper：`ZodLanguage`（`LanguageExt`）、`ZodFormat`（`specta_serde::Format` 统一形态 + 三处改写，见 D4）、`AlreadyMapped`（直通格式）、命令三表渲染与 `degraded` prelude 组装、生成期硬约束自检（9 类禁用片段，见 D3 / AC-7）、降级清单 stderr。`Value` 走引用改写、`Result` 只隐藏声明且其失败面归 AC-8 自检桩（D16）。模块文档注释引用能力时**只写规格名**（如「见 desktop-ipc-type-bindings 规格」），MUST NOT 带域目录路径前缀——命名隔离双禁令扫描覆盖 `src-tauri/src` 全部产品 `.rs`（`*_test.rs` 与 `foundation/src/layout/mod.rs` 除外） | 公共函数表三行；类型表 `ZodLanguage` / `ZodFormat`；D3 / D4 / D14 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 生成物入库：prelude（`degraded()`）+ `argsSchemas` / `resultSchemas` / `channelArgs`（逐项 getter）+ 82 个具名 schema（当前出线类型数；`z.strictObject` 写死、无 `export type`、无注解）。首次导出后与 `bindings.ts` 同批提交 | 类型定义表（生成物形态见「数据模型」）；D2 |
| `packages/desktop/src/types/generated/bindings.zod.check.ts` | 生成期自检桩（**手写、非生成物、不被导出链写入**）：只 `import * as zodSchemas from './bindings.zod'` 并把引用透出（保持 `noUnusedLocals` 干净）；无 DOM / 无 Tauri / 无网络 / 无测试框架 | 校准 4；配置表 `bindings:zod-check` 行 |
| `packages/desktop/tsconfig.zod.json` | 自检专用最小 program：`include` = 产物 + 桩；`strict` / `noEmit` / `skipLibCheck` / `target: ES2022` / `lib: ["ES2022"]`（**零 DOM**）/ `module: ESNext` / `moduleResolution: bundler` / `types: []` | 配置表；校准 4 |
| `packages/desktop/src/lib/ipc-drift.ts` | 漂移聚合器：`reportDrift` / `driftReports` / `driftCount` / `resetDrift` + `IpcDriftReport` / `DriftEntry`；零 `console` / 零 IO / 零 IPC | 公共函数表四行；类型表两行；D12 |
| `packages/desktop/src/lib/ipc.ts` | 装配层：`ipc`（`Proxy`）+ 私有 `IpcArgsError` + 参数面 strict / 结果面与信封面 soft-report + Channel 包裹（幂等）+ DEV 汇总告警 + `import { … } from 'zod'`（knip 依赖可见性） | 公共函数表 `ipc` 行；类型表 `IpcArgsError` 行；D9–D13 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `src-tauri/Cargo.toml` | ① `[workspace.dependencies]`：`specta` `=2.0.0-rc.25 → =2.0.0-rc.26`、`specta-typescript` `=0.0.12 → =0.0.13`、`tauri-specta` `=2.0.0-rc.25 → =2.0.0-rc.26`；新增 `specta-serde = "=0.0.13"` 与 `specta-zod = { path = "crates/tools/specta-zod-patched" }`；② `[workspace] members` 增 `"crates/tools/specta-zod-patched"`；③ 根包 `[dependencies]` 增 `specta-serde = { workspace = true }`（wrapper 用 `specta_serde::Format` / `Phase` / `select_phase_datatype`）与 `specta-zod = { workspace = true }` | AC-10 的落位与 lockstep；`tauri-specta` / `specta-typescript` 仍只在根包；core / infra 依赖零改动 |
| `src-tauri/src/bindings/mod.rs` | `pub(crate) mod zod;` + `use self::zod::ZodLanguage;`；`export_bindings()` 内改为 `let builder = builder();` 后两次导出（先 `bindings.ts` 后 `bindings.zod.ts`，顺序固定；第二产物以 `ZodLanguage`（单位结构体，无 `Default`）传入），错误串分别带产物名（`导出 TS bindings 失败` / `导出 zod schema 失败`）；`builder()` **命令注册面零改动**，仅追加 `.disable_serde_phases()`（相位收敛为统一形态，见校准 5 / D1 / D14） | spec「单 builder 两次导出」；导出顺序与幂等（AC-9）；相位开关必须落 builder 才能让两份产物同源 |
| `src/types/generated/bindings.ts` | 由 rc.26 重导出产生；预期零语义差异，若有格式级差异逐行审阅后入库（MUST NOT 手改） | AC-10 的升级噪声隔离（R-12）；`bindings:check` 显式呈现 |
| `src/lib/agent-transport.ts` | `commands` → `ipc`（`sessionDetail` / `agentSessionWatch` / `agentStart` 三处调用 + import 面）；**类型派生**改 `type AgentStartArgs = Parameters<typeof ipc.agentStart>`（注释同步：派生源仍是 `bindings.ts` 的类型，只是经 `ipc` 取形） | AC-5；spec「类型派生保留」 |
| `src/hooks/use-workspaces.ts` | ① `commands` → `ipc`（`listWorkspaces` / `addWorkspace` / `removeWorkspace`）；② `applyRoot`（`:48-51`）内先判变 `if (rootRef.current !== next) resetDrift();` 再写 ref/state（同根重取数不清）；③ import `resetDrift` | AC-13 的 `resetDrift` 非测试消费方；spec「清空入口有产品消费方」 |
| `src/hooks/use-app-environment.ts` | `commands` → `ipc`（`appEnvironment`） | AC-5 |
| `src/hooks/use-agent-session.ts` | `commands` → `ipc`（`sessionDetail` / `agentSessionTranscript` / `agentStop`） | AC-5 |
| `src/views/agent/hooks/use-agent-options.ts` | `commands` → `ipc` | AC-5 |
| `src/views/agent/hooks/use-agent-run-history.ts` | `commands` → `ipc` | AC-5 |
| `src/views/agents/hooks/use-agent-instances.ts` | `commands` → `ipc`（4 处） | AC-5 |
| `src/views/agents/hooks/use-agent-providers.ts` | `commands` → `ipc`（3 处） | AC-5 |
| `src/views/changes/agent/hooks/use-change-chat.ts` | `commands` → `ipc` | AC-5 |
| `src/views/changes/components/change-create-dialog.tsx` | `commands` → `ipc` | AC-5 |
| `src/views/changes/hooks/use-change-detail.ts` | `commands` → `ipc`（`readArtifact` / `getChangeDetail`） | AC-5 |
| `src/views/changes/hooks/use-change-flow-run.ts` | `commands` → `ipc`（5 处，含 `changeFlowWatch` / `changeFlowStart` 的 `Channel<RunNotice>` 首参） | AC-5 / AC-3 |
| `src/views/changes/hooks/use-change-list.ts` | `commands` → `ipc` | AC-5 |
| `src/views/changes/hooks/use-exec-output.ts` | `commands` → `ipc` | AC-5 |
| `src/views/config/hooks/use-workspace-config.ts` | `commands` → `ipc` | AC-5 |
| `src/views/db/hooks/use-db-inspector.ts` | `commands` → `ipc`（2 处） | AC-5 |
| `src/views/explores/components/explore-create-dialog.tsx` | `commands` → `ipc`（3 处） | AC-5 |
| `src/views/explores/hooks/use-explore-doc.ts` | `commands` → `ipc`（`exploreDocPath` / `watchSubscribe`（`Channel<FileWatchEvent>`）/ `watchUnsubscribe`） | AC-5 / AC-3 |
| `src/views/explores/hooks/use-explore-list.ts` | `commands` → `ipc`（7 处） | AC-5 |
| `src/views/explores/hooks/use-explore-session.ts` | `commands` → `ipc` | AC-5 |
| `src/views/info/hooks/use-code-stats.ts` | `commands` → `ipc` | AC-5 |
| `package.json` | ① `dependencies` 增 `"zod": "4.6.5"`（精确 pin，无 `^`/`~`；MUST NOT 落 `devDependencies`）；② `scripts.bindings:export` 追加自检：`cargo run … && pnpm run bindings:zod-check`；③ 新增 `scripts.bindings:zod-check = tsc --noEmit -p tsconfig.zod.json`；④ `version` `0.5.31 → 0.5.33` | AC-8 / AC-11 / AC-12；`bindings:check` 命令串不变（自检经 `bindings:export` 自动纳入） |

<!-- `knip.json` 零改动：生成目录 ignore 覆盖两份产物（不另开条目）、`ignoreDependencies` 四项不变、MUST NOT 为 zod 加豁免。`openspec/config.json` 零改动。`tsconfig.json` 零改动（自检用独立 `tsconfig.zod.json`）。`crates/core/**` / `crates/infra/**` 全部零改动。 -->

### 公共函数 / API

> 口径：Rust 侧仅列 `pub` / `pub(crate)` 的 crate 内可见入口（`bindings` 为 `pub mod`，`zod` 子模块为 `pub(crate) mod`）；`zod.rs` 的私有辅助不入表，实测存在的是 `ZodFormat::{new, remap_dt, remap_fields}` / `hidden_references` / `is_hidden_ndt` / `is_value_ndt` / `is_result_ndt` / `render_command_tables` / `render_phase_dt` / `result_dt` / `channel_envelope_dt` / `guard_forbidden` / `report_degraded` + 直通格式 `AlreadyMapped` + 常量 `FILE_BANNER` / `FILE_PRELUDE` / `CHANNEL_REASON` / `FORBIDDEN_SNIPPETS`。TS 侧列模块级导出。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `export_bindings` | `src-tauri/src/bindings/mod.rs` | 修改 | `pub fn export_bindings() -> Result<(), String>` | 唯一导出入口：`create_dir_all` 后同一 builder 两次 `export`（`bindings.ts` → `bindings.zod.ts`），任一失败即非零退出 |
| `ZodLanguage::export` | `src-tauri/src/bindings/zod.rs` | 新增 | `fn export(self, cfg: &tauri_specta::BuilderConfiguration, path: &std::path::Path) -> Result<(), specta_zod::Error>` | `impl tauri_specta::LanguageExt for ZodLanguage` 的唯一方法：`ZodFormat::new(cfg)` → 映射图 → 渲染三表 → 组装 `Zod`（header / prelude / raw）→ `export(&cfg.types, AlreadyMapped)` 取字符串 → 生成期自检（9 类禁用片段，命中即 `Err` 且不写盘）→ stderr 降级清单 → 写盘 |
| `ZodFormat::map_types` | `src-tauri/src/bindings/zod.rs` | 新增 | `fn map_types(&self, types: &specta::Types) -> Result<std::borrow::Cow<'_, specta::Types>, specta::FormatError>` | `impl specta::Format for ZodFormat`：`specta_serde::Format.map_types`（统一 serde 形态）→ 大整数 / `char` / `Value` 引用改写 → `Value` 与 `Result` 的 `ty` 置 `None`（不渲染） |
| `ZodFormat::map_type` | `src-tauri/src/bindings/zod.rs` | 新增 | `fn map_type(&self, types: &specta::Types, dt: &specta::datatype::DataType) -> Result<std::borrow::Cow<'_, specta::datatype::DataType>, specta::FormatError>` | 命令参数 / 结果顶层 datatype 的映射（`specta_serde::Format.map_type` + 同一套改写） |
| `ipc` | `src/lib/ipc.ts` | 新增 | `export const ipc: typeof commands` | 分级校验后的调用面：`Proxy` 按 `commands` 自有键派发；参数面 strict（`IpcArgsError`，零往返）、结果面与 `Channel` 信封面 soft-report（原值 / 原消息透传）；DEV 汇总告警消费 `driftCount` / `driftReports` |
| `reportDrift` | `src/lib/ipc-drift.ts` | 新增 | `export function reportDrift(report: IpcDriftReport): void` | 唯一写入口：按 `(cmd, path)` 去重累计；键数达上限后丢弃新键（已有键仍累计）。消费方：`src/lib/ipc.ts` |
| `driftReports` | `src/lib/ipc-drift.ts` | 新增 | `export function driftReports(): ReadonlyArray<DriftEntry>` | 当前聚合快照（`readonly`，调用方不得改聚合）。消费方：`src/lib/ipc.ts` DEV 告警（列出当前键） |
| `driftCount` | `src/lib/ipc-drift.ts` | 新增 | `export function driftCount(): number` | 去重后键数。消费方：`src/lib/ipc.ts` DEV 告警（附条数 + 判新增键） |
| `resetDrift` | `src/lib/ipc-drift.ts` | 新增 | `export function resetDrift(): void` | 清空聚合。消费方：`src/hooks/use-workspaces.ts`（当前根变更时） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `ZodLanguage` | `src-tauri/src/bindings/zod.rs` | 新增 | `pub(crate) struct ZodLanguage;`——单位结构体，本地类型承载 `LanguageExt` impl，零状态（D3） |
| `ZodFormat` | `src-tauri/src/bindings/zod.rs` | 新增 | `pub(crate) struct ZodFormat { cast_bigints: bool }`——由 `cfg.dangerously_cast_bigints_to_number` 构造，与 TS 侧大整数口径同源（D4） |
| `IpcDriftReport` | `src/lib/ipc-drift.ts` | 新增 | `export interface IpcDriftReport { readonly cmd: string; readonly path: string; readonly expected: string; readonly got: unknown }`——漂移报告载荷（不 throw）；`ipc.ts` 为构造方（非测试消费方） |
| `DriftEntry` | `src/lib/ipc-drift.ts` | 新增 | `export interface DriftEntry extends IpcDriftReport { readonly count: number }`——聚合条目（`driftReports()` 元素） |
| `IpcArgsError` | `src/lib/ipc.ts` | 新增（**模块私有，不导出**） | `class IpcArgsError extends Error { readonly cmd: string; readonly path: string; readonly expected: string; readonly got: unknown }`（构造时 `this.name = "IpcArgsError"`）——本地参数拒绝；`extends Error` 使既有 `String(err)` 呈现零改动（校准 1 / D13） |
| `commands` / `Bindings` | `src/types/generated/bindings.ts` | 零改动（唯一类型源） | `ipc` 的类型与契约测试的 Equal 断言只从此处取；MUST NOT 转发 zod 类型 |
| `argsSchemas` / `resultSchemas` / `channelArgs` | `src/types/generated/bindings.zod.ts` | 新增（生成） | 三张表均为 `as const` 对象、逐项 getter：`argsSchemas[cmd]: z.ZodObject`（形参序 = Rust 形参序，键名 lowerCamelCase）、`resultSchemas[cmd]: z.ZodType`（`Result` 的 Ok 臂 / `Option` 的 nullable）、`channelArgs[cmd]: Record<string, z.ZodType>`（键 = `Channel` 形参名，值 = 信封 schema） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `dependencies.zod` | `package.json` | 新增 | string（精确版本） | `"4.6.5"` | 运行期依赖（随 bundle 发布），MUST NOT 用范围符 / MUST NOT 落 `devDependencies`；`src/lib/ipc.ts` 直接 import 使 knip 判定被使用 |
| `version` | `package.json` | 修改 | string | `"0.5.33"` | `0.5.31 → 0.5.33`（AC-12；0.5.32 为回溯前收口、0.5.33 收 D10 形态修正）；`src-tauri/Cargo.toml` 的 crate 版本线不跟随 |
| `scripts.bindings:export` | `package.json` | 修改 | string | `"cargo run --quiet -p export-bindings --manifest-path src-tauri/Cargo.toml && pnpm run bindings:zod-check"` | 自检失败以非零退出阻断导出（AC-8）；命令名不变 |
| `scripts.bindings:zod-check` | `package.json` | 新增 | string | `"tsc --noEmit -p tsconfig.zod.json"` | 只 import zod 产物的桩文件自检（strict 化 / getter 化 / 无别名三面同看） |
| `[workspace.dependencies].specta` | `src-tauri/Cargo.toml` | 修改 | string | `"=2.0.0-rc.26"` | 与 vendored `specta-zod 0.0.4`、`tauri-specta rc.26` 同一窗口 |
| `[workspace.dependencies].specta-typescript` | `src-tauri/Cargo.toml` | 修改 | string | `"=0.0.13"` | 同上 |
| `[workspace.dependencies].tauri-specta` | `src-tauri/Cargo.toml` | 修改 | string | `"=2.0.0-rc.26"` | 同上 |
| `[workspace.dependencies].specta-serde` | `src-tauri/Cargo.toml` | 新增 | string | `"=0.0.13"` | wrapper 复用 `specta_serde::Format` / `Phase` / `select_phase_datatype`（builder 关闭相位后形态统一），版本与 `specta-typescript` 同 |
| `[workspace.dependencies].specta-zod` | `src-tauri/Cargo.toml` | 新增 | path | `{ path = "crates/tools/specta-zod-patched" }` | vendored exporter；仅根包依赖 |
| `[workspace].members` | `src-tauri/Cargo.toml` | 修改 | array | `+ "crates/tools/specta-zod-patched"` | 与 `export-bindings` 同组 |
| `include` / `compilerOptions` | `tsconfig.zod.json` | 新增 | JSON | `include: ["src/types/generated/bindings.zod.ts","src/types/generated/bindings.zod.check.ts"]`；`strict/noEmit/skipLibCheck/target ES2022/lib ["ES2022"]/module ESNext/moduleResolution bundler/types []` | 自检桩最小 program（零 DOM、零 node types），可独立 `tsc` 执行 |

---

## 数据模型

**漂移聚合（进程内，零持久化）**

```
Map<`${cmd}\u0000${path}`, { cmd: string; path: string; expected: string; got: unknown; count: number }>
```

- 键：命令名 × jsonPointer 风格路径（`#` 表示根，`#/a/0/b` 逐段 `/`）；`\u0000` 作分隔符（命令名与路径都不含 NUL）。
- 计数：同键重复 N 次 → 1 条记录 + `count === N`；不同键各占一条。
- 上限：`MAX_DRIFT_KEYS = 200`；达上限后**新键丢弃**、已有键继续累计（噪声封顶不掩盖既有漂移）。
- 生命周期：进程内单例；`resetDrift()` 由当前 workspace 根变更触发（跨 workspace 混计会让漂移归属失真）；零 IPC、零写盘、零 UI、零 `console`。

**生成物结构（`bindings.zod.ts`）**

```
// banner（生成器标识 + 「零 export type，类型见 bindings.ts」提示）
import { z } from "zod";
/** 降级哨兵：运行时等价 z.unknown()，`meta.degraded` 记原因（可 grep / 可 .meta() 回读） */
const degraded = (reason: string) => z.unknown().meta({ degraded: reason });

/** 逐命令参数 schema（键序 = Rust 形参序；键名 = invoke payload 键名 lowerCamelCase） */
export const argsSchemas = {
	get "agentStart"() { return z.strictObject({ "onEvent": degraded("…"), "root": z.string(), … }); },
	…
} as const;
export const resultSchemas = {
	get "agentStart"() { return TurnSummarySchema; },
	…
} as const;
export const channelArgs = {
	get "agentStart"() { return { get "onEvent"() { return AgentRunMessageSchema; } }; },
	…
} as const;

export const AgentBlockSchema = z.union([ … ]);   // 具名类型 schema（无注解、无 export type）
```

- 三表逐项 getter ⇒ 表构造零成本；**表项体内仍用 `z.lazy` 引用具名 schema**（表项 getter 之外不是「lexically inside a getter」位置，裸引用会在模块初始化期 TDZ；表项渲染时 `deferred = false`，见 D8）。
- 模块加载期成本：具名 schema 段仍会构造 82 个 schema 壳（字段体为 getter、体内裸引用，延后到首次 parse），与 `bindings.ts` 的模块体同量级；R-10 所述「懒初始化」在无实测证据前不做（业务代码本就同模块图引入 `bindings.ts`）。
- 具名 schema 段：字段级 getter + 字段 getter 体内裸引用（D8）；`z.strictObject` 写死；`degraded("…")` 只出现在 `Channel<T>` 形参位（当前 5 处）。
- 规模：当前出线面 46 命令（三表各 46 项表项、`channelArgs` 5 个信封键）、82 个具名 schema；`argsSchemas` 的键名来源是生成期 `Casing::CamelCase.apply(rust_param_name)`，前端类型层**拿不到**形参名（校准 6）。
- 与类型面关系：`bindings.ts` 仍是唯一业务类型源；两份产物语义等价由契约测试的 `expectTypeOf` Equal 断言承担（测试相位），`bindings.zod.ts` 零 `z.infer` 转发。

**Phase 口径**：`builder()` 关闭 serde 相位分裂（`.disable_serde_phases()`）⇒ 两份产物同源**统一形态**，格式层复用 `specta_serde::Format`（校准 5）；三表仍按方向标注——参数面 / Channel 信封 `Phase::Deserialize`、结果面 `Phase::Serialize`——经 `select_phase_datatype` 取值，当前恒等；未来重开相位时该选择自动对齐（D14）。

---

## 路由 / API 设计

<!-- 不涉及：本变更零新增 / 零删除 IPC 命令（`all_commands!` 登记面不变、`bindings.ts` 命令清单不变）、零 HTTP 端点、零 CLI 子命令（`bindings:export` / `bindings:check` 命令串形态不变，仅内部多一次产物与一步自检）。故本段省略。 -->

---

## 依赖

**运行时依赖**

| 依赖 | 落位 | 版本形态 | 用途 |
|------|------|----------|------|
| `zod` | `packages/desktop/package.json` → `dependencies` | 精确 `4.6.5` | 生成物在模块加载期构造 schema；`ipc.ts` 的 `safeParse` / `instanceof ZodType`、`ZodObject` 运行期收窄 / `prettifyError`；单实例是 `.meta()` 与 `instanceof` 可观测的前提 |
| `specta` | workspace（`=2.0.0-rc.26`） | 精确 | DTO derive 与 `Format` / `Types` / `datatype` API |
| `specta-typescript` | 根包 + vendored crate（`=0.0.13`） | 精确 | `bindings.ts` 导出；vendored crate 的保留字 / map key `define` |
| `tauri-specta` | 根包（`=2.0.0-rc.26`） | 精确 | `Builder` / `LanguageExt` / `BuilderConfiguration` / `Casing` |
| `specta-serde` | 根包（`=0.0.13`） | 精确 | `Format`（serde 语义统一形态；`PhasesFormat` 不在本设计使用）+ `Phase` / `select_phase_datatype` |
| `specta-zod`（vendored） | 根包 path 依赖 | `0.0.4`（crate 自身版本线） | schema 渲染内核 + P1–P3 补丁 |

**构建 / 检查依赖**：`tsc`（既有 devDependency，经 `bindings:zod-check` 执行单文件自检）、`knip`（既有，零配置改动）、`cargo fmt --all` / `cargo clippy --workspace --all-targets`（vendored crate 进入覆盖范围，故需在 vendoring 后跑一次 fmt 收口，避免 `server:check` 反复改写）。无新增构建期依赖。

---

## 验收标准回应

| AC | 设计落点 |
|----|----------|
| AC-1 参数面本地拒绝 | D10：`argsSchemas` + `shape` 键序 + `Object.fromEntries` 构造校验输入；失败抛 `IpcArgsError`（命令名 + `#/…` 路径 + expected/got），`Reflect.apply` 在 `safeParse` 成功之后才执行（零往返）；schema 由后端接受面生成（`Option<T>` 无默认值仍为必填键，`null` 可过、缺键被拒） |
| AC-2 结果面 soft-report | D13 + D12：`safeParse` 失败 → `reportDrift` 一条（`(cmd, path)` 去重）→ 原值 `return`；干净结果零记录；既有降级渲染零改动 |
| AC-3 信封逐条上报 / 不丢流 / 幂等 | D11：原实例 `onmessage` 拦截 + 稳定 forwarder + `WeakSet` 幂等；handler 调用次数与顺序不变，脏消息只多一条报告 |
| AC-4 错误面三分 | D13：后端 reject 原样穿透（不 catch）；`IpcArgsError extends Error` 走既有 `String(err)`；漂移零 throw / 零 reject |
| AC-5 零手写镜像 / 调用点收口 | D9（`Proxy` 按 `commands` 键派发，新增命令零改 `ipc.ts`）+ 变更清单 21 文件机械切换；`commands` 仅在 `ipc.ts` 与类型派生处出现 |
| AC-6 两份产物语义等价 | D4（`specta_serde::Format` 同源 + 大整数 / `char` / `Value` 改写与 TS 侧同口径）+ D8（递归 getter 化，`z.infer` 不塌 `any`）；`Option` / `f64` / 整数 / literal 枚举 / Intersection / `unknown` 六面实测 Equal 通过（起点核实的 tsc 表）；**`char` 面在产品面零实例**（`bindings.ts` 零命中），其等价性由 wrapper fixture 承担、产品面替代判据是「产物零 `.refine(`」（校准 6 / AC-7） |
| AC-7 生成期硬约束 | D4（`char` 不再出 `.refine`）+ D2（零 `export type`、`z.strictObject` 写死）+ D3（落盘前 `guard_forbidden` 扫描 **9 类**禁用片段：`.default(` / `.transform(` / `.catch(` / `.pipe(` / `.refine(` / `export type ` / `z.infer` / `z.ZodType<` / `z.object(`，命中即 `Err` 且不写盘）。其中 `.refine(` 的**真实触发面**是数值型 map key（校准 6），`z.object(` 是 strict 写死的生成期兜底；`degraded(` 只落 `Channel` 形参位（当前 5 处），合法 `unknown` 仍 `z.unknown()` |
| AC-8 生成期自检 | 校准 4：`bindings.zod.check.ts` + `tsconfig.zod.json` + `bindings:zod-check`，经 `bindings:export` 串联，失败非零退出 |
| AC-9 守卫覆盖两份产物且幂等 | D1（同目录同次导出）+ 产物全部确定性（无时间戳 / 无机器路径 / 表顺序 = `commands` 顺序 / 具名 schema 顺序 = exporter 排序）；`bindings:check` 命令串不变 |
| AC-10 版本 lockstep 与落位 | D5 / D15 / 配置表：四件套 `=` 精确 pin 同一窗口 + vendored `0.0.4`；vendor 只落 `crates/tools`、只被根包依赖；core / infra 仍只有 `specta` |
| AC-11 前端 zod 依赖合规 | 配置表（`dependencies.zod = "4.6.5"`）+ `ipc.ts` 直接 import（knip 可见）+ `knip.json` 零改动（生成目录 ignore 覆盖两份产物） |
| AC-12 门禁 / 覆盖率 / 版本 / config 零改动 | 配置表 `version` 行；`openspec/config.json` / `knip.json` / `tsconfig.json` / core / infra 全部零改动（见变更清单脚注） |
| AC-13 四导出各有非测试消费方 | D12 + 变更清单：`reportDrift` / `driftCount` / `driftReports` ← `ipc.ts`；`resetDrift` ← `use-workspaces.ts`；零 knip 豁免 |

---

## 待决问题

1. **rc.26 重导出后 `bindings.ts` 的实际 diff 量**：若出现 DTO 名单 / 字段名 / 可选性变化，`src-tauri/src/bindings/mod_test.rs` 的既有断言面（38 命令名 / 58 DTO 名单 / 逐字段行）可能需同步——属测试相位范围，本设计只保证「类型面语义零变更」并把它作为实现的显式审阅点（R-12）。
2. **递归 getter 化的更深自引用**：本仓唯一递归是 `TreeEntry ↔ DirNode`（实测通过）。若实现期出现更深自引用触发 `TS7023`，处置按 R-2：该类型位置降级 `degraded("recursive")`（哨兵可见、导出期 stderr 列清单），MUST NOT 改为在 Rust 侧重写 TS 类型渲染器。
3. **表项 getter 是否需缓存**：当前每次访问都重建该命令的 schema 对象（微秒级）。若实现期实测单次 `safeParse` 前开销 > 0.1 ms，再在 `ipc.ts` 侧按命令名缓存 schema（不改产物形态）。
4. **`MAX_DRIFT_KEYS = 200` 的取值**：无既有先例可循，取 200 是「远大于真实漂移面、又能封住无界增长」的估计值；若实际观测到被截断，改常量即可（零结构影响）。
5. **proposal §4 与 delta spec 的措辞同步（文档面，非契约面）**：两处仍写「`ZodFormat` = `specta_serde::PhasesFormat`」与「`builder()` 零改动」，与实现的统一形态（校准 5 / D1 / D14）不同。本设计的契约效力以校准 5 为准；文本同步（proposal 已定稿、spec 的 Module Contract / API 表各一行）由提案相位收口，MUST NOT 由测试或实现相位把两处措辞当成可执行契约。
6. **`src-tauri/src/bindings/zod.rs` 模块文档注释的存量表述**：该文件 `ZodFormat` 文档段仍写「若日后出现引用，导出链会以 `dangling_named_reference` 显式失败」，与实测行为（导出成功、由 AC-8 tsc 桩拦截，见校准 3 / D16）不符；属注释同步，随下一次 desktop 源码改动（含 `version` patch）收口，不影响本设计的契约面与测试生成（`zod_test.rs` 的判据已是「导出成功 + 零 `ResultSchema` 声明」）。

---

## 回溯修正记录

> 触发：test-gen 复核「4 处不符中有 3 处源自 design 的过期 / 错误表述」。逐条回正如下，后续相位（test-design / test-gen / verify）以「现值」列为准。

| # | 触发 | 原表述（错误 / 过期） | 现值 | 落点 |
|---|------|----------------------|------|------|
| R-1 | test-gen 无法生成 `Result` 保留引用臂 | 「保留 `Result` 引用 → 导出链以 `dangling_named_reference` 显式失败」 | 上游 `reference_dt` **不检查目标 `ty`**：被隐藏声明（`ty: None`）的引用**导出成功**、产物留未声明标识符，只由 AC-8 的 tsc 桩（`TS2304`）拦截；真正的 dangling / missing 错误只出现在「引用 `types` 中**不存在**的具名类型」的命令表渲染路径（失败且不写盘） | 校准 3、D16、AC-7 / AC-8 行 |
| R-2 | implement 实测（已建议同步） | `ZodFormat` = `specta_serde::PhasesFormat`；「`builder()` 零改动」 | 统一 `specta_serde::Format` + `builder().disable_serde_phases()`（`specta-serde 0.0.13` 的 `#[serde(default)]` 相位分裂会破坏「线格式单一形态」纪律与 `bindings.ts` 类型面语义零变更）；`builder()` 的**命令注册面**仍零改动 | 校准 5、D1、D4、D14、组件表、公共函数表、配置表、依赖表、AC-6 行、mod.rs 修改行 |
| R-3 | test-design 无可对位类型 | 「AC-6 `char` 面 Equal」隐含产品面存在 `char` | 出线面**零 `char`**（`bindings.ts` 零命中）：`char → z.string()` 是防御性改写，等价性由 wrapper fixture 承担；产品面的替代判据是「产物零 `.refine(`」 | 校准 6、D4、AC-6 / AC-7 行 |
| R-4 | test-design 无名称来源 | 「`argsSchemas` 键集 = `bindings.ts` 形参名集合」隐含类型层可取形参名 | TS 参数名 / 元组元素标签不可查询 ⇒ 名称来源只有两处：生成期 `Casing::CamelCase`（wrapper fixture 断言 `since_seq → sinceSeq` 与键序）+ 运行期 `invokeMock` 第二实参键集；另把 `.refine(` 守卫的命中臂改由数值 map key（`HashMap<u8, _>` / `HashMap<f64, _>`）构造，判据「`Err` 且不写盘」 | 校准 6、AC-7 行 |
| R-5 | 规模清点 | 「45 命令」 | **46 命令**（`bindings.ts` 46 个 `__TAURI_INVOKE`；三表各 46 项；`channelArgs` 5 个信封键；82 个具名 schema） | 校准 3、数据模型、D9、D16 |

---

## 不在本篇范围（已知边界）

- `unknown` 白洞契约化（`ArtifactEnvelope.payload` 等约 12 处）：本篇只把「表达不了的降级」与「合法 unknown」在产物文本上区分（`degraded(` vs `z.unknown()`）。
- `MIN_OUTPUT_TOKENS` 类常量出线（`builder.constant`）：另立变更；本篇只落实「schema MUST NOT 承载领域不变量」（零 `.refine()`）。
- prod 漂移观测面（持久化 / 用户可见）：本篇只有进程内 collector + DEV 汇总告警。
- 补丁 4（struct 路径 Intersection 兄弟键）/ 补丁 5（递归注解兜底）：不做，README 记录触发条件与处置。
- `u64 > 2^53` 静默失真 / `NaN` 出 `null`：两侧一致，由 JSON 表达能力决定，不做额外处理。
