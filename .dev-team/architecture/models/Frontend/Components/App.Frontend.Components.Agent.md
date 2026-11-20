# App.Frontend.Components.Agent

## 职能

agent 会话组件的统一出口：目录入口 `packages/desktop/src/components/agent/index.tsx` 以 7 条
re-export 透出 8 个名字（4 个组件 + 4 个类型），会话消费面（agent 调试页 / 变更右栏 /
变更流程运行记录 / 探索页 / 配置页常驻面板）一律经它取组件。

**边界**：入口只做透传——零实现、零类型定义、零运行期语句。会话状态机在
`src/hooks/use-agent-session`（非本元素），消息与工具卡的呈现各归子元素
（`AgentMessages` / `AgentBlocks` / `ToolCards`），失败投影归 `AgentError`。

## 公共面

| 名字 | 形态 | 来源文件 | 语义 |
|---|---|---|---|
| `AgentInput` | 组件 | `agent-input.tsx` | 会话输入框与提交交互（含 permission-mode 档位、停止入口） |
| `AgentSessionView` | 组件 | `agent-session-view.tsx` | 整段会话视图（自持 / 注入双形态） |
| `SessionPanel` | 组件 | `session-panel.tsx` | 会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条） |
| `ConfigApplyScope` | 组件 | `message-cards/config-apply-scope.tsx` | 提案卡的写回动作面 Provider（root + onApplied） |
| `AskAnswerActions` | 类型 | `agent-messages.tsx` | ask 应答动作面（三态判活的承载类型） |
| `AgentSessionActions` | 类型 | `agent-session-view.tsx` | composer 插槽的运行态 + 动作面 |
| `AgentSessionViewProps` | 类型 | `agent-session-view.tsx` | 会话视图入参 |
| `SessionPanelState` | 类型 | `session-panel.tsx` | 面板公共状态子集（会话面 + 清单面 + 动作面判据 + 状态行） |

## 不变量

- **入口导出面恰等于「跨元素被 import 的名字」集合**：多导一个名字即虚增公共面（`knip` 的
  unused-export 判红），少导一个即断调用点（`tsc` 判红）。MUST NOT 顺手把子元素内部
  `*Props` 一并外露——`SessionPanelProps` 至今不外露即该判据的留痕。
- **入口 MUST NOT 承载实现、类型定义或运行期逻辑**：入口一旦长出实现，「入口 = 公共面」与
  「桶内实现」两件事就混在一处，`api::extract` 的导出闭包随即把内部件当承诺。
- **类型出口一律 `export type`**：`isolatedModules` 下 `export { TypeName } from './x'` 在
  转译期被当成值导出，必须与值出口拆成两条语句。
- **入口 MUST NOT 跨子元素 re-export**：`export … from` 会被 `scan` 记成 import fact，父元素
  的关系集会多出真边（前端关系集是不变量）；每个元素的公共面只从它自己的入口取。
- **扩展名 `.tsx` 保留**：入口正文零 JSX，但入口解析链的 ③ 两条候选（`index.ts` /
  `index.tsx`）都认它，改名属独立变更。

## 契约面留白

入口的 8 个名字分两面留白，成因不同：

- **4 个组件操作不进 `api-op`**：四者形态都是 `(props: <Props>) => React.JSX.Element`——入参
  是具名 props 类型、返回位是 JSX 元素。契约侧 `$ref` 归一为 `?<名>`、代码侧是具名
  `<名>`，两侧短形恒不等（`apiSignatureMismatch` 恒红）；返回位在契约词汇里没有落点。同类
  实测差异见 `App.Frontend.Components.Agent.AgentError` 的留白段。
- **4 个类型不进 `api-reexport`**：两条独立成因。① specifier：透传条目的 TS specifier 由
  `paths::ts_import_specifier` 按入口形态产出（目录形态 → `../<目标根末段>.i`），而前端真值
  ——同目录 `./<目标入口主干>.i`、目标末段含 `.tsx`——尚未接线（该函数的显式留白），此刻写进来
  必然生成指向不存在文件的 `export type … from`。② 生成链：`emit::ts` 在文件末尾多出一个
  空行，`vp check` 的格式化器剥掉它之后 `api:check` 的零 diff 门即判 `mismatched`——前端
  `.i.ts` 的「格式化不动点 ⇄ 生成物零 diff」两条判据此刻互相矛盾（实测见
  `App.Frontend.Components.Agent.AgentBlocks` 的留白段）。两条出路都在源码/配置面，属独立的
  生成链接线变更。四个类型的归属与去向逐条登记在上表，待接线后迁入透传清单（一次一条）。

## 类型

本元素自有 `$defs` 为空（公共面全部为透传，见上）：

```json api-types
{}
```
