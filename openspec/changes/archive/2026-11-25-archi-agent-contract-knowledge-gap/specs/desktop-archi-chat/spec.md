# desktop-archi-chat Specification（delta: archi-agent-contract-knowledge-gap）

## Purpose

本 delta 只改架构页会话的**写面表述**：会话工具面从「七工具 + 五 archi 只读」变为「七工具 + 五只读 + 两写」，而该能力里两句已固化的措辞（「两 archi 只读工具」「无专用写工具…写入不校验、读取报错」）必须同批改写——它们与写工具面直接冲突。

- **零行为变化**：画布仍零写入口、写盘后仍不自动刷新、零自动提交、档位（恒 `bypassPermissions`）与面板 / 命令面逐字不变。
- **失败语义拆两半**：写工具面「坏文本不可产出」（自证链），手改面「写坏模型仍由读取面报错」——本能力此前把两者并成一句「写入不校验，读取报错」，改写后 MUST NOT 再混说。

## MODIFIED Requirements

### Requirement: 写权落点与刷新纪律

架构页会话的工具面 SHALL 为七通用工具（含 `write` / `edit` / `bash`）叠加注入矩阵命中的 archi 工具（五只读 + 两写，名单与矩阵见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）：**写权落在会话里，画布仍零写入口**。

- **画布不参与**：`mode = "full"` 的画布 MUST NOT 因本能力新增任何可提交的 `gate` 编辑入口或写命令（见 `desktop-archi-canvas`「能力差异」的消歧条款）；MUST NOT 以「会话能写」为理由放宽画布，也 MUST NOT 以「画布只读」为理由禁会话写。
- **直改主仓（写面两个入口）**：agent 写模型 SHALL 直改主仓 `<archi_dir>/graph/model/*.toml`（无 worktree、无归档确认回合）：**canonical 入口是两个写工具**（`archi_element` / `archi_relation`，坏文本不可产出——渲染自证不通过即 `Err` 且零落盘），**second path 是 `write` / `edit` 手改**（工具表达不了的才用，写坏模型仍是合法状态、由读取面报错）。两条失败语义 MUST NOT 混为一句「写入不校验」；写权通路与写入口的完整口径见 `desktop-archi-agent`「模型 = 源码（两条写入口与写纪律）」。
- **可见性**：agent 写盘后画布 MUST NOT 自动更新（零 watch 零轮询的必然推论）；用户点 `archi-refresh` 后 SHALL 见到新图，模型被写坏时 SHALL 见到停摆态诊断（`file:line` 逐条，与 agent 读取面同一份 `archi::dto` 投影）。该语义 SHALL 在面板状态行提示，MUST NOT 被当作缺陷处理。
- **零自动提交**：会话 MUST NOT 在轮收口或写盘后自动提交 / 自动归档 / 自动 `git add`；改动一律经用户 `git diff` 评审后自行处置。
- **档位**：composer 不呈档位行（`AgentInput` 缺省 `hidePermissionMode = true`），发送恒携缺省 `bypassPermissions`（与既有 `AgentInput` 一致）；静态权限三档语义零改动（`default` 档仍拒绝 `write` / `edit` / `bash` **与两写工具**），档位选择仅在显式 `hidePermissionMode = false` 的消费点可用。

#### Scenario: 直改主仓模型成功（写工具入口）

- **WHEN** 在架构面板会话中要求 agent 新增一个元素（agent 用 `archi_element` 写入既有模型文件）
- **THEN** 写入落在主仓该文件（工作区其他会话随即可读），响应含 `changed` / `counts` / `hash`，会话行与事件转录正常收口；画布在用户点刷新前保持旧图

#### Scenario: 手改入口仍可用

- **WHEN** 在架构面板会话中要求 agent 新建一个模型文件 `<archi_dir>/graph/model/09-billing.toml`（新建文件用 `archi_element` 的 `file` + `create`，或直接 `write`）
- **THEN** 两条入口都可写入主仓该文件；用 `write` 手改写坏模型时写盘仍成功、随后读取面与画布呈停摆态诊断（MUST NOT 出现「写盘前被拦」的表述）

#### Scenario: 写后刷新才可见

- **WHEN** agent 已写盘但用户未点 `archi-refresh`
- **THEN** 画布不变化（无自动重取）；点刷新后呈现新图；若模型被写坏则呈 `StructureErrorPanel` 的逐条诊断

#### Scenario: 画布仍只读

- **WHEN** 在 `/architecture` 审查元素信息面板与页面源码
- **THEN** 无任何可提交的 `gate` 编辑控件、零写命令调用；面板会话的存在不改变该断言（写工具是 agent 工具，不是画布入口）

#### Scenario: 零自动提交

- **WHEN** 会话收口（`running` 由 true 变 false）
- **THEN** 无自动 `git add` / `commit` / 归档动作；工作区改动保持未提交状态等待用户评审

## Module Contract

### Component: `packages/desktop/src/views/architecture/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchitectureView` / `ArchiChatPanel` / `ArchiChatComposer` / `useArchiChat` | 零改动 | 布局、会话归属（`source = "archi"` / `sourceRef = null` / `cwd = null`）、状态行文案、禁用判据与档位全部逐字不变 |

### API: agent 侧（归属 `desktop-archi-agent`）

| 项 | 变化 | 描述 |
|---|---|---|
| archi 臂工具面 | 改（登记侧见 `desktop-archi-agent` 与 `desktop-agent-execution`） | 七内建全量 + 七 archi 工具（读五 + 写两）；`preamble` / `before_done` / `ask` 三位不变 |
| 写盘可见性 | 零改动 | 零 watch、零自动重取、零自动提交 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `list_archi_chat_sessions` 等命令 | 签名与语义逐字不变 |
| 画布 / 元素信息面板 | 仍是只读视图（`mode = "full"` 零写入口） |
