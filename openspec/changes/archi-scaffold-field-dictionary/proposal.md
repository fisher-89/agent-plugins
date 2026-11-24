# 提案: archi-scaffold-field-dictionary

> **变更**: archi-scaffold-field-dictionary（`archi_scaffold` 升级为字段词典 · P2）
> **日期**: 2026-11-24
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇——从主篇 `archi-agent-contract-knowledge-gap` 第十一节拆出的**前置片 P2**（§一 前置理由、§二 现状盘点、§三 方案 B1「保留 TOML 草稿、注释扩成字段表、零新参数」、§四 七条验收面、§五 四条不做项、§六 与主篇的时序依赖）。
> **性质**: **模板文本 + 机械锚**——`SCAFFOLD_TEMPLATE` 四处注释补漏（`entry` 单值 / `metadata` 值域 / `apis` 三条值面）、工具描述改词典措辞、新增 core 侧字段面常量 `ELEMENT_FIELD_NAMES` 与三组对拍断言。零新参数、零新字段、零新诊断词、零 IPC / UI / 生成物变化、零模型元素与关系变化。
> **不含**: `mode = "draft" | "dict"` 双形态（D2 裁决 B1：一个工具一个形状）；纯 Markdown 词典（会丢掉「自证可解析」这条唯一背书）；按字段查询（`field = "path"` 新参数面）；`archi_module` 响应复述「该元素可改字段」（字段面已由 `archi_element` 参数 schema 承载，explore §3.3 倾向不做）；知识骨架的字段纪律段压缩与相位 prompt 的「词典」指路句（归主篇切片 A）。

---

## 问题

### 一 · 现状：漂移修复（`6a8fb10`）已把模板补了大半

`SCAFFOLD_TEMPLATE`（`crates/infra/agent/src/sdk/archi_tool.rs` L198-236）对八个字段面字段里的大多数，已写到「形态 + 值域 + 缺省 + 基数」四要素。逐字段审计（模板现文 → 还缺什么）：

| 字段 | 模板现状 | 还缺 |
|---|---|---|
| `kind` | 四值语义逐值（`package` 至多一条包根 / `domain` 领域容器 / `module` 含 crate 视图单文件 / `composite` 纯分组） | — |
| `path` | `module` 必填可多条 / `package` 至多一条 / `domain`·`composite` 选填；`./` 前缀、目录尾 `/`、文件到扩展名 | — |
| `entry` | 约定默认三档 + 「指不到时写」 | **单值**（骨架散文的「`entry`：单值」在模板里无对应句） |
| `gate` | 值 `off` / `warn` / `strict` + 缺省 `off` + ratchet 语义 | — |
| `api` | `declared` / `none` + 缺省 `declared` | — |
| `doc` | 单值、仅 `api = none` 可写、`declared` 即 `apiUnexpected` | — |
| `summary` | 一行摘要 | — |
| `metadata` | 子表与内联表等价 | **值 MUST 是字符串**（数字 / 布尔等非字符串值被拒） |
| `[[relation]]` 四键 | `from` / `to` / `desc` / `apis` 各一行 | — |
| `apis` | 「选填名字清单，只写名字、可留空、生成物面留空；差额看 `archi_drift`」 | **逐项非空** / **同名不重复** / **`apis = []` 与省略同义（显式空数组被拒）** |

三处缺口全部有模型侧判据背书（`parse/toml.rs` 的 `validate_apis` 五拒、`classify` 的 `invalid type → invalidValue`、`RawElement.entry: Option<String>`），模板只是没写出来。

### 二 · 病灶：模板是「手写清单」，不是字段面的视图

模板与字段面的一致性当前靠「人记得同批改」。字段面有八项（`parse/toml.rs` 的 `RawElement`），模板注释里逐项出现，但**没有任何机械锚**把两者绑在一起：新增第 9 个字段时，`RawElement` 编译通过、`deny_unknown_fields` 生效、模板那边静默缺一行——agent 拿到的词典少一个字段，而全部测试是绿的。

同一病灶在工具面还有第二处：`archi_scaffold_definition()` 的描述把字段面切成四枚内联短标签（`字段序 / 内联 metadata / path 必填提示 / 关系模板`）——字段面一动，这段描述就静默陈旧。第三处是知识骨架的「字段纪律」段（827 B，主篇 A 的迁移对象）；P2 收口前两处，第三处归 A。

值域同样靠手抄：草稿里的 `off` / `warn` / `strict` 与 `declared` / `none` 是字面量，测试只断言了 `# gate` / `# api` 两个行首词，**没有**与 `GateTier::ALL` / `ApiStatus::ALL` 对拍（`ElementKind::ALL` 的四值语义已有断言）。新增一个 gate 档位时，`ALL` 扩了、模板没扩，同样静默。

### 三 · 为什么必须先于主篇 A

主篇 A 要把骨架的 827 B 字段纪律压成判断句，指路句写「字段形态用 `archi_scaffold` 取词典」。若词典还没补齐：

- `entry` 的「单值」纪律在散文删掉后**无归宿**（模板只写「指不到时写」）；
- `apis` 的三条值面（非空 / 不重复 / 空数组拒收）在散文删掉后无归宿（模板只写「只写名字」）；
- 「字段面新增一个字段」在 A 之后仍然只靠「人记得」——A 只是把清单从散文搬到模板，病灶原样搬家。

P2 因此不是「A 的附属清理」，而是 A 能否安全删散文的硬前提。

### 四 · 一处需要裁决的冲突（explore 字面稿 vs 既有机械锚）

explore §3.1 的 metadata 注释字面含 `invalidValue` 一词，而既有测试 `archi_scaffold草稿零结构诊断词` 明确断言草稿**零** `invalidValue` / `unknownField` / `missingField` / `syntaxError`（其 doc comment 记为「与骨架 `md不列结构诊断全量词表` 同源纪律」；`desktop-archi-agent`「呈现源与对拍源分家」亦要求结构诊断全量词表不进知识文本）。

本提案的裁决：**保留既有锚，语义在场、词面不进**。理由三条：① 词典的职责是「值面规则」，不是「错误词表」——agent 需要知道「metadata 的值只能是字符串」，不需要知道违例叫 `invalidValue`；② 该纪律在知识骨架与模板两面同源，为一个词开口子等于把一个既有机械锚降级为「带例外的锚」；③ 真正违例时 `archi_module` / `archi_drift` 会带回诊断词，词典不必预告。落地形态：注释写「值 MUST 是字符串（数字 / 布尔等非字符串值被拒）」——`被拒` 是行为描述，不是诊断词。explore 的验收第 3 条按此口径改写（见 AC-1），MUST NOT 为了凑字面删测试。

---

## 提案

### 一 · 模板补四处（逐字文本 + 断言 needle）

均落在既有注释行，**不加字段、不加占位符、不加裸 TOML 行**（可解析性零风险）：

| # | 字段 | 改后文本（节选） | needle（断言锚） |
|---|---|---|---|
| 1 | `entry` | `# entry：单值，只在约定默认指不到时写（…）` | `单值`（**行首限定** `# entry：`——`doc` 行也含「单值」，子串匹配会假命中） |
| 2 | `metadata` | `# metadata：选填自由表（字符串 → 字符串）——值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；` + 续行 `#           子表与内联表等价（…）` | `值 MUST 是字符串`、`非字符串值被拒`、`子表与内联表等价`（既有） |
| 3 | `apis` | 首行 `# apis：选填名字清单（可留空 = 不声明 API 粒度）——只写名字（禁签名 / 类型形状 / :: 限定路径 / 空白）；` + 续行 `#       逐项非空、同名不重复；apis = [] 与省略同义（显式空数组被拒）；生成物面留空，差额看 archi_drift`；模板正文里 `apis = []` 带反引号（与本模板既有的 `./` 同风格），故该条 needle 逐字含反引号 | `只写名字`（既有）、`可留空`（既有）、`生成物面留空`（既有）、`archi_drift`（既有）＋ `逐项非空`、`同名不重复`、`` `apis = []` 与省略同义 ``、`显式空数组被拒` |

四处的判据来源逐条对得上代码：`entry: Option<String>`（单值，数组形态由 serde `invalid type` 归 `invalidValue`）、`metadata: BTreeMap<String, String>`（非字符串值同归 `invalidValue`）、`validate_apis` 的五拒（空串 / 含空白 / 含 `::` / 同名重复 / 显式空数组）。

改后模板的三块逐字如下（其余行逐字不动；未改动字段不列）：

```toml
# entry：单值，只在约定默认指不到时写（repo 相对 POSIX；单文件根 = 根文件自身 / 目录型根 = index.ts(x) / crate = src/lib.rs）
# entry = "./src-tauri/crates/core/example/src/lib.rs"

# metadata：选填自由表（字符串 → 字符串）——值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；
#           子表与内联表等价（[element."X".metadata] 写在已声明元素之后零诊断；成为唯一声明才缺 kind）
# metadata = { owner = "team-x", layer = "core" }

# apis：选填名字清单（可留空 = 不声明 API 粒度）——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）；
#       逐项非空、同名不重复；`apis = []` 与省略同义（显式空数组被拒）；生成物面留空，差额看 archi_drift
```

### 二 · 把「手写清单」换成「字段面的视图」：一层常量 + 三层对拍

| 锚 | 落点 | 形式 |
|---|---|---|
| **字段面清单** | `crates/core/archi/src/parse/toml.rs`（`RawElement` 定义旁，与字段注释并列） | `pub const ELEMENT_FIELD_NAMES: [&str; 8]`，顺序 = 字段声明序（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）；经 `parse/mod.rs` 再导出为 `archi::parse::ELEMENT_FIELD_NAMES` |
| **常量 ↔ 字段面**（core） | `parse/toml_test.rs` | `include_str!("toml.rs")` 取 `struct RawElement { … }` 块内的字段名（滤掉 `///` 与 `#[…]` 行），断言**同集同序**——新增 / 改名 / 删除字段而常量未同批更新即红（范式同 `model_test.rs` 的 `element字段数恰十一个`、`toml_test.rs` 的 `解析doc白名单源码自证`） |
| **词典 ↔ 常量**（infra） | `archi_tool_test.rs` | 遍历 `archi::parse::ELEMENT_FIELD_NAMES`，逐字段断言草稿存在**行首形态**：`{field} = ` / `# {field}：` / `# {field} = `。MUST NOT 用 `draft.contains(field)`——`apis` 会让 `api` 假命中，`doc` 一类的行内复述会掩盖整行缺失 |
| **词典 ↔ 值域**（infra） | `archi_tool_test.rs` | 遍历 `GateTier::ALL` / `ApiStatus::ALL`，逐值断言 `as_str()` 出现在对应纪律行且缺省档（`off` / `declared`）以「缺省」字样在场；`ElementKind::ALL` 四值语义沿用既有断言 |

`ELEMENT_FIELD_NAMES` 仍是第二份清单（Rust 无「遍历 struct 字段名」的内建能力），但它**贴在字段定义旁**（新增字段时它就在眼前）且**消费侧零手抄**（测试遍历它、不重抄八个字面量），比散在模板字符串里强一个量级——这正是 `dto.rs` 里 `ElementKind::ALL` / `GateTier::ALL` 的既有范式。

### 三 · 工具描述改词典措辞（去掉字段名枚举）

`archi_scaffold_definition().description` 由

> 取新元素 TOML 模板（字段序 / 内联 metadata / path 必填提示 / 关系模板），只读零写盘——写面仍是既有 write / edit。kind 省略取 domain，值集见 enum。草稿可直接抄改（单段占位 FQN，多段时先声明祖先），改完用 archi_drift 自查

改为

> 取新元素 TOML 草稿，同时是字段词典：模型元素字段面逐字段一行纪律与值域 + 关系四键模板，只读零写盘——写面仍是既有 write / edit。kind 省略取 domain，值集见 enum。草稿可直接抄改（单段占位 FQN，多段时先声明祖先），改完用 archi_drift 自查

两处收益：① 主篇 A 的指路句「取词典」在工具清单里就能对上（描述是 agent 每轮都看得见的那一面）；② 描述不再枚举具体字段名，字段面扩容时它不会静默陈旧——与 `archi_module` 描述「MUST NOT 复制字段语法长表」同一条纪律（`desktop-archi-agent`「描述面（`mount.rs`）」）。

### 四 · 本仓读数：零变化

| 项 | 变更前 | 变更后 |
|---|---|---|
| `.dev-team/architecture` 元素 / 关系 | 55 / 129 | **55 / 129**（不新增元素与关系——两个改动 crate 都在既有 `package` 元素的 `path` 内） |
| `archi_drift` `status` / `passed` / 结构诊断 | clean / true / 0 | **逐字不变**（模板文本不进模型） |
| `INDEX.toml` / `.schema.json` / `.i.rs` / `.i.ts` | — | **零 diff**（`Backend.Core.Archi` 与 `Backend.Infra.AgentRuntime` 都是 `status = "absent"`：无契约资产可对拍；`pub const` 不进 `types` / `ops` 计数） |
| `bindings.ts` / `.zod.ts` | — | **零 diff**（常量无 serde / specta） |
| `scan.*` 三项计数 | 422 文件 | **逐字段不变**（零新增 / 删除文件，测试只在既有文件内追加） |

### 五 · 不做

| 面 | 理由 |
|---|---|
| `mode = "draft" \| "dict"` 双形态 | D2 裁决 B1：一个工具一个形状；草稿本来就是词典 |
| 纯 Markdown 词典（不产 TOML） | 丢掉「草稿经 `TomlParser` 零诊断」这条唯一背书——它是「示例 TOML 从骨架删除」得以成立的机械前提 |
| 按字段查询（`field = "path"`） | 新参数面；草稿是整份，agent 一次读全比多次点查省调用 |
| `archi_module` 响应附「该元素可改字段」 | 字段面已由 `archi_element` 的参数 schema 承载（每字段带 `description`），每个响应再复述一遍是第三份（explore §3.3） |
| `kind` 必填 / `summary` 选填 一类纯修饰 | 草稿以「`kind` 实值行 vs 其余注释行」的形态差自证必填性，加句子只增体量不增信息 |
| `metadata` 未知键接受的说明 | 模型侧忽略未知键，但知识骨架当前不教这条；补它属新关注点，不混进 P2 |

### 六 · 开单前实测（五条全过）

| 硬前提 | 核查方式 | 结果 |
|---|---|---|
| 草稿逐字段的**行首形态**已存在，可做行首匹配 | 逐行检查 `SCAFFOLD_TEMPLATE`：`kind = ` / `# gate：` / `# api：` / `# entry：` / `# path：` / `# doc：` / `# summary：` / `# metadata：` | ✅ 八项齐备；`# api：` 与 `# apis：` 前缀不同（后者的 `s` 挡住假命中） |
| `RawElement` 字段名可由源码扫出 | 字段行以 `,` 收尾，文档注释 `///` 与属性 `#[…]` 各自成行（`include_str!` 扫描范式已在 `model_test.rs` / `toml_test.rs` 用过两次） | ✅ 恰八项可取 |
| 工具描述文本零快照持有者 | 全仓 grep `内联 metadata` → 仅 `mount.rs` 与 `mount_test.rs` 两处；`字段序 / 内联 metadata / path 必填提示 / 关系模板` 在 `*.json` / `*.snap` / `*.txt` 零命中 | ✅ 改描述只需同批改一支测试 |
| 两 crate 的契约状态不影响生成物 | `.dev-team/architecture/graph/INDEX.toml` 的 `Backend.Core.Archi` 与 `Backend.Infra.AgentRuntime` 条目 | ✅ 均 `status = "absent"`（`types = 0` / `ops = 0`）⇒ `api:export` / `api:check` 零 diff |
| 既有零诊断词锚在案 | `archi_scaffold草稿零结构诊断词` 钉住 `invalidValue` / `unknownField` / `missingField` / `syntaxError` 四词 | ✅ 在案 ⇒ explore §3.1 的 `invalidValue` 字面须裁决（已裁，见「问题 · 四」） |

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-agent` | **MODIFIED ×1** | 「archi_scaffold 只读工具的新元素模板契约」：① 语义句由「给模板」扩为「给字段词典（草稿即可抄改模板）」；② 响应形状补「字段顺序 = `ELEMENT_FIELD_NAMES`」；③ 新增四条 bullet——描述面不枚举字段名 / 字段词典语义（八字段各一行纪律，含 `entry` 单值、`metadata` 值域、`apis` 四条）/ 字段面全集对拍（行首匹配、子串不顶替）/ 值域派生对拍（三组 `ALL` 逐值 + 缺省档）；④「模板自证可解析」补零结构诊断词、「体量受上限约束」补草稿体量读数；⑤ 新增三条 Scenario。既有四 Scenario、零写盘与只读档位两条 bullet 逐字保留。 |
| `desktop-archi-model` | **ADDED ×1** | 新 Requirement「元素字段面的机器可读清单」：`parse/toml.rs` 的 `pub const ELEMENT_FIELD_NAMES`（八项、声明序）+ 与 `RawElement` 的源码自证对拍 + 边界（不是值域词表、不进封闭词汇对拍、无线面）。既有「架构模型文件格式与字段封闭」的字段面条目逐字不动（本变更零字段增删）。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-phase-prompts` | `proposal-planner` / `dev-design-planner` 的「取新元素的 TOML 字段模板」句仍准确；「取词典」措辞归主篇 A（同批改骨架与 prompt） |
| `desktop-agent-execution` | 工具面注入清单、per-turn 铸造判据、`ToolTier::Read` 档位零变化 |
| `desktop-archi-drift` / `desktop-archi-gate` / `desktop-archi-api` / `desktop-archi-api-check` | 模板文本不进任何判定面；无新诊断词、无生成物变化 |
| `desktop-corpus-regression` | 语料与金样零变化（模板不在语料面） |
| `repo-archi-model` | 架构建模基线零 delta（55 / 129 不动，无新元素 / 关系） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` | `RawElement` 定义旁新增 `pub const ELEMENT_FIELD_NAMES: [&str; 8]`（注释写明「与 `RawElement` 同批维护、消费侧零手抄」）；`RawElement` / `to_model` / `validate_apis` / `validate_path` / `classify` 零改动 |
| `packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs` | `pub use toml::{TomlParser, ELEMENT_FIELD_NAMES};`（出口一行，沿 `TomlParser` 的既有重导出形态；括号内取 fmt 实序——rustfmt version-sort 把 CamelCase 排在 SHOUTY 前，`cargo fmt --all --check` 零退出） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` | `SCAFFOLD_TEMPLATE` 三块注释（`entry` 一行 / `metadata` 两行 / `apis` 两行）；`scaffold_draft` 与 `scaffold_payload` 零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | `archi_scaffold_definition()` 描述改词典措辞（去四字段名枚举）；`ARCHI_SCAFFOLD_TOOL` 常量 doc 注释同步 |
| `packages/desktop/package.json` | `version` 0.7.9 → 0.7.10（patch +1，AGENT.md 版本纪律） |

### 测试文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs` | 新增一支 `字段面清单与RawElement同序同集`（`include_str!("toml.rs")` 源码自证；滤 `///` 与 `#[…]` 行，断言与 `ELEMENT_FIELD_NAMES` 同序同集） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs` | 新增三支：① `archi_scaffold草稿含字段面全集`（遍历 `archi::parse::ELEMENT_FIELD_NAMES`，行首匹配，`api` 不被 `apis` 顶替）；② `archi_scaffold模板值域逐值且零诊断词`（`GateTier::ALL` / `ApiStatus::ALL` 逐值 + 缺省档 + 既有四词零命中）；③ `archi_scaffold模板值面纪律四条`（`entry` 单值行首限定 / `metadata` 两条 needle / `apis` 四条 needle）。既有九支测试逐字保留（只增不改） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs` | `archi_scaffold定义描述要素` 的 needle 集由四枚短标签换成词典措辞（`字段词典` / `逐字段一行纪律` / `值域` / `关系四键`），并加反向断言：描述零 `字段序` / `内联 metadata` / `path 必填提示` / `关系模板` |

### 不要修改

| 对象 | 理由 |
|---|---|
| `RawElement` 八字段 / `Element` 十一字段 / `Relation` 字段面 | 本变更零字段增删；`ELEMENT_FIELD_NAMES` 只是既有字段面的机器可读副本 |
| `DiagnosticKind` 九值 / `ArchiApiFindingKind` 十三值 / `REPORT_VERSION` | 零新词面；草稿零结构诊断词的既有锚逐字保留 |
| `crates/infra/agent/src/archi-knowledge.md` 与 `archi_knowledge_test.rs` | 知识骨架的字段纪律段压缩与「取词典」指路句归主篇 A；本变更零改动，`骨架_指路表逐句在场` 的 `("字段序", "archi_scaffold", "取模板")` 行保持绿（模板仍给字段序与可抄改草稿） |
| `crates/core/workflow/src/prompts/proposal-planner.md` / `dev-design-planner.md` | 「取新元素的 TOML 字段模板」句仍准确；「词典」措辞与 prompt 引用纪律归 A（`desktop-phase-prompts` 零 delta） |
| `.dev-team/architecture/graph/model/*.toml`、`graph/INDEX.toml`、`models/**`、`.schema.json`、`.i.rs` / `.i.ts` | 零模型 delta、零生成物 diff（两 crate 的元素都是 `status = "absent"`） |
| `packages/desktop/src/**`（含 `types/generated/**`） | 零 IPC / DTO / UI 变化；常量无 specta，`bindings:export` 应零 diff |
| `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` | crate 版本行 MUST NOT 跟随 `package.json`；`tauri.conf.json` 仍 `"version": "../package.json"` |
| `crates/core/archi/tests/corpus/**` 与金样 `expected/*.json` | 模板不在语料面，金样逐字节不变 |
| `openspec/specs/**`（基线）与 `openspec/changes/archive/**` | delta-only：本变更只写 2 份 delta，基线由归档合并 MODIFIED / ADDED 块收口 |
| `openspec/changes/archi-scaffold-field-dictionary/explore.md` | 只读输入 |

---

## 验收标准

1. **AC-1 · 三处值面缺口逐字补齐（needle 表，行首限定处已标注）**：`entry` 行含 `单值`（限定 `# entry：` 行）；`metadata` 块含 `值 MUST 是字符串`、`非字符串值被拒`、`子表与内联表等价`；`apis` 块含 `只写名字`、`可留空`、`逐项非空`、`同名不重复`、`显式空数组被拒`、`生成物面留空`、`archi_drift`，以及「`apis = []` 与省略同义」整句（needle 逐字含反引号：`` `apis = []` 与省略同义 ``）。**`invalidValue` 零命中**（保留既有零诊断词锚，见「问题 · 四」）。
2. **AC-2 · 字段面全集锚不空转**：草稿逐字段存在行首形态（`{f} = ` / `# {f}：` / `# {f} = `），断言遍历 `archi::parse::ELEMENT_FIELD_NAMES`、测试内零手抄八字面量；**反向可判**：手工删掉草稿的 `# api：` 行后该测试必红（`apis` 不得顶替 `api`），删掉 `kind = ` 实值行后同样必红。
3. **AC-3 · 常量与字段面同批**：`ELEMENT_FIELD_NAMES` 恰八项、顺序 = `RawElement` 声明序（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）；源码自证测试取 `struct RawElement { … }` 块内字段名并与常量同序同集比较——在 `RawElement` 增删字段而常量未同批更新时必红。
4. **AC-4 · 值域派生**：草稿逐值含 `GateTier::ALL` 三值（`off` / `warn` / `strict`）且 `off` 以「缺省」字样在场；逐值含 `ApiStatus::ALL` 二值（`declared` / `none`）且 `declared` 以「缺省」字样在场；`ElementKind::ALL` 四值语义的既有断言保持绿；新增任一变体未补词典即红。
5. **AC-5 · 既有锚全绿**：草稿经真实 `TomlParser` 对 `domain` / `package` / `composite` 三种 kind 各落盘一次均零结构诊断、恰一元素、`path` 未声明；草稿零结构诊断词；零写盘（调用前后盘面字节快照相同）；关系块全注释（零裸 `[[relation]]`）且四键齐备；响应经 `tools::cap_result` 单点、未触发截断（草稿体量 ≪ `MAX_RESULT_BYTES` = 50 000 B，实测同量级 ~4.5 KB）。
6. **AC-6 · 描述面改措辞且不再枚举字段名**：`archi_scaffold` 描述含 `字段词典`、`逐字段一行纪律`、`值域`、`关系四键`、`kind 省略取 domain`、`只读零写盘`、`write / edit`；零 `字段序` / `内联 metadata` / `path 必填提示` / `关系模板`（`mount_test` 同批改，正反两向都断言）。
7. **AC-7 · 零读数与零生成物变化**：模型 55 元素 / 129 关系、`status = clean` / `passed = true`、结构诊断 0 逐字不变；`pnpm api:check` 零差异退出（`INDEX.toml` / `.schema.json` / `.i.*` 均无 diff）；`pnpm bindings:export` 后 `git diff` 为零；`scan.*` 三项计数与变更前基线逐字段相同（零新增 / 删除文件）。
8. **AC-8 · 版本纪律**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch（0.7.9 → 0.7.10）；`src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行逐字不变。
9. **AC-9 · spec delta 形态**：恰 2 份 delta（`desktop-archi-agent` MODIFIED ×1 / `desktop-archi-model` ADDED ×1）；`### Requirement` 标题与基线逐字一致；每条 Requirement 至少一个 `#### Scenario:`（恰好 4 个 `#`），WHEN/THEN 成对；MODIFIED 块为基线整段拷贝后再扩展，基线四 Scenario 与未冲突条目逐字保留；两份 delta 各带 `## Module Contract` 章节。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **explore 字面与既有锚冲突**：explore §3.1 想把 `invalidValue` 写进 `metadata` 注释，与 `archi_scaffold草稿零结构诊断词` 正面冲突 | 已裁决「语义在场、词面不进」（「问题 · 四」）：保留既有机械锚，注释写行为描述「非字符串值被拒」。若下游坚持字面，唯一合法路径是**同批删除该锚并在本 delta 里明写降级理由**，MUST NOT 静默弱化或只在测试里放行一词 |
| R-2 | **`ELEMENT_FIELD_NAMES` 仍是第二份清单**：手写常量可能与 `RawElement` 漂移 | 位置贴在字段定义旁（新增字段时视线所及）+ 源码自证对拍（AC-3）+ 消费侧零手抄（AC-2）。三角里任何一角缺失都会让「新增字段即红」失效——三条是同一个锚的三面，MUST NOT 只落其中一条 |
| R-3 | **行首匹配器写歪**：用 `draft.contains(name)` 会被 `apis`（对 `api`）与 `单值`（对 `entry` vs `doc`）假命中，测试变成空转 | AC-2 / AC-1 明确规定行首形态集与行首限定，并要求测试 doc 写明假命中反例；`archi_scaffold草稿含字段面全集` 的注释 MUST 点名「`apis` 不顶替 `api`」 |
| R-4 | **描述改措辞牵动既有断言**：`mount_test` 的 `archi_scaffold定义描述要素` 钉住四枚短标签（`字段序` / `内联 metadata` / `path 必填提示` / `关系模板`）；知识骨架「字段序与形态用 `archi_scaffold` 取模板」句由 `archi_knowledge_test` 的 A 档指路表钉住 | `mount_test` 同批改（AC-6）；本变更**不动**知识骨架（「不要修改」表），该句仍准确，A 档指路表保持绿——A 改骨架时再同批换「取词典」 |
| R-5 | **模板体量膨胀**：本节补五条纪律，若形态失控会挤向 `cap_result` 上限 | 一行一字段的形态约束 + AC-5 的体量读数（实测 ~4.5 KB vs 50 KB）；超限时压缩同类表述，MUST NOT 删纪律 |
| R-6 | **词典被误当成 A 的全部迁出目标**：`path` 的 `dependsOnAncestor` 因果、空 `path` 分档告警、`hasPath` / `resolvedEntry` / `apiFindings` 工具指路不在词典里 | P2 的词典只接**值面纪律**；工具指路与判据长表仍归散文（A 的迁移表已分列：`entry` 三条纪律 → `resolvedEntry`、`doc` 判据 → `apiUnexpected`、`path` 缺省 → `hasPath`）。MUST NOT 期望 `archi_scaffold` 接管它们 |
| R-7 | **`pub const` 进公共面但不进契约资产**：`archi` crate 是 `api = declared` 的元素 | 两个相关元素在 `INDEX.toml` 的 `status` 均为 `absent`（无契约资产），常量不进 `types` / `ops` 计数，故 `api:check` 应零 diff（AC-7）。若将来补契约资产，常量是否入契约由 `desktop-archi-api` 判定——本变更不预判 |
| R-8 | **「字段词典」与「模板」两个名字并存**：工具描述写词典，`archi_module` 描述与骨架仍写「取模板」 | 两者不矛盾（词典同时是可抄改模板，草稿形态未变）；措辞统一归 A（同批改骨架与 prompt）。本变更只确保词典面自身先立得住 |

---

## explore 未决项与「不含」的归属

| explore 条目 | 处置 |
|---|---|
| §3.1 的三条 `apis` 纪律与 metadata 值域（含 `invalidValue` 字面） | **落地**；`invalidValue` 字面按既有零诊断词锚改写为行为描述（R-1） |
| §3.2 字段全集锚（方案 A：`const FIELD_NAMES` 贴字段定义旁；否决方案 B：serde 反序列化探测） | **按方案 A 落地**，常量命名 `ELEMENT_FIELD_NAMES`（`FIELD_NAMES` 在 `parse` 命名空间里指代不明），并经 `parse/mod.rs` 再导出 |
| §3.3 `archi_module` 响应附「该元素可改字段」 | **不做**（字段面已由 `archi_element` 参数 schema 承载） |
| §四 验收 1–7 | 全部收编为 AC-1…AC-5（第 1 条拆为 AC-2 / AC-3，第 2 / 3 条合入 AC-1，第 4 条即 AC-4，第 5 / 6 / 7 条合入 AC-5）；第 7 条的「50 KB 内」落成 AC-5 的体量读数 + 未截断断言 |
| §五 四条不做项 | 逐条照收（「提案 · 五」） |
| 本提案新增（explore 未列） | `entry` 单值（A 删散文后无归宿，见「问题 · 三」）、工具描述改词典措辞并去字段名枚举（「问题 · 二」的第二处手写清单） |
| 主篇 A 的指路句与知识骨架压缩 | **不做**：本变更只保证词典先就位；骨架 / prompt 的措辞统一归 A |
