# 架构 Agent 三资产改造（探索笔记）
workflow_type: requirement

> 本笔记已整理：第 0 章是结论速览；第 1–3 章是事实与调研（原始复盘 → 变更后重估 → 三资产方案与代码落差）；第 4–6 章是**已裁决**的设计与变更集；第 7 章是已裁决的四条实现纪律；第 9–10 章是实现可行性调研与启动前就绪评估；后续 change（判据改版）已迁至 `archi-api-findings-redesign.md`。被后续裁决推翻的旧稿（legacyDoc 双读、13→13 词汇表、`api` 三值等）只保留在「为什么不这么做」的论证里。

## 0 · 结论速览

### 三资产分工（用户方案）

| 资产 | 载体 | 结构 | 回答的问题 | 约束手段 |
|---|---|---|---|---|
| ① 模块文档 | `models/<分层目录…>/<FQN>.md` | 无结构（段落自定） | 为什么这样切、有什么坑、什么不归它管 | prompt 引导 |
| ② 接口定义 | 同目录 `<FQN>.schema.json` | 强结构（JSON Schema） | 跨过边的接触面长什么样 | 工具约束 + 检查 |
| ③ 架构定义 | `graph/model/*.toml` | 强结构（TOML，不动） | 谁在谁的里面、谁依赖谁 | 工具约束 + 检查 |

> 过度设计 = 回答自己不该回答的那个问题。文档不列接口、schema 不写理由、TOML 不造空元素。

### 本变更做什么（七组）

1. **读面换真相**：schema 成为接口唯一凭据，**零 md 回落**；坏 schema 停摆。
2. **判据被动收缩**：13 值 → 10 值（删 3 个失去事实来源的），`severity` 基线随动。
3. **写面补闸**：`write` / `edit` 落 `<FQN>.schema.json` 时内存校验，失败 `Err` 零落盘。
4. **工具面**：`archi_module` 增 `assets`（三资产一次给全）+ `reading` 指引；`archi_api` 的 `bootstrap` 改双草稿、新增 `link`；`archi_scaffold` 只改描述。
5. **Agent 上下文**：骨架重写（三资产分工 / 设计顺序 / 三条不可越界 / 读面纪律）；两份相位 prompt 补设计顺序；change 相位补知识段。
6. **生成面让位**：`render_all` 不再产 schema，`api:check` 收面去 `*.schema.json`。
7. **规则面同步**：`AGENT.md`「模块 API 契约」——生成物四类 → 三类，散文禁令只对历史 fence 生效。

### 裁决记录

| # | 问题 | 裁决 |
|---|---|---|
| Q1 | schema 写闸失败形态 | **`Err` 零落盘**（与 `archi_element`「坏文本不可产出」同族） |
| Q2 | `bootstrap` 的 doc 草稿是否给段落建议 | **不给**（只 H1 + 一句「按内容自定，先例在同类元素」） |
| Q3 | 历史 md fence 的清理 | **本变更不处理**，用户后续专门清理一轮 |
| Q4 | 读面是否回落到 md | **不回落**，schema 是接口唯一凭据 |
| Q5 | 理解模块的读面 | **一款工具一次给全三份资产**（`archi_module.assets`），禁止用别的工具拼片面信息 |
| Q6 | 资产搬迁 | **不做**（文件结构未变，无需搬迁；盘上 schema 已是完整接口定义 ⇒ 切换真相源零迁移） |
| Q7 | 遗留 md fence 是否需要读数 / 提示 | **不需要**：fence 视为普通文档内容（与散文无区别），`archi_api check` 零新增计数；用户后续统一清理 |
| Q8 | 手写 `$ref` 算错怎么办 | **按推荐**：`archi_api` 新增 `link` 动作出正确串 + 描述明写「跨元素 `$ref` 用 link 算，勿手拼」 |
| Q9 | `gating_findings` 锚点二选一 | **schema**：`declared` ∧ schema 在盘（文档在盘与否退出纳管口径） |
| Q10 | `apiUnexpected` 形态②（`declared` ∧ 写了 `doc`） | **不做检查**（同 Q7）：形态② 删除，`apiUnexpected` 只剩「`none` ∧ schema 在盘」单形态；`doc` 字段零读者 |
| Q11 | op 的散文说明放哪 | **`x-function.description`（内层）**：不污染 `properties` 形态白名单、`.i.*` 逐字节不变；内层自由形状不做未知键校验（见 §9.4） |

## 1 · 起点：一次架构 Agent 运行复盘

原始会话 `ses-18-1795519020487`：由架构 Agent 产出 `Desktop.Views.Architecture.md` 后的运行复盘。**最有误导性的不是系统提示，而是 `archi_api bootstrap` 返回的草稿**——它看起来像规格，实际只是一份投影。

### 1.1 四处误导 + 一处矛盾 + 一处缺失

| # | 初始上下文的措辞 | 实际规则（源码实测） | 误读 | 影响 |
|---|---|---|---|---|
| 1 | bootstrap 草稿硬编码六段（`api/mod.rs:782`）+ 作者注「散文**只**留语义：职能 / 边界 / 不变量 / 承载理由」 | 解析器对 `##` 标题**零校验**，任意段数 / 段名 | 当成允许的段清单，往 4 段里塞 | **主因（≈60%）** |
| 2 | AGENT.md + 系统提示 + `contract.rs:9,31` + bootstrap 注四处重复「散文 MUST NOT 用表格复述契约名字」 | ① `contract.rs:514` 的闸在 ops/types/reexports **三项全空时整体跳过**；② 闸启用时也只抓首格 `` `identifier` `` 且须与契约名撞名 | 读成「散文别用表格」的宽泛禁令 | 第一稿零表格，信息只能直线排 |
| 3 | 系统提示把该文件叫「**入口**文档」，只对 `api = none` 可写 | 对 barrel 元素名副其实；对普通元素它就是一整页文档 | 以为要围绕「入口那一个导出」写 | 职能段以 `ArchitectureView` 起手，整体写薄（≈25%） |
| 4 | 系统提示「字段序与形态用 `archi_scaffold` 取模板」 | 那句只管元素 TOML 字段，与文档无关 | 过度迁移成「文档也抄模板」 | 加剧 #1 |

**自相矛盾的一处**：系统提示写面纪律「禁止改架构目录之外的文件」，但 `INDEX.toml` 为有 op 的元素声明了 `generated = "packages/desktop/src/views/architecture/index.i.ts"`（架构目录之外）。本元素零 op 所以没引爆；任何有 op 的元素都会撞上——**原意是「agent 不要手改」，措辞却是「禁止改文件」**。

**缺失的关键信息（≈25%）**：读者身份。上下文把契约 md 定位为「机械闭环的真相资产」，从未说它**同时是产品 UI 内容**——`architecture-node.tsx:95-111` 的「文档」tab 直接把路径交给 `FileViewerBlock` 渲染。缺这一句，可读性对模型只是软要求。

**责任比例**：草稿骨架 + 表格禁令重复放大 ≈ 60%，读者身份缺失 ≈ 25%，自己没验证 ≈ 15%。

### 1.2 那次变更（`archi-agent-contract-knowledge-gap`，已归档）做了什么、没做什么

| 做了 | 没做 |
|---|---|
| 骨架 2 597 → 1 459 B（语法知识迁出） | 段结构冲突（复盘 #1）原样在案 |
| 新增 `archi_element` / `archi_relation` 两写工具 + 七条 warning | 表格禁令仍是宽泛版（复盘 #2） |
| `fileHash` 读侧闭环、乐观锁 | 读者身份一字未补（复盘 #3） |
| 五处立场改写（「无专用写工具」等） | 上下文结构性问题未动（见第 2 章） |

## 2 · 变更后的上下文面重估

### 2.1 体量全景（磁盘 / 源码字节）

```
冻结系统提示（首轮拍一次，此后逐字复用）        每轮随请求全量重发
├─ archi-knowledge.md            1 459 B        tools 定义面
├─ AGENT.md 全文（<agent_md>）   6 532 B        ├─ 7 builtin（generic_definition） ≈ 3.7 KB
└─ GUIDANCE（冻结指引）            273 B        └─ 7 archi                        17 496 B
   合计 ≈ 8.3 KB，架构相关仅 1.5 KB（18%）          ├ archi_element   7 828 B ← 45%
                                                  ├ archi_relation  2 651 B
会话历史                                           ├ archi_api       1 821 B
├─ 用户消息（架构页无 spotlight）                  ├ archi_module    1 764 B
├─ 工具返回（archi_* / read 文档）                 ├ archi_drift     1 398 B
└─ 收口 SystemNotice（preclose，不回灌模型）        ├ archi_search    1 100 B
                                                  └ archi_scaffold    942 B
```

**知识骨架只占每轮上下文的 1/18；大头是工具定义（`archi_element` 一件顶 5 个骨架）。**

### 2.2 发现 A–H

| # | 发现 | 关键实证 | 本变更是否处理 |
|---|---|---|---|
| A | **段结构有四个持有者且互相冲突** | `contract::parse` 零校验 / 语料（职能 11·类型 11·不变量 11·契约面留白 10·公共面 8·承载理由 3·操作 2·边界 **0**）/ `bootstrap_text` 六段 / `BOOTSTRAP_NOTE` 四段 | ✅ **从根上消失**（md 无结构、草稿只给 H1） |
| B | **知识迁进工具面 = 从「1.5 KB 一次」变「7.8 KB 每轮」** | 骨架 −1 138 B，写工具 schema +10 479 B，净 **+9.4 KB** | ⏳ 未处理（schema 减负另议） |
| C | **读者身份缺三个读者里的两个** | ①架构页文档 tab 人类读者 ②`contract::parse` ③其他会话的 agent；提示只讲了 ② | ✅ 部分（骨架明写「文档是产品面」） |
| D | **preamble 单通道混拼，archi 会话 82% 是无关规则** | `preamble::freeze(knowledge, agent_md)`；AGENT.md 6.5 KB 中架构相关约 1.5 KB | ⏳ 未处理 |
| E | **两套架构上下文** | change 相位（proposal / dev-design）有写工具但 `preamble: None`；「两写工具 + 手改出口」有 **5 个持有者** | ✅ 本变更给 change 臂补知识段 |
| F | **冻结快照 + 续接靠自觉** | 骨架自己写「续接先取当下模型」；`drift summary` 投影已现成 | ⏳ 未处理 |
| G | **架构页会话无 UI spotlight** | `use-archi-chat.ts` 固定 `source=archi / sourceRef=null`，首条消息即用户原文 | ⏳ 未处理 |
| H | 两个小刺 | ①「只改架构目录」vs 生成物越界 ②`preclose` 的 T1 只认 `write`/`edit`/`bash`，**不含两写工具**（靠 T2 兜底） | ⏳ 未处理（低成本） |

> A 的实证价值：草稿六段里「边界」在语料中 **0 次**、「操作」2 次；而语料最主流的「契约面留白」（10/11）与「公共面」（8/11）草稿根本没提。反例：`Desktop.Views.Architecture.md`（14 KB、10 段 + 两表格）跟六段模板几乎无交集——它好，恰恰因为作者没照模板写。

### 2.3 上下文侧仍需独立动作的三条（本变更外）

1. **schema 减负**：`archi_element` 7.8 KB 里混着「调用前必须知道」（三态 / 坏文本不可产出 / `expect↔hash` / `cascade` 同文件）与「选择后才需要」（`kind` 语义 / `path` 基数 / 上限）。后者已在 `archi_scaffold` 词典（942 B）有一份，又被逐字写进参数 `description` → 每轮付两遍。
2. **AGENT.md 注入裁剪**：archi 会话只该看到「模块 API 契约」节与版本纪律。
3. **续接现状对齐**：续接首轮注入 `drift summary`（< 300 B）替代让模型自己 `archi_module` 全量薄概览（6–10 KB）。

## 3 · 三资产方案与代码现实的落差

### 3.1 不是新增资产，是**真相搬移 + 生成方向倒置**

`.schema.json` 今天已在盘（11 份），只是**生成物**——文件头写着 `GENERATED_NOTICE`（`api/mod.rs:1116`）：「本文件由元素的契约 markdown 生成，勿手改……」。手写真相是 md 里的三类 fence。

```
今天                                     改后
md(散文 + api-op/types/reexport fence)     md(纯散文) ─────────► 人类 + AI 共读
   │ parse → build_schema                     │
   └────► <FQN>.schema.json ★生成物           <FQN>.schema.json ★手写真相
   └────► 判据面                              ├─► 判据面（唯一凭据）
                                              └─► .i.rs / .i.ts / INDEX.toml ★生成物
```

**关键实测**：盘上 11 份 schema 已是完整接口定义（`build_schema` 产物，`properties` 全为 `x-function` 或 `$ref`）：2 份非空（Foundation 9 op / ArchiRuntime 2 op + 1 def），9 份空壳。**2 份非空的内容与 md fence 逐项等价，9 份空壳等价于空 fence ⇒ 不需要任何搬迁就能切换真相源。**

### 3.2 `api` 字段曾兼任两个资产的存在位（本次解除）

`paths::carrier_path` 的注释原文：「契约 md 与入口文档不是两份资产，只是同一落点的两个用途」。于是 `api = declared` 时 md 既是文档又是接口，`doc` 字段被禁写（`apiUnexpected` 守着）。
拆开后，两件事正交：

| | 接口在（schema 在盘） | 接口无 |
|---|---|---|
| **文档在**（md 在盘） | 11 个元素（今天唯一合法组合） | 允许（仅有理由、无接口） |
| **文档无** | 允许（有接口、缺理由 ⇒ `docMissing` warn） | 允许（两份都没有） |

⇒ **`apiUnexpected` 的「declared 却写了 `doc`」形态、`docNotFound`、`apiDocMissing` 全部失去意义**（见 4.2）。

### 3.3 三资产协同：今天有一条、缺一条、会退一条

```
资产3 TOML ──(.apis)──✗ 资产2 schema 的 properties/$defs 键      ← 今天零判据（缺）
     │                        │
     └──► 代码 ◄──────────────┘  relation 三桶 + apiMissing / signatureMismatch /
                                  constructorDeclared / shadowing / refMissing
资产1 md ──(apiReexportDrift: 散文 vs fence 键集)── 资产2          ← 唯一一条，本次必退
```

- **缺的那条最值得补**：`relation.apis` 已能说「这条边用了目标元素的哪几个名字」，但判据只跟**代码 import** 对差，从不跟**目标接口清单**对拍 ⇒「边上写了个 schema 里不存在的名字」只在代码写完后间接暴露。
  ```
  relation.apis ⊆ 目标元素 schema 的 properties 键 ∪ $defs 键
  ```
  今天恒静默（**实测 0 条关系写 `apis`**）⇒ 加它零噪声。→ 归后续 change（见 `archi-api-findings-redesign.md`）。

- **会退的那条**：md 无结构 ⇒ `apiReexportDrift` 无对象，退役。副作用：**复盘 #2 的「表格禁令」整条规则可删**。替代纪律进 prompt：「文档 MUST NOT 枚举导出名，接口清单的唯一持有者是 schema」。

### 3.4 手写 schema 新长出的面（逐条已有对策）

| 今天由 `build_schema` 保证 | 手写后 | 对策 |
|---|---|---|
| `$id` = `<FQN>.schema.json`、`title` = H1 | 与文件名、FQN 三处持有 | **identity 闸**（`$id` / `title` / 文件主干三者一致） |
| `$comment` = 生成警告 | 语义反转 | 「写者纪律」注解；清理由用户后续轮做 |
| `properties` 键空间唯一（op ∪ type 不撞名） | 手写可能撞 | **keyspace 闸**（沿用既有校验，只换输入） |
| `$defs` 键空间唯一（types ∪ reexports） | 同上 | `verify_defs` 保留 |
| `x-function` 合法落点（封闭二处） | 同上 | 落点闸保留（改判 JSON） |
| op id 由 `### heading` 提供 | 变成 `properties` 键 | **反而更清楚**（少一层 heading↔fence 耦合） |
| **跨元素 `$ref` 相对路径**（`paths::ref_relative`） | **手算必错**：`"../../Backend/Core/X.schema.json#/$defs/Y"`，少一个 `../` 会误报 `apiRefMissing` | 新增 `archi_api` 的 `link` 动作出正确串 |

**注解载体三选**（JSON 无注释）：`description`（调用方要知道的语义，进 `.i.*` 文档注释）/ `$comment`（写者纪律，只在源文件）/ `x-*`（只在本仓既有 `x-common` / `x-class` / `x-function` 内，**MUST NOT 自造新 `x-` 键**）。JSONC 行注释不采纳（解析器多一份真相）。

### 3.5 过度设计的三种形态与机械信号

| 形态 | 资产 | 今天的信号 | 本变更后 |
|---|---|---|---|
| 文档里复述接口清单 | ① | `apiReexportDrift` + 散文表格闸 | **无** → prompt 纪律顶上 |
| schema 里写理由 / 实现路径 | ② | 无 | 无 → `description` 规范 + 人审 |
| TOML 造「为将来准备」的空元素 | ③ | `noPath`（warn）+ 归属判据 + `unfulfilledDeclaration`（观察项） | 不变 |

不对称提醒：**资产 ③ 有专属写工具（自证链 + 乐观锁 + 七条 warning），资产 ② 今天只有 `bootstrap` 出草稿、零写盘**——这是本变更要补写闸的直接理由。

## 4 · 最终设计（已裁决）

### 4.1 读面：schema 唯一凭据

```
load_contracts（新）
  for 元素 where path 非空 ∧ api = declared:
      <FQN>.schema.json 在盘 ?
        ├─ 否 → 跳过（接口未写；读数上 present = false，无判据）
        └─ 是 → 解析 JSON → Contract::from_schema → 两闸（identity / keyspace+defs）
                    └─ 任一闸失败 ⇒ ApiError（**停摆**，与模型结构错同族）
```

**`Contract` 类型保留，改由 schema 装配**。下游对 `Contract` 的字段消费面实测只有六个（其中 `title` 仅 identity 闸在读）：

| 字段 | schema 来源 | 备注 |
|---|---|---|
| `fqn` | 文件主干（`X.schema.json` → `X`） | 与 `$id` 一致即 identity 闸 |
| `title` | `title` | 校验 = `fqn` |
| `file` | schema 路径 | 错误文案从此指向 `.schema.json`（更准） |
| `ops` | `properties` 里带 `x-function` 的项 | `line = 0`（既有「位置未知」口径，schema 无行号） |
| `types` | `$defs` − 纯 `$ref` alias | 既有 `is_pure_ref_alias` 现成 |
| `reexports` | 纯 `$ref` alias | 目标 FQN = `$ref` 中 `.schema.json` 的主干——**文件名即身份**，天然反查 |
| `reexport_prose` / `sections` | — | 恒空，字段随后删 |

**一条实现硬要求**：`Contract::from_schema` 必须把 `properties.<op>.x-function` **解包**存进 `ContractOp.signature`——`emit/rust.rs:283-291` 直接 `op.signature.get("returns")` / `.get("owner")`，保留包裹态会让 `.i.rs` 的签名见证静默退化（返回类型丢成 `()`、`owner` 少一层 `super::`），要到编译才炸（详见 §9.2 D1）。

**退役与保留清单**：

| 对象 | 处置 |
|---|---|
| `contract::parse`（`contract.rs` 主体约 600 行） | **退役**（产品路径） |
| `schema::build_schema` | **退役** |
| `contract::verify` 的 op id 重复闸 / types 在 `$defs` 缺失闸 | **死闸**（从 schema 装配后恒真，随 `parse` 一起删） |
| `contract::verify` 的两条 md 双面闸（散文表格 / reexport 键集） | **退役**（失去事实源） |
| `contract::verify` 的七族名字闸 / `x-function` 落点闸 / `required ⊆ properties` | **保留**，改判 JSON 值（`Contract` 改由 schema 装即可） |
| `contract::duplicate_keys` | **保留**，挪作写闸的文本层扫（`serde_json` 折叠重复键，手写会静默） |
| `schema::verify_defs` | **零改造保留**（入参本就是 `&Value`） |
| 新增 `verify_identity` | `$id` / `title` / 文件主干三者一致 |
| 新增 `verify_ref_prefix` | 跨元素 `$ref` 前缀逐字等于 `ref_relative(本FQN, 目标FQN)`（**今天零判据**，见 §9.2 A3） |
| 新增 `verify_shape` | `properties.<k>` 只允许 `x-function` 或 `#/$defs/<名>`（堵内联 object 静默退化为 `Record<string, unknown>`，见 §9.2 A1） |
| 新增 `verify_mirror` | `$defs` 每个键在 `properties` 有指向自己的 `$ref`（见 §9.2 A2） |
| `enabled` / `facts_enabled` | 「任一 schema 在盘」（本仓 11 个元素 ⇒ 与今天同值） |
| `doc_facts` / `DocFact::has_api_fence` | 收成 `doc_present: bool` |
| `paths::carrier_path`（`declared`→契约 md / `none`→`doc`） | **退役**：资产① 的落点恒为 `models/<分层…>/<FQN>.md`，与 `api` 无关 |
| `paths::doc_path`（任意 repo 相对路径归一 + 越根判据） | **退役**：唯一读者是 `carrier_path` 的 `none` 分支（连带推论，非独立裁决） |

### 4.2 判据面的被动收缩：13 → 10

零 md 读面让三个判据**失去事实来源**；留着就是永不产出的死值（违反 `ALL` 与实际产出的对拍）：

| 值 | 事实来源 | 处置 |
|---|---|---|
| `apiReexportDrift` | 散文透传列表 vs fence 键集 | **删** |
| `apiDocMissing` | 契约 md 在盘但正文无 api fence | **删** |
| `docNotFound` | `doc` 字段指不到盘 | **删**（`doc` 字段惰性化） |
| `docMissing` | **派生** md 不在盘（不再分 `declared` / `none`） | 保留，收成单成因 |
| `apiUnexpected` | `none` ∧ schema 在盘 | 保留，**收成单形态**（形态② `declared` ∧ 写了 `doc` 按 Q10 删除） |
| 其余 8 值 | schema / 代码 / 模型 | 保留 |

- `ALL: [Self; 10]`；`category` 7 → 6（`doc` 只剩 `docMissing`）。
- **这是词汇缩小而非扩张**——三项删除都因「事实源消失」，与「新增判据」方向相反，diff 可逐条解释。
- `severity` 面随删随动：今天的 `error` 恰三项是 `docNotFound` / `apiMissing` / `apiSignatureMismatch`；删 `docNotFound` 后**变两项**（`apiMissing` / `apiSignatureMismatch`）。spec 的「error 恰三项」基线、语料与金样 MUST 同批改写，不允许留一句与实现不符的断言（`schemaInvalid` 补位属后续 change，见 `archi-api-findings-redesign.md`）。

### 4.3 `api` / `doc` / 留白 的最终口径

| 项 | 口径 |
|---|---|
| `api` | 保留二值，语义收窄为**纯资产②声明**：`declared`（承诺有接口）/ `none`（显式无接口面，schema 在盘即 `apiUnexpected`）。**不新增第三值**（`deferred` 省 9 个 7 行文件，代价是九处枚举 + 9 处 TOML）。 |
| 留白（有导出但不声明） | **`declared` + 空 schema**（`$defs {}` + `properties {}`）就是正规形态；`apiUndeclared`（`off` 档）是它的守卫。今天 9 个前端元素**已是这个形态**，零迁移。空 schema 不是仪式——它是「看过，确实没有要声明的接口」的机器可读版本，与「还没写（schema 缺）」严格可分。 |
| `doc` 字段 | **保留但零读者**：无判据、无派生（载体恒为派生 md）、无读面依赖；不删字段（外部工作区写了 `doc` 不会突然变成 `unknownField` 结构错）。Q10 之后连「写了就报一条」也不再检查。 |
| `docMissing` | 判据 = `models/<分层…>/<FQN>.md` 不在盘，与 `api` 无关。 |
| `gating_findings` | **定案（Q9）：`declared` ∧ schema 在盘**。文档在盘与否**退出纳管口径**——文档债（`docMissing`）恒 `warn`、永不纳管。本仓该集合 = 11 个元素，与今日读数零差。 |

### 4.4 写面：schema 写闸

`tools::write` / `tools::edit`（`sdk/tools.rs:372-420`）今天只有一次 `fs::write`，**零领域校验**。

```
落盘前：路径命中 <archi_models_dir>/**/<FQN>.schema.json ?
  ├─ 否 → 原样落盘（零改动）
  └─ 是 → 内存校验（判「文件级自洽」，零跨元素、零读其他文件）
          ① 文本层：重复键扫描（serde_json 会把重复键折叠成「后者胜」）
          ② JSON 可解析
          ③ identity：$id == <文件名> / title == <FQN> / 文件名主干 == FQN
          ④ 键空间：$defs ∪ properties 无重名；op id 与类型名不撞
          ⑤ 形态白名单：properties.<k> 只允许 {x-function: …} 或 {$ref: "#/$defs/<名>"}，
                        且后者 <名> ∈ $defs
          ⑥ 镜像：$defs 每个键在 properties 有指向自己的 $ref
          ⑦ $ref 前缀：跨元素串逐字等于 ref_relative(本FQN, 目标FQN)。**今天零判据**
          ⑧ 零未知顶层键（$schema / $id / title / $comment / $defs / properties 之外一律拒）
          ⑨ 复用装载期三闸：七族名字 ASCII / x-function 落点封闭二处 / required ⊆ properties
          任一失败 ⇒ Err + 零落盘（第一行事自足 + 点名 + 给期望值）
          全过 ⇒ 落盘，响应追加「接口已校验」一行
```

**写闸与装载校验同一实现**：`write` / `edit` 落盘前调 `contract::validate_schema(fqn, &text)`，`load_contracts` 读盘时调**同一个函数**——只差错误映射（`Err` 字符串 vs `ApiError`）。这是「零第二份校验」的直接落法（推导与逐面依据见 §9）。

**分工**：写闸判「文件级自洽」（上列九条全部只需本文件 + 本 FQN）；**跨元素的一切**（`$ref` 目标是否在模型内 / 是否直接子元素 / 是否 `api = none` / 目标文件在不在盘）仍归 `apiRefMissing` 既有判据面——写闸做它就变成第二份判据。唯一例外是 ⑦ 的**前缀**：它只需两个 FQN 做纯计算，故留在写闸。

**对称性**：资产③ 是「渲染 → 重解析 → 回读」，资产② 是「候选文本 → 校验 → 落盘」——同一句纪律：**坏文本不可产出**。

### 4.5 工具面

#### `archi_module` 增 `assets`：一次给全三份资产

```jsonc
{
  "element": { …现有薄字段… },
  "dependsOn": [...], "dependedBy": [...], "children": [...],
  "resolvedEntry": { … }, "fileHash": "sha256:…",
  "assets": {
    "topology":  { "file": "…/06-frontend.toml", "line": 105, "out": 0, "in": 3 },
    "interface": {                                   // 唯一凭据
      "path": "…/models/…/AgentError.schema.json", "present": true, "bytes": 276,
      "ops": [], "types": [], "reexports": [],       // 键名即定义
      "abstract": "接口面留白（schema 在盘且无 op / 无类型）"
    },
    "doc": {
      "path": "…/models/…/AgentError.md", "present": true, "bytes": 3680, "lines": 62,
      "headings": ["职能", "公共面", "不变量", "承载理由", "契约面留白", "类型"]
    }
  },
  "reading": "理解本模块已足够：拓扑看 file/line 与出入边；接口面见 interface 的键名（键名即定义，无需整读 JSON）；文档段名见 doc.headings。读正文用 read <doc.path>（文档无结构，MUST 整读，MUST NOT 用 grep / offset 拼凑）。"
}
```

| 设计决定 | 理由（都对着「避免片面」） |
|---|---|
| 三份资产**各自 `present`**，不搞全或全无 | 今天 `ref_projection`（`api/mod.rs:924-948`）要求 md 与 schema **同时在盘**才报路径，缺一份则三项皆 `null`——这是「片面信息」的投影层版本，必须改 |
| 接口给**键名摘要**，不给 content | 理解接口要的是「暴露什么」= `properties` / `$defs` 键名；整读 JSON 是浪费（9 份空壳、2 份共 11 个 op） |
| 文档给 **`##` 段名 + 字节 + 行数**，不内联正文 | 段名让 agent 决定要不要整读；内联会把 14 KB 的 `Views.Architecture` 灌进上下文并抢走 `read` 的翻页能力 |
| **`reading` 是工具输出的一部分** | 「别用 grep 拼凑」写在返回值里，而不是只写在 prompt（prompt 会被 6.5 KB 的 `AGENT.md` 淹没） |
| **`abstract` 一句话判读** | 「留白 / 只有 op / 只有类型 / 有透传」四形态一眼分辨，避免对着空数组猜 |

配套：`ArchiModuleDetail.api`（`status` + 三路径）→ 收成 `api: ApiStatus`（意图）+ `assets`（事实）。前端两处（`architecture-node.tsx` 文档 tab、`model-info.tsx` 入口文档段）改读 `assets`；`contractPath` 这个名字退役——**它一直是 `docPath`**。

#### `archi_api`：`bootstrap` 改双草稿 + 新增 `link`

| 动作 | 今天 | 改后 |
|---|---|---|
| `check` | 生成物新鲜度 + 十三判据 | 不变（**零新增字段**：遗留 fence 视为普通文档内容，Q7 裁决不读数） |
| `bootstrap` | 出**契约 md 草稿**（H1 + 六段 + 三类 fence + 作者注） | 出**两份**：`{ "schema": "<JSON 骨架>", "doc": "# <FQN>\n" }`。schema 骨架含正确 `$id` / `title` / `$schema`、由代码符号生成的 `properties` 占位（`x-function` 带 `description: ""` 空位 + params / returns）、`$defs: {}`；doc 骨架**只有 H1 + 一句「段落按内容自定，先例见同目录同类元素」** |
| `link`（新） | — | `{ from, to, name }` → 正确 `$ref` 串（`paths::ref_relative` + `#/$defs/<name>`）；`{ element, name }` → 本模块 `#/$defs/<name>` |

`bootstrap` 改双草稿顺手消灭复盘 #1：**没有段骨架可被当成规格**（Q2 裁决）。

#### `archi_scaffold`

TOML 仍是它一份词典，形态不动；只改描述里那句「模型写面是两个写工具」→「接口草稿用 `archi_api` 的 `bootstrap`」。

#### 工具描述必须写死的三句

1. 「**理解模块先调本工具**：三份资产的路径与结构面都在 `assets`；别的工具只给一面。」
2. 「接口的唯一凭据是 `.schema.json`；`.md` 的 api-* fence 读面不读（历史遗留，清理中）。」
3. 「文档正文用 `read` **整读**（无结构，跳读会误读）；接口不必整读——`assets.interface` 的键名即定义。」

### 4.6 Agent 上下文

#### 骨架重写草案（约 1.6 KB，门限附近重定）

```text
# 架构模型须知
## 职责
三份资产各答一个问题，过度设计 = 回答自己不该回答的那个：
- 模型 TOML：谁在谁的里面、谁依赖谁 —— 拓扑
- 接口 <FQN>.schema.json（与文档同目录）：这条边跨过去时接触面长什么样 —— 定义
- 文档 <FQN>.md（同目录）：为什么这样切、有什么坑、什么不归它管 —— 理由
文档是产品面：架构页「文档」tab 直接渲染给用户读，人 + AI 共读；接口由工具校验，人经 UI 阅读。
## 设计顺序
1. 定位现状：archi_search（发现）→ archi_module（邻居 / 三份资产齐备性 / 声明位置，一次给全）
2. 画拓扑：archi_element / archi_relation —— 先边界、后接触面、最后写理由
3. 补两份资产：archi_api bootstrap 取双草稿 → 接口写 .schema.json（write / edit，坏文本零落盘）
   → 理由写 .md（段落按内容自定，先例在同类元素）
## 三条不可越界
- 文档不列接口：导出名 / 签名 / 类型的唯一持有者是 schema；文档只写意图与坑
- schema 不写理由：`x-function.description` 只写调用方要知道的语义（做什么 / 边界 / 为什么），
  MUST NOT 写实现细节、MUST NOT 复述签名；`x-` 面封闭为 x-function / x-class / x-common 三项
- TOML 不造空元素：每个元素至少一条接触面（path / 边 / 导出）
## 工具面
读五件各一句何时用…（沿用）；写两件（archi_element / archi_relation）参数面即字段清单；
接口面：archi_api 的 bootstrap（双草稿）/ link（$ref 计算）/ check（两轴）；
手改出口：工具表达不了的才手改，改完 archi_module 自查（file:line）；
生成物过期用既有命令重生成，MUST NOT 手改。
## 留给判断
kind / path / api 的选择没有机械答案；纯分组用 composite。
接口面留白（无对外约定）就写一个空 schema（$defs {} + properties {}），不要编造导出。
## 兼容
旧元素可能仍把接口写在 .md 的 api-* fence 里：读面不读，接口一律写 .schema.json，
MUST NOT 往 .md 里新增 fence（历史 fence 由专门清理轮移除）。
```

五处增量：三资产分工 / 设计顺序 / 三条不可越界 / 留白的正确写法 / 兼容提示。删掉的：语法段（已在工具面）、原「工具面指路」的一部分（压缩）。

#### 其他四处

| # | 对象 | 改动 | 为什么能提升设计效果 |
|---|---|---|---|
| 1 | `prompts/proposal-planner.md` / `dev-design-planner.md` | archi 段加「先拓扑、后接触面、最后理由」+「接口面留白写空 schema，不编造导出」 | 相位才是真正做设计决策的地方；今天只讲怎么落笔 |
| 2 | `compose.rs` change 臂 | 矩阵命中 archi 工具时 `preamble` 复用 `archi_knowledge::knowledge()`（单来源） | 消灭发现 E「有工具没知识」 |
| 3 | `prompts/implementation-generator.md` | 一句「改了导出面就同步 `.schema.json`」 | 接口漂移正是在实现相位产生 |
| 4 | `AGENT.md`「模块 API 契约」节 | 「四类生成物」→ 三类；散文禁令只对历史 fence 生效（清理轮后删） | 规则面与工具面同步 |

### 4.7 生成面让位

| 对象 | 改动 |
|---|---|
| `api/mod.rs::render_all` | 不再产 `.schema.json`，只产 `.i.rs` / `.i.ts` / `INDEX.toml` |
| `api/diff.rs::api_dir_artifacts` | 收面去掉 `*.schema.json`（**不去掉 = 11 份手写文件全进 `orphaned`，症状是「全红」而不是「少一条」**） |
| `INDEX.toml` 头注释 | 「本文件与 `*.schema.json` 的 diff 视为机械噪声」→ 手写 / 生成分界改写 |

## 5 · 验收与度量

### 5.1 本仓期望读数（改前 → 改后）

| 指标 | 今天 | 改后 | 说明 |
|---|---|---|---|
| 元素 / 关系 | 55 / 129 | 不变 | 三资产不动模型 |
| `path` 非空元素 | 49（46 declared + 3 none） | 不变 | 判据循环面 |
| `doc` 字段使用 | 0 | 不变（字段惰性） | — |
| `relation.apis` 使用 | 0 | 不确定 | 跨资产判据留后续 change |
| schema 在盘 | 11（2 非空 + 9 空壳） | 不变 | 2 份内容与 md fence 等价 |
| md 在盘 | 11 | 不变（清理轮才动） | — |
| `apiFindings` | **38**（全 `docMissing`） | **38**（全 `docMissing`） | 可推：49 − 11 = 38；今天 = 35（declared 缺 md）+ 3（none 缺 doc） |
| error / 纳管 / `status` | 0 / 0 / clean | 不变 | — |
| 唯一变的文本 | — | **38 条 `docMissing` 的 message** | 判据改为「派生 md 不在盘」，不再出现「入口文档 = 派生契约 md」的指针措辞（条数与构成不变） |
| `api:check` 三列差异 | 零 | 零 | 收面去 schema 后仍零 |
| `.i.rs` / `.i.ts` / `INDEX.toml` | 生成 | 逐字节不变 | 输入换成 schema，同一 `emit` |

### 5.2 验收判据

1. `.dev-team` 重跑 `archi_drift`：除 `timestamp` / `durationMs` 与 38 条 `docMissing` 的 `message` 文案外**逐字段不变**（`apiFindings.total` 仍 38、`byCategory` 仍全 `doc`、`status = clean`）。
2. `api:check` 零差异；`bindings:export` 的 diff 只含 `assets` / `api` 字段随动。
3. 写闸：坏 JSON / `$id` 不符 / 键空间冲突三类各一条 `Err` 且盘面零变化。
4. `archi_module` 单元素响应含 `assets` 三段与 `reading`；三资产各自 `present` 独立（构造缺一份的夹具验）。
5. `bootstrap` 双草稿：schema 骨架可直接落盘并通过写闸；doc 骨架只有 H1 + 一句指引，**零段名**。
6. `link` 算出的 `$ref` 被 `apiRefMissing` 判为可达（构造跨元素夹具）。

### 5.3 「提高设计效果」怎么算数

| 层 | 信号 | 今天 | 目标 |
|---|---|---|---|
| 结构完整 | `docMissing` | 38 | 递减（迁移进度条） |
| 接口完整 | `present = false` 的元素数 | 0（都在，但 9 份空壳） | 有导出的元素都有**非空** schema |
| 接触面自洽 | `edgeApiUnresolved` | 不存在（`apis` 零使用） | 一旦写 `apis` 必须可落点（后续 change） |
| 不越界 | ①md 枚举导出名 ②schema 写理由 ③零出边元素 | 不可判 | ①② 靠 prompt + annotation 规范；③ 已有 `unfulfilledDeclaration` 观察项 |

过程信号（不进门禁）：会话里「`bootstrap` 早于 `write`」的比例；收口前 `archi_drift` 被调用的比例。

## 6 · 变更集与边界

### 6.1 本变更（15 项，按面分组）

| 面 | # | 对象 | 改动 |
|---|---|---|---|
| 读 | 1 | `api/mod.rs::load_contracts` | 只读 schema；`Contract::from_schema`；坏 schema `Err`（停摆） |
| 读 | 2 | `api/contract.rs` | md 解析器与两条 fence 闸退役；名字 / 键空间 / 落点三闸改判 JSON |
| 读 | 3 | `api/schema.rs` | `build_schema` 退役；`verify_defs` 保留；新增 `verify_identity` |
| 读 | 4 | `api/mod.rs::{enabled, facts_enabled, ref_projection, gating_findings}` + `paths::{carrier_path, doc_path}` | 启用 = 任一 schema 在盘；投影三资产各自独立；纳管 = `declared` ∧ schema 在盘（Q9）；`carrier_path` / `doc_path` 退役（资产① 恒派生） |
| 读 | 5 | `dto.rs` | `ArchiModuleDetail` 增 `assets` / `reading`；`api` 收成 `ApiStatus` |
| 生成 | 6 | `api/mod.rs::render_all` + `api/diff.rs` | 不产 schema；收面去 `*.schema.json` |
| 判据 | 7 | `drift/report.rs` + `api/check.rs` | 词汇 13 → 10（删 3 值）；`docMissing` 单成因（与 `api` 无关）；`apiUnexpected` 收成单形态（Q10）；`severity` 基线随动 |
| 写 | 8 | `sdk/tools.rs` `write` / `edit` | schema 写闸（Err 零落盘） |
| 工具 | 9 | `sdk/archi_tool.rs` | `archi_module` 组 `assets`；`bootstrap` 双草稿；新增 `link` 动作 |
| 工具 | 10 | `sdk/mount.rs` | 三处描述同步（module / api / scaffold） |
| 上下文 | 11 | `archi-knowledge.md` | 重写：三资产分工 + 设计顺序 + 三条不可越界（含 schema 注解纪律：`x-function.description` 写语义、`x-` 面封闭三项）+ 读面纪律 + 兼容一句 |
| 上下文 | 12 | `prompts/proposal-planner.md` / `dev-design-planner.md` | 设计顺序三句 + 留白写法 |
| 上下文 | 13 | `compose.rs` change 臂 | 矩阵命中 archi 工具时复用知识段 |
| 上下文 | 14 | `prompts/implementation-generator.md` | 一句「改导出面同步 schema」 |
| 规则 | 15 | `AGENT.md`「模块 API 契约」 | 生成物四类 → 三类；散文禁令只对历史 fence 生效 |

生成物随动：`bindings.ts` / `bindings.zod.ts` / 前端 2 组件（`architecture-node.tsx`、`model-info.tsx`）+ 4 处测试。

### 6.2 明确不在本变更

| 项 | 归属 |
|---|---|
| `schemaMissing` 判据（`declared` ∧ schema 不在盘） | 后续 change（本变更「不在盘 = 跳过」，零判据） |
| `schemaInvalid` 判据（装载失败落判据而非停摆） | 后续 change（本变更取「停摆」） |
| `edgeApiUnresolved`（`relation.apis ⊆ 目标接口键集`） | 后续 change |
| `api` 三值与 `doc` 字段退役 | 后续 change |
| `archi_element` schema 减负（7.8 KB → 数 KB） | 独立优化（发现 B） |
| AGENT.md 按来源裁剪注入 | 独立优化（发现 D） |
| 续接首轮 `drift summary` 注入 | 独立优化（发现 F） |
| 画布选中元素注入会话（UI spotlight） | 独立优化（发现 G） |
| `preclose` T1 补两写工具名 | 独立优化（发现 H，低成本） |
| 历史 md fence 清理 | **用户后续专门一轮**（Q3） |

## 7 · 残余风险（全部已裁决，只剩实现纪律）

四条待裁决已定（Q7–Q10），本节转成**实现时必须守住的四条纪律**：

| # | 决策落点 | 实现纪律 | 走偏的症状 |
|---|---|---|---|
| 1 | 遗留 md fence = 普通文档（Q7） | 读面**零解析** fence；`archi_api check` **零新增计数**（不设 `legacyFenceFiles`） | 某处又长出「fence 是否残留」的读数，等于把已裁决的非判据偷偷装回判据面 |
| 2 | `$ref` 用 `link` 算（Q8） | `link` 是**纯计算**（`paths::ref_relative` + `#/$defs/<名>`），零读盘、零写盘；工具描述明写「勿手拼」 | `apiRefMissing` 的文案若不点名「期望串」，手算错误会被读成「目标不存在」 |
| 3 | `gating_findings` = `declared` ∧ schema 在盘（Q9） | 单点判据；文档在盘与否**不得**进入纳管集合 | 两套判据并存会让同一份报告的两个数对不上（且本仓读数相同，测不出错） |
| 4 | `apiUnexpected` 单形态（Q10） | 只判「`none` ∧ schema 在盘」；`declared` ∧ 写了 `doc` **不报** | `doc` 字段若残留任何判据读者，「零读者」的裁决就被推翻 |

另一条不是风险而是**机械随动**，漏改即红：`severity` 的「error 恰三项」基线删 `docNotFound` 后变两项，spec / 语料 / 金样 MUST 同批改写——不允许留一句与实现不符的断言（`schemaInvalid` 补位属后续 change）。

## 8 · 后续 change（已迁出）

判据改版（两条资产轴、`schemaMissing` / `schemaInvalid` / `edgeApiUnresolved` 三个新增判据、装载失败改判据、`api` 三值时点）已迁移到独立笔记：

**`.dev-team/explores/archi-api-findings-redesign.md`**（`# 契约判据改版（三资产分离 · deferred）`）

迁出理由：它动 38 条基线与全部金样，与本变更（语义等价的重构 + 删死判据）混在一起 diff 不可归因。本篇的 §4.2（13 → 10）、§6.2（不在本变更）与 §7（残余风险）保留改版所需的全部前置裁决与读数引用。

## 9 · 实现可行性调研（按需探索）

本节回答上一轮留下的三条候选里最硬的一条：**写闸的校验边界到底划在哪**。做法是把「今天由谁保证 + 明天谁保证」逐面列清，再据此定闸。

### 9.1 先坐实前提：schema 是否真的忠实镜像 md

逐份对拍 11 份资产（`### <op>` heading vs `schema.properties` 键、`api-types` 键 vs `$defs` 键）：

| 元素 | md `###` heading | schema `properties` | schema `$defs` |
|---|---|---|---|
| `Backend.Core.Foundation` | 9 个 op | **完全一致**（同 9 个） | 0 |
| `Backend.Infra.ArchiRuntime` | `file_stem` | `file_stem` + `LocalArchiCheck` | `LocalArchiCheck` |
| 其余 9 份 | 0 | 0 | 0 |

签名内容也逐项一致：md 的 `api-op` fence 原文 `{owner, params, returns}` 在 schema 里就是 `properties.<op>.x-function` 的同形对象。

⇒ **「切换真相源零迁移」成立**，不是推测。

### 9.2 四类「今天谁保证 / 明天谁保证」（这是闸的边界依据）

**A · 今天由 `build_schema` 自动配对 ⇒ 手写后必须新增装载校验**

| # | 今天自动保证 | 手写后若缺校验的症状（按危险度） |
|---|---|---|
| A1 | `properties.<名>` 只可能是两种形态：`{x-function: …}` 或 `{$ref: "#/$defs/<名>"}`，且后者必在 `$defs` | **静默失真**：内联写 `{"type":"object","properties":…}` 会走 `value_type` 的 type 分派，TS 侧退化成 `Record<string, unknown>`（`emit/ts.rs:369-372`）——编译通过、类型全丢 |
| A2 | `$defs` 每个键在 `properties` 有指向自己的 `$ref`（镜像） | `emit` 的导出面（`properties`）漏条目，而 `declared_types`（读 `$defs`）仍计数 ⇒ 索引 `types` 与实际产出不符 |
| A3 | 跨元素 `$ref` 的前缀由 `ref_relative(from, to)` 算 | **今天完全无判据**：`reexport_target`（`api/mod.rs:1065-1071`）只取 `#` 前文件名的**末段**去后缀，前缀从不校验 ⇒ 少一个 `../` 时 `apiRefMissing` 判通过、`.i.ts` 里 import 一个不存在的路径，到 `tsc` / `cargo` 才炸 |
| A4 | `$id` / `title` 由 FQN 产 | 三处（文件名 / `$id` / `title`）漂移后，反查与报告文案各自指向不同元素 |
| A5 | op id 与类型名不撞（`build_schema` 显式判） | `properties` 的键空间冲突 —— JSON 后者覆盖前者，静默丢一个导出 |

**B · 今天由 `contract::parse` 的文本层判 ⇒ 随 md 一起退役，其中一条要补回**

| # | 今天保证 | 退役后 |
|---|---|---|
| B1 | fence 内**重复键**在解析前抓（`duplicate_keys`，`contract.rs:840`） | **要补**：`serde_json` 把重复键折叠为「后者胜」，手写「同名两处定义」会静默。补法最省 = 写闸在解析前跑一次同款文本扫描（现成算法，260 行）|
| B2 | 三类 fence 各同文件唯一 | 概念消失，无需补 |

**C · 今天由 `contract::verify` 的七项检查保证 ⇒ 两项恒真、两项退役、三项可复用**

| 闸 | 今天判什么 | 从 schema 装配后 |
|---|---|---|
| op id 重复 | `contract.ops` 里同名 | **恒真**（JSON 对象键唯一）⇒ 死闸 |
| types 键在 `$defs` 缺失 | `contract.types` vs `$defs` | **恒真**（`types` 就是从 `$defs` 派生）⇒ 死闸 |
| reexport 键集 == 散文列表 | md 双面 | **退役**（无散文面） |
| 散文表格闸 | md 双面 | **退役**（`apiReexportDrift` 一并删） |
| 七族名字 ASCII 标识符 | 名字面 | **可复用**：入参是 `Contract` + `Value`，只需 `Contract` 改由 schema 装 |
| `x-function` 落点闸 | `contract.ops` + `schema` | **可复用**（同签名） |
| `required` ⊆ `properties` | schema 内（与七族闸同趟续判，不占族别编号） | **可复用**（已是 schema 内判据） |

同文件里 **`verify_defs`（`schema.rs:215`）入参本来就是 `&Value`** ⇒ 全仓唯一「零改造可复用」的一块。

**D · 今天由 `emit` 的形态契约隐含保证 ⇒ 手写后要显式满足**

| # | 隐含契约 | 位置 | 手写后要做什么 |
|---|---|---|---|
| D1 | `ContractOp.signature` 必须是**解包后**的内层（`{params, returns, owner?}`） | `emit/rust.rs:283-291` 直接 `op.signature.get("returns")` | `Contract::from_schema` **必须解包** `x-function`；若保留包裹态，Rust 的签名见证会静默丢返回类型（`()`）与 `owner`（少一层 `super::`），`.i.rs` 编译才能发现 |
| D2 | `properties` 键 = op id ∪ 类型名 | 两侧 emit | 沿用 `is_constructor` 过滤（构造器由生成器产，手写进 schema 应报 `apiConstructorDeclared`） |

### 9.3 由此定下的闸面分层（三分，不是两分）

```
① 写闸（write / edit 落 .schema.json 时）
   目标：坏文本不得上盘。判「文件级自洽」，零跨元素、零读其他文件。
   = identity（A4） + 键空间（A5、D2） + 形态白名单（A1、A2） + 重复键文本扫描（B1）
     + verify 可复用三闸（七族名字 / 落点 / required⊆properties）
   失败 ⇒ Err 零落盘

② 装载校验（load_contracts 每次读盘时）
   = ① 的全部（同一函数，不是第二份）
   失败 ⇒ 停摆（ApiError）

③ 判据面（check.rs）
   跨元素（A3 前缀、refMissing、shadowing、nestedCrossCrate）+ 跨代码（signature / missing）
```

**① 与 ② 共用同一实现**——这是「零第二份校验」的直接落法：`write` / `edit` 在落盘前调 `contract::validate_schema(fqn, &text)`，落盘后 `load_contracts` 调同一个函数。两者只差错误映射（`Err` 字符串 vs `ApiError`）。

**A3（`$ref` 前缀）归 ③ 还是 ②？** 归 **②**：它只需「本 FQN + `$ref` 串」即可判（`ref_relative(from, to)` 是纯函数），不需要其他元素表。放在装载期意味着写错的 schema 直接停摆而不是产一条判据——考虑到症状是「生成物编译不过」，停摆是对的。

**A1 的形态白名单**（最容易漏的一条）：

```
properties.<k> 的二选一（MUST）
├─ {"x-function": {…}}            内层自由形状（可含 description，见 9.4）
└─ {"$ref": "#/$defs/<名>"}      且 <名> ∈ $defs
其余形态（内联 object / array / oneOf / 裸 type）⇒ Err
  ——「想复用类型就放进 $defs，想声明操作就用 x-function」
```

### 9.4 op 的散文说明：定案 `x-function.description`（Q11）

md 里 `### <op>` 与 `api-op` fence 之间那段说明是**给人读的操作语义**（`Foundation` 9 个 op 各一段，最长的 4 行）。切到 schema 后必须有落点。

**裁决：`x-function.description`（内层），且本变更不渲染进 `.i.*`。**

```jsonc
"properties": {
  "is_archi_rel_path": {
    "x-function": {
      "description": "一个相对仓库根的路径是否落在架构域之内（/ 与 \ 都先归一……）",
      "owner": "layout",
      "params": [{ "name": "rel", "schema": { "type": "string" } }],
      "returns": { "type": "boolean" }
    }
  }
}
```

选它的三个理由：

1. **不污染形态白名单**（§9.2 A1 只认 `{x-function}` / `{$ref}`）——兄弟键 `description` 方案会让白名单变成「二选一再加一个可选兄弟键」，闸的判据多一维。
2. **`.i.*` 逐字节不变** ⇒ `api:check` 零 diff、验收最简。
3. **`x-function` 是自有块，内层自由形状**——`description` 在这儿不是「自造 `x-` 键」，是规范内既有字段。附带精确化纪律：
   - **MUST NOT 自造新的 `x-` 前缀键**（如 `x-note` / `x-owner`）；
   - 允许的 `x-` 面封闭为既有三项：`x-function` / `x-class` / `x-common`；
   - `description` 是 JSON Schema 标准键，出现在两处：`x-function` 内层（op 语义）与 `$defs` 条目内（类型语义）。

**连带两条实现要求**：

| # | 要求 | 原因 |
|---|---|---|
| 1 | `Contract::from_schema` 解包 `x-function` 时**丢弃** `description` | `emit/rust.rs:283` 只读 `returns` / `owner` / `params`；多带一个键不影响，但 `ContractOp.signature` 是「emit 的输入」，不该混入渲染无关字段 |
| 2 | `properties.<k>` 的 `x-function` 内层**不做未知键校验** | 内层是自由形状（`async` / `description` / 将来可能有的 `deprecated`），闸只判外层形态 |

**写作纪律**（进骨架与 `bootstrap` 草稿的注解）：`x-function.description` 写**调用方要知道的语义**——「它做什么、边界在哪、为什么这样」，MUST NOT 写实现细节、MUST NOT 复述签名（签名就在同一处，复述是第二事实源）。

**`bootstrap` 草稿的 `description` 空位**：占位为空串 `""` 而不是「（待补）」——JSON Schema 的 `description: ""` 与缺省语义等价（都表示「无说明」），填「待补」反而会被 `description` 的读者当正文渲染。草稿只负责**让键存在**（提醒写者有这个位置），不负责提示措辞。写闸**不校验** `description` 是否非空（留空的接口说明是合法的中间态）。

### 9.5 退役面读数（`contract.rs` 的净落点）

`contract.rs` 现 1095 行，函数 30 个：

| 类别 | 函数 | 处置 |
|---|---|---|
| md 解析主体 | `parse` / `finish_section` / `parse_object` / `parse_object_value` / `string_end` / `decode_key` / `file_title` / `collect_reexport_prose` / `collect_surface_table` / `is_cjk` / `info_tokens` / `duplicate_key_error` | **退役**（约 600 行） |
| 可留 | `duplicate_keys`（给写闸用）、`verify` 的三闸 + 辅助（`is_identifier` / `function_param_names` / `methods_of` / `required_elements` / `reference_names` / `property_names` / `scan_x_function` / `verify_x_function_positions`）、`is_constructor` | 保留 / 改入参 |
| 需新写 | `Contract::from_schema`（含 A4 identity、A2 镜像、D1 解包） | 新增 |

⇒ `contract.rs` 从「md 解析器」变成「schema 校验 + `Contract` 投影」，粗略从 1095 行降到 400–500 行量级。

## 10 · 启动前的就绪评估

### 10.1 结论：可以启动，六件事在 proposal 里落定（都不是探索阻塞项）

设计面已闭合（§4 全裁决、§9 闸面推导、Q1–Q11 全部定案）。剩下六件是**提案必须写清、但已无分歧**的实现归属问题：

| # | 事项 | 现状 / 建议 |
|---|---|---|
| 1 | **`INDEX.toml` 的 `status` 语义重锚** | 现在 `ApiFormStatus::{Declared, Absent}` 由「契约 md 在盘」判定（`api/mod.rs:611-617` 的 `present` 来自 `contract_path().is_file()`）。读面切 schema 后 `present` 自然变成「schema 在案」，但**字段名 `declared` 会立刻名不副实**（它读起来像「模型意图」）。建议：值域改 `schema` / `absent`，或保留词而把 doc 提到头部说明 |
| 2 | **`ArchiApiRef` → `ApiStatus` + `assets` 是 breaking IPC** | 前端两处消费（`architecture-node.tsx:95-102` 文档 tab 三路径、`model-info.tsx` 契约段）+ 4 处测试夹具 + bindings 重导。稳妥案：**保留 `ArchiApiRef` 不动，只加 `assets`**（此时 `contractPath` 实际上是 `docPath`，在 JSDoc 里点明即可）；激进案：替换。建议稳妥案，把改名留到清理轮 |
| 3 | **`validate_schema` 的落点与命名** | `contract.rs` 改造后职责变成「schema 校验 + `Contract` 投影」，建议改名 `crates/core/archi/src/api/schema.rs` 承接校验、`contract.rs` 只留 `Contract` 类型与 `verify` 三闸（或反之，二选一须在 design 里点名） |
| 4 | **`write` / `edit` 需要穿 `root`** | `execute(root, name, input)` 持有 root（`tools.rs:73`），但 `write(input)` / `edit(input)` 不接收；路径已是 sandbox 后的**绝对路径**，故判据是「`path` 落在 `root.join(foundation::layout::archi_models_rel_dir())` 之下」。改动面：两个函数签名 + 两个调用点，纯机械 |
| 5 | **spec delta 覆盖面（估 7 个能力）** | `desktop-archi-api`（形态 / 生成物 / 路径派生 / bootstrap / INDEX）、`desktop-archi-api-check`（13 → 10）、`desktop-archi-agent`（`archi_module.assets` / `archi_api` 动作 / 知识骨架）、`desktop-ipc-type-bindings`（DTO 字段面）、`desktop-archi-contract-panel`（三路径 → assets）、`desktop-agent-execution`（write/edit 校验 + change 臂知识段）、`desktop-phase-prompts`（两份 prompt + implementation-generator）。零 delta：`desktop-archi-drift` / `desktop-archi-model` / `repo-archi-model` / `desktop-write-protection`（写闸是「坏文本不可产出」，不属保护面） |
| 6 | **版本纪律** | `packages/desktop/package.json` +1 patch；`src-tauri/Cargo.toml` 不动；`AGENT.md` 的「四类生成物」改三类（AGENT.md 不在 spec 面，但属规则面） |

### 10.2 建议：单一 change，tasks 分四段（每段独立可验收）

不拆两个 change 的理由：四组改动**互为正反面**——只切真相源不补写闸，等于让模型裸写唯一真相；只补写闸不切真相源，写闸校验的是一份没人读的生成物；只加 `assets` 不切真相源，`assets.carrier` 报的是错的载体。

但 tasks 顺序要保证每段都能独立验证：

| 段 | 内容 | 该段自己的验收锚 |
|---|---|---|
| 1 | 读面 + 生成面（core/archi：`load_contracts` 只读 schema、`Contract::from_schema`、四道新闸、`render_all` / `diff` 让位） | `api:check` 零差异；`archi_drift` 除 `docMissing` 文案外逐字段不变；`.i.*` / `INDEX.toml` 逐字节不变 |
| 2 | 判据面（词汇 13 → 10、`docMissing` 单成因、`severity` 基线） | 38 条全 `docMissing`、`status = clean`、0 纳管 |
| 3 | 写面 + 工具面（schema 写闸、`bootstrap` 双草稿、`link`、`archi_module.assets`） | 三处描述同步、写闸三类坏输入各一条 `Err`、`link` 出的串被 `apiRefMissing` 判可达 |
| 4 | 上下文 + 规则（骨架、两份相位 prompt、implementation-generator、change 臂知识段、AGENT.md） | 骨架门限、两份 prompt 断言、`archi_knowledge_test` |

### 10.3 建议的 change 名与探索依据

```
change: archi-three-asset-split        （workflow_type = requirement）
依据: .dev-team/explores/archi-agent-context-review.md §0–§10
      .dev-team/explores/archi-api-findings-redesign.md（后续 change，本变更不消费）
```

### 10.4 启动时最该盯的两个回退风险

1. **`.schema.json` 被误当生成物**（§4.7）——症状是「全红」而不是「少一条」，且 `orphaned` 列表会很长，容易被误读成「生成器坏了」。
2. **`Contract::from_schema` 忘记解包 `x-function`**（§9.2 D1）——`.i.rs` 的签名见证退化成 `const _: fn() -> () = super::name;`，要到 `cargo build` 才炸；建议给 `from_schema` 写一条直接对拍「`ops[i].signature == schema.properties[x].x-function`」的测试。

## 附录 · 证据锚点速查

| 事实 | 位置 |
|---|---|
| bootstrap 六段骨架 + 作者注 | `crates/core/archi/src/api/mod.rs:782`（`bootstrap_text`）、`:737`（`BOOTSTRAP_NOTE`） |
| 散文表格闸（留白豁免） | `crates/core/archi/src/api/contract.rs:505-530` |
| `Contract` 字段面 | `crates/core/archi/src/api/contract.rs:61-78` |
| `build_schema`（生成方向） | `crates/core/archi/src/api/schema.rs:88` |
| `GENERATED_NOTICE` | `crates/core/archi/src/api/mod.rs:1116` |
| `carrier_path`「不是两份资产」 | `crates/core/archi/src/api/paths.rs:300-310` |
| `ref_projection`（全或无） | `crates/core/archi/src/api/mod.rs:924-948` |
| 生成物收面含 `*.schema.json` | `crates/core/archi/src/api/diff.rs:100-116` |
| 十三值与 severity | `crates/core/archi/src/drift/report.rs:200-281` |
| 判据求值序 | `crates/core/archi/src/api/check.rs:1-60` |
| `gating_findings` | `crates/core/archi/src/api/mod.rs:332-353` |
| `write` / `edit` 执行体（零校验） | `crates/infra/agent/src/sdk/tools.rs:372-420` |
| 工具定义体量 | `crates/infra/agent/src/sdk/mount.rs:317-619` |
| 骨架 | `crates/infra/agent/src/archi-knowledge.md`（1 459 B） |
| preamble 合成 | `crates/infra/agent/src/sdk/preamble.rs`（`freeze`） |
| change 臂 `preamble: None` | `crates/infra/agent/src/compose.rs:130` |
| 矩阵 / 写权锁 | `crates/infra/agent/src/compose.rs:217-278` |
| `preclose` T1 判据 | `crates/infra/agent/src/sdk/preclose.rs:100-120` |
| 架构页文档 tab 渲染 | `packages/desktop/src/views/architecture/components/dashboard/architecture-node.tsx:95-111` |
| 架构页会话归属 | `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts:25-30` |
| 现有契约语料（11 份） | `.dev-team/architecture/models/**/*.md` + 同目录 `*.schema.json` |
