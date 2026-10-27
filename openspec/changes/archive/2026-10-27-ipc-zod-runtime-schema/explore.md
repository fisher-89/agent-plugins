# IPC 契约运行期校验：bindings 同源生成 zod schema

workflow_type: requirement

> **一句话**：在 `src-tauri/src/bindings/mod.rs` 的同一个 specta builder 上多挂一个导出器，产出
> `bindings.zod.ts`（纯 schema）；前端参数面 strict、结果面 soft-report，实现「既观测协议漂移、
> 又防脏数据把 UI 打崩」。

本文档为终态梳理：只留结论与证据，演进过程中被否决的方案收在 §10。

---

## 1. 结论摘要

```
产物
  bindings.ts        类型 + 45 个 commands 包装（不变，唯一类型源）
  bindings.zod.ts    纯 zod schema（新增，无 export type，同目录入库）
  lib/ipc.ts         手写 ~20 行装配层（args / result 两个校验出口）

实现
  vendor specta-zod 0.0.4（官方 exporter，5 处本地补丁）
  + 我们自己的 ~150–200 行 wrapper（LanguageExt + Format + args 遍历）

运行期
  参数面（前端 → Rust）  strict，失败本地 throw，零 IPC 往返
  结果面（Rust → 前端）  soft-report，原值返回，现有降级渲染照旧
  Channel 信封           逐条 soft-report（漂移热图）

版本
  specta rc.25→rc.26 / specta-typescript 0.0.12→0.0.13 / tauri-specta rc.25→rc.26
  + 新增 specta-zod = 0.0.4（与 rc.26 唯一 lockstep 窗口）
```

---

## 2. 现状（证据）

```
Rust DTO (specta::Type)
   ├─ main.rs  generate_handler!      ← all_commands! 单一登记面
   └─ bindings/mod.rs builder()       ← 同一登记面 + ErrorHandlingMode::Throw
            │  .semantic_types(serde_json::Value → unknown)
            │  .dangerously_cast_bigints_to_number()
            ▼
   src/types/generated/bindings.ts    (1519 行 / 45 命令 / 81 个 export type)
            ▼
      29 处 commands.xxx()           ← 运行期零校验
      bindings:check = export + git diff --exit-code   ← 只管「产物是否过期」
```

| 事实 | 数字 / 位置 |
|---|---|
| 命令数 | 45（`bindings.ts:6-254`） |
| 出线类型数 | 81 |
| 现有 zod | 0（`zod 4.6.5` 仅作为 ai-sdk 传递依赖在树里，未声明） |
| 出线为 `unknown` 的字段 | 12 处（全部来自 `serde_json::Value`） |
| `?:` 可选字段 | 14 处 |
| 相位分裂（`*_Serialize`/`*_Deserialize`） | **0**（唯一带 Type derive 的 `event.rs` 刻意避开 `skip_serializing_if`，注释明写理由） |
| 仓库内 `#[serde(default…)]` | 78 处，约 50 处在出线类型上（`event.rs` 10 / `runner.rs` 2 / `store/model.rs` 38）；`checks/model.rs` 的 28 处无 Type derive，不出线 |

### `unknown` 白洞清单（校验力收益为 0 的部分）

| 字段 | Rust 源头 |
|---|---|
| `ArtifactEnvelope.payload` | `core/workflow/src/artifacts/envelope.rs:17` |
| `AgentBlock::toolUse.input` | 工具入参原 JSON |
| `AgentBlock.rawUse` / `rawResult` | 回放源 |
| `AgentCitation.raw` | `core/agent/src/event.rs:101` |
| `AgentEventKind::systemNotice.payload` | — |
| `AgentEventKind::turnDone.usage` | `core/agent/src/port.rs:30` |
| `ConfigExtraField.value` | `core/config/src/lib.rs:160` |
| `RecordEnvelope.key` / `.value` | `infra/store/src/envelope.rs:37/39` |
| `SessionRow.configSnapshot` | `core/agent/src/session.rs:40/56` |

---

## 3. 目标与分级策略

**目标：观测漂移 + 防脏数据，二者都要。** 唯一自洽形态是按方向分级：

```
┌───────────────────────────────────────────────────────────────┐
│ 参数面（前端 → Rust）   strict                                 │
│   失败 = 我方 bug → 本地 throw IpcArgsError（零 IPC 往返）      │
│   收益：把「后端反序列化失败」从 ms 级往返提前到 0ms，          │
│         并抓 null / undefined 混用（tauri 对缺键直接失败）      │
├───────────────────────────────────────────────────────────────┤
│ 结果面（Rust → 前端）   soft-report                            │
│   safeParse 失败 → 上报 {cmd, jsonPointerPath, expected, got}  │
│                  → 原值返回（~15 处现有降级渲染照旧）           │
│   漂移可见 + UI 不崩 = 「二者都有」                             │
├───────────────────────────────────────────────────────────────┤
│ 流式面（Channel 信封）  逐条 soft-report                       │
│   漂移最易发生处（事件 schema 演进；bindings.ts 已注明「旧客户端  │
│   读到含 WebSearch 块的事件会整条事件解码失败」）               │
└───────────────────────────────────────────────────────────────┘

错误面必须三分，否则分级无从落地：
   invoke reject: string   —— 后端业务失败（语义不变）
   IpcArgsError            —— 前端校验失败（本地，可定位命令+字段路径）
   IpcDriftReport          —— 结果面漂移（不是 throw，走上报）
```

参数面为什么 strict 反而安全：schema 就是从「后端接受面」生成的，合法输入不可能被拒。

---

## 4. 技术选型

### 4.1 采用 `specta-zod`（官方 exporter），不自写渲染器

`tauri-specta` 每个 exporter 内部都复用同一套语义层，自写也只需写「DataType → 目标语法」，
而这件事上游已经做了：

```rust
// tauri-specta/src/lang/js_ts.rs:17-43（Typescript 的 LanguageExt 实现）
fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
    let types = hide_unused_std_result_type(&cfg, cfg.types.clone());
    let format = SpectaFormat::new(&cfg);          // ← 语义层，可复用
    Exporter::from(self).framework_*(...).export_to(path, &types, format)
}
```

`SpectaFormat::map_types` 一气做完三件事：serde 相位（rename/tag/flatten/default）→
semantic_types（`Value → unknown`）→ Remapper（`bigint → number`）。
`DataType` 只有 10 个变体，上游 0.0.4 已全部覆盖（含保留字、整数范围、intersection 兄弟键补丁）。

### 4.2 版本窗口（唯一全绿组合）

| specta-zod | pin 的 specta | pin 的 specta-typescript | 结论 |
|---|---|---|---|
| 0.0.3 | =rc.25 | =0.0.12 | 与现网零升级对齐，但**无 `z.lazy`**（递归产物运行时 `ReferenceError`） |
| **0.0.4** | **=rc.26** | **=0.0.13** | **采用**（tauri-specta rc.26 也 pin rc.26） |
| 0.0.5 | =rc.27 | =0.0.14 | ✗ tauri-specta 最高只有 rc.26 |

### 4.3 否决：JSON Schema 产物

曾考虑「另一份产物 + 前端 `z.fromJSONSchema`」，已否决（用户决定：前端不依赖 JSON Schema），
且实测有硬伤：

```
JSON Schema 没有「类型交汇」原语；allOf + 各段 additionalProperties:false 会互判对方字段多余。
AJV 实测合法数据被拒：additionalProperties(kind)/(eventType)/(rawJson)/(seq)/(timestampMs) + 连锁 oneOf 失败
```

要正确只有「分配律展开」，而展开会破坏类型等价断言（§9.2）。结论：**不做 JSON Schema 产物**。

### 4.4 否决：`z.json()` 作 `unknown` 的过渡档

实测性能（同 payload，`safeParse` 单次）：

| payload | `z.unknown()` | `z.json()` |
|---|---|---|
| 754 B | 5 µs | 605 µs |
| 7.7 KB | 10 µs | 1673 µs |
| 40 KB | 2 µs | 1997 µs |

`z.json()` 是递归 union，慢 100–1000×。合法 `serde_json::Value` 必须留 `z.unknown()`。
（顺带：降级到 `z.unknown()` 不损失性能，但也不带来校验力——它的价值只是「别让表达不了的
类型搞坏整份产物」，必须靠命名哨兵可见，见 §7.3。）

---

## 5. 产物职责切分

```
bindings.ts        类型 + commands       唯一类型源（stable exporter、递归精确、JSDoc 保留）
bindings.zod.ts    纯 schema            无 export type、无 JSDoc，只供运行期 parse
lib/ipc.ts         装配 ~20 行           import 二者，产出 ipc.xxx / 泛型 guard
tests/contract     Equal 断言            手写，不依赖生成别名
```

三方不可互换，理由逐条：

| 替代方案 | 否决理由 |
|---|---|
| zod 产物替代 bindings.ts | **结构性不可能**：`specta-zod::Zod::export(&self, types: &Types, format)` 只吃 `&Types`，永远看不到 `BuilderConfiguration`。45 个 typed invoke 包装、`Channel<AgentRunMessage>` 参数、`.then()` 变换全部活在 `cfg.commands` 里，只有 tauri-specta 能渲染 |
| bindings.ts 转发 zod 的类型 | ① 丢全部 JSDoc（bindings.ts 每字段一段中文说明）② 类型主权交给 partial 上游（Zod = Partial，TypeScript = Stable）③ 相位分裂时前端会拿到 `X_Deserialize \| X_Serialize` 联合而非线格式类型（今天靠注释纪律维持 0 分裂） |
| zod 产物保留 `export type` | **`z.infer` 在递归类型上退化成 `any`**（§9.2 实测 TS7022），`any` 静默传播；而这些类型没有一个是 specta 类型给不了的 |

---

## 6. 上游能力与缺口（实测）

### 6.1 上游已内建且实测正确

在 `/tmp` 用 `specta rc.25 + specta-zod 0.0.3` 真跑（0.0.4 除 lazy 外同源）：

| 边界 | 上游行为 | 结论 |
|---|---|---|
| 只有 `#[serde(default)]` | Deserialize `.optional()` / **Serialize 不 optional** | ✓ 正是需要的相位规则 |
| `default` + `skip_serializing_if` | 两相位都 `.optional()` | ✓ |
| 无 default 的 `Option<T>` | 两相位 `z.string().nullable()`（required 但值可 null） | ✓ 不吃漂移 |
| `rename_all` / `rename_all_fields` | → camelCase | ✓ |
| `serde(flatten)` | `.and(...)`，与 bindings.ts 的 `A & B` 同构 | ✓ |
| `f64` | `z.number().nullable()`，源码注释「JSON serializers encode non-finite floats as null」 | ✓ 与 specta-typescript 的 `number \| null` 一致 |
| 整数 | `z.int().min(-128).max(127)` 逐个带范围 | ✓ 比 TS 面更严 |
| `char` / map key / 定长数组 / 保留字 | refine / `z.record` / `z.tuple` / `reserved_names` | ✓ |
| 变体级 strict | `specta:strict_object` 属性 → `z.strictObject` | ✓（属性可注入） |
| Intersection + strict 冲突 | 收集兄弟段键补成 `z.unknown().optional()` | ✓ 上游已解 |
| 无 `.default()` / `.transform()` | 产物里从不出现 | ✓ `intersection.mergeValues` 的 throw 风险天然规避 |

### 6.2 四个缺口（对我们都是硬的）

| # | 缺口 | 证据 | 解 |
|---|---|---|---|
| 1 | `u64/i64/u128/…` 直接 `Err(bigint_forbidden)`，且不认 `specta_typescript::Number` opaque | `primitives.rs:1078-1080` | 自己的 `Format` 把 `u64/i64` 重映射为 `specta_zod::define("z.number()")`（原文直出，`z.infer` = `number`） |
| 2 | `serde_json::Value` 出成 serde_json 自己的 enum union | 实测产物：`z.union([z.literal("Null"), z.object({Array: ...}), …])`，还引用未定义的 `ValueSchema` | Format 把该 NamedDataType 换成 `specta_zod::Unknown` → `z.unknown()`（specta-zod 无 `semantic::Configuration` 概念） |
| 3 | **普通 struct 无法 strict**：`struct_dt` 无 strict 参数，marker 只被 `enum_variant_dt` 读 | `primitives.rs:1520` vs `:1762` | fork 补丁 |
| 4 | 产物总是附带 `export type X = z.infer<…>`，无可配开关 | `primitives.rs::export` | fork 删除 |

---

## 7. vendor 补丁清单（5 处，都在固定函数里）

| # | 改动 | 位置 | 规模 | 性质 |
|---|---|---|---|---|
| 1 | 普通 struct 支持 strict（读容器 `specta:strict_object`，把 `z.object` 换成 `z.strictObject`） | `primitives.rs:1520` | ~10 行 | 功能 |
| 2 | 删掉 `export type` 与相位伴生别名输出 | `primitives.rs::export` | ~10 行 | 功能 |
| 3 | 具名/递归引用改用 getter 属性，替换 `z.lazy` | `primitives.rs:2114-2117` | ~15 行 | **必做** |
| 4 | （可选）Intersection 兄弟键补丁从 enum 路径扩到 struct | `primitives.rs:1826` | ~20 行 | 健壮性 |
| 5 | 递归产物类型注解兜底（若 getter 在更深自引用上仍触发 TS7022） | 同上 | 视情况 | 兜底 |

**补丁 3 的实测依据**（复现 `TreeEntry ↔ DirNode` 真实产物形态）：

| 写法 | tsc 结果 |
|---|---|
| `z.lazy(() => X)` 无注解（上游现状） | ✗ `TS7022: 'DirNodeSchema' implicitly has type 'any' because it does not have a type annotation and is referenced directly or indirectly in its own initializer`，连带 `TS7024`；`z.infer` 全变 `any` |
| `z.lazy` + `z.ZodType<Shape>` 注解 | ✓ 但要在 Rust 侧再生成一遍形状类型，等于重写 TS 渲染器 |
| **getter 属性 `get children() { return z.array(TreeEntrySchema); }`** | **✓ Equal 全通过，无注解无报错** |

本仓出线面确实有递归：`TreeEntry ↔ DirNode`（`bindings.ts:1410 / 1022`，explore 目录树）。
非递归的 `z.lazy(() => LeafSchema)` 不受影响。

**vendor 形态**：`crates/tools/specta-zod-patched`（workspace 内置，改动点加 grep 标记）；
同时向上游提 PR（补丁 1、3 对所有人有用）。上游若合入，删 vendor 改回依赖。

### 7.1 生成期硬约束

```
MUST NOT  产物出现 .default() / .transform() / .catch() / .pipe()
          理由：z.intersection 的 mergeValues 在「同一 key 两侧成功但值不同」时
                不是返回 issue，而是 throw "Unmergable intersection"。
                实测触发：两侧同名键各 .default(1)/.default(2)、一侧 .transform。
                soft-report 的 safeParse 抛异常 = 上报链路自己先炸。
          （上游天然满足；这条是防我们自己加）

MUST      strict 写死（z.strictObject），不做 strict/loose 双套 —— 双套就是双套漂移面
MUST      降级哨兵命名化：degraded("原因") 而非裸 z.unknown()
```

### 7.2 生成期自检

产物导出后，对「只 import zod 产物的桩文件」跑一次 `tsc --noEmit`。
一条自检同时看住：strict 化（补丁 1）、getter 化（补丁 3）、删别名（补丁 2）的正确性。

### 7.3 降级哨兵

```ts
// 手写 3 行，非生成物
export const degraded = (reason: string) => z.unknown().meta({ degraded: reason });
```

运行时行为与 `z.unknown()` 完全等价（实测同一输入判定一致），但产物里出现
`degraded("原因")`，可 grep、可 `.meta()` 回读、可统计降级数。
必要性：出线面已有 12 处**合法** `unknown`，裸 `z.unknown()` 作哨兵会永久隐身。

降级触发条件（「不展开」前提下触发面几乎为空）：`z.infer` 与生成类型不 Equal 时降级 +
标记 + 导出期 stderr 列清单。判据就是那条 tsc 断言，不另发明规则。

---

## 8. 集成架构

```
我们自己的 wrapper（~150–200 行，不含测试）
  1. newtype ZodLanguage(specta_zod::Zod)
     impl tauri_specta::LanguageExt                       ~40 行
     （不能给外部类型实现外部 trait，newtype 是本地的 → 合法）
  2. struct ZodFormat; impl specta::Format                ~60 行
       = specta_serde::PhasesFormat
         + u64/i64/u128 → define("z.number()")
         + serde_json::Value → specta_zod::Unknown
  3. 遍历 cfg.commands 生成 args schema                   ~80 行
       specta_zod::primitives::export(&zod, &types, dt)  ← pub API，逐参数渲染
       跳过 Channel<T>（is_channel_type 判据可从 tauri-specta 照抄 5 行）
  4. 产物头（import zod、header、DEGRADED 标记）

export_bindings() 保持单 builder 两次 export（Builder::export 是 &self）：
  builder()
    .export(Typescript::default(), ".../bindings.ts")
    .export(ZodLanguage::default(), ".../bindings.zod.ts")
```

参数面生成的键名与 invoke payload 逐字一致（`name.to_lower_camel_case()`，见 `js_ts.rs:171-188`）。

### 8.1 两个必须显式处理的点

1. **`Channel<T>` 参数**：不可 JSON 表达，跳过校验但**必须原样留在 payload 里**（`onEvent` 键）。
   「跳过校验 ≠ 剔出对象」。
2. **领域不变量不进 schema**：`max_tokens >= 4096` 不写进 `.refine()`（那是第二次实现同一条规则，
   撞「保存期校验单点在 Rust」）。替代：`builder.constant("MIN_OUTPUT_TOKENS", ...)`
   （`builder.rs:254`；产物出 `export const MIN_OUTPUT_TOKENS = 4096 as const;`，见 `js_ts.rs:606-623`），
   把 `provider-panel.tsx:48` 的手抄字面量收成一处。

---

## 9. 关键实测数据

### 9.1 类型面等价性（tsc Equal 断言，本仓 tsconfig）

| 形态 | specta-typescript | `z.infer` | Equal |
|---|---|---|---|
| `#[serde(default)] Option<T>` | `{ a?: string \| null }` | `{ a?: string \| null \| undefined }` | ✓（EOPT 关闭时可互赋） |
| 必填 `Option<T>` | `{ a: string \| null }` | 同 | ✓ |
| `f64` | `number \| null` | `z.number().nullable()` → 同 | ✓ |
| `i32/u32` | `number` | `z.int().min().max()` → `number` | ✓ |
| literal 枚举 | `'raw' \| 'done'` | `z.literal` union → 同 | ✓ |
| `Intersection` | `A & B` | `.and()` → `A & B` | ✓ |
| `serde_json::Value` | `unknown` | `z.unknown()` → `unknown` | ✓ |
| `char` | `string` | `z.string().refine(...)` → `string` | ✓ |
| **递归** | 具名引用，精确 | **`z.lazy` → `any`**；getter → ✓ | ✗ / ✓ |

负例控制：把 `optional` 写成 `required` 时断言报 `TS2344`，说明断言有鉴别力。

### 9.2 递归的类型塌陷 + `z.intersection` 的行为

```
$ZodIntersection（zod/v4/core/schemas.js:1428-1546，源码注释逐字）：
  "A key rejection is reported only when BOTH sides reject it,
   so a key owned by one branch survives the other's key schema."
```

「展开 vs 不展开」（`AgentEvent = head & union`）：

| 输入 | 不展开（`z.intersection`） | 展开（head merge 进每分支） |
|---|---|---|
| 正常 | PASS | PASS |
| 多一个 `extra` | `unrecognized_keys@(root) keys=extra` | 同 |
| 多两个键 | `keys=extra,zz` | 同 |
| 缺 `rawJson` | `invalid_type@rawJson` | 同 |
| 两边都错 | `@seq` + `@eventType` + `@rawJson` | 同 |
| 性能 | 0.46 µs/次 | 0.16 µs/次（差 3×，相对 IPC 的 ms 是噪声） |

⇒ 报错面一致、性能无实质差异，而**展开会破坏类型等价**（交叉 vs 分配后的扁平 union）。
结论：**不展开**。

### 9.3 strict 兼容性

```
z.strictObject + z.lazy：合法数据通过，深层多余键被拒 ✓
z.strictObject + z.intersection：两侧各判 strict，只报两侧都拒绝的键 ✓
```

### 9.4 两者同样「谎」的地方（谁也不比谁准）

| 谎 | 说明 |
|---|---|
| `u64/i64` | 都出 `number`；> 2^53 静默失真（JSON 本身也表达不了） |
| `NaN`/`Infinity` | serde 写 `null`；两侧都出 `number \| null`，都不排除 NaN |
| 「无额外键」 | 类型层面都不表达（TS 结构类型特性）；对象字面量 excess check 是假象 |
| `undefined` | JSON 里不存在；`?:`（EOPT 关）与 `.optional()` 都允许显式 `undefined`（实测 `{a: undefined}` 通过） |

### 9.5 运行期才是 zod 的不可替代价值

| 输入 | TS 类型 | zod schema |
|---|---|---|
| `3.5` 给 i32 字段 | 接受 | 拒绝 |
| `300` 给 u8 字段 | 接受 | 拒绝 |
| 多余键 | 接受 | 拒绝 |
| `'Raw'` 给 `literal('raw')` | 接受 | 拒绝 |

静态类型零运行期成本 = 零运行期约束。两者互补，不是替代关系。

### 9.6 性能（不构成否决理由）

```
31 个嵌套节点   safeParse 平均 0.28 ms
85 个节点       0.16 ms
341 个节点      0.63 ms
```

千级节点在 ms 以内；全量结果侧校验可行。超大响应（代码量统计整树、db inspector 大页）仍建议体积上限或采样。

---

## 10. 已否决方案（存档，附否决理由）

| 方案 | 否决理由 |
|---|---|
| B1：自写 JSON Schema 导出器 + 前端 `z.fromJSONSchema` | 用户决定前端不依赖 JSON Schema；且 `allOf + additionalProperties:false` 在标准语义下会互判对方字段多余（AJV 实测合法数据被拒），要正确必须分配律展开，而展开破坏类型等价断言 |
| 自写 zod 渲染器（400–600 行） | `specta-zod` 已存在且覆盖 10 个 DataType 变体 + serde 相位 + 保留字 + 整数范围 + intersection 补丁；只需 ~150–200 行 wrapper |
| 从 bindings.ts 反推（ts-to-zod 风格） | 产物推产物，双重失真；且要写 TS AST 脚本 |
| 分配律展开 Intersection | 见上 |
| `z.json()` 作 `unknown` 过渡档 | 慢 100–1000×（§4.4 实测） |
| 降级哨兵用裸 `z.unknown()` | 与 12 处合法 unknown 同形，grep 无法区分，降级会永久隐身 |
| strict / loose 双套产物 | 双套就是双套漂移面 |
| 「修 bindings.ts 的 f64 谎」 | **伪问题已撤销**：`specta-typescript` 的 `f64` 本来就出 `number \| null`（`primitives.rs:1508` 逐字，`CoverageThresholds.lines` 实证），与 zod 侧一致 |
| `bindings.ts` 转发 zod 类型 | 见 §5 三条 |
| Event 信封改成显式 `kind` 字段（消掉 flatten） | 是线格式变更（旧客户端读新块会整条解码失败），属另一个 change 的量级 |

---

## 11. 已定决策

| # | 决策 |
|---|---|
| 1 | 目标：观测漂移 + 防脏数据，二者都要 ⇒ 按方向分级（§3） |
| 2 | 产物：只用 TS + zod 两份；**不做 JSON Schema** |
| 3 | 实现：vendor `specta-zod 0.0.4` + 5 处补丁 + ~150–200 行 wrapper |
| 4 | strict 写死（`z.strictObject`），不做双套 |
| 5 | `Intersection` 不展开（zod 路径） |
| 6 | zod 产物**不产出 `export type`**；`bindings.ts` 保持唯一类型源 |
| 7 | 降级哨兵命名化 `degraded("原因")` |
| 8 | 版本：specta rc.26 / specta-typescript 0.0.13 / tauri-specta rc.26 / specta-zod 0.0.4 |
| 9 | 继承上游的 `serde(default)` 相位规则；参数面 `.nullable()` 而非 `.optional()`（Tauri 要求参数键全在） |

---

## 12. 未定项

| # | 问题 | 倾向 |
|---|---|---|
| 1 | 结果面 prod 全量上报还是采样 | 全量上报 + 按 `(cmd, jsonPointerPath)` 去重聚合（否则刷屏）；性能已证明可行 |
| 2 | DEV 是否把结果面升 strict | 升（共用同一 schema，只换失败分支）；代价是不跑 dev 就看不见 |
| 3 | 补丁 4（struct 内 Intersection 兄弟键）是否现在就做 | 出线面只有 `AgentEvent` 一处 flatten 且在 enum 路径，可延后 |
| 4 | vendor 跟版策略 | 先 vendor + 提上游 PR；合入后删 vendor |
| 5 | `unknown` 白洞（12 处）是否契约化 | 这是「防脏数据」能否成立的分水岭（§13）；建议独立 change |

---

## 13. 「防脏数据」的真实缺口（必须说清）

soft-report 不拦数据 ⇒ 脏数据仍流进 UI ⇒ **现有手写降级渲染仍是最后一道防线**。
而手写降级的大头正好落在 `unknown` 白洞上（§2 清单），那些字段的 zod 校验力为 0。

```
现在：  Rust parser ──serde_json::Value──▶ payload: unknown ──手写收窄──▶ renderer
更彻底：Rust parser ──typed enum ArtifactPayload { MarkdownDoc(..), … }──▶
        payload: ArtifactPayload ──specta──▶ 前端 tagged union，tsc 直接收窄
```

这条路把 `attachments.ts` / `renderers/*` 的手写 `typeof` 收窄整体退役，也让 zod 的收益面变大
（不再有 `unknown` 白洞）。代价：改动 core/workflow 的 artifacts 契约（版本号 / 兼容语义要重述）。

⇒ 分层顺序：**先 payload 契约化（结构性收益）→ zod 运行期校验收益才完整**。本轮只做后者，
白洞收益记为已知缺口。

---

## 14. 成本与收益（终版）

```
收益
  ✓ 参数面 strict：本地 0ms 拦截，省一次 IPC 往返，抓 null/undefined 混用
  ✓ 结果面 soft-report：漂移可见 + UI 不崩
  ✓ Channel 信封逐条上报：漂移热图
  ✓ 一条 tsc Equal 断言锁定两份产物语义一致（负例已验证有鉴别力）
  ✓ MIN_OUTPUT_TOKENS 类常量单点（builder.constant）

成本
  ~ vendor specta-zod 0.0.4 + 5 处补丁（~55 行改动）+ 跟版
  ~ wrapper ~150–200 行 + 测试
  ~ 生成期两条硬约束 + 一条 tsc 自检
  ~ 版本升级：三件套 rc.25 → rc.26（本身就是 spec 要求的 lockstep）

风险（可控）
  ! z.intersection mergeValues throw —— 由「禁 .default()/.transform()」根治（上游天然满足）
  ! 上游是 0.0.x partial、每 rc 都改 —— 补丁都在固定函数里，跟版成本可估
  ! 全量 strict 会让「后端先行、前端后跟」窗口期报 unrecognized_keys —— soft-report 下只是噪音
```

---

## 15. spec 侧影响

`openspec/specs/desktop-ipc-type-bindings/spec.md` 需两处措辞调整：

1. 「生成物」从单数 `src/types/generated/bindings.ts` 改为「生成目录 `src/types/generated/`」
2. `bindings:check` 的 `git diff --exit-code` 守卫覆盖面随之扩大（命令不变）

模块契约新增两行：

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `src-tauri/crates/tools/specta-zod-patched` | vendored exporter | 5 处补丁带 grep 标记；跟版记录在 crate README |
| `src/lib/ipc.ts` | args/result 校验装配 | ~20 行，无手写类型镜像；失败面三分（§3） |
