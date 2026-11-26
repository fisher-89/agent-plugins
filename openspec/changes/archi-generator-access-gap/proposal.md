# 提案: archi-generator-access-gap

> **变更**: archi-generator-access-gap（架构生成物的「能力在、通路不在」· 通路补齐 + 契约作者纪律）
> **日期**: 2026-11-26
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（§一 现场 / §二 两条现通路的实测 / §三 方案选型 / §四 D1 定稿 / §五 D2+D3 纪律 / §六 清单 / §七 风险 / §八 D4+D5 / §九 裁决关闭）。
> **性质**: **一条新通路 + 一条纪律收窄的原子交付**——给 agent 面加 `archi_generate`（写生成物，不写模型），把 `models/**` 收进相位锁，并把「生成物过期」的悬空指代全部换成可执行的工具引用。
> **不含**: B 身份判据（接受噪声）、C 覆盖保护（已知并接受的残余风险）、D 配置声明命令、E 桌面端 headless CLI、G TS 侧 `.i.ts` 见证（已移出本 change，用户单独处理）。

---

## 问题

### 一 · 生成能力与根无关，但 agent 一条通路都够不着

桌面端给另一个工作区（`D:\Projects\wpsweb\client\app`，git 根 `D:\Projects\wpsweb`）建好架构域后，agent 只能答「`INDEX.toml` 仍 mismatched，需要你在有生成器的环境跑一次」。这句话**准确**：agent 确实没手改生成物，也确实没有生成通路。

能力本身早已与观察根无关（`archi::workspace_render` + `archi_runtime::api_artifacts::write_artifacts` 纯按 cwd 锚定），app 自己就能给任何工作区生成。缺的不是能力，是**让 agent 够得着的通路**：

```
CLI  archi-api-gen            ✅ 能写任意工作区，但要求「本仓有这条命令」
IPC  archi_api_generate       ✅ 能写（in-process），但只有 incremental 模式的契约面板按钮调它
agent archi_api               ❌ 三动作全只读（check / bootstrap / link）
agent bash                    ⚠️ 取决于「本仓有没有生成命令」← 断在这
full 模式契约面板              ❌ 无按钮（有意为之）
```

断点不在实现，在三处**明文边界**：`desktop-archi-agent`「`archi_api` 三动作零写盘 + MUST NOT 存在『写生成物』的专用工具」；`desktop-crate-layout`「`agent-runtime` 的依赖中不出现 `archi-runtime`」；`desktop-archi-contract-panel` / `desktop-archi-canvas`「生成入口只属 change 模式」。

### 二 · 三个「只在本仓成立」的假设

| # | 假设 | 现实 |
|---|---|---|
| 1 | 每个工作区自带生成命令 | 只有本仓有（`api:export`）。产品侧明文「命令名归属各仓文档面」⇒ 通用工作区里「用既有命令重生成」是**悬空指代**（知识骨架与两份 prompt 三处都在这么写） |
| 2 | `*.i.ts` / `*.i.rs` 是生成物专属命名 | 命名即身份：wpsweb 全树 162 份 `*.i.ts`（皆为更早的手写 interface 文件、被源码 import）⇒ `orphaned` 恒 161 条、生成确认框永远写「保留 161 份孤儿生成物不做处理」 |
| 3 | `.i.ts` 是编译期见证 | 本仓 `.i.rs` 经 `#[path]` 编进 crate，漂移即编译错；`.i.ts` 在本仓零实例，wpsweb 里生成的 `.i.ts` 无消费者 |

假设 2 / 3 的处置已裁决（接受噪声 / 用户单独处理），但它们决定了两条**必须写进工具描述**的事实：孤儿永不删除是常态；生成会覆盖同名手写 `<入口主干>.i.*`。

### 三 · 契约纪律在实现相位自相矛盾

`apiMissing` / `apiSignatureMismatch` 是唯二 `error` 档判据（能阻断归档）。而实现相位今天**持有 `archi_api` 且能改 `.schema.json`**，于是最省事的修法恒为「改契约迎合代码」——`error` 档因此名不副实。三处文本把这条错纪律写成了正面表述：`implementation-generator.md`「改了导出面就同批同步 schema」、`dev-design-planner.md`「（实现相位同一条纪律）」、`compose.rs::model_write_lock` 的成因注释「改代码相位必须能同步契约」。`desktop-write-protection` 的锁面表还给 `models/**` 标了 ❌，理由正是「锁了契约判据会永远飘」。

### 四 · 提示面在别的工作区是假话

`contract-panel.tsx` 的 full 模式提示写「生成需开 change（或让左侧会话执行本仓的生成命令）」——在 wpsweb 里两个分句都不成立（没开过 change，也没有生成命令）。同时 `archi_api(check)` 只回两轴事实、**没有任何「下一步怎么修」的字段**，模型拿到 `mismatched` 只能自己猜。

### 五 · 契约定死之后，test-design 看不全契约

`test-design` 的矩阵只有 `archi_search`（发现面），拿不到 `assets.interface` 的 `ops` / `types` 键名。「对着接口协议设计测试」本应是纪律前提，现在只能退化成「从 design.md 转述」。

---

## 提案

### 一 · 新工具 `archi_generate`（D1）

```
archi_generate   无参数                        ToolTier::Write   ToolExec::Archi
```

**执行链**（三步，任一步失败即零写盘）：

```
archi::workspace_render(root)                 ← 纯读、零写盘、零 spawn（既有单点）
  → 逐份 ProtectRules 判定（目标文件）         ← 命中即整批拒绝，记入 conflicts
    → ArtifactWriter::write(root, files)        ← 壳层注入的实现，委托 write_artifacts 单点
      → { written, unchanged, conflicts }
```

- **响应形态**：`{ written: string[], unchanged: string[], conflicts: [{ path, reason }] }`；`conflicts` 非空 ⇒ `written` / `unchanged` 皆空（整批零写盘）、工具仍返回 `Ok`（冲突要 agent 转告用户处置，不是 run 的错误）。该形状在**工具响应层**拼装，`ArchiApiWriteOutcome` / IPC DTO / bindings 零改动。
- **幂等**：字节相同即计入 `unchanged` 且不重写（mtime 不动）——由 `write_artifacts` 既有比对承担。
- **零产出是正常**：`render_all` 对「`path` 空 / 无 `.schema.json` / 接口面留白 / 入口未解析」的元素一律 `continue`；起草期新模块最多 ⇒ 零产出常见，不是工具坏了。
- **档位**：`ToolTier::Write`（`default` 拒、`acceptEdits` / `bypassPermissions` 放行，与 `write` / `edit` 同档），准入判据仍是 tier 比较单点。
- **工具描述五句硬要求**（MUST NOT 省略任意一句）：① 用法与幂等 + 响应三字段；② 零产出正常；③ 孤儿永不删除、`orphaned` 非空属常态、MUST NOT 为清空它手改生成物；④ 「生成会覆盖同名 `<入口主干>.i.*`；调用前先 `git status` 确认工作树干净，以便意外可 `git restore` 回滚」；⑤ `conflicts` 非空即整批零写盘，报告给用户、MUST NOT 换工具重试。

**工具落 `ToolExec::Archi`**：它复用 per-turn `ArchiContext.root`（change 会话 = worktree，架构页 = 主仓），故「写哪个根」与其它 archi 工具同源，MUST NOT 另立 root 参数。

#### 决策 1 · 为什么是新工具，不是 `archi_api` 的第 4 动作

| 备选 | 否决理由 |
|---|---|
| **`archi_api` 加 `action = "generate"`** | 档位是**挂载项级**（`ToolMount { definition, tier, exec }`）。`archi_api` 一旦含写动作，整只工具升 `ToolTier::Write` ⇒ 在只读下发的相位（`dev-design/evaluator` / `code-review` / `acceptance`，以及 `proposal` / `dev-design` 的只读档）被 `default` 档一并拒，`check` / `bootstrap` / `link` 陪葬 |
| **`agent-runtime` 直接依赖 `archi-runtime`** | 撞 `desktop-crate-layout`「`agent-runtime` 的依赖中不出现 `archi-runtime`」（写盘单点对 agent 面不可达），并把 `orchestration` 传递边拖进 agent 运行时 |
| **壳层注册工具执行体** | 挂载清单是 `agent-runtime` 内的静态 `OnceLock`，无扩展点 |

### 二 · 端口在 core，实现在壳（零新增依赖边）

`agent-runtime` 已依赖 `core/archi`（DTO 投影单源），`GeneratedFile` / `ArchiApiWriteOutcome` 也是 core 类型 ⇒ 端口可落在 core：

```rust
// crates/core/archi：纯类型，零写盘、零 spawn、零 IO
pub trait ArtifactWriter: Send + Sync {
    fn write(&self, root: &Path, files: &[GeneratedFile])
        -> Result<ArchiApiWriteOutcome, String>;
}

// src-tauri：唯一实现，薄委托
impl archi::ArtifactWriter for ArchiArtifactWriter {
    fn write(&self, root: &Path, files: &[archi::GeneratedFile])
        -> Result<archi::ArchiApiWriteOutcome, String> {
        archi_runtime::api_artifacts::write_artifacts(root, files)   // 写盘单点不动
    }
}
```

依赖结论两条**逐字不变**：`agent-runtime → archi`（既有）、`agent-runtime ↛ archi-runtime`；`src-tauri → archi-runtime`（既有）。写盘单点仍只 `infra/archi` 的 `write_artifacts`。

**注入点**（与 `with_ask_port` 同型的一份单点）：`src-tauri` 组合根 → `compose_turn(param)` → `EngineFacade::with_artifact_writer` → `SdkRunner` builder → `PumpPayload` → `LoopTurn` → `ArchiContext.writer`。`mint_archi_context` 的铸造判据由「七名任一」扩为「八名任一」（含 `archi_generate`）；上下文缺席 writer（生产不可达）⇒ `is_error` 回灌。

#### 决策 2 · 端口 vs 另立 crate vs 让 infra 自己实现

| 备选 | 否决理由 |
|---|---|
| 端口落 `core/archi` + 壳层注入（本方案） | 零新依赖边；写盘单点仍是唯一实现；`core/archi` 保持零 IO |
| 在 `infra/agent` 里再造一份「渲染 + 写盘」 | 第二份写盘实现，直接违约 `desktop-archi-api`「写盘有唯一实现」 |
| 把 `write_artifacts` 上移到 `core/archi` | `core/archi` 全程零写盘是既有硬纪律（判据面与写盘面分离） |

### 三 · 注入矩阵（D1 + H1）

| (phase, role) | `archi_generate` | 备注 |
|---|---|---|
| `(proposal, executor)` | ✅ | 契约起草，首次生成 |
| `(dev-design, executor)` | ✅ | 契约修订 + 再生成（主战场） |
| `(test-design, executor)` | ❌ | 但**新增下发 `archi_module`**（H1） |
| `(implement, executor)` | ❌ | 契约冻结，生成物恒新鲜（见 §四） |
| `archi` 来源臂 | ✅ | 通用工作区与 full 模式页内会话的唯一通路（D5） |
| 其余 | ❌ | — |

- **archi 臂**：七 archi 名 → **八**名（+ `archi_generate`）；`preamble` / `before_done` / `ask` 三位逐字不变。
- **test-design 行**：`[archi_search]` → `[archi_search, archi_module]`。理由：契约在 dev-design 定死之后，「对着接口协议设计测试」需要 `assets.interface` 的键名；`test-design` 无知识 preamble，认知全来自工具描述（既有描述已够）。
- **写权不变量收窄（最容易漏）**：`desktop-archi-agent` 现文「矩阵下发**写工具**的 `(phase, role)` 集合 SHALL 与 `model_write_lock` 判定为 `None` 的集合逐格相等」——`archi_generate` 是写工具、但不写模型，故该不变量 SHALL 显式收窄为「**模型**写工具（`archi_element` / `archi_relation`）」。不收窄则对拍必红，而改测试凑合会把纪律改坏。

**红窗只对 Rust 成立、且被 worktree 关住**：`.i.rs` 是编译期见证，起草期生成 ⇒ 见证指向未实现函数 ⇒ worktree 内编译不过，直到 implement 补齐。`proposal` / `dev-design` 的步序列就是 `Executor → Evaluator`（`phase_table.rs`），**没有任何门禁因此变红**；主仓不受影响（change 会话根 = worktree）。TS 侧无此现象（`.i.ts` 无消费者）。

#### 决策 3 · 为什么 implement 不给生成（而不是「先给了再观察」）

implement 的步序列含 `StaticCheck` + `ArchiCheck`，生成只会让「代码变了 ⇒ 生成物旧了」的假象进入门禁；而 `workspace_render` **只读契约**——代码变化不影响生成物 ⇒ 只要起草期生成过，implement 期生成物恒新鲜。「不给」不是妥协，是结论。

### 四 · 契约作者纪律（D2 + D3）

```
              模型 TOML      schema（接口）    md（理由）      代码
proposal       写（主）        写（次）         写（主）         —
dev-design     写（主）        写（主）         写（补修）       —
test-design    —              读               读              —
implement      ✗（既有锁面）   ✗（本裁决）       ✗（本裁决）      写（主）
```

语义 = **契约作者与契约实现者的分界**：起草期定承诺，实现期兑现承诺。支点是「生成物只取决于 schema」（`workspace_render` 只读契约）——implement 不改 schema ⇒ 生成物恒新鲜。

**L3 落地 = 相位锁的锁面表加一行**（`compose::model_write_lock` 的 `Some` 分支加第二条规则 `**/{archi_models_rel_dir}/**`），不新造机制：

- **拒绝文案必须开第二份**：现有 `SESSION_LOCK_TEMPLATE` 是模型专用（「架构模型只允许…」「元素与关系…」），拿它锁 `models/**` 会三重误导。新增 `CONTRACT_LOCK_TEMPLATE`（四要素：受相位锁保护并携路径与 `%t` / 契约只在起草相位执行侧与架构页会话写入 / 三条出路 / 不要换工具绕过），三条出路写死为「契约对就改代码追上它 → 契约不可实现就停下述冲突由用户 backtrack → 或请用户改用架构页会话」。最后一句不是修辞：`reason.rs` 已记明被拒的模型会自然换路重试（write → 重定向 → `cp` → `node -e`）。
- **spec 里那句反驳自己的话要如实翻**：`desktop-write-protection` 现文「`models/**` ❌——锁了契约判据会永远飘」。新政策恰恰选择让它「飘」到 backtrack 为止，故改写后 MUST 写出代价：**实现相位发现契约错的唯一出路是 backtrack，判据在 implement 内不再可自愈**。
- **`archi-knowledge.md` 帮不上忙**：注入条件是 `model_write_lock == None`，implement 挂锁 ⇒ 不注入知识。implement 的纪律只能靠自己的 prompt（`implementation-generator.md` 是唯一载体，没有兜底）。

**prompt 四处**：

| 文件 | 改动 |
|---|---|
| `implementation-generator.md` | 合并改写：不改 schema / md、被相位锁、两条出路（改代码追上契约 / 停下述冲突由用户 backtrack），删掉「既有命令」死指代 |
| `dev-design-planner.md` | 删「（实现相位同一条纪律）」；补 `archi_generate` 一行 + 「入口不在盘时零产出正常」 |
| `proposal-planner.md` | 补 `archi_generate` 一行 + 本 change 元素的 `<叶段>.md` 由本相位写（`archi_module` 的 `contractPath` 取路径，勿手拼） |
| `test-design-planner.md` | 补 `archi_module` 一句 + 「MUST NOT 用 grep 源码导出的方式替代接口定义」边界句（过程 4b 的禁令逐字保留） |

#### 决策 4 · L3 硬闸 vs checklist 判据

| 备选 | 否决/推迟理由 |
|---|---|
| **L3 相位锁**（本方案） | 结构上不可违反，与既有「矩阵集 ⟺ 不挂锁集合」同一机制，零新词汇 |
| proposal R12 / dev-design D11 两条 checklist 判据 | 判据只判产物、判不到「谁改的」，且要先观察「proposal 写 md」是否被 prompt 执行到位（`docMissing` 恒 `warn`、永不纳管）；本轮先观察，指标见风险 R2 |
| 只改 prompt 不加锁 | `implement` 是低档模型，会走最省事的路（改 schema 迎合代码）；仅靠 prompt 等于把纪律交给概率 |

### 五 · 提示面兜底（D4 + D5）

- **`archi_api(check)` 增机器可读处置结论 `remedy`**（工具响应层，不进 IPC DTO）：三态——三列全空 ⇒ `null`；有差异且本轮有 `archi_generate` ⇒ 指向工具本身；有差异且本轮无该工具（implement）⇒ 指向架构页会话 + 「MUST NOT 手改」。不做成枚举 / DTO 的理由：`ArchiApiCheck` 是 specta 出线类型，加字段要连带改 bindings、前端与 `mod_test` 的键集断言，而 agent 面与人类面本就不同源（前者文本、后者结构化）。
- **full 模式提示文案去死指代**：「生成需开 change（或让左侧会话执行本仓的生成命令）」→「生成需开 change，或让左侧会话生成」。**画布不加生成按钮**：`full` 分支零 `archiApiGenerate` 调用路径的既有断言逐字不变；`desktop-archi-canvas` 早就写好的「合法路径是让页内会话执行生成」从此不再是空头支票。
- **知识骨架换指**：`archi-knowledge.md` 的「生成物过期用既有命令重生成」→ 指向 `archi_generate`。不换则「工具在册」与「知识仍指悬空命令」同时成立，模型会两难。

#### 决策 5 · `remedy` 走工具响应层，不走 DTO

| 备选 | 否决理由 |
|---|---|
| 工具响应加顶层 `remedy`（本方案） | 零 bindings / 零前端 / 零 DTO 键集断言改动；agent 面本就是文本 |
| 给 `ArchiApiCheck` 加 `remedy` 字段 | specta 出线 ⇒ 改 bindings.ts / bindings.zod.ts / 前端 / `mod_test` 键集断言，为一个给模型看的字符串付三处代价 |
| 不改，只靠工具描述提示 | 描述是静态的，判不出「本轮有没有下发 `archi_generate`」 |

### 六 · 与 explore 的偏离（逐条）

| # | explore | 本提案 | 理由 |
|---|---|---|---|
| ① | 「零产出 = 入口还不存在」（§4.3） | 零产出**三成因**：无 `.schema.json` / 接口面留白 / 入口未解析 | `render_all` 三处 `continue` 都在；只写「入口」会让 agent 在「schema 还没写」时误判工具坏了 |
| ② | §6.1 只列工具五项描述三句 | 描述五句（用法 / 零产出 / 孤儿 / 覆盖风险 + `git status` / 冲突处置） | explore §9.1 / §9.2 已把「孤儿常态」与「先 `git status`」升为硬要求，两处必须与用法同批 |
| ③ | §6.1 未涉及 `archi-knowledge.md` | 纳入（单句换指 + 体量重冻） | archi 臂拿到工具而知识仍指悬空命令，是同一事实两个相反指引 |
| ④ | §6.3 只列 `desktop-archi-contract-panel` 的文案 | 同时改 `desktop-archi-canvas` | 同一句 UI 文案在两个 spec 各有引述，只改一处即两处互相矛盾 |
| ⑤ | §5.1 责任矩阵未提「模型写面」 | 明确 proposal / dev-design 仍可手改模型（锁面只加 `models/**`） | `INDEX.toml` 与 `.i.*` 仍可被 `write` / `edit` 手改（既有知情边界）；与「不改契约」是两件事，混一起会把 L3 范围做糊 |
| ⑥ | §8.1 只给 `remedy` 三态文案 | 另加一条纪律：`remedy` 字段 MUST NOT 进 DTO / bindings | 防下一个人「顺手补类型」把零改动面做成三处改动 |

### 七 · 明确不做（本轮）

| 项 | 裁决 | 直接后果 |
|---|---|---|
| B 身份判据 | 接受噪声 | wpsweb 类仓 `orphaned` 恒 161 条；确认框恒写「保留 161 份孤儿生成物不做处理」——这是契约面在通用 TS 仓的既定形态，不是缺陷 |
| **C 覆盖保护** | 本轮不做 | `archi_generate` 可覆盖同名手写 `.i.ts`（风险 R4，唯一未消风险）；止损只靠三处（工具描述硬要求 / 首轮在 worktree 跑 / 触发即升优先） |
| D 配置声明生成命令 | 不做 | 通用工作区不自报生成命令，由 `archi_generate` + `remedy` 兜住 |
| E 桌面端 headless CLI | 不做 | 同上 |
| G TS 侧 `.i.ts` 见证 | 用户单独处理 | 归属已移出本 change；`.i.ts` 渲染与落点逻辑一行不改 |

---

## 能力

### 新增能力

无。生成工具归 `desktop-archi-agent`（工具面 / 档位 / 矩阵 / 知识是一套机制，拆出去会让矩阵变成两份持有者）；端口归 `desktop-crate-layout`（依赖与边界登记单份持有）。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-agent` | **RENAMED ×1 + ADDED ×1 + MODIFIED ×5** | ① ADDED：`archi_generate` 工具契约（执行链 / 响应三字段 / 零产出三成因 / 描述五句 / 档位 / 逐份保护 / 端口委托）。② RENAMED：档位 requirement 由「五只读 + 两写」改「五只读 + 三写」后 MODIFIED（19 项、三写工具、铸造判据八名）。③ MODIFIED：注入矩阵（新列 + test-design 行 + archi 臂八名 + 写权不变量收窄）；`archi_api`（零写盘条款改写 + `check` 增 `remedy` + 提示面换指）；`模型 = 源码`（生成物有专用写工具、`models/**` 在非写权相位被相位锁、禁 `MUST NOT 存在专用写工具` 的旧句）；系统知识注入（单句换指 + 体量重冻）。 |
| `desktop-write-protection` | **MODIFIED ×1** | 相位锁：政策与锁面表加 `models/**`（理由如实写代价）、`archi_generate` 逐份判定的射程、`CONTRACT_LOCK_TEMPLATE` 第二份拒串；Scenario「契约资产与生成物不锁」整条反转。Module Contract 增一段落位登记。 |
| `desktop-crate-layout` | **MODIFIED ×1** | `core/archi` 落地登记补 `ArtifactWriter` 端口（纯类型、零实现、零 IO）与「实现在壳层注入」；两条既有依赖结论（`agent-runtime ↛ archi-runtime`、`app → archi-runtime`）逐字不变。 |
| `desktop-archi-api` | **MODIFIED ×1** | 「写盘单点」由两驱动面（两 IPC / 两 CLI）扩为**三驱动面**（+ agent 工具 `archi_generate`），渲染与比对仍单点、写盘仍 `write_artifacts` 唯一实现。 |
| `desktop-archi-api-check` | **ADDED ×1** | 契约优先的修复路径唯一化：`error` 档（`apiMissing` / `apiSignatureMismatch`）在实现相位的修复路径只剩「改代码追上契约」或「停下由用户 backtrack」，改 schema 迎合代码不再可行。 |
| `desktop-archi-contract-panel` | **MODIFIED ×1** | full 模式提示文案去「本仓的生成命令」；场景断言按新串逐字改，`full` 分支零 `archiApiGenerate` 的断言不变。 |
| `desktop-archi-canvas` | **MODIFIED ×1** | 同一句提示文案与「合法路径 = 让页内会话生成」的表述随动；**画布仍零生成入口**（既有只读断言逐字不变）。 |
| `desktop-phase-prompts` | **MODIFIED ×1** | 工具引用纪律：表格加 `archi_generate`（两 planner）、`test-design-planner` 加 `archi_module`；`implementation-generator` 的接口同步句**反转**为「不改契约、被锁、两条出路」；写权段删「（实现相位同一条）」；Scenario 断言随动。 |
| `desktop-agent-execution` | **MODIFIED ×1** | 会话级工具面注入：archi 臂七名 → 八名、tier 归属条加 `archi_generate`（写面）、change 臂 Scenario 的名单计数（16 → 17 名）与 `archi_generate` 不计入矩阵对拍。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-model` | 生成物写盘不碰模型写回原语：`plan_*` / 自证链 / `write_atomic` 落位逐字不变 |
| `desktop-archi-drift` | 判据、五值 kind、`modelHash` 口径零变化；本变更只新增一个「从 agent 调生成」的驱动面 |
| `desktop-archi-chat` | 架构页会话的工具面由 `desktop-archi-agent` 与 `desktop-agent-execution` 持有；该能力写权与刷新纪律零变化（会话已能生成，正是既有条款的兑现） |
| `desktop-archi-gate` | 门禁输入与 `passed` 判据零变化 |
| `desktop-archi-file-tree` | 与本变更无交集 |
| `desktop-ipc-type-bindings` | 命令清单与出线类型清单零变化；`remedy` 走工具响应层 ⇒ `bindings.ts` / `bindings.zod.ts` 零 diff |
| `desktop-config-agent` / `desktop-workspace-config` | 配置写面零变化 |
| `repo-archi-model` | 模型拓扑零 delta：`AgentRuntime → Core.Archi`、`Shell → Infra.ArchiRuntime` 两条边已在案，无需新增元素或关系；六个 `NN-*.toml` 表头计数与 `INDEX.toml` 逐字不变 |
| 前端各能力（画布 / 面板 / 路由） | 除 `contract-panel.tsx` 一行提示文案外零变化 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | 新增端口 `pub trait ArtifactWriter: Send + Sync`（`write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`）；纯类型，零 IO、零写盘、零 spawn |
| `.../crates/core/archi/src/lib.rs` | 出口随动（`ArtifactWriter` 再导出） |
| `.../crates/infra/agent/src/sdk/mount.rs` | 常量 `ARCHI_GENERATE_TOOL = "archi_generate"` + 无参定义 + 五句描述 + `ToolTier::Write` / `ToolExec::Archi`；静态清单 18 → **19** 项（插在两 archi 写之后、两 DB 之前）；`Vec::with_capacity(18)` → `19` 与两段 doc 的「十八项」措辞同批随动；`ToolTier` / `ToolExec` 的 doc 名单同步 |
| `.../crates/infra/agent/src/sdk/archi_tool.rs` | `ArchiContext` 增 `writer: Option<Arc<dyn archi::ArtifactWriter>>` + `with_writer`（与 `with_protect` 同型）；`execute` 第八臂（`workspace_render` → 逐份 `ProtectRules` → `writer.write` → 三字段响应）；writer 缺席 ⇒ `is_error`；`archi_api` 的 `check` 分支加 `remedy` 三态 |
| `.../crates/infra/agent/src/sdk/runner.rs` | `SdkRunner` 字段 + builder 缝（`with_artifact_writer`）；`PumpPayload` 字段；`mint_archi_context` 判据七名 → **八名** |
| `.../crates/infra/agent/src/lib.rs` | `EngineFacade` 字段 + builder + `runner_for` 透传 |
| `.../crates/infra/agent/src/sdk/mod.rs` | 执行体模块头注释「五只读 + 两写」→「五只读 + 三写（两模型写 + 一生成物写）」（措辞随动，零逻辑） |
| `.../crates/infra/agent/src/compose.rs` | `compose_turn` 增参数（writer 透传）；`archi_matrix` 两格加 `ARCHI_GENERATE_TOOL`、`("test-design", "executor")` 加 `ARCHI_MODULE_TOOL`、archi 臂名单七 → 八；`model_write_lock` 的 `Some` 分支加第二条规则（`glob = "**/{archi_models_rel_dir}/**"`、`reason = contract_lock_template()`）；doc 段与「锁面只 `graph/model/**`」的成因注释整段改写 |
| `.../crates/infra/agent/src/sdk/protect/reason.rs` | 新增 `CONTRACT_LOCK_TEMPLATE` + `contract_lock_template()` 出口（MUST NOT 复用 `SESSION_LOCK_TEMPLATE`）；既有三类串逐字不变 |
| `.../crates/infra/agent/src/archi-knowledge.md` | 「生成物过期用既有命令重生成」→ 指向 `archi_generate`（单句换指，门限 1831 / 2031 不抬） |
| `.../crates/infra/agent/src/archi_knowledge.rs` | 模块头注释的门限表述随实测值更新（拼装面与降级语义不变） |
| `.../src-tauri/src/commands/archi/mod.rs` | `ArchiArtifactWriter`（唯一实现，薄委托 `archi_runtime::api_artifacts::write_artifacts`；零自有写盘逻辑） |
| `.../src-tauri/src/commands/exec/mod.rs` / `.../change_flow/mod.rs` | `compose_turn` 调用点构造 `Arc<ArchiArtifactWriter>` 并传入（两处，同型） |
| `.../crates/core/workflow/src/prompts/implementation-generator.md` | 接口同步句反转：不改 schema / md、受相位锁、两条出路（改代码追上契约 / 停下述冲突由用户 backtrack）；删「既有命令」死指代 |
| `.../crates/core/workflow/src/prompts/dev-design-planner.md` | 删「（实现相位同一条纪律）」；补 `archi_generate` 一行 + 零产出正常一句 |
| `.../crates/core/workflow/src/prompts/proposal-planner.md` | 补 `archi_generate` 一行 + 本 change 元素的 `<叶段>.md` 由本相位写（路径经 `archi_module` 的 `contractPath` 取） |
| `.../crates/core/workflow/src/prompts/test-design-planner.md` | 补 `archi_module` 用法一句 + 「MUST NOT 用 grep 源码导出替代接口定义」边界句（4b 禁令逐字保留） |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` | full 模式提示文案：「生成需开 change，或让左侧会话生成」 |
| `packages/desktop/package.json` | `version` patch +1（相对落地时 `HEAD` 现值，0.7.20 → 0.7.21） |

### 测试文件

| 文件 | 改动 |
|---|---|
| `.../crates/infra/agent/src/sdk/mount_test.rs` | `EXPECTED_NAMES: [&str; 18]` → `19` 且 `archi_generate` 按 `registry()` 实现序插入（两 archi 写之后）；全部含「十八项」字面的断言串与测试名逐处改「十九项」（`names.len()` / `unique.len()` / 源码结构锚的三处字面串 / `ask_user_question` 措辞）；新增：`archi_generate` 的档位（`Write`）、无参定义（`parameters` 零属性）、描述五句逐句在场（含「孤儿」「`git status`」「零产出」） |
| `.../crates/infra/agent/src/sdk/policy_test.rs` | 封闭集 `match name` 的写档臂加 `"archi_generate"`（与 `write` / `edit` / 两写工具同臂 `[false, true, true]`）；两处 `names.len()` / `unique.len()` 计数 18 → 19；三个含「十八项」的测试名随动 |
| `.../crates/infra/agent/src/sdk/config_propose_test.rs` | `names.len(), 18` → `19`（断言串随动）；「末项为提案工具」逐字不变（`archi_generate` MUST NOT 落在末位） |
| `.../crates/infra/agent/src/sdk/archi_tool_test.rs` | 第八臂：生成成功（`written` / `unchanged` 与盘上字节、`mtime` 对拍）、幂等二次调用 `unchanged` 全量、零产出（无 schema / 接口留白 / 入口未解析三例）、保护拒（`conflicts` 非空且零写盘）、writer 缺席 ⇒ `is_error`、坏模型 ⇒ 结构化诊断；`check` 的 `remedy` 三态各一例 |
| `.../crates/infra/agent/src/compose_test.rs` | 矩阵八行逐行命中（含 test-design 两名、implement 零 `archi_generate`）；`矩阵与写权锁同集` 断言收窄为「两**模型**写工具 ⟺ 不挂锁」且 `archi_generate` 不在对拍集；`模型写权挂锁格` 由「恰一条」改「恰两条」；新增 implement 写 `models/**` 拒 / archi 臂放行 / 契约拒串取 `contract_lock_template()` |
| `.../crates/infra/agent/src/sdk/runner_test.rs` | 铸造判据八名任一在场（含仅 `archi_generate` 在场即铸） |
| `.../crates/infra/agent/src/sdk/protect/mod_test.rs`（或 `reason` 侧测试） | `CONTRACT_LOCK_TEMPLATE` 四要素齐备、`%s` / `%t` 全量替换、与 `SESSION_LOCK_TEMPLATE` 逐字不同 |
| `.../crates/infra/agent/src/archi_knowledge_test.rs` | needle「生成物过期用本仓的生成命令重生成」改为指向 `archi_generate` 的新串；体量两处精确断言按实测重冻（门限 1831 / 2031 不变） |
| `.../src-tauri/src/commands/archi/mod_test.rs` | `ArchiArtifactWriter` 与 IPC `archi_api_generate` 在同一临时根上产出逐字相同的 `written` / `unchanged`；写盘仍经 `write_artifacts` 恰一处 |
| `.../crates/core/workflow/src/write/phase_table_test.rs` | prompt 断言随动：`implementation-generator` 零「同批同步接口定义」正句且含两条出路；`dev-design-planner` 零「实现相位同一条」；`test-design-planner` 含 `archi_module` 与 grep 禁令；两 planner 含 `archi_generate` |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.test.tsx` | full 模式提示文案断言逐字改为新串 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.../crates/core/archi/src/api/{render.rs,check/**,paths.rs}` | 渲染与三类差异比对是单点：`archi_generate` 只**调用**它们，MUST NOT 复制或分叉（第二份 `collect` 即违约） |
| `.../crates/infra/archi/src/api_artifacts.rs` | 写盘单点零改动：「只增只改、从不删」「非原子、单份失败不回滚」的语义逐字不变；本变更只多一个**调用方** |
| `.../crates/core/archi/src/{parse,edit,drift}/**` | 判据面与写回原语零变化；`archi_generate` 不碰模型 |
| `.dev-team/architecture/graph/**`（模型与 `INDEX.toml`） | 拓扑零 delta（两条既有边已覆盖），表头计数逐字不变；`api:export` 与 `api:check` 应零 diff |
| `.dev-team/architecture/models/**`（契约资产） | `Backend.Core.Archi.schema.json` 未给 `ArtifactWriter` 补 `x-class`：该元素 `apiUndeclared` 判据恒 `off`（不产判据、不进 `status`），补 schema 会把 dev-design 的模型写面拖进本轮。若 dev-design 裁定必须补，走该相位的模型写面并在 `design.md` 留痕（风险 R8） |
| `openspec/specs/**` 基线 | delta-only：手改基线即「同一事实两份持有者」；基线里的旧表述（`models/**` ❌ 的锁面行、「MUST NOT 存在专用写工具」、「本仓的生成命令」、「实现相位同一条」）由本 change 的 delta 在归档合并时替换，**基线保持原样是本 change 的判据** |
| `src-tauri/Cargo.toml` / `tauri.conf.json` | crate 版本行 MUST NOT 跟随 `package.json`（AGENT.md 版本纪律）；本变更零新依赖边，Cargo 清单零改动 |
| `.../crates/infra/agent/src/sdk/protect/{rules.rs,shell.rs,inline.rs}` | 判定面零改动：只多一条**会话规则**（由 `compose` 注入）与一份**文案**（`reason.rs`），规则序 / glob 方言 / 写面提取逐字不变 |
| `packages/desktop/src/types/generated/**`、`src/bindings/**` | `remedy` 与生成响应都在工具响应层（字符串），不进 specta 出线面 ⇒ bindings 零 diff；MUST NOT 手改生成物 |
| `.../crates/tools/archi-api-gen/**` | CLI 驱动面零改动（它已经是同一单点的另一个调用方） |
| 前端其余产品代码（画布 / 面板结构 / 生成按钮 / 路由） | 零 UI 变化：`full` 模式仍无生成入口，`incremental` 交互逐字不变 |

---

## 验收标准

1. **AC-1 · 工具在册且档位正确**：`mount::registry()` 恰 **19** 项（七内建 + `create_explore_note` + 五 archi 只读 + 两 archi 写 + `archi_generate` + 两 DB + `propose_config`）；`archi_generate` 为 `ToolTier::Write` / `ToolExec::Archi`，`permissionMode = default` 调用被拒、`acceptEdits` 放行；五只读工具档位与两写工具档位逐字不变。
2. **AC-2 · 注入矩阵逐格与收窄后的不变量**：矩阵八行逐行命中——`(proposal, executor)` 与 `(dev-design, executor)` 含 `archi_generate`、`(test-design, executor)` 恰为 `archi_search` + `archi_module`、`(implement, executor)` 不含 `archi_generate`、archi 臂为**八**个 archi 名；`矩阵与写权锁同集` 断言逐格成立且比对集合恰为 `{archi_element, archi_relation}`（`archi_generate` 不在其中，两个方向都有断言）。
3. **AC-3 · 生成链三态**：调用 `archi_generate` 后 `written` / `unchanged` 与盘上字节一一对拍（字节相同者在 `unchanged` 且 `mtime` 不变）；再次调用 `written` 为空；任一目标命中 `ProtectRules` ⇒ `conflicts` 非空、`written` 与 `unchanged` 皆空、磁盘零变化；`root` 取会话 cwd（change 会话写 worktree、主仓零变化）。
4. **AC-4 · 端口分层与写盘单点**：`archi::ArtifactWriter` 定义在 `core/archi`（`grep` 该 crate 零 `fs::write` / 零 `create_dir_all` / 零 `std::process`）；唯一实现是壳层 `commands/archi` 的 `ArchiArtifactWriter` 且函数体只调 `write_artifacts` 一次；`agent-runtime` 的 `Cargo.toml` 依赖中零 `archi-runtime`；产品源码对第二份「生成物 `fs::write`」零命中。
5. **AC-5 · 零产出与孤儿如实**：三种零产出成因（无 `.schema.json` / 接口面留白 / 入口未解析）各自调用返回 `written` 与 `unchanged` 皆空且不报错；工具描述含「零产出不是失败」「孤儿生成物永不删除」「调用前先 `git status`」三句；`orphaned` 列照实回显、工具不删除任何文件（删除后调用前后盘上文件集相同）。
6. **AC-6 · `remedy` 三态与零出线面改动**：`archi_api(check)` 在「三列全空」/「有差异且有 `archi_generate`」/「有差异且无该工具」三场景下 `remedy` 分别为 `null` / 指向工具名 / 指向架构页会话并含「MUST NOT 手改」；`bindings.ts` 与 `bindings.zod.ts` 相对落地前零 diff。
7. **AC-7 · L3 锁面与拒串**：`(implement, executor)` / `(dev-design, evaluator)` / `(code-review, evaluator)` / `(acceptance, evaluator)` / `explore` / `debug` 会话用 `write` 或 `edit` 落 `models/**` 一律被拒；`(proposal, executor)` / `(dev-design, executor)` / `archi` 臂放行；拒串取 `CONTRACT_LOCK_TEMPLATE`（含路径、`%t`、「只在起草相位执行侧与架构页会话写入」、三条出路、「不要改用其他命令或工具尝试绕过」），且与模型锁串逐字不同。
8. **AC-8 · 生成物与模型锁面分离**：非写权相位写 `<archi_dir>/graph/INDEX.toml`、`<archi_dir>/config.json`、`.i.*` 仍放行（L3 只加 `models/**` 一行）；`models/**` 的锁为**会话规则**而非内置规则（内置清单仍恰工作区 `config.json` 一条）。
9. **AC-9 · 悬空指代零残留**：产品源码与 prompts 对「本仓的生成命令」「既有命令重生成」两种字面零命中（`openspec/specs/**` 基线与本 change 目录除外）；`contract-panel.tsx` 与 spec 引述的新串逐字一致；`archi-knowledge.md` 含 `archi_generate` 且骨架 ≤ 1831 B / 全文 ≤ 2031 B（实测值两处冻结断言同批更新）。
10. **AC-10 · 契约作者纪律四处生效**：`implementation-generator.md` 含「不改 schema / md」「受相位锁」「两条出路」三要素且零「同批同步接口定义」正句；`dev-design-planner.md` 零「实现相位同一条」；`proposal-planner.md` 含 md 责任句 + `contractPath` 取路径句 + `archi_generate` 行；`test-design-planner.md` 含 `archi_module` 句与 grep 禁令，且过程 4b 的原文禁令逐字仍在。
11. **AC-11 · 测试面必改断言全绿**：`mount_test` / `policy_test` / `config_propose_test` 的 18 → 19、`compose_test` 的「恰两条」与收窄对拍、`runner_test` 的八名判据、`phase_table_test` 的 prompt 断言逐处改后全绿；MUST NOT 以删断言换绿。
12. **AC-12 · 零回归读数**：`pnpm api:check` 零差异（三类生成物零 diff）；`.dev-team/architecture/**` 零字节变化（元素 / 关系数、六个表头计数、`INDEX.toml` 逐字不变）；`apiFindings` 的计数与构成不因本变更改变（落地时实测为准，当前 38 条全 `docMissing`）。
13. **AC-13 · 版本与改动集**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/Cargo.toml` 与 `tauri.conf.json` 逐字不变；`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（另加本 change 目录下的 `proposal.md` / `specs/**`）。
14. **AC-14 · spec delta 形态**：本 change 的 `specs/` 恰**九个**能力目录（`desktop-archi-agent` / `desktop-write-protection` / `desktop-crate-layout` / `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-contract-panel` / `desktop-archi-canvas` / `desktop-phase-prompts` / `desktop-agent-execution`）；RENAMED 的 `FROM:` 与基线标题逐字一致、`TO:` 与随后的 MODIFIED 标题逐字一致；每个 MODIFIED 块为基线整段拷贝后改且未丢任何未冲突条目；每条 `### Requirement` 至少一个 Scenario 且标记恰为四个 `#`。
15. **AC-15 · 幂等与并发安全**：同一会话内连续两次 `archi_generate` 的第二次 `written` 为空且 `mtime` 不变；生成进行中不新增锁机制（单用户单会话，与 IPC 生成同款语义）；生成失败（`workspace_render` 结构错）⇒ `is_error` 且磁盘零变化。
16. **AC-16 · 画布仍只读**：`full` 模式契约视图零生成按钮、源码零 `archiApiGenerate` 调用路径（既有断言逐字不变）；`/architecture` 页内会话经 `archi_generate` 生成的改动落在主仓且画布在用户刷新前保持旧数据。

---

## 风险

| # | 风险 | 应对 |
|---|---|---|
| R1 | **契约定错时无出路**（最危险）。implement 发现契约不可实现有三条路：(a) 硬实现 → `apiSignatureMismatch` 红 → 回岔口；(b) backtrack 改 schema → 正确但重跑两相位；(c) 偷偷改 schema → 最省事。implement 是 `ModelLevel::Low`，会选 (c) | L3 硬闸挡 (c)（AC-7）；`CONTRACT_LOCK_TEMPLATE` 把 (b) 写成三步指路且携工具名；`implementation-generator.md` 同批发（AC-10），不留「只有锁、没有指路」的半套 |
| R2 | **md 责任零判据**：`docMissing` 恒 `warn` 永不纳管，两份 evaluator checklist 零契约条目 | 本轮先观察，指标写死：本仓现 `docMissing = 38`（分母 49）。若新增元素自带 md，`docMissing` 增速应显著低于元素增速；若跑一段后「新 change 的 `docMissing` 只增不减」，说明 prompt 未被执行，那时再上 proposal R12 判据（不在本 change 内实施） |
| R3 | **md 路径依赖 FQN**：proposal 期边界还在变，元素改名 md 要搬家 | prompt 钉死顺序（先 `archi_element` 定 FQN，再写 md，路径经 `archi_module` 的 `contractPath` 取，勿手拼）；`desktop-archi-canvas` 的文档段按路径取数，搬家仅是一次 `git mv` |
| R4 | **覆盖手写 `.i.ts`**（唯一未消风险）：`write_artifacts` 只比字节不认身份，C 不做 ⇒ `archi_generate` 在 archi 臂 / 起草相位可直接覆盖同名手写文件（wpsweb 已知三处 `.ts` + `.i.ts` 并存） | 三条止损全部进本轮：① 工具描述硬要求「调用前先 `git status` 确认工作树干净，以便意外可 `git restore`」（AC-5）；② 首轮在 change worktree 里跑（最坏随 change 丢弃），架构页会话直写主仓的风险面在 `desktop-archi-chat` 既有「事后 `git diff` 评审」纪律内；③ 触发条件写死：**若实际发生一次覆盖，C（身份判据 ∧ 拒绝覆盖）立刻升为优先项** |
| R5 | **161 孤儿噪声**：命名即身份，wpsweb 类仓 `orphaned` 恒非空 | 接受噪声（B 不做）。止损：工具描述一句「孤儿永不删除、`orphaned` 非空属常态」（AC-5）；面板确认文案与 `orphaned` 列照实不动（事实陈述不因 M 大而变错） |
| R6 | **`.i.rs` 红窗**：起草期生成 ⇒ 见证指向未实现函数 ⇒ worktree 内 `cargo check` 红直到 implement 补齐 | 红窗只在 worktree 内、且 proposal / dev-design 步序列无编译步（`Executor → Evaluator`），门禁不因此变红；主仓不受影响；若某仓的 `StaticCheck` 已在起草相位跑（本仓没有），风险落到该仓的相位表，与本工具无关 |
| R7 | **零产出被误读为工具坏**：起草期新模块最多，多数调用零产出 | 工具描述第 ② 句（AC-5）写死三成因；`archi_api(check)` 的 `remedy` 在有差异时指路（AC-6），两者构成「零产出 → 看差异 → 知道该补什么」的闭环 |
| R8 | **接口资产面出现新缺口**：`ArtifactWriter` 是 `core/archi` 的新导出，但本轮不写进 `Backend.Core.Archi.schema.json` | `apiUndeclared` 判据恒 `off`（不产判据、不进 `status`、不影响 `passed`），不构成门禁风险；缺口显式留痕于「不要修改」节，由 dev-design 裁定是否补 `x-class` 并在 `design.md` 留痕（MUST NOT 在 implement 相位顺手补） |
| R9 | **注入点多、易漏一处**：writer 要贯穿 `compose_turn` → 门面 → runner → `PumpPayload` → 上下文（六处），漏一处则生产路径 writer 恒 `None` | 单点 builder（`with_artifact_writer`，与 `with_ask_port` 同型）+ 铸造判据八名任一在场；`archi_tool_test` 覆盖「writer 缺席 ⇒ `is_error`」，`runner_test` 覆盖铸造判据；两处 `compose_turn` 调用点由 AC-3 的「change 会话写 worktree、主仓零变化」端到端兜住 |
| R10 | **矩阵扩面把只读工具带进写档**（`archi_generate` 与 `archi_api` 同相位下发） | 档位是**挂载项级**：`archi_api` 仍 `ToolTier::Read`、`archi_generate` 独立项 `ToolTier::Write`，`policy_test` 的封闭集把两者放进不同臂（AC-1 / AC-11）；`archi_api` 零写盘条款在同批 MODIFIED 中保留 |
