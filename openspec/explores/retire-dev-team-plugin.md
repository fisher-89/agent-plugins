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
