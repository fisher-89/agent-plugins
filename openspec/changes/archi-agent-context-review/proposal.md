# 提案: archi-agent-context-review

> **变更**: archi-agent-context-review
> **日期**: 2026-11-25
> **状态**: 待评估（explore 决策表 Q1–Q11 全部锁定；本提案对 explore §10.1 留待落定的六件事给出裁定，见「提案 · 决策表」P1–P11）
> **来源**: `explore.md`（requirement 类型；11 份 schema / 13 值判据 / `write`·`edit` 执行体 / 工具定义体量均已按源码实测核对；`reports/archi/` 门禁报告与本仓读数逐项对拍）
> **重做说明**: 本稿在相位回溯后重做，五处回溯项已逐一收口——① 纳管锚单锚 **schema**（`api = declared` ∧ `<FQN>.schema.json` 在盘，文档在盘与否零参与，`desktop-archi-drift` 与 `desktop-archi-api-check` 同锚）；② `docMissing` **单因单义**（派生 `<FQN>.md` 不在盘，判据面 ③ 与写面 `DocMissing` 同义），「接口未写」（schema 不在盘）本期**零判据**（`schemaMissing` 归后续 change）；③ `apiUnexpected` 形态②（`declared` 写了 `doc`）已删（`repo-archi-model` 的 `doc` 纪律 1 同步降为约定）；④ `category` 全文统一**七值**；⑤ 能力章节与 11 份 delta 的 requirement 头逐条对齐——`schema.json 与 INDEX.toml 为纯派生且不可手改` 记作 REMOVED、`archi_scaffold` 不设 requirement delta（只落 Module Contract 描述面）、补记 `desktop-archi-agent`「检索面纪律与迁移边界」。
> **二次重做说明**: 上一轮回溯的两项缺陷已在本轮收口——① 三份 delta 里指向**更早变更**的「本变更」断言（`desktop-archi-drift` 的 `modelHash` 段与 `modelHash 随模型变化` 场景、`repo-archi-model` 的生成物中继元素 / 命令面 `desc` / 计数复述三条、`desktop-archi-model` 的 `apis` 第四键与 `doc` 字段两条）改写为「更早变更已落地」的既成事实，并逐处注明**本变更零模型字节改动**（盘面实测：`06` 已含 `Desktop.GeneratedBindings`、55 元素 / 129 关系、`02` / `03` / `05` 已 `composite`），与「零元素 / 零关系」及验收 1 的「含 `modelHash` 逐字段不变」对齐；② 补验收 15（`archi_search` 语料换面）与验收 16（`archi_api` 两格下发），并在测试文件表各补一行对应用例组。

---

## 问题

模块 API 契约的**手写真相**今天是 `<archi_models_dir>/<分层目录…>/<FQN>.md` 里的三类 fenced json（`api-op` / `api-types` / `api-reexport`）：md 同时充当「人读的理由」与「机器读的接口」，`.schema.json` 是它的生成物。一份资产答两个问题，于是两边都不称手。八条缺陷（前四条来自一次架构 Agent 运行复盘，后四条来自代码实测）：

1. **段结构有四个持有者且互相冲突**：`bootstrap` 草稿硬编码六段、作者注说四段、11 份语料的实际段名分布不均（「边界」0 次）、解析器对 `##` 标题零校验。模型把草稿当规格写——复盘归因 ≈60%，是那次文档写坏的主因。
2. **「散文 MUST NOT 用表格复述契约名字」是宽泛禁令**：`contract::verify` 的闸只在 `ops`/`types`/`reexports` 三项全空时整体跳过，放行时也只抓「表格首列整体反引号 + ASCII 标识符」。文书写成无条件禁令，且系统提示、`AGENT.md`、`bootstrap` 作者注四处各写一遍。
3. **读者身份缺两个**：契约 md 同时是产品 UI 内容（架构页右栏「文档」段直接把路径交给 `FileViewerBlock`），而上下文只说它是「机械闭环的真相资产」。
4. **`api` 字段兼任两个资产的存在位**：`paths::carrier_path` 的注释原文「契约 md 与入口文档不是两份资产，只是同一落点的两个用途」；于是 `api = declared` 时 `doc` 字段被禁写（由 `apiUnexpected` 形态②守），接口与理由无法分别缺席。
5. **资产②零写闸**：`.schema.json` 今天只由 `build_schema` 生成，`bootstrap` 只出草稿、零写盘校验；反过来资产③（模型 TOML）有专属写工具（渲染 → 重解析 → 回读 + 乐观锁 + 七条 warnings）。同一份「唯一真相」的两种资产，一边零校验一边全闭环。
6. **`archi_module` 只给片面信息**：`ref_projection` 要求 md 与 schema **同时在盘**才报三条路径，缺一份则三项皆 `null`——投影层把「三份资产各自的存在事实」压成一个开关。
7. **change 臂有写工具、无知识**：`compose.rs` 的 change 臂 `preamble: None`，而 `(proposal|dev-design, executor)` 拿到了两写工具——写作纪律只存在于 archi 臂的知识文本里。
8. **判据面三项死值的来源将随 md 结构消失**：`ArchiApiFindingKind` 十三值里 `apiReexportDrift`（散文透传列表 vs fence 键集）、`apiDocMissing`（契约 md 在盘但无 fence）、`docNotFound`（`doc` 指不到盘）都建在「md 有结构」与「`doc` 是第二个事实」上。

**本仓当前读数**（`reports/archi/20261125T0412296939738Z.json`）：55 元素 / 129 关系、`gate` 全 `off`；`apiFindings` **38 条全 `docMissing`**、`status = clean`；盘上 11 份 `.md` + 11 份 `.schema.json`（2 份非空：`Backend.Core.Foundation` 9 op / `Backend.Infra.ArchiRuntime` 2 op + 1 def；9 份空壳）。**11 份 schema 的内容与对应 fence 逐项等价** ⇒ 切换真相源零迁移。

## 提案

六组一次收口：**真相搬移 → 写面补闸 → 判据被动收缩 → 工具面 → Agent 上下文 → 生成面与规则面**。

```
今天                                      改后
md(散文 + api-* fence) ──parse──► schema   md(纯散文) ──────────────► 人类 + AI 共读
   │ build_schema                          │
   └──► <FQN>.schema.json ★生成物          <FQN>.schema.json ★手写真相（唯一凭据）
        └─► 判据面                          ├─► 写闸 / 装载校验（同一实现）
                                            └─► .i.rs / .i.ts / INDEX.toml ★生成物
```

1. **读面换真相（零 md 回落）**：`load_contracts` 只读 `<FQN>.schema.json` 并经新增的 `Contract::from_schema` 装配 `Contract`；schema 不在盘 ⇒ 跳过（无判据）；schema 在盘但坏 ⇒ `ApiError` **停摆**（与模型结构错同族，不产半真报告）。`contract::parse`（约 600 行 md 解析器）与 `schema::build_schema` 退役；`verify` 的七族名字闸 / `x-function` 落点闸 / `required ⊆ properties` 闸与 `schema::verify_defs` **零改造复用**，只改输入来源。
2. **写面补闸（坏文本不可产出）**：新增 `validate_schema(fqn, text)`——文本层重复键扫描（`serde_json` 会把重复键折叠成「后者胜」）→ JSON 可解析 → identity（`$id` / `title` / 文件主干三者一致）→ 键空间（`properties` 与 `$defs` 无重名）→ 形态白名单（`properties.<k>` 只允许 `{x-function: …}` 或 `{$ref: "#/$defs/<名>"}`，且后者 `<名> ∈ $defs`）→ 镜像（`$defs` 每个键在 `properties` 有指向自己的 `$ref`）→ 跨元素 `$ref` 前缀逐字等于 `ref_relative(本FQN, 目标FQN)` → 零未知顶层键 → 复用三闸。`write` / `edit` 落 `<archi_models_dir>/**/<FQN>.schema.json` 时先过它，失败即 `Err` 零落盘；`load_contracts` 调**同一个函数**（只差错误映射：`Err` 字符串 vs `ApiError`）。跨元素的一切（目标是否直接子元素 / 是否 `api = none` / 是否在盘）仍归 `apiRefMissing` 既有判据面——写闸只判「文件级自洽」。
3. **判据被动收缩 13 → 10**：删 `apiReexportDrift`（无散文面）/ `apiDocMissing`（无 fence 面）/ `docNotFound`（`doc` 惰性化）；`docMissing` 收成单成因（派生 `<FQN>.md` 不在盘，与 `api` 无关）；`apiUnexpected` 收成单形态（`api = none` ∧ schema 在盘）；`category` **七值不变**（`doc` 只剩 `docMissing`，七组无一组被清空——explore §4.2 的「7 → 6」算术不成立，本提案按「成员收缩、分组集合不变」落地）；`severity` 的 `error` 三项 → **两项**（`apiMissing` / `apiSignatureMismatch`）。求值链随之由「none 3 步 / declared 6 步」收成「none 2 步 / declared 4 步」——③ 仍是「派生 `<FQN>.md` 不在盘」（`docMissing` 单因），**「接口未写」（schema 不在盘）不占判据步**：它在 ⑤⑥ 表现为「声明集不存在 ⇒ 零判据」，`schemaMissing` 归后续 change。纳管子集（`gating_findings`）改判 **`api = declared` ∧ schema 在盘**（Q9）：文档在盘与否退出纳管口径，文档债恒 `warn`、永不纳管。
4. **工具面一次给全三资产**：`archi_module` 增 `assets`（`topology` / `interface` / `doc`，三者各自 `present`，不搞全或全无）+ `reading`（静态指引，写在响应里而不是 prompt 里）；`archi_api` 的 `bootstrap` 改**双草稿**（`{ schema, doc }`，顺手消灭复盘 #1：没有段骨架可被当成规格）、新增 `link` 动作（出正确 `$ref` 串，纯计算零读盘）；`archi_scaffold` 只改描述一句。`archi_api` 的下发面 **补 `(proposal, executor)` / `(dev-design, executor)` 两格**——起草发生在哪，草稿工具就必须在哪。
5. **Agent 上下文重写**：`archi-knowledge.md` 改「三资产分工 / 设计顺序 / 三条不可越界 / 留给判断 / 兼容一句」；change 臂在**写权放开**时注入同一份知识（判据单点复用 `model_write_lock(provenance) == None`，与写工具下发同集）；`proposal-planner` / `dev-design-planner` 补「先拓扑、后接触面、最后理由」与「接口面留白写空 schema」；`implementation-generator` 补一句「改了导出面就同步接口定义」。
6. **生成面让位 + 规则面同步**：`render_all` 不再产 `.schema.json`（三类生成物），`diff::api_dir_artifacts` 收面去掉 `*.schema.json`（不去掉 = 11 份手写文件全进 `orphaned`，症状是「全红」）；`INDEX.toml` 头注释改写；`AGENT.md` / `CLAUDE.md` 的「模块 API 契约」节由「四类生成物 / 三类 fence 为唯一真相」改为「三类生成物 / md 与 schema 分工」，散文禁令收窄为「文档不列接口名（唯一持有者是 schema）」。

### 决策表（explore Q1–Q11 照抄 + 本提案新落定 P1–P11）

| 编号 | 决策 | 值 |
|---|---|---|
| Q1 | schema 写闸失败形态 | `Err` 零落盘（与 `archi_element`「坏文本不可产出」同族） |
| Q2 | `bootstrap` doc 草稿是否给段落建议 | 不给（只 H1 + 一句「按内容自定，先例在同类元素」） |
| Q3 | 历史 md fence 的清理 | 本变更不处理，用户后续专门一轮 |
| Q4 | 读面是否回落到 md | 不回落，schema 是接口唯一凭据 |
| Q5 | 理解模块的读面 | 一款工具一次给全三份资产（`archi_module.assets`） |
| Q6 | 资产搬迁 | 不做（schema 内容与 fence 等价 ⇒ 切换真相源零迁移） |
| Q7 | 遗留 md fence 是否读数 / 提示 | 不需要：fence 视为普通文档内容，`archi_api check` 零新增计数 |
| Q8 | 手写 `$ref` 算错怎么办 | `archi_api` 新增 `link` 动作出正确串 + 描述明写「勿手拼」 |
| Q9 | `gating_findings` 锚点 | schema：`declared` ∧ schema 在盘 |
| Q10 | `apiUnexpected` 形态② | 不做检查（形态② 删除，`doc` 字段零**判据**读者：判据 / 纳管 / 派生 / 检索都不读它） |
| Q11 | op 的散文说明放哪 | `x-function.description`（内层，自由形状不做未知键校验） |
| **P1** | `validate_schema` 的落点与命名 | 校验主体落 `api/schema.rs`（`validate_schema(fqn, text) -> Result<Contract, ApiError>`），`contract.rs` 只留 `Contract` 投影类型与可复用三闸；`parse` 与 `build_schema` 退役后 `contract.rs` 由「md 解析器」退为「schema 校验 + `Contract` 投影」（约 1095 → 400–500 行） |
| **P2** | `archi_module` 的 `api` 字段 | **保留 `ArchiApiRef` 四字段**（`status` / `contractPath` / `schemaPath` / `generatedPath`），不做 breaking 改名——`contractPath` 一直是 doc 路径，在 JSDoc 点明即可；`ref_projection` 改为**三条路径各自按自身在盘事实出线**（今读零差），`status` 改判「schema 在盘」。改名留清理轮 |
| **P3** | 前端是否跟着改 | 零功能改动：右栏「文档」段本就只渲非空项；契约面板的 `category` chip 由 `apiFindings` 数据聚合（已核实 `contract-panel.tsx` 零前端词表）。仅两处 JSDoc / 文案随字段语义同步 |
| **P4** | `archi_api` 的下发面 | 补 `(proposal, executor)` / `(dev-design, executor)` 两格（只读档，不扩写权）。理由：`bootstrap` 双草稿与 `link` 的价值恰在起草相位；不补则 `archi_scaffold` 描述里那句「接口草稿用 `archi_api` 的 `bootstrap`」指向一个未下发的工具 |
| **P5** | change 臂知识注入条件 | `model_write_lock(provenance) == None`（= 两个写权相位），**单一判据**、与两写工具下发同集。`implement/executor` 虽命中 archi 只读工具但**不注入**——那份知识包含「写模型」的纪律，注入与相位锁自相矛盾（explore §4.6 表项 2 的收窄，理由在此） |
| **P6** | `INDEX.toml` 的 form status 词 | **值域不动**（`declared` / `absent`），`present` 改判「schema 在盘」；命名债（`declared` 读起来像意图）并入清理轮。这样 `.i.rs` / `.i.ts` / `INDEX.toml` 三份生成物 **逐字节不变**（explore §5.1 的验收前提） |
| **P7** | `bootstrap` 的 CLI 出口 | 与工具出口同形：打印单行 JSON `{ "schema": "…", "doc": "…" }`（`api::bootstrap` 返回两段文本的结构体，两出口共用） |
| **P8** | 空 schema 的顶层键 | `$id` / `title` / `$defs` / `properties` 四键 MUST 在场（identity 与形态闸的对象；空壳 = 两个空对象），`$schema` / `$comment` 可选；其余顶层键一律拒 |
| **P9** | `api` / `doc` 字段 | `api` 保持二值（不新增 `deferred`）；`doc` 保留但零判据读者（判据 / 纳管 / 派生 / 检索都不读；`archi_module` 投影与画布「入口文档」段仍作呈现事实读），外部工作区写了 `doc` 不会变成 `unknownField` |
| **P10** | 11 份存量 `.schema.json` 的 `$comment` | 本变更**不批量改写**（它不参与任何判据，批量改会让 11 份 diff 与语义无关）；新骨架带「写者纪律」注解，存量文案由清理轮统一处理——期间由知识骨架一句说明 |
| **P11** | `docMissing` 的事实面与「接口未写」的归属 | **单因单义**：判据面（③ 与 `none` 支 ②）与写面 warnings 的 `DocMissing` 都判**派生 `<FQN>.md` 不在盘**（与 `api` 无关；写面只由 `created` 诱发，避免既有元素的噪声复读）。**「接口未写」（schema 不在盘）本期零判据**——判据面在 ⑤⑥ 表现为「声明集不存在」、写面零 warning；`schemaMissing` 明属后续 change，MUST NOT 以期内的别名或写面词预铺 |

### 明确不做（知情边界，非遗漏）

- `schemaMissing`（`declared` ∧ schema 不在盘）、`schemaInvalid`（装载失败落判据而非停摆）、`edgeApiUnresolved`（`relation.apis ⊆` 目标接口键集）——归后续 change（`archi-api-findings-redesign.md`）。
- `api` 第三值（`deferred`）与 `doc` 字段退役——后续 change（P9 只保证今天不倒退）。
- `archi_element` 的 schema 减负（7.8 KB → 数 KB）、`AGENT.md` 按来源裁剪注入、续接首轮注入 `drift summary`、画布选中元素注入会话（UI spotlight）、`preclose` T1 补两写工具名——独立优化，本变更不碰。
- 历史 md fence 的清理与 11 份 `.schema.json` 的 `$comment` 改写——用户后续专门一轮（Q3 / P10）。
- 11 份模块文档（`.dev-team/architecture/models/**/*.md`）的正文改写——本变更只改**机制**；文档是否补写「三资产分工」属 doc 层面的数据变更，随清理轮或独立 change。
- 模型元素 / 关系（55 / 129）零改动；`drift` 的扫描、归属、定级、豁免、配置面零改动；写入保护（内置锁 + 相位锁）零改动。

## 能力

### 新增的能力

无。三资产分离、schema 写闸、判据收缩、工具面与知识注入全部落在既有架构能力边界内（不新建 `desktop-archi-three-asset` 一类能力——它会与 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-agent` 造出跨三能力的悬空契约）。

### 修改的能力

- **desktop-archi-api**（机制主体）— MODIFIED「契约资产形态与唯一手写真相」（真相分工 / md 无结构 / 启用判据 / `doc` 与 `apiUnexpected` 口径）；REMOVED「schema.json 与 INDEX.toml 为纯派生且不可手改」（`<FQN>.schema.json` 由生成物升为**手写真相**，与本条核心语义相反；`INDEX.toml` 仍是纯派生，其口径由同批 MODIFIED「契约资产形态与唯一手写真相」与「生成物入仓与零 diff 门挂载」承接）；MODIFIED「引用与命名规则」（来源换面、跨元素 `$ref` 前缀）；MODIFIED「关键字集、类型映射与错误面」（`x-function` 落点改指 `properties.<k>.x-function`、注解载体三选、`x-` 面封闭）；MODIFIED「生成物入仓与零 diff 门挂载」（四类 → 三类、孤儿扫描收面）；MODIFIED「仓库根文档的契约纪律节」（`AGENT.md` / `CLAUDE.md` 五条要点重写）；MODIFIED「生成器、verify 闭环与 bootstrap 剪枝」（CLI `generate` 三类产物、`bootstrap` 出口形状）；MODIFIED「模块嵌套与选择性透传」（透传真相改为 schema `$ref`、`apiReexportDrift` 退役）；REMOVED「bootstrap 草稿的六段骨架与惰性作者注」；ADDED「bootstrap 双草稿（schema 骨架 + doc 骨架）」；ADDED「接口定义文件的装载校验与写闸（零第二份校验）」。
- **desktop-archi-api-check**（判据面）— REMOVED「契约判据词汇封闭十三值与开放分类」/ ADDED「契约判据词汇封闭十值与开放分类」；REMOVED「十三判据的定义与求值互斥顺序」/ ADDED「十判据的定义与求值互斥顺序」；MODIFIED「判据级档位与元素 gate 解耦」（`error` 两项）；MODIFIED「报告并列槽位 apiFindings」（纳管 = `declared` ∧ schema 在盘）；MODIFIED「契约判据的验证语料与报告数基线」（38 / 49、历史链补一段）；REMOVED「装载期名字清单闸与判据面的分工」（散文表格闸随 md 结构一起消失）。
- **desktop-archi-agent**（工具面与知识面）— MODIFIED「archi_module 只读工具的定位契约」（`assets` / `reading` / 描述三句）；MODIFIED「archi_search 只读工具的轻量检索契约」（语料换成「在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`」两条固定规则，与 `api` 状态无关）；MODIFIED「archi_api 只读工具的自检与草稿契约」（三动作 + 双草稿 + `link`）；MODIFIED「archi 工具面注入与 (phase, role) 矩阵」（`archi_api` 两格）；MODIFIED「架构会话的系统知识注入」（五段骨架 + 体量锚重定）；MODIFIED「模型 = 源码（两条写入口与写纪律）」（三类生成物 + schema 手写面写闸为**唯一显式例外**）；MODIFIED「检索面纪律与迁移边界」（语料面两条固定规则换成「在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`」，`doc` 字段退出语料，迁移三条边界逐字保留）。`archi_scaffold` 的描述句改动与 `archi_module` / `archi_api` 的 `mount.rs` 描述面同批落在本能力的 Module Contract 段——它们**没有独立 requirement delta**（`archi_scaffold` 的字段词典正文与形态零改动）。
- **desktop-archi-contract-panel**（呈现面）— MODIFIED「检查命令契约（archi_api_check）」（`enabled` = schema 在盘、冷启动应生成集）；MODIFIED「两轴并列呈现与状态矩阵」（`doc` 组只剩 `docMissing`、chip 集合随数据收缩）；MODIFIED「元素信息面板契约段」（三路径各自独立出线）。
- **desktop-archi-canvas** — MODIFIED「结构错的停摆态（全或无）」（`apiFindings` 词汇引用收窄，`docNotFound` / `apiDocMissing` 离场）。
- **desktop-agent-execution**（写闸装配与注入）— ADDED「`write` / `edit` 的接口定义写闸（坏文本不可产出）」；MODIFIED「会话级工具面注入与 explore 笔记工具」（change 臂 `preamble` 生产条件）；MODIFIED「SDK 引擎系统提示词（AGENT.md 注入）」（知识段生产者增一臂）。
- **desktop-phase-prompts** — MODIFIED「桌面端特性适配（迁移是改写非搬运）」（archi 工具按角色清单加 `archi_api`、设计顺序句、schema 同步句）。
- **desktop-ipc-type-bindings** — ADDED「archi 模块三资产的出线 DTO 面」（`ArchiModuleDetail` 增 `assets` / `reading` + 4 个新 DTO 类型入两份清单守卫）。
- **desktop-archi-drift**（引用同步）— MODIFIED「报告结构与 status 五值」：正文里的「契约判据十三值」「39 条 / 36 条」与两处 `apiDocMissing` 场景句随判据面换锚（十值、38 条 = 35 条 `declared` 缺派生 md + 3 条 `api = none` 缺派生 md）；`apiFindings` 并列槽位的纳管锚由「入口文档承诺已成立」改写为「**接口承诺已成立**」（`declared` ∧ schema 在盘），「把 schema 移走 ⇒ 该元素零判据」替换旧「移走 ⇒ 退化 `docMissing`」；报告字段面 / `derive_status` / `REPORT_VERSION` 零改动。
- **desktop-archi-model**（`doc` 惰性化与写面 warnings）— MODIFIED「架构模型文件格式与字段封闭」（`doc` 的「只有 `none` 可写」降为约定、`doc` 零判据读者）、MODIFIED「消费面词汇集一致性对拍」（十三 → 十值引用）；REMOVED「写面反馈判据（七条 warnings 与降噪不变式）」/ ADDED「写面反馈判据（五条 warnings 与降噪不变式）」——三条 `doc` 事实随字段惰性化删除；`DocMissing` 仍判**派生 `<FQN>.md` 不在盘**（单因、与 `api` 无关，写面只由 `created` 诱发），`ApiUnexpected` 改判 `api = none` ∧ schema 在盘；`DocNotFound` 写面 warning 消失；`api` 字段不再兼任「入口文档载体开关」（形态②检查随 Q10 删除）。
- **repo-archi-model**（基线读数同步）— MODIFIED「基线落位与模型文件划分」（`api` 的角色由「入口文档载体开关」改为「接口承诺开关」；`doc` 的「只有 `api = none` 可写」**降为约定**（写了不报，形态②已删）；资产面表述改为「`.md` 理由 + `.schema.json` 手写接口」；三处 `api = none` 与全表 38 条的成因统一为「派生 md 缺位」；39 → 38）、MODIFIED「图自洽与停摆禁令」（38 条、其余九值 0、删 `docNotFound` 断言）。

其余能力零改动，理由：

- **desktop-archi-drift 的扫描 / 归属 / 三桶 / 豁免 / 配置面**：本变更不动代码与模型，`apiFindings` 的并列槽位形状与 `Model` 计数不变（`REPORT_VERSION` 仍 3）。
- **desktop-archi-model 的格式 / 字段面 / 写回原语 / 九类诊断**：字段集与 `deny_unknown_fields` 面零改动；本变更只改 `doc` 的读者面与写面 warnings 的条件表。
- **repo-archi-model 的元素与关系**：55 元素 / 129 关系 / `gate` 全 `off` / 三处 `api = none` 零改动。
- **desktop-write-protection**：schema 写闸是「坏文本不可产出」而非「谁可以写」——相位锁、内置锁、shell 写面提取逐字不变；写闸不属保护面，MUST NOT 并进 `ProtectRules`。
- **desktop-archi-file-tree / desktop-archi-chat**：文件树只消费 `OwnerIndex` 归属、会话归属与取数纪律零改动；本变更不动画布渲染与取数纪律。
- **desktop-archi-gate / workspace-layout-resolution / desktop-crate-layout**：无新路径派生函数（schema 目录仍取 `archi_models_dir`）、无新 crate、无新依赖边。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | `load_contracts` 只读 schema（零 md）；`enabled` / `facts_enabled` 改判「任一 `api = declared` ∧ `path` 非空元素的 schema 在盘」；`gating_findings` 改判 `declared` ∧ schema 在盘；`ref_projection` 三路径各自独立 + `status` 改判 schema 在盘；`bootstrap` 返回双草稿结构体（`bootstrap_text` 拆 `schema_draft` / `doc_draft`）；`render_all` 不再产 schema；`GENERATED_NOTICE` 语义改写为写者纪律注（新骨架用） |
| `crates/core/archi/src/api/contract.rs` | md 解析器退役（`parse` / `finish_section` / `parse_object*` / `collect_reexport_prose` / `collect_surface_table` / `file_title` 等约 600 行）；`Contract` / `ContractOp` / `ContractType` 保留，增 `Contract::from_schema`（含 `x-function` **解包**、丢弃 `description`）；`duplicate_keys` 保留供写闸；`verify` 的七族名字闸 / 落点闸 / `required ⊆ properties` 闸保留、输入改判 schema 值；`reexport_prose` / `sections` 字段删除；`FENCE_OP` / `FENCE_TYPES` 随解析器退役 |
| `crates/core/archi/src/api/schema.rs` | `build_schema` 退役；`verify_defs` 零改造保留；新增 `verify_identity`（`$id` / `title` / 文件主干）、`verify_keyspace`、`verify_shape`（形态白名单）、`verify_mirror`（`$defs` ↔ `properties` 镜像）、`verify_ref_prefix`（`ref_relative` 逐字相等）、`verify_top_level_keys`（`$schema` / `$id` / `title` / `$comment` / `$defs` / `properties` 之外拒）；新增**唯一公开入口** `validate_schema(fqn, text) -> Result<Contract, ApiError>`（九道闸一趟走完）；`build_index` 头注释改「手写 / 生成分界」 |
| `crates/core/archi/src/api/paths.rs` | `carrier_path` 退役（资产① 落点恒为派生 `<FQN>.md`，与 `api` 无关）；`doc_path` 退役（唯一读者是 `carrier_path` 的 `none` 分支）；`is_repo_relative` 的可见性按需回收（无读者即删）；`contract_path` / `schema_path` / `index_path` / `ref_relative` / `entry_state` 逐字不变 |
| `crates/core/archi/src/api/diff.rs` | `api_dir_artifacts` 收面去掉 `*.schema.json`（只留 `<archi_graph_dir>/INDEX.toml`）；`orphaned` / `mismatched` / `missing` 三列语义不变 |
| `crates/core/archi/src/api/check.rs` | 两支求值链收窄（`none`：`apiUnexpected` → `docMissing`；`declared`：`docMissing` → `apiEntryMissing` → `apiMissing` → 一致性三条）——③ 判**派生 md 不在盘**（`docMissing` 单因），schema 不在盘 ⇒ ⑤⑥ 无声明集、零判据；`doc_facts` 收成 `doc_present: bool`；`apiUnexpected` 单形态；删 `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三支 |
| `crates/core/archi/src/edit/**`（写面 warnings 推导） | 条件表 7 → **5 条**：删三条 `doc` 事实（`none` ∧ `doc` 空 / `doc` 指不到盘 / `doc` 诱发 `ApiUnexpected`）；`DocMissing` 与判据面同义——判**派生 `<FQN>.md` 不在盘**（单因，写面只由 `created` 诱发，避免既有元素的噪声复读）；`ApiUnexpected` 判 `api = none` ∧ schema 在盘；路径派生用 `contract_path`（文档）+ `schema_path`（接口），`doc_path` 退役；降噪不变式补「只改 `doc` ⇒ warnings 恒空」 |
| `crates/core/archi/src/model.rs`（`doc` 字段注释与 `api` 语义注释） | `doc` 的「只有 `api = none` 可写」降为**约定**（无机械判据）、字段零判据读者；`api` 的语义注释去掉「入口文档载体开关」一层，改为「接口承诺开关」（资产① 落点恒为派生 `<FQN>.md`）；字段集与八字段顺序零改动 |
| `crates/core/archi/src/drift/report.rs` | `ArchiApiFindingKind` 十三 → **十值**（`ALL: [Self; 10]`）；`as_str` / `severity`（`error` 恰两项）/ `category` 打标随动（`doc` 只剩 `docMissing`，分组集合仍七值） |
| `crates/core/archi/src/dto.rs` | `ArchiModuleDetail` 增 `assets: ArchiAssetSet` + `reading: String`；新增 `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`（4 个出线类型）；`module_detail` 装配三资产（`interface` 读 schema 键名、`doc` 读 `##` 段名）；`search` 语料改为两条在盘资产路径；`ArchiApiRef` 四字段与 JSDoc 语义同步（`contractPath` = doc 路径） |
| `crates/infra/agent/src/sdk/tools.rs` | `write(input)` / `edit(input)` 增 `root` 入参（两个调用点）；落盘前若 `path` 命中 `<root>/<archi_models_rel_dir>/**/<FQN>.schema.json` 则调 `archi::api::schema::validate_schema`，`Err` 零落盘；成功响应追加「接口已校验」一行 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `archi_module` 响应透传 `assets` / `reading`（DTO 单点投影，零拼装）；`archi_api` 增 `link` 一支（`from` / `to` / `name` 或 `element` / `name` → `$ref` 串，纯计算零读盘）；`bootstrap` 回 `{ schema, doc }`；`SCAFFOLD_TEMPLATE` 描述句按接口面更新（词典正文不动） |
| `crates/infra/agent/src/sdk/mount.rs` | 三处描述同步：`archi_module`（三资产一次给全 + 「理解模块先调本工具」+ 「文档整读、接口不必整读」）、`archi_api`（三动作 + 双草稿 + 勿手拼 `$ref`）、`archi_scaffold`（「接口草稿用 `archi_api` 的 `bootstrap`」）；参数 schema 与档位不变 |
| `crates/infra/agent/src/compose.rs` | change 臂 `preamble` 生产条件：`model_write_lock(provenance) == None` 时注入 `archi_knowledge::knowledge()`（与 config 臂同一「尽力增强、失败降级 `None`」语义）；archi 臂与 config 臂逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` | 骨架重写：三资产分工 / 设计顺序（定位 → 拓扑 → 两份资产）/ 三条不可越界（文档不列接口、schema 不写理由、TOML 不造空元素）/ 工具面指路（读五写二 + 接口面三动作）/ 留给判断 / 兼容一句（历史 fence 读面不读、清理中） |
| `crates/infra/agent/src/archi_knowledge.rs` | 派生段改写：契约资产一句改为「`<FQN>.md`（理由）+ `<FQN>.schema.json`（接口）」两条同目录资产；两处目录派生与降级语义不变 |
| `crates/core/workflow/src/prompts/proposal-planner.md` / `dev-design-planner.md` | archi 段追加「先拓扑、后接触面、最后理由」三句 + 「接口面留白写空 schema（`$defs {}` + `properties {}`），不编造导出」；工具清单补 `archi_api`（`bootstrap` 取双草稿 / `link` 算 `$ref`）；零路径模板、零字段长表 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 一句「改了导出面就同步该元素的接口定义（`.schema.json`），改完用 `archi_api` 的 `check` 自查」 |
| `crates/tools/archi-api-gen/src/main.rs` | `generate` / `--check` 收窄到三类产物；`bootstrap` 打印双草稿 JSON（P7）；三子命令面与退出码语义不变 |
| `AGENT.md` + `CLAUDE.md` | 「模块 API 契约」节五条要点重写：① 真相分工（`<FQN>.md` 写理由、`<FQN>.schema.json` 写接口；文档 MUST NOT 枚举导出名）；② 三类生成物勿手改；③ 两脚本与 `check` 链段序；④ `api = none` 是显式留白（空 schema 是其正规形态）；⑤ 纳管口径（`declared` ∧ schema 在盘；`error` 档进 `passed`）。两份该节正文逐字相同 |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` 现值 +1 patch（`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行不动） |
| `.dev-team/architecture/graph/INDEX.toml` | `api:export` 重生成（头注释「手写 / 生成分界」一句变；值域与条目字典序不变 ⇒ 除该句外逐字节相同） |
| `packages/desktop/src/views/architecture/**`（JSDoc / 文案两处） | `architecture-node.tsx` 文档段注释与 `model-info.tsx` 契约段注释改成「三份资产各自在盘事实」；零功能改动 |

### 测试文件

| 文件 | 改动 |
|---|---|
| `crates/core/archi/src/api/{mod,contract,schema,check,diff}_test.rs` | 装载只读 schema 的正反例；`from_schema` 直接对拍「`ops[i].signature` == `schema.properties[x].x-function`（解包后）」（explore §10.4 的回归锚）；九道闸各一反例；坏 schema 停摆；`enabled` / `gating_findings` 新口径；`render_all` 三类产物 |
| `crates/core/archi/src/api/paths_test.rs` | `carrier_path` / `doc_path` 零命中（已退役）；`contract_path` / `schema_path` / `ref_relative` 逐字不变 |
| `crates/core/archi/src/edit/**` 的侧车语料 | 五条 conditions 逐条正例（第三条只在 `created = true` 时成立；既有元素 upsert 不产 `docMissing`）；「只改 `doc` ⇒ warnings 恒空」；`DocNotFound` 零出现；`ArchiApiFindingKind::ALL` 十值 / `ArchiDriftKind::ALL` 五值 |
| `crates/core/archi/src/drift/{report,mod}_test.rs` | 十值三方对拍（`ALL` 长度 10）；`category` 七值（成员收缩）；`error` 恰 `apiMissing` / `apiSignatureMismatch`；纳管 = `declared` ∧ schema 在盘；`status == errors ⟺ passed == false` 不变量 |
| `crates/core/archi/src/dto_test.rs`（或既有 module_detail 用例） | 三资产各自 `present` 独立（构造「只有 schema」「只有 md」「都没有」三态夹具）；`interface` 键名摘要与 `abstract` 四形态；`doc.headings` 段名与 `lines` / `bytes` |
| `crates/core/archi/src/dto_test.rs`（`search` 语料换面，同文件另一用例组） | 四态夹具 × 两值 `api` 逐格：命中集恰为在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`；`api = none` 只有文档 ⇒ 文档命中、接口不命中；`api = declared` 只有 schema ⇒ 接口命中；`doc` 写值与不写值两例结果相同；`scope = elements` / `doc` 的另一半为空数组、`docs` 与空 `query` 显式 `Err`；`decisions/**` / `openspec/specs/**` / `path` 内散落 md 零命中；源码零 `carrier_path` / `doc_path` 引用 |
| `crates/core/archi/tests/golden_test.rs` + `tests/corpus/drift/**` | 金样按新判据词汇与 `docMissing` 文案重写（`DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工确认）；`api-mismatch` case 仍产非空 `apiFindings` |
| `crates/infra/agent/src/sdk/tools_test.rs`（或归 `archi_tool_test.rs`） | 写闸三类坏输入各一条 `Err` 且盘面零变化（坏 JSON / `$id` 不符 / 键空间冲突）；合法 schema 落盘 + 「接口已校验」；非 schema 路径零校验（原样落盘） |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | `link` 两形态（跨元素 / 本模块）出的串被 `apiRefMissing` 判可达；`bootstrap` 双草稿形状；`archi_module` 响应含 `assets` / `reading` |
| `crates/infra/agent/src/sdk/mount_test.rs` | 三处描述断言随新描述更新（零字段长表、零项目专用命令两条既有禁令保持绿） |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 新骨架的断言表重写（三资产分工 / 设计顺序 / 三条不可越界 / 兼容句在场；零围栏、零字段形态教学、零诊断词、零 `<>` 占位符）；体量锚按实测冻结重定 |
| `crates/infra/agent/src/compose_test.rs` | change 臂：两写权相位 `preamble` 在场、`implement/executor` 与 evaluator 相缺席；与 `model_write_lock` 逐格同集 |
| `crates/infra/agent/src/compose_test.rs`（矩阵扩面，同文件另一用例组） | `(proposal, executor)` / `(dev-design, executor)` 两格的名单含 `archi_api`（前者六名、后者加 `archi_drift`）且 definitions 与 `RunStarted.tools` 逐名一致；`(dev-design, evaluator)` / `(test-design, executor)` / `(code-review, evaluator)` / `(acceptance, evaluator)` 与矩阵外相位不含；「含 `archi_element`」⟺ `model_write_lock == None` 逐格仍相等、`archi_api` 档位仍 `Read`、`READONLY_TOOLS` 三工具名零变化（扩面不扩写权） |
| `crates/core/workflow/src/prompts/**`（既有 prompt 断言面，若有） | `proposal-planner` / `dev-design-planner` / `implementation-generator` 三份的新句断言；其余 11 份零 archi 工具字样仍绿 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | `DTO_TYPES` / `ARCHI_DTO_TYPES` 各追加 4 个新类型（按字典序插入）、计数断言同批更新、`COMMAND_NAMES` / `COMMAND_WRAPPERS` 零变化；`ArchiApiFindingKind` 联合三值离场（`apiReexportDrift` / `docNotFound` / `apiDocMissing`） |
| `packages/desktop/src/views/architecture/components/{model-info,architecture-node}.test.tsx` + `dashboard/contract-panel.test.tsx` + `hooks/*.test.ts` | DTO 夹具补 `assets` / `reading`；`category` 夹具按新成员集收缩（七组不变）；契约面板五态与 chip 断言随新载荷（零功能改动，仅载荷面随动） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/graph/model/*.toml`（55 元素 / 129 关系） | 三资产不动模型；本变更零元素 / 零关系 |
| `.dev-team/architecture/models/**/*.md`（11 份契约文档正文与历史 fence） | Q3：清理归用户后续专门一轮；本变更只改读面（fence 视为普通内容） |
| 11 份 `.schema.json` 的正文（含 `$comment` 文案） | P10：切换真相源零迁移（内容与 fence 等价），批量改写属清理轮 |
| `crates/core/archi/src/api/emit/{mod,rust,ts}.rs` 与 `extract/**` | 渲染面与取数面逐字不变（`.i.rs` / `.i.ts` 逐字节不变的机械前提） |
| `crates/infra/archi/src/api_artifacts.rs`（写盘单点） | 只少产一类文件，写盘实现零改 |
| `crates/core/archi/src/drift/{mod,config}.rs` | 扫描 / 归属 / 定级 / 豁免 / 配置面零改；判据装配只换输入来源 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 命令签名与出参形状零改（`ArchiModuleDetail` 的字段追加经 DTO 自动出线） |
| `src-tauri/Cargo.toml` 的 crate 版本行 | 版本纪律：只有 `package.json` 走 patch |
| 写入保护面（`sdk/protect/**`、`policy.rs`、`SessionProtectRule`） | 写闸是「坏文本不可产出」，不属保护面 |
| 前端组件功能代码（`contract-panel.tsx` / `model-info.tsx` / `architecture-node.tsx` 的渲染与取数） | P3：chip 由数据聚合、文档段只渲非空项——零功能改动，仅注释 / 文案随动 |
| `openspec/specs/**` | 基线由归档合并维护；本 change 只写 `specs/` 增量 |

## 验收标准

1. **零行为回归（读面）**：`.dev-team` 根重跑 `archi_drift`，除 `timestamp` / `durationMs` 与 38 条 `docMissing` 的 `message` 文案外**逐字段不变**——`apiFindings.total` 仍 38、`byCategory` 仍全 `doc`、`status = clean`、`passed` 不变。
2. **生成物零漂移**：`api:check` 零差异（`mismatched` / `orphaned` / `missing` 三列与改前相同，且 `orphaned` **不含**任何 `.schema.json`）；`.i.rs` / `.i.ts` 逐字节不变；`INDEX.toml` 的差异只允许头部说明段一句。
3. **绑定面可归因**：`bindings:export` 后的 `git diff` 只含 `assets` / `reading` 与 4 个新 DTO 类型（外加 `ArchiApiFindingKind` 联合三值离场）；命令包装零新增。
4. **写闸三类坏输入**：坏 JSON / `$id` 与文件名不符 / `properties` 与 `$defs` 重名各产一条 `Err`，且盘上字节零变化；同一份文件随后以合法内容写入成功并回「接口已校验」。
5. **写闸与装载同源**：把写闸放行的合法 schema 手工改成坏 schema（绕过 `write`）后跑 `archi_drift` ⇒ `Err` 停摆而非产出半真报告（`ApiError` 文案点名文件与期望值）。
6. **三资产一次给全**：`archi_module(element = X)` 响应含 `assets.topology` / `assets.interface` / `assets.doc` 三段与 `reading`；三段 `present` 各自独立（「只有 schema」「只有 md」「两者都无」三态夹具各验一次）。
7. **双草稿与 `link`**：`bootstrap` 返回 `{ schema, doc }`，schema 骨架可直接落盘并过写闸、doc 骨架只有 H1 + 一句（零段名）；`link` 算出的跨元素 `$ref` 被 `apiRefMissing` 判为可达。
8. **词汇与档位**：`ArchiApiFindingKind::ALL` 长度 10、`as_str` / `severity` 三方对拍；`error` 恰 `apiMissing` / `apiSignatureMismatch` 两项；`category` 七值、无空组。
9. **纳管口径**：`declared` ∧ schema 在盘的元素集合 = 11（与今日读数同）；`docMissing` 永不纳管；`status == errors ⟺ passed == false` 不变量在语料上成立。
10. **上下文面**：知识骨架与全文体量在**新冻结锚**内且 `archi_knowledge_test` 全绿；`proposal-planner` / `dev-design-planner` / `implementation-generator` 三份 prompt 的新句断言在场，其余 11 份零 archi 工具字样；change 臂两写权相位 `preamble` 在场、`implement/executor` 缺席。
11. **规则面**：`AGENT.md` 与 `CLAUDE.md` 的「模块 API 契约」节各恰 5 条要点、两份节内正文逐字相同、节内零 `api-*` fence 示例、零 `openspec/specs/` 路径。
12. **写面 warnings 收窄**：`archi_element` 的五条 conditions 逐条可绿（`noPath` / `pathNotFound` / `docMissing`（仅 `created`）/ `apiUnexpected` / delete 后的 `apiUnexpected`）；只改 `doc` 三次（写值 / 清键 / 指向不存在文件）`warnings` 均为 `[]`；`DocNotFound` 在写面零出现。
13. **`docMissing` 单因单义**：判据面与写面同判「派生 `<FQN>.md` 不在盘」；构造「派生 md 在盘、schema 缺失、`path` 非空且在盘、入口已解析」的元素 ⇒ 判据面零条目（`apiMissing` / `apiSignatureMismatch` 无声明集、`docMissing` 不成立），既有元素的 upsert 写面 `warnings` 亦为空（本期不为「接口未写」产任何 kind；`docMissing` 只由 `created` 诱发）。
14. **版本纪律**：`packages/desktop/package.json` 相对落地时 `HEAD` +1 patch；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行零变化。
15. **`archi_search` 语料换面**：验收对象 = `archi_search` 响应（`ArchiSearchResult`）与 `dto::search` 的语料面；条件 = 四态夹具（只有派生 `<FQN>.md` / 只有 `<FQN>.schema.json` / 两者都在 / 都没有）× 两值 `api`（`declared` / `none`）逐格调用 ⇒ 语料恰为两条固定规则「在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`」，命中集只由**在盘事实**决定：`api = none` 只有文档 ⇒ 文档命中、接口不命中；`api = declared` 只有 schema ⇒ 接口命中；`doc` 字段的任何取值都不参与语料；`scope = "elements"` / `"doc"` 的另一半恒为空数组、`"docs"` 显式 `Err` 并列三合法值、空 `query` 显式 `Err`；`decisions/**`、`openspec/specs/**`、元素 `path` 目录内散落 `md` 零命中；源码零 `carrier_path` / `doc_path` 引用（MUST NOT 分叉出第二次载体派生）。
16. **`archi_api` 两格下发**：验收对象 = `compose::archi_matrix` 的 `(phase, role)` → 工具名清单，及该轮 definitions 与 `RunStarted.tools`；条件 = 逐行发起 8 行矩阵会话 ⇒ `(proposal, executor)` 的清单为 `archi_module` / `archi_search` / `archi_api` / `archi_scaffold` / `archi_element` / `archi_relation`，`(dev-design, executor)` 同集再加 `archi_drift`，两者的 definitions 与 `RunStarted.tools` 逐名一致且都含 `archi_api`；`(dev-design, evaluator)` / `(test-design, executor)` / `(code-review, evaluator)` / `(acceptance, evaluator)` 与矩阵外相位均不含 `archi_api`；同批断言「矩阵含 `archi_element`」⟺「`model_write_lock` 返回 `None`」逐格仍相等、`archi_api` 仍在 `ToolTier::Read` 且 `READONLY_TOOLS` 三工具名零变化（扩面不扩写权）。

## 风险

| # | 风险 | 影响 | 处置 |
|---|---|---|---|
| R1 | **`.schema.json` 被误当生成物** | `diff::api_dir_artifacts` 若不收面，11 份手写文件全进 `orphaned`——症状是「全红」而不是「少一条」，容易被误读成生成器坏了 | 收面与 `render_all` 让位**同一提交**落地；验收 2 显式断言 `orphaned` 不含 `.schema.json` |
| R2 | **`Contract::from_schema` 忘记解包 `x-function`** | `emit/rust.rs` 直接读 `op.signature.get("returns")` / `.get("owner")`，保留包裹态会让 `.i.rs` 的签名见证静默退化（返回类型丢成 `()`、`owner` 少一层 `super::`），要到 `cargo build` 才炸 | 一条直接对拍测试钉住「`ops[i].signature` == `schema.properties[x].x-function`」；验收 2 的「`.i.rs` 逐字节不变」是第二道锚 |
| R3 | **停摆代价外溢**：坏 schema ⇒ 架构页 / 画布 / 契约面板整面不可用 | 一份写坏的 schema 让全图停摆 | 写闸保证「上盘即合法」（唯一写入口是 `write` / `edit`）；停摆只在绕过写闸（`git checkout` / 外部编辑器）时发生——这是「不产半真报告」的代价，接受并在知识骨架写明「坏文件由读取面报错」 |
| R4 | **遗留 md fence 仍会被人类读到** | 读者可能以为 fence 仍是契约真相 | 知识骨架「兼容」一句 + `AGENT.md` 第 ① 条明写「文档不列接口名；历史 fence 读面不读，清理中」；清理轮（Q3）收尾 |
| R5 | **11 份 schema 的 `$comment` 仍是「生成物、勿手改」** | 新写者读到会误以为它仍不可手改 | P10：写闸不校验 `$comment`，期间由知识骨架说明；清理轮统一改写。本变更**不批量改写**（会让 11 份 diff 与语义无关） |
| R6 | **判据收缩的「被动」性质容易被误读成「放宽」** | 三项删除是事实源消失，不是降低标准 | 提案与 spec 都逐条写明「事实源消失」；验收 8 用长度断言钉住 10 值；`schemaInvalid` / `schemaMissing` 的补位明确归后续 change |
| R7 | **`docMissing` 文案变化会改金样** | `corpus/drift/**` 与 `reports/` 的 message 文本随动 | `DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工确认；`reports/archi/` 是门禁运行产物、不入库 |
| R8 | **`archi_api` 扩面抬高每轮 token** | 两写权相位多有约 1.8 KB 工具定义 | 只读工具、恰在起草相位；与「无写闸不许裸写唯一真相」是同一笔账 |
| R9 | **`INDEX.toml` 的 `declared` 命名债被读成「模型意图」** | 语义歧义 | P6：值域不动（保证生成物逐字节不变），在 `schema.rs` 注释与 `INDEX.toml` 头注释点明「= 接口文件在案」；改名归清理轮 |
| R10 | **change 臂知识注入收窄可能被读成「工具面无知识」** | `implement/executor` 有 archi 只读工具但无知识段 | P5 写明判据（写权同源）与理由（知识含写模型纪律，与相位锁矛盾）；相位 prompt 已承载 implement 的工具用法 |
| R11 | **未决项留白**：`edgeApiUnresolved`（`relation.apis ⊆` 目标接口键集）今天零判据 | 关系边写了 schema 里不存在的名字不会当场报错 | 归后续 change（`archi-api-findings-redesign.md`）；本变更不为它预铺判据 |
| R12 | **历史经验**：`AGENT.md` / `CLAUDE.md` 同源复制会分化 | 两份规则文本漂移 | 同提交改两文件、验收 11 断言节内逐字相同（`desktop-archi-api`「仓库根文档的契约纪律节」的既有纪律） |
| R13 | **判据词汇与基线数字在别的能力 spec 里也有副本**（`desktop-archi-drift` 的「十三值 / 39 条」、`desktop-archi-model` 的「十三值 / 七条 warnings」、`repo-archi-model` 的「39 条 / `docNotFound` 零命中」） | 只改 `desktop-archi-api-check` 会让语料自相矛盾（同一事实两处不符） | 本变更同批给三个能力写增量（引用同步 + 写面 warnings 收窄 + 基线 38）；验收 1 的「逐字段不变」不覆盖文档面，故这三处必须靠 review 逐条核对 |
| R14 | **spec 里的 39 / 36 已是陈旧读数**（现仓 38 = 35 条 `declared` 侧 + 3 条 `api = none` 侧，均因派生 md 缺位） | 若照抄旧数字，实现期对拍必然假红 | 本变更按现仓读数改写为 38，并在历史链末段写清成因（分母 46 → 49、资产齐全 10 → 11）；基线表本身仍不写进断言，断言只锚定「实现期留痕的实测表与报告逐项相等」 |
| R15 | **写面 warnings 由七条收为五条**会让 `archi_element` 响应的 `warnings` 取值面变窄 | 依赖「`api = none` 且 `doc` 空必报一条」的既有会话习惯失效 | 该三条都是 `doc` 字段的判据副本，随 Q10 一起退役；`warnings` 的三通道纪律与降噪不变式逐字保留，工具描述只说「已落盘未阻断的事实」，不承诺条数 |
| R16 | **`docMissing` 在判据面与写面被改出两个含义**（历史草稿曾把它在写面判成「schema 不在盘」） | 同一 `kind` 承载两种缺口——违反「同一个值 MUST NOT 承载两种缺口」纪律，且两处读数不可互相解释 | P11 钉住单因单义（两面都判派生 `<FQN>.md`；写面只由 `created` 诱发）；验收 13 用「md 在盘、schema 缺 ⇒ 两面零条目」一条夹具反向锁住；「接口未写」的任何读数一律归后续 change 的 `schemaMissing` |
