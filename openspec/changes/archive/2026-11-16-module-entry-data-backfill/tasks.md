# 任务: module-entry-data-backfill

> 覆盖 proposal「变更范围 - 实现文件」**4 行全量**（`models/04` 一行 `entry`、`models/05` 两行 `entry`、`package.json` 一次 patch、`tauri.conf.json` 零内容改动确认）+ 设计 D9 的 **delta 两处机械口径收敛**（AC-7）。`openspec/specs/**` 与生成物 / bindings **不在实现面**（前者由归档链 spec-sync 段合并，后者结构上零 diff）。
>
> 依赖序：**阶段一**（三行数据）解锁阶段二（零 diff 面变成可核对的事实）与阶段五（分桶实测有输入）；**阶段三**（delta 口径）与**阶段四**（版本行）互不依赖、也不依赖阶段一，可任意先后；**阶段五**收官。
>
> 三条不可动摇的机械锚点：① `grep -c '^entry = ' .dev-team/architecture/models/*.toml` 合计 **3**（`04` 1、`05` 2，其余四份 0），无第四处；② `.dev-team/architecture/models/05-backend-tools.toml` 的两行 `api = "none"` 逐字保留（删任一行即两工具 crate 掉回 `apiMissing`）；③ `.dev-team/architecture/api/**` 与 `packages/desktop/src/types/generated/**` 零 diff（出现 diff 只可能是 `entry` 落位写错，回退数据而非改生成器）。
>
> 自查 cwd：**仓库根**（模型数据 / delta / `git` 面）；`packages/desktop`（`package.json` / `tauri.conf.json`）。静态门禁（`api:check` / `server:check` / `client:check`）与宿主内实测由桌面端 static-check / test-execution 承接，**不在本相位执行**。

## 阶段一：模型数据三处 `entry`（`04` / `05`）

- [x] 修改 `.dev-team/architecture/models/04-backend-shell.toml`：在 `[element."App.Backend.Shell"]` 块的 `path = ["./packages/desktop/src-tauri/src/"]` 行**之后**插入一行 `entry = "./packages/desktop/src-tauri/src/lib.rs"`；`kind` / `gate` / `path` / `summary`、13 条 `[[relation]]` 与表头注释「1 元素 / 13 关系」逐字不动
- [x] 修改 `.dev-team/architecture/models/05-backend-tools.toml`：`[element."App.Backend.Tools.ExportBindings"]` 块的 `path` 行之后插入 `entry = "./packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs"`；该块原有的 `api = "none"` 位置与内容原样保留
- [x] 同文件：`[element."App.Backend.Tools.ArchiApiGen"]` 块的 `path` 行之后插入 `entry = "./packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs"`；`api = "none"` 原样保留；`Tools` 容器元素、3 条 `[[relation]]` 与表头注释「3 元素 / 3 关系」逐字不动
- [x] 自查 阶段一（值形态与落位）：`grep -c '^entry = ' .dev-team/architecture/models/*.toml` ⇒ `04` **1**、`05` **2**、`01` / `02` / `03` / `06` **0**（合计 3）；三行均为单值字符串、带 `./` 前缀、带扩展名（非数组、非空串、非绝对路径）；三值逐字等于 proposal §一（壳 …`/src/lib.rs`；两个工具 crate …`/src/main.rs`）；且各自落在其元素 `path` 归一后的目录之内（`git diff` 内无 `path` 行改动）
- [x] 自查 阶段一（计数与留白不变）：`grep -c '^api = ' .dev-team/architecture/models/*.toml` ⇒ `05` 恰 **2** 且两行均 `"none"`、其余五份 0；`grep -c '^\[element\.'` ⇒ `3 / 8 / 8 / 1 / 3 / 28`；`grep -c '^\[\[relation\]\]'` ⇒ `0 / 9 / 24 / 13 / 3 / 64`（合计 51 / 113）；`git diff --stat .dev-team/architecture/models/` 只含 `04-backend-shell.toml`（+1 行）与 `05-backend-tools.toml`（+2 行）

## 阶段二：生成物与索引零 diff（结构必然的核对，不跑再生成）

- [x] 核对 `.dev-team/architecture/api/INDEX.toml` 的三处条目逐字不变（以 `fqn` 块为锚，不以行号为锚）：`App.Backend.Shell`、`App.Backend.Tools.ExportBindings`、`App.Backend.Tools.ArchiApiGen` 三者仍为 `status = "absent"` / `generated = ""`（`present` 闸：无契约 ⇒ 无生成物路径），`[[entry]]` 条数仍 **42**
- [x] 核对零生成物：`find packages/desktop/src-tauri -name "*.i.rs" -not -path "*/target/*"` 仍只命中 `crates/infra/archi/src/lib.i.rs` 一份、`.i.ts` 零份、`.dev-team/architecture/api/**/*.schema.json` 仍只一份；**不执行** `pnpm run api:export`（本变更的期望是零产物，跑再生成只会掩盖落位错误）
- [x] 自查 阶段二：`git status --porcelain .dev-team/architecture/api` **零输出**；`git diff --stat packages/desktop/src/types/generated/` **零输出**（`ArchiElement.entry` 已存在，模型数据变化不触发再生成）

## 阶段三：delta 两处机械口径收敛（AC-7）

- [x] 修改 `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api/spec.md` 的 `## Purpose` 段第 1、2 条：第 1 条「（原为 27 / 15 / 0，并注「届时 30 / 12」）」改为「（原为 27 / 15 / 0，另注预告口径 30 / 12）」；第 2 条「原文把 30 / 12 写成「届时」（数据归后续变更），本 delta 把它改为已成立口径」改为「原文把 30 / 12 写成预告口径，本 delta 把它改为已成立口径」；`## MODIFIED Requirements` 与 `## Module Contract` 逐字不动（`Resolved` **30** / `Absent` **12** / `Invalid` 0 不删不改）
- [x] 修改 `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api-check/spec.md` 的「基线表」条目：在「`apiUndeclared` 不适用（`off`）」之后补一句「——两者即字面的 **12 / 27**、总 **39**」；分桶构成（12 的三项 / 27 的四项）、三条切分口径、基线单段条、五个 scenario 与 Module Contract 逐字不动
- [x] 自查 阶段三（AC-7 逐条 grep，cwd = `openspec/changes/module-entry-data-backfill/specs`）：① `grep -rn '15 / 26' .` 的每一命中都落在 `## MODIFIED Requirements` 之前，或同句含「旧」/「退场」/「变为」；② `grep -rn '26 条' .` **零命中**；③ `grep -rn '12 / 27' desktop-archi-api-check/spec.md` 命中「基线表」与「基线单段」**两处**；④ `grep -rn '显式 `entry` SHALL 为 0 处' repo-archi-model/spec.md` **零命中**、`grep -rn '归后续变更' .` **零命中**、该 delta 内「恰三处」在案且字段纪律 scenario 写「恰三行」；⑤ `grep -rn '届时' desktop-archi-api/spec.md` **零命中**，且该 delta 的「本仓入口面实测命中」scenario 写 `Resolved` **30** / `Absent` **12**；⑥ `grep -rn '本变更模型数据零处' desktop-archi-model/spec.md` 的命中只出现在 `## Purpose` 与 `## Module Contract` 的历史叙述行，**需求正文内的示例 TOML 注释块里零命中**（示例注释已是「三处已在案」的现口径）
- [x] 自查 阶段三（不改判据）：`git diff` 内四份 delta 的改动只落在上述两处措辞与一句补语上；新增文本内 MUST NOT 出现任何新的 MUST / SHALL 条款——`entry` 四条判据、三态解析链、十值判据词汇、纳管口径四条表述在 delta 内逐字未变

## 阶段四：版本纪律

- [x] 修改 `packages/desktop/package.json`：`"version"` 由 `0.6.46` 改为 `0.6.47`（相对落地时 `HEAD` 现值**只 +1 一次**，不追溯补变更 A 的那一次 bump）；其余键与 `scripts` 段零改动
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json`：仍是 `"version": "../package.json"` 引用形态、未被写成字面版本号（**零内容改动**，`git diff` 为空）
- [x] 核对 `packages/desktop/src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变（MUST NOT 随 `package.json`）
- [x] 自查 阶段四：`grep -n '"version"' packages/desktop/package.json` ⇒ `0.6.47`；`git diff --stat packages/desktop/src-tauri/Cargo.toml packages/desktop/src-tauri/tauri.conf.json` **零输出**

## 阶段五：工作树核对与移交

- [x] 核对变更清单覆盖面：`git status --porcelain` 只含「变更清单」的文件集——`.dev-team/architecture/models/{04-backend-shell,05-backend-tools}.toml`、`openspec/changes/module-entry-data-backfill/specs/{desktop-archi-api,desktop-archi-api-check}/spec.md`、`packages/desktop/package.json`，加本 change 目录的 `design.md` / `tasks.md`；**零** 测试文件改动、**零** `openspec/specs/**` 改动、**零** `.dev-team/architecture/api/**` 改动
- [x] 核对「不要修改」面零 diff：`git diff --stat .dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,06-frontend}.toml` 为空；`git diff --stat packages/desktop/src-tauri/crates packages/desktop/src-tauri/src packages/desktop/src packages/desktop/src/types/generated` 为空；`git diff --stat AGENT.md CLAUDE.md packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md .dev-team/architecture/decisions/0001-model-granularity.md` 为空
- [x] 核对入口面终态：`grep -rn '^entry = ' .dev-team/architecture/models/` 恰 3 行且三值逐字等于 proposal §一；`grep -rn '^api = "none"' .dev-team/architecture/models/` 恰 2 行（均在 `05`）；`grep -rn 'entry' .dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,06-frontend}.toml` 零命中
- [ ] 移交说明（不勾为任务）：**宿主须由本工作树重建**——先跑一次 `archi_graph(root)` 确认能读出 `entry`（而非 `unknownField`）且 `diagnostics` 为空、`warnings` 为空、51 元素 / 113 关系，否则 MUST 重建宿主后重跑、MUST NOT 删 `entry` 迁就宿主；再跑一次全量检查（门禁步 `LocalArchiCheck` 或 `archi_drift` 任一面，两面逐字段相等）并对拍 `apiFindings` = `apiEntryMissing` **12**（逐条点名：`App.Frontend.Components.Ui` + `Views.{Agents,AgentDebug,Architecture,Changes,Config,DbInspector,Explores,Info}` + `Views.Changes.{Flow,Renderers,Agent}`）/ `apiMissing` **27**（13 crate + `App.Frontend.Shell` + 12 前端单文件根 + `App.Backend.Shell`）/ 其余七条 0 / `apiUndeclared` 0，`status = clean`、`passed = true`；壳 MUST NOT 同时出现在两桶、两工具 crate 两桶皆零；`entry_state` 读数 `Resolved` 30 / `Absent` 12 / `Invalid` 0；分桶实测与 `grep` 计数结果落 `openspec/changes/module-entry-data-backfill/reports/`（按 `kind` 分桶的元素 FQN 清单），并在留痕内写明「净 −2 只来自两工具 crate 退出判据面，壳是 ① → ② 的平移」这一成因（三段读数 `28 + 13` → `15 + 26` → `12 + 27` 同源）；`pnpm run api:check` 三列皆空、`pnpm run server:check` / `client:check` 绿，同由桌面端 static-check / test-execution 承接；若变更 B 已先落地，① 期望改读 0、② 改读 39 并在留痕内说明成因，MUST NOT 改判据或就地改基线数字
