# desktop-archi-api-check Specification (Delta)

## MODIFIED Requirements

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓当前读数；分母 = `path` 非空的 46 个元素）**：`apiEntryMissing` **0**、`apiMissing` **0**（形态① 已改名 `docMissing`，只剩形态②，本仓零契约实现漂移）、**`docMissing` 36**（33 = `api = declared` 且派生契约 md 不在盘；3 = `api = none` 且 `doc` 未声明）、其余十值各 **0**、`apiUndeclared` 不适用（`off`）；`status = clean`、`passed = true`（36 条全 `warn` 且 `docMissing` 不纳管）。
- **本段的两项性质必须与原话一起写**：这不是「新增 36 条债」，而是——
  1. **改名**（更早一段）：`apiMissing` 的原形态①（契约 md 未写）改名为 `docMissing`，事实同一；
  2. **参与面 +3**：三个 `api = none` 元素进入判据面（过去契约面把它们整体排除，现在它们必须给出「本模块无 API + 入口文档在哪」）——两个工具 crate 与**本变更新增的生成物中继元素** `App.Frontend.GeneratedBindings`（见 `repo-archi-model`）；
  3. **分母与读数随仓演进**（本变更同批校正，见下条）。
- **既有读数的陈旧必须留痕（不得假装本次只 +1）**：本要求此前的「分母 44 / `docMissing` 43」是**两次变更之前**的读数——其间 ① 新增了一个 `path` 非空的共享组件元素（分母 44 → **45**，`docMissing` 41 → 42），② 9 份新契约 md 落地（`Foundation` + 8 份 Agent 组件，连同既有试点 `ArchiRuntime` 共 **10** 份契约在盘 ⇒ declared 侧读数 42 → **33**），故本变更前的真实读数是 **45 / 35（33 declared + 2 none）**。本变更再 +1 个 `api = none` 且无 `doc` 的元素 ⇒ **46 / 36（33 declared + 3 none）**。MUST NOT 把「43 → 36」写成「债务下降 7 条」：那是**分母 +1、契约 +10（既有）、参与面 +1（本次）**三种变化叠加后的净值，逐 `kind` 必须按本条口径解释。
- **两条分桶口径**：① `apiEntryMissing` 只由「无可用入口」产生，`api = none` 元素退出该判据 ⇒ 本仓该桶为 0；② 入口可解析的 `api = declared` 元素承担「有门无入口文档」的 `docMissing`（形态①），而 `api = none` 元素在 `doc` 未声明时同样落 `docMissing`——故 36 = 33（declared）+ 3（none），`apiMissing` 只剩「契约声明了而代码没有」一桶且本仓为 0。
- **历史链（七段，逐段成因在案）**：`28 + 13 = 41`（入口靠探测）→ `15 + 26 = 41`（入口判据落地）→ `12 + 27 = 39`（数据面换段）→ `12 + 29 = 41`（共享组件补建元素）→ `0 + 41 = 41`（12 个前端目录根补入口，整桶平移）→ **`0 + 43 = 43`（形态①改名 + `api = none` 两元素入面，当时分母 44）** → **`0 + 35 = 35`（分母 +1 的组件元素 + 10 份契约在盘，spec 未同批随动）** → **`0 + 36 = 36`（本变更：分母 46、`api = none` 三元素入面）**。末段 MUST NOT 被写成「入口债已清」或「契约债清零」。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 假阳，少于基线 = 假阴，两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释；本段总数与分母都变过，故必须**逐 `kind` 对**，不能只看总数。
- **末行归零的前提（必须说准）**：「全 0」的前提是**按现有实现写契约**——契约里只声明已有的 op / 类型，未写的归 `apiUndeclared`（`off`，不产出）。若反过来「先写契约再补实现」，`apiMissing`（`error`）会立刻红——这正是 `error` 档的用途。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更 SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner，本变更**不新增文件**）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的 **13 份** sidecar 用例承担——十三值各自正反例、两支互斥与首错、pair 锚点三条、`doc_facts` 三态、签名归一、嵌套与透传、生成物渲染两侧键集一致；三值新判据的用例 MUST 落在既有 sidecar（如 `check_test.rs` / `mod_test.rs`）内，**MUST NOT 新增 `api/**` 文件**——`golden_test.rs` 断言「`api/**` sidecar 恰 13 份」，新增文件即撞红。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`；既有 `api-mismatch` case SHALL 保持非空 `apiFindings`；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。
- **首错改写会改动金样读数**：一个元素命中第一条即 `break` ⇒ 「同元素多报」的 case（如 `apiMissing` + `apiReexportDrift`、`apiMissing` + `apiConstructorDeclared`）条目数下降乃**预期**；每条变化 SHALL 在 change 留痕中说明「哪一条因首错被短路」。代码注释里「与 ① 独立、MUST NOT 与 ② 双报」一类纪律句 MUST 同批改写，否则后人会按旧口径写回归用例。
- **语料覆盖的已知缺口（就地留痕）**：三条新判据**不新增 sidecar 文件**（撞上 13 份计数断言），其正反例落在既有文件内；因此「十三判据各自正反例」一句 MUST NOT 被读成「十三判据各有独立语料文件」。兜底只剩两层——crate 级 `corpus/drift/` 金样（不经本仓模型）与本仓实测留痕。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `graph/**`（契约索引 + 模型声明）与 `models/**`（契约资产）是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。`reports/archi/` 是**门禁运行时产物**——不入库、不计入交付面、不作判红面（`git status` 出现 `??` 属预期）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `graph/model/` 存在且一级为六个 `NN-<域>.toml`，但没有任何 `path` 非空元素的载体在盘带 fence 时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `docMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_models_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`，含 `api-op` / `api-types` fence）后重跑
- **THEN** 契约面转为启用（载体带 fence），全部 `path` 非空元素（46 个）参与判据：产出基线表所述 **0** 条 `apiEntryMissing` 与 **36** 条 `docMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线表逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `docMissing = 36`（33 条为 `declared` 且派生契约 md 缺失、3 条为 `api = none` 且 `doc` 未声明——其中一条是本变更新增的生成物中继元素）、`apiEntryMissing = 0`、`apiMissing = 0`、其余十值各 0；10 份契约在盘的元素（`Foundation` / `ArchiRuntime` / 8 份 Agent 组件）零判据；`status = clean`、`passed = true`

#### Scenario: 陈旧读数与当前读数不得并排误读

- **WHEN** 审查 spec 与 change 报告内关于历史段 / 当前段的表述
- **THEN** 当前段 SHALL 写明「36 = 33（declared 契约未写）+ 3（`api = none` 未声明）、分母 46」，并把「此前写的 44 / 43 已陈旧」的成因（分母 +1 的组件元素 + 10 份契约在盘）单列一行；不出现「债务由 43 降到 36」一类表述；`apiMissing` 的归零 SHALL 被解释为「形态①移出」而非「实现漂移清零」

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某「派生契约 md 缺失」的 `declared` 元素漏报 `docMissing`（假阴），或让某契约在盘且含 fence 的元素也被判 `docMissing`（假阳）
- **THEN** 前者使 `docMissing` 由 36 变 35（少于基线）；后者使该桶变 37（多于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 新元素进入判据面只增一条且不改其它桶

- **WHEN** 在模型中新增一个 `path` 非空、`api = none` 且未写 `doc` 的元素（本变更的生成物中继元素即此类）后重跑
- **THEN** 分母 45 → 46、`docMissing` 35 → 36，其余十二值仍 0；该元素零 `apiEntryMissing`（`api = none` 退出入口判据，尽管它 `entry_state = Absent`）；`status = clean`、`passed = true` 逐字不变

#### Scenario: sidecar 计数不变

- **WHEN** 运行 `golden_test.rs` 的「零第二套判据语料目录」用例
- **THEN** `crates/core/archi/src/api/**` 的 sidecar 仍恰 **13** 份（三值新判据的用例全在既有文件内），且 `tests/corpus/api/` 仍零存在

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 `crates/core/archi/src/api/**` 的 sidecar 用例与 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（判据产出由 `check::check` 直驱断言，不依赖快照）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`api-mismatch` case 产非空 `apiFindings`；MUST NOT 存在第三套判定语料目录

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认

### Requirement: 装载期名字清单闸与判据面的分工

名字清单的「散文表格复述」形态 SHALL 由**装载期闸**判定，MUST NOT 进入本能力的判据面；该闸 SHALL 让**判据面**零改动（`api/**` 的求值面——`check.rs` 的输入装配与求值序、`severity()` 三档、`gating_findings` 入选判据、纳管子集函数——逐字不变）。**零改动面的边界（本变更显式收窄）**：`drift/report.rs` 与 `drift/mod.rs` **不在**该面内——本变更在这两个文件里加的 `ArchiDriftKind::UndeclaredApi`（`ALL` 4 → 5）与两桶装配归 `desktop-archi-drift` 登记，属**漂移判定面**而非契约判据面；把整文件写成「逐字不变」即与同批 drift delta 自相矛盾。

- **分工单点**：第四条闸（散文表格名字 ∩ 契约名字集 = ∅）的定义与实现归 `desktop-archi-api`「契约资产形态与唯一手写真相」，错误面为 `ApiError::Contract`（`load_contracts` 中止）。分界理由：判据面报的是「契约 vs 代码的比较结论」（可枚举、可渐进消债、进报告数基线），而「契约文件内部两处持有同一个名字」是**契约资产自身的错**——它与代码导出集无关，也不该被算成一条债进入报告（否则一个错字会同时污染基线表与 `status`）。
- **零新增判据词汇**：`ArchiApiFindingKind` SHALL 保持**十三值**（`ALL` 长度断言不变），`category` 保持七值、`severity()` 保持三档；该闸 MUST NOT 产出 `apiFindings` 条目、MUST NOT 参与纳管子集与 `passed` 判据、MUST NOT 改动 `REPORT_VERSION`（仍 3）。
- **判据输入面 MUST NOT 含散文表格**：十三判据的输入 SHALL 仍为「模型 + 契约 fence / `<FQN>.schema.json` + 代码导出集 + `doc_facts`」；`apiUndeclared` 的声明集 SHALL 仍**只取 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown 散文、MUST NOT 新增表格行读取点（判据面 MUST NOT 补写半个 markdown 解析器——与「`apiDocMissing` 只判布尔、不接 `contract::parse`」同一条纪律）。
- **留白期零判据（显式留白，不是疏漏）**：8 个前端组件元素的契约三项（`ops` / `types` / `reexports`）当前全空，其散文「公共面」表共 **28** 条数据行（8 份契约逐份：6 / 2 / 2 / 3 / 3 / 2 / 2 / 8）、名字已存在于代码导出面而未进契约 —— 该形态在本能力**零判据**：MUST NOT 报 `apiUndeclared`（档位保持 `off`），MUST NOT 为此新增第十四值或第八个 `category`，MUST NOT 为「表里列了未声明名字」另立判据。**`apiUndeclared` MUST NOT 抬档**：留白期抬档会把临时债读成判据面债务，且与本能力「完备性判据报的是海、迁移期必然爆炸必须后置」的分档动机冲突；抬档是独立裁决，触发条件是组件契约到场（届时表格已由第四条闸强制退役）。
- **读数与语料零漂移（读数随仓演进，口径不变）**：本仓当前基线 SHALL 为——`docMissing` **36**（33 + 3）、`apiEntryMissing` **0**、`apiMissing` **0**、其余十值各 **0**、`status = clean`、`passed = true`（既有的「43（41 + 2）」是两次变更之前的读数，成因见「契约判据的验证语料与报告数基线」）；`api/**` sidecar SHALL 仍恰 **13** 份（本变更新用例落在既有 `drift/**` sidecar，MUST NOT 新建 `api/**` 文件）；`corpus/drift/**` 的**既有 16 份金样逐字节零变化**（语料契约 `App.Mod.md` 零表格行），本变更新增的 `api-*` case 是**新增文件**、MUST NOT 改写任何既有快照。
- **零改动面（同批断言）**：`api::check` 的输入面与求值序、`ArchiApiFindingKind::severity()` 三档（`apiUndeclared` 仍 `Off`）、`api::gating_findings` 的入选判据逐字不变；`archi_api` 命令响应与门禁 `LocalArchiCheck` 报告形状不变（`ArchiApiFindingKind` 十三值 / `category` 七值零新增 ⇒ 判据面无需重绑）；`pnpm api:check` / `pnpm check` 链的段序与命令串不变；呈现面 `contract-panel.tsx` / `model-info.tsx` 零重算、零改动。**不在此列的三处**（同批由其它能力登记）：`drift/report.rs` 的 `ArchiDriftKind` +1 值、`drift/mod.rs` 的两桶装配、以及 `bindings.ts` 里 `ArchiDriftKind` 类型的随动——`bindings:export` 的幂等只对**判据面**成立，MUST NOT 写成「生成物整体零 diff」。

#### Scenario: 表格复述契约名字只装载期中止，不进报告

- **WHEN** 某 `api = declared` 元素的契约已声明 op `is_set`，其散文段又有一张以 `` `is_set` `` 起首的表格行
- **THEN** 装载期判 `ApiError::Contract` 并中止；报告面 MUST NOT 产出任何新条目（`apiFindings` 内零该事实、`ALL` 长度仍 13、`category` 七值、`severity()` 三档不变）；`status` / `passed` 的推导路径零改动

#### Scenario: 留白期散文表格零判据

- **WHEN** 8 个前端组件元素的契约三项全空、散文表格共 **28** 条数据行，且这些名字在代码导出面成立
- **THEN** 装载零错误、`apiFindings` 零新增条目；`apiUndeclared` 不产出（档位仍 `off`）；把某元素契约补一个 `api-types` 键 `AgentInput` 后，报错来自装载期（`ApiError::Contract`）而**不是** `apiUndeclared`

#### Scenario: 判据输入面零表格读取

- **WHEN** 审查 `check.rs` 的输入装配、`ApiCheckInput` 的字段面与 `apiUndeclared` 的声明集来源
- **THEN** 零表格行读取点（散文表格的唯一读者是 `contract::verify`）；`apiUndeclared` 声明集只取 `<FQN>.schema.json` 的 `$defs` 键集；`doc_facts` 三态与 `entry_state` 口径逐字不变

#### Scenario: 当前段读数与语料守护零漂移

- **WHEN** 对本仓模型跑一次检查并核对语料面
- **THEN** `docMissing 36`、其余十二值各 0、`status = clean`、`passed = true` 逐项与基线表相等；`api/**` sidecar 仍 13 份；既有 16 份 drift 金样逐字节不变且新增 `api-*` case 全部登记在 `DRIFT_CASES`；判据面（`api/**` 的求值面与纳管子集函数）零差异——`drift/report.rs` / `drift/mod.rs` 的差异**仅**为本变更登记的 `ArchiDriftKind` +1 值与两桶装配，零其它改动（`apiFindings` 槽位、十三值、`severity()`、`status` / `passed` 推导逐字不变）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/**` | 十三判据求值（零改动） | 判据种类、两支首错序、`severity()`、`gating_findings` 与 `doc_facts` 三态逐字不变；`api = none` 元素退出入口判据的语义不变（新元素因此零 `apiEntryMissing`） |
| `crates/core/archi/src/drift/report.rs` | 报告（判据面零改动） | `apiFindings` 槽位、`ArchiApiFindingKind` 十三值、`severity()` 三档、`status` / `passed` 推导零改动；同文件内的 `ArchiDriftKind` +1 值（`undeclaredApi`）与 `ArchiObservationKind` 产出条件由 `desktop-archi-drift` 登记，**不属**本能力的零改动面 |
| 本能力基线表（spec 面） | 改 | 分母 45 → **46**、`docMissing` 35 → **36**（33 declared + 3 none）；历史链补两段并写明「44 / 43 早已陈旧」的成因，MUST NOT 让旧读数与新读数并排而无解释 |
| 本仓读数留痕（change `reports/`） | 新增 | 一次全量 `archi_drift` 的逐 `kind` 计数表（与基线表逐项相等）+ `git status` 显示 `reports/archi/` 未入库；MUST NOT 写入 crate 语料 |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 语料面（只加不改） | 既有 16 份快照逐字节不变；新增 `api-*` case（声明满 / 缺一项 / 多一项 / 未填 `apis`）各自登记进 `DRIFT_CASES`；`api-mismatch` 仍非空 `apiFindings` |
| 门禁留痕行（spec 面既有行） | 读数随动 | 「`apiFindings` 长度 **43**」→ **36**；`status = clean` 与「不入库 / 不作判红面」口径逐字保留 |
