# 下线 plugins/dev-team：牵动面盘点与档位选择

workflow_type: requirement

---

## 0. 一句话现状

`AGENT.md` 已经把这件事写成了既成事实的预告——"plugin source 不再积极开发、桌面端接管知识编排、将来整体删除"。真正的问题不是"要不要删"，而是：

> **删到什么深度算"下线"？删之前桌面端还欠哪几笔？**

本笔记只做盘点与分档，不动代码、不建 change。

---

## 1. 插件今天到底交付了什么

`plugins/dev-team` 已冻结在 2.10.45（最后一次实质功能提交 09-28 的 2.10.44；10-17 只做了 explore stance 双源同步 + 版本 bump）。它仍然是一台完整的分发机器：

```
                        plugins/dev-team（237 文件 / 69,101 行 ts+md）
   ┌────────────┬────────────┬────────────┬─────────────┬──────────────┐
   │ skills/ 13 │ agents/ 17 │ hooks/ 1   │ bin/src     │ templates/   │
   │ 斜杠命令    │ 相位角色    │ 4 类回调    │ CLI 2 + MCP 17 │ c4/adr/产物模板│
   └─────┬──────┴─────┬──────┴─────┬──────┴──────┬──────┴──────┬───────┘
         │            │            │             │             │
         └────────────┴────────────┴─────────────┴─────────────┘
                                   │  build/ + vite pack + assemble
                                   ▼
              dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team
                          （124 tracked 文件，3.8MB × 3 份）
                                   │
                    ┌──────────────┴───────────────┐
                    ▼                              ▼
        .claude-plugin/marketplace.json    .cursor-plugin/marketplace.json
        （wps-ai 市场，唯一插件）            （同，pluginRoot=dist/cursor-plugins）
                    │                              │
                    ▼                              ▼
           claude plugin install          /add-plugin 或 ~/.cursor 镜像
                    │                              │
                    └────────► README.md（152 行，全部是插件安装手册）
```

交付面共 5 类（skills / agents / hooks / CLI+MCP / templates），对外通道 3 条（Claude 市场、Cursor 市场、Cursor home 镜像）。

---

## 2. 谁在用：外部通道 vs 本仓库自己

| 使用者 | 依赖形态 | 删了会怎样 |
|--------|----------|-----------|
| 装了插件的团队成员（Claude Code） | 市场缓存副本 | 已装副本继续跑；`plugin update` 拿不到新版 → 静默冻在同版本，无"该插件已下线"提示 |
| Cursor 市场用户 | 同 | 同上 |
| Cursor home 镜像用户 | `~/.cursor` 被 install.mjs 改写（hooks + mcpServers + state 文件） | **没有卸载模式**（`install.mjs` 只认 `--root`），源码删了就没有清理入口 |
| 本仓库自身 | `openspec/config.json` 两处硬引用（`$schema` URL + `write_protection` 的 knip.json 路径）；`.claude/settings.local.json` 7 条 MCP 工具 allowlist | 编辑器 schema 404；写保护 glob 悬空；allowlist 是死条目 |
| desktop 运行时 | **零**（跨包 grep 只命中注释、语料 fixture、spec 措辞） | 无影响 |

"已装副本继续工作"是唯一的软着陆，但它同时意味着：**不主动发一版"遗言"版本，用户永远不知道自己用的是死代码。**

---

## 3. 知识资产盘点：桌面端接住了什么，什么还没家

### 3.1 17 个 agent → 桌面 16 个 prompt 已就位

`packages/desktop/src-tauri/crates/core/workflow/src/prompts/` 现有 16 份 md：

| 插件 agent | 桌面 prompt |
|---|---|
| proposal / dev-design / test-design / implement / test-gen / test-execution 的 planner+generator+evaluator（12） | ✅ 全在 |
| code-review-evaluator、acceptance-evaluator | ✅ 在 |
| code-analyze-planner / code-analyze-evaluator | ✅ 在（16 份中的多出两份） |
| **architecture.md** | ❌ **无对应** |

顺带发现一处 spec 漂移：`desktop-phase-prompts` 写"恰好 14 个文件"，磁盘上 16 个（code-analyze 两份未登记）。

### 3.2 17 个 MCP 工具 + 2 个 CLI 命令

| 能力 | 插件工具 | 桌面替代 |
|---|---|---|
| 相位路由 / 落账 / 回溯 | `phase_next` `phase_start` `phase_log` `backtrack` | ✅ `core/workflow` 原生 |
| change 管理 | `change_create` `change_list` `change_files` `workflow_files` `spec_list` `config_get` | ✅ 原生（含 store） |
| 测试/静态检查 | `test_detect_frameworks` `test_resolve_paths`、CLI `test-execution` `run_static_analysis` | ✅ `infra/checks` 原生 |
| **架构模型 / ADR** | `archi_query` `archi_validate` `archi_write` `archi_check` `archi_decide` | ❌ **无**（`crates/core/archi` 只存在于 `openspec/explores/desktop-architecture-guard.md` 的方案稿，一行代码都没有） |

### 3.3 hooks 4 类

```
protect-files   → ❌ 缺口（orchestration spec 边界 #5 明文：桌面原生护栏"插件真下线前落地"）
record-files    → ✅ 职能被 vcs change_edit_set 取代
static-check    → ✅ 桌面由 spawn 步承接
sweep-phase     → ✅ 桌面 no-op（fail-open）
```

### 3.4 零散知识

| 资产 | 状态 |
|---|---|
| `templates/adr.md` + `templates/model.c4` | 只服务 archi 能力 → 随 3.2 一起无家 |
| `templates/artifacts/{proposal,design,test-design}.md.template` | 插件 skill 专用；桌面由 prompt 承载格式知识（删除 = 接受格式知识只剩 prompt 一份） |
| `bin/dev-team-config.schema.json`（173 行） | 编辑期 `$schema` 补全的唯一来源；桌面只复刻校验语义，不产 schema → 删后 README 教的写法变 404 |
| `openspec-explore/SKILL.md` 的笔记格式约定 | 与 `desktop/src/lib/explore-stance.ts` 双源互链（10-17 刚同步过）→ 删除即双源变单源，`desktop-explore-page` spec 的双源条款要一起退 |

---

## 4. 牵动面清单

| # | 路径 | 数量 | 处置 | 坑 |
|---|---|---|---|---|
| 1 | `plugins/dev-team/**` | 237 tracked / 69k 行 | 整目录删 | 含 `.pack-staging/`（已 ignore）、`node_modules`、`reports/`、`stryker` 配置 |
| 2 | `dist/{claude,cursor}-plugins/dev-team`、`dist/cursor-home-image/dev-team` | 124 tracked / ~11MB | 整目录删 | 与 marketplace 必须**同一提交**摘除，否则市场指向 404 路径 |
| 3 | `.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` | 2 文件 | 删条目，或整个删文件 | 空插件列表的市场是否合法未验证；建议保留壳 + 空 `plugins: []` 并加 deprecated 说明 |
| 4 | `README.md` | 152 行 | **重写** | 现在 100% 是插件手册，零桌面内容；桌面发版走 GitHub Releases（`packages/desktop/RELEASE.md`） |
| 5 | `AGENT.md` / `CLAUDE.md` | 2 文件 | 重写为桌面仓库说明 | 现状把"插件架构"当主叙事 |
| 6 | `openspec/config.json` | 2 处 | 删 `$schema`、删 `write_protection` 的 `plugins/dev-team/bin/knip.json` | 写保护只剩内置的 config.json + eval.json |
| 7 | `openspec/specs/**` | 51 份插件期 spec（约 1/3 语料） | 整目录删 | 含 2 份**早已是僵尸**的 spec（`architecture-commit-gate`、`c4-cli-integration`，其代码半年内已不存在） |
| 8 | `openspec/specs/desktop-*` 的 9 份、17 处插件提及 | 17 处 | 少量 delta MODIFIED | 多为"版本交付：plugins/dev-team 保持 2.10.44 零改动"这类**归档后即失去意义**的条款；少数是行为面（如 `desktop-workflow-write-face` 的"workflow.json 写入方（插件 CLI）"） |
| 9 | `.claude/settings.local.json` | 7 条 | 清 allowlist | 本机残留 |
| 10 | `packages/desktop` 注释/fixture | 3 处注释 + 语料数据 | 顺手改注释即可 | `explore-stance.ts` 双源注释、`corpus_golden_test.rs`「CLI zod 权威」注释；fixture 里的 `"root": "plugins/dev-team/bin"` 只是数据，不用动 |
| 11 | `openspec/explores/*.md` | 4 份 | **不动** | 历史笔记，考古价值 |

对 `openspec/specs` 而言，删除规模是这次变更里最"痛"的一块：**17030 行 spec 语料里约三分之一描述的是即将不存在的代码**。

---

## 5. 档位阶梯：四种"下线"

```
 A 冻结       B 撤市       C 删产物+撤市      D 全删（源码/产物/语料/文档）
 ├─ 只写        ├─ 摘市场     ├─ + 删 dist     ├─ + 删 51 份插件 spec
 │  弃用公告     │  条目       │  与 marketplace│   + 重写 README/AGENT/CLAUDE
 │             │             │                 │   + 清 config.json 引用
 └ 成本≈0      └ 成本≈0      └ 成本≈1 提交     └ 成本≈1 个大 change（~3000 行删除）
   用户不受影响   新用户装不到   老用户副本仍在   仓库变"纯桌面仓"
```

| 档 | 用户影响 | 仓库收益 | 前置条件 |
|---|---|---|---|
| A 冻结 | 无（继续能装到 2.10.45） | 无 | 无 |
| B 撤市 | 新用户装不到；老用户 update 失败/无更新 | 停止对外背债 | 无 |
| C 删产物 | 同上 + 市场不再指向死路径 | 仓库瘦 11MB | 无 |
| D 全删 | 同上；`$schema` 补全、archi 能力、Cursor 镜像清理三处真缺口暴露 | 仓库叙事统一，spec 不自相矛盾 | **两笔欠账要先还（见 §6）** |

值得注意：**A/B/C 都不需要还欠账，D 需要**。这也是"下线"到底指哪一档的答案分水岭。

---

## 6. 两个硬前置 + 一个耦合

### 6.1 protect-files 护栏（spec 自己写的门槛）

`desktop-change-orchestration` V1 边界 #5 原文：

> 写保护面仅剩 SDK 路径沙箱……桌面原生护栏（内核 permission 层方向）的独立 change 为缺口出口（**插件真下线前落地**）

也就是说：跑 D 档 = 主动违背自家 spec 的一句前置承诺，除非：

- (a) 先落地桌面原生护栏 change；或
- (b) 用本 change 显式改掉那句话，把缺口永久留痕（等于承认"写保护只有 workspace 前缀校验"）。

### 6.2 archi 能力无家

C4 模型解析 + 拓扑校验 + ADR 管理，桌面侧只有一份方案稿（`desktop-architecture-guard.md`，S1/S2/S3 三个 Spike 都没开工）。`core/archi` 的探针还给出三条硬约束（七工具封闭集无 MCP 通道、门禁只能挂 `StepNode`、显式取数）。三个选项：

| 选项 | 含义 |
|---|---|
| 先移植再删 | 等 `core/archi` 落地（工作量 = 三 Spike），插件保留到那天 |
| 一并送走 | 承认 archi 能力整体退役；`.c4` / ADR 使用者的仓库从此无工具（本仓库 `openspec/architecture/` 目录其实也不存在，实际使用者可能接近零） |
| 留残骸 | 保留 `templates/{adr,model}.c4` + 一个 `archi_decide` 等价物到别处 —— 最差选项，责任不清 |

### 6.3 耦合：explore stance 双源

`plugins/dev-team/skills/openspec-explore/SKILL.md` ↔ `packages/desktop/src/lib/explore-stance.ts` 是**双源互链**，10-17 刚为它同步过一次。D 档删除后，`desktop-explore-page` 的双源条款、`explore-stance.ts` 的注释都要退成单源。

---

## 7. 若走 D 档：执行顺序草案

```
①  （前置）决定 archi 与 protect-files 的处置  ← 只有人能拍
②  发"遗言"版本：插件内加弃用提示 + README 加迁移指引
    （可选但强烈建议：给了老用户一个"为什么"）
③  摘 marketplaces 条目（与 ④ 同提交）
④  删 dist/*/dev-team + plugins/dev-team
⑤  清引用：openspec/config.json、.claude/settings.local.json、
            desktop 注释、9 份 desktop spec 的 17 处 delta
⑥  删 51 份插件期 spec + 重写 README / AGENT.md / CLAUDE.md
⑦  （顺带）修 desktop-phase-prompts 的 14→16 漂移
```

顺序上有一个硬约束：**③④ 必须同提交**（市场指向相对路径）。⑤⑥ 可以拆成第二个 change（"清语料"），但一旦 ④ 合入，仓库就处于"spec 描述不存在的代码"状态，越短越好。

---

## 8. 待你拍板的问题

1. **档位**：目标是 B（撤市、留源）、C（撤市 + 删产物）、还是 D（连源码语料一起删）？AGENT.md 的措辞指向 D，但 D 有两个前置。
2. **archi 怎么办**：先移植 / 一并送走 / 保留残骸？如果用户群实际不用 `.c4`，送走最干净。
3. **protect-files 缺口**：先做桌面原生护栏，还是本 change 直接改 spec 留痕？后者省事，但等于公开承认"没有实时写保护"。
4. **遗言版本**：要不要在删除前发一版带弃用提示的 2.10.46？（成本 ≈ 一次 bump + 两行文案）
5. **marketplace 归零**：删文件 vs 留空壳 `plugins: []`？（留壳能让老用户 `marketplace update` 时看到"市场还在、插件没了"，而不是 404）
6. **51 份插件期 spec**：整目录删 vs 搬进 `openspec/changes/archive/` 留档？我倾向整目录删——它们描述的是不存在的代码，正是 explore 里被点名过的"僵尸 spec"问题的 51 倍版。

---

## 9. 附带发现（与本主题相邻，可另开线索）

- `desktop-phase-prompts` spec 说 14 份 prompt，磁盘 16 份（code-analyze 未登记）—— spec 漂移。
- `install.mjs` 无卸载路径（只有 `--root`）：如果决定撤掉 home 镜像通道，需要一版能 `--remove` 的安装器，否则用户 `~/.cursor` 里的 hooks/mcpServers 会永久留残。
- `dev-team-config.schema.json` 的"家"问题：桌面复刻了校验语义但不产 schema，`$schema` 补全能力会静默消失。若要保留，schema 得搬去 `packages/desktop` 并加进发版产物。

---

## 10. 决策落定：D 档 · archi 送走 · 无需通知

用户裁决（2026-10-25）：

1. **档位 = D**（源码 + 产物 + 语料 + 文档全删）
2. **archi 直接删除**（不移植、不留残骸；`.c4` / ADR 能力整体退役）
3. **protect-files 缺口已补齐**（桌面原生护栏已落地）
4. **无需通知**（不发弃用版本、不做迁移提示）

### 10.1 护栏核实：已落地，但 spec 措辞过期

`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/`（mod / rules / shell / inline / reason，5 + 5 测试文件）已实现工具调用前写保护闸，落在 loop 守卫链第四段：

```
成员校验 → policy 档位门 → 路径沙箱 → [写入保护闸] → 执行
                                        │
             内置规则（恒在、fail-closed）: **/openspec/config.json
             用户规则（fail-open）: config.write_protection.files[]
             射程: write/edit（按 path）+ bash（命令文本提取 + 批量还原拦截 + node/python 内联判别）
             放行: read/grep/glob/ls + create_explore_note
```

比插件 hook 等价或更强（插件 hook 依赖会话内 PreToolUse，桌面这层是 SDK 引擎内的，SDK 是唯一活引擎）。

**但**：`openspec/specs/desktop-change-orchestration/spec.md:384` 的 V1 边界 #5 仍写着「缺口自本变更起显式打开……桌面原生护栏为缺口出口（插件真下线前落地）」。D 档变更必须一并把这条改写为「护栏已由 desktop-write-protection 承接」，否则删完之后 spec 里躺着一句自我矛盾的过期承诺。

### 10.2 精确删除清单（按组）

**A 组 · 源码与产物（361 tracked 文件）**

| 路径 | tracked | 备注 |
|---|---|---|
| `plugins/dev-team/**` | 237 | 含 skills 13 / agents 17 / hooks 1 / bin/src 57 / build 10 / templates 5；`.pack-staging`、`node_modules`、`reports` 未 tracked 但需一并物理删 |
| `dist/claude-plugins/dev-team/**` | ~41 | |
| `dist/cursor-plugins/dev-team/**` | ~41 | |
| `dist/cursor-home-image/dev-team/**` | ~42 | 含 `install.mjs`（生成物） |

**B 组 · 对外通道（2 文件）**

| 路径 | 处置 |
|---|---|
| `.claude-plugin/marketplace.json` | 删 `plugins[]` 条目；文件本身留空壳（见 §11 待决） |
| `.cursor-plugin/marketplace.json` | 同上（另有 `metadata.pluginRoot` 一并处理） |

**C 组 · 插件期语料（51 份，非 52）**

`openspec/specs/` 下 85 个 capability，52 个无 `desktop-` 前缀。逐个甄别后：

- **必须保留 1 份**：`workspace-layout-resolution` —— 它其实是 **desktop foundation crate 的活 spec**（讲 `Layout` / `resolve`），只是命名没带 `desktop-` 前缀。**按前缀一刀切会误删活 spec。**
- **删除 51 份**，其中三份本身就是尸体，正好一并清掉：
  - `glob-matching/spec.md` —— **0 字节空文件**
  - `mutation-score-gap-fill/` —— **目录里没有 spec.md**
  - `architecture-commit-gate`、`c4-cli-integration` —— 代码半年内已不存在（desktop-architecture-guard explore 已点名）

删除后 `openspec/specs/` = 33 份 `desktop-*` + 1 份 `workspace-layout-resolution` = 34。

**D 组 · 引用清理（12 文件 + 1 配置）**

| 文件 | 行/处 | 处置 |
|---|---|---|
| `openspec/config.json` | `$schema`（plugins 路径 URL）+ `write_protection.files[0].glob`（`plugins/dev-team/bin/knip.json`） | 删 `$schema`；`write_protection` 变空 → 是否保留字段见 §11 |
| `README.md` | 全文 152 行 | 重写为桌面仓库说明（现状 100% 是插件安装手册，零桌面内容） |
| `AGENT.md`、`CLAUDE.md` | 全文 | 重写（现把"插件 slim 架构"当主叙事） |
| `openspec/specs/desktop-*/spec.md` | 17 处 / 9 份 | 见 10.3 分类 |
| `packages/desktop/src/lib/explore-stance.ts` | 注释头 3 行 | 双源 → 单源表述 |
| `.../core/checks/tests/corpus_golden_test.rs` | 注释 1 处 | "CLI zod 权威" → "已固化 Rust 实现 + corpus 金样" |
| `.../core/checks/tests/fixtures/README.md` | 注释 1 处 | 同上 |
| `.../infra/agent/src/sdk/protect/rules_test.rs` | 1 处样本 | `plugins/dev-team/bin/knip.json` 仅作 glob 测试数据，可留（不构成依赖） |

**E 组 · 不动**

- `openspec/changes/archive/**`（132 个历史 change，含大量插件路径引用——历史事实，不改）
- `openspec/explores/**`（4 份考古笔记）
- `packages/desktop/src-tauri/**/fixtures/**` 里的 `"root": "plugins/dev-team/bin"`（纯语料数据）
- `.claude/settings.local.json`（未 tracked，本机残留，可选清理）

### 10.3 desktop spec 的 17 处：两类，处置不同

**A 类 · 「版本交付」历史条款（12 处 / 5 份）**

`desktop-change-archive`(2)、`desktop-change-create`(4)、`desktop-change-orchestration`(2)、`desktop-change-worktree`(2)、`desktop-release-pipeline`(2)，形态都是：

> 本变更 SHALL 将 `packages/desktop/package.json` 由 0.4.x 升级为 0.4.x+1 …… 本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 2.10.44）

这是 change 期的一次性交付约束，归档进 spec 后本已是历史噪声，删除插件后更是指向不存在路径。两种处置：

- (i) 整条 requirement 删掉（spec 里不留"版本交付"历史段）
- (ii) 保留段落、只摘插件半句（保留"某版本升到某版本"的考古价值）

倾向 (i)——这几条 requirement 对当前实现零约束力。

**B 类 · 行为面/权威源引用（5 处 / 4 份，必须改）**

| 位置 | 现文 | 改法 |
|---|---|---|
| `desktop-test-execution:105` | 报告 schema 以 CLI zod schema 为权威对齐源 | 改：以已固化的 Rust 实现 + `tests/golden` corpus 为权威 |
| `desktop-workspace-config:15` | 校验规则逐条复刻 CLI `config.schema.ts` | 改：以 `core/config` 现行实现为权威（复刻已完成，参照物退役） |
| `desktop-phase-prompts:5` | "迁移是对插件 `agents/*.md` 的改写" | 改：历史叙事一句或删 |
| `desktop-explore-page:94/255` | 与 `SKILL.md` 双源互链 | 改：单源（desktop 自持） |
| `desktop-change-flow-view:16` | "写入方（插件 CLI）MUST NOT 因本变更改动" | 改：写入方已不存在，条款失效 |

另有 `desktop-crate-layout:93` 的「见 workspace-layout-resolution」——该 spec 保留，无需动。

### 10.4 执行顺序（D 档）

```
① 清语料（独立第一步，可与 ② 同提交）
   - 删 51 份插件期 spec（保留 workspace-layout-resolution）
   - 改 desktop spec 的 12 处历史条款 + 5 处行为面
② 删源码与产物 + 摘市场（必须同提交：marketplace 指向相对路径）
   - plugins/dev-team/**
   - dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team
   - .claude-plugin/marketplace.json / .cursor-plugin/marketplace.json 摘条目
③ 清配置与文档
   - openspec/config.json（$schema / write_protection）
   - README.md / AGENT.md / CLAUDE.md 重写
   - desktop 侧 3 处注释
   - （顺带）desktop-phase-prompts 14→16 漂移
④ （顺带）desktop-change-orchestration 边界 #5 改写为护栏已承接
⑤ 验证
```

### 10.5 验证点

| 检查 | 命令 / 判据 |
|---|---|
| 无悬空路径引用 | `git grep -n "plugins/dev-team" -- ':!openspec/changes/archive' ':!openspec/explores'` 只应剩历史/考古面 |
| 无悬空 spec | `ls openspec/specs` = 34 项（33 desktop-* + workspace-layout-resolution） |
| 无空壳/空目录 | `find openspec/specs -type d -empty` 与 `find openspec/specs -name spec.md -size 0` 均空 |
| desktop 全绿 | `pnpm -C packages/desktop run check` + `run test`（`cargo test --workspace` 含 corpus golden） |
| 版本交付 | `packages/desktop/package.json` bump（0.5.25 → 0.5.26）；`src-tauri/Cargo.toml` 不随动 |

### 10.6 与桌面端"零依赖"结论的交叉验证

跨包 grep 已确认 desktop 运行时对插件零依赖（只命中注释 / 语料数据 / spec 措辞），因此 ② 步骤不会破坏任何构建或测试——唯一风险是 ① 的 spec 删除若误伤 `workspace-layout-resolution`（已单独标红）。

---

## 11. 补充裁决（2026-10-25 二次）：四项细节定稿

| # | 问题 | 裁决 |
|---|---|---|
| 1 | marketplace 删文件 vs 留空壳 | **删文件** |
| 2 | 12 处「版本交付」历史条款 | **只摘插件半句**，保留版本交付段 |
| 3 | `$schema` 补全 | **迁进 `packages/desktop`** |
| 4 | 边界 #5 措辞 | **整条删除**（我的判断，理由见 11.2） |

### 11.1 裁决 1、2 的落点

**marketplace 两文件整删**：`.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json`。市场整体消失，老用户 `marketplace update` 得到"市场不存在"，与"插件已下线"语义一致（既然不通知，空壳只会延长悬置态）。

**「版本交付」条款只摘插件半句** → 12 处 / 5 份改为「只留 desktop 版本 bump 段，删 `plugins/dev-team` 半句与对应 THEN 断言」：

| 文件 | 行 | 现文形态 | 摘除后 |
|---|---|---|---|
| `desktop-change-archive` | 173 / 178 | "…SHALL NOT 变更 plugins/dev-team（版本保持 2.10.44…）" + "THEN … plugins/dev-team version 仍为 2.10.44，SKILL.md 原文未动" | 删括号半句 + THEN 第二个分句 |
| `desktop-change-create` | 169 / 174 / 202 | 同上 ×3（含影响面表格行「`plugins/dev-team` 不动」） | 删半句 / 删 table 列值 |
| `desktop-change-orchestration` | 400 / 405 | 同上 | 同上 |
| `desktop-change-worktree` | 184 / 189 | 同上 | 同上 |
| `desktop-release-pipeline` | 127 / 132 | 同上 | 同上 |

窗口标题同款：`desktop-change-archive:175` / `desktop-change-orchestration:402` 的 scenario 名「版本号升级与插件零改动」→「版本号升级」。
`desktop-change-create` 的另两处（160「插件侧零改动」、163「与插件 `createChange` parity」）属"变更边界声明"而非版本交付，按 §11.3 的 M 档处理。

### 11.2 裁决 4：边界 #5 整条删除（含 716 行表格行）

**决定：删。** 三条理由：

1. **边界清单的语义是"V1 显式限制 + 后续偿还"**，不是变更日志。该清单自己已有"消失即删"的先例——紧邻的括注写着「原『sessionAnchors 精度损失』『sweep-phase 缺口』两条边界随写面进程内锚点复活而消失，**不再留痕**」。护栏补齐后 #5 同属"已偿还"，按同款惯例该消失。
2. **保留"已补齐"会让 4 条真边界被 1 条假边界稀释**。清单的价值密度来自"每一条都还是约束"。
3. **防回退该落在不变量上，不落在边界留痕里**。若担心未来有人重引 CLI 租户又丢掉护栏，防线是 `desktop-write-protection` spec 本身（护栏是不变量，与引擎形态无关），而不是 orchestration 的一条"历史说明"。

连带要改的两处：
- `desktop-change-orchestration:388` 的 scenario「五条边界均可考」→「四条边界均可考」，AND 子句里的「含计划图静态编译期翻案留痕与护栏缺口显式留痕」→ 去掉后半句。
- `desktop-change-orchestration:716` 表格行「既有 hooks（复用，不改）｜…｜protect-files 原样生效（过渡期）；record-files / sweep-phase 未绑定 no-op」**整行删除**（hooks 不复存在，record-files / sweep-phase 的 no-op 说明也失去对象）。

### 11.3 desktop spec 引用面的四档精分（比 §10.3 的 17 处更细）

初次统计只 grep 了 `plugins/dev-team` 字面量，漏掉大量只写"插件"二字的行。完整盘查后分四档：

**M 档 · 必须改（描述对象已不存在，改完才自洽）— 10 处 / 6 份**

| 位置 | 现文 | 改法 |
|---|---|---|
| `desktop-test-execution:105` | 报告 schema 以 CLI zod schema（plugins 路径）为权威对齐源 | 改为「以已固化的 Rust 实现 + `tests/golden` corpus 为权威」 |
| `desktop-workspace-config:15` | 校验规则逐条复刻 CLI `config.schema.ts` | 改为「以 `core/config` 现行实现为权威，参照物已退役」 |
| `desktop-explore-page:94` | stance 与 `SKILL.md` 双源互链 | 改单源（desktop 自持） |
| `desktop-explore-page:255` | 表格行「`plugins/dev-team/skills/openspec-explore/SKILL.md` ｜笔记落盘约定文档｜双源同步」 | 整行删 |
| `desktop-phase-prompts:5` | "迁移是对插件 `agents/*.md` 的改写而非搬运" | 删该从句（或改「16 份全静态角色知识」陈述，顺带修 14→16） |
| `desktop-change-flow-view:16` | "写入方（插件 CLI）MUST NOT 因本变更改动" | 删该分句 |
| `desktop-change-flow-view:609` | 表格行「磁盘形状与写入方（不改）｜…｜TS `workflow.schema.ts` 与插件 CLI 写入方不动」 | 整行删 |
| `desktop-workflow-write-face:249` | 表格行「插件 MCP / TS 写路径（彻底分离，不改）｜双向墙：desktop 不读不写 workflow.json；插件零改动」 | 整行删（双向墙另一方消失，只剩单边） |
| `desktop-change-create:160` | 边界「插件侧零改动：`plugins/dev-team` 全部冻结…」 | 删该条边界（连带场景计数） |
| `desktop-change-create:163` | "与插件 `createChange` parity" | 改为直述行为（归档树同名不拒绝） |

**H 档 · 历史条款只摘半句 — 12 处 / 5 份**（见 11.1）

**R 档 · 保留（仍是有效约束，插件作为反面参照）— 8 处 / 4 份**

| 位置 | 为什么留 |
|---|---|
| `desktop-phase-prompts:66` | "MUST NOT 携带插件 Agent / MCP 工具假设"——防回退护栏，插件删了更该留 |
| `desktop-phase-prompts:72` | "无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` 注入标记"——同上，扫描断言仍需有靶子 |
| `desktop-crate-layout:11` | "MUST NOT 进入 `dist/` 插件分发链"——dist 没了，但"desktop 不走分发链"仍是有效边界；措辞可微调为"仓库不再有插件分发产物" |
| `desktop-crate-layout:19` | "不受插件 bump 规则约束"——仍需约束 desktop 自己；`plugins/<name>` 路径措辞改掉 |
| `desktop-write-protection:13/14` | "与插件端同款处理"——可保留（叙述"相对 glob 加 `**/`"的由来），建议改为直述规则本身 |
| `desktop-write-protection:126/190/207` | "插件端三态提取之外/此处放行"——同上，语义靠"本能力更强"表达；无对照读者也能读 |
| `desktop-workflow-write-face:106` | "修复插件侧已知缺陷"——可保留（缺陷描述自身完整） |
| `desktop-workflow-write-face:115` | "对照既往 desktop 直跑形态验收"——与插件无关的验收口径，原文即"MUST NOT 建差分 oracle" |

**N 档 · 与 dev-team 无关，零处理**

`desktop-artifact-plugins` 的 9 处"插件"（产物 matcher/renderer 插件化架构）、`desktop-change-queries:54`（artifact 插件发现）、`desktop-change-state-store:59`（写面）、`desktop-artifact-plugins` 全文。**这组是命名撞车，不是依赖**。

### 11.4 裁决 3：schema 迁进 `packages/desktop` 的落点与代价

**落点推荐**：`packages/desktop/src-tauri/crates/core/config/config.schema.json`

```
crates/core/config/
├── Cargo.toml
├── src/lib.rs          ← KNOWN_KEYS[7] + TestFramework 八值枚举（校验实现）
├── src/lib_test.rs
└── config.schema.json  ← 迁入此处，与实现物理相邻
```

三条理由：① 与校验实现同目录，"改字段顺手改 schema"最可能发生；② crate 内可加 `include_str!` 一致性测试；③ 不参与 tauri bundle（schema 是编辑器消费，走 GitHub raw URL），bundle 体积零变化。

核查过 `desktop-crate-layout:93` 的隔离禁令（产品 `.rs` MUST NOT 出现 `config.json` 文件名字面量）——`config.schema.json` 字面量**不含** `config.json` 子串，且只在 `_test.rs` 出现，不触碰禁令。

**`$schema` URL 变更**：

```
旧  …/master/plugins/dev-team/bin/dev-team-config.schema.json
新  …/master/packages/desktop/src-tauri/crates/core/config/config.schema.json
```

两处引用：`openspec/config.json` 自身、README（重写时直接用新 URL）。

**代价（必须记账）**：schema 原由 `configSchema.toJSONSchema()` 从 zod 生成（`build/build-config-schema.ts`），迁出后**失去生成源，变成手工维护的静态 JSON**。防漂移只能靠测试，建议同 crate 加一对断言：

```
KNOWN_KEYS 七项 ⊂ schema.properties.keys()        （7 == 7）
TestFramework 八值 == schema.properties.tests.items.properties.framework.enum
```

（7 项 top props 与 Rust `KNOWN_KEYS` 现已逐字对齐；八值枚举 `jest/vitest/vite-plus/bun/rust/node-test/go/pytest` 同序同串——已核。）

**副作用**：schema 位于 `packages/desktop/**` 下，改动会触发 `desktop-release.yml` 的 detect job；不 bump `package.json` 时 `should=false` 自动跳过发版，无实际影响。

### 11.5 修正后的最终执行清单

```
① 清语料
   - 删 51 份插件期 spec（保留 workspace-layout-resolution）
   - desktop spec：M 档 10 处改 / H 档 12 处摘半句 / R 档 8 处酌情微调
   - 删边界 #5（+ scenario 计数 + 716 表格行）
② 迁 schema
   - 复制 schema → packages/desktop/src-tauri/crates/core/config/config.schema.json
   - 加一致性测试（KNOWN_KEYS / 八值枚举）
   - 改 openspec/config.json 的 $schema
③ 删源码与产物 + 摘市场（同提交）
   - plugins/dev-team/**（含未 tracked 的 .pack-staging / node_modules / reports）
   - dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team
   - 删 .claude-plugin/marketplace.json 与 .cursor-plugin/marketplace.json
④ 清配置与文档
   - openspec/config.json：删 write_protection（files 仅剩 plugin 那条，删后为空）
   - README.md / AGENT.md / CLAUDE.md 重写
   - desktop 侧注释 3 处（explore-stance.ts / corpus_golden_test.rs / fixtures/README.md）
   - （顺带）desktop-phase-prompts 14→16 漂移
⑤ 验证
   - git grep "plugins/dev-team" 只剩 archive / explores / fixture 数据
   - ls openspec/specs = 34 项；无空 spec.md / 空目录
   - pnpm -C packages/desktop run check && run test
   - packages/desktop/package.json bump 0.5.25 → 0.5.26
```

**④ 的一个细节**：`openspec/config.json` 的 `write_protection.files` 当前只有一条（`plugins/dev-team/bin/knip.json`，见仓库现状）。删插件后该数组为空 → 按「不要留空壳配置」原则**整段删 `write_protection`**；`desktop-write-protection` 的内置规则（`**/openspec/config.json` + eval.json 面）不受影响。
