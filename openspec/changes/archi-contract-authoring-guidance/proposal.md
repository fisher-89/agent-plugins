# 提案: archi-contract-authoring-guidance

> **变更**: archi-contract-authoring-guidance（契约起草引导与工具描述补位 · P1 前置片）
> **日期**: 2026-11-24
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（P1：可从主篇拆出、独立验收、不依赖 archi 写工具）。explore 引用的主篇 `archi-agent-contract-knowledge-gap` 不在本仓，故本提案的事实依据只取 explore 与代码实测（下表的行号即实测值）。
> **性质**: 只读面补位——一处草稿骨架文本 + 四条工具描述补位（`archi_api` 三句 / `archi_module` 一句）+ 两条能力 delta。零新增工具、零新增判据、零写盘、零 DTO / IPC / 前端 / 生成物变化；知识骨架体量冻结值不动（2596 B）。
> **不含**: `docMissing` 文案里的 bootstrap 指路（主篇的事，本仓当前 39 条 `docMissing` 文案零指路）；`bootstrap` 新增参数（如 `--with-reexport`）；自动生成 `api-reexport` fence；注入矩阵扩展（`proposal` / `dev-design` 执行相位仍无 `archi_api`）；把散文语义（职能 / 边界 / 不变量 / 承载理由）变成机械判据。

---

## 问题

### 一 · 五个缺口逐条实测（草稿与描述都没到位）

| # | 缺口 | 现状（实测） | 锚点 |
|---|---|---|---|
| 1 | 草稿无作者注 | `bootstrap_text` 只出 H1 + 四段（职能 / 不变量 / 操作 / 类型）+「（待补）」占位；不产 `api-reexport`，也不解释三类 fence 各自的落点与唯一性 | `crates/core/archi/src/api/mod.rs:759` |
| 2 | `check` 的冷启动语义无解释 | `enabled` 本就随响应出线（`artifacts.enabled` ← `api::artifact_check`），但工具描述与知识骨架都没说「`findings.total = 0` 可能是契约面未启用」 | `api/mod.rs:291`、`crates/infra/agent/src/sdk/mount.rs:296` |
| 3 | 四类生成物未点名 | 描述只有泛称「生成物」 | `sdk/mount.rs:296` |
| 4 | `schemaPath` / `generatedPath` 未指路 | 描述已提 `contractPath`（「declared 走 api.contractPath」），另两条路径零提及——而 DTO 三字段早已在案 | `sdk/mount.rs:252`、`crates/core/archi/src/dto.rs:305` |
| 5 | 起草第一步不明确 | 描述未说「新建 / 补契约先 `bootstrap` 取草稿」 | `sdk/mount.rs:296` |

### 二 · 缺口 2 的杀伤力最直接：全空被读成全绿

`archi_api(action = "check")` 的响应形如 `{ artifacts: {...}, findings: { total, byCategory, items } }`；契约面未启用时 `api::findings` 内部短路，`findings.total = 0`——与「零问题」**同形**。知识骨架的自检句只写「查两轴」，读到 0 条即收工。

同一事实在**人**的一面早已落地：契约面板写着「契约面未启用（应生成集此时只含 `INDEX.toml`）」（`packages/desktop/src/views/architecture/components/contract-panel.tsx:276`）。也就是说这口径**人看得见、agent 看不见**——是描述面缺位，而不是判据缺位。

本仓基线不暴露这个形态（`reports/archi/20261124T0413044497016Z.json`：`status = clean`、`apiFindings` 39 条全 `docMissing`、契约面已启用），所以光跑本仓看不见它；新工作区（零契约冷启动）才是。

### 三 · 起草指南放哪儿：解析器只显式跳过一类载体

`contract::parse` 的 fence 状态机只归集三类 `api-*` fence，**其余 fence 一律 `continue`**（`crates/core/archi/src/api/contract.rs:164`「其余 fence（示例代码等）不参与契约，内容不入散文区」）。散文段正文则受两个抽取面扫描：

| 抽取面 | 判据 | 后果 |
|---|---|---|
| `collect_reexport_prose` | 行首 `- ` / `* ` 且整行无 CJK ⇒ 首个 ASCII 标识符算一个透传导出名 | 与 `api-reexport` 键集比对，不等即 `ApiError::Contract` |
| `collect_surface_table` | 表格行首列整体被反引号包裹 + ASCII 标识符 | 与契约名字集（op id ∪ `api-types` 键 ∪ `api-reexport` 键）相交即 `ApiError::Contract`（仅契约非留白时判） |
| `### ` heading | op 锚点 | 其下必须紧随一个 `api-op` fence，否则「op 缺代码块」 |

⇒ 起草说明只能放**非 `api-*` 的 fenced 块**：它是唯一被解析器**结构性排除**的载体。放进裸正文或 `## ` 段，说明的每一句都要替未来的两个抽取面担保（今天能过，是因为 `- 三类 fence：…` 恰好含 CJK；明天加一条纯 ASCII 行即装载期报错）。这不是风格问题，而是「草稿生成得了、写盘后装载失败」——比没有注释更糟。

---

## 提案

### 一 · 草稿：六段骨架 + 尾部惰性作者注

`bootstrap_text` 追加两段散文与一个 `text` fence，H1 与既有四段逐字不动：

| 段 | 来源 | 内容 |
|---|---|---|
| H1 `# <FQN>` | 保留 | 必须等于文件名主干（`file_title` 判据） |
| `## 职能` / `## 不变量` | 保留 | 现占位不动 |
| `## 操作` | 保留 | 每 op 一个 `### <op-id>` + 紧随 `api-op` fence |
| `## 类型` | 保留 | 单个 `api-types` fence |
| `## 边界` | **新增** | `（待补：本模块**不**负责什么）` |
| `## 承载理由` | **新增** | `（待补：为什么它是独立元素）` |
| 文件末尾 `text` fenced 块 | **新增** | 作者注（解析器显式跳过，完全惰性） |

新增两段落在 `## 类型` 之后，既有四段的相对次序与前缀不变；两段正文本就含 CJK，故零抽取风险。作者注块原样：

````text
作者注（本块不参与契约解析）：
- 三类 fence：api-op（`### <op-id>` 下紧随）/ api-types（本模块自有 $defs，同文件唯一）/
  api-reexport（键 = 导出名、值 = 直接子元素 FQN，同文件唯一）
- 散文只留语义：职能 / 边界 / 不变量 / 承载理由；MUST NOT 用表格复述名字清单
  （名字清单的唯一持有者是三类 fence）
- api-reexport 形态：{ "导出名": "直接子元素 FQN" }；散文里的透传列表须与键集逐名相等
- 起草后跑 archi_api check 看两轴（生成物新鲜度 / 契约判据）；契约面未启用时
  enabled = false 且 findings 恒空
````

三条硬约束：① H1 不能动（`file_title` 要求 H1 = 文件名主干，否则解析失败）；② 占位句 MUST NOT 写成英文裸列表项或首列反引号表格行；③ 作者注 MUST 留在非 `api-*` fence 内。

### 二 · 四条工具描述补位：`archi_api` 三句 + `archi_module` 一句（零 preamble 字节）

| 工具 | 追加 / 改写 | 位置 |
|---|---|---|
| `archi_api` | ① `bootstrap` 加「新建 / 补契约先取草稿」的指路；② 把泛称「生成物」换成四类点名——契约 md 同目录 `.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`，仍「MUST NOT 手改」；③ 冷启动口径——`artifacts.enabled = false` 即契约面未启用，此时 `findings` 恒空且**不等于**零问题 | `sdk/mount.rs::archi_api_definition()` |
| `archi_module` | `api.schemaPath` / `api.generatedPath` 给出另两条生成物路径（`contractPath` 已指手写真相） | `sdk/mount.rs::archi_module_definition()` |

四条都落**工具描述**，不进 `archi-knowledge.md`（骨架 2596 B / 全文 2739 B 两个冻结锚不动）。描述面既有禁则不变：零本仓专用命令名——这里点名的是文件后缀，不是命令。

### 三 · 边界（明确不做）

- 不改 `docMissing` 文案：本仓当前文案零 bootstrap 指路，补指路是主篇的事；本片只让「被指的东西」变得值得指。
- 不给 `bootstrap` 加参数、不扩 action 值集（封闭二值不动）。
- 不自动生成 `api-reexport` fence：键值（导出名 ↔ 直接子元素 FQN）是设计判断。
- 不改注入矩阵：`archi_api` 仍只在 `(implement, executor)` 与 archi 来源会话可达。
- 不把散文主题变判据：本片只给骨架与说明，语义仍人审。
- 不批量回填既有 10 份契约的作者注。

---

## 能力

| 能力 | 类型 | delta 头 | 变更要点 |
|---|---|---|---|
| `desktop-archi-api` | 修改 | `## ADDED Requirements`（一条） | 草稿六段骨架 + 尾部惰性作者注 + 「草稿自证可解析」机械锚；契约形态 / 三类 fence / 装载期闸群 / 生成物零变化 |
| `desktop-archi-agent` | 修改 | `## MODIFIED Requirements`（两条） | `archi_module` 描述面补第四句（`schemaPath` / `generatedPath`）；`archi_api` 补描述面 bullet（三句）、语义句改为「自检与草稿」（与 requirement 标题同口径），并把草稿场景从「四段」同步为六段 |

`desktop-archi-api-check` 不动：`enabled` 判据与 `apiFindings` 槽位语义零变化，本片只补「怎么读这个字段」的描述面。

---

## 变更范围

### 实现文件

| 文件 | 变化 | 要点 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | 改（`bootstrap_text`） | 追加 `## 边界` / `## 承载理由` 两段与末尾 `text` fence 作者注；`bootstrap` 签名、错误文案、零写盘逐字不变 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | 改（两个描述函数） | `archi_api_definition()` 加三句（指路 / 四类生成物点名 / 冷启动口径）；`archi_module_definition()` 加一句（两条生成物路径）；参数 schema、工具名常量、`ToolTier::Read` 零变化 |
| `packages/desktop/package.json` | 改（版本） | 0.7.9 → 0.7.10（AGENT.md 版本纪律：desktop 源码改动一拍 patch；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行不动） |

### 测试文件

| 文件 | 变化 | 要点 |
|---|---|---|
| `crates/core/archi/src/api/mod_test.rs` | 改 + 新增 | `bootstrap草稿形态` 扩到六段；新增「草稿过 `contract::parse` 零错误 + `sections` 恰六段 + `reexport_prose` 空 + 六段 body 零说明正文」；新增「无符号元素仍六段」；既有 `bootstrap_text` 签名断言保留 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 改 | `archi_api定义描述与参数形状` 与 `archi_module描述补三句指路` 的 needle 扩充（`第一步` / `.schema.json` / `INDEX.toml` / `.i.` / `enabled` / `findings` / `schemaPath` / `generatedPath`）；两条既有边界（零专用命令、零代码围栏）保持绿 |

### 不要修改

- `crates/core/archi/src/api/contract.rs` 及其语料：本片只**使用**既有「非 `api-*` fence 惰性」语义，解析器零改动。
- `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs`：骨架 / 全文体量锚（2596 / 2739 B）与既有断言不动。
- `crates/core/workflow/src/prompts/*.md`（14 份：6 executor + 8 evaluator）：相位 prompt 零改动（矩阵与措辞都属既有立场）。
- `.dev-team/architecture/models/**`（现存 10 份契约）与 `graph/model/*.toml`：不回填作者注、不新增元素 / 关系（`archi_drift` 的 55 元素 / 129 关系不变）。
- DTO / IPC 命令面 / `src/types/generated/**` / 架构页前端组件 / 四类生成物（`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）。
- `crates/tools/archi-api-gen/**`：CLI `bootstrap` 直出同一份草稿，零改动。
- `crates/infra/agent/src/sdk/archi_tool_test.rs`：`archi_api_bootstrap出草稿` 的「与 `api::bootstrap` 直出逐字相等」断言已自动覆盖新形态，无需改。

---

## 验收标准

| # | 断言 | 机械锚 |
|---|---|---|
| 1 | 草稿含 H1 + 六段，段序为 职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由 | `mod_test.rs` needle |
| 2 | 作者注在文件末尾的非 `api-*` fenced 块内，含「api-reexport」与 `{ "导出名": "直接子元素 FQN" }` | needle + 解析位置 |
| 3 | 草稿自证可解析：`contract::parse(draft, fqn, "<FQN>.md")` 零错误、`sections.len() == 6`、`reexport_prose` 空、六段 body 零「作者注」 | 新增用例 |
| 4 | 无符号元素草稿仍六段（`## 操作` 空、`api-types` 为 `{}`），仍过 `parse` | 新增用例 |
| 5 | `archi_api` 描述含三句：bootstrap 指路（针 `第一步`）/ `.schema.json` + `INDEX.toml` + `.i.` 点名 / `enabled = false` 口径（同句含 `findings`） | `mount_test.rs` needle |
| 6 | `archi_module` 描述含 `schemaPath` 与 `generatedPath` | `mount_test.rs` needle |
| 7 | 描述面两条既有边界仍绿：零本仓专用命令名、`archi_module` 零代码围栏 | 既有用例 |
| 8 | 知识骨架 / 全文体量锚逐字不变（2596 / 2739 B），`archi-knowledge.md` 零 diff | `archi_knowledge_test.rs` 两条冻结锚 |
| 9 | 零写盘不变：`bootstrap` 调用前后架构域字节相同；工具直出与 `api::bootstrap` 逐字相等 | `api/mod_test.rs` / `archi_tool_test.rs` |
| 10 | 零外溢：DTO / IPC / `bindings` / 四类生成物 / 前端零 diff，`api:check` 与 `client:check` 零差异 | `check` 链（`bindings:export` 幂等） |

---

## 风险

| # | 风险 | 影响 | 缓解 |
|---|---|---|---|
| 1 | 作者注错位（裸正文 / `## ` 段正文 / `### ` heading） | 草稿生成成功、写盘后装载期报错，比没有注释更糟 | 说明只落非 `api-*` fence；验收 #3 把「六段 body 零说明 + `reexport_prose` 空」钉成锚；spec 写明三处禁放点及各自后果 |
| 2 | 占位句将来被改成英文裸列表或表格行 | 被抽成透传导出名 / 散文表格复述 ⇒ 装载期 `ApiError::Contract` | 占位句固定为含 CJK 的「（待补：…）」；`reexport_prose` 空断言同批 |
| 3 | 冷启动口径只活在工具描述里（骨架零字节） | 描述一旦被压缩，`enabled = false` 语义丢失 | `mount_test.rs` 以 `enabled` / `findings` needle 固定该句；骨架体量锚反向保护（不许为塞这句改骨架） |
| 4 | 描述承诺与生成物族漂移（后缀改名） | 描述说谎，agent 找不到生成物 | 四条只给后缀形态（四类生成物已在 `desktop-archi-api` 冻结）；needle 随改动同批更新 |
| 5 | 引导可达面有限：`archi_api` 只在 `(implement, executor)` 与 archi 来源会话 | `proposal` / `dev-design` 执行相位（模型可写相位）看不到「先取草稿」的指路 | 本片显式不改注入矩阵（主篇五处立场之一）；起草动作实际发生在持有该工具的会话 |
| 6 | 作者注随草稿进入提交文件，存量契约不回填 | 新旧契约说明不一致；每份新契约多约 0.6 KB | 说明块惰性、零判据、不设删除纪律；不做批量回填 |
| 7 | 草稿体量增长（约 +0.6 KB/份） | 大模块草稿更长 | `cap_result` L1（50000 B）余量充足；草稿仍只出 stdout 不写盘 |
| 8 | spec 分叉：`desktop-archi-agent` 的草稿场景原写「四段」 | 不改即与实现矛盾 | 本变更同批 MODIFIED 该条；形态归属以 `desktop-archi-api` 的 ADDED 为准，agent 侧只保留「与 CLI 同一份草稿文本」 |
