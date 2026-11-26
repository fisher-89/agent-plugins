# 测试设计: archi-api-constructor-static-members

> **日期**: 2026-11-26
> **依据**: `proposal.md`（AC-1–AC-13）+ `design.md`（C1–C7 / D-1–D-14、「变更清单 → 新增文件 / 修改文件」、公共函数 / API、内部接线面、「既有夹具与台账的机械随动」）+ `specs/` 三份 delta（`desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-agent`）。
> **被测模块来源**: design「新增文件」为空；「修改文件」七项——`api/schema.rs` / `api/contract.rs` / `api/emit/rust.rs` / `api/emit/ts.rs` / `api/check.rs` / `infra/agent/src/archi-knowledge.md` / `packages/desktop/package.json`。前六项各有同目录 `<主干>_test.rs` 测试路径；`package.json`（配置面）、生成物（`.i.rs` / `.i.ts` / `INDEX.toml`）、前端 `bindings` 生成物、`specs/**` delta 无单测路径，登记在「不可测试项」。
> **框架与路径约定**: Rust 内建 `cargo test` / `cargo nextest`，测试与源文件**同目录、同模块**，文件名 `<源主干>_test.rs`，在模块底部以 `#[cfg(test)] mod <主干>_test;` 声明（`api/mod.rs` 末尾 `check_test` / `contract_test` / `mod_test` / `schema_test` 等、`api/emit/mod.rs` 末尾 `mod_test` / `rust_test` / `ts_test`、`infra/agent/src/lib.rs` 的 `archi_knowledge_test`）。断言面为 `assert!` / `assert_eq!` / `matches!`，夹具为手写 JSON（`serde_json::json!` / `mirrored_schema_text` / `prepare`），源码锚用 `include_str!` 逐函数体比对。全部用例 `#[test]`（本 change 零异步面）。
> **Mock 纪律**: 六个测试文件既有形态均为**零框架 mock**——真实 `serde_json` 解析、真实 `Contract::from_schema` 投影、真实两个渲染器、真实 `mount::registry()`、真实 `foundation::layout` 派生、真实 `ApiMeta` 字面量。进程边界只用真件：`check_test` 用 std 自建 `TempDir` 真实落盘（本 crate 零 dev-dependency），`archi_knowledge_test` 用 `include_str!("archi-knowledge.md")` 取编译期内嵌的同一份字节。本设计不引入新 mock、不为测试新增导出。
> **路径口径**: 「验收范围」表写全路径（相对仓库根）；「单元测试」各节标题写全路径并以 `->` 分隔源与测试。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 存量零 diff：11 份在案 schema（零 `x-class`、零新键）经 `api:export` 重生成后 `.i.rs` / `.i.ts` / `INDEX.toml` 逐字节不变；`categories` 计数与 `types` / `ops` / `degraded` 三项读数不变；`api:check` 零差异 | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`（非类条目产物零变化）；生成物与 `api:check` 门见「不可测试项」 |
| AC-2 | 正例生成（Rust）：`{"type":"object","x-class":true,"x-constructor":…,"x-static":…}` 的 `.i.rs` 在 trait body 内产 `fn new(params) -> Self;` 与 `fn <名>(params) -> R;`，**零** `impl T { … new() … }`、**零** `const _: fn(…) -> super::T` | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` |
| AC-3 | 正例生成（TS）：同一 schema 的 `.i.ts` 产 `export interface T { … }` + `export declare const T: { new (params): T; <名> (params): R; };`；无 ctor / static 的 `x-class` 与任何 object 条目**零值声明**（产物与今天逐字节相同） | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` |
| AC-4 | 落点负例：`object` / enum / 标量 / 纯 `$ref` alias 上写 `x-constructor` 或 `x-static` ⇒ `ApiError` 点名 `$defs.<T>.<键>` 与出路；两键出现在顶层 / `properties.<k>` / `properties.<k>.x-function` 内 ⇒ 各一条 `ApiError`；顶层新 `x-` 键 ⇒ 顶层键封闭文案列五项 | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` |
| AC-5 | 骨架与语义互斥负例：`{"x-class": true}` / `{"type":"string","x-class":true}` / `additionalProperties` 对象与布尔两形态 —— 四形态各 `ApiError`，文案含可执行出路（补 `"type": "object"` / 去掉 `additionalProperties` 并说明条目层 map 本期不渲染）；`type` 与 `additionalProperties` 两条判据排在 `required` 与属性判据**之前**（判序可断言） | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` |
| AC-6 | 新键形态与保留名负例：ctor 带 `returns` / ctor `async: true` / static 名 `new` / static 名非标识符 / `x-class` 条目 `properties` 含 `new` / `x-static` 名与该条目 `properties` 键重名 —— 六形态各 `ApiError`（N-1 / N-2 / N-3 可逐条红绿） | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` |
| AC-7 | 空 trait 边缘回归：`{"type":"object","x-class":true,"properties":{}}` 只产 `pub trait Gate {\n}\n`；源码层 `is_zero_field_object` 的判据含 `x-class` 排除（源码锚可判），`constructible` 收集是唯一消费点 | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` |
| AC-8 | 良性对照：`{"type":"object"}`（无 `x-class`、`properties` 缺省）仍产 `impl T { pub fn new() -> Self { Self } }` 与 `const _: fn() -> super::T = super::T::new;`；`{"type":"object","additionalProperties":{"type":"string"}}`（无 `x-class`）**仍被写闸放行** | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` |
| AC-9 | 判据面：`apiConstructorDeclared` 只在**模块级** `properties` 命中构造器名；`x-class` + `x-constructor` 零 `apiConstructorDeclared`；`ArchiApiFindingKind::ALL` 长度仍 **10**、`category` 仍七值、`severity()` 不动；无构造通路的 `x-class` **零判据、零警告** | `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` |
| AC-10 | 知识骨架：骨架 `lf_bytes(trim_end)` = **1808 B** 且 ≤ 1831 B；全文 = **1997 B** 且 ≤ 2031 B；四项与三处文案同批（模块头 / 测试 doc / 失败文案）；`x-` 面**五项逐名**在场且零「见 spec」；`指令表收窄且新行集可判` 的 needle 集含两新键（删任一项即红） | `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` |
| AC-11 | 夹具面收敛：`grep '"x-class": true'` 在 `api/**` 下逐处处置完毕——正例与外层负例全部带 `"type": "object"`；`schema_test` 的 `bare` 已翻为负例并断言「缺 `type`」；五处既有负例断言的原因文本未因判序变更而漂移 | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`（夹具随动） |
| AC-12 | 版本与改动集：`package.json` `version` 相对 `HEAD` 恰 +1 patch；`git status --porcelain` 的改动集 ⊆ 提案「变更范围」；`openspec/specs/**` 基线零改动、`src-tauri/Cargo.toml` 逐字不变 | `packages/desktop/package.json`（配置面，见「不可测试项」）、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`（体量上限 ≤ 640 行与零改动面回归） |
| AC-13 | spec delta 形态：`specs/` 恰**三个**能力目录（`desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-agent`）；MODIFIED 块为基线整段拷贝后改；每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:` | `openspec/changes/archi-api-constructor-static-members/specs/**`（文档面，见「不可测试项」） |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs

#### 待测功能

- `validate_schema():` 九道闸**条数**与闸序 ①–⑧ 逐字不变（新规则全落 ⑨）、签名与两处消费（写闸零落盘 / 装载期停摆）不变
- `verify_defs():` `x-class` 分支固定判序（`type` → `additionalProperties` → `required` → `properties` 全方法 → 两键形态）+ 尾部全文档落点单趟；既有五条 `$defs` 规则与 `degraded` 词表扫描逐字保留
- `verify_top_level_keys():` 顶层封闭六项与四键必在场逐字不变；`x-` 面枚举三项 → 五项逐名（文案）
- `verify_class_members()（内部接线面）:` 构造器 / 静态成员形态的唯一判点（单个对象 + 对象型 `x-function` + `returns` / `async` 禁项 + 成员缺 `x-function` 即错）
- `verify_class_key_positions()（内部接线面）:` 全文档落点单趟（含私有递归体）——合法 pointer 集合恰为「`$defs.<T>` 且该条目 `x-class: true` 的顶层」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `verify_defs` | 正向 | 合法骨架面全过：`{"type":"object","x-class":true,"properties":{}}`（空 trait）、同条目带全 `x-function` 方法、`x-class: false` + 带属性、`x-class` 值为字符串 `"true"`（非 bool 不命中 class 分支）四臂 ⇒ 全 `Ok`；既有 `false_flag` 臂逐字保留（AC-5 正例面） | 修改 |
| `verify_defs` | 异常 | 缺骨架两主形态：`{"x-class": true}` 与 `{"type":"string","x-class":true}` ⇒ 各 `Err`，文案点名 `$defs.Gate.type`、报实际形态、含出路「补 `\"type\": \"object\"`」；字符串边界四值同臂：`type: "Object"`（大小写）/ `"object "`（尾空格）/ `""`（空串）/ `null` ⇒ 各 `Err`（str 边界 4 + 正例 1） | 新增 |
| `verify_defs` | 异常 | `additionalProperties` 语义互斥四形态：对象 `{"type":"string"}` / 布尔 `false` / 布尔 `true` / 空对象 `{}` ⇒ 各 `Err`，文案点名 `additionalProperties` 且含两条可执行出路（去掉 `x-class` 变纯数据并说明条目层 map 本期不渲染、键值对容器写在值位；若本意是方法集合则去掉该键）（bool 边界 true / false / 缺省三臂） | 新增 |
| `verify_defs` | 边界 | 只治一侧（AC-8 反向断言）：`{"type":"object","additionalProperties":{"type":"string"}}`（无 `x-class`）⇒ `Ok`；同形态再带 `properties` ⇒ 仍 `Ok`（本期不改它的静默） | 新增 |
| `verify_defs` | 边界 | 判序固定（骨架四条，判序即错误面）：`additionalProperties` + `required` + 非函数属性并存 ⇒ 报 `additionalProperties`；`type` 缺失 + 其余三条并存 ⇒ 报 `type`；有 `type` 时 `required` + 非函数属性 ⇒ 报 `required`；只剩非函数属性 ⇒ 报 `properties`——四条文案互不相等（逐条可辨） | 新增 |
| `verify_defs` | 回归 | 新判据不遮蔽既有五条 `$defs` 规则：嵌套 `$defs` + 缺 `type` 并存 ⇒ 报嵌套；单独保留任一条 ⇒ 各自 `Err`；既有 `verify_defs的x_class不吞并既有规则` 的 or 断言与 `assert_ne!` 逐字保留 | 修改 |
| `verify_class_members` | 正向 | 构造器合法面：`params` 缺省 / `params: []` / 单参 / 100 参（list 超大边界）⇒ 全 `Ok`；`async: false` ⇒ `Ok`；外层另加键（`note` / `description`）⇒ `Ok`（Q-4：外层不设未知键闸） | 新增 |
| `verify_class_members` | 异常 | 构造器形态负例：值为数组 / 字符串 / `null` / `{}`（缺 `x-function`）/ `x-function` 为标量 ⇒ 各 `Err`（点名 `$defs.<T>.x-constructor`）；带 `returns` ⇒ `Err` 点名 `returns` + 出路「返回面由语义固定为本类型；别名构造写 `x-static` 并返回本类型」；`async: true` ⇒ `Err`（N-3，出路「构造器是值而非 future」） | 新增 |
| `verify_class_members` | 异常 | 静态成员形态负例：`x-static` 值为数组 / 字符串 / `null` ⇒ 各 `Err`；任一成员缺 `x-function` / `x-function` 为标量 ⇒ `Err`（缺包裹即静默丢）；`x-static` 100 成员（超大 dict）全合法 ⇒ `Ok` | 新增 |
| `verify_class_members` | 边界 | 缺省与空面：`x-static: {}`（零成员）⇒ `Ok`；两键都缺省 ⇒ `Ok`（`Optional` 缺省 = 无构造入口 / 无静态方法）；成员内层自由形状（`params` / `returns` / `async` / `description` 任意组合）⇒ `Ok` | 新增 |
| `verify_class_key_positions` | 正向 | 合法位：`x-class: true` 条目顶层单独写 `x-constructor` / 单独写 `x-static` / 两键并存 ⇒ 全 `Ok`；同条目带方法 + ctor + static 的完整正例 ⇒ `Ok` | 新增 |
| `verify_class_key_positions` | 异常 | 非类条目四形态：`x-constructor` 写进纯 `object` / `enum` / 标量（`type: "string"`）/ 纯 `$ref` alias ⇒ 各 `Err` 点名 `$defs.<T>.x-constructor` 与出路「只有 `x-class` 条目可声明构造入口与静态方法」；`x-static` 写进 `enum` / 纯 alias 两臂同断言（AC-4） | 新增 |
| `verify_class_key_positions` | 异常 | 深层与模块位（跨模块组合，入口 = 写闸 `validate_schema`）：`properties.<k>` 内直接写新键 ⇒ ⑤ 形态白名单 `Err`（既有文案）；`properties.<k>.x-function` 内写新键 ⇒ ⑨ 落点单趟 `Err` 点名 `properties.<k>.x-function`；`$defs.T.properties.m.x-static`（条目内更深层）/ `items` 内 / `x-static.open.x-static`（嵌套两键）⇒ 各 `Err` 点名 JSON pointer（AC-4） | 新增 |
| `verify_class_key_positions` | 边界 | 递归体退化输入：`{}`（无 `$defs`）、`$defs` 值为标量 / 数组、`$defs: {}` 空表、`$defs` 键超长（1200 chars）⇒ 零 panic、`Ok`；`x-class: "true"`（字符串）条目 ⇒ 不算类条目，其顶层新键按非法落点 `Err` | 新增 |
| `verify_top_level_keys` | 正向 | 六键齐备 + 可选键（`$schema` / `$comment`）在场与缺席两臂 ⇒ `Ok`；五项 `x-` 面文案只在错误分支，合法顶层零误伤（既有 `顶层键封闭` 正例面逐字保留） | 修改 |
| `verify_top_level_keys` | 异常 | 顶层新 `x-` 键：`x-note` / `x-constructor` / `x-static` ⇒ 各 `Err`，文案含封闭六项与 **`x-` 面五项逐名**（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`）；既有额外键集（`additionalProperties` / `definitions` / `type`）断言逐字保留（AC-4） | 修改 |
| `verify_top_level_keys` | 边界 | 顶层值非对象（`[]` / `"x"` / `42` / `null`）⇒ `Err`「顶层必须是 JSON 对象」；四键缺一（`$id` / `title` / `$defs` / `properties`）⇒ 各 `Err` 点名缺键；`$defs` / `properties` 非对象 ⇒ `Err`（既有形态逐字不变） | 回归 |
| `validate_schema` | 正向 | 合法「类 + ctor + static」整链（`schema_text` 造全文，含 `$id` / `title` / 镜像 `$ref`）⇒ `Ok`，且 `Contract` 投影的 `ops` / `types` / `reexports` 与补两键前逐字段相等（新键不进模块 op 投影，AC-1 的投影面） | 新增 |
| `validate_schema` | 异常 | 缺骨架 schema 经写闸 / 装载同一入口（跨模块组合，入口 = 写闸）：`validate_schema` ⇒ `Err(ApiError::Contract)`，文案带文件前缀（`locate`）且与 `verify_defs` 单点同源；源码锚：`validate_schema` 体内闸调用序 ①–⑧ + `contract::verify` + `verify_defs`，**闸数仍九**（零第十道闸） | 新增 |
| `validate_schema` | 边界 | 空壳（`$defs {}` + `properties {}`）⇒ `Ok`；`$defs` 含 100 个类条目（每类带 `"type": "object"` 且 `properties` 有镜像 `$ref`，超大文档）⇒ `Ok` 且不 panic；`x-class` 条目带 ctor 但其 `params[].name` 非法 ⇒ 名字闸（`contract::verify`）先报，`verify_defs` 不再报（跨函数分工可红绿） | 新增 |
| `verify_defs` | 回归 | 既有夹具随动（AC-11）：`:797` / `:806` / `:835` / `:881` / `:1757` 五处 `"x-class": true` 补 `"type": "object"` 后仍断言同一原因（`properties` / `required` / 非函数属性）；`:813` 的 `bare`（`{"$defs":{"Gate":{"x-class":true}}}`）由 `verify_defs(&bare).is_ok()` **翻为负例**——断言 `Err` 且文案点名 `type` 与「补 `\"type\": \"object\"`」的出路 | 修改 |
| `verify_defs` | 回归 | 跨文件夹具随动（跨模块组合，入口 = 写闸判据）：`emit/mod_test.rs:427` / `:529` 与 `api/mod_test.rs:1748`（`Class` / `Both`）补 `"type": "object"`；后者补齐后仍断言 `class` 计数与 `categories` 口径逐字不变（`def_shape` 判序零改动，AC-1 / AC-11） | 修改 |
| `verify_defs` | 边界 | 特殊字符与超长值：`type` 含 `\n` / `\0` / emoji、键名 emoji、`x-static` 成员名含空格或连字符（形态合法但名字族非法——本文件不判，交 `contract::verify`）⇒ 本文件零 panic、零误判；`degraded` 词表扫描仍排在落点单趟之后（取值校验顺序不变） | 新增 |
| `validate_schema` | 回归 | 源码锚：模块头与 `validate_schema` 的闸 ⑨ doc 同批改为「五条 `$defs` 规则 + `x-class` 骨架四判据 + 两键形态与落点」；闸序 ①–⑧ 描述逐字不变；`X_FUNCTION` 旁两条别名常量（引用 `super::contract`）在场 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 手写 JSON（`json!` / `schema_text` 全文构造）+ 真实 `serde_json` 解析 + 真实 `Contract::from_schema` 投影 + 真实 `paths::ref_relative`；`verify_defs` / `verify_top_level_keys` / `validate_schema` 均为纯值入参，零替身 | 本节全部用例 |
| 源码锚（非 mock） | `include_str!("schema.rs")` / `include_str!("../emit/mod_test.rs")` 等只读取真实源文件字节 | 闸数 / doc / 夹具随动断言 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs

#### 待测功能

- `verify():` 名字趟把 `x-static` 名并入「方法名」族（族别数仍七）；三条预留名判据（`x-static` 名 = `new` / `properties` 键 = `new` / `x-static` 名 ∩ `properties` 键）；落点闸二处 → 四处
- `X_CONSTRUCTOR():` 新增常量 `pub(crate) const X_CONSTRUCTOR: &str = "x-constructor"`——协议词汇单点持有者，禁散落字面量
- `X_STATIC():` 新增常量 `pub(crate) const X_STATIC: &str = "x-static"`——同上
- `methods_of()（内部接线面）:` 收集面扩为「`properties` 方法 + 构造器（名字固定 `new`）+ `x-static` 成员」，仅当条目 `x-class: true`；函数名 / 签名 / 调用点数不变
- `verify_class_reserved_names()（内部接线面）:` N-1 / N-2 / `x-static` 名 = `new` 三条文件级自洽判据
- `verify_x_function_positions()（内部接线面）:` 落点二处 → 四处；非 `x-class` 条目整条跳过（落点归 `verify_defs`）
- `x_function_position_error()（内部接线面）:` 文案「二处」→「四处」并列出四条 pointer 形态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `verify_x_function_positions` | 正向 | 四处合法面：模块 op 顶层签名 + `$defs.T.properties.m.x-function` + `x-class` 条目的 `x-constructor.x-function` + `x-static.<名>.x-function` ⇒ 全 `Ok`；`x-static.open.returns.$ref` 指向本类型（工厂）⇒ `Ok`；四种 pointer 形态下零错误文案（AC-4 正例面） | 新增 |
| `verify` | 正向 | 合法契约整闸通过：七族名字（含 `x-static` 名）/ 三条预留名 / `required ⊆ properties` 全过、四处落点合法（含 ctor + static）⇒ `Ok`；公开面零新增导出（AC-4 / AC-6 正例面） | 新增 |
| `verify_x_function_positions` | 异常 | 合法位之外的落点：`items` 内 / `additionalProperties` 内 / `oneOf` 分支内 / `x-function` 自身子树（`x-static.open.x-function.params[0].schema.x-function`）/ `x-constructor.x-function.returns.x-function` ⇒ 各 `Err` 点名 `JSON pointer` 与定形族别串 `x-function 落点` | 新增 |
| `verify_x_function_positions` | 边界 | 非类条目整条跳过（跨函数分工，入口 = 名字 / 落点闸）：`{"Plain":{"type":"object","x-constructor":{"x-function":{}}}}` 经 `verify` ⇒ `Ok`（落点错不抢先）；同一 schema 经 `verify_defs` ⇒ `Err`——两函数分工可红绿，`verify` 内零骨架复核（D-2） | 新增 |
| `verify` | 异常 | `x-static` 名族（并入「方法名」）：`open-it` / `""` / `🎯` / `带 空格` / `带\n换行` ⇒ 各 `Err` 点名「方法名」族与该名字；`a`.repeat(1200)（超长合法 ASCII）⇒ `Ok`（str 边界 4 + 正例 1，族别数仍七） | 新增 |
| `verify` | 异常 | `x-static` 成员签名族：`params[].name` 为非法标识符 ⇒ `Err`（④ 族）；`owner` 非法 ⇒ `Err`（⑥ 族）；`description` 任意文本 ⇒ `Ok`（自由形状） | 新增 |
| `methods_of` | 正向 | 收集面覆盖：`x-class` 条目含 `properties.m` + `x-constructor` + `x-static.{open, default_path}` ⇒ 四个名字（`m` / `new` / `open` / `default_path`）全入 ③ 族；全部合法 ⇒ `Ok`；`x-class: false` 条目的 `properties` 方法仍入族（既有形态） | 新增 |
| `methods_of` | 异常 | 超大与非类：`x-class` 条目 `x-static` 100 成员 + 1 个非法名 ⇒ 恰报那一个；非类条目带 `x-static`（落点非法）⇒ `methods_of` 零收集、`verify` 的落点由 `verify_defs` 报（MUST NOT 名字错抢先）；`x-constructor` 无 params ⇒ `new` 仍入族 | 新增 |
| `methods_of` | 边界 | 空面：无 ctor / 无 `x-static` / `x-static: {}` / `x-class: false` ⇒ 收集面零新增；超长合法名（1200 chars）仍入 ③ 族；`properties` 键与 static 名同名时两路收集都可见（供 N-2 判据取数） | 新增 |
| `verify` | 边界 | 空面：`$defs: {}` / 条目无 ctor / 无 `x-static` / `x-static: {}` ⇒ `Ok`；`x-class: "true"`（字符串）条目 ⇒ 不触发三条预留名判据；`properties` / `$defs` 缺省 ⇒ `Ok`（投影为零） | 新增 |
| `verify_class_reserved_names` | 正向 | 无冲突面：`x-static.{open, default_path}` 名与 `properties` 键互不重名、皆非 `new` ⇒ `Ok`；`x-constructor` 在场不占名字槽位（名字固定 `new`、不入名字族）⇒ 不触发三条判据 | 新增 |
| `verify_class_reserved_names` | 异常 | `x-static` 名 = `new` ⇒ `Err` 点名保留名 + 出路「构造入口用 `x-constructor`」；与 `x-constructor` 并存时仍报该错（构造器在场不豁免）；同时 `properties.new` 在场 ⇒ 同一函数内首报 `x-static` 侧（判序固定） | 新增 |
| `verify_class_reserved_names` | 异常 | N-1：`x-class` 条目 `"properties": {"new": {"x-function": {}}}` ⇒ `Err` 点名 `new` 保留给 `x-constructor`；非 `x-class` 条目 `properties.new`（函数指针字段）⇒ `Ok`（射程边界：条目面与模块面判据分立） | 新增 |
| `verify_class_reserved_names` | 异常 | N-2：同条目 `properties.open` 与 `x-static.open` 并存 ⇒ `Err` 点名重名方法与「改名或合并」出路；另一条目的 `x-static.open` 与本条目 `properties.open` ⇒ `Ok`（文件级自洽、零跨元素） | 新增 |
| `verify_class_reserved_names` | 边界 | 缺省面：无 ctor / 无 `x-static` / `x-static: {}` ⇒ `Ok`（`Optional` 缺省）；`x-class: false` 条目带 `properties.new` + `x-static` ⇒ 不触发三条判据（非类条目走落点错路径） | 新增 |
| `X_CONSTRUCTOR` / `X_STATIC` | 正向 | 常量值与消费点：两常量逐字 `"x-constructor"` / `"x-static"`；`schema.rs` / `emit/rust.rs` / `emit/ts.rs` 经常量名（或 `schema.rs` 别名）引用，源码含 `X_CONSTRUCTOR` / `X_STATIC` 标识符（D-1） | 新增 |
| `X_CONSTRUCTOR` / `X_STATIC` | 异常 | 近形键不误判：`$defs` 顶层写 `x-construct` / `x-Statics` / `x_constructor`（近形 / 大小写 / 下划线变体）⇒ 不命中新键判据、不产新键渲染、不新增错误面（当作普通未知键由既有闸处置） | 新增 |
| `X_CONSTRUCTOR` / `X_STATIC` | 边界 | 单点持有形态：`contract.rs` 内两常量各恰 1 处 `pub(crate) const … &str` 定义；`schema.rs` / 两渲染器内零**带引号字面量** `"x-constructor"` / `"x-static"`；不为 `x-class` 补常量（既有散落面不新增，源码锚） | 新增 |
| `x_function_position_error` | 正向 | 四处合法位 ⇒ 该错误构造函数零产出（`verify` 返回 `Ok`），合法位零 `JSON pointer` 文案 | 新增 |
| `x_function_position_error` | 异常 | 文案面：定形族别串 `x-function 落点` + `JSON pointer` + 「封闭四处」+ 四条 pointer 形态 + 出路「去掉标记层，或把该形状提升为具名 `$defs` 条目」（AC-4） | 新增 |
| `x_function_position_error` | 边界 | 四种 pointer 形态文案：`properties.<k>` / `$defs.<T>.properties.<k>` / `$defs.<T>.x-constructor` / `$defs.<T>.x-static.<名>` 各产出可读 pointer（数组下标 `[0]` 形态亦在案）；单测经 `verify` 的错误文案观测（私有函数零直调、零新增导出） | 新增 |
| `verify` | 回归 | 文案与体量随动：`x_function_position_error` 的「封闭二处」→「封闭四处」并列四条 pointer 形态；模块头与落点闸两处 doc 随动；`md解析器整体退场` 的体量上限 `≤ 500` → **`≤ 640`**（D-10），退役词集逐字不变（AC-12） | 修改 |
| `verify` | 回归 | 公开面与计数零回归：`pub fn` 恰 3、`function_param_names(` 恰 3、`methods_of(` 恰 2、`is_constructor` / `CONSTRUCTOR_NAMES` / `CONSTRUCTOR_PREFIXES` 逐字不变；落点守卫自 `verify` 体内可达（既有 reachability 断言与定形族别串断言逐字保留） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `verify` 吃「投影 + schema 值」两个纯值入参；`Contract` 由真实 `Contract::from_schema` 投影、schema 由手写 `json!` / `mirrored_schema_text` 给；零落盘、零读盘、零替身 | 本节全部行为面用例 |
| 源码锚（非 mock） | `include_str!("contract.rs")` / `include_str!("schema.rs")` / `include_str!("emit/rust.rs")` / `include_str!("emit/ts.rs")` 只读真实字节 | 常量单点、公开面计数、文案与体量断言 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs

#### 待测功能

- `render():` `x-class` 条目的 trait body 增构造器与静态方法；零字段类零 `impl` / 零见证；object 条目产物逐字节不变
- `trait_block()（内部接线面）:` `properties` 方法 → 构造器（`fn new(<params>) -> Self;`）→ `x-static` 成员（`fn <名><method_signature>;`）
- `param_list()（内部接线面）:` 入参序列（`name: T` 逗号连接）的唯一拼装点，构造器与 `method_signature` 共用
- `method_signature()（内部接线面）:` 改为复用 `param_list`（输出逐字不变）
- `is_zero_field_object()（内部接线面）:` 判据增 `x-class != true`（不改名，语义 = 是否由生成器自动产出零参 `new`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `render` | 正向 | 构造器正例（AC-2）：`{"type":"object","x-class":true,"properties":{"count":{"x-function":{}}},"x-constructor":{"x-function":{"params":[{"name":"path","schema":{"type":"string"}}]}}}` ⇒ trait body 恰 `    fn new(path: &str) -> Self;`；全文**零** `impl Store`、**零** `const _: fn() -> super::Store` | 新增 |
| `render` | 正向 | 静态方法正例（AC-2）：`x-static` 两成员（`open` 返回本类型 `$ref`、`default_path` 返回 `string`）+ ctor + `properties.count` ⇒ body 行序 properties 方法 → `fn new(…)` → `fn default_path() -> String;` → `fn open(path: &str) -> Store;`（成员序 = `serde_json::Map` 字典序，D-5 / Q-1）；整块逐字断言 | 新增 |
| `render` | 边界 | 构造器零参与大入参：`params` 缺省 ⇒ `fn new() -> Self;`；`params: []` ⇒ 同；单参 ⇒ `fn new(path: &str) -> Self;`；100 参（list 超大）⇒ 一行内逗号连接、零尾逗号、零换行（list 边界四臂） | 新增 |
| `render` | 边界 | 静态方法形态边界：`returns` 缺省 ⇒ `fn m();`（无 `->`）；`async: true` 沿用既有 `method_signature`（Rust 侧不改产物形态，取值只作留痕）；成员名 `a`.repeat(1200) 合法 ASCII ⇒ 行内含该名；成员名含 emoji ⇒ 装载期已死（渲染器结构上收不到） | 新增 |
| `trait_block` | 正向 | 三段拼装逐字：`properties` 方法 + ctor + `x-static` 的缩进（4 空格）、行序与分号形态逐字；`params[].name` 缺省回退 `arg`、参数 `schema` 缺省回退 param 自身（既有形态） | 新增 |
| `trait_block` | 异常 | 渲染期零判据：直接把 `{x-class:true, properties:{"count":{"type":"integer"}}}` 喂 `render`（绕过写闸）⇒ `count` 行被跳过、零 panic、零 `Err`（`render` 返回 `String`）；源码零新增判错文案（错误面只在 `schema.rs`） | 新增 |
| `trait_block` | 边界 | 零成员三类：`properties: {}` + 无 ctor / static ⇒ 恰 `pub trait Gate {\n}\n`；有 ctor 无 props ⇒ body 只有 `new` 行；有 static 无 props ⇒ body 只有 static 行 | 新增 |
| `is_zero_field_object` | 正向 | 空 trait 不产 `impl`（AC-7）：`{"type":"object","x-class":true,"properties":{}}` ⇒ `render` 只出 `pub trait Gate {\n}\n`，零 `impl Gate {`、零 `const _: fn() -> super::Gate`；源码锚：`is_zero_field_object` 体内含 `x-class` 排除、调用点（`constructible` 收集）恰 1 处 | 新增 |
| `is_zero_field_object` | 异常 | 非命中形态：入参为字符串 / 数组 / `null` / `$ref` alias / 有字段 `object` ⇒ `false`（零 panic、零误产 `impl`）；`type: "object"` + 空 `properties` 时 `x-class` 缺席 ⇒ `true`、`x-class: false` ⇒ `true`、`x-class: true` ⇒ `false`（bool 三臂：true / false / 缺省） | 新增 |
| `is_zero_field_object` | 边界 | 良性对照（AC-8）：`{"type":"object"}` / `{"type":"object","properties":{}}` / `{"type":"object","additionalProperties":{"type":"string"}}`（三者均无 `x-class`）⇒ 仍产 `impl T {\n    /// 构造器…\n    pub fn new() -> Self {\n        Self\n    }\n}` 与 `const _: fn() -> super::T = super::T::new;` | 新增 |
| `param_list` | 异常 | `params` 值非数组（标量 / 对象 / `null`，绕过写闸直喂）⇒ 视为缺省出 `()`、零 panic（渲染期零判据） | 新增 |
| `param_list` | 正向 | 入参拼装：`name: T` 以 `, ` 连接、`params` 缺省即空串；参数 `schema` 缺省回退 param 自身（`{"name":"raw"}` ⇒ `raw: serde_json::Value` 一类既有兜底）；`name` 缺省回退 `arg` | 新增 |
| `param_list` | 边界 | `params` 四臂：缺省 ⇒ `()`；`[]` ⇒ `()`；单元素 ⇒ `(path: &str)`；100 元素 ⇒ 逗号连接（list 空 / 单元素 / 超大 / None 四边界） | 新增 |
| `method_signature` | 正向 | 四形态逐字不变：无参无返回 ⇒ `()`；有返回 ⇒ `(input: &str) -> bool`；`returns` 判出 `()` ⇒ `(…)`（不追加 `-> ()`）；`returns` 缺省 ⇒ `(…)` | 新增 |
| `method_signature` | 异常 | 非法 function 值（标量 / `null` / `params` 为对象）⇒ 输出 `()` 与 `arg: …` 兜底、零 panic（非法形态在装载期已死，渲染器不给第二份判据） | 新增 |
| `method_signature` | 边界 | `returns` 判出 `()` 的折叠臂（不追加 `-> ()`）与 `returns` 缺省臂各一；`params` 100 元素 ⇒ 逗号连接零尾逗号 | 新增 |
| `method_signature` | 回归 | 抽段不改语义：`function_pointer` 的 `fn(…) -> R` 形态逐字不变；`param_list` 为唯一入参拼装点（源码锚：`params` 遍历点在 `param_list` 内 1 处）；`value_type` 的 `x-function` 零分支不变量与两处读取点断言逐字保留 | 修改 |
| `render` | 异常 | 非法形态不可达：`x-class` 条目带 `type: "string"`（绕过写闸直喂 `render`）⇒ 仍按 trait 渲染、不 `Err`；`x-constructor` 带 `returns` ⇒ 渲染器不读取、不报错（装载期已禁）；测试以 `verify_defs` 复现拦截面（跨模块组合，入口 = 装载写闸） | 新增 |
| `render` | 回归 | 非类条目产物逐字节不变（AC-1 / AC-8）：零字段 object / 有字段 object / `enum` / `oneOf` / 标量 / 纯 alias / 空 `$defs` 七形态的整段文本与既有断言逐字相等；「同一 schema 两次渲染逐字相等」幂等断言保留 | 回归 |
| `render` | 回归 | 夹具随动（AC-11）：`:345` 的 `Runner` 与 `:363` 的 `Marker` 补 `"type": "object"` 后 `Marker` 仍断言 `pub trait Marker {`（带骨架的合法空 trait）；`:1325` 的测试 doc「合法命中点恒 3」随 `trait_block` 两处新读取点改写（留痕说明，非判据；`value_type` 体内恰 0 与两读取点在场两硬断言逐字保留） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 手写 JSON 经真实 `Contract::from_schema` 投影；`render` 纯出 `String`（零 IO / 零时钟 / 零子进程）；`ApiMeta` 以真实 `meta()` 构造；`EmitTarget` 以真实字段字面量构造（空查表 + 目录 / crate 形态） | 本节全部用例 |
| 源码锚（非 mock） | `include_str!("rust.rs")` 与 `function_body` 逐函数体切片 | `is_zero_field_object` 判据、`param_list` 单点、`value_type` 零分支 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs

#### 待测功能

- `render():` `x-class` 条目在 ctor 或 `x-static` 至少一项在场时追加同名值声明；两者皆无时零值声明（产物与今天逐字节相同）；object 条目零值声明
- `interface_block()（内部接线面）:` interface body 之后按条件追加 `export declare const T: { … };`（块形态、紧跟 `}` 零空行、成员 2 空格缩进）
- `parameter_list()（内部接线面）:` 入参序列唯一拼装点（含 `optional → T | undefined`）
- `return_text()（内部接线面）:` 返回位文本唯一拼装点（含 `async → Promise<R>`）
- `function_type()（内部接线面）:` 改为 `({parameter_list}) => {return_text}`，整签名 `common_type` 包装仍只在此函数

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `render` | 正向 | 值声明正例（AC-3）：`Store` 条目（`count` 方法 + `x-constructor` + `x-static.{open, default_path}`）⇒ `export interface Store { … }` 之后紧跟 `export declare const Store: {`、`  new (path: string): Store;`、`  default_path (): string;`、`  open (path: string): Store;`、`};`，成员序 `new` → 字典序 static，interface `}` 与值声明之间零空行；整块逐字断言 | 新增 |
| `render` | 正向 | 单项产出：仅 `x-constructor` ⇒ 值声明只有 `new` 一行；仅 `x-static` ⇒ 值声明无 `new`、逐成员一行；类型 / 值同名共存形态（`export interface T` + `export declare const T`） | 新增 |
| `render` | 边界 | 零值声明三臂（AC-3）：无 ctor / static 的 `x-class` ⇒ 零 `export declare const`；任何 object 条目（有字段 / 零字段 / 带 `additionalProperties`）⇒ 零值声明；`x-static: {}` + 无 ctor ⇒ 零值声明（「非空」判定而非「键在场」） | 新增 |
| `render` | 边界 | 成员签名边界：ctor `params` 缺省 ⇒ `new (): Store;`；100 参 ⇒ 一行内逗号连接、零尾逗号；成员名 `a`.repeat(1200) ⇒ 行内含该名；`returns` 缺省 ⇒ `: void`（list / str 边界） | 新增 |
| `parameter_list` | 异常 | `params` 值非数组（标量 / 对象 / `null`）⇒ 空串、零 panic；参数项缺 `schema` 且缺 `type` ⇒ `unknown` 兜底（渲染期零判据） | 新增 |
| `parameter_list` | 正向 | 参数面：`optional: true` ⇒ `path: string \| undefined`（与字段面 `?` 分立）；`params[].name` 缺省回退 `arg`；参数 `schema` 缺省回退 param 自身 | 新增 |
| `parameter_list` | 边界 | `params` 四臂：缺省 ⇒ 空串；`[]` ⇒ 空串；单元素 ⇒ `path: string`；100 元素 ⇒ 逗号连接（空 / 单元素 / 超大 / None） | 新增 |
| `return_text` | 异常 | `returns` 为非法值（标量 / `null`）⇒ `unknown` 兜底、零 panic；`async` 值非布尔 ⇒ 不算异步（只认 `true`） | 新增 |
| `return_text` | 正向 | 返回面：`async: true` ⇒ `Promise<R>`；`returns` 缺省 ⇒ `void`；`returns` 为 `$ref` ⇒ 末段名；显式 null ⇒ `T \| null` 至多一次 | 新增 |
| `return_text` | 边界 | 映射规则内容位（D-7）：`returns` 为 `{"outcome": {"type":"string"}}` ⇒ 经 `value_type(returns, Return)` 按 `common.json` 表展开；**整签名包装**（`common_type(function, Return)` 的 `x-function.<承载键>` 形态）不套到值声明成员——同一函数对象在 interface 成员位套包装、在值声明位只展开 `returns` | 新增 |
| `function_type` | 正向 | 方法成员形态：`(input: string) => boolean`（整签名包装未命中）；零参零返回 ⇒ `() => void` | 新增 |
| `function_type` | 异常 | `common_type` 命中但本语言槽位为空 ⇒ 回退普通签名（不产空包装）；函数对象为标量 / `null`（绕过写闸直喂）⇒ `() => void` 兜底、零 panic | 新增 |
| `function_type` | 边界 | 可选参（`T \| undefined`）/ `async` ⇒ `Promise<R>` / 100 参三臂 | 新增 |
| `function_type` | 回归 | 输出逐字不变：`({parameter_list}) => {return_text}` 四形态（无参无返回 / 可选参 / `async` / `common_type` 命中整签名）与既有断言逐字相等；`common_type` 的唯一调入口仍在 `function_type` | 修改 |
| `interface_block` | 正向 | interface 面零改动：`x-class` ⇒ `export interface T { … }` + degraded 注释逐字保留；方法成员 `run: (input: string) => boolean;`；零字段类 ⇒ `export interface T {}`；body 与值声明之间零空行 | 修改 |
| `interface_block` | 边界 | 零字段类 ⇒ `export interface T {}`；100 成员（超大 body）⇒ 成员缩进 2 空格、body 与值声明之间零空行 | 新增 |
| `interface_block` | 异常 | 渲染期零判据：`x-class` 条目 `properties` 含非函数值（绕过写闸直喂）⇒ 该成员按字段面 `{field}?: T` 渲染、零 panic；`x-constructor` 带 `returns` / `async: true` ⇒ 不被读取、不报错（装载期已禁）；源码零新增判错分支 | 新增 |
| `interface_block` | 回归 | 精确计数断言：`ts.rs` 内 `get("x-function")` 文本出现次数恒 **2**（`interface_block` + `function_of`），值声明成员读取只走 `function_of`（零第三处）；`fn interface_block(` / `fn function_type(` / `fn function_of(` 三函数名与入口形态不变 | 回归 |
| `render` | 异常 | 负例面（AC-3 的「零产出」侧）：object 条目（有字段 / 零字段）与无新键的 `x-class` 条目 ⇒ 全文零 `export declare const`；`x-static` 值写成非对象（绕过写闸直喂）⇒ 渲染器不产该成员、零 panic | 新增 |
| `render` | 边界 | 幂等与零 diff：同一 schema 两次渲染逐字相等；object / 方法面 / 标量 / `enum` / `oneOf` 五形态整段文本与既有断言逐字相同（AC-1） | 回归 |
| `render` | 回归 | 夹具随动（AC-11）：`:283` / `:1080` / `:1839` 三处 `x-class` 补 `"type": "object"`；TS 渲染不读 `type` ⇒ 三处既有断言逐字不变（`同一schema两侧形态收敛` 的 Rust 侧也同批保留） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 手写 JSON 经真实 `Contract::from_schema` 投影；`render` 纯出 `String`；真实 `paths::ts_import_specifier` 与真实 `ApiMeta`；`EmitTarget` 真实字段字面量（空查表 + 目录 / 单文件形态） | 本节全部用例 |
| 源码锚（非 mock） | `include_str!("ts.rs")` + `function_body` 切片 | `get("x-function")` 恰 2 处、三函数入口形态、零 `eprintln!` |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/check.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs

#### 待测功能

- `check():` `apiConstructorDeclared` 只在**模块级** `properties` 命中构造器名；判据词汇十值 / `category` 七值 / 求值序 / `category_of` / `contract.ops` 投影零改动
- `constructor_declared()（内部接线面）:` 文案与口径收窄为「模块级」，出路指向 `$defs.<T>.x-constructor`；文案零 `x-class` 字面

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `constructor_declared` | 正向 | 模块级构造器名仍判：`properties.new`（带 `x-function`）⇒ 恰一条 `apiConstructorDeclared`，`target = Some("new")`、`category = "constructor"`，文案含「模块级」与出路「`$defs.<T>.x-constructor`」（既有 `构造器声明判据` 逐字保留 + 文案断言新增） | 修改 |
| `check` | 正向 | 类型级构造器零判据（AC-9）：`$defs.Store` 带 `"type":"object"` + `"x-class":true` + `x-constructor`（`params` 非空）+ 一条返回本类型的 `x-static`，模块级 `properties` 无构造器名 ⇒ 零 `apiConstructorDeclared`、零 `apiMissing`（`$defs` 条目不是模块导出比对对象）；`ALL.len()` 仍 10、`category` 仍七值 | 新增 |
| `check` | 边界 | 两面分离：同一元素模块级 `new` + `$defs.Store.x-constructor` 并存 ⇒ 恰一条，`target` 点名模块级 `new`（`$defs` 侧不在射程）；`$defs` 侧命名的 `new` 走 `verify` 的 N-1 `ApiError`（跨模块组合，入口 = 判据面 `check`，装载期错面见 `contract_test`） | 新增 |
| `check` | 边界 | 无构造通路零判据零警告（AC-9，跨模块组合，入口 = 判据面 `check`）：`x-class` 条目既无 `x-constructor`、也无返回自身 `$ref` 的 `x-static`、且模块内无 op 的 `returns` 指向它 ⇒ `check` 零 api 条目（`docMissing` 等按夹具折算）；源码锚：`emit/mod.rs` 零 `x-constructor` / `x-static` 字面、`warn_missing_x_class` 对 `x-class` 条目早退（零警告） | 新增 |
| `constructor_declared` | 异常 | 文案面：零 `x-class` 字面（源码自证断言保留）；含「模块级」与 `$defs.<T>.x-constructor`；构造器封闭集逐值仍命中——`default` / `build` / `from_str` / `with_capacity` / `empty` / `never` / `open` 各一臂 ⇒ 各一条且 `target` 点名首个（`is_constructor` 零改动） | 修改 |
| `constructor_declared` | 边界 | `new` 在 `from_str` 之前 / 之后两臂 ⇒ 均只一条且 `target` 取首个命中的构造器（`contract.ops` 投影序）；零构造器 ⇒ `None`（不产条目） | 新增 |
| `check` | 异常 | 首错短路不变：`apiMissing` + `apiSignatureMismatch` + `apiConstructorDeclared` 同场 ⇒ 只留 `apiMissing`；`api = none` 支两步（`apiUnexpected` → `docMissing`）与 pair 锚点序逐字不变 | 回归 |
| `check` | 回归 | 词汇面零扩张：`ArchiApiFindingKind::ALL.len() == 10`、`as_str` / `severity()` 逐值对拍、`category` 七组各自非空（`constructor` 仍归 `apiConstructorDeclared`）、`REPORT_VERSION` 不变；`category_of` 仍 `pub fn` 且单点 | 回归 |
| `check` | 回归 | 源码锚：`check.rs` 内零 `x-class` 字面；`constructor_declared` 零新参数 / 零新调用点（调用点数不变）；`contract.ops` 投影仍是唯一取数面（零 `$defs` 成员比对） | 回归 |
| `check` | 边界 | 空 / 规模输入：零 op 契约 ⇒ 零条；`file_stem` 一类非构造器名 ⇒ 零条；`$defs` 含 100 条目 ⇒ 判据计数不变（`$defs` 规模不进判据面）；元素 `path` 为空 ⇒ 早退零条（既有） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 判据事实源全真实：手写 schema 经 `Contract::from_schema` 投影、真实 `extract` 符号集、真实 `paths::entry_state`；文件系统只用于驱动入口 / 契约路径存在性 | 全部 `check` 行为面用例 |
| 临时目录（进程边界，真件） | std 自建 `TempDir`（`AtomicU64` 唯一命名，沿既有装置）真实落 `src/lib.rs` / schema 文件；不用替身、不捕获 stderr | 需入口 `Resolved` 的 `check` 用例 |
| 源码锚（非 mock） | `include_str!("check.rs")` / `include_str!("emit/mod.rs")` 只读真实字节 | 零 `x-class` 字面、零警告路径、无新调用点 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（archi-knowledge.md 是编译期内嵌的静态资产，无函数面）；变更内容见 design C6 / D-9，断言经 archi_knowledge_test.rs 的 include_str! 同一份字节表达。 -->

- `knowledge():` 唯一入口（`archi_knowledge.rs:20` `pub(crate) fn knowledge() -> Option<String>`，= `build(SKELETON).ok()`）；`build` / `foundation::layout` 派生零改动——空骨架 / 含 `<>` 占位 ⇒ `None`（降级），真实骨架 ⇒ `Some`（不降级），全文以骨架 `trim_end()` 起手追加两处派生目录段
- `骨架文本（archi-knowledge.md 静态面，非函数）:` 编译期内嵌字节的「三条不可越界」② `x-` 面五项逐名（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`，+31 B 全在骨架）；四数体量锚——骨架 `lf_bytes(trim_end)` **1808 B**（≤ 1831）、全文 **1997 B**（≤ 2031）；两处 needle 集同批扩五项，删任一项即红

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 骨架文本 | 正向 | 五项逐名在场（AC-10）：「三条不可越界」② 那一行含 `` `x-function` `` / `` `x-class` `` / `` `x-common` `` / `` `x-constructor` `` / `` `x-static` ``（带反引号原样）；三条不可越界各至少一处；「空 schema」/「不要编造导出」/「历史 fence 读面不读、MUST NOT 新增」在场（`三条不可越界逐条在场` 的 needle 集由三项扩五项） | 修改 |
| 骨架文本 | 边界 | 枚举形态：五项以「 / 」逐名连接、零缩写 / 零「等」/ 零「见 spec」类指路；不带反引号的裸词不计入（needle 带反引号原样）；`x-` 面以外零新协议词（`x-interface` / `x-nominal` / `x-singleSource` 仍零命中） | 新增 |
| 骨架文本 | 异常 | needle 守卫可红绿：「指令表收窄且新行集可判」的 needle 集补 `` `x-constructor` `` / `` `x-static` ``（删任一项即测试红）；既有 `orphanHierarchy` / `entry` / `doc` / 字段序四条退役词仍零命中 | 修改 |
| `knowledge()` | 正向 | 体量实测（AC-10）：骨架 `lf_bytes(trim_end)` `assert_eq!` **1808** 且 `<= 1831`；全文 `assert_eq!` **1997** 且 `<= 2031`；余量 23 B / 34 B 由两条 `<=` 门限表达 | 修改 |
| `knowledge()` | 异常 | 空骨架 `""` / 空白骨架 ⇒ `Err`「知识骨架为空」；含 `<>` 占位 ⇒ `Err`（既有文案）；真实骨架 ⇒ `Some`（不降级）——降级语义零改动 | 回归 |
| `knowledge()` | 边界 | 增量口径：全文增量 == 骨架增量（+31 B，增量全在骨架）；全文以骨架 `trim_end()` 起手、派生段为追加；两处 `foundation::layout` 目录逐字在场、派生段零 `INDEX.toml` 串 | 新增 |
| 骨架文本 | 异常 | 判据面事实不进骨架：零「构造通路」/「至少一条」/「object 等同 JSON 对象」类表述；零 `<>` 占位符；零示例围栏 / 零 `[element.` 前缀 / 零值域行 / 零结构诊断词（既有四条零残留纪律逐字保留） | 回归 |
| `knowledge()` | 回归 | 模块头改「骨架 ≤ 1831 B / 全文 ≤ 2031 B」；`骨架与全文双门限冻结` 的测试 doc 改「抬锚须显式豁免并同批更新四数」；两条 `assert_eq!` 失败文案写死 1808 / 1997——源码锚：测试文件内零「≤ 1800 B」/「1777」/「1966」残留 | 修改 |
| 骨架文本 | 回归 | 零变化面：五段结构标题 / 工具面指路（句内 `archi_*` 全在 `mount::registry()`）/ 手改出口三要素 / 留给判断 + 兼容句 / 拼装与降级语义（空骨架 `Err`、真实骨架不降级）逐字不变 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `include_str!("archi-knowledge.md")` 取编译期内嵌的同一份字节（禁测试内复制副本）；`knowledge()` 走真实 `build` + 真实 `foundation::layout` 派生；`mount::registry()` 是工具名真值 | 本节全部用例 |
| 源码锚（非 mock） | `include_str!("archi_knowledge_test.rs")` 与 `include_str!("archi_knowledge.rs")` 只读真实字节 | 三处文案随动、模块头断言 |

---

## 不可测试项

| 模块 / 对象 | 原因与承载方式 |
|---|---|
| `packages/desktop/package.json` | 仅 `version` 0.7.18 → 0.7.19 的 patch 递增（+1）与三条命令串不变，无函数面 / 无单测路径；由桌面端门禁与 `git diff` 核对（AC-12）。`src-tauri/Cargo.toml` / `tauri.conf.json` 逐字不变同属机械核对面 |
| `.dev-team/architecture/graph/INDEX.toml`、`**/*.i.rs`、`**/*.i.ts` | 生成物、禁止手改；「11 份在案 schema 重生成后逐字节不变 + `api:check` 退出码 0」由静态门禁链（`api:export` / `api:check`）判定（AC-1）。单测只覆盖渲染器的非类条目零变化（`rust.rs` / `ts.rs` 两节） |
| `packages/desktop/src/types/generated/**`（`bindings.ts` / `bindings.zod.ts`） | 零 DTO / 零命令 / 零枚举面变化 ⇒ `bindings:export` 零 diff 由门禁链判定（AC-12 的零改动面）；无单测路径 |
| `openspec/changes/archi-api-constructor-static-members/specs/**` | 文档面（三份 delta）：三能力目录 / MODIFIED 块整段拷贝 / 每条 Requirement 带 SHALL·MUST 与 Scenario 的形态由评审与静态核对承载（AC-13），无 Rust 测试路径 |
| `openspec/specs/**` 基线、本 change 的 `explore.md` | delta-only 纪律：基线零改动由 `git status` 机械核对；测试不读 `openspec/` 路径（`AGENT.md` 配置隔离纪律），无单测路径 |
| `.dev-team/architecture/**`（模型 / ADR） | 零数据变化（55 元素 / 129 关系 / 表头计数逐字不变）；由门禁链与 `git diff` 核对，无单测路径 |

