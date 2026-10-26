# specta-zod（vendored 0.0.4 + 本地补丁）

本 crate 是上游 [`specta-zod`](https://crates.io/crates/specta-zod) **0.0.4** 的原样拷贝
＋三处本地补丁，用于给 `packages/desktop` 生成第二产物
`src/types/generated/bindings.zod.ts`（纯 zod schema，见 `openspec/specs/desktop-ipc-type-bindings`）。

落位理由：`crates/tools/`（与 `export-bindings` 同层）——它是产物生成面的工具，不是
core / infra 的业务能力；`tauri-specta` / `specta-typescript` / `specta-serde` / 本 crate
仍只被 `dev-team` 壳包依赖，core / infra 的依赖面零新增。

## 基线

| 项 | 值 |
|---|---|
| `specta-zod` | `0.0.4`（crate 自身版本线保持上游值，未跟随 workspace） |
| `specta` | `2.0.0-rc.26`（workspace `=` pin） |
| `specta-typescript` | `0.0.13`（workspace `=` pin） |
| `specta-serde` | `0.0.13`（workspace `=` pin，dev-dependency，doctest 用） |
| edition | `2024`（上游原样） |

上游 0.0.4 的 `Cargo.toml` 依赖改成了 `{ workspace = true }`，版本窗口由
`src-tauri/Cargo.toml` 的 `[workspace.dependencies]` 单点收敛——这让「四件套 + vendored
exporter 落在同一 specta RC 窗口」由 workspace 继承机械保证，而不是靠人工对齐两份清单。

> 注：PATCH(2) 之后 crate 内已无 `specta-typescript` 调用点（唯一消费者是别名渲染）。
> 依赖保留是为了让清单与上游同形、并让 `specta-typescript 0.0.13` 继续以 workspace
> 继承锁定；删除它不影响产物字节。

## 三处补丁

改动点一律带可 grep 的 `PATCH(n)` 标记，标记行紧邻改动说明。

### PATCH(1) — struct 面恒 strict

| 位置 | 改动 |
|---|---|
| `src/primitives.rs` 常量区 | 删 `STRICT_OBJECT_MARKER`（`specta:strict_object`）。上游生态无任何写入方（`specta` / `specta-serde` / `tauri-specta` 三方零命中），该 marker 分支实际是死路径；留着就是可被误用的第二形态 |
| `struct_dt` → `Fields::Named` 空 shape / 非空 shape | `z.object` → `z.strictObject`（写死） |
| `enum_variant_dt` | 删 `strict_object` 形参及其 marker 读取，变体对象恒 `z.strictObject`；`allowed_object_keys` 的兄弟键补齐逻辑保留（`z.unknown().optional()`，不再受 strict 开关约束） |

理由：spec「struct 面一律 `z.strictObject`，MUST NOT 提供 strict / loose 双套产物」。
上游只对枚举变体读该 marker，普通 struct 永远 loose → 多余键拒绝在 struct 面失效。

### PATCH(2) — 删 `export type` 别名与类型注解

| 位置 | 改动 |
|---|---|
| `export_single_internal` 开头整段 | 删 `typescript_alias_datatype` 调用、`specta_typescript::primitives::export`（`export type X = …`）、`replace_typescript_code` 的 layout 分支、`alias_prefix` 处理 |
| `export_single_internal` 具名 schema | `export const XSchema: z.ZodType<X> = …` → `export const XSchema = …` |
| `export_single_internal` 泛型重载 | `): z.ZodType<X<…>>;` → `): z.ZodType;` |
| 辅助函数 | 删 `typescript_alias_datatype` / `typescript_alias_fields` / `resolved_typescript_map_key_alias` / `contains_zod_define` / `fields_contain_zod_define` / `replace_typescript_code` 及其单测（别名渲染的唯一消费者） |

理由：`z.infer` 在递归类型上会塌成 `any` 并静默传播，且本仓没有任何类型需要第二产物
提供（类型主权在 `bindings.ts`）。别名是 `z.infer` 的载体，删别名必须同时删引用它的
注解，否则悬空名。本仓出线面零泛型类型，重载分支仅作形态对齐。

### PATCH(3) — 命名字段 getter 化 + 引用按「是否在 getter 体内」降级

| 位置 | 改动 |
|---|---|
| `struct_dt` → 命名字段 | 一律出 `get <key>() { return <expr>; }`（`optional` 落在返回表达式里） |
| `enum_variant_dt` → 命名字段 | 同上 |
| `datatype` / `datatype_with_inline_attr` / `list_dt` / `map_dt` / `tuple_dt` / `struct_dt` / `enum_dt` / `enum_variant_dt` / `reference_dt` / `reference_named_dt` | 新增 `deferred: bool` 形参并透传：`deferred = true` 表示「当前取值表达式词法上位于字段 getter 体内」 |
| `reference_named_dt` 尾部 | `deferred == true` → 裸引用 `XSchema`；否则保持 `z.lazy(() => XSchema)` |
| 置 `true` 的位置 | 仅 `struct_dt` / `enum_variant_dt` 的命名字段取值表达式；其余入口（`primitives::inline` / `primitives::reference` / 具名 schema 顶层 / `map_key_datatype` 的 key 面 / 元组与 newtype 位）传 `false` |

理由（tsc + node 实测，见 change design「起点核实」）：递归出线类型（本仓
`TreeEntry ↔ DirNode`）用 `z.lazy` 会触发 `TS7023`（accessor 返回类型循环）并连带
`z.infer` 全变 `any`；getter 体内的**裸引用**零报错、`z.infer` 精确。但 getter 体**外**
的裸引用会在模块初始化期触发 TDZ（`AgentEvent` 顶层 `.and(...)` 引用字典序在后的
`AgentEventKindSchema`），所以不能全局去 `z.lazy`。`deferred` 旗标就是「词法上是否在
getter 体内」的精确语义。

## 标记索引（可机器清点）

```
grep -rn "PATCH(" src-tauri/crates/tools/specta-zod-patched
```

| 补丁 | 命中数 |
|---|---|
| `PATCH(1)` | 4 |
| `PATCH(2)` | 4 |
| `PATCH(3)` | 3 |
| 合计 | 11 |

## 跟版核对清单（升级 workspace pin 时必须逐条过）

1. `cargo tree -p dev-team | grep specta` —— 只出现**一个** specta RC 版本，且与
   本 crate 的基线窗口一致（`cargo update` 后先看这条，别先看失败堆栈）。
2. `pnpm run bindings:export` 后对产物 grep 四条硬约束：
   `export type` / `z.infer` / `z.ZodType<` 零命中；`.default(` / `.transform(` /
   `.catch(` / `.pipe(` / `.refine(` 零命中；`z.strictObject(` 覆盖 struct 与变体面；
   `degraded(` 命中全部 `Channel` 形参位。
3. `pnpm run bindings:zod-check`（`tsc --noEmit -p tsconfig.zod.json`）——单文件桩只看
   产物，一条自检同时看住 strict 化 / getter 化 / 删别名三件事。
4. 连续两次 `bindings:export`：`bindings.ts` 与 `bindings.zod.ts` 均零 diff（幂等）。
5. 补丁标记清点：命中数与上表一致（上游若已覆盖某条，删除该条补丁并改 README 表）。
6. `cargo fmt --all` + `cargo clippy -p specta-zod --all-targets`：零格式化抖动、零告警。

## 删除 vendor 的判据

判据是**上游版本覆盖 P1–P3 的语义**，不是时间：

- 上游 struct / 变体面恒 `strictObject`（覆盖 PATCH(1)）；
- 上游提供关闭 `export type` 别名与 `z.ZodType<…>` 注解的开关，或默认不再输出（覆盖 PATCH(2)）；
- 上游对递归 / 前置声明引用改出 getter（或在 getter 体重渲染裸引用）使 `z.infer` 不塌（覆盖 PATCH(3)）。

三者齐备即改为 registry 依赖并删除本目录，产物应零 diff（若产物有 diff 说明语义不等价，
按第 2 条核对）。

## 延后补丁（不做，记录触发条件）

| # | 内容 | 触发条件 | 触发后的处置 |
|---|---|---|---|
| P4 | struct 路径的 Intersection 兄弟键补齐（目前只在枚举路径补 `z.unknown().optional()`） | 出现 `#[serde(flatten)]` 的 struct 交叉类型出线 | 该位置降级 `degraded("intersection-struct")`，不改渲染器（本仓当前唯一 `flatten` 在枚举路径 `AgentEvent`） |
| P5 | 递归类型的注解兜底（给深度自引用补显式类型注解） | 出现比 `TreeEntry ↔ DirNode` 更深的递归自引用并触发 `TS7023` | 该类型位置降级 `degraded("recursive")`（哨兵可见、导出期 stderr 列清单），MUST NOT 在 Rust 侧重写 TS 类型渲染器 |

## 上游原文

`src/**` 除上述补丁外逐字保留（便于与上游 diff 审阅）：`error.rs` / `map_keys.rs` /
`opaque.rs` / `references.rs` / `reserved_names.rs` / `types.rs` / `zod.rs` / `lib.rs` 零改动。
`Cargo.toml` 做了 workspace 继承改写，`Cargo.lock` / `.cargo_vcs_info.json` / `.cargo-ok`
未纳入。
