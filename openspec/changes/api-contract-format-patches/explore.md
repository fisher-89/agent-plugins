# 变更 D · 契约格式补丁（可空 / 属性键 / 降级词表）

workflow_type: requirement

> 主线：`openspec/explores/module-entry-declaration.md`。
> 本单元由「给前端模块试写契约草案」的演练暴露，三个缺口都**独立于入口**：
> 验收判据全落在既有契约语料上，零入口依赖，可最先落。

## D.0 scope 与依赖

| 项 | 内容 |
|---|---|
| 前置依赖 | 无 |
| 被依赖 | 无（但与变更 B/C 的「前端契约能不能写」结论强相关） |
| 独立可归档 | 是——建议**先落**，因为 P1 是「判据绿而产物错」的现网风险 |
| 演练样本 | `App.Frontend.Components.Agent`（公共面与抽取结果见变更 B 的 B.2 / B.3） |

## D.1 演练样本契约草案（按现有 fence 规则写）

> 样本元素 = `App.Frontend.Components.Agent`（公共面与抽取结果见变更 B 的 B.2 / B.3）。

```md
# App.Frontend.Components.Agent

## 职能

agent 会话组件的统一出口：把「会话视图 / 输入框 / 会话面板」三件套与它们的
动作面、状态面类型收在一个 barrel 里，供变更详情、探索页与 agent 调试页复用。

## 不变量

- barrel 只重导出本元素的公共面；目录内的 props 类型与内部工具 MUST NOT 出线。
- 会话面的取数与订阅由消费方注入或由组件自持，二者互不前置。
- 组件返回 `React.JSX.Element`：形状不进契约（op 面本期留白）。

## 操作

（本期留白：三个导出均为组件，返回 JSX；契约不表达 JSX 形态。）

## 类型

```json api-types
{
  "AskAnswerActions": {
    "type": "object",
    "properties": {
      "onAnswer": {
        "x-function": {
          "params": [{ "name": "text", "schema": { "type": "string" } }],
          "returns": { "type": "null" }
        }
      },
      "pendingToolCallId": { "type": "string" }
    },
    "required": ["onAnswer"]
  },
  "AgentSessionActions": {
    "type": "object",
    "properties": {
      "running": { "type": "boolean" },
      "loading": { "type": "boolean" },
      "steering": { "type": "boolean" },
      "send": {
        "x-function": {
          "params": [{ "name": "input", "schema": { "x-common": { "note": "待剪枝：AgentSessionSendInput" } } }],
          "returns": { "type": "null" }
        }
      },
      "stop": { "x-function": { "params": [], "returns": { "type": "null" } } },
      "steer": {
        "x-function": {
          "params": [{ "name": "text", "schema": { "type": "string" } }],
          "returns": { "type": "null" }
        }
      }
    },
    "required": ["running", "loading", "steering", "send", "stop", "steer"]
  },
  "AgentSessionViewProps": {
    "type": "object",
    "properties": {
      "root": { "type": "string" },
      "sessionId": { "type": "string" },
      "refreshKey": { "type": "integer" },
      "session": { "x-common": { "note": "待剪枝：UseAgentSessionState（hooks 无元素）" } }
    }
  },
  "SessionPanelState": {
    "type": "object",
    "properties": {
      "sessions": { "type": "array", "items": { "x-common": { "note": "待剪枝：SessionSummary（bindings → App.Backend.Shell）" } } },
      "total": { "type": "integer" },
      "listOpen": { "type": "boolean" },
      "canStart": { "type": "boolean" },
      "note": { "type": "string" }
    },
    "required": ["sessions", "total", "listOpen", "canStart"]
  }
}
```
```

## D.2 三个格式缺口（概述；补丁见 D.3）

| 缺口 | 实测依据 | 后果 |
|---|---|---|
| **可空/可选只能靠 `required`** | `emit::ts::interface_block`：字段不在 `required` → 渲染 `T \| null`；`value_type` 只读 `type` 的**字符串**形态 | `"type": ["string","null"]` 会落到 `unknown`（`Some(_) \| None` 分支）。即：可选 = 不在 `required`，且**无法区分「可选缺失」与「显式 null」**（一律渲染 `T \| null`） |
| **非标识符属性键会产出不可编译文本** | 同上：`format!("  {field}: {ty};\n")` **零转义零引号** | `'data-testid'?: string` 这类键写进契约 → `data-testid: string;`（非法 TS）。故 `AgentSessionViewProps` 的 `data-testid` 在草案里**必须剔除**，或修 `emit::ts` 的属性键写出点（`is_identifier` 判据在契约侧只管七族名字，属性键不在射程） |
| **`x-function` 参数里的外来类型无着落** | `AgentSessionSendInput` 不在本元素公共面 | 只能 note 降级；`$ref` 一条路被「必须指向顶层 `$defs` 或外部契约」堵死（外部契约又受「直接子元素」约束） |

## D.3 三个缺口与补丁

共同根因一句话：**同一份 schema 事实，判据面与渲染面读法不同**——判据面（`schema_type`）已支持 nullable 三分支，渲染面（`emit::{ts,rust}::value_type`）只读 `type` 的字符串；名字闸只覆盖「七族名字」，属性键不在射程；降级有三条来源却只有一条进计数。

### D.3.1 P1 · 可选与可空：一个事实，两套读法

### 现状（三处代码，逐条实测）

| 事实 | 现状 | 位置 |
|---|---|---|
| 契约侧 nullable | **已支持三分支**：`"type": "null"` / `"type": ["string","null"]` / `anyOf`-`oneOf` 含 null 分支 → `ApiType::Optional` | `api/extract/mod.rs:274` `nullable_inner`（签名比对用） |
| TS 渲染 | **不支持**：`value_type` 只 `schema.get("type").and_then(as_str)` → 数组/对象形态落 `unknown` | `api/emit/ts.rs:339` |
| Rust 渲染 | **不支持**：同上 → `serde_json::Value`（`FALLBACK_VALUE_TYPE`） | `api/emit/rust.rs:387` |
| 非 `required` 字段 · TS | 一律渲染 `{field}: T \| null`——**丢了 `?`**，把「可选缺失」与「显式 null」压成一态 | `api/emit/ts.rs:224` |
| 非 `required` 字段 · Rust | `Option<T>`（正确） | `api/emit/rust.rs:187` |

后果链：契约写了 `{"type": ["string","null"]}` → **签名判据绿**（`schema_type` 认得）→ **生成物退化**（`unknown` / `serde_json::Value`）→ 实现侧类型无法绑定。这是「判据绿而产物错」的最坏组合，比两侧都不支持更危险。

### 补丁（三小项，同一条口径）

| # | 改动 | 落点 |
|---|---|---|
| 1.1 | 把 `nullable_inner` 的**形状识别**抽成共享单点（`api::nullable_breakdown(schema) -> Option<(非空分支, 是否显式 null)>`），判据面与两渲染面共用；`value_type` 在常规 `type` 分派**之前**处理 nullable（与 `schema_type` 同序） | `api/extract/mod.rs` 新函数 + `emit/{ts,rust}::value_type` 各一处前置分支 |
| 1.2 | TS 侧非 `required` 字段出 **`{field}?: {ty}`**（不再硬拼 `\| null`）；`\| null` 只由「显式 null」（1.1 的第二返回值）决定 | `emit/ts.rs::interface_block` |
| 1.3 | 两者叠加时**去重**：TS `T \| null` 至多一次；Rust `Option<T>` 至多一层（`Option<Option<T>>` 是退化产物，MUST NOT 出现） | 同上 + `emit/rust.rs` 结构体字段写出 |

### spec 措辞（`desktop-archi-api` 的类型映射段新增一条）

> 字段可选性与可空性 SHALL 为**两个独立事实**：`required` 缺省 = 可选（TS `?`、Rust `Option<T>`）；`type` 内含 `null`（三分支任一）= 可空（TS `| null`、Rust `Option<T>`）。两者叠加时 MUST 去重到「至多一层」。三者 MUST 由同一判定函数产出（判据面与两侧渲染面共用），MUST NOT 保留「判据认得、渲染不认得」的不对称。

### 兼容与验收

- 现有唯一契约（`LocalArchiCheck` 单元结构体、零 `properties`）**零影响**；
- 机械验收：① `schema_type` 与两 `value_type` 对同一 9 组样例（无 null / type 字符串 null / 两元素数组 / 三元素数组 / anyOf-null / oneOf-null / 非 required 单字段 / 两者叠加 / 嵌套数组元素 nullable）产出**同一可空性判定**；② 金样渲染文本零 `unknown` 泄漏（对含 nullable 的样例）。

### D.3.2 P2 · 属性键必须有形态闸（否则两侧产物一个坏一个不能写）

### 现状

```text
emit::ts::interface_block   → format!("  {field}: {ty};\n")     // 零引号零转义
emit::rust（结构体字段）     → format!("    pub {field}: {ty},\n")  // 同上
contract::verify 闸一        → 只判七族名字（op id / 参数名 / owner / $ref 末段 / $defs 键 …）
                               **属性键不在射程**
```

`'data-testid'` 这类键写进契约 → TS 生成 `data-testid: string;`（不可编译）；Rust 侧根本无「带引号字段名」形态（`r#` 只接受标识符）。两侧**同时**坏，且都在生成物里坏（装载期无感）。

### 补丁

| # | 改动 | 落点 |
|---|---|---|
| 2.1 | 闸一扩一条：`$defs.*.properties` 的**键集**与 `required` 的**元素集** MUST 是 ASCII 标识符；违反即 `ApiError::Contract`，文案带 JSON pointer（与 `x-function` 落点闸同形） | `api/contract.rs::verify`（`is_identifier` 家族） |
| 2.2 | 错误文案给出一条可执行出路：「属性键必须是标识符；`data-testid` 一类键不进契约（前端契约须避开 DOM 测试属性）」 | 同上 |
| 2.3 | **明确否决**：给 TS 侧加引号形态（`'data-testid': string`）——那会让「同一 schema 两侧产物形态不同」，正是 D47 纪律要消灭的东西；`x-key` 映射属第二套命名，同样否决 | 记入 spec 的「否决项」 |

### 兼容与验收

- 现有契约零 `properties` → 零影响；
- 机械验收：`contract_test` 新增一条「`'data-testid'` 键 → 装载期 `ApiError::Contract` 且文案含 pointer」；两侧接缝测试加一条「`$defs.*.properties` 键集 ⊆ 标识符」的语料扫描。

### D.3.3 P3 · 降级要有词表，并且进计数

### 现状（三条来源，只有一条进账）

| 来源 | 表达 | 进 `INDEX.toml` 的 `degraded` 计数？ |
|---|---|---|
| bootstrap 草稿 | `x-common.note = "待剪枝：…"` | ✗ |
| 跨语言规则（`common.json` 的 `degraded`） | 生成物**头注释/接口注释**里的 `x-common.degraded` 说明（`degraded` 资产键→ 渲染注释） | 部分（只在 `$defs` 条目带 `x-common.degraded` 时计） |
| 手工「类型不可达」 | `x-common.note`（本仓演练即此形态） | ✗（`api/mod.rs:512` 只认 `x-common.degraded`） |

也就是说：**「这个契约有债」这件事，写 note 看不见、写 degraded 才进账**，但没有任何规则要求写哪个。

### 补丁

| # | 改动 | 落点 |
|---|---|---|
| 3.1 | 定义**封闭降级词表**（首期四值，新增须改 spec）：`foreignElement`（跨域/非直接子元素引用）/ `noElement`（目标无模型元素，如 hooks/lib/types）/ `unrepresentable`（目标语言类型系统表达不了，如 JSX）/ `crossLanguage`（规则表降级） | `common.json` 旁新增资产或 `schema.rs` 常量；`verify_defs` 校验取值 |
| 3.2 | 规则：**凡「引用不可达 / 不可表达」的类型，MUST 写 `x-common.degraded = <词>`**；`note` 只作自由注记（人读），不承担判据 | `verify_defs` 新增一条 + spec 措辞 |
| 3.3 | `INDEX.toml` 的 `degraded` 列因此成为**真实债表**（bootstrap 草稿的 `note` 不计入，避免把「草稿未剪枝」与「结构性不可表达」混为一谈） | 计数逻辑零改动（它已只认 `x-common.degraded`），只补「谁必须写」 |

### 兼容与验收

- 现有契约零 `$defs` 条目的 `x-common` → 零影响；
- 机械验收：① 演练契约（12.3）改完后 `INDEX.toml` 的 `degraded = 3`（`session` / `sessions.items` / `send.input` 三处）；② `verify_defs` 拒收词表外的值，文案列出四值。

### D.3.4 三条补丁的共同验收（可机械复算）

```text
P1  schema_type 与两 value_type 的可空性判定一致（9 组样例，逐组对拍）
P2  verify 拒收非标识符属性键（1 条反例 + 文案含 pointer）
P3  degraded 词表封闭（4 值）+ 演练契约的 INDEX 计数 = 3
    + 三条补丁后，12.3 的草案产物：TS 侧无 unknown 泄漏、无未加引号的可疑键、
      Rust 侧字段名全标识符、Nullable 与 Option 去重到一层
```

### D.3.5 与「入口」这条线的关系

三条补丁都**不依赖**入口字段：它们可以独立成一个变更（契约格式补丁），也可以挂在「入口 + 前端 barrel」之后作为第二步。建议**独立**，理由是：补丁的验收判据完全落在既有契约语料上（零入口依赖），而入口变更的判据落在模型与 barrel 上——两者混在一个 change 里，回归归因会很难看。

## D.4 两处结构性卡点（不在本变更，须单列裁决）

| 卡点 | 具体 | 现规则下 |
|---|---|---|
| **hooks 无元素** | `UseAgentSessionState` 来自 `@/hooks/use-agent-session`；`src/hooks/**` 按 ADR 0001 §7 不建元素 | 无处 `$ref` → 只能 note 降级，或给 hooks 建元素（改粒度） |
| **跨域类型引用被 ref 规则拦** | `SessionSummary` 来自 `types/generated/bindings` → 映射到 `App.Backend.Shell`；但 `apiRefMissing` 要求 `$ref` 目标是**直接子元素**，而 `App.Frontend.Components.Agent` 的直接父是 `App.Frontend.Components` | 引用后端命令面类型**结构性不可表达**（`ref_missing` 判「跨层引用架空层级语义」） |

也就是说，前端契约的**类型面天花板**比 Rust 侧低一档：凡是「props 里引用外部（hooks / 后端 bindings / 第三方）」的类型，都写不完整。

## D.5 三种收敛（按性价比排序）

1. **只写「纯数据 + 回调」型类型**（`AskAnswerActions` / `AgentSessionActions`）：契约完整且绿，但覆盖不到 props 面；
2. **props 面用 `x-common.note` 显式降级**（12.3 的写法）：能写、能绿、能查缺，缺点是类型面丢掉关键字段（`session` / `sessions`）；
3. **改判据或改粒度**（为前端类型引用放宽「直接子元素」为「同域或命令面」，或给 `hooks/` 建元素）：一次性解锁，但改的是 `desktop-archi-api-check` 的既有 requirement 与 ADR 0001 的排除面。

## D.6 演练结论

- 「入口 = 公共面」在 TS 侧**可被机械验证**（barrel 的 re-export 名单就是公共面），`api-types` 面能写出**真实且可校验**的契约；
- 前端契约的**可用面 = 具名接口类型**；组件签名（props 内联字面量）永远抽不出 → 想给前端写契约，先约定「模块公共面必须具名」（`export interface XxxProps`），否则契约只能承载「数据形状」；
- 两个硬卡点（hooks 无元素、跨域 ref 不可表达）决定前端契约的**上限**，且都属于「改判据」而非「改代码」——建议单列一个变更裁决，不要混进入口这条线。

---
