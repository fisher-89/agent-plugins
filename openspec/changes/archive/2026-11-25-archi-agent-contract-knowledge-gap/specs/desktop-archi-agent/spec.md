# desktop-archi-agent Specification（delta: archi-agent-contract-knowledge-gap）

## Purpose

本 delta 给 agent 面加**两个模型写工具**（`archi_element` / `archi_relation`），并把手改面的语法知识从系统提示迁进工具——两层同批，不可只做一半：

- **写工具面**：元素 upsert / delete（可内联出边、可单文件级联）与单条关系 upsert / delete，参数面即字段清单；引擎语义（三态 / 自证链 / 乐观锁 / 落点 / 删除闭包）全在 `desktop-archi-model` 的写回原语族，本 delta 只登记**工具面**的字段、下发面与档位。
- **反馈面**：三通道（`Err` 阻塞零落盘 / `warnings` 已落盘有未阻断事实 / `changed` 幂等凭据）+ 响应体（`hash` / 四计数 / `counts` / `model`）+ 入参面（八行上限与 E1 硬闸）+ Err 分层，全部沿既有词汇与既有渲染单点。
- **知识收缩**：骨架 `archi-knowledge.md` 从六段 2597 B 收为四段（职责 / 工具面 / 留给判断 / 手改出口），语法段（键形 438 + 字段纪律 827 + `kind` 语义 454 + 层级读数 330）迁往工具参数描述、`archi_scaffold` 词典与 parser 诊断；门限 2700 / 2850 → **1500 / 1700**（实测值冻结）。
- **立场改写**：把「MUST NOT 存在专用写模型工具」与「写入不校验，读取报错」两句绝对命题换成两条入口（写工具 canonical + `write` / `edit` second path）与两条失败语义（写工具坏文本不可产出 / 手改面由读取面报错）；两处交叉引用随之换名。
- 零新增 IPC 命令、零画布写入口、零判据面变化；`archi` crate 仍全程零写盘（写盘单点仍是 `write_atomic`，两消费面共用）。

## RENAMED Requirements

- FROM: `### Requirement: 只读档位归属与成员校验不变`
- TO: `### Requirement: archi 工具档位归属与成员校验（五只读 + 两写）`

- FROM: `### Requirement: 模型 = 源码（写纪律与不设专用写工具）`
- TO: `### Requirement: 模型 = 源码（两条写入口与写纪律）`

## ADDED Requirements

### Requirement: archi 写工具面（元素 upsert / delete 与关系 upsert / delete）

SDK 引擎 SHALL 提供两个专用**写**工具（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验）：`archi_element`（元素 upsert / delete，可内联出边）与 `archi_relation`（单条关系 upsert / delete）。执行体 SHALL 在 `sdk/archi_tool.rs` 的 `ArchiContext`（第六、七臂），与五只读工具共用同一 per-turn 上下文与同一 `cap_result` 上限单点：

```jsonc
{ "name": "archi_element",
  "action":   "必填，封闭二值：upsert / delete",
  "element":  "必填，元素完整 FQN",
  "kind":     "enum ← ElementKind::ALL；新建必填，已存在时省略（给了即 ElementAlreadyExists）",
  "gate":     "enum ← GateTier::ALL", "api": "enum ← ApiStatus::ALL",
  "path":     "string[]（不传 = 不动；空数组 = 清键）",
  "entry":    "string | null（null = 清键）", "doc": "string | null",
  "summary":  "string | null", "metadata": "object<string,string>（空对象 = 清键）",
  "relations": "[{ to（必填）, desc?, apis?, action? }]（缺省 action = upsert，另一值 delete）",
  "expect":   "sha256:<hex>（可选：目标文件当前内容哈希；省略 = 不校验）",
  "file":     "模型声明目录一级 *.toml（新建落点覆盖；既有元素只允许同值）",
  "create":   "bool（新建文件需显式许可；缺省 false）",
  "cascade":  "bool（仅 action = delete；缺省 false）" }

{ "name": "archi_relation",
  "action": "必填，upsert / delete", "from": "必填", "to": "必填",
  "desc":   "string | null", "apis": "string[]", "expect": "sha256:<hex>（可选）" }
```

- **平铺 schema、逐字段 `description`**：两工具的 `parameters` SHALL 为平铺对象（MUST NOT 用 `oneOf`，MUST NOT 把元素面与关系面合成一个工具——两面字段完全不相交，合一只能靠大量互斥可选位）；每个字段的 `description` SHALL 写明缺省与陷阱（`path` 的基数随 `kind`、`metadata` 值须为字符串、`relations[].action` 缺省 `upsert`、`cascade` 仅 delete、`doc` 仅 `api = none` 可写、`file` 缺省落父元素文件）。
- **值集派生**：`action`（二值）/ `kind` / `gate` / `api` / `relations[].action` 的合法值 SHALL 由封闭词表派生（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / 两处二值词表），MUST NOT 手抄字面量。
- **下发面**：两工具 SHALL 只在 `(proposal, executor)` / `(dev-design, executor)` 与 `source = "archi"` 臂下发（矩阵见「archi 工具面注入与 (phase, role) 矩阵」）；其余相位与来源 MUST NOT 下发，调用由成员校验拒绝。
- **档位**：两者 SHALL 归 `ToolTier::Write`（`default` 档拒绝、`acceptEdits` / `bypassPermissions` 放行）——与 `write` / `edit` 同档；MUST NOT 为两者单开档位。
- **写面语义全在 `desktop-archi-model`**：三态（省略 = 不动 / `null` = 清空 / 值 = 设值）、内联出边的四条约束、落点四情形、`cascade` 的**单文件闭包**、`expect` 乐观锁与 `hash` 回传，SHALL 由 `archi::edit` 的写回原语族承载；本工具 MUST NOT 自拼 TOML、MUST NOT 重实现任一判据（含 `path` 形态、`kind × path` 基数、`apis` 值面）。执行体 SHALL 只做「入参 → `ElementSpec` / 关系三态 → `plan_*` → 落盘 → 响应装配」。
- **落盘单点**：执行体 SHALL 以 `write_atomic` 单点落盘（临时文件 + 重命名，与壳层命令面共用同一实现），MUST NOT 复制第二份「临时文件 + rename」逻辑，MUST NOT 在 `archi` crate 内出现写盘调用。
- **写保护射程**：解析出的目标文件 SHALL 过**同一** `ProtectRules`（内置 → 会话 → 用户三段序、首命中即拒、拒串沿用既有族），判定细则归 `desktop-write-protection`「会话级写入禁区（相位锁）」；MUST NOT 另立规则集或第二份文案。
- **成员校验与上下文**：两工具 SHALL 走既有成员校验（调用名 ∉ 本轮 definitions ⇒ `tool_unavailable` + `is_error` 回灌，run 不中断）；archi 上下文铸造判据 SHALL 由「名字清单含任一 archi 工具名」表达（七名逐一在场，MUST NOT 漏掉新增两名）。
- **工具描述面**：两工具的描述 SHALL 说明三件事——① 写盘是原子的且**坏文本不可产出**（自证链，零落盘）；② `expect` 的用法与 `hash` 的闭环（「上一轮响应的 `hash` 或 `archi_module` 的 `fileHash` 可直接作 `expect`」）；③ `cascade` 只作用于**同一模型文件**内的子树与边。描述 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 词典与参数 `description`）。

#### Scenario: 两写工具在册且档位与下发面对齐

- **WHEN** 审查挂载清单与三个写权装配面的 definitions
- **THEN** `archi_element` / `archi_relation` 均在册且 `ToolTier::Write` / `ToolExec::Archi`；`(proposal, executor)` / `(dev-design, executor)` / `source = "archi"` 三处 definitions 含两者，`RunStarted.tools` 逐名一致；`(implement, executor)` / `(dev-design, evaluator)` / `explore` / `debug` 零下发且调用被成员校验拒绝（`default` 档下写权相位调用亦被档位拒绝——与 `write` 同款）
- **AND** 五只读工具的档位与下发面逐字不变

#### Scenario: 元素 upsert 走通工具面

- **WHEN** 调 `archi_element { action: "upsert", element: "Backend.Core.Billing", kind: "module", path: ["./src-tauri/crates/core/billing/src/"], summary: "计费域" }`
- **THEN** 模型文件新增该元素表（字段按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序），响应含 `created = true`、`changed` 覆盖给予的字段、`file` 落父元素所在文件、`hash` 为新值；再以同值调一次 ⇒ `created = false`、`changed = []`、盘上字节零变化

#### Scenario: 内联出边与独立关系工具等价

- **WHEN** 对同一 `(from, to)` 分别用 `archi_element { relations: [{ to: X, desc: "…" }] }` 与 `archi_relation { action: "upsert", from, to, desc: "…" }`
- **THEN** 两者产出的模型文本**逐字节相同**（同一 `apply_*`）；`relations` 四计数与入参数组长度守恒；内联边已声明在**别的**模型文件时前者 `Err` 点名该文件并指向 `archi_relation`，后者照常改写

#### Scenario: 级联只作用于同一模型文件

- **WHEN** 对含子元素与出入边、且整棵子树全在同一文件内的元素调 `archi_element { action: "delete", cascade: true }`；另对一个子元素或某条边声明在**另一**模型文件的元素作同样调用
- **THEN** 前者删净子树与相关边、该文件表头计数随动、其余内容逐字保留；后者 `Err` 点名跨向的文件与阻塞项、**零落盘**（盘上字节零变化）

#### Scenario: 值域与上限在入参面即拒

- **WHEN** 分别以 `action: "patch"` / `kind: "service"` / `gate: "high"` / `relations: [{ to: X, action: "keep" }]` / `cascade: true` 配 `action: "upsert"` / 超过八行上限的任一字段调用两工具
- **THEN** 一律 `Err` 且**零落盘**；值域错误点名该参数的合法值（由 `ALL` 派生），上限错误点名字段与上限值；`default` 档下更早由档位拒绝

### Requirement: 写工具的响应体与错误面契约

两个写工具的输出 SHALL 走**三通道**（`Err` 阻塞零落盘 / `warnings` 已落盘有未阻断事实 / `changed` 幂等凭据），MUST NOT 增设 `hints` 一类第二通道（拆开后模型可能只看 message 不看 hint）：

```jsonc
{ "warnings": [ … ],                                   // 恒在场；空数组 = 无异常
  "action": "upsert", "element": "Backend.Core.Billing",
  "file": ".dev-team/architecture/graph/model/02-backend-core.toml",
  "hash": "sha256:9c2e…",                              // 写后新值 = 下一次 expect
  "created": true, "changed": ["kind", "summary"],
  "relations": { "created": 1, "updated": 0, "deleted": 1, "unchanged": 2 },
  "counts": { "elements": 10, "relations": 10 },       // 本文件（重解析结果）
  "model":  { "elements": 56, "relations": 129 } }      // 全模型（前值 ± 增量）
```

- **`warnings` 恒在场**：`archi_element` 的七条判据与降噪不变式归 `desktop-archi-model`「写面反馈判据（七条 warnings 与降噪不变式）」；本工具只负责把它序列化进响应（`kind` / `severity` / `message` 三字段，`kind` 与 `severity` MUST NOT 在此另作裁剪或重定档）。`archi_relation` 的 `warnings` SHALL 恒为空数组。
- **`changed`**：`action = upsert` 时 SHALL 为**参数面字段名的子集**（实际变化的字段；空数组 = 幂等凭据）；`action = delete` 时为空数组。`archi_relation` 的 `changed` SHALL 为 `desc` / `apis` 的子集。
- **`archi_relation` 的响应是薄形**：`{ warnings（恒空数组）, action, from, to, file, hash, created, changed }`——MUST NOT 携带 `relations` 四计数与 `counts` / `model`（一次写只动一条边，计数另经 `archi_drift` 取，不为一行数据付全量重解析）。
- **`relations` 四计数**：`created + updated + deleted + unchanged` SHALL 恒等于入参 `relations` 数组长度（可直接断言的守恒式）；未给 `relations` 时四项皆 `0`（字段不省略）。
- **`counts` / `model`**：`counts` SHALL 等于写后该文件的重解析计数（与表头括号数字同源），`model` SHALL 等于全模型计数（前值 ± 本次增量），两者 MUST NOT 独立于重解析结果另算。
- **`hash`**：SHALL 与 `drift::file_hash` 同一函数产出、同一 `sha256:` 前缀——与 `expect` 可互喂（连续两次写同一文件零额外读）。
- **响应不含元素 / 关系全文**（要看现状用 `archi_module`）；不含协议正文。
- **成功响应永不触达截断**：`warnings` ≤ 7 条 × ≤200 B + 其余计数与短串 ⇒ 结构上界 ≈ 2 KB ≪ `MAX_RESULT_BYTES`（50 000）。故**不设键序断言**——`serde_json` 未开 `preserve_order`，把 `warnings` 写在对象首位不是机制，防御性排布亦不构成契约；真正需要体量纪律的是 `Err`（见下）。

**错误面分层**（判据一律来自既有单点，MUST NOT 另立词汇）：

| 层 | 场景 | 承载 |
|---|---|---|
| E1 入参 | `action` / `element` / `relations[].to` 缺失或空、四处封闭值域、**八行上限** | 执行体显式检查（`ok_or_else` 形态），文案点名参数与合法值 |
| E2 前置读数 | 模型已有结构诊断 | `dto::graph_diagnostics_error` 的**全量形**（MUST NOT 为写工具另写第二份诊断渲染） |
| E3 语义 | 祖先缺失 / 端点不存在 / 删除有引用 / `kind × path` 冲突 / 批量四条 / 跨文件内联边 | `EditError`（`EditErrorKind` + 事实段） |
| E4 落点 | `file` 不在模型目录一级 / 无 `create` 而目标文件缺席 / 目录缺席 | `EditError` |
| E5 删除 | 元素 / 关系（含内联 `delete`）不存在 | `EditError`（附候选或现有出边） |
| E6 自证 | 渲染后仍有诊断 / 回读不等 | `EditError` + **零落盘** + 「这是工具缺陷」语义 |
| E7 并发 | `expect` 与当前文件哈希不等 | `EditError`（expect / 当前双值 + 两条出路） |

- **四条硬纪律**：① 第一行自足（`Err` 含 `重试：` / `下一步：` / `改用 <工具名>` 之一，或含可复制 JSON 参数片段）；② 字面与词表同源（`gate` 不写「档位」、诊断词不译中文）；③ 不猜（给候选不代选）；④ 一次报全（MUST NOT 首错短路）。
- **入参上限是软提示 + 硬闸两段**：八行上限 SHALL 同时进 schema（模型侧提示：FQN ≤256 / `path` ≤32 项 × ≤1024 / `metadata` ≤32 项（键 ≤64、值 ≤512）/ `relations` ≤64 / `relations[].apis` ≤32 × ≤128 / `summary`·`desc` ≤512 / `entry`·`doc` ≤1024 / `file` ≤256）与执行体 E1 检查；两处数字 SHALL 取自**同一组常量**（MUST NOT 各写一份）。

#### Scenario: 三通道可辨

- **WHEN** 分别以「写坏模型」「只改 `summary` 的合法 upsert」「与现状同值的 upsert」「指向不存在关系的内联 `delete`」调用 `archi_element`
- **THEN** 依次为：`Err` 携诊断清单（零落盘）/ `warnings = []` 且 `changed = ["summary"]` / 幂等（`changed = []`、盘上字节零变化）/ `Err` 列出同 `from` 的现有出边（零落盘）

#### Scenario: 降噪不变式在工具面成立

- **WHEN** 对 `api = declared` 且契约 md 缺失的元素分别调「只改 `gate`」与「改 `path`」两次 upsert
- **THEN** 前者 `warnings = []`；后者 `warnings` 含 `path` 诱发的条目；两次的 `warnings[].kind` 恒为既有两词汇集的 `as_str` 之一（`ArchiDriftKind` / `ArchiApiFindingKind`），severity 与判据面单点同值

#### Scenario: 四计数守恒与幂等

- **WHEN** 一次 upsert 传入三条 `relations`（一条新建、一条改 `desc`、一条 `delete`），再一次以完全相同的参数重放
- **THEN** 首次 `relations = { created: 1, updated: 1, deleted: 1, unchanged: 0 }`（四者之和 = 3）且 `changed` 非空；重放时该边已不存在或值已相同 ⇒ 逐项按实计（`unchanged` 吸收），四者之和仍 = 3，`changed = []`，盘上字节零变化

#### Scenario: 成功响应体量与计数可对拍

- **WHEN** 取任一成功响应并序列化，随后对该文件重解析、对全模型取 `archi_drift`
- **THEN** 响应长度 < `MAX_RESULT_BYTES`；`counts` 与文件重解析计数及表头括号数字三者相等；`model` 与 `archi_drift` 报告的 `model.elements` / `model.relations` 逐值相等

#### Scenario: Err 第一行自足且值域点名

- **WHEN** 以非法 `action` 与过期 `expect` 分别调用
- **THEN** 前者文案点名合法值（`upsert` / `delete`）与参数名；后者给 expect / 当前双值并含两条出路之一（`重试：` 或「省略 expect 直接覆盖」）；两例均零落盘

## MODIFIED Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经 `SessionToolset` 受控注入），语义为**定位**（已知在看哪个模块）：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine` / `entry`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、**`doc`（单值入口文档路径，仅 `api = none` 元素有值）**、**`api`（并列契约状态字段）**、**`resolvedEntry`（入口解析结果）**、**`fileHash`（声明文件的单文件内容哈希）**。`docs` 数组与 `warnings` 数组两个位次已退役。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / **`api.status`** / **`hasPath`**，无邻接、无入口文档、无契约路径），并受既有单结果字节上限约束（超限截断留痕）。
- **`resolvedEntry`（入口解析结果）**：SHALL 为 `{ path: string | null, via: "explicit" | "convention" | null, problem: string | null }` 三字段（字段不省略）——`path` 为 repo 相对 POSIX 入口路径，`via` 区分「模型显式 `entry`」与「约定默认命中」（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`），`problem` 在入口不可用或不存在时给出人读成因。判据 SHALL 由 `api::paths::entry_state` **单点**求值（MUST NOT 在工具执行体或 `dto` 内另写入口解析、MUST NOT 复用 `apiFindings` 的判定分支）；该字段 MUST NOT 承载 `entry` 四条判据的长表（判据定义归 `desktop-archi-api` / `desktop-archi-api-check`）。
- **`hasPath`（`path` 事实）**：薄概览每条 SHALL 含 `hasPath: bool`（= 模型 `path` 数组非空，纯投影不补默认值），使「谁缺 `path`」一次调用即可判出；MUST NOT 以「`path` 字段是否省略」表达同义（显式字段不省略是既有投影纪律）。
- **`fileHash`（单文件内容哈希，本变更新增）**：单元素响应 SHALL 含 `fileHash: String`（形态 `sha256:<hex>`），值 SHALL 为「该元素声明文件（`modelFile`）的当前内容哈希」，与 `drift::file_hash` **同一函数产出**（MUST NOT 在执行体或 `dto` 内另算一份），`modelFile` 已在手 ⇒ 零额外读盘。该字段是**写面乐观锁的读侧闭环**：其值可直接作 `archi_element` / `archi_relation` 的 `expect`。语义边界两条：① MUST NOT 与 `archi_drift` 报告的 `modelHash` 比较（后者是全量聚合、非同粒度）；② MUST NOT 承担「模型是否被改动」的判定（那是 `assets_hash` 的用途）。薄概览 MUST NOT 携带该字段（一元素一次读盘，不给全表付账）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`api` 并列字段**：单元素响应 SHALL 并出 `api` 对象（`status`（`declared` / `none`）/ `contractPath` / `schemaPath` / `generatedPath`），定位 SHALL 按 `<archi_models_dir>/<分层目录…>/<FQN>.md` 与相应的 `.schema.json` / 生成物路径**派生**（全经 `foundation::layout` 的派生族与 `api::paths` 单点，MUST NOT 行内拼路径；`api` 段已退役，契约资产落在契约资产目录 `models/` 的 `<分层目录…>/` 子目录内）；文件不存在即 `status = none` 且三个路径字段为 `null`（零配置，「有契约 / 无契约」成为可枚举事实）。该字段 MUST NOT 承载协议正文（正文由 agent 用 `read` 按路径自取），MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵。
- **`doc` 并列字段**：`doc` SHALL 为 repo 相对 POSIX 路径**单值**或 `null`（无 `title` / `kind` / `source` 四字段形态）。只有 `api = none` 的元素会填；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线。`doc` 指向的文件不存在时该元素的判据面报 `docNotFound`（`error` 档）——工具响应本身 MUST NOT 为此降级或补默认值。
- **描述面（`mount.rs`）**：`archi_module` 的工具描述 SHALL 含四句——「缺 `path` 的元素可由 `hasPath` 一次判出」「`resolvedEntry` 给入口解析结果（显式 / 约定默认 / 不可用成因）」「新元素字段形态用 `archi_scaffold` 取模板」「`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」，并另加一句「`fileHash` 可直接作写工具的 `expect`」；且 MUST NOT 复制字段语法长表（字段序 / 内联 `metadata` 的形态知识归模板工具）——MUST NOT 残留 `docs` 词汇或警告语义。

#### Scenario: 定位单元素

- **WHEN** 模型会话中调用 `archi_module(element="App.Auth")`
- **THEN** 返回该元素的 `kind` / `gate` / `path` / `metadata` / `modelFile` / `modelLine` / `entry`，其出边与入边（各含对端 FQN 与 `description`）、直接子元素清单、`doc`（单值或 `null`）、`resolvedEntry`、`fileHash`；响应内 MUST NOT 出现 `docs` 数组

#### Scenario: 省略元素给薄概览并截断

- **WHEN** 在不传 `element` 的调用下模型含数百元素
- **THEN** 返回全元素薄概览（含 `hasPath`，无邻接、无入口文档、无 `fileHash`），超出单结果字节上限时截断并留痕（提示可指定元素收窄）

#### Scenario: 缺 path 一次判出

- **WHEN** 省略 `element` 调用，模型内既有声明了 `path` 的叶元素、也有无 `path` 的容器元素
- **THEN** 前者 `hasPath = true`、后者 `hasPath = false`（字段不省略）；`path` 数组本身只在单元素响应出线

#### Scenario: 入口三态可辨

- **WHEN** 分别对「显式 `entry` 命中」「目录型根由 `index.ts` 约定默认命中」「`path` 指向不存在目录而入口不可用」的三元素调用 `archi_module(element = …)`
- **THEN** 依次为 `resolvedEntry = { path: "<显式值>", via: "explicit", problem: null }` / `{ path: "<index 路径>", via: "convention", problem: null }` / `{ path: null, via: null, problem: "<不可用成因>" }`；判据与 `apiFindings` 的 `apiEntryMissing` 逐例一致（同一 `entry_state` 求值）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 并列字段两态可辨

- **WHEN** 分别对契约在案的元素（`Backend.Infra.ArchiRuntime`）与无契约元素调用 `archi_module(element = …)`
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `.dev-team/architecture/models/Infra/` 下的实际文件、`generatedPath` 指向入口同级生成物；后者 `api.status = "none"` 且三者 `null`

#### Scenario: doc 只在 api = none 元素上有值

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、`api = declared` 的元素调用 `archi_module`
- **THEN** 依次为「路径串」/ `null` / `null`（字段不省略）；declared 元素的入口文档只能经 `api.contractPath` 读到——两处 MUST NOT 同时给出口 entry 文档（否则元素信息面板会出现两条）

#### Scenario: 薄概览带 api.status 但不带路径与 doc

- **WHEN** 省略 `element` 调用 `archi_module`
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`）与 `hasPath`，不含 `contractPath` / `schemaPath` / `generatedPath` / `doc` / `resolvedEntry` / `fileHash`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: fileHash 闭环可走通

- **WHEN** 先 `archi_module(element = X)` 取 `fileHash`，再调 `archi_element { action: "upsert", element: X, summary: "…", expect: <fileHash> }`，随后用响应里的 `hash` 再调一次
- **THEN** 第一次写成功；两次 `hash` 与各自落盘后 `drift::file_hash(声明文件)` 逐字相等；把 `archi_drift` 报告的 `modelHash` 当 `expect` 用则 `Err`（两个哈希不同粒度，MUST NOT 互比）

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径）、`doc` 单值、`resolvedEntry` 三字段、`fileHash` 与 `hasPath` 布尔；零签名、零 `$defs`、零散文、零四字段文档条目；正文由 agent 用 `read` 按路径自取

#### Scenario: 契约路径零 `api` 段残留

- **WHEN** 对本仓契约在案元素（`Backend.Infra.ArchiRuntime`）取 `archi_module` 响应，并审查响应内的三个路径串
- **THEN** `contractPath` 为 `.dev-team/architecture/models/Infra/Backend.Infra.ArchiRuntime.md`、`schemaPath` 为同目录的 `.schema.json`；三串内 MUST NOT 出现 `architecture/api` 形态（旧段已退役）；派生点消失后仍由 `api::paths` 单点产出（工具执行体零路径拼接）

### Requirement: archi_scaffold 只读工具的新元素模板契约

SDK 引擎 SHALL 提供专用只读工具 `archi_scaffold`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**给字段词典**（要写新元素但记不住字段形态）——草稿同时是最新字段面的可抄改模板，一个工具一个形状（MUST NOT 另设 `mode = "draft" | "dict"` 双形态，亦 MUST NOT 改出纯 Markdown 词典）：

```jsonc
{ "name": "archi_scaffold",
  "parameters": { "kind": "可选。元素 kind 四值之一（值集由 ElementKind::ALL 派生）；省略取 domain" } }
```

- **值集封闭且派生**：`kind` 的合法值 SHALL 由 `ElementKind::ALL` 派生（MUST NOT 手抄四值）；词汇外值 SHALL 以 `is_error` 回灌并**点名**合法值（MUST NOT 静默退化为缺省）。
- **响应形状**：`{ "draft": "<TOML 片段>" }`——**一段**元素表模板，字段按模型字段面顺序列出（`kind` 实值；`gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 以注释行给出并各附一行纪律），另附一条 `[[relation]]` 模板注释。字段顺序 SHALL 与 `archi::parse::ELEMENT_FIELD_NAMES` 同序（= `RawElement` 声明序，见 `desktop-archi-model`「元素字段面的机器可读清单」）。
- **描述面不枚举字段名**：工具定义（`mount.rs` 的 `archi_scaffold_definition()`）的描述 SHALL 以词典措辞表述（「字段词典 / 逐字段一行纪律 / 值域 / 关系四键」），MUST NOT 内联枚举具体字段名（如「字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板」）——枚举会随字段面扩容静默陈旧，与 `archi_module` 描述「MUST NOT 复制字段语法长表」同一条纪律。
- **字段词典语义（每个字段一行纪律）**：草稿 SHALL 对八个字段面字段逐一到场并各给一行纪律，覆盖四类事实（字段各取其一或多项）——① 值域（`kind` 四值语义 / `gate` 三值 / `api` 二值）；② 缺省档（`gate` 缺省 `off` / `api` 缺省 `declared` / `entry` 与 `doc` 选填）；③ 基数与形状（`path` 随 `kind` 的基数、`./` 前缀、目录尾 `/`、文件到扩展名）；④ 单值性（`entry` / `doc` 是单值）。纪律 MUST 与模型侧判据同源：`metadata` SHALL 写明值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；关系 `apis` SHALL 写明四条——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）、逐项非空、同名不重复、`apis = []` 与省略同义（显式空数组被拒）。
- **字段面全集对拍**：草稿 SHALL 含字段面全集的逐字段行——断言 SHALL 遍历 `archi::parse::ELEMENT_FIELD_NAMES` 做**行首匹配**（`{field} = ` / `# {field}：` / `# {field} = `），MUST NOT 退化为整串 `contains(field)`（`apis` 会让 `api` 假命中，`doc` 一类行内复述会掩盖整行缺失）。消费侧 MUST NOT 手抄八个字段名字面量。
- **值域派生对拍**：草稿的门 / 状态纪律行 SHALL 逐值覆盖 `GateTier::ALL`（`off` / `warn` / `strict`）与 `ApiStatus::ALL`（`declared` / `none`），且缺省档（`off` / `declared`）以「缺省」字样在场；`kind` 四值语义由 `ElementKind::ALL` 逐值覆盖。三组枚举新增变体而未补词典即红。
- **模板自证可解析且零结构诊断词**：草稿 SHALL 为合法 TOML 且经 `archi::parse` 的 `TomlParser` 解析后**零结构诊断**（机械锚：字段名 / 键形 / `metadata` 形态 / `kind` 四值语义任一漂移即红）。纪律性提示 SHALL 只出现在注释行——MUST NOT 以注释外的非法行（如 `path = "<待填>"`）破坏可解析性。草稿 MUST NOT 含结构诊断词（`invalidValue` / `unknownField` / `missingField` / `syntaxError` 零命中）：值面规则只写行为（「被拒」），诊断词表不进词典（与「结构诊断全量词表 MUST NOT 进知识文本」同源）。
- **零写盘**：本工具 MUST NOT 创建 / 覆盖 / 删除任何文件（与 `archi_api` 的 `bootstrap` 同款「只出 stdout 语义」）；MUST NOT 成为「写模型工具」——写模型的 canonical 入口是 `archi_element` / `archi_relation`，手改面仍是既有 `write` / `edit`（见「模型 = 源码（两条写入口与写纪律）」）。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；草稿体量 SHALL 远低于该上限（字段词典是「一行一字段」，MUST NOT 演化成判据长表）。
- **只读档位**：归 `ToolTier::Read`（三档全放行），无路径参数（沙箱原样返回、写入保护零射程）。

#### Scenario: 草稿可解析且含必填提示

- **WHEN** 调用 `archi_scaffold()`（省略 `kind`）与 `archi_scaffold(kind = "module")`
- **THEN** 两次均返回单段元素表草稿（后者的 `kind` 行取 `module`）；草稿经 `TomlParser` 解析零结构诊断；`path` 的「模块必填（可多条）/ 包至多一条包根 / `domain` · `composite` 选填」与 `metadata` 的「内联表与子表等价」两句以注释行在场，MUST NOT 再出现「子表会被读成另一个元素」一类假因果

#### Scenario: 模板 kind 行覆盖四值语义

- **WHEN** 遍历 `ElementKind::ALL` 四次调用 `archi_scaffold(kind = …)`
- **THEN** 每次草稿的 `kind` 纪律注释都列出四个值各自的语义（`package` 至多一条包根 / `domain` 领域容器 / `module` 模块 / `composite` 纯分组），MUST NOT 缺任一值——`composite` 不得再是「零承载」的第四值

#### Scenario: 词汇外 kind 被拒

- **WHEN** 调用 `archi_scaffold(kind = "service" )`（词汇外值）
- **THEN** 返回 `is_error` 并点名 `package` / `domain` / `module` / `composite` 四值；零写盘、零副作用

#### Scenario: 模板不写盘

- **WHEN** 在任一相位调用 `archi_scaffold` 并比对调用前后的架构域字节
- **THEN** 磁盘零变化（模型文件、契约资产、生成物均逐字不变）

#### Scenario: 草稿覆盖元素字段面全集（行首匹配，子串不顶替）

- **WHEN** 遍历 `archi::parse::ELEMENT_FIELD_NAMES`（八项：`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）并逐字段在草稿里找**行首形态**（`{field} = ` / `# {field}：` / `# {field} = `）
- **THEN** 每个字段名都恰有一个行首载体（`kind` 是唯一的实值行，其余为注释行）；删掉任一行首载体即红——删 `# api：` 行后 `apis` MUST NOT 顶替 `api`；新增字段而未补词典（草稿缺该字段的行首载体）同样即红

#### Scenario: 值域逐值在场且缺省档可辨

- **WHEN** 调用 `archi_scaffold` 并遍历 `GateTier::ALL` / `ApiStatus::ALL` 检查草稿
- **THEN** `gate` 纪律行逐值含 `off` / `warn` / `strict` 且 `off` 以「缺省」字样在场；`api` 纪律行逐值含 `declared` / `none` 且 `declared` 以「缺省」字样在场；新增任一变体而未补词典即红；同一次响应里草稿对 `invalidValue` / `unknownField` / `missingField` / `syntaxError` 四词零命中

#### Scenario: metadata 与 apis 的值面纪律在场

- **WHEN** 审查草稿的 `metadata` 与关系 `apis` 两段注释
- **THEN** `metadata` 段含「值 MUST 是字符串」与「非字符串值被拒」，且「子表与内联表等价」仍在场；`apis` 段含「只写名字」「逐项非空」「同名不重复」「`apis = []` 与省略同义」「显式空数组被拒」「生成物面留空」「`archi_drift`」；`entry` 的注释行以 `# entry：` 起首且含「单值」（`doc` 行的「单值」MUST NOT 顶替它）；三段均不点名诊断词

### Requirement: archi 工具面注入与 (phase, role) 矩阵

archi 专用工具的**下发面** SHALL 由 `agent-runtime` 组合根以**工具名字清单**表达（`SessionInjections.tools: Option<Vec<String>>`；`None` = 七内建全量），MUST NOT 依赖已退役的枚举式 toolset 概念。清单内每个名字 SHALL 经 `mount::lookup` 成员校验（未知名显式 `Err` 携名与来源，不静默滤过）；自由字符串工具名 MUST NOT 进执行面。

会话装配（`crates/infra/agent/src/compose.rs`）SHALL 按 `(source, phase, role)` 决定下发面：

| phase | role | `archi_module` | `archi_search` | `archi_api` | `archi_drift` | `archi_scaffold` | `archi_element` | `archi_relation` |
|---|---|---|---|---|---|---|---|---|
| `proposal` | executor | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ | ✅ |
| `dev-design` | executor | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ |
| `dev-design` | evaluator | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `code-review` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份同名映射。`dev-design` 的 executor / evaluator **分两行**（`drift` 与两写工具只给写模型的执行侧），MUST NOT 合并成一行后按 role 分支。
- **两写工具只给写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂：矩阵下发写工具的 `(phase, role)` 集合 SHALL 与 `compose::model_write_lock` 判定为 `None`（不挂会话锁）的集合**逐格相等**——MUST NOT 出现「矩阵给了工具但写权锁着」或「锁开了却没有写工具」的错配（两条方向都设断言）。
- **`archi_drift` 只给两处直接产出漂移的相位**：`dev-design/executor`（写元素与边）与 `implement/executor`（改代码 ⇒ 未声明依赖）。`code-review` / `acceptance` MUST NOT 下发（越界判断仍只读模型面；「时机差」作为将来放宽的理由需独立裁决，MUST NOT 由本期实现顺手放开）。
- **`archi_scaffold` 只给两处写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂——模型写权之外的下发等于许诺一个不会被允许落地的模板。
- **`archi_api` 只给 implement/executor 与 archi 臂**：其余相位 MUST NOT 下发（生成物新鲜度由门禁与契约面板承担）。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ **七个** archi 工具名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`）+ `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）+ `before_done = Some(ArchiPreclose)`（收口前自检装配档位，见「收口前引擎自检（架构判据读数）」）。该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`），段数 < 4 或末段非数字即 `None`。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒七工具全开）。
- `source = "explore"` 臂的 **archi 面 SHALL 恒为空**（零 archi 工具，逐字沿用）；该臂的**完整**名字清单（六工具收窄 + `create_explore_note` + 只读 DB 工具 `db_tables` / `db_query` 两件）单源见 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」与 `desktop-agent-db-tools`，本能力 MUST NOT 另写一份；其余**非 change、非 config** 来源（debug 等）SHALL 恒「七内建全量 + 零 archi 工具 + `ask = Generic`」的显式构造（`source = "config"` 臂的名字清单与位取值单源见 `desktop-config-agent` 与 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」；该臂零 archi 工具、`preamble` 为配置协议知识而非架构会话知识，本能力 MUST NOT 另写一份；MUST NOT 以默认构造兜底——默认值的 `ask = None` 即静默禁用）。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（生产者 = archi 臂与 config 臂），前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中

- **WHEN** 分别以 8 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 为 `module` / `search` / `scaffold` / `element` / `relation`；dev-design 执行为 `module` / `search` / `drift` / `scaffold` / `element` / `relation`、评估为 `module` / `search`；implement 为 `module` / `api` / `drift`；code-review / acceptance 只 `module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 写工具与写权锁同集

- **WHEN** 遍历全部 `(phase, role)` 组合，对每个组合分别取矩阵结果与 `model_write_lock` 的返回值
- **THEN** 「矩阵含 `archi_element`」⟺「`model_write_lock` 返回 `None`」，两个方向逐格相等（archi 来源臂同判）；任一格不成立即测试红

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落七内建全量），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为七内建全量（含 `write`）另加**七个** archi 工具名、`preamble` 为架构会话知识文本、`before_done` 为 `Some(ArchiPreclose)`；definitions 含七个 archi 定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：七工具全开 + 七内建全量 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零 archi 工具

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note` + `db_tables` / `db_query`（只读 DB 工具两件，清单单源见 desktop-agent-db-tools），debug 为七内建（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

### Requirement: archi 工具档位归属与成员校验（五只读 + 两写）

七个 archi 工具 SHALL 按读写面分两档：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` 归 **只读面**（挂载清单 `sdk/mount.rs` 中准入档 SHALL 为 `ToolTier::Read`，`policy::allows` 对三档——`default` / `acceptEdits` / `bypassPermissions`——**全部放行**）；`archi_element` / `archi_relation` 归 **写面**（准入档 SHALL 为 `ToolTier::Write`：`default` 档拒绝，`acceptEdits` / `bypassPermissions` 放行，与 `write` / `edit` 同档）。MUST NOT 为任一 archi 工具引入单独的放行名单（准入判据单点在挂载清单的准入档）。

以下既有语义 MUST NOT 变化：

- **成员校验先行**：调用名 ∉ 本轮实际下发 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；
- **路径沙箱不介入**：七工具无路径参数，`sandbox` 改写面对其恒原样返回（MUST NOT 因缺 `path` 字段报错；两写工具的落点由 `file` / 父元素推得，`tools::input_path` 对其恒 `None`）；
- **写入保护按面分**：五只读工具为读面，`write_protection` 的守卫 MUST NOT 拦截；两**写**工具写盘前 SHALL 把解析出的目标文件过同一 `ProtectRules`（细则归 `desktop-write-protection`「会话级写入禁区（相位锁）」）；
- **未知工具仍拒**：词汇外的工具名在任何档位一律拒绝（封闭清单语义不变）。
- 工具执行上下文 SHALL 经 `LoopTurn` 的 per-turn 上下文路由（root + 模型读面），MUST NOT 把 store 句柄塞进七通用工具的 `execute` 纯函数签名；上下文缺席 SHALL 以 `is_error` 回灌（生产不可达）。铸造判据 SHALL 为「本轮名字清单含任一 archi 工具名」（**七名**逐一在场判断，MUST NOT 漏掉新增两名）。

#### Scenario: 默认档放行五个 archi 工具

- **WHEN** 以 `permissionMode = default` 在矩阵命中相位调用 `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`
- **THEN** 五者均放行并执行（无 `permission_denied`）；同档调用 `write` 与 `archi_element` / `archi_relation` 仍被拒（只读面未扩宽）

#### Scenario: 写工具在非默认档放行且先过成员校验

- **WHEN** 以 `permissionMode = acceptEdits` 在矩阵命中相位调用 `archi_element` / `archi_relation`；另在矩阵**未命中**相位以同一档位调用
- **THEN** 前者放行并执行；后者被成员校验拒绝（`tool_unavailable` + `is_error`，run 不中断、执行体零调用）——档位与成员校验是两道独立闸，任一不通过都不落到写盘

#### Scenario: 矩阵外调用被成员校验拒绝

- **WHEN** 在 `(test-gen, executor)` 会话中模型幻觉调用 `archi_search` 或 `archi_drift`
- **THEN** 收到 `tool_unavailable` + `is_error` ToolResult 回灌，run 不中断、执行体零调用

#### Scenario: 沙箱与写入保护按面分

- **WHEN** 在七个 archi 工具被下发的会话中审查 `sandbox` 改写与写入保护判定
- **THEN** 七者均原样通过沙箱（无 `path` 入参）；五只读工具零保护射程，两写工具只在解析出目标文件后受保护判定约束；受保护文件的 `write` / `edit` 行为逐字不变

#### Scenario: 挂载清单项数

- **WHEN** 审查 `mount::registry()` 的项数与归组
- **THEN** 恰 **18** 项：七内建 + `create_explore_note` + 五 archi 只读 + 两 archi 写 + 两 DB 只读 + `propose_config`；五只读项 `ToolTier::Read`、两写项 `ToolTier::Write`，七个 archi 项均 `ToolExec::Archi`

### Requirement: 模型 = 源码（两条写入口与写纪律）

架构模型文件 SHALL 与源码同权：agent SHALL 直改 `<archi_dir>/graph/model/*.toml`。**写模型有两个入口，MUST NOT 存在第三个**：

| 入口 | 形态 | 失败语义 | 定位 |
|---|---|---|---|
| 专用写工具 `archi_element` / `archi_relation` | 结构化参数面（`desktop-archi-agent`「archi 写工具面」） | **坏文本不可产出**：渲染 → 重解析自证 → 回读一致，任一失败即 `Err` 且零落盘 | **canonical**：键形 / 值域 / 祖先 / 计数 / 契约告警全在工具里 |
| `write` / `edit` 手改 | 自由文本 | **写坏模型仍是合法状态**：由读取面报错（结构诊断 `file:line`、画布停摆、`archi_preclose` 兜底） | **second path**：工具表达不了的（重排文件 / 批量整理）才用，改完必须自查 |

写权（谁能写）与写入口（怎么写）是两件事，MUST NOT 混说：

- **写权 SHALL 有且只有两条通路**，且**通路 A 按相位收窄**：

| 通路 | 触发面 | 执行根 | 确认时机 | 归属能力 |
|---|---|---|---|---|
| A（既有，按相位收窄） | change 流程相位会话——**仅 `phase ∈ {proposal, dev-design}` ∧ `role = executor`** | `detail.worktree` | 回合末归档确认面（模型区块 + 冲突分流） | `desktop-change-archive` |
| B（既有） | 架构页通用会话（`source = "archi"`），不限相位 | workspace root（主仓，`cwd = null` 回落） | 事后 `git diff` 评审 + 用户手动刷新校验 | `desktop-archi-chat` |

- **写权是强制面问题，不是工具面问题**：`archi` crate 全程只读盘（模型唯一落盘点是 `write_atomic` 单点），故「写入口」才分工具 / 手改两种；写权政策实做是**掐掉其余相位对模型目录的射程**——由会话级写入禁区（第三规则来源）承载，见 `desktop-write-protection`「会话级写入禁区（相位锁）」。两写工具只在写权放开的相位下发（矩阵与写权锁同集，见「archi 工具面注入与 (phase, role) 矩阵」）；MUST NOT 出现「有工具但无写权」或「有写权但无工具」的错配。
- **两条失败语义 MUST NOT 混为一句**：写工具面 MUST NOT 产出坏文本（自证链使它结构上不可达）；手改面 MUST NOT 因加工具而收紧——**写坏模型仍由读取面报错**（`archi_module` 的结构诊断带 `file:line`、画布停摆态、`archi_preclose` 读数）。MUST NOT 把「写盘前校验」当写权护栏（`write` / `edit` 通路零预校验是既有承诺）。
- **两条推论（政策是绝对命题，不是步骤枚举）**：
  - **`dev-design/evaluator` 被锁**：白名单按 `(phase, role)` 而非 phase——评审批次不写模型。
  - **`explore` / `debug` 也被锁**：两者不是「步骤」，但政策覆盖一切来源；explore 的 `edit` 只服务笔记，锁模型零损失。
- **不依赖运行时审批**：写盘那一刻 MUST NOT 要求用户确认（工具分派链只有 `deny`，无挂起等确认能力）。通路 A 的设计权由「回合必须人工确认」承载；通路 B 的设计权由事后 diff 评审 + 手动刷新可见性承载（MUST NOT 承诺写盘前拦截）。
- **内置锁面仍不纳入**：`<archi_dir>/graph/model/` 与架构域配置 MUST NOT 进入 `write_protection` 的**内置**锁面（内置规则是相位无关的绝对路径 glob，纳入即等于两条通路都改不了模型）；相位条件的锁 SHALL 走**会话级**规则面（见 `desktop-write-protection`「会话级写入禁区（相位锁）」）。
- **两通路并存，互不替代**：MUST NOT 因通路 B 存在而移除通路 A（change 侧 worktree 隔离 + 归档确认仍是「改动随 change 一起评审」的唯一形态）；亦 MUST NOT 因通路 A 收窄而收紧通路 B（架构页会话的用途恰是「不进 change 也能维护模型」）。
- **场景归属**：冷启动生成骨架 / 增量维护（新增或重构模块）/ 按现状补声明 / 直接修复图或文档错误，四类场景 SHALL 可走任一条通路（走 change 时落 `dev-design` 阶段，走架构页时落通用会话），MUST NOT 强制单一入口。非白名单相位遇到「模型该改」的需求 SHALL 把需求写进产物（design / tasks）交 `dev-design` 落地，MUST NOT 尝试绕过。
- **新元素缺省档**：两入口生成的新元素均缺省 `gate = "off"`（ratchet 逐块收紧）。
- **通路 B 的边界（软约束，非写入面强制）**：会话知识 SHALL 要求「禁止修改架构目录之外的文件（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）」，但写入面 MUST NOT 为该约束设硬隔离——该缺口 SHALL 在能力 spec 中如实留痕，MUST NOT 被表述为「会话只能改模型」。
- **生成物同权但不可手改**：四类契约生成物（`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）与模型文件同属磁盘可写面，但**没有专用写工具**——两写工具的 `file` 只接受模型声明目录一级 `*.toml`，射程 MUST NOT 扩到生成物；会话知识 SHALL 要求「MUST NOT 手改生成物，用既有命令重生成」——该约束同样只有事后兜底（零 diff 门 + 契约面板），MUST NOT 表述为写入面拦截；手改通路对生成物仍零写前校验。

#### Scenario: worktree 内直改模型成功（白名单相位）

- **WHEN** agent 在 `dev-design/executor` 会话中用 `write` 新建 `<archi_dir>/graph/model/03-billing.toml`，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后读取面解析出该改动，主仓模型零变化

#### Scenario: 白名单外相位写模型被拒

- **WHEN** `implement/executor` / `code-review/evaluator` / `dev-design/evaluator` 会话对 `<archi_dir>/graph/model/*.toml` 调用 `write` 或 `edit`
- **THEN** 拒绝串含路径、工具名、指路（写进 design / tasks 交 `dev-design` 落地，或改用架构页会话）与「不要改用其他命令或工具尝试绕过」；run 不中断、文件零变化

#### Scenario: 主仓直改模型成功（架构页）

- **WHEN** agent 在架构页通用会话（`cwd = null` → 主仓根）内用 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** 写入落在主仓该文件（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后同 workspace 的 `archi_module` 与画布取数读到该改动，改动保持未提交由用户 `git diff` 评审

#### Scenario: 写坏模型不阻断写盘（手改面）

- **WHEN** agent 用 `write` / `edit`（不是写工具）写出语法错的模型文件（白名单相位或通路 B）
- **THEN** 写盘成功；随后 `archi_module` / 画布数据返回结构诊断（`is_error` / 停摆态）且进程不 panic；收口时 `archi_preclose` 读数含该结构错

#### Scenario: 写工具面不可产出坏文本

- **WHEN** 以任何合法入参调用 `archi_element` / `archi_relation`，并构造一个会渲染出诊断的边界（如 `apis = [""]` / 非法 `path` 形态）
- **THEN** 一律 `Err` 且**零落盘**（自证链在交文本之前拦下），盘上字节零变化；该失败语义不得被表述为「与手改面相同」

#### Scenario: 两入口并存且无第三条通路

- **WHEN** 审查执行面工具清单与写模型入口
- **THEN** 写模型恰有两个入口（专用写工具 canonical / `write`+`edit` second path），工具面在写权相位下发两个写工具且在非写权相位零下发；MUST NOT 存在第三条通路（如独立 IPC 写命令或画布写入口）；`archi_set_gate` 与写工具共用同一 `plan_element_upsert`（同实现 ⇒ 同文本）

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下**四段**（本变更是骨架的第二次收缩：语法类知识迁往工具面，散文只剩职责、工具面指路与判断面）：

- **职责（C）**：模型只写**结构**（元素 / 依赖）与三个**指针**（`api` / `entry` / `doc`）——接口、导出符号、协议正文一律归契约资产，MUST NOT 由指针扩张出正文；**只改架构目录内的文件**（源码 / 配置 / 脚本 / 域外文档一律不改，生成物重生成除外）；**直改主仓**、改动随 `git diff` 评审；写盘后画布**不会自动更新**，须提示用户「刷新」；**续接历史会话先取当下模型**（`archi_module`），MUST NOT 依赖会话早前的描述。
- **工具面（A/B，指路）**：字段面、键形、缺省、值域、祖先与计数全部由工具承担——MUST NOT 凭记忆手拼 TOML：
  - 读五件各一句「何时用」：`archi_search`（发现）/ `archi_module`（定位与现状）/ `archi_drift`（漂移读数）/ `archi_api`（契约两轴 + 草稿）/ `archi_scaffold`（字段词典）。
  - 写两件：`archi_element`（元素 upsert / 删除，可内联出边）/ `archi_relation`（单条关系 upsert / 删除）——参数面即字段清单，坏文本零落盘。
  - **手改出口（永久内容）**：工具表达不了的（重排文件 / 批量整理）才手改 TOML，改完必须 `archi_module` 自查——坏模型由读取面报错（结构诊断带 `file:line`），不是静默的。
  - 生成物过期用各仓既有生成命令重生成，MUST NOT 手改生成物；自检纪律：改完 `archi_drift` 非空即逐条修、`archi_api`（`action = "check"`）查两轴。
- **留给判断（C）**：`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——这些没有机械答案；工具的 `kind` / `gate` / `api` 参数面给出全部合法值与语义；**纯分组容器用 `composite`**。
- **派生路径段（程序追加，逐字保留）**：模型声明目录（`archi_graph_model_rel_dir`）与契约资产（`<archi_models_rel_dir>/<分层目录…>/<FQN>.md`）两处由 `foundation::layout` 派生；骨架 MUST NOT 手写目录字面量，MUST NOT 写契约索引 `INDEX.toml` 的方位（生成物，零手写面，需要时经 `archi_api` 的 `generatedPath`）。
- **架构域看护配置（B，一行）**：`<archi_dir>/config.json` 管 `ignore` / `exclude`，未知键与缺 `reason` 即配置非法——非法时 `archi_drift` 以 `is_error` 点名；MUST NOT 在知识文本里复述字段长表。

**语法知识的值域与形态 MUST NOT 进知识文本**（本变更是**迁出**，不是新增）：

- `gate` / `api` 的值域与缺省档 SHALL 由工具面承载（`archi_element` 的 `gate` / `api` 参数 `description` ← `GateTier::ALL` / `ApiStatus::ALL` 与 `default()`）与 `archi_scaffold` 词典各一行；骨架 MUST NOT 再写这两行（值域在工具面是**唯一**持有者）。
- `kind` 四值语义 SHALL 由 `ElementKind` 枚举的 `description`（`ALL` 派生）+ `archi_scaffold` 词典的 `kind` 纪律行承载；骨架 MUST NOT 逐值枚举。
- **键形与字段形态 MUST NOT 进骨架**：带引号 FQN 单键 / 未引号点分的失败形态 / `metadata` 子表与内联表等价 / `[[relation]]` 四键 / `path` 基数三口径，全部由「工具渲染（不产坏文本）+ parser 诊断（转述，零二次判据）+ `archi_scaffold` 词典」承载。
- **结构诊断全量词表 MUST NOT 进知识文本**，且本变更后**两处例外也取消**：`orphanHierarchy` / `dependsOnAncestor` 亦不再出现在骨架——祖先缺失由写工具的 `Err` 承载、越界依赖由 `archi_drift` 读数承载。知识文本因此不是任何诊断词表的消费面。
- `entry` / `doc` / `apis` SHALL **不进**任何值域词表：三者的值域是任意 repo 相对路径 / 任意代码标识符（开放集），判据定义归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-drift`，不在知识文本内复制判据长表。
- 知识文本 SHALL 仍为**静态骨架 md（`include_str!`）+ 两处目录路径段**在装配侧拼成的单条文本；`gate` / `api` / `kind` 的「词汇 ↔ 文本」对拍面随值域迁出改为工具参数与草稿（见上三条），MUST NOT 在骨架内恢复对拍锚。

两条工程约束：

- **自证可解析**：字段形态的机械锚 SHALL 由 `archi_scaffold` 草稿（`TomlParser` 零诊断）与 `archi_element` 参数 schema 承担；知识文本本身 MUST NOT 承载示例 TOML、键形示例或字段值域行（示例是重复的第二事实源）。
- **尽力增强、不炸 run**：知识拼装失败（派生异常 / 读取失败）SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符（路径由派生注入），体量 SHALL 受上限约束（**骨架 ≤ 1500 B、全文 ≤ 1700 B，实测值冻结**；测量口径沿用既有 `lf_bytes(trim_end)`——LF 归一、去尾随换行；基线 2700 / 2850 随语法段迁出而下调，MUST NOT 抬锚）。超限 SHALL 压缩同段冗余表述，MUST NOT 删除职责 / 手改出口 / 留给判断三段的任一条。收口自检的装配与本知识解耦（`before_done` 档位按来源置位），知识降级 MUST NOT 影响自检。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 四段结构在骨架内可指认

- **WHEN** 审查骨架 md 的分段
- **THEN** 恰含四段语义：职责（模型 = 结构 + 三指针 / 只改架构目录 / 主仓直改随 diff 评审 / 写盘后提示刷新 / 续接先取当下模型）、工具面（读五件的「何时用」+ 写两件的「参数面即字段清单」+ **手改出口一句**：工具表达不了的才手改、改完必须 `archi_module` 自查、坏模型由读取面报错带 `file:line`）、留给判断（`kind` / `path` / `api` / 依赖四个判断面 + 纯分组用 `composite`）、派生路径段；MUST NOT 出现第五类语法教学段

#### Scenario: 语法知识零残留

- **WHEN** 对骨架做机械检查
- **THEN** 零示例 TOML 围栏（` ``` ` 计数为 0）、零键形示例（`[element.` 前缀零命中）、零字段值域行（`off` / `warn` / `strict` / `declared` / `none` 不同时作为值域枚举出现）、零结构诊断词（`orphanHierarchy` / `dependsOnAncestor` / `invalidValue` / `unknownField` / `missingField` / `syntaxError` 全零命中）、零 `entry` 三档长表

#### Scenario: 手改出口是永久内容

- **WHEN** 审查骨架「工具面」段的最后一句
- **THEN** 该句在场并含三要素：① 「工具表达不了的才手改」的适用边界；② 「改完必须自查」（点名 `archi_module`）；③ 坏模型由读取面报错（带 `file:line`）；该句 MUST NOT 被表述为过渡措施或待删项（L3a 是终态，L3b 不做）

#### Scenario: 值域迁到工具面而非丢弃

- **WHEN** 分别审查骨架与两工具的 `parameters` / `archi_scaffold` 草稿
- **THEN** 骨架零值域行；`gate` / `api` 的合法值与缺省档在 `archi_element` 的对应 `description` 内逐值在场（由 `ALL` 派生）且在草稿词典各有一行；`kind` 四值语义在枚举描述与草稿词典内逐值在场——「散文删掉的」都能在工具面查到

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 七 archi 工具不变），MUST NOT 抛装配错误；收口前自检照常装配（`before_done` 档位与知识解耦；T1 / T2 皆假时仍零 notice）

#### Scenario: 自检纪律与可见性在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完可调 `archi_drift` 取读数并按条修」「写工具坏了/手改写坏 → 结构诊断经 `archi_module` 带 `file:line`」「契约面可调 `archi_api` 查两轴」「生成物过期用本仓的生成命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」六条；且知识文本内零 `<>` 占位符

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 1500 B、全文 ≤ 1700 B 且实测值被冻结断言；程序追加段恰含模型声明目录与契约资产两处方位，**零契约索引串**，骨架零目录字面量

### Requirement: 知识分层与工具面同批移动

架构会话知识文本的内容面 SHALL 按**可判定性 × 时机**两轴分四档，并把每一句的落档写进本能力（分层判据单点，MUST NOT 在别处另立一份分类）：

| 档 | 判据 | 承载 |
|---|---|---|
| **A 工具承载** | 机器能算 ∧ 动作后才需要 | 工具响应 / `Err`（MUST NOT 在知识文本里复述读数） |
| **B 上下文·一行指针** | 机器能算 ∧ 动作前就需要 | 知识文本**一行**，内容是「何时查哪个工具」或一个派生串 |
| **C 上下文·散文纪律** | 不可判定（语义 / 意图） | 知识文本散文 |
| **D 模板工具** | 不可判定 ∧ 动作后才需要（形态） | 模板工具输出（`archi_scaffold`） |

- **A 类句子 MUST 携带工具指路且工具面确有该字段**：知识文本里每条「来自 A 档」的纪律 SHALL 点名承载它的工具（`archi_module` / `archi_drift` / `archi_api` / `archi_scaffold` / `archi_element` / `archi_relation`），且该工具的响应或 `Err` 面 SHALL 确有对应字段 / 判据。任一方向缺失（句子里点了不存在的字段、工具加了字段但句子没指路）即违约——由机械断言表守（一行为「纪律句关键词 → 工具名 → 工具面字段」）。
- **分层第一判据（本变更新增）：工具可承载的知识 MUST NOT 留在散文**。某条知识若「机器能在动作前算出」且「写面是结构化的」，它 MUST 落在工具的参数 `description` / 响应 / `Err` / 词典（A / D 档），**MUST NOT** 以散文复述——散文里再写一份即「同一事实两份持有者」。语法知识的 C 档身份只允许残留于**手改面**（见补充规则①）。
- **四条补充规则 SHALL 逐条在案**：
  1. **形态错误不静默，失败面按入口分**：未引号点分表头 `[element.App.Auth]` 归 `unknownField`、**整份源解析失败**（全或无）；`metadata` 子表与内联表**等价**、零诊断。两者都有**行为**测试钉住（`parse/toml_test.rs` 的 `未引号点分表头被拒且不与合法形态合并` / `metadata子表与内联表等价` / `metadata子表成为唯一声明即缺kind`）。**归 C 档的依据**：手改面仍是自由文本 `write` / `edit`，结构诊断只在写盘后可得（动作前不可判定）；**结构化写面（`archi_element` / `archi_relation`）对工具通路消除了该前提**——工具渲染不可能产出未引号点分与点分嵌套，故这条知识在工具面是 A/D 档（由渲染与 parser 诊断承载），只在手改面残留为 C 档一句。MUST NOT 再表述为「会被静默读成另一个元素」或「`deny_unknown_fields` 不触发」——后者已被行为测试否证，不是任何档位的依据。
  2. **写入定位必须留一行**：新元素的模型落点（默认父元素所在文件、可由 `file` 覆盖）与手写契约的落点（契约资产目录）都在动作前需要，且新元素的契约三路径为 `null`（`api.contractPath` 指不到不存在的东西）⇒ 归 B 档，各留一行。落点的**推演规则**归工具（`archi_element` 的 `file` 参数描述），骨架只留「工具负责落点」的指路。
  3. **`kind` 的「选哪个」不可脚本化，「四值语义」是词表**：判断面（容器 = `domain` / crate、目录与单文件组件 = `module` / 包根 = `package`）归 C 档；四值语义是封闭词表 ⇒ 归工具参数 `description` 与 `archi_scaffold` 词典（MUST NOT 在散文里再抄一份）。
  4. **结构性事实归响应，不归散文**：祖先先声明（`orphanHierarchy`）与计数（`（N 元素 / M 关系）`）在写工具面分别是 `Err` 与响应 `counts`（工具重算并回填表头），故骨架 MUST NOT 写「子元素须先声明祖先」与「计数以工具读数为准并同批更新表头」两段——它们已是 A 档。
- **移动分层的原子性**：把某句从散文改判为 A/D 档时，**同批**必须落地「工具面确有该字段 / 模板 / 判据」与「散文句替换为指路句」两半——只删不加（知识缺失）与只加不删（知识重复）同样违约。
- **本变更的移动清单**（机械断言逐项在案）：键形与字段形态（438 + 827 B）→ 写工具渲染 + parser `Err` 转述 + `archi_scaffold` 词典；`kind` 四值语义（454 B）→ `ElementKind` 枚举描述 + 词典；层级与读数（330 B）→ 写工具 `Err`（祖先）/ 响应 `counts` + 表头回填（计数）/ `archi_drift` 报错点名（`config.json`）；保留面 = 职责、工具面指路、手改出口、留给判断（边界与自检 / 写面两段压成四句）。

#### Scenario: A 类句子逐句有工具可指

- **WHEN** 对知识文本的 A 档纪律句逐条取「关键词 → 工具名 → 工具面字段」三列核对
- **THEN** 每条纪律句都点到承载它的工具名，且该工具的响应 / `Err` 确有对应字段（`hasPath` / `resolvedEntry` / `fileHash` / 结构诊断 `kind` + `file:line` / `counts` / 七条 `warnings`）；任一列为空即测试红

#### Scenario: 删掉的散文都能在工具面查到

- **WHEN** 逐条取本变更移动清单（键形 / 字段形态 / `kind` 四值 / 祖先 / 计数 / `config.json`）在骨架里确认已无散文，再到工具面确认承载者在场
- **THEN** 六项全部两侧成立：骨架零残留、工具面有对应字段或判据（`archi_element` 参数描述 / `archi_scaffold` 草稿行 / `EditError` 判据 / 响应 `counts` / `archi_drift` 报错）；任一侧缺失即红

#### Scenario: 只删不加即违约

- **WHEN** 从知识文本删掉字段形态段而未同时落地写工具的参数面与渲染自证（或反之：加了写工具但散文仍抄一份字段纪律）
- **THEN** 视为分层未完成——机械断言表对「移动清单」逐项要求两侧同时在场（工具面 + 指路句），单侧缺失即红

#### Scenario: 手改面的 C 档残留可辨

- **WHEN** 审查骨架里与形态有关的那句话
- **THEN** 只剩「工具表达不了的才手改、改完自查、坏模型由读取面报错（`file:line`）」三要素；MUST NOT 出现「子表 / 点分嵌套会被读成另一个元素」的假因果，MUST NOT 出现「`metadata` 必须内联表」的禁令，MUST NOT 出现键形示例与值域行

## Module Contract

### Function: `crates/infra/agent/src/sdk/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `mount.rs` | 改 | 两常量 `ARCHI_ELEMENT_TOOL = "archi_element"` / `ARCHI_RELATION_TOOL = "archi_relation"`；两份平铺定义（`archi_element_definition()` / `archi_relation_definition()`）：逐字段 `description`、`enum` 由 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / 两处二值词表派生、八行上限取自 `WRITE_LIMITS`；静态清单 16 → **18** 项（两写项 `ToolTier::Write` / `ToolExec::Archi`）；`ToolTier::Read` / `Write` 与 `ToolExec::Archi` 的 doc 名单同步；`registry()` / `build_registry()` 的 `Vec::with_capacity` 随动 |
| `archi_tool.rs` | 改 | `ArchiContext::execute` 加两臂（第六、七）：E1 入参解析与硬闸（`WRITE_LIMITS`）→ `ElementSpec` / 关系三态映射 → `edit::plan_*` → 目标文件过 `ProtectRules` → `write_atomic` 落盘 → 三通道响应装配（`warnings` / `changed` / `relations` 四计数 / `counts` / `model` / `hash`）；E2 前置走 `dto::graph_diagnostics_error`；E3–E7 经 `EditError::message()`；输出过 `tools::cap_result`；零自拼 TOML、零第二份判据 |
| `atomic.rs`（新，`crates/infra/agent/src/sdk/atomic.rs`） | 新增 / 迁移 | `write_atomic(path, content)`（同目录临时文件 + rename，失败清临时文件）为**唯一落盘点**，壳层 `commands/atomic.rs` 改薄委托；MUST NOT 出现第二份「临时文件 + rename」实现（落位登记单份持有在 `desktop-config-agent`） |
| `runner.rs` | 改 | `mint_archi_context` 的名字清单判据由五名扩为**七名**（任一在场即铸） |
| `tools.rs` | 改 | `input_path` 的 archi 名单加两名字（返回 `None`——无 `path` 入参，落点由工具推得）；两常量再导出 |
| `protect/mod.rs` | 改 | 写工具的目标文件判定复用既有 `ProtectRules::evaluate_path`（零第二份规则集、零第二份拒串） |
| `policy.rs` | 零改动 | 准入仍是 `allows(mode, tier)` 单点四行映射；两写工具的档位由挂载清单给定 |

### Function: `crates/infra/agent/src/compose.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_matrix` | 改 | 两行加两工具（`proposal/executor`、`dev-design/executor`）；其余行逐字不变；两写工具的勾选集合与 `model_write_lock` 的不挂锁集合逐格相等（测试断言） |
| archi 臂名字清单 | 改 | 五 archi 名 → **七** archi 名（读五 + 写两）；`preamble` / `before_done` / `ask` 三位逐字不变 |
| `model_write_lock` | 零改动 | 政策与锁面逐字不变（两写工具的下发面与之对齐，而不是放宽/收紧它） |

### Function: `crates/infra/agent/src/archi_knowledge.rs` / `archi-knowledge.md`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi-knowledge.md` | 改 | 六段 2597 B → 四段 ≤1500 B（职责 / 工具面 / 留给判断 / 派生路径段）；语法段（键形 / 字段纪律 / `kind` 四值 / 层级读数）迁出；手改出口一句为永久内容；零示例 TOML、零值域行、零结构诊断词 |
| `archi_knowledge.rs` | 改 | 模块头注释随动（骨架描述与四段对齐）；`build` / `knowledge` 的拼装与降级语义逐字不变（骨架 + 两处派生目录） |

### Function: `crates/core/workflow/src/prompts/*.md`（矩阵命中 2 份）

| 项 | 变化 | 描述 |
|---|---|---|
| `proposal-planner.md` / `dev-design-planner.md` | 改 | 工具行补 `archi_element` / `archi_relation`；写面句由「经既有 `read` / `write` / `edit` 维护（**无专用写工具**）」改为「用 `archi_element` / `archi_relation` 写（工具表达不了的才手改）」；保留「只为已确定元素落笔」与自查句；零字段形态长表 |
| 其余 12 份 md | 零改动 | 矩阵未命中者 MUST NOT 出现两写工具字样（防许诺未下发工具面） |

### API: `crates/core/archi/src/dto.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiModuleDetail.fileHash` | 新增字段 | `String`（`sha256:<hex>`），由 `drift::file_hash(声明文件)` 产出；`specta::Type` 出线 ⇒ `bindings.ts` 重导（一字段 + JSDoc）、`bindings.zod.ts` 的 `ArchiModuleDetailSchema` 同字段（一字段，无其它 diff）；薄概览条目（`ArchiElementBrief`）不携带 |
| 其余 DTO | 零改动 | 字段名 / 顺序 / 语义逐字不变；`ArchiGraph` / `ArchiElement` / `ArchiRelation` / `ArchiRelationDetail` 不动 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| 九个 IPC 命令 | 清单与签名零变化：两写工具是 **agent 工具**，不是命令；`archi_set_gate` 仍走同一 `plan_element_upsert`（产同一份文本），签名 / `Result<(), String>` / 失败零写入不变 |
| 画布 `incremental` 改档 / 契约面板 / 门禁 | 逐步零变化；`archi_set_gate` 命令面不暴露 `expect`（前端单用户、无并发） |
| 生成物与架构域数据 | `.dev-team/architecture/**` 零字节变化（55 元素 / 129 关系 / 六个表头计数逐字不变）；`api:check` 零差异、`api:export` 零 diff |
