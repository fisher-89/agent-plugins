# desktop-archi-api Specification（delta: archi-contract-authoring-guidance）

## Purpose

本 delta 只补一处**草稿形态**承诺：`api::bootstrap` 的契约 md 草稿由「四段骨架」扩为「六段骨架 + 尾部惰性作者注」，并把「草稿自证可解析」落成机械锚。

其余全部零变化：契约资产形态与唯一手写真相、三类 fence 与装载期闸群（三条双向 gate + 闸一 + 落点闸 + 自洽闸 + 第四条闸）、`schema.json` / `INDEX.toml` 派生与零 diff 门、取数面与 `visibility` 判据、生成物落点与入口查找逐字不动。作者注之所以能存在，正因为解析期状态机对非 `api-*` fence 显式跳过（`contract.rs` 的 `// 其余 fence（示例代码等）不参与契约，内容不入散文区`）——本 delta 只把这条既有语义用起来，MUST NOT 为此新增判据、改解析器或加豁免清单。

## ADDED Requirements

### Requirement: bootstrap 草稿的六段骨架与惰性作者注

`api::bootstrap` 输出的契约 md 草稿 SHALL 是**可直接落盘并零错误装载**的骨架，并在骨架内自带起草引导：

- **六段骨架**：草稿 SHALL 依次含 `## 职能` / `## 不变量` / `## 操作` / `## 类型` / `## 边界` / `## 承载理由` 六个二级段；H1 SHALL 仍为元素 FQN（与文件名主干逐字相等，`file_title` 判据不变）。既有四段的相对次序与前缀 MUST NOT 变化，`## 边界` / `## 承载理由` 追加在 `## 类型` 之后——这两段对应契约资产形态里散文区已有的两个主题（职能 / 边界 / 不变量 / 承载理由），本 delta 只让草稿把它们占位出来，MUST NOT 引入散文的第七个主题。
- **占位句形态**：六段的占位正文 SHALL 是含 CJK 的「（待补：…）」句；MUST NOT 写成行首 `- <ASCII 标识符>` 的列表项（`collect_reexport_prose` 会把它抽成透传导出名，与 `api-reexport` 键集比对，不等即 `ApiError::Contract`），MUST NOT 写成首列整体反引号包裹的表格行（`collect_surface_table` 会把它抽成「散文表格复述」，与契约名字集相交即 `ApiError::Contract`）。
- **作者注 = 非 `api-*` fenced 块**：草稿 SHALL 在文件末尾带一个语言标非 `api-*` 的 fenced 块（实现取 `text`）承载起草引导，内容 SHALL 含四要点——① 三类 fence 的落点与唯一性（`api-op` 紧随 `### <op-id>`、`api-types` 为本模块自有 `$defs` 且同文件唯一、`api-reexport` 键 = 导出名 / 值 = 直接子元素 FQN 且同文件唯一）；② 散文只留语义、MUST NOT 用表格复述名字清单；③ `api-reexport` 形态与「散文透传列表须与键集逐名相等」；④ 起草后自查两轴（生成物新鲜度 / 契约判据），并写明契约面未启用时 `enabled = false` 且 `findings` 恒空。
- **作者注的落点封闭**：作者注 MUST NOT 放进裸正文、`## ` 段正文或 `### ` heading——散文区受上述两个抽取面扫描，`### ` 是 op 锚点且其下必须紧随一个 `api-op` fence；非 `api-*` fence 是解析期**唯一**被结构性排除的载体，故说明正文可含 `### `、表格与纯 ASCII 行而不影响装载。MUST NOT 为此在 `contract.rs` 增加「说明块」识别或段名白名单。
- **自证可解析（机械锚）**：有符号元素与无符号元素各取一份草稿，SHALL 经 `api::contract::parse` 零 `ApiError`；解析结果的 `sections` SHALL 恰为六段、`reexport_prose` SHALL 为空集、且六段 `body` 内 MUST NOT 出现作者注正文（说明块惰性的判据）。
- **MUST NOT 自动生成 `api-reexport` fence**：透传键值（导出名 ↔ 直接子元素 FQN）是设计判断；也 MUST NOT 以空 fence `{}` 占位冒充已声明。
- **零参数与零出口变化**：本 delta MUST NOT 增删 `bootstrap` 的入参、action 值集与错误面；CLI 与工具两条出口仍出**同一份**草稿文本，仍只出 stdout / 只回 `{ draft }`，零写盘。

#### Scenario: 六段骨架与尾部惰性说明

- **WHEN** 对任一有实现入口的元素调用 `bootstrap`
- **THEN** 草稿以 `# <FQN>` 起手，依次含六段（职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由），文件末尾是一个 `text` fenced 块；该块内含「api-reexport」与 `{ "导出名": "直接子元素 FQN" }` 两个形态要点；`## 边界` / `## 承载理由` 的正文为含 CJK 的「（待补：…）」占位句

#### Scenario: 草稿零错误装载且说明块零抽取

- **WHEN** 以草稿文本直调 `api::contract::parse(draft, fqn, "<FQN>.md")`
- **THEN** 解析成功（零 `ApiError`）；`sections.len() == 6`；`reexport_prose` 为空集；六段 `body` 内零「作者注」与零「三类 fence」字样（说明块不进任何散文段正文）

#### Scenario: 无符号元素仍出六段

- **WHEN** 对入口存在但抽不到任何函数与类型的元素调用 `bootstrap`
- **THEN** 六段齐全，`## 操作` 段为空、`api-types` fence 内容为 `{}`，草稿仍过 `parse` 零错误（第四条闸的前置不成立，占位散文原样合法）

#### Scenario: 说明错位即装载期报错

- **WHEN** 把作者注形态的行首 `- <ASCII 标识符>` 列表项或首列反引号表格行移出 fenced 块、放进任一 `## ` 段正文
- **THEN** 装载期判 `ApiError::Contract`（前者映射「`api-reexport` 键集与散文透传列表不等」、后者映射「散文表格复述契约名字」），文案带 `file` 与点名项——作者注因此 MUST 留在非 `api-*` fence 内

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/mod.rs`（`bootstrap_text`） | 草稿骨架单点（本变更改形态） | 出 H1 + 六段（`## 边界` / `## 承载理由` 追加在 `## 类型` 之后）+ 末尾 `text` fence 作者注；两段占位句含 CJK；`bootstrap(root, element) -> Result<String, ApiError>` 签名、错误文案与「零写盘、只回 stdout 语义」逐字不变；`bootstrap_text(fqn, symbols)` 形参不变（`entry` 仍不进注记面） |
| `crates/core/archi/src/api/mod_test.rs` | 草稿断言面 | `bootstrap草稿形态` 扩到六段；新增「六段 + 过 `parse` 零错误 + `sections == 6` + `reexport_prose` 空 + 六段 body 零说明正文」；新增「无符号元素仍六段」；既有 `bootstrap_text` 签名断言与注记行用例保留 |
| `crates/core/archi/src/api/contract.rs` | 解析器（零改动） | 三类 `api-*` fence 归集、其余 fence `continue`、`collect_reexport_prose` / `collect_surface_table` 两个抽取面、装载期闸群（三条双向 gate + 闸一 + 落点闸 + 自洽闸 + 第四条闸）逐字不动——本变更只依赖它们，MUST NOT 增加「说明块」识别、段名白名单或第五类 fence |
| `crates/tools/archi-api-gen/src/main.rs` | CLI 出口（零改动） | `bootstrap <元素>` 仍直出 `api::bootstrap` 的同一份文本到 stdout、零写盘；`generate` / `--check` 两子命令与前两者共用的路径派生零改动 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具出口（零改动） | `bootstrap` 分支仍回 `{ draft }`（同一份文本），过 `cap_result` 单点截断 |

### Function: `crates/core/archi/src/api/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `bootstrap_text(fqn, symbols) -> String` | 改（私有，签名零变化） | 追加 `## 边界` / `## 承载理由` 两段占位与末尾 `text` fenced 块作者注；既有 H1 / 职能 / 不变量 / 操作 / 类型五处文本逐字保留；作者注为静态文本（零占位符、零变量插值） |
| `bootstrap(root, element) -> Result<String, ApiError>` | 零改动 | 入口解析与错误面（`Element` / 无入口 / 符号抽取失败）逐字不变；仍零写盘 |

### API / CLI: 出口形状（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api(action = "bootstrap")` 响应 | 形状零变化 | 仍 `{ draft: string }`；仅 `draft` 内容按新形态，`cap_result` 单点截断不变 |
| `archi-api-gen bootstrap` | 形状零变化 | 仍 stdout 单段文本、退出码零变化 |
| DTO / IPC / 生成物 | 零改动 | `Contract` / `ApiSnapshot` / `ArchiApiFinding` / `ArchiApiCheck` 零字段新增；`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节零 diff；`bindings:export` 零差异 |

- **零变化面（同批断言，不是省略）**：`api::load_contracts` / `api::enabled` / `api::check` / `api::artifact_check` / `api::findings` / `api::render_all` / `api::api_ref` 签名与语义逐字不变；十三条 finding 判据词汇与 `REPORT_VERSION` 不变；存量的 10 份契约资产与 `.dev-team/architecture/graph/model/*.toml` 零改动（不回填作者注、不新增元素 / 关系）。
