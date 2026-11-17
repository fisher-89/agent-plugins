# 设计: frontend-component-element-coverage

> **变更**: frontend-component-element-coverage（前端组件元素覆盖补齐 · 模型追上判据）
> **日期**: 2026-11-16
> **基于**: `proposal.md`（两元素 + 七边 + 计数三处换段 + `apiFindings` 12 / 29 + 两份 delta 共 8 个 MODIFIED requirement + AC-1…AC-10 + R-1…R-9）、`explore.md`（§3 实测对拍、§4.2 的 ★ 待核项、§9 全量留痕前置），以及本 change `specs/` 下两份已在案的 delta（`repo-archi-model` / `desktop-archi-api-check`）
> **性质**: **纯数据 + 基线数字**——`06` 补 2 个 `[element]` 与 7 条 `[[relation]]`、表头两处数、ADR 六个小节同步、`INDEX.toml` 按新元素面重生成（+2 条全空槽位）、`package.json` 一次 patch。**零源码改动、零判据改动、零测试新增 / 改写**。
> **AC 修正（据实测提出）**: AC-6 的白名单与「零生成物」两处按**结构必然**修正（D5，已获裁决）；AC-4 的两个字面观测面在 `gate` 全 `off` 语义下不存在（`scan/mod.rs:148–176` + 归档报告实测），故替换为更强的等价观测（D6）；AC-2 的 `Owner::Element` 断言改由既有出线面 `archi_file_tree.owners` 机械读出（D7，无需新增 IPC 或测试专用导出）；AC-7 的缺口登记由「三条」改**四条**——`crates/tools/specta-zod-patched` 是 workspace 成员、落在 `§2` crate 判据内却无元素（D4，与两份 delta 的已知缺口清单同源）；AC-1 / AC-3 / AC-4 / AC-5 的**全量实测归属**按机械事实重排（D11）：自动落盘点只有门禁 runner 且恒增量（`runner.rs:72`）、`archi_drift` 命令零写盘（`commands/archi/mod.rs:14–17`、`:163–196`），故全量报告与全量槽位读数一律移交，实现面只核「与扫描模式无关」的静态面。AC-8 / AC-9 / AC-10 按原文落地。

---

## 起点实况核实（工作树实测）

| 面 | 实测 |
|---|---|
| `models/06-frontend.toml`（488 行） | 表头两行写「28 元素 / 64 关系」「共享组件文件 10 + 原语层 1」（`:1–2`）；`[element]` 28 段、`[[relation]]` 64 条；元素顺序为 Shell → `Views` 容器 → 9 视图 → 3 子域 → `Components` 容器 → `Components.Agent`（持 barrel）→ 6 个 agent 组件（字典序）→ `Components.Standard` 容器 → 4 个 standard 组件（字典序）→ `Ui`；关系按 `from` 分组、组内按 `to` 字面字典序（64 条逐条核对无例外） |
| 六模型计数 | `01` 3 / 0、`02` 8 / 9、`03` 8 / 24、`04` 1 / 13、`05` 3 / 3、`06` 28 / 64 ⇒ 合计 **51 / 113** |
| 两个待补文件在盘 | `components/agent/session-panel.tsx`（`import` 行：`:4` ui/button、`:5` ui/resizable、`:6` hooks/use-agent-session、`:7` `import type { SessionSummary } from '@/types/generated/bindings'`、`:9` `import { AgentSessionView, type AgentSessionActions } from './agent-session-view'`）；`components/standard/tooltip.tsx`（`:1` ui/tooltip，全文件单一 import） |
| 桶导出 | `components/agent/index.tsx:5` `export { SessionPanel } from './session-panel'`、`:6` `export type { SessionPanelState }`；同文件另有 `AskAnswerActions` 的 type re-export（`agent-messages`）——属 §8 已登记的 11 条既有边面缺口之一，本变更**不补** |
| 视图侧入边 | `views/changes/change-list-view.tsx:5`、`views/explores/explore-list-view.tsx:5` 各一条 `import { TextTooltip } from '@/components/standard/tooltip'`；全仓 `standard/tooltip` 的 import 仅此两处（其余为测试） |
| 归属判据 | 与既有文件型 `path` 前缀零冲突：`components/agent/` 下只有 `index.tsx` 被容器持有（文件型根，只覆盖自身），`components/standard/` 无目录型元素 ⇒ 两文件必落 `Owner::Element` 而非 `Ambiguous` |
| 扫描面语义 | `scan/mod.rs:148–176`：`Owner::Element { gate: Off }` 的文件**照常入 `unmatched_files`**（与无主同视，依赖仍抽取），只有非 `off` 元素才入 `matched`；⑥ 聚合因此在本仓（`gate` 全 `off`）**恒为 `[]`** |
| 归档报告实测（佐证上行） | `archive/2026-11-12-archi-file-tree/reports/archi/20261112T0154386659152Z.json`：`scannedFiles 12` / `unmatchedFiles 12` / `matched []`，且 12 条全部是**已被元素覆盖**的文件（如 `crates/core/archi/src/scan/mod.rs`、`views/architecture/components/file-tree.tsx`） |
| 声明兑现面 | `drift/mod.rs:277–290`：`declared − 有证据的边` 逐条落 `observations[].unfulfilledDeclaration`（全量 / 增量各一句 message，不进 `status` / `passed`）；`off` 引入方的未声明边零条目（`:190`） |
| 命令面边现值 | 模型内 `to = "App.Backend.Shell"` 的前端边恰 **11** 条（`AgentSessionView` + 10 个视图），全部恒落 `observations`（目标 `src/types/**` 无归属 ⇒ `Owner::None` ⇒ 丢边） |
| 归属可观测面 | `archi_file_tree` 的条目带 `mark` / `owners`（`dto.rs:385–400`、`tree.rs:135–159`）：文件型根命中 `mark = "elementRoot"` + `owners` 恰 1 个 FQN，歧义命中 `mark = "ambiguous"` + `owners ≥ 2` |
| 生成物面 | `INDEX.toml` 恰 **42** 条 `[[entry]]` = 全部 `path` 非空元素（`api/mod.rs:501` `index_entries`，`status` 只由契约文件是否在案决定），按 FQN 字典序；`api/diff.rs:32` `compare` 把 `workspace_render` 的应生成集（**恒含 `INDEX.toml`**）与盘上实况逐字节比对 |
| `apiFindings` 现值 | 归档报告 `.../module-entry-data-backfill/reports/archi/20261116T1024554217206Z.json`：`apiEntryMissing 12` / `apiMissing 27` / 其余 0，`status = noChanges`（增量、`scannedFiles 0`）、`observations 113`（增量不扫描 ⇒ 全部声明皆未兑现） |
| 落盘面 | `infra/archi/src/report.rs:21` 的 `write_report` 是本 crate 唯一写盘面，唯一调用点是门禁 runner（`runner.rs:72` 恒 `Incremental`）；命令面 `archi_drift` 零写盘（`commands/archi/mod.rs:14–17` 明写、`:163–196` 纯组装）⇒ **全量报告无自动产出通道**，只能由持 IPC 的相位手写落盘（D11） |
| 本 change 现有报告 | `reports/archi/20261116T145803082118Z.json`：`mode = "incremental"` / `scan.scannedFiles 0` / `unmatchedFiles []` / `observations 120` / `status = noChanges` ⇒ **裸名 + 增量内容**，按 D11 须改名 `incremental-<UTC>.json` 并在 `model-sync-check.md` 声明「非基线」 |
| 版本面 | `package.json:3` `0.6.47`；`tauri.conf.json:4` `"version": "../package.json"`（引用形态）；`src-tauri/Cargo.toml:6` `version = "0.1.0"` |
| 存活 spec 现值 | `openspec/specs/repo-archi-model/spec.md:24/42/171/188/204/205/209/217/220/301/315/331/347/348/351` 与 `desktop-archi-api-check/spec.md:79/171/187/189/204` 仍是旧口径；两份 delta 已在 change 目录内写好（8 个 MODIFIED requirement） |

---

## 事实校准（不改判据，只定实现形态）

1. **七条边逐条可核**（proposal §二已定口径，本次逐行复核，行号即证据）：①`session-panel.tsx:9` → `AgentSessionView`；②`:4–5` 两处 ui import 按**目标元素**去重 = 1 条 → `Components.Ui`；③`:7` 的 `import type` → `App.Backend.Shell`（`type_only` 在扫描面只标注不豁免，但目标 `src/types/**` 无归属 ⇒ 扫描侧丢边 ⇒ 声明式约定）；④`components/agent/index.tsx:5–6` 两行按目标元素去重 = 1 条（barrel 端点边）；⑤`tooltip.tsx:1` → `Components.Ui`；⑥`change-list-view.tsx:5`、⑦`explore-list-view.tsx:5` → `Standard.Tooltip`。**净 +7 与新元素的出 / 入边集合逐条相等，无遗漏项**：`session-panel.tsx` 的 `@/hooks/use-agent-session` 属封闭排除面、零边。
2. **计数三处同值的算术闭合**：元素 `3+8+8+1+3+30 = 53`（容器 10 + 叶 43）、关系 `0+9+24+13+3+71 = 120`；前端 `71 = 64 + 7`（叶边 `58 + 6 = 64`、barrel 端点边 `6 + 1 = 7`）；源数 `22 → 24`（新增两个 `from`：`SessionPanel`、`Tooltip`；`Views.Changes` / `Views.Explores` / barrel 早已是源）；`06` 表头「30 元素 = 页壳 1 + 视图 9 + 子域 3 + 共享组件文件 12 + 原语层 1（= 26 叶）+ 4 容器」。
3. **`INDEX.toml` 必然 +2，proposal 的「零生成物 diff」不成立**。`index_entries` 对**每个** `path` 非空元素产一条（与契约有无无关），`compare` 恒把 `INDEX.toml` 纳入应生成集；两个新元素各有 `path` ⇒ 该文件必须由 42 条变 44 条。**不重生成** ⇒ `pnpm api:check` 的 `mismatched` 非空（AC-6 的「三列皆空」与仓库 `check` 链同时红）；**重生成** ⇒ 只多这一份生成物 diff。二者只许取一条，故取后者（D5）。
4. **`matched` 在本仓恒空，AC-4 的两个字面判据不存在**。`gate` 全 `off` 时元素覆盖的文件进 `unmatched_files`（事实核实表两行），故：加元素前两文件已在 `unmatchedFiles`（`Owner::None` 路径）、加元素后仍在（`off` 路径）⇒ 「不再含」与「恰少两条」实测**不可能成立**；`matched` 里更不会出现任何元素（含两个新元素）。AC-4 的**意图**（两文件已获归属、扫描面无新增未覆盖）改由「集合差集 + `observations` 差集 + 文件树 `owners`」三点证明（D6 / D7）。
5. **归属的最强单点观测是 `observations` 的差集，而不是「文件是否列在某个槽位里」**：若归属失败（仍 `Owner::None`），`session-panel.tsx` 不在 `owned` 循环内 ⇒ 其三条声明（①②③）**全部**未兑现 ⇒ `observations` 增 3 条；归属成功则只有命令面那条（③）未兑现 ⇒ `observations` **恰增 1 条**。1 与 3 的差是「归属成立」的机械证据，且不依赖任何新测试。
6. **`archi_file_tree.owners` 是 AC-2 的既有出线面**：文件型根条目 `mark = "elementRoot"` 且 `owners` 恰 1 个 FQN；若与既有文件型 `path` 冲突则落 `mark = "ambiguous"` + `owners ≥ 2`——正是 AC-2 的反向判据（无需读 crate 私有 API，也无需新增 IPC）。
7. **门禁步恒增量，且自动落盘点只有它；全量报告没有自动产出通道**：`infra/archi/src/runner.rs:72` 写死 `ArchiDriftMode::Incremental`（「恒增量」语义，`runner_test.rs:337` 断言），该 runner 经 `report.rs:21` 的 `write_report`（本 crate 唯一写盘面）自动落盘；命令面 `archi_drift` 零写盘（`commands/archi/mod.rs:14–17` 明写「其余六条 MUST NOT 写盘」，`:163–196` 是参数转换 → 调判定 → 返回报告的纯组装）。⇒ 全量报告的取得只有两条路：持 IPC 的相位取 `archi_drift(root, "full")` 出参后**手写**落盘，或不产出；实现面（桌面只读工具 + `bash`）无 IPC 通道，跑门禁又只能得增量 ⇒「实现面产出全量报告」在结构上不可达。故 AC-1 / AC-4 的全量槽位读数 MUST 移交（D11）。
8. **忠实于 11 条命令面边的现值**：ADR `§7` 现在写「11 条命令面边」，本次第 12 条落地后该现值陈旧 ⇒ 同批改为 12（AC-1 的「三处同值」不含它，但它是同一次数据改动的直接推论；R-8 的「不写证据清单」纪律不受影响）。
9. **存活 spec 不在实现面手改**：`openspec/specs/**` 由归档链 spec-sync 段按 delta 增量合并（既有先例 `module-entry-data-backfill` 事实校准 7）。故本变更的 spec 落点就是 change 目录里那两份 delta（**已在案、零改动**），实现期 MUST NOT 编辑 `openspec/specs/**`。
10. **两处「42 个」现值按裁决**：`desktop-archi-api-check/spec.md:79`（requirement「十条判据的定义与求值互斥顺序」括注）与 `desktop-archi-api/spec.md:313`（scenario「本仓入口面实测命中」的 42 / 30 / 12）在本变更落地后应为 44 / 32 / 12。前者所在 requirement 不属两份 delta 的 8 个 MODIFIED；后者所在能力被 AC-10 钉为**零 diff**。经裁决：**两处都不改**，只在 change 报告里作为「邻接现值未同步」登记（含 grep 定位与承接方）——AC-8 按字面逐条判据通过（它只点名 `12 / 27` 与 `39`）。
11. **零测试面**：全仓测试零读 `.dev-team/architecture/models/**`（前端 fixture 的 `modelFile` 是字面路径，crate 语料自足）；`ArchiElement` 形状不变 ⇒ 无 fixture 需补；`bindings:export` 幂等。
12. **版本一次到位**：`0.6.47` → `0.6.48`；`tauri.conf.json` 是引用形态（零内容改动）；`src-tauri/Cargo.toml` 的 `0.1.0` MUST NOT 随动。
13. **`apiFindings` 与扫描模式无关，`status` / `observations` 与它强相关**：契约判据只吃模型与契约盘面，不经 `scan`——门禁 runner 落的增量报告（`reports/archi/20261116T145803082118Z.json`）在 `scan.scannedFiles = 0`、`status = noChanges`、`observations` 120 条（全部声明未兑现）的同一份文件里，仍点名 `apiEntryMissing 12` + `apiMissing 29` = **41**。故 AC-3 的全部读数**在实现面可读**（报告槽位现成、与模式无关），而 AC-1 / AC-4 / AC-5 的 `status` / `passed` / `unmatchedFiles` / `observations` MUST 用全量报告（D11）——MUST NOT 用增量报告验收这三条。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 两个元素的落位与字段纪律 | `SessionPanel` 插在 `[element."…Agent.AgentSessionView"]` 之后、`[…Agent.ToolCards"]` 之前；`Tooltip` 插在 `[…Standard.Select"]` 之后、`[…Components.Ui"]` 之前（两组既有顺序均为 **FQN 字典序**，插入处可机械核对）。四字段逐字取 proposal §一；`entry` / `api` **不写** | 字典序是该文件既有唯一顺序，新增段落位即「与兄弟同口径」；单文件根由约定默认解析入口（`repo-archi-model`「基线落位与模型文件划分」`entry` 条），多写即违字段纪律并使「恰三处」失守；`api` 取缺省 `declared` 才会正确落一条 `apiMissing`（AC-3） |
| D2 | 七条边的落位与 `desc` | 按 `from` 分组、组内按 `to` 字典序：④ 追加在 barrel 组（`→ AgentInput` / `→ AgentSessionView`）末尾；① ② ③ 组成 `SessionPanel` 新组，插在 `AgentSessionView` 组之后、`ToolCards` 组之前（组内序 `App.Backend.Shell` → `…Agent.AgentSessionView` → `…Components.Ui`）；⑤ 追加在 `Standard.Select` 组之后；⑥ 插在 `Views.Changes` 组的 `Standard.Dailog` 与 `Components.Ui` 之间；⑦ 插在 `Views.Explores` 组的 `Components.Agent` 与 `Components.Ui` 之间。`desc` 各一句中文用途（见「数据模型 - 关系」表），MUST NOT 写证据清单 | 现状 64 条边「`from` 分组 + 组内目标字典序」无例外，新增必须同规则，否则「按 `to` 排序」这一隐式约定被打破；`desc` 与既有同用途边共用同一句（既有先例：每个视图的 `→ App.Backend.Shell` 都写「经生成的命令面类型取数与调用」） |
| D3 | 计数三处同值 + ADR `§7` 的 11 → 12 | `06` 表头两处、`repo-archi-model` 计数表（delta 已在案）、ADR `§9` 复算表三处同值：`53 / 120`、`06` 段 `30 / 71`、恒等式 `71 = 64 + 7`；ADR `§1` / `§3` / `§6` 同批换值；`§7` 的「11 条命令面边」改 12 并补声明式约定一句 | 事实校准 2 / 8：「三处同值」是既有 requirement 的硬纪律，`§1` / `§3` / `§6` 是同一组结论的复述，漏一处即「模型本体与说明各说各话」 |
| D4 | ADR `§8` 的缺口登记形态（**AC-7 修正：三条 → 四条**） | 在既有条目（`src/hooks` 无载体一条 + 漂移纪律 / 修改纪律两条）之后**新增四条**条目：⓪ `crates/tools/specta-zod-patched`（vendored 上游 exporter，`[package] name = "specta-zod"`；根 `Cargo.toml:76` 的 `[workspace] members` 明列该目录 ⇒ 命中 `§2` crate 档「`crates/{core,infra,tools}/<dir>/` 每个成员 crate 一元素」却无元素；标注「先于本变更存在」，补齐须新增 crate 元素并连带换段全部计数，或整目录删除）；① `components/file/file-viewer.tsx`（写明「补它须改本能力：新增 `components/<其它目录>` 粒度档」）；② `views/architecture/agent/`（递归非测试源文件 3 个，命中原判据未建元素）；③ 11 条已建元素间未声明边（逐条 FQN）+ 反向缺口 1 条（`Views.Changes.Agent → Components.Ui` 已声明无证据）作为同一条目的括注。`§2`–`§5` 逐字不动 | R-3 / R-4：登记 ≠ 补齐，也 ≠ 判据失效；四条各自写清「为什么现在不补 / 补齐须做什么」；反向缺口并进第③条而不是另开条目，使登记数可机械数（四条）。**⓪ 只许取「登记」这一条路**：另一条路是在 `§2` 里把 vendored crate 排除出判据，那是改本能力（proposal 纪律「判据逐字不动」），且 delta 的「已知缺口清单 SHALL 只含『先于该 change 存在』的条目」已把它与 `file-viewer` / `views/architecture/agent` 并列为三条覆盖缺口 ⇒ AC-7 的字面「三条」不再成立，按四条落地 |
| D5 | `INDEX.toml` 的处置（**AC-6 修正**） | 跑 `pnpm run api:export` 重生成，**只允许** `INDEX.toml` 变（+2 条全空槽位：`SessionPanel` 在 `AgentSessionView` 与 `ToolCards` 之间、`Tooltip` 在 `Select` 与 `Components.Ui` 之间，六字段恒 `absent / 0 / 0 / 0 / "" / ""`）；其余三类生成物零 diff。修正后的 AC-6 判据：「`git status` ⊆ {`models/06-frontend.toml`、`decisions/0001-model-granularity.md`、`api/INDEX.toml`、`package.json`、本 change 目录}；`INDEX.toml` 的 diff 恰 +2 条全空 entry；`pnpm api:check` 三列皆空；`bindings:export` 幂等」 | 事实校准 3：生成物是元素面的函数，元素面动则生成物必动。MUST NOT 手改 `INDEX.toml`（AGENT.md「四类生成物勿手改」已提交即零 diff 门失败），也 MUST NOT 让它陈旧（`check` 链一段红）。生成的 `absent` 槽位同时是「补契约」时的落点提示 |
| D6 | AC-4 的观测面替换（**判据修正**） | 判据改为三条并列：① 全量 `scan.unmatchedFiles` 与**加元素前的全量基线**集合**逐条相等**（既不减也不增；gate 全 `off` ⇒ 归属与否都不进 `matched`，故「不再含 / 恰少两条」不可达）；② 全量 `observations` 与基线相差**恰 +1 条**且新增项恰为 `Components.Agent.SessionPanel → App.Backend.Shell`（1 条而非 3 条 ⇒ ①② 已兑现、归属成立，见事实校准 5）；③ `errors` / `warnings` 前后皆空、`scan.scannedFiles` 前后同值 | AC-4 的意图（两文件进模型、扫描面不新增未覆盖）不变，但原判据引用的两个槽位在本仓语义下不存在（事实校准 4）。替代表述比原判据更强：它同时证「归属成功」与「六条值依赖边有真实 import 证据」。MUST NOT 为让原判据成立去改 `scan` 语义（那是改能力） |
| D7 | AC-2 的观测面 | 由 `archi_file_tree` 读出：条目 `packages/desktop/src/components/agent/session-panel.tsx` 与 `…/standard/tooltip.tsx` 各为 `mark = "elementRoot"`、`owners` 恰 `["App.Frontend.Components.Agent.SessionPanel"]` / `["App.Frontend.Components.Standard.Tooltip"]`；反向：任一落 `mark = "ambiguous"` 或 `owners.len() ≥ 2` 即不合格 | 事实校准 6：`owners` 正是 `OwnerIndex::owner_of` 的同源投影（`tree.rs` 明写「归属单源」），无需新增 IPC 或导出测试专用函数（AGENT.md 规则 5） |
| D8 | 版本纪律 | `package.json` 的 `0.6.47` → **`0.6.48`**（一次、不追溯）；`tauri.conf.json` 只核对引用形态；`Cargo.toml` 零改动 | 判据写死在 proposal §六：`.dev-team/architecture/**` 是随应用交付的资产 ⇒ 发布物内容变更 +1 patch；反向分支（不 bump）已被裁决排除 |
| D9 | 两处「42 个」现值 | **不改**（事实校准 10）：只在 change 报告的「邻接现值未同步」段登记 grep 定位、目标值与承接方（下一个动元素面的变更，按 `desktop-archi-api` 的 `entry_state` 场景一次性对齐） | 已获裁决：AC-10 对 `desktop-archi-api` 明文零 diff；`desktop-archi-api-check:79` 属未被 delta 修改的 requirement，给它加第三个 MODIFIED 会越过 proposal §五的 8 个 requirement 面。两条同类，一处改一处不改反而制造新的不一致 |
| D10 | 明确不做 | ① 不补 11 条既有边、不建 `views/architecture/agent/`、不建 `components/file/file-viewer.tsx`、不建 `crates/tools/specta-zod-patched` 元素（只登记）；② 不动 `packages/desktop/src/**` 与 `src-tauri/**`（含 `scan` / `drift` / `api::check` / 生成器）；③ 不动 `01`–`05` 五个模型文件；④ 不动存活 spec（`desktop-change-chat` / `desktop-archi-{model,drift,canvas,file-tree,gate,contract-panel,api}` 零 diff）；⑤ 不给两个新元素写契约（写了就把 AC-3 的 +2 消掉，属独立议题）；⑥ 不新增 / 改写任何测试，不新增判据、诊断、配置键 | 逐条对应 proposal「不要修改」表与「不含」段；①–②保证「加边前后的差恰为 7」可对拍，⑤保证 `apiFindings` 桶构成可一句话归因 |
| D11 | 全量实测归属与报告槽位纪律（**AC-1 / AC-4 / AC-5 取数面修正**） | 分工按机械事实切开。**实现面**只核「与扫描模式无关、现成工具可读」的静态面：`06` / ADR / 两份 delta 的计数同值、`apiFindings` 41 = 12 + 29（事实校准 13）、`INDEX.toml` 44 条与 `api:export` 幂等、`archi_module` 结构化诊断为空、七条边的 `import` 证据行、模型内指向 `App.Backend.Shell` 的前端边 12 条。**移交面**（桌面端 static-check / test-execution / 人）：`archi_graph` 规模与 `diagnostics`、`archi_file_tree` 两处 `owners`、全量 `scan.unmatchedFiles` / `observations` / `errors` / `warnings` / `status`，并把 `archi_drift(root, "full")` 出参**原样手写**落 `reports/archi/full-<UTC 时间戳>.json`。**槽位纪律（硬）**：`full-` 前缀文件 MUST 满足 `scan.mode = "full"` 且 `scannedFiles > 0`；`incremental-` 前缀文件 MUST 满足 `mode = "incremental"`；runner 自动落的增量报告只落 `incremental-<UTC>.json`（或删除），MUST NOT 占 `full-` 槽位、MUST NOT 被当作任何全量基线；裸 `<UTC>.json` 形式的报告 MUST NOT 留在本 change 的 `reports/archi/` 内（无前缀即歧义源） | 事实校准 7 / 13：全量报告**无自动产出通道**（runner 恒增量且是唯一自动落盘点，`archi_drift` 零写盘）。把「全量落盘 + 全量槽位核对」写成实现面 checkbox，等于要求实现者凭空写 JSON 或拿增量报告顶替——两种都是留痕说谎，且会让 AC-1 的「全量」退化成口头限定。切开后每条 AC 都有具体承接者；前缀 + `scan.mode` 双条件把增量报告机械拦在槽位外 |

---

## 架构组件 (Architecture Components)

本变更**零新增组件**：两个既有数据文件的输入换值、一份生成物按新元素面重出、其余面零改动。

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 前端模型数据（**改**） | 两个共享组件文件的元素声明 + 七条 `import` 证据边 + 表头计数与构成注释 | `.dev-team/architecture/models/06-frontend.toml` | 解析面 `crates/core/archi/src/parse/toml.rs`、建图 `dto::graph_view`（**零改动**） | TOML |
| 粒度与口径裁定（**改**） | `§1` / `§3` / `§6` / `§9` 的计数复述换值、`§7` 补命令面边声明式口径与 11 → 12、`§8` **四条**缺口登记 | `.dev-team/architecture/decisions/0001-model-granularity.md` | 被 `archi_module("App").docs` 收录；`§2` 判据本体零改动 | markdown |
| 归属索引（**零改动 · 被消费**） | `OwnerIndex::build` / `owner_of`：唯一最长前缀、文件型根只覆盖自身、等长多命中即 `Ambiguous` | `crates/core/archi/src/scan/owner.rs` | `OwnerIndex` 的 `module_roots` 同时供文件树 | 纯函数 |
| 扫描与成边（**零改动 · 被消费**） | 只对**有归属**的文件抽 `import`；`off` 元素照常抽取但不进 `matched`；目标 `Owner::None` 丢边 | `crates/core/archi/src/scan/mod.rs`（`:148–176` 归属、`:195–260` 抽取成边） | `scan::ts` / `scan::rust` | 纯函数 + 读盘 |
| 声明兑现与报告（**零改动 · 被消费**） | `declared − 有证据边` → `observations[].unfulfilledDeclaration`；`off` 引入方的未声明边零条目 | `crates/core/archi/src/drift/mod.rs` | `scan`、`api::findings` | 纯函数 |
| 契约判据与索引装配（**零改动 · 被消费**） | `path` 非空元素各一条 `INDEX` 条目（`api/mod.rs:501`）；入口 `Resolved` + 无契约 ⇒ `apiMissing`（`api/check.rs`）；生成物落盘与比对单点（`api/render.rs` / `api/diff.rs` / `infra/archi/src/api_artifacts.rs`） | `crates/core/archi/src/api/`、`crates/infra/archi/src/api_artifacts.rs` | `api::paths` / `api::schema` | 纯函数 + 生成器 |
| 契约生成物（**改 · 生成**） | `INDEX.toml` 由 42 → 44 条（+2 条全空槽位），只经 `pnpm run api:export` 产出 | `.dev-team/architecture/api/INDEX.toml` | `archi-api-gen` CLI（`crates/tools/archi-api-gen`） | TOML（生成物） |
| spec 面（**已在案 · 零改动**） | 8 个 MODIFIED requirement 的事实源；存活 spec 由归档链 spec-sync 段合并 | `openspec/changes/frontend-component-element-coverage/specs/repo-archi-model/spec.md`、`…/desktop-archi-api-check/spec.md` | 归档链（`core/orchestration` 的 spec-sync 段） | markdown |
| 实测留痕（**新增 · 分相位**） | 实现面静态对照留痕一份（`model-sync-check.md`）+ 增量旁证一份；全量报告一份**由移交相位手写落盘** | `openspec/changes/frontend-component-element-coverage/reports/archi/{model-sync-check.md, incremental-<UTC>.json, full-<UTC>.json}` | `archi_drift(root, "full")` / `archi_graph` / `archi_file_tree`（IPC，零写盘；唯一的自动落盘点是门禁 runner 且恒增量）、`archi_module` / `archi_api`（实现面可读） | JSON + markdown |
| 版本面（**改**） | `version` 一次 patch | `packages/desktop/package.json`（`tauri.conf.json` 经引用跟随） | 既有 `"version": "../package.json"` | JSON |

### 数据流（改后，前端片段）

```
06-frontend.toml（+2 元素 / +7 边）
        │
        ├─ dto::graph_view ──> elements(53) / relations(120) ──> archi_graph / archi_module / archi_search
        │                                                     └─> api::index_entries ──> INDEX.toml（44 条）
        │
        └─ scan::OwnerIndex ──> owner_of("…/session-panel.tsx") = Element{ …Agent.SessionPanel, gate: Off }
                    │                     │
                    │                     └─> archi_file_tree：mark=elementRoot / owners=[FQN]   ← AC-2 观测面
                    │
                    ├─ 抽取 session-panel.tsx 的 import ⇒ 边 {→ AgentSessionView, → Ui}（③ 目标无归属，丢）
                    ├─ 抽取 tooltip.tsx 的 import      ⇒ 边 {→ Ui}
                    ├─ 两个文件仍入 unmatchedFiles（gate=Off 语义，与加元素前同一集合）
                    └─ 目标归属：Views.Changes / Views.Explores / barrel ⇒ 入边成边
                                      │
                                      ▼
        drift::check：declared(120) − 有证据边 ⇒ observations（+1：SessionPanel → App.Backend.Shell）
                      errors / warnings 仍空 ⇒ status = clean、passed = true（纳管子集仍为空）
                                      │
                                      ▼
        api::check：path 非空元素 44 ⇒ apiEntryMissing 12 + apiMissing 29 = 41（两新元素各一条）
```

> 图中 `scan::` / `drift::check` 两段是**全量模式**语义（增量模式 `scannedFiles = 0` ⇒ 该段零扫描读数、`status = noChanges`、`observations` 退化为「全部声明未兑现」），故其读数按 D11 归移交面；`dto` / `api::index_entries` / `api::check` 段与模式无关，实现面可核。

---

## 变更清单 (Change Inventory)

### 新增文件

| 路径 | 产出相位 | 说明 |
|---|---|---|
| `openspec/changes/frontend-component-element-coverage/reports/archi/model-sync-check.md` | **实现面** | 对照留痕：① 两文件归属的静态判据（`path` 唯一最长前缀、与既有文件型根零冲突）与「全量差集 / `owners` 读数移交」的指向；② `apiFindings` 按 `kind` 分桶的元素 FQN 清单（实现面可读，事实校准 13）；③ 七条边逐条 `import` 证据（含第 ③ 条的声明式标注）+ `INDEX.toml` 的 +2 条读数 + `archi_module` 邻接读数；末尾「邻接现值未同步」登记 |
| `openspec/changes/frontend-component-element-coverage/reports/archi/full-<UTC 时间戳>.json` | **移交面（实现面不产出、不勾选）** | 一次 `archi_drift(root, "full")` 的出参原样落盘（不裁剪、不改键），是 AC-1 / AC-4 / AC-5（D6 ①②③）与 AC-2 的实测载体。**双条件**：文件名前缀 `full-` + 文件内 `scan.mode = "full"` 且 `scannedFiles > 0`，缺任一条即不合格（D11） |
| `openspec/changes/frontend-component-element-coverage/reports/archi/incremental-<UTC 时间戳>.json` | 实现面（旁证，可省） | 门禁 runner 自动落的增量报告，**只作「本期未跑全量」的证据**：`scannedFiles = 0` ⇒ `unmatchedFiles` 恒空、`status = noChanges`、`observations` 为全部声明未兑现（本仓 120 条）。MUST NOT 作任何 AC 的基线，MUST NOT 顶 `full-` 槽位 |

> 三份留痕的字段口径即「数据模型 - 派生读数」表（含「取数面」列）；`full-` 一份的产出者、双条件与失败特征见 D11 与「验收标准回应」表的 AC-1 / AC-2 / AC-4 / AC-5 行。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `.dev-team/architecture/models/06-frontend.toml` | ① 表头 `:1` `（28 元素 / 64 关系）` → `（30 元素 / 71 关系）`；② 表头 `:2` `共享组件文件 10` → `12`（同句「页壳 1 + 视图 9 + 视图内子域 3 + … + 原语层 1，另有 4 个容器」其余逐字不动）；③ 在 `[…Agent.AgentSessionView"]` 段之后插入 `[…Agent.SessionPanel"]` 段（四字段，D1）；④ 在 `[…Standard.Select"]` 段之后插入 `[…Standard.Tooltip"]` 段；⑤ 在 `[[relation]]` 段内按 D2 的四处锚点插入 7 条边（各带一句中文 `desc`）。其余 28 个 `[element]`、64 条 `[[relation]]`、全部 `kind` / `gate` / `path` / `summary` 逐字不动 | AC-1 的 `06` 段计数与 AC-5 的七条边；`entry` / `api` / `metadata` / `docs` 零新增（D1） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | `§1`：`51 元素 / 113 关系` → `53 元素 / 120 关系`、`前端 64 条` → `前端 71 条`；`§3`：端点边 `6 条` / `2 条出边（… AgentInput / AgentSessionView）` → `7 条` / `3 条出边（… / SessionPanel）`；`§6`：`基线 51 个元素` → `53 个元素`；`§7`：`11 条命令面边` → `12 条`，并在「唯一的映射例外」之后补一句声明式约定（`src/types/**` 无归属 ⇒ 扫描丢边 ⇒ 落 `observations`、`off` 下永不进 `errors` / `warnings`、MUST NOT 用观测项为空验收）；`§8`：新增**四条**缺口登记（D4：⓪ `crates/tools/specta-zod-patched` 先于本变更存在 ① `components/file/file-viewer.tsx` ② `views/architecture/agent/` ③ 11 条未声明边 + 反向缺口括注）；`§9`：复算块换值（`06(30)`、`06(71)`、`53`、`120`、前端 `71`、恒等式 `71 = 64 + 7`）与末句 `58 / 6` → `64 / 7`。`§2` / `§4` / `§5` 逐字不动 | AC-1 的「三处同值」+ AC-7 的四条登记；D3 / D4 |
| `.dev-team/architecture/api/INDEX.toml` | 由 `pnpm run api:export` 重生成（**不手改**）：`[[entry]]` 42 → 44 条，新增 `App.Frontend.Components.Agent.SessionPanel`（位于 `AgentSessionView` 与 `ToolCards` 之间）与 `App.Frontend.Components.Standard.Tooltip`（位于 `Select` 与 `Components.Ui` 之间），两者六字段恒 `status = "absent"` / `types = 0` / `ops = 0` / `degraded = 0` / `schema = ""` / `generated = ""`；表头「生成器 archi-api 0.1.0 · 映射规则摘要 sha256:…」逐字不变（摘要取自映射规则资产，非元素面） | 事实校准 3 / D5（AC-6 修正）：生成物必须跟随元素面，否则 `api:check` 红；`.schema.json` / `.i.rs` / `.i.ts` 三类**零 diff**（两元素无契约 ⇒ `present = false` ⇒ 不产 `.i.*`、不产 schema） |
| `packages/desktop/package.json` | `"version"` `0.6.47` → `0.6.48`（只 +1 一次）；其余键与 `scripts` 段零改动 | AC-9 / D8 |
| `packages/desktop/src-tauri/tauri.conf.json` | **零内容改动**（只需确认仍是 `"version": "../package.json"` 引用形态、未被写成字面版本号） | proposal「实现文件」表的确认项；`git diff` 必须为空 |
| `openspec/changes/frontend-component-element-coverage/specs/{repo-archi-model,desktop-archi-api-check}/spec.md` | **零改动**（已在案，实现期只核对 8 个 MODIFIED requirement 与 AC-8 的取值面，不编辑） | 事实校准 9：存活 spec 由归档链合并；编辑 delta 会与归档合并重复 |

<!-- 公共函数 / API：本变更零代码逻辑改动——`scan::owner::{OwnerIndex::build, owner_of}`、
     `scan::scan`、`drift::check`、`api::{findings, index_entries, compare}`、
     `archi_graph` / `archi_module` / `archi_search` / `archi_drift` / `archi_file_tree`
     的签名与行为零变化，故本子表整段省略。 -->

<!-- 类型定义：`ArchiElement` / `ArchiDriftReport` / `ArchiFileEntry` / `IndexEntry` 形状
     零变化（本变更只换数据值：元素与关系条数、`owners` 的取值、`INDEX` 条目数），
     故省略。 -->

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.47` → **`0.6.48`** | D8：只 +1 一次；`tauri.conf.json` 经引用跟随 |
| `version` | `packages/desktop/src-tauri/tauri.conf.json` | **零改动** | `string` | `"../package.json"` | 实现期只核对形态 |
| `version`（crate） | `packages/desktop/src-tauri/Cargo.toml` | **零改动** | `string` | `0.1.0` | crate 版本线独立，MUST NOT 随 `package.json` |
| `elements`（模型数据，非配置键） | `.dev-team/architecture/models/06-frontend.toml` | 新增两段 | TOML 表 | 无（新增） | `kind = "component"` / `gate = "off"` / `path` 单值文件 / 一句中文 `summary`；`entry` / `api` / `docs` / `metadata` 一律不写（D1） |
| `relations`（模型数据，非配置键） | 同上 | 新增七条 | TOML 表数组 | 无（新增） | 各带一句中文 `desc`（用途，不写证据清单，D2） |

> 配置项零新增：`.dev-team/config.json`、`.dev-team/architecture/config.json`（`exclude: ["openspec/**"]`）、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`capabilities/default.json`、`package.json` 的 `scripts` 段全部零 diff。

---

## 数据模型 (Data Model)

### 元素（唯一新增面，2 条）

| FQN | 文件 / 落位 | `kind` | `gate` | `path` | `summary` |
|---|---|---|---|---|---|
| `App.Frontend.Components.Agent.SessionPanel` | `06-frontend.toml`，`AgentSessionView` 段之后（字典序位） | `component` | `off` | `["./packages/desktop/src/components/agent/session-panel.tsx"]` | 会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条，两处会话面共用） |
| `App.Frontend.Components.Standard.Tooltip` | 同上，`Select` 段之后、`Ui` 段之前 | `component` | `off` | `["./packages/desktop/src/components/standard/tooltip.tsx"]` | 文本气泡（原语层 Tooltip 的业务封装） |

- 派生字段取缺省：`entry` 为空（单文件根 = 根文件自身，约定默认命中 ⇒ `entry_state = Resolved`）、`api` 为空（取 `declared` ⇒ 各有契约文件缺失一条 `apiMissing`）、`docs` / `metadata` 为空。
- 层级：两者的祖先链 `App` → `App.Frontend` → `App.Frontend.Components` → `App.Frontend.Components.{Agent,Standard}` 全部在案 ⇒ `orphanHierarchy` 零命中。

### 关系（+7 条，逐条证据与 `desc`）

| # | `from` → `to` | 证据（文件:行） | `desc`（中文一句用途） | 性质 |
|---|---|---|---|---|
| ① | `App.Frontend.Components.Agent.SessionPanel` → `…Agent.AgentSessionView` | `components/agent/session-panel.tsx:9` | 面板正文复用整段会话视图 | 叶 → 叶 |
| ② | 同上 → `App.Frontend.Components.Ui` | `session-panel.tsx:4–5`（两处 ui import 按目标元素去重） | 头区按钮与可拖拽分栏使用原语层 | 叶 → 叶 |
| ③ | 同上 → `App.Backend.Shell` | `session-panel.tsx:7`（`import type … @/types/generated/bindings`） | 经生成的命令面类型取会话清单 | 命令面边（声明式约定，恒落 `observations`） |
| ④ | `App.Frontend.Components.Agent` → `…Agent.SessionPanel` | `components/agent/index.tsx:5–6`（两行按目标元素去重） | barrel 重导出面板组件与其状态类型 | barrel 端点边 |
| ⑤ | `App.Frontend.Components.Standard.Tooltip` → `App.Frontend.Components.Ui` | `components/standard/tooltip.tsx:1` | 以原语层气泡组合文本提示 | 叶 → 叶 |
| ⑥ | `App.Frontend.Views.Changes` → `…Standard.Tooltip` | `views/changes/change-list-view.tsx:5` | 列表工具栏的图标按钮以气泡给出提示 | 叶 → 叶 |
| ⑦ | `App.Frontend.Views.Explores` → `…Standard.Tooltip` | `views/explores/explore-list-view.tsx:5` | 列表工具栏的图标按钮以气泡给出提示 | 叶 → 叶 |

- 自环零条（`from ≠ to` 全部成立）；三个无 `path` 的容器（`Views` / `Components` / `Standard`）不出现任何端点；端点边恰 7 条且全部挂在持 barrel 的 `Components.Agent`（3 出 + 4 入）。
- 新增边的目标集合与两个文件的实际 `import` 集合**逐条相等**：`session-panel.tsx` 的 `@/hooks/use-agent-session` 属封闭排除面（零边）、`tooltip.tsx` 只有一个 import。

### 派生读数（非持久化；落 `reports/archi/` 留痕）

| 读数 | 期望值 | 构成 / 判据 | 取数面（D11） |
|---|---|---|---|
| 三处计数同值 | `53 / 120`、`06` 段 `30 / 71`、`71 = 64 + 7`、源 24 | D3 的算术闭合；`06` 表头、delta 计数表、ADR `§9` 逐字相等 | **实现面**（`grep -c '^\[element\.'` / `^\[\[relation\]\]` + 逐处读文） |
| `archi_graph(root)` 规模 | **53 元素 / 120 关系**、`diagnostics` 空、`warnings` 空 | 与三处计数同值；模型解析面零结构错 | **移交面**（`archi_graph` 只读命令；实现面以 `archi_module` 逐元素邻接 + 结构化诊断为空作等价证据） |
| `archi_file_tree` 两处条目 | `mark = "elementRoot"`、`owners` 各恰 1 个 FQN（反向：`ambiguous` / `owners ≥ 2` 即不合格） | D7 / AC-2 | **移交面**（`archi_file_tree` 零写盘、只读命令） |
| 全量 `scan.unmatchedFiles` | 与加元素前基线**集合相等**（不增不减） | D6 ①（`gate` 全 `off` ⇒ 归属与否都进该槽位） | **移交面**（全量扫描） |
| 全量 `observations` | 与基线相比**恰 +1 条**，新增项 = `Components.Agent.SessionPanel → App.Backend.Shell`；若 +3 条 ⇒ ①② 未兑现 ⇒ 归属失败 | D6 ② / AC-5 | **移交面**（`observations` 随 `mode` 变，事实校准 13） |
| 模型内命令面边 | 指向 `App.Backend.Shell` 的前端声明边 **11 → 12** | grep `to = "App.Backend.Shell"` 逐条点名 | **实现面**（静态表） |
| `errors` / `warnings` / `status` / `passed` | 前后皆空；`status = clean`、门禁 `passed = true` | `gate` 全 `off`，纳管子集仍只有 `ArchiRuntime` 且判据 0 条 | **移交面**（增量报告恒 `noChanges`，事实校准 13；MUST NOT 用增量读数） |
| `apiFindings` | **41** = `apiEntryMissing` **12** + `apiMissing` **29**；其余七条 0；分母（`path` 非空）**44** | AC-3；+2 全落 ②（两新元素有入口、无契约） | **实现面**（`apiFindings` 与 `mode` 无关，事实校准 13；报告槽位或 `archi_api check`） |
| `api::compare` | `mismatched` / `orphaned` / `missing` 三列皆空（`INDEX.toml` 已重生成） | D5 / AC-6 修正 | **实现面**（`api:export` 二跑零写入 = 幂等；`api:check` 归静态门禁相位） |
| `INDEX.toml` | 44 条 `[[entry]]`，新增两条六字段全空；差分恰 +2 条 | D5 | **实现面** |
| 报告槽位纪律 | `full-*.json` 内 `scan.mode = "full"` 且 `scannedFiles > 0`；`incremental-*.json` 内 `mode = "incremental"`；无裸 `<UTC>.json` | D11 | **实现面 + 移交面**（实现面负责本 change 目录内无裸名、增量报告改名；`full-` 一份由移交面产出） |

### 持久化面变化

| 对象 | 变化 |
|---|---|
| `.dev-team/architecture/models/06-frontend.toml` | +2 个 `[element]` 段（各 5 行：表头 + 4 字段）、+7 条 `[[relation]]`（各 3 行）、表头 2 行改数 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | `§1` / `§3` / `§6` / `§7` / `§8` / `§9` 六节改值 + 四条登记 + 两句口径；`§2` / `§4` / `§5` 逐字不动 |
| `.dev-team/architecture/api/INDEX.toml` | 42 → 44 条 `[[entry]]`（生成物，`pnpm run api:export` 产出） |
| `.dev-team/architecture/api/**`（其余） | **零变化**：`*.schema.json` / `*.i.rs` / `*.i.ts` 三类零 diff（两元素无契约） |
| `.dev-team/architecture/models/{01,02,03,04,05}-*.toml` | **零变化** |
| `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}` | **零变化**（`ArchiElement` / `ArchiDriftReport` 形状不变，模型数据变化不触发再生成） |
| `packages/desktop/package.json` | `version` 一行 |

---

<!-- 路由 / API 设计：零新增 IPC / HTTP 端点 / CLI 子命令。`archi_graph` / `archi_module` /
     `archi_search` / `archi_drift` / `archi_file_tree` / `archi_api_check` 的入出参形态与
     auth 面零变化（本变更只换模型数据与生成物内容），故本段省略。 -->

## 依赖 (Dependencies)

运行时依赖：**零新增**。本变更不改任何代码，`Cargo.toml` / `Cargo.lock` / `package.json` 的 `dependencies` / `devDependencies` 段零 diff。

构建 / 测试依赖：**零新增**。

工具面（实现与核对用，均为既有能力）：

| 能力 | 用途 |
|---|---|
| `grep -c '^\[element\.'` / `grep -c '^\[\[relation\]\]'` / `grep -c '^entry = '` / `grep -c '^api = '` | D1 / D3 的形态与计数机械自查（`06` ⇒ 30 / 71；`entry` 合计仍 3；`api` 仍 2 且均 `none`） |
| `grep -n '^from = \|^to = '`（成对读） | 七条新边的落位与组内字典序自查；端点边恰 7 条的机械计数 |
| `git status --porcelain` / `git diff --stat` | AC-6（修正后白名单）与「不要修改」面的零 diff 核对 |
| `pnpm run api:export` | 生成 `INDEX.toml`（+2 条全空槽位）；其余三类生成物零 diff |
| `pnpm run api:check` | AC-6：`missing` / `mismatched` / `orphaned` 三列皆空 |
| `archi_graph` / `archi_file_tree` / `archi_drift(root, "full")` | AC-1 / AC-2 / AC-4 / AC-5 的**移交面**实测与留痕（门禁步恒增量且是唯一自动落盘点，命令面零写盘，见事实校准 7 / D11）——实现面不持该通道，MUST NOT 用增量报告顶替 |
| `archi_module("App")` | AC-7 的 ADR 条目仍在 `docs`（`source = "text"`） |

---

## 验收标准回应

| AC | 设计落点 | 归属阶段（tasks） |
|---|---|---|
| **AC-1** 计数三处同值且图正常态 | D3（三处同值 + `§1` / `§3` / `§6` 同批）、事实校准 2（算术闭合）、D1/D2（落位即诊断零命中的前提：祖先齐全、`path` 合法）、**D11（取数面切分：计数同值归实现面，`archi_graph` 规模与 `diagnostics` 归移交面）** | 一 / 二（实现面静态）+ 移交清单（图本体读数） |
| **AC-2** 归属可复算且不 `Ambiguous` | D7（`archi_file_tree` 的 `mark = elementRoot` + `owners` 恰 1）；事实核实表「归属判据」行（与既有文件型前缀零冲突）；反向：`mark = ambiguous` 即不合格 | 一（静态零冲突核对）+ **移交清单**（`owners` 读数） |
| **AC-3** 判据面 +2 且不抬黄灯 | D1（`api` 不写 ⇒ 取 `declared`）、数据模型「派生读数」（41 = 12 + 29、分母 44）、D5（`INDEX` 44 条与判据面同源）、D10⑤（不写契约 ⇒ 不消掉 +2）、**事实校准 13（`apiFindings` 与 `mode` 无关 ⇒ 实现面可读）** | 一 / 三 / 五（实现面读数即可，不需全量） |
| **AC-4** 扫描面无新增未覆盖 | **判据修正为 D6 三条并列**（`unmatchedFiles` 集合相等 / `observations` 恰 +1 / `errors` + `warnings` 空）；事实校准 4 给出原判据不可达的机械证据（`scan/mod.rs:148–176` + 归档报告实测）；**D11：三条全落全量报告 ⇒ 整条归移交面** | **移交清单**（设计只钉判据与失败特征） |
| **AC-5** 七条边逐条可核 | D2（落位与 `desc`）、数据模型「关系」表（七条证据行号）；③ 的 `observations` 形态与「11 → 12」由 D6 ② 一并读出 | 一（六条 `import` 证据行 + 模型内 12 条命令面边，实现面）+ **移交清单**（`observations` 差集） |
| **AC-6** 零代码扩散 | **白名单修正为 D5**（+`api/INDEX.toml`，且该文件 diff 恰 +2 条全空 entry）；`packages/desktop/src/**` 与 `src-tauri/**` 零 diff；`api:check` 三列皆空；`bindings:export` 幂等 | 三 / 四 / 五 |
| **AC-7** 缺口登记到位且不越界（**修正为四条登记**） | D4（四条：⓪ vendored crate + ① `file-viewer` + ② `views/architecture/agent/` + ③ 11 条未声明边含反向缺口括注）；反向判据：`file-viewer.tsx` / `views/architecture/agent/**` / `crates/tools/specta-zod-patched/**` 仍不在任何元素 `path` 内、`06` 元素数仍 30、`05` 元素数仍 3（`specta-zod-patched` 不入模型）、模型内除七条新边外零新增边（D10①） | 二 / 五 |
| **AC-8** spec 同步且无现值冲突 | 事实校准 9（delta 已在案、存活 spec 由归档链合并、实现面零编辑）、事实校准 10 + D9（两处「42 个」按裁决登记不改）；自查：`repo-archi-model` delta 内旧数只作退场口径、新数在计数表与 `§9` 同值 | 二（`§9` 侧）/ 五（grep 核对） |
| **AC-9** 版本纪律 | D8（`0.6.48` 只 +1；`tauri.conf.json` 引用形态；`Cargo.toml` 零改动） | 四 / 五 |
| **AC-10** 相关能力零漂移 | D9 / D10④（存活 spec 全部零 diff；`desktop-archi-api` 的 42 / 30 / 12 只登记不改） | 五 |

> **不列为 AC 的流程项**（沿 proposal 末尾声明）：全量 / 增量语义、报告槽位形状、十值判据词汇、生成器三子命令、`status` 推导序各有既有 spec 落点且本变更零改动，故本设计不另立验收项。
> **测试面**：零新增 / 零改写（事实校准 11）。`api:check` / `server:check` / `client:check` 由桌面端 static-check / test-execution 承接，实现相位不重复执行机械步骤。**全量漂移读数**（`archi_graph` 规模与 `diagnostics`、`archi_file_tree` 两处 `owners`、`unmatchedFiles` / `observations` / `errors` / `warnings` / `status`）按 D11 全部移交同一相位，且 `full-<UTC>.json` 的落盘只能由持 IPC 的相位完成；实现面只核与扫描模式无关的静态面（计数同值 / `apiFindings` 分桶 / `INDEX` 条目 / `import` 证据行 / 报告槽位卫生）。**增量报告 MUST NOT 被读成基线**（事实校准 13）。

---

## 待决问题 (Open Questions)

1. **两处「42 个」现值的承接方与时点**：本变更按裁决不改（D9 / 事实校准 10），登记后由谁对齐未定。两个候选：① 变更 B（12 个 barrel 入口化）会同时改 `apiEntryMissing` 的桶构成与前端单文件根数，是最自然的承接点；② 独立开一个「spec 数字现值同步」小 change。不设第三条路（改本变更的白名单去碰 `desktop-archi-api` 已被 AC-10 排除）。
2. **`INDEX.toml` 的 diff 是否被评估面接受**：D5 已按裁决落地（重生成），但 AC-6 的字面白名单不放行该文件。若评估相位坚持字面口径，正确处置是**改 AC 而非改数据**（回退生成会让 `api:check` 与仓库 `check` 链同时红，与「生成物勿手改 + 零 diff 门」纪律冲突）——本设计明写这条退路，避免实现期以「删元素」换绿。
3. **`full-<UTC>.json` 的产出时点与责任人在本设计内不指定**：承接面已确定为「持 IPC 的相位」（桌面端 static-check / test-execution / 人，D11），但具体由谁在何时取数不在实现面裁定；本设计只钉**判据、文件名双条件与失败特征**。若全量读数与 D3 / 数据模型「派生读数」不符，MUST 先查证据面（是否有别的元素或边进出扫描面），MUST NOT 单改模型数字去凑预估（proposal R-1）。

> **已裁决（不再列为待决）**：原「全量实测的执行归属」——门禁步恒增量且是唯一自动落盘点、`archi_drift` 零写盘（事实校准 7），故实现面无该通道；归属切分与槽位纪律写死为 **D11**，`full-` / `incremental-` 前缀与 `scan.mode` 双条件机械拦挡增量报告顶替。
