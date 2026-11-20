# relation.apis：架构关系的 API 依赖声明与画布连线详情

workflow_type: requirement

> **需求原文**（两个子句，是一条链的两端）
> 1. **模型侧**：`[[relation]]` 可**选填** `apis`；填了就与代码实际依赖比对，不匹配要检查出来。
> 2. **UI 侧**：画布连线**可点**，看这条依赖的详情。
>
> 本文是取证 + 设计定案，不含实现。**一句话结论**：难点不是"加个字段"，而是「实际依赖」要从**元素对**粒度升到**符号**粒度——今天整条管道（解析 → 扫描 → 对账 → 报告 → DTO → 画布）都在元素对粒度，名字在扫描那一层就被丢掉了。

---

## 0. 速览

### 五项定案

| # | 裁决点 | 定案 | 关键依据 |
|---|---|---|---|
| 1 | `types/generated` 归属（前置） | 新增中继元素 `App.Frontend.GeneratedBindings`（`kind = module` / `api = none` / `path` = 生成目录）；12 条前端关系改指向它；**上游生成链不声明** | 生成物零契约维护（仓里 `ExportBindings` / `ArchiApiGen` 同款先例）；上游边不是 import 事实，声明即永久 unfulfilled |
| 2 | `apis` 取值域 | **代码导出名**（`ApiSymbol.name` 名字空间） | 契约声明面全仓仅 11 个名字、8 份前端契约 `props = 0` ⇒ 契约名不可用（§4.1） |
| 3 | 护栏 | 只开 ①「只许名字」③「可空引用」；**不做 ⊆ 导出集机械证成** | 省掉导出集装载与中继元素的 `entry` 冲突（§4.2） |
| 4 | 门禁口径 | `used ∖ declared` → 新增 `ArchiDriftKind::UndeclaredApi`（`ALL` 4→5，按 `from.gate` 定级）；`declared ∖ used` → 点亮既有空位 `ArchiObservationKind::UnusedDeclaration`（observation，永不进门禁） | 粒度与"元素对级"区分开；`unusedDeclaration` 是既有空位 |
| 5 | 收数面 + UI | 第 9 条命令 `archi_relation(root, from, to)` 按需取；**drift 报告形状零变化、`version` 仍 3**；UI = 右栏面板两态（`ModelInfo ⇄ RelationInfo`） | 点开即取、按需付费；报告与 16 份金样零 churn |

### 三个反直觉结论

1. **契约声明面在这个仓里几乎没有可用名字**（8 份前端契约 `props = 0`）——"`apis` 引用契约名"纸面漂亮、实测不可用。
2. **跨语言名字转换几乎不发生**：前端 import 生成物拿的是 Pascal 类型名（与 Rust 同形）+ 一个 `commands` 桶；被 camel 化的命令 key 只在成员访问里，低于 `apis` 射程（§4.3）。
3. **`declared ∖ used` 天生混两种成因**（真没用 / 写错或看不见），与仓里「一个值 MUST NOT 承载两种缺口」的纪律正面冲突 ⇒ 必须显式豁免（§5.2）。

---

## 1. 现状：五层管道全在元素对粒度

```
模型声明 .dev-team/architecture/graph/model/NN-*.toml
  [[relation]] from / to / desc                        ← 只有三键（RawRelation，parse/toml.rs:58）
        │
        ▼  parse → AST
  Relation { from, to, description, file, line, span }  ← model.rs:72（有行号，无名字）
        │
        ├──► DTO 投影
        │    ArchiRelation { from, to, description, mark, reason }   ← dto.rs:436
        │    ★ 连 file/line 都没出线（画布拿不到"这条边写在哪一行"）
        │
        ▼  漂移判定（drift/mod.rs）
  三类漂移 + 观测项，全部按 (from, to) 聚合：
    ① 声明未兑现 → observations（unfulfilledDeclaration，永不进门禁）
    ② 未声明依赖 → errors / warnings（按引入方 gate）
    ④ path 失效  → errors / warnings
        ▲
        │  扫描事实（scan/）
  ImportFact { specifier, line, type_only }              ← scan/mod.rs:42
  DependencyEdge { from, to, file, line, specifier, type_only, inline_ignore }  ← :56
        │  specifier 是「模块路径」（Rust 规范化 use 路径 / TS 模块 specifier）
        │  ★ 名字（use 的尾段 / import 的具名列表）在 AST 里明明有，一处都没被接住
        │
        ▼  画布
  toEdges() → { id: 'from->to', source, target, style }  ← architecture-canvas.tsx:210
  交互只有 onNodeClick（切 focus）；边无 label、无 onEdgeClick、无选中态
```

⇒ 需求的真实形态：**把 `specifier` 旁边那个「名字」接住，一路送到对账与画布**。

---

## 2. 取证：「实际依赖」能取到多少名字

三支只读探针（真实代码，不上库）。模型共 **122 条关系**（后端 49 / 前端 73）。

| 取数面 | 名字来源 | 本仓命中关系数 | 精度 | 备注 |
|---|---|---|---|---|
| **G1** `use` 尾段 + 具名 import | `use a::b::{C, D}` / `import { C, D }` | 后端 40/49；前端 50/73（另有 `export … from` 边由真扫描器补上） | 高（无假名） | 现成 AST，改动最小 |
| **G2** G1 + 内联限定路径 | `exec::SystemCommand::bare(..)` 这类**没写 use** 的路径表达式 | 后端 38/49 与 G1 互补（合并近全覆盖） | 中（尾段可能是**模块名** `layout` / `dto` / `port`） | 需新 AST 访问器 + 噪声过滤 |
| **G3** G1 ∩ `to` 导出集 | `api::extract_element`（syn 走整包 / TS 走导出闭包） | —— | 最高 | 单个 `to` 元素 syn 解析 8–15 文件；成本随填表率线性增长 |

G1 实测样本（真实 `use` 尾段 / 具名 import）：

```
App.Backend.Core.Config          -> App.Backend.Core.Foundation   { config_path }
App.Backend.Core.Checks          -> App.Backend.Core.Config       { TestSuite }
App.Backend.Core.Orchestration   -> App.Backend.Core.Agent        { AskRegistry, FinishSpec, ModelLevel, AgentEvent, … 11 个 }
App.Frontend.Views.Architecture  -> App.Frontend.Components.Agent { AgentInput, SessionPanel, SessionPanelState }
App.Frontend.Components.Agent.ToolCards -> …Components.Ui         { Alert, Card, CodeBlock, Questionnaire, … 28 个 }
```

**前端精度明显好于后端**：TS 一条边常 5–28 个具名符号，Rust 一条边常 1–3 个（`use` 常只引类型，方法调用不可见）。

### 三条必须承认的硬伤（否则会写成假阳判据）

1. **Rust 方法调用不可见**：`use foundation::cancel::CancelFlag;` + `flag.is_set()` → 事实只有 `CancelFlag`。而 `is_set` 恰是契约里声明的 op。⇒ Rust 侧 `apis` 的真实粒度 = **类型 + 自由函数**，方法级写不出来。
2. **`packages/desktop/src/types/generated/` 不属于任何元素**：本仓最大"无主导入"面（25+ 处 `import … from '../../../types/generated/bindings'`）。扫描对目标无归属的边**直接丢弃**（`scan/mod.rs:276`）⇒ 12 条 `App.Frontend.* → App.Backend.Shell` **今天就是永久 unfulfilled 观察项**，填了 `apis` 也永远"判不了"。这是**前置依赖**（§3）。
3. **别名 / glob / `self` / type-only 的归一要一次定死**：`use x::Y as Z` 记 `Y`（真名）；`use x::*` 无名字；`use x::{self}` 是模块本身（longest-module-prefix 吃掉全部段 ⇒ 尾段空，天然不产名）；`import type { X }` 里 `X` 照算（与"type-only 算架构依赖"同权）。

---

## 3. 定案一：前置依赖 = 中继元素 `App.Frontend.GeneratedBindings`

### 3.1 为什么必须先做

逻辑关系是 `前端 → 后端`，代码关系是 `前端 → bindings → 后端`。12 条边今天判不了；而 `bindings.ts` 实测导出 **136 个名字**，是前端最强的名字面。不解决归属，`apis` 在这族边上恒为"判不了"。

### 3.2 归属方案对比

| # | 方案 | 改动量 | 代价 | 结论 |
|---|---|---|---|---|
| **O1** | 新增 `api = none` 元素，12 条关系改指 | +1 元素 + 12 行 | 分母 44→45、`docMissing` 43→44（须同批改基线） | **采纳** |
| O2 | 生成目录挂成 `App.Backend.Shell` 的第二条 `path` | 1 行 | 元素根混合语言；与"⊆ 导出集校验"永久互斥；文件树里 Shell 名下出现 TS 文件 | 否决（中继元素接管归属后作废） |
| O3 | 给 `[[relation]]` 加"生成关系"语义位（`kind = "generated"`） | 模型字段 + 判定分支 + 边标记第五值 | 三处随动；且上游"生成链"仍无判据 | 否决（贵且不解决上游） |
| O4 | 不归属 | 0 | 12 条边永久灰虚线 | 否决 |

补充：O2 的机械可行性本身没问题（`OwnerIndex` 按 path 逐条建条目、`tree.rs` 的 `roots` 是「path → 元素」映射、`language_of` 只看入口扩展名），否决理由是语义与后续校验面的耦合。

### 3.3 落地

```toml
# .dev-team/architecture/graph/model/06-frontend.toml
[element."App.Frontend.GeneratedBindings"]
kind = "module"          # 复用四值之一，零词汇改动（不引入第五值 generated）
gate = "off"             # 只作 `to`，定级永远看引入方 ⇒ gate 无定级语义
api = "none"             # 无契约面：零契约 md / 零 schema / 零 fence
path = ["./packages/desktop/src/types/generated/"]
summary = "IPC 命令面与出线 DTO 的 TypeScript 投影（tauri-specta 生成，勿手改）"
```

- 12 条 `from = "App.Frontend.*"`、`to = "App.Backend.Shell"` 的关系改 `to = "App.Frontend.GeneratedBindings"`；`desc` 由「经生成的命令面类型取数与调用」微调为「经生成的 IPC 绑定取数与调用」。
- **上游"生成链"不声明**：`GeneratedBindings → Shell / Core.Archi / …` 不是 import 事实，声明了立刻变永久 `unfulfilledDeclaration`——与本次要消掉的噪声同类。要表达它得先有「生成链对账」判据（`bindings.ts` 的 invoke 串集（snake）× `#[tauri::command]` 名集，零格式转换），那是独立能力。
- 生成物面 **`apis` 留空**：生成物是唯一真相，手写清单必然漂；详情面板如实写「生成物面，未声明 API（建议留空）」。

### 3.4 必付的连带成本（机制已核）

| 面 | 变化 | 依据 |
|---|---|---|
| 模型头注释 | `06-frontend.toml` 首行「30 元素 / 71 关系」→「31 元素 / 71 关系」 | 关系数不变（改指不是增删） |
| `api` 判据基线 | 分母 44 → 45、`docMissing` 43 → 44 | 新元素 `api = none` 且无 `doc` ⇒ 恰 +1；`desktop-archi-api-check` 基线表 MUST 同批显式更新（「偏离即检查器缺陷」要求有意改数） |
| 入口判据 | 该元素**零** `apiEntryMissing` | `api = none` 已退出入口判据；`types/generated/` 无 `index.ts`，无需为它造 |
| 扫描 | 3 个生成文件由「无主」变「归该元素」；`scan.scannedFiles` +3 | `OwnerIndex` 最长前缀，单/多 path 同机制 |
| 12 条关系 | 由 `unfulfilledDeclaration` 转「已兑现」 | import `'@/types/generated/bindings'` 的目标从此有主 |
| 文件树 / 元素树 | 各多一处（元素根标记 / 一行） | `tree.rs` 的 `roots` 按 path 建；`FileTreeMark::ElementRoot` |
| DTO / 绑定 / 前端 | **零改动** | 纯模型数据变化（`bindings.ts` 无 diff） |

---

## 4. 定案二：`apis` 取值域 = 代码导出名

### 4.1 死路先算清：契约声明面全仓只有 11 个名字

`api-op` 的 props + `api-types` 的 `$defs`，10 份契约 md 合计：

```
App.Backend.Core.Foundation      props = 9   defs = 0
App.Backend.Infra.ArchiRuntime   props = 2   defs = 1   (file_stem, LocalArchiCheck)
其余 8 份（前端 Agent 组件 ×8）    props = 0   defs = 0   ← 全留白（JSX / 跨模块具名类型都写不进契约词汇）
```

且这 11 个里 `archi_*_rel_dir` / `is_set` 没人跨元素 `use`；真正被依赖的 `config_path` / `Layout` / `ReportSlot` 全在「契约面留白」名单里。⇒「只许契约声明名」**不是略窄，是不可用**。

⇒ 取值域只能是 **`to` 元素的代码导出名**（`ApiSymbol.name`），与 `used` 事实同一名字空间——换名字空间就要多一层映射 = 第二份真相。

### 4.2 红线怎么改写

现行条文（`desktop-archi-model`「协议正文不经模型」）的对象是**元素**、理由是**"存一份正文会漂移"**。`relation.apis` 在这两点上都能被区分开：

| 维度 | 协议正文（被禁） | `relation.apis`（允许） |
|---|---|---|
| 位置 | 元素（提供方的自我描述） | 关系（**消费方**的依赖选择） |
| 内容 | 签名 / 类型形状 / 接口定义 | **只有名字** |
| 权威 | 会在模型里造第二份协议真相 | 名字权威仍在代码导出集 / 契约 fence；模型只"选了哪几个" |
| 抗漂移 | 无机制 | 每次检查机械证成（若做护栏②） |

定案的护栏（只开两条）：

```
① 只许名字：无签名 / 无参数 / 无类型形状 / 无 `::` / 无空白
③ 可空引用：选填；缺省 = 不声明 API 粒度（≠ 无依赖）；`apis = []` 拒收（与省略同义）
（② ⊆ 导出集机械证成：不做 —— 省掉导出集装载，且避开与中继元素 entry 的冲突）
```

**不做②的两个后果必须写进 spec**：错字只能落 `declared ∖ used` 观察项；且 `declared ∖ used` 因此混两种成因（§5.2）。

### 4.3 名称转换发生在哪（跨语言问题的最硬一问）

| 面 | Rust 侧 | `bindings.ts` 侧 | 转换 | 名字级可比 |
|---|---|---|---|---|
| 命令 | `fn archi_graph` | key `archiGraph` **＋ invoke 串 `"archi_graph"`** | 仅 key | **能，零映射**（用 invoke 串 = snake↔snake） |
| DTO 类型 | `struct ArchiGraph` | `type ArchiGraph` | **否** | 能，直接等名 |
| DTO 字段 | `model_file` | `modelFile` | 有 | 不参与（`apis` 是名字不是字段） |
| 枚举线值 | `ApiEntryMissing` | `"apiEntryMissing"` | 有 | 需归一（不建议写进 `apis`） |
| 值形态 | `Result` / `Option` / `i64` / `Value` | throw / `null` / number / unknown | **有损** | 不可比且不该比 |

前端实际 import 的是什么（实测）：

```
import type { ArchiApiFinding, ArchiElement, ArchiGraph, ArchiRelation } from '@/types/generated/bindings'
import { commands, type ArchiApiCheck, type ArchiApiFinding } from '@/types/generated/bindings'
                     ↑ 唯一桶名            ↑ 全是 Pascal 类型名（与 Rust 侧同形）
```

⇒ 「前端 → 中继」段要写的 `apis` 全是 **Pascal 类型名 + `commands` 桶名，零转换**；camel 化的命令 key 只在 `commands.archiGraph(...)` 成员访问里，低于射程。真要写"用了哪条命令"，映射表也不必新增常量——生成物自己双名并存（`archiGraph: … __TAURI_INVOKE<T>("archi_graph", …)`），机械可提。

### 4.4 名字形状规则（一次定死，否则"对不上"会被当 bug 排查半天）

| 项 | 规则 | 依据 |
|---|---|---|
| Rust | **裸符号名**（`config_path` / `CancelFlag`）；模块名不产名 | 最长模块前缀被吃掉，尾段即符号 |
| 方法级 | **写不出来**，只到「类型 / 自由函数」 | §2 硬伤 1 |
| 别名 | 取**被引入名**（`use x::A as B` → `A`） | `flatten_use_tree` 的 `Rename` 臂已是此语义 |
| TS | **元素导出名**（Pascal 类型 / camel 函数 / barrel 名）；**成员访问不算** | TS 导出闭包口径 |
| 比对 | **逐字符**，禁止大小写折叠 / 驼峰归一 | 否则判定侧常驻第二处映射规则 |
| 值面 | 非空、去重、禁空白、禁 `::` | 防"看起来声明了" |

---

## 5. 定案三：门禁口径 = 两桶，档位由取数精度决定

### 5.1 两桶与档位

```
Declared（模型 apis）                 Used（扫描事实，under-approx）
   └── used ∖ declared  → 有证据（import 是硬事实）
                          按 from.gate：strict → errors / warn → warnings / off → 不出现
                          词汇：新增 ArchiDriftKind::UndeclaredApi（ALL 4 → 5）
   └── declared ∖ used  → 无证据（≠ 反证）⇒ observation，永不进门禁、不进 passed
                          词汇：点亮 ArchiObservationKind::UnusedDeclaration（ALL 长度不变）
```

不对称的**理由不是审美，是取数精度的直接后果**：`exec::SystemCommand::bare` / `.is_set()` / glob 都能让真依赖"看不见"，故"没看见"不能判红。

### 5.2 第二桶的两种成因（必须显式豁免一条纪律）

```
(a) 真没用：声明了但确实不再依赖
(b) 写错 / 看不见：`config_paht` 错字；或依赖以「内联限定路径 / 方法调用 / glob」形态存在
```

这与 `desktop-archi-api-check` 的「三值各只有一种成因，同一个值 MUST NOT 承载两种缺口」正面冲突 ⇒ spec 里**显式豁免**并写明理由（成因不可机械分辨，因为取数面是 under-approx、下限不得靠猜）；**报告文案要并列两种可能**，MUST NOT 只写"没用"（否则错字被当成可忽略项）。

（可选的廉价升级，本次不做：scan 本来就逐文件读源码，可顺带收"每个元素的标识符集合"，则"名字不在 `used` 但出现在 `to` 的文件里" ≈ 存在但未被观测到，"根本不在" ≈ 错字强证据。代价是多一条弱事实族 + 注释/字符串噪声要裁。）

### 5.3 冷启动与噪声保障

```
未填 apis 的仓 → 两桶皆空 ⇒ 报告与既有 16 份金样逐字段相同（零噪声）
```

---

## 6. 定案四：收数面 = 第 9 条命令 `archi_relation`

### 6.1 命令与 DTO

```
archi_relation(root, from, to) -> Result<ArchiRelationDetail, String>
  ├─ from / to / description / mark / reason      ← 模型 + 豁免（与 ArchiRelation 同源）
  ├─ modelFile / modelLine                        ← 模型声明行（点开定位；ArchiRelation 现在没有）
  ├─ declared        : 模型 relation.apis（空 = 未声明）
  ├─ used            : 扫描事实里的名字（去重、字典序）
  ├─ usedNotDeclared / declaredNotUsed : 与 drift 判据同一个纯函数求差
  └─ occurrences     : 复用既有命中点形状（file / line / specifier / typeOnly）
```

- **判定单点仍是 `drift`**：求差抽成纯函数（如 `api_buckets(declared, used)`），drift 对账与新命令都只调它；命令层零领域规则（沿 `commands/archi` 的三件事纪律）。
- **`ArchiRelation` 加 `apis: Vec<String>`**（模型拷贝，字段不省略）——画布不必为新面板再取数就能显示声明清单；`modelFile` / `modelLine` 同批加则新命令只剩投影。
- **扫描范围**：只扫 `from` 元素 `path` 下的文件；机制借用 `ScanInput.mode = Incremental` 的 `changed` 白名单（字面语义就是"只扫其中的源文件"），成本 = 解析 `from` 的十来个文件（毫秒级），目标归属走 `OwnerIndex`（不需解析目标）。
- **不落盘、不进报告**：报告的 `errors / warnings / observations` 只承载结论（门禁用），事实面按需取。

### 6.2 为什么不是报告 facts 段

| 维度 | 报告 facts 段 | **第 9 条命令（采纳）** | 只出结论 |
|---|---|---|---|
| 报告版本 | 3 → 4 | **不变（仍 3）** | `occurrences.names` 仍改形状 |
| 金样 / zod / 前端语料 | 16 份金样 + 绑定 + 载荷语料全动 | **零** | 部分 |
| 点开延迟 | 0 | **几十 ms** | 0 |
| 审计价值 | 高 | 无 | 中 |
| 覆盖 | 全部边 | 点过的边 | 仅失配边 |

### 6.3 一条必须显式开的 spec 口子

`desktop-archi-canvas` 规定画布取数只许三种触发（挂载 / `root` 变化 / 用户显式刷新），禁 watch 与轮询。

点边取详情是**显式用户动作的按需查询**，既非 watch 也非轮询 ⇒ 合规，但必须写明否则会被当成第四个触发点：

> 按需查询（边点击）由显式用户动作发起，**不计入三触发**；仍 MUST NOT 缓存失效以外的自动重取。

---

## 7. 定案五：UI = 右栏面板两态

```
CardHeader：元素态 ModelInfo（现状）  ⇄  关系态 RelationInfo（新增）
                  ▲ 点节点                        ▲ 点边 （同一块面板位）

RelationInfo
  关系   Views.Architecture → Components.Agent         [标记徽标：unfulfilled / exempt]
  说明   复用会话视图与输入框
  声明 API (3)   AgentInput · SessionPanel · SessionPanelState     ← ArchiRelation.apis
  实际使用 (3)   AgentInput · SessionPanel · SessionPanelState     ← 命令（点击后取）
  用了没声明     —                                                  ← usedNotDeclared（按 gate）
  声明没用       —                                                  ← declaredNotUsed（观察项语义）
  命中点  archi-chat-panel.tsx:12  import { SessionPanel } from '@/components/agent'
  模型    graph/model/06-frontend.toml:512  [查看]                   ← modelFile / modelLine
  加载态  「取数中…」（首次点击该边）；失败 → 行内错误 + 「重试」
```

### 7.1 四种空态必须彼此可辨

```
① 未声明 apis          「未声明依赖 API（选填）」                  ← 冷启动常态，不发起取数
② 扫描面无该边 import   「扫描面无该关系的 import」                 ← = unfulfilledDeclaration 的事实面
③ 边被豁免             「已豁免」（via = config / inline + reason）
④ 取数失败             行内错误 + 重试（图与元素面板照常渲染）
```

### 7.2 被否决的呈现位

| 方案 | 否决理由 |
|---|---|
| Drawer 弹出 | 盖住画布；详情页已在用抽屉，层级冲突 |
| 边旁浮卡（`EdgeLabelRenderer`） | 1-hop 画布边密集时遮挡；内容多会被截断 |

### 7.3 边上的失配徽标（本次不做）

`EdgeMark` 是封闭四值（`none` / `unfulfilled` / `crossKind` / `exempt`），扩第五值要动 canvas spec + `marks.ts` + 词汇对拍三处；只作 label 文案更便宜但与"画布只认标记不认判定"的纪律摩擦。记在这，不在本次。

---

## 8. 变更清单（按层）

| 层 | 位置 | 变化 |
|---|---|---|
| 模型数据 | `graph/model/06-frontend.toml` | +1 元素、12 关系改指、头注释 30/71 → 31/71（§3.4） |
| 解析 | `parse/toml.rs` `RawRelation` | + `apis: Vec<String>`（`deny_unknown_fields` 仍在 ⇒ 写 `api = [...]` 报 `unknownField`） |
| AST | `model.rs` `Relation` | + `apis`，其余零变化 |
| 模型 spec | `desktop-archi-model` | 字段面三键 → 四键；§4.2 红线改写 + 两条护栏；§5.2 成因豁免；`apis` 是开放值集，**不进**封闭词汇对拍（同 `doc` / `entry`） |
| 判据基线 | `desktop-archi-api-check` 基线表 | 分母 44 → 45、`docMissing` 43 → 44（同批显式改） |
| 知识文本 | `infra/agent/src/archi-knowledge.md:5` | 「（`from` / `to` / `desc`）」补 `apis` + 一句"只写名字、可留空" |
| 扫描 | `scan/mod.rs` `ImportFact` / `DependencyEdge` | + `names: Vec<String>` |
| 扫描 | `scan/rust.rs` | `resolve` 回带"最长模块前缀之后的尾段"（别名取原名；`self` / glob 不产名） |
| 扫描 | `scan/ts.rs` | 从 `specifiers` 取具名列表（`import` + `export … from` 两处；`type` 前缀剥除） |
| 判定 | `drift/mod.rs` | 求差纯函数 + 两桶对账（§5.1） |
| 词汇 | `drift/report.rs` | `ArchiDriftKind::ALL` 4 → 5（+ `as_str` + `report_test` 长度）；`UnusedDeclaration` 注记改产出条件 |
| 报告 | `drift/report.rs` `Report` | **形状零变化、`version` 仍 3** |
| DTO | `dto.rs` | `ArchiRelation.apis`（+ `modelFile` / `modelLine`）；新 `ArchiRelationDetail` |
| 命令 | `src-tauri/src/commands/archi/mod.rs` | `archi_relation` + `_inner`；`all_commands!` archi 段 8 → 9 |
| 守卫 | `src-tauri/src/bindings/mod_test.rs` | 命令 / 包装 48 → 49 邻位、DTO 清单 + 1、尾部序断言 |
| 生成物 | `bindings.ts` / `bindings.zod.ts` | `bindings:export` 重导（提交前人工核对 diff） |
| 前端（新） | `hooks/use-archi-relation.ts` | 按需取数 + 点击去重 + 失败行内错误 |
| 前端（新） | `components/relation-info.tsx` | 四空态 / 命中点 / 模型行号跳转 |
| 前端（改） | `components/architecture-canvas.tsx` | `onEdgeClick` + 选中态 + 面板两态切换（点节点回元素态） |
| 前端零改动 | `lib/marks.ts` / `lib/drift-marks.ts` | 逐字不变 |
| 语料 | `crates/core/archi/tests/corpus/drift/*` | **只加新 case**（声明满 / 缺一项 / 多一项 / 未填零噪声）；16 份既有金样零变化 |

**容易被忘的小尾巴**：`archi` 的模型结构 diff 以 `(from, to)` 作关系身份——只填 `apis` 的 change 在归档确认里是"零关系变更"。本次不做。

---

## 9. 任务骨架（按依赖序）

```
阶段 0  前置（可独立合并）
  T0.1  06-frontend.toml：+1 元素 GeneratedBindings；12 条关系改指；头注释 30/71 → 31/71
  T0.2  desktop-archi-api-check 基线表：分母 44→45、docMissing 43→44（同批显式改）
  T0.3  验证：12 条 unfulfilledDeclaration 归零；scannedFiles +3；树视图 elementRoot 新增

阶段 1  模型字段
  T1.1  RawRelation + apis；Relation.apis
  T1.2  desktop-archi-model spec：字段面 + §4.2 护栏 + 值面（非空/去重/禁 :: /禁空白）
  T1.3  archi-knowledge.md 一行

阶段 2  名字事实
  T2.1  ImportFact / DependencyEdge + names
  T2.2  scan/rust.rs：resolve 回带尾段（别名取原名；self/glob 不产名）
  T2.3  scan/ts.rs：具名列表（import + export … from；type 前缀剥除）
  T2.4  语料：Rust 四形态（use a::B / use a::{B,C} / use a::B as D / use a::*）+ TS 三形态

阶段 3  判定
  T3.1  求差纯函数 api_buckets（drift 与命令共用）
  T3.2  drift 两桶对账（used∖declared 按 from.gate 三档；declared∖used → observation）
  T3.3  词汇：ArchiDriftKind::ALL 4→5 + as_str + report_test；UnusedDeclaration 注记改产出
  T3.4  语料：新 case + 既有 16 份金样零变化

阶段 4  命令面
  T4.1  core：relation_detail(root, from, to)（模型 + 定向 scan + 求差）
  T4.2  DTO：ArchiRelation.apis（+ modelFile/modelLine）、ArchiRelationDetail
  T4.3  commands：archi_relation + _inner；all_commands! 8→9；bindings/mod_test.rs 计数
  T4.4  bindings:export 重导 + 人工核对 diff

阶段 5  前端
  T5.1  use-archi-relation.ts（按需取数 + 去重 + 失败行内错误）
  T5.2  relation-info.tsx（四空态 / 命中点 / 模型行号跳转）
  T5.3  architecture-canvas.tsx：onEdgeClick + 选中态 + 两态切换
  T5.4  用例：边点击取数 / 四空态 / 面板切换 / 未填 apis 不发起取数

阶段 6  收口
  T6.1  perf：零填表夹具零增量；补一个"10 条关系填 apis"的读数
  T6.2  词汇对拍：模型面写明 apis 是开放值集、不进封闭词汇对拍
  T6.3  本文 → change 的 design（五项定案 + S1/S2/S3 三条说明句）
```

---

## 10. 验收判据（可机械验证）

```
A1  未填 apis 的仓：drift 报告与既有 16 份金样逐字段相同（零噪声）——冷启动纪律
A2  填一条 apis 且漏一项 ⇒ 恰 1 条 UndeclaredApi（按 from.gate 落 errors / warnings / 不出现）
A3  填一条 apis 且多一项 ⇒ 恰 1 条 UnusedDeclaration，status / passed 不变
A4  ArchiDriftKind::ALL.len() == 5、ArchiObservationKind::ALL.len() == 2（未扩）
A5  archi_relation 与 archi_drift 对同一 (from, to) 给出同一 usedNotDeclared / declaredNotUsed
A6  报告 version 仍为 3，字段集逐字不变
A7  点边取数失败：图与元素面板零影响（只落行内错误）
```

---

## 11. 不做清单（附触发条件，防范围蔓延）

| 不做 | 理由 | 何时再做 |
|---|---|---|
| ⊆ 导出集机械证成（护栏②） | 导出集装载成本 + 与中继元素 `entry` 冲突（`api = none` 被 `load_contracts` 跳过 ⇒ 12 条前端关系全假阳） | 出现"错字被当观察项忽略"的真实事故 |
| 内联限定路径取数（G2） | 尾段含模块名噪声，需过滤面 | 后端 9 条"只有内联路径"的边（如 `VcsRuntime → Exec`）被认为必须可写 `apis` |
| 标识符集旁证（§5.2 升级项） | 引入"文本出现"这一弱事实族 | `declared ∖ used` 观察项长期无人看 |
| 生成链对账（bindings ↔ 命令名） | 独立能力，不影响本需求判据完整性 | 出现"后端加命令忘重导"事故 |
| 边失配徽标入词汇（第五边标记） | `EdgeMark` 封闭四值 + 三处随动 | 需要画布上一眼扫出失配 |
| 模型结构 diff 覆盖 `apis` | 关系身份仍是 `(from, to)` | 归档确认面要呈现"依赖面变更" |
| `apis` 在生成物面填写 | 生成物是唯一真相，手写必漂 | 永不（生成物面留空） |

---

## 12. 附录

### 12.1 被否决的备选设计（记录理由，供未来复议）

| 备选 | 内容 | 否决理由 |
|---|---|---|
| 契约 fence 第四类（`api-deps`） | 依赖清单写进契约 md 的 fence `{ "<to FQN>": [names] }` | 一个字不碰模型红线，但覆盖率受"from 侧有契约"限制（今天 ≈ 12 条边，`api = none` 的 from 写不了）；关系身份在模型与契约两处各存一份 ⇒ 要新判据；契约解析 / 校验 / 模板 / 语料整链要动 |
| `relation.via` 中继边 | `via = "…/bindings.ts"` 表达"经此文件" | ① import 事实仍要落到元素才进判据 ⇒ 需在归属层开第二条路径（新机制）；② 一条边跨两种语言 ⇒ 判定侧常驻映射函数（第二处映射真相）；③ `via` 单值而生成目录有 3 个文件，形态争议 |
| 报告 facts 段 | `ArchiDriftReport.relationApis[]` | `version` 3→4 + 16 份金样机械重写 + 绑定 + 前端载荷语料全动（§6.2） |
| 只出结论（`occurrences.names`） | 只给失配结论，不暴露 `used` | 全匹配的边点开是空的，体验割裂 |
| 出口只许契约名 | `apis` 只引用 `api-op` / `$defs` 的名字 | 全仓可用名字 11 个、可写目标 2 个元素（§4.1） |

### 12.2 必须同批写进 spec 的三句说明（否则会被后来者当 bug）

```
S1  「关系 apis 是依赖侧引用清单、不是协议正文」+ 两条护栏（§4.2）
S2  「declared ∖ used 的两种成因不可机械分辨」——显式豁免「一个值一种成因」的纪律（§5.2）
S3  「边点击的按需查询由显式用户动作发起，不计入画布三触发」+ 仍禁 watch / 轮询（§6.3）
```

### 12.3 取证方法（可复现）

三支只读探针，全部离线跑、不上库：

```
① 后端名字面：读各 crate Cargo.toml 取包名 → 扫描 .rs 的 use 树（展开 Group / Path / Rename / Glob）
   → 按 crate 名映射到元素 → 与模型 relation 对拍（命中 40/49）
② 前端名字面：按元素 path 建最长前缀归属表 → 解析 import 子句（具名 / 默认 / 星号）
   → 与模型 relation 对拍（命中 50/73）
③ 内联限定路径面：匹配 `crate::seg::Name` 三形态，剔 std / 外部包（命中 38/49，与 ① 互补）
```

### 12.4 关键行号索引（实现时的定位起点）

| 对象 | 位置 |
|---|---|
| `RawRelation`（三键） | `crates/core/archi/src/parse/toml.rs:58` |
| `Relation`（AST） | `crates/core/archi/src/model.rs:72` |
| `ArchiRelation`（DTO） | `crates/core/archi/src/dto.rs:436` |
| `ImportFact` / `DependencyEdge` | `crates/core/archi/src/scan/mod.rs:42` / `:56` |
| 目标无归属即丢弃 | `crates/core/archi/src/scan/mod.rs:276` |
| `flatten_use_tree`（别名 / glob 语义） | `crates/core/archi/src/scan/rust.rs:584` |
| `ArchiDriftKind::ALL`（4 值） | `crates/core/archi/src/drift/report.rs:93` |
| `UnusedDeclaration`（保留未产） | `crates/core/archi/src/drift/report.rs:131` |
| `load_contracts`（`api = none` 跳过） | `crates/core/archi/src/api/mod.rs:443` |
| `toEdges` / `onNodeClick` | `views/architecture/components/architecture-canvas.tsx:210` / `:320` |
| 关系三键的知识文本句 | `crates/infra/agent/src/archi-knowledge.md:5` |
