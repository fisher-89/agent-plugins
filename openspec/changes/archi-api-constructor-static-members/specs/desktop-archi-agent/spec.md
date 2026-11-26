# desktop-archi-agent Specification（delta: archi-api-constructor-static-members）

## Purpose

本 delta 只改**知识骨架里的一行枚举**与配套的体量锚，不碰工具面、矩阵、档位与注入机制：

- `x-` 关键字面由三项扩为五项（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`，协议侧定义见 `desktop-archi-api`「类型的构造入口与静态方法」）。骨架 SHALL **逐名列出五项**——运行期注入给 agent 的只有 `archi-knowledge.md` 编译期内嵌的那一份字节，它够不着 `openspec/`，写「见 spec」在生产面等于永不抵达。这与「导出名的唯一持有者是 schema」不是同一条纪律的冲突：那条管**导出名**，`x-` 关键字是**协议词汇**，持有者不同。
- 体量锚四数同批：骨架 **1808 B**（≤ 1831 B）、全文 **1997 B**（≤ 2031 B）。本次按枚举增量 **+31 B** 显式豁免抬锚（原 1777 / 1800 / 1966 / 2000），并同批改三处文案（`archi_knowledge_test.rs` 的模块头、测试 doc 的「MUST NOT 抬锚」表述、两条 `assert_eq!` 的失败文案）与一道 needle 守卫（`指令表收窄且新行集可判` 补 `` `x-constructor` `` / `` `x-static` ``）。
- **抬锚的最小口径**：只按本次 +31 抬，不顺手塞别的规则。「至少一条构造通路」（二期判据面）与「object ⇔ JSON 对象」（写闸已强制）两条都**不进骨架**，本变更显式拒绝。
- 零变化面：五段结构、工具面指路、手改出口句、留给判断段、四条零残留纪律（零围栏 / 零值域行 / 零结构诊断词 / 零 `<>` 占位符）、拼装与降级语义、矩阵与档位逐字不变。

## MODIFIED Requirements

### Requirement: 架构会话的系统知识注入

`source = "archi"` 会话与 **change 臂的两写权相位** SHALL 注入同一段架构会话知识到 `SessionInjections.preamble`，内容 SHALL 覆盖以下**五段**（本变更是骨架的第三次改写：三资产分工与设计顺序取代语法类知识，散文只留不可脚本化的判断面）：

- **三资产分工（C）**：模型 TOML 答「谁在谁的里面、谁依赖谁」（拓扑）；`<叶段>.schema.json`（与文档同目录）答「跨过这条边时接触面长什么样」（接口，**唯一凭据**）；`<叶段>.md`（同目录）答「为什么这样切、有什么坑、什么不归它管」（理由）。**过度设计 = 回答自己不该回答的那个问题**：文档不列接口、schema 不写理由、TOML 不造空元素。**文档是产品面**——架构页「文档」段直接渲染给用户读，人 + AI 共读；接口由写闸校验、人经 UI 阅读。
- **设计顺序（B/C）**：① 定位现状（`archi_search` 发现 → `archi_module` 一次取全邻居 / 三份资产 / 声明位置）；② 画拓扑（`archi_element` / `archi_relation`）——**先边界、后接触面、最后写理由**；③ 补两份资产（`archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 落 `.schema.json`（坏文本零落盘）→ 理由落 `.md`，段落按内容自定）。
- **三条不可越界（C）**：① **文档不列接口**——导出名 / 签名 / 类型的唯一持有者是 schema；历史 `api-*` fence 是遗留内容（读面不读、清理中），MUST NOT 新增；② **schema 不写理由**——`x-function.description` 只写调用方要知道的语义（做什么 / 边界 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common` / `x-constructor` / `x-static` **五项**（逐名列出，MUST NOT 写成「见 spec」），MUST NOT 自造新前缀键；③ **TOML 不造空元素**——每个元素至少一条接触面（`path` / 边 / 导出）。
- **工具面（A/B，指路）**：字段面、键形、缺省、值域、祖先与计数全部由工具承担——MUST NOT 凭记忆手拼 TOML。读五件各一句「何时用」（`archi_search` 发现 / `archi_module` 定位与三资产 / `archi_drift` 漂移读数 / `archi_api` 接口三动作 / `archi_scaffold` 字段词典）；写两件（`archi_element` / `archi_relation`，参数面即字段清单、坏文本零落盘）；**手改出口（永久内容）**：工具表达不了的才手改 TOML（骨架保持这一简写；**适用边界由本能力裁定** = 批量整理 / 同层手排序——子树连续与关系组连续由写面自动维持，手改拆开的子树会在下一次工具写时被合回，故「重排文件」不在边界内），改完必须 `archi_module` 自查——坏模型由读取面报错（结构诊断带 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改。自检纪律：改完 `archi_drift` 非空即逐条修、`archi_api`（`check`）查两轴。契约资产 `config.json` 管 `ignore` / `exclude`，非法由 `archi_drift` 点名。
- **留给判断 + 兼容（C）**：`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；**纯分组容器用 `composite`**；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），不要编造导出**；旧元素可能仍把接口写在 `.md` 的 `api-*` fence 里——读面不读，接口一律写 `.schema.json`，MUST NOT 往 `.md` 新增 fence。

两条工程约束与下列「值域不进知识文本」纪律逐字沿用：

- `gate` / `api` / `kind` 的值域与缺省档 SHALL 由工具面承载（参数 `description` 与 `archi_scaffold` 词典），骨架 MUST NOT 再写值域行；键形与字段形态 MUST NOT 进骨架；结构诊断全量词表 MUST NOT 进知识文本（`orphanHierarchy` / `dependsOnAncestor` 亦不在骨架，祖先缺失由写工具 `Err`、越界依赖由 `archi_drift` 承载）。**文件序不进骨架**：子树连续与关系组连续是写面默认行为，骨架既不解释也不要求 agent 记得——骨架的手改出口句保持简写（「工具表达不了的才手改」），适用边界的裁定（批量整理 / 同层手排序）与「拆开会在下次写时合回」留在本节与工具描述，**不进骨架**。
- `entry` / `doc` / `apis` MUST NOT 进任何值域词表（开放集）；判据长表 MUST NOT 复制进知识文本。
- **协议词汇的枚举 MUST 逐名进骨架，判据面的事实 MUST NOT 进骨架**：`x-` 面五项是**协议词汇**（写 `.schema.json` 时要用到的名字），枚举本身 SHALL 在骨架内逐名列出；而「每个 `x-class` 至少一条构造通路」是跨元素判据面的事实（判定面在后续 change），MUST NOT 进骨架——两者的区别是「写的时候要用的词」vs「写完才判得出的事」。
- **体量锚（本次显式豁免抬锚）**：骨架 SHALL ≤ **1831 B**、全文 SHALL ≤ **2031 B**，实测值 SHALL 被冻结断言（测量口径沿用 `lf_bytes(trim_end)`——LF 归一、去尾随换行）。相对基线的四数（骨架 1777 B / 全文 1966 B / 门限 1800 / 2000）本变更按 `x-` 面枚举增量 **+31 B** 同批改为 **1808 / 1997 / 1831 / 2031**（两处精确 `assert_eq!` + 两处 `<=` 门限），余量仍为 23 B / 34 B。豁免是**逐次**的、MUST NOT 视为门限作废：往里加东西前仍应先问「这条能不能由工具面 / 参数 `description` 承载」；超限 SHALL 压缩同段冗余表述，MUST NOT 删除三资产分工 / 设计顺序 / 手改出口 / 留给判断四段任一，MUST NOT 借抬锚恢复语法教学（值域行 / 字段形态 / 诊断词表三类仍在禁令内）。
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
- **THEN** 「文档不列接口」/「MUST NOT 复述签名」/「TOML 不造空元素」三条各至少一处；`x-` 面封闭**五项逐名**在场（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`），且该行内 MUST NOT 出现「见 spec」一类指路；「空 schema」与「不要编造导出」在场；「历史 fence 读面不读、MUST NOT 新增」在场

#### Scenario: 五项枚举有机械守卫

- **WHEN** 从骨架的 `x-` 面枚举里删掉一项（如删 `x-constructor`）后跑知识语料测试
- **THEN** 测试红（`指令表收窄且新行集可判` 的 needle 集逐名核对两新键的带反引号原样文本）；恢复该项后绿——枚举本身不得只靠人眼守

#### Scenario: 手改出口是永久内容

- **WHEN** 审查骨架「工具面」段的最后一句
- **THEN** 该句含三要素：①「工具表达不了的才手改」的适用边界；②「改完必须自查」（点名 `archi_module`）；③ 坏模型由读取面报错（带 `file:line`）；MUST NOT 被表述为过渡措施或待删项；且句内不含「重排」一类已被写面接管的动作

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
- **THEN** 骨架 ≤ 1831 B、全文 ≤ 2031 B 且实测值（**1808 B / 1997 B**）被冻结断言；四数与三处文案（模块头门限表述 / 测试 doc 的抬锚口径 / 两条失败文案）同批更新；程序追加段含模型声明目录与契约资产目录（文档 + 接口两条同目录资产）两处方位，零契约索引串，骨架零目录字面量

#### Scenario: 判据面事实不进骨架

- **WHEN** 审查骨架全文
- **THEN** 零「构造通路」/「至少一条」一类跨元素判据表述（它属 `desktop-archi-api` 与后续 change 的判据面）；零「object 等同 JSON 对象」的复述（写闸已强制，骨架再加只是复述）；抬锚增量恰为 `x-` 面枚举那一行的 +31 B（全文与骨架的增量相等）

## Module Contract

### Function: `crates/infra/agent/src/archi-knowledge.md`

| 项 | 变化 | 描述 |
|---|---|---|
| `x-` 面枚举行 | 改 | `x-` 面封闭三项 → **五项逐名**（`` `x-function` / `x-class` / `x-common` / `x-constructor` / `x-static` ``），+31 B；该行仍留在「三条不可越界」② 内，不新开小节 |
| 其余段落 | 零改动 | 五段结构、工具面指路、手改出口句、留给判断 + 兼容、零围栏 / 零值域行 / 零诊断词 / 零 `<>` 占位符逐字不变 |
| 体量 | 改 | 骨架 `lf_bytes(trim_end)` 1777 → **1808 B** |

### Function: `crates/infra/agent/src/archi_knowledge_test.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| 模块头 doc | 改 | 「体量冻结（骨架 ≤ 1800 B / 全文 ≤ 2000 B）」→ 新四数（1808 / 1831 / 1997 / 2031） |
| `骨架与全文双门限冻结` | 改 | 两条 `assert_eq!` 的实测值 1777 → 1808、1966 → 1997；两条 `<=` 门限 1800 → 1831、2000 → 2031；测试 doc 的「内容变更须同批更新，MUST NOT 抬锚」改为「抬锚须显式豁免并同批更新四数」；三条失败文案里写死的数值与括号说明同步 |
| `三条不可越界逐条在场` | 改 | needle 集由「`x-` 面封闭三项」改为**五项逐名**（并加断言：该行不含「见 spec」） |
| `指令表收窄且新行集可判` | 改 | needle 集补 `` `x-constructor` `` / `` `x-static` ``（带反引号原样）——删任一项即红 |

### Function: `crates/infra/agent/src/archi_knowledge.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `build` / `knowledge` / 模块头 | 零改动 | 拼装面（骨架 + 两处 `foundation::layout` 派生路径）、占位符与空骨架校验、降级语义逐字不变；模块头未写体量与枚举数 |

### API / Component: 零改动面

| 项 | 变化 | 描述 |
|---|---|---|
| `sdk/mount.rs` / `sdk/archi_tool.rs` / `sdk/policy.rs` / `sdk/runner.rs` / `compose.rs` | 零改动 | 工具清单 18 项、档位、`(phase, role)` 矩阵、上下文铸造判据、`archi_api` 三动作、`archi_scaffold` 词典逐字不变（无新工具、无参数变化） |
| `SessionInjections.preamble` 装配与降级 | 零改动 | 两处消费（`source = "archi"` 与 change 臂写权相位）与 `preamble = None` 降级逐字不变 |
| DTO / IPC / 前端生成物 | 零改动 | 知识文本不进 `dto`、不经 IPC 出线 |
