# 测试设计: archi-model-path-and-silent-trap

> **日期**: 2026-11-24
> **输入**: 本 change 目录 `proposal.md`（AC-1…AC-13 / R-1…R-11）、`design.md`（「变更清单 - 修改文件」11 行、「公共函数 / API」3 行、「类型定义」3 行、D1–D14、事实校准 13、末节「交接给下游相位的随动清单」5 行）
> **性质**: 零新增测试文件（`scan.*` 三项计数不变，R-10）——全部用例落在既有 `*_test.rs` 内；零新增诊断词 / 漂移词（`DiagnosticKind` 仍九值、`ArchiDriftKind` 仍五值），故测试面只做「新判据 + 收窄 + needle 迁移 + 行为锚补齐」四件事。
> **路径口径**: `crates/**` 指 `packages/desktop/src-tauri/crates/**`；本文一律写仓库相对全路径。
> **同批约束**: L2（模型数据 `kind` 已落盘）与 L3（drift 收窄）同批交付 ⇒ 本节用例必须与已落盘模型一致（AC-4 / AC-5 的 55 / 129 / `model.warnings` 6 → 0 同批成立）。
> **回溯重做说明（AC-6 判据口径）**: 上一版本文的 AC-6 按「`crates/**` 与 `decisions/**` 全零命中 + `openspec/specs/**` 命中只允许出现在禁令句」设计，与两处硬事实冲突：① `crates/**` 的命中里有 `archi_knowledge_test.rs::骨架形态纪律四条在场` 的 `assert!(!source.contains(forbidden))` 列表——它是「骨架零假因果」的防线，测试计划本身要求它**在场**；② 基线 `openspec/specs/**` 在 delta-only 下不可手改，且字面规则会误伤 delta 内合法的直接否定。本版按 `design.md` D14 / 事实校准 13 改为**三面分判**（正向表述面零命中 / 防线面必须在场 / spec 面收窄到本 delta 且只许否定形态），判红看**该句是否把假因果陈述为事实**，不看同行是否含某个词。本文引用该短语集只为**定义判据面**（属禁令 / 描述语境，不是断言陈述）；`test-design.md` 不在 AC-6 的三个追改面内（正向面 = `crates/**` 与 `decisions/**`；spec 面 = 本 change 的 `specs/**`）。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 13 条 AC。被测文件或模块 = 承载用例的测试文件；
     纯静态 / 仓级读数 / 生成物 / 散文面 AC 落「—（见不可测试项 N）」，并写明替代保证。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | `package` 的 `path` 上界三方向成立：2 条 ⇒ **恰一条** `invalidValue` + 行号命中第 2 条 + 文案含「至多一条包根」；省略 `path` 零诊断；`module` / `domain` / `composite` 各 2 条零诊断。反向：`package` 1 条、`module` 5 条（`Desktop.Shell` 形态）全绿 | `packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs` |
| AC-2 | 停摆语义未被改写：带该诊断的模型经 `graph_view` 返回 `elements = []` / `relations = []`（不是「跳过坏元素、保留其余」）；`dto_test` 该分支由**独立**用例覆盖，既有「九类结构错一律停摆」表逐字不动 | `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs` |
| AC-3 | L3 豁免面精确：`domain` / `composite` 空 `path` 零 `noPath`；`package` / `module` 空 `path` 各记一条；`errors` / 顶层 `warnings` 不受影响；`ArchiDriftKind::ALL` 仍五值、`drift/mod.rs` 的 `model_warnings.push(` 仍两处 | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs`（分档用例 + `定级分支单点源码复核` 的 push 计数锚）、`packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs`（既有五值 / 线值用例维持即满足词汇面） |
| AC-4 | L2 零副作用：元素 55 / 关系 129、四类结构诊断零命中、`INDEX.toml` / `.schema.json` / `.i.*` 零 diff、`kind` 分布 `package` 19 / `composite` 3 / `domain` 3 / `module` 30 | —（见不可测试项 1：模型数据资产 + 生成物零 diff 门；行为面背书由 AC-1 的「`package` 0 条 `path` 零诊断」与 AC-3 的「`composite` 空 `path` 零警告」两例提供） |
| AC-5 | baseline 读数换段且三处同值：`model.warnings` 为空（0 条 `noPath`）；`docMissing` 39 / `observations` 53 / `status = clean` / `passed = true` 逐字不变；delta 内读数一律 55 / 129（后端 52 + 前端 77）且六文件段计数与基线一致 | —（见不可测试项 2：spec 文本判据 + 桌面端 `archi_drift` 读数；`passed` 布尔口径由 `drift/report_test.rs` 既有 `passed零error即真` / `passed三面各可阻断` 类用例维持） |
| AC-6 | case B 的假因果**按面分判**（判红看该句是否把假因果陈述为事实，不看同行词面）。短语集 = 四条骨架 forbidden needle（`读成另一个元素` / `读成**另一**元素` / `必须内联表` / `被读成元素`）＋等价旧文（`metadata` MUST 用内联表 / 必须内联表、禁子表）。① **正向表述面零命中**：`crates/**` 除防线单元之外的一切（生产源码与注释、`archi-knowledge.md`、`SCAFFOLD_TEMPLATE`、测试文件内防线单元之外的注释）＋ `.dev-team/architecture/decisions/**`。② **防线面必须在场（删了即红）**：`archi_knowledge_test.rs::骨架形态纪律四条在场` 的 `for forbidden in [四 needle]` + `assert!(!source.contains(forbidden), …)` 与其函数 doc comment——命中依据是**否定断言形态**，不属残留，MUST NOT 因 grep 洁癖删除，MUST NOT 退化成「同行含 `MUST NOT`」的文本匹配。③ **spec 面只许否定形态**：追改面收窄到本 change 的 `specs/**`，命中仅允许否定 / 禁令 / 删除语境且 MUST NOT 以断言形式陈述假因果；基线 `openspec/specs/**`、`archive/**`、`explore.md` 不在追改面、命中不计红、MUST NOT 手改。反向：删禁令句或删 needle 与留断言同判红（R-7） | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（② 防线面 + 骨架侧 ① 的单测可判面）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（模板侧 ① 的零结构诊断词面）；① 的仓库级语义扫描与 ③ 的 delta 文本面见不可测试项 3 |
| AC-7 | case B 每句断言都有行为锚：`parse/toml_test.rs` 恰有「未引号点分 → `unknownField` 全或无」「子表与内联表等价」「子表成为唯一声明 → `missingField`」三条行为用例；「断言 md 含某词」的用例不再以「静默」为名 | `packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs`（三条行为锚）、`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（用例改名 `骨架形态纪律四条在场`） |
| AC-8 | 知识文本与模板同步：骨架含 `path` 基数句、`composite` 标准形态句、键形真因果句且零结构诊断词（`md不列结构诊断全量词表` 保持绿——`unknownField` 不进骨架）；`SCAFFOLD_TEMPLATE` 的 `kind` 行四值齐列、`path` 行按新基数、`metadata` 行写等价；`archi_tool_test` 遍历 `ElementKind::ALL` 断言四值；两个体量锚按实测更新（骨架 **2596 B** / 全文 **2739 B**，LF 归一）且落 2300–2600 / 2450–2750、门限 2700 / 2850 **不抬高** | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（骨架三句 + 零词表 + 规范语义保留 + 两个体量锚）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（模板三行 + 四 kind 遍历） |
| AC-9 | 生成物与零 diff 门：`bindings:export` 后 `git diff` 只含 `bindings.ts` 的 `NoPath` 两处 JSDoc、`.zod.ts` 零 diff；`api:check` 零差异退出；`api:export` 对 `INDEX.toml` / `.schema.json` / `.i.*` 零 diff | —（见不可测试项 4：生成物面；「doc 改写不动线格式」由 `drift/report_test.rs` 的既有五值 / 拒识 / 往返用例背书） |
| AC-10 | 零行为与零分布扩散：`git status --porcelain` 改动集 ⊆ 提案「变更范围」且 ADR `0001-model-granularity.md` **在集内**；`openspec/changes/<change>/specs/` 恰 4 个能力目录、基线 `openspec/specs/**` 与 `archive/**`、`explore.md` 零改动；语料金样 20 份逐字节不变；`scan.*` 三项与基线逐字段相同；前端产品代码零改动 | —（见不可测试项 5：仓级改动集 / 语料金样 / `scan.*` 读数） |
| AC-11 | 版本纪律：`packages/desktop/package.json` 相对落地时 `HEAD` 恰 +1 patch；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本逐字不变 | —（见不可测试项 6：一行字面量，无行为可断言） |
| AC-12 | spec delta 形态：4 份 delta 的 7 条 `### Requirement` 标题与基线逐字一致；每条至少一个 `#### Scenario:` 且 WHEN/THEN 成对；MODIFIED 块为基线整段拷贝后再改 | —（见不可测试项 7：spec 文本面，不进单元测试） |
| AC-13 | ADR 0001 语义收敛：§2 存在同句含三容器指称 + 「分层容器 / 纯分组容器」+ `kind = "composite"`，且同段无「分层容器 × `package`」并置；§3 两组语义（`composite` 纯分组 / `domain` 业务聚合）与成员成对无交叉；全文「同为 `package`」「本仓当前无实例」零命中；与 `02` / `03` / `05` 实测 `kind` 一致 | —（见不可测试项 2：散文裁定记录，与 AC-5 同一表行的静态核对面） |

---

## 单元测试

<!-- 框架识别结论：`packages/desktop/src-tauri` Rust 套件（`cargo test`；用例为属性宏
     #[test] / #[tokio::test]，断言用 std 宏 + serde_json，零第三方断言库），测试文件为与源
     文件同目录共置的 *_test.rs，由 #[cfg(test)] mod <name>_test; 在 lib.rs / mod.rs 登记；
     另有一层 crates/core/archi/tests/** 集成套件（语料金样 / 词汇面），本变更不动它
     （AC-10 要求语料逐字节不变）。本变更零新增测试文件：全部用例进既有测试文件。

     既有断言面的硬约束（落地前必读，违反即红）：
     ① drift/mod_test.rs 以源文本计数 model_warnings.push( = 2 / errors.push( = 3 /
        warnings.push( = 6——L3 只能改条件表达式，不得增删 push 语句；
     ② archi_knowledge_test.rs::骨架含kind四值语义 要求 composite 行含「组合」——
        新增「纯分层容器是标准形态」时 MUST NOT 删既有语义词；
     ③ archi_knowledge_test.rs::骨架_指路表逐句在场 要求「缺 path」+ archi_module + hasPath
        同现——新 path 行 MUST 保留尾句「缺 path 看 archi_module 的 hasPath」；
     ④ archi_knowledge_test.rs::md不列结构诊断全量词表 禁 invalidValue / unknownField /
        missingField / syntaxError 等七词进骨架——新句只能用「整份源解析失败」「≥2 条即模型停摆」表述；
     ⑤ archi_knowledge_test.rs::骨架零字段形态教学 禁 path = [" 与 kind = "——新 path 行
        MUST NOT 引入赋值示例；骨架零围栏零示例零三档表 / 骨架零尖括号占位 同样维持；
     ⑥ archi_knowledge_test.rs::骨架规范语义在体量压缩下不被删 要求五处既有语义在场
        （`domain` 层可省略 / 指定语言 / 例：订单-通知 / 指针不扩张 / path 尾句）——体量压缩只许
        词级等量置换，不删纪律；
     ⑦ archi_knowledge_test.rs::骨架形态纪律四条在场 的四条 forbidden needle 与其 doc comment
        属 AC-6 的**防线面**：命中是判据不是残留，删了与留断言同判红（R-7），MUST NOT 为了
        正向面 grep 的洁癖删 needle，也 MUST NOT 把它改成「同行含 MUST NOT」的文本匹配；
     ⑧ 既有用例名含 ASCII 驼峰（如 noPath / path）时沿用同文件既有写法补 #[allow(non_snake_case)]。

     design 末节「交接清单」5 行的落地面（逐行对应，无遗漏无扩张）：
     parse/toml_test.rs（三条行为锚 + `path` 基数用例 + 一例拆分 + 既有 200 项 path 段 kind 修正）/
     dto_test.rs（独立停摆用例）/ drift/mod_test.rs（分档用例 + 既有计数锚维持）/
     archi_knowledge_test.rs（改名 + needle + 防线面保留 + 两个体量锚）/
     sdk/archi_tool_test.rs（四 kind + 两 needle + 四值语义）。
     跨模块组合用例一律挂最上层调用方章节：停摆链路（parse 新判据 → parse_dir → graph_view）挂 dto.rs；
     `path` 归属与豁免链路（check → graph_view → scan）挂 drift/mod.rs；不新建集成章节、不新建测试文件。 -->

### packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs

<!-- 挂 AC-1（path 基数三方向 + 反向判据）、AC-7（三条行为锚逐条行为化）。 -->

#### 待测功能

- `TomlParser::parse()`（`SourceParser` 实现）: 诊断来源由「serde + `apis` 值面」扩为「serde + `apis` 值面 + `path` 基数」；`package` ≥2 条 `path` ⇒ 恰一条带位置的 `invalidValue`，0 条 / 1 条与 `module` / `domain` / `composite` 零诊断；签名与 `ParseOutcome` 形状逐字不变，失败臂仍 `Model::empty()`
- `validate_path()`（私有，经 `parse` 行为面观测）: `kind == Package` 且 `path.len() >= 2` ⇒ 一条 `invalidValue`（位置 = 第 2 条 `Spanned`，文案含「至多一条包根」并给出路：0 条 = 纯容器 / 多条 `path` 属 `module`），其余 `None`
- 行为锚三条（case B 随动，同属 `TomlParser::parse` 的行为面；来源 = design 末节交接清单第 1 行 + AC-7）：`metadata` 子表与内联表等价 / 子表成为唯一声明即缺 `kind`（`missingField` + 全或无）/ 未引号点分表头 → `unknownField` + 全或无

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TomlParser::parse()` · `path` 基数 | 正向 | `package` 恰 1 条 `path`（本仓 19 个元素的形态）→ 零诊断，元素在场且 `path` 逐字保留（`./` 前缀 / 尾 `/` 不归一）——`package一条path逐字保留且零诊断` | 单点 |
| `TomlParser::parse()` · `path` 基数 | 正向 | `package` 省略 `path` 与显式 `path = []` 两形态（纯容器，迁 `composite` 前的三容器形态）→ 各零诊断、`path == []`、元素照常入模型（「省略」与「空数组」等价）——`package零条path合法` | 矩阵迭代（2 形态） |
| `TomlParser::parse()` · `path` 基数 | 正向 | `module` 2 条与 5 条 `path`（后者即 `Desktop.Shell` 形态）→ 零诊断、条目数与逐项值全量保留（不设上界、不截断）——`module多条path合法` | 矩阵迭代（2 规模） |
| `TomlParser::parse()` · `path` 基数 | 正向 | `domain` 与 `composite` 各 2 条 `path` → 两形态各自零诊断（选填容器不设上界），`path` 逐字保留——`domain与composite多条path合法` | 矩阵迭代（2 值） |
| `validate_path()` | 异常 | `package` 写 2 条与 3 条 `path` → **恰一条**诊断：`kind == DiagnosticKind::InvalidValue`、`line` = 第 2 条 `path` 所在行（`line_at` 定位，非第 3 条、非表头行）、`column` 为 `Some`、文案含「至多一条包根」且两句出路在案（纯容器可留空 / 多条属 `module`）——`package多条path被拒且位置命中第二条` | 单点 |
| `validate_path()` | 异常 | `package` 写 200 条 `path`（超大清单）→ 仍**恰一条**诊断（`Option` 即「至多一条」，不逐条铺开），位置恒为第 2 条——`package两百条path仍恰一条诊断` | 单点 |
| `TomlParser::parse()` · 不跳过元素 | 异常 | `package` 2 条 `path` 与 2 个健康元素（`module` / `domain`）同文件 → 诊断恰一条且 **3 个元素全部入模型**（`elements.push` 不被跳过；「不跳过」的纪律只在 `graph_view`）——`package越界诊断不跳过任何元素` | 单点 |
| `validate_path()` | 边界 | 第 2 条 `path` 取值矩阵：`""` 空串 / >1000 字符超长 / 含 `\n` 与 emoji 转义 / 目录尾 `/` → 判据只按**基数**不按内容：各自仍恰一条、位置恒在第 2 条、两条 `path` 逐字保留不截断——`package第二条path取值矩阵只按基数` | 矩阵迭代（4 形态） |
| `validate_path()` | 边界 | 两条 `path` 布局矩阵：同行 `path = ["a", "b"]` 与跨行数组（每条一行）→ 行号分别为该行 / 第 2 条自身行，两形态都不落到表头行（span 精确性）——`package两条path布局矩阵行号不落表头` | 矩阵迭代（2 布局） |
| `validate_path()` · kind 分档 | 边界 | `ElementKind::ALL` 四值 × 2 条 `path` 全矩阵（四值经 `ALL` 派生，不手抄字面量）→ 仅 `Package` 出诊断，`Domain` / `Module` / `Composite` 零诊断——`path基数按kind分档四值全矩阵` | 矩阵迭代（4 值） |
| `TomlParser::parse()` · 非法值域 | 异常 | `kind = "pakage"`（词表外）→ 既有 `invalidValue` 且文案列出四个合法值；新判据不改 serde 层分类学（`classify` 零改）——`非法枚举值归_invalid_value_且列出全部合法值` | 既有（维持） |
| `TomlParser::parse()` · 既有冲突面 | 边界 | `字符串值边界_超长与转义与数组规模均逐字保留` 的 200 项 `path` 段：原 `kind = "package"` 与 L1 **正面冲突**（该段断言零诊断）→ 该段 kind 定为 **`module`**，以保持「200 项 `path` 全量保留」的既有语义；MUST NOT 改成断言被拒（那会丢掉「超大数组逐字保留」这层原意）——`字符串值边界_超长与转义与数组规模均逐字保留` | 既有（必改） |
| 行为锚① `metadata` 等价 | 正向 | `metadata子表与内联表等价`（**新增**）：`metadata = { owner = "core", tier = "1" }` 内联表源 vs `[element."App".metadata]` 子表源（写在已声明元素之后）→ 两者各零诊断；`App` 的 `metadata` 键值逐项相同、`kind` / `path` / `doc` 逐字段相等；元素集恰 `{App}`（不出现 `…metadata` 元素） | 矩阵迭代（2 形态） |
| 行为锚② 非字符串值 | 异常 | `metadata非字符串值被拒`（**由既有用例拆出**）：`metadata = { n = 1 }` 与 `{ flag = true }` → `invalidValue` + parser 原文含 `invalid type`（断言逐字留，不加工文案） | 矩阵迭代（2 形态） |
| 行为锚③ 子表唯一声明 | 异常 | `metadata子表成为唯一声明即缺kind`（**由既有用例拆出**）：`[element."App.Auth".metadata]` 为**唯一**声明 → `missingField` + 全或无（`model.elements.is_empty()`）；doc 注释去「被读成元素 `App.Auth.metadata` 的父表」假因果（AC-6 的第 9 处持有者，属正向表述面） | 单点 |
| 行为锚④ 未引号点分 | 异常 | `未引号点分表头被拒且不与合法形态合并`（**逐字不动**，它是 AC-7 第一条锚）：`[element.App.Auth]` → `unknownField` + 文案点名 `Auth`；非法与合法形态并存时恰一条诊断且元素集空（全或无） | 既有（维持） |

> **既有用例冲突（设计期实测，非推定）**：`字符串值边界_超长与转义与数组规模均逐字保留` 的 200 项 `path` 段原源文本为 `kind = "package"` + `path = [<200 项>]` 并断言零诊断——L1 一落地该断言即红。design「事实校准 8」记「`crates/**` 内零 `package` + ≥2 条 `path`」与该处不符（全仓仅此一处；其余多条 `path` 的既有用例 kind 均为 `domain` / `module`）。本设计按上表第 12 行处置：**改该段 kind、保 200 项逐字保留**，语料与金样仍零改动（`tests/corpus/**` 的 `package` 元素全为 0 / 1 条）。
>
> 用例名与 design 交接清单同一批语义：清单第 1 行写作 `package两条path被拒且位置命中第二条`，落地名为 `package多条path被拒且位置命中第二条`（同一判据覆盖 2 条与 3 条两规模），其余名称逐字一致。
>
> 新诊断的 `file` 回填走既有 `parse_dir` 逐文件回填路径（`parse/mod.rs`，零改）——本节因「只吃 `&str`、不经盘」的文件纪律不新增落盘用例，回填面由 `dto_test.rs` 的组合用例（见下一节第 3 行）在停摆路径上断言。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `TomlParser::parse` 只吃 `&str`；`toml` crate 的原文案与 span 即被测契约（替身会抹掉判据），用例内零落盘、零 `TempDir` | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

<!-- design.md 未声明该文件的公共 API 变更（graph_view / load 零改，停摆分支逐字不动）；
     本节用例由 design 末节交接清单第 2 行与 AC-2 指定——只钉「停摆语义未被改写」，
     不为其新增公共导出、不为既有「九类」账扩容。 -->

#### 待测功能

- `graph_view()`（零改，承接新判据的停摆收口）: `diagnostics` 非空 ⇒ `elements` / `relations` 清空（全或无的唯一收口点，非「跳过坏元素保留其余」）；`package` 两条 `path` 的新诊断走同一分支

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `graph_view()` | 正向 | 健康模型（零诊断，含显式空值字段）→ `elements` / `relations` 非空且稳定序、`diagnostics` 空——与停摆用例同入口的对照组：「清空」只在诊断非空时发生，新判据不放宽也不误触发——`graph_view健康模型稳定序与显式空值` | 既有（维持） |
| `graph_view()` | 异常（跨模块组合：`parse` 新判据 → `parse_dir` → `graph_view`） | 模型目录唯一文件写 `package` 2 条 `path` → `graph_view` 返回 `Ok`（不 `Err`），`elements` / `relations` 皆空，`diagnostics` 恰一条 `invalidValue`（**不是**跳过坏元素、保留其余）——`graph_view包根越界停摆` | 单点 |
| `graph_view()` | 边界 | 同一非法文件再加 2 个健康元素（`module` / `domain` 各 1，均 0 条 `path`）→ `elements` 仍全空（全或不与健康元素共存），`diagnostics` 仍恰一条（健康元素不冲淡诊断条数）——`graph_view包根越界不与健康元素共存` | 单点 |
| `graph_view()` | 边界 | 两文件模型：`01.toml` 写该越界元素、`02.toml` 健康 → `diagnostics` 恰一条且 `file` 回填为出错文件（`parse_dir` 逐文件回填在停摆路径上仍生效），`elements` 空——`graph_view包根越界诊断file回填到出错文件` | 单点 |
| `graph_view()` | 边界 | 既有 `graph_view九类结构错一律停摆` 的用例名与十行表**逐字不动**（MUST NOT 扩容成「十一类」；新判据另立独立用例名，D9） | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实临时目录（`std` 自建 `TempDir` + `write_model`）+ 真实 `parse_dir` / `ModelGraph` 组合；投影只裁剪，替身会引入第二事实源 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

<!-- 挂 AC-3（豁免面精确：分档用例 + 既有计数锚），并保护 R-10 的「改动落既有文件、不新建文件」。
     `path` 归属与豁免链路（check → graph_view → scan）的组合用例挂本节（最上层调用方 = check）。 -->

#### 待测功能

- `check()`: ④ 模型警告段的 `noPath` 入面按 `kind` 收窄——`package` / `module` 空 `path` 仍各记一条，`domain` / `composite` 静默；`message` 字符串逐字不动，`continue` 对所有空 `path` 元素保留（不进 path 存在性循环）；`errors` / 顶层 `warnings` / `status` / `passed` / `pathNotFound` 段与其余口径逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check()` · `noPath` 仍报面 | 异常 | `package` 空 `path`（模型唯一元素）+ 一个源文件 → `model.warnings` 恰一条、`kind == ArchiDriftKind::NoPath`、`from == Some("App")` / `to == None` / `gate == Some(Off)`、`errors` 空、`status == clean`（模型警告不进 `status` / `passed`）——`package空path仍记模型警告` | 单点 |
| `check()` · `noPath` 豁免面 | 正向 | `domain` 空 `path`（模型唯一元素）+ 同前缀源文件 → `model.warnings` 空、`errors` 空、`status == clean`；与「同输入形态的 `package` 孪生」对拍，读数差**恰好只在** `model.warnings` 的一条（元素数 / 顶层 `warnings` / `scan.unmatched_files` 三项相同）——`domain空path零模型警告` | 单点 |
| `check()` · `noPath` 豁免面 | 正向 | `composite` 空 `path`（模型唯一元素）+ 一个源文件 → `model.warnings` 空、`errors` 空、`status == clean`——`composite空path零模型警告` | 单点 |
| `check()` · `noPath` 分档矩阵 | 边界 | 四 kind 各一元素（`App` package / `App.Domain` domain / `App.Mod` module / `App.Comp` composite）全部空 `path` → `model.warnings` **恰两条**，条目 `from` 集恰 `{App, App.Mod}`（`domain` / `composite` 零条目），`errors` 空、`status == clean`——`四kind空path只有package与module记警告` | 矩阵迭代（4 值） |
| `check()` · 归属面（组合：check → scan） | 边界 | `domain` / `composite` 空 `path` 元素与同前缀源文件共存 → `scan.unmatched_files` 含该文件（豁免不认领文件、不参与归属），且零 `noPath`、`errors` 空——`domain与composite空path不认领文件` | 矩阵迭代（2 值） |
| `check()` · 与 `pathNotFound` 共存 | 边界 | `domain` 空 `path` + 另一元素（`module`）`path` 指向不存在文件 → `model.warnings` 恰一条且 `kind == PathNotFound`（无 `noPath` 混入），`errors` 空——`domain空path与路径失效共存` | 单点 |
| `check()` · 既有 `noPath警告` | 边界 | `noPath警告`（`Ghost` = `module`）**逐字不动**：一条 `noPath` + `errors` 空 + `scan.unmatched_files` 含 `src/a.ts`（收窄后的「仍报面」正好由它钉住） | 既有（维持） |
| `check()` · 源文本计数锚 | 异常 | `定级分支单点源码复核` 的源文本断言**逐字不动**：`model_warnings.push(` = 2 / `errors.push(` = 3 / `warnings.push(` = 6 → 收窄只许改条件表达式，MUST NOT 增删 push 语句或改写 `message` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` 落模型 / 源文件（判定链路的输入事实即被测语义），真实 `dto::graph_view` 与真实两套归属解析器 | 本节全部用例 |
| `foundation::cancel::CancelFlag` | 不 mock——用既有 `CancelFlag::never()`（与生产同一构造，未引入第二实现） | 全部 `check` 调用 |

### packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs

<!-- design.md 未声明该文件的公共 API 变更（仅类型面 doc-only：`NoPath` 变体行 + 枚举级总述尾句），
     可观测面落在生成物 bindings.ts 的 JSDoc（见不可测试项 4）。本节只维持两侧不变项：词汇五值与线格式。
     条目取自 design「类型定义」子表（公共 API 的类型面），未做源文件导出扫描。 -->

#### 待测功能

- `ArchiDriftKind`（含 `NoPath`，doc-only 改写）: 变体集仍五值、`ALL` 序（`NoPath` 居第四）、`as_str` 线值 `noPath`、serde camelCase 往返逐字不变；doc 改写不得进入线形

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiDriftKind` | 正向 | 既有 `漂移词汇五值` 逐字不动：`ALL.len() == 5`、五值 `as_str` 词面（含 `noPath`）、camelCase 序列化与往返恒等 | 既有（维持） |
| `ArchiDriftKind` | 异常 | 既有 `漂移词汇外反序列化即错` 逐字不动：`"undeclaredAPI"` / `"UndeclaredApi"` / `""` / `"other"` 全部反序列化失败（零第六值） | 既有（维持） |
| `ArchiDriftEntry` / 报告段 | 边界 | 既有 `模型面字段面往返` / `完整报告往返` / `字段线形camel_case` 逐字不动 → doc 改写不改条目键集与线形（`noPath` 条目 `from: Some` / `to: None` / `gate: Some` 形态仍可往返） | 既有（维持） |

> doc 文本本身无运行时可断言面（`as_str` / serde 不读 doc），故本文件**零新增用例**；两处 doc 改写经 specta 落 `bindings.ts` 的 JSDoc，判据见 AC-9（不可测试项 4）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 纯枚举 + `serde_json` 真实序列化 / 反序列化，零 IO 零进程边界 | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- design.md 未声明该文件的公共 API 变更（静态资产 `include_str!`，零函数面）：
     条目取自 design「变更清单」该文件行 + 末节交接清单第 4 行 + AC-6 / AC-7 / AC-8
     （needle 承载串与体量锚冻结值由该清单单点持有）。
     挂 AC-6（防线面必须在场 + 骨架侧正向面）、AC-7（用例名去「静默」）、AC-8（三句在场 + 零结构诊断词 + 两个体量锚）。 -->

#### 待测功能

- 知识骨架文本（`include_str!("archi-knowledge.md")`，经 `knowledge()` 拼装）: `path` 基数三口径（模块必填可多条 / 包至多一条包根、纯容器可留空、≥2 条即模型停摆 / `domain`·`composite` 选填且空 `path` 零告警），尾句「缺 `path` 看 `archi_module` 的 `hasPath`」保留
- 键形真因果句: 未引号点分 → 整份源解析失败（诊断点名非法子键）；`metadata` 子表与内联表等价（成为唯一声明才缺 `kind`）；零结构诊断词
- `composite` 语义: 补「纯分层容器是标准形态」且保留既有「组合」「可嵌套任意类型之上」语义词
- C 档形态纪律四条（正向 needle）与其 forbidden needle 的**否定断言**（防线面，AC-6 ②：四条 needle 列表与函数 doc comment MUST 在场，删了即红）
- 体量锚: 骨架 **2596 B** / 全文 **2739 B**（LF 归一），区间与门限不抬

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 骨架 · `path` 基数句 | 正向 | `骨架含path基数三口径`：`path` 行同时含「模块必填」「可多条」「至多一条包根」「纯容器可留空」「≥2 条即模型停摆」「选填」「空 `path` 零告警」七枚 needle，且点名两个豁免 kind（`domain` / `composite`）；尾句「缺 `path` 看 `archi_module` 的 `hasPath`」仍同现 | 单点 |
| 骨架 · `composite` 语义 | 正向 | `骨架含composite纯分层容器句`：含「纯分层容器是标准形态」，且既有「组合」「可嵌套任意类型之上」两语义词不被顶替 | 单点 |
| 骨架 · 键形真因果句 | 正向 | `骨架含键形真因果句`：含「整份源解析失败」「诊断点名非法子键」「子表与内联表等价」「唯一声明才缺 `kind`」四枚 needle（`metadata` 子表**合法**这一点 MUST 留在场——R-7：删「等价」与删「子表合法」都判红） | 单点 |
| 骨架 · 形态纪律与防线面 | 异常 | `骨架形态纪律四条在场`（用例名去「静默」）：四条正向 needle（`带引号的完整 FQN 单键` / `子表与内联表等价` / `目录尾 /` / `dependsOnAncestor`）逐条在场；同时 `for forbidden in [四 needle] { assert!(!source.contains(forbidden)) }` 恒成立。四条 needle 列表与函数 doc comment **MUST 在场**（AC-6 ② 防线面），MUST NOT 用 `unknownField` 作 needle（词表禁令 ④），MUST NOT 退化成同行 `MUST NOT` 文本匹配 | 矩阵迭代（4 正向 + 4 forbidden） |
| 骨架 · 零结构诊断词 | 异常 | `md不列结构诊断全量词表` 保持绿：遍历 `DiagnosticKind::ALL`，除 `orphanHierarchy` / `dependsOnAncestor` 外逐值零命中，且无「- 结构诊断：」词表行——新 `path` 行与键形句不得引入 `invalidValue` / `unknownField` / `missingField` | 矩阵迭代（9 值，2 豁免） |
| 骨架 · 形态禁形 | 边界 | `骨架零字段形态教学`（零 `path = ["` / 零 `kind = "` 等赋值示例）、`骨架零围栏零示例零三档表`、`骨架零尖括号占位`、`骨架零项目专用命令` 四例维持——新增句 MUST NOT 引入示例围栏或赋值示例 | 既有（维持） |
| 骨架 · 既有规范语义保留 | 边界 | `骨架规范语义在体量压缩下不被删`：五处既有语义（`domain` 层可省略 / 指定语言 / 例：订单-通知 / 指针不扩张 / `path` 尾句）逐字在场——体量压缩只许词级等量置换，不删纪律（R-5） | 矩阵迭代（5 句） |
| 骨架 · 指路表 | 边界 | `骨架_指路表逐句在场` 的十行表**逐字不动**（第一行仍「缺 `path`」+ `archi_module` + `hasPath`）——改 `path` 行 MUST NOT 拆散该三元组 | 既有（维持） |
| 骨架 · `骨架体量锚` | 边界 | 冻结值 **2596 B**（LF 归一、`trim_end`），断言仍为「落 2300–2600 B」+「≤ 2700 B」——区间与门限 **MUST NOT 抬高**；净增长超限时压缩同段冗余而非删纪律 | 单点 |
| 全文 · `全文体量锚` | 边界 | 冻结值 **2739 B**，断言仍为「落 2450–2750 B」+「≤ 2850 B」；全文锚与骨架锚同批更新（两值互为约束） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!` 取编译期内嵌的同一份字节（禁测试内复制副本）；`knowledge()` 真实拼装；`ElementKind::ALL` / `DiagnosticKind::ALL` 取真实常量，不手抄词表 | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`SCAFFOLD_TEMPLATE` 私有、`scaffold_draft` 签名与出参零改）：
     条目取自 design「变更清单」该文件行 + 末节交接清单第 5 行 + AC-8（needle 承载串：
     `module 必填` / `子表与内联表等价`；`kind` 四值语义）。可观测面 = `archi_scaffold` 工具出参。
     模板侧假因果属 AC-6 正向表述面：由「等价句在场」与「零结构诊断词」共同兜住（文本面无单测读全仓）。 -->

#### 待测功能

- `SCAFFOLD_TEMPLATE` / `scaffold_draft()`（经 `archi_scaffold` 工具出参观测）: `kind` 行四值齐列（`package` = 包（至多一条包根 `path`，纯容器可留空）/ `domain` = 领域容器 / `module` = 模块 / `composite` = 纯分组（纯分层容器是标准形态））；`path` 行按新基数（含字面 `module 必填`，删「容器（package / domain）恒空」）；`metadata` 行写「子表与内联表等价」（删「子表会被读成另一个元素」）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_scaffold` · 四 kind 逐值 | 正向 | `archi_scaffold四kind逐值`（原名去「三」）：遍历 `ElementKind::ALL`（`kind.as_str()` 派生，不手抄四值）→ 每个 kind 的草稿含 `kind = "<值>"` 实值行且零 `__KIND__` 残留 | 矩阵迭代（4 值） |
| `archi_scaffold` · 默认 kind 与字段序 | 正向 | `archi_scaffold省略kind取domain且字段序齐备` needle 迁移：`叶元素必填` → `module 必填`、`内联表` → `子表与内联表等价`；七个字段序注释行（`# gate` / `# api` / `# entry` / `# path` / `# doc` / `# summary` / `# metadata`）仍齐备、省略 kind 仍取 `domain` 实值行 | 既有（改 needle） |
| 模板 `kind` 行四值语义 | 正向 | `archi_scaffold模板kind行四值语义`：语义词随 `ElementKind::ALL` 派生——`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组，且「纯分层容器是标准形态」在场 | 矩阵迭代（4 组） |
| 模板 `path` 行出路 | 边界 | `archi_scaffold模板path行出路`：`path` 行同时含 `module 必填`（可多条）、「纯容器可留空」（`package` 0 条合法）、「≥2 条即模型停摆」三口径；且**不含**旧口径「容器（package / domain）恒空」 | 单点 |
| 模板零结构诊断词 | 异常 | `archi_scaffold草稿零结构诊断词`：草稿不含 `invalidValue` / `unknownField` / `missingField` / `syntaxError`（与骨架 `md不列结构诊断全量词表` 同源纪律）——新三行 MUST NOT 引入诊断词 | 矩阵迭代（4 词） |
| `archi_scaffold` · 草稿零结构诊断 | 边界 | `archi_scaffold草稿零结构诊断` 维持并扩 kind：`domain`（默认）/ `package` / `composite` 三种草稿各落盘为唯一模型文件 → `module_overview` `Ok`、恰 1 元素、`has_path == false`（`path` 仍是注释行） | 既有（维持 + 扩 kind） |
| `archi_scaffold` · 零写盘 | 边界 | `archi_scaffold零写盘` 维持：调用前后盘面字节快照逐一相同 | 既有（维持） |
| `archi_scaffold` · 关系块 | 边界 | `archi_scaffold关系块全注释` 维持：零裸 `[[relation]]` 表、四键（`from` / `to` / `desc` / `apis`）以注释齐备——模板三行改写 MUST NOT 波及关系段 | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `tempfile::TempDir` + 真实 `ArchiContext` 与 `execute_json`（工具出参即被测契约）；模板是字面常量，替身只会抹掉 needle 判据 | 本节全部用例 |
| 模型 / 语义面 | 不 mock LLM、不走网络：`archi_scaffold` 是纯只读工具调用，零 provider 依赖 | 本节全部用例 |

---

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| 1 · `.dev-team/architecture/graph/model/02-backend-core.toml` / `03-backend-infra.toml` / `05-backend-tools.toml`（各 1 行 `kind = "composite"`） | 架构模型资产：声明式 TOML，无函数 / 分支 / 输出可断言；仓内无害模型 golden 断言（`archi` 既有测试全用合成 fixture，不读仓内模型目录） | AC-4 的分布（`package` 19 / `composite` 3 / `domain` 3 / `module` 30）与 55 / 129 由桌面端 `archi_drift` 读数核对；三容器「`path` 仍空」的合法性由 `parse/toml_test.rs` 的「`package` 0 条 `path` 零诊断」与 `drift/mod_test.rs` 的「`composite` 空 `path` 零警告」两例在行为面背书；生成物零 diff 由 `api:export` / `api:check` 门兜底 |
| 2 · `.dev-team/architecture/decisions/0001-model-granularity.md`（§2 / §3 改写）+ `repo-archi-model` delta 的三处 `model.warnings` 读数 | 决策记录与 spec 是散文资产，无代码单元可断言；AC-5 的读数一致性、AC-13 的文本判据（§2 同句三容器 + `kind = "composite"`、§3 两组语义成对、全文零旧说法）均属文本面 | 桌面端逐条核对 AC-5 / AC-13 判据 + `archi_drift(full)` 读数复核（`model.warnings` 0、`elements` 55 / `relations` 129、`observations` 53、`apiFindings` 39、`status = clean`、`passed = true`）+ ADR 与三个模型文件的 `kind` 实测一致性比对（文本与数据同批） |
| 3 · AC-6 的三面判据（① 正向表述面的仓库级语义扫描、③ 本 delta 的 spec 文本面） | ① 不是可执行单元：把短语集写成「测试读全仓源码」会越过单元测试边界（且会把别的 `*_test.rs` 的防线单元拖进判定面）；③ 是 4 份 delta 的散文，只能按语义逐句读，不能以「同行含 `MUST NOT`」的字面规则替代语义判据。**② 防线面不属本项**——它恰是单元测试可判的部分，已落在 `archi_knowledge_test.rs::骨架形态纪律四条在场` | ① 骨架 / 模板两处静态资产由单测兜住：`骨架形态纪律四条在场`（骨架的四条 forbidden needle 否定断言）+ `骨架含键形真因果句` / `骨架零字段形态教学` / `md不列结构诊断全量词表`（骨架正向面）+ `archi_scaffold草稿零结构诊断词` / `archi_scaffold省略kind取domain且字段序齐备`（模板正向面）；其余源码与 `decisions/**` 由桌面端 static-check 按**正向面列举式** grep 零命中（`| grep -v '_test.rs'` 只是下限，不是唯一判据）。③ 由桌面端把 grep 收窄到本 change 的 `specs/**` 并逐行读句归面（8 处命中全为否定 / 禁令语境），基线 `openspec/specs/**` / `archive/**` / `explore.md` 零改动、零追改。**反向**：任何为通过 grep 删除 needle / doc comment / 禁令句的动作与留断言同判红（R-7） |
| 4 · `packages/desktop/src/types/generated/bindings.ts`（`NoPath` 两处 JSDoc）与 `bindings.zod.ts` | `bindings:export` 派生生成物，MUST NOT 手改；其内容正确性由生成器 + 重导 diff 判定，非单元测试可断言 | `drift/report_test.rs` 的 `漂移词汇五值` / `漂移词汇外反序列化即错` / `模型面字段面往返` 钉住「doc 改写不动线格式」；AC-9 由桌面端重导后在 `git diff` 上核对（只含两处 `NoPath` JSDoc、`.zod.ts` 零 diff）+ `api:check` 零差异退出 |
| 5 · AC-10 的仓级机械面（`git status --porcelain` 改动集 ⊆ 白名单且 ADR 在集内、`openspec/specs/**` 与 `archive/**` 零改动、语料金样 20 份逐字节不变、`scan.*` 三项 422 / 359 / 422 / 0 / 0、前端产品代码零 diff） | 仓级状态与目录读数不是可执行单元；金样与 `scan.*` 由语料套件与 `archi_drift` 产出 | 桌面端 test-execution / static-check 执行 `crates/core/archi/tests/**` 全量语料套件（20 份金样逐字节断言在既有测试内）+ `git diff --stat -- tests` 零输出核对 + `scan.*` 读数复核 |
| 6 · `packages/desktop/package.json`（`version` 0.7.8 → 0.7.9）与 `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` 的版本行 | 一行字面量，无行为可断言 | AC-11 由桌面端 `git diff` 核对（`package.json` 恰 +1 patch、`tauri.conf.json` 仍 `"version": "../package.json"`、`src-tauri/Cargo.toml` 版本行逐字不变） |
| 7 · AC-12 的 spec delta 形态（4 份 delta / 7 条 `### Requirement` 标题逐字一致 / 每条至少一个 `#### Scenario:`） | spec 文本面，不是可执行单元；本变更不改写 delta | 桌面端核对 7 条标题清单（`架构模型文件格式与字段封闭` / `文件归属与两套解析器一套归属逻辑` / `archi_scaffold 只读工具的新元素模板契约` / `架构会话的系统知识注入` / `知识分层与工具面同批移动` / `基线落位与模型文件划分` / `图自洽与停摆禁令`）+ Scenario 计数 |
| 8 · `drift/report.rs` 的两处 doc 文本本身 | Rust doc comment 无运行时读面（`as_str` / serde / `ALL` 均不读 doc）；为其加断言只能复制一份 doc 文本作第二事实源 | 由 AC-9 的生成物 diff 判据（`bindings.ts` 两处 JSDoc）单点持有 |
| 9 · 私有件：`validate_path`（`parse/toml.rs`） | 私有函数不进公共面；为其加 `pub` 或 test-only 导出只服务测试（AGENT.md「no test-only exports」），且会扩大 `parse.rs` 的导出面 | 经 `TomlParser::parse` 行为面全覆盖（`invalidValue` 种类、位置、文案、0 / 1 / 2 / 3 / 200 条基数与四 kind 矩阵均由公共 `parse` 断言） |
