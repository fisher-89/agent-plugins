# desktop-archi-agent Specification（delta: archi-agent-context-review）

## Purpose

本 delta 把 agent 侧读面改成「一次给全三份资产」，并给起草相位补齐草稿、引用计算与写作纪律。

- **`archi_module`**：新增 `assets`（`topology` / `interface` / `doc`，三者各自 `present`）+ `reading`；`api` 三路径各自独立出线（`status` 改判 schema 在盘）。
- **`archi_search`**：语料换面为「在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`」两条固定规则，与 `api` 状态、`doc` 字段无关。
- **`archi_api`**：三动作（`check` / `bootstrap` / `link`）；`bootstrap` 回双草稿、`link` 出正确 `$ref` 串；`archi_scaffold` 只改描述一句。
- **下发面**：`archi_api` 补 `(proposal, executor)` / `(dev-design, executor)` 两格（只读档，不扩写权）。
- **知识**：骨架重写为五段（三资产分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断 + 兼容），体量锚重定为 1800 / 2000 B；change 臂在写权放开（`model_write_lock == None`）时注入同一份知识。
- **写纪律**：生成物由四类改三类，`.schema.json` 是手写真相且其写前校验是本变更**唯一**显式例外（不属保护面、不属写权护栏）。

## MODIFIED Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经工具名字清单受控注入），语义为**定位**（已知在看哪个模块），并 SHALL 一次给全该模块的**三份资产**：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine` / `entry`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、`doc`（模型 `doc` 单值，仅 `api = none` 元素可能有值）、`api`（并列契约状态字段）、`resolvedEntry`、`fileHash`、**`assets`（三份资产的结构面）**、**`reading`（一句话读法指引）**。
- **`assets` 的三段 SHALL 各自独立 `present`，MUST NOT 全或全无**：

  ```jsonc
  "assets": {
    "topology":  { "file": "<模型声明文件>", "line": 105, "out": 0, "in": 3 },
    "interface": {                                       // 唯一凭据
      "path": "<models>/…/<FQN>.schema.json", "present": true, "bytes": 276,
      "ops": [], "types": [], "reexports": [],            // 键名即定义
      "abstract": "接口面留白（schema 在盘且无 op / 无类型）" },
    "doc": {
      "path": "<models>/…/<FQN>.md", "present": true, "bytes": 3680, "lines": 62,
      "headings": ["职能", "公共面", "不变量", "承载理由"] }
  }
  ```

  - `topology` 取模型事实（`modelFile` / `modelLine` + 出边数 / 入边数），零额外读盘；
  - `interface` **只给键名摘要，不给 content**：`ops` = 带 `x-function` 的 `properties` 键、`types` = `$defs` 本地定义键、`reexports` = 纯 `$ref` alias 键；`abstract` 是四形态的一句话判读（接口面留白 / 只有 op / 只有类型 / 有透传）；`present = false` 时三数组为空、`bytes = 0`；
  - `doc` 给 **`##` 段名 + 字节 + 行数**，MUST NOT 内联正文（14 KB 的文档灌进上下文会抢走 `read` 的翻页能力）；`present = false` 时 `headings` 为空。
- **`reading` SHALL 是工具输出的一部分**（静态文本，零变量插值）：内容须表达三条——① 理解本模块已足够（拓扑看 file/line 与出入边、接口看键名、文档看段名）；② 文档正文用 `read` **整读**（文档无结构，MUST NOT 用 `grep` / `offset` 拼凑）；③ 接口不必整读（`assets.interface` 的键名即定义）。理由是这段话写在 prompt 里会被 `AGENT.md` 的体量淹没。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / `api.status` / `hasPath`），并受既有单结果字节上限约束（超限截断留痕）。薄概览 MUST NOT 携带 `assets` / `reading` / `fileHash`（一元素多份读盘，不给全表付账）。
- **`api` 并列字段（换锚）**：SHALL 仍为四字段对象（`status` / `contractPath` / `schemaPath` / `generatedPath`），但——
  - `status` SHALL 为「接口定义在案」的事实值：`declared` ⟺ `<FQN>.schema.json` 在盘，否则 `none`；
  - 三条路径 SHALL **各自按自身在盘事实出线**（`contractPath` = 文档路径、`schemaPath` = 接口路径、`generatedPath` = 入口同级生成物路径，即便文件未生成也给路径串）；MUST NOT 再要求「文档与接口同时在盘」才报路径（旧的全或无投影会把三份资产的存在事实压成一个开关）；
  - 模型意图（`api = declared` / `none`）由 `element.api` 承载，MUST NOT 与 `api.status` 混说；字段名 MUST NOT 在本变更改名（`contractPath` 一直是文档路径，语义在 JSDoc 点明）。
- **`resolvedEntry` / `hasPath` / `fileHash` 三字段逐字沿用**：`resolvedEntry` 由 `api::paths::entry_state` 单点求值；`hasPath` 是纯投影；`fileHash` 与 `drift::file_hash` 同一函数产出、可直接作写工具的 `expect`，MUST NOT 与报告的 `modelHash` 比较。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置；元素不存在 / 模型结构错 SHALL 返回可诊断 `Err`（绝不 panic、绝不返回空对象）；响应 SHALL 经 `archi::dto` 单点投影，MUST NOT 在工具执行体内另拼 JSON。
- **`doc` 并列字段**：`doc` SHALL 为 repo 相对 POSIX 路径单值或 `null`；约定只有 `api = none` 元素会填（`api = declared` 元素的文档走 `api.contractPath`）——该约定**无机械判据**：`declared` 元素写了 `doc` 也**不报**（`apiUnexpected` 形态②已删除）。该字段**零判据**（写了不报、指不到不报），仅作 UI 的呈现事实（检索语料是两条资产路径，见「检索面纪律与迁移边界」）。
- **描述面（`mount.rs`）** SHALL 含五句：① 「**理解模块先调本工具**：三份资产的路径与结构面都在 `assets`；别的工具只给一面」；② 「接口的唯一凭据是 `.schema.json`」；③ 「文档正文用 `read` 整读（无结构），接口不必整读（键名即定义）」；④ 「`resolvedEntry` 给入口解析结果；`fileHash` 可直接作写工具的 `expect`」；⑤ 「`contractPath` 是文档路径、`schemaPath` 是接口路径」。MUST NOT 复制字段语法长表，MUST NOT 残留 `docs` 词汇。

#### Scenario: 定位单元素一次给全三资产

- **WHEN** 模型会话中调用 `archi_module(element="Backend.Infra.ArchiRuntime")`
- **THEN** 返回元素本体、出入边、子元素、`doc`、`api`、`resolvedEntry`、`fileHash` 与 `assets`（`topology` 的 `file` / `line` / 出入度数、`interface` 的键名摘要与 `abstract`、`doc` 的段名 / 字节 / 行数）与 `reading`；响应内 MUST NOT 出现文档正文与 schema 全文

#### Scenario: 三资产各自 present 独立

- **WHEN** 分别对「文档与接口都在盘」「只有接口在盘」「只有文档在盘」「两份都没有」四个元素调用
- **THEN** `assets.interface.present` 与 `assets.doc.present` 逐例为 `(true,true)` / `(true,false)` / `(false,true)` / `(false,false)`；不存在的资产其 `path` 仍给出派生串（或 `null` 仅当元素无 `path`）、`bytes = 0`、数组为空；MUST NOT 出现「缺一份则三段全空」

#### Scenario: 接口摘要四形态可辨

- **WHEN** 分别对空 schema、只有 op、只有类型、含透传 alias 的四个元素取 `assets.interface`
- **THEN** `abstract` 依次为「接口面留白（schema 在盘且无 op / 无类型）」/「只有操作」/「只有类型」/「含透传」；三数组逐键与 schema 逐项相等，且不含 `$defs` 内部的嵌套键

#### Scenario: 文档段名与整读指引

- **WHEN** 对一份 62 行、6 个 `##` 段的文档取 `assets.doc`
- **THEN** `headings` 恰为 6 个段名（不含 `#` 前缀）、`lines` / `bytes` 与磁盘一致；`reading` 含「整读」与「MUST NOT 用 `grep`」两条

#### Scenario: api 三路径各自独立

- **WHEN** 对「接口在盘、文档缺失」与「接口缺失、文档在盘」两元素调用
- **THEN** 前者 `api.status = "declared"`、`schemaPath` 有串、`contractPath` 为 `null`；后者 `api.status = "none"`、`contractPath` 有串、`schemaPath` 为 `null`；两例的 `generatedPath` 都给路径串；`element.api`（模型意图）与 `api.status`（文件事实）分列可辨

#### Scenario: 薄概览不带三资产

- **WHEN** 省略 `element` 调用
- **THEN** 每条含 `fqn` / `kind` / `gate` / `summary` / `api.status` / `hasPath`，不含 `assets` / `reading` / `fileHash` / 三路径 / `doc` / `resolvedEntry`；超单结果字节上限时截断留痕

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断

### Requirement: archi_search 只读工具的轻量检索契约

SDK 引擎 SHALL 提供专用只读工具 `archi_search`，语义为**发现**（还不知道这功能归谁管）：

```jsonc
{ "name": "archi_search",
  "parameters": {
    "query": "必填。关键词 / 元素名片段 / 资产路径片段",
    "scope": "可选。'elements' | 'doc' | 'all'（缺省 all）" } }
```

- 匹配对象 SHALL 恰为：元素 FQN、`summary`、`metadata` 值、**资产路径与文件名**（`path` 非空元素的在盘资产：派生 `<FQN>.md`（文档）与 `<FQN>.schema.json`（接口）两条固定规则，与 `api` 状态无关）。MUST NOT 匹配文档正文、MUST NOT 匹配 `openspec/specs/` 下任何路径或内容、MUST NOT 匹配 `decisions/*.md`。
- **语料面两条固定规则**（替换旧的「载体派生」口径）：文档语料 = 「`path` 非空且在盘的 `<FQN>.md`」∪「`path` 非空且在盘的 `<FQN>.schema.json`」。两条都不经 `api` 分支、都不经 `doc` 字段——`carrier_path` 已退役，检索面 MUST NOT 复活第二次载体派生。
- 匹配方式 SHALL 为 ASCII 大小写不敏感子串 + **驼峰分词**（`AuthToken` → `auth` / `token`）。
- `scope = "elements"` 时文档结果恒为空数组；`scope = "doc"` 时 `elements` 恒为空数组。**`scope` 的值名 SHALL 恰为 `elements` / `doc` / `all`**（`docs` 是非法值，`parse` 报错并列出三个合法值）。
- 文档结果条目 SHALL 是一条 repo 相对路径字符串（`ArchiSearchResult.doc: Vec<String>`），排序按路径字典序；MUST NOT 引入相关度打分。
- 实现 SHALL 为每次现算（按两条固定规则派生 + 存在性判定），MUST NOT 建倒排索引或跨调用缓存。
- `query` 为空白串 SHALL 显式 `Err`；结果 SHALL 受既有单结果字节上限约束。
- **描述面（`mount.rs`）**：`scope` enum 与值名不变；描述内「入口文档路径」改为「资产路径（文档 / 接口）」。

#### Scenario: 关键词同时命中两资产

- **WHEN** 调用 `archi_search(query = "archi", scope = "all")`
- **THEN** 命中 FQN / summary 含该词的元素，与路径含该词的资产（如 `<models>/Infra/Backend.Infra.ArchiRuntime.md` **与** 同目录 `.schema.json` 两条都命中）；结果只含元素薄条目与路径串

#### Scenario: 语料与 api 状态无关

- **WHEN** 某 `api = none` 元素只有文档、某 `api = declared` 元素只有 schema
- **THEN** 前者文档命中、后者接口命中；两者都不因 `api` 状态被排除或纳入；`doc` 字段的值不参与语料

#### Scenario: scope 换词与空串拒绝

- **WHEN** 分别以 `scope = "elements"` / `"doc"` / `"docs"` / `query = ""` 调用
- **THEN** 前两者分别使另一半结果为空数组；`"docs"` 显式 `Err` 并列出三个合法值；空 query 显式 `Err`

### Requirement: archi_api 只读工具的自检与草稿契约

SDK 引擎 SHALL 提供专用**只读**工具 `archi_api`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**接口面自检、起草与引用计算**：

```json
{ "name": "archi_api",
  "arguments": { "action": "check" | "bootstrap" | "link", "element": "<FQN>",
                 "from": "<FQN>", "to": "<FQN>", "name": "<导出名>" } }
```

- **三动作封闭**：`action` SHALL 只接受 `check` / `bootstrap` / `link` 三值；词汇外值以 `is_error` 回灌并点名三个合法值，MUST NOT 静默退化。
- **`check` = 两轴结构化结果**：返回「生成物新鲜度」与「契约 vs 代码」两轴的并列内容（① 三类差异 `mismatched` / `orphaned` / `missing`，取自与 IPC / CLI 同一份比对实现；② 契约判据摘要 + 逐条 `{ kind, category, element, target, message }`）。响应 SHALL 复用既有出线形状，MUST NOT 在执行体内另拼投影。**历史 `api-*` fence MUST NOT 产生任何计数或提示**（Q7：它已退化为普通文档内容）。
- **`bootstrap` = 双草稿（起草第一步）**：`element` 必填，返回 `{ schema, doc }` 两段草稿文本——与 CLI `bootstrap` 同一份内容（形态归 `desktop-archi-api`「bootstrap 双草稿（schema 骨架 + doc 骨架）」）。`element` 缺失 / 元素不存在 / 无实现入口 ⇒ `is_error` 并透传 `ApiError` 文案。
- **`link` = `$ref` 串计算（纯计算，零读盘写盘）**：
  - 跨元素形态：`{ from, to, name }` → `"<ref_relative(from, to)>#/$defs/<name>"`（例：`"../../Desktop/Components/Agent/AgentInput.schema.json#/$defs/AgentInputProps"`）；
  - 本模块形态：`{ element, name }` → `"#/$defs/<name>"`；
  - `from` / `to` 缺失或非 FQN 形态、`name` 非 ASCII 标识符 ⇒ `is_error` 点名成因；两形态互斥（同给 `to` 与省略 `to` 以外的一切组合都拒）。
  - 描述面 SHALL 明写「跨元素 `$ref` 用 `link` 算，勿手拼」——手算 `../` 层数少一层会绕过 `apiRefMissing`，到编译期才炸。
- **零写盘**：三动作 MUST NOT 创建 / 覆盖 / 删除任何文件——落盘仍由会话经 `write` / `edit`（写闸校验）执行。MUST NOT 存在「写生成物 / 写契约」的专用工具。
- **结构错以 `is_error` 回灌**：模型含任一结构诊断时三动作 SHALL 返回 `is_error` ToolResult（诊断带 `file:line`），MUST NOT 返回部分结果、MUST NOT panic。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点；超限时摘要（按 `category` 计数）优先保留。
- **上下文路由沿用**：执行体 SHALL 经 `LoopTurn` 的 per-turn `ArchiContext` 路由；MUST NOT 新增 `ToolExec` 变体、MUST NOT 把 store 句柄塞进七通用工具的 `execute` 签名。
- **无路径参数**：本工具不带 `path` ⇒ 路径沙箱恒原样返回，写入保护零射程。
- **提示面**：被下发本工具的相位 prompt SHALL 有一行说明（沿既有写法），内容限于「改完代码可自查生成物新鲜度与接口判据；生成物过期用既有命令重生成，MUST NOT 手改生成物」。
- **描述面（`mount.rs`）** SHALL 含四句：① `bootstrap` 是新建 / 补接口的第一步（给两份草稿）；② `link` 算 `$ref` 串（勿手拼）；③ **三类**生成物的点名（架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）连同「MUST NOT 手改生成物」；④ 冷启动口径「`artifacts.enabled = false` 即接口面未启用，此时 `findings` 恒空且**不等于**零问题」。MUST NOT 内建本仓专用命令名，MUST NOT 把 `.schema.json` 说成生成物。

#### Scenario: check 返回两轴且历史 fence 零计数

- **WHEN** 在 implement/executor 会话中调用 `archi_api(action = "check")`，某元素的文档仍含两个历史 `api-op` fence
- **THEN** 响应含三类差异与按 `category` 计数的判据摘要；响应内零「fence 残留」类计数键；调用前后磁盘字节完全相同

#### Scenario: bootstrap 双草稿只出文本

- **WHEN** 调用 `archi_api(action = "bootstrap", element = "App.Auth")`
- **THEN** 返回 `{ schema, doc }`：`schema` 含 `$id` / `title` / `properties` 占位与 `$defs`，`doc` 只有 H1 + 一句指引；磁盘零变化

#### Scenario: link 两形态各出正确串

- **WHEN** 分别以 `{ from: "Backend.Infra.ArchiRuntime", to: "Backend.Core.Foundation", name: "Summary" }` 与 `{ element: "Backend.Core.Foundation", name: "Summary" }` 调用
- **THEN** 前者出 `"../../Core/Foundation/Backend.Core.Foundation.schema.json#/$defs/Summary"`（前缀与 `paths::ref_relative` 逐字一致）、后者出 `"#/$defs/Summary"`；两次都零写盘

#### Scenario: link 参数面互斥与校验

- **WHEN** 以 `{ from, name }`（缺 `to` 且缺 `element`）、`{ from, to, name: "a.b" }`、`action = "generate"` 分别调用
- **THEN** 三者均 `is_error`：前者点名缺 `to` / `element`，中者点名 `name` 须为标识符，后者点名合法三值；零写盘

#### Scenario: 结构错回灌而不崩

- **WHEN** 模型被写成语法错后调用 `archi_api(action = "check")`
- **THEN** 返回 `is_error` ToolResult（诊断带 `file:line`），run 不中断、进程不 panic

### Requirement: archi 工具面注入与 (phase, role) 矩阵

archi 专用工具的**下发面** SHALL 由 `agent-runtime` 组合根以**工具名字清单**表达（`SessionInjections.tools: Option<Vec<String>>`），MUST NOT 依赖枚举式 toolset。清单内每个名字 SHALL 经 `mount::lookup` 成员校验（未知名显式 `Err` 携名与来源）。

会话装配（`compose.rs`）SHALL 按 `(source, phase, role)` 决定下发面：

| phase | role | `archi_module` | `archi_search` | `archi_api` | `archi_drift` | `archi_scaffold` | `archi_element` | `archi_relation` |
|---|---|---|---|---|---|---|---|---|
| `proposal` | executor | ✅ | ✅ | **✅** | ❌ | ✅ | ✅ | ✅ |
| `dev-design` | executor | ✅ | ✅ | **✅** | ✅ | ✅ | ✅ | ✅ |
| `dev-design` | evaluator | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `code-review` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 其余相位 | 任意 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

- **`archi_api` 的两格扩面（本变更）**：`(proposal, executor)` / `(dev-design, executor)` 由 ❌ 改 ✅。理由：接口定义成了手写真相，`bootstrap` 双草稿与 `link` 的价值恰在起草相位；不补则 `archi_scaffold` 描述里「接口草稿用 `archi_api` 的 `bootstrap`」指向一个未下发的工具。`archi_api` 是**只读**档，扩面不扩写权（两写工具的矩阵集与 `model_write_lock` 的同集纪律逐字不变）。
- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份。`dev-design` 的 executor / evaluator **分两行**，MUST NOT 合并。
- **两写工具只给写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂：矩阵下发写工具的 `(phase, role)` 集合 SHALL 与 `compose::model_write_lock` 判定为 `None` 的集合**逐格相等**（两条方向都设断言）。
- **`archi_drift` / `archi_scaffold` 的下发面逐字沿用**（`drift` 只给两处直接产出漂移的相位；`scaffold` 只给两处写权相位）。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ **七个** archi 工具名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`）+ `preamble = Some(<架构会话知识>)` + `before_done = Some(ArchiPreclose)`；该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析。
- **change 臂的知识段（本变更）**：`source = "change"` 会话在 `model_write_lock(provenance) == None`（即两个写权相位）时 SHALL 注入**同一份**架构会话知识（`archi_knowledge::knowledge()`，单来源）；其余相位（含 `implement/executor` —— 它有 archi 只读工具但不能写模型）SHALL 保持 `preamble = None`。判据 MUST NOT 另立一份矩阵命中判定（写权与知识同源）。知识拼装失败 SHALL 降级为 `None`，MUST NOT 影响工具面与 run。
- `phase` / `role` SHALL 取自 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段；解析不出即 **fail-closed**（零 archi 工具）。
- `source = "explore"` 臂的 archi 面 SHALL 恒为空；`source = "config"` 臂零 archi 工具、其 `preamble` 为配置协议知识（本能力 MUST NOT 另写一份）；其余非 change、非 config 来源 SHALL 走显式构造。
- 矩阵只决定**下发面**，不决定 prompt 文本；`preamble` 位只由装配层按来源决定，前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中（两写权相位含 api）

- **WHEN** 分别以 8 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** `proposal/executor` 为 `module` / `search` / `api` / `scaffold` / `element` / `relation`；`dev-design/executor` 为 `module` / `search` / `api` / `drift` / `scaffold` / `element` / `relation`；`dev-design/evaluator` 为 `module` / `search`；`implement/executor` 为 `module` / `api` / `drift`；`code-review` / `acceptance` 只 `module`；`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 写工具与写权锁同集

- **WHEN** 遍历全部 `(phase, role)`，分别取矩阵结果与 `model_write_lock` 的返回值
- **THEN** 「矩阵含 `archi_element`」⟺「`model_write_lock` 返回 `None`」，两个方向逐格相等（archi 来源臂同判）；`archi_api` 的扩面 MUST NOT 改变该对拍结果

#### Scenario: 知识段与写权同源

- **WHEN** 分别以 `(proposal, executor)` / `(dev-design, executor)` / `(implement, executor)` / `(code-review, evaluator)` 发起 change 会话并审查 `preamble` 位
- **THEN** 前两者 `preamble` 为架构会话知识（与 archi 臂逐字相同）；后两者 `preamble` 为 `None`；四者的工具面仍按矩阵下发

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；调用按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具、`preamble = None`（写权判据亦不成立），MUST NOT 抛装配错误

#### Scenario: 知识降级不影响工具面

- **WHEN** 知识拼装失败（派生异常）后以 `(dev-design, executor)` 发起会话
- **THEN** `preamble = None`，会话照常发起、矩阵命中的工具面与 `before_done` 位不变

### Requirement: 架构会话的系统知识注入

`source = "archi"` 会话与 **change 臂的两写权相位** SHALL 注入同一段架构会话知识到 `SessionInjections.preamble`，内容 SHALL 覆盖以下**五段**（本变更是骨架的第三次改写：三资产分工与设计顺序取代语法类知识，散文只留不可脚本化的判断面）：

- **三资产分工（C）**：模型 TOML 答「谁在谁的里面、谁依赖谁」（拓扑）；`<FQN>.schema.json`（与文档同目录）答「跨过这条边时接触面长什么样」（接口，**唯一凭据**）；`<FQN>.md`（同目录）答「为什么这样切、有什么坑、什么不归它管」（理由）。**过度设计 = 回答自己不该回答的那个问题**：文档不列接口、schema 不写理由、TOML 不造空元素。**文档是产品面**——架构页「文档」段直接渲染给用户读，人 + AI 共读；接口由写闸校验、人经 UI 阅读。
- **设计顺序（B/C）**：① 定位现状（`archi_search` 发现 → `archi_module` 一次取全邻居 / 三份资产 / 声明位置）；② 画拓扑（`archi_element` / `archi_relation`）——**先边界、后接触面、最后写理由**；③ 补两份资产（`archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 落 `.schema.json`（坏文本零落盘）→ 理由落 `.md`，段落按内容自定）。
- **三条不可越界（C）**：① **文档不列接口**——导出名 / 签名 / 类型的唯一持有者是 schema；历史 `api-*` fence 是遗留内容（读面不读、清理中），MUST NOT 新增；② **schema 不写理由**——`x-function.description` 只写调用方要知道的语义（做什么 / 边界 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common` 三项，MUST NOT 自造新前缀键；③ **TOML 不造空元素**——每个元素至少一条接触面（`path` / 边 / 导出）。
- **工具面（A/B，指路）**：字段面、键形、缺省、值域、祖先与计数全部由工具承担——MUST NOT 凭记忆手拼 TOML。读五件各一句「何时用」（`archi_search` 发现 / `archi_module` 定位与三资产 / `archi_drift` 漂移读数 / `archi_api` 接口三动作 / `archi_scaffold` 字段词典）；写两件（`archi_element` / `archi_relation`，参数面即字段清单、坏文本零落盘）；**手改出口（永久内容）**：工具表达不了的（重排文件 / 批量整理）才手改 TOML，改完必须 `archi_module` 自查——坏模型由读取面报错（结构诊断带 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改。自检纪律：改完 `archi_drift` 非空即逐条修、`archi_api`（`check`）查两轴。契约资产 `config.json` 管 `ignore` / `exclude`，非法由 `archi_drift` 点名。
- **留给判断 + 兼容（C）**：`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；**纯分组容器用 `composite`**；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），不要编造导出**；旧元素可能仍把接口写在 `.md` 的 `api-*` fence 里——读面不读，接口一律写 `.schema.json`，MUST NOT 往 `.md` 新增 fence。

两条工程约束与下列「值域不进知识文本」纪律逐字沿用：

- `gate` / `api` / `kind` 的值域与缺省档 SHALL 由工具面承载（参数 `description` 与 `archi_scaffold` 词典），骨架 MUST NOT 再写值域行；键形与字段形态 MUST NOT 进骨架；结构诊断全量词表 MUST NOT 进知识文本（`orphanHierarchy` / `dependsOnAncestor` 亦不在骨架，祖先缺失由写工具 `Err`、越界依赖由 `archi_drift` 承载）。
- `entry` / `doc` / `apis` MUST NOT 进任何值域词表（开放集）；判据长表 MUST NOT 复制进知识文本。
- **体量锚（本变更重定）**：骨架 SHALL ≤ **1800 B**、全文 SHALL ≤ **2000 B**，实测值 SHALL 被冻结断言（测量口径沿用 `lf_bytes(trim_end)`）。门限由 1500 / 1700 上抬到本组实测值——新增的是不可脚本化的语义内容（三资产分工 / 设计顺序 / 三条不可越界 / 兼容句，约 350 B），**MUST NOT** 借此恢复语法教学（值域行 / 字段形态 / 诊断词表三类仍在禁令内）；超限 SHALL 压缩同段冗余表述，MUST NOT 删除三资产分工 / 设计顺序 / 手改出口 / 留给判断四段任一条。
- **尽力增强、不炸 run**：知识拼装失败 SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 五段结构在骨架内可指认

- **WHEN** 审查骨架 md 的分段
- **THEN** 恰含五段语义：三资产分工（含「文档是产品面」）、设计顺序（三步 + 「先边界、后接触面、最后理由」）、三条不可越界（文档不列接口 / schema 不写理由 / TOML 不造空元素）、工具面（读五件的「何时用」+ 写两件的「参数面即字段清单」+ 手改出口一句 + 自检纪律）、留给判断 + 兼容；MUST NOT 出现语法教学段

#### Scenario: 三条不可越界逐条在场

- **WHEN** 对骨架做关键词断言
- **THEN** 「文档不列接口」/「MUST NOT 复述签名」/「TOML 不造空元素」三条各至少一处；`x-` 面封闭三项（`x-function` / `x-class` / `x-common`）在场；「空 schema」与「不要编造导出」在场；「历史 fence 读面不读、MUST NOT 新增」在场

#### Scenario: 语法知识零残留

- **WHEN** 对骨架做机械检查
- **THEN** 零示例 TOML 围栏（` ``` ` 计数为 0）、零 `[element.` 前缀、零字段值域行（`off` / `warn` / `strict` / `declared` / `none` 不同时作为值域枚举出现）、零结构诊断词（六词全零命中）、零 `entry` 三档长表

#### Scenario: 手改出口是永久内容

- **WHEN** 审查骨架「工具面」段的最后一句
- **THEN** 该句含三要素：①「工具表达不了的才手改」的适用边界；②「改完必须自查」（点名 `archi_module`）；③ 坏模型由读取面报错（带 `file:line`）；MUST NOT 被表述为过渡措施或待删项

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 1800 B、全文 ≤ 2000 B 且实测值被冻结断言；程序追加段含模型声明目录与契约资产目录（文档 + 接口两条同目录资产）两处方位，零契约索引串，骨架零目录字面量

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生不可用）
- **THEN** 结果为 `preamble = None`，session 照常发起、run 正常推进（工具面与 `before_done` 不变），MUST NOT 抛装配错误

### Requirement: 模型 = 源码（两条写入口与写纪律）

架构模型文件 SHALL 与源码同权：agent SHALL 直改 `<archi_dir>/graph/model/*.toml`。**写模型有两个入口，MUST NOT 存在第三个**：

| 入口 | 形态 | 失败语义 | 定位 |
|---|---|---|---|
| 专用写工具 `archi_element` / `archi_relation` | 结构化参数面 | **坏文本不可产出**：渲染 → 重解析自证 → 回读一致，任一失败即 `Err` 且零落盘 | **canonical** |
| `write` / `edit` 手改 | 自由文本 | **写坏模型仍是合法状态**：由读取面报错（结构诊断 `file:line`、画布停摆、`archi_preclose` 兜底） | **second path**：工具表达不了的（重排文件 / 批量整理）才用，改完必须自查 |

写权（谁能写）与写入口（怎么写）是两件事，MUST NOT 混说。以下逐条沿用（仅第 3 条改名）：

- **写权两条通路**：A（change 流程相位，**仅 `phase ∈ {proposal, dev-design}` ∧ `role = executor`**，执行根 `detail.worktree`）/ B（架构页通用会话 `source = "archi"`，不限相位，执行根主仓）。写权政策实做是掐掉其余相位对模型目录的射程（会话级写入禁区，见 `desktop-write-protection`）。
- **两条失败语义 MUST NOT 混为一句**：写工具面 MUST NOT 产出坏文本；手改面 MUST NOT 因加工具而收紧——写坏模型仍由读取面报错。MUST NOT 把「写盘前校验」当写权护栏。
- **两类生成物面（本变更由四类改三类）**：`<FQN>.schema.json` **不再是生成物**（它是手写真相，走 `write` / `edit`），生成物只剩 `INDEX.toml` / `.i.rs` / `.i.ts`。生成物同样没有专用写工具、MUST NOT 手改（用既有命令重生成），事后兜底为零 diff 门 + 契约面板。
- **手写接口面有一处显式写前校验（本变更新增的唯一例外）**：`write` / `edit` 落 `<archi_models_dir>/**/<FQN>.schema.json` 时 SHALL 先过接口校验（坏文本 `Err` 零落盘，见 `desktop-agent-execution`「`write` / `edit` 的接口定义写闸」）。该例外 MUST NOT 被读成写权护栏或保护面：它只判「文件级自洽」，与谁能写无关；手改通路对**其余一切文件**（含 `INDEX.toml` / `.i.*` / 模型 TOML / 普通源码）仍零预校验。
- **不依赖运行时审批**、**内置锁面仍不纳入**、**两通路并存互不替代**、**场景归属**、**新元素缺省档**、**通路 B 的边界（软约束）** 逐条沿用。

#### Scenario: worktree 内直改模型成功（白名单相位）

- **WHEN** agent 在 `dev-design/executor` 会话中用 `write` 新建模型文件，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（会话级锁未在位、内置锁面零命中）；随后读取面解析出该改动

#### Scenario: 白名单外相位写模型被拒

- **WHEN** `implement/executor` / `code-review/evaluator` / `dev-design/evaluator` 会话对 `<archi_dir>/graph/model/*.toml` 调用 `write` 或 `edit`
- **THEN** 拒绝串含路径、工具名、指路与「不要改用其他命令或工具尝试绕过」；run 不中断、文件零变化

#### Scenario: 手写接口面是唯一预校验面

- **WHEN** 分别以 `write` 落一份坏 `.schema.json`、一份坏 `INDEX.toml`、一份坏模型 `*.toml`
- **THEN** 第一者 `Err` 零落盘（写闸）；后两者原样落盘、由读取面 / 零 diff 门报错（写前零校验不变）

#### Scenario: 生成物只三类

- **WHEN** 审查写纪律文本与生成物清单
- **THEN** 只点名 `INDEX.toml` / `.i.rs` / `.i.ts` 三类；`.schema.json` 被表述为**手写真相**（走 `write` / `edit` + 写闸），MUST NOT 与三类并列

### Requirement: 检索面纪律与迁移边界

`archi_search` 与 `archi_module` 两个只读工具面 SHALL 守住三条迁移边界，使将来的「模块入口文档逐步替代 `openspec/specs/`」迁移不被本期实现反向锁死：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 检索语料不读 `openspec/specs/` | 一旦纳入，用户会开始写「给 archi 看的 spec」，迁移被反向锁死 |
| 2 | 检索与工具响应不返回文档正文 | 正文按路径自取（agent 用 `grep` / `read`，前端用 `read_file`）；返回正文会让工具响应体随文档膨胀且与单结果字节上限冲突 |
| 3 | 不提供跨元素的文档反向索引 | 反查「这份文档描述哪些模块」不在本期，避免用户开始写「给 archi 看的文档」 |

- **语料面封闭（本变更换面）**：检索语料 SHALL 恰为「`path` 非空元素的在盘资产」两条固定规则——在盘 `<FQN>.md`（文档）与在盘 `<FQN>.schema.json`（接口）；MUST NOT 新增第三条来源（不出 `specs`、不出 `decisions`、不扫元素 `path` 目录内的散落 md、不扫 `<archi_dir>` 其它子树、MUST NOT 读模型 `doc` 字段——`carrier_path` 已退役，载体派生不再存在）。两条规则都与 `api` 状态无关。
- **正文不返回（结构性成立）**：`ArchiSearchResult.doc` 只承载**路径字符串**（文档与接口各一条规则），模型 `doc` 字段只承载一条路径串——字段里没有 `title` / 摘要 / 正文 / 行区间的容身之处，故「不返回正文」不靠纪律而由形状保证。
- **反向索引零预铺**：MUST NOT 引入「文档 → 元素集合」的索引、缓存或 IPC；`grep` 与文件树是反向查找的既有面。
- **零 `specs` 读取路径**：产品源码 MUST NOT 出现 `openspec` 路径拼接（路径一律经 `foundation::layout`），本能力 MUST NOT 引入 `spec ↔ element` 映射。
- **「模块入口文档替代 specs」的迁移**（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）需独立变更裁决，本期零预铺。

#### Scenario: 检索不命中 specs 内容

- **WHEN** `query = "archi"` 且 `openspec/specs/` 内某文件路径含该词、某入口文档亦含
- **THEN** 命中项只来自模型元素与资产路径（文档 / 接口两类）；`openspec/specs/` 下任何路径与内容均不出现

#### Scenario: 正文零承载位

- **WHEN** 审查 `ArchiModuleDetail.doc` 与 `ArchiSearchResult.doc` 的字段面
- **THEN** 前者是单一路径串或 `null`、后者是路径串数组；两者都没有 `title` / 摘要 / 正文 / 行区间字段，也无「内容」一类可选位

#### Scenario: 关联来源封闭

- **WHEN** 审查文档检索语料来源清单
- **THEN** 语料只取两条规则（在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`），与 `api` 状态、`doc` 字段均无关；`decisions/**` 与元素 `path` 目录内的散落 md 都不在语料内，MUST NOT 出现来源分支


## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/dto.rs` | 投影单点（本变更加两字段 + 四类型） | `ArchiModuleDetail` 增 `assets: ArchiAssetSet` + `reading: String`；新增 `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`（4 个出线类型）；`module_detail(root, fqn)` 装配三资产（`topology` 取模型、`interface` 读 schema 键名与 `abstract`、`doc` 读 `##` 段名与行数）；`ArchiApiRef` 四字段名零变化、`ref_projection` 三路径各自独立出线、`status` 改判 schema 在盘 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体 | `archi_module` 透传 `assets` / `reading`（零拼装）；`archi_api` 增 `link` 一支（`paths::ref_relative` + `#/$defs/<name>`，纯计算零读盘写盘）；`bootstrap` 回 `{ schema, doc }`；`SCAFFOLD_TEMPLATE` 的描述句改为「接口草稿用 `archi_api` 的 `bootstrap`」（词典正文不动） |
| `crates/infra/agent/src/sdk/mount.rs` | 工具定义面 | `archi_module_definition()` 五句（三资产一次给全 / 接口唯一凭据 / 文档整读 / `resolvedEntry` + `fileHash` / 两路径语义）；`archi_api_definition()` 四句（双草稿 / `link` / 三类生成物 / 冷启动口径）；`archi_scaffold_definition()` 一句（接口草稿指路）；工具名常量、参数 schema、`ToolTier::Read` / `ToolExec::Archi` 归属与 `build_registry` 项数零变化 |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `archi_matrix` 两格追加 `archi_api`；change 臂在 `model_write_lock(provenance) == None` 时注入 `archi_knowledge::knowledge()`（失败降级 `None`）；archi 臂与 config 臂逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` | 知识静态骨架 | 五段（三资产分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断 + 兼容）；零围栏、零值域行、零诊断词、零 `<>` 占位符；体量锚 1800 / 2000 B |
| `crates/infra/agent/src/archi_knowledge.rs` | 拼装 | 派生段改为「模型目录 + 契约资产（`<FQN>.md` 理由 + `<FQN>.schema.json` 接口，同目录）」；`build` 的占位符 / 空骨架校验与降级语义逐字不变 |
| `crates/infra/agent/src/sdk/policy.rs` | 静态权限表 | `READONLY_TOOLS` 三工具名与三档全放行逐字不变（`archi_api` 的矩阵扩面不改档位） |
| `crates/core/archi/src/api/paths.rs` | 路径单点（供 `link`） | `ref_relative` 签名与语义零变化；`carrier_path` / `doc_path` 退役后本能力零新依赖（`link` 只用 `ref_relative`） |

### CLI: `crates/infra/agent/src/sdk/archi_tool.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api` 的 `action` 值集 | 改 | `check` / `bootstrap` / `link` 三值（`mount.rs` 的 enum 与执行体解析同批） |
| `bootstrap` 响应 | 改 | `{ schema, doc }`；`cap_result` 单点截断不变 |
| `link` 响应 | 新增 | `{ ref: "<串>" }`；参数面 `from` / `to` / `name` / `element` 的互斥与校验见正文 |

### API: 工具面（响应形状）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_module` 单元素响应 | 加字段 | `assets`（三段各自 `present`）+ `reading`；`api` / `doc` / `resolvedEntry` / `fileHash` / `hasPath` 位置与形状不变 |
| `archi_module` 薄概览 | 零变化 | 仍为 `fqn` / `kind` / `gate` / `summary` / `api.status` / `hasPath` |
| `archi_search` 语料 | 换面 | 两条固定规则（在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`），与 `api` 状态无关 |
