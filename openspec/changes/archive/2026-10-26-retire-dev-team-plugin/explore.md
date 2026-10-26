# 下线 plugins/dev-team：决策与执行清单

workflow_type: requirement

---

## 决策摘要

| # | 项 | 结论 |
|---|---|---|
| 1 | 下线深度 | **D 档全删**：源码 + 产物 + 语料 + 文档，仓库变纯桌面仓 |
| 2 | archi 能力 | **直接删除**，不移植、不留残骸；`.c4` / ADR 整体退役 |
| 3 | 用户通知 | **不做**（无弃用版本、无迁移提示） |
| 4 | marketplace | **两个 json 整删文件**（不留空壳） |
| 5 | 版本交付条款 | **只摘插件半句**，保留 desktop 版本 bump 段 |
| 6 | config schema | **迁进 `packages/desktop`**，`$schema` URL 随之更换 |
| 7 | 写保护 | 缺口已由桌面原生护栏补齐；`write_protection` 改指 `packages/desktop/knip.json` |
| 8 | 边界 #5 | **整条删除**（已偿还的边界不留痕） |

依赖核查结论：**desktop 运行时对插件零依赖**（跨包 grep 只命中注释、语料 fixture、spec 措辞），删除不破坏任何构建与测试。

---

## 1. 现状

`plugins/dev-team` 冻结在 2.10.45（最后一次实质功能提交 09-28 的 2.10.44），仍是一台完整分发机器：

```
                        plugins/dev-team（237 tracked / 69,101 行 ts+md）
   ┌────────────┬────────────┬────────────┬──────────────┬──────────────┐
   │ skills/ 13 │ agents/ 17 │ hooks/ 4类 │ CLI 2 + MCP 17│ templates/ 5 │
   └─────┬──────┴─────┬──────┴─────┬──────┴──────┬───────┴──────┬───────┘
         └────────────┴────────────┴─────────────┴──────────────┘
                                   │  build/ + vite pack + assemble
                                   ▼
              dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team
                          （124 tracked，3.8MB × 3 份）
                                   │
                    ┌──────────────┴───────────────┐
                    ▼                              ▼
        .claude-plugin/marketplace.json    .cursor-plugin/marketplace.json
                    │                              │
                    └────────► README.md（152 行，全部是插件安装手册）
```

### 1.1 桌面端已接走的知识

| 插件资产 | 桌面替代 |
|---|---|
| 17 个 agent | ✅ `core/workflow/src/prompts/` 16 份全静态 md（含 code-analyze 两份），**仅 `architecture.md` 无对应** |
| 相位路由 / 落账 / 回溯 | ✅ `core/workflow` 原生 |
| change 管理（6 工具） | ✅ 原生（含 store） |
| 测试 / 静态检查 | ✅ `infra/checks` 原生 |
| 5 个 `archi_*` 工具 | ❌ 无替代 → 随本次删除退役 |
| hooks `record-files` / `static-check` / `sweep-phase` | ✅ 分别由 vcs `change_edit_set` / spawn 步 / no-op 取代 |
| hooks `protect-files` | ✅ 已由桌面原生护栏承接（见 1.2） |
| `bin/dev-team-config.schema.json` | ⚠️ 无替代 → 本次迁入 `packages/desktop`（见 §4） |
| `explore/SKILL.md` 笔记约定 | ✅ 桌面 `explore-stance.ts` 自持（双源转单源） |

### 1.2 写保护护栏已落地

`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/`（5 实现 + 5 测试）落在 loop 守卫链第四段：

```
成员校验 → policy 档位门 → 路径沙箱 → [写入保护闸] → 执行
                                        │
  内置规则（恒在 / fail-closed）: **/openspec/config.json
  用户规则（fail-open）        : config.write_protection.files[]
  射程: write/edit（按 path）+ bash（命令文本提取 / 批量还原拦截 / node·python 内联判别）
  放行: read/grep/glob/ls + create_explore_note
```

强于插件 hook（后者只在会话内生效；桌面这层在 SDK 引擎内，SDK 是唯一活引擎）。

### 1.3 删除后各类使用者的影响

| 使用者 | 影响 |
|---|---|
| 装了插件的成员（Claude / Cursor） | 已装副本继续跑；`plugin update` 静默无更新（不通知是既定裁决） |
| Cursor home 镜像用户 | `~/.cursor` 里的 hooks / mcpServers 留残（`install.mjs` 无卸载模式，见 §7） |
| 本仓库自身 | `$schema`、`write_protection`、desktop 注释、51 份 spec 需同步清理（§3、§4） |
| desktop 运行时 | 无影响 |

---

## 2. 删除范围

### 2.1 源码与产物（361 tracked）

| 路径 | tracked | 备注 |
|---|---|---|
| `plugins/dev-team/**` | 237 | 含 skills 13 / agents 17 / hooks 1 / bin/src / build / templates 5；`.pack-staging`、`node_modules`、`reports` 未 tracked 但需一并物理删 |
| `dist/claude-plugins/dev-team/**` | ~41 | |
| `dist/cursor-plugins/dev-team/**` | ~41 | |
| `dist/cursor-home-image/dev-team/**` | ~42 | 含生成的 `install.mjs` |

### 2.2 对外通道

`.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` —— **两文件整删**。市场整体消失，与"插件已下线"语义一致。

> **同提交硬约束**：marketplace 指向相对路径 `./dist/claude-plugins/dev-team`，删产物与删市场必须同一提交，否则中间态市场指向 404。

### 2.3 插件期语料（删 51，留 1）

`openspec/specs/` 现有 85 个 capability，52 个无 `desktop-` 前缀。逐个甄别：

```
85 capability
├─ 33  desktop-*                   → 留
├─  1  workspace-layout-resolution → 留（陷阱：它是 desktop foundation crate 的活 spec，
│                                      讲 Layout / resolve，只是命名没带前缀；按前缀一刀切会误删）
└─ 51  插件期                       → 删
      其中三份本就是尸体，一并清掉：
        glob-matching/spec.md        0 字节空文件
        mutation-score-gap-fill/     目录里没有 spec.md
        architecture-commit-gate     代码半年前已不存在
        c4-cli-integration           代码半年前已不存在
```

删除后 `openspec/specs/` = 34 项（33 + 1）。

### 2.4 不动

- `openspec/changes/archive/**` —— 132 个历史 change，含插件路径引用属历史事实
- `openspec/explores/**` —— 考古笔记
- `packages/desktop/src-tauri/**/fixtures/**` 里的 `"root": "plugins/dev-team/bin"` —— 纯语料数据
- `packages/desktop/.../protect/rules_test.rs` 的 `plugins/dev-team/bin/knip.json` 样本 —— glob 测试数据，非依赖
- `.claude/settings.local.json` —— 未 tracked 的本机残留，可选清理

---

## 3. 引用清理

### 3.1 desktop spec 四档

初版只 grep `plugins/dev-team` 字面量会漏掉大量只写"插件"的行。完整盘查后四档：

**M 档 · 必须改（10 处 / 6 份）** —— 描述对象已不存在，改完才自洽

| 位置 | 改法 |
|---|---|
| `desktop-test-execution:105` | 「以 CLI zod schema 为权威」→「以已固化 Rust 实现 + `tests/golden` corpus 为权威」 |
| `desktop-workspace-config:15` | 「逐条复刻 CLI `config.schema.ts`」→「以 `core/config` 现行实现为权威，参照物已退役」 |
| `desktop-explore-page:94` | 双源互链 → 单源（desktop 自持） |
| `desktop-explore-page:255` | 整行删（`plugins/dev-team/skills/openspec-explore/SKILL.md` 表格行） |
| `desktop-phase-prompts:5` | 删「迁移是对插件 `agents/*.md` 的改写」从句 |
| `desktop-change-flow-view:16` | 删「写入方（插件 CLI）MUST NOT 因本变更改动」分句 |
| `desktop-change-flow-view:609` | 整行删（磁盘形状与写入方表格行） |
| `desktop-workflow-write-face:249` | 整行删（双向墙另一方消失） |
| `desktop-change-create:160` | 删「插件侧零改动」边界条（连带场景计数） |
| `desktop-change-create:163` | 「与插件 `createChange` parity」→ 直述行为 |

**H 档 · 历史条款只摘半句（12 处 / 5 份）**

`desktop-change-archive`(173/178)、`desktop-change-create`(169/174/202)、`desktop-change-orchestration`(400/405)、`desktop-change-worktree`(184/189)、`desktop-release-pipeline`(127/132)，形态均为：

> 本变更 SHALL 将 `packages/desktop/package.json` 由 0.4.x 升级为 0.4.x+1 …… SHALL NOT 变更 `plugins/dev-team`（版本保持 2.10.44）

处置：**保留 desktop 版本 bump 段，删插件半句与对应 THEN 断言**；`desktop-change-create:202` 的影响面表格列值同样摘除。两处 scenario 名「版本号升级与插件零改动」→「版本号升级」（`change-archive:175`、`change-orchestration:402`）。

**R 档 · 保留（8 处 / 4 份）** —— 仍是有效约束，插件作为反面参照

| 位置 | 为什么留 |
|---|---|
| `desktop-phase-prompts:66 / 72` | 「MUST NOT 携带插件 Agent / MCP 工具假设」「无 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` 标记」—— 防回退护栏，插件删了更该留 |
| `desktop-crate-layout:11 / 19` | 「不走插件分发链」「不受插件 bump 规则约束」—— 边界仍有效，只微调 `plugins/<name>` 措辞 |
| `desktop-write-protection:13/14/126/190/207` | 「与插件端同款 / 插件端此处放行」—— 建议改直述规则本身，语义不依赖对照读者 |
| `desktop-workflow-write-face:106 / 115` | 缺陷描述自身完整；验收口径原文即「MUST NOT 建差分 oracle」 |

**N 档 · 零处理** —— `desktop-artifact-plugins`(9 处)、`desktop-change-queries:54`、`desktop-change-state-store:59`：这里的"插件"指产物 matcher/renderer 插件化架构，**命名撞车不是依赖**。

### 3.2 边界 #5 删除（含连带两处）

`desktop-change-orchestration` V1 边界清单的语义是「显式限制 + 后续偿还」，不是变更日志。该清单自身已有先例——紧邻括注写明「原『sessionAnchors 精度损失』『sweep-phase 缺口』两条边界随写面进程内锚点复活而消失，**不再留痕**」。护栏补齐后 #5 同属已偿还，按同款惯例消失；保留「已补齐」会让 4 条真边界被 1 条假边界稀释，而防回退本应落在 `desktop-write-protection` 的不变量上（护栏与引擎形态无关），不是 orchestration 的历史说明。

连带：

- `:384` 边界 #5 整条删
- `:388` scenario「五条边界均可考」→「四条」，去掉「护栏缺口显式留痕」子句
- `:716` 表格行「既有 hooks（复用，不改）｜protect-files 原样生效（过渡期）」整行删

### 3.3 配置与文档

| 文件 | 处置 |
|---|---|
| `openspec/config.json` | `$schema` 换新 URL；`write_protection.files[0].glob` 改指 `packages/desktop/knip.json`（见 3.4） |
| `README.md`（152 行） | **重写**为桌面仓库说明（现状 100% 插件安装手册，零桌面内容；发版走 GitHub Releases，见 `packages/desktop/RELEASE.md`） |
| `AGENT.md`、`CLAUDE.md` | 重写为桌面仓库说明（现状把「插件 slim 架构」当主叙事） |
| `packages/desktop/src/lib/explore-stance.ts` | 注释头双源 → 单源表述 |
| `core/checks/tests/corpus_golden_test.rs` / `fixtures/README.md` | 「CLI zod 权威」→「已固化 Rust 实现 + corpus 金样」 |
| `desktop-phase-prompts` spec | 顺带修 14→16 份 prompt 漂移 |

### 3.4 `write_protection` 换指向

```json
"write_protection": {
  "files": [{ "glob": "packages/desktop/knip.json" }]
}
```

保护动机随资产位置平移：knip 配置决定死代码检出范围，agent 只需往 `ignore` / `ignoreDependencies` 塞几行就能让未用导出静默过关。桌面端这个动机更重——`packages/desktop/knip.json` 是 `client:check`（`vp check --fix && knip`）的唯一配置源，而 `client:check` 正是 `openspec/config.json` 的 `static_analysis` 入口（`pnpm run -C ./packages/desktop check`）。

落实要点：相对 glob 照原样写（装配层自动前缀 `**/`）；命中面 `write`/`edit` 按路径 + `bash` 按命令文本；用户规则 fail-open，但内置 `**/openspec/config.json` 恒在，本行被改坏时 `config.json` 仍受保护。

---

## 4. config schema 迁移

**落点**：`packages/desktop/src-tauri/crates/core/config/config.schema.json`

```
crates/core/config/
├── src/lib.rs          ← KNOWN_KEYS[7] + TestFramework 八值枚举（校验实现）
├── src/lib_test.rs
└── config.schema.json  ← 迁入，与实现物理相邻
```

- 与校验实现同目录，"改字段顺手改 schema"最可能发生；crate 内可加 `include_str!` 一致性测试；不参与 tauri bundle（编辑器消费，走 GitHub raw URL），体积零变化。
- 隔离禁令已核：`desktop-crate-layout:93` 禁产品 `.rs` 出现 `config.json` 字面量，`config.schema.json` 不含该子串且只在 `_test.rs` 出现。

**`$schema` URL 变更**

```
旧  …/master/plugins/dev-team/bin/dev-team-config.schema.json
新  …/master/packages/desktop/src-tauri/crates/core/config/config.schema.json
```

引用点：`openspec/config.json`、README（重写时直接用新 URL）。

**已付代价**：schema 原由 `configSchema.toJSONSchema()` 从 zod 生成（`build/build-config-schema.ts`），迁出后失去生成源，**变为手工维护的静态 JSON**。防漂移靠同 crate 一对断言（现状已逐字对齐）：

```
KNOWN_KEYS 七项        == schema.properties.keys()
TestFramework 八值枚举  == schema.properties.tests.items.properties.framework.enum
```

**副作用**：schema 落在 `packages/desktop/**` 会触发 `desktop-release.yml` 的 detect；不 bump 版本时 `should=false` 自动跳过，无实际影响。

---

## 5. 执行顺序

```
⓿ 前置修复（已完成，见 §8）
   - crate 环境锁中毒自愈单点（lock_test_env）
   - 预算类用例夹具 / 档位改造（detect 探测成本不再吃掉预算余量）
① 清语料
   - 删 51 份插件期 spec（保留 workspace-layout-resolution）
   - desktop spec：M 档 10 处改 / H 档 12 处摘半句 / R 档 8 处微调
   - 删边界 #5（+ scenario 计数 + :716 表格行）
② 迁 schema
   - 复制 → crates/core/config/config.schema.json
   - 加一致性测试（KNOWN_KEYS / 八值枚举）
③ 删源码与产物 + 删市场（必须同提交）
   - plugins/dev-team/**（含 .pack-staging / node_modules / reports）
   - dist/{claude-plugins,cursor-plugins,cursor-home-image}/dev-team
   - 删 .claude-plugin/marketplace.json 与 .cursor-plugin/marketplace.json
④ 清配置与文档
   - openspec/config.json（$schema 新 URL / write_protection 换指向）
   - README.md / AGENT.md / CLAUDE.md 重写
   - desktop 侧注释 3 处 + phase-prompts 14→16 漂移
⑤ 验证（见 §6）
```

① 与 ③ 建议同 change 完成：一旦产物删除而 spec 未清，仓库即处于「spec 描述不存在的代码」状态。

---

## 6. 验证判据

| 检查 | 命令 / 判据 |
|---|---|
| 无悬空路径引用 | `git grep -n "plugins/dev-team" -- ':!openspec/changes/archive' ':!openspec/explores' ':!packages/desktop/src-tauri/crates/core/checks/tests/fixtures'` 为空 |
| spec 数量 | `ls openspec/specs` = 34 项（33 `desktop-*` + `workspace-layout-resolution`） |
| 无空壳 | `find openspec/specs -type d -empty` 与 `find openspec/specs -name spec.md -size 0` 均为空 |
| 配置可解析 | `openspec/config.json` 的 `$schema` 可取到；`write_protection.files[0].glob` 命中 `packages/desktop/knip.json` |
| desktop 全绿 | `pnpm -C packages/desktop run check` && `run test`（`cargo test --workspace` 含 corpus golden） |
| 门禁 flake 不回归 | `cargo test -p checks-runtime --lib` 全绿（229 passed，口径见 §8.4；含高负载复跑） |
| 版本交付 | `packages/desktop/package.json` 0.5.25 → 0.5.26；`src-tauri/Cargo.toml` 不随动 |

---

## 7. 附带发现

- `desktop-phase-prompts` spec 写「恰好 14 个文件」，磁盘上 16 份（code-analyze 两份未登记）—— 顺带修。
- `install.mjs` 只有 `--root`、无卸载模式：home 镜像通道随源码删除失去清理入口，用户 `~/.cursor` 的 hooks / mcpServers 会留残（不通知是既定裁决，此处仅留痕）。
- `glob-matching` 与 `mutation-score-gap-fill` 是历史遗留的空 capability，随本次一并清掉。
- 同类门禁开关（`stryker.config.json` / `tsconfig.json` / `vite.config.ts`）暂不纳入 `write_protection`，避免保护面铺得过大——等真出现被改动再补。

---
