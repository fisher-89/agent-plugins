# 提案: archi-agent-contract-knowledge-gap

> **变更**: archi-agent-contract-knowledge-gap（架构模型知识工具化 · 切片 A）
> **日期**: 2026-11-24
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（v2「TOML 知识工具化」整合方案；三片前置 P1 / P2 / P3 均已落地并归档）。change 目录名来自 v1 的「契约 / API 知识缺口」分析，方案本体已按 v2 改为「语法知识迁进读写工具」——目录名不动，`workflow_type = requirement`。
> **性质**: **写面 + 散文 + 立场三件事的一次原子交付**——给 agent 加两个模型写工具（`archi_element` / `archi_relation`）+ 七条写后警告 + 读侧单文件哈希闭环，并把语法类知识从系统提示迁进工具面（骨架 2597 B → 目标 ≤ 1500 B），同批改写五处被固化的立场句与两条交叉引用。
> **不含**: L3b（模型目录进写保护锁面、`write` / `edit` 一律拒绝）——D3 裁决为「L3a 双写面并存是终态」；元素在文件内的插入位置（D6 后置到 `toml-sorting-and-layout`）；`config.json` 写面（D4）；契约债清账（D11）；孤儿契约 md 的 drift 判据（D10-B）。

---

## 问题

### 一 · 语法知识留在散文的唯一理由，已被 P3 抽掉

`desktop-archi-agent`「知识分层与工具面同批移动」补充规则①原文把「未引号点分表头」一类形态错误归 C 档，理由写着**动作前不可判定**（写面是自由文本 `write` / `edit`，结构诊断只在写盘后可得）。P3 落地后，`edit::plan_*` 已能在**产文本之前**跑完「渲染 → 重解析自证 → 回读一致」——判据在动作前可得。**前提消失，C 档身份随之失效**；而同一 requirement 的「移动分层原子性」条款写着只删不加与只加不删同样违约——故删骨架里的语法段不是优化项，是条款要求。

现状读数（`archi-knowledge.md`，LF 2597 B）：位置与键形 438 B / 字段纪律 827 B / 元素类型 454 B / 层级与读数 330 B / 边界与自检 262 B / 写面 259 B。可迁出 ≈ 1850–1900 B（72–74%），其中「字段纪律」与「元素类型」两段（1281 B）的接收端在 P2 已就位（`SCAFFOLD_TEMPLATE` 字段词典）。

### 二 · 写面只有自由文本，五类事实静默

模型写权今天的入口是 `write` / `edit`（`graph/model/**` 由 `model_write_lock` 按相位放开）。这条通路上：

| 事实 | 今天的表现 |
|---|---|
| 键形（带引号 FQN 单键 / 内联 `metadata` / `[[relation]]`） | 写错即整份源解析失败，读面报错 |
| 跨字段判据（`kind` × `path` 基数：`package` 至多 1 条） | 写出来必停摆，**动作前无判据** |
| 祖先先声明（`orphanHierarchy`） | 写出来才在 `archi_drift` / `archi_module` 里现形 |
| 表头计数（`（N 元素 / M 关系）`） | 人工抄写，写一次漏一次 |
| 契约 md 与 `api` 状态的一致性（`docMissing` / `apiUnexpected`） | 无人过问，直到下一次漂移读数 |

五类事实**全部可由工具在渲染后立刻算出**，却全部要模型自己记。这正是「知识归属」判据（explore §1.1 ①）指出的错配。

### 三 · P3 交付了引擎与 19 变体，缺三块语义与一处读侧闭环

`core/archi/src/edit/`（1028 行 + 3315 行测试）已交付 4 × `apply_*` / 4 × `plan_*` / `EditErrorKind × 19` / `file_hash` 族。四处缺口：

| # | 缺口 | 现状锚 |
|---|---|---|
| ① | **删除只有拒绝臂** | `plan_element_delete` 有子元素 / 被引用即 `Err`，无 `cascade` |
| ② | **元素写回不带出边** | `ElementSpec` 无 `relations`；「加元素 + 3 条边」要两次 plan、两次写 |
| ③ | **落点只有最小版** | 新建必须显式给 `file`；「省略即落最近已声明祖先文件」未实现 |
| ④ | **读侧无单文件哈希** | `drift::file_hash` 已导出但无 DTO 承载：`archi_drift.modelHash` 是全资产聚合、`ArchiElement` 只有 `modelFile` / `modelLine` |

④ 直接决定乐观锁能不能用：没有读侧回传，agent 每次写都要自己猜「我有没有基线」，`expect` 就成了摆设。

### 四 · 五处立场把「无专用写工具」写成绝对命题

加工具就和这五处**字面冲突**（explore §7.1）：

| # | 锚点 | 现文 | 问题 |
|---|---|---|---|
| 1 | `desktop-archi-agent`「模型 = 源码（写纪律与不设专用写工具）」 | 「MUST NOT 存在专用『写模型』工具」 | 绝对禁令；加了工具即违约 |
| 2 | 同 requirement | 「写入不校验，读取报错」 | 把两种失败语义并成一句口号；加工具后必须拆成「写工具坏文本不可产出」/「手改面仍由读取面报错」两半 |
| 3 | 同 requirement Scenario | 「无专用写工具（五只读）」 | 断言面直接反向 |
| 4 | 两个 phase prompt（`proposal-planner.md` L38 / `dev-design-planner.md` L44）与 `desktop-phase-prompts` 的同名断言 | 「（**无专用写工具**）」 | 同上，且**prompt 里写死一个会被工具面否证的句子**最危险 |
| 5 | `desktop-archi-agent` 补充规则① | 理由已改为「动作前不可判定」 | 缺一句「结构化写面已消除该前提（手改面除外）」 |

另有两处**交叉引用**随 ① 的改名失效：`desktop-write-protection` 两处、`desktop-archi-chat` 一处（后者还独立写着「无专用写工具 / 写入不校验」）。

### 五 · 只加工具不删散文 = 同一事实两份持有者

骨架里 1281 B 的字段纪律与 `kind` 语义，P2 后的接收端是 `archi_scaffold` 草稿（字段词典）与 `archi_element` 参数 schema。工具落地而散文不动，等于把「字段形态」同时交给三处（散文 / 草稿 / 参数描述）——本仓反复禁止的第二事实源。故本次**删除与新增必须同批**，这不是排期偏好而是分层原子性条款。

---

## 提案

### 一 · 两个写工具与写权

```
archi_element   元素 upsert / delete（可内联出边）    ToolTier::Write   ToolExec::Archi
archi_relation  单条关系 upsert / delete              ToolTier::Write   ToolExec::Archi
```

- **执行体落点**：两者同走 `sdk/archi_tool.rs` 的 `ArchiContext::execute`（第六、七臂），入参是 `Value`——沿既有形态手工解析（`archi_api` / `db_query` 同款），**不假设框架校验**（见 §五）。
- **落 `ToolTier::Write`**：`default` 档拒绝、`acceptEdits` / `bypassPermissions` 放行，与 `write` / `edit` 同档——档位判据仍是 `policy::allows` 的单点比较。
- **写权矩阵**：只加在已有写权的三处——`(proposal, executor)`、`(dev-design, executor)`、`archi` 臂；其余相位零下发。矩阵集合 SHALL 与 `compose::model_write_lock` 的不挂锁集合**逐格相等**（新增断言，防「给了工具但锁着」）。
- **无第三条通路**：仍不新增任何 IPC 写命令；`archi_set_gate` 与两写工具共用 `plan_element_upsert`（同一实现 ⇒ 同一份文本，画布不变）。
- **写工具 MUST NOT 成为唯一入口**：`write` / `edit` 对模型的射程不变（L3a 终态），骨架永久保留一句「手改出口」。

### 二 · 更新语义与批量约束

| 写法 | 含义 |
|---|---|
| 字段省略 | 不动 |
| 字段给 `null` | 清空 |
| `relations` 省略 / 空数组 | 出边不动 |
| `{"to": X}` | 该边 upsert（改 `desc` / `apis`；不存在即建） |
| `{"to": X, "action": "delete"}` | 该边删除；不存在即 `Err`（列同 `from` 的现有出边） |
| 未列出的既有边 | 保持不动（全量替换会一次漏列即静默删依赖） |

四条约束（D5 支持内联删边后新增）：

1. **同一 `to` 在一批里只出现一次** ⇒ 重复即 `Err`（否则「先 upsert 后 delete」取决于数组序，不可判定）；错误定位用 **1 基下标**且与 `action` / `to` 同时出现（`relations[2]（action = delete, to = "Backend.Legacy"）…`）。
2. **`action = delete` 项不得带 `desc` / `apis`** ⇒ 带了即 `Err`（删除动作与字段赋值互斥，静默忽略会让 agent 以为赋值生效）。
3. **内联 `to` 只在目标文件内生效**：`plan_element_upsert` 一次只写一个文件；某条内联边**已声明在别的模型文件**⇒ `Err` 点名该文件并指向 `archi_relation`（MUST NOT 静默只改同文件那份，也不做跨文件写）。
4. **删除仍然显式**：内联 `delete` 省的是往返次数，不是「把删除藏进批量」——与独立 `archi_relation delete` 同一判据、同一文案族（MUST NOT 为内联路径另写「不存在就静默跳过」的宽松版）。

### 三 · 删除与级联（单文件语义）

```
cascade = false（缺省）  有子元素 / 被引用  ⇒ Err 并点名（P3 行为逐字保留）
cascade = true           先算闭包（子树 ∪ 出入边），再按文件分组：
                         全在目标文件 ⇒ 删；跨文件 ⇒ Err 点名文件与阻塞项（不落盘）
```

**为什么级联不出单文件**：`plan_*` 的产物是**一个** `ModelWrite`（一个文件、一次原子写）。跨文件级联要么引入多文件事务（部分失败无解），要么把删除藏成 N 次无人复核的写——两条都比「拒绝并点名」差。故 `cascade` 的承诺面就是「同一模型文件内的子树与边」，spec 与工具描述都按此措辞（MUST NOT 让 `cascade` 字面暗示跨文件）。

纯函数层随之收敛为**一个删除函数**：`apply_element_delete(source, &DeleteSet)`（`DeleteSet` = 元素 FQN 集 + 关系对集）；单元素删除是「集合只有一个元素」的退化形态——MUST NOT 保留第二个「只删一个元素」的实现。

### 四 · 落点

| 场景 | 行为 |
|---|---|
| 既有元素 | 落其声明所在文件；`file` 给了不同值 ⇒ `Err`（迁文件属独立语义） |
| 新建 + 省略 `file` | 落**父元素**声明所在文件（祖先须显式声明是既有前置，故「最近已声明祖先」= 父元素） |
| 新建 + 单段 FQN（无父） | `Err`，点名需给 `file`（新分层的落点必是人的判断） |
| 新建 + `file` + `create = true` | 新建文件；表头由工具生成 `# <名>（1 元素 / 0 关系）`（语义部分不猜） |
| 新建 + `file` 不存在 + 无 `create` | `Err` + 提示 `create = true`（防拼错名字造成幽灵文件） |

**不引入落点约定**（D9）：文件名里的语义序号（`01-app` / `06-frontend`）工具无法推导，MUST NOT 按 `kind` / 首段 FQN 推落点——那是新的隐式知识，且本仓 `Backend.Core` 与 `Desktop.Views` 同文件的反例无法用规则表达。

### 五 · 反馈面：三通道 + 七条 warnings

```
                    ┌─ Err        阻塞，零落盘 ── 判据 + 当前值 + 可复制重试参数
一次写工具调用 ──────┼─ warnings[] 已落盘，有未阻断事实 ── 判据 kind + severity + 下一步
                    └─ changed[]  已落盘，幂等凭据 ── 实际变化的字段名
```

`hints` **并入 `warnings`**（不设第二通道：拆开后 agent 可能只看 message 不看 hint）。

七条 warning 只由**零成本事实**推导（工具已持有的字段值 + `Path::exists`），MUST NOT 跑 `drift::check`（含 scan，写是高频动作）：

| # | 条件 | kind（既有词） | severity | message 尾句 |
|---|---|---|---|---|
| 1 | `package` / `module` 且 `path` 空 | `noPath` | warn | 纯分组容器请改 `composite` |
| 2 | 某条 `path` 不在盘 | `pathNotFound` | warn | 先行声明可忽略 |
| 3 | `api = declared` 且契约 md 不在盘 | `docMissing` | warn | 用 `archi_api` 的 `bootstrap` 取草稿 |
| 4 | `api = none` 且 `doc` 空 | `docMissing` | warn | 入口文档经 `doc` 声明 |
| 5 | `api = none` 且 `doc` 指不到盘 | `docNotFound` | **error** | 先建文件或改 `declared` |
| 6 | `api = none` 但契约 md 已在盘 | `apiUnexpected` | warn | 删契约 md 或改 `declared` |
| 7 | 删除时契约 md 仍在盘 | `apiUnexpected` | warn | 一并清理（否则无人看管） |

kind 字面量取自 `ArchiDriftKind` / `ArchiApiFindingKind` 的 `as_str`；severity 取自 `ArchiApiFindingKind::severity()`（`drift/report.rs` 单点）+ 「drift 侧两值恒落 `warnings` 桶 ⇒ 恒 `warn`」这一既有事实；路径用 `api::paths::contract_path` / `doc_path`。写工具**不定义档位、不另拼路径、不新增词汇**。

**降噪不变式（必需）**——本仓多数元素是 `api = declared` 且契约 md 缺失，不限流会让 `docMissing` 每次都回、三次之后 agent 整体忽略该通道：

```
action = upsert 时：warnings 的 kind 集合 ⊆ 由 changed 诱发的 kind 集合（created ⇒ 全字段）
  noPath / pathNotFound          ← changed ∋ path 或 kind
  docMissing / docNotFound / apiUnexpected ← changed ∋ api 或 doc
  只改 summary / gate / metadata ⇒ warnings 恒空
action = delete 时：仅允许第 7 条（孤儿契约 md），由动作本身解释
```

### 六 · 响应体与截断

```jsonc
{ "warnings": [ … ],                                  // 恒在场（空数组 = 无异常）
  "action": "upsert", "element": "Backend.Billing",   // 身份
  "file": "…/02-backend-core.toml", "hash": "sha256:…",  // hash = 写后新值（喂下一轮 expect）
  "created": true, "changed": ["kind", "summary"],    // 幂等凭据
  "relations": { "created": 1, "updated": 0, "deleted": 1, "unchanged": 2 },
  "counts": { "elements": 10, "relations": 10 },      // 本文件（= 表头重算来源，可对拍）
  "model":  { "elements": 56, "relations": 129 } }     // 全模型（= 前值 ± 增量，可对拍 drift）
```

- `relations` 四计数恒等于入参 `relations` 数组长度（可断言的守恒式）；不带 relations 时四项皆 0（不是省略）。
- `archi_relation` 的响应更薄：`{ warnings: []（恒空）, action, from, to, file, hash, created, changed }`，`changed` 是 `desc` / `apis` 的子集。
- **成功响应结构上界**：7 × ≤200 B + 其余短串 ≈ 2 KB ≪ `MAX_RESULT_BYTES`（50 000）⇒ 成功响应**永不触达截断**，`warnings` 的位次因此是防御而非机制（见「与 explore 的偏离」⑤）。

### 七 · 入参面与 Err 纪律

**E1（入参）是执行体，不是框架**——实测纠偏（见「与 explore 的偏离」②）：archi 工具的执行体拿的是裸 `Value`（`ArchiContext::execute`），没有任何 JSON Schema 校验器在链上；`bash.intent` 的既有断言还明写「长度只软提示，不升为协议约束」。故：

- **schema 是模型侧提示**：八行上限（FQN ≤256 / `path` ≤32 项 × ≤1024 / `metadata` ≤32 项（键 ≤64、值 ≤512）/ `relations` ≤64 / `relations[].apis` ≤32 × ≤128 / `summary`·`desc` ≤512 / `entry`·`doc` ≤1024 / `file` ≤256）与 `enum` / `required` 一并进定义。
- **硬闸在 E1 解析**：`action` / `element` / `kind` / `gate` / `api` / `relations[].action` 的值域与上限，由执行体显式检查（`ok_or_else` 形态，沿 `archi_api` 既有写法）；**限额常量单点**（`WRITE_LIMITS` 一组 const）同时派生出 schema 键值与检查，MUST NOT 在两处各写一份数字。
- **E2 前置读数**：模型已有结构诊断 ⇒ `Err`，沿用 `dto::graph_diagnostics_error` 的**全量形**（MUST NOT 为写工具另写第二份诊断渲染，见偏离④）。
- **E3–E7 走 `EditError`**：语义（祖先缺失 / 关系成环由自证链兜 / 端点不存在 / 删除有引用 / kind×path 冲突 / 批量四条）/ 落点 / 删除 / 自证 / 并发（`expect`）全部沿用 `EditErrorKind`（19 → 23 变体），壳层经 `EditError::message()` 映射。
- **四条硬纪律**：第一行自足；字面与词表同源（`gate` 不写「档位」、`unknownReference` 不译中文）；不猜（给候选不代选）；一次报全。

### 八 · 读侧闭环：`ArchiModuleDetail.fileHash`

P3 的 `drift::file_hash` 已 `pub` 但**没有任何 DTO 承载它**：`archi_drift.modelHash` 是 `graph/model/*.toml` 的全量聚合（6 文件累加），单文件 `file_hash` 与它**数值不可比**。若把新字段也叫 `modelHash`，agent（与人）极可能把聚合值塞进 `expect` ⇒ 恒不匹配的**假冲突**。

故本提案取 explore D12 的**实质**（单文件哈希落在 `ArchiModuleDetail`）、改其**字面**：字段名 `fileHash`。

| 面 | 增量 | 约束 |
|---|---|---|
| `ArchiModuleDetail.fileHash` | `String`（`sha256:<hex>`） | 与 `drift::file_hash` 同一函数产出、同前缀；`modelFile` 已知，零额外读盘 |
| `bindings.ts` / `bindings.zod.ts` | 重导（`bindings:export`） | 生成物勿手改 |

闭环：`archi_module` 取 `fileHash` → `archi_element { …, expect: fileHash }` → 响应回新 `hash` → 下一次写零额外读。`archi_set_gate` 命令面**不暴露** `expect`（前端单用户、无并发场景，暴露只加 IPC 面）。

### 九 · 知识骨架收缩

四段结构（约 1.25 KB，`# 架构模型须知` 保留）：

| 段 | 档 | 内容 |
|---|---|---|
| **职责** | C | 模型只写结构与三个指针（`api` / `entry` / `doc`）；接口与导出符号归契约资产；只改架构目录；直改主仓随 `git diff` 评审；写盘后画布不自动刷新，提示刷新；续接历史会话先取当下模型 |
| **工具面** | A/B | 读五工具各一句「何时用」；写两工具 = canonical（字段面 / 键形 / 缺省 / 值域 / 祖先 / 计数全在工具里）；**手改出口**一句（工具表达不了的才手改；改完必须 `archi_module` 自查；坏模型由读取面报错带 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改 |
| **留给判断** | C | `kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；纯分组容器用 `composite` |
| 派生路径段 | — | 两处目录（模型声明 / 契约资产）由 `foundation::layout` 派生，逐字保留 |

- **门限**：骨架 ≤ **1500 B**、全文 ≤ **1700 B**（实测值冻结；测量口径沿用既有 `lf_bytes(trim_end)`——LF 归一、去尾随换行，故基线冻结值是骨架 2596 B / 全文 2739 B，骨架磁盘字节 2597 B 只是多一个尾随换行）。基线 2700 / 2850 随知识面收窄下调；超限先删冗余，MUST NOT 抬锚。
- **迁移表**（explore §6）：位置与键形 → 工具渲染 + parser 诊断；字段纪律（827 B）→ `archi_element` 参数 description + `archi_scaffold` 词典 + parser `Err`；`kind` 四值（454 B）→ `ElementKind` enum 描述（`ALL` 派生）+ 词典；层级与读数（330 B）→ 祖先 Err + 响应 `counts` + 一行 `config.json`；边界与自检 / 写面（521 B）**保留**并压成四句。
- **散文零结构诊断词**：`orphanHierarchy` / `dependsOnAncestor` 两个「可执行词」也不再进骨架（祖先错误已由工具 `Err` 与 `archi_drift` 读数承载）；A 档三列机械表随之收窄。
- **值域词表出骨架**：`gate` / `api` 两行与 `kind` 四值 bullet 从骨架迁到工具参数 description 与草稿词典；知识语料测试的「md ↔ `ALL` 同序对拍」随之改判（词表由工具面持有）。

### 十 · 五处立场改写（逐条）

| # | 现状 | 改为 |
|---|---|---|
| 1 | 「MUST NOT 存在专用『写模型』工具」 | 「写模型有两条入口：专用写工具（canonical）+ `write` / `edit`（second path，长期保留）」；禁令删除 |
| 2 | 「写入不校验，读取报错」 | **拆两半**：「写工具**坏文本不可产出**（自证链）」/「手改面写坏模型仍是合法状态，由读取面报错」——MUST NOT 混为一句 |
| 3 | Scenario「无专用写工具（五只读）」 | 「两写 + 五只读；两通路并存，写工具非唯一入口」 |
| 4 | 两个 phase prompt 与 `desktop-phase-prompts` 的「（无专用写工具）」 | 「经 `archi_element` / `archi_relation` 写（工具表达不了的才手改）」+ 保留自查句 |
| 5 | 补充规则① 的 C 档理由 | 补一句：结构化写面对**工具通路**消除了「动作前不可判定」，故语法知识的 C 档身份只在**手改面**残留；并新增分层判据「工具可承载的知识 MUST NOT 留在散文」 |

改名两处（交叉引用同批修正）：`desktop-archi-agent` 的「只读档位归属与成员校验不变」→「archi 工具档位归属与成员校验（五只读 + 两写）」；「模型 = 源码（写纪律与不设专用写工具）」→「模型 = 源码（两条写入口与写纪律）」。`desktop-write-protection`（两处）、`desktop-archi-chat`（一处）的引用同步换名。

### 十一 · 写保护面对新工具的射程（提案新增，非 explore 条目）

`protect::evaluate` 只分派 `write` / `edit` / `bash`（`_ => None`）。新工具写的是**同一批文件**，若不过这道闸，用户在工作区配置里 glob 了架构域路径的护栏（`desktop-write-protection` 明写「首命中即拒，MUST NOT 因『模型是源码』而放行」）就对它静默失效。

故：两写工具 SHALL 把**解析后的目标文件**过**同一** `ProtectRules`（内置 → 会话 → 用户三段序、首命中即拒、拒串沿用既有族），MUST NOT 另立规则集或第二份文案。挂点（loop 事前 / 执行体内事后）是实现选择——落点只有 `plan_*` 之后才知道，故在执行体内判定是自然解。

### 十二 · 与 explore 的偏离（逐条）

| # | explore 草案 | 本提案 | 理由 |
|---|---|---|---|
| ① | 「`relations` 批量」未涉及跨文件边 | 跨文件边 ⇒ `Err` 并指向 `archi_relation` | 一次 plan = 一个文件、一次原子写；静默只改同文件那份是骗人 |
| ② | 入参上限「框架拦」 | 上限进 schema（模型侧提示）+ E1 执行体硬闸；限额常量单点 | 执行体拿裸 `Value`，链上无 JSON Schema 校验器；`bash.intent` 既有断言明写「长度只软提示」 |
| ③ | `cascade` = 级联删引用关系 / 子树（深度优先） | `cascade` = **单文件内**级联，跨文件 ⇒ `Err` | 同上：多文件事务无解，拒绝并点名优于半成功 |
| ④ | Err 诊断超 20 条按 kind 聚合 | 沿用 `graph_diagnostics_error` 全量形（截断由 `cap_result` 兜底） | 诊断渲染是四消费面共用的单点，为写工具另设聚合即第二份渲染；首行自带「共 N 条」，「一次报全」纪律不削弱 |
| ⑤ | `warnings` MUST 是响应第一个字段（截断优先级） | 响应含 `warnings` 恒在场，但**不作键序断言** | 成功响应结构上界 ≈ 2 KB ≪ 50 KB，截断不可能命中；`serde_json` 未开 `preserve_order`，键序不宜作机制 |
| ⑥ | `warnings[].message` 须含判据 kind 字面量 | `kind` 字段即字面量，message 只写判据句 + 事实 + 下一步 | 同一事实两份持有者；机械断言改为「`kind` ∈ 既有两词汇集的 `as_str`」 |
| ⑦ | `archi_module` 增 `modelHash`（D12） | 字段名 `fileHash`，实质不变 | 与 `ArchiDriftReport.modelHash`（全资产聚合）同名不同义 ⇒ 假冲突陷阱 |
| ⑧ | 「删除级联」与「内联出边」写在 §4.5 / §4.1 的工具契约里 | 下沉到 `desktop-archi-model` 的原语族（`plan_*` 语义），工具面只做参数映射 | 引擎与工具面分层是 P3 已立的约定；引擎在 core、工具在 infra |
| ⑨ | 写盘链是「壳层 `write_atomic`（唯一落盘点）」（§4.6 ⑤） | 唯一实现迁 infra 侧 `crates/infra/agent/src/sdk/atomic.rs`，壳层 `commands/atomic.rs` 改薄委托；落位登记由 `desktop-config-agent` 的 delta 追改 | agent 写工具的执行体在 infra，infra 不依赖壳层；「模型写盘只有一份实现」的承诺要求壳层命令面与写工具共用同一函数 |

---

## 能力

### 新增能力

无。写工具面归 `desktop-archi-agent`（工具面 / 注入 / 知识是一套机制，拆出去会把矩阵变成两份持有者）。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-agent` | **RENAMED ×2 + MODIFIED ×7 + ADDED ×2** | ① 新增两条 requirement：写工具面（`archi_element` / `archi_relation` 的 schema、落点、删除与级联、批量四条、档位、成员校验）与写工具的响应 / 入参 / Err 面。② MODIFIED：矩阵（两列 + 两行 + 与写权锁对齐断言）、档位归属（16 → 18 项，五读 + 两写）、`模型 = 源码`（改名 + 两入口 + 拆两半的失败语义）、系统知识注入（四段结构 + 新门限 + 词表出骨架 + 零诊断词）、知识分层（补充规则① 与新增判据）、`archi_module`（`fileHash`）、`archi_scaffold`（一句写面指称）。 |
| `desktop-archi-model` | **MODIFIED ×2 + ADDED ×1** | ① MODIFIED 写回原语族：`ElementSpec.relations` + 批量四条、`cascade` 单文件闭包、落点（父元素 / `create` 显式）、`WriteOutcome` 承载反馈、删除纯函数收敛为 `DeleteSet` 一形。② ADDED：写面反馈的判据来源（七条 kind / severity / 路径单源 + 降噪不变式 + 零 drift 调用）。③ MODIFIED IPC 命令面：`archi_module` 出参增 `fileHash`；`archi_set_gate` 走同一 `plan_element_upsert`（命令面不暴露 `expect`），写命令仍两条。 |
| `desktop-agent-execution` | **MODIFIED ×1** | 「会话级工具面注入与 explore 笔记工具」：archi 臂名字清单五 → 七、archi 上下文铸造判据两名字、tier 归属条（两写工具落 `Write`）与 archi 臂 Scenario 的工具计数。 |
| `desktop-phase-prompts` | **MODIFIED ×1** | 「桌面端特性适配（迁移是改写非搬运）」：archi 工具引用纪律按角色补两写工具（仅两个 planner）、写权相位指示改写（工具优先 + 手改出口）、Scenario 的断言串随动。 |
| `desktop-archi-chat` | **MODIFIED ×1** | 「写权落点与刷新纪律」：工具面句（不再写死「两 archi 只读」）、「直改主仓」句拆两半（写工具 canonical / 手改面读取报错）。 |
| `desktop-write-protection` | **MODIFIED ×2** | ①「架构域路径不在内置锁面」与「会话级写入禁区（相位锁）」的交叉引用换名（零语义变化）。② 相位锁 requirement 增一条「判定的工具面覆盖」：两写工具的目标文件过同一 `ProtectRules`（见 §十一）。 |
| `desktop-config-agent` | **MODIFIED ×1**（含 Module Contract 登记追改） | 「写面命令 `workspace_config_apply` 与写权边界」：`write_atomic` 的**唯一实现**迁到 infra 侧 `crates/infra/agent/src/sdk/atomic.rs`，壳层 `commands/atomic.rs` 改薄委托（签名 / 错误文案 / `.archi-tmp` 后缀 / 失败清临时文件 / 父目录缺失即 `Err` 逐字不变）；Module Contract 的 `Component: .../src/commands/atomic.rs（新增）` 改「（改）薄委托」+ 新增 infra 侧 `Function` 条目。行为零变化，配置写面其余条款逐字不动。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-drift` | 判据、五值 kind、`ALL`、报告字段、`fileHash` 的邻居 `modelHash` 口径逐字不变；七条 warning 复用其词表但**不新增判据**（词表归属不动）。P3 已把 `file_hash` 族写进「单文件哈希（写面乐观锁粒度）」，本变更只加消费者。 |
| `desktop-archi-api` / `desktop-archi-api-check` | `ArchiApiFindingKind` 仍十三值、severity 单点、`docMissing` 不纳管、`passed` 判据逐字不变；七条 warning 只**引用**这些判据，不重定义、不扩词汇（D10-B：孤儿契约 md 不进 drift 判据）。 |
| `repo-archi-model` | 零模型数据变化：55 元素 / 129 关系 / 六个文件表头计数逐字不变（本变更不动 `.dev-team/architecture/**`）。 |
| `desktop-archi-canvas` / `desktop-archi-contract-panel` / `desktop-archi-gate` | 画布仍走 `archi_set_gate`（同一 `plan_element_upsert`，产同一份文本）；契约面板与门禁的写盘单点、报告结构零变化。 |
| `desktop-ipc-type-bindings` | 命令清单与「出线类型清单」机制零变化；`ArchiModuleDetail` 增一字段属类型形状随动，由 `bindings:export` 生成（无 spec 条款持有该字段清单）。 |
| `desktop-workspace-config` | 零 delta：该能力的 `write_atomic` 只在命令体描述里作为调用名出现（`check root` → `plan_apply_config` → `write_atomic` → 重读返回），签名与调用面在落位迁移后逐字不变；**实现落位的登记追改归 `desktop-config-agent`**（其 Module Contract 是 `commands/atomic.rs` 的唯一登记处，单份持有）。 |
| `desktop-archi-file-tree` / `desktop-app-shell` / `desktop-agent-db-tools` / 前端各能力 | 与本变更无交集。`desktop-archi-file-tree` 内「唯一写命令仍是 `archi_set_gate`」是**既有陈旧计数**（写命令自 `archi_api_generate` 起为两条），不属本变更追改面（沿「历史读数不追改」的既有处置）。 |
| `desktop-agent-execution` 的「保护闸」条 | 「五 archi 只读工具不在保护射程（`_ => None`）」仍成立——只读工具不写盘；新工具的射程条款落在 `desktop-write-protection`（单处持有）。 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs` | `ElementSpec.relations: Vec<RelationSpec>`（含 `action`）/ `cascade` / 落点（父元素 + `create`）/ `DeleteSet` 收敛删除纯函数 / `WriteOutcome` 装配（`changed` / `counts` / `model` / `relations` 四计数）/ 批量四条约束；既有渲染、自证、三态、表头回填、`expect` 全部不动 |
| `.../crates/core/archi/src/edit/messages.rs` | `EditErrorKind` 19 → 23（`RelationDeleteHasFields` / `RelationDuplicateInBatch` / `CascadeCrossFile` / `RelationOwnedElsewhere`）+ `ALL` + `criterion()` 同批扩；既有 19 条判据句逐字不动 |
| `.../crates/core/archi/src/edit/warnings.rs`（新） | 七条 warning 推导（零成本事实：字段值 + `Path::exists`）+ 降噪不变式；kind 取 `ArchiDriftKind` / `ArchiApiFindingKind` 的 `as_str`、severity 取 `severity()`、路径取 `api::paths`；零 drift 调用、零写盘 |
| `.../crates/core/archi/src/lib.rs` | 出口随动（`RelationSpec` / `DeleteSet` / `WriteOutcome` / `EditWarning` / `warnings` 模块） |
| `.../crates/core/archi/src/dto.rs` | `ArchiModuleDetail.fileHash: String` + `module_detail` 投影（用 `modelFile` 定位后调 `drift::file_hash`） |
| `.../crates/infra/agent/src/sdk/mount.rs` | 两常量 `ARCHI_ELEMENT_TOOL` / `ARCHI_RELATION_TOOL` + 两份定义（平铺 schema、每字段 `description`、八行上限取自 `WRITE_LIMITS`）+ 清单 16 → 18 项（`ToolTier::Write` / `ToolExec::Archi`，插在五 archi 只读之后、`propose_config` 之前）；`Vec::with_capacity(16)` → `18` 与 `registry` / `build_registry` 两段 doc 的项数措辞同批随动（`mount_test.rs` 的结构锚按字面计数）；`ToolTier` / `ToolExec` 的 doc 名单同步；`archi_scaffold` doc 的「不在专用写工具之列」句改写为「模型写面是两个写工具，本工具只出模板」 |
| `.../crates/infra/agent/src/sdk/archi_tool.rs` | 第六、七臂：E1 解析与硬闸（`WRITE_LIMITS`）→ `plan_*` → 目标文件过 `ProtectRules` → `atomic::write_atomic` 落盘（唯一实现）→ 响应组装（三通道）；`graph_diagnostics_error` 前置 |
| `.../crates/infra/agent/src/sdk/runner.rs` | `mint_archi_context` 的名字清单判据加两名字（七名任一在场即铸） |
| `.../crates/infra/agent/src/sdk/tools.rs` | `input_path` 的 archi 名单加两名字（返回 `None`——无 `path` 入参）；两常量再导出 |
| `.../crates/infra/agent/src/compose.rs` | `archi_matrix` 两行加两工具 + archi 臂加两名字 + 与 `model_write_lock` 的对齐断言（测试侧） |
| `.../crates/infra/agent/src/sdk/protect/mod.rs` | 执行体内复用 `ProtectRules::evaluate_path` 判定目标文件（零第二份规则集） |
| `.../crates/infra/agent/src/archi-knowledge.md` | 骨架收缩到四段（2597 → ≤1500 B） |
| `.../crates/infra/agent/src/archi_knowledge.rs` | 模块头注释随动（拼装面与降级语义不变） |
| `.../crates/infra/agent/src/sdk/atomic.rs`（新） | 新增 / 迁移：`write_atomic(path, content)`（同目录临时文件 + rename，失败清临时文件）为模型写盘的**唯一实现**，壳层命令面与 agent 写工具共用；MUST NOT 出现第二份「临时文件 + rename」。落位由 `desktop-config-agent` 的 delta 登记（该能力的 Module Contract 是 `write_atomic` 落位的单份持有者）；`.archi-tmp` 后缀与错误文案逐字沿用 |
| `.../crates/infra/agent/src/lib.rs` | 出口随动：`pub use sdk::atomic::write_atomic;`（壳层经 `agent_runtime::write_atomic` 引用；`sdk` 模块本身仍私有） |
| `.../src/commands/atomic.rs` | 改薄委托（逻辑下移到 infra 侧的同一 `write_atomic`）：签名 / 可见性 / 命令层行为 / 错误文案 / 临时后缀逐字不变，函数体零 `fs::write` |
| `.../src/commands/archi/mod.rs` | `archi_set_gate_inner` 取 `outcome.write`；`archi_module` 出口带 `fileHash`（`dto` 投影已含，零新增逻辑） |
| `.../crates/core/workflow/src/prompts/proposal-planner.md` / `dev-design-planner.md` | 写面句：`archi_element` / `archi_relation`（工具表达不了的才手改）+ 保留自查句 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `bindings:export` 重导（`fileHash` 字段与 JSDoc）；**MUST NOT 手改** |
| `packages/desktop/package.json` | `version` patch +1（0.7.14 → 0.7.15） |

### 测试文件

| 文件 | 改动 |
|---|---|
| `.../crates/core/archi/src/edit/mod_test.rs` | 内联 `relations` 四情形（新建 / 更新 / 内联删边 / 未列出不动）、批量四条违例、跨文件内联边拒绝、`cascade` 单文件闭包（子树 + 出入边）与跨文件拒绝、落点四情形（既有 / 省略落父 / 单段 FQN 拒绝 / `create` 显式）、`WriteOutcome` 四计数守恒、`changed` 幂等（同值重复 upsert ⇒ `created=false` / `changed=[]` / 文本逐字节相等）、`model` 计数与 `parse_dir` 对拍；既有 12 组用例逐条保留 |
| `.../crates/core/archi/src/edit/messages_test.rs` | 23 变体的 `ALL` ↔ `criterion()` 穷尽对拍、逐值非空且互异、判据句零操作专属词 |
| `.../crates/core/archi/src/edit/warnings_test.rs`（新） | 七条逐条红绿；降噪不变式（`warnings.kind` ⊆ `changed` 诱发集合；只改 `summary` / `gate` / `metadata` ⇒ 空）；kind / severity / 路径与既有单点逐值相等；零 `drift::check` 调用（源码断言） |
| `.../crates/core/archi/src/dto_test.rs` | `fileHash` 字段面（形态 `sha256:` + 64 hex、与 `file_hash` 同值、与 `modelHash` 不可比） |
| `.../crates/infra/agent/src/sdk/mount_test.rs` | 清单 18 项与两写工具档位；两定义 schema 的八行上限与 enum 派生；`warnings` 恒在 / 响应结构上界。**必改的既有计数与结构锚**：`EXPECTED_NAMES: [&str; 16]` → `18` 且两写工具名按 `registry()` 实现序插入（五 archi 只读之后）；`registry十六项名字唯一且有序且前七恰内建` 的 `names.len()` / `unique.len()` 两处 → 18；`提案工具挂载面的源码结构锚` 的三处字面串随实现同批改（`Vec::with_capacity(16)` → `(18)`、`十六项静态挂载清单` → `十八项静态挂载清单`、`十六个字面构造项按序` → `十八个字面构造项按序`——该锚按字面计数，实现改了测试不改即红）；`ask_user_question 不在十六项静态清单` 措辞随动 |
| `.../crates/infra/agent/src/sdk/policy_test.rs` | **必改断言两处**（封闭集机械守卫——`policy.rs` 实现零改动与本题随动是同一事实的两半）：① `十六项挂载逐工具三档等价于原三张按名常量表` 的 `match name` 封闭集必须把 `"archi_element" \| "archi_relation"` 加进写档臂 `[false, true, true]`（与 `write` / `edit` / `create_explore_note` 同臂）——否则 18 项里这两名落 `other => unreachable!("挂载清单封闭集外名 {other}")` **直接 panic**；只读臂 `[true, true, true]` 不得添这两名。② `十六项等价表覆盖全表且名字两两不同` 的 `assert_eq!(names.len(), 16)` 与 `assert_eq!(unique.len(), 16)` 两处计数 → 18。三个含「十六项」的测试名（含只断言两两不同的 `挂载清单内十六项名字两两不同`，该条无论文计数、不改也绿，仅措辞随动）与文件头 doc 同批改为「十八项」 |
| `.../crates/infra/agent/src/sdk/config_propose_test.rs:147` | `常量在挂载清单末项恰一处` 的 `assert_eq!(names.len(), 16, "挂载清单十六项")` → 18（断言串「十六项」随动）；同测试「末项为提案工具」逐字不动——两写工具 MUST 插在 `propose_config` 之前，`PROPOSE_CONFIG_TOOL` 仍为清单末项 |
| `.../crates/infra/agent/src/sdk/archi_tool_test.rs` | 七臂分派、E1 值域与上限硬闸、E2 诊断前置、响应三通道与四计数、`changed` 子集、降噪、乐观锁往返（`fileHash` → `expect` → 响应 `hash`）、保护闸拒写 |
| `.../crates/infra/agent/src/compose_test.rs` | 矩阵两行 + archi 臂七名；与 `model_write_lock` 的不挂锁集合逐格相等 |
| `.../crates/infra/agent/src/sdk/runner_test.rs` / `tools_test.rs` | 铸造判据七名任一在场、`input_path` 两名字返回 `None` |
| `.../crates/infra/agent/src/sdk/protect/mod_test.rs` | 目标文件命中用户规则 ⇒ 拒；未命中 ⇒ 放行 |
| `.../crates/infra/agent/src/archi_knowledge_test.rs` | 四段在场 + 手改出口一句 + 零键形示例 / 零值域行 / 零示例 TOML 围栏 + 零结构诊断词 + 新门限冻结值（骨架 / 全文）；A 档三列表收窄 |
| `.../src/commands/archi/mod_test.rs` | `archi_set_gate` 改走 `outcome.write` 后行为等价；`archi_module` 响应含 `fileHash` |
| `.../src/bindings/mod_test.rs:2016-2028` | `ArchiModuleDetail` **七字段序断言必红**：`field_names(&detail) == ["element", "dependsOn", "dependedBy", "children", "doc", "api", "resolvedEntry"]` 在增 `fileHash` 后即失败；改为八字段，`fileHash` 追加表尾（`resolvedEntry` 之后，与 P3「新键追加表尾」同向；若 dev-design 改声明位则以 `dto.rs` 声明序为准）。这是「生成物字段面按声明序出线」的机械守卫，MUST NOT 以「删断言换绿」收场 |
| `.../crates/infra/agent/src/sdk/atomic_test.rs`（新） | 既有原子写用例迁入实现所在侧（内容逐条保留）：逐字写 / 整体替换 / 空与超长 / 无父目录文案 / 临时名不撞目标 / 父目录缺失 / 只读目录 / 结构锚（`fs::write(&temporary` 与 `fs::rename(&temporary, path)` 各恰一处、零 `create_dir_all`、含 `.archi-tmp`） |
| `.../src/commands/atomic_test.rs` | 收窄为薄委托断言：命令层零 `fs::write` / 零 `fs::rename` / 零 `create_dir_all`，`write_atomic` 委托 infra 实现恰一处；签名与错误串原样透传 |
| `.../crates/core/workflow/src/write/phase_table_test.rs` | 两个 planner md 的「无专用写工具」断言改为两写工具 + 手改出口（MUST NOT 留恒真空断言） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.../crates/core/archi/src/parse/**`、`model.rs`、`graph.rs`、`drift/**`、`api/**` | 判据面 / 投影面 / 哈希面零变化：写工具 MUST NOT 重实现 `path` / `apis` 形态校验与 `kind × path` 基数校验（parser 是唯一判据），MUST NOT 跑 `drift::check` |
| `.dev-team/architecture/**`（模型、生成物、ADR） | 零数据变化（55 / 129 / 表头计数逐字不变）；`api:export` 与 `api:check` 应零 diff |
| `openspec/specs/**` 基线、本 change 的 `explore.md` | delta-only：手改基线即「同一事实两份持有者」；explore 只读。基线内旧立场句（`desktop-archi-agent` 的 RENAMED 标题与正文、`desktop-archi-chat` / `desktop-phase-prompts` / `desktop-write-protection` 的引用）与 `write_atomic` 的旧落位登记，均由本 change 的 delta（RENAMED / MODIFIED / Module Contract）追改、随归档合并生效——**基线保持原样是本 change 的判据**（AC-12 豁免面 / AC-17 的 `FROM:` 逐字要求），不是遗漏 |
| `src-tauri/Cargo.toml` / `tauri.conf.json` | crate 版本行 MUST NOT 跟随 `package.json`（AGENT.md 版本纪律） |
| `AGENT.md` / `CLAUDE.md` | 规范面，本变更不触（其「模块 API 契约」节不含写工具表述） |
| `.../crates/core/config/**`、`.../crates/infra/{store,exec,vcs,watch}/**` | 与写面无关 |
| 前端产品代码（`packages/desktop/src/**`，除生成物 `types/generated/`） | 零 UI 变化：`archi_set_gate` 签名与画布交互逐字不变 |
| `.../crates/infra/agent/src/sdk/policy.rs` | 准入判据仍是 tier 比较单点；本变更不动 `allows` 与 `ToolTier` 比较本身——两写工具只是**以 `Write` 档进清单**，判定逻辑零行改动；同族机械守卫的随动在 `policy_test.rs`（见测试文件表），MUST NOT 因测试红而改实现 |
| `.../src/bindings/mod_test.rs` 除 `ArchiModuleDetail` 字段序断言一行外的其余断言语料 | `ArchiGraph` 三字段序、`ArchiResolvedEntry` 三字段序、`ArchiElement` 字段序、退役类型零声明、`ArchiElementBrief` 零出线等逐字不动——本变更的生成物增面只有 `fileHash` 一个字段，其余断言随 `bindings:export` 全绿 |

---

## 验收标准

1. **AC-1 · 工具面在册且档位正确**：挂载清单恰 **18** 项（七内建 + `create_explore_note` + 五 archi 只读 + 两 archi 写 + 两 DB + `propose_config`）；两写工具 `ToolTier::Write` / `ToolExec::Archi`，`default` 档拒绝、`acceptEdits` / `bypassPermissions` 放行；五只读工具档位逐字不变。
2. **AC-2 · 矩阵与写权锁同集**：矩阵下发两写工具的 `(phase, role)` 集合 == `model_write_lock` 判定为 `None`（不挂锁）的集合，逐格相等（新增断言）；archi 臂七名；未命中相位零下发且调用被成员校验拒绝。
3. **AC-3 · 落点四情形**：既有元素落其文件（`file` 给不同值 ⇒ `Err`）；新建省略 `file` 落父元素文件；单段 FQN 省略 `file` ⇒ `Err` 点名需给 `file`；`file` 不存在且无 `create` ⇒ `Err` 提示 `create = true`，给了 `create` ⇒ 新文件带 `# <名>（1 元素 / 0 关系）` 表头。
4. **AC-4 · 内联出边四条约束**：同批重复 `to` ⇒ `Err`（1 基下标 + `action` + `to` 同行）；`delete` 项带 `desc` / `apis` ⇒ `Err`；内联边已声明在别的模型文件 ⇒ `Err` 点名该文件；未列出的既有边零改动；四计数守恒（`created + updated + deleted + unchanged == 入参长度`）。
5. **AC-5 · 删除与级联**：`cascade = false` 时 P3 的拒绝臂逐字保留（有子 / 被引用）；`cascade = true` 时同文件闭包删净（子树 + 出入边 + 表头计数随动），闭包跨文件 ⇒ `Err` 点名文件且零落盘；删除纯函数恰一个（`grep` 对「第二个只删一个元素的删除实现」零命中）。
6. **AC-6 · 反馈三通道**：`warnings` 恒在场（空数组 = 无异常）；`changed` 空 = 幂等（盘上字节不变）、非空 ⊆ 参数面字段名；`Err` 满足 §五 的「第一行自足」检查（含 `重试：` / `下一步：` / `改用 <工具名>` 之一或可复制 JSON 片段）。
7. **AC-7 · 七条 warning 与降噪**：七条逐条红绿；`warnings[].kind` ∈ { `ArchiDriftKind` 五值 ∪ `ArchiApiFindingKind` 十三值 } 的 `as_str`；severity 与 `ArchiApiFindingKind::severity()` 逐值一致、drift 两值恒 `warn`；降噪不变式成立（只改 `summary` / `gate` / `metadata` ⇒ `warnings` 空；`delete` 仅允许孤儿契约 md 一条）；源码零 `drift::check` 引用。
8. **AC-8 · 响应体量与计数**：成功响应的 `counts` 与写后该文件重解析计数相等、与表头括号数字相等；`model` 与写后 `archi_drift` 报告的 `model.elements` / `model.relations` 逐值相等；成功响应序列化长度 < `MAX_RESULT_BYTES`（结构上界）。
9. **AC-9 · 读侧闭环**：`archi_module` 单元素响应的 `fileHash` 与 `drift::file_hash(该元素声明文件)` 逐字相等；`archi_element { expect: fileHash }` 成功；改动该文件后旧 `expect` ⇒ `Err` + 零落盘；响应 `hash` 可直接作下一次 `expect`（连续两次写同一文件零额外读）。
10. **AC-10 · 入参面**：两工具 schema 含八行上限与全部 `enum`（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` 派生）；限额常量单点（schema 键值与执行体检查取自同一组 const，`grep` 无第二份数字）；执行体 E1 对 `action` / `element` / 值域 / 上限逐项拒绝。
11. **AC-11 · 知识收缩**：骨架 ≤ 1500 B、全文 ≤ 1700 B 且实测值冻结；四段在场（职责 / 工具面 / 留给判断 / 手改出口一句）；零示例 TOML 围栏、零键形示例、零字段值域行、零结构诊断词（含 `orphanHierarchy` / `dependsOnAncestor`）；两处派生目录仍在行程。
12. **AC-12 · 旧立场零残留（判据面 = 基线之外）**：判据只在三面成立，基线不在其内。
   - **（a）产品源码面**：`packages/desktop/src-tauri/**` 的 `.rs`（含注释、doc 与测试夹具）对「无专用写工具」/「不设专用写工具」/「MUST NOT 存在专用写」字面零命中——`mount.rs` 的 scaffold doc、`phase_table_test.rs` 的断言串同批改写。
   - **（b）prompts 面**：两份 phase prompt md（`crates/core/workflow/src/prompts/proposal-planner.md` / `dev-design-planner.md`）零命中，且均含两写工具名 + 「工具表达不了的才手改」+ 自查句。
   - **（c）delta 结构面**：五处立场锚点逐一在 `specs/**` 中有承接块——两处 RENAMED 标题、MODIFIED 正文删去「MUST NOT 存在专用『写模型』工具」并把「写入不校验，读取报错」拆成两半、Scenario 由「无专用写工具（五只读）」换成「两入口并存且无第三条通路」、`desktop-phase-prompts` 的写权指示与 Scenario 断言随动——且承接块的新文本（`TO:` 标题与 MODIFIED / ADDED 正文）不再把这两句写成肯定命题；「写入不校验，读取报错」只在 `desktop-archi-agent` 与 `desktop-archi-chat` 的 Purpose 现状引述里出现，正文均为拆两半表述；`desktop-archi-agent` 的矩阵与分层两条含「写工具 canonical + 手改 second path」表述。
   - **基线豁免（本条与 AC-17 的相容点）**：`openspec/specs/**` MUST 零改动（delta-only）。基线内仍存在的旧立场句（`desktop-archi-agent` 的两处 RENAMED 标题与正文、`desktop-archi-chat` / `desktop-phase-prompts` / `desktop-write-protection` 的引用）由本 change 的 delta 在归档合并时替换，**不属于本 change 的判据面**；AC-17 的「`FROM:` 与基线标题逐字一致」正依赖它们保持原样。
   - **基线外豁免**：`openspec/changes/**`（本 change 的 `explore.md`、Purpose 的现状引述、`FROM:` 行、禁止句、归档历史）不在字面判据面。
13. **AC-13 · 两条通路等价与画布不变**：`archi_set_gate` 与 `archi_element(gate)` 在同一临时模型上产**逐字节相同**文本（同实现）；`archi_set_gate` 签名、`Result<(), String>` 错误面、失败零写入逐条不变；模型写盘走 `write_atomic` 且它恰**一份实现**（infra 侧 `sdk/atomic.rs`；壳层 `commands/atomic.rs` 为薄委托；模型与配置两写面共用同一函数，`grep` 对第二份「临时文件 + rename」零命中），错误文案与 `.archi-tmp` 临时后缀逐字不变。
14. **AC-14 · 保护闸射程**：两写工具的目标文件命中用户 `write_protection.files` 规则时被拒（拒串沿用既有族，`%s` / `%t` 替换正确）；未命中时放行；`ProtectRules` 判定面零第二份规则集。
15. **AC-15 · 生成物与读数**：`bindings:export` 后 `bindings.ts` 与 `bindings.zod.ts` 的 diff **均恰为** `ArchiModuleDetail` 增 `fileHash` 一字段（`bindings.ts` 含 JSDoc；`bindings.zod.ts` 的 `ArchiModuleDetailSchema` 同字段），无其它 diff；`api:check` 零差异；`.dev-team/architecture/**` 零字节变化（55 / 129 / `modelHash` / 六个表头计数逐字不变）；`apiFindings` 计数与构成不因本变更变化（落地时实测为准，当前 38 条全 `docMissing`）。
16. **AC-16 · 版本与改动集**：`package.json` `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/Cargo.toml` 逐字不变；`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（另加本 change 目录下的 `proposal.md` / `specs/**` / `reports/**`；`openspec/specs/**` 基线零改动）。
17. **AC-17 · spec delta 形态**：`openspec/changes/archi-agent-contract-knowledge-gap/specs/` 恰**七个**能力目录（`desktop-archi-agent` / `desktop-archi-model` / `desktop-agent-execution` / `desktop-phase-prompts` / `desktop-archi-chat` / `desktop-write-protection` / `desktop-config-agent`）；RENAMED 块的 `FROM:` 与基线标题逐字一致、`TO:` 与随后的 MODIFIED 块标题逐字一致；MODIFIED 块为基线整段拷贝后改（与基线 diff 只含本提案声明的差异，如 `desktop-config-agent` 只多「落盘单点与实现落位」一条、改「写权边界」的括注、加一条 Scenario），未丢任何未冲突条目；每条 `### Requirement` 至少一个 `#### Scenario:`（恰 4 个 `#`）且 WHEN/THEN 成对；各 delta 含 `## Module Contract` 段。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **体量与评审面**：20 个实现文件 + 14 个测试文件 + 7 份 spec delta，一次做完（D1） | 体量本身就是「不拆两步」的理由——半个写面（工具有了、知识没删）比没有更难评审。评审按三条主线读：写工具契约 ↔ 知识迁移表 ↔ 五处立场改写；引擎增量（级联 / 内联 / 落点）只三块且都长在 P3 已有函数上 |
| R-2 | **双写面长期并存（L3a 终态）**：写工具与 `write` / `edit` 都会改模型，两条通路的失败语义不同 | 骨架永久保留「手改出口」一句 + `desktop-archi-agent` 拆两半表述；观察期看两件事（`archi_drift` 结构诊断是否稳定归零 / 手改是否被用于工具能做的事），不看就不动 L3b |
| R-3 | **内联边跨文件拒绝**可能让 agent 困惑（「我明明给了这条边」） | `Err` 文案点名目标文件与 `archi_relation`，且工具描述写明「内联边只在目标文件内生效」；MUST NOT 静默只改同文件那份 |
| R-4 | **`cascade` 的字面承诺大于实现**（只做单文件） | spec 与工具描述三处都写「同一模型文件内」，不写「子树深度优先」；跨文件 `Err` 点名文件 |
| R-5 | **schema 上限是软提示**：模型可能给超长值，响应的回显随之膨胀 | 成功响应结构上界 ≈ 2 KB，`Err` 走 `cap_result` 截断留痕（首行保留）；E1 硬闸对值域与上限显式拒绝，两者都只在一处 |
| R-6 | **`warnings` 位次不设断言**：若将来有人以「`warnings` 第一个字段」写测试，会与 `serde_json` 的缺省序冲突 | 提案与 spec 都写明「成功响应永不触达截断 ⇒ 位次是防御不是机制」，只断言 `warnings` 在场与响应体量上界 |
| R-7 | **工具名 `archi_relation` 与只读 IPC 命令 `archi_relation` 同名** | 两个命名空间（agent 工具 vs Tauri 命令）功能不冲突，但文档须处处限定「工具 / 命令」。若评审认为易混，回落方案是改名 `archi_edge`（改动面 = 两常量 + 定义 + spec 两处） |
| R-8 | **删除后的孤儿契约 md 只有动作面 warning**（D10-B 不做 drift 判据） | 已知并接受：`apiUnexpected` 只在 delete 成功时回一条；不做 drift 判据的理由（依赖「`models/` 下除契约 md 别无他物」的假设、当前零持仓）写在 spec；MUST NOT 列入 backlog 承诺 |
| R-9 | **保护闸射程（§十一）是本提案新增面**，挂点选择有实现风险（执行体内拿到 `ProtectRules` 需把规则集带进 `ArchiContext`） | 只要求「过同一 `ProtectRules`、零第二份文案」，挂点留给 dev-design；若实现代价过高，回落路径是把该条降为「已知限制」并如实留痕（MUST NOT 静默失效） |
| R-10 | **知识压缩过度**：骨架降到 1500 B 以下可能丢掉「只改架构目录 / 渲染后不自动刷新 / 续接先取模型」这类职责句 | 门限只写上限，四段内容清单在 spec 逐条在案；语料测试对四段与手改出口逐句断言，压缩时删的是语法段不是职责段 |
| R-11 | **`expect` 是可选逃生阀**（省略即直接覆盖） | 冲突文案给两条出路（先取现状重试 / 省略覆盖）；`fileHash` 让「我有没有基线」不再靠推理；MUST NOT 把 `expect` 变成必填（首次建元素本就没有基线） |
| R-12 | **`fileHash` 与 `modelHash` 同名不同义的历史包袱**（改名只解决新字段，旧字段仍在） | `drift` 的 `modelHash` 语义逐字不变；两处 doc 与 spec 都明写「只可与自身前后比较，与另一者不可比」；AC-9 只断言与 `file_hash` 同值 |
| R-13 | **`EditErrorKind` 19 → 23**：新增四变体若未同批补 `ALL` / `criterion()` 即测试红 | `ALL` 与 `criterion()` 的穷尽 `match` 是既有机械锚（P3 交付），新增变体在编译期即暴露 |
| R-14 | **`desktop-agent-execution` 的大段拷贝**：MODIFIED 需整段复制 90 行 requirement，手抄易漏 | 落地时以基线文件直接复制后定点改（改动点仅三处：archi 臂名字清单、铸造判据名单、tier 条与 archi 臂 Scenario） |
| R-15 | **`write_atomic` 落位迁移**（壳层 `commands/atomic.rs` → infra 侧同一实现）牵动两个既有消费者：画布改档与 `workspace_config_apply`，并触及 `desktop-config-agent` 基线里的落位登记（`spec.md` 的 Module Contract `Component` 行） | 迁移只搬「临时文件 + rename」与错误文案，两消费者的调用点、签名与断言不动；登记同批由 `desktop-config-agent` 的 delta 追改（该基线是 `write_atomic` 落位的单份持有者，漏改即归档后基线指向已迁走的文件）；原子写用例迁到 infra 侧同一实现、命令层只留薄委托断言；MUST NOT 借迁移改临时后缀 `.archi-tmp`、错误串或原子性语义（失败清临时文件） |

---

## explore 未决项与「不含」的归属

| explore 条目 | 处置 |
|---|---|
| D6 元素在文件内的插入位置 | **不含**：另立 `toml-sorting-and-layout`（explore 已明确后置）；本变更不改 P3 的保序裁决 |
| D3 L3b（模型目录进写保护锁面） | **不做**：观察期持续运行，L3a 为终态 |
| D4 `config.json` 写面 | **不做**：手改是正式组成部分 |
| D10-B 孤儿契约 md 进 drift 判据 | **不做**（且不列 backlog）：只做动作面 warning |
| D11 契约债清账（P4） | **不做**：债由各 change 逐块偿还 |
| D8 截断优先级「`warnings` 最高 + 入参上限降概率」 | **采一半**：入参上限进 schema + E1 硬闸；`warnings` 位次不作机制（偏离⑤） |
| §11.6 可选的 A1 / A2 二分 | **不做**：D1 已裁「一次做完」；若评审受阻，A2（delete / cascade）是唯一不破坏原子性的切法 |
| §7.4「同批文档面」列的 `desktop-archi-drift` / `repo-archi-model` spec / ADR | **零 delta**：它们是**措辞依据**（`noPath` 分档、`path` 基数、FQN×kind×path 表），不是行为变化面；本变更照其口径措辞，不改其条款 |
| 骨架目标 1.25 KB 与「估 1500 B 门限」 | **照收**：门限取 1500 / 1700，实测值冻结 |
