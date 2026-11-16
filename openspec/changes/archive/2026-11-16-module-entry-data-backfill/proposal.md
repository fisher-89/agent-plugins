# 提案: module-entry-data-backfill

> **变更**: module-entry-data-backfill（主线变更 A 的数据面承接 · `entry` 三处归位与基线换段）
> **日期**: 2026-11-16
> **状态**: draft
> **来源**: `openspec/changes/module-entry-data-backfill/explore.md`（requirement 类型；主线 `openspec/explores/module-entry-declaration.md`）
> **性质**: **纯数据 + 基线数字**——三处 `entry` 归位（`04` 一处、`05` 两处）、`apiFindings` 分桶换段（15 / 26 → 12 / 27，总 41 → 39）、六处 spec 表述（跨四份 spec）同步。零代码逻辑改动、零新增判据、零生成物 diff、零新增/改写测试。
> **不含**: 前端 12 个 barrel（变更 B）、TS 取数面（变更 C）、契约格式补丁（D）、模块入口文档（E）、壳补契约。

---

## 问题

### 现状链路（读过的真实数据）

```text
A（module-entry-field-spec）已交付：模型字段 entry + 三态入口解析链 + 四条判据 + 生成物落点改入口同级
  └─ 代码面在 master（8b03cdec2fc2，已归档）；数据面被显式留给「宿主重建后的后续变更」——该变更当时未命名
现状：grep '^entry' .dev-team/architecture/models/ ⇒ 零命中
      apiFindings 实测 15（apiEntryMissing）+ 26（apiMissing）= 41
```

四个真实病灶，共同根因一句话：**入口事实已经可表达，但三处入口仍未写进模型**，于是判据面只能报「无门」，而 spec 只能把基线停在「本段」。

### 病灶一 · 三处入口事实真实存在却无处着陆

| 元素 | 真实入口（盘上已在） | 约定默认为什么指不到 |
|---|---|---|
| `App.Backend.Shell` | `packages/desktop/src-tauri/src/lib.rs` | 目录型根 `.../src-tauri/src/` 拼出的候选是 `.../src-tauri/src/src/lib.rs` |
| `App.Backend.Tools.ExportBindings` | `.../crates/tools/export-bindings/src/main.rs` | 二进制 crate 无 `src/lib.rs`，`main.rs` 不在约定候选列内 |
| `App.Backend.Tools.ArchiApiGen` | `.../crates/tools/archi-api-gen/src/main.rs` | 同上 |

三者在 `apiFindings` 里各产一条 `apiEntryMissing`（`Absent`）——这**不是**迁就实现的假报，而是「有门未声明」的真话；但它是可消的债：`entry` 字段与解析链 A 已交付，缺的只是三行数据。两个工具 crate 还叠着第二层事实：`api = "none"` 已在案（公共面是 CLI 契约，而非库 API）。

### 病灶二 · 基线被钉在「第一段」，换段步骤没有承接者

A 的 spec 把基线与数据都写成两段制：`repo-archi-model` 写「本变更落地时显式 `entry` SHALL 为 0 处」，`desktop-archi-api-check` 写「**本段基线 = 15 / 26**……后续变更落地后 SHALL 更新为 12 / 27」「MUST NOT 在本变更内按第二段数字验收」。A 归档后，这两句就是悬空承诺：基线是「契约债 → 迁移待办」的唯一对拍面，不是愿景面——数据一旦落定，报告会变成 ① 少 3 条、② 多 1 条（总少 2），而 spec 仍写着旧段，此后每次核对都得靠人记得「现在该按第二段读」；「两个工具 crate 的 `api = none` 为何不豁免 ①」这条重要口径也永远只有半段解释（真话其实是「它们此刻不再落在 ① 面上」）。

### 病灶三 · 只改基线表会留下六处自相矛盾的 spec（explore 只点了两处）

逐份读盘后发现换段触点不止 `repo-archi-model:30/54/69` 与 `desktop-archi-api-check:185/189`：

| 位置 | 现文 | 换段后必须同步为 |
|---|---|---|
| `repo-archi-model`「基线落位与模型文件划分」`entry` 条款 | 「本变更落地时显式 `entry` SHALL 为 0 处」+「随宿主重建后的后续变更归位」 | 「恰三处」+ 三值在案 |
| 同 requirement 的 `api = none` 条款 | 「12 个前端目录根 + 壳 + 两个工具 crate 是无入口——后三者的显式 `entry` 归后续变更」 | 无入口只剩 12 个前端目录根；两工具 crate = 有入口 + 无库 API |
| 同 requirement 的两条 scenario | 「`entry` 零行」/「壳与两工具 crate 各产一条 `apiEntryMissing`」 | 「恰三行」/「壳转 `apiMissing`、两工具 crate 零条目」 |
| `desktop-archi-api-check` 基线 requirement（表 + 切分口径 + 两段制 + 三个 scenario） | 15 / 26、总 41、两段制、16→14 的假阳假阴例 | 12 / 27、总 39、单段、13→11 |
| `desktop-archi-api`「生成物落点与入口查找」的「本仓入口面实测命中」scenario | `Resolved` 27 / `Absent` 15，并注「届时 30 / 12」 | `Resolved` 30 / `Absent` 12 |
| `desktop-archi-model`「架构模型文件格式与字段封闭」示例 TOML 注释 | 「本变更模型数据零处：显式值随宿主重建后的后续变更落」 | 指向「三处已在案」的现口径 |

只改前两类（explore 列的 2 处）会留下后面四类陈旧断言——尤其 `desktop-archi-api` 的「届时 30 / 12」一旦成立却仍写「届时」，读者会以为数据还没落。

### 病灶四 · 「零生成物 diff」需要先证明再落数据

三处 `entry` 会让壳与两个工具 crate 首次拥有可解析入口，而它们**没有契约文件**。`INDEX.toml` 的 `generated` 列是 `generated_path_of(...).filter(|_| present)`（`present` = 契约文件在案），故零契约 ⇒ `generated` 仍为空串、生成物照旧为零份。这条必须写成验收判据（AC-6）：若 `entry` 落位写错（比如写成 `src/` 而非 `src/lib.rs`），`generated` 列会立刻出现非空值与孤儿生成物。

---

## 提案

### 一 · 三处 `entry` 归位（数据唯一改动）

| # | 文件 | 元素 | 写入行 |
|---|---|---|---|
| 1 | `.dev-team/architecture/models/04-backend-shell.toml` | `App.Backend.Shell` | `entry = "./packages/desktop/src-tauri/src/lib.rs"` |
| 2 | `.dev-team/architecture/models/05-backend-tools.toml` | `App.Backend.Tools.ExportBindings` | `entry = "./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs"` |
| 3 | 同上 | `App.Backend.Tools.ArchiApiGen` | `entry = "./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs"` |

- **`./` 前缀照写**（与 `path` 同口径）：`normalize_root` 会剥除，不参与身份；不写前缀与 spec 的字段纪律不一致。
- **`api = "none"` 两行逐字保留**：`entry` 陈述实现入口、`api` 陈述公共面性质，两条同时在场才是「有入口但无库 API」的真话。
- **MUST NOT 写第四处**：单文件根元素（12 处）与 14 个 `src/lib.rs` 命中的 crate 由约定默认覆盖，声明即违「仅在约定默认指不到时声明」。

### 二 · 分桶换段（同一份判据，换的是数据）

```text
① apiEntryMissing   15 → 12   壳转 ②、两工具 crate 退出（entry 命中 + api = none ⇒ 零判据）
② apiMissing        26 → 27   壳加入（declared 缺契约），其余 26 条逐条不变
总                  41 → 39   净 −2，仅由两工具 crate 退出判据面贡献（壳是 ① → ② 的平移，净 0）
```

`status` 仍为 `clean`、门禁 `passed` 仍为 `true`：纳管子集（契约文件在案元素）仍只有 `App.Backend.Infra.ArchiRuntime` 一个，其判据 0 条——换段不抬黄灯，MUST NOT 借换段顺手改纳管口径。

### 三 · spec 六处同步（与数据同一提交）

逐处内容见「能力」一节的四份 delta 与「验收标准」AC-7。同步纪律：

- **只改数字与措辞，不改判据**：`entry` 四条判据、三态解析链、十值词汇、求值互斥序、纳管口径全部逐字不动。
- **两段制整段退场**：`desktop-archi-api-check` 的「数据分两批，基线分两段」改为「基线单段 = 12 / 27（总 39）」，并删「MUST NOT 在本变更内按第二段数字验收」——它随两段制一起失效，留着会禁止按现行基线验收。
- **历史数字必须留痕**：旧读数（28 + 13、15 + 26）作为「同一分桶规则的三段读数」写进切分口径，MUST NOT 被抹成单一现值的孤证——否则下次读报告的人无法判断「39 比 41 少 2」是收债还是漏报。

### 四 · 版本纪律

`packages/desktop/package.json` `version` 由 `0.6.46` +1 patch 至 `0.6.47`；`tauri.conf.json` 经 `"version": "../package.json"` 跟随（文件本身零改动）；`src-tauri/Cargo.toml` 的 crate 版本行（`0.1.0`）MUST NOT 随动。**只 +1 一次**：A 的实现提交未 bump（`0.6.44` → `0.6.44`）是既成事实，追溯补账会造出假历史（见 R-5）。

### 前置条件（MUST 先确认，否则整批回滚）

1. **含 `entry` 字段面的发布版已发布且宿主已升级 / 重建**：门禁与 `archi_drift` 在**宿主进程内**用宿主内嵌的 archi crate 解析模型；旧宿主对 `entry` 报 `unknownField`，该文件解析失败又会让依赖它的元素判「未声明」并连带 `unknownReference`（A 实测 13 条结构错 ⇒ `status = errors`）。
2. **验收机上的宿主二进制由本工作树重建**：`entry_state` 是 A 新增的判定逻辑，旧宿主没有它 ⇒ 数字不可信。

两条都属「跑之前先看一眼」的动作，不是实现自由度：`archi_graph` 能读出 `entry`（而非 `unknownField`）即证明宿主够新。

---

## 能力

### 新增能力

**无。** 数据回填不产生新用户可见能力：三处 `entry` 只改既有模型的既有字段数据、六处 spec 只换既有 requirement 的数字与措辞，四条判据、三态解析链、报告结构与门禁布尔全部逐字不动——另立能力会把「模型字段」与「字段的数据」分居两条 requirement。

### 修改的能力

- **repo-archi-model** — MODIFIED「基线落位与模型文件划分」：`entry` 条款由「本变更落地时 0 处 / 随后续变更归位」改为「**恰三处**（三值逐字在案）」+ 三处声明的后果（壳转 ②、两工具 crate 零判据）+ MUST NOT 第四处；`api = none` 条款的两类空态实例籍换位（无入口只剩 12 个前端目录根；两工具 crate = 有入口 + 无库 API）；两条 scenario（字段纪律 `entry` 零行 → 恰三行；入口与留白两类空态 → 三处入口在案后的归位）。元素 / 关系计数（51 / 113、03 段 8 / 24、04 段 1 / 13、05 段 3 / 3）与 `gate` 全 `off` 逐字不动
- **desktop-archi-api-check** — ① MODIFIED「判据级档位与元素 gate 解耦」：无入口元素数 15 → 12（scenario）；② MODIFIED「报告并列槽位 apiFindings」：存量 backlog 场景 15 / 26 / 41 → 12 / 27 / 39（`status` 仍不抬黄灯）；③ MODIFIED「契约判据的验证语料与报告数基线」：基线表 12 / 27、切分口径三段读数串讲、两段制整段退场（改「基线单段」）、三个 scenario（试点契约落地后全量纳入、报告数与基线逐项相等、假阳假阴 13 / 11）
- **desktop-archi-api** — MODIFIED「生成物落点与入口查找」：仅「本仓入口面实测命中」scenario 换数（`Resolved` 30 / `Absent` 12 / `Invalid` 0），并把「届时 30 / 12」的预告口吻改为已成立口径；三态解析链、四条判据、落点规则、`#[path]` 值与归属三候选逐字不动
- **desktop-archi-model** — MODIFIED「架构模型文件格式与字段封闭」：只改示例 TOML 的注释（「本变更模型数据零处」→ 指向「三处 `entry` 在案」的现口径）；字段面八项、`entry` 单值语义、两个指针边界与八条 scenario 逐字不动

| 不涉及 | 理由 |
|---|---|
| `desktop-archi-drift` / `desktop-archi-gate` | 报告结构与 `status` 推导零改动（纳管子集未变，`status` 仍 `clean`）；门禁布尔只看 `errors` 与结构错 |
| `desktop-archi-contract-panel` | 呈现面按 `category` 分组呈现同一份 `apiFindings`，条目由 15 条减为 12 条不需改代码（`entry` chip 仍在，只是不再挂壳） |
| `desktop-archi-agent` | 知识文本 `archi-knowledge.md` 的字段面已含 `entry`（A 落地），本次无数据面知识可补 |
| `desktop-ipc-type-bindings` | `ArchiElement.entry` 已在绑定额面；模型数据变化不触发再生成 |
| `desktop-archi-file-tree` / `desktop-archi-canvas` | 扫描面按 `path` 枚举，`entry` 只影响入口解析，树与画布零变化 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `.dev-team/architecture/models/04-backend-shell.toml` | `App.Backend.Shell` 加一行 `entry`；13 条出边、`path`、`gate`、`summary` 与表头计数（1 元素 / 13 关系）逐字不动 |
| `.dev-team/architecture/models/05-backend-tools.toml` | 两个工具 crate 各加一行 `entry`；两行 `api = "none"`、3 条关系、表头计数（3 元素 / 3 关系）逐字不动 |
| `packages/desktop/package.json` | `version` `0.6.46` → `0.6.47` |
| `packages/desktop/src-tauri/tauri.conf.json` | **零内容改动**（`"version": "../package.json"` 跟随；实现时只需确认未写死版本号） |

### 测试文件

| 对象 | 判断 |
|---|---|
| 新增测试 | **无**。本变更零代码逻辑；仓库模型属数据资产，`desktop-archi-api-check` 明写「MUST NOT 把本仓模型与契约塞进 crate 语料」，基线表亦明写「不写进断言（易随元素增减漂移）」——为两行数据新增测试或把 12 / 27 写进期望值，会把「报告数与实测对拍」的责任从 change 留痕搬到脆弱常量上 |
| 改写既有测试 | **无**。全仓测试零读取 `.dev-team/architecture/models/**`（前端 fixture 里的 `modelFile` 是字面路径字符串，不读盘；crate 语料自足）；`ArchiElement.entry` 字段面 A 已补齐 |
| 验收留痕 | 本 change `reports/` 下留 `apiFindings` 分桶实测（按 `kind` 分桶的元素 FQN 清单）与 `grep` 计数结果；「实现期实测表与报告逐项相等」即本变更的断言锚点 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/models/05-backend-tools.toml` 的两行 `api = "none"` | 与 `entry` 是两条不同事实；删掉任一行，两工具 crate 会从「零判据」掉回 `apiMissing`（AC-5 直接失败） |
| `.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,06-frontend}.toml` | 元素 / 关系 / `path` / `gate` 逐字不变；96 条前端与后端边零改动 |
| `.dev-team/architecture/models/**` 的其余元素 | 单文件根与 14 个 crate 由约定默认覆盖，声明 `entry` 即违字段纪律（AC-1 的「合计 3」是拦挡） |
| `.dev-team/architecture/api/**`（含 `INDEX.toml` 与四类生成物） | 本变更零契约改动、零生成物产出的期望是**验收判据**（AC-6）；任何 diff 都说明 `entry` 落位写错 |
| `crates/**` / `packages/desktop/src-tauri/src/**` / `packages/desktop/src/**` | 零代码逻辑改动（`entry` 字段、三态解析链、四条判据、纳管口径 A 已交付且正确） |
| `AGENT.md` / `CLAUDE.md` | 契约纪律节的第②条 A 已同步为「实现入口同级」；本变更不新增字段知识（`entry` 的字段说明归 `archi-knowledge.md`，已含） |
| `openspec/specs/**` 的其余能力与其余 requirement | 只有「能力」表点名的六个 requirement 换数换措辞；十值词汇、求值互斥序、落点规则、归属三候选、`gate` / `api` 字段语义逐字不动 |

---

## 验收标准

1. **AC-1 三处 `entry` 在案且形态合法**：`grep -c '^entry = ' .dev-team/architecture/models/*.toml` 合计 **3**（`04` 1 行、`05` 2 行，其余四份 0 行）；三行均为带 `./` 前缀的单值字符串（非数组、非空串），且逐条落在各自元素 `path` 之内（壳落在 `./packages/desktop/src-tauri/src/` 内、两工具 crate 落在各自 crate 目录内）——落位越界会由 `entry_state` 判 `OutsidePath` 并仍是 `apiEntryMissing`，AC-3 立刻偏离。
2. **AC-2 模型零结构错、计数不变**：`archi_graph(root)`（宿主须由本工作树重建）返回 51 元素 / 113 关系、`diagnostics` 为空、`warnings` 为空；逐文件计数与表头注释同值（`04` 1 / 13、`05` 3 / 3、其余不变）。反向判据：若 `diagnostics` 出现 `unknownField`（指向 `entry`）即宿主落后于本次数据，MUST 重建宿主后重跑，MUST NOT 改模型去迁就宿主。
3. **AC-3 分桶换段，逐条可点名**：一次全量检查（门禁步 `LocalArchiCheck` 与 `archi_drift` 两面逐字段相等，取任一面并落 `reports/` 留痕）后 `apiFindings` 总 **39**——`apiEntryMissing` **12**（逐条 = `App.Frontend.Components.Ui` + 8 个视图目录根 `Views.{Agents,AgentDebug,Architecture,Changes,Config,DbInspector,Explores,Info}` + 3 个视图内子域 `Views.Changes.{Flow,Renderers,Agent}`）、`apiMissing` **27**（13 个 crate 元素 + `App.Frontend.Shell` + 12 个前端单文件根 + **`App.Backend.Shell`**）、其余七条一致性判据各 0、`apiUndeclared` 0。壳 MUST NOT 同时出现在两个桶里；两个工具 crate 两桶皆零。
   - **口径纪律**：若实测与预期不符，MUST 先查「是否有别的元素进出判据面」（例：变更 B 若已落地，① 应再减 12 而非停在 12；新增 crate 会让 ② 增 1），MUST NOT 改判据、放宽门槛或就地改基线数字来对上预期。
4. **AC-4 壳的两类债各归其位**：`kind == "apiEntryMissing"` 且 `element == "App.Backend.Shell"` 的条目 **0**；同趟 `element == "App.Backend.Shell"` 的 `apiMissing` **恰 1 条**（文案点名缺失的契约文件路径）。契约债 ≠ 入口债：AC-4 只证明「入口债消了」，缺契约照旧在案，MUST NOT 把它读成「壳已清」。
5. **AC-5 两工具 crate 零判据**：`element ∈ {App.Backend.Tools.ExportBindings, App.Backend.Tools.ArchiApiGen}` 在 `apiFindings` 内**零条目**（`entry` 命中 ⇒ ① 不成立；`api = none` ⇒ ② 不适用；⑤ 无契约文件 ⇒ 不成立）。该结论依赖「`entry` 与 `api = none` 两条同时在场」，任一缺失即失败。
6. **AC-6 生成物与索引零 diff**：`pnpm run api:check` 零差异（`missing` / `mismatched` / `orphaned` 三列皆空）；`.dev-team/architecture/api/INDEX.toml` 与四类生成物（`*.schema.json` / `*.i.rs` / `*.i.ts` / `INDEX.toml`）`git status --porcelain` 为空；`INDEX.toml` 内壳与两工具 crate 的 `generated = ""` 逐字不变（无契约 ⇒ 无生成物，`entry` 不改变这条）。
7. **AC-7 spec 六处换段且互不矛盾**：本 change 的 delta（`openspec/changes/module-entry-data-backfill/specs/**`）内：`grep -n '15 / 26'` 的每一命中都必须落在 `## MODIFIED Requirements` 之前的 Purpose 段，或落在同句含「旧」/「退场」/「变为」的历史叙述句内；`grep '26 条'` 零命中——即旧数只作为已退场口径出现，MUST NOT 有任何一处把它当现值；`12 / 27` 在基线表与「基线单段」两处在案；`repo-archi-model` delta 内「显式 `entry` SHALL 为 0 处」与「归后续变更」零命中、「恰三处」在案、字段纪律 scenario 写「恰三行」；`desktop-archi-api` delta 的该 scenario 写 `Resolved` 30 / `Absent` 12 且「届时」零命中；`desktop-archi-model` delta 的示例注释内「本变更模型数据零处」零命中。上述 delta 经 SpecSync 应用后，同一批判据在 `openspec/specs/**` 上逐条成立——四份存活 spec 内不得残留任何一条与 12 / 27 或 30 / 12 冲突的旧数。**MUST NOT 只改存活 spec 而让 delta 与本体各说各话**（归档后 delta 退场，存活 spec 是唯一事实源）。
8. **AC-8 版本纪律**：`packages/desktop/package.json` `version` = `0.6.47`（相对落地时 `HEAD` 现值恰好 +1 patch，**只加一次**）；`tauri.conf.json` 仍为 `"../package.json"` 引用形态；`packages/desktop/src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变。
9. **AC-9 不抬黄灯**：本次全量检查的 `status` 为 `clean`、门禁步 `passed` 为 `true`；纳管子集仍为空（唯一契约在案元素 `App.Backend.Infra.ArchiRuntime` 判据 0 条）——MUST NOT 因 39 条 backlog 把状态词常规化为 `warnings`（`desktop-archi-api-check` 既有口径）。

> **不列为 AC 的流程项**：`check` 四段链形态、生成器三子命令、判据词汇十值封闭——各有既有 spec 落点且本变更零改动，不另设无 spec 落点的断言。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **在未升级 / 未重建的宿主上验收**：旧宿主的 archi crate 不认识 `entry` ⇒ 该文件 `unknownField` + 依赖它的元素「未声明」连带 `unknownReference`（A 实测 13 条 ⇒ `status = errors`），数字仍是 15 / 26 或直接全红 | 验收第一步先跑一次全量检查：`diagnostics` 为空且能读出 `entry` 即宿主够新；否则 MUST 重建宿主后重跑。MUST NOT 以「先删 `entry` 让门禁变绿」收场 |
| R-2 | **只写 `entry` 不保留 `api = "none"`**：两工具 crate 会从「零判据」变成一条 `apiMissing`（`declared` 缺契约），AC-5 失败、② 变 28 | 两条事实同批核对（AC-1 + AC-5）；`05` 的两行 `api = "none"` 已列进「不要修改」表 |
| R-3 | **多写第四处 `entry`**：`entry` 只该出现在约定默认指不到处；给单文件根或 `src/lib.rs` 命中的 crate 补 `entry` 属「模型追代码」的反向漂移，并会让 AC-1 的「合计 3」失守 | 字段纪律写进 delta（MUST NOT 第四处）；AC-1 以 `grep -c '^entry = '` 合计 3 机械拦挡（零改动的四份文件必须是 0 行） |
| R-4 | **两段制只改一半**：`desktop-archi-api-check` 的基线表、切分口径、两段制条款、三个 scenario 与 `repo-archi-model` / `desktop-archi-api` / `desktop-archi-model` 的数字是同一事实的多处表述 | 六处同批落地（AC-7 逐处可 grep）；explore 未点到的四处（`desktop-archi-api` 的 30 / 12、`desktop-archi-model` 的示例注释、api-check 的另外两个 scenario、`repo-archi-model` 的 `api = none` 实例籍条款）已在本提案「病灶三」列全 |
| R-5 | **追溯补 A 的版本号**：A 的实现提交未 bump，那次 +1 被后续提交占用；本变更若「补两档」会造出两连跳的假历史 | 只 +1 一次（AC-8）；版本单调递增是纪律，不是账目对齐 |
| R-6 | **与变更 B 的顺序耦合**：B 落地后 ① 的 12 条会归零（barrel 命中），届时本变更的「① = 12」不再是现状 | 本变更**建议先于 B**（两个未知量分开对拍：本变更只动壳与两工具 crate，B 只动 12 个 barrel）。若 B 已先落地，AC-3 的 ① 期望改为 0、② 相应 +12，并在报告内说明成因，MUST NOT 改判据 |
| R-7 | **`grep` 计数口径写错**：正则写成 `entry` 会命中注释与 spec 文本 | 验收用 `grep -c '^entry = '`（行首、带赋值、带空格），并逐份点出 `04` 1 行 / `05` 2 行 / 其余 0 行 |
| R-8 | **`entry` 形态写偏**：值写成绝对路径、数组、去扩展名形态或落位越界，都会让解析链判 `Malformed` / `OutsidePath` 并把条目留在 ① 面（表面上「数据写了没用」） | 值逐字照抄提案 §一（带 `./`、带扩展名、落在元素 `path` 内）；AC-1 的形态断言 + AC-2 的零结构错双守；越界不得靠放宽落位判据「顺手支持」 |
| R-9 | **把 39 读成「债务变少」的错觉**：39 只比 41 少 2，且壳只是从 ① 平移到 ② | 换段口径写进 spec（三段读数串讲：28 + 13 → 15 + 26 → 12 + 27），并在 AC-3 点名每个桶的构成；MUST NOT 用「总数下降」当作契约面进展的证据 |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §7.1「是否同时给 `App.Backend.Shell` 补契约，把 ② 的 +1 也消掉」 | **不做**。契约债是独立议题，混进来会让「纯数据换段」失去可对拍性（② 的 +1 正是「入口已通、契约未写」的真话，消它要写一份壳契约，属另一 change） |
| §7.2「三处 `entry` 是否带 `./` 前缀」 | **带**。与 `path` 同口径（`repo-archi-model` 字段纪律），`normalize_root` 剥除后不参与身份；验收不受影响 |
| §7.3「基线表是否顺手改成表格 + 元素清单双形态」 | **不做**（可读性优化出 range）。本变更只换数换措辞；元素清单以 change 报告的实测留痕承载 |
| §0「硬前置：发布版已发布、宿主已升级」 | 采纳为前提条件（见「前置条件」）与 R-1：验收机宿主须由本工作树重建，`archi_graph` 能读 `entry` 即证明够新 |
| §5「与 B / C 的顺序」 | 采纳「建议先于 B」（R-6）；C 与本变更零交集，可任意先后 |
