# P1 · 契约起草引导与工具描述补位

workflow_type: requirement

> **来源**：从 [`archi-agent-contract-knowledge-gap`](archi-agent-contract-knowledge-gap.md)
> 第十一节拆出。**前置片 P1**：可作为独立变更先行落地，不依赖 archi 写工具。
>
> **状态：可实施，未落地。** 改动面 = 一个模板函数 + 三处工具描述 + 一个 `##` 段扩充。

## 一、为什么这片能前置

两条判据（与主篇 §11 同源）：

1. **独立验收面**：全部改动都在只读面（`archi_api` / `archi_module` 的描述与草稿），
   端到端可验证，不需要写工具存在。
2. **不触碰五处立场**：主篇 §7.1 那五处（无专用写工具 / 写入不校验 / 注入矩阵 / 两个
   phase prompt / 分档判据）此处零涉及。

这片来自 v1 分析（原「契约文档与 API 知识缺口」）的 5 个缺口，方向转向工具化时被搁置，
但**一个都没有被修**——实测如下。

## 二、现状：五个缺口逐条实测

| # | 缺口 | 实测 | 锚点 |
|---|---|---|---|
| 1 | 草稿无作者注 | `bootstrap_text` 只出「职能 / 不变量 / 操作 / 类型」四段 + 「（待补）」；不含「边界 / 承载理由」，不产 `api-reexport` | `core/archi/src/api/mod.rs` L754-783 |
| 2 | `check` 冷启动语义无解释 | `enabled` 字段**在响应里**（`ArchiApiCheck.enabled`），但工具描述与知识骨架都未说「`findings` 全空可能是契约面未启用」 | `api/mod.rs` L291；`mount.rs` L296-315 |
| 3 | 生成物身份未点名 | 描述只有泛称「生成物」，未点名 `.schema.json` / `INDEX.toml` / `.i.*` | `mount.rs` L296-315 |
| 4 | `schemaPath` / `generatedPath` 未指路 | description 已提 `contractPath`（「declared 走 api.contractPath」），但另两条路径**零提及**——而 DTO 早就给了三个 | `mount.rs` L251-272；`dto.rs` L305-315 |
| 5 | 起草第一步不明确 | 描述未说「新建 / 补契约的下一步是 `bootstrap` 取草稿」 | 同上 |

缺口 2 的杀伤力最直接：契约面未启用时 `findings.total = 0`，而知识骨架写的是「非空即
逐条修」——**全空被读成全绿**。

## 三、方案

### 3.1 草稿作者注：必须是 fenced 块，不能是 HTML 注释

`bootstrap_text` 追加一段作者注。**放哪里**有硬约束，实测契约解析器（`api/contract.rs`
`parse`）的四条判据决定了唯一安全位置：

| 载体 | 后果 |
|---|---|
| 裸正文 / HTML 注释 | 进入 `section.body`：行首 `- X`（无 CJK）被 `collect_reexport_prose`（`contract.rs` L1004） 抽成透传导出名；`\| \`Name\` \| …` 表格行被 `collect_surface_table`（`contract.rs` L1048） 抽成「散文表格复述」⇒ 与 `api-reexport` fence 键集或契约名字集不等即**契约错误** |
| `### ` 标题 | 被当成 op 锚点，**其下必须紧随 `api-op` fence**，否则报「op 缺代码块」 |
| `## ` 标题 | 成为散文段，同样受上两条抽取面扫描 |
| **非 `api-*` 的 fenced 块** | **解析器显式跳过**（`// 其余 fence（示例代码等）不参与契约，内容不入散文区`）⇒ 完全惰性 |

所以注释放 ` ```text ` 块内。附带收益：块内可以放 `### ` 与表格，零解析风险。

草稿结构变为（六段 + 一段惰性作者注）：

| 段 | 来源 | 内容 |
|---|---|---|
| H1 `# <FQN>` | 现状保留 | 必须等于文件名主干（`file_title` 判据） |
| `## 职能` | 现状保留 | `（待补：一句职能）` |
| `## 不变量` | 现状保留 | `（待补）` |
| `## 操作` | 现状保留 | 逐个 `### <op-id>` + 紧随 ` ```json api-op ` fence（自动抽取） |
| `## 类型` | 现状保留 | 单个 ` ```json api-types ` fence（自动抽取） |
| `## 边界` | **新增** | `（待补：本模块**不**负责什么）` |
| `## 承载理由` | **新增** | `（待补：为什么它是独立元素）` |
| 末尾 ` ```text ` 块 | **新增** | 作者注，见下（解析器显式跳过，完全惰性） |

作者注块内容（原样）：

```
作者注（本块不参与契约解析）：
- 三类 fence：api-op（`### <op-id>` 下紧随）/ api-types（本模块自有 $defs，同文件唯一）/
  api-reexport（键 = 导出名、值 = 直接子元素 FQN，同文件唯一）
- 散文只留语义：职能 / 边界 / 不变量 / 承载理由；MUST NOT 用表格复述名字清单
  （名字清单的唯一持有者是三类 fence）
- api-reexport 形态：{ "导出名": "直接子元素 FQN" }；散文里的透传列表须与键集逐名相等
- 起草后跑 archi_api check 看两轴；契约面未启用时 enabled = false 且 findings 恒空
```

三条注意：

1. **H1 不能动**：`file_title`（`contract.rs` L814）要求 H1 = 文件名主干，否则解析失败。
2. **新增两段必须是 `## `**（不是 `###`），且正文只写「（待补：…）」——含 CJK，故不被
   抽成透传导出名。
3. **作者注块放文件末尾**：它惰性，但放在 `api-types` 之后可让"可读部分在前、说明在后"。

### 3.2 三处工具描述补位

`archi_api`（`mount.rs` L296-315）追加三句，`archi_module`（L251-272）追加一句：

| 工具 | 追加内容 |
|---|---|
| `archi_api` | ①「新建 / 补契约先 `bootstrap` 取草稿」；②「契约 md 同目录 `.schema.json` 与 `graph/INDEX.toml`、入口同级 `.i.rs` / `.i.ts` 均为**生成物**，MUST NOT 手改」；③「`enabled = false` = 契约面未启用（`findings` 恒空**不等于**零问题）」 |
| `archi_module` | 「`api.schemaPath` / `api.generatedPath` 给出另两条生成物路径（`contractPath` 已指手写真相）」 |

**零 preamble 字节**：三条都落工具描述，不进知识骨架，故骨架体量锚（现 2596 B）不受影响。

### 3.3 spec 同批

| spec | 改动 |
|---|---|
| `desktop-archi-agent` L121-160（`archi_api` Requirement） | 描述面追加上述三句为 MUST；`bootstrap` 段追加「草稿 SHALL 含作者注 fenced 块与 `## 边界` / `## 承载理由`」 |
| 同 L165-200（`archi_scaffold`…无关） | 不动 |
| `desktop-archi-api`（草稿形态归本能力） | 若其描述 `bootstrap` 输出四段，需同步为六段 |

先核 `desktop-archi-api` 是否已断言四段——若是，P1 要同批改，否则 spec 与代码分叉。

## 四、验收面

| # | 断言 |
|---|---|
| 1 | `bootstrap` 草稿含 `## 职能` / `## 不变量` / `## 操作` / `## 类型` / `## 边界` / `## 承载理由` 六段 |
| 2 | 草稿里作者注在 ` ```text ` 块内；块内含「api-reexport」与 `{ "导出名": "直接子元素 FQN" }` |
| 3 | **草稿本身过契约解析零错误**：把草稿写到临时工作区的派生路径后 `Contract::parse` 成功（作者注惰性、六段零触发） |
| 4 | `archi_api` 描述含「bootstrap」指路、「`.schema.json`」「`INDEX.toml`」「`.i.」三类生成物、`enabled = false` 的冷启动口径 |
| 5 | `archi_module` 描述含 `schemaPath` 与 `generatedPath` |
| 6 | 知识骨架字节**不变**（2596 B 冻结值不动） |
| 7 | 零写盘：`bootstrap` 调用前后架构域字节相同 |

第 3 条是这片最关键的锚：注释块的位置一旦放错（裸正文 / `## ` 段内），草稿会**通过
`bootstrap` 但写盘后契约报错**——那是比没注释更糟的状态。

## 五、不做的

| 面 | 理由 |
|---|---|
| 「职能 / 边界 / 不变量 / 承载理由」变成机械判据 | 那是散文语义，不可判定；本片只给草稿骨架与建议 |
| 给 `bootstrap` 加参数（如 `--with-reexport`） | 保持 action 封闭；作者注是固定内容 |
| 自动生成 `api-reexport` fence | 草稿只能给形态；键值要人填（直接子元素 FQN 是设计判断） |

## 六、与主篇的关系

P1 落地后，主篇 §5.2 的第 3 / 4 条 warnings（`docMissing`）文案里「用 `archi_api` 的
`bootstrap` 取草稿」才真正**指得动**——现在这句指路指向一个不产作者注的草稿。
