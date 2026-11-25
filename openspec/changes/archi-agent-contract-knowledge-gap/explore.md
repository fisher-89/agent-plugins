# 架构模型知识工具化：整合方案（v2）

workflow_type: requirement

> **状态：设计定稿；前置片 P1 / P2 / P3 均已落地，只剩切片 A。** 目标是把「模型
> TOML 怎么写」这类语法知识从 agent 系统提示迁进**读写工具**，文档只保留架构 Agent 的
> **职责**与**工具面**。
>
> **沿革**：v1 是「契约文档 / API 知识缺口」分析（5 个缺口）；v2 依用户判据改为
> 「TOML 知识工具化」。探索途中实测出的两处文本漂移已独立落地并归档为变更
> `archi-model-path-and-silent-trap`（`openspec/changes/archive/2026-11-24-…`），
> 本方案已按其落地结果修正（见 §2.4）。
>
> **阅读顺序**：§1 判据 → §2 基线 → §3–§5 目标形态 → §6–§7 迁移与撞墙点 →
> §8–§9 落地与验收 → §10 **全部裁决已结案** → §11 前置片落地状态（P1–P3 已完成）。
>
> **已裁决（2026-11-24，两批）**：
> ① 切片 A **一次做完**；**不做 L3b**（L3a 双写面为终态，持续观察）；截断优先级
> **`warnings` 最高 + 入参上限降概率**（§5.4 / §5.7）。
> ② `archi_scaffold` 词典取 **B1**（§8 切片 B）；`config.json` **不进写面**；
> **支持内联删边**（§4.1 / §4.3）；乐观锁做 **`expect` 哈希前置校验**（§4.7，附
> `archi_module.modelHash` 读侧闭环）；**元素插入位置后置**到独立篇
> [`toml-sorting-and-layout`](toml-sorting-and-layout.md)。
>
> **三片前置已全部落地（2026-11-24）**：P1 `archi-contract-authoring-guidance`、
> P2 `archi-scaffold-field-dictionary`、P3 `archi-edit-engine-primitives`。
> 落地实况与原设计的七处偏差见 §11.1，其中**两处改变了 A 的实现前提**（§11.1 末两条）。

---

# 一、判据与目标

## 1.1 两条输入判据

1. **知识归属**：当前 TOML 知识更适合转化成工具（读、写）；文档只保留架构 Agent 的
   职责与对应工具。
2. **响应自足**：工具的响应必须让 agent 拿到「改进所需的必要信息」；出现未阻断的
   异常时，在**修改成功**的返回里加 warning。

## 1.2 机制依据：spec 自己写下了这条前提

`desktop-archi-agent` L489（漂移修复时新增）原文：

> 归 C 档的理由仍成立但须换成**动作前不可判定**（**写面是自由文本 `write` / `edit`**，
> 结构诊断只在写盘后可得；四档表的「不可判定」即此义）。

即：**语法类知识留在散文的唯一理由是写面不可控**。提供结构化写面后，该前提对这条通路
失效；配合同 spec「移动分层的原子性」（只删不加 / 只加不删同样违约），
**删骨架语法段不是可选项，而是条款要求**。

```
四档表：A 工具承载 / B 一行指针 / C 散文纪律 / D 模板工具
                        ↑
        语法知识的 C 档身份 ⟸ 「动作前不可判定」
                              │
        结构化写面 ⇒ 判据在动作前可得 ⇒ 语法知识失去 C 档依据
                              ↓
                    迁往 A（工具响应 / Err）与 D（模板 / 字段词典）
```

## 1.3 目标形态

```
知识（常驻 prompt）                  工具（按需出现）
┌──────────────────────────┐        ┌──────────────────────────────────┐
│ 职责：模型=结构+三个指针    │        │ archi_scaffold  字段词典          │
│ 边界：只改架构目录 / 评审   │        │ archi_element   元素 upsert/del   │
│ 工具清单：读5 / 写2 / 自查 │        │ archi_relation  关系 upsert/del   │
│ 留给判断：kind / path / api│        │   ├ 参数 schema = 字段清单        │
└──────────────────────────┘        │   ├ 自证链（判据单源）            │
   ≈1.3 KB（现 2596 B）              │   ├ Err  = 判据+当前值+下一步     │
                                     │   └ warnings = 写后必现的判据债    │
                                     └──────────────────────────────────┘
```

**核心性质**：写工具的全部「知识」都是**查表 + 转述**——不新定义 severity、不新定义
诊断词、不另拼路径、不重实现判据。这与被修掉的漂移病因（某处自己写了一份）正好相反。

---

# 二、基线（实测）

## 2.1 知识骨架分段（`archi-knowledge.md`，LF 2596 B）

| 段 | 字节 | 内容 | L3 后 |
|---|---|---|---|
| 位置与键形 | 438 | 聚合序 / 带引号 FQN / 点分失败 / metadata 等价 / 四键 | **迁出**（工具不产坏文本） |
| 字段纪律 | 827 | `path` 基数三口径 / `entry` / `gate` / `api` / `doc` / `apis` | **大部分迁出**（留「path 覆盖到哪」判断） |
| 元素类型 | 454 | `kind` 四值语义 | **迁出**（→ 工具 enum 描述 + 词典） |
| 层级与读数 | 330 | 祖先先声明 / 计数单源 / 配置两键 | **部分迁出**（祖先→Err；计数→自动；配置留一行） |
| 边界与自检 | 262 | 不写正文 / 自查指路 | **保留**（职责） |
| 写面 | 259 | 只改架构目录 / 直改主仓 / 刷新 | **保留**（职责 + 产品行为） |

可迁出 ≈ **1850–1900 B（约 72–74%）**；目标骨架 ≈ **1.2–1.3 KB**。

## 2.2 工具面（`mount.rs` 静态清单，现 16 项）

七内建 + `create_explore_note` + **五 archi 只读** + 两 DB 只读 + `propose_config`。
五 archi：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`
（全 `ToolExec::Archi`，零写盘）。

## 2.3 写面现状

- 模型写权 = `write` / `edit` 自由文本；两条通路（archi 会话 / change 的
  `proposal|dev-design` × `executor`），其余相位由会话级锁挂 `graph/model/**` 禁区
  （`compose.rs::model_write_lock`）。
- 已有唯一结构化写原语：`archi::edit::{set_gate, plan_set_gate}`（渲染自证 + 回读一致 +
  零文本产出），消费者是画布命令 `archi_set_gate`（`src/commands/archi/mod.rs` L142）。

## 2.4 已落地的漂移修复（本方案按此修正）

| 落地项 | 对方案的影响 |
|---|---|
| `package` 的 `path` **至多 1 条**，≥2 报 `invalidValue` + 整图停摆（`parse/toml.rs::validate_path`） | 写工具**不重实现**该判据（§4.6） |
| 三个纯分层容器改 `kind = composite`；`package` 不再兼任「分组」 | `kind` 参数描述必须含此语义 |
| `noPath` 按 `kind` 分档；`package` 空 `path` 语义 = 「`kind` 可能用错了」 | 写工具 warning 的直接来源（§5.2） |
| `metadata` 子表与内联表等价（行为测试钉住） | 骨架删该句；模板注释保留 |
| spec 四份 + ADR + 模型三行同批 | 迁移清单补三面（§7.4） |

---

# 三、目标知识（草稿）

```markdown
# 架构模型须知

## 职责
你维护架构模型与契约资产。模型只写**结构**（元素 / 依赖）与三个**指针**
（`api` / `entry` / `doc`）：接口、导出符号、协议正文一律归契约资产。

只改架构目录内的文件；直改主仓，随 `git diff` 评审。写盘后画布**不会自动更新**，
提示用户「刷新」。续接历史会话先取当下模型。

## 工具面
字段面、键形、缺省、值域与生成物全部由工具承担——MUST NOT 凭记忆手拼 TOML：
- 读：`archi_search`（发现）/ `archi_module`（定位与现状）/ `archi_drift`（漂移读数）/
  `archi_api`（契约两轴 + 草稿）/ `archi_scaffold`（字段词典）
- 写：`archi_element`（元素 upsert / 删除，可内联出边）/ `archi_relation`（单条关系
  upsert / 删除）——参数面即字段清单，坏文本零落盘
- 自查：改完 `archi_drift` 非空即逐条修；`archi_api`（`action = "check"`）查两轴

工具表达不了的（重排文件 / 批量整理）才手改 TOML：改完必须 `archi_module` 自查——
坏模型由读取面报错（结构诊断带 `file:line`），不是静默的。

## 留给判断
`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——这些
没有机械答案。工具的 `kind` / `gate` / `api` 参数面给出全部合法值与语义；
纯分组容器用 `composite`。
```

约 1.25 KB。**「手改出口」一段是双写面终态（§8 切片 C）的永久内容**——一句话的代价，
换来手改通路的完整性，不是待删的过渡句。

---

# 四、工具契约

## 4.1 `archi_element`（元素 upsert / delete）

```jsonc
{
  "action":    "upsert",                 // 封闭二值：upsert / delete
  "element":   "Desktop.Views.Auth",     // 必填，完整 FQN
  "kind":      "module",                 // 新建必填；已存在省略 = 不动
  "path":      ["./packages/desktop/src/views/auth/"],
  "gate":      "warn",
  "api":       "declared",
  "entry":     "./packages/desktop/src/views/auth/index.tsx",
  "doc":       null,                     // 显式 null = 清空
  "summary":   "登录与注册视图",
  "metadata":  { "owner": "team-x" },    // 渲染为内联表
  "relations": [                         // 出边批量（from 恒 = element）
    { "to": "Desktop.Components.Button", "desc": "表单提交按钮" },
    { "to": "Backend.Shell", "desc": "登录 IPC 命令面", "apis": ["login"] },
    { "to": "Backend.Legacy", "action": "delete" }            // 内联删边（D5）
  ],
  "expect":    "sha256:1f3a…",           // 可选：文件乐观锁（D7，见 §4.7）
  "file":      "07-billing.toml",        // 可选：落点覆盖
  "create":    true,                     // 可选：允许新建模型文件
  "cascade":   false                     // 仅 delete：级联删除引用 / 子元素
}
```

`relations[]` 四键：`to` 必填；`desc` / `apis` 可选；`action` 可选（缺省 `upsert`，
另一值 `delete`）。

## 4.2 `archi_relation`（关系 upsert / delete）

```jsonc
{
  "action": "upsert",                    // upsert / delete
  "from":   "Backend.Shell",             // 必填
  "to":     "Backend.Core.Archi",        // 必填（delete 的键即 from+to）
  "desc":   "架构命令面投影图与模块详情",
  "apis":   ["ArchiGraph"],              // 选填名字清单
  "expect": "sha256:1f3a…"               // 可选：文件乐观锁（D7）
}
```

两工具不合并的理由：元素字段面与关系字段面**完全不相交**，合一只能靠 `oneOf` 或大量
互斥可选位，违背「平铺」——而平铺是「每字段带 `description`（缺省 / 门限 / 陷阱）」
的前提，否则知识又溜回散文。

## 4.3 更新语义

| 写法 | 含义 |
|---|---|
| 字段省略 | **不动** |
| 字段给 `null` | **清空** |
| `relations` 省略 / 空数组 | 出边不动 |
| `{"to": X}`（无 `action`） | 该边 upsert（改 `desc` / `apis`；不存在即建） |
| `{"to": X, "action": "delete"}` | 该边删除；不存在即 `Err`（列出同 `from` 的现有出边） |
| 未列出的既有边 | **保持不动**（全量替换会一次漏列即静默删依赖） |

三条约束（D5 支持内联删除后新增）：

1. **同一 `to` 在一批里只出现一次**：重复即 `Err`（`relations[1] 与 relations[3] 都是
   Backend.Legacy`）——否则「先 upsert 后 delete」的语义取决于数组序，不可判定。
2. **`delete` 项不得带 `desc` / `apis`**：带了即 `Err`（删除动作与字段赋值互斥，
   静默忽略会让 agent 以为赋值生效）。
3. **删除仍然显式**：内联 `delete` 省的是**往返次数**，不是「把删除藏进批量」——
   它和独立 `archi_relation delete` 是同一个显式动作，只是同批执行。

「省略 = 不动」是为了避免「只改 gate 也要先读全字段再回写」——那是手写 TOML 最容易丢
字段的地方。`metadata` 参数是 `Map<String,String>`，**子表陷阱在参数面不可表达**。

## 4.4 落点策略

| 场景 | 行为 |
|---|---|
| 省略 `file` | 落到**最近已声明祖先所在文件**（零决策命中本仓现有分布） |
| 祖先链缺失 | **Err**，点名缺失段 + 该祖先候选落点（不自动造占位祖先：`kind` 是判断） |
| `file` 不存在且无 `create` | Err + 提示 `create = true`（防拼错名字造成幽灵文件） |
| `create = true` + `file` | 新建文件；表头由工具生成 `# <名>（1 元素 / 0 关系）`（工具不猜语义） |
| 关系 upsert | 落 `from` 元素所在文件（与现有 04/06 的 from 分组一致） |

现有六个模型文件是**按分层切片**（01 app / 02 core / 03 infra / 04 shell / 05 tools /
06 frontend），不是一元素一文件。默认新文件会把 06（32 元素 / 77 关系）切成 32 个，
评审面碎掉——故默认最近祖先，`create` 留给「新分层」这类真架构动作。

**D9 裁决：文件名里带语义序号（`01-app` / `06-frontend`），工具无法推导，故 MUST NOT
引入「按 `kind` / 首段 FQN 的落点约定」**——那会成为新的隐式知识，且 `Backend.Core` 与
`Desktop.Views` 的同文件反例无法用规则表达。`file` 由 agent 给，工具只管渲染与写入 +
表头生成（标题取文件名去序号，`# 07 · <名>（1 元素 / 0 关系）`，语义部分不猜）。

> 与 D6 的边界：D6（元素**在文件内**的位置）已后置到
> [`toml-sorting-and-layout`](toml-sorting-and-layout.md)；D9 是**新文件该叫什么**。

## 4.5 删除

| 情形 | 缺省 | `cascade = true` |
|---|---|---|
| 元素被关系引用 | Err，列出引用条数与端点 | 级联删这些关系 |
| 元素有子元素 | Err，列出子元素 FQN | 级联删子树（深度优先） |
| 关系 delete（独立工具 / 内联 `action`） | 键 = `from + to`；不存在即 Err（列出同 `from` 的现有关系） | — |

破坏性动作必须显式（与 `create` 同一条取向）。内联删边（§4.3）与独立删边共用同一
判据与同一文案族——**MUST NOT 为内联路径另写一份「不存在就静默跳过」的宽松版**。

## 4.6 自证链：判据单源

`validate_path`（`kind` × `path.len()`）证明：**跨字段判据无法用 JSON Schema 表达**。
若写工具重实现一遍，就是第二份判据——正是被修掉的漂移病因。因此：

```
archi_element(input)
  ├ ① 参数 schema     单字段判据（type / enum / required / maxItems / maxLength +
  │                   跨字段基线：kind 新建必填），框架报错，非工具文案
  ├ ② 渲染            toml_edit 改文档（内存；带引号 FQN 单键 / 内联 metadata / [[relation]]）
  ├ ③ TomlParser      单文件诊断 → 原文转述（≤20 条 + 聚合计数） + 「下一步」一句，零落盘
  │      syntaxError / missingField / invalidValue(含 kind×path 基数) /
  │      unknownField / duplicateElement(同文件)
  ├ ④ parse_dir       全模型诊断 → 同上转述，零落盘
  │      duplicateElement(跨文件) / cycle / dependsOnAncestor /
  │      unknownReference / orphanHierarchy
  └ ⑤ 回读字段全等 → GateWrite → 壳层 write_atomic（唯一落盘点）
```

写工具**自有的**判据只有三处（都不在 parser 射程内）：落点、删除级联、删除键定位。
`edit::set_gate` 已是这个形状（「渲染后重解析，零诊断才产出文本」），扩的是字段数。

**不做的事**：不重实现 `path` 形态校验（spec 明写 `./` 前缀 / 目录尾 `/` 刻意不校验）、
不跑 `drift::check`（含 scan，写是高频动作）、不读 parser 之外的第二套模型。

## 4.7 乐观锁：`expect`（D7 裁决 ②）

范式取自既有 `edit` 工具（`sdk/tools.rs` L386-411）：**先声明"我以为的当前状态"，对不上
就拒绝**——那里是 `old_string` 必须命中且唯一，这里是 `expect` 必须等于当前文件哈希。

```
调用方：archi_module → 看到 modelHash（或上一轮写工具响应的 hash）
        → archi_element { …, expect: "sha256:1f3a…" }
工具  ：当前文件 hash ≠ expect  ⇒ Err（零落盘）
        hash 相同           ⇒ 正常自证链，写后回新 hash
```

四条设计决定：

1. **`expect` 可选**（缺省不校验）：与 `edit` 的 `old_string` 必填不同——`expect` 是
   防并发覆盖的**增量保护**，不是写入的必要条件；首次建元素时调用方本来就没有基线 hash。
2. **hash 粒度 = 单个模型文件**（不是 `assets_hash` 的全资产）：写工具的落点就是一个
   文件，全资产 hash 会把"别的文件被改"也判成冲突，那是误报。
   - ✅ **已由 P3 交付**：`drift::file_hash(path)` + `file_hash_of(path, content)`，与
     `assets_hash` 共用同一 `hash_files` 循环（`drift/mod.rs` L617-629）。
   - 形态与 `assets_hash` 同前缀 `sha256:<hex>`。
3. **响应回 `hash`**：成功响应的 `file` 旁附 `hash`（写后新值），让调用方**无需再读**
   就能连续写同一文件。这是 `expect` 闭环的另一半，缺了它 agent 每次都要多一次读。
4. **冲突文案给现状**（响应自足）：

```
Err: 模型文件 06-frontend.toml 已被改动（expect sha256:1f3a… / 当前 sha256:9c2e…）。
     先 archi_module 取现状再重试；或省略 expect 直接覆盖。
```

最后一句「或省略 expect 直接覆盖」是有意给的**逃生阀**：`expect` 是保护而非门禁，
把"如何绕过"写清楚，比让 agent 卡死或臆造参数更好。

**闭环缺口由 A 补（D12 裁决 (a)）**：P3 只导出 `file_hash` / `file_hash_of`，没有 DTO
承载它——`archi_drift` 的 `modelHash` 是全资产聚合，`ArchiElement` 只有 `modelFile` /
`modelLine`。故 A SHALL 给 `ArchiModuleDetail` 增一个单文件 hash 字段：

| 面 | 增量 | 约束 |
|---|---|---|
| `ArchiModuleDetail.modelHash` | `String`（`sha256:<hex>`） | 与 `file_hash` **同一函数**产出、同前缀；`modelFile` 已知，零额外读盘（该文件在 `plan_*` 已必然读过） |
| `bindings.ts` | 重导（`ArchiModuleDetail` 的 JSDoc 随动） | 生成物勿手改 |

三处构造点随动：`dto.rs` 的投影、`commands/archi/mod.rs` 的 `archi_module` 出口、
`dto_test.rs` 的字段面断言。这样「读 → 带 expect 写」闭环成立，agent 不必推理
"我有没有基线"。

`archi_set_gate`（画布命令）同样走 `plan_element_upsert`，故实现上自动获得 `expect` 能力；
命令面**不暴露**该参数（前端单用户、无并发场景，暴露只增加 IPC 面）。

---

# 五、反馈面

## 5.1 三通道

```
                    ┌─ Err        阻塞，零落盘 ── 判据 + 当前值 + 可复制重试参数
一次写工具调用 ──────┼─ warnings[] 已落盘，有未阻断事实 ── 判据 kind + severity + 下一步
                    └─ changed[]  已落盘，幂等凭据 ── 实际变化的字段名
```

`hints` **并入 `warnings`**（不设第二通道：拆开后 agent 可能只看 message 不看 hint）。

## 5.2 七条 warnings（零新词汇）

只由**零成本事实**推导（写工具已持有的字段值 + `Path::exists`），MUST NOT 跑 drift：

| # | 条件 | kind（既有词） | severity | message 尾句 |
|---|---|---|---|---|
| 1 | `package`/`module` 且 `path` 空 | `noPath` | warn | 纯分组容器请改 `composite` |
| 2 | 某条 `path` 不在盘 | `pathNotFound` | warn | 先行声明可忽略 |
| 3 | `api = declared` 且契约 md 不在盘 | `docMissing` | warn | 用 `archi_api` 的 `bootstrap` 取草稿 |
| 4 | `api = none` 且 `doc` 空 | `docMissing` | warn | 入口文档经 `doc` 声明 |
| 5 | `api = none` 且 `doc` 指不到盘 | `docNotFound` | **error** | 先建文件或改 `declared` |
| 6 | `api = none` 但契约 md 已在盘 | `apiUnexpected` | warn | 删契约 md 或改 `declared` |
| 7 | 删除时契约 md 仍在盘 | `apiUnexpected` | warn | 一并清理（否则无人看管，见 §5.5） |

kind 字面量取自 `ArchiDriftKind` / `ArchiApiFindingKind`；severity 取自
`ArchiApiFindingKind::severity()`（`drift/report.rs` L255 单点）；路径用
`api::paths::contract_path` / `doc_path`。写工具**不定义档位、不另拼路径**。

### 降噪不变式（必需）

本仓多数元素是 `api = declared` 且契约 md 缺失；若不限流，每次 upsert 都回同一条
`docMissing`，三次之后 agent 就整体忽略该通道。规则：

**每条 warning 必须能由 `changed` 中的某字段解释**（`created = true` 视为全字段变化）：

| warning | 触发字段 |
|---|---|
| `noPath` / `pathNotFound` | `path`（或 `kind` → `package`/`module`） |
| `docMissing` / `docNotFound` / `apiUnexpected` | `api` 或 `doc` |

只改 `summary` / `gate` / `metadata` ⇒ `warnings` 恒空。这是可直接断言的不变式。

## 5.3 Err 分层与「前置」

| 层 | 场景 | 承载 |
|---|---|---|
| E1 入参 | `action` / `element` 缺失、FQN 空段、`kind`·`gate`·`api` 值域、**结构 / 长度上限**（§5.7） | schema（`required` / `enum` / `pattern` / `maxItems` / `maxLength`） |
| E2 前置读数 | 模型已有结构诊断 | 工具文案（沿用 `graph_diagnostics_error` 全量形） |
| E3 语义 | 祖先缺失 / `doc` 门限 / 关系成环 / 端点不存在 / 跨文件搬元素 / 删除有引用 / kind×path 冲突 / **内联 `to` 重复** / **`delete` 项带 `desc` \| `apis`** | 工具文案 |
| E4 落点 | 文件不存在无 `create` / `create` 冲突 / 非模型目录 | 工具文案 |
| E5 删除 | 元素 / 关系（含内联 `delete` 的边）不存在 | 工具文案（附候选 / 现有出边） |
| E6 自证 | 渲染后仍有诊断 / 回读不等 | 工具文案 + **零落盘** + 「这是工具缺陷」 |
| E7 并发 | `expect` 与当前文件 hash 不等（§4.7） | 工具文案（expect / 当前双值 + 两条出路） |

四条硬纪律：**第一行自足**；**字面与词表同源**（`gate` 不写「档位」、
`unknownReference` 不译中文，消费方可 grep）；**不猜**（给候选不代选）；**一次报全**
（与 `TomlParser` 不做首错短路同构；上界与超出后的聚合见 §5.7 ②）。

`kind` × `path` 冲突的「前置」示例（parser 绝不会这么说）：

```
Err: 元素 Backend.Billing 当前 kind = module、有 3 条 path；改为 package 时 path 至多 1 条。
     两条出路任选（可直接复制作参数）：
       ① {"kind":"package","path":null}    // 改判纯容器包，清空 path
       ② {"kind":"composite"}              // 保留 path 则改判纯分组（path 选填）
```

该情形**没有「成功写入」这一支**（写出来必停摆），所以只能 Err；但「提示」实质在此体现：
明说这是 `kind` 变更的连带 + 给可复制参数。

批量关系的错误定位（D5 支持内联 `action` 后，下标必须二维可辨）：

```
Err: relations[2]（action = delete, to = "Backend.Legacy"）不在模型里：Backend.Billing
     的现有出边是 Backend.Shell、Backend.Core.Checks（无 Backend.Legacy）。
     删不存在的边一律显式失败——去掉该项，或核对 to 的 FQN。
```

`relations[i]` 的 `i` 是 **0 基还是 1 基必须与其它错误面统一**（本方案取 1 基，与人在
评审时数行的习惯一致），且下标在文案里 MUST 与 `action` / `to` 同时出现——只给下标
不能让 agent 定位到数组项（它可能重排过参数）。

## 5.4 响应体

**字段顺序即截断优先级**：`cap_result`（`sdk/tools.rs` L91）是**尾部截断**（保留前
`MAX_RESULT_BYTES = 50 000` 字节），所以 `warnings` MUST 是响应体除身份字段外的
**第一个**字段——否则超限时先被砍掉的恰是「改进所需信息」。

```jsonc
{ "warnings": [                                     // ① 最高优先：截断最后才轮到它
    { "kind": "noPath", "severity": "warn",
      "message": "元素 Backend.Billing 是 package 且无 path：若它是纯分组容器请改 kind = composite" },
    { "kind": "docMissing", "severity": "warn",
      "message": "api = declared 且派生契约 md 不在盘：<models>/Backend/Billing.md——用 archi_api 的 bootstrap 取草稿" }
  ],
  "action": "upsert", "element": "Backend.Billing",   // ② 身份
  "file": ".dev-team/architecture/graph/model/02-backend-core.toml",
  "hash": "sha256:9c2e…",                             // ② 写后新值（喂下一轮 expect，§4.7）
  "created": true, "changed": ["kind", "summary"],    // ③ 幂等凭据
  "relations": { "created": 1, "updated": 0, "deleted": 1, "unchanged": 2 },
  "counts": { "elements": 10, "relations": 10 },      // ④ 本文件（= 表头重算来源，可对拍）
  "model":  { "elements": 56, "relations": 129 } }    // ⑤ 全模型（最可牺牲：可另走 archi_drift）
```

- `warnings` 恒在场（空数组 = 无异常，与 `archi_drift` 同口径）。
- `changed` 空数组 = 幂等凭据（重复同值 upsert）。
- 响应**不含**元素 / 关系全文（要看现状用 `archi_module`）。
- `relations` 四计数（D5 后新增 `deleted`）：**`created + updated + deleted + unchanged`
  恒等于入参 `relations` 数组长度**（可直接断言的守恒式）；只写元素不带 relations 时
  四项皆 0（不是省略）。
- `hash` 是写后文件 hash，与 `expect`（§4.7）同一函数产出——**同一字段名、同一前缀**。
- `archi_relation` 的 `warnings` 恒空数组（关系面唯一零成本事实是「`to` 为 `api = none`
  且 `apis` 非空」，留档不实现）。
- 表头计数由工具重算 ⇒ spec「计数复述单源」的手工半边消失。

### 成功响应永不触发截断（可断言）

入参上限（§5.7）把 `warnings` 封在 7 条、其它字段全是计数与短串，故成功响应的结构
上界可算：

```
7 条 warning × ≤200 B ≈ 1.4 KB  +  其余字段（FQN / 路径 / 四个计数）≤ 0.6 KB
                                        ⇒ 成功响应 ≤ 2 KB ≪ 50 KB
```

因此 §9 验收面直接断言「成功响应序列化长度 < `MAX_RESULT_BYTES`」——**成功响应的
字段顺序是防御，不是必需路径**；真正会撞上限的只有 Err（E2-1 坏模型全量诊断）。

## 5.5 新发现：删除元素后的孤儿契约 md

`api::findings(root, elements)` 的语料只取**在盘元素**的载体，因此删元素后
`<models>/…/X.md` 残留**没有任何判据会报**。L3 让这件事容易发生（以前要手改 TOML）。
**D10 裁决**：只做动作面 warning（§5.2 第 7 条）——`delete` 成功时若契约 md 仍在盘，
回一条 `apiUnexpected`。**不做 drift 判据，且不列入 backlog**：B 方案（扫 `models/**`
减在盘元素载体、加第 14 个 `ArchiApiFindingKind`）依赖「`models/` 下除契约 md 别无他物」
这一假设，当前零持仓，不预付复杂度。

## 5.6 五条可测断言（验收「响应自足」）

| # | 面 | 断言 |
|---|---|---|
| 1 | `Err` | 含 `重试：` / `下一步：` / `改用 <工具名>` 之一，或含可复制 JSON 参数片段 |
| 2 | `warnings[].message` | 含对应判据 kind 字面量（可 grep 回定义） |
| 3 | `changed` | 空 = 幂等；非空时是参数面字段名的子集 |
| 4 | 降噪 | `warnings` 的 kind 集合 ⊆ 由 `changed` 诱发的 kind 集合 |
| 5 | 体积 | 成功响应序列化长度 < `MAX_RESULT_BYTES`（结构上界，§5.4） |

前三条管**内容足够**，第四条管**内容不泛滥**，第五条管**承载得住**。落
`edit/messages_test.rs` + `edit/warnings_test.rs`，仿 `骨架_指路表逐句在场` 的三列机制。

## 5.7 入参上限（schema 层，降 Err 体积）

`cap_result` 的另一个入口是 **Err**：E2-1「坏模型全量诊断」与「一次报全」的批量错误
都可能撞 50 KB。两道闸：

**① 入参结构上限**（E1 层，与 `required` / `enum` 同层，框架拦）：

| 参数 | 上限 | 依据 |
|---|---|---|
| `element` FQN | `maxLength` 256 | 本仓最深的 FQN 也不足 40 字符 |
| `path` | `maxItems` 32；每项 `maxLength` 1024 | 多根清单，实际 ≤2 |
| `metadata` | `maxProperties` 32；键 ≤64、值 ≤512 | 本仓全空 |
| `relations` | `maxItems` 64 | 单元素出边实际 ≤20 |
| `relations[].apis` | `maxItems` 32；每项 ≤128 | 名字清单 |
| `summary` / `desc` | `maxLength` 512 | 一行说明 |
| `entry` / `doc` | `maxLength` 1024 | repo 相对路径 |
| `file` | `maxLength` 256 | 模型文件名 |

与 `bash.intent` 禁用长度键（`mount_test.rs` L245）的差别：`intent` 是给人看的一句话、
语义无边界；这里的字段有真实取值边界，与 `enum` 同类。**MUST NOT 在引擎层重复检查**
（schema 已拦；重检查即第二处判据）。

**② Err 条目上限**（工具文案层）：E2-1 与 E3 的批量错误列**至多 20 条**，超出部分
按 kind 聚合：

```
模型存在 37 条结构诊断，拒绝写入（先修好模型），前 20 条：
  unknownField @ 02-backend-core.toml:31
  …
其余 17 条按 kind 计：unknownField ×12 / missingField ×5
修完前 20 条再调一次即可看到后续。
```

这条是对「一次报全」纪律的**显式限定**：报全的上界是 20 条，超出后给聚合计数——因为
一条超 50 KB 的 Err 会被 `cap_result` 砍成"前 20 条 + 被截断"，反而不如主动聚合。

---

# 六、知识迁移表

| 现有段落（字节） | 迁往 | 载体形态 |
|---|---|---|
| 位置与键形 438 | 写工具渲染 + parser 诊断 | 键形由 `toml_edit` 产出；点分失败由 parser 报 |
| 字段纪律 827 | `archi_element` 参数 schema + `archi_scaffold` 词典 + parser Err | 每字段 `description`（缺省 / 门限 / 陷阱） |
| 元素类型 454 | `kind` 参数 enum 描述（`ElementKind::ALL` 派生）+ 词典 | 四值语义逐值 |
| 层级与读数 330 | 祖先 → Err；计数 → 响应 `counts`；`config.json` 留一行 | — |
| 边界与自检 262 | **保留**（职责） | 自检句压成工具清单一行 |
| 写面 259 | **保留**（职责 + 产品行为） | — |
| （新增）手改出口 | 保留一句 | 双写面终态的永久内容 |

`SCAFFOLD_TEMPLATE` 同步升级为**字段词典**：全字段 + 必填 / 缺省 / 陷阱 + `kind`
四值语义（`ElementKind::ALL` 派生，零第二份字面量）。

---

# 七、撞墙点与同批改动

## 7.1 五处已固化立场（必须同批改写）

改写方向由 §8 切片 C 的终态决定：**两通路长期并存**，不是「工具取代手改」。

| # | 锚点 | 现文 | 改为 |
|---|---|---|---|
| 1 | `desktop-archi-agent` L335 | 「MUST NOT 存在专用『写模型』工具」 | 「写模型有两入口：专用写工具（canonical）+ `write`/`edit`（second path）」；原禁令删除 |
| 2 | 同 L346 | 「**写入不校验，读取报错**」 | 拆成两半：「写工具**坏文本不可产出**」（自证链）／「手改面仍由读取面报错」——两条通路各自的失败语义，MUST NOT 混为一句 |
| 3 | 同 L375 Scenario | 「无专用写工具（五只读）」 | 「两写工具 + 五只读；两通路并存，写工具非唯一入口」 |
| 4 | `dev-design-planner.md` L44 / `proposal-planner.md` L38 | 「（**无专用写工具**）」 | 指向 `archi_element` / `archi_relation`，并保留一句「工具表达不了的才手改」 |
| 5 | 同 L489 补充规则① | 理由已于漂移修复时改为「动作前不可判定」 | 补一句：**结构化写面对工具通路消除了该前提**（手改通路仍成立），故语法知识的 C 档身份只在手改面残留 |

第 2 条的拆半是本次改写里最容易做错的一处：原句把两种失败语义合并成一句口号，
改写时必须保留「手改面写坏模型仍是合法状态、由读取面报错」这一半，否则会连带
推翻读取面的既有责任（`graph_view` 停摆 / `archi_module` 诊断 / `archi_preclose` 兜底）。

## 7.2 注入矩阵与写权

`compose.rs::archi_matrix` 的写权行（现仅给 `scaffold`）需加两工具：

| (phase, role) | 现 | 后 |
|---|---|---|
| `("proposal","executor")` | search / module / scaffold | + `archi_element` / `archi_relation` |
| `("dev-design","executor")` | module / search / drift / scaffold | + 两写工具 |
| `archi` 臂 | 五只读 | + 两写工具 |
| 其余 | 只读不变 | — |

写权集合由 `model_write_lock` 已定义（archi ∨ change∧(proposal|dev-design)×executor），
故矩阵只需与它对齐——**MUST NOT 出现「矩阵给了工具但写权锁着」的错配**（加一条断言）。

`ToolTier`：两写工具落 `Write`（Default 档不放行）。

## 7.3 写引擎：P3 已交付（本节已落地，保留作对照）

```
edit::apply_element_spec / apply_element_delete              ← 纯函数层（源文本进、源文本出）
edit::apply_relation_upsert / apply_relation_delete
edit::plan_element_upsert(spec) -> ModelWrite                ← 计划层（读盘定位 + 自证）
edit::plan_element_delete(root, fqn, expect)
edit::plan_relation_upsert / plan_relation_delete
drift::file_hash / file_hash_of                              ← 与 assets_hash 共用 hash_files
        ↑
        └── 壳层 archi_set_gate = plan_element_upsert(ElementSpec { fqn, gate: Some(..) })
```

`plan_set_gate` 已删除（画布与未来 agent 工具共用同一实现）。`ModelWrite` 三字段
`{ file, content, hash }`；`hash` 由 `file_hash_of` 在**未落盘**状态下算出，与落盘后
`file_hash` 逐字相等。

**A 仍需补的三处**（P3 刻意留白，见 §11.3）：`cascade` 级联、`ElementSpec.relations`
内联出边、落点策略的「最近已声明祖先」。三处都长在已有函数上，不是新模块。

## 7.4 同批文档面
## 7.4 同批文档面（原清单漏项）

| 面 | 为什么必须同批 |
|---|---|
| `desktop-archi-drift` spec | `noPath` 分档语义是 warnings 第 1 条的措辞来源 |
| `desktop-archi-model` spec | `path` 基数 / metadata 形态是参数描述的依据 |
| `repo-archi-model` spec | 本仓 FQN×kind×path 表 + 计数条款 |
| ADR `0001-model-granularity.md` | package/composite/domain 分组语义 |
| `archi-knowledge.md` + 两个 `_test.rs` | 体量冻结值下调（2596 → ~1300，门限随动） |
| `bindings.ts` | 响应新增 `warnings` / `hash` 字段、`ArchiModuleDetail` 新增 `modelHash` 需重导 |
| `desktop-archi-agent` L483-500（分档表） | 分层判据本身要加一条：**工具可承载的知识 MUST NOT 留在散文**（§1.2 的机制依据），否则下一次加字段又会写散文 |

---

# 八、落地切片

## 切片 A（主变更，一次做完）

```
archi 写面（两工具 + warnings） + 知识收缩 + spec 立场 + 注入矩阵
```

**一次做完，不拆两步**（D1 裁决）。原子性约束：散文句替换为指路句（§7.1）与工具面
落地（§4）**必须同批**——只删不加即知识缺失，只加不删即知识重复，两者同样违约。

**P1 / P2 / P3 落地后的实际体量**（§11.7）：

| 面 | 已就位（前置片） | A 仍需做 |
|---|---|---|
| 渲染 + 自证 + 错误面 | `edit/mod.rs` + `messages.rs`（19 变体） | 级联 / 内联出边 / 落点三块增量 |
| 字段词典 | `SCAFFOLD_TEMPLATE`（P2） | — |
| 契约起草 | `BOOTSTRAP_NOTE` 六段（P1） | — |
| 工具声明 | — | `archi_element` / `archi_relation` 定义 + 平铺 schema + 8 条入参上限 |
| 反馈面 | — | `edit/warnings.rs` 七条 + 响应组装 |
| 注入与写权 | 画布命令已共用 `plan_*` | `compose.rs` 矩阵两行 + `ToolTier::Write` |
| 知识 | 骨架仍 2597 B | 收缩到 ~1300 B + 两测试冻结值下调 |
| spec | P1/P2 已改各自 Requirement | 五处立场（§7.1）+ 6 份文档面 |

约 **10–12 个文件**。体量本身就是「不拆两步」的理由：半个写面（工具有了、知识没删）
比没有更难评审。

## 切片 B（已落地：P1 + P2）

原文的切片 B 由两个前置片承接并已完成：

| 原切片 B 内容 | 落地于 |
|---|---|
| `archi_scaffold` → 字段词典（B1 形态） | **P2** `2026-11-24-archi-scaffold-field-dictionary` |
| 契约起草引导与工具描述补位（原属 v1 缺口 1–5） | **P1** `2026-11-24-archi-contract-authoring-guidance` |

三片落地后，§6 知识迁移表的两个接收端都已就位：

```
字段纪律 827 B ──→ archi_element 参数 schema（A 建）+ archi_scaffold 词典（✔ P2）
草稿形态 ──────→ bootstrap 六段 + 作者注（✔ P1）
```

故 A 删骨架「字段纪律」时，指路句指向的词典**已经存在**——这是 P2 必须在 A 前的理由，
现在条件已满足。

## 切片 C（不在本方案范围：L3b 不做）

**裁决：不写 L3b。** 模型目录进写保护锁面、`write` / `edit` 对模型目录一律拒绝这件事
**不列入本方案**，改为上线后持续运行观察。

因此本方案的终态是 **L3a（双写面长期并存）**，不是过渡形态。三处措辞随之确立：

| 面 | 长期形态 |
|---|---|
| 写工具 | **canonical 通路**（agent 首选，键形 / 值域 / 祖先 / 计数全在工具里） |
| `write` / `edit` 改模型 | **长期保留的 second path**（工具表达不了的才用），改完必须自查 |
| 骨架的「手改出口」一句 | **永久内容**，不是待删的过渡句 |
| `model_write_lock` 相位锁 | **不变**（仍掐非写权相位对 `graph/model/**` 的射程） |

观察期要看什么（不写进 spec，仅作运行时的自查清单）：`archi_drift` 的结构诊断是否
在自然使用中稳定归零；手改出口是否被用于工具能做的事（那说明工具文案不够省事）。

---

# 九、验收面

| # | 面 | 断言 |
|---|---|---|
| 1 | 知识体量 | 骨架 ≤ 新门限（估 1500 B）、全文 ≤ 新门限；实测值冻结 |
| 2 | 知识内容 | 「职责 / 边界 / 工具清单 / 留给判断」四段在场；**「手改出口」一句在场**（D3 终态，非过渡）；零键形示例、零字段值域行、零示例 TOML 围栏 |
| 3 | 工具面 | 两写工具在 `archi` 臂与两处写权行；`ToolTier::Write`；未命中相位零下发 |
| 4 | 判据单源 | 写工具零 `validate_path` 引用；三处自有判据（落点 / 级联 / 删除键）在案 |
| 5 | 自证 | 渲染后诊断非空 ⇒ `Err` + 零落盘；回读不一致 ⇒ `Err` + 零落盘 |
| 6 | 反馈 | §5.6 五条断言逐条红绿 |
| 7 | 幂等 | 同值重复 upsert ⇒ `created=false, changed=[]`，盘上字节不变 |
| 8 | 计数 | 写后表头计数与 `counts` 相等；`counts` 与全模型 `model` 可加 |
| 9 | 画布一致 | `archi_set_gate` 与 `archi_element(gate)` 产同一份文本（同实现） |
| 10 | 立场 | §7.1 五处改写后，spec 内零「MUST NOT 存在专用写工具」类**绝对禁令**残留；改为「写工具 canonical + 手改通路保留」两通路表述 |
| 11 | 入参上限 | §5.7 ① 八行逐行在 schema 内；引擎层零重复检查 |
| 12 | Err 聚合 | 诊断数 > 20 时列 20 条 + 按 kind 聚合计数（§5.7 ②） |
| 13 | 相位对齐 | 矩阵给写工具的 `(phase, role)` 集合 == `model_write_lock` 的不挂锁集合（防「给了工具但锁着」） |
| 14 | 内联删边（D5） | `{"to":X,"action":"delete"}` 与独立 `archi_relation delete` 产同一份文本；同批重复 `to` ⇒ Err；`delete` 项带 `desc`/`apis` ⇒ Err；四计数守恒式成立 |
| 15 | 乐观锁（D7） | 同值 `expect` ⇒ 成功；改文件后旧 `expect` ⇒ Err + 零落盘；响应 `hash` 与 `expect` 可互喂（连续两次写同一文件零额外读） |
| 16 | hash 单源 | ✅ **P3 已交付**：`file_hash` / `file_hash_of` 与 `assets_hash` 共用 `hash_files`（`drift/mod.rs` L617-629）；工具执行体零 `Sha256` 直接引用 |
| 17 | 读侧闭环（D7 / D12） | `archi_module` 单元素响应含 `modelHash`，与写工具 `expect` **同函数同前缀**（`sha256:`）；「读 → 带 expect 写」闭环成立 |
| 18 | 19 变体封闭 | ✅ **P3 已交付**：`EditErrorKind::ALL` 与 `criterion()` 穷尽 match 逐值相等；`criterion()` 逐值非空且互异（`messages_test.rs`） |
| 19 | 字段序 | ✅ **P3 已交付**：既有键原位不动 / 新键追加表尾 / 新建按规范序整表写出（`新建元素按规范字段序整表写出`）——**A 不必再选「保序 vs 统一序」** |
| 20 | 写引擎无 FS 泄漏 | ✅ **P3 已交付**：`apply_*` 层零文件系统触碰（模块头的层界纪律） |
| 21 | 落点约定缺席（D9） | 工具定义与响应里零「按 kind / 按首段 FQN 推落点」的规则表述；`file` 缺省时只走「最近已声明祖先」一条 |
| 22 | 不做的事不出现（D11 / D10-B） | spec 内零「本期清账」承诺；`ArchiApiFindingKind` 仍十三值（未因孤儿契约 md 扩张） |

---

# 十、决策记录（全部结案）

> **状态：D1–D12 全部结案，本方案无待裁决项。**（编号保持原序，便于对照前几轮的引用。）
> 结案表见下；四条本轮裁决的细节落在各自小节（D9 → §4.4 / D10 → §5.5 / D11 → §11.4 /
> D12 → §4.7 + §11.3 ④）。
>
> | 已结项 | 结论 | 落在 |
> |---|---|---|
> | `package` 的 `path` 基数 | 0..1 条，≥2 停摆（随漂移变更落地） | §2.4 |
> | `confirmKindChange` 开关 | 不做 | — |
> | E1-6 `path` 形态校验 | 删除，交给 drift 读数 | §5.3 |
> | `hints` 独立通道 | 并入 `warnings` | §5.1 |
> | `metadata` 子表知识 | 骨架删、模板注释留 | §6 |
> | **D1 变更切片** | **一次做完，不拆两步** | §8 切片 A |
> | **D3 L3a → L3b 收口** | **不写 L3b；L3a 为终态，持续观察** | §8 切片 C |
> | **D8 截断优先级** | **入参上限 + `warnings` 提到最高** | §5.4 / §5.7 |
> | **D2 词典形态** | **B1（草稿即词典，零新参数）** | §8 切片 B |
> | **D4 `config.json` 写面** | **不做**（手改是正式组成部分，不是缺口） | §6 |
> | **D5 内联删边** | **支持**（`relations[].action`，四项约束见 §4.3） | §4.1 / §4.3 / §4.5 |
> | **D6 元素插入位置** | **本方案不定；另立探索篇：`toml-sorting-and-layout`** | 见该篇 |
> | **D7 乐观锁** | **② `expect` 哈希前置校验** | §4.7 |
> | **D9 新文件命名** | **按推荐：`file` 由 agent 给，工具只管渲染 + 表头；不引入落点约定** | §4.4 |
> | **D10 孤儿契约 md** | **不做 B 方案，且不列入 backlog**（A 方案已在 §5.2 第 7 条） | §5.5 |
> | **D11 迁移期既有债** | **维持现状**（债由各 change 逐块偿还，不清账） | §11.4 |
> | **D12 `modelHash`** | **(a) 做**：`ArchiModuleDetail` 增 `modelHash` | §4.7 / §11.3 ④ |

---

# 十一、前置片落地状态（P1–P3 已完成）

> 拆分判据两条：① 有**独立验收面**（不依赖写工具就能端到端验证）；② **不触碰 §7.1
> 五处立场**（否则 spec 会描述尚不存在的代码）。满足两条即可前置。

## 11.0 落地状态

| 片 | 状态 | 归档变更 | 提交 |
|---|---|---|---|
| **P1 契约起草引导** | **已落地** | `2026-11-24-archi-contract-authoring-guidance` | `21696e6` + `4b12c2b` |
| **P2 字段词典** | **已落地** | `2026-11-24-archi-scaffold-field-dictionary` | `1940916` + `2d8ac4b` |
| **P3 写引擎原语** | **已落地** | `2026-11-24-archi-edit-engine-primitives` | `9fdd96c` + `452a564` |
| **P4 契约债清账** | **不做**（D11 裁决：维持现状） | — | — |
| **切片 A** | **未开工** | — | — |

## 11.1 三片的实际落地：与原设计的七处偏差

纸上设计与真实代码各暴露了几处。**前五处是补强，末两处改变了 A 的实现前提**。

### P1（契约起草引导）

| 偏差 | 为什么必须这样 |
|---|---|
| **占位句必须含 CJK 且非空** | `contract.rs::finish_section` L1089-1095 **丢弃 body 为空的段**——六段若只有标题，`sections.len() != 6`，P1 自己新增的自证 Scenario 直接红 |
| 作者注块内多写了两条 | 实现加了「`## 操作` 段占位句」与「契约面未启用时 `enabled = false` 且 `findings` 恒空」——后者把冷启动口径写进了草稿本身，不只是工具描述 |
| 多字节安全成了显式锚 | 草稿含 `→` / `≥` / `——`，断言「逐行字节数之和 + 换行数 == 全文字节」（多字节不被截半） |

### P2（字段词典）

| 偏差 | 为什么必须这样 |
|---|---|
| `ELEMENT_FIELD_NAMES` 落在 `parse/toml.rs` 并经 `parse` 导出 | 与 `dto.rs` 的 `ALL` 同范式：**清单贴在权威定义旁**。原设计只写「手写一个常量」，没说放哪 |
| 测试用 `line_starting_with` 而非 `contains` | **`apis` 会假命中 `api`**——这条陷阱原设计没提 |
| 新增「解注释自证」 | 把草稿的两条 `# path = ` 样例解注释 ⇒ 真实 `TomlParser` 恰出一条 `invalidValue`。证明**模板示例就是真实判据的示例**，不是装饰 |

### P3（写引擎原语）

| 偏差 | 对 A 的影响 |
|---|---|
| **① 字段序问题用第三种答案解决了**：既有键原位不动（沿用原 decor）、**新键追加到表尾**、新建元素按规范序整表写出（「调用序即规范序」，零第二份常量） | **A 里不必再选「保序 vs 统一序」**——两种 diff 形态被同时拿到：改既有元素零重排 diff，新建元素规范序 |
| **② `plan_element_delete` 没有 `cascade`**：有子元素 / 被引用一律 `Err` 并列出清单；`ElementSpec` 也没有 `relations` 字段 | **A 的删除级联与内联出边必须现做**，不能"顺手复用"。P3 只交付了"拒绝"的一半 |
| ③ `file_hash` / `file_hash_of` 落在 `drift` 并与 `assets_hash` 共用 `hash_files`；`ModelWrite.hash` 由 `file_hash_of` 在**未落盘**状态下算出 | 正是原设计的 TOCTOU 解法——写后值 == 落盘后可读值，逐字相等 |
| ④ `EditErrorKind` 19 个变体 + `ALL` + `criterion()` 穷尽 match | 比原设计的 24 条文案更收敛：**判据句（criterion）与实例句（message）分离** |
| ⑤ **`archi_module.modelHash` 没做** | **读侧闭环仍未通**：`file_hash` 已 `pub` 导出但无 DTO 消费者。**D12 裁决 (a)：A 里补**（§4.7） |

## 11.2 骨架字节：三片均未动知识（2597 B，A 的收益基准不变）

三片都零 preamble 字节：P1 / P2 的改动全在工具描述与草稿模板，P3 全在内核与命令层。
骨架仍 2597 B（+1 来自漂移修复那批的 LF 归一收尾 `c48b8f0`，与三片无关）。

即 **§2.1 的「可迁出 ≈ 1850–1900 B」这个前提没有变**——A 的收益仍按 2596 基准算。

## 11.3 P3 已交付什么、A 还要补什么

### 已交付（`core/archi/src/edit/`，1028 行 + 3315 行测试）

```
edit/mod.rs        apply_element_spec / apply_element_delete
                   apply_relation_upsert / apply_relation_delete        ← 纯函数层（零 FS）
                   plan_element_upsert / plan_element_delete
                   plan_relation_upsert / plan_relation_delete          ← 计划层（读盘定位）
                   ModelWrite { file, content, hash }                   ← 待落盘产物
edit/messages.rs   EditErrorKind × 19 + criterion() + message(ctx)      ← 判据句与实例句分离
```

三层沿同一固定形状（模块头明确写出）：**改 AST → 渲染 → 表头计数回填 → 重解析自证零诊断
→ 回读逐字段一致**，且被自证的文本**逐字等于**交出的 `content`。

三个设计味道值得记住：

| 设计 | 为何好 |
|---|---|
| 纯函数层与计划层分离（`apply_*` 零 FS / `plan_*` 只读盘） | 渲染逻辑可用源字符串直接单测，3315 行测试里绝大多数不需要临时目录 |
| **调用序即规范字段序**（`write_element_fields` 的 doc） | 零第二份字段序常量——序在代码里，不在字符串里 |
| `file_hash_of(path, content)` 的「未落盘同构形态」 | 写后 hash 与落盘后 re-read 的 hash **逐字相等**，TOCTOU 从设计上消失 |

### A 还要补的四处

| # | 缺口 | 为什么 P3 没做 | A 的做法 |
|---|---|---|---|
| ① | **删除级联**：`plan_element_delete` 只有拒绝臂，无 `cascade` | P3 的评审面要干净（只渲染 + 自证） | 加 `cascade` 参数：级联删引用关系 / 级联删子树 |
| ② | **内联出边**：`ElementSpec` 无 `relations` 字段 | 同上（批量语义涉及数组序、重复 `to`、delete 项，错误定位二维） | 加 `relations: Vec<RelationSpec>` + §4.3 的三条约束 |
| ③ | **落点策略只有最小版**：给 `file` 就落那里（且限模型目录一级 `*.toml`）、不给就落元素现有文件、新建须给 `file` | 完整策略（最近已声明祖先）是领域决策，会污染 P3 的评审面 | 加「省略 `file` ⇒ 最近已声明祖先所在文件」（D9 裁决：不引入 kind/FQN 落点约定） |
| ④ | **`ArchiModuleDetail.modelHash`** | 属线面变化（DTO + bindings 重导），性质不同 | **D12 裁决 (a)：加**（详见 §4.7） |

### ④ 的裁决细节

`file_hash` / `file_hash_of` 已在 `drift` 与 `lib.rs` 导出，但**没有任何 DTO 字段承载它**：

```
能拿到 hash 的路径：
  ✓ 写工具响应 ModelWrite.hash（P3 已备好，但工具还不存在）
  ✗ archi_module —— ArchiModuleDetail 无 modelHash
  ✗ archi_drift —— modelHash 是全资产聚合（assets_hash），不是单文件
```

**裁决 (a)**：A SHALL 给 `ArchiModuleDetail` 增 `modelHash: String`（`sha256:<hex>`，
与 `file_hash` 同源同前缀）。三处随动：`dto.rs` 投影 / `commands/archi/mod.rs` 出口 /
`dto_test.rs` 字段面；`bindings.ts` 重导。这样「读 → 带 expect 写」闭环成立。

## 11.4 P4 契约债清账：不做（D11 裁决）

| 内容 | 裁决 |
|---|---|
| 对 39 条 `docMissing` 元素逐个 `bootstrap` + 人补正文 | **维持现状**：债由各 change 逐块偿还，不在本方案里一次性清账 |

理由（D11）：清账会引入 30+ 个契约 md，属独立工作量且与本方案的验收面无关。P1 落地后
单次清账的**成本**确实下降了（草稿自带六段骨架与作者注，人只需填正文），但那是"将来
更方便"，不是"现在该做"。

## 11.5 禁止前置的三类（留档：为什么其余面不能拆）

| 面 | 理由 |
|---|---|
| 五处立场改写（§7.1） | spec 必须描述**已存在**的代码；工具未落地时改 = 描述虚构能力 |
| 知识骨架收缩（§6 迁移表） | 只删不加 = 知识缺失（分层原子性条款）；且 A 的验收面（知识内容断言）会写成空话 |
| 两写工具定义 + 注入矩阵 + `ToolTier::Write` | 定义与执行体一体；只声明不执行 = 死代码 |

## 11.6 可选：A 内按能力再二分的取舍（不改 D1 结论）

D1 已定「一次做完」，指**工具与散文收缩不可分**。在此前提下仍有一种二分：

```
A1  upsert（元素 + 关系，含内联 relations） + 骨架收缩 + spec 五处 + 矩阵
A2  delete / cascade 几个 action
```

A1 已满足原子性（散文收缩与工具同批），A2 只是**新增能力**，无需再动散文。好处是
删除的级联语义（引用 / 子树 / 内联删边）——本方案里最危险的部分——可以等 A1 的
真实使用反馈再定。

> **P3 落地让这个二分更自然了**：删除的「拒绝臂」已经在了（`plan_element_delete`
> 列子元素 / 列引用），A1 直接把它接上工具即可——`cascade` 只是给同一函数加一个
> 分支。所以 A2 的增量比原估还小。

**默认不做这个二分**（D1 已裁决）；若 A 的体量在评审时成为瓶颈，这是唯一不破坏
原子性的切法。

## 11.7 前置后的体量变化

| 切片 | 全部前置前 | 三片落地后 |
|---|---|---|
| A 的文件数 | 15–20 | ≈ 10–12（原文估 8–10，实际要加：`edit/mod.rs` 的级联与内联出边、`dto.rs` + `bindings.ts`（D12）） |
| A 的改动性质 | 引擎 + 声明 + 散文 | **以声明与散文为主**，引擎只剩「级联 / 内联出边 / 落点」三块增量（都长在已有函数上） |
| 风险集中点 | 渲染逻辑混杂在 20 个文件里 | 渲染与自证已在 P3 单测覆盖 3315 行；A 的风险退化为「参数面设计是否正确」 |

## 11.8 建议顺序（终态）

```
P1 ✔ ──┐
P2 ✔ ──┼──→ A（写工具 + 矩阵 + 骨架收缩 + spec 五处 + modelHash）
P3 ✔ ──┘
P4 ✗（D11：不做）
```

**只剩切片 A。** 它现在可以在 `archi::edit` 的稳定地基上直接开工：`plan_*` 已就位、
错误面已封闭 19 变体、渲染与自证已有 3315 行测试。A 的工作重心从"实现"转到
**「参数面设计对不对」**——8 条入参上限、`relations` 批量语义、级联、7 条 warnings、
响应字段序。这些恰恰是最需要人读、最不需要调试的部分。
