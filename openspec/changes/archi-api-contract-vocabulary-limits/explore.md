# 契约词汇的四道边界：Archi 补全后「能说什么」与「说不出口的那三类」

workflow_type: requirement

> 现场：`Backend.Core.Archi.schema.json` 已在盘（12 份 schema 齐），`INDEX.toml` 里
> `Backend.Core.Archi` = `declared / 2 ops / 0 types / 0 degraded`。
> 问题不是「写得对不对」，是「词汇能表达什么、断在哪、补一手要付什么」。

## 0. 本文档归属（一轮变更一份文档）

**本文档 = 变更 1「类型出生能力」**（A1 + B + C + D + F + `oneOf+const`）。§30.1–30.7 /
§31.3 即该变更的提案骨架与验收闸。

已拆出本文档范围、另开文档的内容：

| 去向 | 内容 | 建议文档名 |
|---|---|---|
| 变更 2 | E（`$ref` 可解析 ⇒ `Named`）+ G（`async` 短形与见证分叉）——零现网影响，独立一轮 | `archi-api-signature-parity` |
| 待触发 | A2 TS 值对象（`export declare const` + 变体 JSDoc）——等第一个 TS 元素真要声明封闭枚举 | `archi-api-ts-variant-docs` |
| 不做（记为已知留白） | 效果面（`pure/read/write/spawn/cancel`）、「未声明面」分母口径、形状判据、元组与别名词汇 | — |

下文 §1–§29 是探索过程与裁定依据（含被否方案及其理由），**保留不删**：它们解释了
「为什么范围只到这里」。凡属已拆出的条目，正文内仍按原样记录，但不进变更 1 的范围。

## 1. 一句话结论

契约面有**两套消费者**，它们对同一份 schema 读的是不同侧面：

```
                        ┌── signature_of()  → ApiType 短形 ──► apiSignatureMismatch（判据，error 档）
schema.json ──► Contract ┤
                        └── value_type()   → 语言文本形 + 见证行 ──► lib.i.rs（编译期 E0308）
                                                                      lib.i.ts（无绑定，只有声明）
```

两个场景在不同消费者上断的位置不一样，而且**修法也不一样**；任何一类「补词汇」都必须两处
同时改，只改一处会得到「判据绿了但生成物编译不过」或反之。

## 2. 词汇现状（能写什么）

`schema.json` 的封闭面（spec `desktop-archi-api`「关键字集、类型映射与错误面」）：

| 层 | 封闭内容 |
|---|---|
| 顶层 | 六键：`$schema` / `$id` / `title` / `$defs` / `properties` / `$comment` |
| `x-` 面 | **恰五项**：`x-class` / `x-function` / `x-common` / `x-constructor` / `x-static`（自造 `x-` 键 = `ApiError`） |
| `properties.<k>` | 二选一：`{x-function}` 或 `{$ref: #/$defs/<名>}`（内联 object 拒收） |
| 值 schema（`params[].schema` / `returns` / 字段位） | 标准 JSON Schema 关键字 + `outcome` 协议键；**键不封闭**，未知键静默忽略 |
| `x-function` | `params` / `returns` / `async` / `owner` / `description`；**内层不判未知键** |

值 schema 的**实际读者**（`emit` 与 `extract`）只认这些形态：

```
value_type / schema_type 的识别序
  $ref              → 契约侧 ApiType::Unknown(ref末段)   ≠  代码侧 ApiType::Named(名)
  x-common 协议键    → common.json 表（首期唯一键 outcome → Outcome）
  nullable 三分支     → Optional<…>
  type: string       → Param: str / Return·Field: String
  type: integer/number/boolean
  type: array        → list<T> / &[T] / Vec<T>
  type: object + additionalProperties → dict / BTreeMap
  其它               → serde_json::Value / ?raw（兜底）
```

`ApiType` 十一个变体里，**`Named` 在契约侧零生产者**（全区只有 `extract/rust.rs:671`、
`extract/ts.rs:404` 两处，都在代码侧）。这就是后面第 4 节那类断口的根源。

## 3. 场景一：返回 `&str` 的派生助手

实例就是本 crate 的 `api::paths::leaf_name(&str) -> &str`（`paths.rs:284`）。

```
             契约侧                          代码侧
入参 &str  → Position::Param  → Str("str")   convert_type(&str) → Str("str")        ✅ 同形
返回 &str  → Position::Return → OwnedString("String")   &str → Str("str")          ❌ 恒不等
生成见证    → fn(&str) -> String = …          实际 fn(&str) -> &str                 ❌ E0308
```

- 断点在**位置分派表**：`{"type":"string"}` 的返回位被硬编码成 `String`
  （`emit/rust.rs` 的 `value_type`、`extract/mod.rs:528-530` 两处同源）。
- 两条链同时红：判据 `apiSignatureMismatch`（`error` 档，进 `passed`）+ 见证行 E0308。
- 语义面：`&str` 返回是**真承诺差异**（零分配、借用来源），不是噪声；但见证行
  `fn(&str) -> &str` 是全称量化形态，`&'static str` 与 `fn(&'a str) -> &'a str` 都能过，
  词汇顶多承诺到「借来的」，承诺不到「借自谁」。
- 最小补齐（两处一起改）：值 schema 上加一个位置无关的标记（如
  `{"type":"string","borrowed":true}` 或 `x-common` 之外的专用键），
  `schema_type` → `Str`、`value_type(Return)` → `&str`。
- 不能走的路：不要往 `common.json` 塞规则——它的摘要进**全部**生成物头注释
  （`common.json@sha256:…`），加一条即全量重生成 + 零 diff 门大噪声；且它的语义封闭为
  「跨语言**映射规则**」，`&str` 是 Rust 单侧形态。

## 4. 场景二：`Path` / `PathBuf` / 具名类型

- **判据侧**：没有任何写法能产出 `ApiType::Named`。`$ref` 明确落 `Unknown`，且这是
  **有测试守的设计**（`extract/mod_test.rs:1046` 起：「MUST NOT 把 `$ref` 归成 `Named`
  （第二套命名规则会让留痕失效）」）。短形因此是 `Path` vs `?Path`，恒不等。
- **生成侧**：`$ref` 只取末段裸名，`pub struct` 字段 `Vec<ArchiNode>` 在同元素内尚可，
  跨元素/跨 crate 的裸名在 `#[path] mod` 里**不可解析**（词汇没有 import 位）。
- Archi 的 crate 根出线里这类占绝大多数：`schema_fqn(&Path)`、`workspace_render(&Path)`、
  `validate_schema(&Value)`、`compare(&Model,&Model)`、`check(ApiCheckInput)`…
  142 个 `pub fn` 里只有 2 个是「入参/返回纯标量且能被 `&str/String → bool` 这套词说出」。
- 这是**语言专属形态表**，与 `common.json` 的「协议名 → 各语言槽位」同构。真要补，
  合理形态是把「具名类型」当成协议键的一等公民（`{"x-common":{"ref":"Path"}}` 或
  `{"$ref":"#"}` 加一张 `named.json`），而不是在 `$ref` 上打补丁。

## 5. 场景三：深层子模块的同形标量算子

先纠正一个容易读错的因果：**判据侧根本没有障碍**。

- `extract_crate` 从入口沿 `mod` 树递归，`pub mod` 链可达即 `Visibility::External`
  ——`api::paths::normalize_root` 早就在「代码导出集」里了。
- ⑤ `apiMissing` / ⑥ `apiSignatureMismatch` 都是**按名字**匹配
  （`check.rs:293`、`check.rs:443`），`owner` 完全不参与判据。
- 所以这批算子现在就能写进 `properties`，判据会绿。

唯一红的是**见证行的路径语法**：

```
owner 缺省            → const _: fn(…) = super::<op>;             需要 op 在 crate 根出线
owner: "LocalArchiCheck" → const _: … = super::<owner>::<op>;     单段标识符（is_identifier 闸）
要表达的形态           → super::api::paths::leaf_name             两段以上，被名字闸拒
```

三条出路与代价：

| 方案 | 做法 | 代价 | 风险 |
|---|---|---|---|
| A 多段 `owner` | 名字闸从「标识符」放宽为「`::` 分隔的标识符路径」 | 见证行 `super::api::paths::x` 语法成立（`super::` 在 `#[path] mod` 内 = crate 根） | 契约**钉住内部模块布局**：改名/搬文件即契约红；与「零 FQN 派生」的既有纪律相抵 |
| B crate 根补 `pub use` | 一行一个，`owner` 缺省 | 承诺面显式化（这些 fn 本来就已经 `External`，没有真正扩大） | 极小；与 `lib.rs` 现有的 `pub use api::paths::{is_fqn, schema_fqn}` 同款 |
| C 现状留白 | 只进 `.md` 散文 | 零 | 「承诺面 = 2 个函数」看起来像设计，其实是词汇决定的 |

另有一处现存歧义值得单独记：`owner` 不参与判据 ⇒ 契约里写一个 `render` 会按**声明序**
撞上 `emit::rust::render` 或 `emit::ts::render` 中的第一个（`ApiSymbol.file` 已经带了文件，
却没人用）。要么 `owner`/文件进匹配键，要么在 spec 里明说「同名不同模块不可声明」。

## 6. 场景四：`$defs` 为空与「引用已有类型」

`$defs` 现在有**两档**语义，缺第三档：

```
$defs.X                             语义                      生成
─────────────────────────────────────────────────────────────────────────────
{type:object, properties:{…}}        定义（生成器拥有）          pub struct X {…} / export interface X
{$ref:"../子/X.schema.json#/$defs/X"} 透传（跨元素，纯 alias）    pub use <module_path>::X
── 缺 ──────────────────────────────────────────────────────────────────────
{手写已定义，契约只登记名字与出处}    本地引用                    （不出定义，最多出注释/use）
```

Archi 的出线合成类型（`ArchiGraph` / `ArchiFileTree` / `ArchiDriftReport` / `ApiFinding`…）
属于第三档：它们全在 `dto.rs` 手写、带 `serde` 与 `specta::Type` 派生、内部互指。写进
`$defs` 会变成「生成器再定义一份」——接线后 `E0255`（`infra/archi` 的负向用例
`runner_test.rs:989` 正是守这条：「手写侧零 `pub struct LocalArchiCheck`」）。

三个独立成因，别混为一谈：

1. **定义归属**：`$defs` = 生成器定义。已手写类型无法只「登记」。
2. **引用已有类型**：字段位的 `$ref` **是可表达的**（`value_type` 直接取末段名，跨 `$defs`
   互指有测试语料），真正缺的是①签名位（落 `Unknown`）②跨模块裸名的 import 面。
3. **形状无判据**：`missing_contracts` 只按**名字**核对类型存在性，`$defs` 的字段形状
   没有任何判据读。所以「就地重抄成 `{type:object}`」会**全绿**——这才是「伪形状比不写更坏」
   的确切机制：它不会被抓，只会误导。

> 顺带：`Archi.md` 里「$defs 为空因为词汇没有『引用已有类型』的落点」在字段位这句不成立
> （互指是通的），成立的是签名位与「定义 vs 引用」这条分档。差别值得在文档里改一句，否则
> 下一个人会照着「不可能」去绕过。

## 7. 元问题：这类债在账上看不见

- `x-common.degraded` 的四值词表挂**在某个类型位上**；整个 op / 整个类型被留白时，
  **没有位置挂标记**。
- `INDEX.toml` 的 `degraded` 是「标记出现次数」⇒ Archi 恒 `0`。架构页「契约」视图读的
  也是这几个数字。于是「2 ops / 0 types / 0 degraded」读起来像「这模块就两个函数、
  没有债」，而真实是「142 个 `pub fn`、32 个出线类型，其中 2 个能被词汇说出」。
- 唯一记账处是 `<叶段>.md` 的「契约面留白」段——**读面零解析散文**，所以它永远进不了指标。
- 可选的机器化口径（都不需要碰生成链）：`INDEX` 增两列「出线 op / 出线 type 数」
  （取数器本来就有，`apiUndeclared` 恒 `off` 使这个分母从未被用过），覆盖率即
  `declared_ops / surface_ops`；分母非零而声明数为零时，架构页把「留白」显式标出来。

## 8. 三条已踩到的链路债（不是词汇债）

1. **`.i.ts` 末尾空行 vs 格式化器**：`emit::ts` 多出空行 → `vp check` 剥掉 →
   `api:check` 零 diff 门判 `mismatched` ⇒ `client:check` 与 `api:check` 二选一
   （`Desktop.Components.Agent.AgentError.md` 实测记录）。这直接挡住了「能自洽表达」的
   `$defs` 写盘。
2. **生成物未接线**：`core/archi` 与 `core/foundation` 的 `lib.i.rs` 都在盘，但 `lib.rs`
   没有 `#[path] mod`——两行见证目前只是文本。只有 `infra/archi` 接了
   （`lib.rs:16` `#[path="lib.i.rs"] mod api_gen; pub use api_gen::*;`）。
3. **TS 侧零编译期绑定**：`.i.ts` 只出 `export type` 声明，没有 Rust 那种见证行；所以
   TS 元素的契约只有「判据」一道锁，Rust 元素有两道。

## 9. 决策矩阵（要补的话，先补哪条）

| 目标 | 改动面 | 收益 | 风险 |
|---|---|---|---|
| 借用返回标记（场景一） | `schema_type` + `value_type` + spec 五键表 | 解锁 `leaf_name` / `ts_comment_text` 一类 | 位置分派表多一条分支，两侧必须同步 |
| `$ref → Named`（场景二前半） | `schema_type` + spec（推翻既有测试的裁定） | 解锁全部具名类型签名 | 「引用不可解析」的留痕会退化，需先有可达性判定 |
| 具名/借用形态表（场景二后半） | 新资产或扩 `common.json` | `&Path` / `PathBuf` / `Cow` 一族 | 头注释摘要变化 → 全量重生成 |
| 多段 `owner`（场景三） | 名字闸一族 + spec | 十余个深层算子可直接声明 | 契约耦合内部布局 |
| 本地引用档 `$defs`（场景四） | `Contract::from_schema` 分档 + 两渲染器 + spec 五键 | Archi 的出线类型可登记 | 必须**禁结构关键字**，否则回到伪形状 |
| 覆盖率列（元问题） | `INDEX` 两列 + 架构页 | 债从散文变数字 | 需定义「出线」口径（`pub mod` 链可达） |

## 10. 待选方向

- **稳**：先把「本地引用档 `$defs`」做出来（不改判据、只改分档与渲染），让 Archi 的 32 个
  出线类型至少进账；同时把 `.md` 里那句「不可能引用」改准。
- **狠**：动 `ApiType` 的生产面（`$ref → Named` + 借用标记），让判据词汇与代码词汇真正同构
  ——收益最大，但要先想清「不可解析」的留痕去哪儿。
- **元**：不动词汇，先把「覆盖率」变成 `INDEX` 的列与架构页的数字，让每一次留白都被看见；
  词汇扩展等有了分母再谈优先级。

---

## 11. 选定方向深挖：本地引用档 `$defs`

目标：让「手写已定义、且跨出元素边界」的类型能被契约**登记**，但**不由生成器定义**。
不动判据词汇（不动十条判据的档位与求值），只在 `$defs` 分档上加第三档。

### 11.1 三档的最终形态

```
$defs.X
──────────────────────────────────────────────────────────────────────────────
① {"type":"object", "properties":{…}}            定义      → pub struct X {…} / export interface X
② {"$ref":"../子/X.schema.json#/$defs/X"}        透传      → pub use <module_path>::X
③ {"x-existing": true}                           登记      → 什么都不出（只登记名字）
```

③ 的语义一句话：**「本元素的手写代码里已经有这个名字，且它跨出边界」**。
「出处」被隐含成「同元素的手写代码」——这正是它要表达的事实，所以不必也不该写模块路径
（`INDEX.toml` 的既有纪律是「零语言词」，写 `crate::dto` 会把 Rust 词带进契约资产）。

### 11.2 改动面（逐文件，按依赖序）

| 位置 | 改动 | 备注 |
|---|---|---|
| `api/schema.rs::verify_defs` | ③ 形态判据：`{"x-existing": true}` **且仅此一键**（`$ref` / `type` / `properties` / `enum` / `const` / `items` / `oneOf` / `required` / `x-class` 一律拒） | 照抄②「透传 MUST 纯 alias」那条的力度，防伪形状 |
| `api/contract.rs::verify` | ③ 条目跳过 `x-class` 骨架四条、`$required ⊆ properties` 等判据 | 它没有结构可判 |
| `api/emit/mod.rs::local_defs` | 过滤掉 ③（否则两渲染器会把它当 `object` 出 `pub struct X {}`） | **单点**过滤，两语言同时生效 |
| `api/emit/{rust,ts}.rs` | 无需改（经 `local_defs`）；可选出一行 `// 登记自手写定义，不生成` | 保持零编译风险 |
| `api/check.rs::declared_types` | **无需改**（③ 不是纯 alias ⇒ 已进存在性核对） | 名字回归守卫自动生效 |
| `api/check.rs::shadowing` | ③ 进父/子 `$defs` 交集判定（同名仍 `apiShadowing`） | 名字冲突与档位无关 |
| `api/mod.rs::def_shape` | 新增分类 `"existing"`（`categories` 是开放值集，已有 `reexport` 先例） | INDEX 格式不变，多一行 `categories.existing = N` |
| spec `desktop-archi-api` | `x-` 面五项 → **六项**；`$defs` 三档表；一条 Scenario | 这是唯一「必须改规格」的点 |

### 11.3 三个必须同时拍板的边界

1. **镜像 `properties` 保不保**：闸 ⑥ 要求每个 `$defs` 键在 `properties` 有指向自己的 `$ref`。
   保 → 32 个类型 = 32 行 `$defs` + 32 行镜像（schema 变名单墙，但「公共面 = 第一层
   `properties` + `$ref` 闭包」这条语义完整）；不保 → 要给 ③ 开镜像豁免（契约的「导出面」
   从此有两个定义，判据与面板都要分叉）。**倾向保**：墙是诚实的成本。
2. **`INDEX.types` 要不要拆列**：`types` 现在是 `$defs` 条目数。③ 计入后 Archi 从 `0` 变
   `~33`——数字含义从「生成了几个类型」漂成「声明了几个类型」。`categories.existing` 能补上
   分辨度；但若架构页要显示「生成 N / 登记 M」，就得加列（`build_index` 与前端各一处）。
3. **③ 能不能当签名位 `$ref` 的目标**：能登记 ≠ 能在签名里用。签名位 `$ref` 仍落
   `Unknown`（方向二），且生成侧裸名在 `#[path] mod` 里不可解析。**档三只做「名字承诺」，
   不要顺手承诺「签名可引用」**——否则会得到「登记成功、见证行 E0412」的半成品。

### 11.4 意外发现：完备性判据其实已经是「类型面」的

`check.rs::undeclared`（`apiUndeclared`，档位恒 `off`）**已经过滤掉 `Function`**：

```rust
.filter(|symbol| symbol.kind != ApiSymbolKind::Function)   // 只比类型面
```

也就是说它天生就是「出线类型 − 已声明类型」的自动分母，只差一个档位。于是有两条互斥的路线：

```
手写登记（档三）                                 自动分母（apiUndeclared: off → warn）
──────────────────────────────────────────      ────────────────────────────────────────
schema 里出现 33 条名字                             零手写，代码面自动枚举
承诺显式、可审、可 diff                             只有观察力：warn、不进 status / passed
apiMissing 反向守（声明了但代码没了就 error 档红）   无回归守卫力（不纳管）
将来可为签名位引用提供锚                            只回答「谁还没声明」
```

两者**不互斥但也不必都上**：档三给承诺力，自动分母给可见度。任一都不解决「142 个 op 里
2 个可表达」这件事——那仍属方向二。

### 11.5 风险（诚实清单）

- **名单墙 vs 空**：③ 让 Archi 的 schema 从 15 行变 70+ 行，其中 33 行是纯名字。
  「文档不枚举导出名、名单唯一持有者是 schema」这条纪律会把墙集中到 schema，但墙还是墙。
  收益是名字改动会被 `apiMissing` 以 `error` 档抓住——现在改名零报警。
- **承诺的形状缺口**：③ 承诺「这个名字存在」，不承诺它长什么样。若有人把 `ArchiGraph`
  改成 `{}` 空结构，契约与判据都无感。这不是档三的缺陷（形状判据本来就不存在），
  但要在 spec 里写明「③ 是名字承诺，不是形状承诺」，否则读者会误当类型契约。
- **两档长得太像**：② 与 ③ 都是「不生成定义」，差别只在目标在**别的元素**还是**本元素**。
  可以在 spec 里合并叙述（「引用档」两个分支），代码里仍是两个分支（`reexport_target`
  能解出目标 ⇔ ②）。

### 11.6 落地粒度（两个 commit 的量）

1. **A：③ 的装载与渲染**（`verify_defs` + `local_defs` + `def_shape` + spec 六项 + 语料）。
   验收：一份 33 条 ③ 的 Archi schema 让 `api:check` 零 diff、`apiMissing` 零命中；
   把 `dto::ArchiGraph` 改名则 `apiMissing` 变红（这是本档唯一的新增判据力）。
2. **B：Archi 的名单回填 + `.md` 措辞修正**（「没有引用已有类型的落点」→「签名位不可引用、
   定义位有三档」）。

A 不做、只做 B 就成了「文档说有、schema 里没有」的空承诺；B 不做、只做 A 则 schema 里
33 个名字没有任何解释，读面只能看到 `categories.existing = 33`。

---

## 12. 改选方向：把「未声明类型面」变成可见（零手写）

目标改为：不动 schema、不写 33 条名字，只让「代码里有、契约里没有」的类型面浮出来。

### 12.1 好消息：该有的零件都在

- `check.rs::undeclared` **已实现**且已过滤 `Function`（纯净的类型面判据），`apiUndeclared`
  也已在 `ALL: [Self; 10]` 里、前端 bindings 里已有该字面量。
- `ApiSymbol` 已带 `file` + `line`（`extract/mod.rs:186`），而 `undeclared()` 的
  `occurrences` 现在恒空 `Vec::new()`——顺手补命中点即可，零新取数。
- 前端**没有**档位表（`severity` 不进 IPC，`ArchiApiFinding` 只序列化 `kind`），
  面板按 category 渲。故档位改动 = 前端零改动、bindings 零重生成。

### 12.2 坏消息：一行改档位会把全仓 status 打死

```rust
// report.rs:253
ArchiApiFindingKind::ApiUndeclared => ApiFindingSeverity::Off,   // ← 想改成 Warn

// drift/mod.rs::derive_status
if !warnings.is_empty() || !gating.is_empty() { return Warnings; }   // ← 墙在这
```

`gating_findings` 的筛选口径是**按元素**（`api = declared` ∧ schema 在盘），不看判据种类。
现网 12 个元素全部 `declared` + schema 在盘 ⇒ 一旦 `apiUndeclared` 产出条目，`gating` 恒非空
⇒ **每次架构检查的 `status` 都是 `warnings`**。这正是文档里为 `docMissing` 写下的那条
反模式：「存量债把状态词常规化为永久 `warnings`，门禁形同废纸」。

### 12.3 顺带挖到的既有实现漏洞

`Archi.md` / `desktop-archi-api-check` 都写着「**文档债永不纳管**」。按代码推演，这条今天
**不是结构保证，是碰巧**：

- `gating_findings` 只看元素态 ⇒ 一个 `declared` 且 `schema.json` 在盘、但 `.md` 被删的
  元素，会产出 `docMissing`（warn），且**落在纳管子集里** ⇒ `status = warnings`。
- 现有用例 `gating_findings不在表内与两条docmissing均落选`（`api/mod_test.rs:5485`）之所以
  绿，是因为它的夹具里那个元素**没有 schema**（`write_pair_schema` 没调）——即「不在纳管
  子集」的原因是缺 schema，不是「docMissing 不纳管」。
- 现网 12 份 `.md` 都在盘，所以该形态尚未出现。删一份 `.md`（保留 schema）就能复现。

⇒ 任何「warn 但永不纳管」的判据，都需要**第二根轴**（判据级的纳管位），而不是继续依赖元素态。
这把 `docMissing` 的实现从「碰巧」修成「结构保证」，也是新判据安全入场的前提。

### 12.4 三个载体

| 载体 | 做法 | 可见度 | status 风险 | 改动面 |
|---|---|---|---|---|
| **A 判据面** | `Off → Warn` + 判据级「永不纳管」轴 + `occurrences` 补 `file/line` | 高：面板两轴 + 抽屉契约段现成渲染，点得到文件行 | 需先建好第二轴（见 12.2/12.3） | `report.rs` 档位 + `gating_findings` 筛选 + 文案 + 语料 |
| **B DTO 数字** | `ArchiApiCheck` 加 `surfaceTypes` / `declaredTypes` 两数，面板契约页渲「覆盖率 0/33」 | 中：要新 UI | 零（不进报告 status） | DTO + `artifact_check` + `bindings:export` + 面板 |
| **C INDEX 列** | `INDEX.toml` 增列 | 低：**今天没有任何读面**（`INDEX.toml` 只进哈希与 diff，无解析器），只有 git diff 和人眼 | 零 | `IndexEntry` + `build_index` |

A 的额外好处：`apiUndeclared` 的档位从 `off` 变 `warn` 后，「十条判据的档位表」变成
`error` 2 / `warn` 8 / `off` 0——`ApiFindingSeverity::Off` 这个变体就只剩「装饰」，
要么删（词表收两值），要么留一项真正的 off 判据。这是必须一起决定的事，否则会留下
一个永不使用的档位。

### 12.5 走 A 的完整改动清单

1. `report.rs`：`ApiUndeclared` → `Warn`；若删 `off` 变体则连带 `ApiFindingSeverity::ALL`
   与 `as_str` 收缩（长度即红灯的测试会指路）。
2. `report.rs`：新增判据级纳管位（如 `ArchiApiFindingKind::gated() -> bool`，
   `docMissing` 与 `apiUndeclared` 为 `false`，其余恒 `true`）；`gating_findings` 的
   filter 加这一条。**这是本方向的真改动**，档位翻转只是它的附赠。
3. `check.rs::undeclared`：`occurrences` 补 `{file, line, specifier: Some(name)}`
   （每类型一条，`ApiSymbol.file/line` 现成）。
4. 文案：33 个名字 join 成一行的现状要改成「前 N 个 + 其余 M 个」或按文件分组，
   否则抽屉里一行读不完。
5. 语料：`drift/report_test.rs` 的档位锚点表（`DocMissing → Clean` 那条旁边加
   `ApiUndeclared → Warnings`）、`mod_test.rs` 的 gating 用例改成「schema 在盘 + 无 doc」
   的真实形态（顺带把 12.3 的漏洞变成回归用例）。

### 12.6 红线

- `status == errors ⟺ passed == false` 不得破（warn 不阻断门禁，`passed` 仍真）。
- `skipped` / `noChanges` 两值仍不得被当通过。
- 不得为了让 `status` 好看而给「未声明面」加白名单/元素级豁免——那会让分母再次消失。

---

## 13. 「剩下的 pub 为什么没进契约」——逐条归类（Archi 实测）

用脚本把 `lib.rs` 的 `pub use` 名单与 `src/**` 的 `pub fn` 对上，crate 根出线函数 **35**
（含 2 个别名），其中能进契约的 **2**：

| 断因 | 个数 | 机制 | 补什么 |
|---|---|---|---|
| 入参/返回含 `&Path` | **22** | 契约侧无任何写法能产 `ApiType::Named`；代码侧 `&Path` → `Named("Path")` ⇒ 短形 `Path` vs `?Path` 恒不等 ⇒ `apiSignatureMismatch`（error 档、进 `passed`） | `$ref → Named` + 路径形态位 |
| 具名类型 / 切片 / 元组 | **9** | 同上（`ApiMeta`、`ApiCheckInput`、`&[ArchiDiagnostic]`、`Result<(String,Reorder),EditError>`） | `$ref → Named` + ③ 登记档 |
| 别名出线 | **2** | `check as api_check` / `enabled as api_enabled`：`UseTree::Rename` 只登记**别名**进 visibility 白名单，而 `ApiSymbol.name` 是原名 ⇒ 契约写别名 ⑤ 红、写原名只是模块内名 | 取数器按别名产符号（需按目标拷贝形状）或改代码去掉别名 |
| 返回 `&str` | 深层 2（`leaf_name` 等） | 返回位位置表硬编码 `String`；见证行 E0308 | 借用标记（两处同改） |
| **能写、只是没写** | 深层 **~10** | 判据能过；只差见证行的 crate 根出线 | 一行 `pub use`（或多段 owner） |

**系统观察**：契约能表达的是「纯字符串 → 标量」窄带；Archi 作为驱动面 crate，每个入口都收
`root: &Path`，所以它的契约面**结构上**塌成 2 个。这不是写作纪律问题。

**分母旁注（本轮不做，记方向）**：更诚实的口径不是 `pub` 清单，而是「**其它元素实际从这里
依赖了什么**」——即关系事实（`apis` / import 事实）反推承诺面，`apiUndeclared` 退化为
「外部依赖是否收入 relations」。跨度大、前置多（依赖边抽取 → 元素归属 → 契约面映射），
本轮不做。

## 14. 本轮口径：先让契约「有办法表达」

三个增量，按依赖序；每个增量都同时改**判据侧**与**生成侧**（只改一侧 = 判据绿/编译红的半成品）。

### 增量 1：`$ref` 可解析 ⇒ `Named`（判据侧同构）

- `extract::schema_type` 的 `$ref` 分支要能判「目标可解析」：本文件 `$defs` 命中，或跨元素
  `ref_target` 命中且目标元素 `api != none`。
- 可解析 → `ApiType::Named(末段名)`；不可解析 → 维持 `Unknown(末段名)`（保留 `?` 留痕语义）。
- 签名变化：`schema_type` / `signature_of` 需要拿到「可解析名字集合」——`check.rs` 的调用点
  手上已有本元素 schema + 全元素 schemas，预计算后传入即可；**判据求值序与档位零改动**。
- 会推翻一条既有裁定（`extract/mod_test.rs:1046`：「MUST NOT 归成 Named」）——那条裁定的理由
  是「留痕」，改为「**不可解析**才留痕」是同一条理由的收紧，不是放松。
- 解锁：9 个具名类型 + 所有 ③ 登记类型的签名位。

### 增量 2：形态位（路径 / 借用）

- 一个位置无关的形态键（候选 `x-form` / `x-host`），取值含 `path` / `borrowed` 一类；
  **第六个 `x-` 键 ⇒ spec 五项表变六项**。
- 判据侧：`schema_type` 认它 → `Named("Path")` / `Str`。
- 生成侧：Rust 位置分派 `path` → `&Path`(Param) / `PathBuf`(Return)；`borrowed` →
  `&str`(Return)。TS 侧统一 `string`。
- 不能走 `common.json`：它的规则**必须含恰 ≥1 个 `{}` 内容位占位符**（信封映射语义），
  `&Path` 无内容位；且其摘要进全部生成物头注释。
- **时机红利**：现网生成物只有 3 份（三个 `lib.i.rs`），任何生成侧改动此刻噪声最小。
- 解锁：22 个 `&Path` + 2 个借用返回。

### 增量 3：③ 登记档（`x-existing`）

- 增量 1/2 需要一个「名字合法、且**不生成定义**」的落点：`$ref` 指向 `$defs.Path`，若默认
  当定义处理就会产 `pub struct Path;`。语义见第 11 节。
- 没有它，增量 1 对「宿主/外部类型」是空转：名字合法了但生成物会造出一个假类型。

### 三者的关系

```
③ 登记档  ──提供──►  名字合法且零生成的落点
     │                     ▲
     │                     │ $ref 指过去
     ▼                     │
增量 1（$ref → Named）──►  判据同形（Path vs Path）
     │
     ▼
增量 2（形态位）──────►  生成同形（&Path / &str / string）
     │
     ▼
35 个出线函数里 ~33 可表达（剩 2 个别名出线另议）
```

### 别名出线（2 个）的两条路

- 取数器：`pub use x as y` 为 `y` 产一条符号，形状按目标符号拷贝（要跨模块查目标 → 抽取器
  多一趟）。
- 代码侧：去掉别名（`api_check` / `api_enabled` 改回原名或改 `pub use` 无别名），零机制改动。
  **代价**：跨 crate 调用方要改名。

---

## 15. 裁定一：源引用（`$ref` 指向源文件）被否

理由（用户裁定，记档）：**「目标③一旦成立，等于接口依赖实现，是更严重的问题」**。契约只许引用
①②（本文件 / 另一份契约文件）；「代码引入类型」而不是「接口引用代码」。

这条裁定同时解释了为什么 `x-existing` 那种布尔更差：它虽然不写路径，但语义等价（契约在指认
手写实现里的某个名字），只是把依赖藏起来了。

⇒ 任何「让契约登记手写类型」的方案都出局。剩下唯一自洽的路：**让契约成为类型的出生地**。

## 16. 于是真问题变成：`$defs` 能不能产出与手写等价的类型？

对手写 `dto.rs` 抽样（3 类形态），逐个对契约词汇：

| 形态 | 形状能写吗 | 生成结果等价吗 |
|---|---|---|
| `ArchiDriftOccurrence { file: String, line: u32, specifier: Option<String>, type_only: bool }` + `#[serde(rename_all="camelCase")]` | ✅（`required` 缺省即 `Option`，`type: ["string","null"]` 亦可） | ❌ 生成的 struct **零 derive**（不可序列化）、无 serde 容器属性（wire 名 `type_only` 而非 `typeOnly`）、**无字段文档** |
| `ElementKind` 四值 + `ALL` + `as_str` + 变体文档 | ✅（`enum` 数组） | ⚠️ 变体能出（`ALL` / `as_str` 手写 impl 可补），但**变体文档无处挂**、derive 缺 |
| `ArchiGraph { Vec<ArchiElement> … }` | ✅（字段位 `$ref` 互指是通的） | ❌ 同上，且 `specta::Type` 缺 → bindings 注册表断 |

**决定性的一行**：`grep description emit/rust.rs emit/ts.rs` → **零命中**。生成器今天不输出任何
文档注释（类型级 / 字段级 / 变体级全无）。而 `dto.rs` 的注释是承重的（「封闭四值」「MUST NOT
回写模型字段」「线词表单点」…）。

⇒ 「契约出生」的三个前置，全在**属性层**，不在形状层：

1. 类型 / 字段 / 变体的 `description` → `///`（变体注释还要求 `enum` 从字符串数组升级为可带注释的形态）
2. `x-derive`（每类型不同的 derive 集合；`Copy` / `Hash` / `Default` 无法靠手写 impl 等价替代）
3. `x-serde`（容器级 `rename_all`；字段级 `rename` / `skip` / `default` 留后续）

再加上判据侧那一处：**`$ref` 可解析 ⇒ `Named`**（否则签名位恒 `?ArchiGraph` vs `ArchiGraph`）。
而一旦类型出生在本文件 `$defs`，这一处就自然指向「同文件互指」，不再有「引用外部实现」的歧义。

## 17. 由此得到的本轮最小切片（提供能力，不迁移）

| 序 | 内容 | 为什么它属于「能力」而不是「迁移」 |
|---|---|---|
| 1 | 生成器输出类型 / 字段 `description` | 没有它，`$defs` 一旦当出生地就丢承重注释 |
| 2 | `x-derive` + `x-serde` 两个属性键 | 没有它，生成的类型不可序列化 / wire 名错 |
| 3 | `$ref`（可解析）⇒ `Named` | 没有它，签名位永远引用不了 `$defs` 类型 |
| 4 | `#[path = "lib.i.rs"] mod api_gen;` 接线 | 现在见证链只是文本；接线后编译即证伪（本轮零类型定义，接线的风险最低） |

4 单独也值钱：**这是唯一能用编译器证明「契约↔实现」链成立的时刻**——现在 `core/archi` 与
`core/foundation` 的 `lib.i.rs` 都还没接线，那两行 `const _: fn(…)` 从未被编译过。

迁移（把 `dto.rs` 33 个类型搬进 `$defs`、手写侧改 `use`）是独立一轮：它必须**逐类型原子**
（一个类型要么在手写、要么在生成，否则 `E0255`），且每搬一个都要过 `cargo clippy` +
`bindings:export` 的 diff 审查。

## 18. 实测：`dto.rs` 33 个类型的属性面是**有界**的

对 `dto.rs` 全量统计（脚本按 `#[derive(...)]` / `#[serde(...)]` 聚合）：

```
derive 集合分布（33 个类型）:
  23  ×  Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type      ← 基础族
   7  ×  基础族 + Copy + Hash
   2  ×  基础族 + Copy + Hash + Default
   1  ×  基础族 + Copy

serde 容器属性（33 个类型全部有）:
  28  ×  rename_all = "camelCase"
   5  ×  rename_all = "lowercase"
字段级:
   2  ×  rename = "in" / rename = "abstract"      ← Rust 关键字位
```

⇒ 于是「属性词汇」不是词汇爆炸，而是**三个语义 facet + 两个 wire 形态值 + 两个字段槽位**：

| 需要表达 | 建议形态 | 覆盖 |
|---|---|---|
| 可复制 / 可哈希 / 有缺省 | `x-facets: ["copy","hash","default"]`（**语义词，不是 Rust trait 名**） | 33/33 |
| wire 命名 | `x-case: "camelCase" \| "lowercase"`（wire 格式词，语言无关） | 33/33 |
| 字段改名（关键字） | 字段级 `x-rename: "in"` | 2 处 |
| 文档 | 类型 / 字段 / 变体 `description` → `///` | 全部 |

**为什么必须写成语义 facet 而不是 `x-derive: ["Copy","Hash"]`**：`Priority` 是 Rust 词，写进契约
就把语言词带进了「索引零语言词」的资产；而 `Copy` 在 TS 侧无对应物。`x-facets` 的映射
（facet → 各语言 trait / 无）落在 `emit`，与「位置 × 语言 → 文本形」同一条纪律。

**结论**：`$defs` 当类型的出生地，技术上已经差得很近——差的是**属性层四处**，不是「引用词汇」。

## 19. 裁定二：属性层用**默认策略**推导，不在契约里声明

用户裁定：`x-facets` / `x-case` / `x-rename` 三个键都不要，改成「按 schema 推导 + 实现自己决定」。
逐条用 `dto.rs` 的实测数据验证：

### Copy / Hash / Default：机械可推，且编译器兜底

```
Copy    10 个类型全部是 unit-only enum；23 个 struct 无一 Copy
        ⇒ 规则：无数据变体（unit enum） ⇒ Copy          （10/10 命中）
Hash    需要全部字段 Hash-able（String/i64/bool/Option/Vec/BTreeMap ✓；f64 ✗；具名递归）
Default GateTier = Off（第 1 变体）、ApiStatus = Declared（第 1 变体）
        ⇒ 规则：enum 的 default = 第一个变体           （2/2 命中）
Serialize / Deserialize / specta::Type  跨 IPC 的定义理由身 ⇒ 恒出
```

**关键性质**：这三个 trait 都是**编译器可判**的——推多了必然 E0204 / E0277，不会静默错；
所以规则可以保守（推不出来就不推），错误面是「编译失败」而不是「契约与实现分叉」。

### 契约名 = wire 名：本地标识符机械派生

- 契约 `properties` 键即 wire 名（`typeOnly`）。
- 生成器派生本地标识符（`typeOnly` → `type_only`），并按需自动选 serde 形态：
  全 camelCase ⇒ 容器级 `rename_all = "camelCase"`；全 lowercase ⇒ `lowercase`；混合 ⇒ 逐字段
  `rename`。**没有声明位，选择是输入的函数**。
- 关键字位（`in` / `abstract`）是唯一需要「标识符归一」的地方，而 `RUST_KEYWORDS` 表已在
  `api/paths.rs`（`is_module_segment` 用的就是它）——归一策略照旧落在 emit。

### 唯一推不出来的两处（需要裁决，不是词汇问题）

```
手写现状                       契约名（=wire）      机械派生的本地名
in_degree          wire "in"        in                 in_ / r#in     ← 与 in_degree 不同
abstract_note      wire "abstract"  abstract           abstract_ / r#abstract
```

这两处是「wire 短名 ↔ 本地语义名」的人工映射（`in_degree` 比 `in_` 可读）。三条路：

1. 手写名改成机械派生名（`in_` / `abstract_`）——零词汇、零例外，代价是可读性。
2. 用 `r#in` / `r#abstract`（与 wire 同名，serde 零 rename）——零词汇，代价是代码里 `d.r#in`。
3. 允许声明 rename（`x-rename` / 字段级 `description` 旁的注记）——回到「要声明」，但与
   用户「契约不改名」的裁定相抵。

### 于是 capability 清单收缩（且**一条 `x-` 键都不用加**）

| # | 内容 | 落点 | 是否新词汇 |
|---|---|---|---|
| 1 | 类型 / 字段 / 变体 `description` → `///` | `emit` 两个渲染器 + enum 形态升级 | 否（`description` 已存在） |
| 2 | 命名策略（契约名 = wire 名 + 本地名派生 + serde 形态自动选） | `emit` | 否 |
| 3 | facet 策略（copy / hash / default 推导） | `emit` | 否 |
| 4 | `$ref`（可解析到本文件 `$defs`）⇒ `Named` | `extract::schema_type` | 否 |
| 5 | `#[path = "lib.i.rs"] mod api_gen;` 接线 | 入口两行 | 否 |

**收益**：spec 的「`x-` 面封闭五项」原封不动。属性全靠**生成器策略**表达，
契约只写「形状 + 语义（description）」。这正是「接口不写实现」的彻底版。

## 20. 追问与两个新发现

### 20.1 facet 推错怎么修（`.i` 不可手改）

修复点**在规则，不在文件**：推导是 `schema → facets` 的纯函数，补一条规则 + 一条语料即可，
不需要动 33 个类型。三条兜底（都不加词汇）：

1. **精确判定，不做启发式**：`Hash` / `Default` 递归到字段类型（`$ref` 跟进）；`Copy` 只在
   unit-only enum 上成立。这样 over-derive 在结构上不可能发生。
2. **推导入参要有跨元素闭包**：字段是具名类型时必须能解析（`ApiSnapshot.schemas` 已装载），
   否则「具名字段」只能猜——猜就是 over/under derive 的唯一来源。
3. **装载期前置**：出现「安全推导不了」的形态（如 `type: number` 配基础族 `Eq`/`Hash`）⇒
   生成期 `ApiError`（fail-closed），不留到编译期。
4. 可读性：`.i.rs` 里的 derive 行本身就是推导结果；头注释注明「facets 推导自什么」。

原则：**先零声明；真出现推不出的合法需求，才加语义 facet（有证据再加）**——与「第二消费者
出现才下沉」同一条纪律。

### 20.2 `$defs` 别名 + `$ref` 推字段名：范畴错位

- `$defs` 承载**类型身份**，字段名不是类型；`$ref` 末段只能给出「字段的类型」，给不出
  「字段的名字」。硬做会撞闸 ⑤/⑥（`properties` 的键是导出名 / op id），且会把 33 个类型的
  每个字段都逼成 `$defs` 条目。
- **关键事实**：那两处不是「语义名 vs wire 名」的选择，而是 **Rust 关键字规避**
  （`in` / `abstract` 都是关键字；`abstract_note` 的 doc 明写「Rust 保留字，故字段名带后缀」，
  `in_degree` 同因）。⇒ 机械解就是 `r#in` / `r#abstract`：serde 把 `r#` 剥掉，wire 名恰是
  `in` / `abstract`，**零 rename、零声明**，语义不减（`r#abstract` 读作 abstract）。

### 20.3 新发现：**整数宽度不可表达**（比 facet 更硬）

`dto.rs` 字段标量分布（脚本统计）：

```
String 21 / Vec<String> 20 / Option<String> 18
u32 8 / usize 2 / u64 2 / Option<u32> 1      ← 13 个带宽度
bool 5 / BTreeMap<String,…> 1 / 具名枚举与结构体若干
```

契约的 `{"type":"integer"}` **恒生成 `i64`**（`value_type` / `scalar_type` 都写死）。于是类型
一旦搬到契约出生：

- 13 个字段的本地精度被**静默**抹平（`usize` → `i64`、`u64` 时间戳 → `i64`）；
- **没有编译兜底**：`i64` 能编译，只有逻辑与大量 `as usize` 强制转换会暴露——这是「静默」
  的语义变化，比 facet 推错危险得多；
- wire 面无变化（TS / JSON 都是 `number`），所以这个问题**只在 Rust 侧存在**。

三条出路：

| 出路 | 说明 | 代价 |
|---|---|---|
| 接受 `i64` | 迁移时改 13 个字段与用法 | 大量 `as usize` / 索引算术噪音 |
| 加宽度位（如 `x-width: "u32"`） | 声明式 | 破「零声明」；且宽度是**本地精度**（语言相关），进契约等于把实现细节写进接口 |
| 契约出生只用于「宽度无关」的新类型 | 老 DTO 保持手写，新类型走契约 | 保留两套事实源；但渐进、零回归风险 |

⇒ 「整数宽度」暴露了「契约出生」的根本张力：**同一个类型既要承载 wire 形状（语言无关），
又要承载本地精度（语言相关）**。这一条必须在迁移前裁决，否则会在 13 个字段上留下静默降级。

## 21. 整数宽度的裁决：用值域关键字推导（用户方案）

提案：`integer` / `number` 的 `minimum` / `maximum` / `exclusiveMinimum` /
`exclusiveMaximum` 用来推断宽度，未定义时给最大范围。

### 21.1 必须收紧的一条规则：只认「完整值域」

```
0 ..= 4294967295          → u32        （恰是 u32 值域）
0 ..= 18446744073709551615 → u64
-2147483648 ..= 2147483647 → i32
无 bounds                  → i64        （最大范围，与现状逐字相同）
0 ..= 100                  → i64        ← 不推 u8！部分范围一律落最大范围
0 ..                     → i64        ← 不推 u64（「非负」不等于「无符号 64 位」）
```

不收紧就会长出**静默耦合**：业务上把 `maximum` 从 `4294967295` 改成 `100`，Rust 字段类型
从 `u32` 翻成 `u8`——编译能过、算术会溢出、没有任何红灯。收紧后「推宽度」只对**全值域**生效，
业务约束怎么改都不影响类型。

### 21.2 兼容性已验证

- `type: ["integer","null"]`：`nullable_breakdown` 的窄化是「克隆整个 schema 再换 `type`」
  ⇒ `minimum` / `maximum` 兄弟键**保留**（`extract/mod.rs:484`）。
- `anyOf: [{integer…}, {null}]`：`non_null` 就是分支对象本身 ⇒ 键自然保留。
- 所以 `Option<u32>` 可表达：`{"type":["integer","null"],"minimum":0,"maximum":4294967295}`。

### 21.3 `usize` 不可表达——而这可能是个好事

`usize` 是平台相关类型，线面上它和 `u64` 无法区分（`0..=usize::MAX` 在 64 位机上恰等于
`u64` 值域）。所以：

- 2 个 `usize` 字段（`out` / `in_degree`）迁移时落 `u64`（或 i64）；
- 契约「拒绝表达 usize」其实是**在帮忙发现设计问题**：平台相关类型不该出现在跨边界类型里。
  这条应写进迁移前置，而不是给词汇开口子。

### 21.4 判据不用看宽度（`ApiType` 不动）

- `$defs` 字段本来**零判据**（只有名字核对），所以宽度差异没有「判据红」这回事。
- op 签名的宽度差异由**见证行**兜：契约 `u32` → 见证 `fn() -> u32`，代码若真返回 `usize`
  ⇒ `E0308`。这是「编译器兜底」而不是「判据兜底」，与 facet 那条纪律一致。
- 因此 `ApiType::Int` 保持无宽度，短形词汇零改动。

### 21.5 代价（诚实记账）

- `minimum` / `maximum` 从此有**两个读者**：标准校验语义 + 生成本地宽度。必须在 spec 的类型
  映射表里写明，否则读者不会知道 `"maximum": 4294967295` 意味着 `u32`。
- 13 个字段各多两行（可读性其实变好：值域即文档）。
- `type: "number"` 仍是 `f64`——值域推不出 `f32` / `f64` 之分（那是精度，不是范围）。

## 22. 决策台账与能力清单（收敛）

### 22.1 已裁定

| # | 议题 | 裁定 |
|---|---|---|
| 1 | 「未声明面」的分母口径 | 应由**其它元素的实际依赖**汇总（关系事实），而非 `pub` 清单；本轮不做 |
| 2 | `$ref` 指向源文件（目标③） | **否决**——接口不得依赖实现 |
| 3 | 类型出生地 | 契约（`$defs`）；代码「引入类型」而不是「接口引用代码」 |
| 4 | 属性层（derive / serde / 改名） | **零声明**：facet 推导、契约名 = wire 名、关键字用 `r#` |
| 5 | 整数宽度 | **严格值域推导**：仅完整值域才推（u32/u64/i32…），其余落 `i64` |
| 6 | 文档注释 | 契约 `description` → `///`（唯一需要「写」的属性） |
| 7 | 判据侧 | `$ref` 可解析 ⇒ `Named`（不可解析仍 `Unknown` 留痕） |
| 8 | 见证链 | `#[path = "lib.i.rs"] mod api_gen;` 接线（现两 crate 未接） |

**收益**：`x-` 面封闭五项**一条不加**；属性全靠生成器策略，契约只写「形状 + 语义」。

### 22.2 能力清单（六项，互不阻塞）

| 项 | 内容 | 落点 | 新词汇 |
|---|---|---|---|
| A | 类型 / 字段 / 变体 `description` → `///` | 两渲染器（enum 形态需升级以承载变体注释） | 否 |
| B | facet 推导（copy / hash / default + 条件 Eq） | `emit`（需跨元素 `$defs` 闭包；装载期 fail-closed） | 否 |
| C | 命名策略（wire 名 → 本地名；关键字 `r#`；serde 形态自动选） | `emit` | 否 |
| D | 宽度推导（严格值域） | `emit::value_type` / `scalar_type` | 否 |
| E | `$ref`（可解析）⇒ `Named` | `extract::schema_type` / `signature_of` | 否 |
| F | 生成物接线 + 编译器验证 | 入口两行 | 否 |

### 22.3 验收判据（机械可查）

- **A**：试点类型的生成注释与契约 `description` 逐字相等。
- **B**：33 个类型全部生成后，`derive` 集合与手写逐项相等（脚本比对），`cargo clippy` 零新告警。
- **C**：33 个类型的字段名映射全对（serde round-trip 语料：wire → 本地 → wire）。
- **D**：13 个整数字段的宽度与手写逐项相等（u32×8 / u64×2 / usize×2→u64 / Option<u32>×1）。
- **E**：契约侧短形从 `?ApiCheckInput` 变 `ApiCheckInput`，对应 op 的 `apiSignatureMismatch` 转绿。
- **F**：`cargo build` 通过即见证链成立（`super::is_identifier` / `super::is_fqn` 两条）。

### 22.4 未决（迁移期要裁决，不是本轮）

1. `usize` → `u64` 的 2 处（`out` / `in_degree`）会引出 `as usize` 用法改写。
2. 4 个 `apply_*` 的元组返回（`Result<(String, Reorder), EditError>`）**无法表达**——
   正确解是拆成命名类型（契约反向塑造 API 的一例），不是给词汇加元组位。
3. 别名出线（`api_check` / `api_enabled`）要么取数器按别名产符号，要么代码去别名。
4. 迁移顺序：先「新类型出生在契约」还是先搬老 DTO（影响 F 的验证时机）。
5. 「名承诺 vs 形状承诺」：`$defs` 字段形状**零判据**，迁移后若要形状守卫，需要新判据（本轮不做）。

### 22.5 一句话结论

**本轮不是「补词汇」，是「把属性从契约里请出去、请进生成器」**：契约收敛到「形状 + 语义」，
属性（derive / 命名 / 宽度）全部由 `schema → 生成物` 的纯函数策略决定；判据侧只补 `$ref`
可解析这一处。于是 Archi 的 33 个类型第一次有了可走的路——**出生在契约，而不是被契约抄写**。

---

## 23. 换轴：契约面到底该承诺到什么粒度

### 23.1 先量一遍「每一轴的真相持有者」

| 轴 | 真相持有者 | 守卫方式 | 覆盖 |
|---|---|---|---|
| **名字** | 契约 + 代码 | `apiMissing`（判据，error 档） | ✅ 强 |
| **签名短形** | 契约 + 代码 | `apiSignatureMismatch`（判据，error 档） | ✅ 强 |
| **形状（字段/变体）** | 现状两份（`dto.rs` + `$defs`）／迁移后一份 | **无判据** | ⚠️ 虚 |
| **语义（为什么）** | 契约（`description`） | 人读 | — |
| **效果（读/写/spawn/取消）** | **手写 grep 测试** | `include_str!(..).contains("std::process")` | ⚠️ 逐文件、易漏 |
| **依赖方向** | `relations` + 扫描事实 | drift 对账 / gate 定级 | ✅ 强 |
| **版本 / 兼容** | 无 | 无 | ❌ 空 |

证据（效果面）：`emit/rust_test.rs:688`、`emit/ts_test.rs:1181`、`emit/mod_test.rs:335` 三处
各自手写 `!source.contains("std::process")`——同一个架构断言被抄成三条**按文件**的断言。
新加一个渲染文件而忘记加断言，就没人守；这与「归属单源」的纪律正相反。

### 23.2 粒度不该按「重要程度」分，按「真相持有者」分

```
一份真相（只有契约）      ⇒ 生成，不需要判据        形状、语义
两份真相（契约 + 代码）    ⇒ 必须有判据，否则是伪承诺  名字、签名
零份真相（无持有者）      ⇒ 不是承诺，是运气         效果（op 级）、版本
```

这条规则顺手解释了两个一直觉得别扭的现象：

- **「形状无判据」在现状下是缺陷**（两份真相，没人对账）；**在「契约出生」之后是定义**
  （只有一份真相，没有第二份可比）。所以形状判据**不需要补**——补的前提是保留两份真相。
- **「效果无资产」不是粒度选得粗，是没有持有者**。元素级边界（零 UI / 零库句柄 / 零子进程）
  今天全在散文里，op 级只靠 `async` 一个位——这是唯一一个「该有持有者但还没有」的轴。

### 23.3 四层粒度各自缺什么

```
仓库级    gate / ratchet                      ✅ 有
关系级    谁依赖谁 + apis（用了哪些名字）        ✅ 有（relations）
元素级    名字全集 / 边界（零 UI / 零库句柄 / 零子进程）  ⚠️ 名字有，边界是散文
op 级     签名 / async / 效果                   ⚠️ 签名有，效果只有 async
类型级    形状 / 语义                           ✅ 迁移后成立
```

⇒ 真正的「粒度缺口」只有两处，都在**效果**这一轴：

1. **元素级边界**（「本 crate 零 spawn / 零写盘 / 零库句柄」）——今天零持有者，只有散文与
   零散 grep 测试。
2. **op 级效果**（哪个 op 读盘、哪个纯、哪个可取消）——只有 `async` 一位；而
   `check`（纯）与 `load_contracts`（读盘）在契约里完全同形。

### 23.4 「效果」为什么可以不违反「零声明」裁定

第 19 节裁的是「**实现策略**不可声明」（derive / 命名 / 宽度——都能从形状推导）。
效果**推不出来**：`schema` 里没有任何信息能说明这个 op 会不会写盘。所以效果属于
**语义事实**，与 `description`、`async` 同族——**声明语义事实是契约的本职**，不是越界。

语言无关化的候选值域（与既有纪律词一一对应）：

```
pure / read / write / spawn / cancel
   ↑            ↑        ↑        ↑
零依赖     零写盘的反面  零子进程   可取消（CancelFlag 入参）
```

判据的取数面：`scan` 已能抽依赖边；效果事实要另抽（`std::process::Command` / `fs::write` /
`tokio::spawn` / `CancelFlag` 参数）。或者**折中**：先只做元素级（把现在三条手写 grep
断言升级成一条判据 + 一份元素级声明），op 级效果留白并在 spec 里写明留白。

### 23.5 一个反直觉结论

「承诺粒度」的瓶颈不是契约能写多少，而是**有多少东西有第二份可以对照**。
于是最该做的不是扩词汇，而是决定每根轴「真相几份」：形状收敛成一份（迁移），效果建一份
（新持有者），名字与签名维持两份（继续用判据守）。**粒度 = 真相份数的函数**。

---

## 24. 效果在两侧代码里长什么样：`async` 是唯一现成样本

### 24.1 `async` 的四处事实（逐行定位）

```
schema 位     x-function.async: true（boolean；内层自由形状，不判未知键）
TS 生成       (params) => Promise<R>                    emit/ts.rs:358  ← 唯一实际效果
Rust 生成     什么都不出（不写 async fn）                 emit/rust.rs 零读取
判据           不进短形（normalize_signature 只有 fn(params) -> ret）
起草           bootstrap 会写 "async": true              api/mod.rs:774
```

推论三条：

1. **`async` 不进判据**：契约写 `async: true`、代码是同步 fn，也不会红。
2. **async op 在 Rust 侧不可声明**：见证行是 `const _: fn(..) -> R`，而 `async fn` 的类型是
   `fn(..) -> impl Future` ⇒ 必然 E0308。所以「效果位存在」与「Rust 见证机制」在 `async` 上互斥。
3. `is_async` 被取数器抽了，但**只有 bootstrap 一个消费者**——抽了事实却没人核对。

### 24.2 效果的渲染面：**没有文本形**

`pure` / `read` / `write` / `spawn` / `cancel` 在两侧都**渲染不出东西**：

- Rust 没有 effect system；IO / spawn 不是类型。
- TS 也没有；`Promise` 这个类型构造已被 `async` 占满。

所以效果声明的产物只可能是两样：**生成物里的注释**（`/// 效果：read`，可选）与**判据的输入**
（比对源码事实）。这与 `x-common.degraded` 同类——**只进债表与判据，不进代码**。

⇒ 效果这一轴**不该走 `emit`**，它的落点是 `check` + 索引/债表。

### 24.3 一个对称的观察

```
借用（&str）   Rust 有文本形、TS 无     → 单侧语言事实
async          TS 有文本形（Promise）、Rust 无  → 单侧语言事实
```

两个「单侧事实」都只能靠「**语义声明 + 语言槽位映射**」表达，不能在契约里写实现词。这也是
为什么 `x-function.async` 是个 boolean 而不是 `fn` 前缀——它声明的是语义，不是文本。

### 24.4 若真做效果，两个粒度的可验证性差别巨大

| 粒度 | 声明形态 | 判据能否验 | 难点 |
|---|---|---|---|
| 元素级 | 「本元素禁止 spawn / write」 | ✅ 整 crate 源码扫描即可 | 无（替代现有三条 grep 断言） |
| op 级 | 「本 op 是 pure / read」 | ⚠️ 只能验**直接效果** | `pure` 需要调用图闭包——`check` 调用的都是纯函数才纯，跨 crate 就断链 |

所以现实路径是：**元素级先做**（一条判据 + 一份声明，覆盖从「三条按文件断言」升到「一份声明
守整个元素」），op 级留白并在 spec 里写明「只承诺直接效果、不承诺传递效果」——否则 `pure`
会变成一个无法验证的承诺。

**（裁定：效果整体不做，留给测试层。以下只讨论 `async`。）**

---

## 25. 为什么 `.i.rs` 定义不了 `async`——三层原因，逐层可修

先纠正一个前提：`.i.rs` **不定义函数**，它只出「类型定义 + 签名见证」
（`const _: fn(..) -> R = super::f;`）。所以问题不是「能不能写 `async fn`」，而是
**「能不能给 async 函数写见证」**。

### 25.1 见证形态结构性失效（真原因）

```rust
// 同步：成立
const _: fn(&str) -> bool = super::is_identifier;

// 异步：不成立
const _: fn(&str) -> bool = super::async_fn;   // ✗ E0308
```

`async fn f(x: &str) -> R` 的类型是「返回**匿名 future** 的 fn」，而匿名 future 无法命名
（`impl Future` 不能出现在 fn 指针 / 关联类型位，TAIT 未稳定）⇒ **现有见证形态对 async 无解**。

可用替代（同一编译期强度，换形态）：

```rust
const _: () = {
    #[allow(dead_code)]
    async fn __witness(x: &str) -> bool { super::f(x).await }
};
```

- 断得住：可调用性、参数类型逐项、输出类型。
- 断不住：它到底是 `async fn` 还是「返回 Future 的同步 fn」——这两者对调用方等价，可接受。
- `async fn` 体内的 `.await` 不需要执行器（只是状态机，不被 poll）。

### 25.2 三层缺位（不止一层）

```
判据       normalize_signature 只有 fn(params) -> ret       ← async 不参与比对
Rust 生成   emit/rust.rs 零读取 async                        ← 不写 async fn、见证也不分叉
TS 生成     (…) => Promise<R>（emit/ts.rs:358）              ← 唯一正确的一层
```

所以「契约写 `async: true`」今天的实际后果是：判据不核、Rust 生成物无视、TS 生成物对得上。
而一旦接线（`#[path] mod api_gen`），真 async op 的见证行会立刻 E0308 ⇒ **async op 目前不可声明**。

### 25.3 补齐只需两处（+ 语料）

| 改动 | 内容 |
|---|---|
| A 判据 | 短形加 `async ` 前缀：`signature_of` 与 `normalize_signature` 同改（`fn(…) -> R` → `async fn(…) -> R`），语料两侧对拍 |
| B Rust 见证分叉 | `op_witness` 按 `signature.async` 分叉：同步走 `const _: fn`，异步走 `const _: () = { async fn __witness(..) -> R { super::f(..).await } };` |

TS 侧不动（已对）。改完 async 才有第一个守卫：**契约写 async、代码同步 ⇒ 判据红**。

### 25.4 顺带记一个语义错位

`async` + `Outcome` 在两侧不同构：

```
Rust   async（隐式 Future）+ Result<T, String>    两个正交轴
TS     Promise<T>（异步）+ reject（错误通道）       一个构造承担两件事
```

`common.json` 的 `Outcome` 已把 TS 侧标 `degraded: typescript`——async + Outcome 是**双重降级**，
这是语言事实，不是词汇缺失。

### 25.5 现状量级

后端 `pub async fn` 共 **19** 个（`orchestration::walker::walk_run`、
`infra::agent::sdk::tools::execute`、`infra::exec::run::run_shell`、若干方法），Archi 自身零 async。
所以 async 缺口今天不影响 Archi，但**任何要声明 orchestration / infra 元素契约的变更都会撞上**。

---

## 26. 能力清单定稿（A–G）

| 项 | 内容 | 落点 | 新 `x-` 键 |
|---|---|---|---|
| **A** | 类型 / 字段 / **变体** `description` → `///` | 两渲染器 | 否（`description` 是标准关键字） |
| **B** | facet 推导（copy / hash / default + 条件 Eq） | `emit`（需跨元素 `$defs` 闭包 + 装载期 fail-closed） | 否 |
| **C** | 命名策略（wire 名 → 本地名；关键字 `r#`；serde 形态自动选） | `emit` | 否 |
| **D** | 整数宽度推导（严格值域） | `emit::value_type` / `scalar_type` | 否 |
| **E** | `$ref`（可解析）⇒ `Named` | `extract::schema_type` / `signature_of` | 否 |
| **F** | 生成物接线（`#[path] mod api_gen`） | 入口两行 | 否 |
| **G** | `async`：短形带前缀（判据）+ 见证形态分叉（编译期） | `extract` + `emit::rust::op_witness` | 否 |

**A 的子决策（唯一未定）**：enum 变体注释怎么承载。

`dto.rs` 实测：**10 个 enum / 36 个变体 / 36 行变体注释**（每个变体恰一行，且承重——封闭词汇
的语义全在这些注释里：`Package` = 包级 / `Domain` = 域级 / …）。而 `enum` 在 schema 里是
**字符串数组**，没有逐值注释位。

| 方案 | 形态 | 代价 |
|---|---|---|
| ① 标准组合 | `oneOf: [{"const":"off","description":"…"}, …]` | 两渲染器都要支持 `oneOf + const`（今天 `oneOf` 走「带负载 enum」，`const` 分支会退化成 `VariantN`）；TS 侧 `union_of` 同理 |
| ② 并行映射 | `x-variant-docs: {"off": "…"}` | 破「零 `x-` 键」 |
| ③ 放弃变体注释 | 只生成类型级 + 字段级 | 36 行承重语义丢失 |

①是唯一既标准又不破 `x-` 面的路，且它同时把「封闭词表」的枚举值从裸字符串升级为
「值 + 语义」对——与 `dto.rs` 现状一致。

---

## 27. ① `oneOf + const + description` 的可行性核实

### 27.1 现网零用例 ⇒ 可以顺势退役 `enum` 数组

```
grep '"enum"' */*.schema.json     → 0 命中
grep '"oneOf"|"anyOf"' *.schema.json → 0 命中
```

12 份 schema 的 `$defs` 全空，`enum` 数组与 `oneOf` **都没有生产用例**（只有测试语料）。
所以可以**一次收敛成唯一形态**：封闭枚举 = `oneOf: [{const, description}]`，`enum` 数组
退役。否则同一语义两种写法，就是第二份真相（`def_shape` 还会把两者都叫 `enum`）。

### 27.2 Rust 侧：完美，但要加一支

今天 `oneOf` 走「带负载 enum」：`value_type({"const":"off"})` → 无 `type` → 兜底
`serde_json::Value` → `variants_block` 出 `Variant0`（垃圾）。需要新增一支：分支含 `const`
且无负载 ⇒ 出 unit variant（名字经既有 `variant_name` 归一）+ `/// description`。

```
"oneOf": [
  {"const": "off",    "description": "不参与门禁（缺省）"},
  {"const": "warn",   "description": "违例仅告警"},
  {"const": "strict", "description": "违例阻断门禁"}
]
                ↓
pub enum GateTier {
    /// 不参与门禁（缺省）
    Off,
    /// 违例仅告警
    Warn,
    /// 违例阻断门禁
    Strict,
}
```

`variant_name` 已能把任意文本无损归一为标识符（含 `V1` 前缀、`Unknown` 兜底），故「值 → 变体名」
这一步零新代码。serde 侧由 C（命名策略）自动出 `#[serde(rename = "off")]` 或 `rename_all`。

### 27.3 TS 侧：变体注释**没有落点**（单侧事实，第三次出现）

```
export type GateTier = 'off' | 'warn' | 'strict';
                       ↑ union 成员不能挂 JSDoc
```

三条路：

| 方案 | 说明 | 代价 |
|---|---|---|
| 汇总进类型级 doc | 类型注释里逐值列举（一段散文） | 逐值注释与 Rust 侧不同源，但不丢语义 |
| 改出值对象 | `export declare const GateTier: { /** 不参与门禁 */ off: 'off', … }` | 生成物形态大变，调用方写法也变 |
| 明确留白 | spec 写明「变体注释只对 Rust 生效」 | TS 侧语义只活在类型级 doc |

现状 TS 元素的 `$defs` 全空（零需求），所以**明确留白**是成本最低且不撒谎的选择——但必须写进
spec，不能靠人猜。

### 27.4 顺带一张「单侧语言事实」台账（第三次出现，已成规律）

```
借用 &str     Rust 有、TS 无        → E 的语义声明 + 语言槽位
async         TS 有（Promise）、Rust 无 → G 的见证形态分叉
变体注释       Rust 有（unit variant）、TS 无 → 明确留白
```

规律：**凡「单侧语言事实」，契约只声明语义，两侧各自映射；一侧没有落点就明确留白，不许
用另一种形态硬凑**。

---

## 28. 为什么不能用 TS 的 `enum`（四个硬理由）

| # | 理由 | 证据 |
|---|---|---|
| 1 | **`.i.ts` 是零运行时声明层**：今天只出 `export type` 与 `export declare const`。TS `enum` 是运行时构造（产出对象）；`declare enum` 虽零产出，但要求手写侧真的存在运行时 enum 值，否则 `GateTier.Off` 在运行时是 `undefined` | `emit/ts.rs` 全文零 `enum` 关键字 |
| 2 | **字符串枚举是名义类型**：`'off'` 不能赋给 `GateTier.Off`（反向可以）。而 wire 形态是字符串字面量（`bindings.ts` 里封闭词汇全是 union），枚举会在每个边界处制造不兼容转换。**spec 已明写「TS 侧不做品牌类型」** | `desktop-archi-api/spec.md:126` |
| 3 | **仓内零 `enum`**：`packages/desktop/src` 里 `^export enum` = 0 命中；封闭词表一律字符串字面量联合 + `as const` 数组迭代 | `grep '^export enum'` 零命中 |
| 4 | **工具链**：`const enum` 在 `isolatedModules` / `verbatimModuleSyntax` 下受限；普通 `enum` 有运行时产物，与「生成物只声明形状」的定位冲突 | — |

### 28.1 但「承载变体注释」的诉求不需要 enum

```ts
/** 门禁档位（封闭三值） */
export type GateTier = 'off' | 'warn' | 'strict';

/** 门禁档位词表：值 → 语义 */
export declare const GateTier: {
  /** 不参与门禁（缺省） */
  readonly off: 'off';
  /** 违例仅告警 */
  readonly warn: 'warn';
  /** 违例阻断门禁 */
  readonly strict: 'strict';
};
```

- `declare const` = **零运行时**（值由手写侧提供），JSDoc 可挂 ✅
- 类型仍是字面量联合 ⇒ 不引入名义类型 ✅
- **形态已有先例**：`x-class` 条目的静态方法就用 `export declare const T: { … };`
  （`emit/ts.rs:286`），类型与值两个命名空间各占一份，互不冲突 ✅

⇒ TS 侧的「值对象」方案 ＝ `export type` + `export declare const` 两段，**不是 enum**。

### 28.2 仍未解决的那一半

`declare const` 只是声明「这个值存在」。手写侧若没提供同名 const（或形状不符），**没有编译期
绑定**——这与 TS 侧一贯的处境一致（`.i.ts` 无见证机制）。所以它和 Rust 的
「impl 由手写提供」是同一个承诺强度：**声明 + 判据，没有编译器**。要写进 spec，别让它读起来
像已绑定。

---

## 29. 定稿：能力清单 A1–G 与两个签名级改动

### 29.1 八项（A 拆成 A1 / A2）

| 项 | 内容 | 落点 | 新 `x-` 键 |
|---|---|---|---|
| **A1** | 类型 / 字段 / 变体 `description` → `///`（Rust） | `emit::rust` | 否 |
| **A2** | 封闭枚举 → `export type X = 'a'\|'b'` + `export declare const X: { /** … */ readonly a: 'a' }` | `emit::ts`（沿用 `x-class` 值声明先例） | 否 |
| **B** | facet 推导（copy / hash / default + 条件 Eq） | `emit` + 装载期 fail-closed | 否 |
| **C** | 命名策略（wire 名 → 本地名；`r#`；serde 形态自动选） | `emit` | 否 |
| **D** | 整数宽度推导（严格值域） | `emit::value_type` / `scalar_type` | 否 |
| **E** | `$ref`（可解析）⇒ `Named` | `extract::schema_type` / `signature_of` | 否 |
| **F** | 生成物接线（`#[path] mod api_gen`） | 入口两行 | 否 |
| **G** | `async`：短形带前缀 + 见证形态分叉 | `extract` + `emit::rust::op_witness` | 否 |

**连带退役**：`enum` 数组（现网零用例）⇒ 封闭枚举唯一形态 = `oneOf: [{const, description}]`。

### 29.2 唯一的签名级改动：跨元素类型闭包

`B`（facet 推导要递归具名字段）与 `E`（`$ref` 要判可解析）共用同一份输入：**本元素 + 所有被引用
元素 + 直接子元素的 `$defs` 全表**。

```
今天：emit::render_target(target, contract, schema, meta)
      extract::signature_of(signature)

需要：多一份 resolver（FQN → $defs 表）；
      数据已在 ApiSnapshot.schemas 里，只是没往下穿
```

`render_all(root, snapshot)` 与 `check(input)` 手上都有 `snapshot` / `input.schemas` ⇒ 这是
「多穿一个入参」，不是新读盘。

### 29.3 落地顺序（按风险与依赖）

```
① E + G（判据侧，纯进程内，零生成物变化：短形改了 → 现有 2 个 op 的判据结果要复核）
② D + A1 + A2 + B + C（生成侧，改的是「未来会生成什么」，现网 3 份生成物要重新生成一次）
③ F（接线）：此刻才让编译器介入；riskiest，但也是唯一能证明链成立的动作
④ 迁移 dto.rs 33 个类型（独立一轮，逐类型原子）
```

理由：①②都不改变「已声明契约的语义」，只改变「尚未声明的能怎么声明」；③之后任何遗漏都会
立刻变编译错误；④是真正的资产搬迁，必须单独评审。

### 29.4 一句话总纲

**契约只承诺「名字 + 签名 + 形状 + 语义」；属性（derive / 命名 / 宽度）是生成器的策略，
效果不是契约的事，单侧语言事实各自映射、无落点就留白。**

---

## 30. 变更提案骨架：`archi-api-type-authoring`

> 范围 = 落地 ①②③（判据侧 + 生成侧 + 接线）。**④（迁移 `dto.rs` 33 个类型）不在本变更内**，
> 由架构 Agent 在本变更之后作为独立迁移执行。

### 30.1 目标

让 `$defs` 有资格当类型的**出生地**：契约写形状 + 语义，生成器按策略产出与手写等价的
类型定义；判据侧补齐「签名里能引用具名类型 / 能核 async」。

### 30.2 范围

**IN**

| 组 | 项 |
|---|---|
| 判据侧 | E（`$ref` 可解析 ⇒ `Named`）、G（短形带 `async`） |
| 生成侧 | A1（Rust doc）、A2（TS 枚举值对象）、B（facet 推导）、C（命名策略）、D（整数宽度） |
| 结构 | `enum` 数组退役（唯一形态 = `oneOf: [{const, description}]`） |
| 接线 | F（`#[path] mod api_gen`，三个已声明元素） |

**OUT**：效果面（不做）；「未声明面」的分母口径（应由关系事实汇总，独立变更）；形状判据
（契约出生后无第二份可比，不做）；元组 / 别名词汇；33 类型迁移（④）。

### 30.3 改动面

| 文件 | 改动 |
|---|---|
| `api/extract/mod.rs` | `schema_type` 的 `$ref` 分支带解析表；`signature_of` / `normalize_signature` 短形加 `async ` |
| `api/emit/rust.rs` | `op_witness` 按 async 分叉；`type_block` 支持 `oneOf+const`（unit variant + doc）；derive/serde/宽度/doc 生成 |
| `api/emit/ts.rs` | `oneOf+const` → 字面量联合 + `export declare const` 值对象（每值 JSDoc）；`rename` 形态自动选 |
| `api/emit/mod.rs` | `render_target` 多一份 resolver 入参；`local_defs` / `def_shape` 随枚举形态 |
| `api/schema.rs` | 停放 `enum` 数组（或标记为不可用）；装载期「facet 推导不了」fail-closed |
| `src-tauri/crates/core/{archi,foundation}/src/lib.rs` | `#[path = "lib.i.rs"] mod api_gen; pub use api_gen::*;` |
| `openspec/specs/desktop-archi-api/spec.md` | 映射表：`enum` → `oneOf+const`；新增属性策略节；`async` 短形；单侧语言事实留白（TS 变体注释走值对象，无编译器绑定） |

### 30.4 前置检查（务必先跑，否则 ③ 直接红）

**见证路径可达性审计**：`super::<owner>::<op>` / `super::<op>` 必须在 crate 根可见。

```
Backend.Core.Archi          2 ops  super::is_identifier / super::is_fqn      ✅（lib.rs 有 pub use）
Backend.Infra.ArchiRuntime  1 op   super::LocalArchiCheck::file_stem         ✅（已接线）
Backend.Core.Foundation     9 ops  8 个 super::<模块>::f                      ✅（pub mod）
                                   1 个 super::CancelFlag::is_set            ❌ CancelFlag 未在 crate 根出线
```

修复：`foundation/src/lib.rs` 增 `pub use cancel::CancelFlag;`（该名已在 `pub mod cancel` 下
可达，无实质扩大）。**这条必须与 F 同批**，否则 `cargo build` 在接线那一刻就红。

### 30.5 验收判据（机械可查）

| 项 | 判据 |
|---|---|
| E | 契约侧短形从 `?ApiCheckInput` 变 `ApiCheckInput`，对应 op 的 `apiSignatureMismatch` 转绿 |
| G | 语料两支：契约 `async` + 同步代码 ⇒ 判据红；契约 `async` + async 代码 ⇒ 见证编译通过 |
| A1 | 试点类型的生成注释与契约 `description` 逐字相等 |
| A2 | TS 值对象的每个成员带 JSDoc；`vp check` 通过；类型仍是字面量联合（零 `enum` 关键字） |
| B | 生成类型的 `derive` 集合与手写逐项相等（脚本比对）；推不出的形态在装载期 `ApiError` |
| C | 33 类型的字段名映射全对（wire → 本地 → wire round-trip 语料） |
| D | 13 个整数字段宽度逐项相等（u32×8 / u64×2 / Option<u32>×1 / usize×2→u64） |
| F | 三个 crate `cargo build` 通过 = 见证链成立 |

**中途必红一次**：②之后现网 3 份 `lib.i.rs` 必须重新生成并提交（`api:check` 零 diff 门会先红）。

### 30.6 风险与红线

1. `status == errors ⟺ passed == false` 不得破（本变更不动判据档位与纳管口径）。
2. `skipped` / `noChanges` 仍不得被当通过。
3. 生成物 MUST NOT 成为手写第二真相：A2 的 `declare const` 只声明值存在，形状由手写 `const` 提供
   —— 要写进 spec，别读成已绑定。
4. facet / 宽度推导 MUST 是 schema 的纯函数（可单测、可语料），修复永远落规则而非文件。

### 30.7 交棒 ④ 的输入（给架构 Agent）

- 逐类型原子：一个类型要么在 `dto.rs`、要么在生成物，否则 `E0255`。
- 2 个 `usize` 字段迁 `u64`（`out` / `in_degree`），连带 `as usize` 用法改写。
- 2 个关键字字段用 `r#in` / `r#abstract`（wire 名不变，serde 零 rename）。
- 4 个 `apply_*` 的 `Result<(String, Reorder), EditError>` **无法表达**——正确解是拆命名类型，
  属 API 形态调整，不是词汇缺口。
- 别名出线（`api_check` / `api_enabled`）：要么取数器按别名产符号，要么代码去别名。

---

## 31. 推荐范围（用「是否阻塞 ④」这一条裁）

判据只有一条：**一项能力进本变更 ⟺ 它是 ④（迁移 33 类型）的阻塞前置。**

| 项 | 阻塞 ④？ | 现网影响 | 建议 |
|---|---|---|---|
| **A1** 类型/字段/变体 doc（Rust） | ✅ 否则 33 个类型的承重注释全丢 | 无 | 进 |
| **B** facet 推导 | ✅ 否则生成类型不可序列化、不进 bindings | 无 | 进 |
| **C** 命名策略 | ✅ 否则 camelCase → 本地位段全错 | 无 | 进 |
| **D** 宽度推导 | ✅ 否则 13 个字段静默降级到 i64 | 无 | 进 |
| **F** 接线 | ✅ 否则迁移完是悬空文本（没人编译） | **有**：Foundation 缺 `pub use cancel::CancelFlag;` 一行 | 进 |
| **`oneOf+const`（Rust）** | ✅ 10 个 enum / 36 个变体的注释与变体名 | 无 | 进 |
| **`oneOf+const`（TS 最小）** | ⚪ 不为 ④ 需要，但**退役 `enum` 数组必须同批**，否则 TS 元素一写就是垃圾输出（陷阱） | 无 | 进（仅 `const` → 字面量，三行） |
| **E** `$ref` ⇒ `Named` | ❌ Archi 已声明的 2 个 op 都不涉及具名类型 | 零（现网无 op 用 `$ref`） | **后置** |
| **G** `async` 短形 + 见证分叉 | ❌ Archi 零 async | 零（现网无 async op） | **后置** |
| **A2** TS 值对象（JSDoc） | ❌ TS 元素 `$defs` 全空，零消费者 | 无 | **后置**（等第一个 TS 枚举） |

### 31.1 于是推荐两个变更 + 一个待触发

| # | 变更 | 内容 | 目的 |
|---|---|---|---|
| 1 | **类型出生能力** | A1 + B + C + D + F + `oneOf+const`（Rust；TS 仅 const 字面量） | 让 ④ 可行 |
| 2 | **判据同构** | E + G | 让签名能引用具名类型、`async` 可核；零现网影响，随时可做 |
| 3 | 待触发 | A2 TS 值对象 | 等第一个 TS 元素真的要声明封闭枚举 |

### 31.2 为什么不把三个合成一个

- E / G **零现网影响**（现网无 op 用 `$ref`、无 async op），合进去只是让变更描述变长，
  却把「判据短形变更」这种需要单独复核语料的事混进一次生成物重生成。
- A2-TS 无消费者，且它带来一个**没有编译器绑定**的承诺（`declare const` 只手写侧提供值），
  现在做等于先写一个无法验证的契约形态。
- 反过来 A1/B/C/D/F/`oneOf+const` 六项**共享同一份解析器管道**（跨元素 `$defs` 闭包 +
  `emit` 内部策略），拆开意味着把同一处 plumbing 改两遍。

### 31.3 变更 1 的验收总闸

```
1. api:check 零 diff（重新生成 3 份 lib.i.rs 后）
2. 三个 crate cargo build 通过（含 Foundation 的 CancelFlag 修复）
3. 语料：把 dto.rs 里 3–5 个代表类型（一个 struct / 一个带 Copy+Hash 的 enum /
   一个带 u32+usize 的 struct）手写成 schema，生成物与手写逐字相同
4. cargo clippy 零新告警
```

第 3 条是**关键闸**：它证明「能力」真的等价于手写，而不只是编译得过。挑 3–5 个而不挑 33 个，
是因为这一步只验证能力面；33 个的等价性归 ④ 迁移逐类型验收。

---

## 32. 变更 1 执行清单（文件级、按序、带回滚点）

```
序  动作                                                   验证                          回滚点
────────────────────────────────────────────────────────────────────────────────────────────────
1   emit 内部 resolver：render_target 多一份「FQN → $defs」表   cargo test -p archi          低（纯入参）
    local_defs / def_shape 随枚举形态调整
2   oneOf+const：Rust → unit variant + doc                   -p archi 语料（新支路）      低
    TS → const → 字面量联合（暂不出值对象）
3   A1：类型 / 字段 / 变体 description → ///                 语料：逐字相等              低
4   B：facet 推导（含装载期「推不出来即 ApiError」）            语料：三个代表类型           中（规则面）
5   C：命名策略（wire → 本地；r#；serde 形态自动选）            语料：round-trip            中
6   D：整数宽度（严格值域）                                    语料：u32/u64/usize/i64     低
7   `pnpm api:export` 重生成 3 份 lib.i.rs 并提交              `pnpm api:check` 零 diff    机械
8   F 接线：archi / foundation 各一行 + Foundation 的           cargo build --workspace      高（首次编译见证）
    `pub use cancel::CancelFlag;`
9   语料闸：3–5 个代表类型手写成 schema → 生成物与手写逐字相同    见下                         —
```

### 32.1 验证命令（`packages/desktop/`）

```bash
pnpm api:export                                  # 重生成 INDEX.toml + 3 份 lib.i.rs
pnpm api:check                                   # 零 diff 门
cargo test -p archi -p foundation -p archi-runtime
cargo fmt --all --manifest-path src-tauri/Cargo.toml
cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml
cargo build --workspace --manifest-path src-tauri/Cargo.toml    # 第 8 步之后才有意义
```

### 32.2 语料闸的三个代表类型（挑法）

| 代表 | 覆盖的推导 | 期望 |
|---|---|---|
| `ArchiDriftOccurrence` | struct + `Option<String>` + camelCase + 字段 doc | 生成物与手写逐字相同 |
| `ElementKind` | unit enum + `Copy`/`Hash` + 变体 doc + lowercase | derive 集合与变体注释逐项相同 |
| `ArchiTopologyAsset` | `usize` 字段 + camelCase 混合（`out` / `in_degree`） | 宽度落 `u64`、`in_degree` 走 `r#in` 或弃用该类型（见 ④ 前置） |

第三个类型顺带暴露 ④ 的第一处硬冲突（`usize` + 关键字字段），把它摆在能力验证里比留到迁移时
才发现要好。

### 32.3 本变更 MUST NOT 做的事

- MUST NOT 修改任何已声明契约的语义（三个元素的 op / 类型数量、判据结果逐字不变）。
- MUST NOT 引入 `x-` 新键（`description` / `oneOf` / `const` 都是标准关键字）。
- MUST NOT 让生成器「猜测」：推不出的 facet / 宽度一律落保守值或 `ApiError`，不猜。
- MUST NOT 顺手把 33 个类型搬进 schema（那是 ④，独立一轮、逐类型原子）。
