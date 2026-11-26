# 设计: archi-api-constructor-static-members

> **变更**：`archi-api-constructor-static-members`（`<叶段>.schema.json` 的 `x-` 关键字面由三项扩为五项）
> **输入**：本目录 `proposal.md`（定稿）、`explore.md`（只读）、`specs/` 三份 delta（`desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-agent`）
> **性质**：**协议面一次原子交付**——`x-constructor` / `x-static` 两个协议词汇落 `$defs.<T>`（仅 `x-class` 条目可用），两处静默（`x-class` 缺骨架、`x-class` 带 `additionalProperties`）变成装载期 `ApiError`，两个生成器同批学会渲染，知识骨架的 `x-` 面枚举逐名五项并同比抬锚。
> **一句话**：写闸只多判「位置、骨架、形态、预留名」，生成器只多认两个词；判据词汇零扩张、存量 11 份 schema 的产物逐字节不变。

## 架构组件 (Architecture Components)

### C1 · 装载写闸：骨架规则与两个新键的落点 / 形态（核心闸）

- **职责**：把「`x-class` 条目必须是一类对象」「`additionalProperties` 与 trait 语义互斥」「`x-constructor` / `x-static` 只住在 `x-class` 条目顶层、且内层形态可被生成器无歧义读取」三件事，全部变成装载期 `ApiError`（写闸与装载期共用一处）。零新闸、零新错误变体、零跨元素事实。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`（改）：`verify_defs` 的 `x-class` 分支 + 全文档落点单趟 + `verify_top_level_keys` 文案 + 两个私有 helper + 模块/函数 doc。
- **依赖**：`serde_json::{Map, Value}`（既有）、`super::contract::{X_CONSTRUCTOR, X_STATIC}`（C2 新增的常量）、`ApiError`、`kind_of`；**零**新 crate、零 IO、零 `check.rs` 依赖（判据面在 C5，不在本文件）。
- **技术**：四条落点纪律——

  1. **判序严格固定**（判序即错误面，「你不是 object」比「你不能有 `required`」更根本）：

     ```text
     verify_defs 逐条目（既有五条 $defs 规则逐字不动，仍在前）
       if x-class == true:
           ① type != "object"               → ApiError（缺骨架；出路：补 "type": "object"）
           ② contains additionalProperties  → ApiError（语义互斥；两条出路见下）
           ③ contains "required"            → ApiError（既有，逐字保留）
           ④ properties 有非 x-function      → ApiError（既有，逐字保留）
           ⑤ x-constructor 形态              → ApiError（见 3）
           ⑥ x-static 成员形态               → ApiError（见 3）
     verify_defs 尾：⑦ 全文档落点单趟（见 2）→ ⑧ degraded 词表扫描（既有，仍最后）
     ```

     ①② 只插在既有两条**之前**，不挪外层顺序（嵌套 `$defs` / `$ref` 形态 / 重复键仍先命中）——`schema_test` 的「新判据不遮蔽既有 `$defs` 规则」场景因此仍成立。

  2. **两个新键的落点用一次全文档单趟**（`verify_class_key_positions(schema)`，排在逐条目循环之后）：合法 pointer 集合恰为「`$defs.<T>` 且该条目 `x-class: true` 的顶层」。命中非法位置即 `ApiError`，点名非法 pointer 与出路（「只有 `x-class` 条目可声明构造入口与静态方法」）。覆盖四种非法位：非类 `$defs` 条目（object / enum / 标量 / 纯 `$ref` alias）的顶层、条目内更深层、模块 `properties` 子树、`x-function` 内层（后者是 `x-function` 的**自由形状**里唯一被禁的两个**协议词汇**——禁的是词，不是未知键闸）。顶层与 `properties.<k>` 两处仍由既有闸 ⑧ / ⑤ 先拒（`MUST NOT` 为此新写文案）。排序上放末尾，保证「嵌套 `$defs`」一类外层规则优先。

  3. **两键的形态判据**（私有 `verify_class_members(name, definition)`，只在 `x-class` 分支内调用）：`x-constructor` 的值 MUST 是对象、MUST 含对象型 `x-function`、内层 MUST NOT 有 `returns`（返回面由语义固定为本类型）、内层 `async` MUST NOT 为 `true`（构造器是值而非 future，两语言都没有异步构造）；`x-static` 的值 MUST 是对象、每个成员 MUST 含对象型 `x-function`（缺包裹即静默丢，这是本变更要消灭的同一类故障）。**外层另加键不判**（沿用「`x-function` 内层自由形状、不设未知键闸」的既有纪律）；`params` 可省（= 零参）。名字合法性 / 保留名**不在本文件**（C2）。

  4. **`additionalProperties` 的两条出路 MUST 可执行且 MUST NOT 把人引到第二个静默**：① 去掉 `x-class` 变纯数据，并说明**条目层 map 形态本期不渲染** ⇒ 键值对容器写在值位（`x-function.params[].schema` / `returns` / 字段位）；② 若本意是「方法集合」，去掉 `additionalProperties`——trait 没有实例数据面。**只治一侧**：无 `x-class` 的 `object` 条目带 `additionalProperties` 仍被放行（「缺能力」不是「语义错」）。

### C2 · 协议词汇单点、`x-function` 四处落点与预留名（名字面）

- **职责**：① 两个新键名的唯一持有者（编译期常量，禁散落字面量）；② `x-function` 的合法落点由二处扩到四处；③ `x-static` 的名字并入既有「方法名」族；④ 三条预留名判据在装载期拒收（N-1 / N-2 / `x-static` 名 = `new`），让「写闸放行 ⇒ 必然不可编译」的三条路径都死不掉到渲染期。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`（改）。
- **依赖**：`serde_json::{Map, Value}`（既有）、`super::ApiError`、`is_identifier` / `identifier_error`（同文件既有单点）。零新 crate。
- **技术**：

  1. **常量**：`pub(crate) const X_CONSTRUCTOR: &str = "x-constructor"` / `X_STATIC: &str = "x-static"`，紧随既有 `X_FUNCTION`；`schema.rs` 与 `emit/{rust,ts}.rs` 一律经它引用（`schema.rs` 沿用既有 `const X_FUNCTION = super::contract::X_FUNCTION` 的别名形态）。**不为 `x-class` 补常量**：它已散落在 5 个产品文件（`api/schema.rs` / `api/mod.rs` / `emit/mod.rs` / `emit/rust.rs` / `emit/ts.rs`）且语义不变，补常量只增 diff 面。
  2. **`methods_of` 收集面扩宽**（函数名、签名、调用点数**全不变**）：返回项由「`properties` 里的方法」扩为「`properties` 方法 + 构造器（名字固定 `new`）+ `x-static` 成员」，且**仅当该条目 `x-class: true`**（非类条目上的新键由 C1 的落点闸报，MUST NOT 被名字错抢先）。既有 ③ 族循环因此自动覆盖 `x-static` 名（「方法名」族，族别数仍七）与它们的 `params[].name` / `owner`；`$ref` 末段名族本来就走 `reference_names(definition)` 全树扫描，两个新键的子树已自动覆盖。
  3. **三条预留名判据**（私有 `verify_class_reserved_names(contract, name, definition)`，在 `$defs` 逐条目循环内、`$ref` 末段名检查之后调用）：`x-static` 名 MUST NOT 为 `new`（保留给构造器，出路指向 `x-constructor`）；`properties` 键 MUST NOT 为 `new`（N-1）；`x-static` 名 MUST NOT 与该条目 `properties` 键重名（N-2）。三条出路分别指向 `x-constructor` 与「改名或合并」。
  4. **`verify_x_function_positions` 二处 → 四处**：条目循环内按 `object.get("x-class")` 取值分派——`properties` 值（`allowed = true`，既有）；`x-constructor` 值（`allowed = true`，pointer `$defs.<T>.x-constructor`）；`x-static` 的每个成员值（`allowed = true`，pointer `$defs.<T>.x-static.<名>`）；**未标 `x-class` 的条目整条跳过**（新键的落点由 C1 判，MUST NOT 让 `x-function` 落点错抢先）。错误文案的「二处」改「四处」并列出四条 pointer 形态。`scan_x_function` 主体零改动（`allowed = true` 的节点其子树一律转假，故 `x-static.open.x-function.params[0].schema.x-function` 仍非法）。

### C3 · Rust 渲染：trait body 内的构造入口与静态方法

- **职责**：`x-class` 条目的 trait body 依次渲染 `properties` 方法 → 构造器 → `x-static` 成员；同批修掉「空 trait 被自动构造器段命中」的边缘（R-4）。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`（改）。
- **依赖**：`crate::api::contract::{X_CONSTRUCTOR, X_STATIC}`（C2）、既有 `method_signature` / `value_type` 出口；零新 crate、零 IO。
- **技术**：

  1. **入参序列抽段**：把 `method_signature` 的入参段抽成 `param_list(function, common) -> String`（`name: T` 以 `, ` 连接、`params` 缺省即空串），`method_signature` / `function_pointer` / 构造器三处共用——**零第二份签名拼装**、`value_type` 的 `Position` 分派面零改动。
  2. **`trait_block`**：既有 `properties` 循环之后追加
     - 构造器（若在场）：`format!("    fn new({}) -> Self;\n", param_list(...))`——不读 `returns`（装载期已禁，结构上收不到）；
     - `x-static`（若在场）：对每个成员 `format!("    fn {name}{};\n", method_signature(function, common))`。
  3. **`is_zero_field_object` 加 `x-class != true`**（**不改名**，D-8）：trait 不是类型本体，`impl <trait>` 与 `const _: fn() -> super::<trait>` 都不可编译（E0404）。唯一消费点仍是 `render_scope` 的 `constructible` 收集（`:71-75`）与见证段（`:121`）；**见证段零新增**（trait 不出 `T::new` 见证，绑定在 impl 处由编译器强制）。
  4. `trait_block` 的 doc 补一句「构造器与静态方法的读写面来自装载期形态判据，本函数不重复判错」。

### C4 · TS 渲染：形状承诺 + 同名值声明

- **职责**：`x-class` 条目保持 `export interface T { … }`（形状承诺），并在构造器或静态方法**至少一项在场**时追加同名值声明；两者皆无时产物与今天逐字节相同。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`（改）。
- **依赖**：`crate::api::contract::{X_CONSTRUCTOR, X_STATIC}`（C2）、既有 `function_of` / `function_type` / `value_type`；零新 crate。
- **技术**：

  1. **成员读取只走既有 `function_of(...)`**（它就是「解包 `{x-function: …}`」的现存单点）：`ts.rs` 内 `get("x-function")` 的文本出现次数**恒 2**（`interface_block` + `function_of`）——`ts_test` 的「合法读取点恰 2 处」是精确计数断言，本变更 MUST NOT 新写第三处。
  2. **两段式抽签名**：从 `function_type` 抽出 `parameter_list(function, common) -> String`（含 `optional → T | undefined`）与 `return_text(function, common) -> String`（含 `async → Promise<R>`）；`function_type` 改为 `({parameter_list}) => {return_text}`，值声明成员用 `{名} ({parameter_list}): {return_text}`。整签名包装（`common_type(function, Position::Return)` 的 `x-function.<承载键>` 形态）**只留给 `function_type`**（D-7）。
  3. **值声明形态**（块形态，与 `export interface` 对称、紧跟其 `}` 后零空行）：

     ```text
     export declare const Store: {
       new (path: string): Store;
       default_path (): string;
       open (path: string): Store;
     };
     ```

     成员序：`new` 在前，`x-static` 按 `serde_json::Map` 迭代序；缩进 2 空格（同 interface body）。**产出条件**：`class == true` ∧（构造器在场 ∨ `x-static` 非空）；object 条目与「无 ctor / 无 static 的类」零值声明。

### C5 · 判据口径收窄（文案面）

- **职责**：`apiConstructorDeclared` 的语义在文案与 doc 上收窄为「**模块级** `properties` 声明了构造器名」，并指向类型级构造的正规形态。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`（改，`constructor_declared` + 其 fn doc）。
- **依赖**：零新依赖——判据词汇、求值序、`category_of`、`contract.ops` 投影（本来就只含模块级 `properties`）逐字不变。
- **技术**：文案改为「接口定义 `<file>` 的**模块级** `properties` 声明了构造器 `<op>`——零字段 `object` 由生成器自动产 `new()`，类型级构造写进 `$defs.<T>.x-constructor`；模块级 `properties` MUST NOT 声明它（否则必然误报实现缺失）」。**MUST NOT 出现 `x-class` 字面**：`check_test` 的源码自证断言 `!include_str!("check.rs").contains("x-class")` 仍在案（本文件零 `x-class` 判据 / 提示分支）。

### C6 · 知识骨架：逐名五项与体量锚

- **职责**：把两个新词投到**生产面**——运行期注入给 agent 的只有 `archi-knowledge.md` 编译期内嵌的那一份字节，它够不着 `openspec/`。
- **文件**：`packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`（改一行）+ `archi_knowledge_test.rs`（四数 / 三处文案 / 两处 needle 集）。
- **依赖**：零新依赖——`archi_knowledge.rs` 的拼装、降级、两处 `foundation::layout` 派生路径逐字不动。
- **技术**：`- schema 不写理由：… `x-` 面封闭为 `x-function` / `x-class` / `x-common`。` 一行改为五项逐名（追加 ` / `x-constructor` / `x-static``，**实测 +31 B**）：骨架 `lf_bytes(trim_end)` 1777 → **1808 B**、全文 1966 → **1997 B**，门限同比平移 **1831 / 2031**（余量 23 B / 34 B）。三处文案（模块头门限句 / 测试 doc 的「MUST NOT 抬锚」→「抬锚须显式豁免并同批更新四数」/ 两条 `assert_eq!` 失败文案里的实测值）同批；`指令表收窄且新行集可判` 的 needle 集补 `` `x-constructor` `` / `` `x-static` ``（带反引号原样）。**抬锚最小口径**：只按本次 +31 抬，「至少一条构造通路」与「object ⇔ JSON 对象」两条不进骨架。

### C7 · 版本面

- **职责**：桌面端源改动的版本纪律。
- **文件**：`packages/desktop/package.json`（改）。
- **依赖**：零依赖——只改一个版本字符串；`tauri.conf.json` 经既有 `"version": "../package.json"` 间接跟随（不触），`src-tauri/Cargo.toml` 的 crate 版本行独立、不跟随。
- **技术**：`version` 0.7.18 → **0.7.19**（+1 patch）；`api:export` / `api:check` / `check` 三条命令串逐字不变。

## 变更清单 (Change Inventory)

### 新增文件

<!-- 新增文件：无。本变更全部落既有文件的判据 / 渲染 / 文案增补——零新模块、零新类型、零新工具、零新命令。 -->

### 修改文件

| 路径 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` | ① `const X_FUNCTION`（`:417`）旁补两条别名常量 `X_CONSTRUCTOR` / `X_STATIC`（引用 `super::contract` 的单点）；② `verify_top_level_keys`（`:377`）的未知顶层键文案：「`x-` 面只有 … 三项」→「`x-` 面封闭五项」逐名列全；③ `verify_defs`（`:481`）的 `x-class` 分支（`:526`）在既有 `required` 判据**之前**插 `type: "object"` 与 `additionalProperties` 两条，在既有 `properties` 全方法判据**之后**调 `verify_class_members(name, definition)`；④ 逐条目循环之后、`degraded_marks` 扫描（`:554`）之前调 `verify_class_key_positions(schema)`；⑤ 新增私有 `verify_class_members`（构造器 / 静态成员形态）与 `verify_class_key_positions`（全文档落点单趟，含递归）；⑥ 模块头与 `verify_defs` / `validate_schema` 的闸 ⑨ doc 同批改写（五条 `$defs` 规则 + 骨架四判据 + 两键落点） | 九道闸**条数不变**（新规则全落 ⑨）；闸序 ①–⑧ 逐字不动；既有 `required` / `properties` 两条文案逐字保留 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` | ① `:12` 旁补 `pub(crate) const X_CONSTRUCTOR` / `X_STATIC`；② `methods_of`（`:192`）收集面扩到「`x-class` 条目的构造器 + `x-static` 成员」（函数名、签名、调用点数不变）；③ 新增私有 `verify_class_reserved_names(contract, name, definition)`，在 `$defs` 逐条目循环内、`$ref` 末段名检查之后调用（三条预留名）；④ `verify_x_function_positions`（`:208`）落点二处 → 四处（`x-constructor` / `x-static.<名>`，仅 `x-class` 条目；非类条目跳过，交 C1 判落点）；⑤ `x_function_position_error`（`:275`）文案的「二处」→「四处」并列出四条 pointer 形态；⑥ 模块头 / `verify` / `methods_of` / 落点闸四段 doc 同步 | 名字族族别数仍七、公开面仍恰 3 个 `pub fn`、`verify` 体内 `function_param_names(` 仍恰 2 处、`methods_of(` 仍恰 2 处；`is_constructor` / `CONSTRUCTOR_NAMES` / `CONSTRUCTOR_PREFIXES` 逐字不变 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` | ① `use crate::api::contract::{…}` 补 `X_CONSTRUCTOR` / `X_STATIC`；② `method_signature`（`:295`）抽出 `param_list`，自身与 `function_pointer`（`:325`）改用之；③ `trait_block`（`:207`）在既有 `properties` 循环后追加构造器（`fn new(<params>) -> Self;`）与 `x-static` 成员（`fn <名><method_signature>;`）；④ `is_zero_field_object`（`:482`）判据增 `x-class != true`（doc 说明语义为「是否由生成器自动产出零参 `new`」）；⑤ 模块头渲染清单补一句 trait body 的两类新成员 | 见证段零新增（trait 不出 `T::new` 见证）；`value_type` 的 `x-function` 零分支不变量零改动；object 条目产物逐字节不变 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` | ① `use crate::api::contract::{…}` 补 `X_CONSTRUCTOR` / `X_STATIC`；② `function_type`（`:261`）抽出 `parameter_list` / `return_text` 两段，自身改用之；③ `interface_block`（`:201`）在 interface body 之后按条件追加同名值声明（`export declare const T: {…};`，成员 `new (<params>): T` + 每个 `x-static` 一条），成员读取走既有 `function_of`（`:256`）；④ 模块头补一句「值声明只在构造器或静态方法在场时产出」 | `get("x-function")` 的读取点恒 2 处（精确计数断言）；object 条目零值声明（产物逐字节不变）；`fn interface_block(` / `fn function_type(` / `fn function_of(` 三个函数名与入口形态不变 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` | `constructor_declared`（`:478`）的文案改为「**模块级** `properties` 声明了构造器」+ 出路指向 `$defs.<T>.x-constructor`；fn doc 同步 | 文案零 `x-class` 字面（既有源码自证断言）；判据词汇 / 求值序 / `category_of` / `contract.ops` 投影零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | 「三条不可越界」② 的 `x-` 面枚举由三项改**五项逐名**（`+31 B`）；其余段落逐字不动 | 骨架 1777 → **1808 B**；全文 1966 → **1997 B**；零「见 spec」类指路 |
| `packages/desktop/package.json` | `version` 0.7.18 → 0.7.19（+1 patch） | 版本纪律；`src-tauri/Cargo.toml` / `tauri.conf.json` 不跟随 |

**登记为零改动（评审核对面，出现 diff 即误改）**：

| 对象 | 理由 |
|---|---|
| `crates/core/archi/src/api/mod.rs`（`def_shape` / `categories_of` / `param` / `check` / `reexport_target`） | `def_shape` 判序不变（`$ref` → `x-class` → `variant` → `enum` → `scalar` → `object`）⇒ `INDEX.toml` 的 `categories` 仍记 `class`、`types` / `ops` / `degraded` 三计数口径不变 |
| `crates/core/archi/src/api/emit/mod.rs` | `render_target` 分派、`warn_missing_x_class`、`local_defs` / `reexport_defs` / `declared_ops` 逐字不动（新键不改变条目分类） |
| `crates/core/archi/src/api/{extract/**,paths.rs,diff.rs,render.rs}`、`dto.rs`、`drift/**` | 取数面 / 路径面 / 差异面 / 报告面与本变更零交集；`ArchiApiFindingKind` 十值、`ALL` 长度 10、`category` 七值、`severity()`、`REPORT_VERSION` 零改动 |
| `crates/infra/agent/src/archi_knowledge.rs`、`sdk/**`（`mount.rs` / `archi_tool.rs` / `policy.rs` / `compose.rs` / `runner.rs`） | 拼装与降级语义零改动（模块头未写体量 / 枚举数）；工具清单、档位、`(phase, role)` 矩阵、`archi_api` 三动作、`archi_scaffold` 词典零改动（无新工具、无参数变化） |
| `crates/core/archi/src/edit/**`、`crates/infra/{store,exec,vcs,watch}/**`、`crates/core/config/**` | 与协议面无关 |
| `.dev-team/architecture/**`（模型、`INDEX.toml`、`**/*.i.rs`、`**/*.i.ts`、ADR） | 零数据变化（55 元素 / 129 关系 / 表头计数逐字不变）；在案 11 份 schema 零 `x-class`、零新键 ⇒ `api:export` 逐字节不变、`api:check` 退出码 0 |
| `packages/desktop/src/**`（除 `types/generated/` 的「零 diff」核对）、`types/generated/bindings*.ts` | 零 DTO / 零命令 / 零 IPC 面变化 ⇒ `bindings:export` 零 diff |
| `src-tauri/Cargo.toml` / `tauri.conf.json`、`AGENT.md` / `CLAUDE.md`、`openspec/specs/**` 基线、本 change 的 `explore.md` | 版本纪律 / 规范面（「模块 API 契约」节不含 `x-` 枚举）/ delta-only 纪律 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `validate_schema` | `crates/core/archi/src/api/schema.rs` | 修改（行为随 ⑨，**签名不变**） | `pub fn validate_schema(fqn: &str, text: &str) -> Result<Contract, ApiError>` | 九道闸**条数**不变（新规则全落 ⑨ 的 `$defs` 校验）；闸序 ①–⑧ 与两处消费（装载期停摆 / 写闸零落盘）逐字不变 |
| `verify_defs` | 同上 | 修改（行为，签名不变） | `pub fn verify_defs(schema: &Value) -> Result<(), ApiError>` | `x-class` 分支按固定判序加 `type` / `additionalProperties` 两条、加两键形态两条；尾部加全文档落点单趟；既有五条 `$defs` 规则与 `degraded` 词表扫描逐字保留 |
| `verify_top_level_keys` | 同上 | 修改（**文案**） | `pub fn verify_top_level_keys(value: &Value) -> Result<(), ApiError>` | 顶层封闭六项不变；`x-` 面枚举由三项改五项逐名 |
| `verify` | `crates/core/archi/src/api/contract.rs` | 修改 | `pub fn verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | ① 名字趟把 `x-static` 名并入「方法名」族（族别数仍七）；② 加三条预留名判据（`x-static` 名 = `new` / `properties` 键 = `new` / `x-static` 名 ∩ `properties` 键）；③ 落点闸二处 → 四处 |
| `render`（Rust） | `crates/core/archi/src/api/emit/rust.rs` | 修改（行为，**签名不变**） | `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String` | `x-class` 条目的 trait body 增构造器与静态方法；`is_zero_field_object` 排除 `x-class`（trait 零 `impl` / 零见证）；object 条目产物逐字节不变 |
| `render`（TS） | `crates/core/archi/src/api/emit/ts.rs` | 修改（行为，**签名不变**） | `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String` | `x-class` 条目在构造器或静态方法至少一项在场时追加同名值声明；两者皆无时零值声明（产物与今天逐字节相同） |
| `check` | `crates/core/archi/src/api/check.rs` | 修改（**文案**，签名不变） | `pub fn check(input: ApiCheckInput<'_>) -> Vec<ArchiApiFinding>` | 只改 `apiConstructorDeclared` 的文案与口径（模块级）；词汇、求值序、`category_of`、纳管口径零改动 |

**常量（协议词汇单点持有，新增）**：

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `X_CONSTRUCTOR` | `crates/core/archi/src/api/contract.rs` | 新增 | `pub(crate) const X_CONSTRUCTOR: &str = "x-constructor"` | 与 `X_FUNCTION` 同款单点；`schema.rs` 与两个渲染器一律引用它，禁散落字面量 |
| `X_STATIC` | 同上 | 新增 | `pub(crate) const X_STATIC: &str = "x-static"` | 同上 |

> 本变更**不新增**任何 IPC 命令、agent 工具、CLI 子命令或 HTTP 端点：两个新词是 `<叶段>.schema.json` 的协议关键字，读写面仍是既有的 `write` / `edit` 写闸与 `archi_drift` / `api:check` 装载链。

### 内部接线面

> `pub(super)` / 文件内私有；登记落位与签名，防实现分叉。私有函数不改变任何公开面（`contract.rs` 仍恰 3 个 `pub fn`）。

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `verify_class_members` | `api/schema.rs` | 新增 | `fn verify_class_members(name: &str, definition: &Map<String, Value>) -> Result<(), ApiError>` | 构造器 / 静态成员形态的唯一判点（单个对象 + 对象型 `x-function` + `returns` / `async` 禁项） |
| `verify_class_key_positions` | `api/schema.rs` | 新增 | `fn verify_class_key_positions(schema: &Value) -> Result<(), ApiError>` | 全文档落点单趟（合法 pointer 集合 = `x-class` 条目顶层）；含一条私有递归体 |
| `methods_of` | `api/contract.rs` | 修改（**名字与签名不变**） | `fn methods_of(definition: &Value) -> Vec<(String, &Value)>` | 收集面由 `properties` 方法扩为「+ 构造器 `new` + `x-static` 成员」，仅 `x-class` 条目；`function_param_names(` 与 `methods_of(` 的文本计数不变 |
| `verify_class_reserved_names` | `api/contract.rs` | 新增 | `fn verify_class_reserved_names(contract: &Contract, name: &str, definition: &Map<String, Value>) -> Result<(), ApiError>` | N-1 / N-2 / `x-static` 名 = `new` 三条预留名 |
| `verify_x_function_positions` | `api/contract.rs` | 修改 | `fn verify_x_function_positions(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 二处 → 四处；非 `x-class` 条目的两键跳过（落点归 `verify_defs`） |
| `x_function_position_error` | `api/contract.rs` | 修改（文案） | `fn x_function_position_error(contract: &Contract, pointer: &str) -> ApiError` | 「二处」→「四处」并列出四条 pointer 形态 |
| `trait_block` | `api/emit/rust.rs` | 修改 | `fn trait_block(name: &str, object: &Map<String, Value>, common: &BTreeMap<String, CommonRule>) -> String` | 追加构造器与 `x-static` 成员（properties 方法之后） |
| `param_list` | `api/emit/rust.rs` | 新增 | `fn param_list(function: &Value, common: &BTreeMap<String, CommonRule>) -> String` | 入参序列（`name: T` 逗号连接）的唯一拼装点；`method_signature` / 构造器共用 |
| `method_signature` | `api/emit/rust.rs` | 修改（**签名不变**） | `fn method_signature(function: &Value, common: &BTreeMap<String, CommonRule>) -> String` | 改为复用 `param_list`（输出逐字不变） |
| `is_zero_field_object` | `api/emit/rust.rs` | 修改（**名字不变**） | `fn is_zero_field_object(definition: &Value) -> bool` | 判据增 `x-class != true`；doc 点明语义 = 「是否由生成器自动产出零参 `new`」 |
| `interface_block` | `api/emit/ts.rs` | 修改（**签名不变**） | `fn interface_block(name: &str, object: &Map<String, Value>, class: bool, common: &BTreeMap<String, CommonRule>) -> String` | interface 之后按条件追加同名值声明 |
| `parameter_list` | `api/emit/ts.rs` | 新增 | `fn parameter_list(function: &Value, common: &BTreeMap<String, CommonRule>) -> String` | 入参序列（含 `optional → T \| undefined`）的唯一拼装点 |
| `return_text` | `api/emit/ts.rs` | 新增 | `fn return_text(function: &Value, common: &BTreeMap<String, CommonRule>) -> String` | 返回位文本（含 `async → Promise<R>`）的唯一拼装点 |
| `function_type` | `api/emit/ts.rs` | 修改（**签名不变**） | `fn function_type(function: &Value, common: &BTreeMap<String, CommonRule>) -> String` | 改为 `({parameter_list}) => {return_text}`；整签名包装仍只在此函数 |
| `constructor_declared` | `api/check.rs` | 修改（文案，签名不变） | `fn constructor_declared(element: &ArchiElement, contract: Option<&Contract>) -> Option<ArchiApiFinding>` | 只在模块级 `properties` 命中构造器名；文案指向 `$defs.<T>.x-constructor` |

### 类型定义

<!-- 类型定义：无。本变更零新增 struct / enum / type alias / 公共 API class——只在 contract.rs 新增两个 &'static str 常量（已登记在「公共函数 / API · 常量」），不新增任何承载结构。 -->

### 配置

<!-- 配置：本变更不新增 / 不修改任何配置键（无 config.json 键、无环境变量、无构建特性）。两个体量门限（1831 / 2031）是测试内常量，登记在「既有台账随动」。 -->

### 既有夹具与台账的机械随动（判据收紧的连带，非测试设计）

> 收紧「`x-class` MUST 带 `type: "object"`」后，14 处既有夹具里裸写 `x-class` 的条目会先命中「缺骨架」而非被断言的原因。三类处置**逐处不同**，MUST NOT 统一处理；用例与断言设计归 test-design 相位。

| 文件 | 位置 | 处置 |
|---|---|---|
| `api/schema_test.rs` | `:797` / `:806` / `:835` / `:881` / `:1757` | 补 `"type": "object"`（正例与外层负例；补齐后仍断言同一原因：`properties` / `required` / 非函数属性） |
| `api/schema_test.rs` | `:813`（`bare`） | **正例翻负例**：今天是 `verify_defs(&bare).is_ok()`，本变更后必 `Err`（缺骨架）——改成断言 `Err` 且文案点名 `type` 与出路 |
| `api/contract_test.rs` | `md解析器整体退场` 的体量断言（`:761`） | 上限 `≤ 500` → **`≤ 640`**：本变更在同一文件加「第四处落点 + `x-static` 名族 + 三条预留名 + 三处文案」（+90–110 行，D-10） |
| `api/contract_test.rs` | 模块头（`:1`）与落点闸两处 doc（`:208` / `:278`）的「封闭二处」 | 文案随动改「四处」（说明性文字，非判据；断言逐字不变） |
| `api/emit/rust_test.rs` | `value_type零x_function分支` 的测试 doc（`:1325`）「合法命中点恒 3」 | `trait_block` 新增构造器 / 静态两处读取点 ⇒ 该留痕数字随实现改写（非判据；`value_type` 体内恰 0 与两处读取点在场两条断言逐字不变） |
| `api/emit/rust_test.rs` | `:345` / `:363` | 补 `"type": "object"`；`:363` 的 `Marker` 由「零属性即合法空 trait」变成「带骨架的合法空 trait」 |
| `api/emit/ts_test.rs` | `:283` / `:1080` / `:1839` | 补 `"type": "object"` |
| `api/emit/mod_test.rs` | `:427` / `:529` | 补 `"type": "object"`；`:529` 的「带属性即判错」复现补齐后仍断言同一原因 |
| `api/mod_test.rs` | `:1748`（`categories` 夹具） | `Class` 与 `Both` 两处补 `"type": "object"`；断言 `Class` 计数与 `categories` 口径逐字不变 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | `骨架与全文双门限冻结` / `三条不可越界逐条在场` / `指令表收窄且新行集可判` / 模块头 | 四数改 1808 / 1831 / 1997 / 2031 + 三处文案；两条 needle 集见 C6 |

## 数据模型 (Data Model)

### `$defs.<T>` 的两层（`x-class` 条目）

```text
┌───────────────────────────────────────────────────────────────────────┐
│ 骨架（标准 JSON Schema 能看见的形状）                                  │
│   {"type": "object", "properties": { … }}                              │
│   ↑ 写闸强制：x-class ⇒ type MUST be "object"（缺即 ApiError）          │
├───────────────────────────────────────────────────────────────────────┤
│ 行为面（只有本仓 x- 面能看见；标准校验器当未知关键字）                  │
│   "x-class": true                                                      │
│   properties.<方法>.x-function            实例方法（无 self 的关联函数）│
│   "x-constructor": {"x-function": {"params": […], "async": false}}     │
│   "x-static": {"<名>": {"x-function": {"params": […], "returns": …}}}   │
└───────────────────────────────────────────────────────────────────────┘
```

**类与数据的分界 = 有没有方法**：`object`（无 `x-class`）是纯数据、无行为、非测试主体，其实例由 `properties` + `required` 的字面量构造——两个新键在其上出现即 `ApiError`。

### 两个新键的形态

| 键 | 外层 | 允许的内层键 | 禁项 | 名字 |
|---|---|---|---|---|
| `x-constructor` | **单个对象**（不是「名字 → 签名」表） | `x-function: { params?: […], … }` | `returns` MUST NOT 出现；`async` MUST NOT 为 `true`；MUST NOT 是数组 / 标量 / 缺 `x-function` | **固定 `new`**：不设槽位、不入名字族、不入 `CONSTRUCTOR_NAMES` |
| `x-static` | 「名字 → `{ "x-function": { … } }`」对象 | `params` / `returns` / `async` / `description`（自由形状） | 名 MUST NOT 为 `new`；名 MUST NOT 与该条目 `properties` 键重名；成员 MUST NOT 缺 `x-function` | ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`，并入「**方法名**」族（族别数仍七） |

`params[]` 的元素形态沿用既有 `x-function`：`{ "name": <标识符>, "schema": {…}, "optional": <bool> }`；`optional` 在 Rust 侧仍被丢弃（既有不对称，见风险）。

### 合法落点矩阵（`x-function` 四处 + 两个新键一处）

| 位置 | `x-function` | `x-constructor` | `x-static` | 判点 |
|---|---|---|---|---|
| `properties.<k>` 的值 | ✅ 模块操作 | ❌ | ❌ | ⑤ 形态白名单 / ⑧ 顶层键（既有文案） |
| `$defs.<T>.properties.<k>` 的值 | ✅ 方法 / 函数指针字段 | ❌ | ❌ | C1 落点单趟 |
| `$defs.<T>.x-constructor.x-function` | ✅ **新增**（仅 `x-class`） | ❌ | ❌ | C2 落点闸四处 |
| `$defs.<T>.x-static.<名>.x-function` | ✅ **新增**（仅 `x-class`） | ❌ | ❌ | C2 落点闸四处 |
| `$defs.<T>` 条目顶层 | ❌ | ✅ **仅 `x-class`** | ✅ **仅 `x-class`** | C1 / C2 |
| `items` / `additionalProperties` / `oneOf` / `anyOf` 及更深层 | ❌ | ❌ | ❌ | C2 / C1 |
| 顶层 / 条目内更深层 | ❌ | ❌ | ❌ | ⑧ / C1 落点单趟 |

### 判据分布与判序（写闸，唯一入口）

```text
contract::verify（先于 verify_defs 执行）
  ⒜ x-static 名：ASCII 标识符（「方法名」族）+ MUST NOT 为 new
  ⒝ N-1：x-class 条目的 properties 键 MUST NOT 为 new
  ⒞ N-2：x-static 名 MUST NOT 与同条目 properties 键重名
verify_defs（逐条目，既有五条 $defs 规则在前）
  ① type != "object" → ② contains additionalProperties → ③ required → ④ properties 全方法
  → ⑤ x-constructor 形态 → ⑥ x-static 成员形态
verify_defs（尾）⑦ 全文档落点单趟 → ⑧ degraded 词表（既有）
```

**已知的跨函数判序事实**（不是缺陷，记下防误读）：`contract::verify` 在 `validate_schema` 链上先于 `verify_defs` 跑，故「既违骨架又撞预留名」的输入先报名字错。AC-5 的可断言判序只覆盖骨架四条（①–④ 全在 `verify_defs` 同一分支内，顺序可断言），**MUST NOT** 为对齐跨函数顺序在 `contract.rs` 里复核骨架——那是第二份判据。

### 渲染映射（生成器决策，不是协议判据）

| 输入 | Rust（`.i.rs`） | TS（`.i.ts`） |
|---|---|---|
| `x-class` + 方法 | `pub trait T { fn m(…) -> R; }`（既有） | `export interface T { m: (…) => R; }`（既有） |
| `x-constructor` | trait body `fn new(<params>) -> Self;` | 值声明 `new (<params>): T;` |
| `x-static.<名>` | trait body `fn <名>(<params>) -> R;` | 值声明 `<名> (<params>): R;` |
| 值声明产出条件 | — | `class` ∧（构造器 ∨ `x-static` 非空）；否则零值声明 |
| 零字段 `object`（无 `x-class`） | `impl T { pub fn new() -> Self { Self } }` + 见证行（既有） | 零值声明（既有） |
| 零字段 `x-class` 条目 | **只** `pub trait T {\n}\n`：零 `impl T`、零见证行 | `export interface T {}`（零值声明） |

**成员序**：`properties` 方法 → 构造器 → `x-static`，各按 `serde_json::Map` 的**迭代序**。仓库未开 `preserve_order`（实测 `Cargo.toml` 的 serde_json 特性只有 `raw_value`）⇒ JSON 对象键序在解析期即归为**字典序**，「声明序」只能按 Map 序兑现（与既有 `$defs` / `local_defs` 同一事实）。**MUST NOT** 为源文序开 `preserve_order`：它会改全部生成物的键序（AC-1 逐字节不变即破），且引入新依赖。

两个渲染器的写法差异（Rust trait body / TS `export interface` + 值声明）属**生成器自由**：协议只固定语义（存在一个规范构造入口、名字 `new`、有序入参表、隐式返回本类型；存在一张类型级静态方法表），协议文本 MUST NOT 出现 `pub fn` / `declare const` / `constructor` 一类语言字面。

### 语义约束（本期零判据，可为红绿的反向断言）

| 约束 | 本期状态 | 兜底面 |
|---|---|---|
| 每个 `x-class` 至少一条构造通路（`x-constructor` / 返回自身 `$ref` 的 `x-static` / 任一 `x-function.returns` 指向它） | **判定面缺席**：写闸放行、生成成功、`apiFindings` 零条目、stderr 零警告 | Rust 由 impl 处的编译期强制（实现该 trait 的具体类型必须补齐 `fn new` 且签名逐项相符）；TS 与既有结构类型形态一致；测试生成器调用真实 API，错签名在测试编译期即红 |
| 构造器的返回面 | 隐式本类型：`returns` 在装载期即禁 | `-> Self`（Rust）/ `: T`（TS）由生成器固定 |
| `params[].optional` 在 Rust 侧 | **仍被丢弃**（`method_signature` 不读 `optional`；TS 的 `function_type` 读）——既有 `$defs` 方法的对称缺口，构造器 / 静态复用同一出口即继承 | 本期不治、不新增判据（治它属参数面的统一变更，会扩到既有方法） |
| `INDEX.toml` 的 `categories` | 不变（`def_shape` 判序不变 ⇒ `x-class` 条目仍记 `class`） | 生成物零 diff（AC-1） |

### 持久化

- **零新增持久化**：无新文件类型、无新目录、无新索引字段；`<叶段>.schema.json` 仍是手写 JSON 真相，`.i.rs` / `.i.ts` / `INDEX.toml` 仍是生成物。
- **零迁移**：在案 11 份 schema 零 `x-class`、零新键 ⇒ 生成物逐字节不变；其他工作区的存量 schema 因「新键可选 + 骨架一条全新判据」而只受影响于「存量 `x-class` 条目缺 `type: "object"`」这一条**有意收窄**（代价是一行，文案给可执行出路）。

## 路由 / API 设计 (Route / API Design)

> `auth`：不适用——本机进程内一致性检查与 Tauri IPC 均无鉴权面；读写准入由既有沙箱、`ProtectRules` 与 `write_atomic` 表达，本变更零改动。

**零新增端点 / 零新增命令 / 零新增工具**。本变更的「接口」是 `<叶段>.schema.json` 的关键字面，其三个消费点如下（同一份判据，零第二份实现）：

| 面 | 入口 | 输入 | 输出 | 变更 |
|---|---|---|---|---|
| 写闸 | `write` / `edit` 落 `<archi_models_dir>/**/<叶段>.schema.json` | 落盘后的正文 | `Err`（零落盘）/ 成功 + 「接口已校验」 | 新规则经 `validate_schema` 的 ⑨ 生效；零新命令、零新参数 |
| 装载期 | `archi_drift` → `load_contracts` → `validate_schema`；`api:check` 同链 | 盘上正文 | `ApiError`（**停摆**，不产半真报告）/ `Contract` | 同上 |
| 生成 | `api:export` → `render_target` | `Contract` + schema | `.i.rs` / `.i.ts` / `INDEX.toml` | 两个渲染器新增两个词的分支；分派与条目分类零改动 |

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `serde_json`（既有，特性 `raw_value`） | `Value` 读写 / `Map` 迭代 / 报告文案 | **零新增**（不启用 `preserve_order`、不加特性、不升版本） |
| 既有 `ApiError` / `ApiMeta` / `Contract` | 错误面与契约投影 | 零新增 |
| `std`（`String` / `Vec` / `BTreeMap`） | 名字收集与文本拼装 | 零新增 |

### 构建 / 测试依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| 既有 cargo nextest / 变异工具链 / `vp` 门禁 | 单测与零 diff 门（`api:export` / `api:check` / `bindings:export`） | 零新增 / 门限串逐字不变 |

## 决策留痕

| # | 决策 | 理由 / 边界 |
|---|---|---|
| D-1 | 两个新键名由 `contract.rs` 单点持有（`pub(crate) const`），`schema.rs` / `emit/{rust,ts}.rs` 一律引用；**不为 `x-class` 补常量** | 与 `X_FUNCTION` 同款形态（单点定义 + `schema.rs` 别名）；`x-class` 已散落在 5 个产品文件且语义不变（`api/schema.rs` / `api/mod.rs` / `emit/mod.rs` / `emit/rust.rs` / `emit/ts.rs`），补常量只增 diff 面。改名一处即全链路随动 |
| D-2 | 骨架两条（`type` / `additionalProperties`）与本键形态落 `verify_defs` 的 `x-class` 分支且判序固定；**落点是全文档单趟**、排在逐条目循环之后、`degraded` 扫描之前 | 判序即错误面（AC-5 可断言）；落点放末尾才不遮蔽「嵌套 `$defs`」等外层规则。用全文档扫描而不是只扫 `$defs`，是因为 AC-4 还要求 `properties.<k>.x-function` 内层各一条——`x-function` 内层是自由形状（不可做未知键闸），故只对这两个**协议词汇**做位置封闭。顶层与 `properties.<k>` 两处仍由既有 ⑧ / ⑤ 拒（复用既有文案） |
| D-3 | 三条预留名判据落 `contract::verify` 的 `$defs` 趟；`x-static` 名与 `params[].name` / `owner` 检查**并入既有 `methods_of`**（名字与签名不变，仅扩返回项，且仅 `x-class` 条目） | 名字族单点：`contract_test` 的精确计数断言（`function_param_names(` 恰 3 = 定义 1 + 调用 2、`methods_of(` 恰 2、公开面恰 3 个 `pub fn`）全部保持。仅 `x-class` 门控是为了不让名字错抢先于落点错（非类条目上的新键由 C1 报，文案可执行） |
| D-4 | `check.rs` 的新文案 **MUST NOT 出现 `x-class` 字面** | 既有源码自证断言 `!include_str!("check.rs").contains("x-class")` 仍在案；出路写成「零字段 `object` 自动产 `new()`」与「`$defs.<T>.x-constructor`」即可表达同一事实 |
| D-5 | 「声明序」按 `serde_json::Map` 的**字典序**兑现（`default_path` 在 `open` 之前）；**不开 `preserve_order`** | 仓库 serde_json 特性只有 `raw_value`（实测），JSON 对象键序在解析期已丢；开特性会改全部生成物键序（AC-1 逐字节不变即破）并引入新依赖。与既有 `$defs` / `local_defs` 的「声明序」措辞同一事实（既有措辞亦为宽松表述） |
| D-6 | 值声明与 trait body 的**语言写法**只写在本设计（C3 / C4），不进数据模型的协议面 | spec 明文：协议只固定语义，写法属生成器决策；换语言 / 换写法不改协议 |
| D-7 | TS 值声明成员 **不套整签名包装**（`common_type(function, Position::Return)` 的 `x-function.<承载键>` 形态），只复用 `parameter_list` + `return_text` | 整签名包装是「函数类型」的文本形（错误信封）；值声明的成员是「可调用成员」。其 `returns` 位仍经 `value_type(returns, Return)` 拿到按位置的映射，故 `Outcome` 一类返回规则照常生效 |
| D-8 | `is_zero_field_object` 加 `x-class != true` 但**不改名** | 改名零语义收益、只增 diff 面（proposal §九 ③）；doc 一句点明语义（= 是否由生成器自动产出零参 `new`），唯一消费点是 `render_scope` 的 `constructible` 收集与见证段 |
| D-9 | 知识骨架只改 `x-` 面那一行为五项逐名（+31 B），四数同批 1808 / 1831 / 1997 / 2031 | 实测：骨架 `lf_bytes(trim_end)` 1777 → 1808（增量恰为 ` / \`x-constructor\` / \`x-static\`` 的 31 B），全文 1966 → 1997；门限同比平移，余量仍 23 B / 34 B。抬锚是**逐次豁免**：只按本次 +31，「至少一条构造通路」（二期判据面）与「object ⇔ JSON 对象」（写闸已强制）**不进**骨架 |
| D-10 | `contract.rs` 的 500 行体量要约上调到 **≤ 640 行** | 现 499 行（实测），本变更在同一文件加「第四处落点 + `x-static` 名族 + 三条预留名 + 三处文案」≈ +90–110 行。该断言是上一变更的设计预算（`contract_test` 的 `md解析器整体退场`），判据面扩张必须同批抬；落地后以实测收口，超限先压文案不删判据 |
| D-11 | 判据面与生成物面**零扩张**：`ArchiApiFindingKind` 十值 / `ALL` 长度 10 / `category` 七值 / `REPORT_VERSION` / `def_shape` 判序全不动 | `INDEX.toml` 的 `categories` 仍记 `class`；在案 11 份 schema 零 `x-class` ⇒ `api:export` 逐字节不变、`api:check` 退出码 0；`bindings:export` 零 diff |
| D-12 | 「至少一条构造通路」只进 spec 与后续 change 的笔记，**不进写闸、不进骨架、不产提示** | 通路天然跨元素（父 / 兄弟元素的工厂 op 也算），而 `validate_schema` 的纪律是「文件级自洽、零跨元素」；写闸做它就变成第二份判据。一期的已知窗口由两侧兜住（Rust impl 处编译期强制 / TS 结构类型 / 测试生成器调用真实 API） |
| D-13 | 自动构造器与见证行只对**非类**的零字段 object 产出 | trait 不是类型本体：`impl <trait>`（E0404）与 `const _: fn() -> super::<trait>` 都不可编译。这是骨架强制的配套而非可选项（R-4），与 `x-class` 的 `type` 断言**同批**落地 |
| D-14 | 两个渲染器的成员序取 Map 迭代序，**不排序、不合并** | 与既有 `$defs` / `local_defs` 的「零排序、零按结构合并」同一条纪律；排序会让「同一 schema 两次渲染逐字相等」之外多一个隐含键，且与既有产物形态不一致 |

## 验收标准的设计回应

| AC | 设计回应 |
|---|---|
| AC-1 存量零 diff | D-11 + 变更清单「零改动面」：`def_shape` 判序不变、在案 11 份 schema 零 `x-class`、零新键 ⇒ `api:export` 逐字节不变；`types` / `ops` / `degraded` 三读数与 `categories` 计数由同一投影给出 |
| AC-2 正例生成（Rust） | C3（`trait_block` 追加构造器与静态方法、复用 `param_list`）+ D-5（成员序 = Map 序）+ D-13（零 `impl` / 零见证）；`.i.rs` 形态见数据模型「渲染映射」 |
| AC-3 正例生成（TS） | C4（`interface_block` 条件追加值声明、成员走 `function_of`）+ D-7 + D-5；「无 ctor / static 的类」与 object 条目零值声明 ⇒ 产物逐字节不变 |
| AC-4 落点负例 | C1 技术 2（全文档落点单趟，点名 pointer + 出路）+ 顶层（既有 ⑧，文案列五项）+ `properties.<k>`（既有 ⑤，形态白名单逐字不变） |
| AC-5 骨架与语义互斥负例 | C1 技术 1（判序 ①②③④，① ② 在 ③ ④ 之前即可断言）+ 技术 4（两条可执行出路，且不引入第二个静默） |
| AC-6 新键形态与保留名负例 | C1 技术 3（`returns` / `async` / 成员缺 `x-function`）+ C2 技术 3（`x-static` 名 = `new` / N-1 / N-2）；六形态各一条 `ApiError`，互不吞并（分属两个函数、不同 pointer 文案） |
| AC-7 空 trait 边缘回归 | D-13 + C3 技术 3：`{"type":"object","x-class":true,"properties":{}}` 只出 `pub trait Gate {\n}\n`；`is_zero_field_object` 的判据含 `x-class` 排除（`grep` 可判），`constructible` 收集是唯一消费点 |
| AC-8 良性对照 | C1 技术 4「只治一侧」+ 数据模型「渲染映射」末两行：无 `x-class` 的零字段 object 仍产 `new()` 与见证；`{"type":"object","additionalProperties":{…}}` 仍被写闸放行 |
| AC-9 判据面 | C5（文案收窄）+ D-11（词汇十值 / 七分类 / `ALL` 长度）+ D-12（无构造通路 ⇒ 零判据零警告） |
| AC-10 知识骨架 | C6 + D-9：四数 1808 / 1831 / 1997 / 2031 与三处文案同批；五项逐名、零「见 spec」；needle 集补两新词（删任一项即红） |
| AC-11 夹具面收敛 | 「既有夹具与台账的机械随动」表：14 处逐处处置（三类分开）、`bare` 翻负例、五处负例原因文本不漂移 |
| AC-12 版本与改动集 | C7（0.7.19）+ 变更清单列全（含台账随动的 `contract_test` 上限）；`openspec/specs/**` 与 `src-tauri/Cargo.toml` 零改动 |
| AC-13 spec delta 形态 | 三份 delta 已在 `specs/` 定稿（本设计只消费条款、不改基线）；设计对契约的偏离仅 D-5（成员序按 Map 序），已列待决问题 Q-1 供评审处置 |

## 待决问题 (Open Questions)

| # | 事项 | 处置建议 |
|---|---|---|
| Q-1 | `desktop-archi-api` 的场景文本把 `x-static` 的成员写成 `open` 在 `default_path` **之前**（源文书写序），而实现只能出 Map 序（`default_path` 在前）。两者不冲突于语义，但逐字断言会打架 | spec 的成员序不是判据（那一句是生成形态示意，且「写法属生成器决策」）；建议 test-design 按 Map 序断言，spec 归档合并时把该句的成员序视为示意。若评审要求协议钉死源文序，需引入 `preserve_order`（破坏 AC-1）或给 emit 传入原始文本（四参签名扩张）——两条都被 D-5 否决 |
| Q-2 | TS 值声明的**格式化不动点**：仓内零 `.i.ts`（`ts.rs` 模块头自注「本仓零 `.i.ts`」），无法在本仓实测 `vp fmt` 对块形态值声明的稳定态 | 取块形态（与 `export interface` 对称；prettier 的「`{` 后换行即展开」启发式保持）。首个真实产物落地时若被格式化器改写，改渲染器（生成器自由），不动协议 |
| Q-3 | 值声明成员不套整签名包装（D-7）的边界 | 现网 `common.json` 只有一条 `Outcome`（错误通道），且它按 `returns` 位（`value_type`）生效，故本期零缺口。若将来出现「函数级」包装规则，需为值声明成员补一条映射面——届时属 `common.json` 规则面的变更，不在本 change |
| Q-4 | `x-static` 成员外层是否允许 `x-function` 之外的并列键（如 `description` 写在外层） | 本设计**不判**（外层另加键放行，与「`x-function` 内层自由形状」同一条纪律：不加未知键闸）。带 `x-function` 的形态即被生成器读取，其余键静默不带产物——若评审要求「外层也封闭」，需新增一条形态判据（本变更的面已满，建议留给后续 change） |
