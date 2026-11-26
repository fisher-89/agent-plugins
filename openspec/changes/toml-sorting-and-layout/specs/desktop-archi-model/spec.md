# desktop-archi-model Specification（delta: toml-sorting-and-layout）

## Purpose

本 delta 只给写回原语族补**一条不变量与一个出线字段**——把「文件序」从「某次整理的时机问题」改成「写面输出必须满足的性质」：

- **写入序不变量**：每次写回的产出满足 E1（父先于子）/ E2（子树连续）/ R1（同一 `from` 组连续），规范序取 **DFS + 同层首次出现序**与**组连续 + 组间首次出现序**的**最小移动**形式——对健康文件规范化为 no-op，对 `06-frontend.toml` 的 1 处断裂只搬 3 块。规范化落在 `apply_*` 内、渲染之前，**不进** `finish` 的运行时自证链（序由测试守，美观缺陷不得升级为写入失败），**不产**任何诊断 / warning / 新 `EditErrorKind`。
- **`WriteOutcome.reordered`**：`{ moved, blocks }`——搬迁块计数（真值，永不截断）+ 各块首元素 / `from` FQN（按文件序、cap 8 项）。这是本变更唯一的出线增量；同时把「`changed` 空 ⇒ 逐字节相等」这句无条件承诺限定为「`changed` 空 **且** `moved = 0` ⇔ 逐字节相等」。
- **限定而非改写**：渲染纪律的「原样保留」收窄为**块内字节与键序**（块的相对位置由规范序决定）；判据面（parser 是形态唯一判据）、`expect` / `hash`、表头计数回填、三态、落点、级联、关系端点的既有条款逐字保留。
- 零判据面变化：`DiagnosticKind` 九值、`EditErrorKind` 23 值、`ArchiDriftKind` 五值、`ArchiApiFindingKind` 值集与报告结构逐字不变。

## MODIFIED Requirements

### Requirement: 模型写回原语族（元素 / 关系 upsert 与 delete）

`archi` SHALL 提供**模型写回原语族**：纯函数层 `apply_*`（源文本进、改写后的完整源文本出），计划层 `plan_*`（读盘定位 + 纯函数 + 待落盘产物 + 反馈面）。gate 改档是其中 `ElementSpec { gate }` 的一个退化调用——MUST NOT 保留第二套 gate 专用入口（`set_gate` / `plan_set_gate` / `GateWrite` 均已出射程）：

- **八函数两面**：`apply_element_spec` / `apply_element_delete` / `apply_relation_upsert` / `apply_relation_delete` 为纯函数（零文件系统触碰）；`plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` 为计划函数（`parse_dir` 结构诊断前置 → 存在性前置 → 读源 → `apply_*` → 填 `hash` → 装反馈面），**零写盘**。写盘单点仍是 `write_atomic`（同目录临时文件 + rename）：壳层命令面与 agent 写工具 SHALL 共用同一实现，MUST NOT 各自复制一份。**删除侧只有一个纯函数形态**：`apply_element_delete(source, &DeleteSet)`（`DeleteSet` = 待删元素 FQN 集 + 待删关系对集），单元素删除是「集合恰一个元素」的退化形态——MUST NOT 保留第二个「只删一个元素」的删除实现。四个 `apply_*` 的产出 SHALL 为「交付文本 + `Reorder`」对（`Reorder` 由规范化产出，见「写入序不变量（本变更新增）」条）——MUST NOT 让消费面自行比对前后文本重算搬迁块。
- **产出形状**：`plan_*` SHALL 返回 `WriteOutcome`——`write: ModelWrite { file, content, hash }`（P3 三元组逐字不变）+ `warnings: Vec<EditWarning>` + `changed: Vec<String>` + `created: bool` + `relations: RelationTally { created, updated, deleted, unchanged }` + `counts: { elements, relations }`（本文件重解析）+ `model: { elements, relations }`（全模型）+ **`reordered: Reorder { moved, blocks }`（本变更新增）**。`changed` SHALL 为**参数面字段名的子集**（实际变化者；空 = 字段零变化——**幂等的可断言形态 = `changed` 为空 ∧ `reordered.moved = 0` ⇔ 产出与源逐字节相等**；`moved > 0` 时产出必与源不同而 `changed` 仍可为空，见「写入序不变量」条）；`relations` 四计数之和 SHALL 恒等于本次 `relations` 入参长度（未给则四项皆 0）；`counts` SHALL 等于写后该文件的重解析计数（表头回填的同一来源）；`model` SHALL 等于前值 ± 本次增量（元素 ±1 或 −子树规模、关系 ±added/−deleted），可对拍写后 `drift` 报告的 `model.elements` / `model.relations`。**`reordered` 恒在场**（`moved = 0` 时也写 `{"moved":0,"blocks":[]}`，MUST NOT 因零值省略该字段，与「投影不补默认值 / 显式字段不省略」同款）：`moved` SHALL 为本次搬迁的**块数真值**（`usize`，永不截断）；`blocks` SHALL 为各搬迁块的**首元素 FQN**（元素面）/ **`from` FQN**（关系面），按文件序、**至多 8 项**（超出即截断——`moved` 仍为真值，MUST NOT 附加「…等 N 项」一类省略文本）。
- **渲染纪律（基线三条逐字保留）**：渲染 SHALL 经 `toml_edit`（解析 → 改 AST → 渲染），MUST NOT 字符串正则替换；改写后**每个元素块 / 关系条目块内部的字节**（键序 / decor / 注释 / 空行）SHALL 原样保留，**块在文件内的相对位置**则由「写入序不变量」决定（那是渲染前对 AST 的位置重编号与成员重排的结果，不是重写块内容）；**既有键序 MUST NOT 重排**——新键追加到元素表尾，新建元素的表按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序整体写出（MUST NOT 为「重排既有元素的键」引入第二份字段序常量）。
- **写入序不变量（本变更新增）**：每次写回的交付文本 SHALL 满足三条不变量——**E1 父先于子**（元素的表在其祖先之后）、**E2 子树连续**（一个元素的全部后代占据紧邻它的连续区间）、**R1 同一 `from` 组连续**（同一 `from` 的关系条目聚成一段）。**规范序**为：元素面 = **DFS + 同层保持首次出现序**（顶级元素按文件出现序；某元素的子元素 = 文件中所有 `<父FQN>.` 直接子，按首次出现序递归展开）；关系面 = **组连续 + 组间保持该 `from` 在文件内首次出现的位置 + 组内序原样**（新边落组尾）；元素区恒在关系区之前（混插形态被理顺）。规范化 MUST 取**最小移动**形式：只搬迁「不在其父子树连续区间内」的元素段与「组分裂」的关系段，同层相对序、组内相对序与块的内部字节逐字保留——**对已满足三条不变量的文件，规范化是 no-op（产出与源逐字节相等）**。实现口径两条：① 元素面的顺序靠**重编号 `position`**（元素区连续分配、关系区在其后连续分配），MUST NOT 靠重排条目；关系面 MUST 真移动 `ArrayOfTables` 成员**并同批重编号**（Vec 序与 `position` 序 MUST 同源，只改一处会让渲染序与读回序打架）；② 规范化发生在 `apply_*` 内、渲染之前，产出的 `Reorder` 随写回结果回传。**射程两条**：**单文件**（元素 MUST NOT 跨模型文件搬迁——一次 plan = 一个 `ModelWrite`）；**表形态**（只对 `[element."<FQN>"]` 形态的元素表动手，内联表形态的条目原样保留、不参与重排）。**零暴露面**：序违例 MUST NOT 变成诊断或 warning（`DiagnosticKind` 九值 / `ArchiDriftKind` 五值 / `ArchiApiFindingKind` 值集与 `EditErrorKind` 的 23 值面逐字不变，规范化**无 `Err` 臂**），MUST NOT 进入 `finish` 的运行时自证链——序由测试守，一个美观缺陷 MUST NOT 升级为写入失败；MUST NOT 提供任何手动触发入口（无 `archi_sort` 一类工具，含只读预览形态）。
- **三态语义**：可清空位（`entry` / `doc` / `summary` / `path` / `metadata` / 关系 `desc` / 关系 `apis`）SHALL 以 `Option<Option<T>>`（或「`Some` 空集合」的同构形态）表达三种意图——`None` = 不动、`Some(None)` = 删键（`path` / `metadata` / `apis` 以 `Some(空集合)` 表达同一意图）、`Some(Some(v))` = 设值。封闭枚举位（`kind` / `gate` / `api`）无清空臂：`Some(GateTier::Off)` 是**显式化缺省**（ratchet 的正当动作），不是清空。MUST NOT 用 `Option<T>` 混过三态，使「不动」与「清空」不可辨。
- **内联出边（本变更新增）**：`ElementSpec.relations: Vec<RelationSpec>`（`{ to, desc, apis, action }`）SHALL 在一次元素写回里批量处理出边，四条约束逐条成立——① **同一 `to` 在一批里只出现一次**（重复即 `Err`，否则「先 upsert 后 delete」取决于数组序，语义不可判定；错误定位用 **1 基下标**且与 `action` / `to` 同时出现）；② `action = delete` 项 MUST NOT 带 `desc` / `apis`（带了即 `Err`——删除动作与字段赋值互斥，静默忽略会让调用方以为赋值生效）；③ **内联边只作用于目标文件**：某条内联边已声明在**别的**模型文件 ⇒ `Err` 点名该文件（一次 plan 只写一个文件，MUST NOT 静默只改同文件那份）；④ **未列出的既有边保持不动**（全量替换会一次漏列即静默删依赖）。内联 `delete` 与独立 `apply_relation_delete` SHALL 共用同一判据与同一文案族（MUST NOT 为内联路径另写「不存在就静默跳过」的宽松版）。
- **自证对象 = 交付文本**：产出顺序 SHALL 固定为「改 AST → 规范化 → 渲染 → 表头计数回填 → 重解析自证零诊断 → 回读逐字段一致」；重解析出任何诊断、或回读值与规格不一致即 `Err` 且零文本产出——MUST NOT 把坏文本交给调用方落盘，MUST NOT 先验后改（被验的文本必须逐字等于交付的 `content`）。规范化的成败**不进**该链：它没有 `Err` 臂，MUST NOT 在自证里断言序。
- **表头计数回填**：目标文件**首部注释块**内的第一处 `（<数字> 元素 / <数字> 关系）`，括号内两个数字 SHALL 按该文件的重解析结果回填；括号外（人工标题与散文）逐字保留；源中无该括号 MUST NOT 新增，首部注释块之外的同形文本 MUST NOT 改动。计数的唯一权威是重解析结果——MUST NOT 另行统计源码行数或表头条目数。规范化 MUST 复用这一函数（重排不改计数，MUST NOT 另写一份）。
- **乐观锁 `expect`**：`plan_*` SHALL 接受可选的 `expect`（目标文件**当前**内容哈希，`sha256:…` 形态）并在读源之后、渲染之前校验：不匹配即 `Err` 且零文本产出（目标文件缺席而给了 `expect` 同判不匹配；`None` = 不校验，不存在「期望文件缺席」这一臂）。`ModelWrite.hash` SHALL 为**写后**内容哈希，与 `drift::file_hash` 同口径（`ModelWrite.hash` 逐字等于把 `content` 落盘后的 `file_hash(file)`），使调用方能把它直接作为下一次 `expect`。三条写面（画布改档 + 两个 agent 写工具）共用同一校验点，MUST NOT 出现绕过 `expect` 的写面分支；规范化落在 `expect` 校验之后、`hash` 计算之前，MUST NOT 改动这条链的顺序。
- **落点（本变更改写）**：既有元素 SHALL 落其声明所在文件（`ElementSpec.file` 给了不同值即 `Err`——迁文件属独立语义）。新建元素的落点按序：① 给了 `file` ⇒ 落该文件（repo 相对、`archi_graph_model_dir` 一级 `*.toml`），且目标文件**不存在**时 MUST 同时给 `create = true`（否则 `Err` 并提示——防拼错名字造出幽灵文件）；② 省略 `file` ⇒ 落**父元素**声明所在文件（祖先须显式声明是既有前置，故「最近已声明祖先」即父元素）；③ 单段 FQN（无父）且省略 `file` ⇒ `Err` 点名需给 `file`（新分层的落点必是人的判断）。MUST NOT 引入「按 `kind` / 首段 FQN 推落点」的约定（文件名里的语义序号工具无法推导，且本仓同文件跨分层元素的反例无法用规则表达）。新建时 `kind` 必填；父元素缺席（会造出 `orphanHierarchy`）、目标 FQN 已在案、落点逃出模型声明目录、模型声明目录缺席 ⇒ 一律 `Err`；写面 MUST NOT 代建目录。删除与关系两侧另有「元素必须存在」前置：缺席即 `Err` 并附现有 FQN 清单或模型元素总数。**新建元素的 FQN 形态**另有一道写面卫生前置：MUST NOT 含空白 / `"` / `[` / `]` / `#` / 换行（写面要把该串渲染成键，含这些字符会产出一个转义后不可读的键）——该前置只作用于**新建**，既有元素的 FQN 形态仍只由 parser 判，写面 MUST NOT 借此对既有值重做形态校验。
- **删除与级联（本变更改写）**：`plan_element_delete(root, fqn, cascade, expect)`——`cascade = false`（缺省）时「该元素仍有已声明子元素」或「仍被任一关系引用（出边或入边）」即 `Err` 并点名阻塞者（P3 行为逐字保留）；`cascade = true` 时 SHALL 先算**闭包**（子树 ∪ 出入边），再按文件分组：闭包全在目标文件 ⇒ 删净（子树按 FQN 前缀、关系按 `(from, to)` 对）；闭包**跨文件** ⇒ `Err` 点名跨向文件与阻塞项且零文本产出。**`cascade` 的承诺面就是「同一模型文件内」**——MUST NOT 被表述为跨模型文件的级联（一次 plan = 一个 `ModelWrite`、一次原子写；多文件事务无解，拒绝并点名优于半成功）。
- **关系落点**：新关系 SHALL 落 `from` 元素声明所在文件；`from` / `to` 端点元素缺席即 `Err` 并点名缺失端；既有 `(from, to)` 走更新（改 `desc` / `apis`）而非重复追加；模型面同一 `(from, to)` 多处声明即 fail-closed（既有裁定逐字保留）。新建关系条目 SHALL 在写入后落入其 `from` 组的组尾（不是条目数组尾部）——这是 R1 的直接后果，MUST NOT 被实现成「先追加后人工排序」。
- **值域与形态判据归既有单点**：`kind` / `gate` / `api` 的非法值由封闭枚举 + IPC 反序列化拒绝（列全合法值的文案归 serde），写面 MUST NOT 重写一份值域表；`path` / `apis` / `entry` / `doc` 的形态仍由 parser 判（写面 MUST NOT 重写第二份形态判据），越界值经自证链以结构化诊断落地并令写回失败。
- **错误面**：所有失败 SHALL 为封闭枚举 `EditErrorKind`（附 `ALL`，本变更由 19 值扩为 **23** 值：新增「内联 `delete` 项带字段」「同批重复 `to`」「级联闭包跨文件」「内联边声明在他处」四条）+ 事实段组成的 `EditError`，`message()` 形态恒为「判据句：当前事实 + 下一步」；壳层与工具面经 `EditError::message()` 映射进既有 `Result<T, String>` 错误面。MUST NOT 出现裸 `String` 错误的第二路径；`ALL` 与 `criterion()` 的 `match` 分支集合 SHALL 逐值相等（新增变体未同批补 `ALL` 即测试红）。**本变更零新增变体**：规范化无 `Err` 臂，23 值面与全部判据句逐字不变。
- 该族 MUST NOT 触碰文件系统的**写**动作（读盘归 `plan_*`，写盘归消费面）；`archi` 内 MUST NOT 出现写盘调用。判定域既有边界不变：零 spawn、零 Tauri、零 store、零 `workflow` 依赖。

#### Scenario: 改档保留注释与其余内容

- **WHEN** 对含注释、空行与多个元素的源文本把 `Backend.Core.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 返回文本中该元素 `gate = "strict"`，其余元素、注释、空行与键序逐字保留（例外仅两处：首部注释块的 `（N 元素 / M 关系）` 计数括号按重解析结果回填；块的相对位置按「写入序不变量」归一）；再解析该文本得到新档位且零诊断

#### Scenario: 元素缺席分两路

- **WHEN** 以不存在的 FQN 且不给 `kind` 调 `plan_element_upsert`；另以不存在的 FQN 调 `plan_element_delete`
- **THEN** 前者 `Err`（判据句点名新建缺 `kind`，事实段点名该 FQN 不在案——缺席在 upsert 里是**新建分支**，不是错误本身）；后者 `Err`（判据句点名元素不存在，事实段列出模型现有 FQN 清单或元素总数）；两者零文本产出

#### Scenario: 渲染不可解析或回读不一致即不产出

- **WHEN** 渲染路径产出的文本无法被解析器读回，或回读到的字段值与规格不一致
- **THEN** 返回 `Err` 且不产出任何待落盘文本（自证不可解析即失败、回读不一致即失败），调用方零写入

#### Scenario: 三态逐位可辨

- **WHEN** 对同一元素分三次调用 `plan_element_upsert`，依次传 `summary = None`、`summary = Some(None)`、`summary = Some(Some("一句话"))`
- **THEN** 第一次该键字节不变；第二次该键从源中消失；第三次写入新值；`path` / `metadata` 以 `Some(空集合)` 表达清空（回读为空数组 / 空表、键不在源）；`gate = Some(GateTier::Off)` 写显式 `gate = "off"` 行而非删键

#### Scenario: 新建元素按父元素落点与规范字段序

- **WHEN** 对一个缺席 FQN（父元素已在案）省略 `file` 调 `plan_element_upsert`；另给 `file`（模型声明目录一级、文件已存在）与 `file`（文件不存在 + `create = true`）各调一次；再对一个单段 FQN 省略 `file` 调一次
- **THEN** 前者落父元素所在文件、新增一张 `[element."<FQN>"]` 表、字段按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序写出（该表按 E1 / E2 落在其父的子树区间内——父的子树区间已是文件尾时 `reordered.moved = 0`，其后再有兄弟元素时 `moved ≥ 1`）；第二个改既有文件；第三个在 `file` 不存在且无 `create` 时 `Err` 并提示 `create = true`，给了 `create` 则新建文件且表头为 `# <名>（1 元素 / 0 关系）`；第四个 `Err` 点名需给 `file`。缺 `kind`、父元素缺席、FQN 已在案、落点逃出模型声明目录、模型声明目录缺席、FQN 含空白 / 引号 / 方括号 / `#` / 换行——各返回 `Err` 并点名成因

#### Scenario: 内联出边四条约束

- **WHEN** 一次 `plan_element_upsert` 传入 `relations = [{to: A}, {to: B, desc: Some(None)}, {to: C, action: delete}]`；另构造同批重复 `to`、`delete` 项带 `desc`、内联边已声明在另一文件的三种违例
- **THEN** 前者建 A、清 B 的 `desc`、删 C，未列出的既有边零改动，`relations` 四计数之和 = 3；三种违例分别 `Err`（重复项以 1 基下标 + `action` + `to` 定位；带字段项点名互斥；跨文件项点名该文件），全部零文本产出

#### Scenario: 级联闭包只走同一文件

- **WHEN** 对「子树与出入边全在同一文件」与「某个子元素或某条边在另一文件」两种情形分别调 `cascade = true` 的删除
- **THEN** 前者删净闭包（子树 + 出入边 + 表头计数随动），其余内容逐字保留；后者 `Err` 点名跨向文件与阻塞项且零文本产出；`cascade = false` 的两种拒绝臂（有子 / 被引用）逐字保留

#### Scenario: 反馈面四组可对拍

- **WHEN** 对同一元素以完全相同的参数连调两次 upsert（第二次之前文件已收敛到规范序），并在第一次之后取 `counts` / `model` / `changed` / `relations` / `reordered`
- **THEN** 第一次 `created = true`、`changed` 覆盖给予的字段、`counts` 与该文件重解析计数及表头括号数字三者相等、`model` 与写后 `drift` 报告的 `model.elements` / `model.relations` 相等；第二次 `created = false`、`changed = []`、`reordered = { moved: 0, blocks: [] }`、`content` 与源逐字节相等

#### Scenario: 乐观锁不匹配零文本产出

- **WHEN** 以过期的 `expect`（目标文件在取哈希之后已被改动）调用任一 `plan_*`，随后以 `file_hash` 取到的当前值重试
- **THEN** 前者 `Err`（判据句点名期望与实际的摘要），零文本产出；后者 `Ok`，且 `ModelWrite.hash` 与 `content` 落盘后 `file_hash(file)` 的返回值逐字相等

#### Scenario: 表头计数由重解析回填

- **WHEN** 在表头写 `（9 元素 / 10 关系）` 而文件内实为 8 元素 10 关系的临时模型上做一次元素删除；另对计数准确的本仓模型做一次「把 `gate` 改成其当前值」的写回
- **THEN** 前者产出文本的表头变为 `（8 元素 / 10 关系）`、括号外整行逐字不变；后者产出与源**逐字节相等**（计数准确 ⇒ 回填为零差异，且文件已合规 ⇒ 规范化为 no-op）；源无该括号时不新增，首部注释块之外的同形文本零改动

#### Scenario: 删除 fail-closed

- **WHEN** 分别对「仍有已声明子元素」「仍被关系引用（出边或入边）」「无从属叶元素」三种元素调 `plan_element_delete(cascade = false)`
- **THEN** 前两者 `Err`：事实段点名阻塞的子元素 FQN / 关系对；后者成功删净该元素表，其余元素与注释逐字保留、该文件表头计数随动下降

#### Scenario: 关系 upsert 三态与落点

- **WHEN** 对既有 `(from, to)` 调 `plan_relation_upsert` 改 `desc`；对缺席 `(from, to)` 调一次新增；再以 `apis = Some(&[])` 清空该项
- **THEN** 更新情形关系条数不变且 `desc` 为新值；新增情形落在 `from` 元素声明所在文件（回读 `modelFile` 命中）且条目落在该 `from` 组的组尾；清空情形 `apis` 键从源中消失（MUST NOT 写成 `apis = []`）；端点元素缺席 ⇒ `Err` 点名缺失端

#### Scenario: 结构错模型拒绝全部写回

- **WHEN** 模型含任一结构诊断（如 `cycle`）时依次调用四个 `plan_*`
- **THEN** 四次全 `Err`，判据句为「模型存在结构诊断，拒绝写回」、事实段列出诊断种类，零文本产出

#### Scenario: 三条不变量在写回后成立

- **WHEN** 对含 1 处 E2 断裂（`Desktop.Views.Changes` 的三个子元素被追加到整个 `Views` 组之后）的 `06-frontend` 形态源做一次「`gate` 改成其当前值」的写回；另对六个合规文件各做一次同样的写回
- **THEN** 前者产出满足 E1 / E2 / R1：三个子元素段移到 `Desktop.Views.Changes` 之后、`Config` … `Welcome` 逐字后移，`reordered = { moved: 3, blocks: ["Desktop.Views.Changes.Flow", "Desktop.Views.Changes.Renderers", "Desktop.Views.Changes.Agent"] }`、`changed = []`；后者六份产出与源**逐字节相等**、`reordered.moved = 0`（规范化对健康文件是 no-op）

#### Scenario: 规范序序敏感且幂等

- **WHEN** 对同一份含多级子元素与多组关系的源连续调两次规范化路径；另把同层两个兄弟元素的表在源中对调后再排一次
- **THEN** 两次结果逐字节相等（幂等）；对调兄弟后输出保留对调后的同层序（同层序由输入决定、MUST NOT 按字典序重排）；每个块的内部字节（键序 / 注释 / 空行 / decor）与源块逐字相等

#### Scenario: 关系面组连续且两处同源

- **WHEN** 对一份「同一 `from` 分裂成两段」的源做一次关系 upsert（新增的 `to` 追加在条目数组尾部）；随后读回该文件的关系序列
- **THEN** 产出中该 `from` 的两段并成一段、新条目落该段组尾、组内其余条目相对序不变、组间序按各 `from` 首次出现位置；读回的关系序列与文本中的关系序逐项相等（`ArrayOfTables` 的 Vec 序与 `position` 序同源，未出现「渲染序 ≠ 读回序」）

#### Scenario: 搬迁块有界且真值不截断

- **WHEN** 构造一个搬迁块数 > 8 的源（多个父元素各有后代被追加到文件尾）并做一次写回
- **THEN** `reordered.moved` 为真实块数（不被截断），`reordered.blocks` 恰 8 项、按文件序取前 8 个、不含任何「…等 N 项」文本；成功响应序列化长度 < `MAX_RESULT_BYTES`（典型 FQN 下 < 3 KB）

#### Scenario: 序违例不产诊断也不失败

- **WHEN** 以一份含 E2 违例的源调任一 `apply_*`；随后取该文件的结构诊断与写面 `warnings`
- **THEN** 调用返回 `Ok`（序违例不是 `Err`——规范化没有失败臂）；结构诊断零新增（`DiagnosticKind` 无序类目）、`warnings` 不因搬迁新增条目；`finish` 的自证链里零序断言（美观缺陷不得升级为写入失败）

## Module Contract

### Function: `crates/core/archi/src/edit/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `edit/layout.rs`（新） | 新增 | 写入序规范化纯函数：元素面按 DFS 目标序**重编号 `position`**（元素区 `1..k`、关系区 `k+1..N`）、关系面按 R1 **重排 `ArrayOfTables` 成员并同批重编号**；`Reorder { moved, blocks }` 与 `REORDER_BLOCKS_CAP = 8`（cap 在构造处施加，`moved` 保留真值）；零 IO、零 `Err` 臂、零诊断构造、零 `drift::check` |
| `edit/mod.rs` | 改 | 四个 `apply_*` 在改完 AST 后、`finish` 前调用规范化，产出由 `String` 扩为「文本 + `Reorder`」；`WriteOutcome` 增 `reordered`；`plan_*` 装配该字段。`finish` / 自证 / 回读 / 表头回填 / 三态 / 落点 / 级联 / `expect` 逐字不动 |
| `edit/messages.rs` | 零改动 | `EditErrorKind` 仍 **23** 值：规范化无 `Err` 臂，`ALL` / `criterion()` 逐字不变 |
| `lib.rs` | 改 | 出口补 `Reorder`（`apply_*` / `WriteOutcome` 出口面不变） |

### API: 出线面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| IPC 九命令 | 零改动 | 命令清单 / 签名 / `Result<T, String>` 错误面逐字不变；`WriteOutcome` 与 `Reorder` 是进程内类型，**不出线** |
| `bindings.ts` / `bindings.zod.ts` | 零 diff | 零 DTO 字段变化 ⇒ `bindings:export` 应零差异（出现 diff 即误改） |
| DTO 投影（`ArchiGraph` / `ArchiElement` / `ArchiModuleDetail` 等） | 零改动 | 字段名 / 顺序 / 语义逐字不变；`modelLine` 跟着文本走是**正确行为**（不新增投影逻辑） |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| 画布 `incremental` 改档 | 仍走「一个显式写命令 + 写前校验 + 渲染自证 + 原子写回」；规范化内嵌于同一 `plan_element_upsert`，命令面与交互零感知（首次写到含历史错位的文件时可能夹带搬家，`06` 已在本变更内人工搬迁清零） |
| 契约面板 / 门禁 / 文件树 / 漂移报告 | 判据与报告结构零变化；`modelHash` 随模型字节变（`06` 搬迁）是运行期事实，不是判据变化 |
| `.dev-team/architecture/graph/INDEX.toml` / `models/**` / `.i.rs` / `.i.ts` | 零 diff（生成物按 FQN，与文件内块位置无关） |
