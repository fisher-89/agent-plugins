# 测试设计: archi-api-contract-vocabulary-limits

> **变更**: `archi-api-contract-vocabulary-limits`（契约词汇的「类型出生能力」：属性下沉 + 封闭枚举唯一形态 + 入口接线）
> **角色**: test-design-planner
> **输入**: 本目录 `proposal.md` / `design.md` / `specs/desktop-archi-api/spec.md` / `tasks.md`；既有 `test-design.md`（本文件为回溯补正后的版本）
> **范围**: 只设计单元测试范围与公共 API 签名；不写测试命令细节。静态脚本（`cargo fmt --check` / `cargo clippy` / `api:export` / `api:check` / `bindings:export` / `git status` 一类机械步骤）由桌面端执行，本相位不重复执行、不复制命令串。
> **模型面**: 零 delta——不写 `.dev-team/architecture/graph/**`、不改 12 份 `.schema.json`。被测文件全属 `Backend.Core.Archi` 的接口契约面（生成器 + 装载校验 + 装配），无「用例归到哪个元素」的分歧点。
> **回溯补正的登记**（提案 R-14 ③ / 设计「测试文件」表）：① `api/paths.rs` 独立成章：增量是 `pub(crate)` 内部单点（不计公共 API，故章首注明「公共函数 / API 表无该文件行」），`#### 待测功能` 记两条（`is_rust_keyword` 的词表唯一入口、`is_module_segment` 的常量引用移位），`#### 用例` 三行落定（`rust_keywords单点持有` / `零第二形态与零清洗` 两处改判 + `render_ts` 五参传导）；② `api/extract/mod_test.rs` 的补参是**两处**（`render_both` 夹具的 `ts::render` 与 `rust::render`），不是一处；③ `paths_test.rs` 的 `rust_keywords单点持有` 与 `零第二形态与零清洗` 按新事实改判（旧版「既有断言零改动」一句已删除）。
>
> **测试框架与测区（实测）**: `packages/desktop/src-tauri` 为 Rust 2021 workspace，运行器 `cargo test`（rustc 内建 harness），无外部断言库——断言一律 `assert!` / `assert_eq!` / `assert_ne!` 带中文失败消息。测试文件共置兄弟文件 `<源文件主干>_test.rs`，扩展名恒 `.rs`，由父模块内 `#[cfg(test)] mod <stem>_test;` 挂载（`api/emit/mod.rs` 尾部即 `mod mod_test;` / `mod rust_test;` / `mod ts_test;` 三行）。零新 `tests/` 集成目录（既有 `tests/{golden_test,vocabulary_test,perf}.rs` 属 DSL / 词汇 / 性能面，与本变更零交集）、零新 dev-dependency（临时根用 std 自建 `TempDir`）。
>
> **夹具惯例**（复用、零新造）：手写接口定义文本经 `api::mod_test::{prepare, mirrored_schema_text, empty_schema, op_schema, reexport_def}` 真实投影（`Contract::from_schema`，不过滤、不替身）；真实磁盘侧用 `api::mod_test::TempDir` / `paths_test` 的 std `TempDir`；源码自证用 `include_str!("<源文件>")`；载元用 `crate::api::meta()` 真实值。
>
> **Mock 基调**: 六份映射对全部零替身——渲染器纯出 `String`、`verify_defs` 只吃 `Value`、`render_all` 只在 `TempDir` 真实根内读盘、`paths_test` 的存在性探测落真实空文件（零内容即足以驱动分支）。本变更零进程边界依赖注入面（无网络 / 无子进程 / 无 DB / 无全局可变状态）。

## 验收范围

<!-- 逐条映射 proposal.md 的 14 条 AC。被测文件或模块列 = 「源文件 -> 测试文件」映射对（可多值），
     `.../` = `packages/desktop/src-tauri/crates/core/archi/src/`；
     纯静态 / 编译期 / 文档面 AC 落「—（见不可测试项 N）」。每条源文件映射对至少在一行出现。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 存量零 diff：`api:export` 后 3 份 `lib.i.rs` 与 `INDEX.toml` 逐字节不变、`api:check` 零差异、12 份 `.schema.json` 与 6 份模型 TOML 不变、`bindings:export` 零 diff、`apiFindings` / `status` 读数与基线 `reports/archi/*.json` 逐字相同 | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs`（零字段 `object` 五项 derive 与 `LocalArchiCheck` 产物逐文字面锚、条件 import 零字节）、`.../api/emit/mod.rs -> .../api/emit/mod_test.rs`（`Mystery` / 超长 `description` 两处既有断言逐字保留）、`.../api/mod.rs -> .../api/mod_test.rs`（`INDEX.toml` 内容断言持衡）；diff 门与基线读数见不可测试项 1 |
| AC-2 | golden 等价（核心闸）：`ArchiAssetSet` / `ArchiInterfaceAsset` / `ElementKind` 三类型（另加折行锚 `GateTier`）的**类型定义段**与 `dto.rs` 对应段落逐字相同（比对面不含手写 `impl` 块；`in_degree` / `usize` / `abstract_note` / 类型级多行 `///` 四类例外按设计 D-11 排除） | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs` |
| AC-3 | 封闭枚举：`oneOf+const` → Rust `pub enum`（`///` + 条件 `#[default]`）；TS `export type X = 'off' \| 'warn' \| 'strict';`，源码 `enum` 关键字零命中、零 `as const`、逐值 JSDoc 零产出 | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs`、`.../api/emit/ts.rs -> .../api/emit/ts_test.rs` |
| AC-4 | 三判同闸：条目顶层 `enum` 数组 / 条目顶层裸 `const` / 第一层 `properties` 键 ∈ `{self, Self, super, crate}` 各 `Err` 且零落盘，文案点名 pointer 与可执行出路；`oneOf` / `anyOf` 分支内标量 `const` 放行；判序在既有五条 `$defs` 规则与 `x-class` 骨架之后、闸数仍九道；`emit` 源码 `enum_block` 零命中 | `.../api/schema.rs -> .../api/schema_test.rs`（三判据 + 判序 + 同源）、`.../api/emit/rust.rs -> .../api/emit/rust_test.rs` 与 `.../api/emit/ts.rs -> .../api/emit/ts_test.rs`（`enum_block` / `type_block` 的 `"enum"` 支源码自证零命中） |
| AC-5 | 命名策略：camelCase 键 → `snake_case` + 容器级 `rename_all`；关键字键 → `pub r#in` 且零 `rename`；候选全不命中 → 逐项 `#[serde(rename = "<wire>")]`；round-trip 名字集合 == 契约键集合；零新声明位 | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs`（关键字 `r#` / 四候选逐档 / round-trip，关键字词表经 `api/paths.rs` 的 `is_rust_keyword` **真实组合**覆盖）、`.../api/paths.rs -> .../api/paths_test.rs`（「`is_rust_keyword` 是唯一词表入口」与 `r#` 零第二形态分判两处源码自证） |
| AC-6 | 宽度：三条完整值域 → `u32` / `u64` / `i32`；无 bounds 与部分范围 → `i64`；`maximum: 100` 不推 `u8`；`type: "number"` → `f64`；`["integer","null"]` + 完整 u32 域 → `Option<u32>`；契约零 `usize` 写法 | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs` |
| AC-7 | 文档注释：类型 / 字段 / 变体三处各一条正例；空 `description` 与缺省零行；op 的 `x-function.description` 仍丢弃；含 `\n` / `\u{2028}` / `*/` 不产断行与非法语法 | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs` |
| AC-8 | wire 面射程：有字段 struct（字段位无 `x-function`）/ 封闭枚举 / 标量 newtype 三形态三派生在场；含 `x-function` 字段的 struct 零 wire 面、零 `Eq`；零字段 `object` 零 wire 面且产物逐字节不变；带负载 `oneOf` 零 wire 面、零 `Eq` / `Copy` / `Hash` | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs` |
| AC-9 | facet 正确性：五形态 derive 集合逐项相等且序固定；`number` 字段 ⇒ 无 `Eq`；自引用 / 互引 `$ref` 环 ⇒ 无 `Copy`（不 panic、不死循环）；facet 是 schema 的纯函数（两次渲染逐字相同、单测零读盘）；**非空解析表两支**（目标可 `Eq` ⇒ 在场含 `Eq`、置空即红；目标带改判字段 ⇒ 解析成功而 `Eq` 不推，与「目标缺席」两成因零 `Err`） | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs`（本级判定）、`.../api/emit/mod.rs -> .../api/emit/mod_test.rs`（跨元素解析表半边：链路 `emit::render_target → rust::render → facets → def_entry → schemas`） |
| AC-10 | 接线：`core/archi/src/lib.rs` 入口两行与 `infra/archi` 同款后 `cargo build -p archi` 通过（两条见证真被编译）；`core/foundation` 仍无 `api_gen` 且其 `lib.i.rs` 逐字节不变；两份理由文档成因句与 F 绑定 | —（`.../lib.rs` 见不可测试项 2；`.dev-team/architecture/models/Backend/Core/{Archi,Foundation}.md` 见不可测试项 6） |
| AC-11 | 索引类目：`def_shape` 对「`oneOf` / `anyOf` 非空且全 `const`」记 `enum`、其余 union 记 `variant`；`categories` 键集合与今天相同；在案 `INDEX.toml` 逐字节不变 | `.../api/mod.rs -> .../api/mod_test.rs`（在案 `INDEX.toml` 字节不变属静态门，见不可测试项 1） |
| AC-12 | 生成物格式稳定：`cargo fmt --all --check` 零 diff（含 derive 行折行形态）、`cargo clippy --workspace --all-targets` 零新告警 | `.../api/emit/rust.rs -> .../api/emit/rust_test.rs`（derive 行 88/97 字符固定点、doc 行 `trim_end`、`#[default]` 位置 = 单测可断言半边）、`.../api/emit/ts.rs -> .../api/emit/ts_test.rs`（单引号字面量不动点）；两机械门见不可测试项 3 |
| AC-13 | 改动集与版本：`git status --porcelain` ⊆ 变更清单；`packages/desktop/package.json` version 恰 +1 patch；`src-tauri/Cargo.toml` 逐字不变；`openspec/specs/**` 基线零改动 | —（见不可测试项 4；`.../api/paths.rs` 的增量与 `.../api/paths_test.rs` 的改判已在设计「测试文件」表登记） |
| AC-14 | spec delta 形态：`specs/` 恰一能力目录（`desktop-archi-api`）、MODIFIED 块为基线整段拷贝后改、每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:` | —（见不可测试项 5） |

---

## 单元测试

> **`路径类型` 取值**：`正向` / `异常` / `边界` / `组合`（跨模块链路用例，按链路发起方 / 最上层调用方挂到入口模块章节，方向写在测试条件里；不新建独立集成测试章节）。
> **`迭代类型` 取值**：`新增`（本轮新增用例）、`修改`（既有测试文件随行为 / 签名变化改写）、`保留`（既有用例零改动）。
> **覆盖纪律**：每份 `#### 用例` 表对每个 `#### 待测功能` 条目至少覆盖正向 / 异常 / 边界三类；`#### 待测功能` 条目只取 `design.md`「公共函数 / API」表中「所在文件」为该源文件的行，不 grep 源码导出、不虚构条目。
> **参数边界纪律**：`str`（空 / >1000 字符 / `\n` `\0` emoji / 缺省）、`int`（0 / -1 / MAX / 无 bounds）、`bool`（true / false / 缺省）、`array`（`[]` / 单元素 / 超大 / 缺省）、`object`（`{}` / 缺必填 / 多余字段 / 缺省）、`Optional[T]`（None）按类型系统映射逐类落行——落点见各表「边界」行。

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod_test.rs

<!-- 挂 AC-1（既有语料断言持衡）、AC-9（跨元素解析表半边）、AC-4（泛型判据与 x-class 面不变）。 -->

#### 待测功能

- `render_target(target: &EmitTarget, contract: &Contract, schema: &Value, meta: &ApiMeta, schemas: &BTreeMap<String, Value>) -> Result<String, ApiError>`: 语言分派 + 泛型零支持判据 + `x-class` 反向提示；第五参是「元素 FQN → 接口原文」只读表（直接借用 `ApiSnapshot.schemas`，零拷贝、零读盘），经私有 `def_entry` 定位本文件 / 跨元素 `$ref`；唯一装配点是 `render_all`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `emit/mod_test · render_target`（分派与透传） | 正向 | `GeneratedKind::Rust` / `GeneratedKind::TypeScript` 两值各分派到对应渲染器，产物与直连同五参的 `rust::render` / `ts::render` 逐字相等 | 修改 |
| `emit/mod_test · render_target`（分派与透传） | 异常 | `typeParameters` / `typeArguments` 出现、或类型名带 `<`…`>` ⇒ `Err(ApiError::Contract)` 点名该类型（增参后判据与文案逐字不变） | 保留 |
| `emit/mod_test · render_target`（分派与透传） | 边界 | 空解析表 ⇒ 与今天产物逐字相同（`Mystery` / 超长 `description` 两处既有断言逐字保留）；`properties` 全为函数却未标 `x-class` ⇒ 仍一行 stderr 警告、每元素恰一次、不进返回值、不改退出码 | 保留 |
| `emit/mod_test · render_target`（跨元素 `$ref` 解析） | 正向 | 解析表在场且目标条目可 `Eq`（`App.Backend.Infra.Agent` 的 `$defs.Boxed` 为有字段 object）⇒ `Holder` 的 `derive` 含 `Eq`；同一夹具把第五参换成空表即不含（置空该用例必红，证明第五参被真实读取，而非「永远传空表」也能过） | 新增 |
| `emit/mod_test · render_target`（跨元素 `$ref` 解析） | 组合 | 链路 `emit::render_target → rust::render → facets → def_entry → schemas`：字段 `$ref` 指向外部契约（`.schema.json#/$defs/Boxed`）+ 解析表含该 FQN 且其 `$defs` 含 `Boxed` ⇒ facet 递归可解析（`Eq` 在场），且产品面 `pub use` 段形态不变（五参透传零额外语义） | 新增 |
| `emit/mod_test · render_target`（跨元素 `$ref` 解析） | 异常 | 跨元素目标条目存在但自身不可 `Eq`（目标 `properties` 含 `x-function` 字段）⇒ 解析成功而 `Eq` 不推；与「目标缺席」两成因零 `Err`、`derive` 面向下一致（两成因只在可解析性上不同，不因分叉改写属性面） | 新增 |
| `emit/mod_test · render_target`（跨元素 `$ref` 解析） | 边界 | 三种缺席各判：目标 FQN 不在表 / 目标无 `$defs` 键 / `$defs` 内无该条目（另加空表）⇒ `def_entry` 返 `None`、facet 保守（无 `Eq`）、零 `Err`、零 panic、四种产物逐字一致；本文件引用（`#/$defs/<名>`）在解析表为空时仍可解析（`local` 优先、零查表）；200 条无关 FQN 的超大表 ⇒ 产物与空表逐字相同；空 `BTreeMap` 与「无表」同形 | 新增 |
| `emit/mod_test · render_target`（五参单形源码自证） | 正向 | `include_str!("mod.rs")` 内 `emit::render_target(` 恰 1 处调用，实参五元且序为 `&target, contract, schema_value, &meta, &snapshot.schemas`；两渲染器 `render` 签名自证串按 rustfmt 折行后的**多行**形态更新（第五参使签名超 100 字符，MUST NOT 期待单行串） | 修改 |
| `emit/mod_test · render_target`（五参单形源码自证） | 异常 | 本文件零 `emit::{rust,ts}::render` 直连（批量入口只经 `render_target` 一处分派）；`render_target` 的返回错误族持衡（只在 `reject_generics` 一侧产 `ApiError::Contract`） | 保留 |
| `emit/mod_test · render_target`（五参单形源码自证） | 边界 | 全部直接 `render_target(...)` 调用末位补空解析表后零断言变化（机械传导）；本文件内 `super::{rust,ts}::render(...)` 直接调用同批补第五参（解析表入参） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 `rust::render` / `ts::render` 分派（分派正确性即被测语义）+ 手写 JSON 经 `prepare` 投影；解析表用真实 `BTreeMap<String, Value>`（含跨元素两元素语料）；stderr 不造替身、不捕获输出流（沿「源码自证 + 返回集零影响」两向断言） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs

<!-- 挂 AC-1（零字段逐文字面）、AC-2（golden）、AC-3、AC-4（enum_block 零命中）、AC-5、AC-6、AC-7、AC-8、AC-9、AC-12（固定点）。 -->

#### 待测功能

- `render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget, schemas: &BTreeMap<String, Value>) -> String`: `.i.rs` 全文渲染（头注释 / 条件 import / 类型段 / 自动构造器 / 透传 / 见证）；`schemas` 是 `$ref` 的 facet 递归与可解析性判定的唯一读面；纯函数、零读盘、同输入两次渲染逐字相等。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `rust_test · render`（golden / `ArchiAssetSet`） | 正向 | 契约：`object` 三字段 `topology` / `interface` / `doc`，各为 `$ref` 到本文件 `$defs` 条目 + 逐字段 `description`，`required` 全列，键全 `camelCase` ⇒ 类型定义段与 `dto.rs::ArchiAssetSet` 段落逐字相同：`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]` 单行（≤96 字符）+ `#[serde(rename_all = "camelCase")]` + 三行字段 `///` + 三行 `pub topology: ArchiTopologyAsset,` 等 | 新增 |
| `rust_test · render`（golden / `ArchiAssetSet`） | 异常 | 三字段 `$ref` 指向解析表缺席的跨元素目标（空表）⇒ 字段行照出、`Eq` 保守缺席（derive 无 `Eq`）、零 `Err` / 零 panic、零半截属性行（推不出来只是不推） | 新增 |
| `rust_test · render`（golden / `ArchiAssetSet`） | 边界 | `required: []`（空数组）⇒ 三字段全 `Option<…>`，容器 `rename_all` 与 derive 其余项形态不变（零 `Option<Option<…>>`）；`properties: {}`（空对象）⇒ 零字段形态 `pub struct T;`，不产空花括号块 | 新增 |
| `rust_test · render`（golden / `ArchiInterfaceAsset`） | 正向 | 契约：`path`（`["string","null"]`）+ `present`（`boolean`）+ `bytes`（`integer` `minimum: 0` / `maximum: 18446744073709551615`）+ `ops` / `types` / `reexports`（`array<string>`）+ key 为 Rust 关键字的 `abstract`，`required` 全列 ⇒ 与 `dto.rs::ArchiInterfaceAsset` 段落逐字相同：`pub path: Option<String>,` / `pub present: bool,` / `pub bytes: u64,` / `pub ops: Vec<String>,` / `pub r#abstract: String,`（`abstract_note` 字段名属 D-11 例外：只比 `pub r#abstract: String` 与 wire 名 `abstract`，不比手写后缀名） | 新增 |
| `rust_test · render`（golden / `ArchiInterfaceAsset`） | 异常 | 某字段 `description: ""`（空串）⇒ 该字段零 `///` 行且其余字段行与带说明时逐字相同（单字段归因，零连带改写）；`description` 为 `null` / `true`（非字符串）⇒ 同为零行、不 panic | 新增 |
| `rust_test · render`（golden / `ArchiInterfaceAsset`） | 边界 | `required` 缺省与全列两形态对照：`path` 本身可空 + 可选叠加仍只 `Option<String>`（`Option<Option<…>>` 零产出）；`bytes` 上界恰为 `18446744073709551615`（20 位十进制 `u64::MAX`）⇒ 精确命中 `u64`（不经 f64 近似）；`["integer","null"]` + 完整 u32 值域 ⇒ `Option<u32>` | 新增 |
| `rust_test · render`（golden / `ElementKind`） | 正向 | 契约：`oneOf` 四分支 `const`（`package` / `domain` / `module` / `composite`）+ 逐分支 `description` + 无 `default` ⇒ 与 `dto.rs::ElementKind` 段落逐字相同：9 项 derive 单行（88 字符）+ `#[serde(rename_all = "lowercase")]` + 四变体各带 `///`；零 `Default` / 零 `#[default]`（D-2：`default` 缺席即不推） | 新增 |
| `rust_test · render`（golden / `ElementKind`） | 异常 | `"default": "unknown"`（值不命中任何 `const` 分支）⇒ 零 `Default`、零 `#[default]`、零 `Err`（Q-6 保守口径可红绿） | 新增 |
| `rust_test · render`（golden / `ElementKind`） | 边界 | 单 `const` 分支（`oneOf` 一项）⇒ 仍为封闭枚举形态（unit variant、零悬垂括号）；`const` 文本非法标识符面：`"it's"` / `"a b"` / `"1st"` / `""`（空串）⇒ 变体名经 `variant_name` 归一为常量名，恒为合法标识符 | 新增 |
| `rust_test · render`（封闭枚举 / 折行锚） | 正向 | 契约：`oneOf` 三 `const`（`off` / `warn` / `strict`）+ `"default": "off"` ⇒ 与 `dto.rs::GateTier` 逐字相同：10 项 derive **折三行**形态（`#[derive(` / `    …十项…,` / `)]`，组装串 97 字符）+ `#[serde(rename_all = "lowercase")]` + `#[default]` 在 `Off,` 上方（`///` 之后）+ 逐变体 `///` | 新增 |
| `rust_test · render`（封闭枚举 / 折行锚） | 异常 | 混合分支（两个 `const` + 一个 `{"$ref": …}` 负载）⇒ `const` 支落 unit variant、负载支沿用既有 `Variant{n}(payload)` 形态；wire 面不出、`Eq` / `Copy` / `Hash` 全缺（facet 表「带负载」行） | 修改 |
| `rust_test · render`（封闭枚举 / 折行锚） | 边界 | 10+ 个 `const` 分支（超大 union）⇒ derive 行折三行且两次渲染逐字相同（固定点稳定）；`oneOf: []`（空分支集）⇒ 不 panic、零半截属性行、两次渲染逐字相同（类目归属由 `def_shape` 一侧锚为 `variant`）；既有 6 处 `enum` 数组语料已改写为 `oneOf+const`，旧「`enum` 不落 serde 派生」断言删除、按新 facet 表（封闭枚举出 wire 面）改判 | 修改 |
| `rust_test · render`（整数宽度） | 正向 | 三条完整值域各出对应类型：`[0, 4294967295]` → `u32`、`[0, 18446744073709551615]` → `u64`、`[-2147483648, 2147483647]` → `i32`；字段位与标量 newtype 位两臂同源（`{"type":"integer","minimum":0,"maximum":4294967295}` 作 `$defs` 条目 ⇒ `pub struct T(pub u32);`） | 新增 |
| `rust_test · render`（整数宽度） | 异常 | 部分范围与单边一律 `i64`：`maximum: 100`（不推 `u8`）、仅 `minimum: 0`、仅 `minimum: -1`；`maximum: 4294967296`（越界 1）⇒ `i64`；`exclusiveMinimum: -1` + `exclusiveMaximum: 4294967296` ⇒ 归一后命中 `u32`；`type: "number"` + 完整 u32 值域 ⇒ 仍 `f64`（值域不参与 `number`） | 新增 |
| `rust_test · render`（整数宽度） | 边界 | 无 bounds（`{"type":"integer"}`）⇒ `i64` 且与今天产物逐字相同；`minimum` / `maximum` 为非数字（字符串 / `null` / bool）⇒ `i64`、不 panic；`minimum > maximum` 的畸形区间 ⇒ `i64`（不外溢、不 panic） | 新增 |
| `rust_test · render`（命名策略 / `r#`） | 正向 | 契约：`{"in": {"type":"integer"}, "typeOnly": {"type":"boolean"}}` + `required` 全列 ⇒ `#[serde(rename_all = "camelCase")]` + `pub r#in: i64` + `pub type_only: bool`，源码零 `#[serde(rename = "in")]`（`r#` 剥前缀即 wire 名）；`abstract` 键同式出 `pub r#abstract` | 新增 |
| `rust_test · render`（命名策略 / `r#`） | 异常 | 候选 `rename_all` 全不命中（wire 名 `a-b` / `x y` 一类不可机械回归）⇒ 每字段上方逐项 `#[serde(rename = "a-b")]`，本地名仍 `snake_case`；wire 名含引号 / 反斜杠 / 换行 ⇒ `rename` 值经 `string_literal`（`{:?}` 形态）转义，产物恒为合法 Rust 字面量 | 新增 |
| `rust_test · render`（命名策略 / `r#`） | 边界 | round-trip 语料：本地名经所选 serde 形态（容器 `rename_all` 或逐项 `rename`）映射回的名字集合 == 契约键集合（关键字键 + 容器级 `rename_all` 叠加，R-12）；四候选（`camelCase` / `snake_case` / `PascalCase` / `lowercase`）逐一命中 + 全不命中五形态各断言所选形态；变体面候选序 `lowercase` 首试（单字小写词在 `camelCase` 下亦命中，只有 `lowercase` 首试才与 `ElementKind` 逐字对齐）；`r#self` / `r#Self` / `r#super` / `r#crate` 零产出（四词由写闸拒收） | 新增 |
| `rust_test · render`（文档注释） | 正向 | 类型级（`$defs.<T>.description`）/ 字段级（`properties.<f>.description`）/ 变体级（`oneOf[i].description`）三处各一条 ⇒ 三处各出一行 `/// <文本>`，文本与契约逐字相等；位置固定：类型 `///` 在 derive 行之上、字段 `///` 在字段行之上、变体 `///` 在变体行之上 | 新增 |
| `rust_test · render`（文档注释） | 异常 | 含 `\n` / `\r` / `\u{2028}` / `\u{2029}` / `*/` 的 `description` ⇒ 经 `rust_comment_text` 折成空格、零断行、零非法语法（注释不顶出裸代码行）；`\0`（NUL）与 emoji 原样保留、不 panic | 新增 |
| `rust_test · render`（文档注释） | 边界 | `description: ""` / 纯空白串 / 键缺省（`None`）三形态 ⇒ 零注释行；>1000 字符的 `description` ⇒ 单行产出且行尾零空白（`trim_end` 固定点，`cargo fmt` 不再改）；op 的 `x-function.description` 仍丢弃（产物与不带该键时逐字节相同，`op_witness` 逐字不变） | 新增 |
| `rust_test · render`（wire 面射程） | 正向 | 有字段 struct / 封闭枚举 / 标量 newtype 三形态各出 `Serialize, Deserialize, specta::Type` 三项且落在固定相对序位；头注释后出 `use serde::{Deserialize, Serialize};`（非限定名，`serde` 行在前） | 新增 |
| `rust_test · render`（wire 面射程） | 异常 | 含 `x-function` 字段的 struct ⇒ 零 wire 面（D-12 / Q-3：无 JSON 线形态）、零 `Eq`，字段仍出 `fn(…)` 指针形；带负载 union ⇒ 零 wire 面 + 零 `Eq` / `Copy` / `Hash`、零 panic | 新增 |
| `rust_test · render`（wire 面射程） | 边界 | 零字段 `object` ⇒ derive 逐字 `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`（零 `Hash` / 零 `Default`）且产物与今天逐字节相同（`LocalArchiCheck` 即在案锚）；产物文本（剔除 `//` 与 `///` 行）含 `BTreeMap<` ⇒ 出 `use std::collections::BTreeMap;`（`serde` 行在前、`std` 行在后）；两 import 皆无 ⇒ 零字节（AC-1 零 diff 前提） | 新增 |
| `rust_test · render`（facet 五形态与环） | 正向 | 表驱动五形态逐项对拍：零字段 `object` / 封闭枚举 / 有字段 struct / 标量 newtype / 带负载 union 的 derive 集合与相对序（`Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type` 子序）逐项相等 | 新增 |
| `rust_test · render`（facet 五形态与环） | 异常 | 含 `number` 字段（`f64`）⇒ 无 `Eq`；含 `x-function` 字段 ⇒ 无 `Eq`；字段走映射规则（`common.json` 的 `Outcome` 一类）⇒ 无 `Eq`（保守不假设映射后 `Result` 实现 `Eq`）；`$ref` 不可解析 ⇒ 无 `Eq`（保守、零 `Err`） | 新增 |
| `rust_test · render`（facet 五形态与环） | 边界 | 自引用 `$ref` 环（T 的字段 `$ref` 到 T）⇒ 无 `Eq` / 无 `Copy`、有限步返回、零栈溢出零 panic；互引环（A→B→A）同判；同一 schema 两次 `render` 逐字相等（纯函数、零读盘）；`$ref` 指向解析表缺席的目标与指向环的两形态不混淆（产物可区分、成因不混同） | 新增 |
| `rust_test · render`（出口单形 / 源码自证） | 正向 | `include_str!("rust.rs")` 内**带完整五参签名**的 `pub fn render` 恰 1 处（第五参 `schemas: &BTreeMap<String, Value>` 在末位、返回 `String`）；按 rustfmt 折行后的**多行**签名形态命中，单行串零命中亦断言；`render_scope` 私有（零 `pub`） | 修改 |
| `rust_test · render`（出口单形 / 源码自证） | 异常 | `enum_block` 零命中（函数已删）、`type_block` 的 `"enum"` 数组触发支零命中；`op_witness` / `owner` 见证形态逐字不变（R-6 禁令：MUST NOT 顺手改接收器位）；`emit/rust.rs` 与 `emit/mod.rs` 仍零 `ApiError::Element` 构造（错误族持衡） | 修改 |
| `rust_test · render`（出口单形 / 源码自证） | 边界 | `render_types` / `render_source` 辅助与全部直接 `render(...)` 调用补第五参（空解析表）后，无关语料断言逐字持衡（`Mystery` / 超长 `description` / 头注释四句 / 泛型零支持）；用例名与注释的「四参」措辞同批改五参 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 手写接口定义 JSON 经 `mod_test::prepare` 真实投影出 `(Contract, Value)`；渲染器纯出 `String`，零文件系统 / 时钟 / 子进程依赖；`dto.rs` 段落经 `include_str!("../../dto.rs")` 读真实手写源；源码自证用 `include_str!` 读本文件旁的真实源 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs

<!-- 挂 AC-1（存量零 diff 锚）、AC-3（字面量联合 / enum 关键字零命中）、AC-4（enum_block 零命中）、AC-5 对照面（字段名不派生）。 -->

#### 待测功能

- `render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget, _schemas: &BTreeMap<String, Value>) -> String`: `.i.ts` 全文渲染；`oneOf` / `anyOf` 的 `const` 分支 → 标量字面量并拼联合；解析表结构上不用（签名与 Rust 侧对齐，避免分派器两套形态）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `ts_test · render`（`const` 字面量联合） | 正向 | `oneOf` / `anyOf` 三 `const`（`off` / `warn` / `strict`）⇒ `export type GateTier = 'off' \| 'warn' \| 'strict';`（成员经 `union_of` 连接、零 `enum` 关键字、零 `as const`、零 `(typeof` 前缀）；`anyOf` 同式产出同一形态 | 新增 |
| `ts_test · render`（`const` 字面量联合） | 异常 | 字符串 `const` 含 `'` / `\` / `\n` / `\r` / `\t` / `\u{2028}` ⇒ 逐字符经 `ts_string_literal` 单引号转义（`'\''` / `'\\'` / `'\n'` / `'\u2028'`），产物恒为合法 TS；非字符串 `const`（数字 `1` / 布尔 `true` / `false`）⇒ `to_string()` 原样 | 新增 |
| `ts_test · render`（`const` 字面量联合） | 边界 | 单 `const` 分支 ⇒ 无悬垂 `\|`（`export type T = 'a';`）；混合分支（一个 `const` + 一个 `{"type":"string"}`）⇒ `'a' \| string`；空 union（`oneOf: []`）⇒ `never`（既有 `union_of` 口径）；>1000 字符的 `const` 文本 ⇒ 单行产出不折行、零尾随空白、不 panic | 新增 |
| `ts_test · render`（旧形态退役） | 正向 | 封闭枚举产物零 `enum` 关键字（源码与产物双面断言）、零 `as const` 数组、零同名值对象（`export const`）；逐值 JSDoc 零产出（产物零 `/**`、零逐值 `// 值` 注释——A2 后置的留白可断言） | 修改 |
| `ts_test · render`（旧形态退役） | 异常 | 既有 5 处 `enum` 数组语料改写为 `oneOf+const` 后零 `as const` 命中、零 `(typeof …)[number]` 形态（旧断言已按新形态改判，不得并存）；`enum_block` 源码零命中（函数已删）、`type_block` 内零 `"enum"` 触发支 | 修改 |
| `ts_test · render`（旧形态退役） | 边界 | `$defs` 为空 ⇒ 仅头注释 + 操作段，形态与今天逐字相同（存量零 diff 锚）；`properties: {}` ⇒ `export interface T {}` 逐字持衡 | 保留 |
| `ts_test · render`（字段名不派生） | 正向 | camelCase 键与关键字键（`in` / `abstract`）在 interface 中原样出现（`in?: number;` / `abstract?: string;`），零 `r#` 形态、零 `#[serde` 形态（TS 侧契约键即 wire 名） | 新增 |
| `ts_test · render`（字段名不派生） | 异常 | 可选轴三态（`required` 全列 / 空数组 / 键缺省）⇒ `{field}:` 与 `{field}?:` 各按既有形态，零 `\| null` 拼接（可空与可选两个事实各只落自己的符号）；`x-function` 字段不参与可选轴 | 保留 |
| `ts_test · render`（字段名不派生） | 边界 | 超长键名 / emoji 键名（若通过装载闸）原样透传、不 panic；`type: "number"` 与 `type: "integer"` 同形（`number`），整数宽度在 TS 侧零派生 | 保留 |
| `ts_test · render`（五参签名 / 解析表零使用） | 正向 | `include_str!("ts.rs")` 内带完整五参签名的 `pub fn render` 恰 1 处（`_schemas` 末位、返回 `String`，按 rustfmt 折行后的多行形态命中）；`render_scope` 私有 | 修改 |
| `ts_test · render`（五参签名 / 解析表零使用） | 异常 | 传非空解析表与空表产物逐字相同（TS 侧零 facet 递归、零 `$ref` 可解析性判定）；`enum_block` / `type_block` 的 `"enum"` 支零命中 | 新增 |
| `ts_test · render`（五参签名 / 解析表零使用） | 边界 | 本文件一处跨渲染器调 Rust `render` 的调用与 `render_target` 分派两路产物逐字相等（分派零额外语义）；既有头注释四句 / 透传段 / 操作段断言逐字持衡 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 手写接口定义 JSON 经 `mod_test::prepare` 真实投影；渲染器纯出 `String`（零 IO）；源码自证 `include_str!` | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs

#### 待测功能

<!-- 挂 AC-5（`is_rust_keyword` 是唯一词表入口 / `r#` 零第二形态）、AC-13（`render_ts(...)` 五参传导的登记面）。
     design.md 的「公共函数 / API」表无本文件行——本文件增量为 `pub(crate)` 内部单点，公共 API 零变化（`is_module_segment` 的签名与返回值逐字不变）。以下两条据 design.md 的 C9 与「变更清单」的 `api/paths.rs` 行登记（该行明列两处触点：新增 `is_rust_keyword`、`is_module_segment` 的常量引用改经该入口），不取源码导出声明、不虚构条目。 -->

- `is_rust_keyword(token: &str) -> bool`: `RUST_KEYWORDS`（`paths.rs` 内 51 词唯一词表）的唯一读取入口，只回答「是否命中关键字」这一半；crate 内唯一消费方是 `emit::rust` 的字段名派生（C9 / D-4：MUST NOT 在 `emit` 自抄第二份关键字表）；四不可 raw 词（`self` / `Self` / `super` / `crate`）不在本函数分档——它们由装载期写闸拒收（C4 第三判）。
- `is_module_segment(token: &str) -> bool`: 「字符集 ∧ 非关键字」形态判据的独担者；本次只把体内常量引用改经 `is_rust_keyword`，签名、判序与返回值逐字不变（新增入口对既有模块段判据零语义变化）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `paths_test · rust_keywords单点持有`（`is_rust_keyword` 词表唯一入口） | 正向 | `include_str!("paths.rs")` 内 `const RUST_KEYWORDS` 恰 1 处且类型 `[&str; 51]`、零 `matches!(token` 长臂；`is_module_segment` 体内调 `is_rust_keyword(` 且仍含字符集判定（合取两项都不缺）；`is_rust_keyword` 体内引用 `RUST_KEYWORDS`；`emit/rust.rs` / `emit/ts.rs` / `contract.rs` / `check.rs` 四份真实产品源零 `RUST_KEYWORDS`（第二份词表出现即红）；行为面对拍：关键字词（`fn` / `type` / `in` / `abstract` / `self`）恒 `false`、合法段（`view` / `_x` / `a1`）恒 `true`，与「常量直引」旧形态逐值同判 | 修改 |
| `paths_test · 零第二形态与零清洗`（`r#` 双面分判） | 异常 | `paths.rs` 零 `"r#"` 且零 `r#{`（模块段改原始标识符即第二形态）；`emit/rust.rs` 的 `r#` 只许出在字段名派生单点——`"r#"` 字面量恰 1 处（`RAW_PREFIX`）、`r#{name}` 产出恰 1 处（`field_ident`）；`paths.rs` 零 `replace('-', '_')` 三写变体（零清洗 = 零第二套命名）；任一第二形态 / 第二份关键字表 / 第二处 `r#` 出现即红（词表入口唯一性由此双向锁住） | 修改 |
| `paths_test · render的from经ts_string_literal`（`render_ts(...)` 五参传导） | 边界 | `render_ts(&contract, &schema, &meta(), &target, &BTreeMap::new())` 末位补空解析表后，三值语料（`it's` / `a\b` / `a\nb`）的 `from '<specifier>';` 断言与产物逐字不变、零断言意图变化（不补即 `E0061` 编译失败）；空 `BTreeMap` 与「无解析表」同形（TS 侧零 facet 递归）；另锚 `is_rust_keyword` 的 `str` 边界——`""` / `"\0"` / emoji 键 / >1000 字符一律 `false` 且零 panic（`&str` 无 `None` 形态），与 `is_module_segment` 的空串拒收同判 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 源码自证经 `include_str!("paths.rs")` / `include_str!("emit/rust.rs")` 等读真实产品源（字符串计数 + 函数体切片），真实 `is_module_segment` / `is_rust_keyword` 直调；`render_ts(...)` 走真实 TS 渲染器 + 既有 `mod_test::prepare` 真实投影，末位是真实空 `BTreeMap::new()`（空表即被测语义，不造替身）；本节零 IO / 零进程边界 / 零全局可变状态 | 本节三行 |

### packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs

<!-- 挂 AC-4（三判据 + 判序 + 闸数仍九 + 同源）。 -->

#### 待测功能

- `verify_defs(schema: &Value) -> Result<(), ApiError>`: `$defs` 逐条目规则校验；每条目末尾新增三判（条目顶层 `enum` 数组 / 条目顶层裸 `const` / 第一层 `properties` 键 ∈ `{self, Self, super, crate}`，实现为私有 `verify_closed_vocabulary`），判序在既有五条 `$defs` 规则与 `x-class` 骨架规则之后、降级词表之前；闸数仍九道。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `schema_test · verify_defs`（顶层 `enum` 拒收） | 正向 | `oneOf` / `anyOf` 全 `const` 分支形态放行（零 `Err`）；`oneOf` 分支内的标量 `const`（字符串 / 数字 / 布尔 / `null`）四值各放行 | 新增 |
| `schema_test · verify_defs`（顶层 `enum` 拒收） | 异常 | `{"$defs":{"Plain":{"enum":["a","b"]}}}` ⇒ `Err(ApiError::Contract)`，文案点名 `$defs.Plain.enum` 且含可执行出路（完整 `"oneOf"` + `"const"` + `description` 样例片段，MUST NOT 只写「非法」） | 新增 |
| `schema_test · verify_defs`（顶层 `enum` 拒收） | 边界 | `enum: []`（空数组）同样拒收（只认键在场）；`$ref` 与 `enum` 并列时既有「透传条目不得带结构关键字」先命中（判序单点归因，新三判不遮蔽既有规则） | 新增 |
| `schema_test · verify_defs`（顶层裸 `const` 拒收） | 正向 | 条目顶层无 `const` 的合法形态全过；`oneOf` / `anyOf` 分支内标量 `const` 正常通过 | 新增 |
| `schema_test · verify_defs`（顶层裸 `const` 拒收） | 异常 | `{"$defs":{"T":{"const":"a"}}}` ⇒ `Err` 点名 `$defs.T.const`，文案写明「`const` 只在 `oneOf` / `anyOf` 分支内且 MUST 是标量」 | 新增 |
| `schema_test · verify_defs`（顶层裸 `const` 拒收） | 边界 | `const: 1` / `const: false` / `const: null` 三非字符串值同样拒收（判据只认键在场）；字段位 `properties.<f>.const` 当前不判（Q-8 已知窗口——断言放行并登记为窗口，不虚构判据） | 新增 |
| `schema_test · verify_defs`（不可 raw 键名拒收） | 正向 | 可 raw 关键字键（`type` / `in` / `abstract` / `match` / `move`）放行（`r#` 形态可达） | 新增 |
| `schema_test · verify_defs`（不可 raw 键名拒收） | 异常 | `properties` 键 ∈ `{self, Self, super, crate}` 四词逐词各 `Err`，文案点名 `$defs.T.properties.<word>` 并给出路「改名」；`self` 与 `Self` 两条文案互不相等 ⇒ 零归一 | 新增 |
| `schema_test · verify_defs`（不可 raw 键名拒收） | 边界 | 四词出现在第二层（`properties.a.properties.self`）不判（只判第一层 `properties` 的键）；四词出现在 `required` 数组里不判（判据只认 `properties` 键） | 新增 |
| `schema_test · verify_defs`（判序与存量持衡） | 正向 | 闸数仍九道、九闸别名与 `validate_schema` doc 的三个 needle（`骨架四判据` / `两键形态` / `落点`）仍命中；`DEGRADED_KINDS` 词表与 `degraded_marks` 计数逐字不变 | 修改 |
| `schema_test · verify_defs`（判序与存量持衡） | 异常 | 同时违反 `$ref` 纯度与顶层 `enum` ⇒ 文案归因 `$ref` 结构关键字；`x-class` 带属性与顶层 `enum` 同时在场 ⇒ 归因 `x-class` 骨架规则（新三判排在既有规则之后，零改判） | 新增 |
| `schema_test · verify_defs`（判序与存量持衡） | 边界 | 既有 9 处 `enum` 语料逐处改判后意图不变：`$ref` + 结构关键字负例保留 `enum` 作被测键、`x-class` 落点负例（`:2306` / `:2334`）改 `oneOf+const`、类目名清单（`:1214`）与类目键名（`:1128` / `:1151`）不变；非 schema 路径零校验与 `api = none` 空壳合法持衡；写闸 `validate_schema`（同一实现）对该三形态同样 `Err`（零第二份校验） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 接口定义文本手写（`json!` / 原始 `&str` 经 `schema_text` / `mirrored_schema_text`）+ 真实 `serde_json` 解析 + 真实 `paths::ref_relative`；`INDEX` 侧真实 `toml_edit` 反解析；`ApiMeta` 以字面量构造真实值 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

<!-- 挂 AC-1（生成物文本断言持衡）、AC-11（索引类目触发点）、AC-9（装配面）。 -->

#### 待测功能

- `render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError>`: 批量入口（三类生成物集合与落点零变化）；把 `snapshot.schemas` 装配成 `emit::render_target` 的第五参（唯一装配点）；`def_shape` 的 `enum` 类目改由「`oneOf` / `anyOf` 非空且全 `const`」触发。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `api/mod_test · render_all`（`def_shape` 触发点） | 正向 | `oneOf` 全 `const`（`Choice` 夹具）⇒ `enum`；`oneOf` 带一个负载分支（`{"$ref": …}` 或 `{"type":"object"…}`）⇒ `variant`；`anyOf` 全 `const` ⇒ `enum`；`x-class` + `oneOf` 并存 ⇒ 仍 `class`（既有判序优先） | 修改 |
| `api/mod_test · render_all`（`def_shape` 触发点） | 异常 | `enum` 数组条目在装载期已被写闸拒收（`render_all` 结构上收不到）⇒ `def_shape` 内 `"enum"` 数组触发支源码自证零命中（唯一触发点是 `is_closed_enum(`）；`INDEX` 零新类目键 | 修改 |
| `api/mod_test · render_all`（`def_shape` 触发点） | 边界 | `oneOf: []`（空 union）⇒ `variant`（「非空且全 `const`」的边界）；单 `const` 分支（`oneOf` / `anyOf` 各一）⇒ `enum`；`$defs` 缺省 ⇒ `categories` 空表（整段省略）且不 panic | 新增 |
| `api/mod_test · render_all`（`categories` 持衡） | 正向 | `render_all的def_shape六形状` 夹具的 `Choice` 改 `oneOf+const` 后六形状计数逐项不变（`enum = 1` / `variant = 1` / `types = 8`、sum(categories) == types） | 修改 |
| `api/mod_test · render_all`（`categories` 持衡） | 异常 | 越界形态零污染：任一条目都不产出六值之外的键；`build_index` 的键序（字典序）与键集合不变 | 保留 |
| `api/mod_test · render_all`（`categories` 持衡） | 边界 | 同形状多条目（5 条 `object`）计数合并口径不变；`degraded` 计数仍按标记出现次数（与类目不同源） | 保留 |
| `api/mod_test · render_all`（五参装配与生成物集合） | 正向 | 源码自证实参序 `&target, contract, schema_value, &meta, &snapshot.schemas`（五元、`target` 首位、解析表末位）；`TempDir` 真实根下三类生成物集合与落点与今天相同（`.i.rs` / `.i.ts` / `INDEX.toml`） | 修改 |
| `api/mod_test · render_all`（五参装配与生成物集合） | 异常 | `snapshot.schemas` 缺该 FQN 条目 ⇒ 该接口零生成物（既有 `continue` 行为不变）、其余接口不受影响、零 `Err` | 保留 |
| `api/mod_test · render_all`（五参装配与生成物集合） | 边界 | `snapshot.schemas` 为空表 ⇒ 全部接口跳过且零 `Err`（空态 vs 停摆态不混同）；生成物文本与既有断言逐字不变（`LocalArchiCheck` 零字段 + 两条见证行两处锚） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（临时根） | 真实 `TempDir`（std 自建）+ `write_model` / `write_pair_schema` 落真实模型 TOML 与 `.schema.json`，真实 `load_contracts` 读回——不造替身读盘对象（`render_all` 的入参是根路径，真实目录即被测语义） | 本节全部用例 |

---

## 不可测试项

- AC-1 的零 diff 门与基线读数（`api:export` 后 3 份 `lib.i.rs` + `INDEX.toml` 逐字节不变、`api:check` 零差异、12 份 `.schema.json` 与 6 份 `graph/model/*.toml` 不变、`bindings:export` 零 diff、`apiFindings` / `status` 与 `reports/archi/*.json` 逐字相同） — **原因**: 生成物字节比对与 bindings 幂等属静态脚本面（桌面端执行），单测无法承载跨文件字节快照；单测只承载其不变量锚（`LocalArchiCheck` 零字段逐文字面、条件 import 零字节、`INDEX.toml` 内容断言、既有语料断言持衡，见 `rust_test` / `emit/mod_test` / `api/mod_test` 各节）。
- AC-10 接线（`packages/desktop/src-tauri/crates/core/archi/src/lib.rs` 的 `#[path = "lib.i.rs"] mod api_gen;` + `#[allow(unused_imports)] pub use api_gen::*;`，`cargo build -p archi` 编译两条见证行；`core/foundation` 仍无 `api_gen` 且其 `lib.i.rs` 逐字节不变） — **原因**: 证据是编译期（`E0308` / `E0425` 能否触发）与「该 crate 入口两行无自有行为、无测试文件」；`#[allow(unused_imports)]` 的动机（空 glob 触发 `unused_imports`、`clippy --fix` 会删行使接线静默失效）属 clippy 机械门。接线语义由编译与评审把关。
- AC-12 的两道机械门（`cargo fmt --all --check` 零 diff、`cargo clippy --workspace --all-targets` 零新告警） — **原因**: 格式化器 / linter 固定点由桌面端脚本执行；其**产出形态**半边已在 `rust_test`（derive 行 88/97 字符、doc 行 `trim_end`、属性序 `///` → `#[serde(rename)]` → `#[default]`）与 `ts_test`（单引号字面量不动点）落为可断言用例。
- AC-13 改动集与版本（`git status --porcelain` ⊆ 变更清单、`packages/desktop/package.json` version 恰 +1 patch、`src-tauri/Cargo.toml` 逐字不变、`openspec/specs/**` 基线零改动） — **原因**: git 面与版本字段为静态声明，无行为面可自动化。
- AC-14 spec delta 形态（`specs/` 恰一能力目录 `desktop-archi-api`、MODIFIED 块为基线整段拷贝后改、每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:`（恰 4 个 `#`）） — **原因**: markdown 静态文本约束，随归档流程校验；无源码行为面。
- `.dev-team/architecture/models/Backend/Core/Archi.md` 与 `.../Foundation.md` 的两处 / 一处成因句（契约面「已接线」/「`owner` 接收器不可表达」、类型段 `$defs` 为空的成因改写） — **原因**: 文档是产品面（架构页直接渲染），无行为面；其正确性与 AC-10 的接线 / 不接线决定绑定（R-10：F 若被裁，`Archi.md` 接线句与 `Foundation.md` 接收器句 MUST 同批回退），由实现相位按「F 是否裁剪」核对，评审把关；两份文档 MUST NOT 列导出名（名字清单唯一持有者是 `.schema.json`）。
- `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs` 的**两处** `emit::{ts,rust}::render(...)` 补参（`render_both` 夹具） — **原因**: 属测试文件自身的签名传导（零断言变化，不补即编译失败），不构成被测源文件映射对，故无独立章节；五参单形的定义由 `emit/rust.rs` 的「出口单形 / 源码自证」与 `emit/ts.rs` 的「五参签名 / 解析表零使用」两节统一锚定。
- `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` 的 51 词表本体（`RUST_KEYWORDS`）的**完备性与真值** — **原因**: 「这张表是否覆盖 rustc 全部关键字、是否多收」没有第二份真相可比（`paths_test` 的 `rust_keywords封闭五十一值` 只比「恰 51 项 / 零重复 / 逐值 `is_module_segment == false`」三条自洽性，`schema_test` 只比写闸对四不可 raw 词的反应），真值只能靠评审与 `rustc` 试编译。该文件的可测试半边（`is_rust_keyword` 是唯一词表入口、`is_module_segment` 常量引用移位后行为不变、`r#` 零第二形态、`render_ts` 五参传导）已在 `api/paths.rs -> api/paths_test.rs` 章的 `#### 用例` 三行落定，故该文件**不入**「测试路径无法解析」之列；`emit` 侧的消费面另由 `api/emit/rust_test.rs` 的关键字 `r#` / round-trip 用例真实组合覆盖。
