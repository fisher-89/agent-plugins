# desktop-archi-model Specification（delta: archi-edit-engine-primitives）

## Purpose

本 delta 把「gate 档写回纯函数」泛化为**模型写回原语族**，并把 IPC 九命令里 `archi_set_gate` 的写面口径补齐一处既有副产物：

- **原语族两面**：纯函数 `apply_*`（源文本进 / 改写文本出）+ 计划函数 `plan_*`（读盘定位 + 纯函数 + 待落盘产物 `ModelWrite { file, content, hash }`）；gate 改档退化为 `ElementSpec { gate }` 的一次调用，gate 专用入口（`set_gate` / `plan_set_gate` / `GateWrite`）全部离场。
- **四条新语义**：三态（`None` 不动 / `Some(None)` 清空 / `Some(Some(v))` 设值）、`expect` 乐观锁、表头计数按重解析回填、删除 fail-closed（有子 / 有关系即拒绝）。渲染、排版保留、自证可解析三条基线纪律逐字保留，并加强为「自证对象 = 交付文本」。
- **口径纠偏两处**：① `kind` / `gate` / `api` 的非法值由封闭枚举 + IPC 反序列化拒绝（基线「纯函数层以非法值 `Err` 并列出三档合法值」的分支在实现里不存在，按实收口）；② 写面 MUST NOT 重写 `path` / `apis` 的形态判据（parser 是唯一判据）。
- 零新增 IPC 命令、零 DTO / `bindings.ts` 变化、零模型数据与判据面变化；`archi` 仍全程只读盘，模型唯一落盘点仍是壳层 `write_atomic`。

## RENAMED Requirements

- FROM: `### Requirement: gate 档写回纯函数`
- TO: `### Requirement: 模型写回原语族（元素 / 关系 upsert 与 delete）`

## MODIFIED Requirements

### Requirement: 模型写回原语族（元素 / 关系 upsert 与 delete）

`archi` SHALL 提供**模型写回原语族**：纯函数层 `apply_*`（源文本进、改写后的完整源文本出），计划层 `plan_*`（读盘定位 + 纯函数 + 待落盘产物）。gate 改档是其中 `ElementSpec { gate }` 的一个退化调用——MUST NOT 保留第二套 gate 专用入口（`set_gate` / `plan_set_gate` / `GateWrite` 均已出射程）：

- **八函数两面**：`apply_element_spec` / `apply_element_delete` / `apply_relation_upsert` / `apply_relation_delete` 为纯函数（零文件系统触碰）；`plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete` 为计划函数（`parse_dir` 结构诊断前置 → 存在性前置 → 读源 → `apply_*` → 填 `hash`），**零写盘**。写盘单点仍是壳层 `write_atomic`。
- **渲染纪律（基线三条逐字保留）**：渲染 SHALL 经 `toml_edit`（解析 → 改 AST → 渲染），MUST NOT 字符串正则替换；改写后注释、空行与其余元素的原始排版 SHALL 原样保留；**既有键序 MUST NOT 重排**——新键追加到元素表尾，新建元素的表按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序整体写出（MUST NOT 为「重排既有元素的键」引入第二份字段序常量）。
- **三态语义**：可清空位（`entry` / `doc` / `summary` / `path` / `metadata` / 关系 `desc` / 关系 `apis`）SHALL 以 `Option<Option<T>>`（或「`Some` 空集合」的同构形态）表达三种意图——`None` = 不动、`Some(None)` = 删键（`path` / `metadata` / `apis` 以 `Some(空集合)` 表达同一意图）、`Some(Some(v))` = 设值。封闭枚举位（`kind` / `gate` / `api`）无清空臂：`Some(GateTier::Off)` 是**显式化缺省**（ratchet 的正当动作），不是清空。MUST NOT 用 `Option<T>` 混过三态，使「不动」与「清空」不可辨。
- **自证对象 = 交付文本**：产出顺序 SHALL 固定为「改 AST → 渲染 → 表头计数回填 → 重解析自证零诊断 → 回读逐字段一致」；重解析出任何诊断、或回读值与规格不一致即 `Err` 且零文本产出——MUST NOT 把坏文本交给调用方落盘，MUST NOT 先验后改（被验的文本必须逐字等于交付的 `content`）。
- **表头计数回填**：目标文件**首部注释块**内的第一处 `（<数字> 元素 / <数字> 关系）`，括号内两个数字 SHALL 按该文件的重解析结果回填；括号外（人工标题与散文）逐字保留；源中无该括号 MUST NOT 新增，首部注释块之外的同形文本 MUST NOT 改动。计数的唯一权威是重解析结果——MUST NOT 另行统计源码行数或表头条目数。
- **乐观锁 `expect`**：`plan_*` SHALL 接受可选的 `expect`（目标文件**当前**内容哈希，`sha256:…` 形态）并在读源之后、渲染之前校验：不匹配即 `Err` 且零文本产出（目标文件缺席而给了 `expect` 同判不匹配；`None` = 不校验，不存在「期望文件缺席」这一臂）。`ModelWrite.hash` SHALL 为**写后**内容哈希，与 `drift::file_hash` 同口径（`ModelWrite.hash` 逐字等于把 `content` 落盘后的 `file_hash(file)`），使调用方能把它直接作为下一次 `expect`。两条写面（画布改档与未来工具面）共用同一校验点，MUST NOT 出现绕过 `expect` 的写面分支。
- **落点（最小版）**：既有元素落其声明所在文件；新建元素的落点 SHALL 由调用方以 `ElementSpec.file` 显式给出（repo 相对、`archi_graph_model_dir` 一级 `*.toml`），MUST NOT 走「最近已声明祖先所在文件」一类推演。新建时 `kind` 必填；父元素缺席（会造出 `orphanHierarchy`）、目标 FQN 已在案、落点逃出模型声明目录、模型声明目录缺席 ⇒ 一律 `Err`。既有元素给出不同 `file` 亦 `Err`（迁文件属独立语义，本能力不做）；写面 MUST NOT 代建目录。删除与关系两侧另有「元素必须存在」前置：缺席即 `Err` 并附现有 FQN 清单或模型元素总数。**新建元素的 FQN 形态**另有一道写面卫生前置：MUST NOT 含空白 / `"` / `[` / `]` / `#` / 换行（写面要把该串渲染成键，含这些字符会产出一个转义后不可读的键）——该前置只作用于**新建**，既有元素的 FQN 形态仍只由 parser 判，写面 MUST NOT 借此对既有值重做形态校验。
- **删除 fail-closed**：`plan_element_delete` SHALL 在「该元素仍有已声明子元素」或「仍被任一关系引用（出边或入边）」时 `Err` 并点名阻塞者；级联删除语义不在本能力射程——MUST NOT 用恒被忽略的布尔参数假装支持。
- **关系落点**：新关系 SHALL 落 `from` 元素声明所在文件；`from` / `to` 端点元素缺席即 `Err` 并点名缺失端；既有 `(from, to)` 走更新（改 `desc` / `apis`）而非重复追加。
- **值域与形态判据归既有单点**：`kind` / `gate` / `api` 的非法值由封闭枚举 + IPC 反序列化拒绝（列全合法值的文案归 serde），写面 MUST NOT 重写一份值域表；`path` / `apis` / `entry` / `doc` 的形态仍由 parser 判（写面 MUST NOT 重写第二份形态判据），越界值经自证链以结构化诊断落地并令写回失败。
- **错误面**：所有失败 SHALL 为封闭枚举 `EditErrorKind`（附 `ALL`）+ 事实段组成的 `EditError`，`message()` 形态恒为「判据句：当前事实 + 下一步」；壳层经 `EditError::message()` 映射进既有 `Result<T, String>` 错误面。MUST NOT 出现裸 `String` 错误的第二路径；`ALL` 与 `criterion()` 的 `match` 分支集合 SHALL 逐值相等（新增变体未同批补 `ALL` 即测试红）。
- 该族 MUST NOT 触碰文件系统的**写**动作（读盘归 `plan_*`，写盘归命令层）；`archi` 内 MUST NOT 出现写盘调用。判定域既有边界不变：零 spawn、零 Tauri、零 store、零 `workflow` 依赖。

#### Scenario: 改档保留注释与其余内容

- **WHEN** 对含注释、空行与多个元素的源文本把 `Backend.Core.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 返回文本中该元素 `gate = "strict"`，其余元素、注释、空行与键序逐字保留（唯一例外：首部注释块的 `（N 元素 / M 关系）` 计数括号按重解析结果回填）；再解析该文本得到新档位且零诊断

#### Scenario: 元素缺席分两路

- **WHEN** 以不存在的 FQN 且不给 `kind` 调 `plan_element_upsert`；另以不存在的 FQN 调 `plan_element_delete`
- **THEN** 前者 `Err`（判据句点名新建缺 `kind`，事实段点名该 FQN 不在案——缺席在 upsert 里是**新建分支**，不是错误本身）；后者 `Err`（判据句点名元素不存在，事实段列出模型现有 FQN 清单或元素总数）；两者零文本产出

#### Scenario: 渲染不可解析或回读不一致即不产出

- **WHEN** 渲染路径产出的文本无法被解析器读回，或回读到的字段值与规格不一致
- **THEN** 返回 `Err` 且不产出任何待落盘文本（自证不可解析即失败、回读不一致即失败），调用方零写入

#### Scenario: 三态逐位可辨

- **WHEN** 对同一元素分三次调用 `plan_element_upsert`，依次传 `summary = None`、`summary = Some(None)`、`summary = Some(Some("一句话"))`
- **THEN** 第一次该键字节不变；第二次该键从源中消失；第三次写入新值；`path` / `metadata` 以 `Some(空集合)` 表达清空（回读为空数组 / 空表、键不在源）；`gate = Some(GateTier::Off)` 写显式 `gate = "off"` 行而非删键

#### Scenario: 新建元素按规范化落点与字段序

- **WHEN** 对一个缺席 FQN 调 `plan_element_upsert`，给出 `kind` 与 `file`（模型声明目录一级 `*.toml`）
- **THEN** 该文件新增一张 `[element."<FQN>"]` 表，字段按 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` 规范序写出、回读 FQN 与各字段逐字相符；缺 `kind`、缺 `file`、父元素缺席、FQN 已在案、落点逃出模型声明目录、模型声明目录缺席、FQN 含空白 / 引号 / 方括号 / `#` / 换行——七种情形各返回 `Err` 并点名成因

#### Scenario: 乐观锁不匹配零文本产出

- **WHEN** 以过期的 `expect`（目标文件在取哈希之后已被改动）调用任一 `plan_*`，随后以 `file_hash` 取到的当前值重试
- **THEN** 前者 `Err`（判据句点名期望与实际的摘要），零文本产出；后者 `Ok`，且 `ModelWrite.hash` 与 `content` 落盘后 `file_hash(file)` 的返回值逐字相等

#### Scenario: 表头计数由重解析回填

- **WHEN** 在表头写 `（9 元素 / 10 关系）` 而文件内实为 8 元素 10 关系的临时模型上做一次元素删除；另对计数准确的本仓模型做一次「把 `gate` 改成其当前值」的写回
- **THEN** 前者产出文本的表头变为 `（8 元素 / 10 关系）`、括号外整行逐字不变；后者产出与源**逐字节相等**（计数准确 ⇒ 回填为零差异）；源无该括号时不新增，首部注释块之外的同形文本零改动

#### Scenario: 删除 fail-closed

- **WHEN** 分别对「仍有已声明子元素」「仍被关系引用（出边或入边）」「无从属叶元素」三种元素调 `plan_element_delete`
- **THEN** 前两者 `Err`：事实段点名阻塞的子元素 FQN / 关系对；后者成功删净该元素表，其余元素与注释逐字保留、该文件表头计数随动下降

#### Scenario: 关系 upsert 三态与落点

- **WHEN** 对既有 `(from, to)` 调 `plan_relation_upsert` 改 `desc`；对缺席 `(from, to)` 调一次新增；再以 `apis = Some(&[])` 清空该项
- **THEN** 更新情形关系条数不变且 `desc` 为新值；新增情形落在 `from` 元素声明所在文件（回读 `modelFile` 命中）；清空情形 `apis` 键从源中消失（MUST NOT 写成 `apis = []`）；端点元素缺席 ⇒ `Err` 点名缺失端

#### Scenario: 结构错模型拒绝全部写回

- **WHEN** 模型含任一结构诊断（如 `cycle`）时依次调用四个 `plan_*`
- **THEN** 四次全 `Err`，判据句为「模型存在结构诊断，拒绝写回」、事实段列出诊断种类，零文本产出

### Requirement: IPC 命令面契约（九命令）

命令层 SHALL 提供九个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff、API 求差全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记；元素投影 `ArchiElement` 含 `doc` 模型拷贝，`api = declared` 者恒 `null`；关系投影 `ArchiRelation` 含 `apis` 模型拷贝）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + **`doc`（单值，仅 `api = none` 有值）** + **`api`（并列字段）**；`docs` 与 `warnings` 两个位次已退役 |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（结构诊断非空即拒 / 元素存在 / 渲染自证可解析 / 回读一致）通过后原子写回该模型文件，并顺带把该文件首部注释的「（N 元素 / M 关系）」计数括号按重解析结果回填；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |
| `archi_api_check` | `root` | `ArchiApiCheck`：四类生成物与契约渲染结果的**三类差异**（`mismatched` / `orphaned` / `missing`）+ `enabled` + `meta`；零写盘（契约归 `desktop-archi-contract-panel`） |
| `archi_api_generate` | `root` | `ArchiApiWriteOutcome`：把应生成物写盘（`written` / `unchanged` 两列）；**只增只改、从不删文件**；结构诊断非空 → `Err` 且零写盘 |
| `archi_relation` | `root` + `from` + `to` | `ArchiRelationDetail`（本变更新增）：模型面（`from` / `to` / `description` / `mark` / `reason` / `modelFile` / `modelLine` / `declared`）+ 事实面（`used`）+ 两桶（`usedNotDeclared` / `declaredNotUsed`）+ `occurrences`；**零写盘**、命令层零判定规则（求差调 `drift` 的同一纯函数） |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：架构域不持有专用文档读取命令，原 `archi_read_doc`（关联集成员校验 + 读盘）与关联集路径集函数 `dto::linked_doc_paths` 均已退役。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。`archi_relation` 的两个 FQN 入参 SHALL 显式给定（MUST NOT 由「当前聚焦元素」一类隐式上下文推导），元素或关系缺席即 `Err` 并点名。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_relation` MUST NOT 写盘；写命令 SHALL 恰为两条——`archi_set_gate`（只改 `gate` 单字段 + 该文件表头注释的计数括号回填，零其它字段改写）与 `archi_api_generate`（只写四类生成物，零删除），其余七条 MUST NOT 触碰磁盘。
- **`archi_relation` 是一次按需事实取样，不是一次检查**：它 MUST NOT 组装 / 落盘 `ArchiDriftReport`，MUST NOT 产出 `status` / `passed`，MUST NOT 缓存跨调用结果；扫描面 SHALL 限于 `from` 元素 `path` 下的源文件（机制借 `ScanInput.mode = Incremental` 的 `changed` 白名单，字面语义即「只扫其中的源文件」），看护配置经既有 `drift::config::load_config` 单点读取（MUST NOT 第二份配置解析），目标归属仍走既有 `OwnerIndex`（不解析目标）。`used` SHALL 取该次扫描 `edges` 中 **`(from, to)` 命中**的那些边的 `names` 并集（去重 + 字典序），`occurrences` 取同一批边（复用既有聚合形状）；扫描顺带产出的 `findings` / `unmatchedFiles` / `unresolved` MUST NOT 进入响应。两桶求差 MUST NOT 另写一份：与 drift 对账共用 `drift` 的同一纯函数（零第二套判定）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）；`archi_api_generate` 同理只在 `mode = incremental` 的前端调用路径出现（生成权限归 `desktop-archi-contract-panel`）；`archi_relation` 属只读取数，两处挂载点均可用（呈现归 `desktop-archi-canvas`）。
- 九个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）。
- **`ArchiElement.doc` 的授权进场口径**：判据 / 纳管 / 检索的入参是 `ArchiElement` 而非模型 AST，故 `paths::carrier_path`、`api::doc_facts`、`api::gating_findings` 的 `none` 支与 `check` 的两支求值都只能从该字段取 `doc` 事实——它是**同一事实的投影拷贝**，不是第二个事实源：与 `ArchiModuleDetail.doc` 同值同源（均拷自模型 `doc` 原文），`api = declared` 元素恒 `null`；`ArchiElementBrief` MUST NOT 带 `doc`（薄条目仍只有 `fqn` / `kind` / `gate` / `api` / `summary` 五项），`ArchiApiRef` 逐字不变。
- **`ArchiRelation.apis` 的授权进场口径（本变更新增）**：画布侧需要「这条边声明了哪几个名字」才能在点开前先呈现声明清单，故 `ArchiRelation` SHALL 带 `apis: Vec<String>`（模型字段的拷贝、缺省空数组、**字段不省略**）；它是**同一事实的投影拷贝**，不是第二个事实源，投影 MUST NOT 归一 / 排序以外的改写（顺序由模型原文保留，比对在 `archi` 判定侧做）。`modelFile` / `modelLine` MUST NOT 随本字段一起进 `ArchiRelation`（123 条边的图载荷不为「点开才用的两个字段」付账）——它们只在 `ArchiRelationDetail` 出线。
- **`archi_module` 的 `api` 并列字段**：`ArchiModuleDetail` 的并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_models_dir>/<分层目录…>/<FQN>.md` 约定定位（模型声明迁入 `graph/model/` 后，`models/` 即契约资产目录，契约落其 `<分层目录…>/` 子目录内），文件不存在即 `status = none`（零配置，且「有契约 / 无契约」成为可枚举事实）。字段结构与定位规则定义归 `desktop-archi-api`，本能力只登记「`archi_module` 出参形状扩展、命令层仍零领域规则」。该字段 MUST NOT 承载协议正文（契约正文与模块文档正文同一通路：由调用方经 `read_file` 自取）。
- **`archi_module` 的 `doc` 并列字段**：`ArchiModuleDetail.doc: Option<String>` 为 repo 相对 POSIX 路径单值（无 `title` / `kind` / `source`），**只有 `api = none` 元素会填**；`api = declared` 元素的入口文档走既有 `api.contractPath`，MUST NOT 在 `doc` 里重复出线（否则元素信息面板会出现两条入口文档）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_relation`
- **THEN** 磁盘字节零变化（模型、源码、文档与四类生成物均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: doc 单值与 docs 退役在出参可辨

- **WHEN** 分别对 `api = none` 且声明了 `doc` 的元素、`api = none` 未声明 `doc` 的元素、以及 `api = declared` 的元素调用 `archi_module`
- **THEN** 前者 `doc` 为该路径串；中者 `doc = null`（字段不省略）；后者 `doc = null` 且入口文档只经 `api.contractPath` 出线；三者响应内 MUST NOT 出现 `docs` 数组、MUST NOT 出现 `warnings` 数组

#### Scenario: 元素投影携带 doc 模型拷贝

- **WHEN** 分别取 `archi_graph` 的 `elements[]`、`archi_module` 的 `element`，以及 `archi_module` 省略 `element` 时的薄概览条目（三处都含 `api = none` 且写了 `doc` 的元素与 `api = declared` 的元素）
- **THEN** 前两者的元素对象均含 `doc` 字段：`api = none` 者为模型原文、`api = declared` 者为 `null`（字段不省略），且 `archi_graph` 与 `archi_module` 对同一元素给出同一个值；薄概览条目（`ArchiElementBrief`）零 `doc` 字段（五项不变）

#### Scenario: 关系投影携带 apis 模型拷贝且不带声明行号

- **WHEN** 分别取 `archi_graph` 的 `relations[]` 与 `archi_relation(root, from, to)` 的响应（同一元素对，模型写了 `apis = ["AgentInput", "SessionPanel"]`）
- **THEN** 前者的 `ArchiRelation.apis` 逐字等于模型原文（字段不省略，空则 `[]`），且 MUST NOT 含 `modelFile` / `modelLine`；后者的 `declared` 与前者同值，且另带 `modelFile` / `modelLine`（指向该 `[[relation]]` 的 `from` 键行）

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留（唯一例外：首部注释块的「（N 元素 / M 关系）」计数括号按重解析结果回填，计数本就准确时零差异）；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `<archi_models_dir>/Infra/` 下的实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `doc`）语义零变化，命令总数仍为九个

#### Scenario: 九命令在册且恰两个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰九条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate` / `archi_relation`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 与 `archi_api_generate` 两条

#### Scenario: archi_relation 与 archi_drift 同一两桶

- **WHEN** 对同一 `(from, to)` 分别取 `archi_relation` 的两桶与 `archi_drift` 报告中该元素对的条目（模型写了三项目、代码只用其中两项且多用了一项，`from.gate = warn`）
- **THEN** `archi_relation` 的 `declaredNotUsed` 恰为该一项、`usedNotDeclared` 恰为多出的那一项，与 `archi_drift` 的 `observations[].kind = unusedDeclaration` + `warnings[].kind = undeclaredApi` 逐名一致；`archi_relation` 的响应内零 `status` / `passed` 字段

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/edit/`（新目录，替代 `edit.rs`） | 写回原语族（本变更的引擎面） | `apply_element_spec` / `apply_element_delete` / `apply_relation_upsert` / `apply_relation_delete`（纯函数，零文件系统）+ `plan_element_upsert` / `plan_element_delete` / `plan_relation_upsert` / `plan_relation_delete`（`parse_dir` 结构诊断前置 → 存在性前置 → 读源 → `apply_*` → 填 `hash`，零写盘）；`ModelWrite { file: PathBuf, content: String, hash: String }`；`ElementSpec`（`fqn` + 八个可写字段的三态 + `file` + `expect`，`Default` 可缺省构造）；表头计数回填、自证与回读都作用在交付文本上；零写盘、零 spawn、零 Tauri |
| `crates/core/archi/src/edit/messages.rs`（新） | 文案族（从 `edit.rs` 的内联 `format!` 抽出） | `EditError { kind: EditErrorKind, facts: String }`、`EditErrorKind::criterion() -> &'static str`、`EditErrorKind::ALL`、`EditError::message() -> String`（恒为「判据句：事实」）；判据句操作中立（零「改档」字面）；`ALL` ↔ `criterion()` 的 `match` 分支集合逐值相等 |
| `crates/core/archi/src/drift/mod.rs` | 哈希单点（本变更 +2 个薄封装，既有面零改动） | `pub fn file_hash(path: &Path) -> String` 与 `pub fn file_hash_of(path: &Path, content: &str) -> String`，两者都经**私有** `hash_files`（可见性不放宽、零独立 Sha256 拼装）；`model_hash` / `assets_hash` 的覆盖面与语义逐字不变 |
| `crates/core/archi/src/lib.rs` | 出口面 | `pub mod edit;` 保留、`#[cfg(test)] mod edit_test;` 删（测例随目录化迁入 `edit/mod.rs`）；`pub use` 补 `ModelWrite` / `ElementSpec` / `EditError` / `EditErrorKind` / `file_hash` / `file_hash_of`；既有出口逐字不变 |
| `crates/core/archi/src/edit/mod_test.rs`（由 `edit_test.rs` 迁入并扩写） | 引擎断言面 | 既有 12 条用例改走 `plan_*`（排版保留 / 注释保留 / 500 元素规模 / CRLF / 注入式 FQN / 三档往返逐条保留）；新增三态逐位、`expect` 三态、表头回填四例、删除 fail-closed 三例、关系 upsert / delete 三态与落点、新建元素六拒、`ModelWrite.hash` 与落盘后 `file_hash` 等价、结构错模型四拒、plan 零写盘 |
| `crates/core/archi/src/edit/messages_test.rs`（新） | 文案对拍面 | `ALL` 与 `criterion()` 的穷尽 `match`（新增变体不补 `ALL` 即测试红）、逐值非空且互异、`message()` 形态「判据句：事实」、判据句零「改档」字面 |
| `src-tauri/src/commands/archi/mod.rs` | 写面薄包装（语义零变化） | `archi_set_gate_inner` = root 校验 → `plan_element_upsert(ElementSpec { fqn, gate: Some(gate), ..Default::default() })` → `EditError::message()` 映射 → `write_atomic`；签名 / `Result<(), String>` 错误面 / 失败零写入逐条不变；文件头注释里 `archi::edit::plan_set_gate` 的指称改为原语族 |
| `src-tauri/src/commands/archi/mod_test.rs` | 源文本守卫随动 | `archi_file_tree与唯一写命令的边界` 的负断言 needle 由退役的 `plan_set_gate` 换为 `plan_element_upsert`（MUST NOT 留一条对已删名字的恒真空断言）；`archi_set_gate` 相关用例的文案断言按新判据句核对（`拒绝写回` 一处）；其余零改动 |

### API: 对外面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiGraph` / `ArchiModuleDetail` / `ArchiRelationDetail` / 其余 DTO | 零改动 | 字段名 / 顺序 / 语义与停摆态形状逐字不变；`dto.rs` 不进入本变更改动集 |
| `archi_set_gate` 签名与错误面 | 零改动 | `(root, element, gate) -> Result<(), String>` 逐字不变；错误文案按闭枚举判据句重述（IPC 契约面仍只承诺「`Err(String)` + 文案含原因」） |
| 命令清单与出线类型清单 | 零改动 | `all_commands!` 仍九条、`bindings/mod_test.rs` 的条数断言不动、`bindings.ts` / `.zod.ts` 零 diff（`EditError` / `ModelWrite` / `ElementSpec` 不带 `specta::Type`，不出线） |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| 画布 `incremental` 改档 | 仍走「一个显式写命令 + 写前校验 + 渲染自证 + 原子写回」；成功重取 / 失败零写入零变化。表头回填在本仓（计数准确）为零差异，画布零感知（口径归 `desktop-archi-canvas`） |
| 其余七个 archi 命令 | 只读面与生成面零改动；零新增 / 删除命令 |
| `.dev-team/architecture/**` 与四类生成物 | 零字节变化：`api:check` 零差异、`api:export` 零 diff、`modelHash` 逐字不变 |
| archi 知识文本 / `SCAFFOLD_TEMPLATE` / phase prompts | 零改动（写工具面归主篇切片 A，本变更不加任何工具） |
