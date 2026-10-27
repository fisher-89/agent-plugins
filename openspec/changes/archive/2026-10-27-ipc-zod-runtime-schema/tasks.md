# 任务: ipc-zod-runtime-schema

> 按依赖排序：前一阶段的行为后续阶段的前提。测试编写 / 单测 / 集成测试不在本清单（归测试相位）。
> 路径口径同 design：`src-tauri/...` 与 `src/...` 省略 `packages/desktop/` 前缀。

## 阶段 A：版本窗口与 vendoring（一切改动的前置）

- [x] 在 `src-tauri/Cargo.toml` 升 `[workspace.dependencies]`：`specta = "=2.0.0-rc.26"`、`specta-typescript = "=0.0.13"`、`tauri-specta = "=2.0.0-rc.26"`，新增 `specta-serde = "=0.0.13"`；根包 `[dependencies]` 增 `specta-serde = { workspace = true }`。
- [x] 跑 `cargo update -p specta -p specta-typescript -p tauri-specta --manifest-path src-tauri/Cargo.toml`（必要时 `cargo fetch`），确认解析成功且 `cargo tree -p dev-team | grep specta` 只出现**一个** specta RC 版本（`rc.26`）、无 `rc.25` 残留。
- [x] 只跑 `cargo run --quiet -p export-bindings --manifest-path src-tauri/Cargo.toml`（**不**改任何 DTO），逐行审阅 `src/types/generated/bindings.ts` 的 diff：预期零语义差异，仅可能的格式级差异；确认后入库（MUST NOT 手改产物）。若 diff 触及既有 `src-tauri/src/bindings/mod_test.rs` 断言的命令名 / DTO 名单，记录到交付说明（测试相位收口）。
- [x] 从 registry 源（`~/.cargo/registry/src/*/specta-zod-0.0.4/`）把上游全量复制到新目录 `src-tauri/crates/tools/specta-zod-patched/`，剔除 `Cargo.lock` / `.cargo_vcs_info.json` / `.cargo-ok`（保留 `src/**`、`README.md` 改写、`Cargo.toml` 改写）。
- [x] 改写 vendored `Cargo.toml`：`name = "specta-zod"`、`version = "0.0.4"` 保留；`[dependencies] specta` / `specta-typescript` 与 `[dev-dependencies] specta` / `specta-serde` 改 `{ workspace = true }`（dev-dep 的 `specta` 保留 `features = ["derive", "collect"]`，否则 `cargo test --workspace` 的 doctest 编译失败）。
- [x] 写 `src-tauri/crates/tools/specta-zod-patched/README.md`：基线版本（0.0.4 / specta rc.26 / specta-typescript 0.0.13）、三处补丁位置与理由、`PATCH(n)` 标记索引、跟版核对清单（`cargo tree` 单 RC / 产物 grep / 自检桩）、删除判据（上游覆盖 P1–P3 语义即改回 registry）、延后项（P4 struct 路径 Intersection 兄弟键；P5 递归注解兜底）与触发处置（该类型位置降级 `degraded("recursive")`）。
- [x] 在 `src-tauri/Cargo.toml` 把 `"crates/tools/specta-zod-patched"` 加入 `[workspace] members`，并在 `[workspace.dependencies]` 加 `specta-zod = { path = "crates/tools/specta-zod-patched" }`、在根包 `[dependencies]` 加 `specta-zod = { workspace = true }`。
- [x] 在 vendored crate 上跑 `cargo fmt --all --manifest-path src-tauri/Cargo.toml` 并提交格式化结果（一次收口，避免后续 `server:check` 反复改写 vendor 源）。

## 阶段 B：vendored 三处补丁

- [x] **PATCH(1) struct strict**：`src/primitives.rs` 的 `struct_dt` 命名分支（空 shape 与非空 shape 两处）恒出 `z.strictObject`；`enum_variant_dt` 删除 `strict_object` 形参及其 marker 读取（`STRICT_OBJECT_MARKER` 常量与 `:1762` 调用点同步清理），恒出 strict，保留 `allowed_object_keys` 的兄弟键补齐逻辑；每处加 `PATCH(1)` 标记与一行改动说明。
- [x] **PATCH(2) 删 `export type`**：删除 `export_single_internal` 的别名渲染整段（`typescript_alias_datatype` / `render_type_alias` / `replace_typescript_code` 的 layout 分支 / `alias_prefix` 处理）、删除 `export const XSchema: z.ZodType<X>` 的注解、删除泛型重载里 `z.ZodType<X …>` 的形参（改 `z.ZodType`）；清理因此未使用的 `use`（`Typescript` / `TypescriptLayout`），加 `PATCH(2)` 标记。
- [x] **PATCH(3) getter 化 + deferred 引用**：`struct_dt` 与 `enum_variant_dt` 的每个命名字段改出 `get <key>() { return <expr>; }`（`optional` 仍落在返回表达式里）；`datatype` / `datatype_with_inline_attr` 增 `deferred: bool` 形参并透传，`struct_dt` / `enum_variant_dt` 传 `true`、其余入口（`export_single_internal` / `inline` / `reference` / 顶层交集段）传 `false`；`reference_dt` 在 `deferred == true` 时出裸引用 `XSchema`、否则保持 `z.lazy(() => XSchema)`；加 `PATCH(3)` 标记。
- [x] 对 vendored crate 跑 `cargo clippy -p specta-zod --all-targets --manifest-path src-tauri/Cargo.toml` 与 `cargo fmt --all`，确认补丁不引入告警 / 格式化抖动；核对 `grep -rn "PATCH(" src-tauri/crates/tools/specta-zod-patched` 命中数与 README 记录的补丁条数一致。

## 阶段 C：wrapper（`src-tauri/src/bindings/zod.rs`）

- [x] 新建 `src-tauri/src/bindings/zod.rs`，先落 `ZodFormat { cast_bigints: bool }` + `impl specta::Format`：`map_types` = `specta_serde::Format.map_types`（统一 serde 形态，相位由 builder 关闭，见阶段 C 的回溯修正项）→ 递归改写（`u64/i64/u128/i128/usize/isize` → `specta_zod::define("z.number()")`，仅当 `cast_bigints`；`char` → `define("z.string()")`；`serde_json::Value` 的所有具名引用（含 `NamedReferenceType::Inline`）→ `specta_zod::Unknown`）→ 把 `Value` 与 `std/core::Result` 的 `ty` 置 `None`；`map_type` 做同一套顶层改写。
- [x] 在 `zod.rs` 实现三个私有辅助（落地名）：`result_dt(result, types) -> Option<&DataType>`（`Result` 的 Ok 泛型臂）、`channel_envelope_dt(dt, types) -> Option<&DataType>`（`name == "TAURI_CHANNEL" && module_path.starts_with("tauri::")`，照 `tauri-specta` 判据）、`render_phase_dt(…)`（`select_phase_datatype` + `remap_dt` + `primitives::inline`）。
- [x] 实现三表渲染（私有，落地名 `render_command_tables`）：参数表逐形参 `Casing::CamelCase.apply(name)` 作键、`degraded("channel: 非 JSON 可表达，跳过值校验")` 占位 Channel 位、其余走 `primitives::inline(exporter, mapped_types, remap_dt(select_phase_datatype(…)))`；零形参命令出 `z.strictObject({})`；结果表（`result_dt` → `Phase::Serialize` → inline，`None` 回落 `z.unknown()`）；Channel 表（逐 `Channel` 位出 `{ get <key>() { return <信封 schema 表达式>; } }`）。表项一律 `get <accessorKey>() { return …; }`，accessorKey = `cfg.function_casing.apply(cfg.command_accessor_name(command))`，键统一加引号。
- [x] 组装产物：`.header(生成器 banner)`、`.framework_prelude(import zod + banner + degraded 定义)`、`.with_raw(三表文本)`、`export(&cfg.types, AlreadyMapped)` 取完整字符串；落盘前跑 `guard_forbidden`（9 类禁用片段：`.default(` / `.transform(` / `.catch(` / `.pipe(` / `.refine(` / `export type ` / `z.infer` / `z.ZodType<` / `z.object(` 命中即 `Err`）；对每个 `Channel` 形参向 stderr 打一条降级清单（`[zod-bindings] degraded: <cmd>.<arg> — <原因>`），最后 `std::fs::write(path, text)`。
- [x] 实现 `pub(crate) struct ZodLanguage;` + `impl tauri_specta::LanguageExt for ZodLanguage`（`type Error = specta_zod::Error`），把上述步骤串成 5 步流水（映射 → 三表 → 组装导出 → 自检 → 写盘）。
- [x] 改 `src-tauri/src/bindings/mod.rs`：加 `pub(crate) mod zod;` 与 `use self::zod::ZodLanguage;`，`export_bindings()` 内 `let builder = builder();` 后串两次 `.export(...)`（先 `bindings.ts`、后 `bindings.zod.ts`，第二产物以单位结构体 `ZodLanguage` 传入，错误串分别带产物名）。
- [x] 在 `builder()` 上追加 `.disable_serde_phases()`（命令注册面零改动）：`specta-serde 0.0.13` 会把 `#[serde(default)]` 判为相位差、把 DTO 裂成 `_Serialize` / `_Deserialize` 伴生类型——与「线格式单一形态」纪律冲突，也会让 `bindings.ts` 类型面语义相对升级前变化。关闭相位后两份产物同源统一形态（回溯修正 R-2）。
- [x] 跑 `pnpm run bindings:export`（此时仅 cargo 段，自检脚本尚未接），检查 `src/types/generated/bindings.zod.ts`：三表存在且逐项 getter、具名 schema 段零 `export type` / 零 `z.ZodType` 注解、`z.strictObject` 全覆盖、`degraded(` 命中 5 个 `Channel` 形参位、合法 `unknown` 仍 `z.unknown()`、stderr 打出 5 条降级清单。
- [x] 连跑两次导出确认 `bindings.zod.ts` 与 `bindings.ts` 均零 diff（幂等，AC-9）。
- [x] 把握手产物 `src/types/generated/bindings.zod.ts` 与 `bindings.ts` 同批入库。

## 阶段 D：生成期自检链

- [x] 新建 `src/types/generated/bindings.zod.check.ts`（手写桩，非生成）：只 `import * as zodSchemas from './bindings.zod'` 并透出引用（保持 `noUnusedLocals` 干净），零 DOM / 零 Tauri / 零网络 / 零测试框架。
- [x] 新建 `tsconfig.zod.json`：`include` 只含产物与桩、`strict` / `noEmit` / `skipLibCheck` / `target: ES2022` / `lib: ["ES2022"]` / `module: ESNext` / `moduleResolution: bundler` / `types: []`。
- [x] 在 `package.json` 加 `bindings:zod-check = tsc --noEmit -p tsconfig.zod.json`，并把 `bindings:export` 改为 `cargo run --quiet -p export-bindings --manifest-path src-tauri/Cargo.toml && pnpm run bindings:zod-check`。
- [x] 跑 `pnpm run bindings:zod-check` 确认通过（同时看住 strict 化 / getter 化 / 删别名），并故意在工作副本里把某字段回退成 `z.lazy` 验证自检报错、`bindings:export` 非零退出后再还原（手工验证一次即可，验证结果写进交付说明）。

## 阶段 E：前端装配层

- [x] 在 `package.json` 的 `dependencies` 加 `"zod": "4.6.5"`（精确版本，不加 `^`/`~`），跑 `pnpm install` 后确认 `pnpm why zod` 仍只有 `4.6.5`（一份 4.x）与 `3.25.76`（仅 `shadcn → @modelcontextprotocol/sdk` 链）。
- [x] 新建 `src/lib/ipc-drift.ts`：`IpcDriftReport` / `DriftEntry` 类型、`Map` 聚合（键 `${cmd}\u0000${path}`）、`MAX_DRIFT_KEYS = 200`（超限丢新键）、`reportDrift` / `driftReports` / `driftCount` / `resetDrift` 四导出；零 `console` / 零 IO / 零 IPC。
- [x] 新建 `src/lib/ipc.ts`：`import { commands } from '../types/generated/bindings'`、`import { argsSchemas, channelArgs, resultSchemas } from '../types/generated/bindings.zod'`、`import { type ZodType, ZodObject, prettifyError } from 'zod'`、`import { type IpcDriftReport, driftCount, driftReports, reportDrift } from './ipc-drift'`。
- [x] 在 `ipc.ts` 实现私有 `IpcArgsError`（`extends Error`、`name = "IpcArgsError"`、`cmd` / `path` / `expected` / `got`），消息 = `命令 <accessor> 参数校验失败：` + `prettifyError(error)`；`safeParse` 全包 `try/catch`（schema 自身抛异常也收敛为 `IpcArgsError`）。
- [x] 在 `ipc.ts` 实现参数面（`Reflect.get(argsSchemas, cmd)` + `instanceof ZodObject` 收窄 → `Object.keys(shape)` 得形参序 → `Object.fromEntries` 构造校验输入 → 失败抛 `IpcArgsError`）、结果面（`Reflect.get(resultSchemas, cmd)` + `instanceof ZodType` 收窄 → `safeParse` 失败 → `report(report)` → **原值** `return`）、DEV 告警（`import.meta.env.DEV === true` 时按 `driftCount()` 变化打印一次，附命令、路径、条数、`driftReports()` 键清单）。
- [x] 在 `ipc.ts` 实现 Channel 包裹：`Reflect.get(channelArgs, cmd)` 取该命令的信封表（`Record<string, ZodType>`，逐值 `instanceof ZodType` 收窄）与键名 → 键名映射到形参下标（与参数面同一份 `Object.keys(shape)` 顺序）→ 在**原实例**上 `Object.defineProperty(channel, "onmessage", { get, set })`（set 记 handler 并经原型 setter 装稳定 forwarder：`validate → report → handler(msg)`；原型无 accessor 时 `get` 返回 forwarder 以兼容替身）→ 模块级 `WeakSet` 幂等短路。
- [x] 在 `ipc.ts` 实现 `export const ipc: typeof commands = new Proxy(commands, { get(...) {…} })`：只拦 `Object.hasOwn` 的函数键、按名字 `Map` 缓存包裹函数、其余原样透传；包裹函数 `(...args) => { 参数校验 → Channel 包裹 → await Reflect.apply(command, undefined, args) → 结果 soft-report → 返回原值 }`。逐函数体控制在 lint 的 50 行内（拆辅助函数）。
- [x] 改 `src/hooks/use-workspaces.ts`：`commands` → `ipc`；`applyRoot` 内先判变（`rootRef.current !== next`）再 `resetDrift()` 并写 ref/state。
- [x] 按变更清单把余下 20 个业务模块的 `commands.` → `ipc.`（`use-agent-session` / `use-app-environment` / `agent-transport` / `use-agent-options` / `use-agent-run-history` / `use-agent-instances` / `use-agent-providers` / `use-change-chat` / `change-create-dialog` / `use-change-detail` / `use-change-flow-run` / `use-change-list` / `use-exec-output` / `use-workspace-config` / `use-db-inspector` / `explore-create-dialog` / `use-explore-doc` / `use-explore-list` / `use-explore-session` / `use-code-stats`）——`agent-transport.ts` 的 `AgentStartArgs` 改 `Parameters<typeof ipc.agentStart>`。
- [x] 跑 `grep -rn "commands\." src --include=*.ts --include=*.tsx | grep -v "src/types/generated" | grep -v "\.test\."` 确认只剩 `ipc.ts` 内的 `Proxy` 目标引用与注释（AC-5）。
- [x] 检查 `ipc.ts` 内零 `as` 断言：跑 `pnpm run client:check` 确认 `typescript/no-unsafe-type-assertion` / `no-explicit-any` 零触发（且无残留 disable 注释，`reportUnusedDisableDirectives: error` 会反噬）。
- [x] 按 D10 收口 `ipc.ts` 的 issue 面形态（回溯修正：原实现对 zod issue 做 `unknown` 收窄，真实产物下那些守卫恒为真，只产出语义等价层）：`issues[0].path` 直接 `map(String)`、`code === 'invalid_type' ? expected : message` 取 `expected`；删除不可达守卫与死回退（`jsonPointer` 空段 / 转义、`valueAtPath` 中间段、`channelSchemasOf` 表项判空、参数面 / 结果面的 `null` 回退、Channel set 陷阱里重复装 forwarder）；保留 D10 要求的 `instanceof` fail-open（缺表项退化为空形参 / 不做观测）与 schema 自身异常的收敛降级。

## 阶段 F：收尾与门禁

- [x] 把 `package.json` 的 `version` 从 `0.5.31` 升到 `0.5.32`（`src-tauri/Cargo.toml` 的 crate 版本线不动、`tauri.conf.json` 不手改）。
- [x] 回溯修正随动：`version` `0.5.32 → 0.5.33`（AGENT.md 版本纪律：每处 desktop 源码改动一档 patch；`tauri.conf.json` 经 `../package.json` 跟随、`Cargo.toml` crate 版本线仍不动）。
- [x] 跑 `pnpm run bindings:check`：确认导出幂等（两份产物零 diff）、自检通过、`git diff --exit-code -- src/types/generated` 干净。
- [x] 对产物做静态核对：`grep -c "export type\|z.infer" src/types/generated/bindings.zod.ts`（零）、`grep -E "\.(default|transform|catch|pipe|refine)\(" `（零）、`grep -c "degraded("`（5）、`grep -c "z.strictObject("`（与具名 schema + 命令参数面数量一致）、`grep -c "z.unknown()"`（合法 unknown 面仍在）。
- [x] 跑 `pnpm -C packages/desktop run check`（`server:check` + `bindings:check` + `client:check`）确认全绿，重点看 knip：`ipc.ts` / `ipc-drift.ts` 零 dead code、`zod` 未被报未声明 / 未使用、`knip.json` 零改动。
- [x] 跑 `cargo tree --manifest-path src-tauri/Cargo.toml` 复核 AC-10：单一 specta RC 版本；`tauri-specta` / `specta-typescript` 只出现在根包与 `crates/tools`；`crates/core/**` 与 `crates/infra/**` 的 `Cargo.toml` 零改动。
- [x] 回溯修正 R-1（设计表述）：回正 `Result` 隐藏的失败面——上游 `reference_dt` 不检查目标 `ty`，被隐藏声明的引用**导出成功**、悬空标识符只由 AC-8 的 tsc 桩拦截；真正的 dangling / missing 错误只出现在「引用 `types` 中不存在的具名类型」的三表渲染路径（失败且不写盘）。`zod_test.rs` 的判据据此写成「导出成功 + 零 `ResultSchema` 声明 + 残留泛型调用形态归自检桩」。
- [x] 回溯修正 R-3 / R-4（设计可观测口径）：明确 `char` 在产品面零实例（`bindings.ts` 零命中）、形参名集合在类型层不可得（判据改为生成期 `Casing::CamelCase` fixture + 运行期 `invokeMock` 第二实参键集）、`.refine(` 守卫的真实触发面是数值型 map key（fixture 构造，判据「`Err` 且不写盘」）。
- [x] 回溯修正 R-5：规模清点为 **46 命令**（三表各 46 项、`channelArgs` 5 个信封键、82 个具名 schema），设计全文的「45 命令」一并回正。
- [x] 交付说明里记录：`bindings.ts` 的 rc.26 diff 审阅结论、自检手工反例验证结果、降级清单内容、`pnpm why zod` 复核输出、以及「`src-tauri/src/bindings/mod_test.rs` 是否需要测试相位同步」的判断依据。
- [x] 收口设计的待决问题 6（R-1 / D16 的文档面）：`src-tauri/src/bindings/zod.rs` 的 `ZodFormat` 文档段按实测行为改写——被隐藏声明（`ty: None`）的引用**导出成功**、产物留未声明标识符，只由生成期 tsc 自检桩（`TS2304`）拦截；`dangling_named_reference` 只出现在「引用 `types` 中不存在的具名类型」的三表渲染路径（失败且不写盘）；命令规模按 R-5 回正为 46。同时按错误面三分回正 `src/lib/ipc-drift.ts` 的 `reportDrift` 消费方说明（结果面 / 信封面；参数面走本地 `IpcArgsError`，不入聚合）。
- [x] 上述注释收口随 `version` `0.5.33 → 0.5.34`（AGENT.md 版本纪律；`tauri.conf.json` 经 `../package.json` 跟随、`Cargo.toml` crate 版本线不动）。
