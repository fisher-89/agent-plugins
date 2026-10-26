# 测试设计: retire-dev-team-plugin

> **变更**: retire-dev-team-plugin
> **日期**: 2026-10-26
> **修订**: v3 — 对齐 proposal / design v3「落地校准」：`openspec/config.json` 的两处指针**已值级替换落盘**（改动恰 2 行、CRLF 保持、`JSON.parse` 通过、旧路径命中归零），故该文件计入「修改文件」清单；AC-11 双段判据两段均已达成（第一段保留为「对改写前副本可复跑」的基线），本稿 AC-11 验收行与 `## 不可测试项` 20 随之改为「指针基线无单元测试载体」口径。
> **修订**: v2 — 对齐 proposal / design v2「回溯重定」：`openspec/config.json` 两处指针曾改判为**用户手工交接项**（agent 不可写），AC-3 拆三分层、AC-4 只留可执行面、新增 AC-11 双段基線；本稿随之补 AC-11 验收行与无单元测试载体的判定（`## 不可测试项` 20），并把 `src/types/generated/bindings.ts`（生成物随动）纳入被测文件清点。
> **基于**: `proposal.md`（验收标准 **AC-1 ~ AC-11**；「变更范围 - 实现文件」删除面 4 类 + 新增 1 项 + 修改 14 项 + 配置指针 1 文件 2 处值）；`design.md`（v3：「新增文件」1 项 /「修改文件」15 项（含 `openspec/config.json`）/「删除清单」4 类 /「配置指针（已落盘）」表；「公共函数 / API」与「类型定义」子表为空注释；「数据模型 - 镜像结构与一致性不变量」与「指针基线」两段；D1 ~ D18；事实校准 1 ~ 7；AC 对齐表 AC-1..AC-11）；`tasks.md`（阶段一「schema 镜像落位」/ 阶段二「用户手工交接」/ 阶段四「引用清面」/ 阶段五「主 spec Purpose + delta 核对」/ 阶段七「自查 B1 / 自查 C」）；`specs/{repo-surface-scope,desktop-workspace-config,desktop-test-execution,desktop-explore-page,desktop-phase-prompts,desktop-change-flow-view,desktop-workflow-write-face,desktop-change-create,desktop-change-archive,desktop-change-orchestration,desktop-change-worktree,desktop-release-pipeline,desktop-crate-layout,desktop-write-protection}/spec.md`（14 份 delta 在案）
> **被测模块来源**: design「新增文件」1 项（`packages/desktop/src-tauri/crates/core/config/config.schema.json`）+「修改文件」14 项（`.gitignore`、`README.md`、`AGENT.md`、`CLAUDE.md`、`packages/desktop/src/lib/explore-stance.ts`、`crates/core/config/src/{lib,lib_test}.rs`、`crates/core/checks/tests/{corpus_golden_test.rs,fixtures/README.md,golden/README.md}`、`openspec/specs/{desktop-phase-prompts,desktop-change-create}/spec.md`、`packages/desktop/package.json`、`packages/desktop/src/types/generated/bindings.ts`）。design「删除清单」4 类（`plugins/**`、`dist/**`、两个 marketplace、50 份插件期语料）与「配置指针」行（`openspec/config.json`）**不纳入单元测试被测范围**（删除面无被测代码；指针面是配置数据，判据为命令式复核，见 `## 不可测试项` 1 与 20）。
> **测试框架**: 沿用仓库既有约定 —— **Rust 侧** `#[test]`（`crates/core/config/src/lib.rs` 末尾 `#[cfg(test)] mod lib_test;` 声明的兄弟文件子模块；`crates/core/checks/tests/*.rs` 为 crate 集成测试），断言用 `assert!` / `assert_eq!`，测试函数名可含中文标识符、断言消息为中文，**无外部断言库**；**前端侧** `vite-plus/test`（`import { describe, expect, it } from 'vite-plus/test'`，同目录 `*.test.ts`，`vp test --sequence.shuffle` 执行）。本变更**零新增测试依赖、零新增测试运行器、零新增生产导出**（design D4 / AGENT.md no-test-only-exports）。
> **五处显式判定**:
> 1. design 的「公共函数 / API」与「类型定义」两处为**空注释**（本变更零新增 / 零修改模块级导出函数、零新增 / 零修改 interface 与公共类型）→ 各 per-file 章节的 `#### 待测功能` **不取自源文件导出声明**：首节（`config.schema.json` -> `lib_test.rs`，含断言落位形态）取自 design「数据模型 - 一致性不变量」表（A1 键集 / A2 枚举 / A3 往返 / 编译期守卫）与 D4，该表是本变更唯一的测试契约（design AC-5 责任段「test-gen（契约由本设计定）」；`tasks.md` 阶段一末与阶段七末同口径）；其余三条映射对（`lib.rs` / `explore-stance.ts` / `corpus_golden_test.rs`）在设计上**零行为改动**，其 `#### 待测功能` 行明确标注为**既有 API 的回归锚点**（非本变更声明的 API 变更），章节正文只登记既有用例回归面、**零新增用例**。
> 2. 三条断言以 `include_str!` 取镜像后需要一个**入参化的断言体**才能在「漂移必被抓」的负向 / 边界方向被驱动（否则断言只有 `assert(true)` 的正向观感）。故本设计规定断言以测试内私有辅助函数 `assert_key_set(&Value)` / `assert_framework_enum(&Value)` / `assert_framework_roundtrip(&Value)` 承载：正向行以入仓镜像驱动，异常 / 边界行以 `serde_json::json!` 构造的**内存镜像副本**驱动。三者与 `wire_name` 同属 `lib_test.rs` 内私有项，**不作生产导出**（AGENT.md no-test-only-exports / design D4「零新增导出」）。
> 3. `lib_test.rs` 同时是**被测新增面**（三条断言落位处）与**测试载体**，故其映射对退化为 `lib_test.rs -> lib_test.rs`，不另立章节：断言体与其用例同属首节 `config.schema.json -> lib_test.rs`。
> 4. **AC-11 指针基线无单元测试载体**：`openspec/config.json` 是配置数据（写保护内置锁定面的对象，design 事实校准 6），仓内无测试读取该文件；「零命中 + 可达」判据只能是命令式复核（design D17 三层留痕），登记于 `## 不可测试项` 20。MUST NOT 以「把该文件移出核对面」代替记录（`repo-surface-scope` 明文禁止）。
> 5. **删除面不进被测范围**：design「删除清单」4 类（含 50 份 capability 目录）无被测代码，其核对由提交面与目录统计承接（`## 不可测试项` 1 / 2）。

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。跨模块组合用例按「链路发起方 / 最上层调用方」挂靠入口模块 —— 本变更唯一的组合锚是「schema 镜像 ↔ 校验实现」双向对拍（镜像文件 × `KNOWN_KEYS` × `TestFramework` × `parse_framework` × `wire_name`），入口为 `crates/core/config/src/lib_test.rs`，故其跨模块组合用例写入该文件所在的 per-file 章节，**不设独立集成测试章节**。

边界取值口径（按 design「数据模型 - 镜像结构」段与「一致性不变量」表的载体类型逐项收敛；`openspec/config.json` 的两处指针串无测试载体，见 `## 不可测试项` 20）：

| 类型（载体） | 边界取值 | 最少条数 |
|---|---|---|
| `serde_json::Value`（schema 镜像根对象 / 测试内构造的镜像副本） | `{}`（无 `properties`）/ `properties: {}`（空对象）/ 7 键缺一（缺 `tests`）/ 8 键（多 `extra`）/ `properties` 为 `null` / 数组 / 字符串（非对象）/ 键序打乱 | 4 边界 + 1 正常 |
| `Value` 数组（`framework.enum`） | `[]`（空）/ 单元素 / 8 值（同序正常）/ 9 值（多 `vite`）/ 7 值（缺 `pytest`）/ 缺失或 `null` / 非数组（字符串）/ 同集合异序 | 4 边界 + 1 正常 |
| `&str`（`parse_framework` 入参 / 镜像枚举串） | `""`（空）/ `"jest2"`（近似串）/ `"Node-Test"`（大写连字符非法）/ 超长（>1000 字符）/ 特殊字符（`\n` `\0` emoji） | 4 边界 + 1 正常 |
| `Option<TestFramework>`（`parse_framework` 返回） | `None`（非法串）/ `Some`（八值全覆盖） | 1（与他项并合） |
| enum（`TestFramework` 八变体） | 每个枚举值逐个遍历（封闭集，非法值在类型层不可表达）+ 1 个非法**串**（`parse_framework` 方向） | N+1 |
| `&'static str`（`wire_name` 返回 / 镜像串） | 八变体逐串对位（含 `vite-plus` / `node-test` 连字符形态）；串漂移（`jest` → `jest2`） | N+1 |

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | `plugins/`、`dist/`、`.claude-plugin/`、`.cursor-plugin/` 四目录消失（空目录不留）、未 tracked 残留（`.pack-staging` / `node_modules` / `reports`）清空、四类删除同一提交 | —（删除面，见 `## 不可测试项` 1） |
| AC-2 | `openspec/specs` 计数 35（归档后 36）、无空目录、无 0 字节 `spec.md` | —（见 2） |
| AC-3 ① | agent 可写面零命中 —— `.gitignore`（删 `# 构建相关` + 两条插件期规则，保留 `!.claude-plugin` / `!.cursor-plugin`） | —（见 3） |
| AC-3 ① / AC-6 | 同上 —— `README.md`（整篇重写：安装 / 开发 / 构建 / 发版四段 + 仓库结构 + 项目配置 + 历史一句） | —（见 4） |
| AC-3 ① / AC-6 | 同上 —— `AGENT.md`（四段正文重写，去 `DEPRECATED` 前言与「Slim Plugin」主叙事） | —（见 5） |
| AC-3 ① / AC-6 | 同上 —— `CLAUDE.md`（与 `AGENT.md` 同正文，仅标题与面向对象句不同） | —（见 6） |
| AC-3 ① | 同上 —— `packages/desktop/src/lib/explore-stance.ts`（注释头改单源自持；两形文本与注入行为零改 —— 行为面回归） | `packages/desktop/src/lib/explore-stance.test.ts`（既有 6 `describe` / 16 `it` 回归）；注释文本面见 17 |
| AC-3 ① / AC-7 | 同上 —— `packages/desktop/src-tauri/crates/core/config/src/lib.rs`（12 处 doc 直述化，代码 / 常量 / 类型 / 签名零改 —— 行为面回归） | `packages/desktop/src-tauri/crates/core/config/src/lib_test.rs`（既有 40 条回归）；注释文本面见 16 |
| AC-3 ① | 同上 —— `packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs`（三处措辞改「已固化 `core/checks` 实现 + 入仓金样」口径，断言值 / 语料零改 —— 既有用例回归） | `packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs`（自身，15 条显式 + 8 条 macro 用例回归）；断言文案文本面见 18 |
| AC-3 ① | 同上 —— `.../core/checks/tests/fixtures/README.md`（引用块措辞直述化，语料清单与样本零改） | —（见 7） |
| AC-3 ① | 同上 —— `.../core/checks/tests/golden/README.md`（`:4` / `:26` 两行措辞直述化，重写开关与文件集约定零改；design 盘查补正 #1） | —（见 8） |
| AC-3 ② / AC-7 | 主 spec 命中恰为 delta 覆盖的 16 行 8 位点；`## Purpose` 直改（14→16 计数 + 去插件路径句）—— `openspec/specs/desktop-phase-prompts/spec.md` | —（见 9） |
| AC-3 ② / AC-7 | 同上（删「插件面零改动」分句，保留双向墙条款）—— `openspec/specs/desktop-change-create/spec.md` | —（见 10） |
| AC-4 | 可执行面：镜像在案（173 行）且与迁出前逐字一致；raw URL 尾路径与仓库内现存路径同形；`packages/desktop/knip.json` 在案且 `git check-ignore` 无命中 | `.../core/config/config.schema.json` -> `.../core/config/src/lib_test.rs`（镜像可解析 + 结构锚点，正向 / 边界）；逐字一致性、URL 尾路径、目标文件在案属机械核对，见 11 |
| AC-5 | 三条一致性断言落位并通过（A1 键集 / A2 枚举 / A3 往返 + `wire_name` 穷尽 match 编译期守卫），零新增导出 | `.../core/config/src/lib_test.rs`（断言体所在，用例见 `## 单元测试` 首节） |
| AC-6 | 三份根文档只描述桌面仓（安装 / dev / build / 发版走 GitHub Releases），无插件安装手册段落、无「插件 slim 架构」主叙事 | `README.md` -> 见 4；`AGENT.md` -> 见 5；`CLAUDE.md` -> 见 6（均无测试载体） |
| AC-7 | 14 份 delta 落盘；MODIFIED header 与主 spec 逐字一致；REMOVED / RENAMED 形态合规；每 requirement ≥1 scenario 且 scenario 标记恰 4 个 `#`；`desktop-phase-prompts` 14→16 与磁盘一致；`desktop-change-orchestration` 边界四条 | —（见 12） |
| AC-8 | 新能力 `repo-surface-scope` 三条 requirement + scenario 落盘（归档后进主基线） | —（见 13） |
| AC-9 | `pnpm -C packages/desktop run check`（含 `bindings:check`）/ `run test` / `cargo test --workspace` 全绿；覆盖率与 mutation 达 `openspec/config.json` 阈值；`cargo test -p checks-runtime --lib` 不回归既有 flake 面 | —（见 14） |
| AC-9 | 生成物 `packages/desktop/src/types/generated/bindings.ts` 的 `TestFramework` doc 1 行随动须 `git add` 入索引（否则 `bindings:check` 判漂移） | —（见 19） |
| AC-10 | `packages/desktop/package.json` `0.5.30` → `0.5.31`；`tauri.conf.json` 经 `../package.json` 跟随；`Cargo.toml` 不随动 | —（见 15） |
| AC-11 | **第二段（现行实况）**：`git grep -c "plugins/dev-team" -- openspec/config.json` 零输出、`git diff --numstat` = `2 2`、`$schema` 值以 `…/packages/desktop/src-tauri/crates/core/config/config.schema.json` 结尾、`write_protection.files[0].glob` = `packages/desktop/knip.json`、`protect/**` 零 diff | —（`openspec/config.json` 配置数据，见 20） |
| AC-11 | **第一段（基线，可对改写前副本复跑）**：同一 `grep` = 2 且两行逐值与「配置指针」表「旧值」列一致 | —（见 20） |

> **列口径**：`被测文件或模块` 列按模板填**承载用例的测试文件路径**；无测试载体的行填 `—` 并指向 `## 不可测试项` 对应序号。AC 对应的「被测源文件 -> 测试文件」映射对共 **4 组**，全部登记于 `## 单元测试` 的 per-file 章节标题：`config/config.schema.json -> config/src/lib_test.rs`、`config/src/lib.rs -> config/src/lib_test.rs`、`packages/desktop/src/lib/explore-stance.ts -> packages/desktop/src/lib/explore-stance.test.ts`、`core/checks/tests/corpus_golden_test.rs -> 自身`。design「新增文件 + 修改文件」15 项与 proposal AC 逐条落位：4 项有测试载体（其中 3 项仅回归面）、11 项无载体（逐项列于 `## 不可测试项`）。
> **被测范围边界**：`openspec/changes/archive/**`（132 个历史 change）、`openspec/explores/**`、`openspec/todo.md`、`crates/core/checks/tests/{fixtures/**,golden/*.json}` 的样本字面量、`crates/infra/agent/src/sdk/protect/{rules,reason}_test.rs` 的样本、`packages/desktop/src-tauri/**` 的产品逻辑、`core/orchestration` 的两处「插件同源 / 同形」注释均为 design D14 的零触碰面，不纳入被测范围；其行为不变性由既有用例全绿承接（AC-9）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/config/config.schema.json -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

<!-- design.md 的「公共函数 / API」子表为空注释（本变更零新增、零修改模块级导出），故本节的「待测功能」不取自源文件导出声明，而取自 design.md「数据模型 - 一致性不变量」表（A1 / A2 / A3 / 编译期守卫）与 D4；镜像结构锚点取自「数据模型 - 镜像结构」表。本文件是 `.json` 静态数据物（`include_str!` 编译期绑定），无独立单测文件：其唯一测试载体是同 crate 的子模块 `lib_test.rs`（`lib.rs` 末尾 `#[cfg(test)] mod lib_test;` 声明，子模块可见父模块私有项 → `KNOWN_KEYS` / `parse_framework` / `TestFramework` 无需导出）；`lib_test.rs` 本身的三条断言落位属同一测试面，退化为 `lib_test.rs -> lib_test.rs`，不另立章节。跨模块组合用例（镜像文件 × 实现常量 × 解析函数 × `wire_name`）挂靠本入口。 -->

#### 待测功能

- `mirror() -> Value`（测试内私有取载，D4 / AC-4）：`include_str!("../config.schema.json")` + `serde_json::from_str::<Value>` —— 编译期绑定（无运行时 IO、无 cwd 依赖，禁 `fs::read` 与路径拼装）；镜像须可解析为 JSON 对象
- `assert_key_set(&Value)`（A1 键集断言，D4 / AC-5）：`schema["properties"]` 键集（排序后）== `super::KNOWN_KEYS`（排序去重后）；失败文案指认双向差异（`仅 schema 有: […]` / `仅实现有: […]`）
- `assert_framework_enum(&Value)`（A2 枚举断言，D4 / AC-5）：`properties.tests.items.properties.framework.enum`（8 串，同序）== 八变体按 `TestFramework` 声明序的 `wire_name`
- `assert_framework_roundtrip(&Value)`（A3 往返断言，D4 / AC-5）：`enum` 每串经 `super::parse_framework` 解回变体、再 `wire_name` 必回同串；失败文案含首个不往返的串与解回结果
- `wire_name(&TestFramework) -> &'static str`（测试内私有，D4 编译期守卫 / AC-5）：**穷尽** match 八变体（实现 → schema 方向）；新增第 9 变体未补臂时 `cargo clippy --workspace --all-targets` / `cargo test --no-run` 即编译失败（早于断言）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `mirror()` — 入仓镜像可解析（AC-4） | 正向 | `include_str!("../config.schema.json")` + `from_str::<Value>` → `Ok(Value::Object(_))`；顶层含 `$schema` / `type` / `properties` / `additionalProperties` 四键 | 单值 |
| `mirror()` — 结构锚点与 design「镜像结构」表一致（AC-4 / AC-5） | 边界 | 四锚点：`properties` 恰 7 键；`additionalProperties == {}`（passthrough 表达）；`properties.schema.const == "spec-driven"` 且 `default` 同值；`tests.items.required` 含 `root` / `framework` | 表驱动（4 锚点） |
| `assert_key_set` — 入仓镜像正向通过（AC-5） | 正向 | 以 `mirror()` 驱动 → 不 panic；7 键与 `super::KNOWN_KEYS` 逐项相等（`$schema` / `schema` / `context` / `rules` / `static_analysis` / `tests` / `write_protection`） | 单值 |
| `assert_key_set` — 缺键漂移被抓（AC-5 / D4） | 异常 | 镜像副本删去 `properties.tests` → panic，消息含 `KNOWN_KEYS` 与 `仅实现有: ["tests"]` | 单值 |
| `assert_key_set` — 多键漂移被抓（AC-5 / D4） | 异常 | 镜像副本追加 `properties.extra` → panic，消息含 `仅 schema 有: ["extra"]` | 单值 |
| `assert_key_set` — 键集容器边界（AC-5） | 边界 | 四形态：`properties: {}`（空对象，7 项全落「仅实现有」）/ `properties` 缺失或 `null` → panic 且文案指认「镜像缺 `properties`」（MUST NOT 越界 unwrap）/ `properties` 为数组或字符串（非对象）→ panic 且文案指认类型违例 / 键序打乱（`write_protection` 提前）→ **通过**（排序后比较，顺序无关） | 表驱动（4 形态） |
| `assert_framework_enum` — 入仓镜像正向通过（AC-5） | 正向 | `enum` 逐串 == `["jest","vitest","vite-plus","bun","rust","node-test","go","pytest"]`（同序）；`wire_name` 八串与 `#[serde(rename)]` 串逐字相等 | 遍历枚举（8 值） |
| `assert_framework_enum` — 串 / 值漂移被抓（AC-5 / D4） | 异常 | 三形态：`jest` → `jest2`（改串）→ panic 且双向差异清单含该串；删 `pytest`（7 值）→ panic 指认「仅实现有: ["pytest"]」；追加 `vite`（9 值）→ panic 指认「仅 schema 有: ["vite"]」 | 表驱动（3 形态） |
| `assert_framework_enum` — 枚举容器边界（AC-5） | 边界 | 四形态：`enum: []`（空 → 8 变体全落「仅实现有」）/ `enum` 缺失或 `null` → panic 且文案指认镜像缺 `enum` / `enum` 为字符串 `"jest"`（非数组）→ panic 且文案指认类型违例 / 同集合异序（`pytest` 提前）→ panic 指认顺序漂移（A2 按同序比较，MUST NOT 静默排序后比较） | 表驱动（4 形态） |
| `assert_framework_roundtrip` — 八值逐一往返（AC-5 / D4） | 正向 | 对 `enum` 8 串逐一 `parse_framework(&s)` → `Some(变体)` 且 `wire_name(&变体) == s`（两方向闭合） | 遍历枚举（8 值） |
| `assert_framework_roundtrip` — 非法串不往返被抓（AC-5 / D4） | 异常 | `enum` 副本含 `jest2` / `Node-Test`（大写）/ `""`（空串）/ 1000+ 字符串 / 含 `\n` `\0` emoji 串 → `parse_framework` 返回 `None` → panic，文案含首个不往返串与解回结果 `None` | 参数化（5 形态） |
| `wire_name` — 穷尽 match 编译期守卫（D4 编译期守卫） | 异常 | 新增 `TestFramework` 第 9 变体（如 `Deno`）而 `wire_name` 未补臂 → 编译失败（`cargo clippy --workspace --all-targets`）。**不作为运行时用例**：判据形态登记，由 `cargo test --no-run` 承接 | 单值 |
| `wire_name` — 逐值串对位（AC-5 / 类型面） | 边界 | 八变体各自 `wire_name` 命中预期串；`vite-plus` / `node-test` 连字符形态逐字保留（MUST NOT 归一为下划线或驼峰）；同一变体两次调用返回同一串（纯函数、无状态） | 遍历枚举（8 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（镜像 ↔ 实现对拍面） | 零 mock：断言体为纯函数，入参 `&Value` 显式传入 | A1 / A2 / A3 全部正向行 |
| 镜像文件（进程边界：数据文件） | 不 mock、也不 `fs::read`：`include_str!` 编译期字面量绑定真实入仓文件（design D4 明禁运行时 IO 与路径拼装） | `mirror()` 两行与全部正向行 |
| 镜像变体（异常 / 边界输入） | 不以 mock 框架实现：以 `serde_json::json!` 深拷贝入仓镜像后改写目标字段（内存副本）喂给同一断言辅助函数 | 「缺键 / 多键 / 容器边界 / 串漂移 / 非法串」行 |
| `super::{KNOWN_KEYS, parse_framework, TestFramework}`（被测 API 的实现侧） | 不 mock：真实调用（内部模块必须真实组合；子模块可见父模块私有项，零新增导出） | 全部断言行 |
| `serde_json` | 不 mock：仅作 `Value` 解析 / 构造载体，不逐项断言库自身语义 | 全部行 |

---

### packages/desktop/src-tauri/crates/core/config/src/lib.rs -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

<!-- design.md 的「公共函数 / API」子表为空注释：「修改文件」表对本文件只列 12 处 doc 注释直述化（`:8` / `:11` / `:14` / `:17` / `:20` / `:23` / `:37` / `:72` / `:93` / `:189` / `:344` / `:378`），代码、常量值、类型、签名**零改**；design 未声明该文件的公共 API 变更。故本节**零新增用例**（`tasks.md` 阶段一末：「本相位不写测试用例；三条一致性断言归 test-gen」；阶段四：`lib.rs` 仅注释行）。下方 `#### 待测功能` 行是既有 API 的**回归锚点**（非本变更声明的 API 变更），`#### 用例` 表登记既有 40 条用例的回归面：全绿即证明注释面改动未越界。 -->

#### 待测功能

- `load(root: &Path) -> ConfigReport`（既有公共 API，本变更零改；回归锚点）：文件态四分支（`FileMissing` / `ReadFailed` / `JsonInvalid` / 顶层非对象 `InvalidValue`）+ `assemble` 逐字段组装校验 —— 回归
- `assemble(&Map<String, Value>) -> (WorkspaceConfig, Vec<ConfigDiagnostic>)`（既有 crate 内 API，本变更零改；回归锚点）：顶层 7 键辨认 + 未知键 passthrough 进 `extra`（与 A1 的 `KNOWN_KEYS` 同源，故 A1 断言变更时本组用例应同步红）—— 回归
- `TestFramework` / `KNOWN_KEYS` 的**声明**（本变更零改；回归锚点）：八变体与 7 键常量是 A1 / A2 的源 B；声明面任何改动经 A1 / A2 断言在 `lib_test` 内立即暴露 —— 回归

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `load` — 文件态四分支回归（AC-9） | 正向 | 既有用例（`有效root无配置文件_全默认config加_file_missing_诊断_path_为文件级根` / `配置路径被同名目录占据_全默认config加_read_failed_诊断` / `坏json语法_全默认config加_json_invalid_诊断` / `空文件与仅空白字符落json_invalid分支不panic` / `顶层非对象三形态…`）全绿；`kind + path` 对位不变 | 表驱动（既有 5 分支） |
| `assemble` — 组装校验矩阵回归（AC-9） | 异常 | 既有用例（suite 必需字段缺失剔除 / `root` 空串或含通配剔除 / `framework` 枚举外剔除 / 阈值越界吃默认 / `write_protection` 类型违例 / `rules` 子字段混型 / `tests` 非数组）全绿，剔除与默认与诊断语义逐条不变 | 表驱动（既有矩阵） |
| 自洽不变量回归（AC-9） | 边界 | 既有 `违例矩阵抽样产出的config重序列化回写再组装恒零诊断` 与 `默认基线逐字段对位且过assemble零诊断` 全绿：任意违例输入组装出的 config 重序列化回写后再 `load` 恒零诊断 | 单值 |
| 未知键 passthrough 与 A1 同源回归（AC-5 / AC-9） | 边界 | 既有 `顶层未知字段passthrough进extra_已知字段不进` / `多个未知字段完整保留且两次加载顺序稳定` / `未知字段值为超长字符串或深嵌套对象时原样无损保留` 全绿：`KNOWN_KEYS` 之外的键一律进 `extra` 且零诊断（含 `customObject` / `customArray` / `customText` / `customNull` 四值形态）—— 该行与 A1 断言共同钉住「7 键」口径 | 表驱动（既有 3 用例 × 4 值形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无新增 mock 面 | 零 mock：本变更零代码改动、零新增入参与依赖 | 全部行 |
| 配置文件（进程边界：数据文件） | 不 mock：沿用既有 `TempWs`（`std::env::temp_dir()` + 进程 id + tag 的 RAII 临时根）真实落盘 / 真实删除 / 以同名目录占据路径制造不可读 | `load` 四分支与自洽不变量行 |

---

### packages/desktop/src/lib/explore-stance.ts -> packages/desktop/src/lib/explore-stance.test.ts

<!-- design.md 的「公共函数 / API」子表为空注释：「修改文件」表对本文件只列文件头注释块改**单源自持**（删「与插件 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链——改动需双侧同步」全部表述），两形文本与注入行为**零改**；design 未声明该文件的公共 API 变更。故本节**零新增用例**；既有 `explore-stance.test.ts`（6 个 `describe` / 16 条 `it`）全绿即回归判据，下方 `#### 待测功能` 行为**既有 API 的回归锚点**。注意：`DRAFT_NAME_PREFIX` 的「与后端 store `EXPLORE_PLACEHOLDER_PREFIX` 双源互链」doc 属**保留面**（D14 同类边界），不在清面范围。 -->

#### 待测功能

- `buildExplorePrompt(userInput: string, recordName: string): string`（既有导出，本变更零改；回归锚点）：stance 前导按 `recordName` 是否命中 `draft-` 前缀分两形 + 分隔线 + 用户输入原文拼接 —— 回归
- `DRAFT_NAME_PREFIX = 'draft-'`（模块私有常量，本变更零改；回归锚点）：前缀判别锚点 —— 回归

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `buildExplorePrompt` — 命名指令形回归 | 正向 | 既有 3 条 `it`（kebab 最终名指引 / `create_explore_note(name, content)` 定名叙事与路径续用 / 零路径插值）全绿 | 表驱动（既有 3 用例） |
| `buildExplorePrompt` — 笔记定位形回归 | 正向 | 既有 2 条 `it`（插值 `openspec/explores/<name>.md` / `edit` 指引与缺失同名补建句）全绿 | 表驱动（既有 2 用例） |
| `buildExplorePrompt` — 两形公共段共享回归 | 正向 | 既有 `两形公共段共享` describe 全绿：立场五条与两行笔记约定在两形输出中各出现一次 | 遍历枚举（既有锚点集） |
| `buildExplorePrompt` / `DRAFT_NAME_PREFIX` — 前缀判别精确性回归 | 边界 | 既有 3 条 `it`：`my-draft-thing`（中缀，非前缀）→ 定位形；`draft-`（空尾）→ 命名形；`draft-<毫秒>` → 命名形 | 表驱动（3 形态） |
| `buildExplorePrompt` — 拼接与输入边界回归 | 边界 | 既有 4 条 `it`：`""` 空输入（前导 + 空尾，不抛错）/ 特殊字符（换行、引号、反引号、emoji 原样保留在尾部）/ 超长 >1000 字符（无截断）/ 前导恰出现一次 | 参数化（4 形态） |
| `buildExplorePrompt` — 非法记录名判别回归 | 异常 | 既有 3 条 `it`：大写 / 下划线 / 路径形态非 `draft-` 前缀 → 定位形；`draft-` 前缀的非法 kebab 仍命中命名形；空 `recordName` → 定位形插值空名、不抛错 | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：`buildExplorePrompt` 为纯字符串拼接纯函数，无 IO、无进程边界依赖；既有用例亦零 mock | 全部行 |

---

### packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs -> packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs

<!-- design.md 的「公共函数 / API」子表为空注释：「修改文件」表对本文件只列三处措辞（`:20-22` 头注释「CLI zod 权威」→「已固化 `core/checks` 实现 + 入仓 golden 金样」；`:291` doc；`:311` 断言文案改「与既有金样口径一致」），**断言值、语料清单、行为零改**（design D5；`tasks.md` 阶段四自查 C 同口径）；design 未声明该文件的公共 API 变更。本文件是 `checks` crate 的集成测试载体（`tests/corpus_golden_test.rs`），被测实现为 `crates/core/checks/src/{lib,parser,model,aggregate}.rs` 的既有公共面（本变更对其零改），且无独立单测文件 → 映射对两侧同为自身；`#### 用例` 表登记既有 15 条显式 + 8 条 macro 用例的回归面，零新增用例。`assert_eq!` 的 message 字符串不参与断言判定（仅失败时渲染），故不为文案改动设用例。 -->

#### 待测功能

- `project_corpus_fixture(fixture: &str) -> Value`（既有测试内私有，本变更零改；回归锚点）：全链投影（宽容解析 → 完整性 checklist → 诊断树 → 覆盖聚合），确定性字段全量（时间戳等墙钟字段不入投影）
- `check_or_rewrite(golden_name: &str, projection: &Value)`（既有测试内私有，本变更零改；回归锚点）：`DESKTOP_GOLDEN_REWRITE` 缺省时与入仓 golden 逐字节对比；开关置位时覆写（本变更两分支行为均零改）
- `suites_for(fixture: &str) -> Vec<config::TestSuite>`（既有测试内私有，本变更零改；回归锚点）：三档绿跑语料的 suite 阈值装配（`Serialize`-only 类型显式字面量构造）
- `project_seg_fixture(fixture: &str) -> Value` / `check_or_rewrite_seg(golden_name: &str, projection: &Value)`（既有测试内私有，本变更零改；回归锚点）：段级（mutation）语料投影与段级 golden 对拍

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `project_corpus_fixture` + `check_or_rewrite` — 八套语料 golden 对拍回归（AC-3 相关口径 / AC-9） | 正向 | 8 条 macro 生成的 `golden_*` 用例全绿：`green-istanbul` / `green-llvm-cov` / `green-node-test` / `red-run` / `mutation-block` / `unknown-fields` / `corrupt-invalid-json` / `corrupt-missing-required` 的投影与入仓 golden 逐字节相等 | 表驱动（8 语料） |
| `project_corpus_fixture` — 三档绿跑投影结构回归（AC-9） | 正向 | 既有 3 条用例（`全链绿跑istanbul_conclusion_pass且golden一致` / `全链绿跑llvm_cov_conclusion_pass且聚合面一致` / `全链node_test文本解析计数与golden一致`）全绿：`summary.conclusion` / `subReport.summaryCounts` / `problemTypes` / `integrity` 与金样一致 | 表驱动（3 用例） |
| `project_corpus_fixture` — 损坏样本宽容降级回归（AC-9） | 异常 | 既有 `损坏样本unparsable标记在golden可见` 全绿：`subReport.outcome == "unparsable"`、无 `summary` 面（不产半截子报告） | 表驱动（2 语料） |
| `project_corpus_fixture` — 红跑 / mutation 块 / 未知字段回归（AC-9） | 异常 | 既有 3 条用例全绿：`red-run` 的 `conclusion=fail`、逐条 `test_failure` 归并与聚类诊断文案不变；`mutation-block` 块保真；`unknown-fields` 宽容忽略对拍一致 | 表驱动（3 语料） |
| `suites_for` — 三档阈值装配回归（AC-9） | 边界 | 既有绿跑用例全绿：`vite-plus`(80/70/75) / `rust`(70/65/60) / `node-test`(60/70/75) 三档阈值经投影 coverage 面反映；损坏 / 红跑样本走 `Vec::new()`（suites 不入投影） | 表驱动（4 形态） |
| 语料清单一致性回归（D14 零触碰面） | 边界 | 既有 `语料完整性_corpus目录与清单表逐项对应` / `语料完整性_golden目录与语料集合一致` 全绿（`tests/fixtures/**` 与 `tests/golden/*.json` 本变更零改） | 单值 |
| 段级 golden 回归（AC-9） | 边界 | 既有 5 条段级用例（`golden_mutation_seg_cargo_映射表样本全链投影` / `golden_mutation_seg_stryker_metrics面全链投影` / `段级未知outcome样本runtimeError留痕` / `段级golden重写纪律_开关缺省下文件在位` / `段级语料清单_目录与清单一致且规模下限`）全绿 | 表驱动（5 用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `DESKTOP_GOLDEN_REWRITE`（运行环境 / 全局变量，进程边界） | 不 mock、不置位：用例恒以**开关缺省**态跑对比分支；`std::env::var` 真实读取，MUST NOT 在用例内 `set_var`（并行测试下进程级写入互相污染）。写入分支由既有 `段级golden重写纪律_开关缺省下文件在位` 承接 | 全部 `check_or_rewrite` / `check_or_rewrite_seg` 行 |
| 语料与 golden 文件（进程边界：数据文件） | 不 mock：真实入仓文件（`env!("CARGO_MANIFEST_DIR")` + `tests/fixtures/corpus` / `tests/golden` / `tests/fixtures/mutation-seg` / `tests/golden-mutation-seg`），本变更零改写 | 全部行 |
| `config::{TestSuite, CoverageThresholds, MutationConfig, TestFramework}` | 不 mock：真实构造（`Serialize`-only 类型的字面量装配） | `suites_for` 行与绿跑用例 |

---

## 不可测试项

| 项 | 原因 | 承接方式 |
|---|---|---|
| 1. AC-1 四类删除（`plugins/**` 237 tracked + `dist/**` 124 产物 + 2 marketplace + 50 份语料目录，及未 tracked 残留） | 删除面，无被测代码（design「删除清单」）；本设计不将删除文件纳入被测范围 | 同提交 `git log --stat` 复核（AC-1 判据）；`git ls-files plugins dist \| wc -l` = 0、两 marketplace 路径不存在、空目录不保留 |
| 2. AC-2 语料清点（35 → 归档后 36、无空目录、无 0 字节 `spec.md`） | 目录 / 统计面 | 归档相位复跑 AC-2 原命令（design D12 分层判据）；`find openspec/specs -type d -empty` 与 `-name spec.md -size 0` 均为空 |
| 3. `.gitignore` 删 3 行（`# 构建相关` + `plugins/*/.pack-staging/` + `*/dev-team/bin/*.cjs.map`） | 无运行时语义（忽略规则只对被忽略对象生效；`git ls-files \| grep -c 'cjs\.map\|pack-staging'` = 0） | `git diff .gitignore` 复核：仅 3 行删除，其余零改（含保留 `!.claude-plugin` / `!.cursor-plugin` 两条否定项，D8） |
| 4. `README.md` 整篇重写（安装 / 开发 / 构建 / 发版四段 + 仓库结构 + 项目配置 + 历史一句） | 文档面，无断言载体 | AC-6 人工审查（四段最小集在场、无插件安装手册段落、历史一句不含 `plugins/dev-team` 字面量、项目配置段与 `openspec/config.json` 现值逐字同值）+ AC-3 grep 零命中 |
| 5. `AGENT.md` 整篇重写（四段正文，去 `DEPRECATED` 前言） | 文档面 | 同 4 + `diff <(tail -n +3 AGENT.md) <(tail -n +3 CLAUDE.md)` 仅标题 / 面向对象句差异（D10） |
| 6. `CLAUDE.md` 整篇重写（与 `AGENT.md` 同正文） | 文档面 | 同 5 |
| 7. `.../core/checks/tests/fixtures/README.md` 引用块措辞（`:3-4`） | 文档面；语料清单表与样本零改 | `git diff --stat` 仅该 README 一处 + AC-3 grep 零命中；语料不变性由 `语料完整性_*` 两条用例承接 |
| 8. `.../core/checks/tests/golden/README.md` 两行措辞（`:4` / `:26`，design 盘查补正 #1） | 文档面；重写开关与文件集约定零改 | 同 7 |
| 9. `openspec/specs/desktop-phase-prompts/spec.md` 的 `## Purpose` 直改（14→16 计数 + 去插件路径句） | 语料面（Purpose 不由 delta 承载，design 事实校准 4） | AC-7 审查：Purpose 计数与磁盘 `crates/core/workflow/src/prompts/` 16 份 md 一致（实测 16）；AC-3 归档相位零命中 |
| 10. `openspec/specs/desktop-change-create/spec.md` 的 `## Purpose` 直改（删「插件面零改动」分句） | 同上 | 同 9 + 「MUST NOT 产出 `workflow.json`（双向墙）」条款在场 |
| 11. AC-4 的三段机械核对：镜像逐字一致（迁出前 `diff` 零输出）、`$schema` raw URL 尾路径与仓库内现存路径同形、`write_protection` 目标 `packages/desktop/knip.json` 在案且 `git check-ignore` 无命中 | 文件级 / 路径级核对面，无单元测试读取面（镜像本体已由首节用例覆盖结构与可解析性） | `tasks.md` 阶段一自查 A / 阶段二 B 段 / 阶段七自查 C ① 的 `diff` / `ls` / `git check-ignore` 命令；`git check-ignore -v packages/desktop/knip.json` 须 exit=1 |
| 12. AC-7 的 delta 形态合规（14 份 delta、MODIFIED header 与主 spec 逐字一致、scenario 标记恰 4 个 `#`、`desktop-phase-prompts` 14→16、`desktop-change-orchestration` 边界四条） | 语料结构面（非单元测试；由 OpenSpec 校验链与静态检查承载） | `openspec validate` / 桌面端静态检查脚本（agent 内不重复执行）+ 归档链 spec 同步段；`ls openspec/changes/retire-dev-team-plugin/specs \| wc -l` = 14 |
| 13. AC-8 新能力 `repo-surface-scope` 三条 requirement + scenario | 语料面（归档相位才落 `openspec/specs/repo-surface-scope/`） | 归档后核 `openspec/specs/repo-surface-scope/spec.md` 三条 requirement 与 scenario 在场（「退役整块下线」/「无空壳」/「活跃面无悬空路径 + 配置指针零命中且可达」） |
| 14. AC-9 的门禁统计面（覆盖率 lines ≥ 90 / branches ≥ 85 / functions ≥ 90、mutation 80 / 70、`bindings:check` 零 diff、`cargo test -p checks-runtime --lib` 不回归 flake 面） | 统计与执行面（静态脚本由桌面端执行，agent 内不重复执行） | test-execution 相位门禁；本变更零新增导出 / 零新依赖 / 零 IPC 与 DTO 变动 → `bindings:check` 零 diff 的结构依据（唯一例外为 `bindings.ts` 的 1 行 doc 投影，须入索引，见 19） |
| 15. AC-10 版本交付（`packages/desktop/package.json` `0.5.30` → `0.5.31`；`tauri.conf.json` 经 `../package.json` 跟随；`Cargo.toml` 不随动） | 数据面（JSON / TOML 单字段），无运行时测试路径 | `git diff packages/desktop/package.json` 单行；`tauri.conf.json` 的 `version` 恒为 `"../package.json"`；`git diff src-tauri/Cargo.toml` 零条目 |
| 16. `crates/core/config/src/lib.rs` 的 12 处 doc 注释直述化 | 源码文本面（注释不参与编译语义，无断言可捕获漂移） | `tasks.md` 阶段四自查 + `git diff crates/core/config/src/lib.rs` 仅注释行（零代码行变更）+ AC-3 grep 零命中；行为不变性由该映射对的既有 40 条用例全绿承接 |
| 17. `packages/desktop/src/lib/explore-stance.ts` 文件头注释块改单源自持 | 源码文本面（注释；`DRAFT_NAME_PREFIX` 的「与后端 store 双源互链」doc 属**保留面**，MUST NOT 顺手清，D14） | `git diff packages/desktop/src/lib/explore-stance.ts` 仅注释行 + `git grep -n "plugins/dev-team" -- packages/desktop/src/lib/` 零命中；两形文本与注入行为零变由既有 16 条 `it` 全绿承接 |
| 18. `.../core/checks/tests/corpus_golden_test.rs` 三处措辞（`:20-22` 头注释 / `:291` doc / `:311` 断言文案） | 断言 message 不参与断言判定（仅失败时渲染），无非零断言面 | `tasks.md` 阶段四自查 C（`assert_eq!` 期望值零改）+ 既有 15 条显式与 8 条 macro 用例全绿（语义零变承接） |
| 19. `packages/desktop/src/types/generated/bindings.ts` 的 `TestFramework` doc 1 行随动（生成物，D18） | 生成物面：由 `bindings:export` 产出，手写单元测试无意义（且生成代码不在 `src/**` 手写测试面） | `pnpm -C packages/desktop run bindings:check`（`git diff --exit-code -- src/types/generated`）零输出；生成物须 `git add` 入索引；`TestFramework` 八值串由首节 A2 / A3 断言钉在源侧 |
| 20. `openspec/config.json` 两处指针（`:2` `$schema` 换 URL、`:23` `write_protection.files[0].glob` → `packages/desktop/knip.json`）与 AC-11 双段判据 | 配置数据面（写保护内置锁定面的对象：`rules::builtin_rules` 派生 `**/openspec/config.json`，fail-closed、无 override）；仓内无单元测试读取该文件（`crates/**` 的 `openspec/config.json` 命中项均为 tempdir fixture） | **第二段（现行实况）**：同一 `grep` 零输出、`git diff --numstat` = `2 2`、两值等于 design「配置指针（已落盘）」表「新值」列（`$schema` 尾路径 `packages/desktop/src-tauri/crates/core/config/config.schema.json`、`glob` = `packages/desktop/knip.json`）、`protect/**` 零 diff。**第一段（基线，对改写前副本复跑）**：命中数 = 2 且逐值等于同表「旧值」列。MUST NOT 以「移出核对面」替代记录（D17），MUST NOT 为过判据改产品保护语义或样本 |
