# 设计: module-entry-data-backfill

> **变更**: module-entry-data-backfill（主线变更 A 的数据面承接 · `entry` 三处归位与基线换段）
> **日期**: 2026-11-16
> **基于**: `proposal.md`（三处 `entry` 归位 + 分桶 15 / 26 → 12 / 27 + spec 六处同步 + AC-1…AC-9 + R-1…R-9）、`explore.md`（三段读数、零生成物论证、宿主前置）与四份 spec delta（`repo-archi-model` / `desktop-archi-api-check` / `desktop-archi-api` / `desktop-archi-model`）
> **性质**: **纯数据 + 基线数字**——`04` 一行 `entry`、`05` 两行 `entry`、`package.json` 一次 patch、四份 delta 的六处换段表述（外加 delta 内两处机械口径收敛）。零代码逻辑改动、零新增判据、零生成物 diff、零新增 / 改写测试。

---

## 起点实况核实（工作树实测；Rust 前缀省略为 `packages/desktop/src-tauri/`）

| 面 | 实测 |
|---|---|
| `models/04-backend-shell.toml`（74 行） | 表头注释「1 元素 / 13 关系」；`[element."App.Backend.Shell"]` 在 `:4–8`（`kind` / `gate` / `path = ["./packages/desktop/src-tauri/src/"]` / `summary`）；`entry` 零处；13 条 `[[relation]]`（`:10–74`） |
| `models/05-backend-tools.toml`（36 行） | 表头注释「3 元素 / 3 关系」；`Tools` 容器 `:4–7`；`ExportBindings` `:9–14`（`path` `:12`、`api = "none"` `:13`）；`ArchiApiGen` `:16–21`（`path` `:19`、`api = "none"` `:20`）；3 条 `[[relation]]`（`:23–36`） |
| 模型数据计数 | `grep -c '^entry = ' models/*.toml` ⇒ 六份全 **0**；`grep -c '^api = '` ⇒ `05` **2**（均 `"none"`）、其余五份 0 |
| 逐文件元素 / 关系 | `01` 3 / 0、`02` 8 / 9、`03` 8 / 24、`04` 1 / 13、`05` 3 / 3、`06` 28 / 64 ⇒ 合计 **51 / 113** |
| 入口文件在盘 | `src/lib.rs`、`crates/tools/export-bindings/src/main.rs`、`crates/tools/archi-api-gen/src/main.rs` 三份均存在 |
| 入口解析链（零改动，本变更的消费面） | `paths::entry_candidates` `paths.rs:324`、`paths::entry_state` `:347`（`Resolved` / `Invalid(EntryProblem)` / `Absent`）、私有 `within_path` `:397`、`duplicate_of` `:416`、`form_of` `:435`；读盘只出现在 `:356` / `:372` 的 `is_file()` |
| 判据面（零改动） | `api/check.rs` ① 段经 `entry_state`：`Resolved` ⇒ 继续 ②③④；`Absent` / `Invalid(_)` ⇒ 一条 `apiEntryMissing`；② 为 `missing_contracts`（`api = none` 除外） |
| `INDEX.toml`（42 条 `[[entry]]`） | 壳 `:135–141`、`ArchiApiGen` `:144–150`、`ExportBindings` `:153–159`：三者 `status = "absent"`、`generated = ""`；唯一非空 `generated` 是 `App.Backend.Infra.ArchiRuntime` |
| 生成物现状 | `.i.rs` 全仓仅 `crates/infra/archi/src/lib.i.rs` 一份；`.i.ts` 零份；`.schema.json` 仅 `api/Infra/App.Backend.Infra.ArchiRuntime.schema.json` 一份 |
| 版本 | `packages/desktop/package.json:3` `"version": "0.6.46"`；`src-tauri/tauri.conf.json:4` `"version": "../package.json"`（引用形态）；`src-tauri/Cargo.toml:6` `version = "0.1.0"` |
| 存活 spec 的旧口径 | `openspec/specs/repo-archi-model/spec.md:30` 写「本变更落地时显式 `entry` SHALL 为 0 处」；`openspec/specs/desktop-archi-api-check/spec.md:189` 写「本段基线 = 15 / 26」与两段制 |
| 测试面 | 全仓测试零读 `.dev-team/architecture/models/**`（前端 fixture 的 `modelFile` 是字面路径字符串，如 `app.test.tsx:188`）；`ArchiElement.entry` 字段面 A 已交付（`dto.rs`） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **零生成物是结构必然，不是「碰巧没跑生成」**。`api/mod.rs` 的 `index_entries` 里 `status` 只看 `present`（= 契约文件在案），`generated_rel` 是 `generated_path_of(...).filter(|_| present)`。三个元素都没有契约 ⇒ `present = false` ⇒ `status` 恒 `absent`、`generated` 恒 `""`；`entry` 只进 `generated_path_of` 的第一因子并被 `present` 短路。故 AC-6 的判据是「核对零 diff」，**不是**「跑一次再生成看有没有变」。
2. **三处入口都走「目录型 `path` + 目录内文件」的落位分支**。`within_path` 的目录分支判据是 `!is_source_file(normalized) && entry.starts_with("{normalized}/")`；三条 `path` 均以 `/` 结尾且无扩展名 ⇒ 目录分支生效，归一后前缀分别为 `packages/desktop/src-tauri/src`、`.../tools/export-bindings`、`.../tools/archi-api-gen`。
3. **跨元素唯一判据不会误伤**。`duplicate_of` 判「其它元素的**候选表**是否含同一入口文本」；其它元素的候选只由各自 `path` 派生（`<root>/src/lib.rs` / `mod.rs` / `index.ts` / `index.tsx`，或文件型根的根文件自身），与三条 `.../src/lib.rs`、`.../src/main.rs` 零交集 ⇒ 三处均 `Resolved` 而非 `Duplicated`。
4. **形态位（`EntryForm`）对三处分别派生出值，但不产生任何产物**：壳入口以 `/src/lib.rs` 结尾 ⇒ `Crate`；两个工具 crate 的 `main.rs` ⇒ `SubdirModule`。该值只喂 `EmitTarget.form`（TS 侧形态位），而零契约 ⇒ 零渲染；MUST NOT 因「形态位有了」反推 `INDEX.toml` 该有 `generated` 值（事实校准 1 的闸更硬）。
5. **两条验收口径分离**：`api:check` / `api:export` 走 `cargo run -p archi-api-gen`（工作树编译，内含本工作树的 `archi` crate）⇒ 数据面零 diff 可用工作树机械证（AC-6）；而 `apiFindings` 分桶（AC-3）由门禁 / `archi_drift` 在**宿主进程内**用宿主内嵌的 `archi` crate 解析 ⇒ 宿主必须由本工作树重建（AC-2），否则旧宿主对 `entry` 报 `unknownField` 并连带 `unknownReference`（R-1）。
6. **计数零变化**：`entry` 是元素字段，不改 `Element` 之外任何结构，51 / 113 与逐文件分项（`04` 1 / 13、`05` 3 / 3）逐值不变；`grep -c '^\[element\.'` 与 `grep -c '^\[\[relation\]\]'` 六份逐值相同是机械自查面。
7. **存活 spec 不在实现面手改**：`openspec/specs/**` 由归档链的 spec-sync 段按 delta 增量合并（`core/orchestration/src/prompt.rs` 的 `spec_sync_prompt`：只编辑 `openspec/specs/**`、不改 change 目录）。故「六处换段」的实现落点是**四份 delta**，存活 spec 的同步由归档链承接；本变更实现期 MUST NOT 直接编辑 `openspec/specs/**`（否则与归档合并重复且失去幂等）。
8. **delta 有两处 AC-7 的字面判据未满足**（本次读盘发现，建议与数据同批收敛，见 D9）：`desktop-archi-api/spec.md` 的 Purpose 两条把「届时」原样引用（`grep '届时'` 非零），`desktop-archi-api-check/spec.md` 基线表条目未含字面 `12 / 27`（只在「基线单段」条出现）。
9. **零测试面**：`proposal.md`「测试文件」表成立（无新增、无改写）；本变更不触任何 `*_test.rs` / `*.test.tsx`。`ArchiElement.entry` 的 fixture 需求 A 已补齐。
10. **版本一次到位**：`0.6.46` → `0.6.47`；`tauri.conf.json` 是引用形态（零内容改动，只需确认未被写死）；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 三处 `entry` 的写入位置与形态 | 三处均**紧跟各自元素的 `path` 行**插入（`04` 的 `path` 之后、`05` 两个元素的 `path` 之后，`api = "none"` 行的位置不动）；值**逐字**照 proposal §一（带 `./` 前缀、带扩展名、单值字符串） | 与 `path` 同口径（`normalize_root` 剥 `./`，不参与身份）；「`entry` 紧跟 `path`」一条规则三处同款，`api` 行零位移使 diff 只有三行新增；字段纪律要求的「单值 / 带前缀」因此可 grep |
| D2 | 「MUST NOT 第四处」的机械拦挡 | 落数据后 `grep -c '^entry = ' models/*.toml` 必须合计 **3**（`04` 1、`05` 2，其余四份 0） | 单文件根元素与 14 个 `src/lib.rs` 命中的 crate 由约定默认覆盖（`entry_candidates` 的 ②/③），声明即「模型追代码」的反向漂移；`grep` 是唯一不依赖主观判断的拦挡 |
| D3 | 换段的实现机制 | **只动数据，零代码**：`check.rs` ① 经 `entry_state`，`Resolved` ⇒ 跳过 ①；壳落到 ②（`api = declared` + 契约缺失）；两工具 crate 的 ② 因 `api = none` 不适用 ⇒ 零判据 | AC-3 / AC-4 / AC-5 全部由「同一份判据 + 新数据」导出；MUST NOT 改判据种类、档位、求值互斥序或纳管口径（AC-9），MUST NOT 借换段顺手把 `status` 抬成 `warnings` |
| D4 | 零生成物 diff 的处置 | 只做零 diff **核对**（`git status --porcelain .dev-team/architecture/api` 为空、`api:check` 三列皆空），**不**跑 `api:export` 造文件；若真出现 diff，回退数据而非改生成器 | 事实校准 1：`present` 闸保证零产物；出现 diff 只可能是 `entry` 落位写错（越界 / 指到别的元素路径）或对 `present` 闸理解错——两种都该由数据修正 |
| D5 | 分桶数字的表达 | 只作**留痕**（change `reports/` 实测 + 逐条元素 FQN 清单），**不进断言、不写成常量**；登记口径写成「12 条逐条可点名 + 27 条构成公式（13 crate + `App.Frontend.Shell` + 12 前端单文件根 + `App.Backend.Shell`）」 | `desktop-archi-api-check` 既有纪律「基线表本身不写进断言」（易随元素增减漂移）；「逐条可点名」让桶内成员变动（如变更 B 落地）当场可见 |
| D6 | 宿主前置与反向纪律 | 验收第一步先看 `archi_graph` 能否读出 `entry`（而非 `unknownField`）且 `diagnostics` 为空、51 元素 / 113 关系；不满足 ⇒ **重建宿主后重跑** | R-1：门禁跑在宿主进程内；MUST NOT 以「删 `entry` 让门禁变绿」收场（那会把真话数据删成假绿） |
| D7 | 版本纪律 | `package.json` **只 +1 一次**（`0.6.46` → `0.6.47`）；`tauri.conf.json` 确认引用形态；`src-tauri/Cargo.toml` 的 `0.1.0` 逐字不动 | R-5：A 的实现提交未 bump 是既成事实，追溯补账会造出两连跳的假历史；版本单调递增是纪律，不是账目对齐 |
| D8 | 与变更 B 的顺序 | 本变更**先于 B**（explore §5 / R-6）；若 B 已落地，AC-3 的 ① 期望改读 0、② 改读 39，并在留痕内写明成因 | 两个未知量分开对拍：本变更只动壳与两工具 crate，B 只动 12 个 barrel；MUST NOT 就地改基线数字来「对上」预期 |
| D9 | delta 两处机械口径收敛（AC-7 字面可 grep） | ① `desktop-archi-api/spec.md` 的 Purpose 第 1/2 条：把「届时 30 / 12」的**原文引用**改为「预告口径 30 / 12」，并去掉「数据归后续变更」字样——保留「预告口吻已退场」这一事实，但 `grep '届时'` 与 `grep '归后续变更'` 全 delta 零命中；② `desktop-archi-api-check/spec.md` 基线表条目末尾补一句「两者即字面的 **12 / 27**、总 **39**」 | AC-7 以 grep 为判据且「`12 / 27` 在基线表与基线单段两处在案」是硬条件；宽窄口径的歧义用**更严的实现**消除，成本是两处措辞、零判据零数字改动。若评估按宽口径（phrases 只在该 scenario / 该 capability 内检），这两处收敛是空操作、无副作用 |
| D10 | 明确不做 | ① 不给 `App.Backend.Shell` 补契约（explore §7.1：会把 ② 的 +1 消掉，属独立议题）；② 不把基线表改成「表格 + 元素清单」双形态（§7.3，可读性优化出 range）；③ 不动 `crates/**` / `packages/desktop/src/**` / `src-tauri/src/**`；④ 不动 `AGENT.md` / `CLAUDE.md` / `archi-knowledge.md` / `decisions/0001-model-granularity.md`；⑤ 不动 `.dev-team/architecture/api/**` 与 bindings；⑥ 不动 `01` / `02` / `03` / `06` 四份模型；⑦ 不新增结构化诊断、不新增 finding kind、不加配置键 | 逐条对应 proposal「不要修改」表与 explore 未决项归属；⑦ 的边界是「纯数据换段」可比对性的前提——任何一处扩面都会让「39 比 41 少 2」不再能一句话归因 |

---

## 架构组件 (Architecture Components)

本变更**零新增组件**：三个既有组件的输入数据换值，其余面逐字不动。

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 模型数据（**改**） | 三处入口事实：壳与两个工具 crate 各一行 `entry`；`api = "none"` 两行、全部 `path` / `gate` / `summary` / 关系与表头计数逐字不动 | `.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/models/05-backend-tools.toml` | 解析面 `crates/core/archi/src/parse/toml.rs`（零改动：`#[serde(default)] entry: Option<String>`） | TOML |
| 入口解析链（**零改动 · 被消费**） | `entry_candidates`（纯文本）/ `entry_state`（唯一读盘点）/ `within_path` / `duplicate_of` / `form_of` | `crates/core/archi/src/api/paths.rs` | `crate::scan::is_source_file`、`std::fs`（`is_file`） | 纯函数 + 存在性探测 |
| 契约判据面（**零改动 · 被消费**） | ① 结构前提经 `entry_state`；`Resolved` ⇒ 继续 ②③④，否则一条 `apiEntryMissing`；② 对 `api = none` 不适用 | `crates/core/archi/src/api/check.rs` | `api::paths`、`contract`、`schema` | 纯函数出 `Vec<ArchiApiFinding>` |
| 契约面装配（**零改动 · 被消费**） | `generated_path_of` 取 `Resolved` 入口；`index_entries` 的 `present` 闸使 `generated` 仍为空 | `crates/core/archi/src/api/mod.rs` | `api::paths` | 纯读盘、零写盘 |
| spec 面（**改 · delta**） | 六处换段表述的事实源；存活 spec 由归档链 spec-sync 段合并 | `openspec/changes/module-entry-data-backfill/specs/**`（→ 归档期落 `openspec/specs/**`） | spec-sync 段（`core/orchestration/src/prompt.rs`） | markdown |
| 版本面（**改**） | `version` 一次 patch | `packages/desktop/package.json`（`tauri.conf.json` 经引用跟随） | 既有 `tauri.conf.json` 的 `"version": "../package.json"` | JSON |

### 数据流（改后）

```
models/04,05 ──TomlParser──> Element{ entry: Some(...), path } ──dto::to_element──> ArchiElement{ entry, path }
                                            │
                     paths::entry_state（唯一读盘点 is_file）
                     ① is_repo_relative → ② within_path → ③ is_file → ④ duplicate_of
                                            │
        ┌───────────────────────────────────┼──────────────────────────────────────┐
        ▼                                   ▼                                      ▼
壳 App.Backend.Shell                 ExportBindings / ArchiApiGen              其余 41 个 path 非空元素
Resolved{form: Crate, ...}           Resolved{form: SubdirModule, ...}         数据与解析面零变化
        │                                   │                                      │
check ① 不成立 → ② declared 缺契约   check ① 不成立 + ② api=none 不适用        （27 → 30 的 Resolved 增量仅此 3 条）
⇒ 1 条 apiMissing                    ⇒ 零判据
        │                                   │
index_entries: present=false ⇒ status="absent"、generated=""（三处一致，零生成物）
```

---

## 变更清单 (Change Inventory)

<!-- 新增文件：本变更为纯数据 + 措辞换段，不新增任何源码 / 数据 / 配置 / 文档文件；
     验收留痕（reports/ 下的 apiFindings 分桶实测）由验收与 static-check 面产出，不属实现面。 -->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `.dev-team/architecture/models/04-backend-shell.toml` | 在 `[element."App.Backend.Shell"]` 块的 `path = ["./packages/desktop/src-tauri/src/"]` 行**之后**插入一行 `entry = "./packages/desktop/src-tauri/src/lib.rs"`（新增第 8 行）；`kind` / `gate` / `path` / `summary`、13 条 `[[relation]]` 与表头注释「1 元素 / 13 关系」逐字不动 | D1 / AC-1 / AC-2：目录型壳的约定默认拼成 `.../src-tauri/src/src/lib.rs`（指不到），显式值是字段纪律允许的两类正当用法之一 |
| `.dev-team/architecture/models/05-backend-tools.toml` | ① `App.Backend.Tools.ExportBindings` 块的 `path` 行之后插入 `entry = "./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs"`；② `App.Backend.Tools.ArchiApiGen` 块的 `path` 行之后插入 `entry = "./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs"`；两行 `api = "none"` **位置与内容原样保留**，3 条 `[[relation]]` 与表头注释「3 元素 / 3 关系」逐字不动 | D1 / AC-1 / AC-5：二进制 crate 无 `src/lib.rs`，`main.rs` 不在约定候选列内；`entry` 与 `api = none` 是两个不同事实（有入口 / 无库 API），缺任一即 AC-5 失败（R-2） |
| `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api/spec.md` | 只改 `## Purpose` 段第 1、2 条：第 1 条把「并注『届时 30 / 12』」改为「另注预告口径 30 / 12」；第 2 条把「原文把 30 / 12 写成『届时』」改为「原文把 30 / 12 写成预告口径」，并删去「（数据归后续变更）」括注；`## MODIFIED Requirements` 及 `## Module Contract` 逐字不动 | D9 / AC-7：使 `grep '届时'` 在 `desktop-archi-api` delta 内零命中，同时保留「预告口吻已退场」的事实叙述；不动判据、不动 `Resolved 30 / Absent 12 / Invalid 0` 数字 |
| `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api-check/spec.md` | 只改「基线表」条目末尾：在「`apiUndeclared` 不适用（`off`）」之后补一句「——两者即字面的 **12 / 27**、总 **39**」；分桶构成、三条切分口径、基线单段条、五个 scenario 逐字不动 | D9 / AC-7：使字面 `12 / 27` 在「基线表」与「基线单段」两处均在案；零数字改动（12 与 27 原本已逐条列出） |
| `packages/desktop/package.json` | `"version"` 由 `0.6.46` 改为 `0.6.47`（相对落地时 `HEAD` 现值**只 +1 一次**）；其余键与 `scripts` 段零改动 | D7 / AC-8；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |
| `packages/desktop/src-tauri/tauri.conf.json` | **零内容改动**（只需确认仍是 `"version": "../package.json"` 引用形态、未被写成字面版本号；`git diff` 必须为空） | D7 / AC-8：proposal「实现文件」表登记为「零内容改动」的确认项 |

<!-- 公共函数 / API：本变更零代码逻辑改动——`api::paths` 的 `entry_candidates` / `entry_state` /
     `generated_path`、`api::check::check`、`api::mod::{index_entries, generated_path_of}`、
     `entries` 的 IPC 面无任何签名 / 行为变化，故本子表整段省略。 -->

<!-- 类型定义：`Element.entry` / `RawElement.entry` / `ArchiElement.entry` 三处字段面由变更 A
     交付且正确（proposal「测试文件」表与代码实测一致），本变更不新增 / 不修改任何类型，故省略。 -->

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.46` → **`0.6.47`** | 只 +1 一次（D7）；`tauri.conf.json` 经引用跟随 |
| `version` | `packages/desktop/src-tauri/tauri.conf.json` | **零改动** | `string` | `"../package.json"`（引用形态） | 实现期只核对形态，不写入字面号 |
| `version`（crate） | `packages/desktop/src-tauri/Cargo.toml` | **零改动** | `string` | `0.1.0` | crate 版本线独立，MUST NOT 随 `package.json` |
| `entry`（模型元素字段） | `.dev-team/architecture/models/04-backend-shell.toml` / `05-backend-tools.toml` | 新增三处值 | `string`（单值，`./` 前缀、repo 相对） | 无（字段缺省 `null`） | 三值见「数据模型」表；单文件根与 14 个 `src/lib.rs` 命中的 crate MUST NOT 声明（D2） |
| `entry`（字段定义 / 解析） | `crates/core/archi/src/{model,parse/toml,dto,api/paths}.rs` | **零改动** | `Option<String>` | `None` / 线面 `null` | 字段面与三态解析链由变更 A 交付（事实校准 9）；本变更只写数据 |

> 配置项零新增：`.dev-team/config.json`、`.dev-team/architecture/config.json`（`exclude: ["openspec/**"]`）、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`capabilities/default.json`、`package.json` 的 `scripts` 段全部零 diff。

---

## 数据模型 (Data Model)

### 模型元素字段（本变更唯一的数据面变化）

| 元素 FQN | 文件（落地后位置） | `entry` 值 | 归一后 | 派生态 | 判据面后果 |
|---|---|---|---|---|---|
| `App.Backend.Shell` | `04-backend-shell.toml`，`path` 行后（新增第 8 行） | `./packages/desktop/src-tauri/src/lib.rs` | `packages/desktop/src-tauri/src/lib.rs` | `Resolved { form: Crate }`（以 `/src/lib.rs` 结尾）；落位判据走目录分支 `.../src` + `/` | ① `apiEntryMissing` 不再成立；② `apiMissing` 恰 1 条（`api` 缺省 `declared`、契约文件缺失） |
| `App.Backend.Tools.ExportBindings` | `05-backend-tools.toml`，`path` 行后 | `./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs` | 同左去 `./` | `Resolved { form: SubdirModule }` | ① 不成立 + ② `api = "none"` 不适用 ⇒ **零条目** |
| `App.Backend.Tools.ArchiApiGen` | 同上 | `./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` | 同左去 `./` | `Resolved { form: SubdirModule }` | 同上 ⇒ **零条目** |

- 字段语义（单值 / repo 相对 POSIX / 四条判据 / 三态解析链）由变更 A 定义，本变更零改动。
- 三处均落在各自元素某条 `path` 之内（D1 + 事实校准 2），故 `entry_state` 判 `Resolved`，不落 `OutsidePath` / `NotOnDisk` / `Duplicated` / `Malformed`。
- **不得有第四处**（D2）；`entry` 只出现在 `04` （1 行）与 `05`（2 行），`01` / `02` / `03` / `06` 零行。

### 派生读数（非持久化，落 change `reports/` 留痕）

| 读数 | 期望值 | 构成 |
|---|---|---|
| `entry_state` 分布（42 个 `path` 非空元素） | `Resolved` **30** / `Absent` **12** / `Invalid` **0** | 27（14 crate 的 `src/lib.rs` + `App.Frontend.Shell` 文件型根 + 12 个前端单文件根）+ **3**（本次三处显式 `entry`）；`Absent` = 12 个前端目录根 |
| `apiFindings` 分桶 | `apiEntryMissing` **12** / `apiMissing` **27** / 其余七条 **0** / `apiUndeclared` **0** / 总 **39** | ① 12 = `App.Frontend.Components.Ui` + 8 个视图目录根 + 3 个视图内子域；② 27 = 14 − 1 试点 = 13 crate + `App.Frontend.Shell` + 12 前端单文件根 + `App.Backend.Shell` |
| `status` / `passed` | `clean` / `true` | 纳管子集仍只有 `App.Backend.Infra.ArchiRuntime`（判据 0 条） |
| 模型规模 | 51 元素 / 113 关系，`diagnostics` 空 | `entry` 是元素字段，不改计数（事实校准 6） |

### 持久化面变化

| 对象 | 变化 |
|---|---|
| `.dev-team/architecture/models/04-backend-shell.toml` | +1 行（`entry`），其余逐字不变 |
| `.dev-team/architecture/models/05-backend-tools.toml` | +2 行（两个 `entry`），`api = "none"` 两行与 3 条关系逐字不变 |
| `.dev-team/architecture/api/INDEX.toml` | **零变化**（`present` 闸；事实校准 1） |
| 四类生成物（`*.schema.json` / `*.i.rs` / `*.i.ts` / `INDEX.toml`） | **零变化**（同上）；出现 diff 即 `entry` 落位写错，回退数据（D4） |
| `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}` | **零变化**（`ArchiElement.entry` 已存在；模型数据变化不触发再生成） |

---

<!-- 路由 / API 设计：零新增 IPC / HTTP 端点 / CLI 子命令；`archi_graph` / `archi_module` /
     `archi_drift` 的入出参形态零变化（`ArchiElement.entry` 由变更 A 追加，本次只换其数据值），
     故本段省略。 -->

## 依赖 (Dependencies)

运行时依赖：**零新增**。本变更不改任何代码，`Cargo.toml` / `Cargo.lock` / `package.json` 的依赖段零 diff。

构建 / 测试依赖：**零新增**。

工具面（实现与核对用，均为既有能力）：

| 能力 | 用途 |
|---|---|
| `grep -c '^entry = '` / `grep -c '^api = '` / `grep -c '^\[element\.'` / `grep -c '^\[\[relation\]\]'` | AC-1 的形态与计数机械自查（R-7：必须是行首 + 赋值 + 空格，不能用裸 `entry` 正则） |
| `git status --porcelain` / `git diff --stat` | AC-6 / AC-8 的零 diff 面（`api/**`、bindings、`Cargo.toml`、四份未触模型） |
| `pnpm run api:check`（`cargo run -p archi-api-gen -- --check`） | AC-6：工作树编译的 CLI 读盘比对，`missing` / `mismatched` / `orphaned` 三列皆空 |
| `archi_graph` / `archi_drift`（宿主内，宿主须由本工作树重建） | AC-2 / AC-3 / AC-9：51 元素 / 113 关系、`diagnostics` 空、`apiFindings` 分桶与 `status` |

---

## 验收标准回应

| AC | 设计落点 | 归属阶段（tasks） |
|---|---|---|
| AC-1 三处 `entry` 在案且形态合法 | D1（位置 + 逐字值）、D2（合计 3 的机械拦挡）、事实校准 2（落位判据走目录分支 ⇒ 不落 `OutsidePath`） | 一 |
| AC-2 模型零结构错、计数不变 | 事实校准 6（`entry` 不动计数）、D6（宿主重建前置：`diagnostics` 空 + 能读出 `entry`；反向纪律：MUST NOT 删数据迁就宿主） | 一（数据）/ 五（移交实测） |
| AC-3 分桶换段、逐条可点名 | D3（唯一机制是数据）、D5（只留痕不进断言、口径写成「12 条点名 + 27 条公式」）、D8（与 B 的顺序及降级口径） | 二（零 diff 面）/ 五（移交实测与留痕） |
| AC-4 壳的两类债各归其位 | 数据模型表第一行（① 不成立 + ② 恰 1 条）；D3 明确「入口债消了 ≠ 契约债消了」 | 五 |
| AC-5 两工具 crate 零判据 | 数据模型表后两行 + `05` 的 `api = "none"` 逐字保留（R-2）；`entry` 与 `api` 两条事实同时在场是判据前提 | 一 / 二 |
| AC-6 生成物与索引零 diff | 事实校准 1（`present` 闸的结构论证）、D4（只核对不生成；有 diff 即回退数据） | 二 |
| AC-7 spec 六处换段且互不矛盾 | 四份 delta 为事实源（六处表述已在案）；D9 收敛两处字面判据；事实校准 7（存活 spec 由归档链 spec-sync 合并，实现面不手改） | 三 |
| AC-8 版本纪律 | D7（`0.6.46` → `0.6.47` 只 +1；`tauri.conf.json` 引用形态；`Cargo.toml` 零改动） | 四 |
| AC-9 不抬黄灯 | D3 末句（纳管口径不动）、数据模型表「`status` / `passed`」读数行 | 五（移交实测） |

> **不列为 AC 的流程项**（沿 proposal 末尾声明）：`check` 四段链形态、生成器三子命令、十值判据词汇各有既有 spec 落点且本变更零改动，故本设计不另立验收项。
> **测试面**：本变更零新增 / 零改写测试（proposal「测试文件」表 + 事实校准 9）；`api:check`、`server:check`、`client:check`、`apiFindings` 分桶实测由桌面端 static-check / test-execution 承接，实现相位不重复执行机械步骤。

---

## 待决问题 (Open Questions)

1. **AC-7 的 grep 口径（宽 / 严）**：AC-7 对「届时」写「`desktop-archi-api` delta 的该 scenario … 零命中」，对「归后续变更」写「`repo-archi-model` delta 内零命中」——若按 capability / scenario 作用域读，现状已满足；若按全 `specs/**` 读，则 Purpose 段两处需要收敛。本设计按**更严口径**处置（D9），代价是两处措辞、零判据零数字改动；若评估按宽口径，这两处收敛为空操作。反向判据：收敛后必须仍保留「预告口吻退场」的事实叙述（MUST NOT 把历史抹成孤证，与 R-9 同一条纪律）。
2. **宿主重建的执行归属**：AC-2 / AC-3 / AC-9 需要在「宿主由本工作树重建」的机器上跑。本设计只钉判据（`archi_graph` 能读出 `entry`、`diagnostics` 为空、51 / 113），执行者（人 / 桌面端 static-check / test-execution）不在实现面裁定；若该前置不成立，验收 MUST 停在「宿主落后」这一结论上，MUST NOT 降级为改数据迁就宿主（R-1）。
3. **「先于变更 B」是否升为硬前置**：R-6 给的是「建议先于 B + 若 B 已落地则 ① 读 0、② 读 39 并说明成因」的软约束。本设计保持软约束（spec 已明写 MUST NOT 就地改基线数字），排序由编排保证；若评估要求硬前置，须由 change 顺序而非本设计裁定。
