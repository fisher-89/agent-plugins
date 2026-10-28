# desktop-write-protection Specification（delta: archi-page-agent-window）

## MODIFIED Requirements

### Requirement: 架构域路径不在内置锁面

架构模型文件 SHALL 可被 agent 在 change 的 worktree 内、以及架构页通用会话（`desktop-archi-chat`，执行根 = 主仓根）下自由修改（模型 = 源码，见 `desktop-archi-agent`「模型 = 源码（写纪律与不设专用写工具）」的两条通路）。因此：

- **内置锁面 MUST NOT 纳入架构域路径**：`<archi_dir>/models/**` 与架构域配置文件 MUST NOT 进入「保护规则装配」的内置锁定清单。理由：内置规则是**绝对路径 glob**（`load` → `match_path`，cwd 仅用于 bash 路径解析），在 worktree 与主仓两条通路下同样命中——一旦纳入，agent 就改不了模型；而本仓「模型可经 change worktree 随归档回合合回，也可经架构页通用会话直改主仓」的两条通路都依赖"写得进去"。
- **模型文件的真正护栏是回合确认与事后评审**：`protect` 本就不是安全边界（`bash` 可绕），它是防误触护栏；通路 A 的设计权由「回合必须人工确认」承载（归档确认面的架构变更区块，见 `desktop-change-archive`「归档入口与前置确认面」），通路 B 的设计权由事后 `git diff` 评审与手动刷新校验承载（见 `desktop-archi-chat`「写权落点与刷新纪律」），MUST NOT 由写盘拦截承载。
- **用户显式规则的语义不变**：用户若在工作区配置的 `write_protection.files` 中显式 glob 了架构域路径，规则 SHALL 照常生效（首命中即拒）——那是用户的自选护栏，本 requirement 只禁「内置默认锁」。
- **内置面唯一增删纪律不变**：内置清单的任何增删仍须走本能力既有「内置锁定（独立于配置解析结果）」条款；本条为**禁止扩充**的显式裁定，MUST NOT 被解读为给内置面新增条目。
- 与 `desktop-agent-execution` 的既有挂载纪律不冲突：守卫链位置、工作区根注入缝、fail-open / fail-closed 方向全部零改动。

#### Scenario: 内置规则清单不含架构域

- **WHEN** 装配保护规则（`ProtectRules`）并审查内置锁定清单
- **THEN** 清单仍恰为工作区 `config.json` 一条（既有语义逐字不变）；不含 `models/**`、不含架构域配置文件

#### Scenario: worktree 内写模型不被拦截

- **WHEN** agent 在 change 的 worktree 内以 `write` / `edit` 修改 `<archi_dir>/models/*.toml`
- **THEN** `evaluate` 返回 `None`（未命中任何规则），写入照常执行；写入保护不产生任何拒绝串

#### Scenario: 主仓根直写模型不被拦截

- **WHEN** agent 在架构页通用会话（执行根 = 主仓根）内以 `write` / `edit` 修改主仓的 `<archi_dir>/models/*.toml`
- **THEN** `evaluate` 返回 `None`（未命中任何规则），写入照常执行；内置锁面仍只有工作区 `config.json` 一条（两条通路下判定一致）

#### Scenario: 用户显式规则仍生效

- **WHEN** 用户在工作区配置的 `write_protection.files` 里显式 glob 了架构域模型路径，agent 试图写该路径
- **THEN** 按用户规则拒绝（首命中即拒，拒绝串取用户 reason）；MUST NOT 因"模型是源码"而放行
