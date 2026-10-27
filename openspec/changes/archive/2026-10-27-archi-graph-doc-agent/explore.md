# 架构图 · 模块文档 · 模块接口协议联动（UI 与 agent 辅助生成/维护）
workflow_type: requirement

---

## 0. 范围

这个 explore 只管**模型侧与表达侧**：模型怎么存、怎么检查自洽、图上能看到什么、模块文档怎么关联、agent 怎么用它、怎么帮人写它。

代码扫描、漂移判定、门禁挂载、产物标准归另一个 explore（`archi-check-and-gate`）。两者的接缝只有一个：**本 explore 产出的 `ModelGraph` 是那个 explore 的输入**。

```
                   archi-graph-doc-agent            archi-check-and-gate
                   ─────────────────────            ────────────────────
产出               ModelGraph（元素 / 关系 /     →  消费 ModelGraph + 源码
                   文档关联 / 结构诊断）             产出漂移报告 / 门禁结论
                                                        │
                   UI 画布 ←──── 漂移标记（只认标记语义，不认判定逻辑）────┘
```

**漂移标记的归属**：判定「这条边漂了」属于另一个 explore；本 explore 只规定「画布拿到标记后怎么呈现」（角标与边样式），不规定什么时候给标记。

---

## 1. 模型

### 1.1 实体与映射

```
【 拓扑层 】  C4 element（kind: package / domain / module / component）
              ├─ 嵌套表头（TOML 的 [element."App.Auth"]）⇒ Contains
              └─ [[relation]]                              ⇒ DependsOn
                    │
                    ▼  语义锚点
【 锚定层 】  path = ["./src/x/", "./src/y.ts"]   ⇒ 1:N 物理边界
                    │
                    ▼  引用（不复制正文）
【 文档层 】  docs（显式）/ 邻近 md / ADR                          ⇒ 模块文档
                    │
                    ▼  待探索
【 协议层 】  模块接口协议（见 §1.3，未定）
```

| 项 | 取舍 | 理由 |
|---|---|---|
| 逻辑模块 | **C4 element 即模块**，不另建 LogicalModule | 嵌套结构本身就在表达包含，另立一层只会双写 |
| 物理映射 | **`path` 是数组** | 一个元素可覆盖多个路径（目录 + 单文件混合） |
| 模块文档 | **不规定形态，只做关联与检索**（§3.2） | 文档该写什么因团队而异；钉死 schema 会把「还没想清楚」变成「落后于文档」 |
| 模块接口协议 | **未定**，见 §1.3 | 前身方案明确砍掉过「语言无关 API schema」，现在重新提出，需要单独裁决 |

### 1.2 门禁档位（ratchet）

档位是模型的属性，靠字段表达，不需要新语法：

```toml
[element."App.Auth"]
kind = "domain"
path = ["./src/auth/"]
gate = "strict"          # 缺省 = off

[element."App.Legacy"]
kind = "domain"
path = ["./src/legacy/"]
gate = "warn"
```

| 档位 | 参与归属 | 作为引入方时的违例定级 |
|---|---|---|
| `off`（缺省） | 否——其文件算 unmatched，只计数不报错 | 不报 |
| `warn` | 是 | warning |
| `strict` | 是 | **error** |

**定级只看引入方档位**（谁引入了不该有的依赖谁负责），避免一个 strict 元素把全仓库的 off 元素拽进门禁。档位的语义消费在另一个 explore，但**档位本身是模型的属性**，落在这里。

存量债务由此自然消失：旧元素保持 `off`，想收紧的模块逐个改 `strict`，一个 change 收紧一块。

### 1.3 待探索：模块接口协议

这是本 explore 里**唯一没有结论的部分**，必须说清楚它从哪来、卡在哪。

前身方案有过明确取舍：

> **API 契约不落权威 schema**。契约权威在 specs + ADR；再造一份语言无关 schema 必然漂移，且要维护 Rust/TS 两套提取器的对齐。

当时结论是砍掉。现在「图-模块文档-模块接口协议联动」把它放回来，意味着**要重新裁决**，且必须回答当初否决它的那两条：

| 当时的问题 | 若不解决，重提就会重蹈覆辙 |
|---|---|
| 契约权威在哪 | 如果模型里再存一份接口声明，它与 specs / ADR 谁说了算？两份必漂移 |
| 两套提取器怎么对齐 | Rust 侧 `syn` 与 TS 侧 `oxc` 提取出的符号，如何保证描述的是同一种「接口」？ |

候选形态（都未定，按「越靠引用越安全」排）：

```
A. 协议不落模型，只落「引用」    docs 里指向已有的接口文档 / OpenAPI / proto 文件
                              ← 与「文档层只做关联」同构，风险最低

B. 协议落模型但只到「符号名」    element 上声明 exports = ["login", "logout"]
                              ← 只保证「该模块对外暴露这些名字」，不校验签名
                              ← 需要决定：谁提取、谁比对、和代码不一致怎么处理

C. 完整签名级契约              （类型 / 参数 / 返回）
                              ← 就是被否决过的那层，需先解决上面两个问题
```

本 explore 不对此时下结论。实现顺序建议：先做 A（零成本，和 §3.2 文档关联是同一套机制），观察真实需求后再议 B；C 需要独立 change。

---

## 2. 存储格式：TOML 嵌套表头（已实测）

### 2.1 实测结论

```toml
[element."App"]
kind = "package"

[element."App.Auth"]
kind = "domain"
path = ["./src/auth/", "./src/auth-legacy/"]
gate = "strict"
docs = ["./src/auth/README.md"]

[[relation]]
from = "App.Auth"
to = "App.Shared"
desc = "复用错误类型"
```

实测环境：`toml` 1.1.6 + `toml_edit` 0.25，离线编译运行。

```
依赖      toml 1.1.6 + toml_edit + serde_spanned 均在 Cargo.lock → 零新增依赖
span      serde 反序列化同时给出类型化 AST 与字节区间
写入      toml_edit 的「解析 → 改 AST → 渲染」保留注释与空行
类型层    serde enum + deny_unknown_fields 挡掉非法值与未知字段
```

### 2.2 span 全覆盖

```
[element."App"]                   key span 30..33,  line 2, literal "App"
[element."App.Auth"]              key span 62..72,  line 5, literal "\"App.Auth\""
path = ["./src/auth/", ...]       每项独立 span,  line 7
docs = [...]                      独立 span,       line 9
[[relation]]
from = "App.Auth"                 span,            line 12
to = "App.Shared"                 span,            line 13
```

行号换算只需 `src[..byte].matches('\n').count() + 1`。**`modelFile` + `modelLine` 零额外实现**——这是 agent 能精确定位到元素、UI 能「点击跳转到声明」的前提。

### 2.3 错误消息质量（决定自实现范围）

| 输入 | 输出 |
|---|---|
| `kind = "pakage"` | `TOML parse error at line 2, column 8` + 源码片段 + `^^^^^^^^` + `unknown variant 'pakage', expected one of 'package', 'domain', 'module', 'component'` |
| `gate = "stirct"` | 同上（列出 `off` / `warn` / `strict`） |
| `gatte = "strict"`（拼错键） | 拒绝：未知字段 + 位置 |
| 缺 `kind` | 拒绝：missing field + 位置 |
| `kind = package`（漏引号） | 拒绝 + line/column |
| 漏闭合 `]` | 拒绝 + line/column |
| **同一 FQN 重复声明** | 拒绝：`duplicate key` + 位置 + `^^^` 指向冲突的 key |

### 2.4 结构错的分工

```
parser / serde 免费（含 line/column + caret，无需自实现）
├── syntaxError
├── missingField
├── invalidValue（kind / gate 枚举，错误里列出全部合法值）
├── unknownField（deny_unknown_fields）
└── duplicateElement（TOML map 语义天然拒绝重复 key）

必须自实现（petgraph + 规则，共 3 类）
├── cycle（成环，SCC）
├── dependsOnAncestor（元素依赖自己的祖先）
└── unknownReference（关系引用不存在的元素）
```

从 7 类降到 3 类，省掉的正是最枯燥的一类。

### 2.5 为什么是嵌套表头而不是扁平数组

```toml
# 采用：嵌套表头
[element."App.Auth"]
kind = "domain"

# 不采用：扁平数组 + 显式 FQN
[[element]]
fqn = "App.Auth"
kind = "domain"
```

| | 嵌套表头 | 扁平数组 |
|---|---|---|
| 层级来源 | 表头结构 | FQN 最长前缀推导 |
| `orphanHierarchy` 检查 | **不需要** | 需要 |
| `duplicateElement` | ✅ parser 免费拒绝 | ❌ 数组里看不出，要自己查 fqn 重复 |
| 元素 key span | ✅ 实测可用 | ✅ 实测可用 |
| 视觉层级 | 结构自带 | 靠排序 + 前缀 |

扁平写法唯一的理论优势（按 FQN 排序让层级可见）被两条代价抵消。**采用嵌套表头**；写法约定一律加引号 `[element."App"]`，避免「有时候要引号有时候不要」的判断负担。

### 2.6 为什么不是其它格式

| 候选 | 淘汰理由 |
|---|---|
| **c4 / LikeC4 DSL** | 手写 parser 400-600 行 + span 自建 80 行 + 多文件聚合 50 行；`specification` 的全局性使单文件不可独立校验；三类校验全自写。其唯一优势（`extend` / `->` 是架构界通用词汇）在没有外部 oracle 需求后**没有消费者** |
| **KDL** | 定位与 `toml_edit` 相同、span 也是默认特性，但 **serde 路线丢 span**（必须走 document API 手写 ~100-150 行映射）；attr 是字符串 → kind / gate 校验自写；`miette` 依赖树带进 5~15 个包 |
| **DOT** | 属性全字符串（无类型层）；`cluster_*` 是布局惯例不是语义嵌套；属性无文档位置 |
| **GraphML** | 唯一有标准语义嵌套的图格式，但 XML 把一事实变 4~6 行——agent 写易漏闭合、diff 最差 |
| **JSON / JSON5** | `serde_json` 不记位置（span 需另找）；引号 + 闭合括号移位让 diff 差 |
| **YAML** | 缩进敏感（agent 全量重写最易坏）；`serde_yaml` 已归档 |
| **RON** | 类型层最好（Rust enum 原生），但锁外 + 生态小众 + 无格式保全编辑器 |
| **Mermaid / PlantUML** | 绘图语言，原子是「怎么画」不是「是什么」；metadata 只能塞 label |
| **Structurizr DSL** | 架构语义最贴，但无 Rust 解析器、需 JVM |
| **目录约定（无模型文件）** | 决定性缺陷：无法表达「声明了但代码未实现」，因为声明必须显式存在 |
| **模块文档 front-matter** | 模型散落各处，「整体 review 模型」退化为遍历；且 front-matter 是 YAML |

### 2.7 解析层可替换

```
crates/core/archi/src/
├── model.rs      格式无关 AST（Element / Relation / Docs / Span）
│                  derive PartialEq（为将来的 UI diff 预留）
├── parse/
│   ├── mod.rs    trait SourceParser { fn parse(&str) -> ParseResult<Model> }
│   └── toml.rs   当前实现
└── rules/        结构校验 / 图算法 / 文档关联，只认 model.rs
```

换格式 = 改 `parse/` 一个文件 + 换一套语料，`rules/` 与报告结构不动。这层隔离本来就该有（让语料测试只测 parse、规则测试直接造 AST），不是为换格式预铺。

关系级 span 当前不做，但 AST 里留 `Option<Span>` 位置，否则将来 UI diff 无法「点击一条边跳到它的声明行」。

---

## 3. 模块文档

### 3.1 核心取舍：返回引用，不返回正文

agent 已有 `read` / `grep` / `glob` 三件通用检索工具。架构工具再吐正文是重复造轮子，且会把 token 烧在「用户其实没问的文档」上。所以边界是：**把「元素 ↔ 关系 ↔ 文档」的索引告诉 agent，正文由 agent 自己 `read`。**

```
archi_module  →  「App.Auth 是什么，它依赖谁、谁依赖它，相关文档在哪」
read          →  「把那份文档读出来」
```

代价是 agent 多一次工具调用；收益是 `archi` crate 不需要文档正文的解析 / 索引 / 版本管理，文档形态可以自由演进。

### 3.2 三种关联来源

不预设「模块必须有一篇文档」。只做关联发现，任何未来出现的文档形态都能被吸收：

| 来源 | 机制 | 强度 |
|---|---|---|
| 显式 | `docs = ["./src/auth/README.md"]`（与 `path` 同构的数组） | 强——作者亲口指定 |
| 邻近约定 | 元素 `path` 目录下的 `*.md` | 中——按位置推断 |
| ADR 文本 | `decisions/*.md` 正文出现该元素 FQN | 弱——检索命中，非承诺 |

三者合并去重，按强度排序返回。**契约只有 `{ path, title, kind, source }`**——没有摘要、没有正文、没有 schema。将来加「接口说明文档」这类新形态，只需加一条索引来源，工具契约不变。

* `docs` 指向不存在的文件 → `warnings` 的 `docNotFound`，只报不拦（文档缺失不影响代码正确性）。
* 邻近搜索与路径校验都在元素 `path` 覆盖范围内，与扫描排除规则同一套（测试目录里的 md 不算模块文档）。

### 3.3 文档放哪（不进 architecture 目录）

```
openspec/architecture/          横跨代码的架构资产
├── models/*.toml               模型
├── decisions/*.md              ADR
├── config.json                 豁免与扫描排除
└── reports/*.json              报告

模块文档不进这里，而是跟代码走：
  src/auth/README.md            邻近约定（元素 path 覆盖目录内）
  任意位置                       用 docs 显式指向
```

理由：文档与它描述的代码应当同生命周期（代码搬走文档跟着搬、代码删了文档该被发现）。放进 `architecture/` 会让文档与实现分离，反而更容易过期。

### 3.4 长期演进（不在本 explore 范围）

方向是「模块文档逐步替代 `openspec/specs/`，最终只留少数主要特性文档」。这里**不做任何预铺**，只守四条边界：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 看护不读 specs | 否则替代完成那天，看护逻辑要整体改写 |
| 2 | 检索不读 specs | 一旦纳入，用户会开始写「给 archi 看的 spec」，反向锁死迁移 |
| 3 | 模块文档关联只认三种来源（§3.2） | 不新增 spec 来源 |
| 4 | 门禁不依赖 specs ↔ element 映射 | 该映射将来由「模块文档 ↔ element」承担 |

迁移的真正难点（将来独立裁决）：specs 现在承担六项职责——契约记录 / 可验收断言（Scenario）/ 能力盘点 / 归档 SpecSync 的增量合并 / 测试设计输入 / 验收输入。后两项实际已不直读 specs（test-design 读 design.md、acceptance 读 proposal.md 的 AC），风险低；但**可断言结构**与**增量合并语义**没有散文形态的对应物。若模块文档是纯散文，acceptance 会失去可核对的形状，归档的 `SpecSync` 步（ADDED/MODIFIED/REMOVED/RENAMED 合并）没有对应物、只能退役。

---

## 4. UI

### 4.1 一个画布，两处挂载

复用既有画布与渲染资产，不新建可视化体系：

```
ArchitectureCanvas（共享组件，props = { root, mode }）
├─ 左：元素树（来自嵌套结构）                  ← 复用 standard 组件
│       节点前置状态点：🔴 结构错 / 🟡 声明未兑现 / ⚪ 正常
├─ 中：xyflow 画布（节点 = element，边 = relation）
│       默认只渲染聚焦元素 1-hop，避免过载
│       边样式：实线=已兑现 灰虚线=声明未兑现 橙虚线=跨 kind 向上 灰=豁免
└─ 右：抽屉
    ├─ 元素概览（kind / path / gate / modelFile:line / docs 列表）
    ├─ 相邻报告（最近一次检查的 errors / warnings）
    └─ ADR 列表（markdown-doc-renderer 渲染 decisions/*.md）

挂载点 A  /architecture              root = workspace 主仓根，mode = 全量
挂载点 B  change 详情内的架构页签     root = detail.worktree，mode = 增量
          （归档 change 回落主仓根）
```

**观察根就是「当前工作根」**，不需要新概念：

```
workspace 级 → 主仓根
change 级    → detail.worktree（归档 change 回落主仓根，与会话 cwd 同口径）
```

这与前端既有口径一致：`AppRoutes` 每页拿 `root`（workspace 根），change 侧会话用 `detail.worktree`（`use-change-chat.ts` 的 `cwd` 派生：归档 change 恒 `null` 回落 root）。命令面照抄这个入参形态——**命令签名一律带 `root`，不引入「当前选中 change」这类隐式上下文**。

能力差异（数据语义差异，不是 UI 差异）：

| 能力 | 全量（workspace 级） | 增量（change 级） |
|---|---|---|
| 结构校验 | ✅ 毫秒级 | ✅ 毫秒级 |
| 文档关联 | ✅ | ✅ |
| 漂移判定 | ✅ 全仓（另一个 explore） | ✅ 只扫变更文件（同上） |
| 改 `gate` 档 | ❌ 只读视图（提示「改档需要开一个 change」） | ✅ 编辑 worktree 的模型，随该 change 归档回合 |

### 4.2 结构错时画布停摆（全或无）

模型有**任何**结构错时，画布不渲染残缺图——停止渲染，改显示错误面板：

```
画布区域的两态
├─ 正常态    xyflow 图（元素 / 关系 / 状态点 / 边样式）
└─ 停摆态    错误面板：逐条列出结构错 + 每条带 file:line + 「修好后再看」
             画布零渲染、零降级样式、零幽灵位置
```

**为什么全或无，而不是「画残缺图 + 高亮」**：

| 理由 | 说明 |
|---|---|
| 残缺图的读法本身是歧义的 | 环画成什么样？悬空边画在哪？缺端点时丢弃还是画哑节点？每种处理都要新的视觉约定 + 新的交互解释 |
| 错误面板更可操作 | 图出来了说明模型是对的，看图的心智负担为零 |
| 少一套样式与组件 | 无需「降级样式」系统（环边标红 / 悬空边丢弃 / 缺字段节点灰化） |
| 与写入口的分工一致 | 命令面（画布连边）本来就写前校验 —— 停摆态在 UI 上延续了同一条纪律 |

代价：模型错着的时候看不到现状。可接受——错的是结构（成环 / 引用不存在 / 依赖祖先 / 缺必填字段），不是内容；这些错误都有精确行号，读文本比读畸形图快。

三条策略各管一段（互不冲突）：

```
读取     照常加载，返回结构化 errors（绝不 panic）
渲染     停摆，显示错误面板（绝不画畸形图）
门禁     判 structuralErrors → fail（见另一个 explore）
```

### 4.3 漂移标记的呈现（只认标记，不认判定）

画布需要显示漂移，但**判定逻辑不在本 explore**。契约是「画布接受一组标记，按标记渲染」：

```
标记输入（来自检查侧，本 explore 不定义何时产生）
├─ 声明未兑现    该边灰虚线
├─ 跨 kind 向上   该边橙虚线（提示价值，非约束）
├─ 豁免           该边灰实线 + 抽屉里显示 reason
└─ 节点级         🔴 结构错 / 🟡 有未兑现声明 / ⚪ 正常
```

这样切分的好处：画布不必知道「什么算漂移」，检查侧改造规则时 UI 零改动。

### 4.4 三方一致性：同一份图，三个消费面

图的三个消费面必须给出**同一个结论**——这是架构约束，不是编码纪律：

```
                  crates/core/archi
                  ├── model.rs      Element / Relation / Docs（derive PartialEq）
                  └── graph.rs      ModelGraph（petgraph）+ 诊断
                          │
        ┌─────────────────┼──────────────────┐
        ▼                 ▼                  ▼
   检查判定          archi_module        IPC DTO
   （进程内）        （serde_json）    （specta Type → bindings.ts）
        │                 │                  │
        └── 三者都是「从 ModelGraph 裁剪」，零改写 ──┘
```

损耗不对等：只有进程内消费零序列化，另两个都要过 JSON。所以一致性不能靠「小心点写」，只能靠**同源 + 三条纪律**：

| # | 纪律 | 违反后果 |
|---|---|---|
| 1 | **单点构建**：`ModelGraph` 只在 `archi` 里建一次，三个消费面都不自己再建图 | 出现第二份图算法，环检测结果可能不一致 |
| 2 | **投影零改写**：投影只做裁剪（1-hop / 分页 / 字段子集），不重算、不推导、不补默认值 | 投影层变成第二事实源 |
| 3 | **词汇全量对拍**：用一份含全部词汇的模型语料，断言三个消费面覆盖相同词汇集 | 新增一个诊断类型，UI 静默不显示 |

第 3 条可测，形态照抄仓库既有先例 `src/views/changes/flow/run-state.test.ts` 的「二十一词全词汇经 `runStepNodes` 全量出图」。需要全覆盖的词汇：

```
element kind      package / domain / module / component
gate 档           off / warn / strict
结构诊断          syntaxError / missingField / invalidValue / unknownField /
                  duplicateElement（parser 免费）
                  cycle / dependsOnAncestor / unknownReference（自实现）
文档诊断          docNotFound
关系状态          已兑现 / 声明未兑现 / 跨 kind 向上 / 豁免
```

**已知漂移源（前置条件）**：`Option::None` 在 IPC 上走 specta（`null`）、在 `archi_module` 里走 serde_json（可能缺字段）。`openspec/todo.md` 已挂着「移除生成类型中的 null，使用 undefined 代替」——三方一致性依赖这条先落地。

---

## 5. agent 面

### 5.1 两个只读工具

沿用 `create_explore_note` 的先例（专用工具不进 `TOOL_NAMES` 七工具封闭集，经 `SessionInjections` 受控追加），新增 `SessionToolset::Archi` + 两个工具。

```jsonc
// 工具 1：archi_module —— 按元素定位（概述 + 关系 + 文档引用）
{
  "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" }
}

// 响应（指定 element）
{
  "element": { "fqn": "App.Auth", "kind": "domain", "gate": "strict",
               "path": ["./src/auth/"], "summary": "认证与令牌签发",
               "metadata": { "owner": "platform" },
               "modelFile": "openspec/architecture/models/02-auth.toml",
               "modelLine": 14 },
  "dependsOn":  [{ "target": "App.Shared", "description": "复用错误类型" }],
  "dependedBy": [{ "source": "App.Order",  "description": "登录态校验" }],
  "children":   ["App.Auth.Token", "App.Auth.Session"],
  "docs": [
    { "path": "openspec/architecture/decisions/0007-token-sink.md",
      "title": "令牌下沉到 Shared", "kind": "adr", "source": "text" },
    { "path": "src/auth/README.md", "title": "认证模块说明", "kind": "md", "source": "adjacent" }
  ]
}

// 工具 2：archi_search —— 按标识符检索（元素 + 文档路径，轻量版）
{
  "name": "archi_search",
  "parameters": {
    "query": "必填。关键词 / 元素名片段 / 文档路径片段",
    "scope": "可选。'elements' | 'docs' | 'all'（缺省 all）"
  }
}
```

`modelFile` + `modelLine` 让 agent 能精确定位到要编辑的位置，不必 grep 猜；两者来自解析器已有的 span，零额外成本。

**轻量检索的确切含义**：

```
匹配对象   元素 FQN / summary / metadata 值 / 文档路径与文件名（不含正文）
匹配方式   ASCII 大小写不敏感子串 + 驼峰分词（AuthToken → auth / token）
不做       全文索引、文档正文搜索、相关度打分（按来源强度排序即可）
```

不搜正文：正文搜索是 agent 既有 `grep` 的活，它能带 context、能正则、能按目录缩窄——架构工具重做只会更差。架构工具的价值在「知道去哪里找」，不在「替你去读」。

实现量：`ignore::WalkBuilder` 遍历架构域与元素覆盖路径（自动跳过 `node_modules` / `target` / gitignore），文件名与 FQN 做子串匹配，约百行；不建倒排、不缓存、每次现算。

为什么是两个而不是一个：`archi_module` 是**定位**（已知在看哪个模块），`archi_search` 是**发现**（还不知道这功能归谁管）。合并会让 `element` / `query` 参数语义含混，且两个场景无法差异化收窄工具面。

### 5.2 注入矩阵

| 场景 / 相位角色 | `archi_module` | `archi_search` | 理由 |
|---|---|---|---|
| proposal planner | ❌ | ✅ | 提案要先判断「这事落在哪个能力域」，正是发现场景 |
| dev-design planner | ✅ | ✅ | 写 design.md 要定落点 + 列依赖，模型也在这里被改 |
| dev-design evaluator | ✅ | ✅ | 审设计是否与模型一致 |
| test-design planner | ❌ | ✅ | 找被测模块的现有测试位置 |
| implement executor | ✅ | ❌ | 改文件时确认归属与可引用面 |
| code-review evaluator | ✅ | ❌ | 核对改动是否越界 |
| acceptance evaluator | ✅ | ❌ | 验收确认架构未劣化 |
| test-gen / test-execution / archive SpecSync | ❌ | ❌ | 与架构无关，注入只是噪声 + 加 token |

**没有第三个工具**。改模型用既有的 `write` / `edit`——模型文件与源码同权，不设专门的写工具，也就没有「专门写模型的工具该不该给某个相位」这个问题。

### 5.3 模型修改：模型 = 源码

**变更的 worktree 允许 agent 直接修改模型**，用既有的 `write` / `edit`。改动随变更一起回合并**必须人工确认**。

```
change 的 worktree                         归档            主仓
├─ 源码 *.rs / *.ts                        ↓               ↓
└─ openspec/architecture/models/*.toml ──▶ 人工确认 ──▶  ff_merge
   agent 用 write / edit 直接改              看模型 diff       权威
   （与改代码完全同权）                      （专用区块）
```

这条设计的两个支撑点：

* **不依赖运行时审批**。`loop.rs` 的工具分派对不合法调用只有 `deny`（合成 `SystemNotice` + is_error ToolResult，不中断 run），没有「挂起等用户点确认再继续」的能力。本方案不需要它——隔离层是 worktree，确认发生在归档，不是写盘那一刻。
* **设计权不靠写盘拦截**。worktree 的改动是 git diff 的一部分，与代码同等地进入人工评审；设计权体现在「回合必须人批」，而非「写盘必须拦」。

**有效性保证：写入时不校验，读取时报错**：

```
写入时        不校验（与写源码同等——编译器 / 检查器会告诉我们）
读取时        解析失败 → 返回结构化 errors，绝不 panic
门禁时        判 structuralErrors（见另一个 explore）
归档确认时    模型 diff 呈现（含结构错红字提示）
```

校验只发生在三处（读取 / 门禁 / 归档），不是「写盘前」这一处额外插口——少一个插口，少一套状态。代价是写坏模型期间架构视图显示结构错、门禁失败，这与「写坏 Rust 代码编译不过」同构。

**与 `write_protection` 的关系**：`models/` 与 `architecture/config.json` **都不纳入**。`protect` 的规则是绝对路径 glob（`rules.rs` 的 `load` → `match_path`，cwd 只用于 bash 路径解析），加了会在 worktree 里同样命中，agent 就改不了模型。而且 `protect` 本来就不是安全边界（bash 可绕），它是防误触护栏；模型文件的真正护栏是归档时的人工确认。

**三类使用场景全部走 change**（没有「workspace 级直接改模型」的通路——模型 = 源码，源码也只能在 change 里改）：

| 场景 | 入口 | 落点 |
|---|---|---|
| 冷启动（模型为空） | 建一个 change（如 `init-architecture-model`），dev-design 阶段让 agent 生成骨架 | worktree → 归档回合 |
| 增量维护（新增/重构模块） | 正常 change 的 dev-design 阶段 | 同上 |
| 修正漂移（按现状补声明） | 同上，理由来自全量体检的未声明依赖清单（另一个 explore 产出） | 同上 |

冷启动因此也是「起一个 change」。看似重，但换来的是新模型同样走「设计 → 评审 → 合入」，而不是某天有人往主仓塞了个模型文件。新生成元素默认 `gate = "off"`——符合 ratchet 的逐块收紧。

### 5.4 归档确认：模型 diff 要显眼

模型改动混在几十个代码文件里会被橡皮图章掉。在既有 `(proceed, sync_specs)` 停等点前加一个专用区块（**不是新的确认门**，只是提高可见性）：

```
归档确认
├─ 变更概览：N 个文件改动（M 源码 / K 模型 / J 文档）
├─ 🏛 架构变更（仅当模型有改动时出现）
│     元素 +2（App.Billing, App.Billing.Invoice）
│     元素 -1（App.Legacy）
│     关系 +3 / -1
│     gate: App.Billing off → strict
│     [查看模型 diff]  [查看新图预览]
└─ 同步 delta specs: 是 / 否
```

「查看新图预览」比读模型 diff 直观：归档前能在画布上看到改动后的图。模型无改动时该区块零占位。

### 5.5 模型冲突不进 agent 解算

worktree 合回主仓时若模型文件冲突 → **中断交用户处理**。`git.rs` 已有 `unmerged_files()`（`git diff --name-only --diff-filter=U -z`）；`ResolveConflicts` 步拿到 `cursor.rebase_conflicts` 后**先按路径分流**：

```
pending = cursor.rebase_conflicts
   │
   ├─ 含 openspec/architecture/ 下任何路径 → Terminal::failed
   │     文案：模型冲突需人工裁决，请在 <worktree> 处理模型后重跑归档
   │     理由：模型是设计权威，agent 自动解算等于让 LLM 替人做架构决策
   │
   └─ 全部是代码路径 → 既有 agent 解算流程（零改动）
```

**保留 rebase 半程**：`git.rs::rebase_branch` 的既有语义就是「冲突态保留于 worktree——MUST NOT 自动 abort」（只有非冲突失败才尽力 abort），所以这条路径只要 walker 侧不调 abort 即可，不需要新 vcs 能力。

但保留半程带来必须处理的**重入陷阱**：

```
T0  Rebase：git rebase → 冲突，worktree 处于 rebase 半程（REBASE_HEAD 在场）
T1  ResolveConflicts：检出模型路径 → Terminal::failed（不 abort）
T2  用户重跑归档 → 从 Preflight 开始
        │
        └─ Commit 步：dirty() 为真 → commit_all() = git add -A + git commit -m
              ⚠ 若用户尚未 rebase --continue / --abort，这是在 rebase 中间态提交
```

所以 `Preflight` MUST 检出半程并挡回（`worktree_snapshot()` 已返回 `rebase_head`，零新 git 能力），给两条出路：

```
① 在 <worktree> 内解完模型冲突后 git rebase --continue，再重跑归档
② 放弃本次重放：git rebase --abort，再重跑归档（会重新 rebase 一次）
```

| 用户处置 | Preflight | 结果 |
|---|---|---|
| 已 `rebase --continue` | `rebase_head` 不在 → 放行 | Commit（干净跳过）→ Rebase（up-to-date no-op）→ Merge 快进 |
| 已 `rebase --abort` | 放行 | Commit → Rebase（正常重放，可能再冲突）→ … |
| 什么都没做直接重跑 | 挡住 + 引导 | 不产生「rebase 中间态提交」这类难排查的污染 |

代价：半程 change 持续占用 active 名额（上限 5），worktree 此时不可被 Cleanup 删。前端在归档失败面必须显示 worktree 绝对路径 + 两条处置命令。

---

## 6. crate 落位与目录常量

### 6.1 crate 图

```
                  ┌────────────────────────────────┐
                  │ desktop-app (src-tauri, Tauri) │
                  └───┬────────┬────────┬───────┬──┘
                      │        │        │       │
        ┌─────────────┘        │        │       └──────────────┐
        ▼                      ▼        ▼                      ▼
   ┌─────────┐            ┌────────┐ ┌────────┐          ┌───────────┐
   │ workflow│            │ config │ │ store  │          │ archi  ★  │
   └────┬────┘            └───┬────┘ └───┬────┘          └─────┬─────┘
        │                     │          │                     │
        └──────────┬──────────┴──────────┴─────────────────────┘
                   ▼
            ┌────────────┐
            │ foundation │  目录常量组 / workspace 身份段 / cancel 原语
            └────────────┘

  workflow ⇄ archi 互不依赖（硬约束，desktop-crate-layout spec 已写死）
```

新建 `crates/core/archi`：纯逻辑、零 Tauri、零 store、零 spawn。负责模型解析、内存图、结构校验、文档关联、DTO。

### 6.2 目录常量（foundation）

`foundation::layout` 目前只有 changes / archive / explores 三棵树。新增四个**派生函数**，不动 `Layout` struct：

```
archi_dir(root)            → openspec/architecture           （域根，前缀单点）
archi_models_dir(root)     → <archi_dir>/models
archi_decisions_dir(root)  → <archi_dir>/decisions
archi_config_path(root)    → <archi_dir>/config.json
```

按 foundation 的「第二个消费者到场才下沉」纪律，这些项落地时需在 `desktop-crate-layout` 的 spec 里登记为新增消费者。`openspec` 域根常量已存在（`domain_dir_name`），`architecture` 段由 `archi_dir` 单点持有，其余全从它派生——零散字面量不进任何 crate。

（报告目录常量 `archi_reports_dir` 归检查侧 explore。）

---

## 7. 语料与 Spike

```
crates/core/archi/tests/
├── corpus/
│   ├── dsl/<case>/*.toml      输入模型
│   └── expected/<case>.json   期望（elements / relations / docs / 诊断）
├── golden_test.rs             对拍（与 crates/core/checks/tests/golden 同风格）
└── vocabulary_test.rs         三方一致性（§4.4）
```

### 7.1 模型语料

注释、多文件、数组值、`gate` / `docs` 字段、嵌套包含、空模型、缺必填字段、非法 kind、值类型非法。

其中语法错 / 非法值 / 未知字段 / 重复 key 的期望来自 parser（§2.4 已列），不需自己设计错误文案。

### 7.2 结构校验语料

| 用例 | 期望 |
|---|---|
| `A -> B` 且 `B -> A` | error：成环（SCC 输出环路径），kind = `cycle` |
| `A -> anc(A)`（A 被 anc 包含） | error：依赖自己的祖先，kind = `dependsOnAncestor` |
| 关系引用不存在的元素 | error：未定义元素，kind = `unknownReference` |
| 缺必填 `kind` | error：missing field（parser 给出 + 位置） |
| 同一 FQN 重复声明 | error：`duplicate key`（parser 给出 + 指向冲突 key） |
| 无环有向图 + 合法祖先关系 | clean |
| 命令面写前校验 vs 工具面写后发现 | 前者拒绝写入；后者写盘成功、读取报错 |
| 有任一结构错时请求画布数据 | 返回停摆态 + 错误列表（含 file:line），零图数据 |
| 无结构错时请求画布数据 | 返回完整图数据 |

### 7.3 文档关联语料

| 用例 | 期望 |
|---|---|
| `docs` 指向存在的文件 | `docs` 含该条目，`source: "explicit"`，排第一 |
| `docs` 指向不存在的文件 | `warnings` 有 `docNotFound`；`docs` 不含该条目 |
| 元素 `path` 目录下有 `README.md` | `docs` 含该条目，`source: "adjacent"` |
| `path` 目录在测试夹具内（`__tests__/`） | 其下 md 不出现 |
| ADR 正文提及元素 FQN | `docs` 含该 ADR，`source: "text"` |
| 同一文件被显式与邻近同时命中 | 合并为一条，`source` 取强者（`explicit`） |
| 元素无任何关联文档 | `docs: []`（不报错、不告警） |

### 7.4 检索语料

| 用例 | 期望 |
|---|---|
| `query = "auth"` | 命中 FQN 含 auth 的元素 + 文件名含 auth 的文档 |
| `query = "AuthToken"`（驼峰） | 命中 `App.Auth.Token`（分词后 auth / token 均匹配） |
| `query` 命中文档正文但不在路径 / FQN 中 | **不命中**（不做全文搜索，刻意） |
| `query` 命中 `openspec/specs/` 内内容 | **不命中**（§3.4 边界 2） |
| `scope = "elements"` | `docs` 恒空数组 |
| `query` 为空串 | `Err`（参数校验） |

### 7.5 模型编辑与归档确认语料

| 用例 | 期望 |
|---|---|
| agent 在 worktree 用 `write` 新建 `models/03-billing.toml` | 写入成功（`models/` 不在 `write_protection` 内） |
| 同上提交后归档 | 归档确认面出现架构变更区块（元素 +N） |
| agent 写坏模型文件（语法错） | 写盘成功；`archi_model` 返回结构错不 panic |
| 模型有改动但代码无改动 | 归档确认面仍出现架构变更区块 |
| 模型无改动 | 归档确认面不出现该区块（零占位） |
| `archi_module` 返回 `modelFile` + `modelLine` | 指向声明该元素的文件与起始行，可直接用于 `edit` |
| 模型改动与主仓冲突（rebase） | 中断交人（§5.5），不进 agent 解算 |
| 归档确认「查看新图预览」 | 画布渲染改动后的模型图 |

### 7.6 Spike

| Spike | 内容 | 验证什么 |
|---|---|---|
| **G1 解析 + 结构校验** | §7.1 / §7.2 语料；`petgraph` 做 SCC 与祖先链判定；解析失败返回结构化 errors 不 panic | 解析语义 + 图算法；零 UI |
| **G2 前端画布** | `/architecture` 路由 + `ArchitectureCanvas`；两处挂载点；停摆态；抽屉；改 `gate` 档写回 | UI 复用度、停摆态是否好用、1-hop 是否够 |
| **G3 agent 工具与冷启动** | §7.4 / §7.5 语料；空模型仓库起一个 change，agent 在 worktree 生成模型 → 归档确认看到 diff → 合回主仓 → 主仓视图建图 | 「空图起点」能否落地；归档确认面的模型区块够不够看得出问题 |
| **G4 三方一致性** | 一份含全部词汇的模型语料，对拍进程内判定 / `archi_module` / IPC DTO 三面的词汇覆盖集 | 三个消费面是否真的同源；新增词汇是否会被静默漏掉（§4.4） |

---

## 8. 硬约束

| 约束 | 出处 | 违反后果 |
|---|---|---|
| `workflow` 与 `archi` 互不依赖 | `desktop-crate-layout` spec | crate 图破，纯读域引入编排行为 |
| core 禁 Tauri、core 禁 store | 同上 | 解析域被持久化 / 壳层污染 |
| 七工具封闭集、下发面单源 | `tools.rs` `TOOL_NAMES` / `policy.rs` / `compose.rs` / 16 份 prompts | 越权工具幻觉可执行；prompt 逐字断言测试红 |
| 显式取数、无 watch 无轮询 | `desktop-file-watch` spec | 画布想自动刷新必须先改 spec，不能偷加目录监听 |
| `config.json` 已知键七项，其余进 `extra` | `core/config/src/lib.rs` `KNOWN_KEYS` | 新增配置不登记 → 静默 passthrough |
| 架构域配置只落 `openspec/architecture/config.json` | §3.3 | 配置来源分散，用户不知道改哪 |
| `archi` 目录字面量只经 `foundation::layout::archi_dir` 派生 | §6.2 | 零散 `"architecture"` 字面量散进各 crate |
| 模型 = 源码：agent 在 worktree 内可自由改模型文件，但回合必须人工确认 | §5.3 | 设计权旁落；模型偷渡进主仓 |
| 归档确认面 MUST 单列模型 diff | §5.4 | 模型改动混在几十个代码文件里被橡皮图章 |
| `models/` 与 `architecture/config.json` MUST NOT 纳入 `write_protection` | §5.3 | 规则是绝对路径 glob，worktree 内同样命中 |
| 模型解析失败 MUST 返回结构化 errors，绝不 panic | §5.3 | 写坏模型期间整个架构视图崩溃 |
| 模型文件冲突 MUST NOT 进 agent 解算 | §5.5 | LLM 替人做架构裁决，冲突解错无人能察 |
| 归档 Preflight MUST 检出 rebase 半程并挡回 | §5.5 | 重跑会在 rebase 中间态 commit |
| change 期一切架构读写只碰 worktree 那棵模型 | §4.1 | 读到主仓基线模型，门禁永远滞后一个 change |
| 有结构错时画布 MUST 停摆（不画残缺图） | §4.2 | 畸形图的读法本身歧义，且要维护一套降级样式 |
| `ModelGraph` 单点构建，三个消费面零改写投影 | §4.4 | 出现第二事实源，三方结论可能不一致 |
| 新增 kind / gate / 诊断词汇 MUST 同步进三方对拍语料 | §4.4 | UI 静默不显示某类错误 |

---

## 9. 未决问题

1. **模块接口协议（§1.3）**——本 explore 唯一没有结论的部分。三种候选形态从「只落引用」到「完整签名契约」，需要单独裁决；提出前必须先回答「契约权威在哪」与「两套提取器怎么对齐」这两个当初否决它的理由。

2. **归档确认 UI 的信息密度**——§5.4 的三层（摘要 / 模型 diff / 新图预览）够不够？要不要再加「哪些代码文件改动了这些元素覆盖的路径」这类交叉信息？留到 G3 实测后定。

3. **AST 演进与容错**——未知 metadata key 忽略、未知 kind 报错（与严格性一致），写 spec 时定。

---

## 10. 不在本 explore 范围

| 事项 | 归属 |
|---|---|
| 代码扫描、依赖归属、漂移判定、门禁挂载、报告结构、产物标准 | `archi-check-and-gate` |
| 「模块文档替代 specs」的迁移 | 需独立 change（§3.4 已列难点） |
| `archi` 与 specs 的最终关系 | 届时再裁决，本 explore 不预设 |
| 跨语言依赖（TS 调 Rust 的 Tauri 命令） | 不在本期 |
| 符号级契约核对 | 与 §1.3 的 C 方案同源，需独立能力 |
