# Desktop.Components.Agent.AgentError

## 职能

会话失败横幅的投影：把「最新一轮是否失败 + 底层错误原文」折成展示载荷（友好标题 /
可操作提示 / 原始错误三段），供 `AgentSessionView` 的失败横幅呈现。

**边界**：纯函数面——零 React、零 IPC、零读盘。轮行（`TurnSummary`）是库态权威，事件流
只是「终态行尚未到达」时的兜底；本模块只**投影**两者，不判活、不重试、不改会话状态。

## 不变量

- **成功 / 停止 / 已恢复的失败一律返回 `null`**：判据是「最新轮状态为 `failed`」，或
  「最新轮仍在 `running` 且其后的 `turnDone` 已 `isError`」。已恢复的 `api_retry` 不算失败
  ——载荷解析只认 `systemNotice.subtype === 'api_error'`。
- **兜底 `turnDone` 不得跨轮泄漏**：流刚结束、终态行尚未落库时才用最后一条失败
  `turnDone`，且只在该 done 的 `timestampMs` 不早于最新轮 `startedAt` 时认账；无任何轮行
  时以 `since = 0` 取最近一条失败事件。
- **成因优先级单值**：`api_error` 载荷 → 轮行 `error` → `turnDone.subtype`，三段取首个非空。
- **友好文案表序即优先级**（`ERROR_RULES` 顺序匹配，小写化后比对），未命中落兜底文案
  （「Agent 运行失败」+ 引导展开原始错误）；已知档覆盖：非法 URI / HTML 端点 / 401 / 402 /
  429 / 404 / max_tokens / 拒答 / 未识别终局原因 / 超时。
- **`raw` 恒为原始错误原文**：文案表只产 `title` / `hint`，MUST NOT 用友好措辞替换 `raw`
  （折叠诊断是用户报告问题的唯一证据）。

## 承载理由

文案表与判定逻辑同处一个元素而非拆两处：`describeAgentError` 是**纯私有**函数（只被
`latestAgentFailure` 消费），拆出去即多一个「谁改文案表」的公共面；而它对外的价值只是
`latestAgentFailure` 结果里的两段字符串。

## 契约面留白

- **失败投影函数不进契约**：两侧归一恒不等——契约侧 `$ref` 归一为 `?<名>`（`$ref` 在
  `schema_type` 里落 `Unknown`），代码侧是具名类型、且只有返回位可空。实测差异的形态恒为：
  契约侧每个具名位都带 `?`、列表元素位也不剥，代码侧 `list<T>` 的元素不带可空。补齐的出路是给
  契约词汇补「具名类型位」与「可空返回位」（`$ref` → `Named`、`| null` → `optional`），不是
  放宽判据、也不是把三处类型降级成 `object`。
- **展示载荷类型不进契约**：它本身是**能自洽表达**的类型（三个必填 `string` 字段，零外部引用），
  实测也通过 `apiMissing` 与 `apiSignatureMismatch`。留白的原因是生成链：
  `emit::ts` 在文件末尾多出一个空行，`vp check` 的格式化器剥掉它之后生成物检查（`archi_api` 的
  `check`）即判 `mismatched: …/agent-error.i.ts`——`client:check` 与生成物检查只能二选一。出路在
  源码/配置面（生成器去掉末尾空行，或把 `src/**/*.i.ts` 纳入 `NO_STATIC_CHECK`），属独立的
  生成链接线变更；接线后本元素的第一条 `$defs` 就该是它。

## 类型

本元素自有 `$defs` 为空（成因见上）：契约侧不新建类型。
