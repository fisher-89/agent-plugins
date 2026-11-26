# 别的工程里没有生成器：架构生成物的「能力在、通路不在」

workflow_type: requirement

## 结论速览

**现象**：桌面端给另一个工作区（`D:\Projects\wpsweb\client\app`）建好架构域后，agent 报「INDEX.toml 仍 mismatched，需要你在有生成器的环境跑一次」。这句话准确，且 agent 确实没手改生成物。

**根因**：生成能力本身与根无关（本次已实测可给任意工作区写盘），但三条明文边界把 agent 能触发的通路全部封死，唯一逃逸口是「跑本仓的生成命令」——而那个工程没有这条命令。

**五条裁决（已定）**

| # | 裁决 | 落点 |
|---|---|---|
| D1 | 新增 agent 工具 `archi_generate`，矩阵 = `proposal/executor` + `dev-design/executor` + **`test-design/executor`（仅 `archi_module`）** + `archi` 臂；**不给 implement** | §四 |
| D2 | **implement 不再写 schema / md**；dev-design 主写 schema；proposal 写 md（dev-design 可补） | §五 |
| D3 | 用 **L3 硬闸**（锁 `models/**`）+ prompt 承载 D2；checklist **缓议**，先观察 | §五 |
| D4 | **A 纳入**：`archi_api check` 给机器可读处置结论；full 模式提示改成不撒谎的形态 | §八 |
| D5 | **F + H1 + H2**：架构 agent 可生成（= 画布不加按钮，走会话）；test-design 下发 `archi_module`；`archi_api` 保留在 implement | §八 |

**D2 使 D1 的矩阵选择自洽**：生成物的渲染输入只有 schema，代码变化不影响生成物 ⇒ implement 不改 schema ⇒ implement 期生成物恒新鲜，「不给 implement」不是妥协而是结论。

**本轮明确不做**：B 身份判据（**接受噪声**）、C 覆盖保护（**不做**，已知残余风险）、D 配置声明命令、E headless CLI；G TS 侧见证由**用户单独处理**（已移出本 change 范围）。C 的代价见 §七·R6 与 §9.2。

---

# 一 · 现场：另一台工程打不开生成通路

## 1.1 读数（实测）

| 事实 | 读数 |
|---|---|
| 工作区 | `D:\Projects\wpsweb\client\app`（git 根 `D:\Projects\wpsweb`，巨型 TS 单仓） |
| 架构域 | `.dev-team/architecture`，14:19:33 一次性落盘，35 个文件已 `git add` |
| 模型 | 5 份 `NN-*.toml`，26 个元素，多数 `api = "none"`；`Automation` 无 `api` 字段（缺省 `declared`） |
| 契约 | 25 份 `.md`，**1 份** `.schema.json`（`Automation`，14:27:59 补写） |
| `--check` 三列 | `mismatched 1 / orphaned 161 / missing 0` |
| 生成会写几份 | 2 份；`.i.ts` 已存在且逐字节相同 ⇒ **实际只动 INDEX** |
| 有无生成命令 | **没有**：`static_analysis = "pnpm run typecheck && pnpm run lint"`，`package.json` 无 `api:export`，无 cargo 工具链 |
| 插件时代残留 | `.claude/agent-memory/dev-team-*`、`.cursor/skills/*` ——「插件 CLI」一词大概源自此处 |

INDEX 的差异恰好一条 entry（副本上生成后 diff）：

```diff
 [[entry]]
 fqn = "Applications.Spreadsheet.Core.App.Dbsheet.Automation"
-status = "absent"
-types = 0
-ops = 0
-degraded = 0
-schema = ""
-generated = ""
+status = "declared"
+types = 1
+ops = 0
+degraded = 0
+schema = ".dev-team/architecture/models/…/Automation.schema.json"
+generated = "applications/spreadsheet/core/app/dbsheet/dbsheet_automation.i.ts"
+
+[entry.categories]
+class = 1
```

⇒「状态从 absent 变为 declared」完全准确：**schema 是手写真相，INDEX 是派生真相**，写 schema 必与 INDEX 脱一节拍。

## 1.2 动线：断在哪

```
archi 会话（root = 主仓）或用户手写
   ├─ write Automation.schema.json            ✅ 允许（契约资产是手写真相）
   ├─ archi_api { action: "check" }           ✅ 只读，返回两轴
   │      artifacts: { mismatched: [INDEX.toml], orphaned: [161 条], missing: [] }
   │      ※ 没有任何「下一步怎么修」的字段
   ├─ 想生成 → 工具面        archi_api 三动作 check / bootstrap / link  ❌ 无 generate
   │                         archi_element / archi_relation             ❌ 只管模型
   ├─ 退而 bash：找「本仓的生成命令」
   │      本仓：pnpm api:export ✅（但那是另一个仓库）
   │      wpsweb：没有这条命令，也没有能跑它的工具链 ❌ 死路
   └─ 只能回答「需要你在有生成器的环境跑一次」
```

同一时刻页面给的却是另一句话（`contract-panel.tsx:326-330`，spec 在 `desktop-archi-contract-panel/spec.md:190`）：

> 生成需开 change（或让左侧会话执行本仓的生成命令）

在 wpsweb 里**两个分句都不成立**：没开过 change；本仓也没有生成命令。

## 1.3 能力地图：四扇门，三扇是假门

```
                         能写 INDEX / .i.* 吗？
┌──────────────────────────────┬───────────────────────────────────────────┐
│ CLI  archi-api-gen           │ ✅ 能，cwd 锚定 ⇒ 可写任意工作区           │
│                              │   （main.rs:102-114 向上找最近架构域）      │
│ IPC  archi_api_generate      │ ✅ 能（in-process），但只有 incremental     │
│                              │   模式的契约面板按钮调它                    │
│ agent  archi_api             │ ❌ 三动作全只读                             │
│ agent  bash                  │ ⚠️ 取决于「本仓有没有生成命令」← 断在这     │
│ full 模式契约面板            │ ❌ 无按钮（有意为之，不是遗漏）             │
└──────────────────────────────┴───────────────────────────────────────────┘
```

三条边界都有明文，不是漏做：

| # | 条款 | 原文要点 |
|---|---|---|
| 1 | `desktop-archi-agent/spec.md:188` | 三动作 MUST NOT 创建/覆盖/删除任何文件；**MUST NOT 存在「写生成物 / 写契约」的专用工具**；过期「由会话经既有命令重生成」 |
| 2 | `desktop-crate-layout/spec.md:325` | **`agent-runtime` 的依赖中不出现 `archi-runtime`** |
| 3 | `desktop-archi-contract-panel/spec.md:190` + `desktop-archi-canvas/spec.md:151` | 生成是「写工作树」的动作、是人类可提交入口；full 模式 MUST NOT 有入口 |

```
desktop-app ──→ archi-runtime ──→ archi(core)
     │                ▲
     │                └── archi-api-gen(tools)
     └──→ agent-runtime ──✕── archi-runtime        （第 2 条就是这条 ✕）
```

**关键反差**：`workspace_render` + `write_artifacts` 与根无关，app 自己就能给任何工作区生成；缺的不是能力，是**让 agent 够得着的通路**。

## 1.4 三个「只在本仓成立」的假设

### 假设 1 · 每个工作区自带生成命令

- 本仓：`AGENT.md` 写着 `api:export`，`package.json` 里有。
- 别处：知识文本只说「用**既有命令**重生成」——`archi-knowledge.md:15`、`implementation-generator.md:35`、`mount.rs:387` 三处都是**悬空指代**。
- 产品侧明文「命令名归属本仓文档面，产品 MUST NOT 内建具体命令名」（`desktop-archi-agent/spec.md:188`）：本仓自举时成立，通用工作区里是空指针。

### 假设 2 · `*.i.ts` / `*.i.rs` 是生成物专属命名

`api/diff.rs:57-62`：**命名即身份**，全树任何 `*.i.rs` / `*.i.ts` 都算生成物。

```
git ls-files '*.i.ts'  →  162 份（皆为更早的手写 interface 文件，且被源码 import）
本次 --check orphaned  →  161 条（162 − 1 份本次期望的 dbsheet_automation.i.ts）
```

后果四层：

1. 生成一次后 `mismatched` 归零，但 `orphaned` **永远剩 161 条**（生成器从不删文件）。
2. 生成确认框会写「保留 161 份孤儿生成物不做处理」。
3. 架构域 `config.json` 的 `exclude` **管不到**这条轴：`scan::walk_source_files`（`scan/mod.rs:339-362`）不读排除矩阵，只有 `.gitignore` 能剪，而这些文件是入库的。
4. **最危险**：`<入口主干>.i.ts` 的落点规则会覆盖同名手写文件。本次已撞上——`dbsheet_automation.ts` 旁的 `.i.ts` 与生成结果逐字节相同；`etview.ts` + `etview.i.ts`、`execHandler.ts` + `execHandler.i.ts` 是同款并存。下一次建模就会覆盖手写契约。生成物其实有可识别特征（头注释 `生成于 archi-api … —— 勿手改`），但判据没用它。

### 假设 3 · TS 侧 `.i.ts` 是编译期见证

- `.i.rs` 在本仓经 `#[path = "lib.i.rs"] mod api_gen;` 编进 crate ⇒ 签名漂移 = 编译错。
- `.i.ts` 在本仓**零实例**；wpsweb 里生成的 `.i.ts` **无消费者**（实现 import 的是 `core/data/dbsheet/dbsheet_automation_i`），`tsc` 只检查它自身合法。
- ⇒ 那边的「形状承诺」目前只是文件存在，不是可执行判据。

---

# 二 · 现状可用的两条通路（零产品改动，已实测）

## 通路 A · 跨根生成

```bash
cd /d/Projects/wpsweb/client/app          # cwd 决定观察根
<desktop>/src-tauri/target/debug/archi-api-gen.exe generate
# 或
cargo run -q --manifest-path <desktop>/src-tauri/Cargo.toml -p archi-api-gen -- generate
```

- 副本上验证：写 2 份 → **实际只改 INDEX**（`.i.ts` 逐字节相同）；随后 `--check` 退出 0。
- 注意：`pnpm api:export` 不行——它的 cwd 是本仓，会生成**本仓**的产物。

## 通路 B · 开 change → 架构 tab（incremental）→ 契约 → 生成

- `change-detail-view.tsx:249-275`：写入 `detail.worktree`，不是主仓。
- 陷阱：worktree 从 `HEAD` 拉；主仓未提交的模型 / schema 改动不会进去（本次 archi 文件已 staged 未 commit）。
- 生成仍不删 161 孤儿。

## 两条都不可行的

- 让 agent 自己跑：没有命令可跑（这就是它报的那句话）。
- full 模式面板点生成：没有按钮（有意的只读裁定）。

---

# 三 · 方案选型：为什么必须是新工具

三个「看着更省」的形态都不可行：

| 形态 | 为什么不 |
|---|---|
| `archi_api` 加第 4 动作 `generate` | **档位是挂载项级、不是动作级**（`ToolMount { definition, tier, exec }`）。`archi_api` 一旦含写动作，整只工具升 `ToolTier::Write` ⇒ 在 `proposal` / `dev-design` / `implement` 的只读下发面被 `default` 档一并拒，连带 `check` / `bootstrap` / `link` 陪葬 |
| `agent-runtime` 直接依赖 `archi-runtime` | 撞 `desktop-crate-layout:325` 明文，且把 `orchestration` 传递边拖进 agent 运行时 |
| 壳层注册工具执行体 | 挂载清单是 `agent-runtime` 内的静态 `OnceLock`，无扩展点 |

**正解**：新挂载项 `archi_generate`（`ToolTier::Write` / `ToolExec::Archi`）+ `core/archi` 里的纯端口 trait。

`agent-runtime` **已依赖** `core/archi`（`crates/infra/agent/Cargo.toml:16`，DTO 投影单源），`GeneratedFile` / `ArchiApiWriteOutcome` 也是 core 类型 ⇒ 端口可落在 core：

```rust
// core/archi：纯类型，零写盘、零 spawn
pub trait ArtifactWriter: Send + Sync {
    fn write(&self, root: &Path, files: &[GeneratedFile])
        -> Result<ArchiApiWriteOutcome, String>;
}

// src-tauri：唯一实现，3 行委托
impl archi::ArtifactWriter for ArchiArtifactWriter {
    fn write(&self, root: &Path, files: &[archi::GeneratedFile])
        -> Result<ArchiApiWriteOutcome, String> {
        archi_runtime::api_artifacts::write_artifacts(root, files)   // 写盘单点不动
    }
}
```

⇒ **零新增 crate 依赖边**：`agent-runtime → archi`（既有）、`src-tauri → archi-runtime`（既有），写盘单点仍只 `infra/archi`。

---

# 四 · 方案定稿：archi_generate

## 4.1 数据流

```
src-tauri（组合根）  Arc<ArchiArtifactWriter>
  ▼
compose_turn(param)                        compose.rs:353
  └─ EngineFacade::with_artifact_writer    compose.rs:404-414（与 with_ask_port 同型）
       └─ SdkRunner::with_artifact_writer  runner.rs:99-190（builder 缝）
            └─ PumpPayload{ writer }       runner.rs:287
                 └─ LoopTurn{ archi }      loop.rs:122-160
                      └─ ArchiContext{ root, protect, writer }   archi_tool.rs:23-45
                           ▼
                    archi_generate 执行体
                      ├─ archi::workspace_render(root)      ← 纯读、零写
                      ├─ 逐份 protect.evaluate_target(...)   ← 命中即整批拒绝、零写盘
                      └─ writer.write(root, files)           → { written, unchanged }
```

`mint_archi_context`（`runner.rs:462-474`）判据由「七名任一」扩到「八名任一」；`with_protect` 旁加 `with_writer`（同型单点）。

## 4.2 注入矩阵（D1 + H1）

| (phase, role) | `archi_generate` | 备注 |
|---|---|---|
| `(proposal, executor)` | ✅ | 契约起草，首次生成 |
| `(dev-design, executor)` | ✅ | 契约修订 + 再生成（主战场） |
| `(test-design, executor)` | ❌ | 但**新增下发 `archi_module`**（H1，见 §8.3） |
| `(implement, executor)` | ❌ | 契约冻结，生成物恒新鲜（见 §5.2） |
| `archi` 来源臂 | ✅ | 通用工作区与 full 模式页内的唯一通路（F，见 §8.2） |
| 其余 | ❌ | — |

**红窗只对 Rust 成立、且被 worktree 关住**：

```
proposal               dev-design           test-design          implement
[E→V]                  [E→V]                [E→V]                [E→StaticCheck→ArchiCheck→V]
   ▲ 首次生成             ▲ 再生成                                 ▲ 实现补齐，红窗关闭
   └────────── 两相位零编译步（phase_table.rs:135-170）───────────┘
```

- `.i.rs` 是编译期见证：起草期生成 ⇒ 见证指向未实现函数 ⇒ worktree 内编译不过，直到 implement 补齐。
- proposal / dev-design 步序列就是 `Executor → Evaluator`，**没有任何门禁因此变红**；主仓不受影响。
- TS 侧无此现象（`.i.ts` 无消费者）。

## 4.3 新模块结构性零产出（必须写进工具描述）

`render_all` 对入口非 `Resolved` 的元素直接 `continue`：

```
proposal 给新模块定契约 → archi_generate → { written: [], unchanged: [] }
                                    ↑ 不是失败，是「入口还不存在」
```

起草期恰恰新模块最多 ⇒ 多数调用零产出；收益集中在「已有模块扩 op / 加类型」。工具描述不写这句，低档模型会以为工具坏了、转去手改。

## 4.4 写权不变量要收窄（最容易漏）

`desktop-archi-agent:347` 现文：

> 矩阵下发**写工具**的 `(phase, role)` 集合 SHALL 与 `model_write_lock` 判定为 `None` 的集合逐格相等

`archi_generate` 是写工具、但不写模型 ⇒ 必须显式收窄为「**模型**写工具（`archi_element` / `archi_relation`）」。否则对拍必红，而改测试凑合会把纪律改坏。

## 4.5 与既有裁决的三个「不冲突」

1. **画布只读**：`desktop-archi-canvas:151` 明写 `/architecture` 页的合法清理路径就是「让左侧会话执行本仓的生成命令」。本方案只是把「本仓命令」换成 app 自带能力，**没有新开人类可提交入口**。
2. **写权矩阵**：它约束模型写工具；`archi_generate` 不写模型，相位锁对它零射程（索引 / `.i.*` 本就不在锁面，`compose.rs:272-274` 有明文）。
3. **写盘单点**：`write_artifacts` 仍是唯一实现；端口只是「谁能调它」的缝。

---

# 五 · 契约作者纪律（D2 / D3）

## 5.1 责任矩阵

```
              模型 TOML      schema（接口）    md（理由）      代码
proposal       写（主）        写（次）         写（主）         —
dev-design     写（主）        写（主）         写（补修）       —
test-design    —              读               读              —
implement      ✗（锁面）       ✗（本裁决）       ✗（本裁决）      写（主）
```

语义 = **契约作者与契约实现者的分界**：起草期定承诺，实现期兑现承诺。

`apiMissing` / `apiSignatureMismatch`（唯二 `error` 档）的修复路径随之唯一化——**改代码追上契约**。以前最省事的修法是改 schema 迎合代码，那条路封了：`error` 档终于名副其实。

## 5.2 支点：生成物只取决于 schema

```
schema 变 → 生成物变旧
代码变   → 生成物不变        ← workspace_render 只读契约
```

implement 不改 schema ⇒ **implement 期生成物恒新鲜**（前提是起草期生成过）。D1 的矩阵选择由此自洽。

## 5.3 现状三处对立面（其中一处是锁面设计的成因）

| # | 位置 | 现文 | 性质 |
|---|---|---|---|
| 1 | `implementation-generator.md:36` | 「改了导出面就**同批同步** `<叶段>.schema.json`」 | prompt 直接冲突 |
| 2 | `dev-design-planner.md:45` | 「改完导出面就同批改接口定义**（实现相位同一条纪律）**」 | 括号把错纪律写进对的相位——**最直接的错误源** |
| 3 | `compose.rs:271-273` | 「契约资产 md 不锁（**`implement/executor` 持有 `archi_api`，改代码相位必须能同步契约**）」 | 锁面设计的**成因注释**，不改会让下一个人以为不锁是有意的 |

`proposal-planner.md:39` 干净，与新纪律兼容。

## 5.4 L3 落地清单

| # | 文件 | 改动 |
|---|---|---|
| 1 | `compose.rs::model_write_lock` | `Some` 分支加第二条规则：`glob = "**/{archi_models_rel_dir}/**"` |
| 2 | `protect/reason.rs` | 新增 `CONTRACT_LOCK_TEMPLATE` + 出口（**不能复用**模型那份，见 5.5①） |
| 3 | `compose_test.rs` | `模型写权挂锁格` 断言「恰一条」→「恰两条」；新增 implement 写 `models/**` 拒 / archi 臂放行 |
| 4 | `desktop-write-protection` spec | 锁面表 `models/**` ❌ → ✅ + 理由重写；Scenario「契约资产与生成物不锁」**整条反转** |
| 5 | `desktop-phase-prompts` spec | implement 接口纪律条款改写 |
| 6 | `desktop-archi-agent` / `desktop-archi-api-check` spec | 矩阵理由句；`error` 档语义升格为「实现未追上契约」 |

glob 形态与既有模型规则同构（`**/{rel_dir}/**`），`archi_models_rel_dir()` 已有派生，零新路径字面量。

## 5.5 三个会悄悄变形的地方

### ① 拒绝文案必须开第二份

现有 `SESSION_LOCK_TEMPLATE` 是模型专用（「架构模型只允许…」「元素与关系…」）。拿它锁 `models/**` 会三重误导。必须新增：

```
该路径受相位锁保护：%s（经 %t 工具）。
接口定义（.schema.json）与模块文档（.md）只在 proposal / dev-design 相位的执行侧
（以及架构页会话）写入；实现相位只实现契约、不改契约。
若代码与契约不符：先确认哪一方是对的——契约对就改代码追上它；
契约确实不可实现或写错，就停下来说明冲突，由用户决定走 backtrack 回到 dev-design 修订；
或请用户改用架构页会话直接维护。
不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。
```

最后一句不是修辞：`reason.rs` 模块注释已写明**被拒的模型会自然换路重试**（write → 重定向 → `cp` → `node -e`）。不给具体出路就是被拒进死循环。

### ② spec 里有一句在反驳自己

`desktop-write-protection` 现文：

> `models/**` ❌ —— 契约是**手写真相**，改代码相位必须能同步改契约——**锁了契约判据会永远飘**

新政策恰恰选择让它「飘」到 backtrack 为止。这句不能只翻成 ✅，要如实写出代价：**实现相位发现契约错的唯一出路是 backtrack，判据在 implement 内不再可自愈**。

### ③ `archi-knowledge.md` 帮不上忙

注入条件是 `model_write_lock == None`（`compose.rs:157`）——implement 挂锁 ⇒ 不注入知识。implement 的纪律只能靠自己的 prompt，`implementation-generator.md` 是唯一载体，没有兜底。

## 5.6 prompt 四处

| 文件 | 改动 |
|---|---|
| `implementation-generator.md:35-37` | 合并改写：不改 schema / md、被锁、两条出路（改代码追上契约 / 停下述冲突由用户 backtrack），删掉「既有命令」死指代 |
| `dev-design-planner.md:45` | 删「（实现相位同一条纪律）」 |
| `proposal-planner.md:39` | 加「本 change 涉及元素的 `<叶段>.md` 由本相位写（`archi_module` 的 `contractPath` 取路径，勿手拼）」 |
| `proposal-planner.md:36` / `dev-design-planner.md:42` | 若 D1 落地则补 `archi_generate` 一行 + 「入口不在盘时零产出正常」 |

---

# 六 · 落地清单与机械成本

## 6.1 archi_generate（D1）

| # | 文件 | 改动 |
|---|---|---|
| 1 | `core/archi/src/api/mod.rs` / `lib.rs` 出口 | `ArtifactWriter` trait |
| 2 | `infra/agent/src/sdk/mount.rs` | `ARCHI_GENERATE_TOOL` + 定义（无参数）+ `ToolTier::Write` + `ToolExec::Archi`；`with_capacity(18→19)`；**描述含三句硬要求**（见下方） |
| 3 | `infra/agent/src/sdk/archi_tool.rs` | `ArchiContext.writer` + `with_writer`；`execute` 第八臂；**check 分支加 `remedy`（D4）** |
| 4 | `infra/agent/src/sdk/runner.rs` | `SdkRunner` 字段 + builder；`PumpPayload` 字段；`mint_archi_context` 八名 |
| 5 | `infra/agent/src/lib.rs` | `EngineFacade` 字段 + builder + `runner_for` 透传 |
| 6 | `infra/agent/src/compose.rs` | `compose_turn` 增参；`archi_matrix`：两格加 `archi_generate` + **test-design 行加 `archi_module`（H1）** + archi 臂名单；doc 段 |
| 7 | `src-tauri/src/commands/{exec,change_flow}/mod.rs` | 构造 writer 并传入 |
| 8 | `src-tauri/src/commands/archi/mod.rs` | `ArchiArtifactWriter` 定义（委托） |
| 9-11 | `mount_test.rs` / `config_propose_test.rs` / `compose_test.rs` | 18→19、名字序、档位表、矩阵表（含 test-design 行）、写权对拍收窄 |
| 12 | 新增 `archi_tool` 测试 | 生成成功 / 无装配 / 保护拒 / 空模型 / 结构错 / **`remedy` 三态** |

**无需**改：bindings（工具响应是字符串，不进 IPC DTO）、`package.json`、CLI。

### `archi_generate` 工具描述的三句硬要求

前三句给「怎么用」，后两句给「别误解」与「别闯祸」——都是 §八 / §九 裁决的直接产物，MUST NOT 省略：

1. **一句用法**：按当前模型的契约渲染并写入三类生成物（`INDEX.toml` + 入口同级 `.i.rs` / `.i.ts`），幂等（字节相同即不重写、不动 mtime）；返回 `{ written, unchanged, conflicts }`。
2. **一句零产出解释**（§4.3）：入口还不在盘的新模块零产出是**正常**的，不是失败。
3. **一句孤儿常态**（§9.1）：生成器从不删除孤儿生成物，`orphaned` 非空属常态，MUST NOT 为了清空它去手改生成物。
4. **一句覆盖风险**（§9.2，硬要求）：生成会覆盖同名 `<入口主干>.i.*`——调用前先 `git status` 确认工作树干净，以便意外可 `git restore` 回滚。
5. **一句冲突处置**：`conflicts` 非空即被写保护拒绝，整批零写盘；把冲突报告给用户，MUST NOT 换工具重试。

## 6.2 追加四条（D4 / D5）

| # | 文件 | 改动 |
|---|---|---|
| 13 | `infra/agent/src/sdk/archi_tool.rs` | `remedy` 三态投影（空差异 `null` / 有工具指向工具 / 无工具指向架构页会话）——**已含在 6.1 第 3 行**，此处只标职责 |
| 14 | `src/views/architecture/components/dashboard/contract-panel.tsx:326-330` | full 提示去「本仓的生成命令」→「生成需开 change，或让左侧会话生成」 |
| 15 | `core/workflow/src/prompts/test-design-planner.md:40` | 补 `archi_module` 一句 + 「MUST NOT 用 grep 源码替代接口定义」边界句 |
| 16 | `src/views/architecture/components/dashboard/contract-panel.test.tsx` | 提示文案断言逐字改 |

## 6.3 spec 条款

| spec | 现在 | 改成 |
|---|---|---|
| `desktop-archi-agent` 档位节（:404） | 五只读 + 两写 | 五只读 + 三写；`archi_generate` 归写面 |
| 同 spec「零写盘」（:188） | 「MUST NOT 存在专用写工具」 | 保留三动作零写盘；改写禁令：生成物写工具 SHALL 存在，写盘经端口委托单点 |
| 同 spec 矩阵（:345-347） | 两写工具集 ⟺ 无模型锁 | 加 `archi_generate` 一列；**test-design 行补 `archi_module`（H1）**；不变量收窄为「**模型**写工具集 ⟺ 无模型锁」 |
| 同 spec 18 项（:439） | 18 | 19 |
| 同 spec `archi_api` 契约节 | 响应形状两轴 | 加 `remedy` 字段（工具响应层，非 DTO） |
| `desktop-write-protection` | 射程 + 锁面表 | 加 `archi_generate` 逐份 `evaluate_target`；`models/**` 入锁（D2） |
| `desktop-crate-layout` | `agent-runtime → archi`；`↛ archi-runtime` | 登记端口与「实现由壳层注入」；**两条依赖结论零改动** |
| `desktop-archi-api` | 写盘单点两驱动面 | 加第三驱动面（agent 工具），同调 `write_artifacts` |
| `desktop-archi-contract-panel` | 提示文案（:190 / :207） | 「本仓的生成命令」→「让左侧会话生成」；生成按钮断言不变 |
| `desktop-phase-prompts` | 工具引用纪律 | implement 纪律改写；**test-design 补 `archi_module`（H1）** |

## 6.4 计数

| 面 | 现 | 后 |
|---|---|---|
| `mount.rs` doc / `with_capacity` / `mount_test` / `config_propose_test` | 18 | 19 |
| archi 臂名单 / spec 计数 | 七 archi 名、5+2 | 八 archi 名、5+3 |
| test-design 矩阵行 | 1 名（`archi_search`） | 2 名（+`archi_module`） |

约 14 个产品 / 测试文件、6 份 spec、~10 处计数断言；**仍无 IPC / DTO / bindings / 脚本面改动**（`remedy` 走工具响应层）。

---

# 七 · 风险与留痕

| # | 风险 | 应对 |
|---|---|---|
| R1 | **契约定错时无出路**（最危险）。implement 发现契约不可实现只有三条路：(a) 硬实现 → `apiSignatureMismatch` 红 → 回到岔口；(b) backtrack 改 schema → 正确但重跑两相位；(c) 偷偷改 schema → 最省事。implement 是 `ModelLevel::Low`，会选 (c) | L3 硬闸挡 (c)；contract 专用拒串写明 (b) |
| R2 | **md 责任零判据**。`docMissing` 恒 `warn` 永不纳管；两份 evaluator checklist（proposal R1–R9 / dev-design D1–D10）零契约条目 | 先观察（§7.2） |
| R3 | **md 路径依赖 FQN**。proposal 期边界还在变，元素改名 md 要搬家 | prompt 钉死顺序：先 `archi_element` 定 FQN，再写 md（`contractPath` 取路径） |
| R4 | **契约定死了，test-design 却看不全**。矩阵只有 `archi_search`，拿不到 `properties` / `$defs` 键名 | **已解**（H1 纳入，§8.3） |
| R5 | **161 孤儿**：命名即身份，wpsweb 恒 161 条 | **已裁决：接受噪声**（B 不进本轮）。止损：`archi_generate` 描述写一句「孤儿永不删除，`orphaned` 非空属常态」（§9.1） |
| R6 | **覆盖手写 `.i.ts`** ⚠️ **唯一未消风险**：`write_artifacts` 只比字节不认身份，C 不做 ⇒ `archi_generate` 在 archi 臂 / 起草相位可直接覆盖同名手写文件 | **已裁决：本轮不做 C**。止损三条见 §7.1；其中「工具描述要求先 `git status`」升为硬要求（§9.2） |

## 7.1 R6 的实证与止血（C 不做，止损是唯一防线）

**实测已知地雷**（wpsweb）：

```
applications/spreadsheet/core/app/dbsheet/dbsheet_automation.ts
applications/spreadsheet/core/app/dbsheet/dbsheet_automation.i.ts   ← 模型里 Automation.entry 指向前者 ⇒ 生成落点正是后者
applications/spreadsheet/core/app/etview.ts + etview.i.ts           ← 同款并存
applications/public/services/script/execHandler/execHandler.ts + execHandler.i.ts
```

本次生成的结果与既有 `.i.ts` **逐字节相同**，所以「已经撞上但没炸」——那是因为那份 `.i.ts` 本来就是按同一份 schema 生成的。换成一份与 schema 不符的手写 `interface`，同一次 `generate` 就会把它抹掉。

**三条止损**（不改代码，全部进本轮交付）：

1. **工具描述硬要求**（§9.2）：`archi_generate` 的描述 SHALL 含「生成会覆盖同名 `<入口主干>.i.*`；调用前先 `git status` 确认工作树干净，以便意外可 `git restore`」。
2. **首轮在 worktree 里跑**：change 会话天然在 worktree，最坏随 change 丢弃；架构页会话直写主仓，风险面最大。写进 change 的风险节。
3. **触发条件**：若实际发生一次覆盖，B/C 立刻升为优先项（写进 change 的风险节，作为显式待办而非默记）。

**留档**：C（身份判据 ∧ 拒绝覆盖）是 `write_artifacts` 里的一个前置判断 + 一条测试，约半天；最坏后果是删掉用户手写的类型契约。本轮明确不做，**MUST NOT 在实现相位顺手加进去**——它牵动 `.i.*` 身份判据（B），而 B 已裁决为「接受噪声」，半做的 B 会让 `orphaned` 变成更难解释的混合语义。

## 7.2 checklist 缓议的观察指标

本仓现 `docMissing = 38`（35 declared + 3 none，分母 49）。若「proposal 写 md」有效，**新增元素**应自带 md，`docMissing` 增速应显著低于元素增速。跑一段后若「新 change 的 `docMissing` 只增不减」，说明 prompt 没被执行——那时再上 proposal R12 就有了证据。

## 7.3 有意不扩的边界

新锁只管 `models/**`。`INDEX.toml` 与 `.i.*` 仍可被 `write` / `edit` 手改（既有知情边界）。它与「不改契约」是两件事，混一起会把 L3 范围做糊。

---

# 八 · 追加纳入的四条（D4 / D5）

## 8.1 A · 提示面兜底（纳入）

### ① `archi_api check` 给机器可读的处置结论

**走工具响应层，不动 IPC DTO**——`archi_tool.rs` 的 check 分支本就是自己拼 `serde_json::json!({ "artifacts", "findings" })`，加一个顶层键不进 `ArchiApiCheck`，故 `bindings.ts` / `archi/mod.rs` / DTO 键集断言全部零改动：

```jsonc
{
  "artifacts": { "enabled": true, "mismatched": ["…"], "orphaned": [], "missing": [] },
  "findings": { … },
  "remedy": "生成物有差异：若本轮下发了 archi_generate 就直接调它；否则请用户在架构页会话生成，MUST NOT 手改"
}
```

`remedy` 的取值按三态给（纯字符串，零新词汇）：

| 条件 | 文案 |
|---|---|
| 三列全空 | `null`（无处置可给） |
| 有差异且本轮有 `archi_generate` | 指向工具本身 |
| 有差异且本轮无该工具（implement） | 指向架构页会话 + 「MUST NOT 手改」 |

**为什么不做成枚举/DTO**：`ArchiApiCheck` 是 specta 出线类型，加字段要连带改 bindings、前端、`mod_test` 的键集断言（`["enabled","meta","mismatched","orphaned","missing"]`）——为一个给模型看的字符串付这个代价不值。agent 面与人类面本就不同源（前者是文本，后者是结构化 DTO），这条差异在 `mount.rs` 已有先例。

### ② full 模式提示文案（与 F 合并，见 8.2）

## 8.2 F · 架构 agent 可以生成（画布仍不加按钮）

裁决的准确含义：**不推翻「画布只读 = 人类可提交入口」**，而是让 `desktop-archi-canvas:151` 早就写好的那条通路真正可用——

> `full` 模式下想清理过期生成物，其合法路径是让页内会话执行本仓的生成命令

以前这句是空头支票（会话没有生成工具，只能跑本仓命令，而通用工作区没有命令）。D1 的 `archi` 臂下发布 `archi_generate` ⇒ 会话有了。**画布面板的生成按钮仍然不加**（`full` 分支零 `archiApiGenerate` 的 spec 断言逐字不变）。

落点两处：

| 位置 | 现文 | 改成 |
|---|---|---|
| `contract-panel.tsx:326-330` | 「生成需开 change（或让左侧会话执行本仓的生成命令）」 | 「生成需开 change，或让左侧会话生成」（去掉「本仓的生成命令」这一死指代） |
| `desktop-archi-contract-panel/spec.md:190` + 同 spec 的 `:207` 场景 | 同上 | 同上；场景断言改为「零 `archiApiGenerate` 调用路径」（本来就是这样）+ 提示文案逐字 |

## 8.3 H1 · test-design 下发 `archi_module`（纳入）

**动因**：契约在 dev-design 定死之后，「对着接口协议设计测试」从可选优化变成纪律前提，而 test-design 的矩阵只有 `archi_search`（拿不到 `properties` / `$defs` 键名）。

**已核对无冲突**：`test-design-evaluator` 的 T1–T9 没有「只能从 design.md 推导公共 API」的判据（T5 只管章节实质、T9 只管 mock），所以新增第二来源不撞评估面。

矩阵一格：

```rust
("test-design", "executor") => Some(vec![
    mount::ARCHI_SEARCH_TOOL,
    mount::ARCHI_MODULE_TOOL,   // 新增
]),
```

prompt 改 `test-design-planner.md:40`（现只有 `archi_search` 一句），补：

```
- 额外可用只读工具 `archi_search`（发现）…；`archi_module`（定位）：取 `assets.interface` 的
  `ops` / `types` 键名摘要，用于核对 design.md 的「公共函数 / API」是否与接口定义一致。
  接口定义是手写真相，MUST NOT 用 grep 源码导出的方式替代它（过程 4b 的禁令不变）。
```

**关键边界**：过程 4b 那条「不得 grep 源文件导出声明或虚构条目」**逐字保留**——`assets.interface` 来自 schema（手写真相），不是源码扫描，两者不冲突；但 prompt 不写清这层区别，模型会把它读成「可以 grep 了」。

**留痕**：test-design 无知识 `preamble`（只在写权相位注入），agent 对 `archi_module` 的全部认知来自工具描述——而该描述已有「理解模块先调本工具 / 接口唯一凭据是 `.schema.json`」五句，够用。

## 8.4 H2 · `archi_api` 保留在 implement（纳入）

保留整只（`check` 只剩守门职责：验证起草期有没有漏生成）。理由：

- 拆动作不可行（§三：档位是挂载项级）；
- 不给则 implement 连「生成物新不新鲜」都看不见，而它恰恰是唯一会改写代码、可能间接触发判据变化的相位；
- 收益（少一格矩阵）抵不上一次扩面改动的成本。

**留痕**：`bootstrap` / `link` 在 implement 已无用武之地，但工具描述不为此分叉——多一句「本相位只用 check」会让描述变长且随相位漂移。

## 8.5 明确不做（本轮，已裁决/关闭）

| # | 项 | 裁决 | 不做的直接后果 |
|---|---|---|---|
| B | 身份判据（后缀 → 后缀 ∧ 头注释签名） | **接受噪声** | wpsweb 类仓的 `orphaned` 恒 161 条；确认框永远写「保留 161 份孤儿生成物不做处理」。**这不是缺陷，是这套契约面在通用 TS 仓的既定形态** |
| **C** | **覆盖保护** | **本轮不做** | **`archi_generate` 可覆盖同名手写 `.i.ts`（§七·R6，唯一未消风险）**；止损只靠 §7.1 的三条 |
| D | 配置声明生成命令 | 不做 | 通用工作区仍无法自报生成命令（由 D1 的工具 + D4 的 remedy 兜住） |
| E | 桌面端 headless CLI | 不做 | 同上 |
| G | TS 侧见证（`.i.ts` 无消费者） | **用户单独处理** | 归属已从本 change 移出，不再作为本轮的待办或风险项 |

## 8.6 两个默认值（按我倾向执行，如无异议）

1. **部分写**：工具面比 IPC 更严——任一份目标过不了 `ProtectRules` 即整批拒绝、零写盘（IPC 的「逐份写、失败点之前已写」语义不变）。
2. **响应形态**：结构化 `{ written, unchanged, conflicts }`，冲突进 `conflicts` 而非整条 `Err`——冲突要 agent 转告用户处置。

（C 不做 ⇒ 本轮 `conflicts` 只可能来自写保护，不可能来自「目标非生成物」。）

---

# 九 · 裁决关闭（本轮全部决策已定）

| # | 问题 | 裁决 | 影响 |
|---|---|---|---|
| 1 | 161 孤儿怎么处置 | **接受噪声** | B 永不进本轮；`orphaned` 语义、面板文案、生成器「从不删文件」全部零改动 |
| 2 | TS 侧 `.i.ts` 无消费者的引用问题 | **用户单独处理** | 从本 change 范围移出（既不是风险也不是待办）；本轮的 `.i.ts` 渲染/落点逻辑一行不改 |
| 3 | R6 覆盖手写 `.i.ts` 要不要提前（C） | **不要** | C 不进本轮；R6 转为**已知并接受的残余风险**，止损靠 §7.1 三条 |

**已答（前一轮）**：full 模式开不开主仓生成 → 架构 agent 可以使用（D5 / §8.2），画布按钮不加。

## 9.1 因裁决 1 而必须说清的一件事

「接受噪声」不等于「噪声可以不给说明」。`orphaned` 的语义（盘上有 `*.i.*` 但不在应生成集）在通用 TS 仓会恒非空，于是：

- **面板的二次确认文案不改**（「保留 M 份孤儿生成物不做处理」）—— 这是事实陈述，M 大并不使它变错；
- **工具响应的 `orphaned` 不改**（照实列）；
- 但 **`archi_generate` 的工具描述要写一句**「孤儿生成物永不删除，`orphaned` 非空属常态」——否则起草相位的 agent 看到 161 条会以为必须先清理才能生成，进而在 write/edit 上手改生成物。这一句是 D4 的同类（给人看的事实，成本为零）。

## 9.2 因裁决 3 而必须说清的一件事

C 不做 ⇒ `archi_generate` 是**唯一可能覆盖手写文件的工具**。§7.1 的三条止损里，第 ① 条（工具描述要求「先 `git status` 确认工作树干净」）由**建议**升为**硬要求**：

> `archi_generate` 的描述 SHALL 含一句：生成会覆盖同名 `<入口主干>.i.*`；调用前先 `git status` 确认工作树干净，以便意外可 `git restore` 回滚。

这条与「孤儿常态」那句同批写进 `mount.rs` 的工具描述，属 D1 的组成部分。

---

# 附 · 取证命令（可复跑）

```bash
# 另一工程只读比对（cwd 即观察根）
cd /d/Projects/wpsweb/client/app
<desktop>/src-tauri/target/debug/archi-api-gen.exe --check
# → {"mismatched":[".dev-team/architecture/graph/INDEX.toml"],"orphaned":[161 条],"missing":[]}

# 副本上生成 + 看 INDEX 差异内容
cp -r <other>/.dev-team /tmp/probe/ && cd /tmp/probe && … generate
diff -u <other>/.dev-team/architecture/graph/INDEX.toml /tmp/probe/.dev-team/architecture/graph/INDEX.toml

# legacy .i.ts 规模
cd /d/Projects/wpsweb && git ls-files '*.i.ts' | wc -l    # 162
```
