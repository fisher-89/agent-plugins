# 任务: archi-doc-api-surface-unification

> 覆盖 proposal「变更范围 - 实现文件」**3 行全量**（`api/contract.rs` / `AGENT.md` + `CLAUDE.md` / `packages/desktop/package.json`），零新增文件。测试文件面（`api/contract_test.rs` / `tests/pilot_test.rs`）的用例本体归测试相位——本相位**不写用例、不跑测试、不跑门禁**；编译与 `check` 四段链（`server:check` → `bindings:export` → `api:check` → `client:check`）由桌面端 static-check 承接。
>
> 依赖序要点：**阶段一**（抽取面 + 第四闸）是唯一代码面，解锁阶段四的核对；**阶段二**（根文档两句）与**阶段三**（版本行）互不依赖，但都必须在阶段四之前落地（AC-7 / AC-9 的改动集判据要一次看全）。
>
> 三条不可动摇的机械锚点（既有断言锚它们，改动即红）：① `contract.rs` 公开面恰 3 个 `\npub fn `（`parse` / `verify` / `is_constructor`）⇒ 新增的两个函数一律**私有** `fn`；② `pub(crate) const FENCE_` 恰 2 处 ⇒ 第四闸复用既有 `FENCE_OP` / `FENCE_TYPES` / `FENCE_REEXPORT`，MUST NOT 新增 fence 常量或引号字面量；③ 七族族别串与 `verify` 体内 ①…⑦ 注释逐字保留。
>
> 自查 cwd：`packages/desktop/src-tauri`（Rust 侧）；仓根（`AGENT.md` / `CLAUDE.md`）；`packages/desktop`（`package.json`）。

## 阶段一：散文表格抽取面与装载期第四条闸（`api/contract.rs`）

- [x] 修改 `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`：在私有 `fn collect_reexport_prose(...)`（`:949`）之后、`fn is_identifier(...)` 之前新增私有 `fn collect_surface_table(sections: &[ContractSection]) -> Vec<String>`——逐段 `for section in sections` → 逐行 `for line in section.body.lines()`：`let Some(rest) = line.trim_start().strip_prefix('|') else { continue };`（**首列 = 去行首 `|` 后的第一个 `|` 段**，不是 `split('|').next()`）→ `let cell = rest.split('|').next().unwrap_or("").trim();` → `` let Some(inner) = cell.strip_prefix('`').and_then(|text| text.strip_suffix('`')) else { continue }; ``（**整体被一对反引号包裹**）→ `` if inner.contains('`') || !is_identifier(inner) { continue; } `` → 去重保序 push（`if !names.iter().any(|name| name == inner) { names.push(inner.to_owned()) }`），返回 `Vec<String>`
- [x] 同文件为该函数写 doc：三重收窄（表格行 + 首列整体反引号 + `is_identifier`）、去重保序；并写明三条「零抽取」理由——列表项归既有 `collect_reexport_prose`（那是 `api-reexport` 第三条 gate 的机械镜像面，MUST NOT 在此重判）、裸正文是语义散文、fence 内容结构上不在 `section.body` 里（`parse` 的 fence 状态机已隔断）；再补一句「`### <op-id>` 段的正文不进 `sections`（`parse` 在 `###` 处 `finish_section`），故抽取面 == 全部 `##` 散文段」
- [x] 同文件在私有 `fn required_dangling_error(...)`（`:681`）之后新增私有 `fn surface_table_error(contract: &Contract, repeated: &[String]) -> ApiError`：`ApiError::Contract(format!(...))`，文案按序含 ① `contract.file` ② 交集名字 `repeated.join(" / ")` ③ 三处 fence 名（`{FENCE_OP}` / `{FENCE_TYPES}` / `{FENCE_REEXPORT}` 常量插值）④ 固定出路句「名字归 fence，散文只留语义：删掉表格里这些名字（语义用散文句承载）」⑤ 收束句「同一契约 MUST NOT 两处持有同一个名字」；**零行号**（段落正文未保留行号，与 `api-types` 缺键 / `api-reexport` 差集两闸同档）
- [x] 同文件 `pub fn verify(...)` 末尾（`verify_x_function_positions(contract, schema)?;` 之**后**、`Ok(())` 之**前**）追加第四段闸：`if !(contract.ops.is_empty() && contract.types.is_empty() && contract.reexports.is_empty()) { let repeated: Vec<String> = collect_surface_table(&contract.sections).into_iter().filter(|name| contract.ops.iter().any(|op| op.id == name.as_str()) || contract.types.contains_key(name.as_str()) || contract.reexports.contains_key(name.as_str())).collect(); if !repeated.is_empty() { return Err(surface_table_error(contract, &repeated)); } }`
- [x] 同处写注释（三句）：前置 `ops` / `types` / `reexports` 任一非空是**留白豁免的唯一来源**（三项全空即契约面留白，散文表格原样合法——前端 8 份当前形态的唯一通道）；断言是**交集为空**，MUST NOT 读成「散文必须镜像契约」（闸判的是同一名字不得两处持有）；本闸**只判点名、不判语义**（职能 / 边界 / 不变量 / 承载理由仍是人审面）
- [x] 同文件 `pub fn verify(...)` 的 doc 注释（`:264`–`:276` 段）追加一段第四闸说明：前置（三字段任一非空）/ 断言（`collect_surface_table` ∩ 名字集 = ∅）/ 错误面（`ApiError::Contract`，无行号）/ 「判的是同一名字不得两处持有，不占族编号、不新增 finding kind」
- [x] 同文件模块头 `:23`–`:25`：`三条双向 gate + 闸一…+ 自洽闸` 改为「**四条装载期闸**（既有三条双向 gate + 第四条名字清单闸）+ 闸一（七族名字，D45）+ 落点闸（D47）+ `required` 与 `properties` 的自洽闸」；「四者同函数、同趟收口」改为零数字表述（如「这些闸同函数、同趟收口」）；并在该段末补一句「第四条只判点名、不判语义」
- [x] 同文件模块头 `:30` 的「错误一律带 `file:line` 与原文片段」补限定：段落级判据（`api-types` 缺键 / `api-reexport` 差集 / 散文表格复述）只有 `file` 前缀，因为段落正文未保留行号
- [x] 自查 阶段一：`grep -c "^pub fn " crates/core/archi/src/api/contract.rs` 仍为 3（两个新函数全是私有 `fn`）；`grep -n "fn collect_surface_table\|fn surface_table_error" crates/core/archi/src/api/contract.rs` 各恰 1 处；`grep -c "pub(crate) const FENCE_" crates/core/archi/src/api/contract.rs` 仍为 2；`grep -n '"api-op"\|"api-types"' crates/core/archi/src/api/contract.rs` 仍恰 2 处（只落在 `FENCE_OP` / `FENCE_TYPES` 两行常量定义，第四闸一律走常量插值）；`grep -n "⑧" crates/core/archi/src/api/contract.rs` **零命中**（第四闸不占族编号）；`grep -n "① \|② \|③ \|④ \|⑤ \|⑥ \|⑦ " crates/core/archi/src/api/contract.rs` 的七族编号注释逐字在案；`grep -n "verify_x_function_positions(contract, schema)?" crates/core/archi/src/api/contract.rs` 之后紧接第四闸（顺序：落点闸 → 第四闸 → `Ok(())`）

## 阶段二：仓库根文档的纪律句（`AGENT.md` + `CLAUDE.md`）

- [x] 修改仓根 `AGENT.md` 的 `## 模块 API 契约` 第 ① 条（`:26`，`-**手写真相只有一份**` 那一条）：在「…三类 fenced json 块；」之后插入一句——`名字清单的唯一持有者也是这三类 fence——散文段 MUST NOT 用表格复述契约名字（重复持有即装载期契约错误），散文只留语义；`。该条其余文字逐字不动；该节其余条目（含 `doc` 条与第 ② / ③ / ④ / ⑤ 条）逐字不动
- [x] 修改仓根 `CLAUDE.md` 的同一位置插入**逐字相同**的一句（两文件该节正文必须逐字相同）
- [x] 自查 阶段二：`diff AGENT.md CLAUDE.md` 仍恰 2 处（`:1` 标题、`:3` 首行说明）；`sed -n '/^## 模块 API 契约/,/^## /p' AGENT.md | diff - <(sed -n '/^## 模块 API 契约/,/^## /p' CLAUDE.md)` 零输出；该节 `grep -c '^## 模块 API 契约'` = 1；`grep -n 'ApiError::Contract' AGENT.md CLAUDE.md` 在该节**零命中**；该节 `grep -n 'openspec/specs/'` 零命中、无 fenced json 块；要点条数相对 `HEAD` 不变（当前 6 条——`doc` 条在案，MUST NOT 删条、MUST NOT 新增第七条）

## 阶段三：版本行（`packages/desktop/package.json`）

- [x] 修改 `packages/desktop/package.json`：`"version": "0.7.0"` → `"version": "0.7.1"`（单行；桌面源码改动的版本纪律，无例外）
- [x] 不改 `packages/desktop/src-tauri/tauri.conf.json`（`"version": "../package.json"` 逐字不变）与 `packages/desktop/src-tauri/Cargo.toml`（crate 版本行 `0.1.0` 逐字不变）

## 阶段四：零扩散收尾核对（只读，不跑生成、不跑门禁）

- [x] `git status --porcelain` 的改动集 ⊆ `api/contract.rs` / `AGENT.md` / `CLAUDE.md` / `packages/desktop/package.json`（`openspec/changes/archi-doc-api-surface-unification/` 的文档属本 change 自身产物）
- [x] `git diff --stat .dev-team/architecture/models/ .dev-team/architecture/graph/` **零输出**（前端 8 份表格原样、后端 2 份零表格、`INDEX.toml` 零改动）
- [x] `git diff --stat packages/desktop/src-tauri/crates/core/archi/src/api/{mod.rs,check.rs,schema.rs,render.rs,diff.rs} packages/desktop/src-tauri/crates/core/archi/src/drift/` **零输出**（判据面与装载入口零改动；`api/mod.rs` 的「双向 gate」泛称 doc 也不动）
- [x] `git diff --stat packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` **零输出**（3KB 骨架贴边，本变更不加纪律句）
- [x] `git diff --stat packages/desktop/src/ packages/desktop/src/types/generated/` **零输出**（零 IPC / 零 DTO）
- [x] `git diff -U0 packages/desktop/package.json` 只有 `version` 一行变化

> **移交（不在本相位，不勾为任务）**：`api:check` 零差异退出与四段 `check` 链由桌面端 static-check 承接；`api/contract_test.rs`（抽取面正反例 + 第四闸正反例 + 留白豁免「补契约即转红」两态 + `api-reexport` / `$defs` / op id 三类撞名）与 `tests/pilot_test.rs`（递归 `api_dir()` 全部 `*.md`，路径串经 `archi::api::display` 归一，经 `parse` + `build_schema` + `verify` 全线通过；不钉死份数，改为断言「至少一份非留白契约、至少一份留白契约各通过」以免目录漂移让用例空转）的用例本体归 test-design / test-gen / test-execution。新用例 MUST NOT 新建 `api/**` sidecar（`golden_test.rs:1482` 的 13 份计数断言不动），演示用的「补 `api-types` 键 + 同名表格行」夹具 MUST NOT 落进 `tests/corpus/**`。
