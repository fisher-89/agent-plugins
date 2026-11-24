# P3 · 写引擎原语（`edit` 模块泛化）

workflow_type: requirement

> **来源**：从 [`archi-agent-contract-knowledge-gap`](archi-agent-contract-knowledge-gap.md)
> 第十一节拆出。**前置片 P3**：把主篇切片 A 里体量最大、最易出 bug 的纯引擎工作前置，
> 使 A 只剩声明式改动（工具声明 / 矩阵 / 知识 / spec）。
>
> **状态：可实施，未落地。** 这片**不改任何 spec**——它是既有命令的内部实现演进。

## 一、为什么能前置

`archi::edit::plan_set_gate` 现在只有一个消费者：壳层命令
`archi_set_gate`（`src/commands/archi/mod.rs` L142）。把它泛化成
`plan_element_upsert` 后，**消费者仍是它**——画布行为逐字不变，agent 工具还不存在。

因此 P3 的验收可以完全不依赖 agent 面：**同输入产同文本**。

## 二、现状

```rust
// core/archi/src/edit.rs（88 行）
pub struct GateWrite { pub file: PathBuf, pub content: String }

pub fn set_gate(source: &str, fqn: &str, gate: GateTier) -> Result<String, String>
pub fn plan_set_gate(root: &Path, fqn: &str, gate: GateTier) -> Result<GateWrite, String>
```

`set_gate` 的三步即主篇 §4.6 自证链的雏形：

```
toml_edit 改 gate 行 → 重解析（TomlParser）零诊断 → 回读档位一致 → 产出文本
```

`plan_set_gate` 再做三件：`parse_dir` 结构诊断前置（坏模型拒写）、元素存在性、
源文件读取。

## 三、方案：三步可独立提交

### 3.1 P3a `messages.rs` 抽取（纯重构，零行为）

现在错误文案内联在 `edit.rs`。抽成封闭枚举：

```rust
pub enum EditError {
    ModelHasDiagnostics { count: usize, lines: String },
    ElementNotFound { fqn: String, total: usize },
    ElementNotTable { fqn: String },
    SourceReadFailed { file: String, cause: String },
    RenderSelfCheck { count: usize, lines: String },
    ReadBackMismatch { field: String, actual: String, expected: String },
    // …
}
impl EditError { pub fn message(&self) -> String; pub const ALL: … }
```

范本：`api/check.rs` 的 `EntryState → message` 文案族（判据句 + 当前事实 + 下一步）。
`ALL` 常量让测试可遍历（与 `DiagnosticKind::ALL` / `ElementKind::ALL` 同构）。

**验收**：所有既有 `Err` 串逐字不变（用现测试断言），新增一条「错误变体集合 == match
分支集合」（零孤儿文案）。

### 3.2 P3b `file_hash` 单点

```rust
// core/archi/src/edit.rs（或 drift 的邻居）
pub fn file_hash(path: &Path) -> String   // "sha256:<hex>"
```

必须与 `drift::assets_hash` **共用同一 `hash_files`**（`drift/mod.rs` L580）——
`hash_files` 现在是私有，需要提成 `pub(crate)` 或抽到共享位置；**MUST NOT 另写一份
Sha256 拼装**。粒度是单文件：`hash_files(&[path], root)`。

> 注意 `assets_hash` 在名字里也混入文件相对路径（`hasher.update(name)`），单文件时
> 这一项恒定，不影响判据——但要在 doc 里写明，免后人以为两者输出可比。

可选同期（主篇 §4.7 的读侧闭环）：`ArchiModuleDetail` 增 `modelHash`，`bindings.ts` 重导。
**单独一条提交**，因为它是线面变化，评审性质与纯引擎演进不同。

### 3.3 P3c `plan_*` 泛化

```rust
pub struct ElementSpec {
    pub fqn: String,
    pub kind: Option<ElementKind>,        // 新建必填（由调用方保证）
    pub gate: Option<GateTier>,
    pub api: Option<ApiStatus>,
    pub entry: Option<Option<String>>,    // Some(None) = 清空
    pub path: Option<Vec<String>>,
    pub doc: Option<Option<String>>,
    pub summary: Option<Option<String>>,
    pub metadata: Option<BTreeMap<String, String>>,
    pub relations: Vec<RelationSpec>,     // 出边：to + desc? + apis? + action?
    pub expect: Option<String>,           // 文件乐观锁
}
pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<GateWrite, EditError>
pub fn plan_element_delete(root: &Path, fqn: &str, cascade: bool) -> Result<GateWrite, EditError>
pub fn plan_relation_upsert(root: &Path, from: &str, to: &str, desc: Option<&str>, apis: &[String], expect: Option<&str>) -> Result<GateWrite, EditError>
pub fn plan_relation_delete(root: &Path, from: &str, to: &str, expect: Option<&str>) -> Result<GateWrite, EditError>
```

`plan_set_gate` 不再单独存在，壳层改为：

```rust
let spec = ElementSpec { fqn: element.into(), gate: Some(gate), ..Default::default() };
write_atomic(&plan.file, &plan.content)
```

四条实现约束（都来自主篇的裁决）：

1. **`Option<Option<T>>` 三态**：`None` = 不动、`Some(None)` = 清空、`Some(Some(v))` = 设值。
   这是「省略 = 不动」语义的唯一实现，MUST NOT 用 `Option<T>` 混过去。
2. **`expect` 校验在 `plan_*` 入口**（读完源文本、渲染之前），使画布与未来工具两条写面
   **要么都受保护、要么都不受**。
3. **`GateWrite` 扩 `hash` 字段**（写后新值），在 `plan_*` 内部算——它已持有最终文本；
   让执行体再读一次盘会产生 TOCTOU 窗口。
4. **表头计数重算**：`# 02 · core 分层（9 元素 / 10 关系）` 的括号内由重解析结果回填，
   括号外（人工标题）原样保留。这是主篇「计数复述单源」的手工半边消失之处。

   副作用：`archi_set_gate` 从此也会顺带修正陈旧表头。**这是修正而非破坏**——但要单列
   一条断言（避免与"同输入产同文本"混在一起被判成回归）。

### 3.4 一个必须现在想清的问题：渲染的保序风险

`toml_edit` 保留注释与空行，但**字段插入到已存在元素的内联表尾部**，与文件里既有字段序
不一定一致（现有模型字段序是 `kind / gate / path / entry / doc / summary / metadata`，
见 `02-backend-core.toml`）。两种取向：

- **保序**：按文件既有字段顺序插（需要读原元素的键序再对齐）——产出的 diff 最小；
- **统一序**：一律按 `RawElement` 字段序重排该元素的键——既有元素首次被工具改时会多出一
  次"字段重排"diff。

**倾向统一序**：与 `archi_scaffold` 的模板顺序一致，且不必为"对齐原键序"写第二套逻辑。
代价是一次性的字段重排 diff，可接受。**但需在 P3 的评审里确认**。

## 四、验收面

| # | 断言 |
|---|---|
| 1 | 既有 `archi_set_gate` 的全部测试继续绿（文案逐字不变） |
| 2 | `plan_element_upsert(spec { gate })` 与旧 `plan_set_gate` 产出的 `content` 逐字节相等（表头陈旧的情形单列：新实现多修正表头） |
| 3 | `EditError::ALL` == 全部 match 分支（零孤儿文案 / 零无语料分支） |
| 4 | `file_hash` 与 `assets_hash` 对同一文件的前缀一致（`sha256:`），且实现零独立 Sha256 |
| 5 | `expect` 不匹配 ⇒ `Err` + **零文本产出**（沿用「不把坏文本交给调用方」） |
| 6 | 表头重算：括号内 == 重解析计数，括号外逐字不变 |
| 7 | 三态语义：`None`（不动）/ `Some(None)`（清空）/ `Some(Some)`（设值）逐条用例 |
| 8 | 结构错模型 ⇒ `plan_*` 全部 `Err`（继承 `plan_set_gate` 的 fail-closed） |
| 9 | 画布命令行为不变：`full` 模式零调用、`incremental` 改档落 worktree（既有断言） |

## 五、不做的事

| 面 | 理由 |
|---|---|
| 在 P3 里加 agent 工具 | 定义与执行体一体；P3 只做引擎，工具留 A |
| 跑 `drift::check` 做前置判定 | 含 scan，写是高频动作 |
| 重实现 `path` 形态校验 | parser 刻意不校验（spec 明写），重实现即第二份判据 |
| 落点策略 / 删除级联的完整实现 | 属 A 的语义层；P3c 只需留出参数位（`file` / `create` / `cascade`） |

> 第五行值得展开：`plan_element_upsert` 的**落点解析**（最近已声明祖先所在文件）在 P3
> 里可以只做"给 `file` 就落那个文件、不给就落元素现有文件（新建时报错）"的最小版，
> 完整策略留 A。这样 P3 的评审面只有渲染与自证，不被落点决策污染。

## 六、与主篇的关系

P3 落地后，主篇切片 A 剩：

```
两工具定义（平铺 schema，含 8 条入参上限）
+ 注入矩阵两行 + 两处 phase prompt
+ 骨架收缩（2596 → ~1300）+ 两个 _test.rs
+ spec 五处立场 + 6 份文档面
+ bindings 重导（warnings / hash）
```

**全是声明与散文**——最需要人读、最不需要调试的那部分。
