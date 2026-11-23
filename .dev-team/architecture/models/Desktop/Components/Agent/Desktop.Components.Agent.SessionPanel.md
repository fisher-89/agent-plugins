# Desktop.Components.Agent.SessionPanel

## 职能

会话面板骨架：折叠窄条 ⇄ 展开态（头区动作 / 状态行 / 历史清单 / 会话体 + composer 插槽）
可拖拽分栏骨架，供两处会话面（架构页变更对话、探索页对话）共用。

**边界**：骨架只排布与呈现，**零业务取数**——会话面、历史清单、动作面判据与状态行文案全在
`SessionPanelState` 里由来源侧 hook 持有；骨架内唯一的「业务」决定是折叠几何与 testid 命名。

## 公共面

| 名字 | 形态 | 语义 |
|---|---|---|
| `SessionPanel` | 组件 | 可折叠面板骨架（窄条 / 展开两态） |
| `SessionPanelState` | 接口 | 面板公共状态子集：会话面 + 历史清单面 + 动作面判据 + 状态行文案 |

`SessionPanelProps`（`state` / `title` / `testIdPrefix` / `composer`）**不外露**：入参形状是骨架
内部事实，跨元素消费者只经本元素入口取 `SessionPanel` 与 `SessionPanelState`。

## 不变量

- **折叠判据单值 `MIN_SIZE = 500`**：`onResize` 以 `inPixels < MIN_SIZE` **单向**写
  `isCollapsed`，展开 / 收起两个回调同时写几何与本地态；`defaultSize` / `collapsedSize` 为
  32px，`collapsedThreshold` 为半宽。MUST NOT 另立第二条折叠判据。
- **testid 前缀零写死**：骨架内一切 `data-testid` 经 `testId(prefix, name)` 由消费方给的前缀
  派生（`panel` / `rail` / `expand` / `collapse` / `actions` / `new` / `history` / `note` /
  `session` …），同一个骨架在两处消费点因此各有自己的前缀。
- **判断权在 state，不在骨架**：「新会话」可用性取 `state.canStart` 单源；「历史 ▾」恒可用且
  展开即 `state.onOpenHistory()`；状态行只在 `state.note !== null` 时占位。
- **历史清单四分支**：加载中（且零条）→「加载中…」；`listError` 非空 → 错误文案；空清单 →
  「（暂无历史会话）」；否则按 `state.sessions` 呈单选清单，且仅当
  `sessions.length < total` 才出现「加载更多」（分页累加，`loading` 时禁用）。
- **骨架自接 ask 动作面**：展开态把 `askAnswer = { onAnswer: state.session.answer }` 交给会话
  体——两处消费点的 hook 与面板零改动即得可应答活卡。
- **窄条不是空态**：收起态仍留竖向标题与展开入口（`aria-label` 齐备），MUST NOT 渲染空白。

## 契约面留白

- **`SessionPanel` 不进 `api-op`**：形态是 `(props: SessionPanelProps) => React.JSX.Element`
  ——入参具名类型、返回位是 JSX 元素，两侧归一恒不等，同类实测差异见
  `Desktop.Components.Agent.AgentError` 的留白段。
- **`SessionPanelState` 不进 `api-types`**：十个字段位里四个引用外部类型——`UseAgentSessionState`
  （`hooks/use-agent-session`）、`SessionSummary[]`（生成的绑定类型）与三个函数位。契约词汇没有
  跨模块具名类型位；把外部类型就地重抄即造第二个真相源，故本元素 `$defs` 为空。
- **`SessionPanelProps` 本就不在导出集**：代码未 `export`，契约声明它会立刻触发 `apiMissing`
  （「有约定无兑现」）——名字不外露这件事由入口的导出面守，不靠契约。

## 类型

本元素自有 `$defs` 为空（状态面形状全在留白里，见上）：

```json api-types
{}
```
