# Desktop.Components.Agent.AgentBlocks

## 职能

消息块与工具块的呈现：`message-cards/blocks.tsx` 提供 1 个部件判定（`isToolPart`）与 5 个
`memo` 块组件，覆盖文本 / 思考 / 工具成对卡 / 网络搜索 / 收口检查清单五类消息部件。

**边界**：块组件只吃已解析好的 props（文本、引用、工具入参/结果），零会话取数、零 IPC、
零流订阅；部件从消息到块的**分派**在 `AgentMessages`，五类块中两类的注册项在 `ToolCards`
的注册表里（`web_search` / `finish_step`）。

## 公共面

| 名字 | 形态 | 语义 |
|---|---|---|
| `isToolPart` | 判定 | 消息部件 → tool 部件收窄（`tool-<name>` 命名，显式排除 `dynamic-tool`） |
| `TextBlock` | 块组件 | 文本部件（markdown + 引用列表） |
| `ThinkingBlock` | 块组件 | 思考部件（默认折叠，流式态呈 spinner） |
| `ToolPairCard` | 块组件 | 工具调用的 input / 同 id output 成对呈现（默认折叠，`isError` 高亮） |
| `WebSearchCard` | 块组件 | 云端网络搜索卡（summary 恒为「网络搜索」） |
| `FinishStepCard` | 块组件 | 收口检查清单卡（evaluator 形态，逐项 pass / fail） |

## 不变量

- **`isToolPart` 是部件收窄的唯一判据**：判据是 `type` 以 `tool-` 起始**且**不等于
  `dynamic-tool`（动态工具不在消息信封内）；MUST NOT 在别处另写一份前缀判定。
- **外链只有一个调用点**（`ExternalLink`）：仅 `http:` / `https:` 目标渲染为可点击 `<a>`
  并经壳侧 opener 打开；其余 scheme（`javascript:` / `file:` / `mailto:` 等）降级纯文本、
  不触达插件。打开失败（scope 拒绝 / 无默认浏览器）以 `.catch` 吞掉——不打断呈现、不改用
  其他打开通道。
- **markdown 插件数组是模块级稳定引用**（`REMARK_PLUGINS`）：`react-markdown` 每次渲染都会
  重建 processor，插件数组每拍新建即每拍重算。
- **`TextBlock` 的比较器只认两轴**：`text` 与引用集合（逐条 `url` / 标题同序同值）；其余
  props 变化不触发重渲——流式增量因此只重渲正在长的那一块。
- **块组件不认轮状态词表**：`turnRunning` 一类判据由消费方给出，块内只按 `state` / `output`
  呈现；未配对的运行态判据不在本元素（单点在 `ToolCards`）。
- **未知形态一律回退原样**：入参解析走 `asRecord` / `textOf` 的宽容读法（非对象 / 非字符串
  → 缺席），MUST NOT 因一个坏载荷抛错（一条坏消息不该让整段会话白屏）。

## 契约面留白

- **5 个块不进 `api-types`**：它们是 `export const … = memo(…)` 的组件（取数器按组件形态登记
  为非契约类型），其形状是 React 组件而非可序列化的值形态，`memo` 比较器也表达不了。
  `apiUndeclared` 是 `off` 档，本元素不为凑齐 `$defs` 而给组件编形状。
- **`isToolPart` 不进 `api-op`**：入参是索引访问类型 `AgentUIMessage['parts'][number]`、返回是
  类型谓词 `part is AgentToolPart`，代码侧归一为 `fn(?) -> ?`（`Unknown` 空原文）。契约侧唯一
  能与它相等的写法是「两个位置都给 `{}`」（归一 `fn(?) -> ?`）——那等于用「什么都不声明」换
  「操作存在」一条判据；本期连它一并留白，成因是下一条。
- **前端 `.i.ts` 生成链与格式化不动点尚未对齐**（实测，本元素与 `AgentError` /
  `AgentMessages` 同批触发）：`emit::ts` 的段落分隔在文件末尾多出一个空行，`vp check` 的
  格式化器会把它剥掉，剥掉之后 `api:check` 的零 diff 门即判
  `mismatched: packages/desktop/src/components/agent/message-cards/blocks.i.ts`。两条出路都在
  源码/配置面（生成器去掉末尾空行，或把 `src/**/*.i.ts` 纳入 `vite.config.ts` 的
  `NO_STATIC_CHECK`），属独立的生成链接线变更；在那之前，前端的任何声明都会让
  `client:check` 与 `api:check` 二选一。故本元素 `$defs` 为空、操作面留白如上。

## 类型

本元素自有 `$defs` 为空（无值形态的类型承诺，见上）：

```json api-types
{}
```
