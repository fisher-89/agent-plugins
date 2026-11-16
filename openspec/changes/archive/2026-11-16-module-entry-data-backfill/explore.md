# 候选变更 · entry 数据回填与基线换段

workflow_type: requirement

> 来源：`2026-11-16-module-entry-field-spec`（变更 A，已归档）的遗留 —— 归档 proposal §六「后续变更（宿主重建后）」
> 与 R-10 只写了动作，**未命名承接变更**；spec 两处也把基线留在「本段」。
> 主线：`openspec/explores/module-entry-declaration.md`。
>
> 本单元 = **纯数据 + 基线数字**，零代码逻辑改动：三处 `entry` 归位、两处 spec 换段、三条收益验收。

## 0 触发条件与前置

| 项 | 内容 |
|---|---|
| 硬前置 | 含 `entry` 字段（A 的代码面）的**发布版**已发布、目标用户已升级到该版本 |
| 另一前置 | 执行验收的那台机器上，桌面端宿主二进制**必须由本工作树重建**（门禁跑在宿主进程内，用宿主内嵌的 archi crate 解析模型） |
| 现状 | A 的代码面已在 master（`8b03cdec2fc2`）；模型 `grep '^entry' models/` **零命中**；`05` 两行 `api = "none"` 已落 |
| 依赖 | 无（不依赖 B/C）；但**建议先于 B 落地**（见 §5） |
| 独立可归档 | 是 |

## 1 基线三段：现在停在第 2 段

| 段 | ① `apiEntryMissing` | ② `apiMissing` | 总 | 状态 |
|---|---|---|---|---|
| 旧（A 之前） | 28 | 13 | 41 | 历史 |
| **本段（当前 master）** | **15** | **26** | 41 | ← 停在这里 |
| 目标段（本候选） | **12** | **27** | **39** | 本变更为它而开 |

```text
① 15 = 12 个前端目录根（缺 index.ts）+ 壳 + 2 个工具 crate
        └─ 补三处 entry 后 → 12（壳与两工具 crate 退役出 ①）
② 26 = 13 个 crate + App.Frontend.Shell + 12 个前端单文件根
        └─ 壳转入 ②（api = declared 缺契约）→ 27
总   41 → 39（净 −2：两工具 crate 的 entry 命中 + api = none ⇒ 零判据，退出判据面）
```

实测对照留痕：`openspec/changes/archive/2026-11-16-module-entry-field-spec/reports/api-baseline/measured.json`
（该文件的第一台状态即 12 / 27 / 39，是「分批前」的实测，不是预测）。

## 2 要改的位置（逐处）

| # | 位置 | 改动 | 现状锚点 |
|---|---|---|---|
| 1 | `.dev-team/architecture/models/04-backend-shell.toml` | `App.Backend.Shell` 补 `entry = "./packages/desktop/src-tauri/src/lib.rs"` | 该文件当前零 diff（A 刻意未动） |
| 2 | `.dev-team/architecture/models/05-backend-tools.toml` | 两工具 crate 各补 `entry = "./packages/desktop/src-tauri/crates/tools/<crate>/src/main.rs"`（`api = "none"` **保留不动**） | 两行 `api = "none"` 在案 |
| 3 | `openspec/specs/repo-archi-model/spec.md:30` | 「本变更落地时显式 `entry` SHALL 为 0 处」→「恰三处」；删「随宿主重建后的后续变更归位」的过渡语；`api = none` 段的「后三者的显式 `entry` 归后续变更」同步为「已在案」 | 现文写 0 处 |
| 4 | `openspec/specs/repo-archi-model/spec.md:54`（scenario） | 「`entry` 零行」→「恰三行，各为单值字符串」 | 现文写零行 |
| 5 | `openspec/specs/repo-archi-model/spec.md:69`（scenario） | 「壳与两个工具 crate 各产一条 `apiEntryMissing`」→ 壳转 `apiMissing`、两工具 crate 零条目 | 现文写三者各一条 |
| 6 | `openspec/specs/desktop-archi-api-check/spec.md:185`（基线表） | `15 / 26` → `12 / 27`，重写分桶说明（①剩下的 12 = 前端目录根；壳移入 ②；两工具 crate 零条目） | 现文写 15 / 26 |
| 7 | `openspec/specs/desktop-archi-api-check/spec.md:189`（「数据分两批，基线分两段」） | 该条整段退场或改写为「基线单段 = 12 / 27（总 39）」；`MUST NOT 在本变更内按第二段数字验收` 随之删除 | 现文是两段制 |
| 8 | `packages/desktop/package.json` | `version` +1 patch（`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） | — |

**零生成物 diff**：三处 `entry` 都不改变任何生成物。`INDEX.toml` 的 `generated` 列只在「契约在案」时非空
（`api/mod.rs` 的 `generated_rel` 带 `.filter(|_| present)`），而壳与两工具 crate 没有契约 ⇒ `api:check` 应零差异。
本变更 MUST NOT 出现 `.i.rs` / `.i.ts` / `.schema.json` / `INDEX.toml` 的任何改动；出现即说明 `entry` 落位写错。

## 3 为什么必须等发布版升级

门禁不在工作树里跑，而在**宿主进程内**用宿主内嵌的 archi crate 解析模型：

```text
发布版（旧 archi crate）读到 entry
  → deny_unknown_fields ⇒ unknownField
  → 该文件解析失败 ⇒ 元素「未声明」 ⇒ 依赖它的文件连带 unknownReference
  → 实测 13 条结构错 ⇒ status = errors（门禁从可判定掉到全红）
```

所以本变更的**数据提交**必须晚于发布；代码提交无此限制（A 已按此拆分）。
另一面：`archi_drift` 的验收**必须在重建宿主的机器上**跑，否则跑的是旧判定逻辑（旧逻辑没有 `entry_state`），数字不可信。

## 4 验收判据

| # | 判据 | 断言 |
|---|---|---|
| AC-1 | 三处 `entry` 在案 | `grep -c '^entry = ' .dev-team/architecture/models/*.toml` 合计 **3**（`04` 1 行、`05` 2 行），其余四份 0 行 |
| AC-2 | 模型零结构错 | 重建宿主后 `archi_drift` 的 `diagnostics` 为空；`archi_graph` 仍 **51 元素 / 113 关系**；元素计数与关系计数逐值不变 |
| AC-3 | 分桶换段 | `apiFindings` 总 **39**；① = **12**（仅 12 个前端目录根）、② = **27**；其余八类各 0 |
| AC-4 | 壳的收益 | `element == "App.Backend.Shell"` 且 `kind == "apiEntryMissing"` = **0**；同趟 `apiMissing` **在案**（契约债 ≠ 入口债，不许混谈） |
| AC-5 | 两工具 crate 的收益 | 两 FQN 在 `apiFindings` 中**零条目**（`entry` 命中 + `api = none` 两条同时在场） |
| AC-6 | 生成物零 diff | `pnpm api:check` 零差异；`INDEX.toml` 与四类生成物 `git diff` 为空 |
| AC-7 | spec 换段 | `grep -c '15 / 26' specs/` 零命中；`grep '基线 = 12 / 27'` 命中；`repo-archi-model` 的「0 处」零命中、「恰三处」在案 |
| AC-8 | 版本纪律 | `package.json` +1 patch；`tauri.conf.json` 跟随；`Cargo.toml` 版本行不变 |

**口径纪律**：AC-3 的数字若与预期不符，MUST 先查「是否有别的元素进出判据面」（B 若已落地，① 会再减），
MUST NOT 直接改判据或放宽门槛来对上数字。

## 5 与 B / C 的顺序

```text
本候选（数据） ──► B（12 个 barrel）
   ① 12           ① 12 − 12 = 0
   ② 27           ② 27 + 12 = 39（12 个视图/组件元素转为「有门无契约」）
```

**建议本候选先于 B**：否则 B 落地时基线要同时叠加两个未知量（barrel 让 ① 归零、本候选让壳与两工具退出 ①），
报告数无法单调对拍。顺序反过来也可（数据与 barrel 互不影响），只是 B 的验收基线要写「12 − 12」而不是「0」。

C（TS 取数面）与本候选无交集，可任意先后。

## 6 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **在未升级的宿主上跑验收**：旧宿主没有 `entry_state`，数字仍是旧的 15/26，或直接 13 条结构错 | 验收前先确认宿主版本；`archi_graph` 能读 `entry` 字段（非 `unknownField`）即证明宿主够新 |
| R-2 | **只写 `entry` 不保留 `api = none`** | 两工具 crate 会从「零判据」变成「`apiMissing` 1 条」，AC-5 失败；两条必须同时在场 |
| R-3 | **多写第四处 `entry`** | 单文件根与 14 个 crate 由约定默认覆盖，写进去即违 `repo-archi-model`「仅在约定默认指不到时声明」；AC-1 的「合计 3」是对它的机械拦挡 |
| R-4 | **把 spec 的「两段制」只改一半** | 基线表（§2 #6）与「数据分两批」条（#7）是同一事实的两处，必须同批改；只改一处会留下自相矛盾的 spec |
| R-5 | **追溯补 A 的版本号** | A 的实现提交 `8b03cdec2fc2` 未 bump（`0.6.44` → `0.6.44`），那次 +1 被后续提交占用（`1faeb4bdcfd9` → `0.6.45`）。本变更落地时**只 +1 一次**，MUST NOT 为「补 A 的账」连加两档——版本单调递增，追溯会造出假历史 |

## 7 未决

1. 是否需要同时给 `App.Backend.Shell` 补契约（把 ② 的 +1 也消掉）？**建议不做**：那是契约债的独立议题，混进来会让本候选的「纯数据换段」失去可对拍性。
2. 三处 `entry` 是否要写 `./` 前缀？`repo-archi-model` 要求与 `path` 同口径（带 `./`），而 `normalize_root` 会剥除——按 spec 写 `./` 前缀，验收不受影响。
3. 本候选是否顺手把 `desktop-archi-api-check` 的基线表改成「表格 + 元素清单」双形态（现在清单散在散文里）？属可读性优化，可以不做。
