# desktop-archi-drift Specification (Delta)

## ADDED Requirements

### Requirement: 依赖 API 两桶对账与求差单点

判定域 SHALL 提供一份**求差纯函数**，供两处消费面共用（漂移对账与 IPC 命令面 `archi_relation`），MUST NOT 存在第二份求差实现：

```text
api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)
  → (used ∖ declared, declared ∖ used)        两侧各自去重 + 字典序
```

- **输入口径**：`declared` = 某条 `[[relation]]` 的模型 `apis`（空 = 未声明）；`used` = 该元素对的扫描事实名（`DependencyEdge.names` 按元素对聚合、去重后的并集）。比对 SHALL **逐字符**（MUST NOT 大小写折叠、MUST NOT 驼峰归一——否则判定侧常驻第二处映射规则）。
- **两侧对称去重**：同一名字在多文件命中只算一项；输出 SHALL 为稳定字典序，使两消费面对同一元素对输出逐字相同、报告可逐字 diff。
- **`used` 是 under-approx（已知上限三条，显式登记）**：① Rust 方法调用不可见（`use ... CancelFlag` + `flag.is_set()` 只产 `CancelFlag`）；② 内联限定路径（`exec::SystemCommand::bare(..)` 未写 `use`）不产名；③ **观察根无 workspace 清单时 Rust 事实为零**（本仓现状：清单在 `packages/desktop/src-tauri/Cargo.toml`，crate 发现只读 `root/Cargo.toml` ⇒ 49 条后端关系零事实）。三条都只影响「看得见多少」，MUST NOT 被读成「依赖不存在」。
- **`declared ∖ used` 的两种成因不可机械分辨（显式豁免「一个值一种成因」）**：(a) 真没用；(b) 写错或看不见。本能力在既有纪律「同一个值 MUST NOT 承载两种缺口」上**显式豁免这一处**——理由是取数面是 under-approx、下限不得靠猜（把 (b) 判红就是假阳）。补偿三条：① 该桶**只落 `observations[]`**，永不进 `status` / `passed`；② 条目 `message` SHALL **并列**两种可能（「可能已不再依赖，也可能名字写错或该依赖以方法调用 / 内联限定路径 / glob 形态存在而扫描看不见」），MUST NOT 只写「没用」；③ 呈现面（`desktop-archi-canvas` 的关系面板）同款并列。
- **`used ∖ declared` 才是有证据的一侧**：import 是硬事实——模型写了 `apis` 却用了清单外的名字，即可按引入方档位报出（语义与定级见「三类漂移与定级只看引入方」）。
- **零噪声边界**：关系省略 `apis`（或空数组不被接受）⇒ 两桶皆空、报告零新增条目；`apis` 非空但该元素对零事实 ⇒ 只有 `declared ∖ used` 一桶（另一侧本就为空）。
- **不缓存、不落盘、不进报告字段**：本函数是纯计算；`used` 的取样成本由消费面自负（命令面按需、drift 面复用既有一次扫描），MUST NOT 引入跨调用缓存。

#### Scenario: 求差逐例

- **WHEN** 依次以 `(declared, used)` = `([] , [])` / `(["A","B"], ["A","B"])` / `(["A","B"], ["A"])` / `(["A"], ["A","B"])` / `(["B","A","A"], ["B","A"])` 调用
- **THEN** 依次得 `([] , [])` / `([] , [])` / `([] , ["B"])` / `(["B"], [])` / `([] , [])`；输出两侧恒字典序、重复项恒合并（模型侧重复本应由 `apis` 值面拒收，此例只证函数自身稳健）

#### Scenario: 两消费面同源

- **WHEN** 对同一 `(root, from, to)` 分别经 `drift::check` 的对账与 `archi_relation` 命令取两桶
- **THEN** 两侧的 `used ∖ declared` / `declared ∖ used` 逐名相同；源码审查显示两处都只调 `api_buckets`，无第二份 `contains` / 集合差实现

#### Scenario: 未填 apis 零噪声

- **WHEN** 模型所有关系的 `apis` 均省略，跑一次全量检查
- **THEN** `errors` / `warnings` / `observations` 与不加本能力时逐字段相同（零 `undeclaredApi`、零 `unusedDeclaration`）

#### Scenario: 事实面为零时只留观察项且文案并列两种成因

- **WHEN** 某后端关系写了 `apis = ["SystemCommand"]`，而观察根无 workspace 清单（Rust 事实面为零）
- **THEN** 报告产出恰一条 `observations[].kind = unusedDeclaration`（`errors` / `warnings` 零新增、`status` / `passed` 不变）；其 `message` 同时含「可能已不再依赖」与「名字写错或扫描看不见」两种可能，MUST NOT 只写「没用」

## MODIFIED Requirements

### Requirement: 三类漂移与定级只看引入方

检查 SHALL 判定三类漂移，并 SHALL 把「模型自身不自洽的结构错」作为独立门禁判据：

| # | 漂移 | 定义 | 产出位置 |
|---|---|---|---|
| ① | 声明未兑现 | 模型声明 `A -> B`，代码中无任何 `A -> B` 的 import | `observations[]`（永不进 `status` / `passed`） |
| ② | 未声明依赖 | 代码存在跨元素 import，模型未声明该关系 | `errors[]` / `warnings[]`（按引入方 `gate` 档） |
| ④ | path 失效 | 元素 `path` 指向不存在的目录 / 文件 | `model.warnings[]` + `warnings[]` / `errors[]`（按该元素 `gate` 档） |

① 与 ② 各有一个**名字级对偶**（本变更新增，粒度更细、前置条件是关系已声明且写了 `apis`）：

| # | 名字级对偶 | 定义 | 产出位置 |
|---|---|---|---|
| ②ᵃ | 用了没声明（`undeclaredApi`） | 关系写了 `apis`，而代码对该元素对用了 `apis` 之外的名字 | `errors[]` / `warnings[]`（按引入方 `gate` 档，与 ② 完全同档规则） |
| ①ᵃ | 声明没用（`unusedDeclaration`） | 关系写的名字在扫描事实里没出现 | `observations[]`（永不进 `status` / `passed`；两桶口径与成因豁免见「依赖 API 两桶对账与求差单点」） |

- **② 的定级只看引入方**：`A` 引入 `B` 且模型无 `A -> B` 时级别 = `A` 的 `gate` 档（`strict` → `errors`、`warn` → `warnings`、`off` → 不出现）。`B` 的档位 MUST NOT 影响本条——否则一个 `strict` 元素会把全仓库 `off` 元素拽进门禁。
- **②ᵃ 复用 ② 的档位规则，不新增档位轴**：同一元素对若关系缺失，报 ②（元素对级）；若关系在且写了 `apis`，关系内的名字差额报 ②ᵃ（符号级）。两者 MUST NOT 互相替代——② 判的是「依赖没声明」，②ᵃ 判的是「声明的清单不完整」；`undeclaredApi` MUST NOT 在关系缺失时产出（那时没有 `apis` 可比）。
- **关系省略 `apis` 即退出 ①ᵃ / ②ᵃ**：两桶皆空（`apis` 的缺省语义是「不声明 API 粒度」，不是「零依赖」）。
- **②ᵃ 是唯一进 errors 的新增路径，且带证据**：import 是硬事实，故 ②ᵃ 可按 `gate` 定级；①ᵃ 无证据（≠ 反证），故恒为观察项。这条不对称是取数精度的直接后果，MUST NOT 被「对称美观」抹平。
- **层级逆向并入 ②**：kind 层级是**包含**层级不是约束层级，「允许的依赖方向」无权威来源；「声明了就合法」是唯一自洽规则。`A -> anc(A)` 一类图结构自相矛盾由结构校验判（`desktop-archi-model`），MUST NOT 在此另立方向规则。
- **type-only 算架构依赖**：`import type` 与普通 import 同权（只记 `typeOnly: true`）；MUST NOT 因类型导入而豁免；`apis` 的取数同理（`type` 前缀剥除，不因 type-only 少产名字）。
- **目标未建模**：跨元素 import 的 `to` 元素名不在模型内时 SHALL 记 warning（归因不明），MUST NOT 记 error。②ᵃ 不适用（它的前置就是关系在模型内）。
- **结构错**：模型 `structuralErrors` 非空时检查结论 SHALL 为不通过（见「判定单源与报告结构」的 `passed` 定义），三类策略分工不变——读取照常（返回结构化错误、绝不 panic）、渲染停摆（画布）、门禁判 fail。

#### Scenario: 三档定级

- **WHEN** 同一 `A 引入 B` 的未声明依赖分别在 `A.gate = strict` / `warn` / `off` 三个模型下检查
- **THEN** 分别产出 1 条 `errors[]` / 1 条 `warnings[]` / 零记录（off 下该文件计入 `unmatchedFiles[]`）

#### Scenario: 按元素对聚合不按行铺开

- **WHEN** 同一 `(App.Billing, App.Auth)` 元素对在 3 个文件各命中 1 处
- **THEN** `errors[]` 恰 1 条，其 `occurrences[]` 含 3 项（各带 `file` / `line` / `specifier` / `typeOnly`），MUST NOT 铺开成 3 条

#### Scenario: 声明未兑现只作观察项

- **WHEN** 模型声明 `App.Billing -> App.Auth`，代码中零 `A → B` import
- **THEN** 报告 `observations[]` 含一条 `unfulfilledDeclaration`，`status` 不因它变化、`passed` 不因它变假

#### Scenario: 名字级对偶的三档定级

- **WHEN** 模型某关系写 `apis = ["A"]`，代码对该元素对用了 `A` 与 `B`，分别在引入方 `gate = strict` / `warn` / `off` 三个模型下检查
- **THEN** 分别产出 1 条 `errors[]` / 1 条 `warnings[]` / 零记录（`kind = undeclaredApi`）；条目按元素对聚合恰一条，`message` 点名 `B`（多出的名字）与引入方档位，`occurrences` 复用既有形状（MUST NOT 新增报告字段）

#### Scenario: 声明多余只作观察项

- **WHEN** 模型某关系写 `apis = ["A", "B"]`，代码只用了 `A`
- **THEN** 报告 `observations[]` 恰一条 `unusedDeclaration`（点名 `B`），`errors` / `warnings` 零新增，`status` 与 `passed` 不变

#### Scenario: 关系缺失时不出 undeclaredApi

- **WHEN** 代码存在 `A → B` 的 import 而模型完全无该关系（因此也无 `apis`）
- **THEN** 只产 ①/② 元素对级结论（此处为一条 `undeclaredDependency`），MUST NOT 产出 `undeclaredApi`

#### Scenario: 同一元素对的两条观察并存

- **WHEN** 模型关系写 `apis = ["A"]` 而代码对该元素对零 import
- **THEN** `observations[]` 同时含一条 `unfulfilledDeclaration`（元素对无任何 import）与一条 `unusedDeclaration`（`A` 未出现）；两条各有自己的 `message`，MUST NOT 合并成一条，MUST NOT 因并存而把任一条抬进 `status`

#### Scenario: path 失效按元素档位定级

- **WHEN** `App.Billing`（`gate = strict`）的 `path` 指向不存在的目录，另一元素 `App.Legacy`（`gate = off`）同样失效
- **THEN** 前者产 `errors[]`（或与其它错误并列）、后者只在 `model.warnings[]` 与 `warnings[]` 留痕；两者都记 `pathNotFound`

#### Scenario: 结构错即不通过

- **WHEN** 模型含一条 `dependsOnAncestor` 结构错，代码侧零违例
- **THEN** 报告 `model.structuralErrors[]` 记录该错，检查结论为不通过（`passed = false`），且检查过程零 panic

### Requirement: 报告结构与 status 五值

检查 SHALL 产出单一报告结构（两模式共用同一形状；落盘与否由消费面决定——命令面全量仅进程内返回、门禁面全量 / 增量均落 `reports/archi/<timestamp>.json`），字段面 SHALL 覆盖：

```jsonc
{
  "version": 3,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",              // incremental | full
  "root": "/abs/path",
  "change": "add-billing-gate",       // 无 change 上下文（命令面）时为 null；门禁面两模式均携 change 名
  "base": "HEAD",                     // full 模式为 null（增量恒 HEAD 差，见「全量与增量两模式」）
  "modelHash": "sha256:…",            // graph/model/*.toml 聚合哈希

  "model": {
    "elements": 24, "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [ … ],        // 读存量模型遇错也照录
    "warnings": [ … ]                 // pathNotFound / noPath / ambiguousOwner
  },
  "scan": { "mode": …, "changedFiles": 17, "scannedFiles": 12,
            "excluded": [ … ], "unmatchedFiles": [ … ], "matched": [ … ],
            "unresolved": [ … ] },
  "errors":   [ { "kind": "undeclaredDependency", "from": …, "to": …, "gate": …, "occurrences": [ … ] } ],
  "warnings": [ … ],
  "suppressed": [ { "from": …, "to": …, "via": "config|inline", "reason": …, "file": …, "line": … } ],
  "observations": [ { "kind": "unfulfilledDeclaration", "from": …, "to": …, "message": … } ],
  "apiFindings": [ { "kind": "docMissing", "category": "doc", "element": …, "target": null,
                     "message": …, "occurrences": [ … ] } ],   // 契约判据（并列槽位；纳管子集按档位抬 status，error 档进 passed）
  "status": "errors"                  // clean | errors | warnings | noChanges | skipped
}
```

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings 且纳管的契约判据为空）、`errors`（`errors` 非空**或纳管子集含任一 `error` 档判据**）、`warnings`（零 errors，且 `warnings` 非空**或**纳管子集非空且其中无 `error` 档）、`noChanges`（无文件可判）、`skipped`（无模型）。**`status` 推导与模式无关**：全量模式在零可判文件时同样落 `noChanges`、无模型时同样落 `skipped`。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`observations` 的两种成因不对称（本变更显式豁免）**：`unusedDeclaration` 的两种成因（真没用 / 写错或看不见）不可机械分辨，故该值只作观察项、`message` SHALL 并列两种可能；这是「同一个值 MUST NOT 承载两种缺口」纪律的**唯一豁免点**，理由与补偿见「依赖 API 两桶对账与求差单点」——MUST NOT 在别处援引本条作为放宽口径的先例。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`graph/model/*.toml` 聚合哈希（文件名 + 字节累加，不含父目录），供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。模型字节变化即哈希变化（本变更新增 `06-frontend.toml` 的一个元素表头、改 12 条命令面关系的 `to` / `desc`、改 `06` 表头注释字节 ⇒ 本仓 `modelHash` 必然变；元素计数 54 → 55、关系计数 ±0 不变——MUST NOT 写成「模型零变化」或「计数全不变」）。
- **`scan` 三面按条目级账目验收（本变更留痕，防同一误判复发）**：`scan.scannedFiles` / `scan.unmatchedFiles[]` / `scan.excluded[]` 是**事实面的直接投影**，计数随仓内源文件与新增测试 / 语料变化——`gate = off` 元素覆盖的源文件与无主文件同落 `unmatchedFiles[]`（故某个文件由「无主」改为「`off` 元素覆盖」时条目不变，只是成因变了），命中排除规则（默认 13 条 + 配置追加项）的新增文件必进 `excluded[]`（`rule` 记命中的具体形态）。因此「报告只因模型 / 判定面而变」SHALL 按**条目级账目**验收：既有 `excluded[]` 条目（`file` + `rule`）与 `unmatchedFiles[]` 路径零增删改（数组排序带来的位序平移不计），新增条目逐条可指到本变更新增文件与既有规则；MUST NOT 用三面计数相等充当「归属 / 判定面零变化」的证据（`gate` 全 `off` 的仓 `matched[]` 恒空，计数相等根本不含归属信息）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（**五值**：`undeclaredDependency` / `undeclaredApi` / `pathNotFound` / `ambiguousOwner` / `noPath`；`ALL` 由四值扩为五值）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`，前者由 ① 产出、后者由 ①ᵃ 产出——两个值此前就有，本变更只点亮后者的产出路径、`ALL` 长度不变）、**契约判据 `kind`**（`ArchiApiFindingKind` **十三值**，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）与 `EdgeMark` / `NodeMark`（画布标记）语义面各不相同——混用会让「结构错」「漂移违例」两类在类型上不可辨。**`WarningKind`（旧文档警告，单值 `docNotFound`）已随 `docs` 三源退役删除**：入口文档面的读数改由契约判据的 `docMissing` / `docNotFound` / `apiDocMissing` 承载（`category = doc`），MUST NOT 在报告里恢复「文档警告」这一类别。
- **`version`**：报告结构版本号 SHALL 走显式兼容说明（`version = 1` 无 `apiFindings` 字段；`version = 2` 有该字段但 `status` 推导不读契约判据；`version = 3` 纳管口径进 `status`）。**本变更不递增**：字段面无增删，变的只是判据取值、档位与纳管口径——`version` 仍为 **3**，MUST NOT 因语义收紧而无字段变化地递增（否则消费方按版本分支会读到「形状未变的伪新版本」）。**依赖 API 面同样不递增**：`undeclaredApi` 是既有漂移词汇组的取值扩充、`unusedDeclaration` 是既有空位的产出点亮，`errors[]` / `observations[]` 的**条目形状逐字不变**（名字写进 `message`，不新增字段）——故 `version` 仍为 3。
- **`apiFindings` 为并列槽位**：契约判据结果 SHALL 落本字段（空数组 = 无契约问题），MUST NOT 与 `errors` / `warnings` / `observations` 混装——三者事实源与定级语义各不相同（模型关系图 / 可解析性与归属 / 设计态陈述 vs 契约文件）。对 `status` 的影响 SHALL 限于**纳管子集**（口径、判据与单点实现归 `desktop-archi-api-check`「报告并列槽位 apiFindings」）——本变更把该子集的口径由「契约在案元素」改写为「**入口文档承诺已成立**」（`api = declared` 且派生契约 md 在盘 / `api = none` 且 `doc` 已声明），并允许子集内 `error` 档条目把结果抬到 `errors`。`docMissing` 永不纳管 ⇒ 存量入口文档债 MUST NOT 把状态词常规化为永久 `warnings`。条目形状 `{ kind, category, element, target, message, occurrences }`：`occurrences` 复用既有聚合形状，`category` 为开放值集字符串（无 `ALL`，渲染层据此分组）。
- **`passed` 与 `apiFindings` 的关系（本变更改写）**：门禁布尔 `passed` 的判据 SHALL 为 `errors` 空 ∧ `model.structuralErrors` 空 ∧ **`error` 档 `apiFindings` 为空**——原纪律「`apiFindings` MUST NOT 影响 `passed`」**作废**，MUST NOT 在任何 spec / 代码注释里残留（它只在「判据级档位最高到 `warn`」的前提下成立）。`unusedDeclaration` MUST NOT 进 `passed`（观察项）。
- **推导序不变**：`skipped` → `errors`（结构错 / `errors[]` 非空 / 纳管子集含 `error` 档）→ `noChanges`（可判文件为 0）→ `warnings`（`warnings[]` 非空 或 纳管子集非空且无 `error` 档）→ `clean`；按此序 `status == errors ⟺ passed == false` SHALL 恒成立（`error` 档判据的触发条件都蕴含纳管成立，故不存在「打断 `passed` 却不进 `status`」的旁路）。

#### Scenario: 五值可辨

- **WHEN** 分别以「零模型」「无变更文件」「零违例」「仅 warn 档违例」「存在 strict 档违例」构造五次检查
- **THEN** `status` 依次为 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`skipped` 与 `noChanges` 的报告 `errors` 均为空且不带任何「通过」语义字段

#### Scenario: 观察项不影响结论

- **WHEN** 报告含 3 条 `observations` 与零 `errors`
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空或纳管子集非空）、`passed = true`；`observations` 不进门禁判据

#### Scenario: 报告词汇与 DTO 词汇互不混用

- **WHEN** 审查报告 `kind` 词汇与 `desktop-archi-model` 的 `DiagnosticKind` / `EdgeMark` / `NodeMark`
- **THEN** 各词汇组各自单点定义、互不复用；报告结构模块为 `status`、漂移条目、观察项与**契约判据**各持一份 `ALL` 常量（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`），语料对拍覆盖全部取值；`WarningKind` 不在任何一组内（已删除）

#### Scenario: modelHash 随模型变化

- **WHEN** 修改任一 `graph/model/*.toml` 字节后重跑（本变更新增 `06` 的一个元素表头、改 12 条关系的 `to` / `desc` 与 `06` 表头注释即属此列）
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同

#### Scenario: apiFindings 并列而不混装

- **WHEN** 报告同时含 1 条 `warnings` 与 36 条 `apiFindings`（全部 `docMissing`，且这些元素均未写入口文档）
- **THEN** `warnings` 仍只 1 条、`status = warnings`（成因是 `warnings` 非空而非契约判据）、`passed` 不受这 36 条影响；两类条目不互相出现

#### Scenario: 入口文档在案的判据按档位抬 status

- **WHEN** 报告 `errors` 为空、`warnings` 为空，`apiFindings` 含一条属于「契约 md 在案」元素的 `apiSignatureMismatch`（`error` 档）
- **THEN** `status = errors`、`passed = false`；而同元素只含一条 `apiDocMissing`（`warn` 档）时 `status = warnings`、`passed = true`；把该元素的契约 md 移走后两者都退化为 `docMissing` ⇒ `status = clean`、`passed = true`

#### Scenario: 存量入口文档债不进状态词

- **WHEN** 报告含 36 条 `docMissing`（本仓当前读数：33 条 `declared` 契约未写 + 3 条 `api = none` 未声明 `doc`）、零 `errors`、零 `warnings`
- **THEN** 纳管子集为空 ⇒ `status = clean`、`passed = true`；MUST NOT 因这 36 条把状态词抬成 `warnings`

#### Scenario: 版本未递增而口径已变

- **WHEN** 分别读取 `version = 3` 的新报告与 `version = 3` 的历史报告
- **THEN** 两者字段面相同；差异只在 `apiFindings` 的取值 / 档位与 `status` 推导结论（历史报告由旧口径产生，不回写、不重算）；消费方 MUST NOT 期望靠 `version` 区分两种口径——本次口径变化由 `desktop-archi-api-check` 的 spec 与 change 留痕承载

#### Scenario: 依赖 API 面不新增报告字段

- **WHEN** 审查含 `undeclaredApi` 与 `unusedDeclaration` 的报告 JSON
- **THEN** `errors[]` / `observations[]` 的条目键集与不含二者时逐字相同（`kind` / `from` / `to` / `gate` / `message` / `occurrences`）；多出的名字只出现在 `message` 内；`version` 仍为 3

#### Scenario: 全量报告形状与增量同构

- **WHEN** 取一份 initialize 基线全量报告与一份 implement 增量报告
- **THEN** 两者字段集完全一致（无「全量独有 / 增量独有」字段），差异只在字段值：`mode`（`full` / `incremental`）、`base`（`null` / `HEAD`）、`scan.changedFiles`（`0` / 变更数）与扫描清单

#### Scenario: 无模型与零可判文件两态可辨

- **WHEN** 分别以「模型目录缺席」与「模型在但零源文件通过排除约定」的观察根执行全量检查
- **THEN** 前者 `status = skipped`、后者 `status = noChanges`；两者 `errors` 皆空但 MUST NOT 呈 `clean`，消费方（门禁 / UI）按各自文案分列「未检查（无模型）」与「未检查（无可判文件）」

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/scan/mod.rs` | 扫描事实面（名字通道） | `ImportFact` / `DependencyEdge` 各增 `names: Vec<String>`（去重 + 字典序单点；空 = 取不到名字）；既有字段与归属 / 排除 / 定级语义逐字不变；`gate = off` 分支零改动 |
| `crates/core/archi/src/scan/rust.rs` | Rust 取数 | `use` 树同时产出「路径」（供前缀匹配）与「尾段」（供名字）；`Rename` 取被引入名、`Glob` / `self` 不产名；`resolve` 签名与语义零改动（名字不走前缀匹配） |
| `crates/core/archi/src/scan/ts.rs` | TS / JS 取数 | `ImportDeclaration` 的具名 specifier 与 `ExportFromDeclaration` 的具名清单产名（`type` 前缀剥除）；default / namespace / `export *` 不产名；specifier 解析与 `unresolved` 面零改动 |
| `crates/core/archi/src/drift/mod.rs` | 两桶对账 + 求差单点 | `pub fn api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)`；对账后按关系 `apis` 非空产出两桶（`used ∖ declared` 按 `from.gate` 落 `errors` / `warnings`，`declared ∖ used` 落 `observations`），`message` 并列两种成因；既有三类漂移与豁免 / `status` 推导零改动 |
| `crates/core/archi/src/drift/report.rs` | 报告结构与词汇 | `ArchiDriftKind` 增 `UndeclaredApi`（`ALL: [Self; 5]` + `as_str` 线格式 `undeclaredApi`）；`ArchiObservationKind::UnusedDeclaration` 注释由「本期不产出」改为「`declared ∖ used` 时产出」（`ALL` 仍二值）；`ArchiDriftReport` 字段集与 `REPORT_VERSION = 3` 逐字不变 |
| `crates/core/archi/src/drift/report_test.rs` | 报告面测试 | `ArchiDriftKind::ALL.len()` 4 → 5、`as_str` 逐值对拍随动；新增「`unusedDeclaration` 不进 `status` / `passed`」与「条目键集不变」用例 |
| `crates/core/archi/tests/corpus/drift/**` | 报告面语料 | 16 份既有金样零变化；**新增 case**（`api-declared-exact` 双桶皆空 / `api-used-not-declared` 缺一项 / `api-declared-not-used` 多一项 / `api-absent` 未填 `apis` 零噪声），各自登记进 `DRIFT_CASES`；golden 只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
| `crates/core/archi/src/relation.rs`（新，消费面） | 求差第二消费面 | `relation_detail` 调 `api_buckets`（MUST NOT 自写集合差）；本能力只出「求差口径与不缓存」这两条约束，DTO / 命令形状归 `desktop-archi-model` |

### Function: `crates/core/archi/src/drift/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `api_buckets` | 新增 | 纯函数求差，两侧去重 + 字典序；`used ∖ declared` / `declared ∖ used` |
| `check` | 改 | 对账后追加两桶装配（仅对 `apis` 非空的关系）；其余判定 / 豁免 / `status` 推导逐字不变 |
| `derive_status` / `Report` / `passed` | 零改动 | 结论面不吃 `unusedDeclaration`；`undeclaredApi` 只经 `errors` / `warnings` 既有通道 |

### API: 词汇与报告

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiDriftKind` | 4 → 5 值 | 新增 `UndeclaredApi`（线格式 `undeclaredApi`）；`ALL` 长度断言随动 |
| `ArchiObservationKind` | 零扩张（二值） | `UnusedDeclaration` 由空位转产出；值集与 `ALL` 不变 |
| `REPORT_VERSION` | 保持 3 | 字段面零增删（名字进 `message`），不足以为版本递增提供理由 |
