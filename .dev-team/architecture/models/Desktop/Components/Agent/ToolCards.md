# Desktop.Components.Agent.ToolCards

## 职能

工具调用卡片的专门呈现与内置注册表：7 个工具各有一张特化卡（ask_user_question / finish_step /
web_search / bash / read / write / edit），并以 `BUILD_IN_TOOL_CARDS` 暴露「工具名 → 卡组件」
的注册表，供 `AgentMessages` 的分派消费。

**边界**：只呈现已解析的输入 / 输出与轮状态，零取数、零 IPC、零工具执行；卡层不认识轮状态
词表（只穿 `turnRunning` 布尔原语）。两张卡（`web_search` / `finish_step`）的实现落在
`AgentBlocks`，本元素只注册它们——注册表按键归此，块实现按呈现归彼。

## 不变量

- **注册表键 = 工具名（无 `tool-` 前缀）**：前缀剥离发生在消费者（`AgentMessages` 的
  `part.type.slice('tool-'.length)`），注册表 MUST NOT 收录带前缀或无前缀两种形态。
- **未配对运行态判据单点**（`unpairedRunning`）：有结果即非运行态；无结果且轮已终结也不呈
  运行态——未配对只说明结果没落库，不说明正在跑。四处 `TraceLine status` 与 Ask / Bash 的运行
  图标共用这一判据，MUST NOT 各写一份。
- **注册表未命中即回退**：`AgentMessages` 拿不到卡时回退到通用工具对卡（`ToolPairCard`），
  本元素 MUST NOT 为未知工具补兜底卡。
- **ask 可交互的条件在消费者**：动作面只在「在场 ∧ 本卡是 ask 卡 ∧ 活卡」时传来；卡内不再
  判活，缺席即静态只读。
- **静态解析一律宽容**：问题 / 选项 / 输出字段走 `asRecord` / `textOf` / `numberOf` 的宽容
  读法，未知形态回退 JSON 原样——一条坏载荷不得让卡片抛错。
- **`BUILD_IN_TOOL_CARDS` 是常量值**：不注册「按需可变」的入口（无写入函数、无 setter），
  扩展新卡只经代码改动（一次改动 + 一次注册）。

## 契约面留白

- **可插拔位类型不进契约**：它是 `ComponentType<内部 props>`——泛型别名，且参数是未导出的内部
  接口。契约的泛型零支持（`reject_generics`）与「跨模块具名类型位缺位」两条在此叠加；把内部
  props 抄成 `$defs` 会让「内部形状」变成公共承诺，与「不外露」的不变量正面对撞。
- **注册表不进契约**：取数器只把 `export function` 记作操作、`export const` 里的对象字面量记作
  值（排除），故它不是符号；其形状是「运行时卡组件表」，无法用 JSON Schema 表达，条数与键集在
  入口代码与 `AgentMessages` 的分派处共同守。
- **本元素零操作面**：唯一可进操作面的是各卡组件本身，而它们是 `memo(...)` 组件（组件面非契约
  操作），形态与 `AgentBlocks` 的块同族。

## 类型

本元素自有 `$defs` 为空（公共面全部为留白，见上）：契约侧不新建类型。
