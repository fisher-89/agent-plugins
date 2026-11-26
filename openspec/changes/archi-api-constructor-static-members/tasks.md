# 任务: archi-api-constructor-static-members

> 顺序即依赖：前一组解锁后一组。每项以文件为入口，落地口径见 `design.md`（决策 `D-1`…`D-14`）与 `specs/` 三份 delta。
> 测试用例与断言设计归 test-design 相位；本清单只到「实现 + 既有夹具 / 台账的机械随动」。门禁链（`api:export` / `api:check` / `bindings:export` / 静态检查）由桌面端执行，agent 内不重复执行。

## 阶段 1 · 协议词汇与写闸（`crates/core/archi/src/api`）

- [x] `api/contract.rs`：`X_FUNCTION`（`:12`）旁补 `pub(crate) const X_CONSTRUCTOR: &str = "x-constructor"` 与 `pub(crate) const X_STATIC: &str = "x-static"`（doc 一句：`x-` 协议词汇的单点持有者，MUST NOT 散落字面量；D-1）。**不为 `x-class` 补常量**。
- [x] `api/schema.rs`：`const X_FUNCTION`（`:417`）旁补两条别名常量（`super::contract::X_CONSTRUCTOR` / `X_STATIC`）；`verify_top_level_keys`（`:377`）的未知顶层键文案由「`x-` 面只有 … 三项」改为「`x-` 面封闭五项」并逐名列全（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`）；同函数 doc（`:375-376`）同步。顶层封闭六项与四键必在场逐字不变。
- [x] `api/schema.rs`：`verify_defs`（`:481`）的 `x-class` 分支（`:526`）**在既有 `required` 判据之前**插两条 —— ① `definition.get("type").and_then(Value::as_str) != Some("object")` ⇒ `ApiError`（点名 `$defs.<T>.type`、报实际形态、出路「补 `\"type\": \"object\"`」）；② `definition.contains_key("additionalProperties")` ⇒ `ApiError`（布尔形态同拒；两条可执行出路：去掉 `x-class` 变纯数据 + 说明条目层 map 形态本期不渲染、键值对容器写在值位（`x-function.params[].schema` / `returns` / 字段位）；若本意是方法集合则去掉该键、trait 没有实例数据面）。既有 `required` 与「`properties` 全为方法」两条文案**逐字保留**（判序即错误面，D-2）。
- [x] `api/schema.rs`：新增私有 `fn verify_class_members(name: &str, definition: &Map<String, Value>) -> Result<(), ApiError>`，在既有「`properties` 全为方法」判据之后调用 —— `x-constructor` 的值 MUST 是对象、MUST 含对象型 `x-function`、内层 MUST NOT 有 `returns`（出路：返回面由语义固定为本类型，别名构造写 `x-static` 并返回本类型）、内层 `async` MUST NOT 为 `true`（出路：构造器是值而非 future，没有异步构造）；`x-static` 的值 MUST 是对象、每个成员 MUST 含对象型 `x-function`（缺包裹即静默丢）。`params` 可省；**外层另加键不判**（沿用 `x-function` 内层自由形状的纪律）。
- [x] `api/schema.rs`：新增私有 `fn verify_class_key_positions(schema: &Value) -> Result<(), ApiError>`（含一条私有递归体），在 `verify_defs` 的逐条目循环**之后**、`degraded_marks` 扫描（`:554`）**之前**调用 —— 合法 pointer 集合恰为「`$defs.<T>` 且该条目 `x-class: true` 的顶层」；命中非法位（非类 `$defs` 条目顶层 / 条目内更深层 / 模块 `properties` 子树 / `x-function` 内层）即 `ApiError`，点名非法 JSON pointer 与出路「只有 `x-class` 条目可声明构造入口与静态方法」（D-2）。
- [x] `api/schema.rs`：模块头与 `validate_schema` 的闸 ⑨ doc、`verify_defs` 的 doc 同批改写（五条 `$defs` 规则 + `x-class` 骨架四判据 + 两键形态与落点）；闸数仍九、闸序 ①–⑧ 逐字不动。

## 阶段 2 · 名字面与 `x-function` 落点（`crates/core/archi/src/api/contract.rs`）

- [x] `api/contract.rs`：`methods_of`（`:192`）收集面扩为「`properties` 方法 + 构造器（名字固定 `new`）+ `x-static` 成员」，**仅当该条目 `x-class: true`**；函数名、签名、调用点数**不变**（`function_param_names(` 文本计数仍 3、`methods_of(` 仍 2 —— D-3）；doc 改指「③ 族的收集面：`properties` 方法 + 类条目的构造器与静态方法」。
- [x] `api/contract.rs`：新增私有 `fn verify_class_reserved_names(contract: &Contract, name: &str, definition: &Map<String, Value>) -> Result<(), ApiError>`，在 `$defs` 逐条目循环内、`$ref` 末段名检查（`:176-181`）之后调用 —— ① `x-static` 名 MUST NOT 为 `new`（点名保留名 + 出路「构造入口用 `x-constructor`」）；② N-1：`x-class` 条目的 `properties` 键 MUST NOT 为 `new`（同上出路）；③ N-2：`x-static` 名 MUST NOT 与该条目 `properties` 键重名（出路「改名或合并」）。三条都是文件级自洽事实、零跨元素。
- [x] `api/contract.rs`：`verify_x_function_positions`（`:208`）落点由二处扩到四处 —— `properties` 值（既有）、`x-constructor` 值（pointer `$defs.<T>.x-constructor`）、`x-static` 每个成员值（pointer `$defs.<T>.x-static.<名>`）；后两处仅当条目 `x-class: true`，**非类条目整条跳过**（其新键由 `verify_defs` 判落点，MUST NOT 让 `x-function` 落点错抢先）；`scan_x_function` 主体零改动（`allowed = true` 的节点子树仍一律转假）。
- [x] `api/contract.rs`：`x_function_position_error`（`:275`）文案的「二处」改「四处」并列出四条 pointer 形态；`verify` / 落点闸 / 模块头四段 doc 同步；公开面仍恰 3 个 `pub fn`、`is_constructor` / `CONSTRUCTOR_NAMES` / `CONSTRUCTOR_PREFIXES` 逐字不变。

## 阶段 3 · Rust 渲染（`crates/core/archi/src/api/emit/rust.rs`）

- [x] `emit/rust.rs`：`use crate::api::contract::{…}` 补 `X_CONSTRUCTOR` / `X_STATIC`；把 `method_signature`（`:295`）的入参段抽成 `fn param_list(function: &Value, common: &BTreeMap<String, CommonRule>) -> String`，`method_signature` 与 `function_pointer`（`:325`）改用之（**输出逐字不变**，零第二份签名拼装）。
- [x] `emit/rust.rs`：`trait_block`（`:207`）在既有 `properties` 循环之后追加构造器（`format!("    fn new({}) -> Self;\n", param_list(...))`，不读 `returns`）与 `x-static` 成员（`format!("    fn {name}{};\n", method_signature(function, common))`）；成员序按 `serde_json::Map` 迭代序（D-5 / D-14）。
- [x] `emit/rust.rs`：`is_zero_field_object`（`:482`）判据增 `x-class != true`（**不改名**），doc 点明语义「是否由生成器自动产出零参 `new`」；唯一消费点仍是 `render_scope` 的 `constructible` 收集（`:71-75`）与见证段（`:121`）——见证段零新增（D-8 / D-13）。
- [x] `emit/rust.rs`：模块头渲染清单补一句 trait body 的两类新成员；`value_type` 的 `x-function` 零分支不变量与 object 条目产物零改动。

## 阶段 4 · TS 渲染（`crates/core/archi/src/api/emit/ts.rs`）

- [x] `emit/ts.rs`：`use crate::api::contract::{…}` 补 `X_CONSTRUCTOR` / `X_STATIC`；从 `function_type`（`:261`）抽出 `fn parameter_list(function: &Value, common: &BTreeMap<String, CommonRule>) -> String`（含 `optional → T | undefined`）与 `fn return_text(function: &Value, common: &BTreeMap<String, CommonRule>) -> String`（含 `async → Promise<R>`），`function_type` 改为 `({parameter_list}) => {return_text}`（输出逐字不变）。
- [x] `emit/ts.rs`：`interface_block`（`:201`）在 interface body 之后按条件追加同名值声明 —— 条件 `class && (构造器在场 || x-static 非空)`；成员 `new (<params>): T;` 在前、每个 `x-static` 一条 `<名> (<params>): R;`（Map 序）；块形态 `export declare const T: {` + 每成员一行 2 空格缩进 + `};`，紧跟 interface 的 `}` 后零空行。成员读取**只走既有 `function_of`（`:256`）**，`ts.rs` 内 `get("x-function")` 的文本出现次数 MUST 恒为 2（精确计数断言，D-6）。
- [x] `emit/ts.rs`：值声明成员**不套整签名包装**（D-7：`common_type(function, Position::Return)` 仍只留给 `function_type`）；object 条目与「无 ctor / 无 static 的类」零值声明；模块头补一句「值声明只在构造器或静态方法在场时产出」。

## 阶段 5 · 判据口径（`crates/core/archi/src/api/check.rs`）

- [x] `api/check.rs`：`constructor_declared`（`:478`）的文案收窄为「接口定义 `<file>` 的**模块级** `properties` 声明了构造器 `<op>` —— 零字段 `object` 由生成器自动产 `new()`，类型级构造写进 `$defs.<T>.x-constructor`；模块级 `properties` MUST NOT 声明它（否则必然误报实现缺失）」，fn doc 同步；文案 **MUST NOT 出现 `x-class` 字面**（既有源码自证断言，D-4）。判据词汇、求值序、`category_of`、`contract.ops` 投影零改动。

## 阶段 6 · 知识骨架与体量锚（`crates/infra/agent`）

- [x] `archi-knowledge.md`：仅改「三条不可越界」② 的那一行 —— `x-` 面枚举由 `x-function` / `x-class` / `x-common` 改为五项逐名（追加 ` / \`x-constructor\` / \`x-static\``，**实测 +31 B**）；其余段落逐字不动，零「见 spec」类指路（D-9）。
- [x] `archi_knowledge_test.rs`：四数同批 —— 骨架 `assert_eq!` 1777 → **1808**、门限 `<= 1800` → **1831**、全文 `assert_eq!` 1966 → **1997**、门限 `<= 2000` → **2031**（余量仍 23 B / 34 B）。
- [x] `archi_knowledge_test.rs`：三处文案同批 —— 模块头（`:4`）的「体量冻结（骨架 ≤ 1800 B / 全文 ≤ 2000 B）」、`骨架与全文双门限冻结` 的测试 doc（「内容变更须同批更新，MUST NOT 抬锚」→「抬锚须显式豁免并同批更新四数」）、两条 `assert_eq!` 失败文案里写死的实测值与括号说明。
- [x] `archi_knowledge_test.rs`：`三条不可越界逐条在场` 的 needle 集由三项改**五项逐名**（并断言该行不含「见 spec」）；`指令表收窄且新行集可判`（`:233`）的 needle 集补 `` `x-constructor` `` / `` `x-static` ``（带反引号原样）——删任一项即红。**落地口径**：现语料无 `三条不可越界逐条在场` 用例（归 test-design 新增），本阶段只补 `指令表收窄且新行集可判` 的两条新键 needle。


## 阶段 7 · 既有夹具与台账的机械随动

> 判据收紧的连带，不是用例设计：只补骨架 / 翻正负例 / 抬体量上限，MUST NOT 新增用例。

- [x] `api/schema_test.rs`：`:797` / `:806` / `:835` / `:881` / `:1757` 五处补 `"type": "object"`（补齐后仍断言同一原因：`properties` / `required` / 非函数属性）；`:813` 的 `bare`（`{"$defs": {"Gate": {"x-class": true}}}`）由**正例翻负例** —— 断言 `Err` 且文案点名 `type` 与「补 `\"type\": \"object\"`」的出路。
- [x] `api/emit/rust_test.rs`：`:345` 与 `:363` 补 `"type": "object"`（`:363` 的 `Marker` 变成「带骨架的合法空 trait」，断言仍为 `pub trait Marker {`）。
- [x] `api/emit/ts_test.rs`：`:283` / `:1080` / `:1839` 三处补 `"type": "object"`。
- [x] `api/emit/mod_test.rs`：`:427` / `:529` 两处补 `"type": "object"`（`:529` 的「带属性即在 `verify_defs` 判错」复现补齐后仍断言同一原因）。
- [x] `api/mod_test.rs`：`:1748` 的 `categories` 夹具两处 `x-class`（`Class` / `Both`）补 `"type": "object"`；断言 `Class` 计数与 `categories` 口径逐字不变。
- [x] `api/contract_test.rs`：`md解析器整体退场` 的体量断言（`:761`）上限由 `≤ 500` 抬到 **`≤ 640`**（D-10：判据面扩张的必然体量，落地后以实测收口）；模块头（`:1`）与落点闸两处 doc（`:208` / `:278`）的「封闭二处」改「四处」；其余既有断言（公开面 3 个 `pub fn`、`function_param_names(` 恰 3、`methods_of(` 恰 2）逐字保留。
- [x] `api/emit/rust_test.rs`：`value_type零x_function分支` 的测试 doc（`:1325`）「合法命中点恒 3」随 `trait_block` 的两处新读取点改写（留痕说明，非判据；该用例的两条硬断言 —— `value_type` 体内恰 0、`struct_block` / `trait_block` 读取点在场 —— 逐字保留）。

## 阶段 8 · 版本与收口自查

- [x] `packages/desktop/package.json`：`version` 0.7.18 → **0.7.19**（+1 patch）；`src-tauri/Cargo.toml` 与 `tauri.conf.json` 不动（版本纪律）。
- [x] 源码锚自查（只读 grep，不执行门禁链）：`grep '"x-class": true'` 在 `api/**` 下逐处已按阶段 7 处置（正例与外层负例全带 `"type": "object"`、`bare` 已翻负例）；`is_zero_field_object` 的判据含 `x-class` 排除；`check.rs` 内零 `x-class` 字面；`ts.rs` 内 `get("x-function")` 恰 2 处；`contract.rs` 内零新增 `pub fn`。
- [x] 零改动面自查：`api/mod.rs` / `api/emit/mod.rs` / `api/{extract/**,paths.rs,diff.rs,render.rs}` / `dto.rs` / `drift/**` / `archi_knowledge.rs` / `sdk/**` 零字节；`openspec/specs/**` 基线零改动；`.dev-team/architecture/**` 零字节（生成物零 diff 由门禁链 `api:check` 判定，本阶段不重复执行）。
- [x] 改动集自查：`git status --porcelain` ⊆ proposal「变更范围」所列文件（另加本 change 目录的 `proposal.md` / `design.md` / `tasks.md` / `explore.md` / `specs/**` / `reports/**`）+ 阶段 7 登记的既有夹具与台账随动；`src-tauri/Cargo.toml` 零改动。
