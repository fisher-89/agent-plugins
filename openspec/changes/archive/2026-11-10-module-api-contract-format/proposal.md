# 提案: module-api-contract-format

> **变更**: module-api-contract-format
> **回溯对齐（2026-11-04 · 第三稿）**: 三处冲突全在本文件单侧文本、design 已定稿，故只改提案与 spec，零实现面变化 —— ①**判定语料落点**：旧稿 `测试文件` 表与 AC-7 只写「漂移语料」与 `corpus/api/`（**全仓不存在该目录**），与 design 定稿的「唯一驱动面 = 既有 `DRIFT_CASES` + `corpus/drift/` + 十一份 sidecar 用例」不一致 ⇒ 测试表与 AC-7 双双补齐：`api-mismatch` 新 case 三个 fixture + 非空金样、既有 15 份金样重写、`DRIFT_CASES` 登记面三形态失配皆红、逐判据正反例由 `api/**` sidecar 直驱，并显式 MUST NOT 另建 `tests/corpus/api/`（本轮回溯同步改 `desktop-archi-api-check` delta 的「语料守护」条、其两个 Scenario、Module Contract 一行）；②**`unused` / `knip` 口径**：旧稿「§9-`visibility`」与「实现文件 `api/extract/**`」两处写「TS 侧 `unused` 复用 `knip` 报告」与「`unused` 读 `knip` 报告」，与 design D39 / S-12 及 spec「`apply_knip_report` 已实现但本期未接线、生产面不产出 `Unused`」相反 ⇒ 两处改判为「生产面只产 `external`；`Unused` 的降级接口本期未接线、产品路径零调用者、生产面不可达为显式留白」，AC-20 ④ 与测试表 TS 抽取行同步；③**`api/mod.rs` 出口**：旧稿写 `load_contracts` / `verify_contracts` / `bootstrap` 三函数（`verify_contracts` 全仓零存在），与 design 公共函数表八行（`meta` / `enabled` / `load_contracts` / `render_all` / `bootstrap` / `check` / `api_ref` / `api_ref_of`）不一致 ⇒ 按八行逐名重写（**本处「八行」按第三稿时点值读**：design 后续 D55 把 `display` 升为 `pub fn` 并经 `lib.rs` 出口，终态为**九行**——终态以 design「变更清单 - 新增文件」表 `api/mod.rs` 行为唯一源，本处记录不重写），并写明「MUST NOT 另立比对 / 归一函数」。**回溯记录**：本稿只改 proposal 与 spec 文本，零实现面变化）
> **回溯对齐（2026-11-04 · 第四稿，R11）**: 评估报 **R11 冲突属实**——delta spec 三处（`specs/desktop-archi-api/spec.md` 的 `:329` / `:330` / `:352`）仍以现在时断言「本期读 `knip` 并产出 `Unused`」，与本文件 `:225`（§9-`visibility`）/ `:311`（实现文件 `api/extract/**` 行）/ `:347`（测试文件 TS 抽取行）/ AC-20 ④ 及 design **D39** 互斥。根因是 **S 项射程不全**：S-12 只登记 `:423`，S-7 的替换文本自留「`knip` 报告未使用 → `unused`」（等于把冲突搬了个位置），S-8 未随 D39 收紧。**处置**：S-7 射程由 `:295` 扩至 `:329` + `:330`、S-7 / S-8 替换文本按 D39 改写为「四值词表封闭 + 本期可控值域按语言分列（Rust 四值可判 / **TS 生产面只产 `external`**）+ `Unused` 未接线、生产面不可达为显式留白」，并消除 S-8 新串与 S-12 判据串的串味；本文件 §9 的 `unused` 表述同步收紧为「**将来**判据面（接线后）」，与 AC-20 ④ 同源。**回溯记录**：本稿只改 proposal 与 spec 文本，零实现面变化）
> **回溯收口（2026-11-04 · 第五稿，R11 复检）**: 复检 delta spec 的 `:329` / `:330` / `:352` 三处，旧串（「`knip` 报告未使用 → `unused`」/「`unused` 输入 SHALL 复用 `knip` 报告」/「仅测试引用四档判定」）**已零命中**、design「S-7 / S-8 替换文本」**逐字落地**，R11 的主诉已消解；但同源措辞尚有两处残留，一并收口（零实现面）：① `specs/desktop-archi-api/spec.md:329` 的括号笔误（`均不作 proxy））。` → `均不作 proxy）。`），使该条与 design S-7 替换文本逐字相等；② 本文件 §9-`visibility` 与「实现文件」表的 `api/extract/**` 行仍写「Rust 四值可判 / TS 生产面只 `external`」，读作「`unused` 在 Rust 侧本期产得出」——与 AC-20 ④（`Visibility::Unused` 在 `rust.rs` **零命中**、唯一产出点在被零调用的 `apply_knip_report` 内）及本文件同节的「`unused` 生产面不可达」互斥。改判为「四值词表封闭、两语言的 `unused` 产出点都在『调用方按引用事实降级』一侧、两个取数器都不产、该调用方本期不存在 ⇒ 生产面不可达」，与 design D19-b / D39、AC-20 ④ 同源。**回溯记录**：本稿只改 proposal 与 spec 文本，零实现面变化）
> **回溯对齐（2026-11-04 · 第六稿，R11 之后的三条主诉）**: 三条主诉经只读复核全部成立，且都属「proposal 文本未随 design / spec 收敛」，零实现面改动 —— ① **fmt 段缺项**：delta spec 的「生成物入仓与零 diff 门挂载」requirement 正文、其 Scenario「check 链扩一段且 fmt 段改校验模式」与 Module Contract 表 `package.json` 行三处（S-1 / S-3 / S-6 的落地）强制 `server:check` 的 fmt 段由写模式改 `cargo fmt --all --check`，而本文件实现文件表与 AC-14 均未登记该项、全文零 `cargo fmt` 命中 ⇒ 三处补齐（实现文件表 `package.json` 行 / §10 工程落位 / AC-14），并把同源的「生成物 = 格式化器不动点」（design D29）写进 §10 与 AC-12：不动点不成立时 `api:check` 的字节门与 `client:check` 的 `vp check --fix` **永久对撞**；② **试点 op 与 spec 规则 4 互斥**：旧稿四处（§9 试点段 / 实现文件表试点 md 行 / `.i.rs` 生成物行 / R-13）把 op 定为 `compose_text`，其入参类型由 `App.Backend.Core.Archi` 提供，而跨模块 `$ref` MUST 指向**直接子元素**（`desktop-archi-api`「模块嵌套与选择性透传」规则 4）⇒ 与 design **D23**、已落盘的 `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md` 的 `### file_stem`、`crates/infra/archi.i.rs` 的签名见证、AC-7（其余各 0）与 AC-13 互斥；改判为 `file_stem(timestamp: string) -> string`（纯标量、`report_test` 在测、零跨模块引用），`report.rs` 纳入试点行，`compose_text` 明确「保持私有、不进契约」；③ **`#[path]` 产出语与 spec :201 相反**：旧稿写「值由单点函数产出」，而属性值 MUST 是**字面量**且该函数**零产品调用者**（design D38 / spec S-11 落地文本）⇒ 两处（实现文件表 `lib.rs + runner.rs + report.rs` 行 / AC-4）改写为「字面量 + 与 `paths::mod_path_attr` 输出逐字相等的机械断言」，并同步清掉 delta spec 同源残句两处（`specs/desktop-archi-api/spec.md` 的两个 Scenario 里「取单点函数产出的 `#[path]`」→ 取 `paths::mod_path_attr` 的输出为真值源、入口行写字面量）；**S 项替换文本本体零改动**（第十二稿已收口，本轮不重开 S-1…S-12）。**行数口径**：实现文件表 **27 行**（`report.rs` 并入既有试点行，不增行）、测试文件表 **17 行**（试点形状断言的落点写进 AC-13 与既有 `runner_test` / `report_test` 扩展行，不新增表行）——design 的封闭数与 tasks 首部行数口径逐值沿用第五稿。**回溯记录**：本稿只改 proposal 与 spec 文本，零实现面变化）
> **回溯对齐（2026-11-04 · 第七稿，spec 文本面收口）**: 回溯主诉是「实现面 D43–D52 已逐项在盘，缺口全在 delta spec 文本面」——**S-13 / S-14 / S-15**（`tasks.md` 阶段二十三 ⑧ 已勾 `[x]`）与 **S-16 / S-17**（`tasks.md:328` / `:329` 仍 `[ ]`）在 `specs/**` 内**旧串 0 命中、新串 0 命中**，即「已勾选未兑现」；而 implement 的可写面只有 `tasks.md`，MUST NOT 代写 `specs/**`，故重做本相位必复现同一失败。**处置（本稿即 spec 文本面 owner，零实现面变化）**：按 design「spec 措辞同步项」表逐字落地五条新串——`specs/desktop-archi-api/spec.md` 的①位置分派条追加「位置词汇与签名短形 MUST 各只有一份」（S-13）、②「四条明确不做」之前新增「契约文本进生成器 SHALL 经两道人字闸，且 `x-function` 的落点 SHALL 封闭二处」（S-15、纯新增）、③「生成物落点与入口查找」末段新增「`#[path]` 的读盘面 SHALL 与写盘面同一条限根纪律」三条（S-16、纯新增）、④ Module Contract 表 `api/paths.rs` 行补「末段进源码的形态闸按语言分列 + `checked_root` 只判整条不判末段字符集」（S-17、纯新增）；`specs/desktop-archi-api-check/spec.md` 的⑤判据表 `apiMissing` 行、⑥「`apiMissing` 覆盖两类缺口」条各钉死「同一条判据、同一次求值 + 三条排除面 + 两条结构前提」、Module Contract 表 `api/check.rs` 行改为「声明集（op **与 `$defs` 类型**）vs 导出集」（S-14，其三处锚点）。**同批修正 design 的丁类旧串计数互斥**：旧文「旧原文串数 = 14」与「丁类旧串计 0（S-13 / S-14 各一处但属保留型…）」并存 ⇒ 丁类改判为 **1**（唯一被替换消失的是 S-14 的 Module Contract 表 `api/check.rs` 行旧串「声明集 vs 导出集」；S-13 与 S-14 的 `:12` / `:61` 属「补句 + 钉死」保留型、S-15 纯新增 0），四处复述（射程判据段 / 表列顺序说明段 / AC 表 S-1…S-17 行 / 丁类纪律第 6 条）同值，算式回到自洽：甲 4 + 乙 5 + 丙 2 + **丁 1** + 戊 0 + 主基线 2 = **14**（17 条 S 项）、新原文串仍 **18**。**判据（本稿交付态）**：`grep -cF` 对五条新串在 `specs/**` 各 **≥1** 命中（S-14 的钉死句按 design 指定出现在判据表行与 requirement 条两处，故为 **2**），旧串「声明集 vs 导出集」在 `specs/**` **0** 命中；S-13 / S-14 / S-15 / S-16 / S-17 的旧串判据仍按 design：保留型项（S-13、S-14 的 `:12` / `:61`）**只判新串在场**、S-15 / S-16 / S-17 为纯新增条（旧串 0）。**零实现面**：`specs/**` 与 `design.md` 两处文本、不进实现相位封闭集合，`tasks.md` 由 implement 相位勾选（阶段二十三 ⑧ 的丁类三条补证、阶段二十的 S-16 / S-17 两条），本稿不代勾；`tasks.md` 阶段二十首部的「条数 12 / 旧原文串数 13」是该文件成文前的旧值，现行口径按 design 为「S 项 **17** 条 / 旧原文串 **14** / 新原文串 **18**」，由 implement 相位在勾选时同步。
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/module-api-contract-format/explore.md`（1803 行，10 章：定位 / 资产形态 / 关键字集 / 引用与命名 / 类型映射 / 错误面 / 留白与门禁 / 存量实测 / 完整示例 / 待决与边界。`§10.4` 已定 7 条、`§10.3` 列 6 条真未决、`§10.5` 列明确不做 4 条）
> **范围接缝**: 本变更只做**契约资产的格式与同步机制**（手写 `<FQN>.md` + 四类生成物 + 十条判据 + 命令行生成器 + `api` 元素字段），MUST NOT 做：存量代码债务消解（`§10.5`）、组件 API（本期留白）、基于 API 生成单测、泛型支持、`openspec/specs/**` 的 `## Module Contract` 改写（`§10.1` 双写过渡）。
> **前置依赖**: 全部在位——`crates/core/archi` 的模型解析 / 图 / DTO 投影 / `scan/`（`syn` + `oxc` 已直接依赖）/ `drift/` 报告结构；`foundation::layout` 的 `archi_dir` 族；`crates/tools/export-bindings`（工具 crate 先例）；`desktop-archi-gate` 的 `ArchiCheckRunner`（消费报告结构，本变更不新增 step 与 port）。
> **模型同步（范围硬约束）**: 本变更新增 workspace crate `crates/tools/archi-api-gen` ⇒ 按 `repo-archi-model`「漂移纪律」（`:90`）与双向覆盖判据（`:91`），MUST 在同一 change 内追加元素 `App.Backend.Tools.ArchiApiGen` 与 2 条 `[dependencies]` 边，并把全基线计数同步为 **51 元素 / 113 关系（后端 49 + 前端 64）**、`05` 段 **3 元素 / 3 关系**。计数复述三处（模型表头 / 本提案 / ADR `§9`）同值。

---

## 问题

### 现象一：46 份 spec 的 `## Module Contract` 是散文，机器读不懂

仓库每个能力 spec 都有一张 `## Module Contract` 表（模块 / 职责 / 关键契约），三列、文件级粒度、零签名。它对人有用，对机器无用：没有一条判据能回答「这个模块承诺了哪些操作」「这份签名和代码还一致吗」。今天「模块的公共 API 是什么」只能靠人读 `pub` 面猜，而 `core/archi` 一处就有 123 个对外可达 `pub` 项（含纯内部工序）——`§8.7` 实测四种可见性策略并存，**没有任何 spec 约束这一层**。

### 现象二：接口与代码的同步靠纪律，纪律必然失效

`AGENT.md` / `CLAUDE.md` 是现成的反例：同源复制，`diff` 8 行后标题已分化（`§2.5`）。任何「两份手写文件靠约定配对」的方案都会重演这一幕。因此本变更不接受「契约与代码各写一份、靠 review 保持一致」，只接受**机械闭环**：

```
契约（唯一手写真相）→ 生成物 → 编译器 → 零 diff 门
```

### 现象三：格式若绑单一语言，接管其他项目那天就得重写

桌面端最终要接管自己与其他项目的开发工作（用户一以贯之的目标）。`crate` 是 Rust 工具链概念，架构层 MUST NOT 依赖它（`dto.rs` 的 `ElementKind` 已无 `Lang` 字段，原则已被遵守）。契约格式一旦写成「Rust 的 `pub` 面投影」，`§8.1` 的 330 类型 / 349 函数实测就只是一次性快照，换语言即归零。

### 症结一：先定「谁是真相」，否则派生面会变成第二真相

`config.schema.json` 是手维护静态镜像，靠双向一致性测试强制（`AGENT.md` 明文记为已知负担）。本变更的 `<FQN>.schema.json` 必须走相反的路：**给 schema 一个专门的上游真相**（`<FQN>.md`），使它可被生成、零手改自由度。由此提炼的判据（`§2.9`）：

> 投影可以安全替代原始真相，当且仅当「投影 ← 真相」方向全部由机械门禁守住。

缺这条，`apiUndeclared` 一类判据就得并读 markdown，成本翻倍且判据会二次漂移。

### 症结二：两侧编译器验的不是同一件事，必须显式留痕

Rust 侧 `impl` 必须匹配生成签名（强）；TS 侧结构化类型下 `implements` 可选（弱——抽象承诺强制不了，具体形状可以强制）。若把两侧写成「对等」而不标注差异，读 spec 的人会以为 TS 侧也有 Rust 级保证。降级必须记 `x-common.degraded`，而非默默弱一档。

### 症结三：判据与既有 `ArchiDriftKind` 混装会让 `match` 分支被迫处理不相关变体

`ArchiDriftKind` 是「模型 vs 代码」的漂移，契约判据是「契约 vs 代码」的漂移——事实源不同（relation 图 vs 契约文件）、定级语义不同（元素 `gate` vs 判据自带 `severity()`）、扩展节奏不同。混成一个 14 值枚举要动既有 corpus 金样、前端 `driftMarks` 与全部 `match` 臂（`§7.4`）。

### 症结四：完备性判据必须后置，否则迁移期立刻红

「代码每个导出都要有契约」是**完备性判据**（报的是「海」）；「契约说 A、代码是 B」是**一致性判据**（报的是「债」）。存量 27 个元素连入口都没有（`§2.10` 实测清单），一次性全开完备性判据等于让门禁永久红——于是「存量债务不消解」的前提直接失效。分界线须写死：一致性判据可即开，完备性判据（唯一一条 `apiUndeclared`）必须后置。

### 症结五：explore 的期望数与判据优先级不自洽

`§2.10` 表写 `apiMissing` 期望 40（= 41 持 `path` 元素 − 1 试点），但 `§2.6` 说 `apiEntryMissing`「在契约生成之前就失效」——若两条判据对同一元素同时报，报告数无法单调对拍（消解一条减一）。本提案正面裁定**互斥优先级**（见「提案」§7）并重算期望数（**`apiEntryMissing` 28 / `apiMissing` 13**，而非 explore 表的 27 / 40），把数字口径写进验收标准，避免实现期各自取一套。

## 提案

给「模块 = 架构元素」配一份**手写唯一真相 + 四类生成物 + 十条判据 + 一个命令行生成器**的闭环。手写面只有 `<FQN>.md` 一份；`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 全是它或代码的机械投影，全部 commit、全部进零 diff 门。

```
<archi_dir>/api/<Domain>/<FQN>.md           手写唯一真相（职能散文 + 分段的 fenced json）
<archi_dir>/api/<Domain>/<FQN>.schema.json  生成：json schema 投影（工具/校验器消费）
<archi_dir>/api/INDEX.toml                  生成：状态 / 类型数 / 操作数 / 覆盖率 / 降级计数

Rust 元素（<root> = 元素根路径「无后缀」，<name> = <root> 的末段——`crates/infra/archi` → `archi`）
<root>/src/lib.rs | <root>/mod.rs | <root>.rs   实现入口（按序查找）
<root>.i.rs                                     生成：类型 / trait / 枚举 + 自动构造器
<root>/**/*.rs                                  手写：impl 块与实现体

TS 元素
<root>/index.ts | <root>/index.tsx | <root>.ts | <root>.tsx   实现入口（按序查找）
<root>.i.ts                                                   生成：类型别名 + interface
<root>/**/*.ts(x)                                             手写：import type 引用 .i.ts

verify ≡ generate（`--check` 零写入、工具内比对即零 diff 判定）+ cargo check / tsc --noEmit
```

### 1. 资产形态：合写一份 md，fenced json 承载协议

`<FQN>.md` 是唯一手写资产，结构约定封闭：

| 位置 | 承载 |
|---|---|
| H1 标题 | `<FQN>`（与文件名一致，作锚点校验） |
| `## 职能` 等二维标题 | 散文区（人审主体：职能 / 边界 / 不变量 / 承载理由） |
| `### <op-id>` + 一个语言标为 `api-op` 的 json 代码块 | 操作锚点与签名（代码块归属于其前 heading，块内不自包含 op 名） |
| 语言标为 `api-types` 的 json 代码块 | 本模块自有 `$defs` 定义 |
| 语言标为 `api-reexport` 的 json 代码块 | 透传清单（键 = 导出名，值 = 直接子元素 FQN） |

三个载体方案实测后取 C（md 为主 + fenced json）：A（JSON 内嵌 `description`）散文是一整行超长字符串，改一个词整行替换；B（YAML block scalar）踩到致命陷阱——YAML 1.1 把 `off` / `on` / `yes` / `no` 解析成布尔，而 `off` 恰是本仓 `GateTier` 的真实枚举值，PyYAML 与 serde_yaml 版本不一致会**破坏跨语言一致性**，与「语言无关」正面对撞，否决。C 的实测性质：散文逐行 diff、一 op 一个 fence（diff 粒度 = op）、fence 内是标准 JSON 零歧义、md 由编辑器原生渲染。

**双向 gate（可机械校验）**：`### <op-id>` heading 集合 **≡** `api-op` fence 归属集合；`api-types` fence 的键集 ⊆ `.schema.json` 的 `$defs` 键集；`api-reexport` fence 键集 **≡** 散文透传列表。任一不成立即契约错误。

### 2. 两层结构：`<FQN>.schema.json` 与 `INDEX.toml` 都是纯派生

`schema.json` 的每一项都有 markdown 出处（`§2.9` 逐字段已验证，无「只有手改才能表达」的内容）：`$schema` / `$id` 常量 + FQN、`title` 取 H1、`$defs` 取 `api-types` + `api-reexport` 透传 alias、`properties` 取 op 键 + `x-function`、类型项取 `$defs` 键集。生成器只做**装配**（拼 `$defs` + `properties` + `$id` + 常量），不做**翻译**——因为 `api-types` / `api-op` 的 fence 内容本身就是原生 JSON Schema（`minLength` / `pattern` / `enum` / `anyOf` / `x-*` 全都在 md 内可表达）。

- **不可手改**：生成物头部带 `$comment` / 首行注释警示（沿 `bindings.ts` / `bindings.zod.ts` 既有惯例）。手改已提交 → 零 diff 门持久失败（响）；手改未提交 → 下次 generate 静默覆盖（哑，与今日 `bindings.ts` 同款已知行为，见风险 R-1）。
- **必须提交**：零 diff 门需要 git 基线；工具可离线消费；审计型 review 可 `git diff --stat <archi>/api/` 一眼看全。
- **review 口径**：每次改契约产生两份 diff（`.md` + `.schema.json`），因 schema 零手改自由度，**review 只读 `.md`**，schema diff 视为机械噪声（写进 `INDEX.toml` 的说明段）。
- **目录统一（不跟代码走）**：审计型问题（「这批改动动了哪些模块的契约」）只有统一目录能答，且无替代物；伴随型问题（「我改的这行契约同步了吗」）已由门禁机械回答，不需要文件相邻。代价（明确留痕）：在 crate 里读代码时看不到契约文件，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。
- **路径派生**：`<archi_dir>` 经 `foundation::layout::archi_dir`；新增 `archi_api_dir(root) → <archi_dir>/api`（与服务绝对路径族同模式）与 `archi_api_rel_dir()`（相对族）。`<Domain>` 分组名取元素**父元素 FQN 末段**（`App.Backend.Infra.ArchiRuntime` → `Infra`）；`$ref` 的相对路径（同目录 `./` / 跨目录 `../`）由单点函数产出，MUST NOT 手算。

### 3. 引用与命名：身份靠名字，去重靠名字

| # | 规则 | 违反后果 |
|---|---|---|
| 1 | 身份 = `$defs` 键名，不是 pointer 路径 | 同名两处定义 → 契约错误 |
| 2 | 每个具名类型在顶层 `$defs` 定义恰一次；嵌套 `$defs` 一律提升、**原名保留** | 提升名碰撞 → 契约错误，禁静默改名 |
| 3 | 所有 `$ref` target 必须是 `$defs` 顶层条目 | 指向 `properties` / `items` / `allOf/0` → 契约错误 |
| 4 | 多 `$ref` 指向同一类型时只定义一次 | 重复定义 → 契约错误 |
| 5 | **严禁按结构去重** | 17 组同形不同义（`ArchiDependsOn` vs `ArchiDependedBy` 是出入边，语义相反）合并即语义错 |

公共面 = **第一层 `properties` + 第一层经 `$ref` 可达的闭包**；此约定自动排除不出现在任何签名里的纯内部类型。`$ref` 的指向天然表达契约性质：对定义它的模块是「我提供的类型」，对引用它的模块是「我要求外部提供的契约」——跨模块 `$ref` 无需任何标记。

### 4. 关键字集：只有两个扩展构造

| 关键字 | 语义 |
|---|---|
| 标准关键字 | `object` / `enum` / `const` / `default` / `oneOf` / `$ref` |
| `x-class` | 声明为 trait（仅允许方法；出现属性即契约错误） |
| `x-function` | 函数的 `params` / `returns` / `async` |
| `x-common` | 自由注记（`degraded` / `unit` / `note` / `since` / `owner`），不驱动生成、不加强校验 |

- `x-function` 不可删：JSON Schema 无表达函数的标准方式，且它是唯一能区分「模块操作 `pub fn parse(&str)`」与「函数指针字段 `pub parse: fn(&str) -> _`」的标记。仓库当前 0 例函数指针字段，接管其他项目时必然出现。
- `x-class` 不可省：形状推导（「properties 全是函数 → trait」）无法区分「本意 trait」与「本意函数指针结构体」，会静默误判。反向补一条**警告**（非错误）：`properties` 全为函数却未标 `x-class` → 提示可能意图为 trait。
- 标量不生成 trait 而生成 newtype（`pub struct TimestampMs(pub i64)`）——trait 的名义性为零（任何类型都能 impl）。`§8.6` 实测 `i64`/`u64` 字段 162 处，时间戳簇 62 处、db id 18+ 处，名义化的真实价值在此；这些 newtype 放**各模块 `$defs`**，不进 `common.json`。
- 不用 Zod：它是 TS 生态产物，当通用载体等于把「语言无关」偷换成「TS 优先」。语言无关的注解载体是 JSON Schema 原生机制，且规范要求校验器 MUST 忽略未知关键字，故 `x-*` 不破坏生态工具。

### 5. 语言映射：位置分派是生成器内部表，不是门禁

| 协议 | Rust | TS |
|---|---|---|
| `object`（具名） | `struct` | `export interface X { … }` |
| `enum` | `enum`（派生 `Deserialize`；需列举时声明 `all() -> list<LocalEnum>`，函数体由枚举派生） | `export const X = [...] as const` + `export type X = (typeof X)[number]` |
| `oneOf` / `anyOf` | 带负载 `enum`（`#[serde(tag=…)]` 或 `untagged`） | union 类型 |
| `x-class` | `pub trait` | `export interface`（不强制实现 → 记 `degraded`） |
| 标量 | `newtype struct` | 别名（**按设计退化，不记 degraded**） |
| 非 `required` 字段 | `Option<T>` | `T \| null` |
| `x-function` | `pub fn`（入参位 `&str` / `&[T]`，返回位 `String` / `Vec<T>`） | `(…) => R` / `Promise<R>`（`async`） |
| 错误面（`Outcome` 信封） | `Result<T, String>`（std 类型，不生成新类型） | `{ ok: T } \| { err: string }`，或 `Promise` reject（**不在类型系统内** → 记 `degraded`） |

- **位置分派**（入参位 `string` → `&str` 97%、返回位 → `String` 96%、字段位 → `String` 99.6%、入参位 `list<T>` → `&[T]` 94%）是生成器的**内部表**，MUST NOT 变成对外门禁；**位置词汇（`Position` 三值）与签名短形写出点 MUST 各只有一份**，判据侧只经 `schema_type` + `signature_of` 取数、MUST NOT 自建第二份（否则可选参数与 `Option<T>` 的归一必然漏分支、稳定假阳），`emit` 仍独占「位置 × 语言 → 文本形」的映射。偏离携带语义（入参位那 3 例 `String` 全是存储型），须在契约登记理由。
- **泛型明确不支持**：Rust 侧零泛型公开面、TS 侧无真泛型组件；500 模块规模下会让协议退化成需要类型推导。
- **构造器由生成器自动产出，不进契约**：`§8.2` 实测 349 `pub fn` − 51 构造器 = 298 真实操作，构造器是类型的推论。契约 `### 操作` MUST NOT 声明构造器（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`），违反即 `apiConstructorDeclared`。试点纸面验证（`§10.2.1`）首版草案就因为写了 `### new` 被 `apiMissing` 误报，这是本规则的直接来源。
- **TS 不做品牌类型**：`number & { __brand }` 会让 cast 传染整条调用链（`JSON.parse` 出裸 `number`），收益只是防误传；TS 侧「实质同类型即可」。
- **契约文本进生成器经两道人字闸**：闸一（装载期 `contract::verify`，语言无关）判**七族名字**是 ASCII 标识符——op id / `$defs` 键 / 透传键 / `params[].name` / `$defs` 条目的 `properties` 键 / `api-op` 的 `owner` / `$ref` 末段名，不成立即 `ApiError::Contract` 并点名族别与 fence 行；同趟另判 **`x-function` 落点封闭二处**（`api-op` 顶层签名、`$defs` 条目第一层 `properties` 的值），其余位置即契约错误并点名 JSON pointer。闸二（渲染期，语言特定）一切来自契约的**字符串字面量与注释** MUST 经该语言单点转义（`ts_string_literal` / `ts_comment_text` / `rust_comment_text`；单引号形态不变，MUST NOT 用双引号形态，Rust 枚举值走 `variant_name` 无损归一并写成不变量）。渲染期 MUST NOT 重复校验名字或位置（二次派生）。

### 6. 生成物落点与入口查找

- **生成物放根同级**（`<root>.i.rs` / `<root>.i.ts`），不强制模块成目录：规则只有一条、12 个前端单文件元素零改动。代价留痕：目录形态的引用路径带 `../`，生成物与模块文件在 git 视角不聚拢。
- **归属需一条剥离特例**：`owner.rs` 的 `prefix_matches` 判据是 `rel == prefix || rel.startsWith(prefix + "/")`，故 `<root>.i.rs` 不匹配 `<root>`；解法是归属规则加一条「`.i.<ext>` 生成物按去掉 `.i` 后的路径判定」（四种形态已实测正确）。
- **入口查找顺序封闭**：Rust `<dir>/src/lib.rs` → `<dir>/mod.rs` → `<dir>.rs`（`<dir>/lib.rs` 不在列表——crate 的 `lib.rs` 恒在 `src/` 下）；前端 `<dir>/index.ts` → `index.tsx` → `<dir>.ts` → `<dir>.tsx`。
- **`<root>` 无后缀是硬约束**：无 `./` 前缀、无尾 `/`、无文件扩展名。`<root>` 是**模块身份**不是文件路径，同时决定生成物落点、实现入口位置与 `#[path]` 写法。归一在模型解析层完成（`OwnerIndex::normalize` 已处理），**不改存量 models**。
- **不做 `_ext.rs`**：生成物只含类型定义与自动构造器，`impl` 块写在模块内任意手写文件（Rust 允许同 crate 内 `impl` 分散）；生成/手写分离靠文件命名（`.i.rs` 后缀）。
- **入口编排（实测编译通过）**：`#[path = "<相对路径>"] mod api_gen; pub use api_gen::*;` + 既有 `pub mod` 声明不变。`#[path]` 的**值**是**字面量**（属性值在 Rust 里无法由函数产出），且 MUST 与 `paths::mod_path_attr` 的输出逐字相等——该函数是等值判据的唯一真值源（三形态：crate 形态 `"../../<name>.i.rs"` / 子目录模块 `"../<name>.i.rs"` / 单文件形态 `"<name>.i.rs"`），由 `paths_test` 三形态断言与逐入口自查行双守（design D38 / spec S-11）；目标是恒指向**根同级**的 `<root>.i.rs`，手写「入口同级」会错位到 `<dir>/<name>.i.rs`。仓库现状 12 个子目录模块全部 `mod.rs` 形态。与既有同名类型冲突时编译器直接报 E0255，不会静默覆盖。
- **`#[path]` 的读盘面与写盘面同一条限根纪律**（design D49 / spec S-16）：取数器定位 `#[path = "…"] mod` 时 MUST ①在折叠之前归一反斜杠、②按段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）、③`join` 后复核目标仍在观察根之下（`starts_with(root)`）；任一条不成立即不命中该子树（零新错误变体、不 panic）。理由是 `#[path]` 的值经手写入口行落地，属可被写错的入参。
- **末段进目标语言源码的形态闸按语言分列**（design D48 / D50 / spec S-17）：`module_name` 是纯派生零形态判据；Rust 侧由 `paths::is_module_segment` 判「形态 `[A-Za-z_][A-Za-z0-9_]*` 且非空 **且非 Rust 关键字**」（关键字集由一个 `const` 封闭持有；放行关键字即产出 `pub use crate::loop::X;` 这种不可编译文本，而产出 `r#loop` 是第二形态、MUST NOT），不成立即令该透传目标不入查表、渲染器只出 `// 透传自 <FQN>` 注释（零改名 / 零清洗）；TS 侧由 `ts_string_literal` 转义、零筛选。`checked_root` 只判整条 `<root>` 的段结构、**不判末段字符集**。

### 7. 判据：十条，一次性判据一律 `warn`、唯一完备性判据 `off`

新增平行封闭枚举 `ArchiApiFindingKind`（**10 值**，与 `ArchiDriftKind` 分立），条目结构 `ArchiApiFinding { kind, category, element, target, message, occurrences }`，报告新增并列槽位 `apiFindings`。

| 判据 | 方向 | 判据级档位 |
|---|---|---|
| `apiEntryMissing` | 元素根路径下既无生成物也无实现入口 | `warn` |
| `apiMissing` | 契约 → 代码（`declared` 但契约文件缺失 / 声明的 op 与类型在代码里没有） | `warn` |
| `apiUndeclared` | 代码 → 契约（有 external 面未声明） | **`off`**（唯一完备性判据） |
| `apiSignatureMismatch` | 双向 | `warn` |
| `apiUnexpected` | 留白 ↔ 文件矛盾（`api = none` 而契约文件存在；反方向归 `apiMissing`，两条不双报） | `warn` |
| `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空 | `warn` |
| `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 未 declared / 非直接子元素 | `warn` |
| `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate | `warn` |
| `apiReexportDrift` | markdown 散文列表与 `api-reexport` fence 键集不等 | `warn` |
| `apiConstructorDeclared` | 契约 `### 操作` 声明了构造器 | `warn` |

三条裁决写死：

1. **判据级档位 ≠ 元素 `gate`**：`severity()` 由 `ArchiApiFindingKind` 自带，元素 `gate` 全 `off` 不动。理由：`ArchiDriftKind` 的定级看元素 `gate`，契约判据的收紧节奏与元素 `gate` 必须解耦（否则收紧契约要改模型，两套语料同时动）。
2. **首期 `apiFindings` 不影响 `status` / `passed`**：与 `observations` 同款（「永不影响 status」的既有先例）。若 28 + 13 条 warn 立刻进 `status`，门禁当场变红，违背「不消解债务」前提。将来收紧由 `severity()` 决定。
3. **求值顺序互斥（本提案正面裁定 `§2.10` 与 `§2.6` 的不自洽）**：先判 `apiEntryMissing`；命中则该元素**不再**参与 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`（结构上不可判），`apiUnexpected` 仍独立判。因此本期期望数 = **`apiEntryMissing` 28 / `apiMissing` 13 / 其余 0 / `apiUndeclared` 不适用**，而非 explore 表的 27 / 40。两处差异的解释：① 新增 crate `archi-api-gen` 仅 `main.rs`、无 `lib.rs` 入口 ⇒ entry-missing +1（**不消解**，与 `ExportBindings` 同形）；② 互斥优先级把 27 条无入口元素从 `apiMissing` 里剔除（它们连「契约文件是否存在」都无意义）。数字口径以本提案为准（验收表 AC-7 逐项对拍），并已按本仓实测复核（`<dir>/src/lib.rs` 命中 14 个 crate 元素、其余三分支 / 前端四分支零命中）。

4. **契约面启用条件（冷启动纪律）**：`<archi_api_dir>` 缺席或零份 `*.md` ⇒ 契约面未启用，`apiFindings = []` 且既有漂移语料与报告逐字不变；目录一旦存在（本变更落地即存在，试点契约在内）则**全部 `path` 非空元素**（本仓 42 个）重新纳入判据，无白名单、无开关、无「部分元素豁免」。缺这条，既有金样「仅追加空槽位」的兼容承诺与迁移期 28 + 13 条实测数会互相冲突。

`category` 是**开放值集字符串**（无 `ALL`），用于报告分组（`entry` / `contract` / `signature` / `reexport` / `nesting` …）：加一个分类只改打标处，不动枚举、不动 `ALL`、不动金样词汇对拍。

**两个考虑过但删除的判据**：`apiChildToParent`（由既有 `UndeclaredDependency` 覆盖）、`apiLeakedDependency`（契约不是依赖事实源）。判据建在事实源（代码）上，不在投影（契约）上；新增判据只在「契约自身内部矛盾」时成立。

### 8. 模块嵌套与透传：父的 API 面 ≠ 子元素面的并集

真实案例已存在（9 对父子元素全在前端）：`App.Frontend.Components.Agent` 有 6 个子元素，barrel 只透传 2 个（`AgentInput` / `AgentSessionView`），且还导出 `SessionPanel`（该文件不是元素）。故：

| 来源 | 契约表达 | 生成 |
|---|---|---|
| 透传自子元素 | `api-reexport` fence + `$defs` 纯 `$ref` 条目 | `pub use` / `export { X } from` |
| 自有（非元素文件） | `$defs` 内本地定义 | `struct` / `interface` |
| 子元素但不导出 | **不出现** | — |

六条规则：透传 SHALL 同时表达于 `api-reexport` fence 与 `schema.json` 的纯 `$ref` 条目且两处键集相等；透传条目 MUST NOT 携带结构关键字（纯 `$ref` alias）；父子 `$defs` 类型名交集必须为空（**不能依赖编译器**——glob 形态的重复定义只给 warning，实测）；透传目标与跨模块 `$ref` 必须是**直接子元素**（FQN 前缀相邻）且契约存在且 `api = declared`；嵌套元素父子 MUST 同属一个 crate（跨 crate 即 cargo `cyclic package dependency`）；子元素 MUST NOT 依赖父元素（共享类型须下沉兄弟元素或消解，代码层由既有 `UndeclaredDependency` 报）。

**禁断子→父的最大价值不在禁断本身**，而在它让「把子目录提升为元素」变成一次真实的解耦决策——`infra/agent` 要把 `sdk/` 提升为元素，先得把 3 个类型从 `lib.rs` 挪出来（本期不消解）。

**契约记录「提供什么」，不记录「实现了什么」**：`implements` 注记已被否决（重复表达 model relation、污染契约语义、判据被编译器与 `UndeclaredDependency` 双重覆盖）。推论：三个 runner（`LocalArchiCheck` / `ProcessTestExecution` / `ProcessStaticCheck`）的契约面就是 `new()`——契约如实记录不是缺陷，是代码形状的事实。策略是**契约变薄时改代码，不改格式**（为契约有效性暴露已存在的私有逻辑，只改可见性 + 挪进 `impl`，不动实现）。这与「存量债务不消解」不矛盾：前者禁「格式追代码」，后者是「代码追上契约应有内容」。

### 9. bootstrap：从存量代码生成草稿，人工/agent 剪枝

抽取器按语言分派，两个语言的可达性语义不同：

- **Rust（`syn`，已是 `core/archi` 直接依赖）**：`pub struct` / `pub trait` / `pub enum` → `$defs`；`pub fn`（自由函数 + `impl` 方法）→ `x-function`；签名**必须走 AST**（多行签名占 `pub fn` 的 30–62%：`core/checks` 13/21、`core/archi` 16/27，`grep` 单行抽取会截半）。
- **TS（`oxc`）**：`scan/ts.rs` 现只抽 import specifier，bootstrap 需**新增声明抽取**——同 AST、同 crate，零新依赖。关键实测结论：`export const` 里**零个箭头函数**（0/132），故**只有 `export function` 是操作**；`export const = memo(...)` → 组件（本期 `api = none`）；`export type` / `export interface` → `$defs`；`export {…}` / `export * from` 仓库实测 0 例。
- **`visibility` 封闭四值**（bootstrap 剪枝的机械判据）：`external`（跨元素可达）/ `crateOnly`（仅同 crate 或同元素目录内）/ `unused`（全仓无引用者，**伪公共首选删除对象**）/ `testOnly`（仅测试树引用，不进 API）。**词表封闭 ≠ 两语言都判得出四值，更 ≠ 本期生产面都产得出**：本期可判值域按语言分列（design D19-b）——Rust 侧四值可判（`pub mod` 链 / `pub(crate)` / 私有 `mod` 内的 `pub` / `#[cfg(test)]` 树），TS 侧**生产面只产 `external`**（有 `export`）；两语言的 `unused` 都由「调用方按引用事实降级」产生、两个取数器都不产（`rust.rs` 零 `Unused`，`ts.rs` 的唯一产出点在被零调用的 `apply_knip_report` 内），而该调用方本期在产品面不存在 ⇒ `Visibility::Unused` 生产面不可达；`unused` 的将来判据面（**接线后**）才声明为「`knip` 报告未使用 → `unused`」（spec 侧措辞由 S-7 / S-8 承接，第十二稿已把两处从「本期读 knip」收紧为「本期不读」），**该降级接口本期不接线**（design D39、spec 侧 S-12）：`apply_knip_report` 已实现且**零产品调用者**，`archi-api-gen` 与 `api::bootstrap` 均不读 `knip` ⇒ `Visibility::Unused` 在**生产面不可达**，这是**显式留白**而非疏漏（`ts_test` 五条用例继续测接口正确性，接线随首个 TS bootstrap 变更）；`crateOnly` / `testOnly` **本期留白**（判据需跨文件 import 反向事实，取数器只吃单文件 AST）。不为 TS 造第二套死代码扫描（`knip` 若接线则复用其报告，`client:check` 已含）。留白 MUST NOT 被「文件是否落在元素目录内」或「文件名含 `.test.`」冒充（伪判据，`.spec.*` / `__tests__/**` / `fixtures/**` 同禁），`used_by` 同源留白（需 import 反向图）。

**试点元素 `App.Backend.Infra.ArchiRuntime`**（14 个 crate 元素里公开面最小的 2 项，形态齐全：`pub struct` + `pub fn` 构造器 + trait impl + `pub(crate) fn`），选它以让格式问题不被数量淹没。纸面验证已跑完：删掉 `### new` 段后 `apiMissing` 归零、双向 gate 可机械执行、生成的 `.i.rs` 与手写 `impl Default` 共存编译通过（`rustc --edition 2021`）、`schema.json` 在 ajv（draft 2020-12 + `strict: "log"`）下编译通过。试点要回答的三个原问题已有结论：`BoxToolFuture` 类型别名不需表达（`ArchiCheckRunner::run` 的签名归 `Core.Orchestration` 的契约）、`impl ArchiCheckRunner` 是跨元素依赖进 relation 不进契约、`pub(crate) fn write_report` 不进契约。

- **试点 op 取 `file_stem(timestamp: &str) -> String`，不取 `compose_text`**（design D23）：`compose_text(&ArchiDriftReport, Option<&str>)` 的入参类型由 `App.Backend.Core.Archi` 提供，而跨模块 `$ref` MUST 指向**直接子元素**（本能力「模块嵌套与选择性透传」规则 4）；`App.Backend.Infra.ArchiRuntime` 与 `App.Backend.Core.Archi` 既非父子也非兄弟，契约无法表达该签名而不违反规则 4（写成内联 object 即重复定义他模块类型，语义错）。`file_stem` 纯标量、零跨模块引用，且 `report_test` 已在测其行为，是该元素内唯一同时满足「真实操作、标量签名、已在测试覆盖」的候选；代码形状处置是把 `report.rs` 的私有自由函数挪进 `runner.rs` 的 `impl LocalArchiCheck` 并提升为 `pub`（实现体逐字照搬）。`compose_text` **保持私有、不进契约**（`visibility = crateOnly`）。

### 10. 工程落位：判定在 `archi`，写盘在工具 crate

- **`crates/core/archi` 新增 `api/` 模块族**（纯读、零写盘、零 spawn，沿用既有纪律）：契约 md 解析（fence 提取 + 双向 gate）、`schema.json` / `INDEX.toml` 装配、`x-*` 校验与 `$defs` 规则检查、Rust / TS 声明抽取（bootstrap）、十条判据求值。`syn` / `oxc_*` / `sha2` / `globset` 均已在 `Cargo.lock` 且已是该 crate 直接依赖 ⇒ **零新增外部依赖**。
- **新建 `crates/tools/archi-api-gen`**（裸名 `archi-api-gen`，唯一写盘面）：CLI 三子命令 `generate`（写四类生成物）/ `--check`（生成到内存后比对，**零写入**，差异即非零退出）/ `bootstrap`（输出候选草稿到 stdout，供人工或 agent 剪枝）。与 `export-bindings` 同族：工具 crate、仅经 `cargo run -p` 驱动、不被 app 构建拖带。允许写盘是**工具边界**的正当属性（`export-bindings` 同理），`archi` 的零写盘纪律不动。
- **`foundation::layout` 增 2 个派生函数**：`archi_api_dir(root)`（服务绝对路径族）与 `archi_api_rel_dir()`（相对族，供 `INDEX.toml` 说明段与知识文本）。
- **`package.json` 增 2 脚本**：`api:export`（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`）、`api:check`（同一调用加 `--check`，生成到内存后比对，**零写入**，差异即非零退出），并挂进既有 `check` 链。`check` 链的第二段是 **`bindings:export`**（`package.json` 的 `scripts` 内只有 `bindings:export` / `bindings:zod-check` 两个 `bindings:` 键，生成物零 diff 守卫已退役）：`check` = `server:check && bindings:export && api:check && client:check`。**同一行还须改 fmt 段**：`server:check` 由写模式 `cargo fmt --all` 改为校验模式 **`cargo fmt --all --check`**（clippy 段逐字不动，design D28）——写模式门每次都重写工作树，使「提交态 = 不动点」在数学上不收敛（实测连续 6 次失败）；只报不写后树一旦处于格式化态即永久稳定。该段是 delta spec 三处（requirement 正文 / Scenario「check 链扩一段且 fmt 段改校验模式」/ Module Contract 表 `package.json` 行）已落地的强制要求，本提案 MUST 与之一致。
- **生成物 SHALL 是格式化器不动点**（design D29）：`.i.rs` 的输出 MUST 是 `rustfmt --edition 2021` 的不动点（实测试点 `crates/infra/archi.i.rs`：`rustfmt --check` exit 0），`.i.ts` 的输出 MUST 是 `vp fmt` 的不动点（prettier 会把 `["a", "b"]` 归一为 `['a', 'b']` ⇒ 生成器一律用单引号）。落地方式是把格式化器的正常形**内建进渲染模板**、由任务自查行的 `--check` 判据兜住，**MUST NOT 在生成器里 spawn 格式化器**（保 `archi` 零 spawn 与工具零子进程红线），也 MUST NOT 为此给 `package.json` 加第二段守卫（`api:check` 保持单条）。理由与 fmt 段同源：不动点不成立时 `api:check` 的字节零 diff 门会与 `client:check` 的 `vp check --fix` **永久对撞**。
- **`knip.json` 的 `ignore` 增 `src/**/*.i.ts`**：实测未被 import 的 `.i.ts` 会被报为 unused file（`project` glob 覆盖 `src/**/*.ts`），与既有 `src/types/generated/**` 同处理；MUST NOT 新增任何 `ignoreExportsUsedInFile` / `ignoreDependencies` 条目（那是关判据而非满足判据）。
- **TS 侧形状校验**：`.i.ts` 生成接口，实现侧 `import type` 引用并标注返回类型，形状不符即 `tsc` 报 TS2322（已实测）。`isolatedModules` 下重导出须写 `export type { … }`。

### 11. 本仓模型同步（新 crate 的漂移纪律）

| 项 | 现值 | 变更后 | 落点 |
|---|---|---|---|
| 元素总数 | 50 | **51** | 新增 `App.Backend.Tools.ArchiApiGen`（`kind = "module"`、`gate = "off"`、`path = ["./packages/desktop/src-tauri/crates/tools/archi-api-gen/"]`、中文 `summary`） |
| `05-backend-tools.toml` | 2 元素 / 1 关系 | **3 元素 / 3 关系** | 元素 + 2 条 `[[relation]]`（→ `Core.Archi` 契约解析与判据 / → `Core.Foundation` 路径派生） |
| 关系总数 | 111（后端 47 + 前端 64） | **113（后端 49 + 前端 64）** | 前端零新增：`.i.ts` 是生成物，不建元素（沿 `types/generated/**` 排除先例） |

三条纪律：`gate` 档位仍全 `off`（契约判据自带 `severity()`，与元素 `gate` 解耦）；计数复述三处（模型表头 / `repo-archi-model` 计数表 / ADR `§9`）同值；新增元素 MUST NOT 顺手为 `specta-zod-patched` 补缺（既有已知缺口单列）。

## 能力

### 新增的能力

- **desktop-archi-api** — 契约资产与生成链：① 资产形态（`<FQN>.md` 唯一手写真相 + 三个 fenced json 位置语义 + 双向 gate、`schema.json` / `INDEX.toml` 纯派生不可手改、统一目录与 review 口径、`archi_api_dir` 派生）；② 引用与命名（`$defs` 身份规则五条、按结构去重禁令、公共面闭包、跨文件 `$ref` 相对路径与 draft 2020-12 / `strict: "log"` 消费前置）；③ 关键字集与类型映射（两扩展构造、`x-common` 自由注记、标量 newtype、位置分派为内部表但**位置词汇与签名短形各只有一份**（判据只调用 `schema_type` / `signature_of`，`emit` 独占「位置 × 语言 → 文本形」）、**契约文本进生成器经两道人字闸**（装载期七族名字的 ASCII 标识符判据 + `x-function` 落点封闭二处；渲染期字符串字面量与注释单点转义）、泛型不支持、构造器不进契约、错误面 `Outcome` 信封与 `degraded` 留痕、TS 无品牌类型）；④ 生成物落点与入口查找（`<root>.i.rs` / `<root>.i.ts` 根同级、`.i` 剥离归属特例、`#[path]` 读盘面与写盘面同一条限根纪律（反斜杠归一架在折叠之前 / 分段折叠 / `join` 后 `starts_with(root)` 复核）、**末段进源码的形态闸按语言分列**（Rust 侧 `paths::is_module_segment` 判「形态且非关键字」、TS 侧转义零筛选；`checked_root` 只判整条 `<root>` 不判末段字符集）、三分支 / 四分支入口顺序、`#[path]` 值为字面量且 MUST 与 `paths::mod_path_attr` 输出逐字相等（等值单点，S-11）/ TS import 路径由 `paths::ts_import_specifier` 单点产出、零 `_ext.rs`）；⑤ 生成物入仓与零 diff 门挂载（四类生成物齐入库、`api:export` / `api:check` 两脚本、`check` 链三段扩四段、`server:check` 的 fmt 段由写模式改校验模式 `cargo fmt --all --check`（写模式门使「提交态 = 不动点」不收敛）、生成物须为格式化器不动点（`.i.rs` = `rustfmt --check`、`.i.ts` = `vp fmt` 单引号，不 spawn 格式化器）、`knip` ignore 增 `src/**/*.i.ts`、`bindings:export` 再生成、版本 patch +1）；⑥ 生成器与 bootstrap（判定在 `archi` 零写盘、写盘在工具 crate、CLI 三子命令、`verify ≡ generate + 编译器 + 零 diff`、Rust `syn` / TS `oxc` 抽取、`visibility` 四值剪枝（生产面按语言分列，`unused` 降级接口本期未接线、`crateOnly` / `testOnly` 留白）、试点元素 `App.Backend.Infra.ArchiRuntime` 与已验结论——试点 op 为 `file_stem(timestamp: string) -> string`，`compose_text` 因入参类型属非直接子元素而不进契约（规则 4））；⑦ 模块嵌套与透传（选择性透传、三处落位等键集、六条结构规则、末段形态闸归 `paths::is_module_segment` 与 `ts_string_literal` 两处承接、递归透传与直接子元素约束、跨 crate 不可表达、禁断子→父、契约只记「提供什么」）；⑧ `api` 元素字段（显式字段封闭二值 `declared` / `none`、`ALL` + 拼写校验、完备性判据的定义）；⑨ TS 侧形状校验机制前提（实现侧 `import type` + `tsc` 报 TS2322、`export type` 重导出）；⑩ 语言无关纪律（架构层不依赖 crate / 语言概念，语言特定只落取数器与生成物投影）。
- **desktop-archi-api-check** — 判据与报告面：① `ArchiApiFindingKind` 封闭十值与 `ALL` / `as_str` 对拍、`category` 开放值集；② 十条判据的定义与方向（含 `apiUnexpected` 单向 / `apiMissing` 双向缺口的口径分工、`apiMissing` 的 op 与 `$defs` 类型**同口径**（同一条判据、同一次求值 + 三条排除面 + 两条结构前提）、`occurrences.line` 分列（op 取 heading 行、类型缺口取 `0`）、契约面启用条件——目录缺席或零契约即全量关停，一旦存在则全量纳入无白名单）；③ 判据级档位（一致性 9 条 `warn` / 完备性 1 条 `off`）与元素 `gate` 解耦；④ 求值顺序互斥（entry 优先，命中即不再判 missing / undeclared / signatureMismatch）；⑤ 报告并列槽位 `apiFindings`（既有 14 字段零改动、`version` 递增、首期不进 `status` / `passed`）；⑥ 判定单源（命令面与门禁面同源、契约只读 schema.json 的门禁前提）；⑦ 债务即验证语料（27→28 条清单、报告数逐项对拍、`warn` 报告即迁移 backlog；语料分两层——逐判据正反例由 `api/**` 十一份 sidecar 用例直驱 `check::check`，报告面沿既有 `DRIFT_CASES` + `corpus/drift/` 驱动：既有 15 份金样追加 `"apiFindings": []` 与 `"version": 2`，另增 `api-mismatch` 非空金样，MUST NOT 另建 `tests/corpus/api/` 第二套语料）。

### 修改的能力

- **desktop-archi-model** — **MODIFIED**「架构模型文件格式与字段封闭」：元素字段面新增显式 `api`（`declared` / `none`，缺省 `declared`；走 `deny_unknown_fields` 的拼写校验而非 `metadata` 自由键）；把「协议不经模型」的边界改写为「协议正文不经模型，模型只存**状态指针** `api`」并正面回答该边界要求的两个问题（契约权威 = `<archi_dir>/api/<FQN>.md`；不提两套提取器——单一 schema + 生成器，`scan/rust.rs` / `scan/ts.rs` 是既有取数器非权威，只供 bootstrap）。**MODIFIED**「消费面词汇集一致性对拍」：封闭词汇表新增 `api 状态 declared / none` 一面，`ApiStatus::ALL` 纳入 `ALL` 与线格式词逐值对拍，知识文本须含该词汇。**MODIFIED**「IPC 命令面契约（五命令）」：`archi_module` 行注明出参在既有投影之上并出 `api` 并列字段（定义归 `desktop-archi-api`），命令数量仍为五个。
- **desktop-archi-drift** — **MODIFIED**「报告结构与 status 五值」：报告新增并列槽位 `apiFindings`（`ArchiApiFinding[]`，空数组 = 无契约问题），既有 `errors` / `warnings` / `suppressed` / `observations` / `status` 字段面与语义逐字不变；`version` 递增并留兼容说明；封闭词汇组由三组扩为四组（新增 `ArchiApiFindingKind` 十值，定义归 `desktop-archi-api-check`），并明确「`apiFindings` 首期不影响 `status` / `passed`」。
- **desktop-archi-agent** — **MODIFIED**「archi_module 只读工具的定位契约」：单元素响应新增 `api` 并列字段（`status` / `contractPath` / `schemaPath` / `generatedPath`），按 `archi_api_dir + FQN` 约定定位、文件不存在即 `status = none` 且路径为 `null`（零配置、可枚举「有契约 / 无契约」）；省略 `element` 的薄概览同步带 `api.status`；工具仍只读。
- **workspace-layout-resolution** — **MODIFIED**「架构域路径派生（archi_dir 族）」：新增 `archi_api_dir(root) → <archi_dir>/api`（从 `archi_dir` 派生，唯一合法定位函数，消费者 MUST NOT 行内拼 `api` 段）。**MODIFIED**「架构域 repo 相对路径派生（会话知识用）」：新增 `archi_api_rel_dir()`，并把「唯一消费者 = 知识文本」扩为「知识文本 + 生成器 INDEX 说明段」。
- **repo-archi-model** — **MODIFIED**「基线落位与模型文件划分」（`05` 段 2→3 元素 / 1→3 关系、合计 50→51 元素 / 111→113 关系、自研 crate 15→16）、「后端下钻到 crate 层的元素覆盖」（元素清单新增 `App.Backend.Tools.ArchiApiGen`、自研 crate 集合 16、新增 crate 的同 change 补齐实例）、「后端关系只取 Cargo 依赖声明」（新增两条 `archi-api-gen` 出边、后端 47→49）、「图自洽与停摆禁令」（正常态计数 51 / 113）。


### 追加关注点的既有能力（ADDED）

- **desktop-crate-layout** — **ADDED**「工具 crate `archi-api-gen` 落位与契约面依赖扩展」：落 `crates/tools/archi-api-gen`（裸名 `archi-api-gen`）、workspace 内依赖恰为 `archi` + `foundation`、允许写盘（工具边界，与 `export-bindings` 同族）、零 Tauri / 零 store / 零 config / 零 orchestration / 零 `workflow`；`archi` 不新增外部依赖（`syn` / `oxc_*` / `sha2` / `globset` 已在锁内且已是直接依赖），零 spawn 与零 `workflow` 互不依赖两条红线不变；`archi-api-gen` MUST NOT 被 app / core / infra 依赖（只由命令驱动）。
- **desktop-archi-module-docs** — **ADDED**「契约资产不经 docs 三源」：`<archi_dir>/api/**` 的契约 MUST NOT 成为 `docs` 关联的第四条来源，MUST NOT 出现在 `element_docs` 的四字段条目里；反向也不许把 `openspec/specs/**` 或契约路径写进元素 `docs`。`archi_module` 的 `api` 字段是**并列字段**而非承重进 `docs`（沿用「关联来源恰三种」的封死口径）。

### 零改动能力与理由

- **desktop-archi-gate**：`passed` 定义（`errors.is_empty() && model.structuralErrors.is_empty()`）、反馈边上限、port 与报告槽位全部不动；`apiFindings` 既不进 `errors` / `warnings` 也不进 `passed`，门禁语义零变化。收紧契约判据的将来变更才动它。
- **desktop-archi-canvas**：`marks.ts` 的四值样式契约、`driftMarks` 只读 `observations` / `suppressed` / 停摆态全部不动；契约状态本期不上画布（`api` 状态经 `archi_module` 与门禁提示可见）。
- **desktop-ipc-type-bindings**：「生成物入库 + 提交前人工核对 + knip 边界」三条纪律被**沿用而非修改**——本变更的生成物是第二种生成物（`src/**/*.i.ts`）与第三种入仓产物（`.i.rs`、`archi/api/**`），形态同款；`bindings.ts` / `bindings.zod.ts` 因新增 `ApiStatus` / `ArchiApiRef` / `ArchiApiFinding*` / `apiFindings` 而**必然有 diff**（`bindings:export` 再生成后即最新，入库由提交前 `git status` / `git diff` 人工核对——零 diff 守卫早已退役，`check` 链只前置重导出、不做生成物比对）。
- **desktop-workspace-config**：契约 `api` 是**架构模型元素字段**不是工作区配置——`KNOWN_KEYS` 七项、`config.schema.json` 镜像、前端 config 页全零变化。
- **desktop-phase-prompts**：16 份角色 md 零改动——契约纪律经门禁报告与 `archi_module` 的 `api` 字段抵达 agent，不给 prompt 加格式说明（避免静态面与 spec 双写漂移）。
- **desktop-write-protection**：`<archi_dir>/api/**` 是模型同级资产，MUST NOT 进内置锁面（纳入即断「模型 = 源码」通路，与既有架构域裁定同源）；本期新增写面只有生成物，不构成锁面变更。
- **desktop-archi-module-docs**（除上述 ADDED）：三源关联、四字段契约、`docNotFound` 只报不拦、邻近扫描范围、看护与检索不读 specs 全部不动。
- **desktop-corpus-regression / desktop-change-* / desktop-test-execution / desktop-mutation-testing / desktop-build-modes / desktop-release-pipeline**：运行史语料、流程行为、执行链、阈值、构建与发版链零触碰（本变更加的是一个不 spawn 的判定面与一个命令驱动工具）。
- **repo-surface-scope**：两个新增 capability 各对应可指认的存活实现（`crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/**`、`.dev-team/architecture/api/**`）；不退役任何资产，不新增指向已删资产的路径引用。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/archi/src/api/mod.rs`（新） | 契约面装配与出口：**现状八行 → 终态九行**（终态以 design「变更清单 - 新增文件」表 `api/mod.rs` 行的九行为唯一源）——**现状八行** = `meta`（生成载元单点）/ `enabled`（契约面启用判据）/ `load_contracts`（逐元素定位 md → 解析 → 双向 gate → 装配内存 schema + 符号集）/ `render_all`（装配四类生成物为内存字符串）/ `bootstrap`（出 stdout 草稿）/ `check`（十判据入口）/ `api_ref` 与 `api_ref_of`（契约状态投影的两个入口），即盘上起点 `grep -c '^pub fn '` ⇒ **8**；**终态九行** = 上述八行 + **`display`**（design D55：`pub(crate) fn display` 升为 `pub fn` 并经 `lib.rs` 出口，供 `archi-api-gen` 复用、删工具侧逐字重复实现），终态 `grep -c '^pub fn '` ⇒ **9**；**出口面即这九行，MUST NOT 另立「比对 / 归一」函数**——契约与代码是否一致由 `generate --check` 的字节比对 + 语言编译器两步回答（spec「生成器、verify 闭环与 bootstrap 剪枝」首条明禁归一函数、双层锁与语义 diff 比对器）；`check` 的判据语义归 `desktop-archi-api-check`（本表只登记它为契约面唯一出口）；零写盘、零 spawn。**计数口径留痕**：本行原写「按 design 公共函数表 **八行**」，与 design 新增文件表的九行互斥；回溯对齐裁定九行为终态、八行为起点，本行不得再自称终态值（`desktop-archi-api` 主基线 Module Contract 表的「出口恰八行」由归档 SpecSync 承接，不属本 change 可写面） |
| `crates/core/archi/src/api/paths.rs`（新） | 契约 / 生成物 / `#[path]` / TS import specifier / 跨模块 `$ref` 相对路径的**单点产出**（全经 `archi_api_dir` 与 layout 派生，零行内 `api` 段、零手算 `../` 层数） |
| `crates/core/archi/src/api/contract.rs`（新） | `<FQN>.md` 解析：H1 锚点、`### <op-id>` 分段、`api-op` / `api-types` / `api-reexport` fence 提取、双向 gate 校验、构造器声明检测 |
| `crates/core/archi/src/api/schema.rs`（新） | `$defs` 规则（身份 / 提升原名 / `$ref` 顶层 / 禁按结构去重）、透传条目纯 `$ref` 校验、`schema.json` 装配（`$id` / `title` / `properties` / `$comment`）、`INDEX.toml` 装配 |
| `crates/core/archi/src/api/extract/mod.rs` + `rust.rs` + `ts.rs`（新） | bootstrap 声明抽取：Rust `syn`（类型 / trait / 枚举 / `pub fn`，AST 归一签名，正则零用）、TS `oxc`（**取数面 = 导出面**：`export function` → 操作、`export type` / `interface` / `class` → 类型、`export const` 按初始化表达式分类）；`visibility` 四值词表封闭但可判值域按语言分列（Rust 四值可判 / TS 生产面只 `external`；`unused` 的降级接口 `apply_knip_report` 已实现但**本期未接线**（零产品调用者；两语言的 `Unused` 产出点都在「调用方按引用事实降级」一侧，该调用方本期不存在 ⇒ `Unused` 生产面不可达，沿 design D39 / spec S-12）、`crateOnly` / `testOnly` TS 侧本期留白，见 AC-20） |
| `crates/core/archi/src/api/check.rs`（新） | 十条判据求值 → `Vec<ArchiApiFinding>`：entry 优先互斥、契约 vs 代码导出集、签名归一比对、留白矛盾、shadowing / refMissing / reexportDrift / nestedCrossCrate / constructorDeclared；`$defs` 只读 `schema.json`（不并读 markdown） |
| `crates/core/archi/src/api/emit/rust.rs` + `ts.rs`（新） | 生成物渲染：`.i.rs`（`struct` / `trait` / `enum` / newtype / 自动构造器 / 透传 `pub use` + 头注释与工具版本行）、`.i.ts`（`interface` / `as const` + `type` / 别名 / 透传 `export { } from`）；纯函数出字符串，写盘归工具 crate |
| `crates/core/archi/src/drift/report.rs` | 新 `ArchiApiFindingKind`（10 值 + `ALL` + `as_str` + `severity()`）+ `ArchiApiFinding`（`kind` / `category` / `element` / `target` / `message` / `occurrences`）+ `ArchiDriftReport.api_findings` + `REPORT_VERSION` 1 → 2 |
| `crates/core/archi/src/dto.rs` | 新 `ApiStatus`（`declared` / `none`，`ALL` + `as_str`，`specta::Type`）+ 新 `ArchiApiRef`（`status` / `contractPath` / `schemaPath` / `generatedPath`）；`ArchiElement.api: ApiStatus` / `ArchiElementBrief.api: ApiStatus` / `ArchiModuleDetail.api: ArchiApiRef` 出线（路径由 `archi_api_dir` 派生） |
| `crates/core/archi/src/model.rs` | `Element.api: ApiStatus` |
| `crates/core/archi/src/parse/toml.rs` | `RawElement.api`（`#[serde(default)]`，`deny_unknown_fields` 面自动生效） |
| `crates/core/archi/src/scan/owner.rs` | 归属规则加一条：`.i.rs` / `.i.ts` 生成物按去掉 `.i` 后的路径判定 |
| `crates/core/archi/src/drift/mod.rs` | `check()` 装配 `api_findings`（调 `api::check`，契约面与模型面并列）；`REPORT_VERSION` 随动 |
| `crates/core/foundation/src/layout/mod.rs` | 新增 `API_DIR_NAME` 常量、`archi_api_dir(root)`、`archi_api_rel_dir()`（全从 `archi_dir` / `archi_rel_dir` 派生，零新文件名字面量） |
| `crates/tools/archi-api-gen/Cargo.toml` + `src/main.rs`（新） | 工具 crate：裸名 `archi-api-gen`；依赖 `archi`（契约解析 / 装配 / 判据 / 渲染）+ `foundation`（路径派生）+ `serde_json`；CLI 三子命令 `generate` / `--check` / `bootstrap`；零 Tauri / 零 store / 零 config / 零 orchestration |
| `src-tauri/Cargo.toml` | `members` 增 `crates/tools/archi-api-gen`；`[workspace.dependencies]` 增 `archi-api-gen = { path = "crates/tools/archi-api-gen" }`；壳 `[dependencies]` MUST NOT 增该 crate |
| `packages/desktop/package.json` | 增 `api:export` / `api:check` 脚本；`check` 链插入 `api:check`（三段 → 四段）；**`server:check` 的 fmt 段由写模式 `cargo fmt --all` 改为校验模式 `cargo fmt --all --check`**（clippy 段逐字不动，design D28——delta spec 三处强制项，S-1 / S-3 / S-6 的落地）；`version` 按**当前 `HEAD` 值 +1 patch** 递增（相对递增，MUST NOT 写死字面号；`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 版本不随动）；行尾取 `vp fmt` 不动点（LF） |
| `packages/desktop/knip.json` | `ignore` 增 `src/**/*.i.ts`（生成物非手写代码），`ignoreDependencies` / `ignoreExportsUsedInFile` 零新增 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 再生成（`ApiStatus` + `ArchiElement.api` + `ArchiApiFindingKind` / `ArchiApiFinding` + 报告 `apiFindings`） |
| `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md`（新） | 试点契约（手写唯一真相，已落盘）：职能 + `### file_stem` 一个 `api-op`（`file_stem(timestamp: string) -> string`）+ `LocalArchiCheck` 的 `api-types`（零字段 object）；无 `### new`；`compose_text` 不进契约（其入参类型由非直接子元素 `Core.Archi` 提供，规则 4 不可表达，design D23） |
| `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json`（新，生成） | 试点 `schema.json` 投影 |
| `.dev-team/architecture/api/INDEX.toml`（新，生成） | 状态 / 类型数 / 操作数 / 覆盖率 / 降级计数 + review 口径说明段 |
| `crates/infra/archi/src/lib.rs` + `runner.rs` + `report.rs` | 入口编排：`#[path = "../../archi.i.rs"] mod api_gen; pub use api_gen::*;`——`#[path]` 的值是**字面量**（Rust 属性值不能由函数求值），MUST 与 `paths::mod_path_attr(EntryForm::Crate, "archi")` 的输出逐字相等，等值性由 `paths_test` 三形态断言与逐入口自查行双守（design D38 / spec S-11；`mod_path_attr` 零产品调用者是等值真值源的正常形态，不是缺陷），目标是根同级的 `crates/infra/archi.i.rs`；删手写 `pub struct LocalArchiCheck` 与 `impl` 内 `new()`（改由生成器接管，否则 `pub use api_gen::*` 报 E0255）；`file_stem` 从 `report.rs` 的私有自由函数挪进 `impl LocalArchiCheck` 并提升为 `pub`（实现体逐字照搬），`report.rs` 的调用点改为 `LocalArchiCheck::file_stem(&report.timestamp)`；`compose_text` **保持私有、不进契约**；`impl Default` / `impl ArchiCheckRunner` 逐字不动 |
| `crates/infra/archi.i.rs`（新，生成） | 试点生成物（落 `<root>` 同级 = `crates/infra/` 下，与上行的 `#[path = "../../archi.i.rs"]` 同指一处）：头注释（工具版本 + `common.json` 摘要）+ `pub struct LocalArchiCheck;` + `impl` 内 `new()` + 两行签名见证（`const _: fn(&str) -> String = super::LocalArchiCheck::file_stem;` / `super::LocalArchiCheck::new`） |
| `.dev-team/architecture/models/05-backend-tools.toml` | 表头注释 2 元素 / 1 关系 → **3 元素 / 3 关系**；新增 `App.Backend.Tools.ArchiApiGen` 元素 + 2 条 `[[relation]]`（→ `Core.Archi` / `Core.Foundation`） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 计数复述同步（`§1` 基线 51 / 113、`§4` 后端边 47→49、`§6` 元素总数 50→51、`§9` 复算表）；六条判据零改动 |
| `AGENT.md` + `CLAUDE.md` | 两文件各新增一节「模块 API 契约」（对象 = 本仓现有内容，已落地）：手写真相位置、四类生成物与零 diff 门、`api:export` / `api:check` 与 `check` 链段序、`api = none` 留白语义、`apiFindings` 不进 status；**MUST NOT 复制契约正文或判据长表**（指向 `.dev-team/architecture/api/INDEX.toml` 与相关 capability）；两文件保持既有同源纪律（除标题与首行外逐行相等）。判据见 AC-18、规范锚点见 `desktop-archi-api`「仓库根文档的契约纪律节」 |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 契约解析与双向 gate：H1 与文件名不一致 / `### <op-id>` 缺 fence / fence 归属错位 / 重复 op id / `api-types` 键缺 / `api-reexport` 键集与散文列表不等 / 构造器声明 | `crates/core/archi/src/api/contract_test.rs`（新） | 新增 |
| `$defs` 规则：身份=键名 / 嵌套提升改名 / `$ref` 指向非顶层 / 同结构不同类型不合并 / 透传条目带结构关键字 | `crates/core/archi/src/api/schema_test.rs`（新） | 新增 |
| 装配：`schema.json` 逐字段派生完备（`$id` / `title` / `$defs` / `properties` / `$comment`）/ `INDEX.toml` 计数与状态 / 幂等（同输入两次生成零 diff） | `crates/core/archi/src/api/schema_test.rs` | 新增 |
| Rust 抽取：多行签名（AST 归一 vs `grep` 截半反例）/ `pub` 于私有 `mod` / `pub(crate)` / `#[cfg(test)]` 树 / glob 导入 / 同名符号 / 构造器识别 | `crates/core/archi/src/api/extract/rust_test.rs`（新） | 新增 |
| TS 抽取：`export function` → 操作（唯一判定）/ `memo(...)` const → 组件留白 / 对象与 Proxy const 排除 / `export type` / `interface` / `class` → 类型 / 箭头函数 const → **不作操作**（反例在案）/ `export default` 不抽 / 六个非导出顶层分支零符号 / `visibility` 生产面值域仅 `external`（**`unused` 由 `apply_knip_report` 单测直接驱动**——接口正确性证据，产品路径零调用者的否定断言在案；`crateOnly` / `testOnly` 留白的否定断言在案） | `crates/core/archi/src/api/extract/ts_test.rs`（新） | 新增 |
| 判据十值：逐值正例与反例 / entry 优先互斥（entry 命中后不再报 missing / undeclared / signatureMismatch）/ 签名归一比对（多行 / 参数名 / `&str` vs `String`）/ 留白 ↔ 文件矛盾 / 父子类型名交集 / 跨模块 `$ref` 非直接子元素 / 跨 crate 嵌套 / 透传键集漂移 / `severity()` 与 `ALL` 逐值对拍 | `crates/core/archi/src/api/check_test.rs`（新） | 新增 |
| 生成物渲染：`.i.rs`（struct / trait / enum / newtype / 自动构造器 / 透传 `pub use` / 头注释含工具版本）/ `.i.ts`（interface / `as const` + `type` / 别名 / 透传 `export { } from`）/ 两侧对同一契约的键集一致 | `crates/core/archi/src/api/emit/rust_test.rs` + `ts_test.rs`（新） | 新增（出字符串，零写盘） |
| 报告面：`apiFindings` 空数组 / 非空分组 / `version` 2 / 既有 14 字段逐字不变 / JSON 往返 / `ArchiApiFindingKind::ALL` 十值对拍 | `crates/core/archi/src/drift/report_test.rs` | 扩展 |
| 归属剥离特例：四种形态（`views/architecture.i.ts` / `views/welcome-view.i.ts` / `crates/core/archi.i.rs` / `crates/core/archi/src/scan.i.rs`）各归其元素；非 `.i` 路径判据零变化 | `crates/core/archi/src/scan/owner_test.rs` | 扩展 |
| 路径单点函数：三种入口形态（crate / `mod.rs` / 单文件）产出的 `#[path]` 与 TS import 路径规范化后逐字等于 `<root>.i.rs` / `<root>.i.ts`；把值改成「入口同级字面量」时解析错位（反例在案） | `crates/core/archi/src/api/mod_test.rs`（新） | 新增 |
| 模型字段：`api = "declared"` / `"none"` / 缺省 `declared` / `api = "non"` 拼写错（`invalidValue` 列全合法值）/ 与 `metadata.api` 的口径差 | `crates/core/archi/src/parse/toml_test.rs` + `model_test.rs` | 扩展 |
| 词汇对拍：`ApiStatus::ALL` 二值与 `as_str` 逐值 / 四面词汇集新增 `api` 一面 / 知识文本含 `declared` / `none` | `crates/core/archi/tests/vocabulary_test.rs` + `crates/infra/agent/src/archi_knowledge_test.rs` | 扩展 |
| 判定面语料（**唯一驱动面 = 既有 `DRIFT_CASES` + 十一份 sidecar 用例，MUST NOT 另建 `tests/corpus/api/`**）：① `api-mismatch` 新 case 的三个 fixture（`contracts/App/App.Mod.md` / `models/01-app.toml` / `repo/src/mod/mod.rs`）+ 其非空 `apiFindings` 金样（`apiSignatureMismatch`，`occurrences` 逐项）；② 既有 12 份 `.full.json` + 3 份 `.incremental.json` 金样重写，只追加 `"apiFindings": []` 与 `"version": 2`（其余字节逐字不变）；③ `DRIFT_CASES` 登记面失配三形态（新 case 未登记 / 登记但目录不存在 / 旧金样缺 `apiFindings` 键）皆红；④ 逐判据正反例由 `api/**` 十一份 sidecar 用例直驱 `check::check` 断言（不依赖快照） | `crates/core/archi/tests/corpus/drift/{api-mismatch/**,expected/**}` + `tests/golden_test.rs` + `crates/core/archi/src/api/**` sidecar 用例 | 扩展（`DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认；新增 4 文件 + 重写 15 份金样） |
| layout：`archi_api_dir` 与 `archi_dir` 同源 / `archi_api_rel_dir` 前缀一致 / 纯推导零 IO / 与 `archi_models_dir` / `archi_decisions_dir` 互不遮蔽 | `crates/core/foundation/src/layout/mod_test.rs` | 扩展 |
| 工具 crate CLI：`generate` 写四类生成物 / `--check` 零写盘且差异非零退出 / `bootstrap` 只出 stdout / 幂等 / 目录创建经 layout 派生（零行内字面量） | `crates/tools/archi-api-gen/src/main_test.rs`（新） | 新增（tempdir） |
| 端到端零 diff：改契约 md 后不重生成 → `api:check` 非零退出并列出差异文件；重生成后零 diff（`--check` 自身零写入，比对由工具内完成，不依赖任何 `git diff` 段脚本） | `packages/desktop/package.json` 的 `api:check` + 手工留痕 | 机械门面（不新增测试代码，与 `api:export` 同源单命令） |
| 模型同步核对：`crates/{core,infra,tools}` 目录 ↔ 元素 `path` 双向差集（自研 crate 16 ↔ 16，`specta-zod-patched` 单列）/ `05` 段 3 元素 3 关系 / 合计 51 / 113 / 三处计数同值 / `archi_graph` 零诊断零告警 | 命令 + 留痕（结果落 `openspec/changes/module-api-contract-format/reports/`） | 新增（MUST NOT 把本仓模型塞进 crate 语料） |

### 不要修改

- `openspec/specs/**` 主基线（落主基线只在归档 `SpecSync` 步）。
- `openspec/specs/*/spec.md` 的 `## Module Contract` 章节：本变更**双写过渡**，不改 46 份既有表（`§10.1` / `§10.4` 已定）；退出条件另案裁决。
- `.dev-team/architecture/models/**` 的 `gate` 档位：全 `off` 不动；契约判据的档位在 `ArchiApiFindingKind::severity()` 内，不经元素 `gate`。`.dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,06-frontend}.toml` 五文件零改动（只有 `05` 段追加元素与边）。
- `.dev-team/architecture/models/**` 的 `path` 归一化改写：存量 41 个元素 `path` 全部带 `./` 前缀 + 尾 `/`（本变更新增第 42 条同形），归一在模型解析层完成（`OwnerIndex::normalize` 已处理），**MUST NOT 批量改写存量 models**（27 条既有 `apiEntryMissing` 里 16 条前端单文件元素就是保留原样的直接结果）。
- `.dev-team/architecture/config.json`：架构域看护配置**不作为交付物创建**（缺失即空配置）。
- 存量代码债务：不为合规建 barrel、不改前端元素根路径去扩展名、不挪 `infra/agent/sdk` 的 3 处子→父类型依赖、不上移壳元素根路径（实验留作检查器验证语料）。**唯一例外**是试点元素为让契约有效而做的代码形状调整（design D23）：`file_stem` 从 `report.rs` 的私有自由函数挪进 `runner.rs` 的 `impl LocalArchiCheck` 并提升为 `pub`（只改可见性 + 挪位置，实现体逐字照搬），以及删掉手写 `struct LocalArchiCheck` / `impl` 内 `new()`（改由生成物接管，否则 `pub use api_gen::*` 报 E0255）；`compose_text` 保持私有、不进契约。
- `crates/core/archi/src/{model,parse/toml,graph,docs,search,diff,edit}` 的既有逻辑：本变更只在 `model.rs` / `parse/toml.rs` 各加一个 `api` 字段，另在 `scan/owner.rs` 加一条 `.i` 剥离规则，其余零改动。
- `crates/infra/checks/**`、`crates/infra/vcs/**`、`crates/infra/store/**`、`crates/core/orchestration/**`、`crates/core/workflow/**`：检查域 runner、git 能力、库表、port 与相位表零改动（本变更不新增 step、不新增 port、不新增 runtime 开关）。
- `crates/core/workflow/src/prompts/**`（16 份角色 md）：零改动。
- `crates/infra/agent/**` 除 `archi_knowledge.rs` 与 `archi-knowledge.md`（若需补 `api` 词汇）：工具面 / 注入矩阵 / policy 零改动。
- `packages/desktop/src/**` 除生成物（`src/**/*.i.ts`）：前端视图、hooks、组件、路由零改动（本变更不产生可见 UI 面）；`packages/desktop/knip.json` 只加一条 `ignore` 条目（见实现文件表）。
- `.github/workflows/**`、`src-tauri/tauri.conf.json`（版本经 `../package.json` 跟随）、`src-tauri/capabilities/default.json`（权限面零扩张）。
- `openspec/changes/module-api-contract-format/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | **资产形态闭环**：`<FQN>.md` 为唯一手写资产，含 H1 锚点 + 散文区 + `api-op` / `api-types` / `api-reexport` 三类 fence；双向 gate 三条（heading ≡ op fence、`api-types` 键 ⊆ `$defs`、`api-reexport` 键 ≡ 散文列表）任一处不成立即契约错误并点名位置 | `api/contract_test` 七条用例 |
| AC-2 | **`schema.json` 纯派生不可手改**：生成物每字段有 markdown 出处（`$schema` / `$id` / `title` / `$defs` / `properties` / `$comment` 逐项）；头部带警示；手改已提交 → `api:check` 持久失败并列出差异文件；手改未提交 → 下次 `generate` 覆盖（留痕于风险 R-1） | `api/schema_test` + `api:check` 手工留痕 |
| AC-3 | **引用与命名规则**：身份 = `$defs` 键名；嵌套 `$defs` 提升且原名保留（改名即错）；`$ref` 指向非顶层条目即错；同结构不同类型（`ArchiDependsOn` vs `ArchiDependedBy`）不合并；多 `$ref` 同类型只定义一次 | `api/schema_test` 五条用例 |
| AC-4 | **生成物落点与入口查找**：`.i.rs` / `.i.ts` 与 `<root>` 同级，`.i` 剥离归属四种形态全对；`#[path]` 的值是**字面量**且逐字等于 `paths::mod_path_attr` 对三种入口形态的输出（`crates/infra/archi/src/lib.rs` 的实际值 `"../../archi.i.rs"` 逐字相等），TS import 路径由 `paths::ts_import_specifier` 单点产出，两者规范化后逐字等于 `<root>.i.*`；MUST NOT 出现「函数产出属性值」的表述（不可执行），`mod_path_attr` 零产品调用者是等值真值源的正常形态；Rust 三分支 / 前端四分支入口顺序封闭且实测命中数与预期一致（`<dir>/src/lib.rs` 14 / `mod.rs` 0 / `<dir>.rs` 0 / 前端四分支 0）；`#[path]` 编排示例编译通过；与既有同名类型冲突报 E0255 而非静默覆盖 | `owner_test` 扩展 + `paths_test` 三形态断言 + 试点编译 + 入口命中清单留痕 |
| AC-5 | **类型映射两侧对等**：`object` / `enum` / `oneOf` / `x-class` / 标量 / `x-function` / 错误面七种协议在 `.i.rs` 与 `.i.ts` 各出正确形态；构造器由生成器产出且**不出现**在契约 `### 操作`；位置分派表（入参 `&str` / 返回 `String` / 字段 `String` / 入参 `&[T]`）为生成器内部表不外露；泛型零支持（契约写泛型即错） | `api/emit/*_test` + `api/check_test` 反例 |
| AC-6 | **TS 形状校验闭环**：实现侧 `import type` 引用 `.i.ts` 并标注返回类型，形状不符即 `tsc` 报错（`--strict --isolatedModules --moduleResolution bundler` 实测）；`export const X = [...] as const` + `export type X` 同名值/类型共存编译通过；`knip.json` 的 `ignore` 含 `src/**/*.i.ts` 且 `ignoreExportsUsedInFile` / `ignoreDependencies` 零新增 | `tsc --noEmit` + `client:check`（含 knip） |
| AC-7 | **十条判据与期望数逐项对拍**：`apiEntryMissing` **28**（27 条既有清单 + 新增 `App.Backend.Tools.ArchiApiGen`）、`apiMissing` **13**（14 个命中入口的 crate 元素 − 1 试点）、`apiShadowing` / `apiRefMissing` / `apiReexportDrift` / `apiNestedCrossCrate` / `apiSignatureMismatch` / `apiUnexpected` / `apiConstructorDeclared` 各 **0**、`apiUndeclared` 不适用（`off`）；多于期望 = 假阳、少于 = 假阴，两者都是缺陷；`apiFindings` 首期不影响 `status` / `passed`。**语料落点（唯一驱动面）**：逐判据正反例由 `crates/core/archi/src/api/**` 十一份 sidecar 用例直驱 `check::check`；报告面逐 case 由 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` + `corpus/drift/expected/<case>.<mode>.json` 对拍（本变更新增 `api-mismatch` 非空金样 + 重写既有 15 份）；MUST NOT 另建 `tests/corpus/api/` 第二套语料 | 全量检查实跑 + 报告数基线表留痕 + `DRIFT_CASES` 逐 case 对拍 |
| AC-8 | **判据词汇与报告槽位**：`ArchiApiFindingKind` 十值 + `ALL` + `as_str` 双向对拍；`category` 为开放值集（无 `ALL`、渲染层按它分组）；`ArchiDriftReport` 既有 14 字段逐字不变、新增 `apiFindings`、`version` = 2；既有漂移金样仅追加 `"apiFindings": []`（其余字节不变）+ 至少一个非空金样在案 | `report_test` + `vocabulary_test` + corpus golden |
| AC-9 | **判定单源**：`archi::api::check` 是十条判据的唯一实现；命令面（`archi_drift`）与门禁面（`LocalArchiCheck`）对同一输入产出的 `apiFindings` 逐字段相等；`$defs` 只读 `schema.json`（不并读 markdown，安全性由 AC-2 / AC-10 前置门禁保证） | 同源用例 + 源码审查 |
| AC-10 | **`api` 元素字段**：显式字段封闭二值 `declared`（缺省）/ `none`，`ApiStatus::ALL` 二值；`api = "non"` 报 `invalidValue` 并列全合法值（`metadata = { api = "non" }` 静默变「未声明」为在案反例）；`archi_module` 单元素响应含 `api` 并列字段（`status` / `contractPath` / `schemaPath` / `generatedPath`），无契约即 `status = none` 且路径 `null`；容器元素 `path` 为空即天然无契约 | `parse/toml_test` + `dto_test` + `commands/archi/mod_test` |
| AC-11 | **嵌套与透传**：父契约的选择性透传（`Components.Agent` 只透传 2 子元素、自有 `SessionPanel`、4 个未导出子元素不出现）在案；`api-reexport` fence 键集 ≡ `schema.json` `$defs` 透传键集；透传条目零结构关键字；父子 `$defs` 类型名交集为空；非直接子元素引用 → `apiRefMissing`；跨 crate 父子 → `apiNestedCrossCrate` | `api/check_test` + 试点与前端案例用例 |
| AC-12 | **生成器工程面**：`generate` 写四类生成物（`<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）且幂等；`--check` **零写盘**、有差异即非零退出并列文件；`bootstrap` 只出 stdout；`archi` 零写盘零 spawn 纪律不变；`archi-api-gen` 的 workspace 内依赖恰为 `archi` + `foundation`（外部依赖仅 `serde_json`）、零 Tauri / 零 store / 零 config / 零 orchestration、不被 app / core / infra 依赖；**生成物须是格式化器不动点**（design D29）：`.i.rs` 通过 `rustfmt --edition 2021 --check`（exit 0）、`.i.ts` 通过 `vp fmt --check`（单引号形态），否则 `api:check` 的字节门与 `client:check` 的 `vp check --fix` 永久对撞；不动点由渲染模板内建、MUST NOT 由生成器 spawn 格式化器实现 | `main_test`（tempdir）+ 依赖审查 + 两条 `--check` 判据（零写入） |
| AC-13 | **试点元素端到端**：`App.Backend.Infra.ArchiRuntime` 契约 md + `schema.json` + `.i.rs` 三者落地；契约 op 恰为 `file_stem(timestamp: string) -> string`（`compose_text` 零出现——其入参类型由非直接子元素提供，规则 4 不可表达，design D23）；`### new` 不存在（构造器自动产出）；手写侧零同名 `struct LocalArchiCheck` / 零 `impl` 内 `new()`（否则 E0255）、`report.rs` 零 `fn file_stem` 且调用点走 `LocalArchiCheck::file_stem`、`compose_text` 仍私有；生成物与手写 `impl Default` 共存编译且签名见证（`super::LocalArchiCheck::file_stem`）成立；`archi_module` 该元素 `api.status = declared`；判据该元素零命中（除 `apiUndeclared` 关闭） | 试点编译 + 命令实调 + 判据报告 |
| AC-14 | **交付面**：`bindings:export` 重导出后生成物入库、提交前人工核对为零 diff（含 `ApiStatus` / `ArchiApiFinding` / `apiFindings`；零 diff 守卫已退役，核对形态沿 `desktop-ipc-type-bindings` 既有口径）；`api:check` 退出 0 且零写入；`check` 链逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`（第二段是 `bindings:export`，与 `package.json` 实存键逐字一致——判据 `grep -c '"bindings:export"' package.json` ≥ 1，且 `scripts` 内零已退役键）；**`server:check` 的 fmt 段为校验模式**：`grep -c 'cargo fmt --all --check' packages/desktop/package.json` = 1 且 `grep -c 'cargo fmt --all --manifest-path' ...` = 0、clippy 段逐字不变，`cargo fmt --all --check --manifest-path src-tauri/Cargo.toml` exit 0（design D28；delta spec S-1 / S-3 / S-6 的强制项，交付态下 `pnpm run check` 连跑两次第二次 `git status --porcelain` 与第一次逐行相等）；`packages/desktop` 的 `version` 为「本 change 落地时 `HEAD` 值 + 1 patch」（**相对当前值递增，MUST NOT 写死字面号**——现值随每次源码改动滚动，写死即下次改动即失效；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行 +1，并核对 `src-tauri/tauri.conf.json` 仍为 `"../package.json"` 跟随、`src-tauri/Cargo.toml` 版本不随动）；`vp check` / `knip` / `cargo clippy -D warnings` 通过 | 机械门面 |
| AC-15 | **模型与新增 crate 同 change 同步**：`crates/tools/archi-api-gen` 目录 ↔ `App.Backend.Tools.ArchiApiGen`（`kind = module`、`gate = off`、`path` 逐字、`summary` 非空且不复述 FQN）；`05` 段 **3 元素 / 3 关系**（→ `Core.Archi` / `Core.Foundation` 逐条在案）；全基线 **51 元素 / 113 关系（后端 49 + 前端 64）**；自研 crate 16 ↔ 16 零差集（`specta-zod-patched` 单列为既有缺口）；模型表头 / spec 计数表 / ADR `§9` 三处同值；`archi_graph(root)` 零诊断零告警 | 双向覆盖比对命令 + `archi_graph` + `grep` 计数 + 留痕 |
| AC-16 | **零债务消解副作用**：`git diff --stat` 不含「为消解 27 条 entry-missing 而建 barrel / 改元素根路径 / 挪类型」的改动；前端元素 `path` 与 `models/*.toml` 的 `path` 写法逐字未变；**契约相关**代码形状改动限于试点元素的三处形状对齐（`file_stem` 可见性提升并移入 `impl`、删除手写 `struct LocalArchiCheck` 与 `impl` 内 `new()` 由生成物接管，design D23），零实现体改写。**既有并列项**（六份 `sdk/**` 纯 fmt 清偿、`package.json` 的 fmt 段改 `--check` 与行尾归一）逐字沿用 | `git diff` 逐文件审查 |
| AC-17 | **语言无关纪律**：契约格式与判据面零 `crate` / `Cargo.toml` / Rust 专有概念进入资产形态与 schema（语言特定部分只落 `api/extract/{rust,ts}.rs` 与 `api/emit/{rust,ts}.rs`）；`<FQN>.md` / `.schema.json` / `INDEX.toml` 内零 `crate` 词 | 源码审查 + 生成物审查 |
| AC-18 | **`AGENT.md` / `CLAUDE.md` 契约纪律节（对象 + 条件）**：对象 = 仓库根 `AGENT.md` 与 `CLAUDE.md`；条件 = 两文件各恰有一节 `## 模块 API 契约`，该节恰含 5 条要点（手写真相位置 + 三类 fence / 四类生成物勿手改 / `api:export`·`api:check` 两脚本与 `check` 链四段 / `api = none` 留白语义 / `apiFindings` 不进 `status` 且指向 `INDEX.toml`），且 MUST NOT 复制契约正文、判据长表（含 `ArchiApiFindingKind` 十值）或 fenced json 示例；该节内零 `openspec/specs/**` 路径；两文件的该节逐字相同（沿既有同源纪律）。判据：`grep -c '^## 模块 API 契约'` 在两文件各为 1；`grep -c 'apiEntryMissing'` 在两文件各为 0（判据长表未复制）；`diff AGENT.md CLAUDE.md` 恰 2 处差异且**全部在节外**（`:1` 标题、`:3` 首行说明）；节内正文出现于两文件且逐字相同（节内无 diff 行） | `grep` 双向对拍 + `diff`（机械门面，不新增测试代码） |
| AC-19 | **`foundation::layout` 派生函数（对象 + 条件）**：对象 = `crates/core/foundation/src/layout/mod.rs` 的 `API_DIR_NAME` 常量、`archi_api_dir(root)` 与 `archi_api_rel_dir()`；条件 = 三者全从 `archi_dir` / `archi_rel_dir` 派生（`archi_api_dir(root)` 结果逐字等于 `archi_dir(root).join(API_DIR_NAME)`，`archi_api_rel_dir()` 逐字等于 `archi_rel_dir() + "/" + API_DIR_NAME`）、纯推导零 IO（不存在的 root 亦返回）、段名字面量 `"api"` 只出现于 `API_DIR_NAME` 一处（产品 `.rs` 内零第二处 `"api"` 段拼接，`*_test.rs` 除）、与 `archi_models_dir` / `archi_decisions_dir` / `archi_config_path` 互不为前缀；既有 `archi_dir` 族与 `Layout` 三字段语义零变化。判据：`mod_test.rs` 四条用例（同源 / 前缀一致 / 零 IO / 互不遮蔽）+ 四条机械 `grep`（对象 = `crates/core/foundation/src/layout/mod.rs`；凡涉全仓扫描的命令均排除 `*_test.rs`，比对前须先滤测试面）。① `grep -c 'API_DIR_NAME' crates/core/foundation/src/layout/mod.rs` **恰 4 处**（`:46` 常量定义、`:168` doc 引用、`:172` `archi_api_dir` 消费、`:212` `archi_api_rel_dir` 消费）；② `grep -rl 'API_DIR_NAME' --include=*.rs crates/ src/` 的输出先滤掉 `*_test.rs` 行后**恰 1 个文件**（`layout/mod.rs`——常量零外溢）；③ `grep -rn '"api"' --include=*.rs crates/ src/` 的输出先滤掉 `*_test.rs` 行后**恰 1 处**（`:46` 的 `const API_DIR_NAME: &str = "api";`——段名字面量零第二处）；④ `grep -cF "pub fn archi_api_dir(" crates/core/foundation/src/layout/mod.rs` 与 `grep -cF "pub fn archi_api_rel_dir(" 同文件` 各**恰 1 处**（两函数的 `pub fn` 签名行；两串分列计数，MUST NOT 用扩展正则的竖线交替——该形态在 markdown 表格内还会破坏列切分） | `crates/core/foundation/src/layout/mod_test.rs` 扩展用例 + `grep` |
| AC-20 | **bootstrap 取数面：AST 归一签名与 `visibility` 剪枝（对象 + 条件）**：对象 = `crates/core/archi/src/api/extract/mod.rs` / `rust.rs` / `ts.rs` 三文件。条件六条：① **归一签名单源**：`normalize_signature` 在产品 `.rs` 内恰 **1** 处定义，`check.rs` 逐字消费 `extract::normalize_signature` 恰 **1** 处，且 `check.rs` 对语言取数器的直连**零命中**（`rust::extract_crate` / `ts::extract_file` 各 **0**——判据只经 `extract_element` 取数，MUST NOT 各走一套）；② **Rust 侧走 AST 不截行**：`rust.rs` 的 `syn::parse_file` 恰 **3** 处（`extract_file` / `extract_crate` / 内层模块解析），`regex` / `Regex` **零命中**（正则抽签名即「截半」缺陷的复发路径），多行签名用例断言 `params.len()` 全取且「按行截取得 0 个参数」反例在案；③ **`visibility` 词表封闭四值**：`Visibility` 四变体 + `pub const ALL: [Visibility; 4]` 恰 **1** 处 + `as_str` 四词（`external` / `crateOnly` / `unused` / `testOnly`）各恰 **1**；④ **可判值域按语言分列（剪枝面）**：`rust.rs` 的三值产出点合计恰 **6** 处（`External` / `CrateOnly` / `TestOnly`，`TestOnly` 由 `#[cfg(test)]` 子树语法判定而非文件名），`Visibility::Unused` 在 `rust.rs` **零命中**（`unused` 由调用方按引用事实降级）；`ts.rs` 的 `CrateOnly` / `TestOnly` **零命中**（两值本期留白），生产面只产 `External`（有 `export`）；`Unused` 的唯一产出点在 `apply_knip_report` 函数体内、该函数**产品路径零调用者**（`pub fn apply_knip_report` 恰 **1** 处，`archi-api-gen` 的 `main.rs` 与 `api::bootstrap` 对 `knip` **各零命中**）⇒ `Visibility::Unused` 在生产面**不可达**，是**显式留白**（D39 / S-12），MUST NOT 用「已实现」掩盖；⑤ **TS 取数面 = 导出面**：`Statement::ExportDeclaration` 恰 **1** 条分支；六个非导出顶层分支（`FunctionDeclaration` / `ClassDeclaration` / `VariableDeclaration` / `TSTypeAliasDeclaration` / `TSInterfaceDeclaration` / `TSEnumDeclaration`）**零命中**；`ArrowFunctionExpression` **零命中**（箭头函数不作操作，MUST NOT 从 `const` 初始化式推断函数）；`ExportDefault` **零命中**（`export default` 本期留白，对外名由导入方决定）；⑥ **不猜、不用伪判据**：`Language::Unknown => Vec::new()` 恰 **2** 处（`extract` / `extract_element`，未知语言返回空集不猜测）；`used_by` 在产品 `.rs` 内**零命中**（本期留白，需 import 反向图）；文件名族（`.test.` / `.spec.` / `__tests__` / `fixtures`）在产品侧**零非注释命中**（用文件名冒充 `testOnly` 是伪判据而非近似）。判据（对象 = `crates/core/archi/src/api/`，凡涉全仓扫描的命令先滤 `*_test.rs` 再计数）：六条机械 `grep` 逐条对拍 + 用例集 `extract_file多行签名ast归一` / `extract_file的api_type映射表` / `visibility四值与all` / `extract_file的visibility值域仅两值` / `extract_file的箭头函数不作操作` / `extract_file的非导出声明不抽` / `extract_file的export_default不抽` / `extract_file的pub链可达为external` / `extract_file的私有mod内pub为crateonly` / `extract_file的restricted可见性` / `extract_file的cfg_test树为testonly` / `extract_file的文件名不作testOnly的proxy` / `extract_file不产testOnly的否定断言` / `apply_knip_report降级未使用导出` / `extract_element与bootstrap同源`。spec 侧对应措辞（TS 生产面只 `external` / `unused` 本期未接线 / 箭头函数不作操作 / `used_by` 留白）由 `desktop-archi-api` 的 S-7 / S-8 / S-9 / S-10 / S-12 承接，本条只钉产品代码事实、不复制 spec 文本 | `crates/core/archi/src/api/extract/{mod,rust,ts}_test.rs` 用例 + 六条机械 `grep` |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **未提交手改生成物被静默覆盖**：手改未 commit 时下次 `generate` 覆盖，零报错零痕迹（与今日 `bindings.ts` 同款已知行为） | 有人以为改了契约而实际没有 / 改了 schema 而契约未改，diff 上看不出来 | 中 | 生成物头部带警示（`$comment` / 首行注释）；契约 md 是唯一手写面，schema 零手改自由度（AC-2）；`api:check` 在 `check` 链前置，提交后手改必红 |
| R-2 | **`## Module Contract` 双写过渡期的语义冲突**：46 份 spec 的表格与 `<FQN>.md` 粒度与内容不同（文件路径 vs FQN、散文 vs 签名） | 两处对同一模块说法不一致（spec 说「零 spawn」，契约里出现 `std::process`） | 中 | 过渡期唯一纪律写死：**不要求逐字一致，要求语义不冲突**；不机械校验（避免造第二套对拍）；退出条件另案裁决，本变更 MUST NOT 顺手删 `## Module Contract` |
| R-3 | **`apiUndeclared` 长期 `off` 留下完备性空洞**：代码新增导出永远不会被要求补契约 | 契约逐渐落后于代码，「有契约的模块」覆盖面停滞 | 中高 | 这是迁移期的**设计选择**而非缺陷：完备性判据一次全开会立刻红（存量 27+ 条无入口）；收紧由 `severity()` 单点驱动（一个元素一个 change 收紧），`apiMissing` 天然支持渐进（只写已确定的 op 即可） |
| R-4 | **报告数口径与 explore 不一致（27→28 / 40→13）**：`§2.10` 的期望表与 `§2.6` 的判据优先级不自洽 | 实现期若照 explore 表对拍会一直「少报 27 条」，被误判为漏报 | 高（数字分歧已在案） | 本提案正面裁定互斥优先级与两条判据的口径分工（`apiUnexpected` 只判「留白但文件存在」，反方向归 `apiMissing`），把期望数写进 AC-7（28 / 13 / 其余 0）并已按本仓实测复核入口命中数（14）；spec 侧只要求「基线表逐项相等 + 数字变更须显式留痕」，不硬编码易漂数字 |
| R-5 | **TS 侧形状校验比 Rust 弱一档**：结构化类型下 `implements` 可选，抽象承诺强制不了 | 有人以为 TS 侧也有 Rust 级保证 | 高（机制属性） | `x-common.degraded` 显式留痕（错误通道走 `Promise` reject 同理）；spec 明写两侧编译器验证点不同（Rust 验签名与实现 / TS 验形状）；导出集完整度只能到门禁期（`apiUndeclared` 关闭，故本期也不到） |
| R-6 | **父透传 + glob 形态重复定义只给 warning**：实测 `pub use drift::*` + 同名 `struct` 仅 `ambiguous glob re-exports` 警告，编译器不报错 | 静默覆盖 / 类型身份漂移 | 中 | 「父子 `$defs` 类型名交集必须为空」由契约层自校验（`apiShadowing`）兜住，**不依赖编译器**；透传条目禁带结构关键字（纯 `$ref` alias） |
| R-7 | **`apiEntryMissing` 28 条债务长期挂 `warn`**：`warn` 报告会持续出现在每次检查里 | 报告噪声，真实问题被淹 | 中 | 按 `category` 聚合呈现（同一类债 N 条而非 28 行平铺）；报告本身即迁移 backlog（每消一条减一，可单调验证）；本变更显式禁止为消债而重构（AC-16） |
| R-8 | **`<Domain>` 分组名取父元素末段可能撞名**（`App.Backend.Core.Agent` 的父是 `Core`，`App.Frontend.Components.Agent` 的子父段是 `Agent`） | 同目录混装不同层元素，`$ref` 相对路径算错 → `apiRefMissing` 误报 | 中 | 分组只是磁盘布局，元素身份仍是 FQN（同目录文件名唯一）；`$ref` 相对路径由**单点函数**产出（MUST NOT 手算），撞名不影响正确性；`apiRefMissing` 会把它暴露而非静默 |
| R-9 | **新增 crate 未入模型即漂移**：本变更新增 `crates/tools/archi-api-gen` | `repo-archi-model` 的双向覆盖与漂移纪律同时红，功能全绿也不合格 | 高（旧稿口径风险） | 提案已把 `.dev-team/architecture/models/05-backend-tools.toml` 与 ADR 移入「实现文件」并给出逐文件改法（§11 / AC-15）；设计 / 测试设计相位 MUST 同步口径，旧稿不得作为豁免依据 |
| R-10 | **`knip.json` 的 `ignore` 面扩张掩盖死代码**：`src/**/*.i.ts` 进 ignore 后，未被 import 的生成接口不会被报 | 生成物与实现脱节却无人发现 | 中 | ignore 只覆盖生成物 glob（与 `src/types/generated/**` 同款）；「实现侧必须 `import type` 引用生成物」是机制要求（否则校验不发生），`tsc --noEmit` 一轮即暴露；MUST NOT 新增 `ignoreExportsUsedInFile` / `ignoreDependencies` |
| R-11 | **生成器是新增代码，自身正确性无外部背书**：`verify ≡ generate + 编译器 + 零 diff` 覆盖不到生成器自己的 bug | 生成器错则全线错，且错得一致（零 diff 门不拦） | 中 | 生成器自己的单测（`api/emit/*_test` / `api/schema_test` 逐字段断言）；`schema.json` 用 ajv（draft 2020-12 + `strict: "log"`）实测编译通过；试点元素端到端（AC-13）作为首个真实闭环 |
| R-12 | **`common.json` 若长出「带生成产物的类型」需回炉**：当前只含 `Outcome` 信封（映射规则，生成时就地展开为各语言标准类型，零生成物零 vendor） | 一旦有人往里加生成型类型，分发 / 版本 / 冻结方案全部要重裁 | 低（已显式记录约束） | 该边界写成 requirement 内的显式约束（加「带生成产物的类型」即触发再评估），并把「工具版本 + `common.json` 摘要」写进生成物头注释，使变更来源可分辨 |
| R-13 | **试点元素契约面过薄**（若只剩 `new()`），无法支撑将来测试生成 | 格式有效性无法被真实数据验证 | 中 | 按「契约变薄时改代码，不改格式」暴露已存在的私有纯函数 `file_stem`（`report_test` 已在测其行为），落成「一个真实操作 + 一个类型 + 构造器」三形态齐备（design D23）；**不取 `compose_text`**——其入参类型由非直接子元素 `Core.Archi` 提供，写成契约即违反「跨模块 `$ref` 须指向直接子元素」规则 4，写作内联 object 则重复定义他模块类型（语义错），故它保持私有、不进契约。该形状调整（只改可见性 + 挪位置，不动实现）已在「不要修改」中显式单列，不与「存量债务不消解」冲突 |
| R-14 | **`.i.ts` 生成物与前端源码同目录**（根同级，`src/**` 内散布） | 手写文件旁多出生成文件，`git` 视角不聚拢；有人误当手写文件编辑 | 中 | 扩展名 `.i.ts` / `.i.rs` 为首层标识（仓库当前两者均为 0，命名空间干净）；文件头注释警示；`api:check` 零 diff 门兜住手改 |
| R-15 | **`<archi_dir>/api/**` 与 `.i.ts` 生成物使「生成物 `git diff` 噪声」成为常态**：一处契约改动会带出 schema / INDEX / 两个 `.i` 四份 diff | review 疲劳，有人开始跳读 | 中 | review 口径写死：只读 `<FQN>.md`，其余 diff 视为机械噪声（`INDEX.toml` 说明段承载该口径）；`category` 聚合让判据报告不像逐行铺开；口径与 `bindings.ts` 同款（既有先例已被接受） |
