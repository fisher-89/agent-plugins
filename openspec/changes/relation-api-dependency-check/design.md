# 设计: relation-api-dependency-check

> **变更**: relation-api-dependency-check（`[[relation]]` 第四键 `apis` + 名字级两桶判定 + 第 9 条命令 `archi_relation` + 右栏面板两态）
> **日期**: 2026-11-20
> **基于**: `proposal.md`（问题四条 + 提案六件 + 事实校正九处 + 决策表 D1–D18 + AC-1…AC-13 + R1–R12）、`explore.md`（§0–§12 + 附录 12.2 的 S1/S2/S3 三句）、`specs/` 下 8 份 delta（`desktop-archi-model` / `desktop-archi-drift` / `desktop-archi-canvas` / `desktop-archi-api-check` / `desktop-archi-api` / `repo-archi-model` / `desktop-archi-agent` / `desktop-ipc-type-bindings`）
> **性质**: **粒度升级 + 只读取数面扩一条命令**。把「实际依赖」从元素对粒度升到符号粒度：接住名字（扫描）→ 拿到声明（模型第四键）→ 按引入方档位对差（两桶）→ 按需取（第 9 条命令）→ 点开看（右栏两态面板）。前置是 `types/generated/` 的中继元素归属（否则 12 条最强名字面永远判不了）。报告**形状零变化**（`version` 仍 3、16 份金样零变化）、**判定单源**（求差抽成纯函数）、**画布零 watch 零轮询**。
> **不含**: Rust 工作区清单发现（D18，本仓 49 条后端关系的 `used` 仍为零）；⊆ 导出集机械证成（护栏②）；内联限定路径取数（G2）；标识符集旁证；生成链对账；边失配徽标第五值；报告 facts 段 / `version` 4；`openspec/todo.md`。

---

## 起点实况（`HEAD` 基线实测）

| 面 | 实测 |
|---|---|
| 字段面（关系） | `parse/toml.rs:58` `RawRelation { from, to, desc }`（`desc: Option<String>`，`deny_unknown_fields`）；`model.rs:72` `Relation { from, to, description, file, line, span }`；`parse/toml.rs:94` `to_model(raw, source) -> Model`（**只出模型、不产诊断**——诊断只来自 serde 错误） |
| 扫描事实 | `scan/mod.rs:42` `ImportFact { specifier, line, type_only }`；`:56` `DependencyEdge { from, to, file, line, specifier, type_only, inline_ignore }`；`:239` 成边处逐字段透传；`:233` 目标不可归属即 `continue`（丢弃） |
| Rust 取数 | `scan/rust.rs:91` `extract`（逐 `Item::Use` → `flatten_use_tree` 产路径字符串）；`:584` `flatten_use_tree`（`Rename` 丢弃别名、`Glob` 记模块路径）；`:129` `resolve`（最长模块前缀 → 文件，签名与语义要零改动） |
| TS 取数 | `scan/ts.rs:52` `extract`：`ImportDeclaration`（全 type 才 `type_only`）/ `ExportFromDeclaration` / `ExportAllDeclaration` 三分支各产一条 `ImportFact`，**specifier 之外零字段** |
| 判定面 | `drift/mod.rs:87` `check`；`:175` `buckets`（按 `(from,to)` 聚合）；`:270` ① 观察项；`:418` 私有 `occurrences_of`；`derive_status` 只读 `errors` / `warnings` / 纳管子集 |
| 词汇 | `drift/report.rs:93` `ArchiDriftKind`（四值，`ALL` 在 `:116`）；`:128` `ArchiObservationKind`（二值，`UnusedDeclaration` 注释「本期不产出」）；`REPORT_VERSION = 3`（`drift/mod.rs:42`） |
| DTO | `dto.rs:436` `ArchiRelation { from, to, description, mark, reason }`；`:632` `to_relation`；`:603` **私有** `load`（`ModelGraph::build` 唯一调用点）；`:667` 私有 `diagnostics_error`；`:708` `graph_view`；`:735` `module_detail` |
| 命令面 | `commands/archi/mod.rs` 文件头「八命令」+ 八个 `*_inner`；`commands/mod.rs:87–94` archi 段 8 条（末条 `archi_api_generate`，其后 `read_file`） |
| 清单守卫 | `bindings/mod_test.rs:127` / `:187` 各 **56** 条；`:247` `DTO_TYPES` **104**；`:1852` `ARCHI_COMMANDS` **8**；`:1864` `ARCHI_DTO_TYPES` **21**；`:1915` 尾部序断言（`app_environment` 之后 `… archi_api_generate, read_file`） |
| 前端画布 | `architecture-canvas.tsx:210` `toEdges`（`id = '<from>-><to>'`，无 label / 无 `onEdgeClick`）；`:299` `ModelRelationGraph`（只有 `onNodeClick`）；`:338` `CanvasBody`（`CardHeader` 只渲 `ModelInfo`）；`:408` `ArchitectureCanvas`（`driftMarks(report, graph)` → `marked`） |
| 前端面板 | `model-info.tsx:328` `ModelInfo`（元素态）；`:262` `EntryDocSection`（`FileViewer` + 通用 `read_file` 的既有范式）；`lib/drift-marks.ts` 已把 `suppressed[]` → `exempt` + `reason`（**不带 `via`**） |
| 模型数据 | `.dev-team/architecture/graph/model/06-frontend.toml` 表头「30 元素 / 72 关系」；实测 **31 元素 / 74 关系**；`to = "App.Backend.Shell"` 的前端关系 **12** 条（11 条 `desc = "经生成的命令面类型取数与调用"` + `SessionPanel` 一条「…取会话清单」）；`api = "none"` **2** 处、显式 `entry` **3** 处、`doc` **0** 行（落位后：表头 32 / 74、`to = "App.Frontend.GeneratedBindings"` 12 条、`api = "none"` 3 处，见「落地面」段） |
| ADR | `decisions/0001-model-granularity.md` `§1` 53/121、`§2` 「`components/ui/` 的 39 个文件」、`§3` barrel 端点边 **7** 条（3 出 + 4 入）、`§6` 「53 个元素」、`§7` 「映射 → `App.Backend.Shell`，落 `observations`」、`§8` 命令面边段、`§9` 53（容器 9 + 叶 44）/ 121 / 06(30) / 前端 72 = 65 + 7 |
| 索引 | `.dev-team/architecture/graph/INDEX.toml` 45 条 `[[entry]]`（`status = "declared"` **10** 条 = 10 份在盘契约 md，其余 **35** 条 `absent`；落位后 46 条 = 10 + 36，见「落地面」段） |
| 报告基线 | `reports/archi/20261120T1042330799149Z.json`：`version = 3`、`model.elements = 54`、`relations = 123`、`byGate.off = 54`、`observations = 62`（全 `unfulfilledDeclaration`）、`apiFindings = 35`（全 `docMissing`）、`model.warnings = 9`（全 `noPath`）、`warnings = []`、`scan.scannedFiles = 412` / `unmatchedFiles = 412` / `excluded = 341` / `matched = 0`（`gate` 全 `off` 的必然结果）、`status = clean` |
| 知识文本 | `crates/infra/agent/src/archi-knowledge.md` 实测 **2143 B**；`archi_knowledge_test.rs` 冻结骨架 2142 B / 全文 2632 B，并断言骨架落 `2000..=2150`、全文落 `2550..=2750`（门限 2252 / 2867） |
| 版本 | `packages/desktop/package.json` = `0.7.4`；`tauri.conf.json` = `"version": "../package.json"`；`src-tauri/Cargo.toml` 自立版本不随动 |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **`dto::load` / `diagnostics_error` 是私有的**：`relation_detail` 需要 `Relation.file` / `line`（`graph_view` 的 `ArchiRelation` 不带）与「结构错即 `Err`」的文案单点。两处按 `to_element` 的先例提 `pub(crate)`（`Loaded` 两字段同步 `pub(crate)`），**不新增读盘路径**（`ModelGraph::build` 仍只在 `load` 一处）。
2. **`occurrences_of` 是私有的**：`archi_relation` 的 `occurrences` 要与报告逐字段同形（按 `file` / `line` 排序的 `ArchiDriftOccurrence`）。提 `pub(crate)` 复用，**不另拼第二份聚合形状**。
3. **`ImportFact` 无构造函数**：名字的「去重 + 字典序」若散在 Rust / TS 两个抽取器里就是两处规则。加 `ImportFact::new(...)`（`pub(crate)`）作为**唯一归一化点**，`DependencyEdge` 原样透传。
4. **`to_model` 当前不产诊断**：`apis` 的值面四查需要「带位置的 `invalidValue`」，而 serde 层只能在类型错时给出位置——`deserialize_with` 里的 custom 错误通常不带 span（退化为 `line = 0`），无法满足「带位置」的口径。故 `to_model(raw, source) -> (Model, Vec<Diagnostic>)`，逐项用 `Spanned<String>` 的 span 换算行列；`parse` 把两份诊断合并。停摆语义不变（`graph_view` 见诊断非空即返回零图 + 诊断清单）。
5. **`ArchiRelation.apis` 是 drift 侧的必需输入，不只是画布便利**：`drift::check` 从 `dto::graph_view` 拿的是 `ArchiGraph`（DTO），`graph.relations` 就是 `ArchiRelation`；两桶装配只能从该字段取声明。⇒ 字段进场是判定面的**前置**，不是 UI 优化；故实现序把它与 `Relation.apis` 同批（模型字段阶段）落地，`ArchiRelationDetail` / `relation.rs` / 命令面 / 生成物留在命令面阶段（见 D1）。
6. **`edit.rs` 零改动**：`archi_set_gate` 走 `toml_edit` 文本级改写并重解析自证，`apis` 行原样保留；若同文件另有非法 `apis`，改档会因诊断非空而 `Err`（与既有「渲染自证」同一条纪律）。
7. **`diff.rs` 零改动**：结构 diff 的关系身份是 `(from, to)` 计数（`relation_counts`），`apis` 进不进去都不改变读数——与「明确不做：模型结构 diff 覆盖 `apis`」一致。
8. **`scan` 的 `gate = off` 分支不变，但扫描面计数必然随新增文件变**：本仓 `matched` 仍为 0、3 个生成文件仍计入 `unmatchedFiles`（`off` 元素与无主同视）。12 条命令面边之所以兑现，是因为 `owned` 对 `off` 元素**照常抽取**、`to` 归属从 `Owner::None` 变为 `Owner::Element { GeneratedBindings }` —— MUST NOT 用 `matched` / `unmatchedFiles` / `excluded` 的计数验收归属或判定面零变化。**扫描面按条目级账目验收（AC-8，MUST NOT 写成「三面逐条相同」）**：`unmatchedFiles[]` 本就收纳 `gate = off` 的归属文件与无主文件（本仓 `gate` 全 `off` ⇒ 全部源文件在此），`excluded[]` 收纳既有排除规则命中项 ⇒ 本方新增的 3 个源文件必然进 `unmatchedFiles[]`（412 → 415）、11 份测试 / 语料必然进 `excluded[]`（341 → 352）；既有条目零增删改（规则集与既有文件集合零 diff），新增条目全部命中既有规则（逐条见「落地面」§5），候选 `scannedFiles + excluded` 753 → 767 自洽（+14 = 3 源 + 11 测试 / 语料）。
9. **空态③ 的 `via` 无现成通路**：`ArchiRelation` 只有 `mark` / `reason`，`lib/drift-marks.ts` 逐字不变、`EdgeMark` 零扩张。故 `via` 由**画布**从已取到的 `report.suppressed[]` 按 `(from, to)` 查一次，作为 `RelationInfo` 的 prop 传入（零新 invoke、零第二套判定、`drift-marks.ts` 与 `ArchiRelation` 的形状零改）。
10. **豁免边在现模型面不可达**：`suppressed[]` 只承载**未声明元素对**（声明即第 1 层豁免，`continue`），而画布只渲染模型声明的边 ⇒ `mark = 'exempt'` 的边实际上不会出现。空态③ 仍按 spec 实现并排好优先级（见 D14），但本变更 MUST NOT 借它去改 drift 的豁免层级。
11. **「取数失败降级时选中边清空」只指图面**：`archi_relation` 自身失败必须留在面板行内（空态④ 才有意义），故清空条件取「`root` 变化 / 图停摆 / 图取数失败 / 选中对已不在图内」，**不含** `archi_relation` 失败（见 OQ-3）。
12. **`Relation` 字面量夹具面**（字段进场的机械面，测试相位随动）：`diff_test.rs:39`、`graph_test.rs:34` `:46`、`model_test.rs:214` `:224` `:236`、`dto_test.rs:871`（`ArchiRelation`）。产品代码侧只有 `parse/toml.rs:119` 一处构造。
13. **`archi_scaffold` 草稿的关系模板仍是三键**（`from` / `to` / `desc` 全注释三行）：`apis` 是选填键，模板不列不构成矛盾；本变更按 proposal「变更范围」不触碰 `sdk/archi_tool.rs`（见 OQ-4）。
14. **知识文本的体量是硬约束**：现行冻结 2142 B + 断言区间 `2000..=2150`，而本次增量约 +60–130 B ⇒ 必须在同段压缩等量冗余（合并「点分嵌套 / metadata 子表」两句、删「（`archi_module` 报错）」等旁注），**净增长后仍落回区间**并同批更新冻结值（2142 → 新实测值、2632 → 新实测值）；MUST NOT 抬高区间或门限。
15. **`bindings/mod_test.rs` 的注释仍写「38 条命令」**（实际 56 条）：既有陈旧，本变更不在追改面内（proposal「读数写实边界」），计数断言与尾部序断言按新值改。
16. **ADR 的内部一致性面比 proposal 点名的段更宽**：`§3` 的「端点边恰 7 条（4 条入边）」与 `§9` 的「前端 72 = 65 + 7」是同一事实，只改 `§9` 会让单文件内两种自称并存；`§2` 的「39 个文件」与本次要改写的 `repo-archi-model` 场景（实测 **41** 条 `export *`）是同一读数。故设计把 `§2` / `§3` / `§6` 一并纳入写实面（各一行 / 一句），理由即 R2 的「同一事实两种自称」。
17. **既有 spec 的历史读数不追改**（`desktop-frontend-module-entry` 的「71 条 / 24 个源 / 44 个 `path` 非空」、`desktop-archi-contract-panel` 的「43 条存量」）：沿 proposal「读数写实边界」，它们是各自变更交付时的读数。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 实施序 | 中继元素（数据面）→ 模型字段（`Relation.apis` + `ArchiRelation.apis` 同批）→ 名字事实 → 两桶判定 → 命令面（`ArchiRelationDetail` + `relation.rs` + 生成物）→ UI → 收口（知识 / ADR / 版本） | 与 proposal D1 / explore §9 的 T0–T6 一致；`ArchiRelation.apis` 是两桶装配的读取面（事实校准 5），MUST NOT 排到命令面阶段（否则两桶装配无字段可读）；每阶段可独立编译、独立复核 |
| D2 | 中继元素 | `App.Frontend.GeneratedBindings`（`module` / `off` / `api = "none"` / `path = ["./packages/desktop/src/types/generated/"]` / 无 `entry` / 无 `doc` / 无 `metadata` / 零出边）；12 条 `to` 改指 + `desc` 微调 | 生成目录无 `index.ts` ⇒ 入口 `Absent`，`api = none` 已退出入口判据 ⇒ 零 `apiEntryMissing`；`off` 只关定级、不关事实（12 条边由 `Owner::Element` 命中即兑现） |
| D3 | 字段面 | `RawRelation.apis: Option<Vec<::toml::Spanned<String>>>`（`#[serde(default)]`）+ `Relation.apis: Vec<String>`；`deny_unknown_fields` 逐字保留 | 与 `doc` / `entry` 同族的「拼错键即 `unknownField`」；`Spanned` 保住值面违例的行列 |
| D4 | 值面五拒 | ① 空串 ② 含空白（任一 `char::is_whitespace`）③ 含 `::` ④ 同一关系内同名重复 ⑤ `apis = []`；逐条 `invalidValue` + 位置，**模型整体停摆**（`graph_view` 零图 + 诊断） | 机械可判、文案点名四条值面；MUST NOT 静默去重 / 静默截断 / 把空数组当省略 |
| D5 | 名字事实（Rust） | `flatten_use_tree` 一次产出「路径 + 可选尾段」：`Name` → 末段；`Rename` → **被引入名**（`node.ident`，非 `rename`）；`Glob` → 无名；末段恰为 `self` → 无名。`resolve` 签名与行为零改动（名字不走前缀匹配） | 尾段在 AST 里本就有；别名取原名与既有 `Rename` 语义同向；`self` / glob 不产假名 |
| D6 | 名字事实（TS） | `ImportDeclaration` 的 `ImportSpecifier` 取 `imported` 名（`ModuleExportName::Identifier` 或 `StringLiteral` 字面值；`type` 前缀只是 `import_kind`，**照产名**）；`ExportFromDeclaration` 的具名 `ExportSpecifier` 取 `local` 名；`ImportDefaultSpecifier` / `ImportNamespaceSpecifier` / `ExportAllDeclaration` **不产名** | 默认导入绑的是本地变量名、命名空间与全量重导出没有可枚举导出名——静默产名会造出指向不存在导出名的假阳 |
| D7 | 名字归一点 | `scan::normalize_names(impl IntoIterator<Item = String>) -> Vec<String>`（去重 + 字典序）为唯一实现；`ImportFact::new(...)` 调用它；`DependencyEdge` 透传 `fact.names` | 「去重 + 字典序」只有一处，Rust / TS 两个抽取器不各写一份 |
| D8 | 两桶与档位 | `used ∖ declared` → `ArchiDriftKind::UndeclaredApi`（`ALL` 4 → 5），按 `from.gate`：`strict` → `errors` / `warn` → `warnings` / `off` → 不出现；`declared ∖ used` → `ArchiObservationKind::UnusedDeclaration`（观察项，永不进 `status` / `passed`）；关系 `apis` 为空 → 两桶皆空、零产 | 不对称是取数精度的直接后果（import 是硬事实，方法调用 / 内联路径 / glob 看不见）；「没看见」不能判红 |
| D9 | 求差单点 | `drift::api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)`：两侧去重 + 字典序，逐字符比对（禁大小写折叠 / 驼峰归一）；drift 与 `relation_detail` 只调它 | 命令层零领域规则；AC-5 的源码复核锚 |
| D10 | `used` 聚合 | `drift::edge_names(&[&DependencyEdge]) -> Vec<String>`（`pub(crate)`，内部转调 `scan::normalize_names`），drift 两桶装配与 `relation.rs` 共用 | 同一元素对多文件命中去重后只算一项；两消费面对同一对输出逐字相同 |
| D11 | 命令行文 | `archi_relation(root, from, to) -> Result<ArchiRelationDetail, String>`；只取 `ScanOutcome.edges`，MUST NOT 组装 `Report`、MUST NOT 产出 `status` / `passed`、零缓存、零写盘 | 报告只承载结论（门禁用），事实面按需付费 |
| D12 | 定向扫描 | `changed` 白名单 = 「归属为 `from` 元素」的源文件（`OwnerIndex` 同一归属规则 + 既有 `walk_source_files` 单次遍历），`mode = Incremental`、`cancel = CancelFlag::never()`、`exclude` 经既有 `drift::config::load_config` 单点 | 「按需事实取样，不是一次检查」；`from` 侧十来个文件 = 毫秒级 |
| D13 | 面板取数 | 声明清单取图里的 `ArchiRelation.apis`（零取数）；实际使用 / 两桶 / 命中点 / 模型行号走 `archi_relation`；`apis` 为空 ⇒ hook 入参 `null` ⇒ **零 invoke** | 未声明就没有可比对的清单，取数只是空转 |
| D14 | 空态优先级 | ①`archi_relation` 失败（行内错误 + 重试）> ②`mark === 'exempt'`（已豁免 + `via` + `reason`）> ③`apis` 为空（未声明依赖 API（选填））> ④`occurrences` 为空（扫描面无该关系的 import）；「取数中…」是独立 loading 态、不属空态 | 四态彼此可辨（各自 testid 与文案）；③ 优先于 ④ 是因为「没声明」比「没命中」更前置 |
| D15 | `via` 通路 | `ArchitectureCanvas` 从 `report.suppressed[]` 派生 `Map<"from->to", 'config' \| 'inline'>`，经 `CanvasBody` 传到 `RelationInfo.exemptVia` | `drift-marks.ts` 与 `ArchiRelation` 形状逐字不变；零新 invoke、零重算 |
| D16 | 选中态与清空 | `selectedEdge: { from, to } \| null`（身份 = 元素对）；`onEdgeClick` 置选中、`onNodeClick` 置 `focus` 并清选中；`root` 变化清空；选中对已不在 `graph.relations` 时按「无选中」渲染（派生失效，不用副作用）；`/architecture` 的显式刷新经 `key={refreshTick}` 重挂载自然复位 | 与既有 edge `id = '<from>-<to>'` 同身份；零 `useEffect` 抖动（不让 `driftMarks` 产出的新对象清掉选中） |
| D17 | hook 形状 | `useArchiRelation(root, edge)`：`Map<key, ArchiRelationDetail>` 引用缓存（key = `root\0from\0to`），命中即不 invoke；失败不写缓存、保留 `error`；`retry()` 删键并 bump tick | 「同一元素对重复选中零重复 invoke」跨两态切换仍成立（hook 挂在 `CanvasBody` 而非面板内，缓存不随面板卸载丢失） |
| D18 | 报告零变化 | `REPORT_VERSION = 3`、`ArchiDriftReport` 字段集与条目键集逐字不变；名字只进 `message`；`UnusedDeclaration` 只改注释与产出路径 | 16 份金样 + 绑定 + 前端载荷语料零 churn 的前提 |
| D19 | 文案口径 | `undeclaredApi` message 点名多出的名字与引入方档位；`unusedDeclaration` message **并列两种成因**（可能已不再依赖 / 也可能名字写错或该依赖以方法调用、内联限定路径、glob 形态存在而扫描看不见），并点名「本对无 import 时另有 `unfulfilledDeclaration` 一条」 | R4 / R5 的补偿；MUST NOT 只写「没用」 |
| D20 | 知识文本与体量 | 关系键面补 `apis` + 一行纪律（只写名字 / 可留空 / 生成物面留空 / 结论看 `archi_drift`）；同段压缩约 100 B 冗余使骨架净增长后仍落 `2000..=2150`；`apis` 不进任何派生词表 | 体量锚是冻结断言；`apis` 是开放值集（同 `doc` / `entry`） |
| D21 | 版本纪律 | `packages/desktop/package.json` `0.7.4` → `0.7.5`；`tauri.conf.json` 跟随 `../package.json`；`src-tauri/Cargo.toml` 版本行逐字不变 | AGENT.md 版本纪律 |
| D22 | 零扩散白名单 | 允许出 diff：本 change 目录文稿与 8 份 delta、`06-frontend.toml`、`decisions/0001-model-granularity.md`、`graph/INDEX.toml`（生成物）、`parse/toml.rs`、`model.rs`、`scan/{mod,rust,ts}.rs`、`drift/{mod,report,report_test}.rs`、`dto.rs`、`relation.rs`（新）、`lib.rs`、`commands/archi/mod.rs`、`commands/mod.rs`、`bindings/mod_test.rs`、`types/generated/{bindings,bindings.zod}.ts`（生成物）、`use-archi-relation.ts`（新）、`relation-info.tsx`（新）、`architecture-canvas.tsx`、`archi-knowledge.md`、`package.json`、`models/**/*.schema.json`（生成物，零变化：新元素 `api = "none"` 不产 schema、既有 10 份字节不变）；测试相位承接面：`crates/infra/agent/src/archi_knowledge_test.rs`、`parse/toml_test.rs`、`model_test.rs`、`graph_test.rs`、`diff_test.rs`、`dto_test.rs`、`scan/{rust,ts}_test.rs`、`drift/*_test.rs`、`relation_test.rs`（与 `lib.rs` 的 `#[cfg(test)] mod` 同批）、`commands/archi/mod_test.rs`、`tests/corpus/drift/**`（只加新 case，既有 16 份金样零改）、`tests/golden_test.rs`（`DRIFT_CASES` 登记四新 case）、`architecture-canvas.test.tsx`、`model-info.test.tsx`（`relation()` 夹具补 `apis: []`）、`relation-info.test.tsx`（新）、`use-archi-relation.test.ts`（新）、`src/lib/ipc-contract.test.ts` 与 `views/architecture/**` 既有 DTO 夹具 | 出现白名单外 diff 即回退；`drift-marks.ts` / `marks.ts` / `model-info.tsx` / `owner.rs` / `edit.rs` / `diff.rs` / `scan/exclude.rs` / `api/**` 零 diff 是判据之一 |

**三条「必须同批写进 spec 的说明句」的落点**（explore 附录 12.2）：S1（关系 `apis` 是消费侧的名字清单、不是协议正文）→ D3 / D4 与 `desktop-archi-model` 的字段面四键；S2（`declared ∖ used` 的两种成因不可机械分辨）→ D8 / D19 与 `desktop-archi-drift` 的显式豁免；S3（边点击的按需查询由显式用户动作发起、不计入画布三触发）→ D12 / D16 / D17 与 `desktop-archi-canvas` 的开窗段。

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 字段面（改） | `apis` 第四键：`Spanned` 反序列化 + 值面五拒（`invalidValue` + 位置） | `crates/core/archi/src/parse/toml.rs` | `toml` / `serde` | `deny_unknown_fields` + 落模型期逐项校验 |
| 模型 AST（改） | `Relation.apis: Vec<String>`（原文序，零归一）；`file` / `line` / `span` 零变化 | `crates/core/archi/src/model.rs` | — | 纯结构体 |
| 扫描事实面（改） | `ImportFact` / `DependencyEdge` 各加 `names`；`normalize_names` + `ImportFact::new` 单一归一化点 | `crates/core/archi/src/scan/mod.rs` | `ignore` | `BTreeSet` 去重 + 字典序 |
| Rust 取数（改） | `use` 树一次产出「路径 + 尾段」；别名取原名、`self` / glob 不产名 | `crates/core/archi/src/scan/rust.rs` | `syn` | `UseTree` 递归 |
| TS 取数（改） | 具名 import / `export … from` 产名；default / namespace / `export *` 不产名 | `crates/core/archi/src/scan/ts.rs` | `oxc_parser` / `oxc_ast` | AST 访问 |
| 判定面（改） | `api_buckets` 求差单点、`edge_names` 聚合单点、两桶装配（档位只看引入方） | `crates/core/archi/src/drift/mod.rs` | `crate::dto`（DTO 投影）、`crate::scan` | 纯计算 + 既有定级分支 |
| 词汇与报告（改） | `ArchiDriftKind` + `UndeclaredApi`（`ALL` 5）；`UnusedDeclaration` 产出点亮；`version` 仍 3 | `crates/core/archi/src/drift/report.rs` | `serde` / `specta` | 封闭枚举 + `ALL` 常量 |
| 投影（改） | `ArchiRelation.apis`（末尾追加）；新 `ArchiRelationDetail`；`load` / `Loaded` / `diagnostics_error` 提 `pub(crate)` | `crates/core/archi/src/dto.rs` | `specta` | 投影零改写 |
| 关系详情（新） | 模型面（声明 + 声明行）+ 定向扫描事实 + 两桶 + 命中点 | `crates/core/archi/src/relation.rs` | `dto` / `scan` / `drift` / `foundation::cancel` | 纯读盘、零写盘、零缓存 |
| 出口面（改） | `pub mod relation` + `relation_detail` 出口（`relation_test` 模块随测试相位同批落地） | `crates/core/archi/src/lib.rs` | — | 模块声明 |
| 命令面（改） | `archi_relation` + `archi_relation_inner`（root 校验 → `archi::relation::relation_detail` → 错误透传） | `packages/desktop/src-tauri/src/commands/archi/mod.rs` | `archi` | Tauri 命令薄包装 |
| 命令登记（改） | `all_commands!` archi 段 8 → 9（尾部追加于 `archi_api_generate` 之后、`read_file` 之前） | `packages/desktop/src-tauri/src/commands/mod.rs` | — | 宏清单 |
| 清单守卫（改） | 四处计数随动 + 尾部序期望 vec + `ARCHI_DTO_TYPES` 插位 | `packages/desktop/src-tauri/src/bindings/mod_test.rs` | `tauri-specta` 产物 | 逐名命中 + 长度连带核对 |
| 前端取数（新） | 按需取数 hook：选中对变化即取、同对零重复 invoke、失败保留 `error` + `retry` | `packages/desktop/src/views/architecture/hooks/use-archi-relation.ts` | `commands.archiRelation` | React hook（零 watch 零轮询） |
| 前端面板（新） | `RelationInfo`：七段 + 四空态 + 行内错误与重试 + 模型行号「查看」 | `packages/desktop/src/views/architecture/components/relation-info.tsx` | `FileViewer` / UI 原语 / hook 状态 | React |
| 前端画布（改） | `onEdgeClick` + 选中态 + `CardHeader` 两态 + `via` 通路 | `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | `drift-marks`（零改） / `RelationInfo` / hook | React + xyflow |
| 知识文本（改） | 关系四键 + `apis` 一行纪律；同段压缩保体量 | `crates/infra/agent/src/archi-knowledge.md` | `include_str!` + 派生段 | 静态 markdown |
| 数据面（改） | 中继元素 + 12 条改指 + 表头；ADR 读数写实；`INDEX.toml` 重导 | `.dev-team/architecture/graph/model/06-frontend.toml`、`decisions/0001-model-granularity.md`、`graph/INDEX.toml` | — | TOML / markdown |

依赖方向零新增边：`core/archi` 仍只依赖 `foundation`，`infra/agent` → `archi` 既有边不变，命令层仍只依赖 `archi` / 既有 store / vcs 缝。`archi` crate 零新依赖。

### 数据流（改后）

```text
graph/model/*.toml ──parse_dir──> Relation { …, apis: Vec<String> }（值面五拒在 to_model 产出 invalidValue）
                                        │
                     scan（Rust / TS 抽取器）──> ImportFact { specifier, line, type_only, names }
                                        │  （ImportFact::new 去重 + 字典序）
                                        ▼
                 OwnerIndex 最长前缀 ⇒ DependencyEdge { from, to, file, line, specifier, type_only, inline_ignore, names }
                                        │
        ┌───────────────────────────────┴───────────────────────────────┐
        ▼                                                               ▼
  drift::check（一次扫描，报告）                              relation_detail（按需取样）
  dto::graph_view → ArchiRelation.apis                       同一 OwnerIndex 取 from 侧白名单
  for 每条 apis 非空的关系：                                  scan(Incremental, changed = 白名单)
     used = edge_names(该对边)                                  used = edge_names(该对命中边)
     (u∖d, d∖u) = api_buckets(apis, used)                       (u∖d, d∖u) = api_buckets(apis, used)
     u∖d → errors / warnings（按 from.gate）                    → ArchiRelationDetail（含 occurrences）
     d∖u → observations[]（永不进门禁）
```

---

## 数据模型 (Data Model)

### 模型声明面（手写真相，持久化 = TOML 文件）

```toml
[element."App.Frontend.GeneratedBindings"]     # 新增，位置：Components.Ui 之后、首个 [[relation]] 之前
kind = "module"
gate = "off"
api = "none"
path = ["./packages/desktop/src/types/generated/"]
summary = "IPC 命令面与出线 DTO 的 TypeScript 投影（tauri-specta 生成，勿手改）"

[[relation]]                                   # 既有 12 条，只改 to 与 desc
from = "App.Frontend.Views.Architecture"
to   = "App.Frontend.GeneratedBindings"        # 原 App.Backend.Shell
desc = "经生成的 IPC 绑定取数与调用"            # 原「经生成的命令面类型取数与调用」
apis = ["AgentInput", "SessionPanel"]          # 选填示例；本仓基线一律省略（生成物面留空）
```

字段与约束：

| 对象 | 字段 | 类型 | 缺省 | 约束 |
|---|---|---|---|---|
| `[[relation]]` | `from` / `to` | string | — | 必填；元素须在模型内（缺席即 `unknownReference` 结构诊断） |
| | `desc` | string | `null` | 选填 |
| | `apis` | string[] | `[]` | 选填；**非空串 / 无空白 / 无 `::` / 同关系内不重复**；显式 `[]` 即 `invalidValue`；开放值集、不进封闭词汇对拍；`to` 元素的代码导出名（逐字符比对） |
| `App.Frontend.GeneratedBindings` | — | — | — | 只作 `to`（零出边）；无 `entry` / 无 `doc`；`api = none` 已退出入口判据 |

### 进程内 AST

| 类型 | 字段变化 | 说明 |
|---|---|---|
| `Relation` | `+ apis: Vec<String>` | 原文顺序保留；去重 / 排序只在 `api_buckets` 内做 |
| `ImportFact` | `+ names: Vec<String>` | 空 = 取不到名字（glob / `self` / default / namespace / `export *`） |
| `DependencyEdge` | `+ names: Vec<String>` | 与 `ImportFact.names` 同值透传 |

### 出线面（`bindings.ts` / `bindings.zod.ts`，生成物）

| 类型 | 形状 | 说明 |
|---|---|---|
| `ArchiRelation` | `{ from, to, description, mark, reason, apis }` | `apis` **末尾追加**（既有字段零位移）、字段不省略、顺序按模型原文 |
| `ArchiRelationDetail` | `{ from, to, description, mark, reason, modelFile, modelLine, declared, used, usedNotDeclared, declaredNotUsed, occurrences }` | 新增；`occurrences: ArchiDriftOccurrence[]`（复用既有形状） |

### 判定产物（报告，形状零变化）

| 载体 | 变化 | 持久化 |
|---|---|---|
| `errors[]` | 新增取值 `kind = "undeclaredApi"`（键集与既有条目逐字相同，名字进 `message`） | 门禁面落 `reports/archi/<timestamp>.json`（运行期产物，不入库） |
| `observations[]` | `kind = "unusedDeclaration"` 由空位转产出（`ALL` 长度不变） | 同上 |
| `version` | 仍 `3` | 不变 |

`modelHash` 必然变化（`06-frontend.toml` 新增元素行、12 条关系 `to` / `desc`、表头注释）；元素计数 54 → 55、关系计数不变（123）——MUST NOT 写成「模型零变化」。

---

## 路由 / API 设计 (Route / API Design)

架构域无 HTTP 面；契约面是 Tauri IPC 命令（`Result<T, String>`、显式 `root`、无 auth 概念）。

| 命令 | 入参 | 出参 | 语义 / 约束 | 写盘 |
|---|---|---|---|---|
| `archi_relation` | `root: string`（必填，非空目录） / `from: string`（FQN） / `to: string`（FQN） | `ArchiRelationDetail` | 模型面（`from` / `to` / `description` / `mark` / `reason` / `modelFile` / `modelLine` / `declared`）+ 事实面（`used`）+ 两桶（`usedNotDeclared` / `declaredNotUsed`）+ `occurrences`。元素或关系缺席即 `Err` 并点名；结构诊断非空即 `Err`（附诊断清单）；**零写盘、零缓存、不组装报告、不产出 `status` / `passed`** | 无 |

响应示例（字段序由生成器决定）：

```jsonc
{
  "from": "App.Frontend.Views.Architecture",
  "to": "App.Frontend.GeneratedBindings",
  "description": "经生成的 IPC 绑定取数与调用",
  "mark": "none",
  "reason": null,
  "modelFile": ".dev-team/architecture/graph/model/06-frontend.toml",
  "modelLine": 512,
  "declared": ["AgentInput", "SessionPanel"],
  "used": ["AgentInput", "SessionPanel", "commands"],
  "usedNotDeclared": ["commands"],
  "declaredNotUsed": [],
  "occurrences": [{ "file": "packages/desktop/src/views/architecture/components/archi-chat-panel.tsx",
                    "line": 12, "specifier": "@/types/generated/bindings", "typeOnly": true }]
}
```

---

## 变更清单 (Change Inventory)

> 下表 `crates/**` / `src/**` 均指 `packages/desktop/src-tauri/` 之下；`src/views/**` / `src/types/**` 指 `packages/desktop/src/` 之下。proposal「变更范围 - 实现文件」的 23 行逐行覆盖，另加设计侧必需的可见性 / 归一化点（事实校准 1–3）、ADR 一致性面（事实校准 16）、`models/**/*.schema.json` 生成面登记（零新增、既有 10 份字节不变）与测试承接的体量锚（`archi_knowledge_test.rs`）。

### 新增文件

| 文件 | 说明 |
|---|---|
| `crates/core/archi/src/relation.rs` | D11 / D12 的实现本体：`relation_detail(root, from, to)`——模型面 + 定向扫描 + 两桶 + 命中点；`from_side_sources` 私有助手（`OwnerIndex` 同一归属规则 + `walk_source_files`）；关系缺席 / 元素缺席的错误文案点名（含元素数与关系数提示）；扫描错误经 `CheckError::message()` 透传；零写盘 / 零缓存 / 零 `Report`（签名见「公共函数 / API」） |
| `src/views/architecture/hooks/use-archi-relation.ts` | D17：按需取数 hook：`(root, edge)` → `{ detail, loading, error, retry }`；同对零重复 invoke（引用缓存）；失败不写缓存；零 watch / 零轮询 / 零 `setInterval` |
| `src/views/architecture/components/relation-info.tsx` | D13 / D14 / D19：`RelationInfo` 面板：七段 + 四空态优先级 + 行内错误与重试 + `declaredNotUsed` 的并列成因措辞 + 模型行号「查看」；根 testid `archi-relation-info` |

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `.dev-team/architecture/graph/model/06-frontend.toml` | ① 表头注释 `# 06 · 前端（30 元素 / 72 关系）` → `（32 元素 / 74 关系）`，构成行写实（共享组件文件 13 + 原语层 1 + 目录模块 1 + **生成物中继 1**，另有 3 个容器）；② 新增元素 `App.Frontend.GeneratedBindings`（D2 的六字段，置于 `Components.Ui` 之后、首个 `[[relation]]` 之前）；③ 12 条 `to = "App.Backend.Shell"` → `"App.Frontend.GeneratedBindings"`，`desc` 的「命令面类型」→「IPC 绑定」（`SessionPanel` 条为「经生成的 IPC 绑定取会话清单」）；④ 其余五个模型文件零改动 | D2；与本文件表头同批（计数复述单源） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | ① `§1` 53/121 → **55/123**（后端 49 + 前端 74）、`06` 72 → 74；② `§2`「`components/ui/` 的 39 个文件」→ **41**（实测 41 条 `export *`，`index.ts` 与 1 个测试文件不计）；③ `§3` 端点边 7 → **8**（3 出 + **5** 入，补 `Views.Config`）+ 前端恒等式 + 「生成物中继持目录 `path` 而无模块入口」一句；④ `§6`「53 个元素」→ 55；⑤ `§7` 排除面 `src/types/**` 一条改写（`types/generated/` 已归属、其余文件仍无元素）、映射例外改指 `App.Frontend.GeneratedBindings`，删「落 `observations[]`」的声明式约定段，改写为「有主 ⇒ 兑现」；⑥ `§8` 命令面边段落改写（12 条离开观察项、`observations` 62 → 50、既有 1 条边面缺口与三条目录缺口保留）+ 新增「生成物中继元素」覆盖判据一行 + 「上游生成链不声明」理由一行；⑦ `§9` 复算 `55`（容器 9 + 叶 46）、`06(32) / 74`、`前端 74 = 66 + 8`，并留痕「53 / 121、30 / 72、65 + 7 为既有陈旧，本变更同批校正」 | 事实校准 16；R2 |
| `.dev-team/architecture/graph/INDEX.toml` | `pnpm api:export` 重导：45 → **46** 条 `[[entry]]`（新元素 `status = "absent"` / `types = 0` / `ops = 0`），其余条目字节不变 | **生成物，零手改** |
| `.dev-team/architecture/models/**/*.schema.json` | 同一 `pnpm api:export` 重导后**零变化**：新元素 `App.Frontend.GeneratedBindings` 为 `api = "none"` ⇒ **不产** `<FQN>.schema.json`；既有 **10** 份（`Backend/Core` 1 + `Backend/Infra` 1 + `Frontend/Components` 8）逐字节不变 | **生成物，零手改**；登记生成面而非改动面（tasks「阶段七」的 `api:export` 一条同批覆盖） |
| `crates/core/archi/src/parse/toml.rs` | ① `RawRelation` 加 `#[serde(default)] apis: Option<Vec<::toml::Spanned<String>>>`；② `to_model(raw, source) -> (Model, Vec<Diagnostic>)`（私有签名变更）：逐项执行值面五拒，违例推 `Diagnostic { kind: InvalidValue, message: <点名值面四条>, line/column: <该项 span> }`；③ `parse` 把 `to_model` 的诊断与 serde 诊断合并；④ 新增私有 `fn validate_apis(source, from, to, apis) -> Vec<Diagnostic>`；`deny_unknown_fields` / `classify` / `line_of` / `column_of` 逐字保留 | D3 / D4 |
| `crates/core/archi/src/model.rs` | `Relation` 加 `pub apis: Vec<String>`（doc：「模型原文序；缺省空 = 不声明 API 粒度；去重 / 排序只在判定侧的 `api_buckets`」）；`Element` 与 `SourceSpan` / `Diagnostic` 零变化 | D3 |
| `crates/core/archi/src/scan/mod.rs` | ① `ImportFact` 加 `pub names: Vec<String>`（doc：空 = 取不到名字）；② 新增 `impl ImportFact { pub(crate) fn new(specifier: String, line: u32, type_only: bool, names: impl IntoIterator<Item = String>) -> ImportFact }`（内部调 `normalize_names`）；③ 新增 `pub(crate) fn normalize_names(names: impl IntoIterator<Item = String>) -> Vec<String>`（`BTreeSet` → 字典序，**去重单点**）；④ `DependencyEdge` 加 `pub names: Vec<String>` 并在成边处 `names: fact.names` 透传；⑤ 归属 / 排除 / 丢弃 / `inline_ignore` / `gate = off` 分支逐字不变 | D7；`ImportFact` 构造点仅 4 处（`rust.rs:117`、`ts.rs` 三处） |
| `crates/core/archi/src/scan/rust.rs` | ① `flatten_use_tree(tree, prefix, out: &mut Vec<(String, Option<String>)>)`：`Name` → `(path, Some(ident))`；`Rename` → `(path, Some(node.ident))`（**被引入名**）；`Glob` → `(prefix, None)`；末段恰为 `self` → `None`；② `extract` 用 `ImportFact::new(...)`（一条 `use` 声明内的多路径各自 facts 不变：`use a::{B, C}` 仍两 facts，各带自己的名字）；③ `resolve` / `locate` / `discover_crates` / 模块树逐字不变 | D5；名字不走前缀匹配 |
| `crates/core/archi/src/scan/ts.rs` | ① `ImportDeclaration` 分支收集 `ImportSpecifier` 的 `imported` 名（`ModuleExportName::Identifier` 取 `name`、`StringLiteral` 取字面值；`import_kind == Type` 照产名）；default / namespace 跳过；② `ExportFromDeclaration` 分支收集具名 `ExportSpecifier` 的 `local` 名；③ `ExportAllDeclaration` 分支 → 空 names；④ 三分支改用 `ImportFact::new(...)`；`type_only` 判定、`unresolved` 面、resolver 逐字不变 | D6 |
| `crates/core/archi/src/drift/mod.rs` | ① 新增 `pub fn api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)`（两侧去重 + 字典序，逐字符比对）；② 新增 `pub(crate) fn edge_names(edges: &[&DependencyEdge]) -> Vec<String>`（转调 `scan::normalize_names`）；③ `occurrences_of` 由私有提 `pub(crate)`；④ `check` 在 ⑦ 之后追加「两桶装配」段：遍历 `graph.relations`（`graph_view` 的 `(from, to)` 字典序），`apis` 为空即跳过；`used ∖ declared` 非空 ⇒ 一条 `ArchiDriftEntry { kind: UndeclaredApi, gate: Some(from.gate), occurrences: occurrences_of(&edges), message: D19 }` 按档位落 `errors` / `warnings`（`off` 不落）；`declared ∖ used` 非空 ⇒ 一条 `ArchiDriftObservation { kind: UnusedDeclaration, … }`；⑤ 模块 doc 补「名字级两桶」一句；既有 ⑥ / ⑦ / ⑧ / `derive_status` / `passed` 逐字不变 | D8 / D9 / D10 |
| `crates/core/archi/src/drift/report.rs` | ① `ArchiDriftKind` 加 `UndeclaredApi`（`as_str` = `"undeclaredApi"`，`ALL: [ArchiDriftKind; 5]`，新值置末）；② `ArchiDriftKind` doc 由「封闭四值」改「封闭五值」；③ `ArchiObservationKind::UnusedDeclaration` 注释由「本期不产出」改「`declared ∖ used` 时产出（`ALL` 仍二值）」；④ `REPORT_VERSION`（在 `drift/mod.rs`）与 `ArchiDriftReport` 字段集逐字不变 | D18 |
| `crates/core/archi/src/drift/report_test.rs` | `ArchiDriftKind::ALL.len()` 4 → 5、`as_str` 逐值对拍补 `undeclaredApi`；新增「`unusedDeclaration` 不进 `status` / `passed`」「条目键集不变」用例 | 测试相位承接（proposal「测试文件」表） |
| `crates/core/archi/src/dto.rs` | ① `ArchiRelation` **末尾**加 `pub apis: Vec<String>`（doc：「模型字段的投影拷贝、字段不省略、顺序按原文」）；② `to_relation` 拷贝 `apis: relation.apis.clone()`；③ 新增 `ArchiRelationDetail`（12 字段，`specta::Type`，`#[serde(rename_all = "camelCase")]`）；④ `struct Loaded` / `fn load` / `fn diagnostics_error` 提 `pub(crate)`（`Loaded` 两字段 `pub(crate)`），doc 补「`relation` 的按需取数复用同一份装载，MUST NOT 第二份 parse → build」；⑤ 其余投影逐字不变 | D11 / D13 / D15（字段面）；事实校准 1；`ArchiRelation.apis` 与 `Relation.apis` 同批落地（事实校准 5 / D1，判定面前置） |
| `crates/core/archi/src/lib.rs` | 加 `pub mod relation;` 与 `pub use relation::relation_detail;`；模块 doc 第 1 行补「关系详情」。`#[cfg(test)] mod relation_test;` 与 `relation_test.rs` **同一提交由测试相位落地**（避免声明悬空、保证任一提交点可编译） | 出口面 |
| `src-tauri/src/commands/archi/mod.rs` | ① 文件头「八命令」→「九命令」、「八个 `*_inner`」→「九个」、写命令两条与「其余七条 MUST NOT 写盘」；② 新增 `archi_relation(root: String, from: String, to: String)` + `archi_relation_inner(root: &Path, from: &str, to: &str)`（`check_root` → `archi::relation::relation_detail`）；③ 导入 `ArchiRelationDetail`；既有八条零改动 | 三件事纪律（参数转换 → 调用 → 错误映射） |
| `src-tauri/src/commands/mod.rs` | `all_commands!` archi 段在 `archi_api_generate` 之后、`read_file` 之前追加 `commands::archi::archi_relation` | 8 → 9 |
| `src-tauri/src/bindings/mod_test.rs` | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 56 → **57**（`archi_relation` / `archiRelation` 插在 `archi_api_generate` / `archiApiGenerate` 之后、`read_file` / `readFile` 之前）；`DTO_TYPES` 104 → **105** 与 `ARCHI_DTO_TYPES` 21 → **22**（`ArchiRelationDetail` 字典序插在 `ArchiRelation` 之后）；尾部序期望 vec 追加 `archi_relation`；首尾锚断言零改动 | 清单只作连带核对 |
| `src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 重导：`ArchiRelationDetail` 类型与 schema、`ArchiRelation.apis`、`commands.archiRelation` 包装与 `argsSchemas` / `resultSchemas` 表项 | **生成物，零手改** |
| `src/views/architecture/components/architecture-canvas.tsx` | ① `CanvasBody` 增 `selectedEdge` state（`{from,to} \| null`）与 `exemptVias` prop；② `ModelRelationGraph` 接 `onEdgeClick`（置选中）与选中边高亮（只在既有 `style` 上换 stroke 变量，零新词汇），`onNodeClick` 追加清选中；③ `CardHeader` 两态分支：无选中 → `ModelInfo`；有选中 → `RelationInfo`（`root` / `edge` / `relation` / `state` / `exemptVia` / `onRetry`）；④ hook 挂在 `CanvasBody`（缓存不随面板卸载丢失）；⑤ `ArchitectureCanvas` 从 `report.suppressed[]` 派生 `exemptVias`；⑥ `toEdges` / `toNodes` / `neighborhoodPositions` / 入边分区 / 停摆分支逐字不变 | D14 / D15 / D16 / D17；`drift-marks.ts` / `marks.ts` 零改动 |
| `crates/infra/agent/src/archi-knowledge.md` | ① 「位置与键形」段关系键面 `（from / to / desc）` → `（from / to / desc / apis）`；②「字段纪律」段新增一行 `apis` 纪律（只写名字 / 可留空 / 生成物面留空 / 结论看 `archi_drift`）；③ 同段压缩约 100 B 冗余（如「点分嵌套会读成**另一元素**。`metadata` 必须内联表（子表同样读成另一元素）」合并、「（`archi_module` 报错）」删）使骨架净增长后仍落 `2000..=2150`；零示例围栏 / 零判据长表 / 零 `<>` 占位符 / 零新词表 | D20 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 冻结值随实测更新：骨架 **2142 B** → 新实测、全文 **2632 B** → 新实测；断言区间 `2000..=2150` / `2550..=2750` 与门限 **2252 / 2867 逐字不抬高**（MUST NOT 以「贴近门限」冒充达标） | 测试相位承接（AC-11 的体量锚）；知识文本净增长后仍须落区间；本相位只登记机械面、不写用例 |
| `packages/desktop/package.json` | `"version"`: `0.7.4` → `0.7.5` | D21 |

<!-- 配置（config）子表省略：本变更零配置键改动——`.dev-team/architecture/config.json` 仍只有 `exclude`（`openspec/**`）且不新增 `ignore` 规则（新元素只作 `to`，不产生需豁免的元素对）；工作区 `KNOWN_KEYS` 与 `config.schema.json` 镜像零改动（架构域看护配置形状不变）。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `api_buckets` | `crates/core/archi/src/drift/mod.rs` | 新增 | `pub fn api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)` | 返回 `(used ∖ declared, declared ∖ used)`；两侧去重 + 字典序；逐字符比对。**求差唯一实现**（drift 对账与 `relation_detail` 共用），MUST NOT 第二份集合差 |
| `edge_names` | 同上 | 新增 | `pub(crate) fn edge_names(edges: &[&DependencyEdge]) -> Vec<String>` | 元素对事实名并集（去重 + 字典序），内部转调 `scan::normalize_names`；两消费面共用 |
| `occurrences_of` | 同上 | 修改（可见性） | `pub(crate) fn occurrences_of(edges: &[&DependencyEdge]) -> Vec<ArchiDriftOccurrence>` | 由私有提 `pub(crate)`（`relation_detail` 复用同一聚合形状与排序）；函数体逐字不变 |
| `check` | 同上 | 修改（装配） | `pub fn check(input: CheckInput<'_>) -> Result<Report, CheckError>` | 签名与既有语义不变；⑦ 之后追加两桶装配段（仅 `apis` 非空的关系） |
| `ArchiDriftKind::as_str` | `crates/core/archi/src/drift/report.rs` | 修改 | `pub fn as_str(self) -> &'static str` | 补 `"undeclaredApi"`；`ALL` 扩为 `[ArchiDriftKind; 5]`（`pub const ALL`） |
| `relation_detail` | `crates/core/archi/src/relation.rs` | 新增 | `pub fn relation_detail(root: &Path, from: &str, to: &str) -> Result<ArchiRelationDetail, String>` | 模型面（`from` / `to` / `description` / `mark` / `reason` / `model_file` / `model_line` / `declared`）+ 事实面（`used`）+ 两桶 + `occurrences`；零写盘 / 零缓存 / 零 `Report` |
| `load` | `crates/core/archi/src/dto.rs` | 修改（可见性） | `pub(crate) fn load(root: &Path) -> Loaded` | `ModelGraph::build` 的唯一调用点；`relation` 复用同一份装载（MUST NOT 第二份 parse → build） |
| `diagnostics_error` | 同上 | 修改（可见性） | `pub(crate) fn diagnostics_error(diagnostics: &[Diagnostic]) -> String` | 结构错文案单点，`relation_detail` 的 `Err` 复用 |
| `normalize_names` | `crates/core/archi/src/scan/mod.rs` | 新增 | `pub(crate) fn normalize_names(names: impl IntoIterator<Item = String>) -> Vec<String>` | 去重 + 字典序**单点**；`ImportFact::new` 与 `drift::edge_names` 共用 |
| `ImportFact::new` | 同上 | 新增 | `pub(crate) fn new(specifier: String, line: u32, type_only: bool, names: impl IntoIterator<Item = String>) -> ImportFact` | 唯一构造点（归一化在内）；Rust / TS 抽取器同用 |
| `RustResolver::extract` | `crates/core/archi/src/scan/rust.rs` | 修改（产出内容） | `pub fn extract(&self, file_rel: &str, source: &str) -> (Vec<ImportFact>, Vec<ArchiUnresolvedFile>)` | 签名不变；每条 fact 带 `names`（别名取原名、`self` / glob 空） |
| `RustResolver::resolve` | 同上 | 零改动 | `pub fn resolve(&self, file_rel: &str, specifier: &str) -> Option<String>` | 名字不走前缀匹配（签名与语义逐字不变） |
| `TsResolver::extract` | `crates/core/archi/src/scan/ts.rs` | 修改（产出内容） | `pub fn extract(&self, file_rel: &str, source: &str) -> (Vec<ImportFact>, Vec<ArchiUnresolvedFile>)` | 签名不变；具名 specifier 产名、default / namespace / `export *` 不产名 |
| `TomlParser::parse` | `crates/core/archi/src/parse/toml.rs` | 零改动（内部形态随动） | `fn parse(source: &str) -> ParseOutcome`（`SourceParser` 实现） | 诊断来源由「只有 serde」扩为「serde + `to_model` 值面校验」，`ParseOutcome` 形状不变 |
| `archi_relation` | `src-tauri/src/commands/archi/mod.rs` | 新增 | `pub fn archi_relation(root: String, from: String, to: String) -> Result<ArchiRelationDetail, String>` | IPC 命令（`#[tauri::command] #[specta::specta]`） |
| `archi_relation_inner` | 同上 | 新增 | `pub fn archi_relation_inner(root: &Path, from: &str, to: &str) -> Result<ArchiRelationDetail, String>` | 领域组装纯函数（离 Tauri 运行时可测）：`check_root` → `archi::relation::relation_detail` |
| `useArchiRelation` | `src/views/architecture/hooks/use-archi-relation.ts` | 新增 | `export function useArchiRelation(root: string \| null, edge: RelationEdge \| null): ArchiRelationState` | 无根 / 无选中 / `apis` 为空（由调用方传 `null`）即不取数；同对零重复 invoke；失败保留 `error`；`retry()` 强制重取 |
| `RelationInfo` | `src/views/architecture/components/relation-info.tsx` | 新增 | `export function RelationInfo(props: RelationInfoProps): React.JSX.Element` | 面板本体：七段 + 四空态 + 行内错误与重试 + 模型行号「查看」；不自行求差、不读漂移报告 |
| `ArchitectureCanvas` | `src/views/architecture/components/architecture-canvas.tsx` | 零改动（签名） | `export function ArchitectureCanvas({ root, mode }: ArchitectureCanvasProps): React.JSX.Element` | 入参与导出不变；内部新增选中态与两态面板 |

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `Relation`（`apis: Vec<String>`） | `crates/core/archi/src/model.rs` | 修改（字段） | 模型原文序；缺省空 = 不声明 API 粒度 |
| `ImportFact`（`names: Vec<String>`） | `crates/core/archi/src/scan/mod.rs` | 修改（字段） | 去重 + 字典序（`ImportFact::new` 保证）；空 = 取不到名字 |
| `DependencyEdge`（`names: Vec<String>`） | 同上 | 修改（字段） | `ImportFact.names` 原样透传 |
| `ArchiRelation`（`apis: Vec<String>`） | `crates/core/archi/src/dto.rs` | 修改（字段，末尾追加） | 模型字段的投影拷贝、字段不省略、顺序按原文；不含 `modelFile` / `modelLine` |
| `ArchiRelationDetail` | 同上 | 新增（`struct` + `specta::Type`） | 12 字段：`from` / `to` / `description` / `mark` / `reason` / `model_file` / `model_line` / `declared` / `used` / `used_not_declared` / `declared_not_used` / `occurrences`（线上 camelCase） |
| `Loaded` | 同上 | 修改（可见性） | 两字段 `pub(crate)`（`graph` / `diagnostics`），`load` 的返回体 |
| `ArchiDriftKind::UndeclaredApi` | `crates/core/archi/src/drift/report.rs` | 新增（枚举变体） | 线格式 `undeclaredApi`；`ALL: [ArchiDriftKind; 5]`，新值置末 |
| `RelationEdge` | `src/views/architecture/hooks/use-archi-relation.ts` | 新增（`interface`） | `{ from: string; to: string }`——选中边的元素对身份 |
| `ArchiRelationState` | 同上 | 新增（`interface`） | `{ detail: ArchiRelationDetail \| null; loading: boolean; error: string \| null; retry: () => void }` |
| `RelationInfoProps` | `src/views/architecture/components/relation-info.tsx` | 新增（`interface`） | `{ root: string \| null; edge: RelationEdge; relation: ArchiRelation; state: ArchiRelationState; exemptVia: 'config' \| 'inline' \| null; onRetry: () => void }` |

---

## 依赖 (Dependencies)

### 运行时依赖

零新增。`crates/core/archi` 既有依赖即够用：`syn`（Rust `use` 树，名字与路径同一次展开）、`oxc_parser` / `oxc_ast` / `oxc_span`（TS specifier 名）、`toml` / `toml_edit`（`Spanned` 与 `gate` 文本写回）、`serde` / `specta`（DTO 与词汇）、`ignore`（既有枚举）、`petgraph` / `sha2`（判定与哈希）。`Cargo.toml` 零改动；前端零新包（`@xyflow/react` / `@/components/ui` / `FileViewer` 均已在用）。

### 构建 / 测试依赖

零新增。复用既有脚本：`pnpm api:export`（`INDEX.toml` 与 `*.schema.json`）、`pnpm bindings:export`（两份生成物 + zod 自检）、`pnpm api:check`、`pnpm check`（`server:check` → `bindings:export` → `api:check` → `client:check`）、`pnpm test`。语料面与门禁面零新工具。

---

## 验收标准回应

| AC | 设计落点 |
|---|---|
| AC-1 | D8 / D18：`apis` 省略 ⇒ 两桶皆空、零新条目；新增语料 case 只加不改，`corpus/drift/expected/` 既有 16 份逐字节不变 |
| AC-2 | D8：`used ∖ declared` ⇒ 恰一条 `undeclaredApi`（按 `from.gate` 三档），条目按元素对聚合、`occurrences` 复用既有形状 |
| AC-3 | D8 / D18：`declared ∖ used` ⇒ 恰一条 `unusedDeclaration`，只落 `observations[]`（`derive_status` / `passed` 逐字不读它） |
| AC-4 | `ArchiDriftKind::ALL: [_; 5]`（新值置末）、`ArchiObservationKind::ALL` 仍二值、`as_str` 逐值对拍（`drift/report_test.rs`） |
| AC-5 | D9 / D10：`api_buckets` 为唯一求差实现，`drift::check` 与 `relation_detail` 各一处调用（源码复核锚：两文件各 1 处 `api_buckets(`） |
| AC-6 | D18：`REPORT_VERSION = 3`、`ArchiDriftReport` 字段集与条目键集不变，名字只进 `message` |
| AC-7 | D13 / D14 / D17：`apis` 为空 ⇒ hook 入参 `null` ⇒ 零 invoke；`archi_relation` 失败只落面板 `error` + 重试，`graph` / `ModelInfo` / 左栏三视图零影响 |
| AC-8 | 变更清单第 1、2 行（模型 + ADR 写实）；**模型面读数**（55 / 123、`06` 32 / 74、74 = 66 + 8、`observations` 62 → 50、`model.warnings` 9 条 `noPath`）与**扫描面条目级账目**（既有条目零增删改 + 新增条目全部命中既有规则：`scannedFiles` / `unmatchedFiles` 412 → 415、`excluded` 341 → 352、候选 753 → 767；`matched` / `unresolved` 仍 0；逐条见「落地面」§5）由桌面端命令采集并落本 change `reports/`；MUST NOT 以「`scan` 三面逐条相同」或三面计数相等充当归属未变 / 判定面零变化的证据 |
| AC-9 | 新元素 `api = "none"` 且无 `doc` ⇒ 分母 46、`docMissing` 36（33 + 3）、其余十二值 0、`status = clean`、`passed = true`；`api/**` sidecar 13 份零增删 |
| AC-10 | `INDEX.toml` / `bindings.ts` / `bindings.zod.ts` 只由生成步骤产生；`models/**/*.schema.json` 零新增（新元素 `api = "none"` 不产 schema，既有 10 份字节不变）；清单守卫四处计数随动（57 / 57 / 105 / 9 / 22） |
| AC-11 | D20：关系四键在场、`apis` 纪律句含三关键词、`apis` 不进派生词表、骨架净增长后仍落 `2000..=2150`；`archi_knowledge_test.rs` 冻结值（2142 / 2632）随实测同批更新，区间与门限不抬高 |
| AC-12 | 交接 static-check / test-execution 相位（本相位不执行、不产出结论） |
| AC-13 | D21：`package.json` = `0.7.5`；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 版本行不变 |

> **测试面交接**（本相位不写用例、不跑测试）：proposal「测试文件」表 12 行全部归测试相位，逐行对应如下（设计侧只钉住必改机械面与不可回退的判据锚）；另加 AC-11 体量锚一行——proposal 未列该文件，但冻结值属测试机械面，区间与门限 MUST NOT 抬高。

| proposal 测试行 | 归属文件 | 设计侧钉住的机械面 |
|---|---|---|
| `apis` 四键解析 + 五类形状拒收 + `api = [..]` 仍 `unknownField` | `parse/toml_test.rs` + `model_test.rs` | 五类各一条 `invalidValue`（带位置）；`apis = []` 同拒；`Relation` 字面量夹具补 `apis: Vec::new()`（六处：`diff_test` 1 / `graph_test` 2 / `model_test` 3） |
| `Relation.apis` 三态与字面量夹具随动 | `model_test.rs` / `dto_test.rs` | `ArchiRelation` 夹具补 `apis`（`dto_test.rs:871`）；投影零归一 / 零排序 |
| Rust 四形态产名 | `scan/rust_test.rs` | `use a::B` → `B`；`use a::{B, C}` → `B`,`C`；`use a::B as D` → `B`；`use a::*` / `use a::{self}` → 空 |
| TS 三形态产名 | `scan/ts_test.rs` | `import { A, type B }` → `A`,`B`；`export { A } from` → `A`；default / namespace / `export *` → 空 |
| `api_buckets` 逐例 | `drift/*_test.rs` | 双空 / 全匹配 / 少一项 / 多一项 / 乱序重复五例；输出恒去重 + 字典序 |
| 两桶对账（三档定级 / 观察项 / 未填零产） | `drift/*_test.rs` | `strict` → `errors` / `warn` → `warnings` / `off` → 零记录；`declared ∖ used` 一条观察且 `status` / `passed` 不变；`apis` 空零产 |
| `ALL` 长度 / `version` / 16 份金样零变化 | `drift/report_test.rs` + `tests/corpus/drift/**` + `tests/golden_test.rs` | `ArchiDriftKind::ALL.len() == 5`、`ArchiObservationKind::ALL.len() == 2`、`REPORT_VERSION = 3`；既有 16 份 `expected/*.json` 逐字节不变，新增 `api-declared-exact` / `api-used-not-declared` / `api-declared-not-used` / `api-absent` 四 case 登记进 `DRIFT_CASES` |
| `relation_detail` 与命令对拍 | `relation_test.rs` + `commands/archi/mod_test.rs` | `relation_test.rs` 与 `lib.rs` 的 `#[cfg(test)] mod relation_test;` 同批；命令侧 root 校验由八命令扩到九命令、`_inner` 一致性、零写盘、同一 `(from, to)` 与 drift 同两桶 |
| 命令清单随动（57 / 105 / 9 / 22） | `bindings/mod_test.rs` | 见「修改文件」表该行（计数 + 尾部序期望 vec + 首尾锚零改） |
| 前端：边点击 / 四空态 / 两态切换 / 未声明零取数 / 失败隔离 | `architecture-canvas.test.tsx` + `relation-info.test.tsx`（新）+ `use-archi-relation.test.ts`（新） | 断言零定时器、零重复 invoke、四空态 testid 可辨、`apis` 空时 invoke 计数为 0；既有 `architecture-canvas.test.tsx` / `model-info.test.tsx` 的 `relation()` 夹具补 `apis: []` |
| 前端载荷语料随动 | `src/lib/ipc-contract.test.ts` + `views/architecture/**` 既有夹具 | `ArchiRelationDetail` 与 `ArchiRelation.apis` 字段集由 `tsc` 报错逐处补齐（零新增判据语言） |
| 本仓读数留痕 | 本 change `reports/`（命令 + 留痕，不入 crate 语料） | `observations` 62 → 50、`docMissing` 35 → 36、`model.warnings` 9 条 `noPath` 前后不变、`scan` 按条目级账目（`scannedFiles` / `unmatchedFiles` 412 → 415、`excluded` 341 → **352**、候选 753 → **767**；既有条目零增删改、新增 3 源 + 11 测试 / 语料全部命中既有规则，逐条见「落地面」§5）、`INDEX.toml` 46 条 |
| 知识文本体量锚（AC-11，proposal 测试表外一行） | `crates/infra/agent/src/archi_knowledge_test.rs` | 冻结值骨架 **2142 B** / 全文 **2632 B** → 新实测；区间 `2000..=2150` / `2550..=2750` 与门限 **2252 / 2867 逐字不抬高**；断言随文本跟四键 + `apis` 纪律句，`apis` 不进派生词表 |

> 另两条不可回退的判据锚：① `crates/core/archi/src/api/**` sidecar 仍恰 **13** 份（新两桶用例落 `drift/*_test.rs`，MUST NOT 新建 `api/**` 文件）；② 报告条目键集零变化（名字只进 `message`）。

---

## 待决问题 (Open Questions)

1. **`apis` 值面能否机械拒收「无空白的签名形态」**：四查（非空 / 无空白 / 无 `::` / 不重复）放行 `config_path()` 这类无空白但带括号的写法。本设计按 proposal D6 + 本文件 D4 落地四查，不加标识符字符集（避免自造第五查）。若评审要求更强拒收，须改 `desktop-archi-model` 增「标识符字符集」一条（连带值面文案与语料）。
2. **空态③ 的 `via` 数据来源**：`ArchiRelation` 无 `via`、`drift-marks.ts` 按 spec 逐字不变，故 `via` 从画布已持有的 `report.suppressed[]` 查得（D15）。另一条路是给 `ArchiRelation` 加 `via` 字段——那会动 123 条边的图载荷与生成物，本设计不取。
3. **「取数失败降级时选中边清空」的射程**：本设计读作「图面降级（`root` 变化 / 图停摆 / 图取数失败）」清空选中，`archi_relation` 自身失败**不清空**（否则空态④ 不可达，与 spec 的空态④ 自相矛盾）。若评审坚持字面口径，需同批裁定空态④ 的呈现位置。
4. **`archi_scaffold` 草稿的关系模板是否进四键**：现模板三行注释（`from` / `to` / `desc`），`apis` 选填不列不构成矛盾；proposal「变更范围」未列 `sdk/archi_tool.rs`，故本变更不改。若后续要让模板自证四键形态，须改 `SCAFFOLD_TEMPLATE` 与既有模板断言（独立小变更）。

---

## 落地面（本相位已落位：架构模型 / ADR / 生成物；产品代码零改动）

**事实**：implement 相位对 `.dev-team/architecture/graph/model/06-frontend.toml` 的写入曾被平台相位锁拒绝（架构模型只允许 proposal / dev-design 相位执行侧或架构页会话写入），故「数据面」四项改动留给本相位。本相位已按 D2 与「变更清单」逐行落盘：模型 / ADR / `INDEX.toml` 三处均已在盘，`api:check` 零 diff，产品代码零改动。

### 1. 模型（`06-frontend.toml`）——已落位

```toml
# 表头（第 1–2 行）
# 06 · 前端（32 元素 / 74 关系）
# 元素 = 页壳 1 + 视图 9 + 视图内子域 3 + 共享组件文件 13 + 原语层 1 + 目录模块 1 + 生成物中继 1，另有 3 个容器。

# 新元素：置于 [element."App.Frontend.Components.Ui"] 之后、首个 [[relation]] 之前（落位后 modelLine = 188）
[element."App.Frontend.GeneratedBindings"]
kind = "module"
gate = "off"
api = "none"
path = ["./packages/desktop/src/types/generated/"]
summary = "IPC 命令面与出线 DTO 的 TypeScript 投影（tauri-specta 生成，勿手改）"
```

- **零** `entry` / `doc` / `metadata` / `apis`；零出边（只作 `to`）——`archi_module` 实测 `dependsOn = []`、`dependedBy` 恰 **12** 条（11 条 `desc = "经生成的 IPC 绑定取数与调用"`，`SessionPanel` 一条为「经生成的 IPC 绑定取会话清单」）、`resolvedEntry.problem = "未声明 entry，且约定默认候选皆不在盘"`（`api = none` 已退出入口判据 ⇒ 零 `apiEntryMissing`）。
- 12 条命令面关系改指清单（`from`）：`Components.Agent.AgentSessionView` / `Components.Agent.SessionPanel` / `Views.AgentDebug` / `Views.Agents` / `Views.Architecture` / `Views.Changes` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Config` / `Views.DbInspector` / `Views.Explores` / `Views.Info`。落位后 `grep -c 'to = "App.Backend.Shell"' 06-frontend.toml` = **0**、`grep -c 'to = "App.Frontend.GeneratedBindings"'` = **12**。
- 计数实测：`^\[element` **32** / `^\[\[relation\]\]` **74** / `api = "none"` **3** 处（`05` 两处 + 本元素）；其余五个模型文件零改动。

### 2. ADR（`0001-model-granularity.md`）——已落位七项

`§1` 55 / 123（后端 49 + 前端 74）；`§2` **41** 个文件；`§3` 端点边 **8** 条（3 出 + 5 入，补 `Views.Config`）+ 「生成物中继持目录 `path` 而无模块入口」一句；`§6` **55** 个元素；`§7` 排除面 `src/types/**` 改写（`types/generated/` 已归属）、映射例外改指 `App.Frontend.GeneratedBindings`、删「落 `observations[]`」的声明式约定段并改写为「有主 ⇒ 兑现」；`§8` 新增「覆盖判据第 4 档（生成物中继）」与「上游生成链不声明」两行；`§9` 复算 `55`（容器 9 + 叶 46）/ `06(32) / 74` / 前端 `74 = 66 + 8`，并留痕「`53 / 121`、`30 / 72`、`65 + 7` 与 `§2` / `§3` 的旧读数同为既有陈旧，本变更同批校正」。

### 3. 生成物（`INDEX.toml`）——已重导

`pnpm -C packages/desktop run api:export` → `1 份生成物已写入（12 份未变）`：`graph/INDEX.toml` **45 → 46** 条 `[[entry]]`，新元素条目 `status = "absent"` / `types = 0` / `ops = 0` / `degraded = 0` / `schema = ""` / `generated = ""`；`models/**/*.schema.json` 仍 **10** 份、逐字节不变（`api = "none"` 不产 schema）；`pnpm run api:check` 退出码 **0**（重复执行 `api:export` 零新增行）。

### 4. 落位后读数（`archi_drift` / `archi_module` 实测；报告落盘仍交 implement）

| 面 | 落位后 | 与 proposal 预期 |
|---|---|---|
| `model.elements` / `relations` / `byGate.off` | **55 / 123 / 55** | 一致 |
| `observations` | **50**（49 后端 + 1 既有边面缺口） | 一致（62 → 50） |
| `apiFindings` | **36**、全 `docMissing`（33 `declared` + 3 `api = none`） | 一致（35 → 36） |
| `model.warnings` / `warnings` / `errors` | **9** 条 `noPath` / `[]` / `[]` | 一致 |
| `status` / `passed` | `clean` / `true` | 一致 |
| `scan` | `matched` **0**、`unresolved` **0**、`excluded` **352**、`scannedFiles` / `unmatchedFiles` **415**（412 → 415）、候选 **767**（753 → 767） | 一致（条目级）：+3 源文件进 `unmatchedFiles`（`gate` 全 `off` 的既有分支）、+11 测试 / 语料进 `excluded`（既有规则命中）；既有条目零增删改，逐条账目见 §5。3 个生成文件前后都在 `unmatchedFiles` 内，故 MUST NOT 用这两个计数验收归属 |
| `modelHash` | `sha256:bbf53e45…` → `sha256:57901d7a…` | 必然变化（模型字节变） |

### 5. 扫描面条目级账目（AC-8 的机械证据；`archi_drift(full)` 实测）

**口径**：报告 `scan` 段只出计数（`mode` / `changedFiles` / `scannedFiles` / `excluded` / `unmatchedFiles` / `matched` / `unresolved`），`unmatchedFiles[]` / `excluded[]` 的条目明细不经报告出线。故 AC-8 的「集合对拍」按三步落成：① 规则集零 diff；② 既有文件集合零删改；③ 新增候选 14 条逐条点名并各带命中规则——与计数差（+3 / +11 / +14）逐面自洽。

| 面 | 变更前（`20261120T1042330799149Z.json`，full） | 变更后（`archi_drift(full)` 实测） | 差 |
|---|---|---|---|
| `scannedFiles` / `unmatchedFiles` | 412 / 412 | **415 / 415** | +3（新增源文件，`gate` 全 `off` ⇒ 与无主同视） |
| `excluded` | 341 | **352** | +11（既有规则命中） |
| 候选（`scannedFiles + excluded`） | 753 | **767** | +14 = 3 源 + 11 测试 / 语料 |
| `matched` / `unresolved` | 0 / 0 | 0 / 0 | 0 |

**既有条目零增删改**（三条机械证据）：① 排除规则集零 diff——`scan/exclude.rs` 的 `DEFAULT_EXCLUDE_RULES`（13 条具体 glob）与 `.dev-team/architecture/config.json`（`exclude = ["openspec/**"]`，唯一配置追加）本变更零改动；② 既有源文件零删除 / 零重命名（`git status --porcelain` 无 `D` / `R` 条目）⇒ 同一路径的 `file` / `rule` 不可能变；③ 既有 412 条 `unmatchedFiles` 与 341 条 `excluded` 的条目集合因此逐条保持（数组排序带来的位序平移不计）。

> 时序留痕：`excluded = 341` 是 20:25 采集点的快照——那 11 份测试 / 语料在 20:44–20:56 才落地，当时新增候选只有 3 个源文件；全量落地后为 **352**。MUST NOT 把 341 与 352 当作两种结论，它们是同一候选集合在两个采集时点的读数（差额逐条可解释，见上表与下节点名）。

**新增条目逐条点名**（14 条）：

| # | 新增文件 | 落点 | 命中规则 / 理由 |
|---|---|---|---|
| 1 | `crates/core/archi/src/relation.rs` | `unmatchedFiles[]` | 归属 `App.Backend.Core.Archi`（`gate = off`）⇒ 与无主同视 |
| 2 | `src/views/architecture/hooks/use-archi-relation.ts` | `unmatchedFiles[]` | 归属 `App.Frontend.Views.Architecture`（`gate = off`）⇒ 同上 |
| 3 | `src/views/architecture/components/relation-info.tsx` | `unmatchedFiles[]` | 归属 `App.Frontend.Views.Architecture`（`gate = off`）⇒ 同上 |
| 4 | `crates/core/archi/src/relation_test.rs` | `excluded[]` | `**/*_test.rs` |
| 5–12 | `crates/core/archi/tests/corpus/drift/{api-absent,api-declared-exact,api-declared-not-used,api-used-not-declared}/repo/src/{api/index.ts,consumer/x.ts}`（8 份） | `excluded[]` | `**/tests/**` |
| 13 | `src/views/architecture/components/relation-info.test.tsx` | `excluded[]` | `**/*.test.tsx` |
| 14 | `src/views/architecture/hooks/use-archi-relation.test.ts` | `excluded[]` | `**/*.test.ts` |

> 语料 case 下的 `models/01-app.toml`、`expected/*.json` 与 `openspec/**` 文稿都不是源文件（`is_source_file` 不命中；`openspec/**` 另有配置规则命中）⇒ 不占候选位。
>
> **MUST NOT**：以 `scannedFiles` / `unmatchedFiles` / `excluded` 的计数相等（或「三面逐条相同」）充当「归属未变」或「判定面零变化」的证据——归属兑现的证据是 `observations` 62 → 50（12 条命令面边离开观察项），扫描面证据是上述条目级账目（R10 的反面封死）。

`reports/archi/` 现有三份留痕（1 份 full 基线 `20261120T1042330799149Z.json` + 2 份增量采集 `20261120T1208557123693Z.json` / `20261120T1225333779671Z.json`）仍是改动前 / 增量读数（`54 / 123`、`observations` 62 / `apiFindings` 35）——**变更后 full 读数**以上文 §4 / §5 的 `archi_drift(full)` 实测为准。**implement 相位已实测复核 §4 表每一项读数**（`55 / 123`、`observations` 50、`apiFindings` 36、`model.warnings` 9、`errors` / `warnings` 空、`status = clean` / `passed = true`、`scan` 415 / 415 / 352 / 0 / 0（候选 767）、`modelHash = sha256:57901d7a…`）并据此勾掉 tasks.md 阶段八两条；`reports/archi/` 的**全量报告 JSON 落盘**仍由桌面端 archi 门禁命令采集（运行期产物，本相位零手写），故该目录文件数不随本节变化。MUST NOT 以「`reports/archi/` 未新增文件」反推「读数未成立」。
