# Desktop.Components.Agent.AgentMessages

## 职能

消息列表组装与滚动：把全量消息过成可见会话（system 过滤）、按部件分派到块 / 工具卡，并以
虚拟滚动 + user 消息粘性头呈现；ask 部件在此获得「活卡 / 静态卡」的唯一判定。

**边界**：只吃 `messages` 与一个可选动作面——零取数、零 IPC、零流订阅；消息到块的分派与虚拟
化几何在本元素，块的内部呈现归 `AgentBlocks` / `ToolCards`，user 气泡归同目录的
`message-cards/user-message.tsx`（未单列元素，随本元素）。

## 不变量

- **system 消息在组件内过滤**：入参是全量转录，`filterConversation` 是唯一过滤点；调用方 MUST
  NOT 预过滤（两处过滤必然在「谁的 system 算内部」上分叉）。
- **虚拟器重置判据是会话身份**：`resetKey = conversationIdentity(conversation)`——同一会话的
  增量只重算几何，换会话才重置滚动位置。
- **几何只经虚拟器表达**：行距走 `gap`、纵向内边距走 `paddingStart` / `paddingEnd`，内容容器
  MUST NOT 再用 flex `gap` 表达同一事实（两处表达必然漂移）；首帧估值 72px（user）/ 240px
  （assistant）在首次测量后即校正，overscan 8。
- **ask 三态判活矩阵**（`pendingToolCallId` 的三态各自可辨，MUST NOT 用 `?? null` 归一）：
  `null` = 当前无等待 → 全静态；字符串 = 只有 `toolCallId` 相等的那条是活卡（历史里崩溃残留的
  未配对 `tool_use` 同形，无 id 对账会误呈可应答）；`undefined` = 未提供对账位 → 由该轮是否仍
  在运行判活，`input-available` 只作必要条件。**结果缺席不足以单独判活**——中断 / 终止轮的
  未配对部件同样没有结果。
- **部件分派先注册表后回退**：`tool-<name>` 命中 `BUILD_IN_TOOL_CARDS` → 特化卡，miss →
  `ToolPairCard`；动作面只达 ask 卡（其余卡的渲染与行为零变化）。
- **空态与还原态互斥**：`loading` 且可见会话为空 → 「会话还原中…」；否则空会话呈引导文案。

## 契约面留白

- **组件不进契约**：形态是 `(props: <具名 props>) => React.JSX.Element`——入参具名类型、返回位
  是 JSX 元素，两侧归一恒不等（`apiSignatureMismatch` 恒红），同类实测差异见
  `Desktop.Components.Agent.AgentError` 的留白段。
- **应答动作面类型不进契约**：它本身是**能自洽表达**的类型（一个必填函数位 + 一个可空字符串位，
  零外部引用），实测也通过 `apiMissing`。留白的原因是生成链：`emit::ts` 在文件末尾多出一个空行，
  `vp check` 的格式化器剥掉它之后生成物检查（`archi_api` 的 `check`）即判 `mismatched`——`client:check` 与
  生成物检查只能二选一。出路在源码/配置面（生成器去掉末尾空行，或把 `src/**/*.i.ts` 纳入
  `NO_STATIC_CHECK`），属独立的生成链接线变更；接线后本元素的第一条 `$defs` 就该是它。
- **入参形状不进契约**：三个字段位的类型都在本元素之外——消息信封（`lib/agent-adapter`）与本元素
  自有的应答动作面。前者是外部具名类型（契约词汇无跨模块类型位），把一个引用外部的 props 形状写
  进 `$defs` 只会得到「字段名对、字段类型是 `object`」的伪形状，比不写更坏。

## 类型

本元素自有 `$defs` 为空（成因见上）：契约侧不新建类型。
