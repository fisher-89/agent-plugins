# desktop-change-orchestration（delta：「引擎恒 CLI」边界翻案）

## MODIFIED Requirements

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎随默认 agent 解析（SDK 承载）**：change 执行会话经组合根缺省解析（默认 agent）发起——CLI 租户已随 retire-cli-engine 退役，change 执行由 SDK 租户承载。原「引擎恒 CLI（插件 dev-team 天然加载）」依据随桌面相位知识静态化（desktop-phase-prompts：14 个纯静态 prompt、无插件 Agent / MCP 工具假设）与插件废弃失效；原「SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）」禁令同步退役。change run 发起自此依赖 SDK 连接配置（默认 sdk agent + provider），缺失时按既有显式 `Err` 面失败并引导管理页，MUST NOT 静默回退。
2. **工作流类型**：requirement 与 test-only 工作流全链；bug-fix 相位表后续独立 change。
3. **计划图静态编译期、非通用图引擎**：计划图（workflow → phase → step 序列 + 依赖边 + artifact 清单）SHALL 为编译期 const 表 + `include_str!` 静态装配，MUST NOT 实现运行时加载的通用图引擎或图定义文件格式；执行图（节点 + 边）为 db 落库的运行记录，MUST NOT 上位为路由权威（路由仍问 `phase_next`）。原「walker 只硬编码 walk 本相位循环图」表述随本变更翻案显式立项（step 序列下沉为 `PhaseDefinition.steps`）。
4. **file_log 载体退役**：desktop run 与视图全链零 file_log 触点（git diff 供变更上下文）为既定裁定；外部归档对账（openspec CLI 归档改名后的 db status 翻转时机）留 design 定稿（见 proposal 待决问题）。
5. **protect-files 护栏缺口（原过渡依赖失效）**：步前实时护栏原过渡期依赖插件 hook（仅 CLI 会话内生效）；CLI 租户退役后该路径失去载体，缺口自本变更起**显式打开**——写保护面仅剩 SDK 路径沙箱（workspace root 前缀校验）。桌面原生护栏（内核 permission 层方向）的独立 change 为缺口出口（插件真下线前落地），本条更新为缺口显式留痕，不由实现隐式吸收。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕；原「file_log 不记录」边界随本变更升级为载体退役，不再是单侧不写。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含计划图静态编译期翻案留痕与护栏缺口显式留痕），后续变更无需重新论证是否知情

#### Scenario: 引擎边界生效

- **WHEN** 审查 walker 发起 WorkerAgent 会话的引擎解析路径
- **THEN** 走组合根缺省解析（默认 agent）构造引擎，SDK 租户承载、无 CLI 分支；默认 agent 缺失或 SDK 连接配置不齐时显式 `Err`（管理页引导），change run 发起依赖 SDK 连接配置

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration`（不改） | 薄图 walker | 零引擎分支现状保持（`compose_turn(..., None)` 默认解析）；本 delta 仅契约措辞翻案，代码面零改动 |
| 既有 hooks 行（护栏口径更新） | 步前护栏 | 「protect-files 原样生效（过渡期）」表述随 CLI 租户退役失效——插件 hook 路径无载体，缺口显式留痕（见边界 #5），桌面原生护栏归独立 change |
