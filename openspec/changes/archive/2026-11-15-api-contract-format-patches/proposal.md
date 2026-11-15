# 提案: api-contract-format-patches

> **变更**: api-contract-format-patches
> **日期**: 2026-11-14
> **状态**: draft
> **来源**: `openspec/changes/api-contract-format-patches/explore.md`（requirement 类型；由「给前端模块试写契约草案」的演练暴露的三个格式缺口）
> **性质**: 契约**格式面**的三个补丁（可空 / 属性键 / 降级词表），判据面与两侧渲染面各收口到单点。零新增判据种类、零新增 IPC、零契约资产改动；验收判据全部落在既有契约语料与测试语料上，**零入口依赖**。

---

## 问题

### 现状链路（读过的真实代码）

```
<FQN>.md（手写真相）
   │ api::contract::parse → api::schema::build_schema
   ▼
装配后的 schema（内存）── api::contract::verify（闸一七族 + 落点闸）
   │                    └─ api::schema::verify_defs（$defs 结构规则）
   ├── api::schema_type（extract/mod.rs，语言无关逻辑类型：判据面）
   │      └─ nullable_inner：识别 nullable 三分支 → ApiType::Optional
   ├── api::emit::ts::value_type / interface_block（渲染面）
   ├── api::emit::rust::value_type / struct_block（渲染面）
   └── api::mod::index_entries → INDEX.toml 的 degraded 计数
```

三处真实病灶，共同根因一句话：**同一份 schema 事实，判据面与渲染面读法不同**。

### 缺口一 · 可选与可空：一个事实、两套读法（P1）

| 事实 | 现状 | 落点 |
|---|---|---|
| 契约侧 nullable | **已支持三分支**：`"type": "null"` / `"type": ["string","null"]` / `anyOf`-`oneOf` 含 null 分支 → `ApiType::Optional` | `api/extract/mod.rs` 的 `nullable_inner`（`schema_type` 内部） |
| TS 渲染 | **不支持**：`value_type` 只 `schema.get("type").and_then(as_str)`，数组 / 对象形态落 `unknown` | `api/emit/ts.rs` `value_type` |
| Rust 渲染 | **不支持**：同上 → `FALLBACK_VALUE_TYPE`（`serde_json::Value`） | `api/emit/rust.rs` `value_type` |
| 非 `required` 字段 · TS | 一律渲染 `{field}: T \| null`——**丢了 `?`**，把「可选缺失」与「显式 null」压成一态 | `api/emit/ts.rs` `interface_block` |
| 非 `required` 字段 · Rust | `Option<T>`（正确） | `api/emit/rust.rs` `struct_block` |

后果链：契约写 `{"type": ["string","null"]}` → **签名判据绿**（`schema_type` 认得）→ **生成物退化**（`unknown` / `serde_json::Value`）→ 实现侧类型无法绑定。这是「判据绿而产物错」的最坏组合，比两侧都不支持更危险：作者会以为契约已经生效。

叠加还有两个未被 explore 点名的机械后果（本次一并收口）：

1. **`| null` 会重复**：后端在 `value_type` 侧补 nullable 后，`interface_block` 若继续硬拼 `| null`，非 `required` 且 nullable 的字段会得到 `T | null | null`。
2. **Rust 会出 `Option<Option<T>>`**：同上，`struct_block` 对非 `required` 再包一层 `Option<>` 即产生嵌套 `Option`——退化产物，MUST NOT 出现。

### 缺口二 · 属性键：首层已被 ⑤ 族覆盖，但缺 `required` 侧与出路文案（P2）

explore 的原判（「属性键不在射程」）**与代码不符**，实测复核：`contract::verify` 的闸一已含 ⑤ 族「`$defs` 条目的 `properties` 键」（`property_names` + `is_identifier`），故 `'data-testid'` 这类首层属性键**今天已经被装载期拦住**。真正剩下的两处空缺是：

| 空缺 | 现状 | 后果 |
|---|---|---|
| `required` 元素集与 `properties` 键集不闭合 | `required` 只被 `required_of` 读成字符串表；既不判形态也不判是否真在 `properties` 里 | 写成 `required: ["nmae"]`（笔误）时产物静默把 `name` 渲染为**可选**——又一条「判据绿而产物错」 |
| 错误文案没有可执行出路 | `identifier_error` 给的是通用句（「MUST NOT 含空格 / 连字符 / 点」），不点名 pointer，也不说 `data-testid` 一类键该怎么办 | 作者知道「键非法」却不知道「那这个键该放哪」；下一步动作是猜 |

另有两条**必须明确否决**的诱惑（否则下一轮会有人实现它们）：给 TS 侧加引号形态（`'data-testid': string`）、给契约加 `x-key` 重命名映射。两者都会让「同一 schema 两侧产物形态不同」——这正是既有 D47 纪律要消灭的不对称。

### 缺口三 · 降级：三条来源、一条进账（P3）

| 来源 | 表达 | 进 `INDEX.toml` 的 `degraded` 计数？ |
|---|---|---|
| bootstrap 草稿 | `x-common.note = "待剪枝：…"` | ✗（且**不该**进——草稿未剪枝 ≠ 结构性不可表达） |
| 跨语言规则（`common.json` 的 `degraded`） | 规则级标记（`"degraded": "typescript"`） | ✗（不是契约资产） |
| 手工「类型不可达」 | `x-common.note`（本仓演练即此形态） | ✗ |

也就是说：**「这个契约有债」这件事，写 `note` 看不见、写 `degraded` 才进账**，但没有任何规则要求写哪个、写什么值。同时实测计数口径比 explore 假设的更窄——`index_entries` 只看 `$defs` **条目自身**的 `x-common.degraded`：

```rust
// api/mod.rs index_entries
defs.values().filter(|definition| definition.get("x-common")…
    .get("degraded").is_some()).count()
```

而真实降级落点是**属性位**（演练草案的 `session` / `sessions.items` / `send.input` 三处都在 `properties` 值上）。故 explore 的「计数逻辑零改动」与它自己的验收「`degraded = 3`」互相矛盾——本提案按后者落：计数口径 SHALL 从「条目数」改为「**标记出现次数**（全深度）」。零影响论证：现网唯一契约（`App.Backend.Infra.ArchiRuntime`）零 `degraded` 标记，现网计数仍为 `0`。

现有测试还会被这条收紧打红一处：`mod_test.rs::render_all的index_degraded派生` 用的是 `"x-common": {"degraded": true}`（布尔）——词表落地后该 fixture 须改为词表内的字符串值。

### explore 未决项的归属（不在本变更）

| explore 条目 | 处置 |
|---|---|
| D.4 两处结构性卡点（hooks 无元素 / 跨域 `$ref` 不可表达） | 不在本变更：属「改判据或改粒度」，与入口/barrel 一并裁决（变更 B/C） |
| D.5 三种收敛 | 本变更只做第 1、2 种共同依赖的**格式前提**：契约能写得出、产物不错；不碰 `apiRefMissing` 的「直接子元素」规则 |
| D.1 演练草案本身 | 只作**测试语料**，MUST NOT 落 `.dev-team/architecture/api/**`（否则本变更变成「给前端写契约」，验收面混入跨域 ref 判据） |
| 前端 ceiling（只能写「纯数据 + 回调」型类型） | 记入风险，不改 |

---

## 提案

### 一、P1 · 可选与可空：判据面与渲染面共用一个判定

| # | 改动 | 落点 |
|---|---|---|
| 1.1 | 抽出 nullable **形状识别单点** `nullable_breakdown(schema) -> Option<NullableBreakdown>`（`non_null: Option<&Value>` + `explicit_null: bool`），`schema_type` 与两 `value_type` 共用；`value_type` 在 `$ref` / 映射规则之后、常规 `type` 分派**之前**处理 nullable（与 `schema_type` 同序） | `api/extract/mod.rs` + `emit/{ts,rust}::value_type` 各一处前置分支 |
| 1.2 | TS 侧：`| null` 的**唯一产出点是 `value_type`**；`interface_block` 对非 `required` 字段只出 `{field}?:`，MUST NOT 再拼 `| null`（两事实各自只落一个符号） | `emit/ts.rs::interface_block` |
| 1.3 | 叠加去重：TS `T | null` 至多一次；Rust `Option<T>` 至多一层（`Option<Option<` 零产出）。Rust 侧由 `struct_block` 经单点幂等包装（`optional_of`）实现 | `emit/{ts,rust}` 字段写出 |
| 1.4 | TS 数组元素位为可空时 MUST 加括号：`(T \| null)[]` | `emit/ts.rs::value_type` 的 `array` 分支 |
| 1.5 | **纯 `null` 形态（`non_null` 缺省）单独定档**：TS `null`、Rust 保持既有兜底 `serde_json::Value`（不新增 `Option<>`、不造 `()`）；判据面对该档仍出 `Optional(Unknown("null"))`（既有行为逐值不变）。本档 MUST NOT 折进 1.1～1.4 的三面等价式（它只会让等价式两头必假） | `emit/{ts,rust}::value_type` 的 nullable 前置分支 |
| 1.6 | **可选轴不进判据面**：`schema_type` 只吃 schema 片段、无 `required` 语境，故同一片段在必填位 / 可选位判出**同值**逻辑类型；可选性只在两侧渲染面落符号（`?` / `Option<T>`），由 `required_of` 单点读。`schema_type` 的签名比对结果因此**逐值不变**，MUST NOT 为可选轴改它的入参 | 零代码改动（口径落 spec） |

保持既有语义不动：`nullable_breakdown` 沿用 `nullable_inner` 现行「取首个非空分支」的行为，故判据面的签名比对结果**逐值不变**（多分支 union 的完整表达不在本变更，见风险）。

### 二、P2 · 属性键：补 `required` 闭合性，把出路写进文案

| # | 改动 | 落点 |
|---|---|---|
| 2.1 | 装载期同趟续判一条：`required` 元素集 MUST ⊆ 同条目 `properties` 键集（悬垂即契约错误）。**不新增族别数**——属性键已由 ⑤ 族判标识符，闭合性成立即标识符性自动传递 | `api/contract.rs::verify` |
| 2.2 | 属性键族与 `required` 族的错误文案 MUST 带 JSON pointer（`$defs.<name>.properties.<key>` / `$defs.<name>.required[<i>]`）并给可执行出路：「属性键必须是标识符；`data-testid` 一类键不进契约（人类可读措辞写散文区）」 | `api/contract.rs`（`identifier_error` 增族别化出路句） |
| 2.3 | **明确否决**（写进 spec 的否决项）：TS 侧加引号形态（`'data-testid': string`）、契约加 `x-key` 重命名映射。二者都让同一 schema 两侧产物形态不同；且 `x-key` 是第二套命名体系 | 零代码，纪律落 `desktop-archi-api` |

### 三、P3 · 降级词表与真实债表

| # | 改动 | 落点 |
|---|---|---|
| 3.1 | 定义**封闭降级词表**（首期四值，新增须改 spec）：`foreignElement`（跨域 / 非直接子元素引用）/ `noElement`（目标无模型元素，如 hooks / lib / types）/ `unrepresentable`（目标语言类型系统表达不了，如 JSX）/ `crossLanguage`（规则表降级） | `api/schema.rs` 常量 `DEGRADED_KINDS`（**MUST NOT** 进 `common.json`：该资产语义封闭为映射规则，且其摘要进全部生成物头注释） |
| 3.2 | 单点扫描 `degraded_marks(schema) -> Vec<(pointer, value)>`（**全深度**，`$defs` 与 `properties` 均在内） | `api/schema.rs` |
| 3.3 | `verify_defs` 同趟校验：`x-common.degraded` 的值 MUST ∈ 词表（非字符串亦拒），错误点名 pointer 并列四值 | `api/schema.rs::verify_defs` |
| 3.4 | 计数口径改为「标记出现次数」，实现走 3.2 同一单点（零第二套遍历）；`IndexEntry.degraded` 的 doc 语义同步 | `api/mod.rs::index_entries` |
| 3.5 | 纪律：凡「引用不可达 / 不可表达」的类型 MUST 写 `x-common.degraded`；`note` 只作自由注记（人读），**不承担判据、不计入计数**；bootstrap 草稿的 `待剪枝` note 因此零计入 | 纪律落 `desktop-archi-api` spec |
| 3.6 | **显式留白**：把「必须写 `degraded`」机械强制的判据**不在本期**——它需要跨元素可达性知识（属 `check` 面），本变更只给词表、校验与计数 | 记入 spec 的留白条款 |

### 四、版本与生成物

`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 的现值 +1 patch（当前 `0.6.42` → `0.6.43`）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。本变更零契约改动 ⇒ 重跑 `api:export` 后 `.dev-team/architecture/api/**` SHALL 零 diff。

---

## 能力

| 能力 | 分类 | 变更内容 |
|---|---|---|
| `desktop-archi-api` | **修改** | ① MODIFIED「关键字集、类型映射与错误面」：`x-common` 行的 `degraded` 改为受封闭词表校验、映射表「非 `required` 字段」行由 `T \| null` 改为 `{field}?: T`，并新增「可选与可空是两个独立事实、同一判定函数、叠加去重」条款；② MODIFIED「语言无关纪律与语言特定落点」：把「当前两类降级」改为指向封闭四值词表，场景 `两类降级可枚举` 改写为 `降级词表封闭且可枚举`；③ ADDED「属性键与 required 的自洽闸」；④ ADDED「降级词表与真实债表」 |
| （新增能力） | 无 | 三个补丁都落既有能力，不新增能力 |

**不涉及**：`desktop-archi-api-check`（零新判据、零新 finding kind、`REPORT_VERSION` 不动）、`desktop-archi-contract-panel`（`INDEX.toml` 字段集不变，只有 `degraded` 的语义精度变化）、`workspace-layout-resolution`（零新路径），以及 `desktop-archi-api` 的其余八个 requirement（逐字不动）。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` | 新增 `NullableBreakdown` + `nullable_breakdown`（`nullable_inner` 改由它产出，`schema_type` 对外结果逐值不变） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` | `value_type` 前置 nullable 分支（`\| null` 唯一产出点、数组元素位加括号、纯 `null` 档出 `null`）；`interface_block` 非 `required` 出 `?`、零二次 `\| null` |
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` | `value_type` 前置 nullable 分支（含非空分支 → `Option<T>`；纯 `null` 档保持既有兜底）；`struct_block` 经 `optional_of` 幂等包装（`Option<Option<` 零产出） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` | `verify` 同趟续判 `required ⊆ properties`；`identifier_error` 加族别化出路句与 pointer |
| `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` | `DEGRADED_KINDS` 常量 + `degraded_marks` 单点扫描 + `verify_defs` 取值校验；`IndexEntry.degraded` 语义注记 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | `index_entries` 的 `degraded` 改经 `schema::degraded_marks`（零第二套遍历） |
| `packages/desktop/package.json` | `version` +1 patch |

### 测试文件

| 文件 | 新增 / 改写 |
|---|---|
| `.../api/extract/mod_test.rs` | 9 组样例按**三轴分列**对拍（可空轴六组：两元素数组 / 三元素数组 / `anyOf`-null / `oneOf`-null / 两者叠加 / 嵌套数组元素 nullable；可选轴两组：`required` 内非 null 字段 / 非 `required` 非 null 字段；纯 `null` 兜底一组：`type: "null"`），逐组记 `schema_type` 与两侧渲染文本；`schema_type` 结果与改动前逐值对拍（含可选轴两组**同值**与纯 `null` 组 `Optional(Unknown("null"))` 不变） |
| `.../api/emit/ts_test.rs` | 非 `required` → `{field}?: T`；显式 null → `T \| null` 至多一次；数组元素可空 → `(T \| null)[]`；演练草案 fixture（D.1 改写成词表形态）渲染文本零 `unknown` 泄漏 |
| `.../api/emit/rust_test.rs` | 非 `required` → `Option<T>`；nullable → `Option<T>`；叠加后 `Option<Option<` 零命中；同上 fixture 的字段名全为标识符 |
| `.../api/contract_test.rs` | 首层属性键 `data-testid` → 装载期 `ApiError::Contract` 且文案含 `properties.data-testid` pointer 与出路句；`required` 悬垂（`required: ["nmae"]`）→ 拒收并点名 `required[0]`；`required` 与 `properties` 对齐 → 通过 |
| `.../api/schema_test.rs` | 词表外值（`"typo"`）与非字符串值（`true`）→ 拒收，文案列四值；四值逐个通过；`note` 不计入 |
| `.../api/mod_test.rs` | `INDEX` 的 `degraded` = 全深度标记出现次数（嵌套属性位计入）；演练 fixture 三处标记 ⇒ `degraded = 3`；既有 `render_all的index_degraded派生` 的布尔 fixture 改为词表字符串 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/api/**` | 本变更零契约改动；`api:export` 重跑应零 diff（生成物勿手改） |
| `AGENT.md` / `CLAUDE.md` | 「仓库根文档的契约纪律节」的机械判据是**该节恰 5 条要点**；加第六条即违反既有 requirement，留给后续变更一并重裁 |
| `crates/core/archi/src/api/check.rs` + `openspec/specs/desktop-archi-api-check/**` | 零新判据种类、零新 finding kind；「必须写 degraded」的机械强制是显式留白 |
| `crates/core/archi/src/api/common.json` | 词表 MUST NOT 进映射规则资产（其摘要进全部生成物头注释，改动即批量重生成） |
| `crates/tools/archi-api-gen/**` | CLI 三子命令（`generate` / `--check` / `bootstrap`）形态不变 |
| `packages/desktop/src/**` 前端与 `src/types/generated/**` | 无 IPC 面变化，`bindings:export` 应零 diff |
| `openspec/specs/desktop-archi-api/spec.md` 其余八个 requirement | 逐字不动 |

---

## 验收标准

1. **AC-1 判定单点一致（三轴分列，压不成一条等价式）**：9 组样例逐组对拍，按三条轴各列期望——MUST NOT 用「`ApiType::Optional` ⟺ TS 含 `| null` ⟺ Rust 含 `Option<`」一条式覆盖全部九组（可选轴与纯 `null` 组在该式下必假）：
   - **(a) 可空轴 · 显式 null 且有非空分支（六组）**：`["string","null"]` 两元素 / `["string","integer","null"]` 三元素 / `anyOf` 含 null / `oneOf` 含 null / 非 `required` 且显式 null 叠加 / 数组元素位 nullable。三面同真同假：`schema_type` = `ApiType::Optional` ⟺ TS 文本含 `| null` ⟺ Rust 文本含 `Option<`（数组组在元素位对拍）。
   - **(b) 可选轴 · `required` 语境（两组）**：`required` 内非 null 字段 / 非 `required` 非 null 字段。`schema_type` 对二者**同值**（`OwnedString`——`schema_type` 无 `required` 语境，本轴 MUST NOT 折算成判据面差异）；TS 分别为 `a: string;` / `b?: string;`（`a` 零 `?`、`b` 零 `| null`）；Rust 分别为 `pub a: String,` / `pub b: Option<String>,`。本轴 MUST NOT 拿「Rust 含 `Option<`」当可空判据。
   - **(c) 纯 `null` 兜底（一组）**：`{"type": "null"}`。`schema_type` = `Optional(Unknown("null"))`（既有行为逐值不变）；TS 渲染 `null`（MUST NOT 是 `unknown`）；Rust 渲染 `serde_json::Value`（既有兜底，按 R-4 是**本组显式期望值**，MUST NOT 记为泄漏，MUST NOT 出 `()`）。本组 MUST NOT 参与 (a) 的等价式。
   - **泄漏面**：TS 侧九组 `unknown` 零命中；Rust 侧除 (c) 一组外 `serde_json::Value` 零命中。
2. **AC-2 TS 产物正确**：非 `required` 字段渲染 `{field}?: T`；显式 null（含非空分支）渲染 `| null` 且每字段至多一次；纯 `null` 组渲染 `null`；数组元素位可空渲染 `(T | null)[]`；含 nullable 的金样文本 `unknown` 零命中。
3. **AC-3 Rust 产物正确**：非 `required` → `Option<T>`；显式 null（含非空分支）→ `Option<T>`；纯 `null` 组保持既有兜底 `serde_json::Value`（形状不变）；可空与可选叠加后 `Option<Option<` 全仓测试文本零命中。
4. **AC-4 属性键闸可执行**：`data-testid` 首层属性键 ⇒ 装载期报错，文案含 `$defs.<name>.properties.data-testid` 与「`data-testid` 一类键不进契约」出路句；`required` 悬垂 ⇒ 报错含 `required[<i>]`；生成物面零引号键（`grep -c "^  '"` 于 `.i.ts` 零命中）。
5. **AC-5 词表封闭**：词表外值 / 非字符串值 ⇒ 契约错误并列四值；`note` 不计入；`INDEX.toml` 的 `degraded` 等于全深度标记出现次数；演练 fixture 三处 ⇒ `3`。
6. **AC-6 零回归**：`App.Backend.Infra.ArchiRuntime` 的四类生成物与 `INDEX.toml` 逐字不变（`pnpm api:check` 零差异）；`App.Backend.Infra.ArchiRuntime.i.rs` 不受 Rust 侧 nullable 分支影响。

> **不列为 AC 的流程项**：门禁四段（`server:check` / `bindings:export` / `api:check` / `client:check`）与 `package.json` 的版本纪律各有现成落点——`desktop-archi-api` 既有 requirement「生成物入仓与零 diff 门挂载」的 `check` 链条款与**版本纪律**条款，本 delta 不改该 requirement ⇒ 不在验收标准里另设无 spec 落点的 AC；实现 / 归档侧仍照该既有 requirement 执行（`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 的 crate 版本行不动）。

**AC ↔ spec 落点**：

| AC | spec requirement / scenario |
|---|---|
| AC-1 (a) / AC-2（`\| null`、括号） | `desktop-archi-api` MODIFIED「关键字集、类型映射与错误面」：Scenario「显式 null 六组上判据面与两渲染面同一判定」「数组元素位可空加括号」+ 映射表「显式 null…且有非空分支」行 |
| AC-1 (b) / AC-3（可选轴） | 同 requirement：Scenario「可选轴不进判据面且各落自己的符号」「可选与显式 null 各落自己的符号且叠加去重」+ 映射表「非 `required` 字段」行 |
| AC-1 (c)（纯 `null` 档） | 同 requirement：Scenario「纯 null 组按显式兜底且不参与可空对拍」+ 映射表「纯 `null` 形态」行 |
| AC-4 | ADDED「属性键与 required 的自洽闸」三条 Scenario |
| AC-5 | ADDED「降级词表与真实债表」三条 Scenario + MODIFIED「语言无关纪律与语言特定落点」Scenario「降级词表封闭且可枚举」 |
| AC-6 | ADDED「属性键与 required 的自洽闸」Scenario「自洽契约零误伤」+ ADDED「降级词表与真实债表」的**零影响**条款 |

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **`INDEX.toml` 的 `degraded` 语义从「条目数」变「标记数」**：同一列在两套口径下含义不同，读者 / 下游可能误读；且 explore 明写「计数逻辑零改动」，本提案与之相反 | 提案按代码事实与 explore 自身验收（`= 3`）落；口径写进 `IndexEntry.degraded` 的 doc 与新增 requirement 正文，测试用嵌套 fixture 钉死；现网零标记 ⇒ 现网 diff 为零，可事后回退而不留残迹 |
| R-2 | **词表封闭的连带打红**：现网 `mod_test.rs` 用 `"degraded": true`（布尔），词表落地即红；同理任何旧草稿写 `degraded: true` 的契约会在装载期失败 | 已知且限定在测试语料（现网唯一真契约零 `degraded`）；实现时同步改 fixture，并在 requirement 里写明「非字符串亦拒收」的理由（`true` 不携带任何原因信息，债表不可读） |
| R-3 | **多分支 union 表达不完整**：`nullable_breakdown` 沿用「取首个非空分支」，`anyOf: [string, integer, null]` 仍只表达 `string \| null`；判据面同源所以「两面一致」成立，但契约语义被静默削窄 | 本变更**故意不动**（改它会动判据面的签名比对，风险大于收益）；在 requirement 里写明单点函数沿用既有单分支语义，并列为留白，后续变更单独裁决 |
| R-4 | **纯 `null` 形态的渲染口径是新增决定**：`{"type": "null"}` 现在落 `unknown` / `serde_json::Value`。新口径为 TS `null`、Rust 保留既有兜底类型 | 该形态在现网语料零命中；写进 requirement 的判定条款并由 9 组样例中的**独立一档**钉住（TS `null` / Rust `serde_json::Value` 均是该档显式期望值，不折进可空轴的等价式），避免实现者自行发明（如 Rust 出 `()`） |
| R-5 | **`required ⊆ properties` 新判据可能拦下既有草稿式契约** | bootstrap 草稿不产 `required`（`x-common.note` 骨架），现网零命中；风险面只在人工手写契约，且拦的是笔误 |
| R-6 | **词表可能不够用**：真实降级原因出现第五类时会被拦 | 刻意摩擦：新增须改 spec（与「判据词表封闭」同一条纪律）；错误文案并列四值，作者能立刻判断是「词表该扩」还是「自己写错」 |
| R-7 | **前端契约的能力上限未变**：props 里引用 hooks / 后端 bindings 的类型仍结构性不可表达（explore D.4） | 本变更不承诺解决；P3 的 `noElement` / `foreignElement` 两值正是给这类**已知不可达**留的记录位——债可见，但解不解由后续变更裁决 |
| R-8 | **explore 与代码的两处不一致**（P2 首层属性键已被 ⑤ 族覆盖；P3 计数口径） | 提案已按代码事实收窄 / 改写；实现第一件事是复核这两处，若实测与本提案不符须先回到提案层修正，MUST NOT 直接加第二份判据或第二套计数 |
