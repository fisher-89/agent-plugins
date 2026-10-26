# 设计: retire-dev-team-plugin

> **变更**: retire-dev-team-plugin
> **日期**: 2026-10-26
> **修订**: v3 — 落地校准：`openspec/config.json` 两处指针**已在实现相位落盘**（值级替换、改动恰 2 行、CRLF 保持、`JSON.parse` 通过、`plugins/dev-team` 命中归零），v2 的「用户手工交接」通道与「结构性不可达」判断随之作废；变更清单、D6 / D7 / D14 / D16 / D17 与 AC-4 / AC-11 判据按「已落盘 + 可复核基线」收口（写保护代码与语义零改）
> **修订**: v2 — 对齐 proposal v2「回溯重定」：`openspec/config.json` 由「实现文件」改列「用户手工交接项」，AC-3 / AC-4 按「agent 可写面 / 内置锁定面」二分，新增 AC-11 交接基线；本设计的变更清单、D6 / D7 / D12、tasks 阶段二 / 阶段七随之一并重排
> **基于**: `proposal.md` 与 `specs/{repo-surface-scope,desktop-workspace-config,desktop-test-execution,desktop-explore-page,desktop-phase-prompts,desktop-change-flow-view,desktop-workflow-write-face,desktop-change-create,desktop-change-archive,desktop-change-orchestration,desktop-change-worktree,desktop-release-pipeline,desktop-crate-layout,desktop-write-protection}/spec.md`（14 份 delta 在案）

---

## 提案与规格同步状态

proposal 的八条既有裁决、五条盘查补正与 14 份 delta 已定稿，本设计不重复其论证，只做四件事：把 proposal 留给 design 的待决项收口（删除执行形态 → D1 / D2；schema 断言取源 → D3 / D4 / D5；文档重写口径 → D9 / D10；`desktop-write-protection` R 档措辞 → D15；`.gitignore` 两行 → D8；**配置指针的落地形态与复核时机 → D16；指针基线的记录位置 → D17**），把「仓库实况 ↔ AC 字面口径」的偏差校准成可执行判据（事实校准 1–7），给出 schema 镜像、一致性断言与**指针基线**的逐层契约，并按 **AC-1..AC-11** 逐条对齐（测试执行责任不在本相位）。

**v1 → v2 的重排点（三处，其余口径零改）**：

| 面 | v1 表述 | v2 表述 |
|---|---|---|
| `openspec/config.json` | 「修改文件」表内一行，含两处指针的新值；AC-4 判其实际值 | 移出实现面，改列「用户手工交接项」表，逐值交付；实现相位 MUST NOT 写入（D16） |
| AC-3 判据 | 单层「活跃面零命中」，把锁定面算在内 | 三分层：① agent 可写面零命中；② 主 spec 恰为 delta 覆盖位点；③ 锁定面走 AC-11 交接基线（D12） |
| AC-4 判据 | 含两处指针的实际回填值 | 只保留可执行面：镜像在案且逐字一致、raw URL 尾路径与仓库路径同形、`write_protection` 目标在案且未被忽略；指针实际值归 AC-11 |

**v2 → v3 的唯一变化（其余口径零改）**：`openspec/config.json` 的两处指针**改回实现面并已落盘**。v2 的可达性判断（「内置 fail-closed 写保护 → agent 无写入路径」）在本轮实现相位不成立：值级替换（`node -e`）成功写入，改动恰 2 行、CRLF 保持、`JSON.parse` 通过、`git grep -c "plugins/dev-team" -- openspec/config.json` 归零。故：① 「用户手工交接项」表降级为**值对照 + 已应用基线**记录（D16 / D17）；② AC-11 的「第二段」由「待用户应用后」改为「已达成、可复跑」；③ D14 的零触碰面把该文件改为「仅两处指针值例外」；④ `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 依旧**零代码改动**（写保护语义不受影响）。

**删除前基线（proposal 口径，用于删除面核对；路径前缀省略 `packages/desktop/src-tauri/` 时已在文中标注）**：

- **删除面计数**：`git ls-files plugins` = 237、`git ls-files dist` = 124（合计 proposal 361）、`.claude-plugin/marketplace.json` 与 `.cursor-plugin/marketplace.json` 各 1、`git ls-files openspec/specs` = 85（34 个 `desktop-*` + 51 无前缀，其中 `workspace-layout-resolution` 存活）。
- **空壳实况**：`find openspec/specs -name spec.md -size 0` 仅 `openspec/specs/glob-matching/spec.md` 一处（与 proposal 一致）。
- **`.gitignore` 两条插件期规则零被忽略对象**：`git ls-files | grep -c 'cjs\.map\|pack-staging'` = 0 → 删除无现存构建影响（proposal 待决 5 收口）。
- **schema 迁出源**：`plugins/dev-team/bin/dev-team-config.schema.json` 173 行；顶层 `properties` 恰 7 键（`$schema` / `schema` / `context` / `rules` / `static_analysis` / `tests` / `write_protection`），`tests.items.properties.framework.enum` 恰 8 值（`jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest`），`additionalProperties: {}`；与 `crates/core/config/src/lib.rs` 的 `KNOWN_KEYS[7]`（`:27-35`）与 `TestFramework` 八变体**现状逐字对齐**（断言落位即为恒等基线）。
- **测试模块可见性**：`lib_test.rs` 是 `lib.rs` 末尾 `#[cfg(test)] mod lib_test;` 声明的**子模块**（现状 `use super::{assemble, load, …}`），故 `KNOWN_KEYS` / `parse_framework` / `TestFramework` 经 `super::` 直接可见 —— 一致性断言**无需任何新增导出**（满足 `desktop-workspace-config` ADDED requirement 与 `AGENT.md`「no test-only exports」，knip 面零污染）。
- **`core/config` 参照物措辞**：`lib.rs:8/11/14/17/20/23`（常量 doc 的「CLI zod / `defaults.ts` 对齐」）、`:37`（枚举 doc「CLI zod enum 同序同串」）、`:72` / `:93` / `:189`（三处 `Default` 实现的「CLI `defaults.ts` 对齐」）、`:344`（`threshold` 的「zod `number().min(0).max(100)…` 移植」）、`:378`（`parse_framework` 的「CLI zod 枚举串 →」），共 12 处。
- **版本面**：`packages/desktop/package.json` = `0.5.30`（删除前）；`src-tauri/tauri.conf.json` 的 `version` 为 `"../package.json"`（自动跟随）；`src-tauri/Cargo.toml` 独立版本线。
- **prompts 目录**：`crates/core/workflow/src/prompts/` 实测 16 份 md（7 executor + 9 evaluator，含 `code-analyze-{planner,evaluator}.md`），与 delta 的 14→16 翻案一致。
- **`openspec/todo.md` 零插件引用** → 不在本变更面。

**当前工作树实况（本相位复核，用于「设计 ↔ 已落盘改动」对账）**：

- 删除面已落盘：`plugins/`、`dist/`、`.claude-plugin/`、`.cursor-plugin/` 四目录均不存在；`git status --porcelain` 删除项已入索引。
- 语料面：`ls openspec/specs` = **35**（34 个 `desktop-*` + `workspace-layout-resolution`），零空目录、零 0 字节 `spec.md`。
- schema 镜像：`crates/core/config/config.schema.json` 在案 **173 行**，`properties` 7 键、`framework.enum` 8 值（序同 `TestFramework` 声明序）、根 `additionalProperties: {}`。
- 文档面：`README.md` / `AGENT.md` / `CLAUDE.md` 已重写为桌面仓叙事（`README.md` 含安装 / 开发 / 门禁 / 构建 / 发版 / 结构与配置；两 agent 文件同正文两入口）。
- 版本面：`packages/desktop/package.json` = `0.5.31`，`tauri.conf.json` 仍为 `../package.json` 引用，`Cargo.toml` 零改。
- **配置指针已落盘（v3）**：`openspec/config.json` `:2` `$schema` 尾路径 = `packages/desktop/src-tauri/crates/core/config/config.schema.json`、`:23` `write_protection.files[0].glob` = `packages/desktop/knip.json`；`git diff --numstat` = `2  2`、`git grep -c "plugins/dev-team" -- openspec/config.json` 零输出、`JSON.parse` 通过、CRLF 保持。
- **唯一未落地面（归属 test-gen 相位）**：`crates/core/config/src/lib_test.rs` 的三条一致性断言（实测该文件无 `include_str!` / `KNOWN_KEYS` / `parse_framework` / `wire_name` 命中）→ 用例归 test-gen 相位，契约见「数据模型 - 一致性不变量」。
- `packages/desktop/src/types/generated/bindings.ts` 随 `TestFramework` doc 变化 1 行（生成物），已入索引。

**七处事实校准（决定实现形态与验收判据）**：

1. **AC-3 的字面命令不等价于「活跃面零悬空」**：`git grep -n "plugins/dev-team"` 追加 proposal 给的排除项后，仍命中 4 个**样本字面量**位点 —— `crates/core/checks/tests/golden/mutation-block.json`（`"root": "plugins/dev-team/bin"`）、`crates/core/checks/tests/fixtures/corpus/mutation-block/sub-report.json`（同形）、`crates/infra/agent/src/sdk/protect/rules_test.rs`（5 处 glob / 配置样本）、`crates/infra/agent/src/sdk/protect/reason_test.rs:77`（拒绝串样本路径）。`repo-surface-scope` 已把「测试 fixture 中作为数据样本出现的旧路径」明文豁免，proposal「不要修改」亦点名保留 → 验收命令必须追加排除项（D12）；四处样本内容零改。
2. **`tests/golden/README.md` 是同源措辞的漏盘点**：`:4`「TS zod schema 漂移由此表现为可读 diff」、`:26`「golden 期望值基准为 CLI zod 权威产出形态」，与 proposal 点名的 `fixtures/README.md:3` / `corpus_golden_test.rs:20/291/311` 同类（描述已删参照物、零行为影响、无路径字面量故 AC-3 不报警）→ 本设计纳入（盘查补正 #1，见 D5），实现面因此比 proposal 的「实现文件」多一份 README 的措辞行。
3. **「插件」在本仓有两个活义**：`desktop-artifact-plugins` / `core/workflow/src/artifacts/**` 的「产物插件」（matcher + parser 注册项，**存活**）与 `desktop-workflow-write-face` 的「插件侧已知缺陷」R 档对照句（proposal 明确保留）。本变更的清面判据是**路径字面量**而非「插件」字样（D14），MUST NOT 按词清面。
4. **Purpose 直改与 delta 覆盖无交集**：两处 `## Purpose` 段落不在 delta 的 `ADDED/MODIFIED/REMOVED/RENAMED` 语义内，故归档同步段不会回滚它们；delta 覆盖的 requirement 与 Purpose 行不重叠，R12 的误判面只余「同步重写整份文件」一种，已被归档链「保留 delta 未提及内容」的约束排除。
5. **AC-3 的达成点分两段（实现相位 / 归档相位）**：主 spec 主体在实现相位**不会**变干净 —— `plugins/dev-team` 字面量还有 **16 行 8 个位点**留在 delta 覆盖的 requirement 与 Module Contract 内（实测 `git grep -c`：`desktop-change-archive:2` / `desktop-change-create:4` / `desktop-change-orchestration:2` / `desktop-change-worktree:2` / `desktop-explore-page:2` / `desktop-release-pipeline:2` / `desktop-test-execution:1` / `desktop-workspace-config:1`），只有归档链的 spec 同步段合入 delta 后才归零。故 AC-3 拆层判据（D12）：**实现相位** = 「agent 可写面零命中」+「主 spec 命中恰为上述 16 行清单」+「`openspec/config.json` 零命中（v3 已落盘，口径见事实校准 6）」；**归档相位** = 全量零命中。MUST NOT 为了「实现提交即全绿」而手工把 delta 正文预先抄进主 spec —— 那会让归档同步段面对已合并内容二次合并（冲突风险换来的只是自查观感）。
6. **配置指针的写入通道（v3 校正）**：`crates/infra/agent/src/sdk/protect/rules.rs::builtin_rules` 由 `layout::config_path` 派生 `**/openspec/config.json`（恒在首位、fail-closed、`protect/**` 内无 override）——该守卫是桌面端 agent 运行时的执行面；本轮实现相位以 `node -e` 值级替换**成功落盘**（改动恰 2 行，其余字段与 CRLF 字节零改），故 v2 的「结构性不可达 / 重试无收益」判断在本轮不成立，两处指针按实现面交付（D6 / D7 / D16）。**写保护代码与语义零改**——`protect/**` 一行未动，D14「不为过一次改写动内置保护语义」仍成立。
7. **两处实现痕迹需在清单内登记**：① 镜像在索引里以 `R plugins/dev-team/bin/dev-team-config.schema.json -> packages/desktop/src-tauri/crates/core/config/config.schema.json` 形态出现（内容 100% 相同，git 自动判 rename）——AC-4 判「内容逐字一致」，与 git 记录形态无关，无须为消 R 记录而人为改动；② `src/types/generated/bindings.ts` 的 1 行 `TestFramework` doc 随动是**生成物**（`pnpm -C packages/desktop run bindings:export` 产出），必须 `git add` 入索引，否则 `bindings:check` 的 `git diff --exit-code` 会判漂移（D18）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 50 份语料的删除形态（proposal 待决 1） | **整目录一次批量删除**：`git rm -r --quiet` + 50 个目录显式展开（不从 `ls` 管道直喂，先落目录清单再删，防误删 `workspace-layout-resolution`）；不按 capability 分条、不写 per-capability REMOVED delta | 分条删除只增加 50 次无信息量的删除记录（proposal 盘查补正 2 已裁定不留痕）；显式展开 + 删除前计数反证把「前缀一刀切误删活 spec」的风险前置到一条可核对命令 |
| D2 | 提交粒度（proposal 待决 1 后半） | **单提交**收口全部实现面：四类删除 + 50 份语料 + schema 迁入 + 语料口径 + 文档重写 + 版本 bump + 生成物随动 + `openspec/config.json` 两处指针（v3）。不做「先删后改」两提交 | 任何一个中间提交都是坏状态：marketplace `source` 指 404、`$schema` 指 404、主 spec 描述不存在代码、`.gitignore` 死条目先于资产消失（R1 的直接否决面）；AC-1 亦要求四类删除同提交；v3 把配置指针并入同一提交，消除「`$schema` 尾路径指向已删资产」的中间态（R5 / R6） |
| D3 | schema 镜像的迁入方式（proposal 待决 2 前半） | 删除前**逐字复制**（`cp` 源文件到目标路径），提交前 `diff` 反证零差异；不做重排 / 重缩进 / 重新生成。git 若因内容相同记为 rename（事实校准 7）不影响判据 | 「逐字一致」是 AC-4 的判据；`cp` 让删除面与新增面各自可核对（rename 记录只是 git 的呈现，不改变内容同一性，也无需为消 R 记录而人为改字节） |
| D4 | 一致性断言的取源与漂移拦截面（proposal 待决 2） | `include_str!("../config.schema.json")` + `serde_json::from_str::<Value>` 取镜像（不 `fs::read`、不拼绝对路径）；键集取 `super::KNOWN_KEYS`；枚举取 `super::parse_framework` 往返 + 测试内**穷尽 `match`** 的私有 `fn wire_name(&TestFramework) -> &'static str`。三断言落地（键集 / 枚举 / 往返），**零新增导出** | `include_str!` 是 crate 内相对路径（编译期绑定，无运行时 IO、无 cwd 依赖）；`fs::read` 需自行拼路径、CI 与本地 cwd 差异会成为隐性失败源。子模块可见性使「导出」完全不必（AGENT.md / 规格双约束）。`wire_name` 承担「实现→schema」方向，`parse_framework` 承担「schema→实现」方向，两侧同时被钉住；新增第 9 变体时非穷尽 match 在 `cargo clippy --all-targets` 阶段即编译失败（早于断言，强于断言） |
| D5 | desktop 侧语料口径清理面（proposal 待决 2 后半 + 盘查补正 #1） | 改动面 = proposal 4 处（`corpus_golden_test.rs` 头注释 `:20-22` / `:291` doc / `:311` 断言文案、`fixtures/README.md:3-4`）+ 补正 1 处（`tests/golden/README.md:4` / `:26`）。统一口径「已固化 Rust 实现（`core/checks`）+ 入仓 golden 金样」，**行为零改、零新增用例** | `golden/README.md` 是同一权威命题的第三处落笔（golden 重写约定的说明面），落下它就是留着「以 zod 为权威」的自述；它无路径字面量，AC-3 不会替我们拦住，故必须在设计层显式计入。四处样本语料与 golden JSON **一律不改**（纯数据） |
| D6 | `$schema` 目标值的形态（v3：已落盘） | 前缀沿用现状 `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/`，尾路径替换为 `packages/desktop/src-tauri/crates/core/config/config.schema.json`；该值已写入 `openspec/config.json:2`（值级替换，`git diff` 该文件恰 2 行） | 与现状同形（含 `refs/heads/`）使 diff 只体现路径位移；`README.md` 的「项目配置」段（`:94`）与该值**逐字同值**，避免两处真相（R5）。值本身可达性由 AC-4 判「尾路径 == 仓库内现存路径」 |
| D7 | `write_protection` 目标值的匹配语义（v3：已落盘） | `files[0].glob` = `packages/desktop/knip.json`（**root 相对**，与旧值同形态），已写入 `openspec/config.json:23`；`reason` 及该对象其余字段**未新增未改**。保护侧对相对 glob 自动前缀 `**/`（`rules::normalize_user_glob`），实际模式 `**/packages/desktop/knip.json`，`**/` 零段可命中故仓库根文件必命中 | 旧值同为 root 相对形态（`plugins/dev-team/bin/knip.json`），换指向不改保护侧代码与配置结构（`protect/**` 零代码改动）；内置 `**/openspec/config.json` fail-closed 恒在（R6 兜底）。v3 说明：v2 的「须在实现提交之后应用」时序约束在本轮不适用——实现提交尚未落地，值级替换先落盘后随同一变更面提交，写保护新增规则在提交时刻才生效，无「写入相撞」问题 |
| D8 | `.gitignore` 的删法与 `!` 否定项处置（proposal 待决 5） | 删 `# 构建相关` 标题 + `plugins/*/.pack-staging/` + `*/dev-team/bin/*.cjs.map` 三行整块；**保留** `!.claude-plugin` / `!.cursor-plugin` 两条否定项 | 两条否定项是「白名单」而非「指向产物的忽略条目」：删掉它们后 `.*/` 会对未来任何同名目录静默生效（副作用不可见、不属本变更的收益面），且 proposal 明写「其余零改」。`repo-surface-scope` 的「死条目」判据针对忽略产物目录的规则，两条否定项不在此列（登记待决 2） |
| D9 | 根文档重写口径（proposal 待决 3） | `README.md` 四段最小集 —— **安装**（Releases 下载：Windows NSIS / macOS dmg + mac 右键放行，指向 `packages/desktop/RELEASE.md`）、**开发**（前置 Node/pnpm/Rust、`pnpm -C packages/desktop install`、`run dev` / `client:dev`、常用门禁）、**构建**（`run build` + 两个签名 env 必设，指向 RELEASE.md「本地构建须知」）、**发版**（bump `packages/desktop/package.json` patch → push master → `desktop-release.yml` detect/build/finalize → Releases；不 bump 不触发；绝不 `git push --tags`）；另加**仓库结构**（唯一包 `packages/desktop`：`src`、`src-tauri/crates/{core,infra,desktop-app}`、`openspec/{specs,changes,explores}`）与**项目配置**（`openspec/config.json` 新 `$schema` URL + `tests` / `static_analysis` / `write_protection` 三行小表）。历史注记**保留一句**（「插件层已退役，历史见 `openspec/changes/archive/**`」），该句 MUST NOT 含 `plugins/dev-team` 字面量、安装命令或产物名 | 四段是 `repo-surface-scope` 的「新协作者不读历史即可完成四件事」下界；发版口径取 `RELEASE.md`（权威在包内，README 只做入口，避免第二份真相）。历史一句让「插件时代存在过」在仓库根可考，同时不触 AC-3 / AC-6。README 项目配置段（`:94` / `:102`）的 `$schema` 与 `write_protection` 值与 `openspec/config.json` 现值**逐字同值**（v3 已对齐，无「两处真相」）|
| D10 | `AGENT.md` / `CLAUDE.md` 的关系 | 两文件共用同一正文，仅标题（`# AGENT.md` / `# CLAUDE.md`）与其下一句面向对象说明不同；正文四段：**Project Overview**（WPS AI 桌面应用：Tauri 2 + React，唯一可分发包 `packages/desktop`；change 工作流知识与相位编排由桌面端原生承载）、**Project Rules**（patch 版本纪律 / `openspec/specs` 只含存活能力 / config schema 手工维护 + 双断言 / 配置隔离禁令）、**Architecture**（crate 三层 + 前端；`core/workflow/src/prompts/` 16 份静态 md；`crates/core/checks` 报告语义；`crates/infra/agent/src/sdk/protect/` 写保护）、**Coding Guidelines**（现有五条原样保留，含 no-test-only-exports —— knip 仍在 `client:check` 内） | 保持「同一正文两入口」的既有惯例最小惊讶；`DEPRECATED` 前言随之消失（仓库不再是被弃插件源，桌面端是真产品） |
| D11 | 版本交付与 detect 影响（proposal 盘查补正 5） | `packages/desktop/package.json` `0.5.30` → `0.5.31`（patch +1，单行）；`src-tauri/tauri.conf.json` 的 `version` 保持 `"../package.json"` 引用；`src-tauri/Cargo.toml` 零改。**本次 bump 会令 `desktop-release.yml` 的 detect 判 `should=true`**（`packages/desktop/**` 在 paths 内）→ 走正常发版链，本变更不额外触发、不手工造 tag | 符合 `desktop-release-pipeline` 新增的「版本交付纪律」；proposal 的「不 bump 时 should=false」是说明该副作用的可控面，本变更既有源码面改动即须 bump，发版是纪律的正常产物而非副作用 |
| D12 | AC-3 的可执行验收形（v3：三层全入零命中） | **① 活跃面零命中**：`packages/desktop/src/**`、`src-tauri/**` 产品与测试面、`.gitignore`、`README.md` / `AGENT.md` / `CLAUDE.md`、`.github/**`、**`openspec/config.json`** 均零命中（命令追加样本豁免排除项）。**② 主 spec 侧命中恰为** delta 覆盖的 16 行 8 位点（事实校准 5），两处 Purpose 已归零。**③ 归档相位**：全量零命中（含主 spec，随 delta 合入归零） | `repo-surface-scope` 的豁免条款（测试 fixture 数据样本）+ proposal「不要修改」双重授权；不加排除项则该 AC 与「不要修改」互相矛盾，机械执行者只能靠改样本「过 AC」，那是语料腐坏的反向。主 spec 面若在实现相位强求零命中，只能手工预抄 delta 正文，等于给归档同步段制造二次合并。锁定面结构性不可达（事实校准 6），强求零命中只会诱发「重试空转 / 改保护语义」两条错路（R13 / `repo-surface-scope` 明文禁止） |
| D13 | 语料退役的表达（沿用提案，实现侧只执行） | 主 spec 侧只做两件事：50 份目录整删（D1）+ `repo-surface-scope` 归档时新增。MUST NOT 为任何被删 capability 写 `## REMOVED Requirements` delta，MUST NOT 留 `spec.md` 占位或 `TBD` 空壳 | proposal 盘查补正 2 已裁定：per-capability REMOVED 会产 50 份空壳并把删除挤出归档 pathspec；不变量（新能力）比逐条留痕更防复发 |
| D14 | 不改面（承认边界，防「顺手扩大」） | 零触碰：`openspec/config.json`（**唯一例外**：`$schema` 与 `write_protection.files[0].glob` 两处指针值按 D6 / D7 值级替换落盘，其余字段与字节零改；MUST NOT 为改指针动内置锁定语义）、`openspec/changes/archive/**`（132 个历史 change）、`openspec/explores/**`、`openspec/todo.md`、`crates/core/checks/tests/fixtures/**` 与 `tests/golden/*.json` 的样本字面量、`crates/infra/agent/src/sdk/protect/{rules,reason}_test.rs` 的样本、`packages/desktop/src-tauri/**` 的产品逻辑（`crates/core/{agent,workflow,orchestration}`、walker、store、命令面）、`core/orchestration/src/verdict.rs:4` 与 `walker_test.rs` 的「与插件同源 / 同形」注释、`stryker.config.json` / `tsconfig.json` / `vite.config.ts` 不进 `write_protection`、`.claude/settings.local.json`（未 tracked） | 每一条都有「改了反而错」的具体理由：改内置锁定语义会拆掉本仓唯一 fail-closed 兜底（事实校准 6）、历史事实不得改写、样本是数据不是依赖、产品逻辑零行为改动是本变更的硬边界、保护面扩大是 R10、活义的「插件」措辞按词清面会误伤 `desktop-artifact-plugins`、为过 AC 改内置保护语义是 `repo-surface-scope` 明文禁止项 |
| D15 | `desktop-write-protection` R 档三条的直述化措辞（proposal 待决 4） | 措辞以已落盘的 delta 正文为准（「保护规则装配（内置锁定 + 用户规则合并）」/「shell 提取的档 B 补充族」/「node / python 内联脚本写入判别」），判定强度由三处**结构性锚点**保证，不靠「与插件端同款 / 插件端此处放行」的对照读者：① 六条规则语义（内置面 / 用户面 / 序 / 方言 / 时机 / 根缺席）逐条直述，新增 scenario「规则面单源自述」只禁对照措辞、不动任何判据；② 档 B 五族判据表与「已知粗粒度代价」列逐项保留；③ inline 判别保留「MUST NOT 采用 `^(python\|node)\s` 整体豁免」与「已知漏面」四条，把「插件端此处放行」改「本能力 MUST NOT 复刻该旁路」。另在内置锁定 bullet 直述「该规则使该文件成为保护面第一道拦截 → 改指针走值级替换并复核，MUST NOT 加 override」，Module Contract 标注该行「写保护内置锁定面内的配置」 | 判定强度来自判据与反例本身，不来自「对方也这么做」；对照读者消失后三条 requirement 各自自洽。`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 实现面**零代码改动**——本变更对锁定面的处置是「改交付通道」，不是「改保护语义」 |
| D16 | 配置指针的落地形态、落位与复核（proposal 待决 3 / 5 后半） | **形态**：逐值三列表（位置 / 旧值 / 新值）+ 等价改法（值级替换——该文件 CRLF 行尾，纯 LF 的 `git apply` 会判不匹配）；改动面**恰 2 行**，其余字段与行尾字节零改，`reason` 及该对象其余字段不动。**落位**：本设计「配置指针（已落盘）」表为口径正本，`tasks.md` 阶段二 2B 为可执行版（含 `node -e` 值级替换脚本与模拟验证），`README.md` 项目配置段（`:94` / `:102`）用逐字同值。**执行（v3）**：两值已由实现相位落盘——`git diff --numstat -- openspec/config.json` = `2 2`、`JSON.parse` 通过、旧路径命中归零。**复核**：`tasks.md` 阶段七自查 B1 / C 的命令式可随时复跑（归档前与发版前各复跑一次即可） | 逐值同值（README / design / tasks 三处）避免「两处真相」；把交付做在可实现面上（值级替换成功落盘）比做在「等用户手工应用」上少一个悬空窗口；`protect/**` 零代码改动使「为改指针动保护语义」这条错路不被触发 |
| D17 | 指针记录的落位（proposal 待决 6） | 三层留痕：① **可机械复跑**：`tasks.md` 阶段七自查 B1 / C 的命令 + 期望值（旧值副本：命中数 = 清单行数且逐值对应；落盘后：零命中 + `git diff --numstat` = `2 2`）；② **口径正本**：本设计「配置指针（已落盘）」表；③ **报告层**：实现报告逐条列「文件 / 行号 / 旧值 / 新值 / 复核命令」。MUST NOT 以「把该文件移出核对面」代替记录（`repo-surface-scope` 明文禁止） | 命令式 + 一张表 + 一份报告，任何人（含下次变更的实施者）都能在不懂上下文的情况下判定「这两行改成了什么、怎么复核」；命令式是唯一能防「文件被误改后无人发现」的形态 |
| D18 | 生成物 `bindings.ts` 的随动（事实校准 7） | `src/types/generated/bindings.ts` 的 `TestFramework` doc 注释随 `lib.rs:37` 措辞变化，跑 `pnpm -C packages/desktop run bindings:export` 重生成并 **`git add` 入索引**；`bindings:check` 比对的是工作区 vs 索引，未入索引即判漂移 | 生成物不是手写改动，但不入索引会让 AC-9 的 `run check`（含 `bindings:check`）失败；把它显式登记进变更清单，避免「改了注释却忘了重生成」这类静默失败面 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 配置 schema 镜像 | 编辑器 `$schema` 补全的唯一来源；校验实现的机器可读镜像（顶层 7 键 / `framework` 8 值 / `additionalProperties: {}`）；迁入后**手工维护** | `packages/desktop/src-tauri/crates/core/config/config.schema.json`（新增，已在案 173 行） | 无（静态 JSON，不参与 tauri bundle） | JSON Schema draft 2020-12（逐字副本） |
| 一致性断言 | 镜像 ↔ 实现的双向对拍：键集等于 `KNOWN_KEYS`、枚举等于 `TestFramework` 八值、枚举串 ↔ 变体往返；漂移时指认方向 | `…/crates/core/config/src/lib_test.rs`（修改，用例归 test-gen 相位） | `serde_json`（既有）、`super::{KNOWN_KEYS, parse_framework, TestFramework}` | `include_str!` + 子模块可见性（零新增导出） |
| 校验实现注释面 | 校验语义权威自述：去掉「CLI zod / `defaults.ts` 对齐」参照物措辞，改直述（语义零改） | `…/crates/core/config/src/lib.rs`（修改，12 处 doc） | 无 | Rust doc 注释 |
| 仓库面配置指针 | `$schema` 指向 crate 内镜像的 raw URL；`write_protection` 用户规则改指 `packages/desktop/knip.json` | `openspec/config.json`（修改，**已落盘**：`:2` / `:23` 两值；其余字节零改） | 目标 `packages/desktop/knip.json`（现存 346B）、镜像（现存） | JSON 值级替换（CRLF 保持） |
| 指针清单（值对照 + 复核） | 逐值给出「位置 / 旧值 / 新值」+ 最小改动改法 + 复核命令 | 本设计「配置指针（已落盘）」表（口径正本）+ `openspec/changes/retire-dev-team-plugin/tasks.md` 阶段二 2B（可执行版）+ `README.md` 项目配置段（逐字同值） | D6 / D7 / D17 | Markdown 表 + `node -e` 值级替换 |
| 资产删除面 | 插件源码 + 三类交付产物 + 两个 marketplace 清单的整块删除；`git rm` 保证索引同步，`rm -rf` 兜底未 tracked 残留 | `plugins/**`、`dist/**`、`.claude-plugin/`、`.cursor-plugin/`（删除，已落盘） | 无 | git 索引操作 |
| 语料删除面 | 50 份插件期 capability 整目录删除（目录随内容消失，不留空壳） | `openspec/specs/<50 目录>`（删除，已落盘 → 35 保留项） | 无 | git 索引操作 |
| 语料不变量 | 「只含存活能力 / 无空壳 / 退役整块下线 / 仓库根文档与市场只描述存活资产 / 活跃面无悬空路径（含 `openspec/config.json` 的指针零命中且可达）」的 requirement + scenario 承载 | `openspec/changes/retire-dev-team-plugin/specs/repo-surface-scope/spec.md`（在案）→ 归档合入 `openspec/specs/repo-surface-scope/` | 无 | OpenSpec delta |
| 语料口径清面 | `core/checks` 报告语义的权威表述改为「已固化 Rust 实现 + 入仓 golden 金样」；`explore` stance 注释改单源自持 | `…/crates/core/checks/tests/{corpus_golden_test.rs,fixtures/README.md,golden/README.md}`、`packages/desktop/src/lib/explore-stance.ts`（修改） | 无 | 注释 / 文档文本 |
| 仓库根文档 | 桌面仓的安装 / 开发 / 构建 / 发版四段最小集 + 仓库结构 + 配置入口；市场通道随产物消失而无登记项 | `README.md`、`AGENT.md`、`CLAUDE.md`（重写，已落盘） | `packages/desktop/RELEASE.md`（权威口径来源）、`.github/workflows/desktop-release.yml` | Markdown |
| 生成物随动 | `TestFramework` doc 变化的生成投影必须入索引，否则 `bindings:check` 判漂移 | `packages/desktop/src/types/generated/bindings.ts`（生成物，已入索引） | `bindings:export` 脚本 | 生成代码（1 行注释） |
| 版本交付 | 单包 patch +1 纪律的本次实例 | `packages/desktop/package.json`（修改 → `0.5.31`） | — | JSON |

---

## 变更清单

> 路径口径：新增文件用仓库根相对全路径；修改文件与删除清单中 `crates/...` / `src/...` 省略 `packages/desktop/src-tauri/` 前缀者已在行内标注。本清单覆盖 proposal「变更范围 - 实现文件」全部文件，含 `openspec/config.json` 的两处指针（v3：已落盘，见表首行）。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/core/config/config.schema.json` | 由 `plugins/dev-team/bin/dev-team-config.schema.json`（173 行）**逐字**迁入的 JSON Schema 镜像：draft 2020-12，顶层 `properties` 7 键，`tests.items.properties.framework.enum` 8 值，`additionalProperties: {}`；迁入后手工维护、不参与 tauri bundle、只由同 crate `*_test.rs` 读取；不含 `config.json` 子串（`desktop-crate-layout` 隔离禁令不破）；索引中因内容全同被 git 记为 `R`（D3 / 事实校准 7） | 配置表「镜像文件」行；「数据模型 - 镜像结构与一致性不变量」；D3 / D4 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `.gitignore` | 删 `# 构建相关` 标题与其下两行（`plugins/*/.pack-staging/`、`*/dev-team/bin/*.cjs.map`）；`.*/` 与四条 `!` 否定项（含 `!.claude-plugin` / `!.cursor-plugin`）、`memory`、`node_modules`、突变测试与构建条目之外的一切**零改** | 死条目清理（`repo-surface-scope` Module Contract）；两条否定项按 D8 保留 |
| `openspec/config.json` | `:2` `$schema` 值整体替换为 `…/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json`；`:23` `write_protection.files[0].glob` 替换为 `packages/desktop/knip.json`。改动面**恰 2 行**，`reason` 及该对象其余字段、行尾（CRLF）与全文件其余字节零改 | 仓库面指针换源的唯一落点（D6 / D7 / D16）；AC-4 / AC-11；与 `README.md:94` / `:102` 逐字同值 |
| `README.md` | **整篇重写**为桌面仓说明：安装（Releases 双平台 + mac 右键放行）/ 开发 / 常用门禁 / 构建（签名 env 两变量）/ 发版（bump + push master + 不 bump 不发版 + 绝不 push tags）/ 仓库结构 / 项目配置（新 `$schema` URL + 三字段小表）/ 历史一句（不含 `plugins/dev-team` 字面量） | 现状 152 行 100% 插件安装手册、零桌面内容；AC-6 判据；D9。项目配置段（`:94` / `:102`）与 `openspec/config.json` 的现值**逐字同值**（D16） |
| `AGENT.md` | **整篇重写**：去掉 `DEPRECATED` 前言与「Slim Plugin」主叙事，落 Project Overview / Project Rules / Architecture / Coding Guidelines 四段（正文与 `CLAUDE.md` 同） | AC-6；D10 |
| `CLAUDE.md` | 同上（标题与面向对象句与 `AGENT.md` 差异，正文一致） | AC-6；D10 |
| `packages/desktop/src/lib/explore-stance.ts` | 文件头注释块改为**单源自持**：删「与插件 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链——改动需双侧同步」全部表述，改述「desktop 包自持，stance 两形与笔记约定为本文件单源」；两形文本与注入行为**零改** | `desktop-explore-page` MODIFIED 场景「stance 模板单源自持」；AC-3 |
| `crates/core/config/src/lib.rs`（`packages/desktop/src-tauri/`） | 12 处 doc 注释参照物措辞直述化：`:8`（`literal('spec-driven')` 对齐 → 字面量直述）、`:11`（`prefault('.')` 对齐 → cwd 默认值直述）、`:14/17/20/23`（`CLI defaults.ts 对齐` → 默认值直述）、`:37`（`CLI zod enum 同序同串 / 显式 rename 对齐 CLI 串` → 枚举与 wire 串直述）、`:72/93/189`（三处 `Default` doc → `默认值口径`）、`:344`（`zod number().min(0).max(100).optional().prefault(N) 移植` → 组装语义直述）、`:378`（`CLI zod 枚举串 → 框架枚举` → `wire 串 → 框架枚举`）。**代码、常量值、类型、签名零改** | `desktop-workspace-config` MODIFIED 场景「校验权威单源」；AC-3 / AC-7 |
| `crates/core/config/src/lib_test.rs`（`packages/desktop/src-tauri/`） | 三条一致性断言落位（契约见「数据模型 - 一致性不变量」）：① 键集：`schema.properties` 键集 == `super::KNOWN_KEYS`；② 枚举：`tests.items.properties.framework.enum` == 八变体同序 `wire_name`；③ 往返：每条 enum 串经 `super::parse_framework` 解回变体且 `wire_name` 同串。新增测试内私有 `fn wire_name(framework: &TestFramework) -> &'static str`（穷尽 match，非穷尽即编译失败）。`use` 行补 `KNOWN_KEYS` / `parse_framework` / `TestFramework`。**零新增生产导出、零新增 dev-dependency** | `desktop-workspace-config` ADDED requirement；AC-5。用例编写归 test-gen 相位（本相位只定契约） |
| `crates/core/checks/tests/corpus_golden_test.rs`（`packages/desktop/src-tauri/`） | 三处措辞：`:20-22` 头注释「golden 期望值基准：CLI zod 权威（`plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts`）产出形态录制」→「基准为已固化 `core/checks` 实现 + 入仓 golden 金样（语料采自真实工作区产出形态）」；`:291` doc「与 CLI 权威一致」→「与 `core/checks` 实现口径一致」；`:311` 断言文案「与 CLI 权威一致」→「与既有金样口径一致」。**断言值、语料、行为零改** | `desktop-test-execution` MODIFIED 场景「权威源单点」；AC-3；D5 |
| `crates/core/checks/tests/fixtures/README.md`（`packages/desktop/src-tauri/`） | `:3-4` 引用块去「golden 期望以 CLI zod 权威形态录制」溯源措辞，改「golden 期望基准为已固化 `core/checks` 实现 + 入仓金样；重写开关与两步工作流见 `tests/golden/README.md`」；语料清单表与样本**零改** | 同上；AC-3；D5 |
| `crates/core/checks/tests/golden/README.md`（`packages/desktop/src-tauri/`） | `:4`「TS zod schema 漂移由此表现为可读 diff」→「实现漂移由此表现为可读 diff」；`:26`「golden 期望值基准为 CLI zod 权威产出形态」→「golden 期望值基准为 `core/checks` 实现落定的产出形态」。重写开关与文件集约定**零改** | 盘查补正 #1（事实校准 2）；D5 |
| `openspec/specs/desktop-phase-prompts/spec.md`（主 spec，Purpose 直改） | `## Purpose`：`14 个全静态 md（6 executor + 8 evaluator）` → `16 个全静态 md（7 executor + 9 evaluator，含 code-analyze 两份）`；「迁移是对插件 `plugins/dev-team/agents/*.md` 的改写而非搬运」→ 直述「按桌面端特性（七工具 / cwd=workspace root / change 名走上下文头 / 静态脚本外置）自持」；删插件路径字面量。requirement 段**零改**（由 delta 承载） | AC-7；事实校准 4（Purpose 不由 delta 承载）；R12 |
| `openspec/specs/desktop-change-create/spec.md`（主 spec，Purpose 直改） | `## Purpose`：删「插件面零改动」分句，保留「MUST NOT 产出 workflow.json（双向墙），禁止 CLI 子进程」。requirement 段**零改** | 同上 |
| `packages/desktop/package.json` | `version` `"0.5.30"` → `"0.5.31"`（单行） | 版本交付纪律；AC-10；D11 |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | `TestFramework` doc 注释 1 行随 `lib.rs:37` 变化（`bindings:export` 重生成）；必须 `git add` 入索引 | AC-9（`bindings:check`）；D18；事实校准 7 |

> **13 份主 spec 的正文改动由 delta 承载**：`openspec/changes/retire-dev-team-plugin/specs/<13 个 desktop capability>/spec.md` 已在案（提案相位产物），本相位零改动；主 spec 正文在归档链的 spec 同步段按 `ADDED/MODIFIED/REMOVED/RENAMED` 合并（`desktop-phase-prompts` 走 RENAMED 14→16，其余走 MODIFIED；`repo-surface-scope` 为纯 ADDED 新能力目录）。

### 删除清单

| 路径 | tracked | 形态 | 说明 |
|------|---------|------|------|
| `plugins/**` | 237 | `git rm -r` + `rm -rf plugins` | 插件全部源码：`skills/` 13、`agents/` 17、`hooks/`、`bin/`（含 CLI / MCP / `dev-team-config.schema.json` / `knip.json` 与 `__tests__`）、`build/`、`templates/` 5、`_fragments/`、`package.json` 与 lockfile；未 tracked 残留（`node_modules` / `.pack-staging` / `reports`）随物理删清空 |
| `dist/claude-plugins/dev-team/**`、`dist/cursor-plugins/dev-team/**`、`dist/cursor-home-image/dev-team/**` | 124 | 同上 | 三类交付产物（含生成的 `install.mjs`）；删除后 `dist/` 为空目录，整目录不保留 |
| `.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` | 2 | `git rm` + 空目录删 | 两文件整删（各自只登记 `dev-team` 一项，`source` 指向已删产物）；`.claude-plugin/`、`.cursor-plugin/` 空目录不保留 |
| `openspec/specs/<50 份插件期 capability>/**` | 50 目录 | 单条批量 `git rm -r`（D1） | 清单：`architecture-agent` / `architecture-commit-gate` / `architecture-decisions` / `architecture-model` / `architecture-validation` / `c4-cli-integration` / `c4-dsl-syntax` / `change-create` / `change-files` / `change-list` / `cli-unit-test-execute` / `config-get` / `config-schema` / `cursor-home-image` / `dual-platform-plugin-build` / `embedded-cli` / `eval-check-cli` / `eval-json-protection` / `file-type-isolation` / `glob-matching` / `hook-output-import` / `include-fragments` / `intent-router` / `json-design-schemas` / `marketplace-config` / `mcp-project-root` / `mcp-tool-namespace` / `module-boundary-contracts` / `multi-plugin-layout` / `mutation-testing` / `pge-workflow-engine` / `phase-agents` / `phase-lifecycle` / `phase-skills` / `pipeline-backtrack` / `protect-files-hook` / `spec-list` / `static-check-hook` / `test-detect-frameworks` / `test-exclude-filter` / `test-execution-diagnostics` / `test-get-framework-config` / `test-only-workflow` / `test-path-resolver` / `unit-test-executor` / `update-architecture-skill` / `user-prompt-hook` / `workflow-file-inventory` / `workflow-orchestration` / `write-protection-config`（**保留** 34 个 `desktop-*` 与 `workspace-layout-resolution`） |

### 配置指针（已落盘：值对照 + 复核命令）

`openspec/config.json` 位于写保护**内置锁定面**（`rules::builtin_rules` → `**/openspec/config.json`，恒在首位、fail-closed、无 override；事实校准 6）。两处指针按**值级替换**落盘（`protect/**` 零代码改动，未给内置规则加任何 override）：

| # | 位置 | 旧值（改写前，可对任一副本 grep 复核） | 新值（**现行实况，已落盘**） | 改法 |
|---|---|---|---|---|
| 1 | `:2` `$schema` | `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/plugins/dev-team/bin/dev-team-config.schema.json` | `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json` | 值级替换（整串） |
| 2 | `:23` `write_protection.files[0].glob` | `plugins/dev-team/bin/knip.json` | `packages/desktop/knip.json` | 值级替换（`reason` 与该对象其余字段不动） |

- **改动面恰 2 行**，其余字段与行尾字节零改；该文件为 **CRLF** 行尾，纯 LF 的 `git apply` 补丁会被判不匹配 → 已用值级替换（可执行版脚本见 `tasks.md` 阶段二 2B）。
- **复核命令（AC-11 第二段，现行通过）**：`git grep -c "plugins/dev-team" -- openspec/config.json` **零输出**；`git diff --numstat -- openspec/config.json` = `2  2`；`node -e 'JSON.parse(…)` 通过；两值与上表「新值」列逐字相等。
- **基线命令（AC-11 第一段，对改写前的副本仍有效）**：同一 `grep` 命中数 = 2 且逐值等于上表「旧值」列。

<!-- 公共函数 / API：本变更零新增、零修改模块级导出函数；无 CLI 子命令、无 HTTP 端点、无 IPC 命令、无 `bindings.ts` 手写 API 变动（仅有 1 行 doc 注释的生成投影）。一致性断言的载体 `wire_name` 为 `lib_test.rs` 内私有函数，按口径不入表。 -->

<!-- 类型定义：本变更零新增 / 零修改 interface、type alias、enum 与公共 API class；`TestFramework` / `WorkspaceConfig` / `KNOWN_KEYS` 的**声明**零改（仅其 doc 措辞、`lib_test.rs` 侧对拍与生成投影变化）。 -->

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `$schema` | `openspec/config.json` | 修改（**已落盘**） | string（URL） | 无（现状必填） | 旧值 `…/refs/heads/master/plugins/dev-team/bin/dev-team-config.schema.json` → 新值 `…/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json`（D6 / D16；判据 AC-4 / AC-11） |
| `write_protection.files[0].glob` | `openspec/config.json` | 修改（**已落盘**） | string（root 相对 glob） | 无 | 旧值 `plugins/dev-team/bin/knip.json` → 新值 `packages/desktop/knip.json`；保护侧自动前缀 `**/`（D7 / D16） |
| `.gitignore` 条目 | `.gitignore` | 删除 | 文本行（3 行） | — | `# 构建相关`、`plugins/*/.pack-staging/`、`*/dev-team/bin/*.cjs.map`；两条 `!` 否定项保留（D8） |
| `version` | `packages/desktop/package.json` | 修改 | string（semver） | — | `0.5.30` → `0.5.31` |
| `version` | `packages/desktop/src-tauri/tauri.conf.json` | 零改（核对） | string | — | 恒为 `"../package.json"` 引用，自动跟随 |
| `[package] version` | `packages/desktop/src-tauri/Cargo.toml` | 零改（核对） | string | — | crate 版本线独立，MUST NOT 随包版本 bump |
| 镜像文件本体 | `packages/desktop/src-tauri/crates/core/config/config.schema.json` | 新增（逐字副本） | JSON Schema | — | 迁入后手工维护；双断言守护（D4）；`$schema` 目标 URL 的尾路径与它逐字同形（D6） |

---

## 数据模型

本变更**零持久化改动**：无 db 迁移、无 `workflow.json` 字段变动、无 store / DTO 变动、无新 IPC 载荷（`bindings.ts` 仅 1 行 doc 注释的生成投影）。三个「数据物」是配置 schema 镜像、其一致性不变量、以及**配置指针的值对照与复核基线**。

### 镜像结构（`crates/core/config/config.schema.json`，逐字迁入）

| 层 | 内容 | 用途 |
|----|------|------|
| 根 | `$schema` = `https://json-schema.org/draft/2020-12/schema`、`type: object`、`additionalProperties: {}` | draft 声明与「顶层未知字段 passthrough 保留」的机器可读表达（对应 `WorkspaceConfig::extra`） |
| `properties`（7 键） | `$schema` / `schema` / `context` / `rules` / `static_analysis` / `tests` / `write_protection` | 与 `KNOWN_KEYS[7]`（`lib.rs:27-35`）一一对应（断言 1 的锚点） |
| `properties.schema` | `const: "spec-driven"`、`default: "spec-driven"`、`@deprecated` 描述 | 与 `SCHEMA_LITERAL` 对应 |
| `properties.tests.items.properties.framework` | `enum` 8 值（`jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest`），`required: ["root","framework"]` | 与 `TestFramework` 八变体 + `parse_framework` 对应（断言 2/3 的锚点；序同声明序） |
| `properties.tests.items.properties.{coverage,mutation}` | 阈值 `number`、`minimum: 0`、`maximum: 100`、默认 80/70/75、mutation `score` 默认 70 | 与 `COVERAGE_*_DEFAULT` / `MUTATION_SCORE_DEFAULT` / 0–100 值域对应（**不由断言守护**，见下） |
| `properties.write_protection.files.items` | `glob`（`minLength: 1`）、`reason`（支持 `%s` / `%t`） | 与 `WriteProtection` / `WriteProtectionFile` 对应（**不由断言守护**） |

### 一致性不变量（`lib_test.rs` 三条断言 + 一条编译期守卫）

| 断言 | 源 A（镜像） | 源 B（实现） | 失败文案须指认 |
|------|--------------|--------------|----------------|
| A1 键集 | `schema["properties"]` 的键集（排序后） | `super::KNOWN_KEYS`（排序去重后） | 「config.schema.json 与 `KNOWN_KEYS` 漂移」+ 双向差异清单（`仅 schema 有: […]` / `仅实现有: […]`） |
| A2 枚举 | `schema["properties"]["tests"]["items"]["properties"]["framework"]["enum"]`（8 串，同序） | 八变体按 `TestFramework` 声明序取 `wire_name` | 「镜像枚举与 `TestFramework` 八值漂移」+ 双向差异清单 |
| A3 往返 | A2 取到的每个串 | `super::parse_framework(&s)` → 变体 → `wire_name` 同串 | 「镜像枚举串与 `parse_framework` 映射漂移」+ 首个不往返的串与解回结果 |
| 编译期守卫 | 测试内 `fn wire_name(framework: &TestFramework) -> &'static str` 的**穷尽 match** | 新增第 9 变体 | 非穷尽匹配 → `cargo clippy --all-targets` 直接编译失败（`lib_test` 在 `--all-targets` 面内） |

**不守护面（知情边界，登记以免疫「断言覆盖面」质疑）**：数值阈值默认值 / 0–100 值域、`tests[].root` 非空与无通配符、`cwd` / `config` / `includes` / `excludes` / `write_protection` 的字段级形状，均**不在**断言面 —— 规格只钉「`KNOWN_KEYS` 键集」与「`TestFramework` 八值」两条镜像锚点，超出的部分会让断言变成实现复写。

### 指针基线（`openspec/config.json` 的唯一「数据」形态）

| 项 | 值（现行实况） | 判据 |
|---|---|---|
| 命中文件 | `openspec/config.json` 零命中 | 活跃面内其他文件出现 `plugins/dev-team` 即属清面遗漏（D12） |
| 命中行数 | **0**（改写前为 2） | `git grep -c "plugins/dev-team" -- openspec/config.json` |
| 改动面 | 恰 2 行值：`:2` `$schema`、`:23` `write_protection.files[0].glob` | `git diff --numstat -- openspec/config.json` = `2  2`；`JSON.parse` 通过；CRLF 保持 |
| 其余字节 | 零改（`reason` 与该对象其余字段未动） | `git diff -- openspec/config.json` 仅两行 `-` / `+` |
| 保护面 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 零 diff | 未给内置规则加 override（`repo-surface-scope` 明文禁止） |
| 改写前副本 | 命中 2 行且逐值等于上表旧值 | AC-11 第一段（对任一副本可复跑） |

### 删除侧的「数据」语义

- 50 份 capability 目录是**语料**不是数据：删除后无残留占位、无 `TBD`、无空目录（`repo-surface-scope` scenario「无空壳」即判据）。
- 两个 marketplace 清单是**对外登记**不是数据：整文件删除，零 `plugins: []` 空壳。
- `dist/` 三份产物是**生成物**不是数据：删后无再生成源（`plugins/*/build` 同批消失）。

---

## 验收标准对齐（AC-1..AC-11）

| AC | 设计锚点 | 判据（机械可复跑） | 责任段 |
|----|----------|--------------------|--------|
| AC-1 源码与产物删除 | 删除清单前三行；D1 / D2 | `ls plugins dist .claude-plugin .cursor-plugin` 四空（空目录不留）；同提交 `--stat` 同时含四类删除 + 语料删除 | 实现 |
| AC-2 语料清点 | D1 / D13；删除清单第 4 行 | `ls openspec/specs \| wc -l` = 35（归档后 36）；`find … -type d -empty` 与 `-name spec.md -size 0` 均空 | 实现（归档前复核）+ 归档（+1） |
| AC-3 无悬空引用 | D12（v3：活跃面含 `openspec/config.json`） | ① 活跃面 `git grep` 零输出（源码 / `.gitignore` / 根文档 / `.github/**` / `openspec/config.json`，仅保留样本豁免排除项）；② 主 spec 恰 16 行 8 位点；③ 配置指针归 AC-11 复核 | 实现（①②③ 均已达成）+ 归档（全量零命中） |
| AC-4 schema 迁移与指针换源 | D3 / D6 / D7 / D16；配置表镜像行与两处指针行 | 镜像在案（173 行）且与迁出前逐字一致；`openspec/config.json` 的 `$schema` 尾路径 `packages/desktop/src-tauri/crates/core/config/config.schema.json` 与仓库现存路径同形；`write_protection.files[0].glob` = `packages/desktop/knip.json`，该文件在案且 `git check-ignore` 无命中（exit=1） | 实现（**已全部达成**） |
| AC-5 schema 防漂移 | D4；「数据模型 - 一致性不变量」 | 三条断言（A1 / A2 / A3）在位并通过；零新增导出 | test-gen（契约由本设计定）+ test-execution |
| AC-6 文档重写 | D9 / D10 | 三文件 `grep "plugins/dev-team"` 零命中；含安装 / dev / build / 发版四段入口；无 `claude plugin` / `install.mjs` 类命令 | 实现 |
| AC-7 spec delta 一致 | D13 / D15；14 份 delta | 14 份在案；MODIFIED header 与主 spec 逐字一致；RENAMED 形态合规；每 requirement ≥1 scenario、scenario 标记恰 4 个 `#`；prompts 计数 14→16 与磁盘一致；`desktop-change-orchestration` 边界四条 | 实现（delta 已在案，核对） |
| AC-8 语料不变量在案 | `repo-surface-scope` 三条 requirement（含「退役整块下线」「无空壳」「活跃面零悬空 + 配置指针零命中且可达」） | 文件在案；其 scenario 可考（`ls openspec/changes/retire-dev-team-plugin/specs \| wc -l` = 14） | 实现 |
| AC-9 desktop 全绿 | 依赖节；D18（生成物入索引） | `pnpm -C packages/desktop run check`（含 `bindings:check`）/ `run test` / `cargo test --workspace` 全绿；覆盖率与 mutation 达阈值；`cargo test -p checks-runtime --lib` 全绿 | test-execution（静态脚本由桌面端跑） |
| AC-10 版本交付 | D11 | `package.json` = `0.5.31`；`tauri.conf.json` 仍 `"../package.json"`；`Cargo.toml` 零改 | 实现 |
| AC-11 配置指针（双段，v3 两段均已达成） | D6 / D7 / D16 / D17；「配置指针（已落盘）」表；「数据模型 - 指针基线」 | 第一段（旧值基线，可对任一副本复跑）：命中数 = 清单行数且逐值对应。第二段（现行实况）：同一命令**零输出**、`git diff --numstat -- openspec/config.json` = `2 2`、两值等于清单「新值」、`protect/**` 零 diff | 实现（两段均已达成，归档 / 发版前复跑） |

---

## 依赖

**本变更零新增运行时依赖、零新增构建 / 测试依赖。**

- 镜像读取用 `include_str!`（std 内建）+ `serde_json`（`crates/core/config` 既有普通依赖，已被实现侧使用）；无 `dev-dependencies` 变动、无 workspace `Cargo.toml` 变动。
- `core/config` 的 `foundation` / `serde` / `serde_json` / `specta` 依赖集零改（一致性断言不引入新 crate，也不用 specta 反射）。
- 删除面带来的依赖净减少：`plugins/dev-team` 的 `node_modules` 不在任何 pnpm workspace 内（`plugins/dev-team/pnpm-workspace.yaml` 为其自身根），删除后无 jest / vitest 残留配置、无 lockfile 引用、无 `pnpm` 命令指向它。
- `packages/desktop` 的依赖与脚本零改（`check` / `test` / `build` / `bindings:export` / `bindings:check` 全链不变）。
- 配置指针零依赖：值级替换只需文本编辑器或 `node`（脚本见 `tasks.md`），不引入新工具。

---

## 待决问题

1. **AC-3 字面命令的最终口径（已收口）**：本设计以 D12 追加两个样本豁免排除项（`crates/core/checks/tests/golden`、`protect/{rules,reason}_test.rs`）——proposal 原字面命令必然命中 4 个被「不要修改」保护的样本。`openspec/config.json` 自 v3 起**不再需要排除**：两值已落盘、命中归零。实现侧不会为过 AC 改样本或改保护语义（已复跑通过）。
2. **`tests/golden/README.md` 的补正归属**：本设计将其纳入清面（盘查补正 #1），故实际改动面比 proposal「实现文件」清单多 1 个 markdown 文件。若验收以「变更范围 - 实现文件」为闭合集（而不是下界），需确认该补正可否随本变更落。
3. **`.gitignore` 两条 `!` 否定项**：`.claude-plugin` / `.cursor-plugin` 目录删除后，`!.claude-plugin` / `!.cursor-plugin` 成无对象条目。本变更按 proposal「其余零改」保留（D8），是否在后续变更清理未定。
4. **`core/orchestration` 的两处「插件」措辞**：`verdict.rs:4`（「与插件 `phaseLogSchema.report` max 2000 同源」）与 `walker_test.rs`（「与插件 buildEntry 同形 / 直跑形态对照一致」）无路径字面量、不在 AC-3 检索面、proposal 未列 → 本变更不动（D14）。是否直述化留待后续变更，未定。
5. **配置指针的长期可维护性（原「交接项未应用」风险已消解）**：v3 起 `$schema` 与 `write_protection` 两值均指向仓库内现存目标，编辑器补全与用户写保护规则即刻生效（不再有悬空窗口）；proposal R14 的「用户长期不应用」风险随之关闭。剩余动作只是复核：归档前 / 发版前各复跑一次 `tasks.md` 阶段七自查 C（`grep` 零输出 + `git diff` 恰 2 行）。
6. **50 份 capability 退役后的可考性**：归档后主基线只留 `repo-surface-scope` 的不变量与本 change 全文（proposal 清单 + 本设计删除清单）承载「因何下线」。已由 proposal 风险 R3 知情接受，无剩余实现动作。
