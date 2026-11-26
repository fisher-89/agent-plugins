# desktop-archi-agent Specification（delta: toml-sorting-and-layout）

## Purpose

写面落地规范化后，工具面有三处必须同批对齐——它们都是**同一事实的转述面**，不是新机制：

- **手改出口的举例校正（两处）**：现有「工具表达不了的（**重排文件** / 批量整理）才手改」与写面真相已不相容——子树连续与关系组连续由写面自动维持，手改的正当余量是**同层手排序与批量整理**；同时如实写明「手改拆开的子树会在下一次工具写时被合回」。
- **工具描述面三件事 → 四件事**：写入会把目标文件归一到规范序，这一语义属工具面（无字节预算限制），落进两写工具的 `description`。
- **响应体加 `reordered`**：`{ moved, blocks }`——搬迁块的可读回报，两写工具同形（元素面与关系面各一处装配）；`archi_relation` 的薄形键集因此 8 → 9。语义、cap 与「`changed` 空而 `moved > 0`」的正当性归 `desktop-archi-model`「模型写回原语族」，本能力只序列化。
- **零改动面**：知识骨架（`archi-knowledge.md`）的字节与 `1800 / 2000` 双门限、工具清单与档位归属、注入矩阵、脚本侧 `mount.rs` 的 `parameters` / `WRITE_LIMITS` 全部逐字不变——骨架现值 1777 B（净余 23 B），不花也不抬锚。

## MODIFIED Requirements

### Requirement: 模型 = 源码（两条写入口与写纪律）

架构模型文件 SHALL 与源码同权：agent SHALL 直改 `<archi_dir>/graph/model/*.toml`。**写模型有两个入口，MUST NOT 存在第三个**：

| 入口 | 形态 | 失败语义 | 定位 |
|---|---|---|---|
| 专用写工具 `archi_element` / `archi_relation` | 结构化参数面（`desktop-archi-agent`「archi 写工具面」） | **坏文本不可产出**：渲染 → 重解析自证 → 回读一致，任一失败即 `Err` 且零落盘 | **canonical**：键形 / 值域 / 祖先 / 计数 / 契约告警全在工具里 |
| `write` / `edit` 手改 | 自由文本 | **写坏模型仍是合法状态**：由读取面报错（结构诊断 `file:line`、画布停摆、`archi_preclose` 兜底） | **second path**：工具表达不了的（批量整理 / 同层手排序）才用，改完必须自查 |

两条与文件序相关的边界 SHALL 与本表同批成立（本变更新增）：

- **子树连续与关系组连续由写面维持**：专用写工具的产出恒满足「父先于子 / 子树连续 / 同一 `from` 组连续」（见 `desktop-archi-model`「写入序不变量」）；手改拆开的子树 MUST NOT 被承诺长期保留——下一次工具写会把它合回原位（同层兄弟的手排相对序则被保留）。
- **手改的正当余量 = 批量整理与同层手排序**：MUST NOT 把手改的适用边界再表述为「重排文件」——整份文件的规范化已由每次工具写承担。

写权（谁能写）与写入口（怎么写）是两件事，MUST NOT 混说：

- **写权 SHALL 有且只有两条通路**，且**通路 A 按相位收窄**：

| 通路 | 触发面 | 执行根 | 确认时机 | 归属能力 |
|---|---|---|---|---|
| A（既有，按相位收窄） | change 流程相位会话——**仅 `phase ∈ {proposal, dev-design}` ∧ `role = executor`** | `detail.worktree` | 回合末归档确认面（模型区块 + 冲突分流） | `desktop-change-archive` |
| B（既有） | 架构页通用会话（`source = "archi"`），不限相位 | workspace root（主仓，`cwd = null` 回落） | 事后 `git diff` 评审 + 用户手动刷新校验 | `desktop-archi-chat` |

- **写权是强制面问题，不是工具面问题**：`archi` crate 全程只读盘（模型唯一落盘点是 `write_atomic` 单点），故「写入口」才分工具 / 手改两种；写权政策实做是**掐掉其余相位对模型目录的射程**——由会话级写入禁区（第三规则来源）承载，见 `desktop-write-protection`「会话级写入禁区（相位锁）」。两写工具只在写权放开的相位下发（矩阵与写权锁同集，见「archi 工具面注入与 (phase, role) 矩阵」）；MUST NOT 出现「有工具但无写权」或「有写权但无工具」的错配。
- **两条失败语义 MUST NOT 混为一句**：写工具面 MUST NOT 产出坏文本（自证链使它结构上不可达）；手改面 MUST NOT 因加工具而收紧——**写坏模型仍由读取面报错**（`archi_module` 的结构诊断带 `file:line`、画布停摆态、`archi_preclose` 读数）。MUST NOT 把「写盘前校验」当写权护栏（`write` / `edit` 对**接口定义文件之外**的一切路径零预校验是既有承诺）。
- **两条推论（政策是绝对命题，不是步骤枚举）**：
  - **`dev-design/evaluator` 被锁**：白名单按 `(phase, role)` 而非 phase——评审批次不写模型。
  - **`explore` / `debug` 也被锁**：两者不是「步骤」，但政策覆盖一切来源；explore 的 `edit` 只服务笔记，锁模型零损失。
- **不依赖运行时审批**：写盘那一刻 MUST NOT 要求用户确认（工具分派链只有 `deny`，无挂起等确认能力）。通路 A 的设计权由「回合必须人工确认」承载；通路 B 的设计权由事后 diff 评审 + 手动刷新可见性承载（MUST NOT 承诺写盘前拦截）。
- **内置锁面仍不纳入**：`<archi_dir>/graph/model/` 与架构域配置 MUST NOT 进入 `write_protection` 的**内置**锁面（内置规则是相位无关的绝对路径 glob，纳入即等于两条通路都改不了模型）；相位条件的锁 SHALL 走**会话级**规则面（见 `desktop-write-protection`「会话级写入禁区（相位锁）」）。
- **两通路并存，互不替代**：MUST NOT 因通路 B 存在而移除通路 A（change 侧 worktree 隔离 + 归档确认仍是「改动随 change 一起评审」的唯一形态）；亦 MUST NOT 因通路 A 收窄而收紧通路 B（架构页会话的用途恰是「不进 change 也能维护模型」）。
- **场景归属**：冷启动生成骨架 / 增量维护（新增或重构模块）/ 按现状补声明 / 直接修复图或文档错误，四类场景 SHALL 可走任一条通路（走 change 时落 `dev-design` 阶段，走架构页时落通用会话），MUST NOT 强制单一入口。非白名单相位遇到「模型该改」的需求 SHALL 把需求写进产物（design / tasks）交 `dev-design` 落地，MUST NOT 尝试绕过。
- **新元素缺省档**：两入口生成的新元素均缺省 `gate = "off"`（ratchet 逐块收紧）。
- **通路 B 的边界（软约束，非写入面强制）**：会话知识 SHALL 要求「禁止修改架构目录之外的文件（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）」，但写入面 MUST NOT 为该约束设硬隔离——该缺口 SHALL 在能力 spec 中如实留痕，MUST NOT 被表述为「会话只能改模型」。
- **生成物同权但不可手改（本变更由四类改三类）**：`.schema.json` **不再是生成物**（它是手写真相，走 `write` / `edit`），生成物只剩 `INDEX.toml` / `.i.rs` / `.i.ts`。三类生成物与模型文件同属磁盘可写面，但**没有专用写工具**——两写工具的 `file` 只接受模型声明目录一级 `*.toml`，射程 MUST NOT 扩到生成物；会话知识 SHALL 要求「MUST NOT 手改生成物，用既有命令重生成」——该约束同样只有事后兜底（零 diff 门 + 契约面板），MUST NOT 表述为写入面拦截；手改通路对生成物仍零写前校验。
- **手写接口面有一处显式写前校验（本变更新增的唯一例外）**：`write` / `edit` 落 `<archi_models_dir>/**/<FQN>.schema.json` 时 SHALL 先过接口校验（坏文本 `Err` 零落盘，见 `desktop-agent-execution`「`write` / `edit` 的接口定义写闸」）。该例外 MUST NOT 被读成写权护栏或保护面：它只判「文件级自洽」，与谁能写无关；手改通路对**其余一切文件**（含 `INDEX.toml` / `.i.*` / 模型 TOML / 普通源码）仍零预校验。

#### Scenario: worktree 内直改模型成功（白名单相位）

- **WHEN** agent 在 `dev-design/executor` 会话中用 `write` 新建 `<archi_dir>/graph/model/03-billing.toml`，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后读取面解析出该改动，主仓模型零变化

#### Scenario: 白名单外相位写模型被拒

- **WHEN** `implement/executor` / `code-review/evaluator` / `dev-design/evaluator` 会话对 `<archi_dir>/graph/model/*.toml` 调用 `write` 或 `edit`
- **THEN** 拒绝串含路径、工具名、指路（写进 design / tasks 交 `dev-design` 落地，或改用架构页会话）与「不要改用其他命令或工具尝试绕过」；run 不中断、文件零变化

#### Scenario: 主仓直改模型成功（架构页）

- **WHEN** agent 在架构页通用会话（`cwd = null` → 主仓根）内用 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`
- **THEN** 写入落在主仓该文件（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后同 workspace 的 `archi_module` 与画布取数读到该改动，改动保持未提交由用户 `git diff` 评审

#### Scenario: 手写接口面是唯一预校验面

- **WHEN** 分别以 `write` 落一份坏 `.schema.json`、一份坏 `INDEX.toml`、一份坏模型 `*.toml`
- **THEN** 第一者 `Err` 零落盘（写闸）；后两者原样落盘、由读取面 / 零 diff 门报错（写前零校验不变）

#### Scenario: 生成物只三类

- **WHEN** 审查写纪律文本与生成物清单
- **THEN** 只点名 `INDEX.toml` / `.i.rs` / `.i.ts` 三类；`.schema.json` 被表述为**手写真相**（走 `write` / `edit` + 写闸），MUST NOT 与三类并列

#### Scenario: 写坏模型不阻断写盘（手改面）

- **WHEN** agent 用 `write` / `edit`（不是写工具）写出语法错的模型文件（白名单相位或通路 B）
- **THEN** 写盘成功；随后 `archi_module` / 画布数据返回结构诊断（`is_error` / 停摆态）且进程不 panic；收口时 `archi_preclose` 读数含该结构错

#### Scenario: 写工具面不可产出坏文本

- **WHEN** 以任何合法入参调用 `archi_element` / `archi_relation`，并构造一个会渲染出诊断的边界（如 `apis = [""]` / 非法 `path` 形态）
- **THEN** 一律 `Err` 且**零落盘**（自证链在交文本之前拦下），盘上字节零变化；该失败语义不得被表述为「与手改面相同」

#### Scenario: 两入口并存且无第三条通路

- **WHEN** 审查执行面工具清单与写模型入口
- **THEN** 写模型恰有两个入口（专用写工具 canonical / `write`+`edit` second path），工具面在写权相位下发两个写工具且在非写权相位零下发；MUST NOT 存在第三条通路（如独立 IPC 写命令或画布写入口，含任何 `archi_sort` 一类手动排序入口）；`archi_set_gate` 与写工具共用同一 `plan_element_upsert`（同实现 ⇒ 同文本）

#### Scenario: 手改与写面对文件序的分工可辨

- **WHEN** 审查本 requirement 的手改出口表格行与两条「文件序边界」（骨架 md 侧只保留简写「工具表达不了的才手改」）；随后用 `edit` 把某元素的子表搬到其父之前，再调一次 `archi_element` 改该元素的 `summary`
- **THEN** 本条把边界表述为「批量整理 / 同层手排序」并写明子树连续与关系组连续由写面维持（「重排文件」不在其中）；子先于父不产生结构诊断（`orphanHierarchy` 判「父是否声明」，不判顺序）；`archi_element` 的响应 `reordered.moved ≥ 1`，块回到父的子树区间内

### Requirement: 架构会话的系统知识注入

`source = "archi"` 会话与 **change 臂的两写权相位** SHALL 注入同一段架构会话知识到 `SessionInjections.preamble`，内容 SHALL 覆盖以下**五段**（本变更是骨架的第三次改写：三资产分工与设计顺序取代语法类知识，散文只留不可脚本化的判断面）：

- **三资产分工（C）**：模型 TOML 答「谁在谁的里面、谁依赖谁」（拓扑）；`<FQN>.schema.json`（与文档同目录）答「跨过这条边时接触面长什么样」（接口，**唯一凭据**）；`<FQN>.md`（同目录）答「为什么这样切、有什么坑、什么不归它管」（理由）。**过度设计 = 回答自己不该回答的那个问题**：文档不列接口、schema 不写理由、TOML 不造空元素。**文档是产品面**——架构页「文档」段直接渲染给用户读，人 + AI 共读；接口由写闸校验、人经 UI 阅读。
- **设计顺序（B/C）**：① 定位现状（`archi_search` 发现 → `archi_module` 一次取全邻居 / 三份资产 / 声明位置）；② 画拓扑（`archi_element` / `archi_relation`）——**先边界、后接触面、最后写理由**；③ 补两份资产（`archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 落 `.schema.json`（坏文本零落盘）→ 理由落 `.md`，段落按内容自定）。
- **三条不可越界（C）**：① **文档不列接口**——导出名 / 签名 / 类型的唯一持有者是 schema；历史 `api-*` fence 是遗留内容（读面不读、清理中），MUST NOT 新增；② **schema 不写理由**——`x-function.description` 只写调用方要知道的语义（做什么 / 边界 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common` 三项，MUST NOT 自造新前缀键；③ **TOML 不造空元素**——每个元素至少一条接触面（`path` / 边 / 导出）。
- **工具面（A/B，指路）**：字段面、键形、缺省、值域、祖先与计数全部由工具承担——MUST NOT 凭记忆手拼 TOML。读五件各一句「何时用」（`archi_search` 发现 / `archi_module` 定位与三资产 / `archi_drift` 漂移读数 / `archi_api` 接口三动作 / `archi_scaffold` 字段词典）；写两件（`archi_element` / `archi_relation`，参数面即字段清单、坏文本零落盘）；**手改出口（永久内容）**：工具表达不了的才手改 TOML（骨架保持这一简写；**适用边界由本能力裁定** = 批量整理 / 同层手排序——子树连续与关系组连续由写面自动维持，手改拆开的子树会在下一次工具写时被合回，故「重排文件」不在边界内），改完必须 `archi_module` 自查——坏模型由读取面报错（结构诊断带 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改。自检纪律：改完 `archi_drift` 非空即逐条修、`archi_api`（`check`）查两轴。契约资产 `config.json` 管 `ignore` / `exclude`，非法由 `archi_drift` 点名。
- **留给判断 + 兼容（C）**：`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；**纯分组容器用 `composite`**；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），不要编造导出**；旧元素可能仍把接口写在 `.md` 的 `api-*` fence 里——读面不读，接口一律写 `.schema.json`，MUST NOT 往 `.md` 新增 fence。

两条工程约束与下列「值域不进知识文本」纪律逐字沿用：

- `gate` / `api` / `kind` 的值域与缺省档 SHALL 由工具面承载（参数 `description` 与 `archi_scaffold` 词典），骨架 MUST NOT 再写值域行；键形与字段形态 MUST NOT 进骨架；结构诊断全量词表 MUST NOT 进知识文本（`orphanHierarchy` / `dependsOnAncestor` 亦不在骨架，祖先缺失由写工具 `Err`、越界依赖由 `archi_drift` 承载）。**文件序不进骨架**（本变更）：子树连续与关系组连续是写面默认行为，骨架既不解释也不要求 agent 记得——骨架的手改出口句保持简写（「工具表达不了的才手改」），适用边界的裁定（批量整理 / 同层手排序）与「拆开会在下次写时合回」留在本节与工具描述，**不进骨架**。
- `entry` / `doc` / `apis` MUST NOT 进任何值域词表（开放集）；判据长表 MUST NOT 复制进知识文本。
- **体量锚（本变更重定）**：骨架 SHALL ≤ **1800 B**、全文 SHALL ≤ **2000 B**，实测值 SHALL 被冻结断言（测量口径沿用 `lf_bytes(trim_end)`）。门限由 1500 / 1700 上抬到本组实测值——新增的是不可脚本化的语义内容（三资产分工 / 设计顺序 / 三条不可越界 / 兼容句，约 350 B），**MUST NOT** 借此恢复语法教学（值域行 / 字段形态 / 诊断词表三类仍在禁令内）；超限 SHALL 压缩同段冗余表述，MUST NOT 删除三资产分工 / 设计顺序 / 手改出口 / 留给判断四段任一条。**本变更零字节改动**：1777 B / 1960 B 两个冻结值与两道上限逐字不变（净余 23 B 不花）。
- **尽力增强、不炸 run**：知识拼装失败 SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 五段结构在骨架内可指认

- **WHEN** 审查骨架 md 的分段
- **THEN** 恰含五段语义：三资产分工（含「文档是产品面」）、设计顺序（三步 + 「先边界、后接触面、最后理由」）、三条不可越界（文档不列接口 / schema 不写理由 / TOML 不造空元素）、工具面（读五件的「何时用」+ 写两件的「参数面即字段清单」+ 手改出口一句 + 自检纪律）、留给判断 + 兼容；MUST NOT 出现语法教学段

#### Scenario: 语法知识零残留

- **WHEN** 对骨架做机械检查
- **THEN** 零示例 TOML 围栏（` ``` ` 计数为 0）、零 `[element.` 前缀、零字段值域行（`off` / `warn` / `strict` / `declared` / `none` 不同时作为值域枚举出现）、零结构诊断词（`orphanHierarchy` / `dependsOnAncestor` / `invalidValue` / `unknownField` / `missingField` / `syntaxError` 全零命中）、零 `entry` 三档长表

#### Scenario: 三条不可越界逐条在场

- **WHEN** 对骨架做关键词断言
- **THEN** 「文档不列接口」/「MUST NOT 复述签名」/「TOML 不造空元素」三条各至少一处；`x-` 面封闭三项（`x-function` / `x-class` / `x-common`）在场；「空 schema」与「不要编造导出」在场；「历史 fence 读面不读、MUST NOT 新增」在场

#### Scenario: 手改出口是永久内容

- **WHEN** 审查骨架「工具面」段的最后一句
- **THEN** 该句含三要素：①「工具表达不了的才手改」的适用边界；②「改完必须自查」（点名 `archi_module`）；③ 坏模型由读取面报错（带 `file:line`）；MUST NOT 被表述为过渡措施或待删项；且句内不含「重排」一类已被写面接管的动作（骨架的字节与门限断言不受本变更影响）

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
- **THEN** 骨架 ≤ 1800 B、全文 ≤ 2000 B 且实测值（1777 B / 1960 B）被冻结断言；程序追加段含模型声明目录与契约资产目录（文档 + 接口两条同目录资产）两处方位，零契约索引串，骨架零目录字面量

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
- **写面语义全在 `desktop-archi-model`**：三态（省略 = 不动 / `null` = 清空 / 值 = 设值）、内联出边的四条约束、落点四情形、`cascade` 的**单文件闭包**、`expect` 乐观锁与 `hash` 回传、**写入序不变量（子树连续 / 关系组连续 / 新边落组尾）**，SHALL 由 `archi::edit` 的写回原语族承载；本工具 MUST NOT 自拼 TOML、MUST NOT 重实现任一判据（含 `path` 形态、`kind × path` 基数、`apis` 值面），MUST NOT 自行计算搬迁块。执行体 SHALL 只做「入参 → `ElementSpec` / 关系三态 → `plan_*` → 落盘 → 响应装配」。
- **落盘单点**：执行体 SHALL 以 `write_atomic` 单点落盘（临时文件 + 重命名，与壳层命令面共用同一实现），MUST NOT 复制第二份「临时文件 + rename」逻辑，MUST NOT 在 `archi` crate 内出现写盘调用。
- **写保护射程**：解析出的目标文件 SHALL 过**同一** `ProtectRules`（内置 → 会话 → 用户三段序、首命中即拒、拒串沿用既有族），判定细则归 `desktop-write-protection`「会话级写入禁区（相位锁）」；MUST NOT 另立规则集或第二份文案。
- **成员校验与上下文**：两工具 SHALL 走既有成员校验（调用名 ∉ 本轮 definitions ⇒ `tool_unavailable` + `is_error` 回灌，run 不中断）；archi 上下文铸造判据 SHALL 由「名字清单含任一 archi 工具名」表达（七名逐一在场，MUST NOT 漏掉新增两名）。
- **工具描述面（本变更由三件事扩为四件事）**：两工具的描述 SHALL 说明——① 写盘是原子的且**坏文本不可产出**（自证链，零落盘）；② `expect` 的用法与 `hash` 的闭环（「上一轮响应的 `hash` 或 `archi_module` 的 `fileHash` 可直接作 `expect`」）；③ `cascade` 只作用于**同一模型文件**内的子树与边；④ **写入会把目标文件归一到规范序**（子树连续 + 关系组连续，见 `desktop-archi-model`「写入序不变量」）——手改拆开的子树会在下一次工具写时被合回，顺带搬家的块经响应 `reordered` 报出（`moved` 真值、`blocks` 有界）。描述 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 词典与参数 `description`），MUST NOT 教学规范序算法。

#### Scenario: 两写工具在册且档位与下发面对齐

- **WHEN** 审查挂载清单与三个写权装配面的 definitions
- **THEN** `archi_element` / `archi_relation` 均在册且 `ToolTier::Write` / `ToolExec::Archi`；`(proposal, executor)` / `(dev-design, executor)` / `source = "archi"` 三处 definitions 含两者，`RunStarted.tools` 逐名一致；`(implement, executor)` / `(dev-design, evaluator)` / `explore` / `debug` 零下发且调用被成员校验拒绝（`default` 档下写权相位调用亦被档位拒绝——与 `write` 同款）
- **AND** 五只读工具的档位与下发面逐字不变

#### Scenario: 元素 upsert 走通工具面

- **WHEN** 调 `archi_element { action: "upsert", element: "Backend.Core.Billing", kind: "module", path: ["./src-tauri/crates/core/billing/src/"], summary: "计费域" }`
- **THEN** 模型文件新增该元素表（字段按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序），响应含 `created = true`、`changed` 覆盖给予的字段、`file` 落父元素所在文件、`hash` 为新值、`reordered.moved ≥ 1`（该元素的父已有后续兄弟，故新表被规范到父的子树区间内）；再以同值调一次 ⇒ `created = false`、`changed = []`、`reordered.moved = 0`、盘上字节零变化

#### Scenario: 内联出边与独立关系工具等价

- **WHEN** 对同一 `(from, to)` 分别用 `archi_element { relations: [{ to: X, desc: "…" }] }` 与 `archi_relation { action: "upsert", from, to, desc: "…" }`
- **THEN** 两者产出的模型文本**逐字节相同**（同一 `apply_*`），两响应的 `reordered` 也逐字段相同（元素面首元素 FQN 与关系面 `from` FQN 各自口径）；`relations` 四计数与入参数组长度守恒；内联边已声明在**别的**模型文件时前者 `Err` 点名该文件并指向 `archi_relation`，后者照常改写

#### Scenario: 级联只作用于同一模型文件

- **WHEN** 对含子元素与出入边、且整棵子树全在同一文件内的元素调 `archi_element { action: "delete", cascade: true }`；另对一个子元素或某条边声明在**另一**模型文件的元素作同样调用
- **THEN** 前者删净子树与相关边、该文件表头计数随动、其余内容逐字保留；后者 `Err` 点名跨向的文件与阻塞项、**零落盘**（盘上字节零变化）

#### Scenario: 值域与上限在入参面即拒

- **WHEN** 分别以 `action: "patch"` / `kind: "service"` / `gate: "high"` / `relations: [{ to: X, action: "keep" }]` / `cascade: true` 配 `action: "upsert"` / 超过八行上限的任一字段调用两工具
- **THEN** 一律 `Err` 且**零落盘**；值域错误点名该参数的合法值（由 `ALL` 派生），上限错误点名字段与上限值；`default` 档下更早由档位拒绝

#### Scenario: 描述面四件事齐备且不教算法

- **WHEN** 取两工具的 `description`
- **THEN** 各含「坏文本不可产出」「`expect` / `hash` 闭环」「`cascade` 同一模型文件内」「写入归一到规范序 + `reordered`」四组要点；零字段形态示例（`kind = ` / `[[relation]]` / `path = [` / `metadata = {`）、零规范序算法描述（不含「DFS」「`position`」「`ArrayOfTables`」一类实现词）

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
  "model":  { "elements": 56, "relations": 129 },      // 全模型（前值 ± 增量）
  "reordered": { "moved": 3, "blocks": ["Desktop.Views.Changes.Flow",   // 搬迁块（cap 8）
                 "Desktop.Views.Changes.Renderers", "Desktop.Views.Changes.Agent"] } }
```

- **`warnings` 恒在场**：`archi_element` 的判据与降噪不变式归 `desktop-archi-model`「写面反馈判据」；本工具只负责把它序列化进响应（`kind` / `severity` / `message` 三字段，`kind` 与 `severity` MUST NOT 在此另作裁剪或重定档）。`archi_relation` 的 `warnings` SHALL 恒为空数组。**写入序规范化 MUST NOT 新增 warning 条目**（它不是判据）。
- **`changed`**：`action = upsert` 时 SHALL 为**参数面字段名的子集**（实际变化的字段；空数组 = 字段零变化）；`action = delete` 时为空数组。`archi_relation` 的 `changed` SHALL 为 `desc` / `apis` 的子集。**`changed` 空而盘上字节变化是正当形态**（首次把含历史错位的文件规范化），判据取 `changed` 空 **且** `reordered.moved = 0`。
- **`reordered` 恒在场（本变更新增）**：`{ moved, blocks }`——`moved` 为本次搬迁的**块数真值**（永不截断），`blocks` 为各块首元素 FQN（元素面）/ `from` FQN（关系面），按文件序、**cap 8 项**（超出即截断，MUST NOT 附「…等 N 项」文本）；零值时也写 `{"moved":0,"blocks":[]}`（MUST NOT 省略字段）。语义与 cap 归 `desktop-archi-model`「模型写回原语族」的产出形状条，本工具 MUST NOT 在执行体内另算。
- **`archi_relation` 的响应是薄形**：`{ warnings（恒空数组）, action, from, to, file, hash, created, changed, reordered }`——**键面恰九项**；MUST NOT 携带 `relations` 四计数与 `counts` / `model`（一次写只动一条边，计数另经 `archi_drift` 取，不为一行数据付全量重解析）。`reordered` 照带（规范化已在 `apply_*` 内完成，零额外解析）。
- **`relations` 四计数**：`created + updated + deleted + unchanged` SHALL 恒等于入参 `relations` 数组长度（可直接断言的守恒式）；未给 `relations` 时四项皆 `0`（字段不省略）。
- **`counts` / `model`**：`counts` SHALL 等于写后该文件的重解析计数（与表头括号数字同源），`model` SHALL 等于全模型计数（前值 ± 本次增量），两者 MUST NOT 独立于重解析结果另算。
- **`hash`**：SHALL 与 `drift::file_hash` 同一函数产出、同一 `sha256:` 前缀——与 `expect` 可互喂（连续两次写同一文件零额外读）。首次规范化会让 `hash` 随搬迁变化，这是正当结果（`reordered` 已解释变化来源）。
- **响应不含元素 / 关系全文**（要看现状用 `archi_module`）；不含协议正文。
- **成功响应永不触达截断**：`warnings` ≤ 7 条 × ≤200 B + `reordered.blocks` ≤ 8 × ≤256 B + 其余计数与短串 ⇒ 结构上界 ≈ 4 KB ≪ `MAX_RESULT_BYTES`（50 000）。故**不设键序断言**——`serde_json` 未开 `preserve_order`，把 `warnings` 写在对象首位不是机制，防御性排布亦不构成契约；真正需要体量纪律的是 `Err`（见下）。

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

- **四条硬纪律**：① 第一行自足（`Err` 含 `重试：` / `下一步：` / `改用 <工具名>` 之一，或含可复制 JSON 参数片段）；② 字面与词表同源（`gate` 不写「档位」、诊断词不译中文）；③ 不猜（给候选不代选）；④ 一次报全（MUST NOT 首错短路）。**序违例不在任何一层**——规范化没有 `Err` 臂，E1–E7 七层与 `EditErrorKind` 的 23 值面逐字不变。
- **入参上限是软提示 + 硬闸两段**：八行上限 SHALL 同时进 schema（模型侧提示：FQN ≤256 / `path` ≤32 项 × ≤1024 / `metadata` ≤32 项（键 ≤64、值 ≤512）/ `relations` ≤64 / `relations[].apis` ≤32 × ≤128 / `summary`·`desc` ≤512 / `entry`·`doc` ≤1024 / `file` ≤256）与执行体 E1 检查；两处数字 SHALL 取自**同一组常量**（MUST NOT 各写一份）。

#### Scenario: 三通道可辨

- **WHEN** 分别以「写坏模型」「只改 `summary` 的合法 upsert」「与现状同值的 upsert」「指向不存在关系的内联 `delete`」调用 `archi_element`
- **THEN** 依次为：`Err` 携诊断清单（零落盘）/ `warnings = []` 且 `changed = ["summary"]` / 幂等（`changed = []` ∧ `reordered.moved = 0`、盘上字节零变化）/ `Err` 列出同 `from` 的现有出边（零落盘）

#### Scenario: 降噪不变式在工具面成立

- **WHEN** 对 `api = declared` 且契约 md 缺失的元素分别调「只改 `gate`」与「改 `path`」两次 upsert
- **THEN** 前者 `warnings = []`；后者 `warnings` 含 `path` 诱发的条目；两次的 `warnings[].kind` 恒为既有两词汇集的 `as_str` 之一（`ArchiDriftKind` / `ArchiApiFindingKind`），severity 与判据面单点同值；规范化搬迁块不产任何 warning

#### Scenario: 四计数守恒与幂等

- **WHEN** 一次 upsert 传入三条 `relations`（一条新建、一条改 `desc`、一条 `delete`），再一次以完全相同的参数重放
- **THEN** 首次 `relations = { created: 1, updated: 1, deleted: 1, unchanged: 0 }`（四者之和 = 3）且 `changed` 非空；重放时该边已不存在或值已相同 ⇒ 逐项按实计（`unchanged` 吸收），四者之和仍 = 3，`changed = []`，盘上字节零变化

#### Scenario: 成功响应体量与计数可对拍

- **WHEN** 取任一成功响应并序列化，随后对该文件重解析、对全模型取 `archi_drift`
- **THEN** 响应长度 < `MAX_RESULT_BYTES`；`counts` 与文件重解析计数及表头括号数字三者相等；`model` 与 `archi_drift` 报告的 `model.elements` / `model.relations` 逐值相等

#### Scenario: 搬迁块在两写工具同形

- **WHEN** 新建一个父元素已有后续兄弟的元素；随后对一条 `from` 已有组的 `(from, to)` 调 `archi_relation { action: "upsert" }`；再各重放一次
- **THEN** 元素面响应 `reordered.moved ≥ 1` 且 `blocks` 含该新元素 FQN；关系面响应 `reordered.moved = 1`、`blocks = [<from>]`（新条目落组尾）；两次重放均 `moved = 0` 且盘上字节零变化；`archi_relation` 响应的键面恰九项（含 `reordered`，仍无 `counts` / `model` / `relations`）

#### Scenario: Err 第一行自足且值域点名

- **WHEN** 以非法 `action` 与过期 `expect` 分别调用
- **THEN** 前者文案点名合法值（`upsert` / `delete`）与参数名；后者给 expect / 当前双值并含两条出路之一（`重试：` 或「省略 expect 直接覆盖」）；两例均零落盘

## Module Contract

### Function: `crates/infra/agent/src/sdk/`

| 项 | 变化 | 描述 |
|---|---|---|
| `sdk/mount.rs`（两写工具 `description`） | 改 | 各加一句「写入归一到规范序（子树连续 + 关系组连续）+ 手改拆开的子树会在下次工具写时被合回 + 顺带搬家经响应 `reordered` 报出」；`parameters` / `WRITE_LIMITS` / 挂载清单 18 项 / 档位 / 矩阵**零改动** |
| `sdk/archi_tool.rs` | 改 | `element_response` 与 `relation_payload` 各加 `"reordered": &outcome.reordered`（零额外解析、零计数重算）；保护闸 / 落盘 / 三通道其余键逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs` | **零改动** | 骨架 1777 B / 全文 1960 B 冻结值与 `1800 / 2000` 双门限逐字不变（净余 23 B 不花、不抬锚）；「重排」字样本就零命中 |
| `crates/infra/agent/src/sdk/policy.rs` / `compose.rs` / `sdk/runner.rs` / `sdk/tools.rs` / `sdk/protect/**` | **零改动** | 档位归属、注入矩阵、上下文铸造判据、`input_path` 名单、保护闸规则集逐字不变（本变更不新增工具） |

### API: 工具响应

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_element` 响应 | 新增字段 | `reordered: { moved, blocks }`（恒在场；`moved` 真值、`blocks` cap 8）；其余八键与取值面逐字不变 |
| `archi_relation` 响应 | 新增字段（薄形键集 8 → 9） | `reordered` 进场；`counts` / `model` / `relations` 仍不携带 |
| 工具 `parameters` / 档位 / 下发面 | 零改动 | schema 键值、`enum` 派生、`ToolTier::Write`、三处下发面逐字不变 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| 知识骨架与注入面 | `preamble` 拼装、派生路径段、降级语义逐字不变（本变更不加知识句） |
| 两份 planner prompt（`crates/core/workflow/src/prompts/`） | 零改动：现有句「工具表达不了的才手改」不含「重排」字样，与规范化相容；口径校正归 `desktop-phase-prompts` 的 delta |
| IPC 命令面 / 前端 | 零改动：`reordered` 只出现在 agent 工具响应，不进任何 Tauri 命令与 DTO |
