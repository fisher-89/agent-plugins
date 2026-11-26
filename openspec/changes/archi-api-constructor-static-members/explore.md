# 接口协议固定构造方式与静态方法（x-constructor / x-static）
workflow_type: requirement

## 一句话

为「对照协议生成测试用例」铺路：在 `<叶段>.schema.json` 里给**类**（`x-class`，测试主体）固定**一个规范构造入口**（固定名 `new`，只声明 `params`）与**类型级静态方法表**；`object` 是纯数据、无行为、非测试主体，不在射程内。协议只约束语义，Rust / TS 的具体语法由各语言生成器分派。

## 0.5 · 已决点（本轮裁决，收口原 §9）

| # | 原未决点 | 裁决 |
|---|---|---|
| 1 | 工厂兜底规则 | **接受并升格**：每个 `x-class` MUST 至少有一条构造通路（`x-constructor` / 返回自身 `$ref` 的 `x-static` / 任一 `x-function.returns` 指向它）；「至少一条」的**判定**在 Rust 解析面（取数器可判 owner + 方法名），不在写闸（跨元素，见 §5） |
| 2 | `x-static` 值是否保留 `{ "x-function": … }` 外壳 | **保留**（与 `properties` 同形：落点闸 / 名字族 / 渲染三处都不用另开分支） |
| 3 | 二期判据比对是否随本期做 | **本期不做**，拆到 `.dev-team/explores/archi-api-defs-member-check.md` 留档 |
| 4 | `x-constructor` 是否也挂 `x-class` | **只挂 `x-class`**。`object` 是 schema 原有类型，代表纯数据、无行为、**非测试主体**；类与数据的分界即「有没有方法」 |
| 5 | `x-static` 的落点 | **同 4**：只挂 `x-class`。没有 `x-class` 的 object **等同于 JSON 对象**（纯数据），不承载任何行为面 |
| 6 | `x-class` 与 `type: "object"` 的关系 | **是，所有 `x-class` MUST 带 `type: "object"`**（写闸判据）。骨架（`type`）与行为面（`x-class`）分离声明，见 §2.5 |
| 7 | 一期是否提示「无构造通路」 | **不提示**，判定与提示一并归二期（`archi-api-defs-member-check.md`） |
| 9 | `$defs.<T>` 上的 `additionalProperties` 怎么处置 | **只治 `x-class` 一侧**：`x-class` + `additionalProperties` ⇒ `ApiError`（今天静默、且裁决 6 后必产不可编译的 `impl <trait>`）。`object` 条目的 `additionalProperties`（条目层 map 形态未实现）**本期不动**，仍留观察。见 §2.6 |
| 8 | 知识骨架的「x- 面封闭三项 → 五项」怎么落 | **逐名列出五项**，MUST NOT 写「见 spec」——知识是生产资产，运行期没有 `openspec`；体量冻结**本次豁免、允许抬锚**（只按本次需要 +31 B 抬，不顺手塞别的规则）。见 §6.1 |

## 0 · 目标与边界

**目标**：测试生成器读协议即可机械回答两件事——「这个类型怎么造」（`T::new(args)`）与「这个类型有哪些类型级方法可调」（`T.<name>(args)`）。

**边界**：

- 本期只加「构造 + 静态方法」，且**只作用于 `x-class`**；object 面零变化（既无构造入口、也无静态方法表）。实例方法语义不变（`x-class` 的 trait 方法仍渲染为无 self 的关联函数，object 的 `x-function` 仍是函数指针字段）。
- **协议不写目标语言语法**：`pub fn` / `declare const` / `constructor` 一类字面只属于生成器，新增语言或换写法不需要改协议。
- 兼容：`x-class` 的**既有语义**（仅允许方法、带属性即错、`$defs` 位置）逐条不动；新增的只是骨架规则（裁决 6：`type: "object"` 必在场）。仓库在案 11 份 schema 零 `x-class`、零新键 ⇒ **产品契约零迁移**；产品面其他工作区的存量 schema 也因「新键可选」而零破坏。**测试夹具要改 12 处**（§2.5 ⚠ 条）。

## 1 · 现状缺口

| # | 缺口 | 事实 | 落点 |
|---|---|---|---|
| 1 | 构造器整体禁止 | 构造器名是封闭集（`new` / `default` / `build` / `empty` / `never` / `open` + `from_*` / `with_*`）；模块 op 声明即 `apiConstructorDeclared`；唯一的构造器是零字段 object 自动产出的**零参** `new()` | `contract.rs:14-16`、`contract.rs:369-373`、`check.rs:476-494`、`emit/rust.rs:86-92` |
| 2 | 静态方法无表达 | `$defs.<T>.properties.<k>.x-function` 在 `x-class` 上是 trait 方法（Rust 渲染无 self），在 object 上是函数指针**字段**；`x-function.owner` 只服务模块 op 的签名见证 | `emit/rust.rs:178-185`、`emit/rust.rs:207-225`、`emit/rust.rs:287-291` |
| 3 | 写了无绑定 | `$defs` 成员不参与判据（只判类型名存在性）；签名比对只覆盖模块 op | `check.rs:366-377`、`check.rs:428-459` |

第 1 条是硬阻塞：没有入参槽位，构造方式无从固定。

## 2 · 协议设计

### 2.1 关键字面

`x-` 面由三项扩为五项：`x-function` / `x-class` / `x-common` / **`x-constructor`** / **`x-static`**。两个新键只允许出现在 `$defs.<T>` 条目**内部**（顶层键封闭六项不变），且**仅 `x-class: true` 的条目可用**——`object` / enum / `oneOf` / 标量 / 纯 `$ref` alias 上出现即 `ApiError`（object 的实例由字段字面量构造，声明构造入口只会制造第二份真相）。

**五项名 MUST 在知识骨架里自持**（裁决 8）：运行期注入给 agent 的只有 `archi-knowledge.md` 那一份文本，它够不着 `openspec/`——「名字清单归 spec」在生产面等于「永不抵达」。这不是「名字清单唯一持有者是 schema」那条纪律的反例：那条管的是**导出名**，`x-` 关键字是协议词汇，两者的持有者不同。

### 2.2 形态

```json
"$defs": {
  "Store": {
    "type": "object",
    "x-class": true,
    "properties": {
      "count": { "x-function": { "returns": { "type": "integer" } } }
    },
    "x-constructor": {
      "x-function": {
        "params": [ { "name": "path", "schema": { "type": "string" } } ]
      }
    },
    "x-static": {
      "open": {
        "x-function": {
          "params": [ { "name": "path", "schema": { "type": "string" } } ],
          "returns": { "$ref": "#/$defs/Store" }
        }
      },
      "default_path": { "x-function": { "returns": { "type": "string" } } }
    }
  },
  "Ticket": {
    "type": "object",
    "properties": { "id": { "type": "string" } },
    "required": ["id"]
  }
}
```

`Store` 是**测试主体**（有条目方法 ⇒ `x-class`）：`x-constructor` 给规范构造入口、`x-static` 给类型级方法（`open` 返回自身 = 工厂）。`Ticket` 是**纯数据**（无方法 ⇒ object）：构造 = `{ id: "…" }` 字段字面量；把 `x-constructor` / `x-static` 写进 `Ticket` 即 `ApiError`。

### 2.3 规则

| 键 | 形态与规则 |
|---|---|
| `x-constructor` | **仅 `x-class` 条目**可写（object / enum / `oneOf` / 标量 / alias 上出现即 `ApiError`）。**单个对象**（不是「名字 → 签名」表）：`{ "x-function": { "params": [...] } }`。`params` 可省（= 零参）；`returns` **禁止出现**（隐式 `Self`）；其余内层键沿用「`x-function` 内层自由形状」，不设未知键闸（文档只写 `params`）。名字**固定 `new`**，不入名字族。 |
| `x-static` | 落点与 `x-constructor` **完全相同**（**仅 `x-class: true` 的条目**，见 §2.5——没有 `x-class` 的 object 等同于 JSON 对象，不承载任何行为面）。「名字 → `{ "x-function": { … } }`」对象：名字 MUST 是 ASCII 标识符，MUST NOT 为 `new`（保留给构造器），并入既有「方法名」标识符族；值的内层自由形状（`params` / `returns` / `async` / `description`）。返回本类型 `$ref` 即工厂方法。 |

- `x-class` 带属性判错不变；ctor / static 不算属性。
- **`x-class` MUST 带 `type: "object"`**（裁决 6，论证见 §2.5）：骨架（`type`）与行为面（`x-class`）分两处声明，缺骨架即 `ApiError`。
- **`x-class` ⇔ 有方法**：类与数据的分界是「有没有方法」——object 是 schema 原有类型，代表纯数据、无行为、**非测试主体**，它的实例由 `properties` + `required` 的字面量构造。**没有 `x-class` 的 object 等同 JSON 对象**，两个新键一律不可用。
- **至少一条构造通路**：每个 `x-class` MUST 至少有一条——`x-constructor` / 返回自身 `$ref` 的 `x-static` / 任一 `x-function.returns` 指向它。判定面是 Rust 解析（取数器可见 `owner` + 名字）而非写闸：通路天然跨元素（父 / 兄弟元素的工厂 op 也成立），写闸的「文件级自洽、零跨元素」纪律不容纳它（见 §5）。
- **`x-class` MUST NOT 带 `additionalProperties`**（裁决 9，论证见 §2.6）：`additionalProperties` 断言「这个对象是字典（键任意、值同型、无行为）」，与「trait 仅允许方法」语义互斥，且条目层对它是零消费者——放行即静默丢。
- `x-function` 落点由二处扩为四处：`properties.<k>` / `$defs.<T>.properties.<k>` / `$defs.<T>.x-constructor.x-function` / `$defs.<T>.x-static.<名>.x-function`（后两处只在 `x-class` 条目内可达）。
- 构造器名不设槽位：构造是类型的**唯一**入口，名字不携带信息；需要别名构造（`open` / `from_path`）的类型写进 `x-static`，返回本类型即可。

### 2.4 为什么不是别的形态

- **不把 `x-class` 从 boolean 升为对象**：值类型变 `bool | object`，所有 `as_bool() == Some(true)` 落点（`schema.rs` / `emit` 两侧 / kind 判读 / 测试）都要分支，漏一处就把对象形态静默当「非 class」。
- **不把 ctor / static 挂在 object 上**：object 是纯数据、无行为、非测试主体，声明构造入口只会制造第二份真相（字段字面量已经能造）；类与数据的分界「有没有方法」因此保持唯一判据。
- **不复用模块 op + `x-function.owner`**：`properties` 键是模块命名空间，两个类型都有 `new` 时键冲突，且污染 op 计数。

### 2.5 · `x-class` MUST 带 `type: "object"`（裁决 6 的完整论证）

**结论：是。** 骨架与行为面分两处声明，且骨架必在场。

```
$defs.<T> 的两层，缺一层即静默故障
┌──────────────────────────────────────────────────────────────┐
│ 骨架  type: "object" + properties + required                  │
│        ↑ 标准 JSON Schema 能看见的形状                         │
│        （写闸判：x-class ⇒ type 必为 "object"）                │
├──────────────────────────────────────────────────────────────┤
│ 行为面  x-class: true + properties.<m>.x-function             │
│        ↑ 只有本仓的 x- 面能看见；标准校验器当未知关键字（log） │
└──────────────────────────────────────────────────────────────┘
```

四条理由：

1. **不写 `type` 的条目在标准 JSON Schema 里是「任意值」**——`{"$defs": {"Gate": {"x-class": true}}}` 对任何标准校验器都等于「这个类型可以是 42、可以是字符串」。而它是类（测试主体、有构造入口、有方法），意图是「一类对象」。骨架缺席等于把契约的可读性让给扩展键。
2. **它把「类 / 数据」分界变成机械判据**：两者共享同一个 `type: "object"` 骨架，差别只在 `x-class` 这一位。今天的隐式约定是「`type` 可省、`type_block` 的分支顺序决定形态」——`{"type": "string", "x-class": true}` 会被 `emit/rust.rs:145` 与 `emit/ts.rs:175` 的**先判 `x-class`** 静默吞成 trait/interface，`type: "string"` 无声消失。强制 `object` 后该组合在写闸即死。
3. **它是 §3 ⚠ 那条边缘的配套前置**（见下）：断言「类必然带 `type: "object"`」之后，`is_zero_field_object` 与 `x-class` 的碰撞从「偶发边缘」变成「结构性必答」——必须显式排除，不能再靠「类通常不写 `type`」的口头纪律。
4. **收窄是安全的**：trait 确实可由任意类型实现（含 enum），但那是 `impl` 关系、不是类的骨架。契约的「类」是**测试主体**（可构造 + 有方法），构造入口天然是 object 形状；把「谁实现了它」塞进 `$defs.T` 会造第二份真相。

判据写法（`api/schema.rs` 的 `verify_defs`，追加在既有两条 x-class 判据**之前**——判序即错误面，「你不是 object」比「你不能有 required」更根本）：

```
if x-class == true:
    type != "object"  → ApiError（点名 $defs.<T>.type，出路：补 "type": "object"）
    required 在场      → ApiError（原判据不变）
    properties 有非函数 → ApiError（原判据不变）
```

反例面：`{"x-class": true}`（缺 `type`）/ `{"type": "string", "x-class": true}` / `{"type": "object", "x-class": "true"}`（字符串值，按 `as_bool` 不命中 ⇒ 反方向是「未标 class」，既有警告面接住）。

⚠ 判序变更的连带影响：`schema_test.rs:829` 的「`x-class` 判据不遮蔽既有五规则」按固定判序断言先命中者。新增判据插在 x-class 分支**内部的首位**，不改动外层（嵌套 `$defs` / `$ref` / 重复键）的顺序，故该测试的 or 断言仍成立。**夹具改造实测 12 处**（`"x-class": true` 的精确实例数：`schema_test` 6 / `emit/ts_test` 3 / `emit/rust_test` 2 / `mod_test` 1）——每处都要补 `"type": "object"`；`check_test.rs` 零此类夹具（它用的是 `"x-class": "Trait"` 字符串值测 alias 兄弟键），且其「本文件零 `x-class` 判据」断言不受影响（判据落在 `schema.rs`，不在 `check.rs`）。存量**产品契约**零修改（在案 11 份零 `x-class`）。

### 2.6 · 为什么 `x-class` 必须显式拒收 `additionalProperties`（裁决 9）

**判定：拒。** 这不是「条目层还没实现 map 形态」的能力缺口，而是**语义互斥**——即使将来条目层支持了 map，`x-class` 也不该带它。

| 键 | `x-class` 条目 | `object` 条目 |
|---|---|---|
| `additionalProperties` | **语义非法** ⇒ 写闸 `ApiError` | 语义合法（具名字典是正当数据），条目层未实现 ⇒ **缺能力**（本期不动） |

三条理由：

1. **语义互斥**：`additionalProperties` 的定义是「键任意、值同型、无行为」；`x-class` 按裁决 4/5/6 是「仅允许方法的 trait」。交集是「能动态挂任意键的方法集合」——没有任何生成器能表达，也没有任何标准 JSON Schema 消费者能理解。
2. **结构上零消费者**：`trait_block`（`emit/rust.rs:207`）与 `interface_block`（`emit/ts.rs:199`）的读取面只有 `properties`；`additionalProperties` 的唯一消费点在 `value_type` 的 `type: "object"` 分支，而**条目层根本不走 `value_type`**。放行 = 静默丢。
3. **它会反噬出不可编译产物**：`is_zero_field_object`（`emit/rust.rs:482`）只判 `type == "object"` ∧ `properties` 空，不看 `x-class`：

```
{"type": "object", "x-class": true, "additionalProperties": {"type": "string"}}
        ↓ 写闸现状：放行（x-class 分支只查 required / 属性）
        ↓ render：trait_block → pub trait T {}
        ↓ is_zero_field_object → true（properties 缺省视作空）
        ↓ 自动构造器段 → impl T { pub fn new() -> Self { Self } }   ← impl 在 trait 上，E0404
```

今天这是坏的（只是没人踩）；裁决 6 强制 `type: "object"` 后，它变成**必现**。

**判据写法**（`api/schema.rs` 的 `verify_defs`，落在既有 x-class 分支内，排在裁决 6 的 `type` 断言之后、`required` 之前——「你是字典」比「你不能有 required」更根本）：

```
if x-class == true:
    type != "object"            → ApiError（裁决 6）
    contains additionalProperties → ApiError（裁决 9，点名该键 + 列出路）
    required 在场                → ApiError（原判据不变）
    properties 有非函数           → ApiError（原判据不变）
```

文案的出路 MUST 是可执行的，且 MUST NOT 把人引到另一个静默里：① 去掉 `x-class` 变纯数据（并说明条目层 map 形态本期不渲染 ⇒ 键值对容器要写在**值位**：`x-function.params[].schema` / `returns` / 字段位）；② 若本意是「方法集合」，去掉 `additionalProperties`——trait 没有实例数据面。

**只治一侧的边界**：`object` 条目上的 `additionalProperties` 仍静默（条目层 `type_block` 六种形态里没有 map，落到 `struct_block` 零字段）。那是「缺能力」不是「语义错」，属另一个变更的面，见 §9 观察 1。

## 3 · 语义与语言生成

**协议固定的只有语义**：存在一个规范构造入口、名字 `new`、有序入参表（`name` + `schema` + `optional`）、返回隐式本类型；静态方法 = 类型级可调用成员（名字 + 签名）。**目标语言语法由生成器决定**，Rust 与 TS 语法不同，协议 MUST NOT 出现任何语言字面。

当前两个生成器的落地写法（**生成器决策，不是协议判据**；未来换写法协议不变）：

- **Rust（`emit/rust.rs`）**
  - object 条目零产出：新键不在其射程内，只有既有的「零字段 object 自动 `new()`」不变。
  - `x-class` 条目把构造器 / 静态写进 trait body：`fn new(params) -> Self;` / `fn open(...) -> R;`（`method_signature` 已是现成出口）。
  - **「可直接构造」在 Rust 的精确含义**：trait 本身不可实例化，`.i.rs` 也产不出 `T::new` 的见证行（`const _: fn(…) -> super::T = super::T::new;` 对 trait 不成立）。绑定发生在 **impl 处**：实现该 trait 的具体类型必须补齐 `fn new` 且签名逐项相符——编译器强制执行，比见证行更硬、不需新机制。
  - ⚠ **裁决 6 把这条边缘变成必答题**：`is_zero_field_object`（`emit/rust.rs:482`）只认 `type: "object"` + 空 `properties`，不以 `x-class` 为排除项。既然所有类现在都带 `type: "object"`，`{"type":"object","x-class":true,"properties":{}}`（合法空 trait）必然被它命中，产出 `impl <trait> { pub fn new() -> Self { Self } }` 与见证 `const _: fn() -> super::Gate = super::Gate::new;`——两处都不可编译（trait 不是类型本体）。**裁决：判据加 `x-class != true`**，函数语义随之变成「是否由生成器自动产出零参 `new`」（名字可改为 `is_auto_constructible`），唯一消费点是 `emit/rust.rs:73` 的 `constructible` 收集。
- **TS（`emit/ts.rs`）**
  - 类保持 `export interface T { … }`（方法面）+ 追加值声明 `export declare const T: { new (params): T; open (...): R };`（类型 / 值同名共存，`isolatedModules` 下成立）；object 不产值声明（纯数据，值面就是 interface 本身）。
  - 若将来改用 `class` 语法，只改生成器。

由此得到的绑定强度：Rust 侧是 impl 处的编译期硬约束；TS 侧类型声明不做实现绑定（与现有一致）。

## 4 · 测试生成器的构造策略

**测试主体只有 `x-class`**；object 是夹具材料，不是被测对象。构造优先级：

1. `x-class` + `x-constructor` → `T::new(args)`；
2. `x-class` 无 ctor → `x-static` 中**返回本类型**的方法（**工厂兜底，已决**；多个时取声明序首个）；
3. `x-class` 两者皆无 → 走跨元素通路：任一 `x-function.returns` 指向 `T` 的 op / 方法（本元素模块工厂、父元素操作）。本条**判定属二期**（§5），一期**既不判也不提示**（裁决 7）；规则文本只进 spec 与二期笔记，**不进知识骨架**——体量按本次需要最小抬（+31 B 全花在五项枚举上），不为其再抬（§6.1「抬锚的最小口径」）。
4. object（纯数据）→ 只作夹具：`properties` + `required` 字段字面量；零字段 object 沿用自动 `new()`。

「至少一条通路」是**类的存在条件**（§2.3），不是测试生成器的运行时兜底——一期判不出来是已知窗口，不是静默降级。

## 5 · 判据面

- **一期**：`apiConstructorDeclared` 口径收窄为「模块级 op MUST NOT 用构造器名」，类型级构造器改走 `x-constructor`；判据词汇十值**不扩**。`declared_ops` / `emit` 对模块 op 的构造器过滤保留。
- **二期已移出本篇** → `.dev-team/explores/archi-api-defs-member-check.md`：`$defs` 成员按 `(owner, name)` 比对 + 「至少一条构造通路」判定。移出的硬理由是**判据面纪律**而非排期：`validate_schema` 是「文件级自洽、零跨元素」（`api/schema.rs` 模块头），而构造通路与成员存在性都是跨元素事实（父 / 兄弟元素的工厂 op 返回 `T` 也算通路；`impl` 方法散在任意手写文件）。
- 一期的已知窗口：Rust 侧由 impl 处的编译期强制兜底；TS 侧与既有形态一致（结构类型不做实现绑定）；测试生成器调用真实 API，错签名在测试编译期即红。

## 6 · 改动清单

| 面 | 文件 | 改动 |
|---|---|---|
| 写闸 / 装载 | `api/schema.rs` | `verify_defs` 增四组校验：① **`x-class` MUST 带 `type: "object"`**（裁决 6，排在既有两条 x-class 判据之前）；② **`x-class` MUST NOT 带 `additionalProperties`**（裁决 9，紧随其后）；③ **新键落点**（仅 `x-class: true` 条目，object / enum / 标量 / alias 上出现即错）；④ **新键形态**（ctor 单对象、`returns` 禁止、static 名标识符且非 `new`）。`verify_top_level_keys` 文案 x- 面 3 → 5 |
| 名字 / 落点闸 | `api/contract.rs` | `verify_x_function_positions` 二处 → 四处（后两处只在 `x-class` 条目内可达）；`x-static` 名并入「方法名」族 |
| 渲染 | `api/emit/rust.rs` / `api/emit/ts.rs` | 见 §3；`is_zero_field_object` 加 `x-class != true`（裁决 6 的配套，否则空 trait 会被自动构造器段命中） |
| 渲染分派 | `api/mod.rs` | `def_shape` 判序**不变**（`x-class` 在 `scalar` / `object` 之前）⇒ `INDEX.toml` 的 `categories` 仍记 `class`；`type: "object"` 的出现不改变计数口径 |
| 判据 | `api/check.rs` | 一期仅 `constructor_declared` 文案 / 口径；二期移出（`archi-api-defs-member-check.md`） |
| spec | `openspec/specs/desktop-archi-api/spec.md` | 关键字表 3 → 5；x-function 落点二处 → 四处；「构造器 MUST NOT 进契约」改写为「模块 op MUST NOT，类型级走 `x-constructor`」；**新增「`x-class` MUST 带 `type: "object"`」与「`x-class` MUST NOT 带 `additionalProperties`」**；新增 scenario：object 上写两键即错 / `x-class` 缺 `type` / `x-class` 带 `additionalProperties` 即错（含布尔形态）/ 空 trait 只产 trait 不产 `impl` |
| spec | `openspec/specs/desktop-archi-api-check/spec.md` | `apiConstructorDeclared` 口径与场景 |
| spec / 知识 | `openspec/specs/desktop-archi-agent/spec.md`、`crates/infra/agent/src/archi-knowledge.md`、`crates/infra/agent/src/archi_knowledge_test.rs` | 「x- 面封闭三项」→ **逐名列出五项**（MUST NOT 指向 spec）；体量锚同批抬（**本次豁免**）⇒ 四数与三处文案见 §6.1 |
| 测试 | `schema_test` / `contract_test` / `emit/{rust,ts}_test` / `mod_test` / `archi_knowledge_test` | 正例 / 边界 / 负例（含「object 上写两键」「`x-class` + `type: object` 空 properties」「`x-class` + `additionalProperties` 显式拒收」三条回归）。既有夹具零冲突（实测：`x-class` 与 `additionalProperties` 同段出现 0 次） |
| 生成物 | `.dev-team/architecture/graph/INDEX.toml`、`*.i.rs` / `*.i.ts` | `api:export` 重生成；存量契约逐字节不变；INDEX 计数口径不变 |
| 版本 | `packages/desktop/package.json` | 协议面用户可见变化 ⇒ patch |

### 6.1 · 知识骨架与体量锚（裁决 8）

**为什么不能写「见 spec」**：运行期注入给 agent 的只有 `archi-knowledge.md` 编译期内嵌的那一份字节，它够不着 `openspec/`——生产资产指向设计资产，在生产面等于「永不抵达」。这不是「导出名的唯一持有者是 schema」那条纪律的反例：那条管**导出名**，`x-` 关键字是**协议词汇**，两者的持有者不同。既有形态本来就支持同文两处：`desktop-archi-agent` spec:529 逐字写着知识文本应含的内容（「`x-` 面封闭为 … 三项」），骨架复述它——spec 是对生产资产的断言，不是生产资产的依赖。

**成本（实测）**：`x-` 面一行 +31 B ⇒ 骨架与全文各 +31 B，两处精确锚与骨架门限都要动：

| 锚 | 现值 | 改后 | 必要性 |
|---|---|---|---|
| 骨架实测（精确 `assert_eq!`） | 1777 B | **1808 B** | 必改（内容 +31） |
| 骨架门限（`<=`） | 1800 B | **1831 B** | 必改（1808 > 1800）；同比平移保住 23 B 余量 |
| 全文实测（精确 `assert_eq!`） | 1966 B | **1997 B** | 必改（派生段不变，增量全在骨架） |
| 全文门限（`<=`） | 2000 B | **2031 B** | 可选（1997 已过线但余量只剩 3 B）；同比平移保住 34 B |

三处**文案**同批（数值改了而文案不改，注释与断言互相打脸）：

1. `archi_knowledge_test.rs:4` 模块头「体量冻结（骨架 ≤ 1800 B / 全文 ≤ 2000 B）」；
2. `:138-139` 测试 doc 注释「（内容变更须同批更新，MUST NOT 抬锚）」——改为「抬锚须显式豁免并同批更新四数」，把「禁止」换成「有门槛」；
3. 两条 `assert_eq!` 的失败文案里写死的实测值与「（LF 归一，五段骨架 + 三资产分工）」括号内容。

**守卫**：`指令表收窄且新行集可判`（`:233`）的 needle 集补 `x-constructor` / `x-static`（带反引号原样）——否则未来编辑删掉一项，断言仍绿，枚举本身没有守卫。

**抬锚的最小口径**：只按本次 +31 抬，不顺手塞别的规则。「至少一条构造通路」「object ⇔ JSON 对象」这两条都**不进骨架**——前者是判据面的事（二期）、后者已由写闸强制，骨架再加只是复述。

## 7 · 验收锚

1. **存量零 diff**：11 份在案 schema（零 `x-class`、零新键）重生成后 `.i.rs` / `.i.ts` / `INDEX.toml` 逐字节不变；`categories` 计数口径不变。
2. **正例**：`{"type":"object","x-class":true, "x-constructor":…, "x-static":…}` → Rust trait body `fn new(params) -> Self;` / `fn open(...) -> R;`，TS `export declare const T: { new (params): T; open (...): R };`；**object 条目的产物逐字节不变**（新键不在其射程内）。
3. **负例（落点）**：`object` / enum / 标量 / 纯 alias 上写 `x-constructor` 或 `x-static`；两键出现在 `$defs` 之外；顶层新 `x-` 键。
4. **负例（形态 + 裁决 6 / 9）**：`{"x-class": true}`（缺 `type`）；`{"type": "string", "x-class": true}`；`{"type": "object", "x-class": true, "additionalProperties": {"type": "string"}}`（**显式拒收**，裁决 9）；`{"type":"object","x-class":true,"additionalProperties": false}`（布尔形态同拒）；`x-constructor` 带 `returns`；`x-static` 名 `new` / 非标识符。
5. **判据**：`apiConstructorDeclared` 只在模块 op 上触发；`ArchiApiFindingKind::ALL` 长度仍 10。
6. **边缘回归（裁决 6 的配套）**：`{"type":"object","x-class":true,"properties":{}}` 只产 `pub trait Gate {\n}\n`，**不产** `impl Gate { … }` 与 `const _: fn() -> super::Gate = super::Gate::new;`（§3 ⚠ 条）。
7. **良性对照**：`{"type":"object"}`（无 `x-class`、空 `properties`）仍产自动 `new()` 与其见证——裁决 6 只收窄类，不动纯数据。
8. **边界对照（裁决 9 只治一侧）**：`{"type":"object","additionalProperties":{"type":"string"}}`（无 `x-class`）**仍被写闸放行**——本期不改它的静默（条目层 map 形态是缺能力，见 §9 观察 1）；`x-class` + 同键才拒。

## 8 · 备选方案（已否决）

| 方案 | 否决理由 |
|---|---|
| `x-class` 升为对象承载 ctor / static | 值类型 `bool \| object`，落点分支多且漏一处即静默误判；且新键的落点仍要靠「是不是类」判 |
| 模块 op + `owner` 表达构造 / 静态 | `properties` 键无类型命名空间，多类型 `new` 冲突；污染 op 面 |
| `x-constructor` 带名字槽位 | 构造器是唯一入口，名字无信息量；别名构造归 `x-static` |
| ctor / static 挂在 object（推荐过的 §9.4 原案） | **已否决**：object 是纯数据、无行为、非测试主体；字段字面量已是唯一真相，再挂构造入口即第二份真相，且会打散「有没有方法」这个类 / 数据分界。**没有 `x-class` 的 object 等同于 JSON 对象** |
| `x-class` 允许不带 `type`（今天的 marker trait 写法） | **已否决**（裁决 6）：不带 `type` 的条目在标准 JSON Schema 里是「任意值」，骨架缺席；且今天的「`type_block` 先判 `x-class`」会让 `{"type": "string", "x-class": true}` 中的 `type` **静默消失**——写闸强制 `object` 是消灭这一类静默的唯一位置 |

## 9 · 未决点

§0.5 的 9 条全部收口，本期无遗留未决点。

三条**邻近观察**（不入本期，记录以免下次重新发现）：

1. **`object` 条目层的 `additionalProperties` 仍静默**（裁决 9 只治 `x-class` 一侧）：`type_block` 的六种形态（`$ref` / `x-class` / `oneOf` / `enum` / 标量 / `object+properties`）里没有 map，命中「object+additionalProperties」会落进 `struct_block` 产零字段 `pub struct T;`，同时还被 `is_zero_field_object` 当成可构造、白送一个 `new()`。同族还有条目层 `type: "array"`（同样落进 `struct_block`）。三档处置（写闸拒收 / 渲染补形态 / 只记 degraded）留给下一个变更裁。
2. `{"$ref": "…", "x-class": true}` 这类「带扩展键的 alias」不满足 `is_pure_ref_alias`（该判据要求仅一键），故进 `types` 而非 `reexports`；而 `type_block` 先判 `$ref` 返回空串 ⇒ 该条目在任何产物里都**不出现**（不定义、不报错）。既有的坑，裁决 6 不涉及它。
3. **知识骨架的体量冻结这次被豁免**（裁决 8）：`archi_knowledge_test.rs` 用「精确 `assert_eq!` + 上限」双断言锁住体量，本次按 +31 B 抬锚（四数同批）。但豁免是**逐次**的，不是把门限作废——`archi-knowledge.md` 的设计初衷是「只留不可脚本化的判断」（spec:525「散文只留不可脚本化的判断面」），所以下次要往里加东西，仍应先问「这条能不能由工具面 / 参数 `description` 承载」。
