# 提案: archi-edit-engine-primitives

> **变更**: archi-edit-engine-primitives（架构写引擎原语 · `edit` 模块泛化）
> **日期**: 2026-11-24
> **状态**: draft
> **来源**: 本 change 目录 `explore.md` 全篇（主篇 `archi-agent-contract-knowledge-gap` 第四节切片 A 的前置片 P3；主篇本身未入库，其结论经 explore 转述）。
> **性质**: **纯引擎演进**——把 `archi::edit` 的单函数（gate 改档）泛化为写回原语族（4 × `apply_*` + 4 × `plan_*`），并补齐三件既有缺口：文案族封闭枚举、单文件内容哈希、写面乐观锁与表头计数回填。零新增 IPC 命令、零线面（DTO / `bindings.ts`）变化、零模型数据变化、零判据面变化。
> **不含**: agent 工具定义 / 注入矩阵 / 知识文本 / phase prompt（主篇切片 A）；`ArchiModuleDetail.modelHash`（线面变化，A 一并接线）；落点策略（最近已声明祖先）与删除级联语义（A）；`archi` 内的任何写盘（写盘单点仍是壳层 `write_atomic`）。

---

## 问题

### 一 · 写面只有一条退化路径，泛化无处落脚

`archi::edit`（89 行）的公开面恰三件：`set_gate(source, fqn, gate) -> Result<String, String>`、`plan_set_gate(root, fqn, gate) -> Result<GateWrite, String>`、`GateWrite { file, content }`。唯一消费者是壳层 `archi_set_gate_inner`（`commands/archi/mod.rs:144`）。任何元素级 / 关系级写回（A 的工具面在等）都必须把「改 AST → 渲染 → 自证可解析 → 回读一致 → 定位文件 → 读源」六步重写一遍；六步里前四步是纯引擎工作，与被改字段无关。

### 二 · 三件单点能力缺口

| # | 缺口 | 现状锚 |
|---|---|---|
| ① | **文案族无载体** | 10 条错误串内联在 `edit.rs` 的 `format!` / `ok_or_else` 里（如「元素不存在：{fqn}（现有元素：…）」「模型存在 {} 条结构诊断，拒绝改档」）。新增一条写面判据就只能再写一份措辞，无法对拍「变体集合 == 文案集合」 |
| ② | **单文件内容哈希缺席** | 累加实现 `hash_files`（`drift/mod.rs:580`）是私有的，只有两个目录粒度消费者（`model_hash` / `assets_hash`）。写面要判「我读到的是否还是我要改的那份」时没有可比对的单值 |
| ③ | **写面乐观锁缺席** | 画布（`incremental` 改档）与 A 的工具面是两条写面；今天两者都只有「读 → 渲染 → 写」的裸顺序，中间的 TOCTOU 窗口无人管。三态语义（省略 = 不动 / 清空 / 设值）也无处表达——`Option<T>` 混用会让「不动」与「清空」不可辨 |

### 三 · 表头计数复述的手工半边

`repo-archi-model`「计数复述单源」要求模型文件表头注释（`# 02 · core 分层（9 元素 / 10 关系）`）与模型本体在同一次改动内同值。实测六个文件的括号内计数与文件内的元素 / 关系条数逐文件相等（`2/0`、`9/10`、`8/25`、`1/14`、`3/3`、`32/77`）——**这是一份完全由重解析结果决定的事实，却被人工抄写**。写面每加一次元素 / 关系，就多一处待人工核对的陈旧计数。

### 四 · 一处 spec 与实现不符（顺带纠偏）

`desktop-archi-model` 的 gate 写回要求写着「目标值 ∈ `off` / `warn` / `strict`（非法值即 `Err`）」并配 scenario「以非法值调用 ⇒ `Err` 并列出三档合法值」。实现里 `GateTier` 是封闭枚举，非法值在 IPC 反序列化阶段即被拒（列全合法值的是 serde 文案）——纯函数层**不存在**这条值域分支。本次重写该要求时把这条口子按实收口，MUST NOT 在写面重写第二份值域表。

---

## 提案

### 一 · 错误面：封闭枚举 + 判据句 + 事实

```rust
pub struct EditError { pub kind: EditErrorKind, pub facts: String }
impl EditError { pub fn message(&self) -> String }        // "{判据句}：{当前事实}"

pub enum EditErrorKind { /* 封闭集合；见下 */ }
impl EditErrorKind {
    pub fn criterion(self) -> &'static str;                // 判据句（操作中立，不含「改档」）
    pub const ALL: [EditErrorKind; N];                     // 与 match 分支集合对拍
}
```

- **判据句与事实分家**：`criterion()` 是「哪条判据失败」，`facts` 是「当前事实 + 下一步」（缺席元素的现有清单、回读到的值、阻塞删除的子元素 / 关系对、期望与实际的摘要）。
- **封闭集合的两组**：**前置组**（结构诊断非空、元素不存在、元素非表形态、父元素缺席、关系端点缺席、落点非法、新建缺 `kind`、新建时 FQN 已在案、FQN 形态非法、源文件读取失败、源不可解析、缺 `element` 表、`expect` 不匹配、删除仍有子元素、删除仍有关系、关系不存在）与**自证组**（渲染自证失败、回读不一致）。两组都以 `EditErrorKind` 的变体承载，MUST NOT 出现「裸 `String` 错误」的第二条路径。
- **对拍纪律**：`ALL` 与 `criterion()` 的 `match` 分支集合逐值相等（穷尽 `match` 让新增变体在**编译期**就被测试文件发现）；`criterion()` 逐值非空且互异。
- **文案纠偏**：判据句里的「改档」一律改操作中立措辞（基线 10 条文案里有 5 条含「改档」：`拒绝改档` / `改档渲染自证失败`×3 / `无法改档`）。这是一次**知情文案变更**（见 R-7），与 explore「文案逐字不变」的验收条偏离，逐条对照见 AC-7。

### 二 · 哈希：两个薄封装，一份累加

```rust
// crates/core/archi/src/drift/mod.rs（hash_files 保持私有）
pub fn file_hash(path: &Path) -> String                  // 读盘 → 累加
pub fn file_hash_of(path: &Path, content: &str) -> String // 字节由调用方给（写面用）
```

- 两者都经既有私有 `hash_files`（`hash_files(&[path], base)`），**MUST NOT 另写一份 Sha256 拼装**；`hash_files` 的可见性不放宽。
- **文件名参与累加**（既有口径），故值与 `modelHash` / `assets_hash` **不可直接比较**——文档与 spec 都写明这一点，免后人以为三者可比。
- 写面等价锚：`ModelWrite.hash` == 把该 `content` 落盘后 `file_hash(file)` 的值。这条等价是 `expect` 乐观锁能闭环的唯一前提，用测试钉住。

### 三 · 原语族：4 × `apply_*`（纯）+ 4 × `plan_*`（读盘）

沿既有「纯函数 + plan 包装」双层约定（`set_gate` / `plan_set_gate` 的同一形状，MUST NOT 自造第三种形状）：

| 函数 | 签名 | 语义 |
|---|---|---|
| `apply_element_spec` | `(source: &str, spec: &ElementSpec) -> Result<String, EditError>` | 单文件内元素 upsert（缺席即按 `kind` 新建） |
| `apply_element_delete` | `(source: &str, fqn: &str) -> Result<String, EditError>` | 单文件内删元素表 |
| `apply_relation_upsert` | `(source: &str, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>) -> Result<String, EditError>` | 单文件内 upsert 一条 `[[relation]]` |
| `apply_relation_delete` | `(source: &str, from: &str, to: &str) -> Result<String, EditError>` | 单文件内删一条 `[[relation]]` |
| `plan_element_upsert` | `(root: &Path, spec: &ElementSpec) -> Result<ModelWrite, EditError>` | 定位 + 读源 + `apply_*` + 填 `hash`，**零写盘** |
| `plan_element_delete` | `(root: &Path, fqn: &str, expect: Option<&str>) -> Result<ModelWrite, EditError>` | 同上；有子 / 有关系即拒绝 |
| `plan_relation_upsert` | `(root: &Path, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>, expect: Option<&str>) -> Result<ModelWrite, EditError>` | 同上；新关系落 `from` 元素所在文件 |
| `plan_relation_delete` | `(root: &Path, from: &str, to: &str, expect: Option<&str>) -> Result<ModelWrite, EditError>` | 同上 |

```rust
pub struct ModelWrite { pub file: PathBuf, pub content: String, pub hash: String }

#[derive(Debug, Clone, Default)]
pub struct ElementSpec {
    pub fqn: String,
    pub kind: Option<ElementKind>,          // 新建必填
    pub gate: Option<GateTier>,
    pub api: Option<ApiStatus>,
    pub entry: Option<Option<String>>,      // Some(None) = 清空
    pub path: Option<Vec<String>>,          // Some(vec![]) = 清空
    pub doc: Option<Option<String>>,
    pub summary: Option<Option<String>>,
    pub metadata: Option<BTreeMap<String, String>>,
    pub file: Option<String>,               // 新建必填；既有元素只允许同值（换文件属 A）
    pub expect: Option<String>,             // 目标文件的当前内容哈希（sha256:…）
}
```

`file` 与 `expect` 两个「落点 / 并发控制旋钮」跟着 `ElementSpec` 走（元素写回的入参对象）；关系侧两函数参数本来就少，`expect` 以末位形参承载同一语义。两种承载位置背后是**同一校验点**——MUST NOT 出现第三种位置。

壳层的调用面收缩为一段：

```rust
let spec = ElementSpec { fqn: element.into(), gate: Some(gate), ..Default::default() };
let plan = plan_element_upsert(root, &spec).map_err(|error| error.message())?;
write_atomic(&plan.file, &plan.content)
```

### 四 · 五条实现约束

1. **`Option<Option<T>>` 三态**：`None` = 不动、`Some(None)` = 清空（删键）、`Some(Some(v))` = 设值（写键）。`path` / `metadata` / `apis` 用「`Some` 空集合 = 删键」同构表达（MUST NOT 写出 `apis = []` 一类被 parser 判 `invalidValue` 的形态）。**`gate` / `kind` / `api` 无「清空」臂**——它们是必填 / 有缺省的封闭枚举，`Some(Off)` 是「显式化缺省」（ratchet 的正当动作），不是清空。MUST NOT 用 `Option<T>` 混过三态。
2. **`expect` 校验在 `plan_*` 入口**（读完源、渲染之前）：不匹配即 `Err` 且**零文本产出**；目标文件缺席 + `Some(_)` 同判不匹配。这样画布与 A 的工具两条写面**要么都受保护、要么都不受**。
3. **`ModelWrite.hash` 在 `plan_*` 内部算**（它已持有最终文本）：MUST NOT 让执行体写完再读一次盘（那是新的 TOCTOU 窗口）。
4. **表头计数回填**：目标文件**首部注释块**里的第一处 `（<数字> 元素 / <数字> 关系）`，括号内两个数字按重解析结果回填；括号外（人工标题与散文）逐字保留。MUST NOT 新增该括号、MUST NOT 改动首部之外的任何同形文本。**副作用知情**：`archi_set_gate` 从此也会顺带修正陈旧表头（本仓六个文件计数当前准确 ⇒ 今日零差异，见 AC-5）。
5. **新建 FQN 的写面卫生前置**：新建元素的 FQN MUST NOT 含空白 / `"` / `[` / `]` / `#` / 换行（写面要把它渲染成键，这些字符会产出转义后不可读的键）。**只作用于新建**——既有元素的 FQN 形态仍只由 parser 判，MUST NOT 借这道前置对既有值重做形态校验（那是第二份判据）。

### 五 · 键序裁决：保序（否决 explore 的「统一序」倾向）

explore 倾向「一律按 `RawElement` 字段序重排该元素的键」，本提案**裁为保序**：

| 取向 | 内容 | 裁决 |
|---|---|---|
| 保序 | 既有键原位不动（含行内注释 / 缩进 / 相对序）；新键追加到元素表尾；**创建**新元素时按规范字段序整体写出 | ✅ 采纳 |
| 统一序 | 每次写回把该元素的键按规范序重排 | ❌ 否决 |

否决三条理由：① 统一序需要一份「规范字段序」常量，与 `RawElement` 的声明序构成**第二事实源**（本仓反复禁止的第二份持有者），保序需要零份；② 统一序推翻既有多条断言与 spec 承诺（`edit_test` 的「写面零重排」、`desktop-archi-model` scenario 的「键序逐字保留」），保序零推翻；③ 保序的 diff 最小，正是「同输入产同文本」验收想守的性质。

代价（已知）：给既有元素新增字段会追加到表尾，可能与规范序不一致（纯外观，无语义影响）；创建路径按规范序写出（模板序 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）。

### 六 · 自证对象 = 交付文本

重算表头**在自证之前**：`apply_*` 的产出顺序固定为「改 AST → 渲染 → 表头回填 → 重解析自证零诊断 → 回读逐字段一致」，任一失败即 `Err` 且零文本产出。纪律：**被自证 / 被回读的文本必须逐字等于交付给调用方的 `content`**——MUST NOT 先验后改（验完再动注释同样是「验的不是交付物」）。

### 七 · 与 explore 的偏离

| explore 草案 | 本提案 | 理由 |
|---|---|---|
| 验收 #1「既有错误串逐字不变」 | 判据句里的「改档」改操作中立（基线 10 条文案中 5 条含该词），逐条对照见 AC-7 | 通用引擎里写「改档」，关系 / 元素写回失败时会误导读文案的一方；改动面收敛到 5 条判据句 + 1 处既有测试断言 |
| `ElementSpec.relations: Vec<RelationSpec>` | 元素 spec **不含** relations；关系走两条独立原语 | 单函数单职责（一棵 AST 一次自证）；关系落点（`from` 元素所在文件）不再需要跨节点决策；A 若要「单次写内原子补边」，另立组合层（见 R-6） |
| `plan_element_delete(..., cascade: bool)` | 无 `cascade` 入参；有子 / 有关系即 fail-closed 拒绝并点名阻塞者 | 一个恒被忽略的布尔是谎；级联语义属 A 的语义层，P3 只给「拒绝 + 出路」 |
| `GateWrite { file, content, hash }` | 改名 `ModelWrite`（`{ file, content, hash }`） | 类型不再只服务 gate；连带追改 `desktop-config-agent` 里引用退役类型名的一处句子（该 spec 的 `ConfigWrite` 形状本身零变化） |
| §3.4 倾向「统一序」 | 保序（见上节） | 见五 |
| 「`messages.rs` 抽取」（P3a） | 采纳，且 `edit.rs` → `edit/` 目录（`mod.rs` + `messages.rs` + 测例） | 沿 `api/` `drift/` `parse/` `scan/` 的既有目录约定；文案族与渲染分家 |
| `file_hash` 落 `edit.rs` 或 drift 邻居；`hash_files` 提可见性 | `drift::file_hash` / `drift::file_hash_of`；`hash_files` 保持私有 | 哈希口径的持有者是 `drift`；不加宽私有 helper 的可见性，零新暴露 |
| 可选同期：`ArchiModuleDetail.modelHash` + `bindings.ts` 重导 | **不含** | 线面变化且 P3 无消费者；`file_hash` 就位后由 A 一并接线（本变更的「`bindings.ts` 零 diff」因此可作硬判据） |
| `set_gate` / `plan_set_gate` 保留为退化入口 | 两者删除（`grep` 零命中），gate 改档 = `ElementSpec { gate }` 的一处调用 | 双入口 = 同一语义两份持有；退化入口没有独立行为可测 |

### 八 · 本仓读数（落地当天应成立）

| 项 | 变更前 | 变更后 |
|---|---|---|
| `model.elements` / `model.relations` | 55 / 129 | **55 / 129**（零数据变化） |
| `model.warnings` / `model.structuralErrors` | 0 / 0 | **0 / 0** |
| `apiFindings` / `observations` | 39 / 53 | **39 / 53** |
| `status` | `clean` | **`clean`** |
| `modelHash` | `sha256:6cc50dbf…` | **逐字不变**（零模型字节变化） |
| 六个模型文件表头计数 | 2/0、9/10、8/25、1/14、3/3、32/77 | **逐字不变**（计数本就准确 ⇒ 回填为零差异） |
| `bindings.ts` / `bindings.zod.ts` | — | **零 diff**（零线面变化） |
| `api:export`（`INDEX.toml` / `.schema.json` / `.i.*`） | — | **零 diff** |
| 语料与金样（`crates/core/archi/tests/**`） | — | **逐字节不变** |

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-model` | **RENAMED ×1 + MODIFIED ×2** | ① RENAMED：「gate 档写回纯函数」→「模型写回原语族（元素 / 关系 upsert 与 delete）」，随后按新名整段 MODIFIED：三态语义、`expect` 乐观锁、表头计数回填、落点最小版、删除 fail-closed、错误面封闭枚举、`ModelWrite` 三元组、值域校验归封闭枚举（纠偏）；既有三条 scenario 保留并按新名改写，新增七条（三态 / 新建 / 乐观锁 / 表头回填 / 删除 / 关系 / 结构错）。② MODIFIED「IPC 命令面契约（九命令）」：`archi_set_gate` 行与「写命令恰为两条」的括注补「表头计数括号回填」，scenario「gate 写回原子与零残留」的「逐字保留」收窄为「唯一例外 = 表头计数括号」。 |
| `desktop-archi-drift` | **ADDED ×1** | 新增 requirement「单文件哈希（写面乐观锁粒度）」：`file_hash` / `file_hash_of` 两薄封装、共用 `hash_files`、零独立 Sha256 拼装、文件名参与累加 ⇒ 与 `modelHash` / `assets_hash` 数值不可比、写后等价锚（`ModelWrite.hash` == 落盘后 `file_hash(file)`）。 |
| `desktop-config-agent` | **MODIFIED ×1** | 「文件面写入器 `to_file_value`（模板驱动）」的「公开面配套」条：`ConfigWrite` 沿用的形状引用由退役的 `archi::edit::GateWrite` 改为 `archi::edit::ModelWrite` 的 `{ file, content }`（`hash` 不进入 `ConfigWrite`）；该要求的其余内容逐字保留，`ConfigWrite` 形状与实现零变化。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-canvas` | 改档仍走「一个显式写命令 + 写前校验 + 渲染自证 + 原子写回」；成功重取 / 失败零写入逐字不变。表头回填是本仓计数准确下的零差异，画布零感知 |
| `desktop-archi-gate` / `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-file-tree` / `desktop-archi-drift` 既有面 | 判据、投影、生成链、报告结构逐字不变；本变更不动 `dto.rs` / `report.rs` / `parse/` / `model.rs` |
| `desktop-archi-contract-panel` | 写面范围契约定在 `desktop-archi-model` 的 IPC requirement（本变更已改），该能力的旁述「第一条仍是只改 `gate` 单字段的 `archi_set_gate`」是**识别句**而非写面契约——且元素改档时计数不变 ⇒ 回填对它是 no-op，措辞仍成立，故不追改；同段「两条之外的六条命令」是九命令基线之前的既有陈旧计数（应为七条），属既有漂移，不在本变更追改面（沿「历史读数不追改」的既有处置） |
| `desktop-archi-agent` | 零工具面变化：本变更**不加任何 agent 工具**（`desktop-archi-agent` 的「MUST NOT 为写模型新增专用工具」逐字有效）；`archi` crate 仍全程只读盘，模型唯一落盘点仍是壳层 `write_atomic` |
| `desktop-ipc-type-bindings` | 命令签名与出线类型零变化（`EditError` / `ModelWrite` / `ElementSpec` 不带 `specta::Type`、不出线）；`bindings.ts` 零 diff |
| `repo-archi-model` | 零数据变化；「计数复述单源」的三处（表头 / 计数表 / ADR）读数逐字不变。表头自动回填是写面行为（归 `desktop-archi-model`），不改变该数据纪律 |
| `desktop-app-shell` | 九命令与同步 / 异步形态零变化；`archi_set_gate` 仍在「明确不在适用面」的毫秒级写清单内 |
| `desktop-corpus-regression` / `desktop-workspace-*` / 前端各能力 | 与写引擎原语无关 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit.rs` | **删除**（拆为 `edit/` 目录；`set_gate` / `plan_set_gate` / `GateWrite` 三件随之离场） |
| `packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs`（新） | 4 × `apply_*`（`toml_edit` 改 AST → 渲染 → 表头回填 → 重解析自证 → 回读一致）+ 4 × `plan_*`（`parse_dir` 结构诊断前置、元素 / 父 / 端点存在性、落点校验、`expect` 校验、读源、填 `hash`）；`ModelWrite` / `ElementSpec`；零写盘、零 spawn |
| `packages/desktop/src-tauri/crates/core/archi/src/edit/messages.rs`（新） | `EditError` / `EditErrorKind` / `criterion()` / `ALL` / `message()`；判据句操作中立、事实段承载「当前事实 + 下一步」 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` | 增 `pub fn file_hash` / `pub fn file_hash_of`（都经私有 `hash_files`）；既有 `model_hash` / `assets_hash` 逐字不动 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | 模块登记随动（`pub mod edit;` 保留、`#[cfg(test)] mod edit_test;` 删——测例随目录化迁入），`pub use` 补 `ModelWrite` / `ElementSpec` / `EditError` / `EditErrorKind` / `file_hash` / `file_hash_of`；其余出口逐字不变 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | `archi_set_gate_inner` 改调 `plan_element_upsert` + `EditError::message()` 映射（签名 / 返回类型 / 零写入语义不变）；文件头注释里 `archi::edit::plan_set_gate` 的指称改为原语族 |
| `packages/desktop/package.json` | `version` patch +1（`0.7.9` → `0.7.10`） |

### 测试文件

| 文件 | 改动 |
|---|---|
| `.../crates/core/archi/src/edit/mod_test.rs`（由 `edit_test.rs` 迁入并扩写；`edit_test.rs` 同批删除） | 既有 12 条用例改走 `plan_*`（排版保留 / 500 元素规模 / CRLF / 注入式 FQN / 三档往返逐条保留）；新增：三态逐字段（`entry` / `doc` / `summary` / `path` / `metadata` / `gate` / `api` / `kind`）、`expect` 匹配 / 不匹配 / 文件缺席、表头回填（括号内 == 重解析计数、括号外逐字、无括号不新增、非首部同形文本不动）、删除 fail-closed（有子 / 有关系 / 无从属）、关系 upsert / delete 三态与落点、新建元素（规范字段序 + `file` 必填 + 父缺席拒绝 + FQN 形态）、`ModelWrite.hash` == 落盘后 `file_hash(file)`、结构错模型全 `plan_*` 拒绝、临时目录零写盘 |
| `.../crates/core/archi/src/edit/messages_test.rs`（新） | `ALL` 与 `criterion()` 的穷尽 `match` 对拍（新增变体即编译期发现）、逐值非空且互异、`message()` 形态「判据句：事实」、判据句零「改档」字面 |
| `.../crates/core/archi/src/drift/mod_test.rs` | 新增 `file_hash` 族四条：形态（`sha256:` + 64 hex）与同盘稳定、内容变即变、文件名参与累加（同名同内容跨目录相等 / 异名同内容不等）、`file_hash_of` 与落盘后 `file_hash` 等价；既有 `assets_hash` / `model_hash` 用例逐字不动 |
| `.../src-tauri/src/commands/archi/mod_test.rs` | 两处源文本 needle 随动（`archi_file_tree与唯一写命令的边界` 的 `plan_set_gate` → `plan_element_upsert`，避免负断言变空断言）；`archi_set_gate` 用例的文案断言按新判据句核对（含 `拒绝写回` 一处）；其余零改动 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `crates/core/archi/src/parse/**`、`model.rs`、`dto.rs`、`graph.rs`、`drift/report.rs` | 判据面 / 投影面 / 报告形状零变化；写面 MUST NOT 重实现 `path` / `apis` 的形态校验（parser 是唯一判据） |
| 前端产品代码（`packages/desktop/src/**`，含生成物） | 零线面变化；`bindings.ts` / `bindings.zod.ts` 零 diff |
| 其余七个 archi 命令 | 只读面与生成面零改动 |
| `crates/core/archi/tests/**`（语料 / 金样 / 词汇对拍） | 零行为与零数据变化，金样必须零变化 |
| `.dev-team/architecture/**`（模型数据、生成物、ADR） | 零计数 / 零字节变化；`api:export` 与 `api:check` 应零 diff |
| archi 知识文本、`SCAFFOLD_TEMPLATE`、phase prompts | 属主篇切片 A；本变更不加工具、不改知识面 |
| `crates/core/config/**`（`ConfigWrite`） | `desktop-config-agent` 的 delta 只改 spec 里引用退役类型名的一句话，实现零变化 |
| `openspec/specs/**` 基线、`openspec/changes/archive/**`、本 change 的 `explore.md` | delta-only：手改基线即「同一事实两份持有者」；explore 只读 |
| `src-tauri/Cargo.toml` / `tauri.conf.json` | crate 版本行 MUST NOT 跟随 `package.json`（AGENT.md 版本纪律） |
| `AGENT.md` / `CLAUDE.md` | 规范面，本变更不触 |

---

## 验收标准

1. **AC-1 · 公开面恰为原语族**：`archi::edit` 的公开面 = 8 个函数（4 `apply_*` + 4 `plan_*`）+ 4 个类型（`ElementSpec` / `ModelWrite` / `EditError` / `EditErrorKind`）；全仓 `grep` 对 `set_gate`（排除 `archi_set_gate`）、`plan_set_gate`、`GateWrite` **零命中**；`apply_*` / `plan_*` 的签名与提案表逐字一致（注释里的「改档」表述不计入判据）。
2. **AC-2 · `archi_set_gate` 行为等价**：同一临时模型上 `archi_set_gate` 改档 ⇒ 产出的 `content` 与「只把目标行 `gate` 值换掉」的期望文本逐字节相等（按行 diff 恰 1 行）；表头计数准确的源上，`plan_element_upsert(ElementSpec { gate })` 与基线的 `plan_set_gate` 语义等价靠此条断言（本仓六个模型文件按 AC-5 另判）；命令签名、`Result<(), String>` 错误面、失败零写入逐条不变。
3. **AC-3 · 三态逐条成立**：对 `entry` / `doc` / `summary` / `path` / `metadata` / `apis` 六个可清空位，`None` ⇒ 该键字节不变、`Some(None)`（或 `Some(vec![])` / `Some(BTreeMap::new())`）⇒ 该键从源中消失、`Some(Some(v))` ⇒ 该键写入新值；对 `gate` / `kind` / `api` 三个封闭枚举：`None` 不动、`Some(v)` 写线值（含 `Some(GateTier::Off)` 显式化缺省）。反向判据：写面 MUST NOT 产出 `apis = []` 一类被 parser 判 `invalidValue` 的形态。
4. **AC-4 · `expect` 乐观锁**：`expect = Some(当前 file_hash)` ⇒ `Ok`；`expect = Some(错值)` ⇒ `Err` 且**零文本产出**（`plan_*` 返回的 `Err` 里没有任何 `content`）；目标文件缺席 + `Some(_)` ⇒ `Err`；`None` ⇒ 不校验且照常产出。两条写面（画布改档与未来工具面）走同一 `plan_*` 入口、同一 `expect` 校验点：画布当前以 `None` 调用（不上锁），未来工具面给出 `Some(_)` 即受保护——MUST NOT 出现「某条写面绕过 `expect`」的分支。
5. **AC-5 · 表头回填可判**：括号内数字 == 该文件重解析得到的 `elements` / `relations` 条数；括号外整行逐字不变；源中无该括号 ⇒ 不新增；首部注释块之外的 `（数字 元素 / 数字 关系）` 同形文本零改动；对**本仓六个模型文件**跑一次「把某元素 `gate` 改成其当前值」的 plan，`content` 与源逐字节相等（计数准确 ⇒ 回填为零差异）。
6. **AC-6 · 自证链闭合**：结构诊断非空的模型 ⇒ 四个 `plan_*` 全部 `Err`（继承基线 fail-closed）；渲染后重解析出诊断 ⇒ `Err` 且零文本；回读逐字段与 spec 不一致 ⇒ `Err` 且零文本；`apply_*` 内部顺序固定为「改 AST → 渲染 → 表头回填 → 重解析 → 回读」，自证与回读的对象逐字等于交付的 `content`（用一条「回填后计数与回读计数一致」的用例钉住顺序）。
7. **AC-7 · 文案族封闭且操作中立**：`EditErrorKind::ALL` 与 `criterion()` 的穷尽 `match` 分支集合逐值相等；`criterion()` 逐值非空、互异、零「改档」字面；`message()` 形态恒为「判据句：事实」。**基线 10 条文案的逐条归宿**（知情变更，逐条核；含「改档」的 5 条改词、其余 5 条措辞逐字保留）：
   - 改词 5 条：`元素 {fqn} 的声明不是表形态：无法改档` → `…：无法写回`；`改档渲染自证失败（重解析出 N 条诊断），未产出文本` → `写回渲染自证失败：重解析出 N 条诊断，未产出文本`；`改档渲染自证失败：回读档位为 X，目标为 Y` → `回读不一致：档位为 X，目标为 Y`；`改档渲染自证失败：回读找不到元素 {fqn}` → `回读不一致：找不到元素 {fqn}`；`模型存在 N 条结构诊断，拒绝改档（先修好模型）：…` → `模型存在结构诊断，拒绝写回：N 条（先修好模型）…`。
   - 逐字保留 5 条：`模型源解析失败：{error}` / `模型源缺少 element 表：无从定位元素` / `元素不存在：{fqn}（现有元素：…）` / `元素不存在：{fqn}（模型共 N 个元素）` / `模型文件读取失败（{path}）：{error}`。既有测试断言里只有 `拒绝改档` 一处需随动（改 `拒绝写回`）。
8. **AC-8 · 哈希单源**：`file_hash` 输出为 `sha256:` + 64 位小写 hex；同盘连续两次调用逐字相等；内容变即变；文件名参与累加（同名同内容跨目录相等、异名同内容不等）；`file_hash_of(path, content)` 与「把 `content` 落盘后 `file_hash(path)`」逐字相等；`ModelWrite.hash` 与该值逐字相等；`crates/core/archi/src/edit/` 内零 `Sha256` / `sha2` 字面（源码断言，零独立拼装）。
9. **AC-9 · 删除 fail-closed**：元素有已声明子元素 ⇒ `plan_element_delete` `Err` 且 facts 点名列出的子元素 FQN；元素仍被任一关系引用（出边或入边）⇒ `Err` 且点名关系对；无从属叶元素 ⇒ 删除成功、该文件表头计数随动下降、`content` 内该元素表与其余内容逐字保留（其余元素 / 注释 / 空行零改动）。
10. **AC-10 · 关系原语**：新关系落 `from` 元素所在文件（模型面 `modelFile` 回读命中）；既有 `(from, to)` 走更新而非重复追加（回读关系条数不变）；`desc` / `apis` 三态逐条成立；`apis = ["A","A"]` / `[""]` / `["a::b"]` 这类值由重解析自证拒绝（写面 MUST NOT 自写值面判据）；`plan_relation_delete` 对缺席关系 `Err`、对在场关系删除成功且计数随动；端点元素缺席 ⇒ `Err` 点名缺失端。
11. **AC-11 · 生成物与读数零变化**：`bindings:export` 后 `git diff` 对 `bindings.ts` / `bindings.zod.ts` **零变化**；`api:check` 零差异退出、`api:export` 对 `INDEX.toml` / `.schema.json` / `.i.*` 零 diff；`crates/core/archi/tests/**` 语料与金样逐字节不变；架构报告读数与 AC 表「本仓读数」逐字一致（55 / 129 / 0 / 0 / 39 / 53 / `clean` / `modelHash` 不变）。
12. **AC-12 · 版本与改动集**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 逐字不变；`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（`edit.rs` 与 `edit_test.rs` 的**删除**须与 `edit/` 下四个新增文件同批出现）。
13. **AC-13 · spec delta 形态**：`openspec/changes/archi-edit-engine-primitives/specs/` 恰三个能力目录；RENAMED 块的 `FROM:` 与基线要求标题逐字一致、`TO:` 与随后的 MODIFIED 块标题逐字一致；各 MODIFIED 块为基线整段拷贝后再改（与基线逐段 diff 只含本提案声明的差异），未丢任何未冲突条目；每条 `### Requirement` 至少一个 `#### Scenario:`（恰 4 个 `#`）且 WHEN/THEN 成对；`## Module Contract` 段按受影响模块列 Function / API / CLI 表。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **P3 的公开面几乎没有产品消费者**：`plan_*` 的全部新能力（三态 / `expect` / 删除 / 关系 / `hash`）在 A 落地前只有单测在用，属「前置片」的固有形态，可能沦为死代码（Rust 的 `pub` 不产生 dead-code 告警） | 每个原语在本提案里有具名验收条（AC-3 / 4 / 8 / 9 / 10），A 的清单里逐一对上消费者（元素 upsert ↔ 元素写工具、relation upsert/delete ↔ 边写工具、`expect` / `hash` ↔ 两工具的乐观锁、`file_hash` ↔ 读侧回传）。评审时按「原语 ↔ 未来消费者」逐行核对，不接受无消费者的原语 |
| R-2 | **`archi_set_gate` 不再是严格的「同输入产同文本」**：表头回填会让陈旧计数被顺带修正 | 本仓六个文件计数实测准确 ⇒ 今日零差异（AC-5 用「gate 改成当前值 ⇒ 零字节变化」钉住）；陈旧表头单列一条用例（多修正一处，且括号外逐字）；`desktop-archi-model` 的 IPC requirement 与 scenario 同批改口径，不留「逐字保留」的绝对化措辞 |
| R-3 | **保序方案的长期外观代价**：给既有元素新增字段会追加到表尾，可能与规范序不一致 | 接受（纯外观）；创建路径按规范序（AC-6 的用例断言新建元素的字段序）；若将来要统一序，须另立变更并同批改「键序逐字保留」的承诺与测试（MUST NOT 在本变更里半途混用两种取向） |
| R-4 | **无图级后置校验**：新建关系可能引入环（`cycle`）、新建元素可能造成跨文件重复（跨文件重复 FQN 只能靠 `parse_dir` 的既有诊断回灌） | P3 只做「父元素 / 关系端点存在」前置（用既有模型，零第二份判据）；环与跨文件重复由写后读面回灌（`archi_graph` 停摆态 / `archi_drift` 的 `structuralErrors` 是既有反馈通道）。MUST NOT 为补这一格而重实现图规则或在写面伪造第二份 `parse_dir` |
| R-5 | **元素与关系写面非原子**：一次写只能改一处，`ElementSpec` 不含 relations ⇒ A 的「加元素 + 3 条边」需两次 plan / 两次写 | 中间态合法（无边元素不触任何结构诊断），可接受；A 若要求单次原子，须加组合层（把 `apply_*` 串在一条文本上）——那是 A 的引擎增量，本变更不预埋 |
| R-6 | **砍 `ElementSpec.relations` 与 explore 草案偏离**：A 的工具 schema 可能按「元素 + 边一次给」设计 | 偏离已在提案 §三 / §七 写明；A 的规划阶段若坚持单调用，回落路径是在本原语族上加一层组合（不动 `apply_*` 语义），MUST NOT 回退到「plan 内部跨文件多写」 |
| R-7 | **判据句文案变更**（5 条含「改档」的文案改操作中立措辞）影响画布行内错误呈现与既有断言 | 知情变更，逐条对照在 AC-7；改动面 = 5 条判据句 + `edit_test` 一处断言；若评审要求逐字不变，回落方案是 `criterion()` 保留「改档」措辞（零测试 churn，代价是通用引擎里留一个专属词） |
| R-8 | **`edit.rs` → `edit/` 目录化的搬家风险**：`lib.rs` 的 `#[cfg(test)] mod edit_test;` 与文件位置不一致会编译失败；`edit_test.rs` 迁入后内容需按新 API 重写（不是纯搬移） | 搬家与重写同批提交；`lib.rs` 的模块登记与 `edit/mod.rs` 内侧例声明两处同改（编译期即兜底）；测试文件组织沿 `api/` `drift/` 既有目录约定，MUST NOT 保留 `edit_test.rs` 幽灵文件 |
| R-9 | **哈希口径误用**：`file_hash` 含文件名参与累加 ⇒ 与 `modelHash` / `assets_hash` 数值不可比；误当作可比会得到「永远不等」的假冲突 | 函数 doc 与 `desktop-archi-drift` 的 delta 明写「只可与自身前后比较」；AC-8 只断言前缀、稳定性与写后等价，MUST NOT 断言与 `assets_hash` 等值 |
| R-10 | **写面误扩**：顺手把 `path` / `apis` 的形态校验、级联删除、多文件事务塞进 P3，把评审面从「渲染与自证」污染成「语义裁决」 | 提案 §「不要修改」与 R-4 / R-5 明确三件不做；评审按「本变更零第二份判据」核（AC-6 / AC-10 的反向断言） |
| R-11 | **生成物漏核对**：虽然是零线面变化，但漏跑 `bindings:export` / `api:check` 会让「零 diff」这一硬判据失效 | AC-11 把两条命令的零差异写成交付判据；`bindings.ts` MUST NOT 手改（会被下次生成静默覆盖） |
| R-12 | **`expect` 的语义歧义**（`None` 既表「不校验」又像「期望文件不存在」） | spec 与 doc 双重明写：`None` = 不校验；「文件缺席 + `Some(_)`」= 不匹配（`Err`），不存在「期望缺席」这一臂（新建落点由 `ElementSpec.file` 明确给出） |

---

## explore 未决项与「不含」的归属

| explore 条目 | 处置 |
|---|---|
| §3.4「保序 vs 统一序」留待 P3 评审确认 | **已裁：保序**（§五，三条理由）；否决理由与回落条件一并记入 |
| §3.2 可选同期 `ArchiModuleDetail.modelHash` + `bindings.ts` 重导 | **不含**：线面变化且 P3 无消费者；`file_hash` 就位后由 A 接线（属主篇 §4.7 的读侧闭环） |
| §3.3 `ElementSpec.relations` / `cascade: bool` 参数位 | **不收**：`relations` 由两条关系原语承载；`cascade` 不收布尔（§七、R-5 / R-6） |
| §五「不做」四条（agent 工具 / `drift::check` 前置 / 重实现 `path` 校验 / 落点与级联完整实现） | **照收**，并各配一条反向判据（AC-6 / AC-10 / §不要修改 / R-4） |
| §五「落点最小版」：给 `file` 落该文件、不给落现有文件、新建必须给 | **照收**：`ElementSpec.file` 为唯一落点入参；新建必填、既有元素只允许同值（换文件属 A）；模型声明目录缺席即 `Err`（写面不代建目录） |
| 「消息「改档」措辞」 | **改操作中立**（R-7 记知情变更，附回落方案） |
| `edit.rs` 是否目录化（explore 只提 `messages.rs`） | **目录化**（沿既有约定；R-8 记搬家风险） |
