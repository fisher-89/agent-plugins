# P2 · `archi_scaffold` 升级为字段词典

workflow_type: requirement

> **来源**：从 [`archi-agent-contract-knowledge-gap`](archi-agent-contract-knowledge-gap.md)
> 第十一节拆出。**前置片 P2**：必须先于主篇切片 A 落地（否则 A 删骨架「字段纪律」后，
> 指路句会指向一个不完整的词典）。
>
> **状态：可实施，未落地。形态已裁决 B1**（保留 TOML 草稿，注释扩成字段表，零新参数）。

## 一、为什么能前置

1. **独立验收面**：Read 档、零写盘，纯模板 + 断言。
2. **不碰五处立场**：`archi_scaffold` 是只读模板工具，不在"写模型工具"之列（主篇 §7.1
   第 1 条改的是 `write`/`edit` 的禁令，与本工具无关）。
3. **时序必须在 A 前**：A 会把骨架的「字段纪律」827 B 压成判断句，指路句写
   「字段形态用 `archi_scaffold` 取词典」——若词典还没建好，A 上线即知识缺失。

## 二、现状：漂移修复后已经做到哪一步

`6a8fb10`（漂移修复）已经把模板补了一大截，P2 的剩余 delta 比原先估计小：

| 字段 | 模板现状 | 还缺 |
|---|---|---|
| `kind` | 四值语义逐值（`package` 至多一条包根 / `domain` 容器 / `module` 含 crate 视图单文件 / `composite` 纯分组） | — |
| `path` | module 必填可多条 / package 至多一条 / domain·composite 选填；`./` 前缀、目录尾 `/` | — |
| `entry` | 约定默认三档（单文件根 / index.ts(x) / src/lib.rs）+「指不到时写」 | — |
| `gate` | 值 off / warn / strict + 缺省 off + ratchet 语义 | — |
| `api` | declared / none + 缺省 declared | — |
| `doc` | 单值、仅 `api = none` 可写、declared 即 `apiUnexpected` | — |
| `summary` | 一行摘要 | — |
| `metadata` | 内联表与子表等价（正反条件） | **非字符串值 ⇒ `invalidValue`** |
| `[[relation]]` 四键 | from / to / desc / apis 各一行 | — |
| `apis` | 「选填名字清单，只写名字、可留空、生成物面留空；差额看 `archi_drift`」 | **非空 / 不重复 / `apis = []` 拒收**三条 |

实测锚：`sdk/archi_tool.rs` L198-236（`SCAFFOLD_TEMPLATE`）；测试 `archi_scaffold模板kind行四值语义`、
`archi_scaffold模板path行出路`、`archi_scaffold关系块全注释`。

## 三、方案

### 3.1 模板补三条 + 一条

```
# apis：选填名字清单，只写名字（禁签名 / 类型形状 / :: 限定路径 / 空白）；
#       逐项非空、同名不重复、apis = [] 与省略同义（显式空数组被拒）；生成物面留空
```

```
# metadata：内联表与子表等价（[element."X".metadata] 写在已声明元素之后零诊断）；
#           值 MUST 是字符串（数字 / 布尔 → invalidValue）
```

其余字段不动——**P2 的大部分工作已经在漂移修复里完成了**，这里只补漏。

### 3.2 真正的增量：把模板从「手写清单」变成「`ALL` 的视图」

模板现在与字段面的一致性靠"人记得同批改"。P2 给它上机械锚：

| 锚 | 形式 |
|---|---|
| 字段全集 | 断言 `ArchiElement` 的每个字段名（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）都在草稿里出现（键行或注释行），**新增字段未补词典即红** |
| 值域派生 | `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` 逐值 + 语义在场（前两者已有测试，`ApiStatus` 补一条） |
| 关系四键 | `from` / `to` / `desc` / `apis` 逐键（已有） |
| 可解析 | 草稿经 `TomlParser` 零诊断（已有，`archi_scaffold草稿零结构诊断`）——**MUST 继续成立**：扩注释时不得引入裸 TOML 行 |

字段全集那条要解决一个技术细节：Rust 里没有"遍历 struct 字段名"的内建能力。两种取法：

- **A**：手写一个 `const FIELD_NAMES: [&str; 8]` 与 `RawElement` 同文件，紧邻定义——新增
  字段时它就在眼前，且可用 `#[deny(dead_code)]` 之外的方式提醒（比如与字段注释并列）。
- **B**：用 serde 的 `Deserialize` 尝试反序列化一个"全字段 JSON"来探测——过度设计。

取 A。它仍是"第二份清单"，但**位置贴在字段定义旁**（同 `dto.rs` 的 `ALL` 常量范式），
且模板断言直接消费它——比散在模板字符串里强。

### 3.3 可选：`archi_module` 响应附「该元素可改字段」

主篇原 L1 设计里有这一项。现在**倾向不做**：字段面已由 `archi_element` 的**参数 schema**
承载（每字段带 `description`），每个响应再复述一遍是第三份。

## 四、验收面

| # | 断言 |
|---|---|
| 1 | 草稿含 `ArchiElement` 字段全集逐名；`FIELD_NAMES` 与 `RawElement` 同批维护（测试断言两者集合相等） |
| 2 | `apis` 注释含「只写名字」「非空」「不重复」「`apis = []` 与省略同义」四条 |
| 3 | `metadata` 注释含「非字符串值 ⇒ `invalidValue`」 |
| 4 | `ElementKind::ALL` 四值语义、`GateTier::ALL` 三值、`ApiStatus::ALL` 二值逐值在场 |
| 5 | 草稿经 `TomlParser` 零结构诊断（既有断言继续绿） |
| 6 | 零写盘（既有断言继续绿） |
| 7 | 响应仍过 `cap_result`，且模板体量在 50 KB 内（估算 ~4 KB） |

## 五、不做的

| 面 | 理由 |
|---|---|
| `mode = "draft" \| "dict"` 双形态 | D2 裁决 B1：一个工具一个形状 |
| 纯 Markdown 词典（不产 TOML） | 会丢掉「自证可解析」这条锚（它是「示例 TOML 从骨架删除」的唯一背书） |
| 按字段查询（`field = "path"`） | 新增参数面；草稿是整份，agent 一次读全更省调用 |

## 六、与主篇的关系

| 主篇位置 | 依赖 P2 的点 |
|---|---|
| §3 目标知识的「工具清单」 | 写「`archi_scaffold`（字段词典）」——词典得先是词典 |
| §6 迁移表「字段纪律 827 B → 参数 schema + 词典」 | 词典是迁出目标之一 |
| §8 切片 B | 本篇即该切片的展开 |
