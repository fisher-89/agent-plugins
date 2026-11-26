# Desktop.Components.Agent

## 职能

agent 会话组件的统一出口：目录入口 `packages/desktop/src/components/agent/index.tsx` 只做桶
透传，会话消费面（agent 调试页 / 变更右栏 / 变更流程运行记录 / 探索页 / 配置页常驻面板）一律
经它取组件与配套类型。透传清单的唯一持有者是入口代码本身——契约面两侧都留白（见下），把名单
抄进散文只会多出一处要跟着改的副本。

**边界**：入口只做透传——零实现、零类型定义、零运行期语句。会话状态机在
`src/hooks/use-agent-session`（非本元素），消息与工具卡的呈现各归子元素
（`AgentMessages` / `AgentBlocks` / `ToolCards`），失败投影归 `AgentError`。

## 部件地图

各件的职责与落点如下——「哪个文件装了什么」，名字清单不在这里（唯一持有者是入口代码）：

| 位置 | 内容 |
|---|---|
| `agent-input.tsx` | 会话输入框与提交交互（含 permission-mode 档位、停止入口） |
| `agent-session-view.tsx` | 整段会话视图（自持 / 注入双形态）与 composer 插槽的运行态 / 动作面 |
| `session-panel.tsx` | 会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条）与面板公共状态子集 |
| `message-cards/config-apply-scope.tsx` | 提案卡的写回动作面 Provider（root + onApplied） |
| `agent-messages.tsx` | ask 应答动作面的承载类型（三态判活） |

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

入口的公共面分两半留白，成因不同：

- **组件操作不进契约**：形态都是 `(props: <具名 props>) => React.JSX.Element`——入参是具名
  props 类型、返回位是 JSX 元素。契约侧 `$ref` 归一为 `?<名>`、代码侧是具名 `<名>`，两侧短形
  恒不等（`apiSignatureMismatch` 恒红）；返回位在契约词汇里没有落点。同类实测差异见
  `Desktop.Components.Agent.AgentError` 的留白段。
- **透传类型不进契约**：两条独立成因。① specifier：透传条目的 TS specifier 由
  `paths::ts_import_specifier` 按入口形态产出（目录形态 → `../<目标根末段>.i`），而前端真值
  ——同目录 `./<目标入口主干>.i`、目标末段含 `.tsx`——尚未接线（该函数的显式留白），此刻写进来
  必然生成指向不存在文件的 `export type … from`。② 生成链：`emit::ts` 在文件末尾多出一个
  空行，`vp check` 的格式化器剥掉它之后 `api:check` 的零 diff 门即判 `mismatched`——前端
  `.i.ts` 的「格式化不动点 ⇄ 生成物零 diff」两条判据此刻互相矛盾（实测见
  `Desktop.Components.Agent.AgentBlocks` 的留白段）。两条出路都在源码/配置面，属独立的
  生成链接线变更；接线后逐条迁入 `$defs` 与透传清单（一次一条），迁移先后不预设。

## 类型

本元素自有 `$defs` 为空（公共面全部为透传，见上）：契约侧不新建任何类型，入口的形状承诺只有
代码持有。
