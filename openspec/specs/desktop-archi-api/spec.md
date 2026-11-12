# desktop-archi-api Specification

## Purpose

定义模块 API 契约的资产形态与生成链：以 `<archi_dir>/api/<Domain>/<FQN>.md` 为唯一手写真相（散文 + `api-op` / `api-types` / `api-reexport` 三类 fenced json），`<FQN>.schema.json` 与 `INDEX.toml` 为纯派生生成物，加上引用与命名规则、关键字集与类型映射、生成物落点与入口 `#[path]` 纪律、零 diff 门挂载、语言无关纪律、`verify` 闭环与 bootstrap 剪枝、模块嵌套与选择性透传。

## Requirements

### Requirement: 契约资产形态与唯一手写真相

模块公共 API 契约 SHALL 以**手写 markdown 为唯一真相**，落位 `<archi_dir>/api/<Domain>/<FQN>.md`（`<archi_dir>` 经 `foundation::layout::archi_dir`、`api` 段经新增的 `foundation::layout::archi_api_dir` 派生，MUST NOT 行内字面量）。`<Domain>` SHALL 取元素**父元素 FQN 末段**（`App.Backend.Infra.ArchiRuntime` → `Infra`），元素身份仍是 FQN、目录只是布局。

契约文件的章节位置语义 SHALL 封闭为五类：

| 位置 | 承载 |
|---|---|
| H1 标题 | `<FQN>`（与文件名一致，作锚点校验） |
| `## <中文二标题>` | 散文区（人审主体：职能 / 边界 / 不变量 / 承载理由） |
| `### <op-id>` + 紧随一个语言标为 `api-op` 的 json 代码块 | 操作锚点与签名（代码块归属于其前 heading，块内 MUST NOT 自包含 op 名） |
| 语言标为 `api-types` 的 json 代码块 | 本模块自有 `$defs` 定义 |
| 语言标为 `api-reexport` 的 json 代码块 | 透传清单（键 = 导出名，值 = 直接子元素 FQN） |

- **合写一份（不拆两文件）**：`<FQN>.json` + `<FQN>.md` 两份手写文件靠约定配对必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制 8 行后标题已分化为既有反例）。合写的收益不是省文件名，而是把「改契约必须同时改理由」从纪律变成**编辑动作**——改了 op 签名却不改不变量，在同一文件里是显眼的遗漏。
- **载体为 md 内含 fenced json，MUST NOT 用 JSON 内嵌 `description`（散文变整行超长字符串）与 YAML block scalar**：YAML 1.1 把 `off` / `on` / `yes` / `no` 解析为布尔，而 `off` 恰是本仓 `GateTier` 的真实枚举值，各语言解析器版本不一致会破坏跨语言一致性。
- **双向 gate（机械可校验）**：`### <op-id>` heading 集合 SHALL ≡ `api-op` fence 归属集合；`api-types` fence 的键集 SHALL ⊆ `<FQN>.schema.json` 的 `$defs` 键集；`api-reexport` fence 键集 SHALL ≡ 同文件散文区透传列表。任一不成立 SHALL 判为契约错误并点名缺失项。
- **统一目录（不跟代码走）**：契约 MUST NOT 落元素主目录。审计型问题（「这批改动动了哪些模块的契约」）只有统一目录能答（`git diff --stat <archi>/api/` 即列出），且无替代物；伴随型问题（「我改的这行契约同步了吗」）由门禁机械回答，不需要文件相邻。代价 SHALL 显式留痕：在 crate 里读代码时看不到契约，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。
- **不接入 `docs` 三源**：契约 MUST NOT 成为 `desktop-archi-module-docs` 的第四种关联来源（该能力已封死为恰三种）。
- **不依赖语言概念**：资产形态、目录布局、schema 与索引中 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` 一类语言专有概念——语言特定部分只落取数器（`api/extract/{rust,ts}`）与渲染器（`api/emit/{rust,ts}`），见「语言无关纪律与语言特定落点」。

#### Scenario: 五类位置语义与 H1 锚点校验

- **WHEN** 解析一份含 H1 `<FQN>`、`## 职能` 散文、两个 `### <op-id>` + `api-op` fence、一个 `api-types` fence 的契约
- **THEN** 解析成功；两个 op 的 id 与 heading 逐字对应，fence 内不含 op 名；H1 与文件名不一致时报契约错误并指出两处原文

#### Scenario: 双向 gate 三处不成立各报错

- **WHEN** 分别构造「`### new` 有 heading 无 fence」「`api-types` 里有个键在生成的 `$defs` 缺失」「`api-reexport` fence 有 3 个键而散文只列 2 个」
- **THEN** 三次均判契约错误，错误文案分别点名缺 fence 的 op、缺定义的键、fence 与散文的差集；MUST NOT 静默通过

#### Scenario: 拆两文件与 JSON 内嵌描述被拒

- **WHEN** 审查资产形态与提交内容
- **THEN** `<archi_dir>/api/` 下每个元素只有一份 `*.md` 手写真相（`.schema.json` 与 `INDEX.toml` 为生成物且头部带警示）；不存在与 `.md` 配对的第二份手写 `.json`，散文不出现在 JSON 字符串里

### Requirement: schema.json 与 INDEX.toml 为纯派生且不可手改

每个契约 SHALL 派生 `<FQN>.schema.json`（JSON Schema 投影，与 `<FQN>.md` 同目录），并由全体契约派生出 `INDEX.toml`（落 `<archi_api_dir>` 根下、全局唯一一份）。两者都是生成物、都不可手改。

`<FQN>.schema.json` SHALL 的每一项字段都必须有 markdown 出处，生成器只做**装配**不做**翻译**：

| `schema.json` 字段 | 来源 |
|---|---|
| `$schema` / `$id` | 常量 + FQN（`$id` 取文件名 `<FQN>.schema.json`，不合成 URI） |
| `title` | H1 标题 |
| `$defs` 本地定义 | `api-types` fence |
| `$defs` 透传 alias | `api-reexport` fence（值 → 子元素契约的相对文件路径 + `#/$defs/<name>`；同目录用 `./`、跨目录用 `../`，路径由单点函数产出） |
| `properties` 操作项 | `api-op` fence 的键集 + `x-function` |
| `properties` 类型项 | `$defs` 键集 → `{"$ref": "#/$defs/<name>"}` |
| `$comment` | 常量警示文本 |

- **不可手改**：生成物 SHALL 带警示（`$comment` 文本 / 首行注释），文案须说明「由 `<FQN>.md` 生成、手改将在下次 generate 时被覆盖、且零 diff 门会失败」。MUST NOT 为任何内容开口手改——`api-types` / `api-op` fence 的内容本身就是原生 JSON Schema（`minLength` / `pattern` / `enum` / `anyOf` / `x-*` 均可在 md 内表达），表达不了即生成器能力不足，应修生成器。
- **必须提交**：两份生成物 SHALL 入库（MUST NOT `.gitignore`）——零 diff 门需要 git 基线、工具可离线消费、审计型 review 可 `git diff --stat` 一眼看全。
- **review 口径**：每次改契约产生多份 diff；因 schema 零手改自由度，review SHALL 只读 `.md`，schema diff 视为机械噪声——该口径 SHALL 落 `INDEX.toml` 的说明段（不写进散文约定）。
- `INDEX.toml` SHALL 按元素列出：`fqn` / `api` 状态 / 类型数 / 操作数 / 生成物路径 / `degraded` 计数 / `category` 分布，并含 review 口径说明段；条目序 SHALL 为 FQN 字典序（稳定序，便于断言与 diff）。
- **派生前置（消费方约束）**：读 `schema.json` 的工具 SHALL 以 **draft 2020-12 校验器**配合 `strict: "log"` 或 `false`（`x-function` / `x-common` / `x-class` 是未知关键字，strict 模式必然报错），并 SHALL 先按 `$id` 逐个注册被引用 schema（跨文件 `$ref` 是结构性的，`schema.json` 不自包含）。

#### Scenario: 派生完备性逐字段可回溯

- **WHEN** 对一份含 3 个 op、5 个 `$defs`、1 条透传的契约执行生成，逐字段比对 `.md` 与 `.schema.json`
- **THEN** `title` = H1、`$defs` = 5 个本地定义 + 1 个纯 `$ref` 透传条目、`properties` = 3 个 `x-function` 项 + 5 个类型 `$ref` 项、`$id` = 文件名、`$comment` 为警示文本；不存在无 markdown 出处的字段

#### Scenario: 手改提交后零 diff 门持久失败

- **WHEN** 直接编辑已提交的 `<FQN>.schema.json`（不经 generate）后执行 `api:check`
- **THEN** 生成差异非空、检查以非零退出并列出该文件；回退手改后检查恢复通过；MUST NOT 出现「手改被接受」的路径

#### Scenario: 孤儿生成物在任意深度均被检出

- **WHEN** 删除元素或改其根路径后，在 `crates/core/`、`views/`、`a/b/c/d/` 等任意深度留下 `*.i.rs` / `*.i.ts` 残留，随后执行 `api:check`
- **THEN** 三类偏差中的「应删未删」逐条列出这些文件并以非零退出（元素根同级生成物的落点分布在任意深度，MUST NOT 只扫仓库顶层）；`target` / `node_modules` / `dist` / `build` / `.git` 里的同名文件不参与（非生成物的合法落点），普通源文件（`*.ts` / `*.rs` / `*.md`）零误伤

#### Scenario: INDEX 为纯投影

- **WHEN** 修改一份契约的类型数（加 1 个 `$defs` 条目）后重生成
- **THEN** `INDEX.toml` 该元素的类型数与 `degraded` 计数随动，其余元素条目逐字不变；条目序为 FQN 字典序

### Requirement: 引用与命名规则

契约内的类型身份与引用关系 SHALL 遵循五条硬规则：

| # | 规则 | 违反后果 |
|---|---|---|
| 1 | 身份 SHALL 为 `$defs` **键名**，MUST NOT 为 pointer 路径 | 同名两处定义 → 契约错误 |
| 2 | 每个具名类型 SHALL 在顶层 `$defs` 定义恰一次；嵌套 `$defs` 一律提升且**原名保留** | 提升名碰撞 → 契约错误，禁静默改名 |
| 3 | 所有 `$ref` target SHALL 为 `$defs` 顶层条目 | 指向 `properties` / `items` / `allOf/0` → 契约错误 |
| 4 | 多个 `$ref` 指向同一类型时 SHALL 只生成一个定义 | 重复定义 → 契约错误 |
| 5 | MUST NOT 按结构去重（字段类型序列相同不等于同一类型） | 合并即语义错 |

- 规则 2 的命名理由：作者写的是 `B` 就还叫 `B`；加父前缀 `AB` 会让类型名取决于它「恰好被谁嵌套」，换个位置就改名，是静默漂移。嵌套 `$defs` 只是书写分组，MUST NOT 构成作用域。
- 规则 3 的理由：`properties/attr` / `items` / `allOf/0` 这些位置没有稳定名字——名字取决于父级结构，父级改一个属性名就变了。
- 规则 5 的理由：仓库实测 17 组「字段类型序列完全相同」的类型，其中 `ArchiDependsOn` / `ArchiDependedBy` 是**出入边、语义相反**，`Diagnostic` / `ArchiDiagnostic` 是进程内 vs 线面。合并等于把两个语义变成同一个类型；身份靠名字、去重靠名字。
- **公共面** SHALL 定义为「第一层 `properties` + 第一层经 `$ref` 可达的闭包」，该约定自动排除不出现在任何签名里的纯内部类型。
- **位置语义**：第一层 `properties` 为模块导出成员（`x-function` 操作 / `$ref` 到具名类型）；`$defs` 为本模块自有类型与透传纯 `$ref` alias。
- **跨模块 `$ref` SHALL 用相对文件路径**（`./<FQN>.schema.json#/$defs/<name>` / `../<Domain>/<FQN>.schema.json#/$defs/<name>`），路径由**单点函数**产出，MUST NOT 手算、MUST NOT 用自定义 URI scheme（实测 ajv 对 `module://X#/$defs/Y` 报 `can't resolve reference`，只认标准 URI）。

#### Scenario: 嵌套提升原名保留且碰撞报错

- **WHEN** 契约在 `Parent` 的嵌套 `$defs` 内定义 `B`，另一处顶层也定义 `B`
- **THEN** 提升后产生两个顶层 `B` → 契约错误并点名；MUST NOT 自动改名为 `ParentB` 后静默通过

#### Scenario: 键空间撞名即契约错误（不静默覆盖）

- **WHEN** 契约同时把同一名字用作 op id 与 `$defs` 类型名（`### T` + `api-types` 里的 `T`），或 `api-types` 与 `api-reexport` 各自声明同名条目 `T`
- **THEN** 装配期判契约错误并点名该名与冲突的键空间（`properties` / `$defs`）；MUST NOT 由后写者静默覆盖成其中一种形态——撞名意味着契约内部自相矛盾，产物只能有一个赢家，而下游无从知道有冲突

#### Scenario: `$ref` 指向非顶层被拒

- **WHEN** 契约写 `$ref: "#/properties/attr"` 或 `#/properties/x/items`
- **THEN** 契约错误，文案指明 target 不是 `$defs` 顶层条目

#### Scenario: 同结构不同类型不合并

- **WHEN** 契约声明两个 `type: object` 且 `properties` 序列完全相同的类型 `TurnQuestion` 与 `ProjectConfig`
- **THEN** 生成两个独立定义、`$ref` 各自指向自己的键；MUST NOT 合并为一条

#### Scenario: 跨模块引用解析成功

- **WHEN** 以 draft 2020-12 校验器按 `$id` 注册父子两份 `schema.json` 后编译父契约（含 `../<Domain>/<子 FQN>.schema.json#/$defs/<name>` 引用）
- **THEN** 编译通过、无 `can't resolve reference`；两种加载顺序（父先 / 子先）均通过；`strict: "log"` 下不因 `x-*` 关键字报错

### Requirement: 关键字集、类型映射与错误面

契约 SHALL 只用标准 JSON Schema 关键字加**两个**扩展构造：

| 关键字 | 语义 | 生成 |
|---|---|---|
| `object` / `enum` / `const` / `default` / `oneOf` / `$ref` | 标准关键字 | 见映射表 |
| `x-class` | 声明为 trait，**仅允许方法**（出现属性即契约错误） | `pub trait` / `export interface` |
| `x-function` | 函数签名：`params` / `returns` / `async` | `pub fn` / `export type Fn = (…) => R` |
| `x-common` | 自由注记（`degraded` / `unit` / `note` / `since` / `owner` / `tag`），不驱动生成、不加强校验 | 不变换 |

映射 SHALL 为：

| 协议 | Rust | TS |
|---|---|---|
| `object`（具名） | `struct`（每具名 schema 一 struct，建立名义性） | `export interface X { … }` |
| `enum` | `enum`（派生 `Deserialize`） | `export const X = [...] as const` + `export type X = (typeof X)[number]` |
| `oneOf` / `anyOf` | 带负载 `enum`（`#[serde(tag = …)]` 或 `untagged`） | union 类型 |
| `x-class` | `pub trait` | `export interface`（不强制实现 → 记 `degraded`） |
| 标量（`type: integer` 等） | `newtype struct`（`pub struct TimestampMs(pub i64)`） | 别名（**按设计退化，不记 degraded**） |
| 非 `required` 字段 | `Option<T>` | `T \| null` |
| `x-function` | `pub fn`（入参位 `&str` / `&[T]`，返回位 `String` / `Vec<T>`） | `(…) => R` / `Promise<R>`（`async`） |
| 错误面 `Outcome` 信封 | `Result<T, String>`（std 类型，不生成新类型） | `{ ok: T } \| { err: string }`，或 `Promise` reject（不在类型系统内 → 记 `degraded`） |

- **两个扩展构造不可删**：`x-function` 是唯一能区分「模块操作」与「函数指针字段」的标记（JSON Schema 无表达函数的标准方式）；`x-class` 消灭「形状推导误判 trait」的静默故障。反向 SHALL 产**警告**（非错误）：`properties` 全为函数却未标 `x-class` → 提示可能意图为 trait（函数指针结构体是合法意图，不能一刀切拒绝）。
- **标量 MUST NOT 生成为 trait**：`pub trait TimestampMs {}` 的名义性为零（任何类型都能 impl）；`newtype` 才是零开销名义类型。名义化需求真实：实测 `i64`/`u64` 字段 162 处（时间戳簇 62、db id 18+、计数 21+、时长 4），这些 newtype SHALL 落**各模块 `$defs`**，MUST NOT 进 `common.json`。单位须进名称或 `x-common.unit`。
- **位置分派 SHALL 是生成器内部表，MUST NOT 成为对外门禁**：入参位 `string` → `&str`、返回位 → `String`、字段位 → `String`、入参位 `list<T>` → `&[T]`；偏离携带语义（入参位用 `String` 的多为存储型），须在契约登记理由；**但位置词汇与签名短形 MUST 各只有一份**：`Position`（`Param` / `Return` / `Field`）与短形写出点（`str` / `String` / `list<T>` / …）各恰一处，判据侧经 `schema_type` + `signature_of` 取数、MUST NOT 自建第二份位置表或第二份短形拼装（否则可选参数 `optional: true` 与代码 `Option<T>` 的归一必然漏分支、稳定假阳）；`emit` 仍独占「位置 × 语言 → 文本形」的映射。
- **构造器 MUST NOT 进契约**：契约 `### 操作` MUST NOT 声明构造器（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`）——构造器是类型的推论，由生成器从 `$defs` 条目自动产出。违反 SHALL 报 `apiConstructorDeclared`（见 `desktop-archi-api-check`）。
- **枚举列举 SHALL 由函数承担**：需要运行期列举时声明普通函数（如 `all() -> list<LocalEnum>`），**函数体由枚举派生**（语义条件，非标记）；MUST NOT 设计 `x-singleSource` 一类标记，也 MUST NOT 让生成物产出 `pub const ALL`。
- **泛型 MUST NOT 支持**：契约写泛型参数即契约错误。
- **TS 侧 MUST NOT 做品牌类型**：`number & { __brand }` 会让 cast 传染整条调用链，收益只是防误传；TS 侧「实质同类型即可」。
- **`common.json` 的边界**：随工具发布、`include_str!` 编译期内嵌；它 SHALL 只承载**映射规则**（当前仅 `Outcome` 信封），生成时就地展开为各语言标准类型 ⇒ 零生成物、零 vendor 文件、零版本声明。一旦想往里加「带生成产物的类型」（而非映射规则），MUST 回到设计层重裁分发方案，MUST NOT 就地新增。
- **生成物头 SHALL 带工具版本与 `common.json` 摘要**（如 `// 生成于 archi-api 0.4.2 · common.json@sha256:1a2b…`），使「工具升级 → 批量重生成」的变更来源可分辨。
- **契约文本进生成器 SHALL 经两道人字闸，且 `x-function` 的落点 SHALL 封闭二处**：① 装载期（`contract::verify`）**七族名字**——op id / `$defs` 键 / 透传键 / `params[].name` / `$defs` 条目的 `properties` 键 / `api-op` 的 `owner` / `$ref` 末段名——MUST 是 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`，不成立即契约错误并点名族别与 fence 行；**同趟遍历另判 `x-function` 的落点**——MUST 只在 `api-op` 顶层签名与 `$defs` 条目第一层 `properties` 的值二处，其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及其更深层）即契约错误并点名 JSON pointer（位置类错误的正确处置是**在契约里搬位置**，不是在渲染期兜底）；② 渲染期一切来自契约的**字符串字面量与注释** MUST 经该语言单点转义（单引号形态不变，MUST NOT 用会产出双引号的方式生成），Rust 侧枚举值经无损驼峰归一。MUST NOT 在渲染期重复校验名字或位置（装载期已保证，`emit/{rust,ts}.rs` 内零标识符判据、两侧 `value_type` 零 `x-function` 分支是它的机械判据）。
- **四条明确不做**：`x-interface` / `x-nominal` / `x-singleSource` 独立构造（分别由 `x-class`、标量形状、函数返回消解）；自造协议记法（`[op.xxx]` 信封 / `module://` scheme）；L/S 双层锁 + 归一函数 + 偏离登记表（`verify ≡ generate + 类型检查`）；Zod 作通用载体。

#### Scenario: 函数指针字段与模块操作可辨

- **WHEN** 契约同时含 `x-function` 操作 `parse` 与一个 `type: object` 且 `properties.parse` 为 `x-function` 的结构体字段
- **THEN** 前者生成为模块级 `pub fn`，后者生成为 `pub parse: fn(&str) -> …` 字段；两者不互相吞并

#### Scenario: `x-class` 带属性即契约错误

- **WHEN** 某 `$defs` 条目标 `x-class` 但同时声明 `properties: { "count": { "type": "integer" } }`
- **THEN** 契约错误（trait 仅允许方法）；去掉 `x-class` 改为普通 `object` 后通过，生成 `struct`

#### Scenario: 标量生成 newtype 而非 trait

- **WHEN** 契约声明 `TimestampMs: { "type": "integer", "x-common": { "unit": "ms" } }`
- **THEN** Rust 侧生成 `pub struct TimestampMs(pub i64)`（非 `trait`）；TS 侧生成 `export type TimestampMs = number` 且不记 `degraded`

#### Scenario: 契约声明构造器被拒

- **WHEN** 契约含 `### new` + `api-op` fence
- **THEN** 报 `apiConstructorDeclared` 并点名该 op；删掉该段后生成物仍自动产出 `new()`（构造器双向都不参与判据比对：既不在契约声明集，也不计入代码导出集）

#### Scenario: 错误面显式留痕

- **WHEN** 契约声明返回 `$ref: "#/$defs/Outcome"`，TS 侧实现以 `Promise` reject 表达错误
- **THEN** Rust 侧生成 `Result<T, String>`；TS 侧生成 union 且该类型记 `x-common.degraded`（错误通道不在类型系统内，必须显式留痕）；`INDEX.toml` 的 `degraded` 计数 +1

### Requirement: 生成物落点与入口查找

生成物 SHALL 落元素根路径**同级**（`<root>.i.rs` / `<root>.i.ts`），其中 `<root>` 为元素根路径**无后缀形态**（无 `./` 前缀、无尾 `/`、无文件扩展名），`<name>` 为 `<root>` 的**最后一段**（`crates/core/archi/src/scan` → `scan`、`crates/infra/archi` → `archi`、`views/welcome-view` → `welcome-view`）。

- **`<name>` 取路径末段而非 FQN 末段**：两者多数情况相同但不恒等（`App.Backend.Infra.ArchiRuntime` 的 FQN 末段是 `ArchiRuntime`、路径末段是 `archi`），而文件名必须与 `#[path]` / TS import 的实际路径对齐——故判据只有路径末段一条，MUST NOT 引入 FQN 派生的第二套命名。
- **`<root>` 无后缀是硬约束**：`<root>` 是**模块身份**而非文件路径，同时决定生成物落点、实现入口位置与 `#[path]` 写法。归一 SHALL 在模型解析层完成（`OwnerIndex::normalize` 已处理 `./` 前缀与尾 `/`），MUST NOT 批量改写存量 models——实测存量 41 条 `path` 全部带 `./` 前缀 + 尾 `/`、一律需要归一（本变更新增的第 42 条同形）。
- **不强制模块成目录**：两条路对比后取「按文件（根同级）」——生成物路径只有一条规则、无形态分叉，前端 12 个单文件元素零改动；代价（留痕）是目录形态的引用路径带 `../`，生成物与模块文件在 git 视角不聚拢。
- **入口查找顺序封闭**（MUST NOT 增分支）：
  ```
  Rust    <dir>/src/lib.rs   →  <dir>/mod.rs   →  <dir>.rs
  前端    <dir>/index.ts     →  <dir>/index.tsx →  <dir>.ts →  <dir>.tsx
  ```
  `<dir>/lib.rs` MUST NOT 在列表内（crate 的 `lib.rs` 恒在 `src/` 下）。三分支 / 四分支各有真实存量支撑或如实零命中，MUST NOT 因「未来可能需要」加分支。
- **入口编排 SHALL 固定为**：入口文件顶部 `#[path = "<相对路径>"] mod api_gen; pub use api_gen::*;`，既有 `pub mod` 声明原样不动。生成类型经 `pub use` 对外可引用；与既有同名类型冲突时编译器直接报 E0255，**不会静默覆盖**。
- `#[path]` 的值 SHALL 为**字面量**，且 MUST 与 `paths::mod_path_attr` 的输出逐字相等（该函数是等值判据的唯一真值源，由 `paths_test` 三形态断言与逐入口自查行双守）；MUST NOT 手写与函数输出不等值的形态。目标是恒指向 `<root>.i.rs`——即生成物在元素根路径同级、而非入口文件同级：

  | 入口形态 | `#[path]` 基准 | `#[path]` 值 |
  |---|---|---|
  | `<dir>/src/lib.rs`（crate 形态） | `<dir>/src/` | `"../../<name>.i.rs"` |
  | `<dir>/mod.rs`（子目录模块形态） | `<dir>/` | `"../<name>.i.rs"` |
  | `<dir>.rs`（单文件形态） | `<dir>` 的父目录 | `"<name>.i.rs"` |

  同一单点函数 SHALL 产出 TS 侧的 `import type` 路径（目录形态 `"../<name>.i"`、单文件形态 `"./<name>.i"`）。三形态的解析结果 SHALL 逐字等于 `<root>.i.rs` / `<root>.i.ts`；只凭「入口文件同级」写字面量会落到 `<dir>/<name>.i.rs`（错位），故 MUST NOT 手写。
- **MUST NOT 生成 `_ext.rs`**：生成物只含类型定义（`struct` / `trait` / `enum` / newtype）与自动构造器；`impl` 块写在模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）。生成 / 手写分离靠**文件命名**（`.i.rs` / `.i.ts` 后缀）而非目录分离。
- **归属规则 SHALL 为「双形态并集」**：`.i.<ext>` 生成物（`ext ∈ {rs, ts, tsx, js, jsx}`，标记段恰为 `i` 且模块名非空）的归属 SHALL 由**两个剥离去向取并集**判定——

  ```
  候选 1  整组剥离      <name>.i.<ext>  →  <name>        archi.i.rs → archi
  候选 2  替换扩展名    <stem>.i.<ext>  →  <stem>.<ext>  archi.i.rs → archi.rs
  命中判据 = 任一候选满足 rel == prefix 或 rel.startsWith(prefix + "/")
  ```

  理由（两种落点形态互不相交，单条规则覆盖不了）：生成器 `paths::generated_path` 产出的是**元素根同级**（`crates/infra/archi.i.rs`，需候选 1）；历史 / 假设形态是**元素目录内**（`<dir>/<name>.i.<ext>`，需候选 2）。判据改造 SHALL 只此一处（`OwnerIndex` 的剥离函数与其调用点），`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动，既有非 `.i` 路径的归属语义零变化。
- **并集 MUST NOT 实现为候选短路**：命中集 SHALL 为「对每个候选各跑一遍最长前缀后合并去重」，再按前缀长度定档（同长仍归 `Ambiguous`）；MUST NOT 写成「候选 1 命中即返回」——那会让 `src/dir.i.ts`（候选 1 全等 `src/dir`、候选 2 得 `src/dir.ts`）只走一条路径，与并集语义不符。定档用例：元素根 `src/dir/` 与 `src/dir.ts` 同时在场、探针 `src/dir.i.ts` ⇒ 归更具体的单文件根（前缀更长者胜出）。
- **白名单外后缀零误剥**：`.i.py` / `.d.rs` / 大写 `.I.rs` / 无扩展名 `.i` SHALL 原样返回单个候选（不剥），归属与改动前逐值相同。
- **父子路径重叠不缩窄父契约**：子元素根路径是父元素根路径的前缀，最长前缀匹配已正确处理（`.../src/scan/x.rs` 归 `Scan`、`.../src/dto.rs` 归 `Archi`）。
- **新判据 `apiEntryMissing`**：元素根路径下既无生成物（`.i`）也无实现入口 → 该元素不是真模块或已删除。它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约（定义与档位见 `desktop-archi-api-check`）。
- **`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律**：取数器定位 `#[path = "…"] mod` 时 MUST 满足三条——① **反斜杠归一在折叠之前**（否则反斜杠形态被当作单段名，`Path::join` 的纯拼接会把 `..` 原样带到 Windows 的路径解析里、逐级上溯到观察根之外）；② 分段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）；③ **`join` 后复核**目标仍在观察根之下（`starts_with(root)`）。任一条不成立即**不命中该子树**（与「模块定位失败即跳过」同口径，MUST NOT 新增错误变体、MUST NOT panic）。理由是 `#[path]` 的值经**手写入口行**落地（属性的值只能是字面量，等值性由机械断言守），属可被写错的入参；放行即让「模块树被带出观察根」，与 `paths` 侧的「任意文件读」是同一件事。判据与 `scan` 的 `normalize_rel` / `docs` 的 `resolved_within` 同一条纪律。

#### Scenario: 两种落点形态各归其元素

- **WHEN** 分两组求归属：① 根同级形态（`crates/infra/archi.i.rs` 对目录形前缀 `crates/infra/archi`、`views/welcome-view.tsx.i.ts` 对单文件形前缀 `views/welcome-view.tsx`）；② 目录内形态（`views/architecture/architecture.i.ts`、`crates/core/archi/src/scan/scan.i.rs` 对各自目录形前缀）
- **THEN** 四组用例各归其元素（①由整组剥离命中、②由替换扩展名命中，两者并集）；非 `.i` 路径（如 `crates/core/archi/src/dto.rs`）归属结果与改造前逐字相同

#### Scenario: 并集定档不短路

- **WHEN** 元素根 `src/dir/` 与 `src/dir.ts` 同时在场，探针为 `src/dir.i.ts`
- **THEN** 归属为更具体的单文件根（`src/dir.ts` 所在元素，前缀更长）；若实现成「候选 1 命中即短路」则归目录形根——该差异即本 Scenario 的判红面

#### Scenario: 白名单外后缀不误剥

- **WHEN** 对 `src/mod/mod.i.py`、`src/mod/mod.d.rs`、`src/mod/mod.I.rs`、`src/mod/mod.i` 求归属（元素根 `src/mod/`）
- **THEN** 四者按原路径判定归属（不剥），结果与改动前逐值相同；`.i.<白名单外后缀>` 不产生第二个候选

#### Scenario: 入口三分支与四分支实测命中

- **WHEN** 对全部非容器元素按入口顺序查找实现入口
- **THEN** `<dir>/src/lib.rs` 命中 **14** 个 crate 元素（`core` 7 + `infra` 7，实测复核）、`<dir>/mod.rs` 与 `<dir>.rs` 当前零命中、前端四分支当前零命中；`crates/tools/export-bindings` 与 `crates/tools/archi-api-gen`（均只有 `main.rs`）以及壳元素 `App.Backend.Shell`（根路径 `src-tauri/src` 拼出的三条候选均不存在）记为 `apiEntryMissing`（**不消解**）；`crates/tools/specta-zod-patched` 无元素，不参与入口查找

#### Scenario: 三种入口形态的 #[path] 各自解析到根同级生成物

- **WHEN** 分别对 crate 形态（`crates/infra/archi/src/lib.rs`）、子目录模块形态（`crates/core/archi/src/scan/mod.rs`）、单文件形态（`views/architecture.ts`）取 `paths::mod_path_attr`（`#[path]` 等值判据的真值源；入口行里的值本身是字面量）与 `paths::ts_import_specifier`（import 路径单点产出）的输出并规范化
- **THEN** 三者分别解析为 `crates/infra/archi.i.rs`、`crates/core/archi/src/scan.i.rs`、`views/architecture.i.rs`（等于各自的 `<root>.i.rs`）；把任一形态的值改写成「入口同级字面量」后解析结果错位（`<dir>/<name>.i.rs`）且编译 / 导入失败

#### Scenario: 生成物与手写 impl 共存编译

- **WHEN** 入口文件写与 `paths::mod_path_attr(EntryForm::Crate, "archi")` 输出逐字相等的 `#[path]` **字面量**（crate 形态 `"../../archi.i.rs"`）+ `pub use api_gen::LocalArchiCheck;`，生成物 `<root>.i.rs` 含 `pub struct LocalArchiCheck;` + `impl` 内 `new()`，手写文件另有 `impl Default for LocalArchiCheck`
- **THEN** 编译通过（`rustc --edition 2021`），外部可经入口路径引用 `LocalArchiCheck::new()` 且签名受生成物约束

#### Scenario: 同名冲突不静默覆盖

- **WHEN** 生成物定义了元素内已手写的同名类型且入口 `pub use api_gen::*;`
- **THEN** 编译器报 E0255（重复定义），MUST NOT 静默采用其一

#### Scenario: 路径形态非法即拒收（安全边界）

- **WHEN** 元素 `<root>` 或契约 FQN 取绝对路径 / 含盘符 / 含 `..` 段 / 含空段 / 为空串（如 `path = ["/abs/elsewhere"]`、`path = ["crates/../../outside"]`、`fqn = "App..X"`）
- **THEN** `contract_path` / `schema_path` / `generated_path` / `entry_form` 一律返回空（`None`），不 panic、不落到仓外；`root.join(绝对路径)` 的「替换 base」与 `..` 的「上溯」因此结构上不可达——「证据在盘」的探测不会变成任意文件读，工具 crate 的写盘不会变成任意文件写。**第二层**：`archi-api-gen` 的写盘函数 SHALL 另以目标须落在观察根之下（`starts_with(root)`）复核，越界即拒绝并零副作用（两层判据交叉验证，任一层失效另一层仍拦得住）

### Requirement: 生成物入仓与零 diff 门挂载

四类生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `<FQN>.schema.json` | `<archi_api_dir>/<Domain>/` | 是 | `api:check` 零 diff |
| `INDEX.toml` | `<archi_api_dir>/` 根（全局唯一一份） | 是 | `api:check` 零 diff |
| `<root>.i.rs` | 元素根同级 | 是 | `api:check` 零 diff + `cargo check` |
| `<root>.i.ts` | 元素根同级 | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **脚本面封闭两条**：`api:export`（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`，写盘）与 `api:check`（同一调用加 `--check`；生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，**不依赖任何 `git diff` 段**）。`check` 链 SHALL 由三段扩为四段：`server:check && bindings:export && api:check && client:check` —— 该扩展只加一段（`api:check` 插在第二段之后）；`bindings:export` 与 `client:check` 两段命令形态零变化，`server:check` 的 fmt 段 SHALL 由写模式 `cargo fmt --all` 改为校验模式 `cargo fmt --all --check`（写模式门使工作树无法停在不动点，故本变更 MUST 改它；clippy 段逐字不动）。**第二段 SHALL 是 `bindings:export`**（`check` 的生成物零 diff 守卫已退役，见 `desktop-ipc-type-bindings`——`check` 只前置重导出、不比对）；规范内 MUST NOT 引用任何已退役的守卫脚本名；段名 SHALL 以 `package.json` 的 `scripts` 实存键为准（`grep -c '"bindings:export"' package.json` ≥ 1 即证该键在案）。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 增 `src/**/*.i.ts`（生成物非手写代码；实测未被 import 的 `.i.ts` 会被报为 unused file，因 `project` glob 覆盖 `src/**/*.ts`）；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies` 条目（那是关判据而非满足判据）。
- **类型绑定再生成**：`ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` 与报告字段 `apiFindings` SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役）；本能力 MUST NOT 手写前端类型镜像。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型（否则校验不发生）；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（相对递增，规范内 MUST NOT 写死具体版本号——现值随每次源码改动滚动，写死即失效）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 四类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 四类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改任一生成物即被拦

- **WHEN** 手改已提交的 `<FQN>.schema.json`、`INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: check 链扩一段且 fmt 段改校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与既有 `server:check` / `bindings:export` / `client:check` 三段
- **THEN** `check` 为四段，逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`（新增 `api:check` 位于 `bindings:export` 与 `client:check` 之间）；`bindings:export` 与 `client:check` 两段命令原文逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check` 且其 clippy 段逐字不变；`client:check`（含 knip）在 `src/**/*.i.ts` 进 ignore 后仍零 dead code 报告

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含五条要点：① 手写真相位置 `<archi_api_dir>/<Domain>/<FQN>.md` 与三类 fence 的名字（`api-op` / `api-types` / `api-reexport`）；② 四类生成物勿手改（`.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」；⑤ `apiFindings` 首期不进 `status` / `passed`，索引与计数经 `INDEX.toml` 查看。
- 该节 MUST NOT 含契约示例正文、`ArchiApiFindingKind` 的十值长表或判据定义（定义归本能力与 `desktop-archi-api-check`）——文档与 spec 双写必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制即既有反例）。
- 该节 MUST NOT 出现 `openspec/specs/**` 路径（spec 语料非 agent 的契约操作面），且 MUST NOT 把契约路径写进模型元素 `docs`（见 `desktop-archi-module-docs`）。
- `CLAUDE.md` SHALL 同步同一节且与 `AGENT.md` 保持既有同源纪律：两文件该节正文逐字相同，全文件差异 SHALL 限于节外既有差异（标题行、首行说明行、文件末尾换行）；**该节自身的两份副本若分化即违反本条**。

#### Scenario: 纪律节只指路不带正文

- **WHEN** 审查 `AGENT.md` 的 `## 模块 API 契约` 一节
- **THEN** 该节恰有 5 条要点、`grep -c '^## 模块 API 契约' = 1`；`grep 'apiEntryMissing'` 在该节零命中（判据长表未复制）；节内零 `openspec/specs/` 路径；节内无 fenced json 代码块

#### Scenario: 两份副本不分化

- **WHEN** 对 `AGENT.md` 与 `CLAUDE.md` 做 `diff`
- **THEN** 差异恰 2 处且全在 `## 模块 API 契约` 节外（`:1` 标题、`:3` 首行说明）；该节在两文件各恰一处且节内正文零 diff 行（逐字相同）

### Requirement: 语言无关纪律与语言特定落点

契约资产层 SHALL 与语言无关，语言特定能力 SHALL 只落取数器与渲染器：

```
语言无关    models · api 契约 · docs · graph · 漂移与契约判据 · FQN · gate
────────────────────────────────────────────────────────────────
语言特定    api/extract/{rust,ts}.rs（取数器，仅 bootstrap）· api/emit/{rust,ts}.rs（生成物投影）
```

- `crate` 是 Rust 工具链概念，架构层 MUST NOT 依赖：`dto::ElementKind` 无语言字段的原则已被遵守，契约格式 MUST NOT 重复 models 注释里的历史错误（`02-backend-core.toml:2` 写「Cargo `[dependencies]`」、`06-frontend.toml:3` 写「import 证据」）。
- `<FQN>.md` / `<FQN>.schema.json` / `INDEX.toml` 内 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 一类语言专有词（`x-common.note` 里说明具体语言的实现细节除外，且须显式标注为注记）。
- **生成物按模块落盘，MUST NOT 按 crate 聚合**：按 crate 聚合会让架构层依赖语言概念，且 diff 粒度串味。
- **跨语言语义标注**：语言能力缺失 SHALL 经 `x-common.degraded` 显式留痕（当前两类：错误通道走 `Promise` reject 不在类型系统内；TS 结构化类型无法强制实现）。降级是机制属性，MUST NOT 靠散文掩盖。

#### Scenario: 资产面零语言词

- **WHEN** 审查三份手写 / 生成资产（`.md` / `.schema.json` / `INDEX.toml`）的全部文本
- **THEN** 零 `crate` / `Cargo.toml` / `Rust` / `TypeScript` 词（注记内标注为注记的语言说明除外）；语言词只出现在 `api/extract/**` 与 `api/emit/**` 的源码注释里

#### Scenario: 两类降级可枚举

- **WHEN** 取 `INDEX.toml` 的 `degraded` 计数与各契约的 `x-common.degraded` 条目
- **THEN** 降级原因只来自两类（`Promise` reject 错误通道 / TS 不可强制实现）；新增第三类原因须改本 requirement，MUST NOT 自由发挥

### Requirement: 生成器、verify 闭环与 bootstrap 剪枝

`verify` SHALL 定义为机械判定而非独立模式：**`generate`（`--check` 零写入，比对在工具内完成，等价于 git 零 diff）+ 语言编译器**。MUST NOT 引入归一函数、双层锁或语义 diff 比对器——契约与代码是否一致由「生成物是否最新（零 diff）」与「实现是否匹配生成物（编译器）」两步回答。

- **判定在 core，写盘在工具 crate**：契约解析、装配、判据求值与渲染 SHALL 全部落 `crates/core/archi/src/api/**`（纯读、零写盘、零 spawn，沿该 crate 既有纪律）；唯一写盘面 SHALL 为 `crates/tools/archi-api-gen`（CLI 工具，与既有 `export-bindings` 同族），允许写盘是该工具边界的正当属性、MUST NOT 反向稀释 `archi` 的零写盘纪律。
- **CLI 三子命令封闭**：`generate`（写四类生成物：`<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）、`--check`（生成到内存后与磁盘比对，**零写入**，差异即非零退出并列文件名）、`bootstrap`（输出候选草稿到 stdout，不写盘）。`generate` SHALL 幂等（同输入同输出，重跑零 diff）。
- **`verify` 覆盖不到的三件事 SHALL 显式列出**：bootstrap 方向（代码 → 契约，属一次性迁移工具，非常态门禁）、跨模块语义不变量（散文 + 专项测试）、生成器自身正确性（生成器自己的单测）。MUST NOT 让人误以为零 diff 门能验这些。
- **bootstrap SHALL 走 AST，MUST NOT 正则抽签名**：Rust 侧实测多行签名占 `pub fn` 的 30–62%（`core/checks` 13/21、`core/archi` 16/27），`grep` 单行抽取会截半签名。TS 侧只需判定 `export function`——实测 `export const` 里零个箭头函数（0/132），故 MUST NOT 为「从初始化表达式推断函数」造启发式。
- **`visibility` SHALL 为封闭四值**（bootstrap 剪枝的机械判据）：`external`（跨元素可达，真承诺保留）/ `crateOnly`（仅同 crate 或同元素目录内可达，内部契约保留但降格）/ `unused`（全仓无引用者，**伪公共首选删除对象**）/ `testOnly`（仅测试树内引用，不进 API）。**词表封闭 ≠ 两语言都判得出四值，更 ≠ 本期生产面都产得出**：本期可控值域 SHALL 按语言分列——Rust 侧四值可判（`pub mod` 链或 `pub use` 白名单 / `pub(crate)` / 私有 `mod` 内的 `pub` / `#[cfg(test)]` 树）；TS 侧**本期生产面只产 `external`**（有 `export`），`unused` 的降级接口 `apply_knip_report` 已实现但**产品路径零调用者**、`archi-api-gen` 与 `api::bootstrap` 均不读 `knip` ⇒ `Visibility::Unused` 在**本期生产面不可达**，这是**显式留白**而非疏漏（MUST NOT 用「已实现」掩盖；接线是独立变更）；`crateOnly` / `testOnly` **本期留白**（判据需跨文件 import 反向事实，取数器只吃单文件 AST）。留白项 MUST NOT 以「文件是否在元素目录内」或「文件名含 `.test.`」充当 proxy（`.spec.*` / `__tests__/**` / `fixtures/**` 均不作 proxy）。
- **MUST NOT 为 TS 造第二套死代码扫描**：本期 SHALL NOT 读 `knip` 报告（`apply_knip_report` 产品路径零调用者，`archi-api-gen` 与 `api::bootstrap` 对 `knip` 零命中）⇒ `unused` 降级在本期生产面不可达，属显式留白；接线时 `unused` 判据 SHALL 取自 `knip` 报告（`client:check` 已含），且 `knip.json` 的 `ignore` 面与 archi 模型排除面高度一致，直接沿用其判定，MUST NOT 另造一套死代码扫描。
- **试点先行**：bootstrap 的首次落地 SHALL 只做**一个**元素（`App.Backend.Infra.ArchiRuntime`，14 个 crate 元素里公开面最小的 2 项），由人工或 agent 删除伪公共面形成初版契约；其余元素的契约 SHALL 按后续变更逐个追加（MUST NOT 一次性铺开）。
- **bootstrap 的近似误差 SHALL 如实留痕**：glob 导入（`use a::*`）、同名符号、宏生成引用都会让「有引用者」判定偏宽——这些是 bootstrap 的已知近似，MUST NOT 被伪装成精确结果。

#### Scenario: verify 三件套闭环

- **WHEN** 改动代码签名但不重生成生成物
- **THEN** `api:check` 的 `--check` 路径报差异并以非零退出（点名文件）；重生成后零 diff；若改动与契约不符则编译器报错（Rust 签名不匹配 / TS 形状不符）

#### Scenario: `--check` 零写盘

- **WHEN** 以 `--check` 在被修改过的仓库上运行（有差异）
- **THEN** 磁盘字节零变化（含生成物、`INDEX.toml` 与 mtime 语义上的「不重写」），退出码非零、输出列出差异文件

#### Scenario: 多行签名抽取不截半

- **WHEN** 对含多行 `pub fn`（参数逐行排列）的源文件跑 bootstrap 抽取
- **THEN** 归一签名含全部参数与返回类型；以 `grep` 单行抽取的对照输出被截半（该反例在案）

#### Scenario: visibility 四值判定

- **WHEN** 对同一 crate 的 `pub fn`（`pub mod` 链可达）/ `pub(crate) fn` / 私有 `mod` 内的 `pub fn` / `#[cfg(test)]` 树内被引用的 `pub fn` 跑 bootstrap
- **THEN** 依次判为 `external` / `crateOnly` / `crateOnly` / `testOnly`；TS 侧本期只产 `external` 一档（有 `export`），`Unused` 的降级接口 `apply_knip_report` 本期不接线（生产面不可达，显式留白），`crateOnly` / `testOnly` 同属本期留白（与 requirement 正文同口径，MUST NOT 在 Scenario 里给出更宽的承诺）

#### Scenario: bootstrap 只出 stdout 不落盘

- **WHEN** 执行 `bootstrap` 子命令
- **THEN** 候选草稿只写 stdout（含 `symbol` / `kind` / 定义位置 / 归一签名 / `visibility` / 引用方（`used_by`，本期留白）），工作区磁盘零变化；`git status` 干净

### Requirement: 模块嵌套与选择性透传

父子元素的契约关系 SHALL 遵循「父 → 子」单向：父元素契约 SHALL 逐项列出它透传的子元素导出，MUST NOT 由「子元素面的并集」自动推导。

透传 SHALL 同时表达于三处且互相一致：

| 载体 | 内容 | 用途 |
|---|---|---|
| `<FQN>.md` 的 `api-reexport` fence | 透传清单（键 = 导出名，值 = 直接子元素 FQN） | 人读 + 生成器输入 |
| `<FQN>.md` 散文区 | 透传意图 | 人读 |
| `<FQN>.schema.json` 的 `$defs` | 本地定义 **+ 透传项（纯 `$ref`）** | 机器检查（悬空引用编译期即报） |

六条结构规则：

1. `api-reexport` fence 键集 SHALL ≡ `schema.json` 的透传 `$defs` 键集（漂移即 `apiReexportDrift`）。
2. 父 `$defs` 的透传条目 MUST NOT 携带结构关键字（`type` / `properties` / `enum` / `const` / `items` / `oneOf`）——它是纯 `$ref` alias，携带即沦为重复定义。
3. 父 `$defs` 与任一子元素 `$defs` 的**类型名交集必须为空**（`apiShadowing`）。MUST NOT 依赖编译器兜住：实测 `pub use drift::*` + 同名 `struct` 只给 `ambiguous glob re-exports` 警告，不报错。
4. 透传目标与跨模块 `$ref` SHALL 是**直接子元素**（FQN 前缀相邻）且其契约存在且 `api = declared`；跨层引用（父直连孙辈）SHALL 报 `apiRefMissing`——层级语义被架空的信号。
5. 嵌套元素的父子 MUST 同属**一个 crate**（Rust 侧跨 crate 父子即 `cyclic package dependency`，不可表达）→ `apiNestedCrossCrate`；TS 侧无此约束。
6. 子元素 MUST NOT 依赖父元素（含类型）；共享类型 SHALL 下沉到兄弟元素或消解依赖。该规则在代码层由既有 `UndeclaredDependency` 判据报（父子关系不进 relation 图，故必然报），本能力 MUST NOT 新增第二条判据。

两条推论：

- **递归透传 MUST NOT 展开**：若 A ⊃ B ⊃ C，A 只声明透传 B；B 内部已透传 C 则自动带出。
- **透传是选择性的**（实测非假设）：`App.Frontend.Components.Agent` 有 6 个子元素但只透传 2 个（`AgentInput` / `AgentSessionView`），另有自有导出 `SessionPanel`（其文件不是元素）、4 个未导出子元素不出现。故父契约是「透传自子元素 / 自有 / 子元素但不导出」三类混合，MUST NOT 自动推导。
- **环依赖从此不可能**：跨元素 `$ref` 只剩「父→子」与「兄弟→兄弟」两类边，必然可拓扑排序。禁断子→父的最大价值不是禁断本身，而是让「把子目录提升为元素」变成一次真实的解耦决策（`infra/agent` 要把 `sdk/` 提升为元素，先得把 3 个类型从 `lib.rs` 挪出来）。
- **MUST NOT 新增契约层依赖判据**：契约不是依赖事实源，事实链是「契约 → 生成物 → 代码 import → `scan` 取数 → `OwnerIndex` 归属 → `drift::check` 对账既有 `UndeclaredDependency`」。判据建在事实源（代码）上，不在投影（契约）上；新增判据只在「契约自身内部矛盾」时成立。
- **契约记录「提供什么」，不记录「实现了什么」**：MUST NOT 在 `$defs` 条目上记 `implements` 注记（重复表达 model relation、污染契约语义、判据已被编译器与 `UndeclaredDependency` 双重覆盖）。推论：三个 runner 的契约面就是 `new()`——该薄面是代码形状的事实，处置是改代码（暴露已存在的私有纯函数）而非放宽格式。

#### Scenario: 父契约三类混合

- **WHEN** 解析 `App.Frontend.Components.Agent` 的契约（透传 `AgentInput` / `AgentSessionView`、自有 `SessionPanel`、4 个未导出子元素）
- **THEN** `api-reexport` fence 含 2 条、`$defs` 含 2 条纯 `$ref` 条目 + 1 条本地定义；4 个未导出子元素零出现；生成的 `.i.ts` 为 `export { AgentInput } from …` / `export { AgentSessionView } from …` / `export interface SessionPanel { … }`

#### Scenario: 透传条目带结构关键字即错

- **WHEN** 父 `$defs` 的透传条目同时写 `$ref` 与 `type: object` / `properties`
- **THEN** 契约错误（透传条目必须是纯 `$ref` alias）；`apiShadowing` 亦报（与子元素类型名交集非空）

#### Scenario: glob 形态重复定义由契约层拦住

- **WHEN** 父元素代码用 `pub use child::*` 且手写同名 `struct`（编译器只给 warning）
- **THEN** `apiShadowing` 报出父子 `$defs` 类型名交集，检查失败；MUST NOT 以「编译器没报错」为由放行

#### Scenario: 跨层与跨 crate 引用各报其判据

- **WHEN** 分别构造「父契约 `$ref` 孙辈元素」与「父子元素分属两个 crate」
- **THEN** 前者报 `apiRefMissing`（非直接子元素），后者报 `apiNestedCrossCrate`；两者都点名涉事 FQN

#### Scenario: 子→父依赖由既有判据报

- **WHEN** 子元素代码 `use` 父元素直接定义的类型（父 `lib.rs` 里的 `EngineFacade` 一类）
- **THEN** 既有 `UndeclaredDependency` 报出该依赖（契约层不加新判据、不出现第二份判定实现）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/mod.rs` | 契约面装配与出口 | 出口恰八行（与实现相的函数表逐项相等）：`meta() -> ApiMeta`（生成载元单点）/ `enabled(root) -> bool`（契约面启用判据）/ `load_contracts(root, elements) -> Result<ApiSnapshot, ApiError>` / `render_all(root, snapshot) -> Result<Vec<GeneratedFile>, ApiError>` / `bootstrap(root, element) -> Result<String, ApiError>` / `check(input) -> Vec<ArchiApiFinding>`（判据语义归 `desktop-archi-api-check`，此处只登记它是契约面唯一出口）/ `api_ref` / `api_ref_of`（契约状态投影两入口）；MUST NOT 另立「比对 / 归一」函数——一致性由 `generate --check` 字节比对 + 编译器两步回答；纯读盘、零写盘、零 spawn。`ApiSnapshot` 为「契约 / 内存 schema / 代码符号集」三面，**不含缺契约清单**（缺口由判据 ② 从模型意图与 `contracts` 的差集直接得出，MUST NOT 另存第二份同一事实） |
| `crates/core/archi/src/api/paths.rs` | 契约与生成物路径单点 | `normalize_root` / `module_name(element_root)`（透传 `<name>` 唯一产出点）/ `rust_module_path(element_root)`（Rust 透传模块路径唯一产出点）/ `domain_of(fqn)` / `directory(root)`（`archi_api_dir` 唯一转发点）/ `index_path(root)` / `language_of(file)` / `generated_kind(language) -> GeneratedKind`（语言分派单点）/ `contract_path(root, fqn) -> Option<PathBuf>` / `schema_path(root, fqn) -> Option<PathBuf>` / `generated_path(root, element_root, kind) -> Option<PathBuf>`（入参是已归一 `<root>` 字符串，容器元素由调用方 `path.first()?` 表达）/ `entry_form(root, element)`（三分支 / 四分支查入口，形态单点）/ `mod_path_attr(form, name)`（`#[path]` 值的等值真值源，属性值本身是字面量）/ `ts_import_specifier(form, name)` / `ref_relative(from_fqn, to_fqn)`；全经 `archi_api_dir` 派生，零行内 `api` 段与零手算相对路径；**四类路径派生 MUST 做路径形态校验**（FQN 或元素 `<root>` 非 repo 相对形态——绝对路径 / 盘符 / `..` 段 / 空段 / 空串——即 `None`），否则 `Path::join` 会替换 base 或上溯，把在盘探测与工具写盘带出仓库；`module_name` 是**纯派生零形态判据**（文件名与 `#[path]` 值对任意末段都成立）；末段**进目标语言源码**时的形态闸**按语言分列**：Rust 侧由 `paths::is_module_segment` 判**「形态 `[A-Za-z_][A-Za-z0-9_]*` 且非空 **且非 Rust 关键字**」**（关键字集由一个 `const` 封闭持有；放行关键字即产出 `pub use crate::loop::X;` 这种不可编译文本，而产出 `r#loop` 是第二形态、MUST NOT），不成立即令该透传目标不入查表、渲染器只出 `// 透传自 <FQN>` 注释（零改名 / 零清洗）；TS 侧由 `emit::ts::ts_string_literal` 转义、**零筛选**（带扩展名的 specifier 是前端单文件元素的合法形式）。`checked_root` 只判整条 `<root>` 的段结构，**不判末段字符集**，故 `module_name` 的 doc MUST NOT 声称其输出「受 `checked_root` 约束即可进源码」 |
| `crates/core/archi/src/api/contract.rs` | `<FQN>.md` 解析 | H1 锚点校验、`### <op-id>` 分段、三类 fence 提取、双向 gate 三条校验、构造器声明检测；错误带文件与行 |
| `crates/core/archi/src/api/schema.rs` | `$defs` 规则 + 装配 | `$defs` 身份 / 提升原名 / `$ref` 顶层 / 禁按结构去重 / 透传条目纯 `$ref`；`schema.json` 六字段装配（`$schema` / `$id` / `title` / `$defs` / `properties` / `$comment`）；`INDEX.toml` 装配 |
| `crates/core/archi/src/api/extract/mod.rs` | 取数器分派 | 按语言分派到 `rust` / `ts`；产出候选 `{ symbol, kind, location, signature, visibility }` 与引用方（`used_by`，本期留白）栏；仅 bootstrap 使用，非权威 |
| `crates/core/archi/src/api/extract/rust.rs` | Rust 声明抽取 | `syn` 遍历 `pub struct` / `trait` / `enum` → `$defs`，`pub fn` → `x-function`；签名 AST 归一单行；`visibility` 四值（`pub mod` 链 / `pub(crate)` / 私有 `mod` 内的 `pub` / `#[cfg(test)]` 树） |
| `crates/core/archi/src/api/extract/ts.rs` | TS 声明抽取 | `oxc` 遍历：`export function` → 操作（唯一判定），`export type` / `interface` / `class` → `$defs`，`export const` 按初始化表达式分类（`memo(...)` → 组件留白 / 对象与 Proxy → 排除 / 箭头函数 → **不作操作**，沿本 capability「MUST NOT 为从初始化表达式推断函数造启发式」）；`unused` 的降级接口 `apply_knip_report` 已实现但**本期未接线**（生产面不产出 `Unused`，见待决 5 / 12）；接线是独立变更 |
| `crates/core/archi/src/api/emit/rust.rs` | `.i.rs` 渲染 | `struct` / `trait` / `enum` / newtype / 自动构造器 / 透传 `pub use`；头注释含工具版本与 `common.json` 摘要；纯函数出字符串 |
| `crates/core/archi/src/api/emit/ts.rs` | `.i.ts` 渲染 | `interface` / `as const` + `type` / 别名 / 透传 `export { } from`；ambient-free（MUST NOT 用 `.d.ts`）；头注释同上 |
| `crates/core/archi/src/scan/owner.rs` | 生成物归属（判据修正） | 剥离由「替换扩展名」改为**候选集**：整组剥离 `<name>.i.<ext> → <name>` ∪ 替换扩展名 `<stem>.i.<ext> → <stem>.<ext>`；`owner_of` 对每个候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重后定档（同长仍 `Ambiguous`），**MUST NOT 候选短路**；`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不变；非 `.i.<ext>` 路径归属零变化 |
| `crates/core/archi/src/scan/owner_test.rs` | 判据断言面（修正） | `生成物剥离四种形态各归其元素` 保留既有四条目录内形态并**另加**四条根同级实形态正例（含 `crates/infra/archi.i.rs`）；`src/dir.i.ts`（根 `src/dir/`）期望由 `Owner::None` 翻转为 `Owner::Element`（把 bug 写成契约的那条断言即修复本身），`生成物剥离使单文件元素根同级生成物归元素` 的用例名 / doc / 断言意图同步改写；新增并集定档用例（`src/dir/` 与 `src/dir.ts` 同场 ⇒ 单文件根胜出）；白名单外后缀 4 条反例保持绿 |
| `crates/core/archi/src/scan/mod.rs` | 扫描编排（零判据改动） | `walk_source_files` / `is_source_file` 提为 `pub(crate)` 供文件树复用；`scan()` 的枚举 / 排除 / 归属 / 抽取 / 聚合六段语义逐字不变（`owner_of` 的调用点两处——源文件归属与目标归属——自动获得并集判据） |
| `crates/core/archi/src/tree.rs`（新增，见 `desktop-archi-file-tree`） | 文件树投影 | 归属与剥离一律经 `OwnerIndex`；本能力只要求「生成物按并集判据归属，树侧零第二份实现」 |
| `foundation::layout::archi_api_dir` / `archi_api_rel_dir`（新增，见 workspace-layout-resolution） | 契约目录派生 | `archi_api_dir(root) -> PathBuf` = `<archi_dir>/api`；`archi_api_rel_dir() -> String`；全从既有派生族派生，纯推导零 IO |
| `crates/tools/archi-api-gen/src/main.rs`（新，见 desktop-crate-layout） | 唯一写盘面 | CLI 三子命令 `generate` / `--check` / `bootstrap`；`--check` 零写入、有差异即非零退出并列文件；目录创建经 layout 派生 |
| `packages/desktop/package.json`（脚本面） | 生成与门禁驱动 | `api:export`（写盘）/ `api:check`（`--check` 零写入 + 工具内比对）；`check` 由三段扩为四段（`server:check && bindings:export && api:check && client:check`——第二段是 `bindings:export`，生成物零 diff 守卫已退役），`bindings:export` 与 `client:check` 两段原文不变，`server:check` 的 fmt 段为 `cargo fmt --all --check`（clippy 段逐字不变） |
| `packages/desktop/knip.json` | 生成物可见性边界 | `ignore` 增 `src/**/*.i.ts`；`ignoreExportsUsedInFile` / `ignoreDependencies` 零新增 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | IPC 类型再生成 | 含 `ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` / 报告 `apiFindings`；再生成后入库、过期由提交前人工核对（零 diff 守卫已退役） |
| `.dev-team/architecture/api/**`（新资产） | 契约与生成物 | `<Domain>/<FQN>.md`（唯一手写）+ `<Domain>/<FQN>.schema.json`（生成）+ `INDEX.toml`（生成）；三者入库、进零 diff 门 |
| `AGENT.md` + `CLAUDE.md`（仓库根文档，见「仓库根文档的契约纪律节」） | agent 侧操作纪律入口 | 各新增一节 `## 模块 API 契约`（恰 5 条要点：手写真相位置与三类 fence / 四类生成物勿手改 / 两脚本与 `check` 链段序 / `api = none` 留白 / `apiFindings` 不进 `status`）；MUST NOT 复制契约正文、判据长表或 fence 示例；节内零 `openspec/specs/**` 路径；两文件该节正文逐字相同（同源纪律） |
