# desktop-archi-api Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**资产落点换段**（E-1 目录迁移的计划面），契约格式、五类位置语义、双向 gate、引用与命名规则、关键字集与类型映射、入口解析链、十条判据与四类留白**逐字不动**：

1. **契约资产进模型目录**：手写真相由 `<archi_dir>/api/<Domain>/<FQN>.md` 改为 `<archi_dir>/models/<Domain>/<FQN>.md`，`.schema.json` 与它同目录（`$ref` 相对路径不嵌目录 ⇒ 生成物字节不变）；`api` 段退役，契约目录不新增派生函数，直接取既有 `archi_models_dir`（见 `workspace-layout-resolution`）。
2. **索引换家**：`INDEX.toml` 由「契约目录根」改为 `<archi_dir>/graph/INDEX.toml`（`archi_graph_dir`）。理由是模型解析与索引争同一根：`parse_dir` 按「一级 + `.toml` 后缀」收模型文件面，索引若落 `models/` 根会被当模型文件解析（`deny_unknown_fields` 收 `[[entry]]` ⇒ `unknownField` 诊断 ⇒ 模型 `status = errors`），且 `drift::model_files` 会把它计入 `modelHash`（全部金样换段）。换家而非收紧模型收面，是为了让「一级 `NN-<域>.toml`」这条判据保持零改动。
3. **两处生成物字节真实变化**：`INDEX.toml` 的说明段目录串（`契约面：目录 …`）与已声明元素的 `schema` 列（repo 相对路径含 `models/<Domain>/`）随段名变化——**恰 2 行**，其余逐字不变；`.schema.json` / `.i.rs` / `.i.ts` 零字节变化。
4. **与其他能力的边界不变**：判据十值词汇、互斥求值序、档位与纳管子集、`REPORT_VERSION`、`enabled` 的语义（「递归含 ≥1 份 `*.md`」）零改动——只换被监测的目录；`api:export` / `api:check` 两脚本名与 `check` 链四段逐字不变。
5. **两处 doc 散文串随段换位（doc-only，代码零改动）**：`api/extract/mod.rs` 模块 doc 的「契约权威在 `<archi_dir>/api/<Domain>/<FQN>.md`」与 `model.rs` 的 `api` 字段 doc 的「架构域 `api` 目录」同批改指 `models/<Domain>/`——这两处既不是引号字面量也不含 `architecture/api` 形态，只按符号 / 字面量扫描会漏，留着则「产品 `.rs` 无旧段残留」在 doc 面不成立。

## MODIFIED Requirements

### Requirement: 契约资产形态与唯一手写真相

模块公共 API 契约 SHALL 以**手写 markdown 为唯一真相**，落位 `<archi_models_dir>/<Domain>/<FQN>.md`（`<archi_dir>` 经 `foundation::layout::archi_dir`、契约目录取既有 `foundation::layout::archi_models_dir` —— `api` 段与 `archi_api_dir` 族已退役，MUST NOT 行内字面量、MUST NOT 新增第二个目录派生函数）。`<Domain>` SHALL 取元素**父元素 FQN 末段**（`App.Backend.Infra.ArchiRuntime` → `Infra`），元素身份仍是 FQN、目录只是布局。

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
- **统一目录（不跟代码走）**：契约 MUST NOT 落元素主目录。审计型问题（「这批改动动了哪些模块的契约」）只有统一目录能答（`git diff --stat <archi>/models/<Domain>/` 即列出），且无替代物；伴随型问题（「我改的这行契约同步了吗」）由门禁机械回答，不需要文件相邻。代价 SHALL 显式留痕：在 crate 里读代码时看不到契约，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。
- **与模型同树（`models/` 双身份，本变更承认的事实）**：`<archi_models_dir>` SHALL 同时容纳两类住户，且两类互不参与对方的判据——**一级文件**是模型声明（恰六个 `NN-<域>.toml`，见 `repo-archi-model`「基线落位与模型文件划分」）；**`<Domain>/` 子目录**是契约资产（`<FQN>.md` 手写真相 + `<FQN>.schema.json` 生成物）。模型解析的收面判据因此 SHALL 为「一级 + `.toml` 后缀」（不递归、不按目录名猜），MUST NOT 因同目录出现 md / schema / 子目录而改变；反向，契约面的收面判据 MUST NOT 把一级 `*.toml` 当契约或启用证据。文件命名因此承担了全部身份区分，MUST NOT 引入第二份目录级豁免清单。
- **不接入 `docs` 三源**：契约 MUST NOT 成为 `desktop-archi-module-docs` 的第四种关联来源（该能力已封死为恰三种；契约 md 落在 archi 树内，而 archi 树整棵对 `docs` 三源不可见）。
- **不依赖语言概念**：资产形态、目录布局、schema 与索引中 MUST NOT 出现 `crate` / `Cargo.toml` / `Rust` 一类语言专有概念——语言特定部分只落取数器（`api/extract/{rust,ts}`）与渲染器（`api/emit/{rust,ts}`），见「语言无关纪律与语言特定落点」。

#### Scenario: 五类位置语义与 H1 锚点校验

- **WHEN** 解析一份含 H1 `<FQN>`、`## 职能` 散文、两个 `### <op-id>` + `api-op` fence、一个 `api-types` fence 的契约
- **THEN** 解析成功；两个 op 的 id 与 heading 逐字对应，fence 内不含 op 名；H1 与文件名不一致时报契约错误并指出两处原文

#### Scenario: 双向 gate 三处不成立各报错

- **WHEN** 分别构造「`### new` 有 heading 无 fence」「`api-types` 里有个键在生成的 `$defs` 缺失」「`api-reexport` fence 有 3 个键而散文只列 2 个」
- **THEN** 三次均判契约错误，错误文案分别点名缺 fence 的 op、缺定义的键、fence 与散文的差集；MUST NOT 静默通过

#### Scenario: 拆两文件与 JSON 内嵌描述被拒

- **WHEN** 审查资产形态与提交内容
- **THEN** `<archi_dir>/models/<Domain>/` 下每个元素只有一份 `*.md` 手写真相（同目录的 `.schema.json` 为生成物且头部带警示；`INDEX.toml` 不在契约目录内，落 `<archi_dir>/graph/`）；不存在与 `.md` 配对的第二份手写 `.json`，散文不出现在 JSON 字符串里

#### Scenario: 契约与模型同树互不干扰

- **WHEN** 在 `models/` 于六个 `NN-<域>.toml` 之外同时放入 `<Domain>/<FQN>.md`、`<Domain>/<FQN>.schema.json` 与三级子目录，随后跑一次模型解析与一次契约装载
- **THEN** 模型解析结果与只放六个 toml 时逐字相同（`diagnostics` 空、元素 / 关系计数不变——收面判据是「一级 + `.toml` 后缀」）；契约装载恰好读到那份 md；任一方向 MUST NOT 因同目录混居产生诊断或判据

### Requirement: schema.json 与 INDEX.toml 为纯派生且不可手改

每个契约 SHALL 派生 `<FQN>.schema.json`（JSON Schema 投影，与 `<FQN>.md` 同目录，即 `<archi_models_dir>/<Domain>/`），并由全体契约派生出 `INDEX.toml`（落 `<archi_graph_dir>` 下、全局唯一一份，MUST NOT 落契约目录 = 模型目录的根）。两者都是生成物、都不可手改。

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
- **索引与契约目录分家（本变更）**：`INDEX.toml` SHALL 落 `<archi_graph_dir>`，MUST NOT 落 `<archi_models_dir>` 根。判据是机械的：模型聚合的收面是「一级 + `.toml` 后缀」（`desktop-archi-model`「多文件聚合与声明定位」）且单文件反序列化 `deny_unknown_fields`——索引落模型目录根会被当模型文件解析，产出 `unknownField` 诊断并使模型停摆；`drift` 的 `modelHash` 取同一收面，索引落根还会改变全部漂移金样。故索引路径 SHALL 由 `paths::index_path(root)` 单点派生为 `<archi_graph_dir>/INDEX.toml`，MUST NOT 由契约目录 `join` 得到。
- **review 口径**：每次改契约产生多份 diff；因 schema 零手改自由度，review SHALL 只读 `.md`，schema diff 视为机械噪声——该口径 SHALL 落 `INDEX.toml` 的说明段（不写进散文约定）。
- `INDEX.toml` SHALL 按元素列出：`fqn` / `api` 状态 / 类型数 / 操作数 / 生成物路径 / `degraded` 计数 / `category` 分布，并含 review 口径说明段；条目序 SHALL 为 FQN 字典序（稳定序，便于断言与 diff）。说明段的「契约面：目录 …」串 SHALL 取 `<archi_models_rel_dir>`（契约资产所在的相对路径，MUST NOT 取索引自身所在目录——索引与契约已分家，写成索引目录会指错地方）。
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
- **THEN** `INDEX.toml` 该元素的类型数与 `degraded` 计数随动，其余元素条目逐字不变；条目序为 FQN 字典序；说明段的「契约面：目录」取 `<archi_models_rel_dir>`（与索引自身所在的 `<archi_graph_rel_dir>` 不同串）

#### Scenario: 索引落模型目录根被拒

- **WHEN** 手工把 `INDEX.toml` 复制到 `<archi_models_dir>/INDEX.toml`（契约目录根）后跑一次模型解析
- **THEN** 该文件被当模型文件解析并产出 `unknownField` 诊断（`[[entry]]` 非模型字段），模型进入停摆态；`drift` 的 `modelHash` 随该文件变化——两处判红即本条的存在理由；删除该副本后恢复零诊断、`modelHash` 回到原值

### Requirement: 生成物入仓与零 diff 门挂载

四类生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `<FQN>.schema.json` | `<archi_models_dir>/<Domain>/` | 是 | `api:check` 零 diff |
| `INDEX.toml` | `<archi_graph_dir>/`（全局唯一一份） | 是 | `api:check` 零 diff |
| `<入口主干>.i.rs` | **元素实现入口同级**（`crates/infra/archi/src/lib.i.rs`） | 是 | `api:check` 零 diff + `cargo check` |
| `<入口主干>.i.ts` | **元素实现入口同级**（`views/welcome-view.i.ts`） | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **`INDEX.toml` 的 `generated` 列取入口同级路径**：无入口（`Absent` / `Invalid`）的元素该列留空——生成物路径是入口的函数，无入口即无路径可写。
- **孤儿扫描 SHALL 覆盖两个根**：契约目录侧的 `*.schema.json`（`<archi_models_dir>/**` 递归）与索引侧的唯一 `INDEX.toml`（`<archi_graph_dir>/`）。两处的枚举语义 SHALL 逐条同形——深度上限防环、符号链接不跟随、`read_dir` 只出现在有界枚举里、repo 相对路径串由同一 `display` 单点产出；MUST NOT 出现第二份手写递归（两处差异只允许是「根不同、收面后缀不同」）。
- **脚本面封闭两条**：`api:export`（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`，写盘）与 `api:check`（同一调用加 `--check`；生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，**不依赖任何 `git diff` 段**）。`check` 链 SHALL 为四段：`server:check && bindings:export && api:check && client:check`；`bindings:export` 与 `client:check` 两段命令形态零变化，`server:check` 的 fmt 段为校验模式 `cargo fmt --all --check`。**第二段 SHALL 是 `bindings:export`**（`check` 的生成物零 diff 守卫已退役，见 `desktop-ipc-type-bindings`——`check` 只前置重导出、不比对）；规范内 MUST NOT 引用任何已退役的守卫脚本名；段名 SHALL 以 `package.json` 的 `scripts` 实存键为准（`grep -c '"bindings:export"' package.json` ≥ 1 即证该键在案）。
- **应用内两驱动面与脚本面同源**：生成物的渲染与三类差异比对 SHALL 各有唯一实现（`core/archi` 的渲染 / 比对纯函数），写盘 SHALL 有唯一实现（`infra/archi` 的写盘单点）；应用内两条 IPC（检查 / 生成，见 `desktop-archi-contract-panel`）与 CLI 两子命令 MUST NOT 各写一份渲染、比对或写盘——**四条消费路径共用同一实现**是「零 diff 门」与「应用内检查」结论一致的前提。脚本面两条命令形态本文逐字不变（仅落点换段，命令串不变）。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 含 `src/**/*.i.ts`（生成物非手写代码；实测未被 import 的 `.i.ts` 会被报为 unused file，因 `project` glob 覆盖 `src/**/*.ts`）；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies` 条目（那是关判据而非满足判据）。
- **类型绑定再生成**：`ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` 与报告字段 `apiFindings` SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役）；本能力 MUST NOT 手写前端类型镜像。**本变更零 DTO 形状改动，但绑定文件不是零 diff**：`dto.rs` 的 doc 方位串换段（`ApiStatus` / `ArchiApiRef::contract_path`）经 specta 原样抄进 `bindings.ts` ⇒ 该文件恰 **2 行 doc 注释**随动（diff 全落在注释内），形状面（类型名 / 联合成员 / 字段名 / 字段序 / `null` 语义）SHALL 逐字不变、`bindings.zod.ts` SHALL 零 diff、`bindings:export` SHALL 幂等（重复导出不再产生第三行 diff）。**MUST NOT 把本变更表述为「零 DTO 形状改动 ⇒ 绑定零再生成」**——两者不是等价命题，出现形状面 diff 或 zod 面 diff 才是误动。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型（否则校验不发生）；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（相对递增，规范内 MUST NOT 写死具体版本号——现值随每次源码改动滚动，写死即失效）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 四类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 四类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改任一生成物即被拦

- **WHEN** 手改已提交的 `<FQN>.schema.json`、`INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: 落点迁移后旧位置零残留（机制与判红面分列）

- **WHEN** 落点换段完成后审查旧位置 `<archi_dir>/api/**` 与产品 `.rs` 的旧段名
- **THEN** `<archi_dir>/api` 目录不存在（`ls` 零命中）；产品 `.rs`（`*_test.rs` 除）内 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` 三符号、`"api"` 路径段引号字面量、`architecture/api` 与 `<archi_dir>/api/` 两串（含 `api/extract/mod.rs` 与 `model.rs` 的 doc 散文）全部零命中
- **AND** 机制边界 MUST 说准：孤儿扫描严格覆盖 `<archi_models_dir>/**` 的 `*.schema.json` 与 `<archi_graph_dir>/INDEX.toml` **两处派生**（见「生成物入仓与零 diff 门挂载」），故 `api/` 下的残留文件**不在 `api:check` 的枚举面内**，「旧位置清空」MUST NOT 被表述为「`api:check` 报应删未删 / 孤儿索引」——那在「索引与契约分家」之后不成立；清空由迁移动作（`git mv` 与代码改动同一提交内完成）、目录树读数与 `pilot_test` 的「`<archi_dir>/api` 不存在」负向断言保证。若日后确需机械守旧段缺席，应单开变更把枚举根放宽到 `<archi_dir>` 并同步改本条

#### Scenario: 换段后的字节差异恰为两行

- **WHEN** 迁移前后各执行一次 `api:export`，对 `INDEX.toml` 与全部 `.schema.json` 做逐字节比对
- **THEN** `INDEX.toml` 的差异恰为两行——说明段的「契约面：目录」串（`<archi_models_rel_dir>`）与已声明元素的 `schema` 列（repo 相对路径含 `models/<Domain>/`）；其余元素条目与全部 `.schema.json` / `.i.rs` 逐字不同为零；MUST NOT 把该差异记成「零 diff」（说明段与路径列按设计变化，见「schema.json 与 INDEX.toml 为纯派生且不可手改」）

#### Scenario: check 链四段且 fmt 段为校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与 `server:check` / `bindings:export` / `api:check` / `client:check` 四段
- **THEN** `check` 逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`（段序与段数零变化）；`bindings:export` 与 `client:check` 两段命令原文逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check` 且其 clippy 段逐字不变；`api:check` 的调用形态（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- --check`）逐字不变

#### Scenario: 应用内检查与脚本面结论一致

- **WHEN** 在同一观察根分别调用 `archi_api_check`（IPC）与 `api:check`（CLI）
- **THEN** 两者报出的三类差异文件集合逐条相等；随后用 `archi_api_generate`（IPC）生成，再跑一次 `api:check` 得零差异——四条消费路径（两 IPC / 两 CLI 子命令）零第二份渲染、比对或写盘实现

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含五条要点：① 手写真相位置 `<archi_models_dir>/<Domain>/<FQN>.md` 与三类 fence 的名字（`api-op` / `api-types` / `api-reexport`）；② 四类生成物勿手改（`.schema.json`（与 md 同目录）/ `INDEX.toml`（`<archi_graph_dir>`）/ **实现入口同级**的 `<入口主干>.i.rs` / `<入口主干>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」；⑤ **契约在案元素**的判据进 `status`（可把状态词抬到 `warnings`）、不进 `passed`，索引与计数经 `<archi_graph_dir>/INDEX.toml` 查看，两轴经架构页「契约」视图一眼看全。
- 该节需换段的目录串 SHALL 恰为三条要点：第①条（`<archi_models_dir>/<Domain>/`）、第②条（`INDEX.toml` 方位 `<archi_graph_dir>`）、第⑤条（索引串 `<archi_graph_dir>/INDEX.toml`）；第③ / ④ 条零改动。**换段完成后该节 `grep 'architecture/api'` 与 `grep '契约目录根'` 均零命中**——第②条不含 `architecture/api` 字面量，只测字面量归零不足以证明换段完成。
- 第①条的目录串 SHALL 与「契约资产形态与唯一手写真相」的落位口径逐字一致（`<archi_models_dir>/<Domain>/`，零 `api` 段残留）；第②条的 `INDEX.toml` 方位 SHALL 写「`<archi_graph_dir>`」而非「契约目录根」——索引与契约已分家，写成契约目录根会指错路径。
- 第②条的**方位词与文件名形态 SHALL 与「生成物落点与入口查找」的落点口径一致**：「实现入口同级」+ `<入口主干>.i.<后缀>`（`crates/infra/archi/src/lib.rs` → `lib.i.rs`）。该条只说方位与命名，MUST NOT 复述入口解析链、四条判据或 `entry` 字段知识（字段面归 `desktop-archi-model`，agent 侧字段清单归 `desktop-archi-agent` 的知识文本）——文档与 spec 双写必然漂移。本条同步 MUST NOT 增删要点条目（条目数恒 5，`entry` 不得成为第六条）。
- 该节 MUST NOT 含契约示例正文、`ArchiApiFindingKind` 的十值长表或判据定义（定义归本能力与 `desktop-archi-api-check`）——文档与 spec 双写必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制即既有反例）。
- 该节 MUST NOT 出现 `openspec/specs/**` 路径（spec 语料非 agent 的契约操作面），且 MUST NOT 把契约路径写进模型元素 `docs`（见 `desktop-archi-module-docs`）。
- `CLAUDE.md` SHALL 同步同一节且与 `AGENT.md` 保持既有同源纪律：两文件该节正文逐字相同，全文件差异 SHALL 限于节外既有差异（标题行、首行说明行、文件末尾换行）；**该节自身的两份副本若分化即违反本条**。

#### Scenario: 纪律节只指路不带正文

- **WHEN** 审查 `AGENT.md` 的 `## 模块 API 契约` 一节
- **THEN** 该节恰有 5 条要点、`grep -c '^## 模块 API 契约' = 1`；`grep 'apiEntryMissing'` 在该节零命中（判据长表未复制）；节内零 `openspec/specs/` 路径；节内无 fenced json 代码块

#### Scenario: 两份副本不分化

- **WHEN** 对 `AGENT.md` 与 `CLAUDE.md` 做 `diff`
- **THEN** 差异恰 2 处且全在 `## 模块 API 契约` 节外（`:1` 标题、`:3` 首行说明）；该节在两文件各恰一处且节内正文零 diff 行（逐字相同）

#### Scenario: 纪律节第 ⑤ 条与纳管口径一致

- **WHEN** 对照该节第 ⑤ 条与 `desktop-archi-api-check`「报告并列槽位 apiFindings」的纳管条款
- **THEN** 两处都表达「契约在案元素的判据影响 `status`、不影响 `passed`」，MUST NOT 残留「不进 `status`」一类旧口径

#### Scenario: 第 ② 条方位词与真实落点一致

- **WHEN** 审查 `AGENT.md` 与 `CLAUDE.md` 该节第 ② 条，并对照入口同级的实盘生成物 `crates/infra/archi/src/lib.i.rs` 与索引实盘 `<archi_dir>/graph/INDEX.toml`
- **THEN** 该条写「实现入口同级」且文件名形态为 `<入口主干>.i.rs` / `<入口主干>.i.ts`，索引方位写 `graph` 目录；`grep '元素根同级'`、`grep '<root>.i'` 与 `grep 'architecture/api'` 在两文件该节零命中；第 ① / ② / ⑤ 三条各含新落点串（`models/<Domain>/<FQN>.md` / `graph` / `graph/INDEX.toml`）而第 ③ / ④ 条逐字不变，`grep '契约目录根'` 在节内零命中（该串无 `architecture/api` 前缀，字面量归零抓不到它）；该节要点数仍恰 5（措辞同步 MUST NOT 增删条目、MUST NOT 出现 `entry` 字段说明）；两文件全文 `diff` 仍恰 2 处且全在节外

## Module Contract

### Function / Component: 受影响入口与资产落点

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/api/paths.rs` | 改（落点与索引分家） | `directory(root)` SHALL 转发 `foundation::layout::archi_models_dir(root)`（原 `archi_api_dir` 转发点退役）；`index_path(root)` SHALL 为 `foundation::layout::archi_graph_dir(root).join(INDEX_FILE_NAME)`（MUST NOT 由 `directory(root).join(…)` 派生）；`contract_path` / `schema_path` 只换 base（`domain_of` / `domain_dir` / 跨域 `$ref` 相对前缀 `../<Domain>/` 零改动）；其余函数（`normalize_root` / `module_name` / `rust_module_path` / `language_of` / `generated_kind` / `entry_candidates` / `entry_state` / `EntryForm` / `generated_path` / `ts_import_specifier` / `ref_relative`）逐字不变 |
| `crates/core/archi/src/api/mod.rs` | 改（启用判据换根） | `enabled(root)` SHALL 改为「`<archi_models_dir>` 递归含 ≥1 份 `*.md`」（`has_markdown` 的深度上限与符号链接不跟随语义逐字不变）；语义仍是「契约面是否启用」，MUST NOT 因模型目录恒在而恒真——收面是 `*.md`，一级 `NN-<域>.toml` 不构成启用证据；其余出口（`meta` / `load_contracts` / `render_all` / `bootstrap` / `check` / `api_ref` / `api_ref_of`）零变化 |
| `crates/core/archi/src/api/diff.rs` | 改（孤儿扫描两处派生） | `api_dir_artifacts(root)` SHALL 派生两处：`<archi_models_dir>/**` 的 `*.schema.json` + `<archi_graph_dir>/**` 的 `INDEX.toml`；两处共用**同一私有递归**（形如 `collect_artifacts(directory, suffix, depth, found)`，原名 `collect_api_dir` 的收面判据由「等于索引路径」换成「以 `suffix` 结尾」），深度上限 / 符号链接不跟随 / `read_dir` 只此一处三条语义照搬，MUST NOT 出现第二份手写递归；`orphaned` / `mismatched` / `missing` 三列语义零变化（`starts_with(root)` 越根复核驻写盘单点，见本表 `api_artifacts.rs` 行，本变更零触及） |
| `crates/core/archi/src/api/schema.rs` | 改（说明段目录串） | `build_index` 的说明段「契约面：目录 …」SHALL 取 `foundation::layout::archi_models_rel_dir()`（原 `archi_api_rel_dir()` 已删）；`IndexEntry.schema_rel` 仍是 repo 相对路径（现含 `models/<Domain>/`）；`$defs` 规则 / 装配 / 债表扫描逐字不变 |
| `crates/core/foundation/src/layout/mod.rs` | 改（段名增删） | 新增 `GRAPH_DIR_NAME` / `archi_graph_dir` / `archi_graph_rel_dir`；删除 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`；`archi_models_dir` 等其余派生逐字不变（见 `workspace-layout-resolution`） |
| `crates/infra/agent/src/archi_knowledge.rs` | 改（知识文本方位） | 注入段 SHALL 由「契约目录：`<archi_api_rel_dir>`」改为「契约资产：`<archi_models_rel_dir>/<Domain>/<FQN>.md`（索引 `<archi_graph_rel_dir>/INDEX.toml`）」；`api` 段串零残留；纯函数零 IO、降级为 `None` 的语义与四面词表注入逐字不变 |
| `crates/infra/archi/src/api_artifacts.rs` | 零改动 | 写盘单点按 `GeneratedFile.path` 的父目录创建（路径由上游 `api::paths` 派生，本层零段名知识）；`starts_with(root)` 越根复核与「只增只改、从不删文件」语义逐字不变——换段不需动它，出现 diff 即误动 |
| `crates/tools/archi-api-gen/src/main.rs` | 改（只经派生族） | 三子命令编排 SHALL 只经 `paths` 与 `foundation::layout` 取落点，零行内 `models` / `graph` / `api` 段字面量；`generate` / `--check` / `bootstrap` 的命令形态与退出码语义逐字不变 |
| `.dev-team/architecture/models/<Domain>/**`（契约资产新落点） | 迁移 | `<Domain>/<FQN>.md`（唯一手写）+ `<Domain>/<FQN>.schema.json`（生成）；由 `git mv` 自 `<archi_dir>/api/<Domain>/**` 搬入，内容逐字不变 |
| `.dev-team/architecture/graph/INDEX.toml`（契约索引新落点） | 迁移 | 由 `git mv` 自 `<archi_dir>/api/INDEX.toml` 搬入，随后由 `api:export` 重生成（差异恰为说明段目录串与 `schema` 列两行） |
| `.dev-team/architecture/api/**` | **删除** | `api` 段退役；迁移后该目录不存在。**判红面说准**：孤儿枚举严格两处派生（`models/**` + `graph/`），`api/` 残留对 `api:check` 不可见——缺席由 `git mv`（同提交）+ 目录树读数 + `pilot_test` 负向断言守，MUST NOT 写成「`api:check` 报应删未删」 |
| `crates/core/archi/src/api/extract/mod.rs` | 改（**仅模块 doc 方位串**） | 模块 doc 的「契约权威在 `<archi_dir>/api/<Domain>/<FQN>.md`」改为 `<archi_models_dir>/<Domain>/<FQN>.md`；取数器分派、签名归一、`extract_element` / `extract` 两个入口的能力面与「本期产品路径零调用者」的裁定逐字不变 |
| `AGENT.md` + `CLAUDE.md` | 改 | `## 模块 API 契约` 节第 ① / ② / ⑤ 条（现文件第 26 / 27 / 30 行，各 3 行）的目录串换段：手写真相 `<archi_models_dir>/<Domain>/<FQN>.md`、② 的 `INDEX.toml` 方位 `<archi_graph_dir>`、⑤ 的索引串 `<archi_graph_dir>/INDEX.toml`（② 原写「契约目录根」，无法靠 `grep 'architecture/api'` 抓到）；条目数仍 5、两份副本该节正文逐字相同 |
