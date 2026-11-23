# Desktop.Components.Agent.AgentSessionView

## 职能

整段会话视图：元信息行（会话 id / 运行徽章 / 轮数 / tokens）+ 失败横幅 + 消息透镜 +
composer 插槽。以**同一渲染体**承载两种形态——自持形态（给 `root` + `sessionId`，内部建流）
与注入形态（给 `session`，用来源侧已建的 hook 状态）。

**边界**：单会话、单渲染体。取数与流订阅全在 `hooks/use-agent-session`（非本元素），失败投影
在 `AgentError`，消息与工具卡的呈现归 `AgentMessages` / `AgentBlocks` / `ToolCards`；本组件只
决定「谁的状态面在场」并在缺会话时给占位。

## 公共面

| 名字 | 形态 | 语义 |
|---|---|---|
| `AgentSessionView` | 组件 | 会话视图（双形态） |
| `AgentSessionActions` | 接口 | composer 插槽的运行态 + 动作面（`running` / `loading` / `send` / `stop` / `steer` / `steering`） |
| `AgentSessionViewProps` | 接口 | 入参：`root` / `sessionId` / `refreshKey` / `session` / `composer` / `askAnswer` / `data-testid` |

## 不变量

- **形态判据单值**：注入形态 ⟺ `session !== undefined`。注入时内部 hook 以
  `sessionId = null` **空载**——零取数、零订阅、零第二条流；自持形态才把 `root` / `sessionId` /
  `refreshKey` 交给 hook。两条形态共用同一渲染体与同一 hook，MUST NOT 分叉成两个组件。
- **无会话即占位**：自持形态且 `sessionId` 为 `null` 时呈空态占位，零取数零订阅——槽位缺席
  ≠ 虚构会话。
- **失败横幅单点**：`latestAgentFailure(state.events, state.chain)` 经 `useMemo` 按两轴缓存；
  横幅只在有值时出现，原始错误以 `<details>` 折叠。
- **动作面在插槽外组装**：`composer(actions)` 的 `actions` 由本组件从状态面组装（含 `steer` /
  `steering` 两个 additive 加位），插槽自足取用；不接的消费点零影响。
- **`askAnswer` 原样下行、不自动接线**：与 composer 插槽互不前置——只读消费点可只接它。
- **testid 缺省 `agent-session-view`**：四个消费面各有自己的前缀，组件只提供缺省值。

## 契约面留白

- **`AgentSessionView` 不进 `api-op`**：形态是 `(props: AgentSessionViewProps) =>
  React.JSX.Element`——入参具名类型、返回位是 JSX 元素，两侧归一恒不等（`apiSignatureMismatch`
  恒红），同类实测差异见 `Desktop.Components.Agent.AgentError` 的留白段。
- **两个导出类型都不进 `api-types`**：`AgentSessionActions` 的三个函数位引用
  `AgentSessionSendInput`（`hooks/use-agent-session`）；`AgentSessionViewProps` 更是
  `UseAgentSessionState`、`React.ReactNode`、`SessionSummary`（生成的绑定类型）与自有
  `AskAnswerActions` 的合成。契约词汇没有跨模块具名类型位，就地重抄这些类型即造第二个真相源，
  故本元素 `$defs` 为空。

## 类型

本元素自有 `$defs` 为空（入参 / 动作面形状全在留白里，见上）：

```json api-types
{}
```
