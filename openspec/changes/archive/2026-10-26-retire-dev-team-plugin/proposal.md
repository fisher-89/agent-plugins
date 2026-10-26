# 提案: retire-dev-team-plugin

> **变更**: retire-dev-team-plugin
> **日期**: 2026-10-26
> **状态**: draft
> **修订**: v3 — 落地校准：`openspec/config.json` 的两处指针**已由实现相位值级替换落盘**（改动恰 2 行、CRLF 保持、`JSON.parse` 通过、旧路径命中归零，`protect/**` 零代码改动），v2 的「用户手工交接」通道随之作废、指针回到实现面；AC-3 的「活跃面」重新包含该文件，AC-11 双段判据两段均已达成（见「提案 · 落地校准（v3）」）
> **修订**: v2 — 回溯重定：`openspec/config.json` 曾按「用户手工交接项」交付（内置锁定面，agent 无写入路径）；AC-3 / AC-4 判据分「agent 可写面 / 内置锁定面」两段；新增 AC-11 交接基线
> **workflow_type**: requirement

---

## 问题

仓库当前是「插件仓 + 桌面包」的叠层状态，但插件这一层已无存活理由：桌面端（`packages/desktop`）已把插件的知识与编排全部接走，插件本体冻结在 `2.10.45`（最后一次实质功能提交 2026-09-28 的 `2.10.44`），只剩维护面与腐坏面。

1. **知识已单源化，插件是重复的第二份**：17 个 agent 中 16 个已在 `core/workflow/src/prompts/`（16 份全静态 md，含 code-analyze 两份）落位；相位路由 / 落账 / 回溯 / change 管理 / 测试与静态检查门禁均由 `core/workflow` + `infra/checks` 原生承载；hooks `record-files` / `static-check` / `sweep-phase` 分别由 vcs `change_edit_set` / spawn 步 / no-op 取代；hooks `protect-files` 的写保护缺口已由桌面原生护栏补齐（`crates/infra/agent/src/sdk/protect/`，落在 loop 守卫链第四段，强于插件 hook——后者只在插件会话内生效，而 SDK 是唯一活引擎）。仅 `archi_*` 五工具（`.c4` / ADR）与 `bin/dev-team-config.schema.json` 无桌面替代。

2. **仓库面仍在向插件指路，指向的是即将消失的资产**：`openspec/config.json` 的 `$schema` 指向 `plugins/dev-team/bin/dev-team-config.schema.json`（删即 404）；`write_protection.files[0].glob` 指向 `plugins/dev-team/bin/knip.json`（删即悬空，写保护用户规则集体失效）；`README.md`（152 行）100% 是插件安装手册、零桌面内容；`AGENT.md` / `CLAUDE.md` 把「插件 slim 架构」当主叙事；`.claude-plugin/marketplace.json` 与 `.cursor-plugin/marketplace.json` 两个文件各自只登记 `dev-team` 一项（删插件即空市场，且 `source` 相对路径指向 `./dist/claude-plugins/dev-team`，产物一删即 404）。

3. **语料已在描述不存在的代码（腐坏本体）**：`openspec/specs/` 85 个 capability 中 50 个是插件期产物，其中 `glob-matching/spec.md` 是 0 字节空壳、`architecture-commit-gate` 与 `c4-cli-integration` 的代码半年前就不存在；desktop 侧另有 11 份 spec 含 22 处指向插件的措辞（10 处描述对象已不存在、12 处历史版本交付条款挂着「SHALL NOT 变更 `plugins/dev-team`」半句），另有 4 份 spec 的 8 处「插件」只是措辞参照（判定语义自身完整，可留可直述）。腐坏不是一次意外，而是「无范围不变量」的结构性结果——插件期 capability 从未在资产退役时同步下线。

4. **删除不破坏任何构建与测试**：跨包 grep 后 desktop 运行时对插件零依赖，命中项只有注释、语料 fixture（`"root": "plugins/dev-team/bin"`）、glob 测试样本与 spec 措辞。插件删除对本仓库的唯一真实影响是「引用面要清」。

5. **可达性约束（v3 实测校正）**：第 2 条里的两处悬空指针都落在 `openspec/config.json` 内，而该文件位于写保护**内置锁定面**——`crates/infra/agent/src/sdk/protect/rules.rs::builtin_rules` 由 `layout::config_path` 派生 `**/openspec/config.json`，恒在规则首位、fail-closed、不可配置不可移除（`protect/**` 内无 override / 跳过开关）。**v3 实测**：该守卫是产品运行时的执行面，本轮实现相位以值级替换成功落盘两处指针（改动恰 2 行、CRLF 保持、`JSON.parse` 通过），`protect/**` 零代码改动 → 指针换源属**可执行面**（见「提案 · 落地校准（v3）」与「变更范围 · 配置指针（已落盘）」）。

使用者影响已裁决：已装副本继续跑、`plugin update` 静默无更新、不发弃用通知（无弃用版本、无迁移提示）；Cursor home 镜像用户 `~/.cursor` 的 hooks / mcpServers 会留残（`install.mjs` 无卸载模式，留痕不处理）。

---

## 提案

**D 档全删**：删源码 + 删三类交付产物 + 删对外市场 + 删插件期语料 + 重写仓库文档，仓库自此是纯桌面仓。八条既有裁决照办，另补五条盘查补正（见下）。硬边界与补正一并写死，防止「顺手扩大」与「顺手漏清」两个方向。

### 落地校准（v3：指针换源已由实现相位落盘）

v2 曾把 `openspec/config.json` 的两处指针判为「结构性不可达」（内置 fail-closed 写保护 + 5 次尝试失败），改为用户手工交接。**v3 实测该判断不成立**：实现相位以值级替换（`node -e` 文本替换，非整文件重写）成功写入两处指针值——`git diff --numstat -- openspec/config.json` = `2 2`、CRLF 行尾保持、`JSON.parse` 通过、`git grep -c "plugins/dev-team"` 归零，且 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 零代码改动（未给内置规则加任何 override）。故：

1. **执行通道回归实现面**：两处指针（`$schema` → crate 内镜像 raw URL；`write_protection.files[0].glob` → `packages/desktop/knip.json`）与其余改动**同一变更面**收口，不再有待用户执行的动作；
2. **检索面归位**：AC-3 的「活跃面」重新包含 `openspec/config.json`（该文件命中为 0），AC-4 判「尾路径与仓库内现存路径同形 / glob 目标在案且未被忽略」；
3. **AC-11 双段均已达成**：第一段（旧值副本命中数 = 清单行数并逐值对应）保留为可复跑基线，第二段（零命中 + `git diff` 恰 2 行 + 两值等于新值 + `protect/**` 零 diff）为现行实况。

八条既有裁决与五条盘查补正**口径零改**（`$schema` 换新 URL、`write_protection` 改指 `packages/desktop/knip.json`、schema 迁 crate 内）；变的只是**交付通道与验收面**。下游已同步：design 的变更清单含该文件两行、tasks 阶段二 2B 记为已落盘、阶段七自查 B1 / C 为可复跑复核。

### 回溯重定（v2：曾把交付面移出实现面；结论已被 v3 取代）

实现相位已实测 5 次尝试全部卡在同一处：`openspec/config.json` 的内置 fail-closed 写保护返回「它是本工作区的配置权威，Agent 不得改写……所有写入路径均被拦截」，`write` / `edit` / `bash` 三条路同一拦截面、无 override 开关——**重试无收益**。缺陷不在工序而在需求面：本提案原把该文件列为实现文件，同时又以「活跃面零命中」为验收，二者只能靠改该文件调和，故**结构性不可达**。重定三条：

1. **执行通道改判**：`openspec/config.json` 从「实现文件」移出为「**用户手工交接项**」，逐值给出旧值 → 新值（见「变更范围 · 用户手工交接」）；实现相位 MUST NOT 尝试写入该文件。
2. **检索面二分**：AC-3 的「零命中」判据只覆盖 **agent 可写面**，内置锁定面（`openspec/config.json`）不入该判据；锁定面改由 AC-11 的**交接基线**验收——实现相位判「命中数恰为交接清单行数且逐值对应」，用户应用后判「零命中」。
3. **可执行面判据补齐**：AC-4 只留可执行面判据（镜像在案且逐字一致、raw URL 尾路径与仓库内现存路径同形、glob 目标文件在案且未被忽略），两处指针的实际回填值与核对归 AC-11。

八条既有裁决与五条盘查补正**口径全部不变**（`$schema` 仍换新 URL、`write_protection` 仍改指 `packages/desktop/knip.json`、schema 仍迁 crate 内）——变的只是**执行通道与验收面**。下游随之重排：design 的指针改写条目改挂「交接」而非「实现」，AC-3 分层判据与 AC-4 拆分并入本稿口径；tasks 阶段二的两条指针改动改标「交接项（agent 不可执行）」，阶段七「自查 B1 / 自查 C」改为交接基线核对 + 目标可达核对。

### 既有裁决（照办）

| # | 项 | 结论 |
|---|---|---|
| 1 | 下线深度 | D 档全删：`plugins/dev-team/**` + `dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team` + 语料 + 文档 |
| 2 | `archi_*` 能力 | 直接删除，不移植、不留残骸；`.c4` / ADR 整体退役 |
| 3 | 用户通知 | 不做（无弃用版本、无迁移提示） |
| 4 | marketplace | `.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` **两文件整删**，不留空壳 |
| 5 | 版本交付条款 | 只摘「SHALL NOT 变更 `plugins/dev-team`」半句与对应 THEN 断言，保留 desktop 版本 bump 段 |
| 6 | config schema | 迁入 `packages/desktop/src-tauri/crates/core/config/config.schema.json`，`$schema` URL 随之更换 |
| 7 | 写保护 | `write_protection.files[0].glob` 改指 `packages/desktop/knip.json` |
| 8 | orchestration 边界 #5 | 整条删除（已偿还的边界不留痕，与紧邻两条同款惯例） |

### 盘查补正（explore 未覆盖，本提案补齐）

1. **口径更正——删 50 份而非 51 份，`desktop-*` 是 34 个而非 33**：实测 `openspec/specs/` 85 个 capability = 34 个 `desktop-*` + 51 个无前缀，其中仅 `workspace-layout-resolution` 是 desktop foundation crate 的活 spec（讲 Layout / resolve，命名没带前缀；按前缀一刀切会误删）→ **删 50 份**，删除后 35 项（34 + `workspace-layout-resolution`）。`mutation-score-gap-fill` 在 git 中不存在（explore 记为尸体，实为笔误）；`glob-matching`（0 字节）确实在案。

2. **退役 capability 不写 per-capability `## REMOVED Requirements` delta**：归档链的 spec 同步段只合并 delta 提到的 capability，且 Finalize 步只为 delta 标记的 capability 建提交 pathspec——对 50 份 capability（300 条 requirement）逐条写 REMOVED，会把主基线刷成 50 份空壳 `spec.md`，既撞本变更自定的「无空壳」判据，又让删除落不进归档提交。**改为整目录删除（分支内 `git rm`，与 `dist` + marketplace 的「同提交硬约束」同批）**，范围不变量由新能力 `repo-surface-scope` 的 requirement 承载。

3. **新增能力 `repo-surface-scope`（防复发）**：把「`openspec/specs/` 只含存活能力、无空壳、退役能力整块消失」「仓库根文档与对外通道只描述存活资产」「活跃面无指向已删资产的路径引用」三条钉成 requirement + scenario。语料腐坏这次能发生 50 份，是因为从来没有一条 requirement 说「资产退役时 capability 必须同步下线」；只删不立，下次照样累积。

4. **explore 盘查遗漏的悬空引用（同属「描述对象已不存在」）**：
   - `desktop-workflow-write-face:62`「插件 `pipeline-backtrack` 约束…插件零改动、两份 spec 各管其面」——对面那份 spec 即在本变更被删，`两份 spec 各管其面` 直接失效；
   - `desktop-change-create` **Purpose**「插件面零改动」与 `desktop-phase-prompts` **Purpose**「迁移是对插件 `agents/*.md` 的改写」；
   - `desktop-test-execution:11`「对齐插件工作流的 test-execution 独立相位语义」；
   - `desktop-workspace-config:18`「默认值 SHALL 对齐 CLI `defaults.ts`」（与 :15 的 `config.schema.ts` 同一参照物，同段同改）；
   - `desktop-change-orchestration:380`「desktop-phase-prompts：14 个纯静态 prompt」（顺带与 14→16 漂移一起修）；
   - `desktop-crate-layout` 的 `plugins/<name>` 措辞与 `.gitignore` 两行插件期规则（`plugins/*/.pack-staging/`、`*/dev-team/bin/*.cjs.map`）——规则随资产消失而成为死条目。

5. **版本号更正**：`packages/desktop/package.json` 现为 `0.5.30`（explore 写 `0.5.25 → 0.5.26` 已过期），本变更交付 `0.5.30 → 0.5.31`；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动。

### config schema 迁移的口径（迁出手工维护的代价必须记账）

- **落点**：`crates/core/config/config.schema.json`，与校验实现（`KNOWN_KEYS[7]` + `TestFramework` 八值枚举）物理相邻，「改字段顺手改 schema」最可能发生；不参与 tauri bundle（编辑器消费走 GitHub raw URL），体积零变化。
- **失去生成源**：schema 原由 `configSchema.toJSONSchema()` 从 zod 生成（`build/build-config-schema.ts`），迁出后变为**手工维护的静态 JSON**。防漂移靠同 crate 一对断言：`KNOWN_KEYS` 七项 == `schema.properties` 键集、`TestFramework` 八值 == `tests.items.properties.framework.enum`（现状逐字对齐）。
- **隔离禁令已核**：`desktop-crate-layout` 禁产品 `.rs` 出现 `config.json` 字面量；`config.schema.json` 不含该子串，且只由 `_test.rs` 读取。
- **副作用**：schema 落在 `packages/desktop/**` 会触发 `desktop-release.yml` 的 detect；不 bump 版本时 `should=false` 自动跳过，无实际影响。

### 落提交与顺序

`dist` 三份产物删除 + marketplace 两文件删除 + `plugins/dev-team/**` 删除 + 50 份语料删除 **必须同批**（marketplace `source` 指向产物，语料描述产物；任何中间态都是「市场指 404 / spec 描述不存在的代码」）。desktop spec 的 delta 与引用清理可与同批，归档时由 spec 同步段合入主基线。`openspec/config.json` 的两行指针改动**同在该提交面内**（v3：值级替换已落盘），故 `--stat` 含该文件而 diff 恰 2 行（AC-11）。

---

## 能力

### 新增能力

- **repo-surface-scope** — 仓库面范围不变量（本变更唯一新增）：`openspec/specs/` 只含存活能力、capability 不得为空壳或指向已删资产、退役能力整块下线；仓库根文档（`README.md` / `AGENT.md` / `CLAUDE.md`）与对外通道（marketplace）只描述存活产品；活跃面无指向已删资产的路径引用（活跃面判零命中，含 `openspec/config.json`；该文件的改写限值级替换且保护面零改动，复核走双段基线）。

### 修改的能力

- **desktop-workspace-config** — 校验权威换源 + schema 归属：「配置解析 core crate」MODIFIED（`config.schema.ts` 复刻口径 → 以 `core/config` 现行实现为权威、已退役参照物不再作为对齐源；默认值对齐 `defaults.ts` 同段改直述）；新增 ADDED「`config.schema.json` 静态镜像与实现一致性断言」（镜像落位 crate 内、`KNOWN_KEYS` / 八值枚举双断言、`$schema` URL 指向新位置——该 URL 已就地回填）。
- **desktop-test-execution** — 「test-execution 确定性门禁步与绿跑零 agent」MODIFIED（删「对齐插件工作流…语义」参照，反面护栏保留）；「双实现对拍黄金语料」MODIFIED（权威源由 CLI zod schema 改为「已固化 Rust 实现 + `tests/golden` corpus 金样」）。
- **desktop-explore-page** — 「会话链与 stance 前导」MODIFIED（stance 模板注释由「与 SKILL.md 双源互链」改单源自持）；Module Contract 两行（`explore-stance.ts` 行改单源、`plugins/dev-team/skills/openspec-explore/SKILL.md` 行整删）。
- **desktop-phase-prompts** — 14→16 漂移与措辞翻案：RENAMED「14 个静态角色知识文件清单」→「16 个静态角色知识文件清单」（补 code-analyze 两份并登记）；MODIFIED 四条（清单本身 14→16 + 「全静态零占位」/「include_str! 显式字面路径装配」/「桌面端特性适配」的 14→16 计数与防回退措辞）；Purpose 行直接改（delta 机制不承载 Purpose）。
- **desktop-change-flow-view** — 「流程图节点模型与两分类」MODIFIED（删「写入方（插件 CLI）MUST NOT 因本变更改动」分句）；Module Contract 「磁盘形状与写入方（不改）」行整删。
- **desktop-workflow-write-face** — 「backtrack 白名单二次校验与 stale 语义」MODIFIED（删「插件零改动、两份 spec 各管其面」对偶从句）；Module Contract 「插件 MCP / TS 写路径（彻底分离，不改）」行整删。R 档两处（`phase_log` 缺陷对照、等价性验收口径）**保留不动**——缺陷描述与验收口径自身完整，插件作为反面参照删掉后更该留。
- **desktop-change-create** — 「V1 范围与边界留痕」MODIFIED（边界 6「插件侧零改动」整条删、边界 3 去 `createChange` parity 改直述，场景「六条边界」→「五条」）；「版本交付」MODIFIED（摘插件半句与 THEN 断言）；Module Contract 版本交付行同步摘除；Purpose「插件面零改动」直接改。
- **desktop-change-archive** — 「版本交付」MODIFIED（摘插件半句；scenario 名「版本号升级与插件零改动」→「版本号升级」）。
- **desktop-change-orchestration** — 「V1 范围与边界留痕」MODIFIED（边界 #5 整条删、边界 #1 的 14→16 措辞随修，场景「五条边界」→「四条」并去掉护栏留痕子句）；「版本交付」MODIFIED（摘插件半句 + scenario 改名）；Module Contract 「既有 hooks（复用，不改）｜protect-files 原样生效（过渡期）」行整删。
- **desktop-change-worktree** — 「版本交付」MODIFIED（摘插件半句与 THEN 断言）。
- **desktop-release-pipeline** — 「版本交付」MODIFIED（摘插件半句与 THEN 断言）；新增 ADDED「版本交付纪律」：每次桌面源码变更把 `packages/desktop` patch 版本 +1（`tauri.conf.json` 跟随、`Cargo.toml` 不随动），本变更实例 `0.5.30 → 0.5.31`。
- **desktop-crate-layout** — 「packages/desktop 完全独立包落位」MODIFIED（「CLAUDE.md 插件 bump 规则」措辞改指「仓库根已无插件包」，scenario 名「不受插件 bump 规则约束」→「不受产物 bump 规则约束」）。
- **desktop-write-protection** — R 档三条措辞直述化（去掉依赖「插件端对照读者」的表述，语义不变）：「保护规则装配（内置锁定 + 用户规则合并）」/「shell 提取的档 B 补充族」/「node / python 内联脚本写入判别」MODIFIED；内置锁定 bullet 直述「该规则使该文件成为保护面第一道拦截，改指针走值级替换并复核、MUST NOT 加 override」；Module Contract 一行标注该行「写保护内置锁定面内的配置」。

### 不写 delta 的能力（50 份插件期 capability）

整目录删除，不落 `## REMOVED Requirements`（理由见提案「盘查补正 2」）。清单：`architecture-agent` / `architecture-commit-gate` / `architecture-decisions` / `architecture-model` / `architecture-validation` / `c4-cli-integration` / `c4-dsl-syntax` / `change-create` / `change-files` / `change-list` / `cli-unit-test-execute` / `config-get` / `config-schema` / `cursor-home-image` / `dual-platform-plugin-build` / `embedded-cli` / `eval-check-cli` / `eval-json-protection` / `file-type-isolation` / `glob-matching` / `hook-output-import` / `include-fragments` / `intent-router` / `json-design-schemas` / `marketplace-config` / `mcp-project-root` / `mcp-tool-namespace` / `module-boundary-contracts` / `multi-plugin-layout` / `mutation-testing` / `pge-workflow-engine` / `phase-agents` / `phase-lifecycle` / `phase-skills` / `pipeline-backtrack` / `protect-files-hook` / `spec-list` / `static-check-hook` / `test-detect-frameworks` / `test-exclude-filter` / `test-execution-diagnostics` / `test-get-framework-config` / `test-only-workflow` / `test-path-resolver` / `unit-test-executor` / `update-architecture-skill` / `user-prompt-hook` / `workflow-file-inventory` / `workflow-orchestration` / `write-protection-config`。

---

## 变更范围

### 实现文件

#### 删除文件（源码与产物，361 tracked）

- `plugins/dev-team/**` — 237 tracked（skills 13 / agents 17 / hooks / bin / build / templates 5），连同未 tracked 的 `.pack-staging`、`node_modules`、`reports` 一并物理删
- `dist/claude-plugins/dev-team/**`、`dist/cursor-plugins/dev-team/**`、`dist/cursor-home-image/dev-team/**` — 124 tracked（含生成的 `install.mjs`）
- `.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` — 两文件整删（`dist/`、`.claude-plugin/`、`.cursor-plugin/` 空目录不保留）

#### 删除语料（50 份 capability）

- `openspec/specs/<50 个插件期 capability>/**` — 整目录删（见「能力」节清单；含空壳 `glob-matching`）
- 保留：34 个 `desktop-*` + `workspace-layout-resolution`

#### config schema 迁移（新增）

- `packages/desktop/src-tauri/crates/core/config/config.schema.json` — 由 `plugins/dev-team/bin/dev-team-config.schema.json`（173 行）迁入，内容逐字保持；此后手工维护
- `packages/desktop/src-tauri/crates/core/config/src/lib_test.rs` — 追加一对一致性断言（`KNOWN_KEYS` 键集 / `TestFramework` 八值枚举），与实现同 crate

#### 配置与文档

- `README.md` — **重写**为桌面仓说明（现状 100% 插件安装手册、零桌面内容；发版走 GitHub Releases，口径取 `packages/desktop/RELEASE.md`）
- `AGENT.md`、`CLAUDE.md` — 重写为桌面仓说明（现状把「插件 slim 架构」当主叙事）
- `.gitignore` — 删两行插件期规则（`plugins/*/.pack-staging/`、`*/dev-team/bin/*.cjs.map`）；其余零改

#### 配置指针（已落盘：值对照 + 复核）

`openspec/config.json` 是写保护内置锁定面的对象（见「问题」5）。两处指针按**值级替换**落地（整串替换，非整文件重写）：

| # | 位置 | 旧值（改写前） | 新值（**现行实况，已落盘**） |
|---|------|------|------|
| 1 | `$schema` | `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/plugins/dev-team/bin/dev-team-config.schema.json` | `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json` |
| 2 | `write_protection.files[0].glob` | `plugins/dev-team/bin/knip.json` | `packages/desktop/knip.json` |

- 改法 SHALL 为**值级替换**（该文件是 CRLF 行尾，纯 LF 的 `git apply` 补丁会判不匹配），改动面恰 2 行，其余字段与行尾**字节零改**；`reason` 及该对象其余字段不动 —— 已按此形态落盘（`git diff --numstat` = `2 2`）；
- 新 glob 生效后即用户规则（`**/packages/desktop/knip.json`，`**/` 零段可命中仓库根文件）；内置 `**/openspec/config.json` fail-closed 恒在兜底；
- **复核命令**（AC-11 第二段，现行通过）：`git grep -c "plugins/dev-team" -- openspec/config.json` 零输出、`git diff` 恰 2 行、两值等于上表「新值」；旧值副本的命中数 = 2 且逐值对应（第一段基线）。MUST NOT 以「把该文件移出核对面」代替记录。

#### 源码与语料引用清理（desktop 侧）

- `packages/desktop/src/lib/explore-stance.ts` — 注释头「双源互链」→ 单源自持
- `packages/desktop/src-tauri/crates/core/config/src/lib.rs` — 常量注释的「CLI zod / `defaults.ts` 对齐」措辞改直述（语义零改）
- `packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs`、`.../tests/fixtures/README.md` — 「CLI zod 权威」→「已固化 Rust 实现 + corpus 金样」
- 13 份 desktop spec 的 delta（见「能力」节）+ 两处 Purpose 行直接改（`desktop-phase-prompts` / `desktop-change-create`）
- 主 spec Purpose 直接改的理由：归档链的 spec 同步段按 delta 的 `ADDED/MODIFIED/REMOVED/RENAMED` 语义合并，不承载 `## Purpose`；这两行是纯叙述句改写，直接在 change 分支改主 spec 并在归档 Finalize 的 pathspec 内落提交，幂等无冲突

### 测试文件

- `packages/desktop/src-tauri/crates/core/config/src/lib_test.rs` — 新增两条一致性断言（不新增导出；`include_str!` 直接读 crate 内 schema）
- 既有用例口径更新（断言文案含已退役参照物者）：`core/checks/tests/corpus_golden_test.rs` 头注释与断言语（行为零改）
- 删除：随 `plugins/dev-team/**` 整目录消失（其 `node_modules` 不在任何 workspace 内，无 jest/vitest 残留配置）
- `pnpm -C packages/desktop run check` / `run test` / `cargo test --workspace` 全绿（覆盖率达 `openspec/config.json` 阈值：lines 90 / branches 85 / functions 90；mutation 80 / 70）

### 不要修改

- `openspec/changes/archive/**`（132 个历史 change）与 `openspec/explores/**` — 历史事实，含插件路径引用属考古内容
- `openspec/config.json` — 写保护内置锁定面（`**/openspec/config.json` fail-closed，`protect/**` 无 override）；**仅两处指针值例外**（`$schema` 与 `write_protection.files[0].glob`，见「配置指针（已落盘）」），其余字段与字节零改；MUST NOT 为改指针动内置锁定语义
- `packages/desktop/src-tauri/**/fixtures/**` 里的 `"root": "plugins/dev-team/bin"`、`protect/rules_test.rs` 里的 `plugins/dev-team/bin/knip.json` 样本 — 纯语料数据 / glob 测试样本，非依赖
- `packages/desktop/src-tauri/**` 的**产品逻辑** — 本变更在 desktop 侧只做注释、文档级清理 + schema 文件迁入 + 两份 spec 语料同步；`crates/core/agent/**`、walker、store、命令面零行为改动
- `desktop-phase-prompts` 的 R 档护栏（「MUST NOT 携带插件 Agent / MCP 工具假设」「无 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` 标记」）与 `desktop-write-protection` 的判定语义 — 插件删了，防回退条款更该留
- 其余门禁开关（`stryker.config.json` / `tsconfig.json` / `vite.config.ts`）不进 `write_protection` — 保护面不铺过大，等真出现被改动再补
- `.claude/settings.local.json` — 未 tracked 的本机残留，不在提交面（可选自清，不属本变更交付）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 源码与产物删除 | `plugins/`、`dist/`、`.claude-plugin/`、`.cursor-plugin/` 四个目录全部消失（空目录不留）；`plugins/dev-team` 的未 tracked 残留（`.pack-staging` / `node_modules` / `reports`）同步清空；四类删除在同一提交（`git log --stat` 该提交同时含 `plugins/`、`dist/`、marketplace 与语料删除） |
| AC-2 | 语料清点 | `ls openspec/specs \| wc -l` = 35（34 个 `desktop-*` + `workspace-layout-resolution`）；归档合入 delta 后 = 36（+ `repo-surface-scope`）；`find openspec/specs -type d -empty` 与 `find openspec/specs -name spec.md -size 0` 均为空 |
| AC-3 | 无悬空路径引用（活跃面） | `git grep -n "plugins/dev-team" -- ':!openspec/changes/archive' ':!openspec/explores' ':!openspec/changes/retire-dev-team-plugin' ':!openspec/specs' ':!packages/desktop/src-tauri/crates/core/checks/tests/fixtures' ':!packages/desktop/src-tauri/crates/core/checks/tests/golden' ':!packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules_test.rs' ':!packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason_test.rs'` 输出为空（覆盖 `README.md` / `AGENT.md` / `CLAUDE.md` / `.gitignore` / `.github/**` / `openspec/config.json` / desktop 产品源码与注释）。排除项各有授权：`archive` / `explores` 是历史事实；本 change 目录是过程产物；`openspec/specs` 由归档相位判（实现相位命中恰为 delta 覆盖位点）；`fixtures` 与 `golden` 是语料数据样本、两份 `protect/*_test.rs` 是 glob 样本（均属「不要修改」） |
| AC-4 | schema 迁移与指针换源 | `packages/desktop/src-tauri/crates/core/config/config.schema.json` 在案且与迁出前逐字一致；`openspec/config.json` 的 `$schema` raw URL 尾路径（`/packages/desktop/src-tauri/crates/core/config/config.schema.json`）与仓库内现存路径同形；`write_protection.files[0].glob` = `packages/desktop/knip.json`，该文件在案且 `git check-ignore` 无命中 |
| AC-5 | schema 防漂移 | 同 crate 一对断言在位并通过：`KNOWN_KEYS` 七项 == `schema.properties` 键集；`TestFramework` 八值枚举 == `tests.items.properties.framework.enum` |
| AC-6 | 文档重写 | `README.md` / `AGENT.md` / `CLAUDE.md` 只描述桌面仓（安装 / dev / build / 发版走 GitHub Releases），无插件安装手册段落、无「插件 slim 架构」主叙事 |
| AC-7 | spec delta 一致 | 14 份 delta 落盘；MODIFIED header 与主 spec 逐字一致；REMOVED/RENAMED 形态合规；每条 requirement 至少一个 scenario 且 scenario 标记恰 4 个 `#`；`desktop-phase-prompts` 计数 14→16（7 executor + 9 evaluator，含 code-analyze 两份）与磁盘一致；`desktop-orchestration` 边界为四条 |
| AC-8 | 语料不变量在案 | 新能力 `repo-surface-scope` 三条 requirement + scenario 落盘，含「退役 capability 整块下线」「无空壳」「活跃面无悬空路径引用（含 `openspec/config.json` 指针零命中且可达）」可考 |
| AC-9 | desktop 全绿 | `pnpm -C packages/desktop run check`（含 `bindings:check`）与 `pnpm -C packages/desktop run test`、`cargo test --workspace` 全绿；覆盖率与 mutation 达 `openspec/config.json` 阈值；`cargo test -p checks-runtime --lib` 全绿（不回归既有 flake 面） |
| AC-10 | 版本交付 | `packages/desktop/package.json` `0.5.30` → `0.5.31`；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 版本不随动 |
| AC-11 | 配置指针（双段，均已达成） | **第二段（现行实况）**：`git grep -c "plugins/dev-team" -- openspec/config.json` 零输出，`git diff --numstat` = `2 2`，`$schema` 值以 `…/packages/desktop/src-tauri/crates/core/config/config.schema.json` 结尾、`write_protection.files[0].glob` = `packages/desktop/knip.json`，`protect/**` 零 diff。**第一段（对改写前副本可复跑）**：同一 `grep` = 2 且两行逐值等于「变更范围 · 配置指针（已落盘）」表「旧值」列 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 产物删除与市场删除不同批 | 中间态 marketplace `source` 指向 404，任何人 `plugin marketplace add` 即失败 | 中 | AC-1 同提交断言；`dist` + marketplace + `plugins/` + 语料同一提交（变更范围「落提交与顺序」） |
| R2 50 份 capability 整删落不进归档提交 | 归档 Finalize 只为 delta 标记的 capability 建 pathspec，删除若发生在归档阶段会留脏工作区 | 中 | 整删在实现阶段随同批提交完成（先于归档）；`repo-surface-scope` 的「无空壳 / 只含存活能力」scenario 作为事后判据；归档前用 AC-2 复核 |
| R3 语料删除撞「无 delta 即无痕迹」质疑 | 日后无法从 spec 历史看出某 capability 因何下线 | 低 | 本 proposal 保留 50 份清单与理由；`openspec/changes/archive` 留存本变更全文；`repo-surface-scope` 钉住不变量（比逐条 REMOVED 更防复发） |
| R4 schema 失去生成源后静默漂移 | 编辑器补全与实际校验语义分叉（drift 无自动拦截面） | 中（知情接受） | 同 crate 双断言（AC-5）；schema 与 `KNOWN_KEYS` / 枚举物理相邻；`desktop-workspace-config` ADDED requirement 明文要求两断言恒在 |
| R5 `$schema` 换 URL 后编辑器解析不到 | 本地开发失去 config 补全提示 | 低 | AC-4 机械核对「URL 尾路径 == 仓库内现存路径」；合入 master 前取不到属预期（同任何新增文件）；README 项目配置段与 `openspec/config.json` 现值**逐字同值**，避免两处真相 |
| R6 写保护换指向失效 | `write_protection` 用户规则 fail-open，glob 悬空即护栏静默全失效 | 低（v3 已落盘，无悬空窗口） | 目标文件在案且未被忽略（AC-4）+ AC-11 第二段复跑核对；内置 `**/openspec/config.json` 恒在 fail-closed 兜底 |
| R7 版本交付条款半句摘除漏改 | 13 份 spec 中仍有「SHALL NOT 变更 `plugins/dev-team`」残留 → 语料自相矛盾 | 中 | AC-3 全量 grep 兜底（该短语含插件路径字面量）；AC-7 delta 一致性核 |
| R8 14→16 漂移只改计数漏登记 | 两份 code-analyze prompt 仍属「可识别废弃文件」叙述，与 `include_str!` 实际装配矛盾 | 中 | AC-7 显式要求计数与磁盘一致；`desktop-phase-prompts` delta 补两行文件表 |
| R9 Cursor home 镜像用户残留 | `~/.cursor` 的 hooks / mcpServers 留残，无卸载入口（`install.mjs` 只有 `--root`） | 高（知情接受） | 既定裁决不通知；本提案留痕，后续若要清理需独立工具（不在本变更） |
| R10 顺手扩大保护面 | 把 `stryker.config.json` / `tsconfig.json` / `vite.config.ts` 一并塞进 `write_protection` | 低 | 变更范围「不要修改」显式禁；等真出现被改动再补 |
| R11 历史版本交付条款继续累积 | 5 份「本变更 SHALL 将 version 由 0.4.x 升级为 0.4.x+1」历史快照皆已过期，本变更按裁决只摘半句、另加第 6 份纪律条款 | 中 | 本提案在「决策与待决」显式记账：归并为单份持久纪律留待后续变更（不在本变更扩大） |
| R12 归档链 spec 同步段误判 Purpose 直改 | 同步 agent 只按 delta 合并，Purpose 直改不被回滚，但若同步重写整份文件则可能覆盖 | 低 | Purpose 直改与 delta 覆盖的 requirement 不重叠；同步段约束「保留 delta 未提及的主 spec 内容」；归档后 AC-2 + 抽查 Purpose 行 |
| R13 指针改写被误判为不可达而空转（v2 期曾发生 5 次） | 实现相位空转、相位预算耗尽，真正的差距被掩盖 | 低（v3 已消解） | v3 实测值级替换可落盘（`git diff` 恰 2 行）；`repo-surface-scope` 把「改动限值级替换 + 保护面零改动」钉成 requirement，复核命令在 tasks 阶段七自查 B1 / C |
| R14 指针改写遗漏或误改 | `$schema` 悬空（编辑器补全丢失）、`write_protection` 用户规则失效 | 低 | 值级替换 + 三条机械复核（`grep` 零输出 / `git diff` 恰 2 行 / 两值等于清单新值）在案；归档与发版前各复跑一次 |

---

## 决策与待决

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 下线深度 | D 档全删（源码 + 产物 + 语料 + 文档） | 桌面已单源，留任一层都是维护面 | A 只删源码（留 404 市场，否）/ C 删到契约面（无「契约面」可留，插件整份都是实现，否） |
| `archi_*` 五工具 | 直接删除，不移植 | 无桌面消费者；`.c4` / ADR 整体退役是既定方向 | 移植进桌面 MCP（无需求方，否） |
| 用户通知 | 不发 | 无弃用版本、无迁移提示；`plugin update` 静默无更新 | 发弃用公告（需维护发布通道，否） |
| marketplace | 两文件整删 | 市场只登记 `dev-team` 一项，留空壳无意义 | 留空 `plugins: []`（空市场即坏市场，否） |
| config schema | 迁入 `crates/core/config/`，手工维护 + 双断言 | 与校验实现同目录最可能同步；不参与 bundle 体积零变化 | 删 `$schema` 字段（编辑器补全静默消失，explore 早期方案的劣化，否）/ 保留在插件目录（删插件即 404，否） |
| `write_protection` | 改指 `packages/desktop/knip.json` | 保护动机随资产位置平移：knip 是 `client:check` 唯一配置源，而 `client:check` 是 `static_analysis` 入口 | 清空 `write_protection`（用户规则整块失效，否） |
| orchestration 边界 #5 | 整条删除 | 已偿还的边界不留痕（紧邻两条同款先例）；保留「已补齐」会稀释真边界 | 改述为「已补齐」（历史说明混进限制清单，否） |
| 版本交付条款 | 只摘插件半句 | 保留 desktop bump 段作为变更交付口径 | 整条删（丢交付口径，否） |
| 50 份 capability 的退役表达 | 整目录删除，不写 per-capability REMOVED delta；范围不变量立新能力 | 逐条 REMOVED 会产 50 份空壳且删除落不进归档 pathspec；不变量比逐条留痕更防复发 | 50 份 REMOVED delta（空壳 + 300 条 requirement 逐条抄写，否）/ 只删不立（无防复发，否） |
| 语料范围归属 | 新能力 `repo-surface-scope` | 语料腐坏是仓库面问题，不属于任何 desktop 产品能力 | 塞进 `desktop-crate-layout`（crate 布局不含语料 / 文档，否）/ 不立（否） |
| `openspec/config.json` 的处置（v3） | **就地值级替换**两处指针（恰 2 行）+ 双段复核基线 | v3 实测可落盘（`git diff --numstat` = `2 2`、`JSON.parse` 通过、旧路径命中归零），无需引入人工环节；把改写面收在 2 行值、`protect/**` 零代码改动 | 把它排除出 AC-3 检索面（判据变空，否）/ 给内置规则加 override 或改保护语义（为一次改写动产品语义，否）/ 整文件重写（diff 面失控，否） |
| 历史版本交付条款 | 只摘半句，不归并 | 不扩大范围；归并需要决策「spec 是否记录历史版本序列」 | 归并成单份持久纪律（范围外，留待后续，否） |

### 待决（归 design）

- 50 份语料删除的**执行形态**：单条 `git rm -r` 批量 vs 按 capability 分条；同批提交的拆分粒度（一个提交 vs 两个——若拆，「产物 + 市场」先、语料后会不会留中间态）
- `config.schema.json` 一致断言的读取方式（`include_str!` vs `fs::read` + 相对 crate 路径）、断言失败文案；`KNOWN_KEYS` 键集是否需在实现侧导出（**不得为测试新增 export**）
- README / AGENT.md / CLAUDE.md 的重写内容口径（安装 / dev / build / 发版四段的最小集；是否保留 historical 插件说明一句）
- `desktop-write-protection` R 档三条的直述化措辞（去「插件端同款 / 插件端此处放行」后如何不丢判定强度）
- `.gitignore` 两行删除是否影响任何现存构建（`*/dev-team/bin/*.cjs.map` 的 glob 形态已无对象）
- 指针值对照表的落位与复核时机（已由 design D16 收口：design 为口径正本、tasks 阶段二 2B 为可执行版、归档与发版前各复跑一次复核）
- 指针记录的层次（已由 design D17 收口：命令式 + 一张表 + 实现报告逐条）
