# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎随默认 agent 解析（SDK 承载）**：change 执行会话经组合根缺省解析（默认 agent）发起——CLI 租户已随 retire-cli-engine 退役，change 执行由 SDK 租户承载。原「引擎恒 CLI」依据随桌面相位知识静态化（desktop-phase-prompts：16 个纯静态 prompt、MUST NOT 携带插件 Agent / MCP 工具假设——插件已退役，该条为防回退护栏）与插件整体退役失效；原「SDK 引擎 MUST NOT 承载 change 执行」禁令同步退役。change run 发起自此依赖 SDK 连接配置（默认 sdk agent + provider），缺失时按既有显式 `Err` 面失败并引导管理页，MUST NOT 静默回退。
2. **工作流类型**：requirement 与 test-only 工作流全链；bug-fix 相位表后续独立 change。
3. **计划图静态编译期、非通用图引擎**：计划图（workflow → phase → step 序列 + 依赖边 + artifact 清单）SHALL 为编译期 const 表 + `include_str!` 静态装配，MUST NOT 实现运行时加载的通用图引擎或图定义文件格式；执行图（节点 + 边）为 db 落库的运行记录，MUST NOT 上位为路由权威（路由仍问 `phase_next`）。原「walker 只硬编码 walk 本相位循环图」表述随本变更翻案显式立项（step 序列下沉为 `PhaseDefinition.steps`）。
4. **file_log 载体退役**：desktop run 与视图全链零 file_log 触点（git diff 供变更上下文）为既定裁定；外部归档对账（openspec CLI 归档改名后的 db status 翻转时机）留 design 定稿（见 proposal 待决问题）。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕；原「file_log 不记录」边界随本变更升级为载体退役，不再是单侧不写；原「protect-files 护栏缺口」边界随桌面原生写入保护落地（`crates/infra/agent/src/sdk/protect/`，loop 守卫链第四段，见 desktop-write-protection）而偿还，按同款惯例**不再留痕**——已偿还的边界留在清单里只会稀释真边界，防回退职责归 desktop-write-protection 的不变量，与引擎形态无关。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 四条边界均可考（含计划图静态编译期翻案留痕），后续变更无需重新论证是否知情

#### Scenario: 已偿还边界不留痕

- **WHEN** 审查四条边界清单
- **THEN** 清单不含已偿还项（护栏缺口 / sessionAnchors 精度 / sweep-phase / file_log 不记录）
- **AND** 写入保护的不变量只在 desktop-write-protection 承载，不由本清单重复声明

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.37` 升级为 `0.4.38`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成（归档时执行版本升级）
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.38，`src-tauri/Cargo.toml` 版本不随动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs` | 相位机 walker | 四条边界口径；引擎走组合根缺省解析；写入保护护栏由 SDK 内 protect 层承载（不在 walker） |
| `crates/core/orchestration/src/prompt.rs`（不改） | prompt 组装 | 模板取自 core/workflow 相位表单源（16 份静态 md） |
| 既有 hooks（复用，不改） | 步前护栏与无外围账 | **（原「protect-files 原样生效（过渡期）」表述整行删除）**：插件 hook 路径随插件退役无载体；写入保护由 `crates/infra/agent/src/sdk/protect/` 承接（desktop-write-protection）；record-files / sweep-phase 已分别为 `change_edit_set` 与 no-op 取代 |
