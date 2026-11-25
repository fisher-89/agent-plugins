# desktop-archi-api-check Specification

## Purpose

定义契约判据面：`ArchiApiFindingKind` 十值封闭词汇与开放 `category`（七值含 `doc`，`doc` 只剩 `docMissing`）、按 `api` 状态分两支的十判据首错求值顺序、判据级三值档位与元素 `gate` 解耦（`error` 恰两项）、报告并列槽位 `apiFindings`（`api = declared` ∧ schema 在盘元素的判据进 `status`，其中 `error` 档进 `passed`；`docMissing` 永不纳管），以及判据的验证语料与报告数基线（`0 / 38`，分母 49）。

## Requirements

### Requirement: 判据级档位与元素 gate 解耦

十条判据的档位 SHALL 由判据自身承载（`ArchiApiFindingKind::severity()`，**判据级档位**，**封闭三值** `off` / `warn` / `error`），MUST NOT 读元素 `gate`：

| 类别 | 判据 | 档位 |
|---|---|---|
| **文档未就位**（warn，不纳管） | `docMissing` | `warn` |
| **有约定无兑现 / 实现漂移** | `apiMissing` / `apiSignatureMismatch` | **`error`** |
| **判不了 / 两事实矛盾 / 结构债** | `apiEntryMissing` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiConstructorDeclared` | `warn` |
| **完备性判据**（代码有的接口都得有） | `apiUndeclared` | **`off`** |

- **分界线写死**：一致性判据报的是「债」（可枚举、可消除，可即开 `warn` 验证检查器是否工作）；完备性判据报的是「海」（迁移期必然爆炸，必须后置）。`apiUndeclared` 是唯一后者。
- **`error` 恰两项**（本变更由三项收窄）：`docNotFound` 随 `doc` 字段惰性化退役，`apiMissing` / `apiSignatureMismatch` 两项的档位逐字不变。**`error` 恰两项是本条的新基线**，spec 语料与金样 MUST 同批改写，MUST NOT 留一句「error 恰三项」的断言。
- **结构性安全网（不靠白名单）**：`error` 档只可能落在「接口定义已就位」的元素上（`apiMissing` / `apiSignatureMismatch` 都需要 schema 在盘且过装载校验）⇒ 存量项目没写接口就永远不会红。
- **唯一的刻意缺口**：`docMissing` 是 `warn` 且占首错名额 ⇒ 它是 `apiMissing` 的**可达性上界**——派生 md 未写的元素到不了 `error` 面；**「接口未写」（schema 不在盘）同样到不了** `error` 面（⑤⑥ 无声明集、零判据）。补偿是 `docMissing` 仍在报告里逐条可枚举（迁移 backlog），但不纳管 `status`。
- **`apiEntryMissing` MUST NOT 收紧**：它报的是「**判不了**」而不是「**判错了**」——有对象、结论为否才该红。
- **与元素 `gate` 解耦的理由**：`ArchiDriftKind` 的定级看引入方元素的 `gate`；契约判据若也接 `gate`，收紧契约就要改模型，两套语料同时动、收紧节奏无法独立。故元素 `gate` 全 `off` 与本能力判据的档位表 SHALL 同时成立——判据级档位是契约面的独立收紧轴。
- **收紧路径**：收紧由 `severity()` 单值改动驱动（一类判据一个 change 收紧），MUST NOT 引入运行时开关、MUST NOT 引入配置键。
- **`warn` 报告即迁移 backlog**：`docMissing` 的 38 条仍是迁移待办，每消一条减一，可单调验证。
- **MUST NOT 为消债而重构**：本能力 MUST NOT 要求建入口文件、改元素根路径去扩展名、挪子→父类型依赖或上移壳元素根路径，也 MUST NOT 为消 `docMissing` 而批量新建契约 md——存量债务只作检查器的验证语料；补文档的节奏归数据面变更。唯一例外是**为让契约有效而暴露已存在的私有逻辑**（改可见性 + 挪进 `impl`，不动实现逻辑）——判据不同：前者是「格式追代码」，后者是「代码追上契约应有内容」。入口面同理：`entry` 字段与解析链只让「本来存在的入口」可被表达，本能力 MUST NOT 在判据面要求补入口。

#### Scenario: 元素 gate 全 off 而判据仍报

- **WHEN** 模型内全部元素 `gate = off`（本仓现状）且合成语料中存在若干无可用入口元素
- **THEN** 每个无可用入口元素各产一条 `apiEntryMissing`（判据级 `warn`），与元素 `gate` 无关；把某元素 `gate` 改成 `strict` 后该数量与内容零变化

#### Scenario: 完备性判据保持 off

- **WHEN** 代码有 `external` 导出而契约未声明
- **THEN** `apiUndeclared` 档位为 `off`，报告不产出该条目（`apiFindings` 内零 `apiUndeclared`）；把档位改为 `warn` 属独立裁决

#### Scenario: 三档各自的代表值（本变更改写）

- **WHEN** 遍历 `severity()` 的取值面
- **THEN** `error` 恰为 `apiMissing` / `apiSignatureMismatch` **两项**；`off` 恰为 `apiUndeclared` 一项；其余七项 `warn`——新增变体落哪一档必须显式决定（`match` 无通配兜底、或兜底处留注释）

#### Scenario: 零运行时开关

- **WHEN** 审查判据档位的来源
- **THEN** 唯一来源是 `ArchiApiFindingKind::severity()`；无配置键、无环境变量、无命令行 flag 可改档位

### Requirement: 报告并列槽位 apiFindings

`ArchiDriftReport` SHALL 保留**并列**字段 `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`），MUST NOT 改造既有 `errors` / `warnings` / `suppressed` / `observations` / `status`：

- **既有字段零改动**：`ArchiDriftReport` 既有 14 字段 SHALL 逐字不变（字段名 / 类型 / 序列化形态 / 语义）；`REPORT_VERSION` **保持 3**（字段面无增删——本变更只换判据取值、档位与 `status` 推导口径，不足以为版本递增提供理由）。
- **纳管子集口径（本变更换锚）**：

  ```text
  纳管(entry) ⟺ entry.element 非空 ∧ 该元素 api = declared ∧ <FQN>.schema.json 在盘
  ```

  纳管子集非空 ⇒ `status` 至少为 `warnings`，其中任一 `Error` 档条目 ⇒ `status = errors`；`element = None` 的条目落选。
  - **文档在盘与否退出纳管口径**：`docMissing` 恒 `warn` 且两侧都不成立 ⇒ **永不纳管**（否则 38 条读数会把状态词常规化为永久 `warnings`）；`api = none` 元素因其 schema 在盘即违约（`apiUnexpected`、`warn`）⇒ 也不纳管——留白违约是判据面的债，不是「承诺未兑现」。
  - **口径理由（承诺即纳管，锚点换面）**：旧锚是「入口文档承诺已成立」（`declared` 看 md 在盘 / `none` 看 `doc` 已声明）；真相搬移后，可被兑现的承诺只剩**接口定义**（schema 在盘），文档是产品面内容、不是承诺面。
  - **判据单点**：纳管子集筛选 SHALL 在 `api::gating_findings` 内完成（schema 在盘的一次性判定），`drift` 侧 MUST NOT 自行判文件存在性、MUST NOT 在前端或命令层二次筛选。
- **`passed` 判据（本变更改写，唯一跨进门禁面处）**：

  ```text
  passed = errors 空 ∧ model.structuralErrors 空 ∧ error 档 apiFindings 空
  ```

  `error` 档判据（`apiMissing` / `apiSignatureMismatch`）从此能阻断归档。这是既有 spec 自己规定的同批动作（「把某类判据改 `error` 档与改 `passed` 判据 SHALL 同批进行」）；其反向纪律「`apiFindings` MUST NOT 影响 `passed`」**随之作废**，MUST NOT 在任何 spec / 注释里残留。
  - **不变量**：`status == errors ⟺ passed == false` SHALL 恒成立——`error` 档判据的触发条件都蕴含纳管成立（见上），故不存在「打断了 `passed` 却不进 `status`」的旁路。
- **`derive_status` 分档**：`warnings` 分支条件扩为「`warnings[]` 非空 **或** 纳管子集非空（且其中无 `Error`）」，新增 `errors` 分支「纳管子集含任一 `Error`」；`skipped` / `errors`（结构错 / `errors[]`）/ `noChanges` 的先后序与语义逐字不变。
- **聚合与稳定序**：`apiFindings` SHALL 按 `(kind, element, target)` 稳定排序（同 `kind` 内按元素 FQN 字典序），使报告可逐字 diff；命中点复用 `occurrences` 聚合（同元素对多处命中聚在一条内，不按行铺开）。
- **词汇组表述校准**：报告自带封闭词汇仍四组（`status` / 漂移条目 `kind` / 观察项 `kind` / 契约判据 `kind`），但 `ArchiApiFindingKind` 由十三值收为**十值**；`WarningKind`（旧文档警告单值）已删除，MUST NOT 再出现在「互不混用」的对照清单里。
- **判定单源**：`api_findings` 的装配 SHALL 在 `crates/core/archi/src/drift/mod.rs` 的 `check()` 内单点完成（调 `api::findings`）；命令面（`archi_drift`）与门禁面（`LocalArchiCheck`）产出 SHALL 逐字段相等（除 `timestamp` / `change` 与落盘路径）。
- **呈现面**：`apiFindings` SHALL 经架构页左栏「契约」视图与元素信息面板「契约」段呈现（`desktop-archi-contract-panel`），两处 MUST NOT 重算判据、MUST NOT 对同一报告二次 invoke。

#### Scenario: 存量 backlog 不拉黄灯

- **WHEN** 报告含 **38** 条 `docMissing`（本仓读数：35 条 `declared` 缺派生 md + 3 条 `api = none` 缺派生 md），`errors` 为空且无结构错
- **THEN** 纳管子集为空 ⇒ `status = clean`、`passed = true`；MUST NOT 因这 38 条把状态词常规化为 `warnings`

#### Scenario: 接口在案的 error 判据进 status 与 passed

- **WHEN** 报告含 1 条 `apiMissing`（该元素 schema 在案且过装载校验）、`errors` 为空且无结构错
- **THEN** `status = errors`、门禁步 `passed = false`（不变量保持）；把该元素的 schema 移走后该条消失（⑤⑥ 无声明集 ⇒ 零判据；`docMissing` 是否成立只看派生 md）⇒ 该元素若 md 在盘则 `apiFindings` 为空、`status = clean`、`passed = true`

#### Scenario: api = none 带 schema 不进纳管

- **WHEN** 报告含 1 条 `apiUnexpected`（`api = none` 而 schema 在盘）、`errors` 为空
- **THEN** `status = clean`、`passed = true`（该元素不满足纳管口径的 `declared` 项）；但报告内该条仍逐条可枚举

#### Scenario: 纳管与 `passed` 的双向对拍

- **WHEN** 对同一份报告分别取 `status`、`passed` 与纳管子集
- **THEN** 「纳管子集含 `Error`」⟺`status = errors`⟺`passed = false` 三向同真同假；两次运行逐字相同

#### Scenario: 既有字段与金样零改动（除首错改写）

- **WHEN** 对既有漂移语料跑检查
- **THEN** `errors` / `warnings` / `suppressed` / `observations` / `model` / `scan` 与 `"version": 3` 逐字不变；十六份金样的变化只允许来自「首错改写使同元素多报降为一条」与「纳管分档」，逐份变化 SHALL 在 change 留痕中说明成因

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` / `status` / `passed` 逐字段相等（含排序、`category` 与纳管结论）

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓当前读数；分母 = `path` 非空的 **49** 个元素）**：`docMissing` **38**（35 = `api = declared` 且派生 md 不在盘；3 = `api = none` 且派生 md 不在盘——单成因，两侧按 `api` 分列只为可追溯）、`apiEntryMissing` **0**、`apiMissing` **0**、`apiSignatureMismatch` **0**、`apiUnexpected` **0**、其余五值各 **0**、`apiUndeclared` 不适用（`off`）；`status = clean`、`passed = true`（38 条全 `warn` 且 `docMissing` 不纳管）。
- **该段的三项性质必须与原话一起写**：
  1. **词汇收缩不改读数**：删掉的三值在本仓读数均为 0，故**条数与构成不变**；
  2. **38 = 49 − 11**：11 个元素的两份资产都在盘（文档 + 接口），38 个元素缺文档（其中 35 个还缺接口）；
  3. **唯一变的文本**是 38 条 `docMissing` 的 `message` 文案——判据从「入口文档 = 派生契约 md」改为「派生文档不在盘」，不再出现「入口文档」指针措辞；**条数与 `kind` 构成零变化**。
- **历史链（八段，逐段成因在案）**：`28 + 13 = 41`（入口靠探测）→ `15 + 26 = 41`（入口判据落地）→ `12 + 27 = 39`（数据面换段）→ `12 + 29 = 41`（共享组件补建元素）→ `0 + 41 = 41`（前端目录根补入口）→ `0 + 43 = 43`（形态①改名 + `api = none` 两元素入面，分母 44）→ `0 + 35 = 35`（分母 +1 的组件元素 + 10 份契约在盘）→ `0 + 36 = 36`（分母 46、`api = none` 三元素入面）→ **`0 + 38 = 38`（本变更：分母 46 → 49，契约在盘 10 → 11）**。末段 MUST NOT 被写成「文档债上升」——它是**分母 +3 与契约 +1 的净值**（三新增元素里一带有接口文档、二没有）。
- **判定口径**：报告数与基线表**逐项相等**即「检查能正确提示」。多于基线 = 假阳，少于基线 = 假阴，两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释；本段总数与分母都变过，故必须**逐 `kind` 对**，不能只看总数。
- **基线须显式留痕**：数字写入 change 报告（`reports/`）与实现期实测对照；数字变更 SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（两层，各有 owner，本变更不新增文件）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的既有 sidecar 用例承担（十值各自正反例、两支互斥与首错、pair 锚点三条、签名归一、嵌套与透传、生成物渲染两侧键集一致）；用例 MUST 落在既有 sidecar 内，**MUST NOT 新增 `api/**` 文件**。② **crate 级报告面语料**沿既有 `tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`；既有 `api-mismatch` case SHALL 保持非空 `apiFindings`；新增 case 未登记即红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认。
- **首错改写与词汇收缩会改动金样读数**：三值删除后，含这三种判据的 case 条目数下降乃**预期**；每条变化 SHALL 在 change 留痕中说明成因。代码注释里「与 ① 独立、MUST NOT 与 ② 双报」一类纪律句 MUST 同批改写。
- **MUST NOT 把本仓模型与资产塞进 crate 语料**逐字沿用；`reports/archi/` 是门禁运行时产物（不入库、不作判红面）。
- **末行归零的前提（必须说准）**：「全 0」的前提是**按现有实现写接口定义**——schema 里只声明已有的 op / 类型，未写的归 `apiUndeclared`（`off`，不产出）。若反过来「先写 schema 再补实现」，`apiMissing`（`error`）会立刻红——这正是 `error` 档的用途。
- **语料覆盖的已知缺口（就地留痕）**：判定面用例**不新增 sidecar 文件**（撞上既有计数断言），其正反例落在既有文件内；因此「十判据各自正反例」一句 MUST NOT 被读成「十判据各有独立语料文件」。兜底只剩两层——crate 级 `corpus/drift/` 金样（不经本仓模型）与本仓实测留痕。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `graph/model/` 存在且一级为六个 `NN-<域>.toml`，但没有任何 `declared` 元素的 schema 在盘时跑一次全量检查
- **THEN** `apiFindings = []`，`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `docMissing` / `apiEntryMissing`

#### Scenario: 接口在盘后全量纳入

- **WHEN** 同一根下给某 `api = declared` 元素补入合法 `<FQN>.schema.json` 后重跑
- **THEN** 契约面转为启用，全部 `path` 非空元素（49 个）参与判据，产出基线表所述 **0** 条 `apiEntryMissing` 与 **38** 条 `docMissing`（该元素自身零命中）

#### Scenario: 报告数与基线表逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `docMissing = 38`（35 条 `declared` 缺派生 md + 3 条 `api = none` 缺派生 md）、其余九值各 0；11 个两份资产齐全的元素零判据；`status = clean`、`passed = true`

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某「派生 md 缺失」的元素漏报 `docMissing`（假阴），或让某派生 md 已在盘的元素也被判 `docMissing`（假阳）
- **THEN** 前者使 `docMissing` 由 38 变 37（少于基线）；后者使该桶变 39（多于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 词汇收缩零读数漂移

- **WHEN** 对改动前后各跑一次全量检查，逐 `kind` 比对
- **THEN** 十值桶与三值删除前的九值桶逐项相等（被删三值桶原本为 0）；唯一差异在 `docMissing` 的 `message` 文案；`status` / `passed` / `model` / `scan` 逐字相同

#### Scenario: sidecar 计数不变

- **WHEN** 运行既有「零第二套判据语料目录」用例
- **THEN** `crates/core/archi/src/api/**` 的 sidecar 计数与改动前相同（新用例全落在既有文件内），`tests/corpus/api/` 仍零存在

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 sidecar 用例与 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（由 `check::check` 直驱断言）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`.schema.json` 相关 case 的夹具改为「schema 在盘 / 缺席」两态而非「md fence 形态」

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认


### Requirement: 契约判据词汇封闭十值与开放分类

契约判据 SHALL 归入**新平行封闭枚举** `ArchiApiFindingKind`（**十值**，线格式 camelCase），MUST NOT 并入既有 `ArchiDriftKind`（四值）：

| 枚举值 | 线格式词 | 语义 |
|---|---|---|
| `ApiEntryMissing` | `apiEntryMissing` | **该元素没有可用入口**：无候选（未声明 `entry` 且无语言约定默认）**或**候选被判不可用（形态非法 / 落在元素 `path` 之外 / 文件不在盘上 / 与另一元素同一入口）。解析链与四条判据见 `desktop-archi-api`「生成物落点与入口查找」 |
| `ApiMissing` | `apiMissing` | **有约定无兑现**：接口定义的 `properties` / `$defs` 声明的 op / 类型在代码里不存在 |
| `ApiUndeclared` | `apiUndeclared` | 代码有 `external` 面但接口未声明（**唯一完备性判据**，档位 `off`） |
| `ApiSignatureMismatch` | `apiSignatureMismatch` | 接口签名与代码签名（归一后）不一致 |
| `ApiUnexpected` | `apiUnexpected` | 留白被违反（**单形态**）：`api = none` 而 `<FQN>.schema.json` 在盘 |
| `ApiShadowing` | `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空 |
| `ApiRefMissing` | `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 目标未 `declared` / 非直接子元素 |
| `ApiNestedCrossCrate` | `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate |
| `ApiConstructorDeclared` | `apiConstructorDeclared` | 接口的 `properties` 声明了构造器（应由生成器产出） |
| `DocMissing` | `docMissing` | **文档未就位**（单成因）：派生的 `<archi_models_dir>/<分层目录…>/<FQN>.md` 不在盘，**与 `api` 状态、`doc` 字段都无关** |

- **三项删除的事实源口径（必须逐条说准，MUST NOT 读成「放宽」）**：

  | 删除值 | 它的事实源 | 为何消失 |
  |---|---|---|
  | `apiReexportDrift` | md 散文透传列表 vs `api-reexport` fence 键集 | md 无结构、fence 不再被读取 ⇒ 两侧事实都不存在 |
  | `apiDocMissing` | 接口 md 在盘但正文无 api fence | 接口真相换到 `.schema.json`，md 不再承载接口 ⇒ 「有文档无约定」不再是缺口形态 |
  | `docNotFound` | `doc` 字段声明指不到盘 | `doc` 字段惰性化（零读者、零判据、零纳管） ⇒ 无对象 |

  三项都是**词汇缩小**（事实源消失），不是标准降低；补位判据（`schemaMissing` / `schemaInvalid`）明属后续 change，MUST NOT 在本期以别名预铺。
- **`docMissing` 收成单成因**：旧的双成因（`declared` 缺 md / `none` 未声明 `doc`）与三值分工（`docMissing` = 还没写 / `docNotFound` = 声明写错 / `apiDocMissing` = 承诺未展开）随之作废——本值只判「派生 md 不在盘」。
- **`apiUnexpected` 收成单形态**：旧形态②（`api = declared` 而写了 `doc`）**删除**；`doc` 字段零读者，写了不报。
- **`.i` 生成物的存在 MUST NOT 作为入口的替代证据**（生成物路径是入口的函数）。
- **`api = none` 元素退出入口判据**：其求值链只剩 `apiUnexpected` → `docMissing` 两步，MUST NOT 报 `apiEntryMissing`。
- **`apiEntryMissing` 的四种不可用理由挤进同一个值**（形态 / 落位 / 存在 / 跨元素唯一），文案点名具体理由；MUST NOT 新增第 11 个值。
- **`ALL` 与 `as_str` / `severity` 三方对拍**：`ArchiApiFindingKind::ALL: [Self; 10]` SHALL 与 `as_str` 逐值对拍、与 `severity()` 的取值面逐值对拍；新增变体未补入 `ALL`（长度断言）即测试红。
- **`category` SHALL 仍为开放值集字符串（MUST NOT 设枚举、MUST NOT 设 `ALL`）**，取值集**仍是七值**——三处删除只收缩成员、不清空任何分组：`entry`（`apiEntryMissing` / `apiUnexpected`）/ `contract`（`apiMissing` / `apiUndeclared`）/ `signature`（`apiSignatureMismatch`）/ `reexport`（`apiShadowing` / `apiRefMissing`）/ `nesting`（`apiNestedCrossCrate`）/ `constructor`（`apiConstructorDeclared`）/ **`doc`（只剩 `docMissing`）**。MUST NOT 为「只剩一个成员的分组」做合并或改名（chip 由数据聚合，空组自然不渲）。
- **条目形状**：`ArchiApiFinding { kind, category, element, target, message, occurrences }` 逐字不变（`occurrences` 复用 `ArchiDriftOccurrence`）。

#### Scenario: 十值词汇三方对拍

- **WHEN** 遍历 `ArchiApiFindingKind::ALL`、`as_str` 与 `severity()`
- **THEN** 十项一一对应、线格式词唯一且与语料词一致；从 `ALL` 删掉一项、新增变体未补 `ALL`、或 `severity()` 漏一个变体时测试红；`ALL` 长度恰 10

#### Scenario: 三值离场且线面同步

- **WHEN** 审查 `bindings.ts` / `bindings.zod.ts` 的 `ArchiApiFindingKind` 联合
- **THEN** `"apiReexportDrift"` / `"docNotFound"` / `"apiDocMissing"` 三值零命中；其余十值逐字在场；两份产物的联合成员集逐字相同

#### Scenario: category 七值无空组

- **WHEN** 遍历十个判据值并取 `category_of`
- **THEN** 七组各自至少一个成员（`doc` 只剩 `docMissing`）；无第八值、无空串、无空组；`apiUnexpected` 仍落 `entry`

#### Scenario: 与既有漂移词汇互不混用

- **WHEN** 审查 `ArchiApiFindingKind` 与 `ArchiDriftKind` / `ArchiCheckStatus` / `ArchiObservationKind`
- **THEN** 四组词汇各自单点定义、互不复用；`ArchiDriftKind::ALL` 仍四值、`match` 臂零改动

#### Scenario: none 元素不再报入口缺失

- **WHEN** 构造一个 `api = none` 且无可用入口（无 `entry`、目录型根无 `index.ts(x)`）的元素
- **THEN** 该元素零 `apiEntryMissing`；若其派生 md 不在盘则恰一条 `docMissing`

### Requirement: 十判据的定义与求值互斥顺序

检查 SHALL 对每个 **`path` 非空**的元素求值，求值顺序固定、**按 `api` 状态分两条互斥支链**，每支内部**首错短路**：

```text
api = none 支（2 步；不依赖入口）
  ① apiUnexpected   `<FQN>.schema.json` 在盘 ⇒ 留白被违反（必须先判）
  ② docMissing      派生 `<FQN>.md` 不在盘

api = declared 支（4 步；③ 与 ⑤⑥ 的对象条件互不替代）
  ③ docMissing      派生 `<FQN>.md` 不在盘（文档未就位；单成因、与 `api` 无关）
  ④ apiEntryMissing entry_state = Absent / Invalid（code 面的结构前提）
  ⑤ apiMissing      schema 在盘时：接口声明 op / `$defs` 类型而代码导出集里没有
  ⑥ 一致性判据      schema 在盘时：apiSignatureMismatch / apiConstructorDeclared

pair 锚点（不进元素循环，与元素锚点并列、互不挤占）
  ⓐ apiNestedCrossCrate → ⓑ apiRefMissing → ⓒ apiShadowing
```

- **③ 判的是派生 md，不是 schema**：`docMissing` 的语义在本变更收成单因（派生 `<FQN>.md` 不在盘），与 `api` 状态无关，故它在两支里同义。**「接口未写」（schema 不在盘）不产任何判据**：它在 ⑤⑥ 表现为「声明集不存在 ⇒ 零对象」（`schemaMissing` 归后续 change，MUST NOT 在本期以别名预铺）；③ 短路时 ⑤⑥ 自然无对象，而 schema 缺失但 md 在盘的元素直接跳过 ⑤⑥、不报 `apiMissing` / `apiSignatureMismatch`。
- **两支互斥，跨支不比较**（同一元素只走一支）；支内序号即求值序，**顺序写死在 `check.rs` 函数体**，MUST NOT 由配置驱动。
- **① 排第一的理由不变**：`apiUnexpected` 与 `doc` / 文档无关，而 `docMissing` 是 `warn` 且不纳管——若让它先短路，一个 `api = none` 元素只要没有文档，就能把它那份放错位置的接口遮住。
- **④ 的判定 SHALL 单点取自 `paths::entry_state`**：`Resolved` ⇒ 继续 ⑤⑥；`Absent` / `Invalid(_)` ⇒ 一条 `apiEntryMissing`；判据面 MUST NOT 自行判文件存在性、MUST NOT 保留第二份入口探测。
- **⑤ 的声明集 / 导出集口径**：声明集 = 接口 `properties` 的 `x-function` 键集（**构造器双向都不参与比对**；schema 不在盘 ⇒ 无声明集 ⇒ ⑤⑥ 零判据）；导出集 = 元素的 `external` 代码导出（Rust 经 `pub mod` 链或 `pub use` 白名单可达的 `pub fn`；TS 经**入口导出闭包**可达的导出，定义见 `desktop-archi-api`「TS 导出闭包的取数面」，本能力 MUST NOT 另立第二份口径）。签名比对 SHALL 在**归一后**进行（AST 归一单行、参数名与空白不参与）。
- **`apiUndeclared` 只读 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown（markdown 已无结构）；其声明集前提依赖 schema 在盘（schema 不在盘的元素没有声明集可对拍，故档位虽 `off`、语义上也无对象）。
- **pair 锚点的形状与内部序**逐字不变（nested → ref → shadowing）；pair 锚点 MUST NOT 引用 `Model.relations`。
- **零第二份判定**：十条判据 SHALL 只有一处实现（`crates/core/archi/src/api/check.rs`），命令面与门禁面 MUST NOT 各写一份。
- **契约面启用条件（换面）**：`api::enabled` SHALL 为「存在任一 `path` 非空 ∧ `api = declared` 元素的 `<FQN>.schema.json` 在盘」——与判据面同源。判据 MUST NOT 退化为「`<archi_models_dir>` 递归任意 `*.md` / `*.schema.json`」。启用后**全部 `path` 非空元素**（本仓 49 个）纳入判据，不存在「部分元素豁免」的中间态，也不设白名单或开关。

#### Scenario: declared 支首错短路

- **WHEN** 某 `api = declared` 元素的派生 `<FQN>.md` 不在盘
- **THEN** 只报一条 `docMissing`；同元素不出现 `apiEntryMissing` / `apiMissing` / `apiSignatureMismatch`（对象不存在）

#### Scenario: 接口未写不产判据

- **WHEN** 某 `api = declared` 元素的派生 md 在盘而 `<FQN>.schema.json` 不在盘（入口存在、代码有导出）
- **THEN** 该元素零判据（`docMissing` 不成立、⑤⑥ 无声明集）——「接口未写」本期不产条目（`schemaMissing` 归后续 change）；报告其余字段与「schema 在盘」场景的差异只在契约面计数

#### Scenario: none 支两步与 apiUnexpected 前置

- **WHEN** 构造两个 `api = none` 元素：一为派生目录里有 `<FQN>.schema.json` 且派生 md 不在盘；一为派生目录干净且派生 md 也不在盘
- **THEN** 前者恰报 `apiUnexpected`（MUST NOT 同时报 `docMissing`）；后者恰报 `docMissing`；两者均零 `apiEntryMissing`

#### Scenario: 接口声明 op 而代码无

- **WHEN** 元素 schema 在案、入口存在，`properties` 声明 `parse` 与 `all_conclusions` 两个 op，代码只有 `parse`
- **THEN** 报一条 `apiMissing`，点名 `all_conclusions`；`apiUndeclared`（`off`）不产出条目；同元素不再出第二条判据（首错）

#### Scenario: 签名归一后比对

- **WHEN** 接口声明 `parse_cargo_test(input: string) -> SummaryReport`，代码把签名排成多行且参数名不同但类型序列相同
- **THEN** 归一后判定一致、零 `apiSignatureMismatch`；把返回类型改为 `Vec<SummaryReport>` 时报一条并给出两侧归一签名

#### Scenario: 构造器声明即契约自身矛盾

- **WHEN** 接口 `properties` 声明 `new`（`x-function`）而代码的 `new()` 由生成器产出
- **THEN** 报一条 `apiConstructorDeclared`，**不**同时报 `apiMissing`

#### Scenario: pair 锚点不引用 relations

- **WHEN** 构造「父 `$defs` 与子元素 `$defs` 类型名交集非空」且父子在模型里没有 `[[relation]]` 边
- **THEN** 仍报一条 `apiShadowing`（`element` = 父、`target` = 子）；模型零结构诊断

#### Scenario: 启用判据与接口文件同源

- **WHEN** 观察根下 `graph/model/` 一级有六个 `NN-<域>.toml`，另有 `models/notes.md`（非任何元素的载体）与一份非 `<FQN>` 主干的 `*.schema.json`
- **THEN** `enabled = false` 且 `apiFindings = []`；随后给任一 `api = declared` 元素补一份合法 schema ⇒ `enabled = true`、全部 `path` 非空元素纳入判据

#### Scenario: entry 优先互斥

- **WHEN** 某 `api = declared` 元素既无可用入口、schema 在案且 `properties` 声明了 2 个 op
- **THEN** 只报一条 `apiEntryMissing`（④）；同元素不出现 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`；schema 缺失不产判据（⑤⑥ 无对象），该元素只要派生 md 在盘就仍是这一条（MUST NOT 被读成 `docMissing`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 判据词汇与报告槽位 | `ArchiApiFindingKind`（**十三值** + `ALL` + `as_str` + `severity()`）、`ApiFindingSeverity`（**三值** `Off` / `Warn` / `Error` + `ALL` + `as_str`）、`ArchiApiFinding { kind, category, element, target, message, occurrences }`、`ArchiDriftReport.api_findings`、`REPORT_VERSION = 3`（不递增，纳管口径入 `status` 推导）；`ApiMissing` 的 doc 语义收窄为「有约定无兑现」，`DocMissing` / `DocNotFound` / `ApiDocMissing` 的 doc 语义各自单成因（`ApiEntryMissing` 的四理由同值，MUST NOT 加第 14 值） |
| `crates/core/archi/src/api/mod.rs::enabled` | 启用判据（**收紧，行为变化**） | `enabled(root, elements) -> bool` = 任一 `path` 非空元素的入口文档载体在盘且 `DocFact::Present { has_api_fence: true }`；MUST NOT 退化为「递归任意 `*.md`」；判据与判据面同源（同一 `doc_facts` / 同一 fence 语言标常量），本能力**不另判一套**（`desktop-archi-contract-panel` 的 `enabled` 字段直接取它） |
| `crates/core/archi/src/api/mod.rs::gating_findings` | 纳管子集筛选单点 | 入选 = `element` 非空 ∧（declared：派生契约 md 在盘 / none：`doc` 已声明）；`docMissing` 两侧都不成立 ⇒ 永不纳管；`drift` 侧 MUST NOT 自行判文件存在性 |
| `crates/core/archi/src/api/mod.rs::load_contracts` | load 阶段事实装配 | `ApiSnapshot` 增 `doc_facts: BTreeMap<Fqn, DocFact>`（`Absent` / `Missing` / `Present { has_api_fence }`），每份 doc 只读一次即弃（不缓存全文）；`ApiCheckInput` 同批携入 |
| `crates/core/archi/src/api/contract.rs` | fence 语言标单点 | `FENCE_OP` / `FENCE_TYPES` 提 `pub(crate)`，供 `apiDocMissing` 与 `enabled` 复用；MUST NOT 抄第二份字面量 |
| `crates/core/archi/src/api/check.rs` | 十三判据单点求值 | `check(ApiCheckInput) -> Vec<ArchiApiFinding>`：**两支首错短路**（none 支 `apiUnexpected` → `docMissing` → `docNotFound`；declared 支 `docMissing` → `apiDocMissing` → `apiUnexpected` → `apiEntryMissing` → `apiMissing` → 一致性）+ **pair 锚点**（`nested → ref → shadowing`，MUST NOT 引用 `relations`）；`category_of` **七值**（+ `doc`）；`missing_contracts` 只剩形态②；`apiDocMissing` 只读 `input.doc_facts`（零读盘、零 `contract::parse`）；声明集（op **与 `$defs` 类型**）vs 导出集、签名归一比对、嵌套透传五项语义逐字不变；**已删** `has_generated`（`.i` 存在不再充当入口证据）；`$defs` 只读 `schema.json`；零第二份判定；**导出集单点**：导出集由 `extract::extract_element` 给出（TS 侧即入口导出闭包）⇒ `apiMissing` 只减不增、`apiUndeclared` 仍 `off`、`apiSignatureMismatch` 仍逐符号归一比对；判据面 MUST NOT 自行解析 re-export、MUST NOT 内联 owner 归属判断（那是取数面的单点） |
| `crates/core/archi/src/drift/mod.rs` | 报告装配单点 | `check()` 内调 `api::check` 装配 `api_findings`（与模型面并列）并取纳管子集传 `derive_status`；`derive_status` 在纳管子集上**分档**（任一 `Error` ⇒ `errors`）；`passed` 判据追加「error 档 `apiFindings` 空」；`status == errors ⟺ passed == false` 不变量保持 |
| `crates/core/archi/src/api/check_test.rs` | 判据语料（本变更：夹具落点换段） | 契约夹具由 `<archi_api_dir>/<分层目录…>/` 换到 `<archi_models_dir>/<分层目录…>/`，每条判据的正反例与断言值逐字不变； 十三判据正反例（三值新判据的用例全在既有 sidecar 内）/ entry 互斥（含 `Absent` 与四种 `Invalid` 理由各一条）/ 「`.i` 在盘但无入口仍报 ①」/ `api = none` + `entry` 有值零判据 / 签名归一 / 留白双向 / 交集 / 非直接子元素 / 跨 crate / 透传漂移 / `severity()` 与 `ALL` 逐值对拍；`function()` / `type_symbol()` 夹具补 `ApiSymbol.file`（形状适配，断言值零变化，不新增判据用例） |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 报告面语料 | 十六份金样随「首错短路 + 纳管分档」显式重录并逐份留痕成因；`corpus/drift/<case>/{contracts,models,repo}` + `expected/<case>.<mode>.json`；`DRIFT_CASES` 登记面即驱动表（未登记 / 目录不存在 / 缺 `apiFindings` 键三种失配皆红）；`api-mismatch` case 保持非空 `apiFindings`；golden 沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
| `crates/core/archi/tests/vocabulary_test.rs` | 词汇对拍 | `ArchiApiFindingKind::ALL` 长度 **13** 与 `as_str` / `severity()` 逐值对拍；`WarningKind` / `DocKind` / `DocSource` 三组对拍删除；`SearchScope` 三值词面换 `doc` |
| `<change>/reports/archi/<timestamp>.json`（门禁留痕） | 运行时产物 | 报告由门禁进程写出并作移交留痕（`apiFindings` 长度 **36**、`status = clean`）；`reports/archi/` 不入库、不计入交付面、不作判红面（`??` 属预期） |

### API: `crates/core/archi/src/drift/mod.rs` + `src/api/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `api::gating_findings` | 改 | `(root, &[ArchiElement], &[ArchiApiFinding]) -> Vec<ArchiApiFinding>`：纳管子集筛选单点（口径由「契约文件在案」改为「入口文档承诺已成立」；`element = None` 落选） |
| `drift::check` | 改 | `api_findings` 装配后取纳管子集传给 `derive_status`；其余装配序与字段逐字不变 |
| `derive_status` | 改 | 纳管子集入参保留；新增 `errors` 分支（纳管子集含 `error` 档）；`warnings` 分支条件收为「纳管子集非空且其中无 `error` 档」；`skipped` / `noChanges` 的先后序与语义逐字不变 |
| `drift/report.rs` | 改 | `REPORT_VERSION` 2 → 3 与兼容说明；14 字段形状零改动 |

### Component: 呈现面（归 `desktop-archi-contract-panel`）

| 项 | 描述 |
|---|---|
| `views/architecture/components/dashboard/contract-panel.tsx` | 判据轴按 `category` 分组呈现同一份 `apiFindings`（零重算） |
| `views/architecture/components/model-info.tsx` | 按 `element === fqn` 过滤呈现本元素判据（零二次 invoke） |

### 模块增量：报告数与入口面读数随动（relation-api-dependency-check）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/**` | 十三判据求值（零改动） | 判据种类、两支首错序、`severity()`、`gating_findings` 与 `doc_facts` 三态逐字不变；`api = none` 元素退出入口判据的语义不变（新元素因此零 `apiEntryMissing`） |
| `crates/core/archi/src/drift/report.rs` | 报告（判据面零改动） | `apiFindings` 槽位、`ArchiApiFindingKind` 十三值、`severity()` 三档、`status` / `passed` 推导零改动；同文件内的 `ArchiDriftKind` +1 值（`undeclaredApi`）与 `ArchiObservationKind` 产出条件由 `desktop-archi-drift` 登记，**不属**本能力的零改动面 |
| 本能力基线表（spec 面） | 改 | 分母 48 → **49**、`docMissing` 38 → **39**（36 declared + 3 none）；历史链补两段并写明「44 / 43 早已陈旧」的成因，MUST NOT 让旧读数与新读数并排而无解释 |
| 本仓读数留痕（change `reports/`） | 新增 | 一次全量 `archi_drift` 的逐 `kind` 计数表（与基线表逐项相等）+ `git status` 显示 `reports/archi/` 未入库；MUST NOT 写入 crate 语料 |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 语料面（只加不改） | 既有 16 份快照逐字节不变；新增 `api-*` case（声明满 / 缺一项 / 多一项 / 未填 `apis`）各自登记进 `DRIFT_CASES`；`api-mismatch` 仍非空 `apiFindings` |
| 门禁留痕行（spec 面既有行） | 读数随动 | 「`apiFindings` 长度 **43**」→ **36**；`status = clean` 与「不入库 / 不作判红面」口径逐字保留 |

### 模块增量：判据词汇十值与纳管换锚（archi-agent-context-review）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 判据词汇单点 | `ArchiApiFindingKind` 十值 + `ALL: [Self; 10]`；`as_str` 十词；`severity()` 的 `error` 恰 `ApiMissing` / `ApiSignatureMismatch`、`off` 恰 `ApiUndeclared`、其余七项 `warn`；`category` 七值（`doc` 只剩 `doc_missing`）；`ArchiApiFinding` 形状逐字不变 |
| `crates/core/archi/src/api/check.rs` | 十判据单点求值 | `ApiCheckInput.doc_facts` → `doc_present: bool`（派生 md 是否存在）；`none` 支 2 步、`declared` 支 4 步（③ 判派生 md；schema 不在盘 ⇒ ⑤⑥ 零对象、不产判据）+ pair 锚点三条；删三支与 `FENCE_*` 依赖；`enabled` 输入沿用（判据面零 fence 知识） |
| `crates/core/archi/src/api/mod.rs` | 纳管子集单点 | `gating_findings(root, elements, findings)`：入选 = `element` 非空 ∧ `api = declared` ∧ `<FQN>.schema.json` 在盘（**文档在盘与否零参与**）；`findings` 装配入口零第二份判定 |
| `crates/core/archi/src/drift/mod.rs` | 报告装配 | `api_findings` 经 `api::findings` 单点；`derive_status` 分档逐字不变；`REPORT_VERSION = 3` 不变 |
| `crates/core/archi/src/drift/{report,mod}_test.rs` + `api/check_test.rs` | 判据语料 | 十值三方对拍（长度 10）；`category` 七值无空组；`error` 恰两项；两支首错与 pair 锚点；纳管 = `declared` ∧ schema 在盘；`status == errors ⟺ passed == false`；三值离场反向断言 |
| `crates/core/archi/tests/golden_test.rs` + `tests/corpus/drift/**` | 报告面语料 | `DRIFT_CASES` 登记面零改动（语料自足、不读本仓资产）；金样按十值词汇与新首错序重写（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认）；`api-mismatch` case 仍非空 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` 与 `src/types/generated/**` | 线面随动 | `ArchiApiFindingKind` 联合三值离场、十值在场；`DTO_TYPES` 清单零增删（枚举不是 DTO 类型）；`bindings:export` 幂等 |

### Function: `crates/core/archi/src/api/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `gating_findings` | 改（判据换锚） | 入选 = `element` 非空 ∧ `api = declared` ∧ schema 在盘；`api = none` 分支与 `carrier_path` 依赖删除 |
| `findings` / `enabled` | 改（输入换面） | 由 `load_contracts` 装配的 schema 事实派生启用；fence 判定随 `doc_facts` 收窄删除 |

### API: 报告与工具响应（形状零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiDriftReport.apiFindings` | 零形状变化 | 条目形状与排序不变；`kind` 取值面收为十值 |
| `archi_drift` 的 `byKind` 计数 | 随词汇收窄 | 键集合由十三变十；`counts.api` 三档计数语义不变 |
| `archi_api(action = "check")` | 零形状变化 | 仍回三类差异 + `byCategory` 计数 + 逐条判据；`byCategory` 键集合不变（七组） |
