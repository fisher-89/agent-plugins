# desktop-write-protection Specification (Delta)

## ADDED Requirements

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
- **fail-closed 方向**：`source_ref` 畸形 / 短段 / 未知 phase / 非 change 与 archi 来源 → **挂锁**（与工具面「零 archi 工具」同方向）。装配层 MUST NOT 因解析失败而放行写权。
- **拒绝文案 SHALL 指路**（四个要素，缺一即违约）：① 该路径受相位锁保护并携路径与工具名（`%t`）；② 架构模型只在 `proposal` / `dev-design` 相位的执行侧（及架构页会话）可写；③ 补救路径——把需求写进 `design.md` / `tasks.md` 由 `dev-design` 相位同步模型，或请用户改用架构页会话直接维护；④ **不要改用其他命令或工具尝试绕过**（写入路径均走同一闸）。
- **与架构页通路零冲突**：`source = "archi"` 的会话 MUST NOT 挂会话锁（通路 B 的可写性是既有承诺，见 `desktop-archi-agent`「模型 = 源码（写纪律与不设专用写工具）」）。
- **不是安全边界（如实留痕）**：会话锁与内置锁同一口径——`bash` 内容级写入检测是启发式，`git stash` / `git clean` 等**批量动作**不按相位判别（`BulkGlobs` 与相位无关）；`git checkout <ref> -- <path>`、重定向 / `tee` / node-inline 等**单路径**形态已被会话规则覆盖。该缺口 MUST NOT 被表述为「模型目录在非白名单相位不可改」。

#### Scenario: 白名单相位写模型成功

- **WHEN** 以 `source = "change"`、`source_ref = <id>/dev-design/executor/2` 发起会话并以 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** `evaluate` 返回 `None`（会话规则缺席），写入照常执行；内置锁面仍只有工作区 `config.json` 一条

#### Scenario: 非白名单相位被拒且文案指路

- **WHEN** 以 `<id>/implement/executor/1` / `<id>/code-review/evaluator/1` / `<id>/dev-design/evaluator/1` / `source = "explore"` / `source = "debug"` 发起会话并 `write` 模型文件
- **THEN** 一律拒绝；拒绝串含路径、工具名、「只在 `proposal` / `dev-design` 执行侧可写」、「写进 `design.md` / `tasks.md` 或改用架构页会话」与「不要改用其他命令或工具尝试绕过」四要素；`SystemNotice.payload.reason` 与 is_error `ToolResult.content` 为同一串

#### Scenario: 契约资产与生成物不锁

- **WHEN** 以 `<id>/implement/executor/1` 会话分别写 `<archi_dir>/models/<分层…>/<FQN>.md`、`<archi_dir>/graph/INDEX.toml`、`<archi_dir>/config.json`
- **THEN** 三者均放行（会话锁只命中 `graph/model/**`）；模型目录同轮的写入仍被拒

#### Scenario: 畸形 source_ref 挂锁

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别，随后写模型文件
- **THEN** 拒绝（fail-closed）；装配层零 `Err`、零放行写权

#### Scenario: 单路径绕过面已被覆盖

- **WHEN** 非白名单相位以 `bash` 执行 `echo x > <archi_dir>/graph/model/03-x.toml`、`tee <…>/model.toml`、`node -e "require('fs').writeFileSync('<…>/03-x.toml','')"`、`git checkout HEAD -- <…>/graph/model/03-x.toml`
- **THEN** 四条均被会话规则拒绝（写入面提取与批量还原的 write 三态同源）；`git stash` 一类批量动作仍未覆盖（知性缺口留痕，见「不是安全边界」）

## MODIFIED Requirements

### Requirement: 保护规则装配（内置锁定 + 用户规则合并）

保护规则 SHALL 由**三部分**按次合并而成，装配为单点纯数据结构 `ProtectRules`：

- **内置锁定（独立于配置解析结果，恒首位）**：SHALL 只收一条 `**/.dev-team/config.json`。该 glob MUST 由 `foundation::layout::config_path` 派生后加 `**/` 前缀，产品源码 MUST NOT 出现 `openspec` 目录名字面量或 `config.json` 文件名字面量（沿 desktop-crate-layout 两条隔离禁令）。内置规则 MUST NOT 依赖 `config::load` 是否成功——工作区配置缺失、坏 JSON、字段违例时内置面 SHALL 照常生效。`**/openspec/changes/**/workflow.json` MUST NOT 纳入内置锁定：工作流状态以库为唯一权威，legacy 文件零读取零产出，锁定它无对象可保护。该规则的存在使**该文件成为保护面的第一道拦截**（`protect/**` 内无 override 或跳过开关）——仓库面若需更新该文件内的指针，SHALL 以值级替换交付并复核（不变量见 `repo-surface-scope`），MUST NOT 为改写指针而改动本条内置锁定语义或给规则加 override。
- **会话级规则（第三来源，插在内置之后、用户之前）**：SHALL 取自 `SessionInjections.protect`（`Option<Vec<SessionProtectRule>>`；`None` = 不追加）。规则 glob SHALL 为**装配层已派生的绝对形态或 `**/` 前缀相对形态**（`core` 不解释路径语义），`reason` 为携 `%s` / `%t` 占位符的模板串；装配 SHALL 经 `ProtectRules::with_session(...)` 完成（`load(workspace_root)` 的签名、唯一 IO 点与「每 run 一次」语义逐字不变）。本来源的语义与范围归「会话级写入禁区（相位锁）」。
- **用户规则**：SHALL 取自 `config::load(workspace_root).config.write_protection.files[]`（`config::load` 永不失败，恒返回合法配置 + diagnostics）。逐条规则：`glob` 为 `None`（config 组装层已剔除空串 / 非法值）SHALL 跳过；`glob` 为绝对路径 SHALL 原样使用；`glob` 为相对路径 SHALL 前缀 `**/`（无通配模式在匹配面走「相等或 `pattern/` 起手」语义，不加前缀只命中 root 相对形态）。`reason` SHALL 原样保留（含 `%s` / `%t` 占位符，替换见「拒绝指令文案」）。
- **规则序**：位次 SHALL 为**内置 → 会话 → 用户**；首个命中规则决定拒绝串（内置 `config.json` 规则恒最先；相位锁恒先于用户自选规则，使指路文案不被用户 reason 覆盖）。同一次判定 MUST NOT 拼接多条 reason。
- **方言单点**：规则匹配 SHALL 一律经 `checks::globmatch::match_glob`（`**/` 零段可命中、Windows 盘符归一、无通配模式走目录前缀、花括号交替已覆盖）；`agent-runtime` MUST NOT 自造第二份 glob 匹配实现。非法 / 超限模式 SHALL 按不命中收敛（不 panic、不拒绝启动）。
- **装配时机与唯一 IO 点**：规则装配 SHALL 为每 run 一次（会话泵起播时），是 protect 模块的唯一 IO 点（`config::load` + 规则合并）；**会话规则的合并发生在同一装配点**（不新增 IO —— 会话规则是纯数据）；loop 内每次判定 MUST NOT 触碰文件系统（run 内改 config 不改变本 run 行为）。
- **工作区根缺席**：装配入参为 `None`（未注入 workspace root）时 SHALL 只产出内置规则（用户规则缺席，会话规则照常在位）——MUST NOT 产出「保护关闭」的空规则集。

#### Scenario: 内置规则独立于配置解析

- **WHEN** 工作区无 `.dev-team/config.json`，或该文件为非法 JSON，或 `write_protection` 字段类型违例
- **THEN** 装配结果含内置 `**/.dev-team/config.json` 一条（另加在位的会话规则），write / edit 指向该文件仍被拒；`config::load` 的 diagnostics 不参与保护面判定

#### Scenario: 相对 glob 自动前缀

- **WHEN** 用户配置 `write_protection.files = [{ glob: "openspec/changes/**/proposal.md" }]`，模型在 `<root>/openspec/changes/x/proposal.md` 上调用 write
- **THEN** 判定命中（相对 glob 加 `**/` 后可匹配绝对路径），拒绝串为通用回落串

#### Scenario: 绝对 glob 原样

- **WHEN** 用户配置绝对 glob `D:/ws/secret.md`（或 `/home/u/secret.md`），模型写入该绝对路径
- **THEN** 判定命中（绝对 glob 不加 `**/` 前缀，原样匹配）

#### Scenario: 内置优先于用户规则

- **WHEN** 用户同时配置 `{ glob: "openspec/**", reason: "自定义原因" }`，模型写 `<root>/.dev-team/config.json`
- **THEN** 拒绝串取内置 `config.json` 规则的原因（内置先行，首个命中即定），自定义 reason 不被拼接

#### Scenario: 会话规则先于用户规则

- **WHEN** 用户配置里有一条也命中同一模型路径的规则（带自定义 reason），且当前会话挂在相位锁下
- **THEN** 拒绝串取会话规则（相位锁模板），用户自定义 reason 不被拼接——三个来源的位次逐格可辨

#### Scenario: 工作区根缺席仅内置

- **WHEN** 装配入参为 `None`（root 未注入）且会话规则在场
- **THEN** 规则集只含内置一条 + 会话规则；写 `.dev-team/config.json` 仍被拒、写模型目录按会话规则拒；用户配置的任何 glob 均不生效

#### Scenario: 规则面单源自述

- **WHEN** 审查 `ProtectRules` 装配逻辑的注释与 spec 措辞
- **THEN** 每条规则语义按本 spec 直述（内置面 / 会话面 / 用户面 / 序 / 方言 / 时机 / 根缺席七条）
- **AND** 无「与插件端同款处理 / 插件端此处放行」类对照表述（插件端已不存在，语义不依赖对照读者）

### Requirement: 拒绝指令文案

命中保护时 SHALL 产出一条面向 **Agent**（首要读者，模型可见）兼可读于用户（次读者）的拒绝指令串，四类形态：

- **内置 `config.json` 规则串**：SHALL 含五要素——① 该文件受写入保护（携路径）；② 该文件是工作区配置权威、Agent 不得改写；③ 如需调整须在回复中说明「改哪个字段、改成什么、为什么」并由用户自行操作；④ 若任务无法在不改该文件的前提下完成，立即停止并通知用户；⑤ 不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。④⑤ MUST NOT 省略——`deny` 不中断 run，模型被拒后会自然换路重试（write → 重定向 → `cp` → `node -e`），这两句是掐断该链的唯一手段。
- **会话级相位锁串（本变更新增）**：SHALL 含四要素——① 该路径受**相位锁**保护（携路径与工具名 `%t`）；② 架构模型只在 `proposal` / `dev-design` 相位的执行侧（及架构页会话）可写；③ 补救路径（把需求写进 `design.md` / `tasks.md` 由 `dev-design` 相位同步模型，或请用户改用架构页会话直接维护）；④ 不要改用其他命令或工具尝试绕过（写入路径均走同一闸）。该模板 SHALL 单点定义在 `sdk/protect/reason.rs`（装配层取它构造规则串），MUST NOT 在装配层再抄一份文案。
- **用户规则带 `reason`**：SHALL 原样透传配置文案，并完成占位符替换——`%s` → 命中路径，`%t` → 工具名（`write` / `edit` / `bash`），全量替换所有出现。
- **用户规则无 `reason` 回落串**：SHALL 为通用串——该文件受写入保护（携路径与工具名）+ 请调整方案避免修改该文件 + 如无法规避立即停止并通知用户、不要改用其他命令绕过。
- **传递面**：拒绝串 SHALL 以单条字符串返回，由 loop 侧同时写进 `SystemNotice.payload.reason`（前端 notice 展示）与 is_error `ToolResult.content`（模型可见）；保护模块 MUST NOT 自行发事件、MUST NOT 分叉出第二套文案。
- **「中断」语义边界**：文案中的「立即停止」指模型结束本轮（正常收敛），引擎 MUST NOT 为此新增机制；相位会话停手后的收敛归编排层判定，保护模块不越界。

#### Scenario: 内置串含掐断换路链两要素

- **WHEN** 命中内置 `.dev-team/config.json` 规则
- **THEN** 拒绝串含「不要改用其他命令或工具尝试绕过」与「立即停止并通知用户」两句，且含「改哪个字段、改成什么、为什么」的用户处置指引

#### Scenario: 相位锁串四要素齐备

- **WHEN** 非白名单相位（`implement/executor`）写 `<archi_dir>/graph/model/03-x.toml`
- **THEN** 拒绝串含路径、「只在 `proposal` / `dev-design` 执行侧可写」、补救路径（`design.md` / `tasks.md` 或架构页会话）与「不要改用其他命令或工具尝试绕过」四要素；`%t` 已替换为实际工具名（`write` / `edit` / `bash` 三形态各一例）

#### Scenario: 用户 reason 占位符替换

- **WHEN** 用户配置 `{ glob: "docs/**", reason: "受保护：%s（%t）不可写" }`，模型以 `bash` 写 `docs/a.md`
- **THEN** 拒绝串为该模板全文，`%s` 替换为实际路径、`%t` 替换为 `bash`（多处出现全部替换）

#### Scenario: 无 reason 回落

- **WHEN** 用户配置的规则只有 `glob` 无 `reason`，判定命中
- **THEN** 拒绝串为通用回落串（含路径与工具名 + 调整方案 + 不绕过 + 停止并通知用户）

#### Scenario: 单串双面传递

- **WHEN** 保护闸拒绝一次工具调用（内置 / 会话 / 用户三类规则各一例）
- **THEN** `SystemNotice.payload.reason` 与同 id 的 is_error `ToolResult.content` 为同一字符串，模型与用户看到同一份指令（保护模块自身零事件流出）

### Requirement: 架构域路径不在内置锁面

架构模型文件 SHALL 可被 agent 在 change 的 worktree 内、以及架构页通用会话（`desktop-archi-chat`，执行根 = 主仓根）下自由修改（模型 = 源码，见 `desktop-archi-agent`「模型 = 源码（写纪律与不设专用写工具）」的两条通路）。因此：

- **内置锁面 MUST NOT 纳入架构域路径**：`<archi_dir>/graph/model/**`（模型声明）、`<archi_dir>/models/**`（契约资产）与架构域配置文件 MUST NOT 进入「保护规则装配」的**内置**锁定清单。理由：内置规则是**相位无关的绝对路径 glob**（`load` → `match_path`，cwd 仅用于 bash 路径解析），在 worktree 与主仓两条通路下同样命中——一旦纳入，agent 就改不了模型；而本仓「模型可经 change worktree 随归档回合合回，也可经架构页通用会话直改主仓」的两条通路都依赖"写得进去"。
- **被禁的是「相位无关的锁」，不是「锁」**：本条的禁令**只对内置面**成立。按相位条件生效的**会话级**锁（`SessionInjections.protect` → `with_session`）不住在内置清单里，且命中范围由 provenance 决定（写权相位与架构页会话不挂锁），故它是形态而非扩充——见「会话级写入禁区（相位锁）」。MUST NOT 以本条禁令为由否决会话级相位锁，亦 MUST NOT 把会话规则搬进内置清单（搬进去即重新变成相位无关）。
- **模型文件的真正护栏是回合确认、事后评审与相位条件**：`protect` 本就不是安全边界（`bash` 可绕），它是防误触护栏；通路 A 的设计权由「回合必须人工确认」承载（归档确认面的架构变更区块，见 `desktop-change-archive`「归档入口与前置确认面」），通路 B 的设计权由事后 `git diff` 评审与手动刷新校验承载（见 `desktop-archi-chat`「写权落点与刷新纪律」），非写权相位的写权由相位锁承载——MUST NOT 由写盘前的设计质量校验承载。
- **用户显式规则的语义不变**：用户若在工作区配置的 `write_protection.files` 中显式 glob 了架构域路径，规则 SHALL 照常生效（首命中即拒；但位次仍在会话规则之后）——那是用户的自选护栏，本 requirement 只禁「内置默认锁」。
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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/protect/mod.rs` | 装配入口 + 判定 | `ProtectRules::load(Option<&Path>) -> ProtectRules`（唯一 IO 点，签名不变）；**新增** `with_session(self, Option<Vec<SessionProtectRule>>) -> ProtectRules`（纯数据合并，零 IO）；`evaluate(tool, input, cwd) -> Option<String>`（纯函数零 IO）；规则位次由装配序表达（内置 → 会话 → 用户），判定侧零优先级分支 |
| `crates/infra/agent/src/sdk/protect/rules.rs` | 来源合并 + 规则序 | `load`（内置 + 用户）保持签名；新增 `with_session_rules(rules, session)` 把会话规则插在内置之后、用户之前；`ReasonKind::Session` 变体（持模板串）；glob / 相对前缀 / 方言纪律零改动 |
| `crates/infra/agent/src/sdk/protect/reason.rs` | 四类拒绝串 | 新增 `SESSION_LOCK_TEMPLATE` + `session_lock_template()` 出口（`compose` 取它构造规则）；`deny_reason` 增 `ReasonKind::Session` 分支（`%s` / `%t` 全量替换）；三类既有串逐字不变 |
| `crates/infra/agent/src/sdk/protect/mod.rs`（写入路径访问器） | 新增 `pub(crate) fn written_paths(command: &str, cwd: &Path) -> Vec<String>`：只把 `shell::extract_file_ops` 的 **write** 态目标经既有 `resolve` 归一后出线（`revert` / `delete` 不入），供 `preclose` 复用同一写面提取面（MUST NOT 第二份写面解析）；`shell.rs` 的类型、可见性与判据零改动 |
| `crates/infra/agent/src/sdk/protect/shell.rs` | 零改动 | 掩码 / 三态 / 档 B / 批量还原判据逐字不变（只被 `mod.rs` 的新访问器间接复用） |
| `crates/core/agent/src/runner.rs` | 会话契约 | `SessionInjections.protect: Option<Vec<SessionProtectRule>>`（`#[serde(default)]`、无 `specta::Type`）；`SessionProtectRule { glob: String, reason: String }`（`Clone + Eq`）；旧线 JSON 缺字段读 `None` |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `model_write_lock(provenance) -> Option<Vec<agent::SessionProtectRule>>`：archi 臂与 `(proposal｜dev-design, executor)` → `None`；其余（含畸形 `source_ref` / 未知 phase / 非 change 来源）→ `Some([一条])`；glob 由 `foundation::layout::archi_graph_model_rel_dir` 派生、reason 取 `protect::session_lock_template()`；三臂装配产物携带该位 |
| `crates/infra/agent/src/sdk/runner.rs` | 装配挂载 | `ProtectRules::load(workspace_root).with_session(injections.protect)`；其余装配语义零改动 |
| `.dev-team/architecture/graph/model/**` | 不再有任何内置锁 | 相位锁为会话规则（非内置清单条目）；内置面仍恰一条（工作区 `config.json`）；用户自选规则照常可叠加 |
