# 任务: archi-model-path-and-silent-trap

> **覆盖口径**：覆盖 proposal「变更范围 - 实现文件」**全量 9 行 / 11 个文件**（`parse/toml.rs`、`drift/mod.rs`、`drift/report.rs`、`02` / `03` / `05` 三个模型文件、`decisions/0001-model-granularity.md`、`archi-knowledge.md`、`sdk/archi_tool.rs`、`types/generated/bindings.ts`（生成物）、`package.json`）。proposal「测试文件」的 5 行**不属本相位、不入本文**——用例名 / needle / 拆分边界 / 体量锚冻结值由 design 末节「交接给下游相位的随动清单」单点持有。
>
> **不在本 change**：L4（`module` 多 `path` × 契约面只认 `path.first()`）、元纪律入 spec、「`metadata` 子表推荐形态」改向、`kind` 层级序校验、语料 / 金样的任何改动（含新增 case）。
>
> **数据面已由 dev-design 相位落盘（本相位只在阶段零核对，不重写）**：`02` / `03` / `05` 三个容器的 `kind` 已改 `composite`、ADR §2 / §3 已改写（相位锁把 `<archi_dir>/graph/model/**` 的写入收在 proposal / design 相位）。落盘后实测：55 元素 / 129 关系 / 四类结构诊断零命中 / `modelWarnings` 仍 **6**（drift 尚未读 `kind`，中间态无回归）/ `observations` 53 / `apiFindings` 39 / `status = clean` / `passed = true` / `scan` 422 / 359 / 422 / 0 / 0 / `modelHash` `sha256:05b61bde…` → `sha256:6cc50dbf…`。
>
> **测试面交接**：本文件不写用例、不跑测试，也不为任何测试文件登记任务。必改机械面（用例名 / needle / 拆分边界 / 体量锚冻结值）由 design 末节「交接给下游相位的随动清单」单点持有。与之相关的跨相位约定只有两条：① 行为锚由测试相位按该清单第 1 行落地；② 锚若失败，按 design 待决问题 3 把骨架 / 模板的「等价」句降级为「不推荐但合法」（本文件的阶段三文案即为回落时会改的那处）。
>
> **AC-6 判据口径（回溯修订，design D14；取代旧的「`crates/**` 全零命中 + delta 命中须同行含 `MUST NOT`」）**：case B 的假因果按**面**分判——① **正向表述面**（`crates/**` 除防线单元之外的源码与注释、`archi-knowledge.md`、`SCAFFOLD_TEMPLATE`、`decisions/**`）**零命中**；② **防线面**（`archi_knowledge_test.rs::骨架形态纪律四条在场` 的 `for forbidden in [四 needle]` + `assert!(!source.contains(forbidden), …)` 与函数 doc comment）**必须在场，删了即红**；③ **spec 面**收窄到本 change 的 `specs/**`，命中只许是否定 / 禁令形态（按**语义**判该句是否把假因果陈述为事实，不按同行词面判）。基线 `openspec/specs/**` / `archive/**` / `explore.md` 不在追改面（delta-only）、命中不计红、MUST NOT 手改。逐面复核见**阶段五**。
>
> **静态步骤归属**：`pnpm check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `pnpm test` 由桌面端 static-check / test-execution 执行；本文件只登记边界与自查命令。`bindings:export` / `api:export` 属**生成步骤**（产物须入库），按阶段列出。
>
> **阶段收口即可编译**：阶段零零改动（只核对已落盘的数据面与 ADR）；阶段一只动 `parse/toml.rs`；阶段二只动 `drift/{mod,report}.rs` + 重导生成物；阶段三只动知识骨架与模板字面（零代码路径变化）；阶段五零改动（只读 grep 复核 AC-6 三面）——每阶段收口时一次提交。
>
> 自查 cwd：仓库根（模型 / ADR / 知识文本）；`packages/desktop/src-tauri/crates/core/archi`（Rust 侧判据）；`packages/desktop/src-tauri/crates/infra/agent`（知识 / 模板）；`packages/desktop`（`package.json` / 生成物）。

## 阶段零：数据面与 ADR 落盘核对（已由本 change 的 design 相位落盘，本相位只核对 / 被回退才补）

- [x] `.dev-team/architecture/graph/model/02-backend-core.toml` / `03-backend-infra.toml` / `05-backend-tools.toml`：三个分层容器的 `kind = "composite"` 已落盘——逐文件核对恰 1 行 `kind`（`Backend.Core` / `Backend.Infra` / `Backend.Tools`）、`path` 仍空、表头计数不变（`9 / 10`、`8 / 25`、`3 / 3`）；若工作树被回退则补这三行，MUST NOT 动其他元素与表头
- [x] `.dev-team/architecture/decisions/0001-model-granularity.md`：§2 末段有「纯分组容器 + 三容器指称 + `kind = "composite"`」同句且无「分层容器 × `package`」并置、§3 首段两组语义（`composite` 纯分组 / `domain` 业务聚合）与成员成对、全文「同为 `package`」「本仓当前无实例」零命中（AC-13）；被回退则按 design「修改文件」表该行重写

## 阶段一：L1 判据（`package` 的 `path` 上界，parse 期）

- [x] `crates/core/archi/src/parse/toml.rs` 的元素循环（`to_model`）：先取 `let fqn = key.into_inner();`（`span` 在 `into_inner` 前取好），再 `diagnostics.extend(validate_path(source, &fqn, raw_element.kind, &raw_element.path));`，随后**照常** `elements.push(Element { fqn, … })`——MUST NOT 跳过元素（「不跳过」的纪律只在 `dto::graph_view` 的停摆分支）
- [x] 同文件新增私有 `fn validate_path(source: &str, fqn: &str, kind: ElementKind, path: &[::toml::Spanned<String>]) -> Option<Diagnostic>`：`kind != ElementKind::Package || path.len() < 2` ⇒ `None`；否则取 `path[1].span()`，产出 `Diagnostic { kind: DiagnosticKind::InvalidValue, message: format!("元素 {fqn} 是 package：一个包至多一条包根目录（纯容器可留空）——多条 path 属 module"), file: String::new(), line: line_of(source, span.start), column: Some(column_of(source, span.start)) }`（message MUST 含「至多一条包根」与出路；行号 MUST 落在第 2 条 `path` 所在行）
- [x] 同文件模块 doc（`:1`–`:5`）补一句「`path` 的基数随 `kind`：`package` 至多一条包根、`module` / `domain` / `composite` 不设上界」；`RawElement` / `RawDocument` / `validate_apis` / `classify` / `line_of` / `column_of` / `SourceParser::parse` 的合并逻辑逐字不变
- [x] 自查 阶段一（判据只对生产码生效：目录级 grep 一律加 `| grep -v '_test\.rs'`）：`grep -c 'fn validate_path' …/parse/toml.rs` = 1；`grep -c 'ElementKind::Package' …/parse/toml.rs` = 1（只在 `validate_path` 体内）；`grep -n 'path.len()' …/parse/toml.rs` 只出现在 `validate_path`；`grep -c 'diagnostics.extend' …/parse/toml.rs` = 2（`validate_apis` 与 `validate_path` 各一）；`grep -n '至多一条包根' …/parse/toml.rs` ≥ 1；`grep -rn 'fn validate_path' packages/desktop/src-tauri/crates --include=*.rs | grep -v '_test\.rs'` 恰 1 处

## 阶段二：L3 收窄 + 词汇 doc（与已落盘的 L2 数据同批收口，`model.warnings` 6 → 0）

- [x] `crates/core/archi/src/drift/mod.rs`：`use crate::dto::{self, GateTier};` → `use crate::dto::{self, ElementKind, GateTier};`
- [x] 同文件 `:114` 分支收窄：`if element.path.is_empty() { if matches!(element.kind, ElementKind::Package | ElementKind::Module) { model_warnings.push(ArchiDriftEntry { … }); } continue; }`——`message` 字符串、条目字段（`from` / `to` / `gate` / `occurrences`）与 `pathNotFound` 段逐字不动；`matches!` 外**保留** `continue`，`domain` / `composite` 空 `path` 元素照样不进 path 存在性循环
- [x] 同文件模块 doc 的 ④ 段补「`noPath` 按 `kind` 分档（`domain` / `composite` 豁免）」；`derive_status` / 纳管子集 / `apiFindings` 装配 / `passed` 逐字不变
- [x] `crates/core/archi/src/drift/report.rs` 两处 doc（两处都经 specta 落 `bindings.ts` 的 JSDoc）：① 枚举级总述尾句 →「`noPath`（`package` / `module` 未声明 `path`；`domain` / `composite` 空 `path` 是合法形态）」；② `NoPath` 变体注释 →「`package` / `module` 未声明 `path`（不参与归属；容器空 `path` 是合法形态）」；五值 / `ALL` 序 / `as_str` / `REPORT_VERSION` / DTO 字段集逐字不变
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 并入库；核对 `git diff -- packages/desktop/src/types/generated/` 只含 `bindings.ts` 的两处 `NoPath` JSDoc、`bindings.zod.ts` **零 diff**；MUST NOT 手改生成物
- [x] 阶段二读数复核：L3 收窄的豁免面由代码路径确认（`matches!` 只放 `package` / `module`）；桌面 `archi_drift` 工具链读数为旧二进制（仍按 6 条 `noPath` 输出、`modelHash` 与 design 落地面一致 55 / 129 / `sha256:6cc50dbf…`），`modelWarnings` **0** 由静态 / 测试相位复核；`elements` 55 / `relations` 129 / `byGate.off` 55 / `structuralErrors` [] / `observations` 53 / `apiFindings` 39（全 `docMissing`）/ `errors` / `warnings` [] / `status = clean` / `passed = true` / `scan` 422 / 359 / 422 / 0 / 0
- [x] 自查 阶段二：`grep -c 'model_warnings.push(' …/drift/mod.rs` = 2；`grep -c 'errors.push(' …/drift/mod.rs` = 3；`grep -c 'warnings.push(' …/drift/mod.rs` = 6；`grep -cF 'matches!(element.kind, ElementKind::Package | ElementKind::Module)' …/drift/mod.rs` = 1；`grep -n 'path.is_empty()' …/drift/mod.rs` 仍 1 处；`grep -n 'noPath' packages/desktop/src/types/generated/bindings.ts` 命中枚举级总述与变体行两处新文案

## 阶段三：知识骨架与模板（假因果收口 + 四值语义；`metadata` 等价句与 design 末节交接清单第 1 行的行为锚绑定，锚失败按 design 待决问题 3 回落）

- [x] `crates/infra/agent/src/archi-knowledge.md` 键形句（`:5`）：删「点分嵌套与子表读成**另一元素**（`metadata` 必须内联表）」，改为「未引号点分 `[element.App.Auth]` 使**整份源解析失败**（诊断点名非法子键）；`metadata` 子表与内联表等价（子表写在已声明元素之后零诊断，成为唯一声明才缺 `kind`）」——**零结构诊断词**（`invalidValue` / `unknownField` / `missingField` / `syntaxError` / `duplicateElement` / `cycle` / `unknownReference` 一律不出现）
- [x] 同文件 `path` 纪律（`:10`）：改为「模块必填（可多条 = 若干根的清单）；包至多一条包根（纯容器可留空，≥2 条即模型停摆）；`domain` / `composite` 选填（缺省空）」+ 既有「repo 相对 / `./` 前缀 / 目录尾 `/` / 文件到扩展名 / 容器填了即持文件 / `dependsOnAncestor`」句 + 压缩句「`domain` / `composite` 空 `path` 零告警」，**尾句「缺 `path` 看 `archi_module` 的 `hasPath`」原样保留**（`骨架_指路表逐句在场` 的 needle）
- [x] 同文件 `package` bullet 补「至多一条包根 `path`」、`composite` bullet（`:24`）补「纯分层容器是标准形态」（保留「组合」「最小可编译单元」两个既有语义词）
- [x] 同文件同段压缩等量冗余（合并「边界与自检」段里重复的「生成物 / 指针」短句、删旁注），使骨架净增长后仍落 **2300–2600 B**；零示例围栏 / 零 `path = ["` / 零判据长表 / 零 `<>` 占位
- [x] `crates/infra/agent/src/sdk/archi_tool.rs` 的 `SCAFFOLD_TEMPLATE`（`:198`）三行：① `kind` 行四值齐列（`package` = 包（至多一条包根 `path`，纯容器可留空）/ `domain` = 领域容器（`path` 选填）/ `module` = crate、目录型模块、视图或单文件组件（`path` 必填、可多条）/ `composite` = 纯分组（无层级，可嵌套任意类型之上；纯分层容器是标准形态））；② `path` 行改「**module 必填**（可多条 = 若干根的清单）；package 至多一条包根（纯容器可留空；≥2 条即模型停摆）；domain / composite 选填；`./` 前缀、repo 相对；目录以 `/` 结尾、文件精确到扩展名」（删「容器（package / domain）恒空」）；③ `metadata` 行改「**子表与内联表等价**（`[element."X".metadata]` 写在已声明元素之后零诊断；成为唯一声明才缺 kind）」（删「子表会被读成另一个元素」）；`scaffold_draft` / `__KIND__` / `scaffold_payload` / 参数与响应形状逐字不变
- [x] 自查 阶段三（**正向表述面**按面列举，不以 `| grep -v '_test\.rs'` 作唯一判据——三面口径见阶段五）：`grep -rnF -e '读成另一个元素' -e '读成**另一**元素' -e '必须内联表' -e '被读成元素' -e 'MUST 用内联表' -e '必须内联表、禁子表' packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs .dev-team/architecture/decisions` **零命中**；`grep -n 'unknownField\|invalidValue\|missingField\|syntaxError' packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` **零命中**；`grep -c 'path = \["' packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` = 0；`grep -c '缺 `path`' packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` ≥ 1；`grep -n '子表与内联表等价' packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` 各 ≥ 1；`grep -n 'module 必填' packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` = 1；骨架 / 全文 LF 归一字节实测值落在 2300–2600 / 2450–2750

## 阶段四：版本与收口核对（不重复执行桌面端静态步骤）

- [x] `packages/desktop/package.json`：`"version"` `0.7.8` → `0.7.9`（相对落地时 `HEAD` 恰 +1 patch）；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变
- [x] 执行 `pnpm -C packages/desktop run api:export`（生成步骤）：`graph/INDEX.toml` 仍 **49** 条 `[[entry]]`、`models/**/*.schema.json` 与 `.i.rs` / `.i.ts` **零 diff**（`kind` 不进索引；三容器本就 `path` 空而退出契约面）；随后 `pnpm -C packages/desktop run api:check` 零差异退出
- [x] 零扩散核对（`git status --porcelain` 的改动集 ⊆ design D12 白名单 = 实现面 11 行 + design 末节交接清单登记的断言面 5 行）：出现 `scan/owner.rs`、`edit.rs`、`diff.rs`、`dto.rs`、`graph.rs`、`model.rs`、`api/**`、`tests/corpus/**`、`expected/*.json`、`bindings.zod.ts`、`INDEX.toml`、`models/**`、`src/**`（前端产品代码）、`AGENT.md` / `CLAUDE.md`、`openspec/specs/**`、`src-tauri/Cargo.toml` 任一 diff 即回退
- [x] 语料 / 金样零 diff 核对（只比 diff，不跑测试）：`git diff --stat -- packages/desktop/src-tauri/crates/core/archi/tests` **零输出**（20 份 `expected/*.json` 与 `corpus/**` 逐字节不变）；`git status --porcelain` 无 `D` / `R` / 新增 `.rs`（`scan.*` 三项与基线逐字段相同的前提）
- [x] 常量核对：`grep -n 'REPORT_VERSION' crates/core/archi/src/drift/mod.rs` 仍 `3`；`grep -c 'pub const ALL' crates/core/archi/src/drift/report.rs` 与 `dto.rs` 的九值 `ALL` 逐字不变
- [x] 验收对照：逐条回应 design「验收标准回应」表（AC-1…AC-13）——AC-1 / AC-2 / AC-3 由阶段一 / 二的自查与测试相位断言背书；AC-4 / AC-5 由上条读数复核；**AC-6 由阶段五的三面复核**（阶段三只覆盖其中正向面的一角）；AC-8 由阶段三自查；AC-9 由阶段二 / 四的生成步骤；AC-10 / AC-11 由本条白名单与版本核对；AC-12 由 4 份 spec delta 承接（本变更不改写）；AC-13 由已落盘的模型三行 + ADR 两段（「同为 `package`」「本仓当前无实例」零命中）背书

## 阶段三修：知识骨架定稿复核（回溯修复）

> 上一轮为了压回体量删掉了四处不在 design 授权面内的既有规范语义（`domain` 层可省略 / `package` 的「指定语言」/ `domain` 的「例：订单/通知」/ 边界的「指针不扩张」），并把 `path` 尾句缩成 ``archi_module.hasPath``。本节按 design 授权面回写：四处语义与尾句逐字复原，净增字节只由**词级等量置换**抵消，未再删任何纪律句（R-5 / AC-8 的「不删纪律」）。

- [x] 四处规范语义逐字复原：「## 元素类型（Element.kind，从大到小，`domain` 层可省略）」（「`domain` 层可省略」= delta `desktop-archi-model:61` 同口径）、`package` = 包（最小可编译单元、独立依赖、**指定语言**；至多一条包根 `path`）、`domain` = 领域（按业务实体边界聚合，**例：订单/通知**）、边界句「正文归契约资产，**指针不扩张**」
- [x] `path` 尾句按 design 原样写回：「缺 `path` 看 `archi_module` 的 `hasPath`」（`骨架_指路表逐句在场` 的「缺 `path`」/ `archi_module` / `hasPath` 三 needle 仍同场）
- [x] 净增字节的抵消面（全部为词级等量置换与同段合并，零纪律删除）：① 跨段合并重复的「生成物」短句——「生成物过期用既有生成命令重生成，MUST NOT 手改生成物」从「自查」段移入「写面」段，同批删去被它包住的 `（重生成除外）`；② `/` 两侧空格（`接口/导出符号`、`model.elements`/`model.relations`、`ignore`/`exclude`、`覆盖/归属/契约读数`），`gate` / `api` 值域行与「`domain` / `composite`」的空格按断言面原样保留；③ 重复词与冗余动词（`选填名字清单` → `选填清单`（同行已有「只写名字」）、`若干根的清单` → `若干根清单`、`写了即` → `即`、`子元素依赖它会触发` → `子元素依赖即`、`按包含关系从大到小` → `从大到小`、`提示用户「刷新」` → `提示刷新`）；④ 键形句的加粗句式保留，未动 `**带引号的完整 FQN 单键**` / `**整份源解析失败**` / `**不会自动更新**`
- [x] 体量实测（LF 归一、`trim_end`，只读盘测量）：骨架 **2596 B** / 全文 **2739 B**——落 2300–2600 / 2450–2750，且与测试现状冻结值一致
- [x] 断言面自检（只读盘比对，不跑测试）：关系四键串 `` `from` / `to` / `desc` / `apis` `` 逐字在场（MUST NOT 压实空格）、「- `apis`」行四要素（只写名字 / 可留空 / 生成物面留空 / `archi_drift`）、`带引号的完整 FQN 单键` / `子表与内联表等价` / 「唯一声明才缺 `kind`」/「目录尾 `/`」 / `dependsOnAncestor` / `纯分层容器是标准形态` / `既有生成命令` / `MUST NOT 手改生成物` / `git diff` / `禁止改架构目录之外的文件` / `不会自动更新` / `续接历史会话` 全在场；七个结构诊断词与三句假因果零命中；零 `<>` / 零示例围栏 / 零 `path = ["`
- [x] **交接（测试相位执行，本相位不改测试文件）**：`crates/infra/agent/src/archi_knowledge_test.rs` 的两处体量锚——`骨架体量锚` 冻结值 `2596`、`全文体量锚` 冻结值 `2739`——与本次实测**相同**，值本身当前无需变更；仍请测试相位按落地后的工作树复测一次（`lf_bytes(skeleton().trim_end())` / `lf_bytes(&text())`），若有偏差以复测值同批更新冻结值与注释数字，**区间 2300–2600 / 2450–2750 与门限 2700 / 2850 不抬**（AC-8 / R-5）。**本相位已按落地工作树只读复测**：骨架 `2596 B` / 全文 `2739 B`，与冻结值逐字节一致——交接条件已满足，测试文件零改动
- [x] **交接（同上）**：「生成物过期…」一句已从「自查」段移入「写面」段（重复短句合一），`骨架零项目专用命令`（`既有生成命令` + `MUST NOT 手改生成物`）与 `骨架_指路表逐句在场` 的「生成物过期」一行都用 `source.contains`（非行内定位），改动后仍成立——仅登记核对，无需随动。**本相位已只读核对**：「写面」段同行含「生成物过期用既有生成命令重生成，MUST NOT 手改生成物」，三 needle 同段在场

## 阶段五：AC-6 三面复核（回溯修订；只读 grep / 读盘比对，不跑测试、不改任何文件）

> 触发：上一轮 AC-6 的旧口径（`crates/**` 全零命中 + delta 命中须同行含 `MUST NOT`）与测试计划冲突且对基线不可满足——`archi_knowledge_test.rs` 的 `assert!(!source.contains(…))` 四条 needle 属「骨架零假因果」防线（必须在场），基线 `openspec/specs/**` 在 delta-only 下不可手改。本节按 design D14 三面重判，**MUST NOT 为通过 grep 删除任何 needle 或禁令句**（删与留断言同判红，R-7）。

- [x] **正向表述面零命中**（`crates/**` 除防线单元 + `decisions/**`；needle 集 = 六个 `-e`，与阶段三自查同一串）：`grep -rnF -e '读成另一个元素' -e '读成**另一**元素' -e '必须内联表' -e '被读成元素' -e 'MUST 用内联表' -e '必须内联表、禁子表' packages/desktop/src-tauri/crates --include='*.rs' --include='*.md' | grep -v 'archi_knowledge_test.rs'` **零命中**（生产源码与注释、`archi-knowledge.md`、`SCAFFOLD_TEMPLATE`、其余测试文件内防线单元之外的注释一并覆盖；`| grep -v '_test\.rs'` 只是下限，不作唯一判据）；**同一 needle 集**对 `.dev-team/architecture/decisions` 的 `grep -rnF` **零命中**
- [x] **防线面必须在场**（删任一 needle / doc comment 即红）：`archi_knowledge_test.rs` 内命中恰 **5 行**且全落防线单元——`:339` 函数 doc comment（「假因果…零命中」说明）+ `:352`–`:355` 四条 needle；`grep -n 'fn 骨架形态纪律四条在场' …/archi_knowledge_test.rs` 命中 1 处；`grep -c '!source.contains(forbidden)' …/archi_knowledge_test.rs` = 1（循环体 `:351`–`:360`，未退化成同行文本匹配）
- [x] **spec 面只许否定形态**（收窄到本 delta）：**同一 needle 集**对 `openspec/changes/archi-model-path-and-silent-trap/specs/` 的 `grep -rnF` 恰 **8 行**——`desktop-archi-agent/spec.md` `:8`（`MUST NOT 再出现…`）/ `:34`（`MUST NOT 再出现…`）/ `:60`（`不是「读成另一个元素」`）/ `:129`（`MUST NOT 再表述为…`）/ `:143`（`MUST NOT 出现…`）、`desktop-archi-model/spec.md` `:8`（`MUST NOT 再写成…`）/ `:56`（`MUST NOT 再表述为…`）/ `:91`（`MUST NOT 把该表读成另一个元素`）；逐行读句全为否定 / 禁令语境，**零断言陈述**（判据是语义，不是同行词面）
- [x] **追改面之外零改动**：`git status --porcelain -- openspec/specs openspec/changes/archive` **零输出**（delta-only：基线旧句 `desktop-archi-model:51` / `:80`、`desktop-archi-agent:184` / `:384` / `:498` 的命中**不计红**、MUST NOT 手改，由其归档合并本 delta 的 MODIFIED 块收口）；`explore.md` 只读输入、无实现面改动
- [x] **反向锚（R-7）**：骨架侧四条 needle 与 doc comment 在场、4 份 delta 的否定 / 禁令句在场——两侧否定锚是防假因果回流的唯一把手；本条与上条互为反向判据：删锚与留断言同判红
