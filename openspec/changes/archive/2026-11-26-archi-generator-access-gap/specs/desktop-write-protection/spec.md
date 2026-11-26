# desktop-write-protection Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 把**契约资产目录**收进相位锁，并给新生成工具补一条射程：

- **锁面加一行**：`<archi_dir>/models/**`（接口定义 `.schema.json` + 模块文档 `.md`）由 ❌ 改 ✅——实现相位不再能改契约，「改契约迎合代码」从结构上不可达（`error` 档判据因此名副其实）。
- **代价如实写**：实现相位发现契约错的唯一出路是 `backtrack`，判据在 `implement` 内不再可自愈；该代价 SHALL 进锁面表的理由列，MUST NOT 只翻符号。
- **第二份拒串**：契约目录的拒串 MUST NOT 复用模型专用 `SESSION_LOCK_TEMPLATE`（它通篇说「架构模型 / 元素与关系」），新增 `CONTRACT_LOCK_TEMPLATE`，把三条出路写死（改代码追上契约 / 停下述冲突由用户 backtrack / 改用架构页会话）。
- **射程补一角**：`archi_generate` 无 `path` 入参，仍 SHALL 在写盘前**逐份**把应生成物过同一 `ProtectRules`（任一份命中即整批拒绝、零写盘）。
- 内置锁面**零改动**：`graph/model/**`、`models/**` 与架构域配置仍 MUST NOT 进内置清单——相位锁仍是**会话级**规则。

## MODIFIED Requirements

### Requirement: 会话级写入禁区（相位锁）

保护面 SHALL 支持**会话级**规则（第三来源），用于表达「按相位条件」的写入禁区；本变更在该面上落一条**架构模型 + 契约资产相位锁**：

- **政策（绝对命题）**：架构模型（`<archi_dir>/graph/model/**`）与**契约资产**（`<archi_dir>/models/**`）可写 ⟺ ① 架构页通用会话（`source = "archi"`）∨ ② (`source = "change"` ∧ `phase ∈ {proposal, dev-design}` ∧ `role = executor`)。其余一切会话锁定——**含 `dev-design/evaluator`**（白名单按 `(phase, role)` 而非 phase：评审批次不写模型、不写契约）与 `explore` / `debug`（不是「步骤」，但政策不设旁路）。
- **载体**：会话规则的来源 SHALL 为 `SessionInjections.protect`（装配层从 provenance 派生，规则 glob 由 `foundation::layout` 的架构域派生族给出，`core` 不解释路径语义）；`ProtectRules` SHALL 以 `with_session(Option<Vec<SessionProtectRule>>)` 追加（`load` 的签名与「唯一 IO 点」语义逐字不变）。挂锁格 SHALL 为**两条**规则（模型声明目录 + 契约资产目录），不挂锁格仍为 `None`。
- **规则序**：**内置（`config.json`）→ 会话（相位锁）→ 用户（自选）**，首个命中即定，MUST NOT 拼接多条 reason。会话规则排在用户之前的理由是文案：相位锁串带指路，MUST NOT 被用户自选 reason 盖掉。
- **锁面范围（封闭四项）**：
  | 路径 | 锁 | 理由 |
  |---|---|---|
  | `<archi_dir>/graph/model/**` | ✅ | 它就是「模型」 |
  | `<archi_dir>/models/**`（契约资产 md / schema） | ✅ | 契约是**手写真相**，`apiMissing` / `apiSignatureMismatch` 是唯二 `error` 档判据 ⇒「改契约迎合代码」必须结构上不可达，否则 `error` 档名不副实。**代价如实记账**：实现相位一旦发现契约写错或不可实现，判据在 `implement` 内**不再可自愈**——唯一出路是 `backtrack` 回 `dev-design` 修订（或请用户改用架构页会话）；这条「飘到 backtrack 为止」是本次政策的显式选择，不是遗漏 |
  | `<archi_dir>/graph/INDEX.toml` | ❌ | 生成物，「MUST NOT 手改」已足够；生成归 `archi_generate` 与 IPC 生成命令 |
  | `<archi_dir>/graph/model` 之外的 `<archi_dir>` 其它子树（`.i.rs` / `.i.ts` 生成物、`config.json`） | ❌ | 生成物与看护配置不是契约；生成物由专用工具写、配置不是模型 |
- **判定的工具面覆盖（本变更扩面）**：本相位锁与用户规则 SHALL 对**全部写模型 / 写契约入口**生效——`write` / `edit` 按入参 `path` 判定、`bash` 按写入面提取判定（既有三路），两个 agent 模型写工具（`archi_element` / `archi_relation`）SHALL 在**落点解析之后**以解析出的目标文件过**同一** `ProtectRules`，生成物写工具（`archi_generate`）SHALL 在**写盘之前**把**每一份**应生成物过同一 `ProtectRules`（同一规则序、同一首命中即拒、同一拒串族；MUST NOT 另立规则集、MUST NOT 另写文案）。落点只有 `plan_*` 之后才知道、应生成集只有 `workspace_render` 之后才知道，故判定点可在执行体内（实现选择归 `desktop-archi-agent`）；MUST NOT 因为「工具没有 `path` 入参」而把它判在保护射程之外。
- **fail-closed 方向**：`source_ref` 畸形 / 短段 / 未知 phase / 非 change 与 archi 来源 → **挂锁**（与工具面「零 archi 工具」同方向）。装配层 MUST NOT 因解析失败而放行写权。
- **拒绝文案 SHALL 分两份（本变更新增第二份）**：模型目录与契约目录的拒串 MUST NOT 共用一份——模型串通篇说「架构模型 / 元素与关系」，拿它拒 `.schema.json` 会三重误导（说错了对象、说错了动作、给错了出路）。
  - **模型串**（既有 `SESSION_LOCK_TEMPLATE`，逐字不变）：四要素——① 该路径受相位锁保护并携路径与工具名（`%t`）；② 架构模型只在 `proposal` / `dev-design` 相位的执行侧（及架构页会话）可写；③ 补救路径——把需求写进 `design.md` / `tasks.md` 由 `dev-design` 相位同步模型，或请用户改用架构页会话直接维护；④ **不要改用其他命令或工具尝试绕过**（写入路径均走同一闸）。
  - **契约串**（新增 `CONTRACT_LOCK_TEMPLATE`，单点定义在 `sdk/protect/reason.rs`）：① 该路径受相位锁保护并携路径与工具名（`%t`）；② 接口定义（`.schema.json`）与模块文档（`.md`）只在 `proposal` / `dev-design` 相位的执行侧（以及架构页会话）写入，实现相位只实现契约、不改契约；③ **三条出路写死**——代码与契约不符时先确认哪一方对：契约对就**改代码追上它**；契约确实不可实现或写错，就停下来说明冲突、由用户决定走 `backtrack` 回 `dev-design` 修订；或请用户改用架构页会话直接维护；④ **不要改用其他命令或工具尝试绕过**（所有写入路径均被拦截）。③ 不是修辞：被拒的模型会自然换路重试（`write` → 重定向 → `cp` → `node -e`），不给具体出路就是被拒进死循环。
- **与架构页通路零冲突**：`source = "archi"` 的会话 MUST NOT 挂会话锁（通路 B 的可写性是既有承诺，见 `desktop-archi-agent`「模型 = 源码（两条写入口与写纪律）」）。
- **不是安全边界（如实留痕）**：会话锁与内置锁同一口径——`bash` 内容级写入检测是启发式，`git stash` / `git clean` 等**批量动作**不按相位判别（`BulkGlobs` 与相位无关）；`git checkout <ref> -- <path>`、重定向 / `tee` / node-inline 等**单路径**形态已被会话规则覆盖。该缺口 MUST NOT 被表述为「模型目录或契约目录在非白名单相位不可改」。

#### Scenario: 白名单相位写模型成功

- **WHEN** 以 `source = "change"`、`source_ref = <id>/dev-design/executor/2` 发起会话并以 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** `evaluate` 返回 `None`（会话规则缺席），写入照常执行；内置锁面仍只有工作区 `config.json` 一条

#### Scenario: 非白名单相位被拒且文案指路

- **WHEN** 以 `<id>/implement/executor/1` / `<id>/code-review/evaluator/1` / `<id>/dev-design/evaluator/1` / `source = "explore"` / `source = "debug"` 发起会话并 `write` 模型文件
- **THEN** 一律拒绝；拒绝串含路径、工具名、「只在 `proposal` / `dev-design` 执行侧可写」、「写进 `design.md` / `tasks.md` 或改用架构页会话」与「不要改用其他命令或工具尝试绕过」四要素（取**模型串**）；`SystemNotice.payload.reason` 与 is_error `ToolResult.content` 为同一串

#### Scenario: 写工具在保护闸内且档位／成员校验在其前

- **WHEN** 在非白名单相位（`<id>/implement/executor/1`）以 `archi_element` 尝试写模型；另在白名单相位以同一工具写命中用户规则的落点
- **THEN** 前者零下发（成员校验先拒），执行体零调用；后者被用户规则拒绝（同族拒串、`%t` = `archi_element`），盘上字节零变化。两例均 MUST NOT 出现「工具无 `path` 入参 ⇒ 不受保护」的分支

#### Scenario: 契约资产入锁、生成物与配置不锁（本变更反转）

- **WHEN** 以 `<id>/implement/executor/1` 会话分别写 `<archi_dir>/models/<分层…>/<叶段>.md`、同目录 `<叶段>.schema.json`、`<archi_dir>/graph/INDEX.toml`、`.i.rs` / `.i.ts`、`<archi_dir>/config.json`
- **THEN** 前两者**一律拒绝**（拒串取**契约串**：携路径与 `%t`、说明「实现相位只实现契约、不改契约」、给出三条出路、「不要改用其他命令或工具尝试绕过」）；后三者放行；模型目录同轮的写入仍被拒（模型串）。两串 MUST NOT 互换、MUST NOT 拼接

#### Scenario: 挂锁格恰两条规则且文案各自单源

- **WHEN** 审查非白名单相位的 `SessionInjections.protect` 与 `sdk/protect/reason.rs`
- **THEN** 规则恰两条（`graph/model/**` + `models/**` 的派生 glob），`reason` 分别取 `session_lock_template()` 与 `contract_lock_template()`；两条模板互不相同且各自只定义一处；`load` 的签名与唯一 IO 点语义不变

#### Scenario: 生成物写工具逐份判定且整批拒绝

- **WHEN** 在白名单相位、用户规则 glob 了某一份应生成物时调用 `archi_generate`
- **THEN** 该份命中即整批拒绝（其余目标零写盘），响应的 `conflicts` 携带该路径与 reason（取用户规则文案）；盘上字节零变化。同一工具在无命中时正常写盘——射程条款 MUST NOT 因「工具无 `path` 入参」被绕过

#### Scenario: 畸形 source_ref 挂锁

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别，随后写模型文件
- **THEN** 拒绝（fail-closed，两条规则同时在位）；装配层零 `Err`、零放行写权

#### Scenario: 单路径绕过面已被覆盖

- **WHEN** 非白名单相位以 `bash` 执行 `echo x > <archi_dir>/graph/model/03-x.toml`、`tee <…>/model.toml`、`node -e "require('fs').writeFileSync('<…>/03-x.toml','')"`、`git checkout HEAD -- <…>/graph/model/03-x.toml`；以及 `echo '{}' > <archi_dir>/models/<分层…>/<叶段>.schema.json`
- **THEN** 五条均被会话规则拒绝（写入面提取与批量还原的 write 三态同源；契约目录一条取契约串）；`git stash` 一类批量动作仍未覆盖（知性缺口留痕，见「不是安全边界」）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/infra/agent/src/compose.rs`（相位锁）

| 项 | 变化 | 描述 |
|---|---|---|
| `model_write_lock` | 改 | 政策与不挂锁格逐字不变；`Some` 分支由「恰一条」改**恰两条**：模型声明目录（`**/{archi_graph_model_rel_dir}/**`，reason = `session_lock_template()`）+ 契约资产目录（`**/{archi_models_rel_dir}/**`，reason = `contract_lock_template()`）；两条 glob 均由 `foundation::layout` 派生，零新路径字面量；成因注释整段改写（去掉「改代码相位必须能同步契约」的旧理由，如实写「判据在 implement 内不再可自愈」的代价） |

### Function: `crates/infra/agent/src/sdk/protect/reason.rs`（拒串）

| 项 | 变化 | 描述 |
|---|---|---|
| `CONTRACT_LOCK_TEMPLATE` / `contract_lock_template()` | 新增 | 契约资产专用拒串（四要素，见上）；`%s` / `%t` 全量替换；MUST NOT 复用 `SESSION_LOCK_TEMPLATE`、MUST NOT 在装配层再抄一份文案 |
| 既有三类串（内置 / 会话模型 / 用户无 reason 回落） | 零改动 | 逐字不变；`ReasonKind::Session` 的模型串仍是模型目录的唯一来源 |

### Function: `crates/infra/agent/src/sdk/archi_tool.rs`（射程）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_generate` 臂 | 新增 | 写盘前逐份过 `ProtectRules`（零第二份规则集、零第二份拒串）；任一份命中 ⇒ `{ written: [], unchanged: [], conflicts: [...] }`，零写盘 |

### Component: 内置锁面与判定面（零改动）

| 项 | 描述 |
|---|---|
| `sdk/protect/rules.rs` 内置清单 | 仍恰工作区 `config.json` 一条；`graph/model/**`、`models/**` 与架构域配置 MUST NOT 进内置面（相位无关的锁会同时掐断两条通路） |
| `sdk/protect/{mod,rules,shell,inline}.rs` 判定面 | 规则序 / glob 方言 / 写面提取 / 三态回灌逐字不变；本变更只多一条**会话规则**与一份**模板** |
