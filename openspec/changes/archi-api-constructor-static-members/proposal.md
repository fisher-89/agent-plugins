# 提案: archi-api-constructor-static-members

> **变更**: archi-api-constructor-static-members
> **日期**: 2026-11-26
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（`workflow_type = requirement`，§0.5 的裁决表已收口原 §9 九条未决点）。目录名沿用，方案本体即 explore 的裁决结果。
> **性质**: **协议面一次原子交付**——`<叶段>.schema.json` 的 `x-` 关键字面由三项扩为五项（`x-constructor` / `x-static`），给 `x-class`（测试主体）固定一个规范构造入口与一张类型级静态方法表；同批把两处静默（`x-class` 缺骨架、`x-class` 带 `additionalProperties`）变成装载期 `ApiError`，并把知识骨架的「x- 面封闭三项」改为逐名五项（体量锚按本次需要 +31 B 抬）。
> **不含**: `$defs` 成员按 `(owner, name)` 比对与「至少一条构造通路」的机器判定（二期，`archi-api-defs-member-check`）；object 条目层的 `additionalProperties` / `type: "array"` 静默（explore §9 观察 1，属另一个变更的面）；`params[].optional` 在 Rust 侧的既有丢弃（explore 未涉，见风险 R-7）。

---

## 问题

### 一 · 构造器整体禁止：类怎么造没有槽位

构造器名今天是一个封闭集（`CONSTRUCTOR_NAMES` 六值 `new` / `default` / `build` / `empty` / `never` / `open` + `from_*` / `with_*` 两族前缀，`contract.rs:15-16`），模块 op 一旦命中就由 `apiConstructorDeclared` 判为「接口自身矛盾」（`check.rs:478-494`），两个生成器也用同一 `is_constructor` 把这类 op 从产物里剔掉（`emit/mod.rs::declared_ops`）。唯一的构造器是 `is_zero_field_object`（`emit/rust.rs:482`）为零字段 object **自动**产出的零参 `new()`（`emit/rust.rs:73` 与 `:86-92`）。

于是「这个类型怎么造」在协议里没有一个可写的槽位：有入参的构造方式只能靠 `x-function` 顶层 op 绕，绕了即违约。

### 二 · 静态方法无表达

`$defs.<T>.properties.<k>.x-function` 在 `x-class` 上是 trait 方法（Rust 渲染为无 self 的关联函数，`emit/rust.rs:207-225`），在 object 上是函数指针**字段**（`emit/rust.rs` 的 `struct_block`）。两类条目都没有「类型级可调用成员」这一位——`T.<name>(args)` 既写不出来，也没有生成物承载它。

### 三 · 写了无绑定（本期只登记，不改）

`$defs` 成员不参与判据（`check.rs` 的声明集只取模块 `properties` 的 op 键），签名比对也只覆盖模块 op。故本期新键的「实现是否真的实现了」没有判据面——二期才有（`archi-api-defs-member-check`）。

### 四 · 两处静默：骨架缺席与 `additionalProperties`

1. **骨架缺席**：`{"$defs": {"Gate": {"x-class": true}}}` 在标准 JSON Schema 里等价于「任意值」（可以是 42、可以是字符串）。更糟的是 `emit/{rust,ts}.rs` 的 `type_block` **先判 `x-class`**（`emit/rust.rs:144` / `emit/ts.rs:174`），故 `{"type": "string", "x-class": true}` 会被静默吞成 trait / interface，`type: "string"` 无声消失。
2. **`additionalProperties` 语义互斥却放行**：`x-class` 是「仅允许方法的 trait」，`additionalProperties` 断言「键任意、值同型、无行为」；条目层对 `additionalProperties` 零消费者（`trait_block` / `interface_block` 只读 `properties`），放行即静默丢。今天它还会反噬出不可编译产物：

```text
{"type": "object", "x-class": true, "additionalProperties": {"type": "string"}}
        ↓ 写闸现状：放行
        ↓ render：trait_block → pub trait T {}
        ↓ is_zero_field_object → true（properties 缺省视作空，不判 x-class）
        ↓ 自动构造器段 → impl T { pub fn new() -> Self { Self } }   ← impl 在 trait 上，E0404
```

今天这是坏的（只是没人踩）；一旦所有 `x-class` 被强制带 `type: "object"`（见提案 §三），它就从「偶发边缘」变成**必现**。

### 五 · 五项关键字名字的唯一生产持有者是知识骨架

运行期注入给 agent 的只有 `archi-knowledge.md` 编译期内嵌的那一份字节（`include_str!` + 两处 `foundation::layout` 派生路径），它够不着 `openspec/`。骨架今天写着「`x-` 面封闭为 `x-function` / `x-class` / `x-common` 三项」——名字清单若只留在 spec，生产面等于永远不会抵达。这与「导出名的唯一持有者是 schema」不是同一条纪律的冲突：那条管**导出名**，`x-` 关键字是**协议词汇**，持有者不同。

---

## 提案

### 一 · 关键字面：三项 → 五项

```
x- 面封闭五项   x-function   x-class   x-common   + x-constructor   + x-static
```

- 两个新键只允许出现在 `$defs.<T>` 条目**内部**；顶层键封闭六项不变（`verify_top_level_keys` 只需文案 3 → 5）。
- 两个新键**仅 `x-class: true` 的条目可用**：`object` / enum / `oneOf` / 标量 / 纯 `$ref` alias 上出现即 `ApiError`。object 是 schema 原有类型，代表**纯数据、无行为、非测试主体**——它的实例由 `properties` + `required` 的字面量构造，再声明构造入口只会制造第二份真相。类与数据的分界就是「有没有方法」。

```json
"$defs": {
  "Store": {
    "type": "object",
    "x-class": true,
    "properties": { "count": { "x-function": { "returns": { "type": "integer" } } } },
    "x-constructor": {
      "x-function": { "params": [ { "name": "path", "schema": { "type": "string" } } ] }
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
  "Ticket": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] }
}
```

`Store` 是测试主体（有条目方法 ⇒ `x-class`）；`Ticket` 是纯数据（无方法 ⇒ object，构造 = `{ id: "…" }`）。把两个新键写进 `Ticket` 即 `ApiError`。

### 二 · 落点与形态（四条校验）

| 键 | 形态与规则 |
|---|---|
| `x-constructor` | **单个对象**（不是「名字 → 签名」表）：`{ "x-function": { "params": [ … ] } }`。`params` 可省（= 零参）；`returns` MUST NOT 出现（隐式本类型）；内层其余键沿用「`x-function` 内层自由形状」，不设未知键闸。名字**固定 `new`**，不设名字槽位、不入名字族。 |
| `x-static` | 「名字 → `{ "x-function": { … } }`」对象：名字 MUST 是 ASCII 标识符、MUST NOT 为 `new`（保留给构造器），并入既有「方法名」族；值的内层自由形状（`params` / `returns` / `async` / `description`）。返回本类型 `$ref` 即工厂方法。 |

`x-function` 的合法落点由**二处扩为四处**（后两处只在 `x-class` 条目内可达）：

```
① properties.<k>.x-function                      模块操作
② $defs.<T>.properties.<k>.x-function            方法 / 函数指针字段
③ $defs.<T>.x-constructor.x-function             规范构造入口（新增）
④ $defs.<T>.x-static.<名>.x-function             类型级静态方法（新增）
```

构造器不设名字槽位：构造是类型的**唯一**入口，名字不携带信息；需要别名构造（`open` / `from_path`）的类型写进 `x-static`，返回本类型即可（工厂）。

### 三 · `x-class` 的骨架规则与判序

```
$defs.<T> 的两层，缺一层即静默故障
┌──────────────────────────────────────────────────────────────┐
│ 骨架  type: "object" + properties + required                  │
│        ↑ 标准 JSON Schema 能看见的形状（写闸判：x-class ⇒ type 必为 "object"）│
├──────────────────────────────────────────────────────────────┤
│ 行为面  x-class: true + properties.<m>.x-function             │
│        ↑ 只有本仓的 x- 面能看见；标准校验器当未知关键字（log）      │
└──────────────────────────────────────────────────────────────┘
```

判据落在 `verify_defs` 的既有 `x-class` 分支内，**判序固定**（判序即错误面）：

```
if x-class == true:
    type != "object"              → ApiError（缺骨架；出路：补 "type": "object"）
    contains additionalProperties → ApiError（语义互斥；见下）
    contains "required"           → ApiError（原判据不变）
    properties 有非 x-function     → ApiError（原判据不变）
```

- **`x-class` MUST 带 `type: "object"`**：把「类 / 数据」分界变成机械判据（两者共享同一骨架，差别只在 `x-class` 这一位），并消灭 §四-1 的静默（`type: "string"` 先于 `x-class` 被吞掉）。收窄是安全的：trait 确可由任意类型实现，但那是 `impl` 关系、不是类的骨架；契约的「类」是测试主体（可构造 + 有方法），构造入口天然是 object 形状。
- **`x-class` MUST NOT 带 `additionalProperties`**（对象形态与布尔形态同拒）：语义互斥（字典 vs 仅允许方法的 trait）、结构上零消费者（放行即静默丢）、且会反噬出 `impl <trait>`（E0404）。文案的出路 MUST 可执行且 MUST NOT 把人引到第二个静默：① 去掉 `x-class` 变纯数据（并说明条目层 map 形态本期不渲染 ⇒ 键值对容器写在**值位**：`x-function.params[].schema` / `returns` / 字段位）；② 若本意是「方法集合」，去掉 `additionalProperties`——trait 没有实例数据面。
- **只治一侧**：`object` 条目上的 `additionalProperties`（无 `x-class`）本期仍被放行，静默原封不动（那是「缺能力」不是「语义错」，见风险 R-6）。

### 四 · 语义与语法分工

协议固定的只有**语义**：存在一个规范构造入口（名字 `new`、有序入参表 `params`、隐式返回本类型）与一张类型级静态方法表（名字 + 签名）。**目标语言语法由生成器决定**——协议文本 MUST NOT 出现 `pub fn` / `declare const` / `constructor` 一类字面，换语言或换写法不改协议。

两个生成器的落地写法（**生成器决策，不是协议判据**）：

- **Rust（`emit/rust.rs`）**：`x-class` 条目把构造器与静态写进 trait body——`fn new(params) -> Self;` / `fn open(...) -> R;`（`method_signature` 已是现成出口）。trait 不可实例化 ⇒ `.i.rs` 不产 `T::new` 见证行；绑定发生在 **impl 处**（实现该 trait 的具体类型必须补齐 `fn new` 且签名逐项相符，编译器强制，比见证行更硬）。
- **TS（`emit/ts.rs`）**：类保持 `export interface T { … }`（形状承诺）+ 追加同名值声明 `export declare const T: { new (params): T; open (...): R };`（类型 / 值同名共存，`isolatedModules` 下成立）。**值声明只在 `x-constructor` 或 `x-static` 至少一项在场时产出**——两者皆无时值面是空对象、零信息，产出即噪声；该取舍同时保住「无新键 ⇒ 逐字节不变」这条不变量。
- **两语言共同的边缘（必答）**：`is_zero_field_object` 只判 `type == "object"` ∧ `properties` 空，不看 `x-class`。所有类现在都带 `type: "object"`，故 `{"type":"object","x-class":true,"properties":{}}`（合法空 trait）必然被它命中，产出 `impl <trait> { pub fn new() -> Self { Self } }` 与 `const _: fn() -> super::Gate = super::Gate::new;`——两处都不可编译。**判据加 `x-class != true`**，函数语义随之变成「是否由生成器自动产出零参 `new`」；唯一消费点是 `emit/rust.rs:73` 的 `constructible` 收集（函数是否改名属实现自由，本提案不约束）。
- **object 条目（无 `x-class`）零产出变化**：新键不在其射程内，零字段 object 仍自动产 `new()` 与见证行。

### 五 · 构造通路：类的存在条件（一期留白）

每个 `x-class` MUST 至少有一条构造通路：

1. `x-constructor`；
2. `x-static` 中返回本类型 `$ref` 的方法（工厂；多个时取声明序首个）；
3. 任一 `x-function.returns` 指向它（父 / 兄弟元素的工厂 op 也算——**跨元素**事实）。

判定面**本期缺席**（零判据、零提示）：通路天然跨元素，而 `validate_schema` 的纪律是「文件级自洽、零跨元素」（`api/schema.rs` 模块头；唯一例外是 `$ref` 前缀的纯计算）。故规则文本进 spec 与二期笔记，**不进知识骨架**（体量按本次需要最小抬，不为其再抬）。一期的已知窗口由两侧兜住：Rust 由 impl 处的编译期强制、TS 与既有形态一致（结构类型不做实现绑定）、测试生成器调用真实 API（错签名在测试编译期即红）。

### 六 · 判据面：口径收窄，词汇不扩

- `apiConstructorDeclared` 的语义收窄为「**模块级** op MUST NOT 用构造器名」；类型级构造改走 `$defs.<T>.x-constructor`。`declared_ops` / 两个生成器对模块 op 的构造器过滤保留。
- `ArchiApiFindingKind` 仍**十值**（`ALL` 长度 10）、`category` 仍七值、`severity` 单点、`REPORT_VERSION` 不变。
- 二期（`$defs` 成员比对 + 构造通路判定）移出本篇，硬理由是判据面纪律而非排期。

### 七 · 提案新增三条（explore 之外）

三条同源：都让「写闸放行的声明」产出不可编译产物或静默丢字段，与 §三 的 `additionalProperties` 属同一条判据逻辑（写闸消灭静默，位置类错误在装载期死、不到渲染期）。

| # | 规则 | 不写的后果 |
|---|---|---|
| N-1 | `x-class` 条目的 `properties` 键 MUST NOT 为 `new`（保留给 `x-constructor`），命中即 `ApiError` 并给出路「构造入口用 `x-constructor`」 | 与构造器同名 ⇒ trait body 两处 `fn new` ⇒ `E0428`（duplicate definitions）；今天该类条目可写且渲染成两份 |
| N-2 | `x-static` 的名字 MUST NOT 与该条目 `properties` 的键重名 | Rust 侧同一 trait body 出现两个同名关联函数 ⇒ `E0428`（两表合成一个命名空间） |
| N-3 | `x-constructor` 内层 `async` MUST NOT 为 `true` | 构造器是个值而非 future；Rust 与 TS 都没有异步构造 ⇒ 该标志只能被静默丢弃 |

三条都是**文件级自洽**事实（只需本条目），故落 `validate_schema` 同一入口；N-1 / N-2 属名字面（`contract::verify` 的名字趟），N-3 属形态面（`verify_defs` / 构造器形态校验）。

### 八 · 知识骨架：逐名五项 + 四数同批

骨架的「`x-` 面封闭三项」改为**逐名列出五项**（MUST NOT 写成「见 spec」——知识是生产资产，运行期没有 `openspec`）。成本是 `x-` 面那一行 +31 B，骨架与全文各 +31 B，两处精确锚与两处门限同批改：

| 锚 | 现值（实测） | 改后 | 必要性 |
|---|---|---|---|
| 骨架（精确 `assert_eq!`） | 1777 B | **1808 B** | 必改（内容 +31） |
| 骨架门限（`<=`） | 1800 B | **1831 B** | 必改（1808 > 1800）；同比平移保住 23 B 余量 |
| 全文（精确 `assert_eq!`） | 1966 B | **1997 B** | 必改（派生段不变，增量全在骨架） |
| 全文门限（`<=`） | 2000 B | **2031 B** | 选做（1997 已过线但余量只剩 3 B）；同比平移保住 34 B |

（基线 spec 正文两处把全文实测写作 1960 B，而 `archi_knowledge_test.rs` 的冻结断言是 1966 B——陈旧读数；本变更按**测试实测** 1966 起算，spec 侧随本次 delta 一并对齐。）

三处文案同批（数值改了而文案不改，注释与断言互相打脸）：`archi_knowledge_test.rs` 的模块头「体量冻结（骨架 ≤ 1800 B / 全文 ≤ 2000 B）」、测试 doc 注释「内容变更须同批更新，MUST NOT 抬锚」（改为「抬锚须显式豁免并同批更新四数」）、两条 `assert_eq!` 的失败文案里写死的实测值。

守卫：`指令表收窄且新行集可判` 的 needle 集补 `` `x-constructor` `` / `` `x-static` ``（带反引号原样）——否则未来编辑删掉一项，断言仍绿，枚举本身没有守卫。

**抬锚的最小口径**：只按本次 +31 抬，不顺手塞别的规则。「至少一条构造通路」「object ⇔ JSON 对象」两条都**不进骨架**（前者是二期判据面的事、后者已由写闸强制）。

### 九 · 与 explore 的偏离（逐条）

| # | explore | 本提案 | 理由 |
|---|---|---|---|
| ① | 夹具改造「实测 12 处」（`schema_test` 6 / `emit/ts_test` 3 / `emit/rust_test` 2 / `mod_test` 1） | **实测 14 处**：上述 12 处之外还有 `emit/mod_test.rs` 2 处（`:427` / `:529`） | 逐文件 `grep '"x-class": true'` 的实测值；且其中 `schema_test.rs:813` 的 `bare` 今天是**正例**（断言 `Ok`），本变更后必须翻成负例，属三类处置中最容易漏的一处 |
| ② | 行号引用 `contract.rs:14-16` / `:369-373` / `check.rs:476-494` | 实测 `contract.rs:15-16` / `:370-374` / `check.rs:478-494` | 落笔前按行号逐个复核（行号只用于定位，不作判据） |
| ③ | `is_zero_field_object`「名字可改为 `is_auto_constructible`」 | 只要求判据加 `x-class != true`，**不要求改名** | 改名零语义收益、只增 diff 面；属 dev-design 的实现自由 |
| ④ | TS 侧未区分「有 / 无 ctor+static」的值声明 | **只在两者至少一项在场时产值声明** | 空值声明零信息即噪声；同时保住「无新键 ⇒ 逐字节不变」不变量 |
| ⑤ | 把「至少一条构造通路」写成类的存在条件（无判定） | 保留为 MUST，但**明确标注判定面缺席**，并加一条「本期零判据、零警告」的可断言场景 | 无断言面的 MUST 无法验收；把「已知窗口」写成可红绿的事实（见 AC-9） |
| ⑥ | 三条新键规则只在散文里出现（`returns` 禁止 / static 名合法） | 补 **N-1 / N-2 / N-3** 三条并指出各自的不可编译 / 静默丢后果 | explore 未覆盖 `properties.new` 与 `x-static` 重名两个命名空间碰撞；见 §七 |

---

## 能力

### 新增能力

无。五项关键字面是 `desktop-archi-api` 内「关键字集」的扩张，拆成新能力会把同一份词表变成两份持有者。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-api` | **ADDED ×1 + MODIFIED ×2** | ① ADDED：`x-constructor` / `x-static` 的落点、形态、`x-class` 骨架规则与判序、语义 / 语法分工、构造通路与一期留白、两条静默的消灭（含 N-1 / N-2 / N-3）。② MODIFIED「关键字集、类型映射与错误面」：表三项 → 五项、`x-function` 落点二处 → 四处、构造器禁令改写为「模块 op MUST NOT，类型级走 `x-constructor`」、注解载体 3 → 5、映射表补投影、名字族补 `x-static`。③ MODIFIED「接口定义文件的装载校验与写闸」：⑨ 的落点计数与 `x-class` 骨架判序（闸数仍九道）。 |
| `desktop-archi-api-check` | **MODIFIED ×1** | 「契约判据词汇封闭十值与开放分类」：`ApiConstructorDeclared` 的语义行收窄为**模块级** `properties` 声明构造器名（类型级构造走 `$defs.<T>.x-constructor`）；词汇仍十值、`ALL` 长度 10、`category` 七值；新增一条「类型级构造器不产该判据」场景。 |
| `desktop-archi-agent` | **MODIFIED ×1** | 「架构会话的系统知识注入」：`x-` 面三项 → 逐名五项；体量锚四数（1808 / 1831 / 1997 / 2031）与三处文案；两条断言场景（「三条不可越界逐条在场」的 needle 集、体量实测）同批。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-drift` | 判据词汇、`ArchiDriftKind` 五值、报告槽位、`status` / `passed` 推导逐字不变：本变更只在既有 `apiFindings` 槽位里改一条判据的语义，不扩词汇、不改桶。 |
| `desktop-archi-model` | 写回原语族、parser、`write_atomic`、写面反馈面零改动——本变更不触 `.dev-team/architecture/graph/**`。 |
| `repo-archi-model` | 模型数据零变化：55 元素 / 129 关系 / 六个文件表头计数逐字不变（`reports/archi/20261126T0418228619184Z.json` 基线：`status = clean`、0 errors / 0 warnings / 0 apiFindings）。 |
| `desktop-archi-contract-panel` / `desktop-archi-canvas` / `desktop-archi-gate` | `INDEX.toml` 的 `categories` 口径不变（`api/mod.rs::def_shape` 判序不变 ⇒ `x-class` 条目仍记 `class`）；前端零 UI 变化。 |
| `desktop-ipc-type-bindings` / `desktop-app-shell` | 零 DTO / 零命令 / 零 IPC 面变化：`bindings:export` 应零 diff。 |
| `desktop-archi-agent` 的工具面段（矩阵 / 档位 / 挂载清单 / 参数） | 零变化：不新增工具、不改 `archi_api` 三动作（`bootstrap` / `link` / `check`）、不改 `archi_scaffold` 词典。 |
| `desktop-archi-chat` / `desktop-phase-prompts` / `desktop-write-protection` / `desktop-agent-execution` | 与本变更无交集（改的是协议词汇与判据语义，不是工具面 / 写权 / 提示词）。 |
| `desktop-checks-domain` / `desktop-test-execution` / `desktop-corpus-regression` / `desktop-mutation-testing` | 门禁链与测试编排零改动（`check` 四段与 `api:check` 命令串逐字不变）。 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` | `verify_defs` 的 `x-class` 分支增四类判据（骨架 `type` / `additionalProperties` / N-3 的 ctor `async` / 既有两条保留），判序按提案 §三；`verify_top_level_keys` 文案 x- 面 3 → 5。九道闸闸数不变（新规则落 ⑨）。 |
| `.../crates/core/archi/src/api/contract.rs` | `verify_x_function_positions` 落点二处 → 四处（③ `x-constructor.x-function` / ④ `x-static.<名>.x-function`，只在 `x-class` 条目内可达；`x-function` 自身子树仍转假）；`verify` 的名字趟增 `x-static` 名（并入「方法名」族，族别数仍七）与 N-1 / N-2 两条保留名判据；`is_constructor` 双向用法（`declared_ops` / 判据）逐字不变。 |
| `.../crates/core/archi/src/api/emit/rust.rs` | `trait_block` 渲染 `x-constructor` → `fn new(params) -> Self;`、`x-static` → `fn <名>(params) -> R;`（复用 `method_signature`）；`is_zero_field_object` 增 `x-class != true`（空 trait 不再被自动构造器段命中）；见证段零新增（trait 不出 `T::new` 见证）。 |
| `.../crates/core/archi/src/api/emit/ts.rs` | `interface_block` 保持现状；命中 `x-constructor` / `x-static` 时追加值声明 `export declare const T: { … };`（`new (params): T; <名>(…): R;`），两者皆无时零产出；object 条目不产值声明。 |
| `.../crates/core/archi/src/api/check.rs` | `constructor_declared` 的文案与口径收窄为「模块级 op」；指向出路 `$defs.<T>.x-constructor`。判据词汇、求值序、`category_of` 零改动。 |
| `.../crates/infra/agent/src/archi-knowledge.md` | `x-` 面「三项」→ 逐名五项（+31 B）；其余段落逐字不动。 |
| `packages/desktop/package.json` | `version` patch +1（0.7.18 → 0.7.19）；`api:export` / `api:check` / `check` 三条命令串逐字不变。 |

### 测试文件

| 文件 | 改动 |
|---|---|
| `.../api/schema_test.rs` | 新增：`x-class` 缺 `type`（`{"x-class": true}`）/ `{"type": "string", "x-class": true}` / `x-class` + `additionalProperties`（对象与布尔形态）/ ctor 带 `returns` / ctor `async` / static 名 `new` / static 名非标识符 / 新键落点（object / enum / 标量 / alias）/ 顶层新 `x-` 键各一条负例。既有夹具：`:797` / `:806` / `:835` / `:881` / `:1757` 五处补 `"type": "object"`（否则先命中「缺骨架」而非被断言的那条原因）；**`:813` 的 `bare` 由正例翻为负例**（今天断言 `Ok`，本变更后必 `Err`）。 |
| `.../api/contract_test.rs` | 落点四处的正反例（③④ 合法、`items` / `additionalProperties` / `x-function` 子树内非法）；`x-static` 名并入方法名族的反例；N-1 / N-2 各一条。既有「本文件零 `x-class` 判据」类断言不受影响。 |
| `.../api/emit/rust_test.rs` | 正例：ctor + static 的 trait body 逐字；边缘回归：`{"type":"object","x-class":true,"properties":{}}` 只产 `pub trait Gate {\n}\n`，**零** `impl Gate`、零 `const _: fn() -> super::Gate`；良性对照：`{"type":"object"}` 仍产自动 `new()` 与见证。夹具 `:345` / `:363` 补 `"type": "object"`。 |
| `.../api/emit/ts_test.rs` | 正例：interface + 值声明逐字；无 ctor/static 的类零值声明；object 零值声明。夹具 `:283` / `:1080` / `:1839` 补 `"type": "object"`。 |
| `.../api/emit/mod_test.rs` | 夹具 `:427` / `:529` 补 `"type": "object"`（后者是「带属性即在 `verify_defs` 判错」的复现，补齐后仍断言同一原因）。 |
| `.../api/mod_test.rs` | `categories` 夹具（`:1748`）两处 `x-class` 补 `"type": "object"`；断言 `Class` 计数与 `categories` 口径逐字不变。 |
| `.../api/check_test.rs` | `apiConstructorDeclared` 只在模块 op 触发；`x-class` + `x-constructor` 零判据；`ALL` 长度仍 10。 |
| `.../crates/infra/agent/src/archi_knowledge_test.rs` | 四数同批（1808 / 1831 / 1997 / 2031）+ 三处文案（模块头 / 测试 doc / 两条失败文案）；`三条不可越界逐条在场` 的 needle 集改五项逐名；`指令表收窄且新行集可判` 补 `` `x-constructor` `` / `` `x-static` ``。 |

### 生成物

| 产物 | 期望 |
|---|---|
| `.dev-team/architecture/graph/INDEX.toml`、`**/*.i.rs`、`**/*.i.ts` | `api:export` 重生成后**逐字节不变**（在案 11 份 schema 零 `x-class`、零新键；`categories` 口径不变）。`api:check` 零差异、退出码 0。 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/**`（模型、生成物、ADR） | 零数据变化（55 / 129 / 表头计数逐字不变）；本变更不新增元素 / 关系。 |
| `openspec/specs/**` 基线、本 change 的 `explore.md` | delta-only：手改基线即「同一事实两份持有者」；explore 只读。基线内旧文本（三项枚举、十值表里的构造器语义行）由本 change 的 delta 在归档合并时替换，基线保持原样是本 change 的判据。 |
| `.../crates/core/archi/src/api/mod.rs`（`def_shape` / `categories_of`） | 判序不变 ⇒ `INDEX.toml` 的 `categories` 仍记 `class`、`types` 计数不变。 |
| `.../crates/core/archi/src/api/extract/**`、`paths.rs`、`diff.rs`、`render.rs` | 取数面、路径面、差异面与本变更零交集。 |
| `.../crates/core/archi/src/api/emit/mod.rs` | `render_target` 的分派、`warn_missing_x_class`、`local_defs` / `reexport_defs` 逐字不动（新键不改变条目分类）。 |
| `.../crates/infra/agent/src/sdk/**`（`mount.rs` / `archi_tool.rs` / `policy.rs` / `compose.rs` / `runner.rs`） | 工具面零变化：不新增工具、不改参数 schema、不改档位与矩阵。 |
| `.../crates/infra/agent/src/archi_knowledge.rs` | 模块头未写体量/枚举数，`build` 的拼装与降级语义零改动。 |
| 前端产品代码（`packages/desktop/src/**` 除 `types/generated/`） | 零 UI 变化、零 DTO 变化 ⇒ 生成物零 diff。 |
| `src-tauri/Cargo.toml` / `tauri.conf.json` | crate 版本行 MUST NOT 跟随 `package.json`（`AGENT.md` 版本纪律）。 |
| `AGENT.md` / `CLAUDE.md` | 「模块 API 契约」节不含 `x-` 关键字枚举，本变更不触。 |
| `.../crates/core/archi/src/edit/**`、`crates/infra/{store,exec,vcs,watch}/**`、`crates/core/config/**` | 与协议面无关。 |

---

## 验收标准

1. **AC-1 · 存量零 diff**：11 份在案 schema（零 `x-class`、零新键）经 `api:export` 重生成后 `.i.rs` / `.i.ts` / `INDEX.toml` **逐字节不变**；`categories` 计数与 `types` / `ops` / `degraded` 三项读数不变；`api:check` 零差异。
2. **AC-2 · 正例生成（Rust）**：`{"type":"object","x-class":true,"x-constructor":…,"x-static":…}` 的 `.i.rs` 在 trait body 内产 `fn new(params) -> Self;` 与 `fn open(params) -> R;`（按声明序），**零** `impl T { … new() … }`、**零** `const _: fn(…) -> super::T`。
3. **AC-3 · 正例生成（TS）**：同上条目的 `.i.ts` 产 `export interface T { … }` + `export declare const T: { new (params): T; open (params): R; };`；**无** ctor / static 的 `x-class` 与任何 object 条目**零值声明**（产物与今天逐字节相同）。
4. **AC-4 · 落点负例**：`object` / enum / 标量 / 纯 `$ref` alias 上写 `x-constructor` 或 `x-static` ⇒ `ApiError` 点名 `$defs.<T>.<键>` 与出路；两键出现在顶层 / `properties.<k>` / `properties.<k>.x-function` 内 ⇒ 各一条 `ApiError`；顶层新 `x-` 键 ⇒ 顶层键封闭文案列五项。
5. **AC-5 · 骨架与语义互斥负例**：`{"x-class": true}`（缺 `type`）/ `{"type": "string", "x-class": true}` / `{"type": "object", "x-class": true, "additionalProperties": {"type": "string"}}` / `…"additionalProperties": false` 四形态各 `ApiError`，文案含可执行出路（补 `"type": "object"` / 去掉 `additionalProperties` 并说明条目层 map 形态本期不渲染）；`x-class` 的 `type` 与 `additionalProperties` 两条判据排在 `required` 与属性判据**之前**（判序可断言）。
6. **AC-6 · 新键形态与保留名负例**：ctor 带 `returns` / ctor `async: true` / static 名 `new` / static 名非标识符 / `x-class` 条目 `properties` 含 `new` / `x-static` 名与该条目 `properties` 键重名 —— 六形态各 `ApiError`（N-1 / N-2 / N-3 可逐条红绿）。
7. **AC-7 · 空 trait 边缘回归**：`{"type":"object","x-class":true,"properties":{}}` 只产 `pub trait Gate {\n}\n`；源码层 `is_zero_field_object` 的判据含 `x-class` 排除（`grep` 可判），`emit/rust.rs:73` 的 `constructible` 收集是唯一消费点。
8. **AC-8 · 良性对照**：`{"type":"object"}`（无 `x-class`、`properties` 缺省）仍产 `impl T { pub fn new() -> Self { Self } }` 与 `const _: fn() -> super::T = super::T::new;`；`{"type":"object","additionalProperties":{"type":"string"}}`（无 `x-class`）**仍被写闸放行**（裁决 9 只治一侧，本期不改它的静默）。
9. **AC-9 · 判据面**：`apiConstructorDeclared` 只在**模块级** `properties` 命中构造器名时产条目；`x-class` + `x-constructor` 零 `apiConstructorDeclared`；`ArchiApiFindingKind::ALL` 长度仍 **10**、`category` 仍七值、`severity()` 不动；无构造通路的 `x-class`（无 ctor / 无返回自身的 static / 无 op 指向它）**零判据、零警告**（本期已知窗口，可红绿）。
10. **AC-10 · 知识骨架**：骨架 `lf_bytes(trim_end)` = **1808 B** 且 ≤ 1831 B；全文 = **1997 B** 且 ≤ 2031 B；四项与三处文案同批（模块头 / 测试 doc / 失败文案）；`x-` 面**五项逐名**在场（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`），且骨架内零「见 spec」类指路；`指令表收窄且新行集可判` 的 needle 集含两新键（删任一项即红）。
11. **AC-11 · 夹具面收敛**：`grep '"x-class": true'` 在 `api/**` 下逐处处置完毕——正例与外层负例全部带 `"type": "object"`；`schema_test` 的 `bare` 用例已翻为负例并断言「缺 `type`」；五处既有负例断言的原因文本未因判序变更而漂移（逐条点名 `properties` / `required` / `additionalProperties`）。
12. **AC-12 · 版本与改动集**：`package.json` `version` 相对落地时 `HEAD` 恰 +1 patch；`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（另加本 change 目录下的 `proposal.md` / `specs/**` / `reports/**`）；`openspec/specs/**` 基线零改动、`src-tauri/Cargo.toml` 逐字不变。
13. **AC-13 · spec delta 形态**：`openspec/changes/archi-api-constructor-static-members/specs/` 恰**三个**能力目录（`desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-agent`）；MODIFIED 块为基线整段拷贝后改（与基线 diff 只含本提案声明的差异），未丢任何未冲突条目；每条 `### Requirement` 带 SHALL / MUST 且至少一个 `#### Scenario:`（4 个 `#`）。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **夹具实测与 explore 的记录不一致**：explore 记 12 处 `"x-class": true`，实测 14 处（多 `emit/mod_test.rs` 2 处）。 | 以 `grep` 实测为准（AC-11）；三类处置分开：正例 / 外层负例补 `"type": "object"`、`bare` 正例翻负例。 |
| R-2 | **判序变更遮蔽既有断言**：`type` 判据插在 `x-class` 分支首位，`{"x-class": true, "properties": {...}}` 会先报「缺骨架」而非「声明了属性」，`schema_test` 的 `error.contains("properties")` 直接失效。 | 负例夹具一律补齐 `"type": "object"`，只留 `bare` 一条专测缺骨架（AC-11）；`verify_defs的x_class不吞并既有规则` 的 or 断言因新判据只落分支内部而仍成立。 |
| R-3 | **`type: "object"` 是对既有键的收紧**（不是新键可选）：外部工作区存量 schema 里若已有不带 `type` 的 `x-class`，本变更后写闸会拒。 | 这是**有意的**收窄（explore 裁决 6），代价是一行 `"type": "object"`；错误文案必须给可执行出路且不得把人引到第二个静默（AC-5）。仓内零受影响（11 份 schema 零 `x-class`）。 |
| R-4 | **空 trait 必产不可编译 `impl`**：`is_zero_field_object` 若不与 `x-class` 排除同批改，`{"type":"object","x-class":true,"properties":{}}` 必产 `impl <trait>` + 见证（E0404）。explore 已定位，但它是「裁决 6 的配套」而非独立需求，最易被当可选项延后。 | 列入 AC-7 与实现文件表，与 `type` 断言**同提交**；判据加在唯一消费点上游（`emit/rust.rs:73`）。 |
| R-5 | **二期窗口被误读成一期义务**：有人会把「至少一条构造通路」当写闸判据实现，从而把跨元素事实塞进 `validate_schema`（违反「文件级自洽、零跨元素」）。 | 提案 §五与 spec 明确标注判定面缺席；AC-9 反向断言「无通路 ⇒ 零判据、零警告」。 |
| R-6 | **顺手扩大射程**：`object` 条目层的 `additionalProperties` / `type: "array"` 仍静默（explore §9 观察 1），容易被「既然改了 `verify_defs` 就一起修」。 | AC-8 反向断言该形态仍放行；`type_block` 的六形态属下一个变更的面。 |
| R-7 | **`params[].optional` 在 Rust 侧静默丢**：`method_signature`（`emit/rust.rs`）不读 `optional`，而 TS 的 `function_type` 读——这是 `$defs` 方法的既有不对称，ctor / static 复用同一出口即继承它。 | 本期**不治、不新增判据**（治它属 `x-function` 参数面的统一变更，会扩到既有方法）；记在此处，避免下次重新发现。 |
| R-8 | **知识体量锚是逐次豁免**：本次 +31 B 抬四数，若被当「门限可以随时抬」，会侵蚀「只留不可脚本化判断」的设计初衷。 | 抬锚最小口径写进 spec（只按本次 +31，不顺手塞规则）；两次**不进**骨架的候选（构造通路、object ⇔ JSON 对象）在本提案里显式点名拒绝。 |
| R-9 | **知识文本与 spec 双持有一份枚举**：五项逐名同时出现在骨架与 `desktop-archi-agent` spec（后者是对生产资产的断言）。 | 这是既有的同文两处形态（spec 断言生产资产，不是它的依赖）；两处同批改、数值与 needle 断言互为守卫（AC-10）。 |
| R-10 | **`x-static` 与 `properties` 的命名空间合并**：两表在 Rust 合成同一 trait body，重名即 `E0428`；explore 未提。 | 提案 N-2 在装载期拒收（文件级事实，不跨元素）；AC-6 红绿。 |
