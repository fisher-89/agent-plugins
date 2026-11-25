# desktop-write-protection Specification（delta: archi-agent-contract-knowledge-gap）

## Purpose

本 delta 只做两件事：

- **一处实质增量**：把新的两个 agent 写工具（`archi_element` / `archi_relation`）纳入**同一** `ProtectRules` 判定——它们写的是同一批模型文件，若不过闸，用户在工作区 `write_protection.files` 里显式 glob 的护栏（本能力明写「首命中即拒，MUST NOT 因『模型是源码』而放行」）就会对它们静默失效。
- **两处交叉引用换名**：`desktop-archi-agent` 的两条 requirement 改名（「模型 = 源码（写纪律与不设专用写工具）」→「模型 = 源码（两条写入口与写纪律）」；「只读档位归属与成员校验不变」→「archi 工具档位归属与成员校验（五只读 + 两写）」），本能力内的引用随之逐字更新。

零语义变化：内置锁面仍恰 `config.json` 一条、会话锁的相位政策与锁面（只 `graph/model/**`）、规则序（内置 → 会话 → 用户）、拒串族、fail-closed 方向全部逐字不变。

## MODIFIED Requirements

### Requirement: 架构域路径不在内置锁面

架构模型文件 SHALL 可被 agent 在 change 的 worktree 内、以及架构页通用会话（`desktop-archi-chat`，执行根 = 主仓根）下自由修改（模型 = 源码，见 `desktop-archi-agent`「模型 = 源码（两条写入口与写纪律）」的两条写权通路）。因此：

- **内置锁面 MUST NOT 纳入架构域路径**：`<archi_dir>/graph/model/**`（模型声明）、`<archi_dir>/models/**`（契约资产）与架构域配置文件 MUST NOT 进入「保护规则装配」的**内置**锁定清单。理由：内置规则是**相位无关的绝对路径 glob**（`load` → `match_path`，cwd 仅用于 bash 路径解析），在 worktree 与主仓两条通路下同样命中——一旦纳入，agent 就改不了模型；而本仓「模型可经 change worktree 随归档回合合回，也可经架构页通用会话直改主仓」的两条通路都依赖"写得进去"。
- **被禁的是「相位无关的锁」，不是「锁」**：本条的禁令**只对内置面**成立。按相位条件生效的**会话级**锁（`SessionInjections.protect` → `with_session`）不住在内置清单里，且命中范围由 provenance 决定（写权相位与架构页会话不挂锁），故它是形态而非扩充——见「会话级写入禁区（相位锁）」。MUST NOT 以本条禁令为由否决会话级相位锁，亦 MUST NOT 把会话规则搬进内置清单（搬进去即重新变成相位无关）。
- **模型文件的真正护栏是回合确认、事后评审与相位条件**：`protect` 本就不是安全边界（`bash` 可绕），它是防误触护栏；通路 A 的设计权由「回合必须人工确认」承载（归档确认面的架构变更区块，见 `desktop-change-archive`「归档入口与前置确认面」），通路 B 的设计权由事后 `git diff` 评审与手动刷新校验承载（见 `desktop-archi-chat`「写权落点与刷新纪律」），非写权相位的写权由相位锁承载——MUST NOT 由写盘前的设计质量校验承载。
- **用户显式规则的语义不变（本变更明确覆盖到新写工具）**：用户若在工作区配置的 `write_protection.files` 中显式 glob 了架构域路径，规则 SHALL 照常生效（首命中即拒；但位次仍在会话规则之后）——那是用户的自选护栏，本 requirement 只禁「内置默认锁」。**该语义 SHALL 覆盖全部写模型入口**：`write` / `edit` / `bash` 与两个 agent 写工具（`archi_element` / `archi_relation`）一律走同一 `ProtectRules`（判定点见「会话级写入禁区（相位锁）」的「判定的工具面覆盖」）。
- **内置面唯一增删纪律不变**：内置清单的任何增删仍须走本能力既有「内置锁定（独立于配置解析结果）」条款；本条为**禁止扩充**的显式裁定，MUST NOT 被解读为给内置面新增条目。
- 与 `desktop-agent-execution` 的既有挂载纪律不冲突：守卫链四段顺序（成员校验 → 档位 → 沙箱 → 保护闸）、工作区根注入缝、fail-open / fail-closed 方向全部零改动（会话规则经同一 `ProtectRules` 结构参与判定，不是第五段闸）。

#### Scenario: 内置规则清单不含架构域

- **WHEN** 装配保护规则（`ProtectRules`）并审查内置锁定清单
- **THEN** 清单仍恰为工作区 `config.json` 一条（既有语义逐字不变）；不含 `graph/model/**`、不含 `models/**`、不含架构域配置文件；相位锁以**会话规则**形态出现（`with_session` 注入，不是内置第二条）

#### Scenario: worktree 内写模型按相位放行

- **WHEN** agent 在 `dev-design/executor` 的 worktree 内以 `write` / `edit` 修改 `<archi_dir>/graph/model/*.toml`
- **THEN** `evaluate` 返回 `None`（未命中任何规则），写入照常执行；写入保护不产生任何拒绝串

#### Scenario: 主仓根直写模型不被拦截

- **WHEN** agent 在架构页通用会话（执行根 = 主仓根）内以 `write` / `edit` 修改主仓的 `<archi_dir>/graph/model/*.toml`
- **THEN** `evaluate` 返回 `None`（未命中任何规则），写入照常执行；内置锁面仍只有工作区 `config.json` 一条（两条通路下判定一致）

#### Scenario: 用户显式规则仍生效

- **WHEN** 用户在工作区配置的 `write_protection.files` 里显式 glob 了架构域模型路径，agent 在白名单相位（无会话锁）试图写该路径
- **THEN** 按用户规则拒绝（首命中即拒，拒绝串取用户 reason）；MUST NOT 因"模型是源码"而放行

#### Scenario: 用户显式规则同样命中新写工具

- **WHEN** 同一用户规则在位，agent 在白名单相位用 `archi_element`（落点解析为被 glob 的模型文件）写模型
- **THEN** 同样拒绝（同一 `ProtectRules`、同一拒串族，`%s` 为解析出的目标路径、`%t` 为工具名）；盘上字节零变化；未命中规则的落点照常写入

### Requirement: 会话级写入禁区（相位锁）

保护面 SHALL 支持**会话级**规则（第三来源），用于表达「按相位条件」的写入禁区；本变更在该面上落一条**架构模型相位锁**：

- **政策（绝对命题）**：架构模型（`<archi_dir>/graph/model/**`）可写 ⟺ ① 架构页通用会话（`source = "archi"`）∨ ② (`source = "change"` ∧ `phase ∈ {proposal, dev-design}` ∧ `role = executor`)。其余一切会话锁定——**含 `dev-design/evaluator`**（白名单按 `(phase, role)` 而非 phase：评审批次不写模型）与 `explore` / `debug`（不是「步骤」，但政策不设旁路）。
- **载体**：会话规则的来源 SHALL 为 `SessionInjections.protect`（装配层从 provenance 派生，规则 glob 由 `foundation::layout` 的架构域派生族给出，`core` 不解释路径语义）；`ProtectRules` SHALL 以 `with_session(Option<Vec<SessionProtectRule>>)` 追加（`load` 的签名与「唯一 IO 点」语义逐字不变）。
- **规则序**：**内置（`config.json`）→ 会话（相位锁）→ 用户（自选）**，首个命中即定，MUST NOT 拼接多条 reason。会话规则排在用户之前的理由是文案：相位锁串带指路，MUST NOT 被用户自选 reason 盖掉。
- **锁面范围（封闭三项）**：
  | 路径 | 锁 | 理由 |
  |---|---|---|
  | `<archi_dir>/graph/model/**` | ✅ | 它就是「模型」 |
  | `<archi_dir>/models/**`（契约资产 md / schema） | ❌ | `implement/executor` 持有 `archi_api`；契约是**手写真相**，改代码相位必须能同步改契约——锁了契约判据会永远飘 |
  | `<archi_dir>/graph/INDEX.toml` | ❌ | 生成物，「MUST NOT 手改」已足够 |
  | `<archi_dir>/config.json`（看护配置） | ❌ | 看护策略不是模型 |
- **判定的工具面覆盖（本变更新增）**：本相位锁与用户规则 SHALL 对**全部写模型入口**生效——`write` / `edit` 按入参 `path` 判定、`bash` 按写入面提取判定（既有三路），两个 agent 写工具（`archi_element` / `archi_relation`）SHALL 在**落点解析之后**以解析出的目标文件过**同一** `ProtectRules`（同一规则序、同一首命中即拒、同一拒串族；MUST NOT 另立规则集、MUST NOT 另写文案）。落点只有 `plan_*` 之后才知道，故判定点可在执行体内（实现选择归 `desktop-archi-agent`）；MUST NOT 因为「工具没有 `path` 入参」而把它判在保护射程之外。
- **fail-closed 方向**：`source_ref` 畸形 / 短段 / 未知 phase / 非 change 与 archi 来源 → **挂锁**（与工具面「零 archi 工具」同方向）。装配层 MUST NOT 因解析失败而放行写权。
- **拒绝文案 SHALL 指路**（四个要素，缺一即违约）：① 该路径受相位锁保护并携路径与工具名（`%t`）；② 架构模型只在 `proposal` / `dev-design` 相位的执行侧（及架构页会话）可写；③ 补救路径——把需求写进 `design.md` / `tasks.md` 由 `dev-design` 相位同步模型，或请用户改用架构页会话直接维护；④ **不要改用其他命令或工具尝试绕过**（写入路径均走同一闸）。
- **与架构页通路零冲突**：`source = "archi"` 的会话 MUST NOT 挂会话锁（通路 B 的可写性是既有承诺，见 `desktop-archi-agent`「模型 = 源码（两条写入口与写纪律）」）。
- **不是安全边界（如实留痕）**：会话锁与内置锁同一口径——`bash` 内容级写入检测是启发式，`git stash` / `git clean` 等**批量动作**不按相位判别（`BulkGlobs` 与相位无关）；`git checkout <ref> -- <path>`、重定向 / `tee` / node-inline 等**单路径**形态已被会话规则覆盖。该缺口 MUST NOT 被表述为「模型目录在非白名单相位不可改」。

#### Scenario: 白名单相位写模型成功

- **WHEN** 以 `source = "change"`、`source_ref = <id>/dev-design/executor/2` 发起会话并以 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** `evaluate` 返回 `None`（会话规则缺席），写入照常执行；内置锁面仍只有工作区 `config.json` 一条

#### Scenario: 非白名单相位被拒且文案指路

- **WHEN** 以 `<id>/implement/executor/1` / `<id>/code-review/evaluator/1` / `<id>/dev-design/evaluator/1` / `source = "explore"` / `source = "debug"` 发起会话并 `write` 模型文件
- **THEN** 一律拒绝；拒绝串含路径、工具名、「只在 `proposal` / `dev-design` 执行侧可写」、「写进 `design.md` / `tasks.md` 或改用架构页会话」与「不要改用其他命令或工具尝试绕过」四要素；`SystemNotice.payload.reason` 与 is_error `ToolResult.content` 为同一串

#### Scenario: 写工具在保护闸内且档位／成员校验在其前

- **WHEN** 在非白名单相位（`<id>/implement/executor/1`）以 `archi_element` 尝试写模型；另在白名单相位以同一工具写命中用户规则的落点
- **THEN** 前者零下发（成员校验先拒），执行体零调用；后者被用户规则拒绝（同族拒串、`%t` = `archi_element`），盘上字节零变化。两例均 MUST NOT 出现「工具无 `path` 入参 ⇒ 不受保护」的分支

#### Scenario: 契约资产与生成物不锁

- **WHEN** 以 `<id>/implement/executor/1` 会话分别写 `<archi_dir>/models/<分层…>/<FQN>.md`、`<archi_dir>/graph/INDEX.toml`、`<archi_dir>/config.json`
- **THEN** 三者均放行（会话锁只命中 `graph/model/**`）；模型目录同轮的写入仍被拒

#### Scenario: 畸形 source_ref 挂锁

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别，随后写模型文件
- **THEN** 拒绝（fail-closed）；装配层零 `Err`、零放行写权

#### Scenario: 单路径绕过面已被覆盖

- **WHEN** 非白名单相位以 `bash` 执行 `echo x > <archi_dir>/graph/model/03-x.toml`、`tee <…>/model.toml`、`node -e "require('fs').writeFileSync('<…>/03-x.toml','')"`、`git checkout HEAD -- <…>/graph/model/03-x.toml`
- **THEN** 四条均被会话规则拒绝（写入面提取与批量还原的 write 三态同源）；`git stash` 一类批量动作仍未覆盖（知性缺口留痕，见「不是安全边界」）

## Module Contract

### Function: `crates/infra/agent/src/sdk/protect/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `ProtectRules::match_path` / `evaluate_path` | 改（可见性） | 写工具需要在落点解析后复用同一路径判定；`evaluate` 的 `write` / `edit` / `bash` 三路分派与 `_ => None` 的形状逐字不变（写工具**不经** `evaluate` 分派，而在执行体内调 `evaluate_path` 等价面），避免出现「无 `path` 入参 ⇒ 不判」的空分支 |
| `ProtectRules::load` / `with_session` | 零改动 | 唯一 IO 点、三段规则序、`SessionProtectRule` 形状逐字不变 |
| 内置清单 | 零改动 | 仍恰工作区 `config.json` 一条 |

### Function: `crates/infra/agent/src/sdk/protect/{rules,shell,inline,reason}.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| 内置 / 用户规则派生、shell 三态提取、node / python 关联式判别、三条拒串 | 零改动 | 逐字不变；本变更只增加一个消费入口（写工具的落点判定），不新增规则、不新增文案 |

### API: 零改动面

| 项 | 描述 |
|---|---|
| `SessionInjections.protect` 与 `model_write_lock` | 形态与置位点逐字不变（两写工具的下发面与该锁对齐，不是改动锁本身） |
| 拒串族（内置 / 用户 / 相位锁三形态） | 逐字不变（写工具沿用同一族） |
