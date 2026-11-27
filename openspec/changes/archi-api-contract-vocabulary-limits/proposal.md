# 提案: archi-api-contract-vocabulary-limits

> **变更**: archi-api-contract-vocabulary-limits（即 explore 里的「变更 1 · 类型出生能力」；目录名沿用，方案本体 = explore §30–§32 的收敛结果 + 本篇五处偏离）
> **日期**: 2026-11-26
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（`workflow_type = requirement`；§1–§29 是裁定依据，§30–§32 是提案骨架与验收闸）
> **性质**: **生成侧能力一次原子交付**——契约收敛为「形状 + 语义」，属性（derive / 本地命名 / 整数宽度 / serde 形态）下沉为 `schema → 生成物` 的纯函数；同批把封闭枚举的唯一写法收敛为 `oneOf: [{const, description}]`（`enum` 数组退役），并把 `core/archi` 的见证链接上入口（首次编译期核对）。
> **不含**: 判据侧（`$ref` 可解析 ⇒ `Named`、`async` 短形与见证分叉）归独立变更 `archi-api-signature-parity`；TS 值对象（逐值 JSDoc）归 `archi-api-ts-variant-docs`；`dto.rs` 33 个类型的迁移是独立一轮（逐类型原子）；效果面、「未声明面」分母口径、覆盖率列、元组 / 别名词汇、形状判据均不做。

---

## 问题

### 一 · `$defs` 写得下形状，写不出「与手写等价」的类型

四处缺口，全都不是「词汇不够」，而是**属性层没有归属**（契约说不了，生成器也不推）：

| 缺口 | 现状证据 | 后果 |
|---|---|---|
| 文档注释 | `grep description emit/` **零命中**（而 spec 早已承诺 `description` 是「进 `.i.*` 文档注释的语义」） | 一出生就丢承重语义：`dto.rs` 的 10 个 enum / 36 条变体注释、33 个类型的类型级与字段级说明 |
| derive | 有字段 `struct` **零 derive**（`struct_block` 只对零字段形出固定五项 `Debug, Clone, Copy, PartialEq, Eq`） | 不可序列化、不进绑定注册表 ⇒ 与 `dto.rs` 任何一个类型都不等价 |
| 本地字段名 | 字段名 = 契约键原样（camelCase 直进 Rust；关键字键 `in` / `abstract` 直接不可编译） | 与手写代码对不上；关键字位无从表达 |
| 整数宽度 | `{"type":"integer"}` 恒 `i64`（`value_type` 与 `scalar_type` 各写死一处） | 手写侧 13 个带宽度字段的精度被**静默**抹平（`i64` 能编译，无任何红灯） |

### 二 · 封闭枚举今天只有「垃圾产物」一条路

```
{"enum": ["off","warn","strict"]}
   Rust → pub enum GateTier { Off, Warn, Strict }      ← 变体注释零落点（语义全丢）
   TS   → as const 数组 + typeof 派生                   ← 逐值语义零落点

{"oneOf": [{"const":"off","description":"…"}, …]}        ← 想带语义的正确形态
   Rust → value_type({"const":"off"}) 落兜底 → Variant0  ← 垃圾变体名
   TS   → unknown                                        ← 同上
```

两条路各缺一半，而现网 `enum` / `oneOf` **零生产用例**（12 份 schema 的 `$defs` 里只有 1 个零字段 `object`）⇒ 此刻把两条路收敛成一条，迁移成本为零。

### 三 · 见证链在一个 crate 里还是悬空文本

spec 已规定入口编排为 `#[path = "<文件名>"] mod api_gen; pub use api_gen::*;`；`infra/archi` 照做了（那两份见证**真被编译过**），`core/archi` 没接 —— 它那两行 `const _: fn(&str) -> bool = super::is_identifier;` 至今没有任何编译器看过。

### 四 · 一句话结论

**这不是「补词汇」，是把属性从契约里请出去、请进生成器**：契约只承诺「名字 + 签名 + 形状 + 语义」；`x-` 面一条不加，`common.json` 一个字不改。

---

## 提案

### 一 · 总图

```
契约（.schema.json）              生成器（schema → 生成物的纯函数）              生成物
─────────────────────────────────────────────────────────────────────────────────────────
形状 object / oneOf+const  ──►  facet 推导（std 面 + wire 面，跨元素闭包）  ──►  #[derive(…)]
语义 description           ──►  文档注释（类型 / 字段 / 变体）              ──►  ///
名字 properties 键 = wire 名 ──►  本地标识符派生 + serde 形态自动选         ──►  snake_case / r#in / rename_all
精度 minimum / maximum     ──►  严格完整值域 ⇒ u32 / u64 / i32             ──►  u32 / u64 / i64
```

零声明位：**不新增任何 `x-` 键** —— `description` / `oneOf` / `const` / `default` / `minimum` / `maximum` 全是标准关键字。修复点永远在规则与语料，不在 `.i.*`（不可手改）。

### 二 · 决策表

| # | 决策 | 内容 |
|---|---|---|
| D1 | **零新词汇** | `x-` 面仍五项（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`）；属性零声明位 |
| D2 | **facet 表（std 面）** | 见下；`Eq` 递归到字段类型（`$ref` 跟随、环即不推）；`Default` 只对封闭枚举，且只在 `$defs.<T>.default` 在场并命中某 `const` 分支时出（命中分支带 `#[default]`；`default` 缺省 / 值不命中 ⇒ 无 `Default`、不报错） |
| D3 | **wire 面射程** | `Serialize` / `Deserialize` / `specta::Type` **只对有数据位**的条目出（偏离 ①） |
| D4 | **命名策略** | 契约键即 wire 名；本地名是其纯函数；serde 形态先试容器级 `rename_all`（`camelCase` / `lowercase` / `PascalCase` / `snake_case`），全对不上才逐项 `rename`；关键字用原生转义 `r#in` |
| D5 | **整数宽度** | 只认**完整值域**（`0..=4294967295` → `u32`、`0..=18446744073709551615` → `u64`、`-2147483648..=2147483647` → `i32`）；无 bounds / 部分范围一律 `i64`——`maximum: 100` **不**推 `u8`（业务约束 ≠ 本地精度） |
| D6 | **文档注释** | 类型 / 字段 / 变体三处 → `///`；空串与缺省零行；op 的 `x-function.description` **仍丢弃**（既有规定不变） |
| D7 | **封闭枚举唯一形态** | `oneOf: [{const, description}]`；`enum` 数组退役（装载期拒收，出路指向新形态） |
| D8 | **接线射程** | `core/archi` 接线；`core/foundation` **不接**（偏离 ②） |

**facet 表**（`derive` 行内顺序固定为 `Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type`，缺席项略去、在场项保持该相对序 —— 这样「生成物 vs 手写」才能逐字比对）：

| 形态 | std 面 | wire 面 |
|---|---|---|
| 零字段 `object`（名义类型） | `Debug, Clone, Copy, PartialEq, Eq`（**与今天逐字相同**，不扩 `Hash` / `Default`） | 不出（零数据位） |
| `oneOf` 全 `const`（封闭枚举） | 上述 + `Hash` + 条件 `Default`（`$defs.<T>.default` 在场且命中某 `const` 分支 ⇒ 出，命中分支带 `#[default]`；`default` 缺省 ⇒ 无 `Default`） | 出 |
| `object` 有字段（struct） | `Debug, Clone, PartialEq` + 条件 `Eq` | 出（字段位无 `x-function`；函数指针没有 JSON 线形态） |
| 标量 newtype | `Debug, Clone, PartialEq` + 条件 `Eq`（`String` 不 `Copy`） | 出 |
| `oneOf` / `anyOf` 带负载 | `Debug, Clone, PartialEq`（不推 `Eq` / `Copy` / `Hash`） | 不出（线形态未定义） |

### 三 · 与 explore 的五处偏离

| # | explore | 本提案 | 证据 / 理由 |
|---|---|---|---|
| ① | §19「`Serialize` / `Deserialize` / `specta::Type` 恒出」 | **只对「有数据位」的条目出** | 实测 7 个 crate 无 serde 依赖，其中 `crates/infra/archi` 是**已声明元素**且今天就有生成类型（`LocalArchiCheck`，零字段），`crates/core/foundation` 的准入纪律是「外部依赖只保留一个哈希实现」。恒出会让这两个 crate 的生成物编译不过，并把 `serde` / `specta` 带进 `archi-runtime`——而 `desktop-crate-layout` 已登记它「零新外部依赖」，`Foundation.md` 还明写「本模块不会为了『让生成物能编译』多开导出面」。<br>**「能否补 serde 依赖」的答复**：能（两行 `workspace = true` 引用，非新供应链），但**本变更不需要补**——④ 迁移的目标 crate `core/archi` 已同时具备 `serde` 与 `specta`（`dto.rs` 的 33 个类型就是这条面）。规则化为消费方前置：**声明有数据位类型的元素 crate MUST 具备 `serde` + `specta`**（缺依赖的表现是 `cargo build` 失败 = fail-closed，MUST NOT 由渲染期探测依赖）。 |
| ② | §30.4 前置检查：`Foundation` 9 条见证「8 条 ✅ + 1 条缺 `pub use cancel::CancelFlag;`」，补一行即可 | **`core/foundation` 不接线**（F 射程 3 → 2 crate） | `CancelFlag::is_set(&self) -> bool` 的 fn 指针类型是 `fn(&CancelFlag) -> bool`，而见证行是 `const _: fn() -> bool = super::CancelFlag::is_set;` —— 补了 `pub use` 仍然 `E0308`。**`owner` 方法的接收器在契约里不可表达**（把 `self` 写进 `params` 会让 `apiSignatureMismatch` 反向变红，因为取数器本来就把接收器排除在签名比对之外），见证形态对它结构性失效。这一条正是「接线才能发现」的第一笔收益。 |
| ③ | §20.2「机械解就是 `r#in` / `r#abstract`」 | 一致，但手写侧今天是 `#[serde(rename = "in")] pub in_degree` | `in_degree` / `abstract_note` 不可机械派生（是人为加后缀）；`r#in` 剥前缀后恰为 wire 名、serde 零 `rename`。改名属 ④ 的迁移动作，本变更只承诺 `r#` 形态与该形态的 golden 语料。 |
| ④ | §31.1 把「TS 最小（`const` → 字面量）」放进变更 1、A2 后置 | 一致 | TS 侧对封闭枚举只出 `export type X = 'a' \| 'b';`；逐值 JSDoc（`export declare const` 值对象）后置——它带来的承诺没有编译期绑定，先做等于先写一个无法验证的契约形态。 |
| ⑤ | §27.1「`def_shape` 还会把两者都叫 `enum`」 | **`enum` 类目保留名字、改触发点** | `categories` 的键集合 MUST NOT 变（`INDEX.toml` 与面板零改动）：`enum` 改由「`oneOf` / `anyOf` 全 `const` 分支」触发，`variant` 承接其余 union；`enum` 数组路径由装载期闸消灭，类目自然归零。 |

### 四 · 能力清单（对照 explore §31.1，六项全进）

| 项 | 内容 | 落点 |
|---|---|---|
| A1 | 类型 / 字段 / 变体 `description` → `///`（Rust） | `emit::rust` |
| B | facet 推导（std 面 + wire 面；跨元素 `$defs` 闭包；环即不推；装载期 fail-closed） | `emit`（新 resolver 入参） |
| C | 命名策略（本地名派生 + `r#` + serde 形态自动选） | `emit` |
| D | 整数宽度（严格完整值域） | `emit::value_type` / `scalar_type` |
| `oneOf+const` | Rust：unit variant + 变体 `///`；TS：字面量联合（变体注释留白） | 两渲染器 |
| F | 见证链接线（`core/archi`） | 入口两行 |

---

## 能力

### 新增能力

无。本变更改的是「`$defs` 能生出什么」与「`$defs` 的合法写法」，两者都是 `desktop-archi-api` 内既有的两节（关键字集与类型映射 / 装载校验与写闸）的正面扩张；拆成新能力会把同一份词表变成两份持有者。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-api` | **ADDED ×1 + MODIFIED ×2** | ① ADDED「类型出生与生成属性策略（facets / 命名 / 宽度 / 文档注释 / 封闭枚举）」：原则（属性零声明位、修复落规则）、facet 表、wire 面射程与三条不出理由（零字段 / 带负载 union / 含 `x-function` 字段的 struct）、命名策略（含既有 `x-key` 否决的边界澄清）、宽度值域表与「`usize` 不可表达」、文档注释三处、封闭枚举唯一形态、`def_shape` 触发点、消费方依赖前置、明确不做清单。② MODIFIED「关键字集、类型映射与错误面」：标准关键字表删 `enum`、映射表 `enum` 行退役、`oneOf`/`anyOf` 拆成「封闭枚举 / 带负载」两支、注解载体三选里的 `description` 落点具体化。③ MODIFIED「接口定义文件的装载校验与写闸（零第二份校验）」：⑨ 增三条判据与出路（`$defs` 条目顶层 `enum` 拒收 / 裸 `const` 拒收 / 不可 raw 的键名拒收；闸数仍九道，判序在既有五条 `$defs` 规则与 `x-class` 骨架之后、降级词表之前），并补一条 scenario。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-api-check` | 判据词汇仍十值（`ALL: [Self; 10]`）、`category` 仍七值、档位单点、纳管口径、`REPORT_VERSION` 逐字不变；本变更不碰 `check.rs` / `extract/**` 的**产品面**（`$ref` ⇒ `Named` 与 `async` 短形是独立变更；测试面唯一例外是 `extract/mod_test.rs` 的签名传导补参，见「变更范围 · 测试文件」）⇒ `apiFindings` 与 `status` 读数逐字不变。**已知窗口**：`$defs` 的字段形状零判据（`apiMissing` 只按名字核对类型存在性），本变更不为它补判据——契约出生之后形状只有一份真相，没有第二份可比。 |
| `desktop-archi-agent` | 知识文本的 `x-` 面仍五项、零新规则、体量锚不动（`archi-knowledge.md` 1824 B）。「types 出生在 schema」这条纪律知识文本已经写着（schema 是唯一凭据），不需要新增句子。 |
| `desktop-archi-drift` | 报告槽位、`status` / `passed` 推导、判据分桶零改动。 |
| `desktop-crate-layout` | **零依赖变化**：本变更不新增 / 删除任何 `Cargo.toml` 条目（`core/archi` 早已具备 `serde` + `specta`；`infra/archi` / `core/foundation` 按偏离 ① ② 不动）。「元素 crate 需 `serde` + `specta`」以消费方前置写在新 ADDED requirement 里，不改依赖登记表。 |
| `desktop-archi-contract-panel` | `INDEX.toml` 的 `categories` 键集合不变（`enum` 只改触发点），面板零 UI 改动。 |
| `desktop-archi-model` / `repo-archi-model` | 拓扑与模型数据零变化：不新增元素 / 关系 / 字段，`.dev-team/architecture/graph/**` 逐字节不变。 |
| `desktop-ipc-type-bindings` / `desktop-app-shell` | 零 DTO / 零命令 / 零 IPC 面；`bindings:export` 应零 diff。 |
| `desktop-archi-canvas` / `desktop-archi-gate` / `desktop-checks-domain` / `desktop-test-execution` | 门禁链四段与命令串逐字不变；渲染面改动不进判据面。 |
| `desktop-phase-prompts` / `desktop-write-protection` / `desktop-archi-chat` | 与本变更无交集（改的是生成策略与词汇，不是工具面 / 写权 / 提示词）。 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs` | `render_target` 增一份「FQN → `$defs`」解析表入参（跨元素 `$ref` 的 facet 递归要它；数据已在 `ApiSnapshot.schemas`，只是没往下穿——**这是「多穿一个入参」，不是新读盘**）；`local_defs` / `reexport_defs` / `notice_lines` / `warn_missing_x_class` / `reject_generics` 逐字不变 |
| `.../api/emit/rust.rs` | ① 类型 / 字段 / 变体 `description` → `///`（经既有 `rust_comment_text` 单点转义）；② derive 行 + 容器 serde 属性 + `#[default]`（facet 推导，含跨元素闭包与环的最小不动点）；③ 字段本地名派生（`camelCase` → `snake_case`）+ 关键字 `r#`；④ 宽度：`value_type` 与 `scalar_type` 共用同一值域判定；⑤ `variants_block` 增「`const` 分支 ⇒ unit variant + `///`」，`enum_block` **删除**；`op_witness` / `param_list` / `method_signature` / `function_pointer` / `optional_of` 逐字不变（**MUST NOT** 顺手改 `owner` 的见证形态，见风险 R-6） |
| `.../api/emit/ts.rs` | ① `oneOf` / `anyOf` 的 `const` 分支 → 标量字面量（经 `ts_string_literal` / 数字 / 布尔原样），与其余分支拼成字面量联合；② `enum_block` **删除**；③ 字段名不派生（TS 侧契约键即 wire 名）；注释与值声明面**零新增**（A2 后置） |
| `.../api/paths.rs` | 新增 `pub(crate) fn is_rust_keyword(token: &str) -> bool`（实现即 `RUST_KEYWORDS.contains(&token)`，复用既有 51 词唯一词表）：`emit::rust` 的字段名派生判关键字 MUST 复用这张表、MUST NOT 自抄第二份；`is_module_segment` 的常量引用改经该入口（**行为与返回值逐字不变**），其余函数零改动 |
| `.../api/schema.rs` | `verify_defs` 在既有五条 `$defs` 规则与 `x-class` 骨架之后、降级词表之前增三条：条目顶层 `enum` ⇒ `ApiError`（出路「封闭词表写成 `oneOf: [{const, …}]`」）、条目顶层裸 `const` ⇒ `ApiError`（`const` 只在 `oneOf` / `anyOf` 分支内合法）、条目第一层 `properties` 键命中不可 raw 的四个词（`self` / `Self` / `super` / `crate`）⇒ `ApiError`（出路「改名」）；`DEGRADED_KINDS` / `degraded_marks` / `build_index` / `verify_class_*` 逐字不变 |
| `.../api/mod.rs` | `render_all` 把 `snapshot.schemas` 装配成解析表传给 `render_target`；`def_shape` 的 `enum` 支改判「`oneOf` / `anyOf` 全 `const` 分支」，其余 union 仍 `variant`；`categories_of` / `index_entries` / `bootstrap` 逐字不变 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | 入口编排补齐（spec 既有规定、实现缺口）：`#[path = "lib.i.rs"] mod api_gen; pub use api_gen::*;`，位置与 `infra/archi` 同款 |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` +1 patch（`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动）；三条命令串逐字不变 |

### 契约资产（理由文档；本相位写）

| 文件 | 改动 |
|---|---|
| `.dev-team/architecture/models/Backend/Core/Archi.md` | 两处成因句改准：① 契约面段的「编译期绑定尚未生效……与 `Backend.Core.Foundation` 同状」→ 已接线（入口 `#[path] mod api_gen`；生成物零类型，故当前只编译两条见证行），Foundation 的不接理由改指其自身文档；② 类型段的「`$defs` 为空，但成因不是『类型都在进程内』」→ 保持这个判断，但把成因从「词汇没有引用已有类型的落点」改成「属性层此前无归属（derive / 命名 / 宽度 / 文档注释）+ 迁移是独立一轮、逐类型原子」。MUST NOT 列导出名 |
| `.dev-team/architecture/models/Backend/Core/Foundation.md` | 契约面段末句改准：不接生成物的原因**不是**「这些类型在入口处不可见」（补一行导出并不能解决），而是 `owner` 方法的接收器在契约里不可表达 ⇒ 见证形态对 `is_set` 结构性失效；「本模块不会为了『让生成物能编译』多开导出面」的立场逐字保留 |
| 12 份 `<叶段>.schema.json` + `graph/model/*.toml` | **零改动**（接口面与拓扑逐字节不变；本变更不新增 / 删除任何元素、关系、op、类型） |

### 测试文件

| 文件 | 改动 |
|---|---|
| `.../api/emit/rust_test.rs` | 新增：三个代表类型的 golden（`ArchiAssetSet`：struct + 具名 `$ref` 字段 + camelCase + 字段 `///`；`ArchiInterfaceAsset`：`Option<String>` + `bool` + `u64` + `Vec<String>`；`ElementKind`：unit-only enum + `Copy` / `Hash` + `lowercase` + 变体 `///`）；宽度五形态（u32 / u64 / i32 / 无 bounds / 部分范围）+ `Option<u32>`；关键字 `r#in` 与零 `rename`；空 `description` 零行；注入语料（含 `\n` / `*/` / emoji）；facet 表逐形态 + facet 保守四支（`number` / `x-function` / 映射规则 / 不可解析 `$ref` ⇒ 无 `Eq`）+ 自引用与互引环不推 `Eq` / `Copy`（「环」与「目标缺席」两形态可区分）；`const` 分支 → unit variant。既有 `enum` 数组语料（6 处）改写成 `oneOf+const` |
| `.../api/emit/ts_test.rs` | 新增：`const` 分支 → 字面量联合（含混合分支与转义）；`enum` 关键字零命中。既有 `enum` 数组语料（5 处）改写 |
| `.../api/emit/mod_test.rs` | `render_target` 新入参的夹具补齐（解析表）；**跨元素非空解析表四支**（两元素语料，A 的字段 `$ref` 指向 B 元素 `$defs` 条目）：① 目标可 `Eq` ⇒ `derive` 含 `Eq`（同一夹具把第五参换成空表即变红）；② 目标带 `x-function` ⇒ 解析成功而 `Eq` 不推（与「目标缺席」分开断言、两成因零 `Err`）；③ 三种缺席（FQN 不在表 / 目标无 `$defs` 键 / `$defs` 内无该条目）产物一致；④ 本文件 `$ref` 在空表下仍可解析、超大无关表与空表产物逐字相同。`Mystery` / 超长 description 两处既有语料断言不变 |
| `.../api/schema_test.rs` | 新增：条目顶层 `enum` 拒收（文案点名 pointer 与出路）、条目顶层裸 `const` 拒收、`oneOf` 分支内 `const` 放行；**不可 raw 键名四词逐词各一条语料**（`self` / `Self` / `super` / `crate` 各 `Err`，逐条断言 pointer `$defs.T.properties.<f>` 与「改名」出路；`self` 与 `Self` 两条文案互不相等 ⇒ 零归一），另两条边界（四词出现在第二层 `properties.a.properties.self` 与 `required` 数组时不判——闸只认第一层 `properties` 键）、两条判序（与 `$ref` 纯度 / `x-class` 骨架并存时归因既有规则）与一条同源（`validate_schema` 对该三形态报同一 pointer）；既有 9 处 `enum` 语料分别改判（`$ref` + 结构关键字的负例保留 `enum` 作被测键，值改为 `oneOf`）、`def_shape` 类目名清单（`:1214`）与 `x-class` 新键落点负例（`:2306` / `:2334`）改用 `oneOf+const` |
| `.../api/mod_test.rs` | `categories` 夹具增一条「`oneOf` 全 `const` ⇒ `enum`」与「带负载 ⇒ `variant`」；边界（`oneOf: []` ⇒ `variant`、单 `const` 分支 ⇒ `enum`），并断言 `enum` 类目只由「非空且全 `const`」触发（`def_shape` 零 `enum` 数组支） |
| `.../api/extract/mod_test.rs` | **测试面唯一的例外**（产品面见「不要修改」表）：`render_both` 夹具的两处 `emit::{ts,rust}::render(...)` 直接调用按五参签名传导补空解析表——不补即编译失败；提取逻辑、断言意图与其余用例逐字不变 |
| `.../api/paths_test.rs` | `render_ts(...)` 直接调用补空解析表（签名传导）；`rust_keywords单点持有` 增「`is_rust_keyword` 是唯一词表入口」的源码自证（`RUST_KEYWORDS` 的引用点从 `is_module_segment` 体内移到该入口）；`零第二形态与零清洗` 按新事实分判（`paths.rs` 仍零 `r#`，`emit/rust.rs` 的 `r#` 只许出在字段名派生单点） |
| `.../api/emit/{rust,ts}_test.rs` 的 `render_types` 辅助 | 签名随解析表入参调整（同文件内，零新增测试文件） |

### 生成物

| 产物 | 期望 |
|---|---|
| `crates/core/{archi,foundation}/src/lib.i.rs`、`crates/infra/archi/src/lib.i.rs`、`.dev-team/architecture/graph/INDEX.toml` | `api:export` 后**逐字节不变**（三个元素零有数据位类型、零 `description`、零值域关键字、零 `enum`/`oneOf`；`LocalArchiCheck` 走「零字段固定集合」）⇒ `api:check` 零差异 |
| `packages/desktop/src/types/generated/**` | 零 diff（`bindings:export` 幂等） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.../api/extract/{mod,rust,ts}.rs` 的**产品面**（含 `schema_type` / `signature_of` / `normalize_signature`） | 判据侧短形词汇归独立变更 `archi-api-signature-parity`（`$ref ⇒ Named`、`async` 前缀）；本变更 MUST NOT 顺手改短形，否则判据读数与本变更的验收锚（AC-1）互相污染。<br>**不存在「`extract/**` 一律不许动」这条**：唯一的测试面例外已在「测试文件」表登记——`.../api/extract/mod_test.rs` 的 `render_both` 夹具两处 `emit::{ts,rust}::render(...)` 调用按五参签名传导补空解析表（不补即编译失败）；该例外只补入参，不改提取逻辑、不改任何断言意图 |
| `.../api/check.rs`、`.../drift/report.rs` | 判据词汇十值、档位、`category` 七值、纳管口径逐字不变；本变更零新判据、零档位改动 |
| `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs` | `is_set` 的接收器缺口未裁决 ⇒ MUST NOT 接线；也 MUST NOT 为「让见证行能编译」删该 op 或多开导出面（`Foundation.md` 的立场） |
| `.../crates/*/Cargo.toml` | 本变更零依赖变化（见零 delta「`desktop-crate-layout`」） |
| `.../api/common.json` | 语义封闭为「跨语言**映射规则**」，且摘要进全部生成物头注释 ⇒ 改它即全量重生成 + 零 diff 门大噪声；`&Path` / `&str` 一类形态位不在本变更 |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | 迁移是独立一轮（逐类型原子；一个类型要么在手写、要么在生成，否则 `E0255`）；本变更 MUST NOT 顺手搬类型 |
| `.../crates/infra/agent/src/archi-knowledge.md` | `x-` 面仍五项、零新规则、体量锚不动 |
| `.dev-team/architecture/graph/**`、`INDEX.toml`、`.i.*` | 拓扑与生成物零数据变化；生成物 MUST NOT 手改 |
| 前端产品代码、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` | 零 UI / 零版本跟随（`AGENT.md` 版本纪律） |

---

## 验收标准

1. **AC-1 · 存量零 diff**：`api:export` 后 3 份 `lib.i.rs` 与 `INDEX.toml` 逐字节不变；`api:check` 零差异、退出码 0；12 份 `.schema.json` 与 6 份模型 TOML 逐字节不变；`bindings:export` 零 diff；`apiFindings` 与 `status` 读数与基线 `reports/archi/*.json` 逐字相同。
2. **AC-2 · golden 等价（核心闸）**：把 `ArchiAssetSet`、`ArchiInterfaceAsset`、`ElementKind` 三个类型按 `dto.rs` 现状手写成 schema，生成的**类型定义段**（`///` 注释 + derive 行 + 容器 serde 属性 + 变体 / 字段行）与 `dto.rs` 对应段落**逐字相同**（比对面不含手写 `impl` 块；`in_degree` / `usize` 两处按偏离 ③ 与 ④ 的前置除外；`abstract_note` 与类型级多行 `///` 两处按设计 D-11 的分歧登记除外）。`ElementKind` 的 schema MUST NOT 写 `default`——手写侧零 `Default`，条件 `Default` 的正面锚在 AC-3。
3. **AC-3 · 封闭枚举**：`oneOf: [{"const":"off","description":"…"}, …]` + `"default": "off"` → Rust `pub enum GateTier { /// … #[default] Off, … }`（与手写逐字相同；`default` 缺省的同类枚举零 `Default`——见 AC-2 的 `ElementKind`）；TS `export type GateTier = 'off' | 'warn' | 'strict';`（源码 `enum` 关键字零命中）；TS 侧逐值注释零产出（留白可断言）。
4. **AC-4 · `enum` 数组退役与不可 raw 键名拒收（三判同闸）**：① `{"$defs":{"Plain":{"enum":["a","b"]}}}` 在写闸 `Err` 且零落盘，文案点名 `$defs.Plain.enum` 与出路；② 条目顶层裸 `const` 同拒，文案点名 `$defs.<T>.const` 并写明「`const` 只在 `oneOf` / `anyOf` 分支内且 MUST 是标量」；③ 条目第一层 `properties` 的键 ∈ `{self, Self, super, crate}` ⇒ `ApiError`，文案点名 `$defs.<T>.properties.<f>` 并给出路「改名」。三判对 `verify_defs` 与 `validate_schema`（同一实现）同时成立、零第二份校验；判序在既有五条 `$defs` 规则与 `x-class` 骨架之后（新判不遮蔽既有规则），闸数仍九道。闸按**契约键（wire 名）**判、`r#` 派生不改变单词 ⇒ 派生后的本地名不做二判（不新增第二条名字闸）。`oneOf` 分支内的标量 `const` 放行；`emit` 源码 `enum_block` 零命中（`grep` 可判）。
5. **AC-5 · 命名策略**：camelCase 键 → `snake_case` 本地名 + 容器级 `#[serde(rename_all = "camelCase")]`；关键字键 → `pub r#in` 且**零** `rename`；候选 rename_all 全不命中时逐项 `#[serde(rename = "<wire>")]`；round-trip 语料：本地名经生成的 serde 形态映射回的名字集合 == 契约键集合。`x-key` 一类声明位零新增。
6. **AC-6 · 宽度**：`[0, 4294967295]` → `u32`、`[0, 18446744073709551615]` → `u64`、`[-2147483648, 2147483647]` → `i32`、无 bounds → `i64`、部分范围（`maximum: 100` / 仅 `minimum: 0`）→ `i64`、`type: "number"` → `f64`、`["integer","null"]` + 完整 u32 值域 → `Option<u32>`；契约里 `usize` 零写法（`grep` 可判）。
7. **AC-7 · 文档注释**：类型 / 字段 / 变体三处各一条正例；空 `description` 与缺省零注释行；op 的 `x-function.description` 仍不进 `.i.*`（与今天逐字节相同）；含 `\n` / `\u{2028}` / `*/` 的 `description` 不产断行、不产非法语法。
8. **AC-8 · wire 面射程**：有数据位的三种形态（字段位无 `x-function` 的有字段 struct / 封闭枚举 / 标量 newtype）三个派生在场；含 `x-function` 字段的 struct **零** wire 面、零 `Eq`（字段仍出 `fn(…)` 指针形）；零字段 `object` **零** wire 面且产物逐字节不变（`LocalArchiCheck` 即在案锚）；带负载 `oneOf` 零 wire 面、零 `Eq` / `Copy` / `Hash`（已知窗口，可红绿）。
9. **AC-9 · facet 正确性**：五形态各自的 `derive` 集合逐项相等且序固定；含 `number` 字段 ⇒ 无 `Eq`；自引用 `$ref` 环 ⇒ 无 `Copy`（且不 panic、不死循环）；facet 推导是 schema 的纯函数（同一 schema 两次渲染逐字相同、单测零读盘）；**非空解析表用例**（两元素语料：A 元素的字段 `$ref` 指向 B 元素 `$defs` 条目）——① 目标条目可 `Eq` ⇒ 解析表在场时 A 的 `derive` 含 `Eq`、**解析表置空即不含**（置空该用例必红，证明第五参被真实读取，而非「永远传空表」也能过）；② 目标条目带 facet 改判字段（含 `number` 或 `x-function`）⇒ 解析成功但 `Eq` 不推，与「目标缺席」两成因零 `Err`、`derive` 面向下一致（两成因只在可解析性上不同，不因分叉改写属性面）。
10. **AC-10 · 接线**：`core/archi/src/lib.rs` 的入口行与 `infra/archi` 同款后 `cargo build -p archi` 通过（`super::is_identifier` / `super::is_fqn` 两条见证真被编译）；`core/foundation` 仍无 `api_gen` 且其 `lib.i.rs` 逐字节不变。
11. **AC-11 · 索引类目**：`def_shape` 对「`oneOf` 全 `const`」记 `enum`、对其余 union 记 `variant`；`categories` 的键集合与今天相同；在案 `INDEX.toml` 逐字节不变。
12. **AC-12 · 生成物格式稳定**：生成物 MUST 是格式化器固定点 —— `cargo fmt --all --check` 零 diff（含 derive 行超宽折行形态）；`cargo clippy --workspace --all-targets` 零新告警。
13. **AC-13 · 改动集与版本**：`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（另加本 change 目录全量：`proposal.md` / `explore.md` / `design.md` / `tasks.md` / `test-design.md` / `specs/**` / `reports/**`）；`package.json` version 恰 +1 patch、`src-tauri/Cargo.toml` 逐字不变；`openspec/specs/**` 基线零改动（delta-only）。
14. **AC-14 · spec delta 形态**：`specs/` 恰一个能力目录（`desktop-archi-api`）；MODIFIED 块为基线整段拷贝后改（与基线 diff 只含本提案声明的差异），未丢任何未冲突条目；每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:`（恰 4 个 `#`）。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **golden「逐字相同」的工程难点**：`dto.rs` 的 derive 行受 `cargo fmt` 折行（`GateTier` 的十项 derive 就是折行形态），而 `.i.rs` 在 crate 内 ⇒ **也受 `cargo fmt --all --check` 管**（`server:check` 会红）。生成器必须已是格式化器固定点，否则「文本等价」与「fmt 绿」二选一 —— 这与 explore §8.1 的 `.i.ts` 尾空行债同族。 | AC-12 把「fmt 固定点」写成硬闸；试点类型**必须**包含折行形态（`GateTier`）；dev-design 先定死属性行的产出形态（单行 / 折行的判据），再改渲染器 |
| R-2 | **文本等价 ≠ 编译通过**：wire 面（`serde` / `specta` derive）在本变更里**零编译证据** —— 在案的唯一生成类型是零数据位的 `LocalArchiCheck`，golden 只比文本。`specta::Type` 对某些形态（跨元素 `$ref` 字段、`BTreeMap`、`Option<Vec<…>>` 嵌套）是否都能派生，没有实测。 | 首个真实编译证据归 ④ 迁移的第一个类型；本变更把「有数据位 ⇒ 三派生」写成 spec 的 MUST，AC-8 只断言三种形态的**在场性**。若 ④ 撞到不可派生形态，处置是装载期 fail-closed（扩 facet 判据），不是放宽 spec |
| R-3 | **raw identifier 的禁区**：`r#self` / `r#Self` / `r#super` / `r#crate` 非法 —— wire 名恰为这几个词时 `r#` 归一路径不成立，而后缀改名不可机械派生。 | 写闸 fail-closed：`$defs.<T>.properties` 的键 ∈ `{self, Self, super, crate}` ⇒ `ApiError` 并给出路「改名」。语料具体化为四词逐词各一条（`self` / `Self` / `super` / `crate` 各断言 pointer `$defs.T.properties.<f>` 与「改名」字样，`self` 与 `Self` 文案互不相等）+ 边界两条（第二层 `properties.a.properties.self` 与 `required` 数组里的四词不判）+ 同源一条（`validate_schema` 与 `verify_defs` 同 pointer）。闸只判契约键（wire 名），派生后的本地名不做二判（见 R-15 ①） |
| R-4 | **宽度值域与 JSON 数字精度**：`u64::MAX` 是 20 位十进制，draft 2020-12 校验器若用 double 会近似比较。 | spec 明写「`minimum` / `maximum` 从此有两个读者（标准校验语义 + 本地宽度）」；AC-6 的 `u64` 用例是唯一 20 位实例；若评审否决该字面量，回退序是「`u64` 不表达，`bytes` 一类字段落 `i64`」并同步改 ④ 的前置 |
| R-5 | **`render_target` 签名变化波及五条驱动面**：`emit::render_target` 增参 ⇒ `render_all` / IPC / CLI / agent 工具 / 测试夹具同批改；spec 的 Module Contract 里「`emit/mod.rs` 逐字不变」那条既有断言必须同批改写，否则 spec 自称不变而实现已变。 | 变更范围已把 `emit/mod.rs` 与 spec 的 Module Contract 行列进同一批；AC-13 的改动集包含它。语料具体化为 AC-9 的**非空解析表**两支（目标可 `Eq` ⇒ 在场含 `Eq`、置空即红；目标带改判字段 ⇒ `Eq` 不推）——没有它，增参可以「永远传空表」通过全部断言，第五参等于没接 |
| R-6 | **顺手改 `owner` 见证形态的诱惑**：接线时发现 `is_set` 必红，最容易的「修法」是改 `op_witness`（比如给 owner 方法补接收器位）——那会改判据短形与既有 3 份生成物，超出本变更。 | 变更范围「不要修改」显式列出 `op_witness`；偏离 ② 给出「不接线 + 登记」的处置；`owner` 见证形态的裁决留给 `archi-api-signature-parity` 或独立变更 |
| R-7 | **`enum` 数组退役是对既有键的收紧**：外部工作区的 schema 若已写 `enum` 数组，本变更后写闸会拒。 | 有意收窄（与 `x-class` 强制 `type: "object"` 同款纪律）；出路文案必须可执行（给出 `oneOf+const` 的完整样例片段），MUST NOT 只写「非法」。仓内零受影响（12 份 schema 零 `enum`） |
| R-8 | **负载 `oneOf` 的已知窗口**：契约不定义判别位，Rust 缺省外部标记会产出契约没写的线形态 ⇒ 本变更对该形态不出 wire 面。若有人以为「写了 `oneOf` 就自动可序列化」，会在 ④ 或更晚发现。 | AC-8 反向断言「零 wire 面、零 `Eq`/`Copy`」；spec 写明「负载 union 的线形态未定义（缺判别位）」，裁决属第一个真实需求 |
| R-9 | **`def_shape` 触发点改判可能漏掉既有断言**：`schema_test.rs:1214` 的类目名清单、`mod_test.rs` 的 `categories` 夹具都按旧触发点写。 | 两处列进测试文件表；AC-11 用「键集合不变 + 触发点可红绿」两条同时锚 |
| R-10 | **文档债与本变更的边界**：`Archi.md` / `Foundation.md` 的两处成因句在本相位改准；若实现被裁剪（例如 F 不做），文档就成了「写了没做」。 | 文档改动与 F 绑定列在同一份变更范围；F 若被裁，偏离 ② 与两处文档句必须同批回退（评审时一并看） |
| R-11 | **`categories` 里 `enum` 与 `variant` 的语义漂移**：负载 union 与封闭枚举都是 `oneOf`，前者记 `variant`、后者记 `enum`，读者需要知道这条分界。 | `def_shape` 的文档注释与 spec 同步写清分界；面板零改动（只渲键与计数） |
| R-12 | **`r#` 与容器级 `rename_all` 的叠加**：serde 先剥 `r#` 再套 `rename_all`，故 `r#type` 在 `rename_all = "camelCase"` 下仍是 `type`（`camelCase` 不改单词）。 | AC-5 的 round-trip 语料覆盖「关键字键 + 容器级 rename_all」组合；若某形态叠加后得不到 wire 名，回退到逐项 `rename`（生成器自动选，无需声明） |
| R-13 | **宽度规则的边界之外**：本变更的宽度规则只对「完整值域」生效，手写侧若带业务值域（例如 0..=100 的百分比），迁移时落 `i64`；2 个 `usize` 字段落 `u64` 并引出 `as usize` 用法改写。 | 这是**有意**的：业务约束不推类型（否则改 `maximum` 就静默换类型）。④ 迁移时若需要窄类型，处置是手写侧引入 newtype，不是给契约加宽度声明位 |
| R-14 | **本轮回溯补正的五处口径**（提案与 spec 的自洽代价）：① 原「变更范围」两表漏列 `.../api/paths.rs`（C9 增量）与 `.../api/paths_test.rs`、`.../api/extract/mod_test.rs`（五参签名传导），已补进实现 / 测试两表并把 `extract` 的「不要修改」从目录 glob 收窄到**产品面**；② spec 的 `Default` 措辞（「首变体」）与 wire 面射程（未覆盖 `x-function` 字段）与实现不符，已按 `$defs.<T>.default` 的条件语义与「无 JSON 线形态 ⇒ 不出」改准；③ `test-design.md` 里 `paths_test.rs`「既有断言零改动」一句与实现不符（`rust_keywords单点持有` / `零第二形态与零清洗` 两处按新事实改判），该文件归测试相位、**本相位不改**，登记在此以免归档后遗失；④ **AC-4 补第三判**（`$defs.<T>.properties` 键 ∈ `{self, Self, super, crate}` ⇒ `ApiError` + 出路「改名」；闸只判契约键，派生后的本地名不做二判）并把四词逐词语料写进「测试文件 · `schema_test.rs`」行——原 AC 只锚了 `enum` / 裸 `const` 两判，四词判据在 spec ⑨ 与实现里都有，缺的是 AC 与语料表；⑤ **AC-9 补非空解析表用例**（两元素语料：目标可 `Eq` ⇒ 在场含 `Eq` / 置空即红；目标带改判字段 ⇒ `Eq` 不推），并同步 `emit/rust_test.rs`（facet 保守四支 + 环）与 `mod_test.rs`（跨元素四支）两行、把 R-3 / R-5 的语料具体化——原来验收与语料表只走空解析表，跨元素 `$ref` 分支无守卫。 | 改动的只是**口径与登记**（④⑤ 对应的判据与跨元素用例在实现侧已在案），不改任何实现意图：落地侧若再动 `emit::render_target` 签名、`verify_closed_vocabulary` 三判或 facet 规则，提案两表与 spec 三处措辞 MUST 同批回退 / 再改准（否则归档时 spec 与实现再次分叉）。`api/paths.rs` 的增量是 `pub(crate)` 内部单点，不计入公共 API |
| R-15 | **两处已登记但本变更不修的判据形态 nit**：① 不可 raw 的四词闸（`self` / `Self` / `super` / `crate`）按**契约键（wire 名）**判，派生后的本地名不做二次判；② `emit::rust` 的 `is_unit_object` 与 `is_zero_field_object` 判据近重复（前者判 `Shape::Unit`、后者判「是否自动产零参 `new`」，差异只有 `x-class` 排除一项）。 | 两者都 **fail-closed**（宁可多拒 / 多判，不放行），不阻断本变更：四词闸对派生不改变单词的映射等价，`x-class` 排除项使零字段 object 与 trait 的构造面正确分叉。留后续变更收敛（收敛时须同批复核 `emit/rust_test.rs` 的 `r#` 与构造器语料），本变更 MUST NOT 为「顺手清理」扩闸数或改形态 |
