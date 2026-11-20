# desktop-change-chat Specification (Delta)

## MODIFIED Requirements

### Requirement: 面板骨架归一（共享 SessionPanel）

change 详情左栏面板的**骨架**（头区动作（含历史下拉）/ 状态行 / 折叠窄条）SHALL 抽为共享组件 `SessionPanel`（`components/agent/session-panel.tsx`，经 `components/agent` 桶导出），并由本能力、架构页通用会话（`desktop-archi-chat`）与配置页常驻 config 会话（`desktop-config-agent`）共同消费——它是**三处唯一**的面板骨架实现，MUST NOT 在任一视图侧另存第二套头区动作 / 历史清单 / 折叠窄条：

- **testid 集合**：`data-testid` 集合（`-actions` / `-collapse` / `-new` / `-history` / `-note` / `-history-list` / `-history-item` / `-history-more` / `-history-empty` / `-history-error` / `-rail` / `-expand` / `-session`）与「头区动作（历史清单在 `-history` 下拉内）→ 状态行 → 会话体」的渲染次序 SHALL 保留；面板根元素由分栏库占用（`data-slot="resizable-panel"`），消费面以 `-rail` / `-actions` 定位面板分支。
- **testid 前缀参数化**：骨架 SHALL 以 `testIdPrefix` 生成其全部查询挂钩（`change-chat` / `archi-chat` / `config-chat` 三套前缀各命中一次），MUST NOT 在骨架内写死单一前缀。
- **业务判据不外移**：可发送 / 可开新会话 / 只读矩阵与禁用理由 SHALL 留在各自来源侧 hook（change 侧仍为 `detail.source` / `activeRun` / 本会话 `running` 三判据），骨架 MUST NOT 自判禁发、MUST NOT 引入第二套判据源。
- **状态行只渲染 `note`**：状态行文案 SHALL 由来源侧 hook 供 `note: string | null`（change 侧 = `disabledReason ??（空态 ? 草案文案 : null）`；`null` = 不占位），骨架 MUST NOT 自行推导文案生成规则。
- **会话流不抽**：骨架 MUST NOT 自建会话流 / 自调 `useAgentSession`——会话状态面由消费者以注入形态传入（既有「对话体单一实现」纪律零变化）。
- **折叠自持**：骨架 SHALL 自持折叠态并自带分栏（`ResizablePanel` + 窄条展开入口 + 头区「收起Agent」），默认收起（收起尺寸 32px、展开下限 500px）；消费者 MUST NOT 传入 `collapsed` / `onToggleCollapsed` 一类的折叠入参，MUST NOT 在视图侧持有第二份折叠 state。

#### Scenario: change 面板经骨架渲染且 testid 集不变

- **WHEN** 打开 change 详情左栏面板并审查渲染树
- **THEN** 头区动作（含历史下拉）/ 状态行 / 折叠窄条来自共享骨架组件，`change-chat-*` testid 集合（含 `-collapse`）与文案逐字符合上文清单；既有行为级用例全绿

#### Scenario: 三处共用同一实现

- **WHEN** 检索 change 详情、架构页与配置页三处面板的实现
- **THEN** 三处均消费同一骨架组件（仅 `testIdPrefix` / 文案 / 注入的状态面不同），仓库内不存在第二份头区动作或历史清单实现

#### Scenario: 骨架不自判禁发

- **WHEN** 审查骨架组件源码
- **THEN** 其中零 `detail.source` / `activeRun` / `canSend` / `disabledReason` 的业务判定，零 `useAgentSession` 调用；判据与 `note` 全部来自传入的状态面

#### Scenario: 折叠自持

- **WHEN** 审查骨架组件与三处消费面的源码
- **THEN** 折叠态与分栏命令面（`collapse()` / `expand()`）全在骨架内部，消费面零折叠入参、零折叠 state；挂载默认呈窄条，窄条展开入口与头区「收起Agent」互逆


## Module Contract

### 模块增量：配置页成为骨架第三处消费者（config-agent-session）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 类型 | 职责 / 契约 |
|------|------|-------------|
| `packages/desktop/src/components/agent/session-panel.tsx` | Component | **零改动**（骨架自持折叠、判据来源、note 渲染面均不变）；新增第三处消费只经 `testIdPrefix = "config-chat"` 参数化命中 |
| `packages/desktop/src/views/config/components/config-chat-panel.tsx`（新，消费方） | Component | 第三处消费者：`SessionPanel`（`testIdPrefix = "config-chat"`）+ `AgentSessionView` 注入形态 + composer 插槽；状态面来自 `useConfigChat`（`source = "config"`），骨架零业务判据不变 |
| `packages/desktop/src/views/config/config-view.tsx` | Component | 左栏挂载该面板（横向分栏），页面不持折叠 state、不传折叠入参 |
