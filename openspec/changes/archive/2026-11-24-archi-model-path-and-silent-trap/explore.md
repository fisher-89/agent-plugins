# 架构模型的两处文本漂移：`package` 的 `path` 与「静默陷阱」

workflow_type: requirement

> 来源：从 `archi-agent-contract-knowledge-gap` 第七 / 八节抽出，独立成篇。两处都是
> 「同一事实在 spec / 知识骨架 / 工具模板 / 实现之间给出不同说法」，且都能**脱离
> 工具化方向独立修**。本篇持有裁决与修法；主篇只引用结论，不复述表格。

## 摘要

| # | 漂移 | 性质 | 影响面 |
|---|---|---|---|
| A | `package` 的 `path` 必填性四方不一致 | 措辞 / 语义漂移 | 知识骨架、`archi_scaffold` 模板、未来写工具的入参描述 |
| B | spec 声称的「静默陷阱」实测不静默 | **理由不成立**（结论仍成立） | `desktop-archi-agent` 的分档判据、一条 C 档纪律的理由 |

共同根因：**「不可判定 / 静默」的判断没有实测就写进了规格**。

---

## 一、case A：`package` 的 `path`

### 1.1 四种说法

| 来源 | 说法 |
|---|---|
| `desktop-archi-model` L56（解析层） | `path` SHALL 为字符串数组（**缺省空**），不强制 |
| `desktop-archi-agent` L380（kind 语义段） | `package` = 包（最小编译单元 / 独立依赖；**包根与持有文件的包可带 path**） |
| `archi-knowledge.md`「字段纪律」 | 「`path`：模块 / **包必填**」 |
| `SCAFFOLD_TEMPLATE`（`sdk/archi_tool.rs` L198+） | 「kind：package = 包根（**无 path**）」＋「path：容器（package / domain）**恒空**」 |

实现侧：`parse/toml.rs` 的 `RawElement.path` 是 `#[serde(default)]`（缺省空，不报错）；
`drift/mod.rs` L113-127 对 `path.is_empty()` 出 `noPath` **警告**（不是错误）。

### 1.2 实测分布（本仓 22 个 package）

19 带 `path`，3 不带 —— 不带的恰好是纯分层容器：

| 元素 | `path` | 角色 |
|---|---|---|
| `Backend` / `Desktop` | `./packages/desktop/src-tauri/` / `./packages/desktop/src/` | **兜底持有者**（认领未被更具体元素覆盖的文件） |
| `Backend.Core` / `Backend.Infra` / `Backend.Tools` | 无 | **纯分层容器**（自身不对应任何目录） |
| 其余 16 个 | 各自 crate / 目录 | 持文件包 |

（`01-app.toml` 表头注释自己写着「两个包根……各持根路径，兜底未被更具体元素认领的文件」
——真实用法与模板的「包根无 path」正好相反。）

### 1.3 裁决（终裁补「至多一条」上界，见 §9.1）

**`path` 对 `package` 可选：纯容器留空；持文件 / 兜底必填。**

- 知识骨架「模块 / 包必填」——错在把纯容器也要求了（本仓 3 个容器全部违反）。
- 模板「package = 包根（无 path）」「容器恒空」——错在把"包根"当成 package 的默认形态，
  而最外两层的包根（`Backend` / `Desktop`）恰恰**必须**带 path 才能兜底。
- spec L56 与 kind 语义两处是对的。

**为什么这条必须在写工具之前裁**：写工具的 `path` 参数描述会直接引用这条规则；
把错误规则固化进代码后，改回来要动参数面与全部用例。

### 1.4 修法（措辞已被 §7 / §9 取代：终裁为 package 的 `path` ≤ 1 条、0 条合法；落地以 §10.2 清单为准）

1. `archi-knowledge.md`：「模块 / 包必填」→「模块 / 包填了才参与归属；纯容器（不对应
   任何目录）留空」。字节持平（±10 B），体量锚无需改。
2. `SCAFFOLD_TEMPLATE` 两行：
   - `kind` 注释：「package = 包根（无 path）」→「package = 包（纯容器可无 path；持文件 / 兜底必填）」
   - `path` 注释：「容器（package / domain）恒空」→「纯容器留空；持文件必填」
3. 补机械锚（模板现在只有"零诊断"一条锚）：`archi_tool_test.rs` 增用例，断言模板的
   `kind` 语义覆盖 `ElementKind::ALL` **四值**（现在只覆盖三值，`composite` 零承载），
   并断言「package 的 path 可选」一句在场。

---

## 二、case B：所谓「静默陷阱」

### 2.1 spec 的断言（原文）

`desktop-archi-agent` L484 补充规则①：

> **静默错误必须留上下文**：`metadata` 写子表、`[element.App.Auth]` 点分嵌套会被解析成
> **另一个元素**（`deny_unknown_fields` 不触发），工具无法在动作前拦住 ⇒ 归 C 档，散文必留。

同 spec L384：

> 一元素一表头、键为带引号的完整 FQN 单键（禁点分嵌套写法——`[element.App.Auth]` 会被
> 读成**另一个元素**）；`metadata` MUST 用内联表（子表同样会被读成另一个元素）

### 2.2 实测三形态

| 形态 | 实测 | 锚点 |
|---|---|---|
| `[element.App.Auth]`（未引号点分） | **`unknownField`**，文案点名非法子键 `Auth`；整份源**全或无**（不产出任何元素） | `parse/toml_test.rs` L342-365 |
| `[element."App.Auth".metadata]` | **`missingField`**（`App.Auth` 缺 `kind`），不产出元素 | 同文件 L304-333 |
| `[element."X".metadata]`（子表挂自己） | **零诊断、语义正确** —— TOML 内联表与子表**等价** | 无测试；TOML 语义直推 |

### 2.3 机制：为什么"会被读成另一个元素"不成立

`[element.App.Auth]` 在 TOML 里是 `element` → `App` → `Auth` 的嵌套表。反序列化目标
`BTreeMap<String, RawElement>` 拿到键 `App`、值 `{ Auth = {…} }`；`RawElement` 带
`#[serde(deny_unknown_fields)]`，`Auth` 不在字段面 ⇒ **立刻 `unknownField`**。
不是"读成另一个元素"，是**整份源解析失败**。

`[element."App.Auth".metadata]` 则是 `element` 下键为字符串 `"App.Auth"` 的元素，
它只含 `metadata`、缺 `kind` ⇒ `missingField`。也不是静默。

真正静默的只有 `[element."X".metadata]`——而它是**合法且语义正确**的写法。

### 2.4 后果

1. **结论仍成立，理由要换**：点分表头确实该禁、该留散文——但原因是「写前无法校验
   （写面是自由文本）」＋「代价是整份源解析失败」，不是「静默错读」。
2. **「`metadata` MUST 用内联表」这条本身是错的**：子表等价，写子表不产生任何诊断。
   真正该禁的只有**未引号点分表头**。
3. spec L484 三条补充规则里，①的支撑失效；②③不受影响（写入定位 / `kind` 语义）。
4. 测试 `骨架静默陷阱四条在场`（`archi_knowledge_test.rs` L340）四个断言里
   「内联表」一条建立在错误认知上；其余三条（带引号 FQN / 目录尾 `/` / `dependsOnAncestor`）
   仍有效。

### 2.5 修法

1. `desktop-archi-agent` L484 ①整条重写：

   > **形态错误不是静默的，但发生在写盘之后**：未引号点分表头 `[element.App.Auth]`
   > 归 `unknownField`、整份源解析失败（全或无）；`metadata` 子表与内联表**等价**，
   > 无需禁止。归 C 档的理由是**写前不可校验**（自由文本写面），不是不可判定。

2. 同 spec L384 同步改写（删「会被读成另一个元素」与「`metadata` MUST 内联」）。

3. `archi-knowledge.md`「位置与键形」一句改写：

   > 表头键 MUST 带引号写完整 FQN 单键（未引号点分 → `unknownField`，整份源不解析）；
   > `metadata` 内联表与子表等价。

   字节持平。

4. `archi_knowledge_test.rs` L340：断言「内联表」换成「带引号」，测试名去掉「静默」。

---

## 三、共同根因

两处同形：**把推断当事实写进规格**。

```
作者推断（未实测）
   ↓ 写进 spec / 模板
规格文本
   ↓ 实现与测试按真实语义写
实测行为 ≠ 规格断言
   ↓ 没有「规格 vs 实测」的对拍
漂移长期存活
```

- case A 的存活机制：四方各说一套，**没有任何测试对拍"package 的 path 是否必填"**。
- case B 的存活机制：前提是 `toml::Value` 时代的手工解析（那时确实静默）；改成强类型
  `struct + deny_unknown_fields` 后陷阱消失，**spec 没跟着改**；而测试只断言 md 里
  有这四个词、不断言这四个词是否真实。

提炼：**「可判定性」是需要实测的属性**，不能靠读代码推断。可选元纪律：凡以
「静默 / 不可判定 / 工具拦不住」为由把知识留在散文的句子，MUST 附带一个钉住该断言的
**行为**测试（而不是"断言 md 含某词"）。

---

## 四、变更建议

两处都**不依赖工具化方向的裁决**，可作为独立小变更先落地：

| 面 | 文件 | 改动 |
|---|---|---|
| 知识骨架 | `infra/agent/src/archi-knowledge.md` | 两处改写（字节持平） |
| 骨架测试 | `infra/agent/src/archi_knowledge_test.rs` | `骨架静默陷阱四条在场` 改名 + 换一条断言 |
| 工具模板 | `infra/agent/src/sdk/archi_tool.rs` `SCAFFOLD_TEMPLATE` | 两行改写 |
| 模板测试 | `infra/agent/src/sdk/archi_tool_test.rs` | 加 kind 四值覆盖断言 |
| 能力 spec ×2 | `desktop-archi-agent` L384 / L484 | 改写（换理由、留结论） |

风险：极低（纯文本 + 断言），零行为变更。

---

## 五、证据锚点

- 解析字段面：`packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` L20-60
- 点分表头：`.../parse/toml_test.rs` L342-365
- metadata 子表：`.../parse/toml_test.rs` L304-333
- `path` 缺失：`.../drift/mod.rs` L113-127（`noPath` 警告）
- 知识骨架：`.../infra/agent/src/archi-knowledge.md`
- 模板：`.../infra/agent/src/sdk/archi_tool.rs` L198-236
- 知识分层：`openspec/specs/desktop-archi-agent/spec.md` L375-408、L471-500
- 字段面 spec：`openspec/specs/desktop-archi-model/spec.md` L56
- 真实分布：`.dev-team/architecture/graph/model/*.toml`

---

## 六、未决

1. 第三节的"元纪律"要不要真写进 spec？它是防同类漂移复发的机制，但可能被视为过度约束。
2. `metadata` 子表既然合法，是否该**主动推荐**子表形态（多行更好读）？现在推荐内联表，
   会让 `metadata = { owner = "x", layer = "y" }` 一行变长；改推荐则现有模型"风格不一致"。
   倾向不动。

---

## 七、修正 case A：`path` 是**根**，不是**清单**（root vs paths）

> 来源：用户提示「package 的 path 代表包根目录（此处应该区分 root 和 paths）」。

### 7.1 一个字段装了两套语义

`path: Vec<String>` 同时被当作：

```
语义 A「根(root)」：我的领地 = 根之下、未被更细元素认领的一切
    Backend        = ./packages/desktop/src-tauri/     ← 目录前缀根（包根，兜底）
    Views.Welcome  = ./views/welcome-view.tsx          ← 文件级根（单文件组件）

语义 B「清单(paths)」：我就是这几个文件，别的不沾
    Desktop.Shell  = app.tsx + routes.tsx + main.tsx + sidebar + bell   ← 5 条

机器上二者同一：owner.rs 只做「归一化 + 最长前缀匹配」，
文件型 path 就是「恰好一个文件的根」。→ 两种语义都没写错，但**读模型的人分辨不出**。
```

实测基数（`.dev-team/architecture/graph/model/*.toml` 全 55 元素）：

| kind | path 缺省 | 1 条·目录 | 1 条·文件 | 多条·文件 |
|---|---|---|---|---|
| `package` | **3** | **19** | 0 | 0 |
| `domain` | **3** | 0 | 0 | 0 |
| `module` | 0 | 18 | 7 | **1** |
| `composite` | 0 | 0 | 0 | 0 |

规律强到可以当判据：**path 的基数与形态完全由 kind 决定，而代码零校验**
（`kind` 在 `crates/core/archi` 里除 `as_str` / `ALL` 外**无任何语义分支**，
`scan/owner.rs` 不读 kind）。19 个 package 全部「一条、目录形态」；
唯一的多条 path 元素是 `Desktop.Shell`（`module`）。

### 7.2 「包根」这个词现在指两个相反的东西

| 用法 | 出处 | 含义 |
|---|---|---|
| 兜底包根 | ADR 0001 §3、`01-app.toml` 表头注释 | `Backend` / `Desktop`，**必须**带 path |
| 包根（无 path） | `SCAFFOLD_TEMPLATE` 的 kind 注释 | **MUST NOT** 带 path |

模板那句「package = 包根（无 path）」是本篇最锋利的一处措辞事故：它把「包根」定义成
了「没有根的包」。

### 7.3 新裁决（§7.3 初版 → 经 §9 修正为终裁）

初版裁决曾写成「package 恒有且仅有一个包根（恰 1 条）」。**终裁（§9.1）是「至多一条」**：
0 条合法（纯容器），≥2 条才停摆。两版的共同点是不再要求「package 必须带 path」，
差别只在「0 条」这一侧——`Backend.Core` / `Backend.Infra` / `Backend.Tools` 三个
纯容器因此**不必迁 kind**，`composite` 归位降为独立可选项（§8 / §10）。

```
package   = 包（最小编译单元 / 独立依赖）⇒ 至多一个包根：path ≤ 1 条（0 = 纯容器）
module    = 模块（目录型 / 单文件 / 多文件装配）⇒ path 不设上界（多根清单合法）
domain    = 业务聚合容器 ⇒ path 选填
composite = 纯分组容器 ⇒ path 选填（用户裁：可持文件，不设恒空约束）
```

「包根」一词由此只有一个用法：**package 的那一条 path（若写了）就是它的包根目录**。
模板里「package = 包根（无 path）」的定义事故随之消解（改写见 §9.3）。

## 八、`composite` 是纯容器的**专名**，而本仓零实例

> 来源：用户提示「纯分层容器有 composite 类型」。

### 8.1 四处说法（第六版不一致，且 ADR 自己就矛盾）

| 来源 | 分层容器（`Backend.Core` 等）是什么 |
|---|---|
| 模型实际 | `kind = "package"` + 无 path |
| ADR 0001 **§2** | 「`core` / `infra` / `tools` 分层容器同为 `package`（其子元素是包）」 |
| ADR 0001 **§3** | 列进「`domain` / `composite` 容器（…`Backend.Core` / `Backend.Infra` / `Backend.Tools`…）」一组 |
| 知识骨架 | 「模块 / 包必填」⇒ 这 3 个元素当场违规 |
| 模板 | 「package = 包根（无 path）」/「容器（package / domain）恒空」，**composite 零承载** |
| `desktop-archi-agent` L386 | 「`package` = 包（…；包根与持有文件的包可带 `path`）」——替「package 无 path」背书 |

ADR 0001 §2 与 §3 互相否定，是这一族漂移的**发生器**（不是接受器）。

### 8.2 迁过去会发生什么：几乎什么都不会

- `kind` 无任何判定消费 ⇒ `archi_drift` 读数零变化（`model.warnings` 仍 6 条 `noPath`、
  `status` 仍 `clean`、`passed` 仍 `true`）；结构性诊断（`orphanHierarchy` /
  `dependsOnAncestor` / `unknownReference` / `cycle`）只看 FQN 前缀，零影响。
- 前端 `element-tree.tsx` 已备好第四枚图标（`composite` → `LayersIcon`），
  即 UI 面无缺口、只换图标。
- 代价：模型 3 行 kind + ADR §2/§3 合并矛盾 + 知识骨架 kind 段 + 模板两行
  + `repo-archi-model` baseline 计数说明（元素数不变，只有 kind 分布变）。

### 8.3 顺带暴露的第二类缺陷：`noPath` 对容器是**误报**（本线未采纳）

> 用户终裁「composite 可选 path」⇒ 容器无 path 仍会报 `noPath`，本条修法链
> **不进本次变更**，原样保留为未跟进线索（§10）。

`drift/mod.rs` L113-127 对**所有**无 path 元素出 `noPath`，文案是
「元素 X 未声明 path（不参与归属；依赖提取会把它视作无主）」。
但容器**本来就不该**认领文件——这句话对容器是错的语义。
而 baseline spec 把噪音钉成了契约：`repo-archi-model` 三处逐字写着
「`model.warnings` SHALL 恰为 **6 条 `noPath`**（6 个容器当前未填 `path`——选填字段，
本基线留空）」。

于是存在一条自洽的修法链：

```
给纯容器一个专名（composite）
      ↓ 于是「不持文件」成为 kind 的属性，而非「忘了写 path」
noPath 对 composite 豁免（不是警告，是定义）
      ↓
baseline 的「6 条 noPath」改为 3 条（只剩 3 个 domain 容器）/ 或 0 条（domain 也豁免）
      ↓
「未声明 path」重新只指真正的问题：忘了给叶子元素写根
```

这条链与第七节的「package ⇒ 恰一条包根」共用同一个判据表，是一件事的两面。

### 8.4 未决

1. 三个分层容器迁 composite 后，`domain` 与 `composite` 的分工要不要写死
   （domain = 业务聚合、可持文件；composite = 纯分组、恒空）？ADR 现在两者混说。
2. `noPath` 豁免的边界：composite 恒豁免？domain 有子元素时豁免？
   一旦豁免，`repo-archi-model` 的三处「恰 6 条」都要同批改写（baseline 契约变更）。
3. `kind` 层级序（包 > 领域 > 模块，spec L56 写了）至今零校验；
   若同时补「父 kind 不得小于子 kind」这类判据，就是个独立能力，不该顺手做。

---

## 九、L1 落地设计（用户已裁：**parse 期拒收 ≥2 条；package 允许 0 条**）

### 9.1 终稿判据（一条，只守 package 的上界）

| kind | path | 本仓实测 | 误伤 |
|---|---|---|---|
| `package` | **≤ 1 条**（0 条合法：纯容器；≥2 条 ⇒ `invalidValue`，整图停摆） | 19/19（0 条 3 个、≥2 条 0 个） | 0 |
| `module` | 不校验（多条合法：`Desktop.Shell` 5 条） | 26 个 | — |
| `domain` / `composite` | 不校验（选填） | 3 个全 0 条 | — |

**为什么正是这条**：`path` 的「根本义」在契约面**早已是单值假设**（见 §9.6），
而 `scan/owner.rs` 的归属面按多前缀建索引。两个消费面对同一条字段的基数假设不同，
今天靠「没人给 package 写两条」维持平衡。这条判据把**契约面已有的假设**变成模型面的
显式纪律——不是新增约束，是把暗假设点亮。

「区分 root 与 paths」由此落成可判形态：**package 至多一个 root；module 是若干根的清单**，
且多根的歧义（哪条是 first）只可能出现在 module 上，而 module 的契约面缺口另有处置
（§9.6 末）。

### 9.2 落点：`parse/toml.rs::to_model`（用户裁 ①）

```
to_model 逐元素校验（现有 validate_apis 的形态可直接复用）
   ↓ 诊断带 span：指向**第 2 条 path 的 Spanned 位置**（比元素表头更准）
parse_dir 照常累积（不停摆）
   ↓
graph_view（L754-777）：diagnostics 非空 ⇒ elements/relations 清空 ⇒ 整图停摆
```

零新词（`DiagnosticKind` 仍九值、`invalidValue` 复用）、零生成物改动、与 `apis` 五拒同源。
容错为零是**已知并接受的代价**：package 写两条 path 在今天的模型里不可能有正当用法。

文案草案（点名「一个包一个包根」+ 给出合法形态）：

> 元素 `Backend.Foo` 的 path 有 2 条：`package` 是一个包（最小编译单元），至多一条包根
> 目录（纯容器可留空）——多条 path 属 `module`（目录型 / 多文件装配）。

### 9.3 二阶咬合已消解：模板不再产出「必挂」草稿

上一轮担心的「`kind = "package"` 草稿落盘即停摆」在「0 条合法」下**自动消失**：
`archi_scaffold草稿零结构诊断`（断言 `Ok` + `!has_path`）对 package 也成立。
模板只需改那句错误注释：

- 现状：`# kind：package = 包根（无 path）/ domain = 容器（path 恒空）/ module = …`
- 改为：`# kind：package = 包（至多一条包根目录；纯容器可留空）/ domain = 领域容器 / module = 模块 / composite = 纯分组`
- 现状 `# path：… 容器（package / domain）恒空`
- 改为：`# path：package 至多一条包根（尾 "/"）；module 必填（可多条）；domain / composite 选填`

（两行同时把 `composite` 从零承载补回四值覆盖——笔记 §1.4 修法 3 的断言仍在。）

### 9.4 连带代价（比上一轮小一档）

1. 3 个分层容器改 `composite` **不再是必做项**（0 条合法 ⇒ 不迁也不过期）。
   它降为**独立的语义归位**：`composite` 至今零实例（§8），而「纯分层容器」正是它
   的教科书用例。「不迁」的唯一代价是 `composite` 继续零承载、ADR §2/§3 的矛盾继续。
2. `repo-archi-model` 的 `6 条 noPath` **零改动**（不迁 kind 就还是那 6 个元素）。
3. 文案四处：`desktop-archi-model` L56（`path` 值域句加「`package` 至多 1 条」）、
   知识骨架「字段纪律」首条（现「模块 / 包必填」→「模块必填；包至多一条包根，纯容器可空」，
   注意 2459 B 体量锚）、`SCAFFOLD_TEMPLATE` 两行、`desktop-archi-agent` L386（补「至多一条」）。
4. 新测试（`parse/toml_test.rs`）四条：

| 用例 | 期望 |
|---|---|
| `package` 两条 path | `invalidValue`，行号指向第 2 条；文案含「至多一条包根」 |
| `package` 零条 path | **零诊断**（纯容器合法） |
| `module` 两条 path | 零诊断（`Desktop.Shell` 同形） |
| `composite` 两条 path | 零诊断（用户裁：可选，不设上界） |

外加一条跨面锚：`parse_dir` 带该诊断时 `graph_view` 的元素数为 0（停摆语义，
既有一刀切行为，用新用例钉住它不被误改成「跳过元素」）。

### 9.5 已裁决

- 落点：① parse 期 `invalidValue`（用户裁）。
- 范围：只守 package 的上界；`module ≥ 1` 与「尾 `/`」形态**不做**（别顺手扩张，
  且两条实测零误伤意味着它们不是当下痛点）。

### 9.6 新证据：契约面**早已**是单根假设（L1 的最强论据）

```
scan/owner.rs（归属面）      多条 path → 多个前缀根，各自参与最长前缀匹配
archi/api/*（契约面）        element.path.first() 是元素根，其余条目**静默不可见**

  paths.rs:11     「元素根路径（`path.first()`）只回答『根在哪』」
  paths.rs:377/485  checked_root(element.path.first())
  check.rs:746      paths::normalize_root(element.path.first()?)
  extract/mod.rs:211 「MUST NOT 索引 `path[0]`：容器元素无 `path`」——同一文件的
                      `extract` 用的却是 `path.first()`
```

⇒ 「package 至多 1 条」不是新约束，而是把契约面的 first() 假设在模型面点名。
**正交裂缝（另开线）**：`Desktop.Shell` 的 5 条 path 在契约面只有 `app.tsx` 可见，
另 4 个文件的导出不在入口闭包起点上——`api = declared` 的元素若有多条 path，
契约完备性判据会漏掉其余根。这条与 L1 无关，但同源于「path 是根还是清单」。

---

## 十、收尾：终裁与未跟进线索

### 10.1 本次定下的裁决（case A 收口）

1. **`path` 是「根」，不是「清单」**：package 至多一条（= 它的包根），module 可多条
   （= 若干根的清单）。两套语义在同一字段上的歧义，由此在**模型面**被点名（§7）。
2. **机械闭环只守上界**：`package` 的 `path` ≥2 条 ⇒ `parse/toml.rs::to_model` 出
   `invalidValue`（指向第 2 条 span）⇒ 经 `graph_view` 整图停摆。0 条合法（§9.2）。
3. **`composite` 可选 path**：可持文件、不设恒空约束 ⇒ 三个分层容器**不必**迁 kind。
4. **`noPath` 与 baseline 零改动**：不迁 kind 就还是那 6 个元素报 `noPath`，
   `repo-archi-model` 三处「恰 6 条」原样成立（§9.4）。
5. **模板无形状变更**：仅两句注释改写（`kind` 语义补 `composite` 第四值 + `path` 的
   「package 至多一条 / 模块可多条」）（§9.3）。

### 10.2 落地清单（给未来变更，本次不动代码）

| 面 | 文件 | 改动 |
|---|---|---|
| 判据 | `core/archi/src/parse/toml.rs` | `to_model` 内新增 package 的 `path` 条数校验，`invalidValue` + 第 2 条 span |
| 测试 | `core/archi/src/parse/toml_test.rs` | 四条：package 2 条拒收 / package 0 条零诊断 / module 2 条零诊断 / composite 2 条零诊断 |
| 测试 | `core/archi/src/dto_test.rs` 或 `parse/mod_test.rs` | 跨面锚：带该诊断时 `graph_view` 元素数 0（停摆语义不被误改） |
| 文案 | `openspec/specs/desktop-archi-model/spec.md` L56 | `path` 值域句加「`package` 至多 1 条（0 = 纯容器）；≥2 条即 `invalidValue` 整图停摆」 |
| 文案 | `infra/agent/src/archi-knowledge.md` | 字段纪律首条改写；**2459 B 骨架锚 + 2602 B 全文锚同批更新** |
| 文案 | `infra/agent/src/sdk/archi_tool.rs` | `SCAFFOLD_TEMPLATE` 的 `kind` / `path` 两行（补 `composite` 第四值） |
| 文案 | `openspec/specs/desktop-archi-agent/spec.md` L386 | package 语义补「至多一条包根」；测试遍历 `ElementKind::ALL` 的断言顺带覆盖 composite |
| 纪律 | `packages/desktop/package.json` | 版本 patch +1（AGENT.md 版本纪律） |

不动：`DiagnosticKind` 九值、`ArchiDriftKind` 五值、生成物、配置镜像、`INDEX.toml`。

### 10.3 未跟进的三条线索（裁决更新：L2 / L3 **已批准执行**（见 §11）；L4 暂不处理）

| # | 线索 | 状态 |
|---|---|---|
| **L2** | `composite` 归位：迁 `Backend.Core` / `Backend.Infra` / `Backend.Tools` + 合并 ADR 0001 §2 与 §3 的自相矛盾 + 前端 `element-tree` 第四枚图标首次点亮 | ✅ **已批准**，规格见 §11.2 |
| **L3** | `noPath` 对容器是误报：文案「不参与归属；依赖提取会把它视作无主」在容器上语义错误；修法需重写 `repo-archi-model` 三处「恰 6 条」baseline 契约 | ✅ **已批准**，规格见 §11.3（豁免边界待裁，§11.5） |
| **L4** | `module` 多 path × 契约面只认首条：`Desktop.Shell` 的 5 条 path 只有 `app.tsx` 进入口闭包；当前 `status = "absent"`（39 条 `docMissing` 之一）零损失 | ⏸ **暂不处理**（一旦有人给 `Desktop.Shell` 写契约就立刻暴露） |

### 10.4 与 case B 的关系

本次收尾只收 case A。case B（「静默陷阱」理由不成立）结论与修法**逐字不变**（§二），
仍未落地——两条共享同一个元纪律：**规格里凡以「静默 / 不可判定 / 工具拦不住」为由
留下的句子，MUST 附带钉住该断言的**行为**测试**（§三）。case A 这次的教训正好是它的
正面样本：`path` 的「暗假设」并非不可判定，只是没人去 `path.first()` 三处看一眼。

---

## 十一、L2 / L3 实施规格（用户已批：L2、L3 执行；L4 暂不处理）

> 本节是**交接规格**（探索模式不写应用代码）。逐条给出「文件 → 原文 → 新文」，
> 可直接转成变更的 tasks。

### 11.1 关键发现：baseline 早就写了第五种说法，而且它是对的

`repo-archi-model` L31 逐字写着：

> `path`：模块 / 包必填，`domain` / `composite` 与**仅作分组的包**选填（缺省空，
> 本基线六个容器全部留空）

「**仅作分组的包**」这个别扭的复合概念就是 L2 要消除的东西：它把「包（有根）」
和「分组（无根）」塞进同一个 kind，于是每个读者都得靠上下文猜这是哪一种。
L2 把两者拆开（package / composite），这句话就能直说成「包至多一条包根」。

至此 case A 的「四方不一致」实为**五方**，且第 5 方（baseline）站在正确一侧：

```
desktop-archi-model L56      path 缺省空，不强制          ← 正确（但没上界）
repo-archi-model L31         模块/包必填 + 仅作分组的包选填  ← 正确（且已区分「分组」）
desktop-archi-agent L386     包根与持有文件的包可带 path    ← 语义正确、缺上界
archi-knowledge.md           模块 / 包必填                 ← 错（忽视分组）
SCAFFOLD_TEMPLATE            包根无 path / 容器恒空         ← 错（把包根定义为无根）
```

### 11.2 L2：三个分层容器迁 `composite`

#### 变更面（模型，3 行）

```toml
# 02-backend-core.toml
[element."Backend.Core"]
-kind = "package"
+kind = "composite"
# 03-backend-infra.toml
[element."Backend.Infra"]
-kind = "package"
+kind = "composite"
# 05-backend-tools.toml
[element."Backend.Tools"]
-kind = "package"
+kind = "composite"
```

零副作用核验（已逐项查证）：

| 面 | 影响 |
|---|---|
| `orphanHierarchy` / `dependsOnAncestor` / `unknownReference` / `cycle` | 零——全部只看 FQN 前缀，不读 kind |
| `scan/owner.rs` 归属 | 零——这三元素本就 `path` 空、不入 `OwnerIndex` |
| `api` / `entry` / `doc` 判据 | 零——三者皆 `path` 空 ⇒ `contract_path` 不派生；`api = declared` 的 `docMissing` 计数不变 |
| 元素数 / 关系数 | 零变化（55 / 129） |
| 前端 | 零——`kind` 只影响 `element-tree.tsx` 的图标，`composite → LayersIcon` 已就位 |
| 测试 | 零——`dto_test` / `vocabulary_test` 只读 `ElementKind::ALL`；`骨架含kind四值语义` 只断言「组合」二字 |

#### 文案面（5 处）

**(1) ADR 0001 §2 末段** —— 消除自相矛盾的源头：

> 原文：「crate（…）与两个包根…`kind = "package"`；`core` / `infra` / `tools` 分层容器
> **同为 `package`**（其子元素是包）；…`composite` 是聚合类型，**本仓当前无实例**——」
>
> 新文：「crate（…）与两个包根…`kind = "package"`；`core` / `infra` / `tools` 三个
> **纯分组容器 `kind = "composite"`**（无层级、不持文件，只把同级包分类）；…」

**(2) ADR 0001 §3 首句** —— 把「六容器」拆成两组：

> 原文：「`domain` / `composite` 容器（`Backend.Core` / `Backend.Infra` / `Backend.Tools` /
> `Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard`）的 `path` 是选填字段」
>
> 新文：「六个容器的 `path` 都是选填字段，但两组语义不同：**`composite` 纯分组**
> （`Backend.Core` / `Backend.Infra` / `Backend.Tools`，无层级、典型形态就是空 `path`）
> 与 **`domain` 业务聚合**（`Desktop.Views` / `Desktop.Components` /
> `Desktop.Components.Standard`，可持文件）」

**(3) `repo-archi-model` L31** —— 去掉「仅作分组的包」：

> 原文：「`path`：模块 / 包必填，`domain` / `composite` 与**仅作分组的包**选填（缺省空，
> 本基线六个容器全部留空）」
>
> 新文：「`path`：**模块必填**（可多条）；**包至多一条包根**（0 条 = 纯容器，缺省空）；
> `domain` / `composite` 选填（缺省空，本基线六个容器全部留空）」

**(4) `archi-knowledge.md` 字段纪律首条**：

> 原文：「`path`：模块 / 包必填，`domain` / `composite` 选填（缺省空），repo 相对…」
>
> 新文：「`path`：模块必填（可多条）；包至多一条包根（纯容器可空）；`domain` /
> `composite` 选填，repo 相对…」

（体量核算：+约 18 B ⇒ 骨架 2459 → ~2477 B、全文 2602 → ~2620 B，均在目标区间内，
但**两个冻结锚必须同批实测更新**，不是「无需改」。）

**(5) `archi-knowledge.md` 元素类型段** + **`desktop-archi-agent` L386**：
`composite` 补一句典型用例——「纯分层容器（`Backend.Core` 一类）是它的标准形态」，
让 `composite` 从「零实例的理论值」变成有教科书的实值。

**(6) `SCAFFOLD_TEMPLATE` 的 `kind` 行**（与 §9.3 合并为一次改写）：
四值齐列并给 composite 一句语义——这是 `archi_tool_test` 补「kind 四值覆盖」断言的落点。

#### 附带产出

`composite` 首次获得实例（0 → 3），ADR 0001 的 §2/§3 矛盾就此消失；
`element-tree.tsx` 的第四枚图标（`LayersIcon`）首次真正被用到。

### 11.3 L3：`noPath` 对容器是误报

#### 缺陷本体

`drift/mod.rs` L113-127 对**所有**空 `path` 元素出 `noPath`，文案：

> 元素 X 未声明 path（**不参与归属；依赖提取会把它视作无主**）

后半句对容器是**错的**：容器本就不该认领文件，「视作无主」把「设计如此」
说成了「可能出错」。而 baseline 把这个噪音钉成了契约（三处「恰 6 条 `noPath`」）。

#### 豁免边界（**已裁：方案 B**）

| 方案 | 判据 | `noPath` 剩余 | 结论 |
|---|---|---|---|
| A | 只有 `module` 空 path 报 | 0 条 | 未采纳（package 忘写包根会静默） |
| **B ✅** | `domain` / `composite` 豁免；`package` / `module` 仍报 | **0 条**（L2 落地后） | **用户裁决** |
| C | 只改文案，不豁免 | 6 条 | 未采纳（放弃实质改动） |

B 把 `noPath` 的语义**升级**成一个更具信息量的问句：

```
元素空 path
   ├─ kind = domain / composite  → 静默（选填容器，空 path 是标准形态）
   ├─ kind = module              → 告警（模块必填）
   └─ kind = package             → 告警（包应有包根；真要纯分组请改用 composite）
```

最后一条不是「缺陷」而是「**kind 可能用错了**」的提示——它与 L1 的「package 0 条合法
（不停摆）」不矛盾：parse 期放行，drift 期提醒。这正是 L2 存在的收益：
**「纯分组的包」这个别扭形态有了正经去处（composite）**，`noPath` 才好心地把人往那儿指。

#### 代码改动（`drift/mod.rs`）

```rust
if element.path.is_empty() {
    // domain / composite 是选填容器，空 path 是标准形态；package / module 有
    // 必填期望（包应有包根、模块必填）⇒ 空 path 值得提醒（package 的真·纯分组
    // 形态已由 composite 承担）
    if matches!(element.kind, ElementKind::Package | ElementKind::Module) {
        model_warnings.push(ArchiDriftEntry { kind: ArchiDriftKind::NoPath, .. });
    }
    continue;
}
```

面不变：`ArchiDriftKind` 仍五值、`model_warnings.push(` 仍两处（`drift/mod_test.rs`
的源文本计数断言不破）。需 `use crate::dto::ElementKind;`（`ArchiElement.kind` 已是
该类型，零转换）。

#### 文案（`drift/report.rs` 的变体注释）

> 原文：`/// 元素未声明 `path`（不参与归属）`
>
> 新文：`/// `package` / `module` 未声明 `path`（不参与归属；容器空 `path` 是合法形态）`

**生成物连带（易漏）**：该 doc comment 经 specta 导出到
`src/types/generated/bindings.ts` L998 与 `bindings.zod.ts`，改完 MUST 重跑
`bindings:export` 并核 `git diff`——否则 `api:check` 的零 diff 门失败。

#### baseline 契约改写（3 处，`repo-archi-model`）

| 行 | 原文 | 新文 |
|---|---|---|
| L258 | 「`model.warnings` SHALL 恰为 **6 条 `noPath`**（6 个容器当前未填 `path`…）」 | 「`model.warnings` SHALL 为空（六个容器 = 3 `composite` + 3 `domain`，空 `path` 是合法形态 ⇒ 零 `noPath`）」 |
| L279 | 「`model.warnings` 恰 **6** 条 `noPath`（…）」 | 同上 |
| L432 | 「`model.warnings` = 6 条 `noPath`（纯容器面）」 | 「`model.warnings` = 0 条」 |

#### 文案面（`desktop-archi-drift` L39）

> 原文：「元素未声明 `path` 时 SHALL 记 `noPath` warning 且不参与归属」
>
> 新文：「`package` / `module` 未声明 `path` 时 SHALL 记 `noPath` warning 且不参与归属
> （包应有包根、模块必填）；`domain` / `composite` 是选填容器，空 `path` 是标准形态，
> MUST NOT 产 `noPath`。`package` 空 `path` 的告警语义是「kind 可能用错了」——真·纯分组
> 应声明 `composite`」

#### 测试（`drift/mod_test.rs`）

| 用例 | 期望 |
|---|---|
| 现有 `noPath警告`（Ghost 是 `module`） | **不变**（module 仍报） |
| 新增 `composite空path零警告` | `model.warnings` 空 |
| 新增 `domain空path零警告` | `model.warnings` 空 |
| 新增 `package空path仍报` | 一条 `noPath`（且 `errors` 空——不属定级面） |

### 11.4 L2 与 L3 的执行顺序（**强耦合，必须同批**）

```
先 L2（迁 3 个容器 package → composite）
     ↓
再 L3（豁免 domain / composite 的 noPath）
     ↓
同批改 baseline 三处计数（6 → 0）+ ADR §2/§3 + 知识骨架 + 模板 + drift spec 文案
```

方案 B 下顺序与批次都是硬的：若先做 L3 而 kind 未迁，3 个 package 容器仍报
⇒ baseline 只能降到 **3 条**；等第二个变更迁完 kind 又要从 3 改到 0
——**baseline 契约改两次**。故 L2 + L3 MUST 落在同一个变更内。

反过来说，如果将来只想做 L2（不碰 noPath），baseline 的 6 条**不用改**
（迁移只换 kind，package 仍属告警组）——L2 单独交付是安全的，只是没收益到 0。
真正的耦合只发生在「L3 已做而 L2 未做」这一个方向。

### 11.5 已裁

- `noPath` 豁免边界：**方案 B**（`domain` / `composite` 豁免，`package` / `module` 仍报）。
- L2 + L3 **同批执行**（§11.4）。
- L4 暂不处理。

### 11.6 版本纪律与交付面

- `packages/desktop/package.json` version patch +1；`src-tauri/Cargo.toml` 的 crate
  版本行 MUST NOT 跟随（AGENT.md）。
- 本变更是**热修类小变更**：不动 `DiagnosticKind` 九值、`ArchiDriftKind` 五值、
  配置镜像、`INDEX.toml`。
- **生成物必动一处**：`drift/report.rs` 的 `NoPath` doc comment 改写经 specta 落入
  `bindings.ts` / `bindings.zod.ts`（§11.3），改完 MUST 重跑 `bindings:export`
  并核 `git diff`，否则零 diff 门失败。
- 交付前必跑：`archi_drift` 全量（`model.warnings` **0 条**、`status = clean`、
  `passed = true`）、`pnpm api:check` 零 diff、骨架两个体量锚（2459 / 2602 B）
  的实测更新。

### 11.7 改动总量一览（交接用）

| 类别 | 文件 | 条数 |
|---|---|---|
| 模型数据 | `02-backend-core.toml` / `03-backend-infra.toml` / `05-backend-tools.toml` | 3 行 `kind` |
| 判据代码 | `core/archi/src/drift/mod.rs` | 1 处条件 + 1 个 `use` |
| 判据代码 | `core/archi/src/parse/toml.rs`（L1 的 package 上界） | 1 处校验 |
| 注释 | `core/archi/src/drift/report.rs`（`NoPath` 变体 doc） | 1 行 |
| 生成物 | `src/types/generated/bindings.ts` + `.zod.ts` | 重导（勿手改） |
| 测试 | `parse/toml_test.rs`（4 条）+ `drift/mod_test.rs`（3 条） | 7 条 |
| 测试 | `sdk/archi_tool_test.rs`（kind 四值覆盖） | 1 条 |
| 文案 | `archi-knowledge.md`（字段纪律 + 元素类型）、`SCAFFOLD_TEMPLATE`（2 行） | 4 处 |
| 文案 | ADR `0001-model-granularity.md` §2 / §3 | 2 处 |
| 契约 | `repo-archi-model` L31 / L258 / L279 / L432 | 4 处 |
| 契约 | `desktop-archi-drift` L39、`desktop-archi-model` L56、`desktop-archi-agent` L386 | 3 处 |
| 版本 | `packages/desktop/package.json` | +1 patch |

**零行为的确认**：元素 55 / 关系 129 不变、`status = clean`、`passed = true`、
`scan.*` 三项计数不变、四类结构诊断零命中、`apiFindings` 的 39 条 `docMissing` 不变。
本次唯一「变」的读数是 `model.warnings`：6 → 0。

---

## 十二、启动决定（用户裁：**一个变更含 L1 + L2 + L3**）

### 12.1 四处硬前提已实测通过

开单前必须核的四件事，全部通过——它们是「这个变更会不会把仓库自己搞红」的判据：

| 硬前提 | 核查方式 | 结果 |
|---|---|---|
| L1 新判据不砸既有语料 | 扫 `core/archi/tests/corpus/**/*.toml` 全部 `package` 元素 | **23 个，零个 ≥2 条 path**；`array-values` 的双 path 属 `domain` ✅ |
| L3 不砸漂移金样 | 解析 19 份 `expected/*.full.json` 的 `model.warnings` | 仅 `api-mismatch` / `target-unowned` 两份有 `noPath`，其 `App` 是 **package** ⇒ 方案 B 仍报 ⇒ **金样零变化** ✅ |
| L2 不砸契约面 | `INDEX.toml` 49 条 entry = 55 元素 − 6 容器；`api/*.rs` 全线 `path.is_empty()` 收面 | 三个容器本就退出契约面 ⇒ **`api:export` 零 diff** ✅ |
| 无代码固化容器数 | `grep 六个容器 packages/desktop` | 零命中（只在 spec / ADR 文本里） ✅ |

`INDEX.toml` 不含 `kind` 字段（仅 `fqn` / `status` / `types` / `ops` / `degraded` /
`schema` / `generated`），故 kind 迁移对索引零影响——这是最容易被误判成「要重导」的地方。

### 12.2 变更名与形态

**`archi-kind-path-discipline`**（`openspec/changes/archi-kind-path-discipline/`）

一句话：**把「kind 与 path 的搭配」从人工纪律变成机械判据**。

```
L1  package 至多一条包根（≥2 停摆）      → kind 有了 path 的基数上界
L2  纯分组容器归 composite（迁 3 个）     → 「无根的包」这个别扭形态有了专名
L3  noPath 只说真问题（domain/composite 豁免） → 告警语义从「噪音」升级为「kind 可能用错」
```

三块的共同点：**同一个事实（元素是否持有文件）此前散在五处文本里各说一套，
现在被收成一张判据表**。

### 12.3 交付形态

| 文件 | 内容 |
|---|---|
| `explore.md` | 从本篇笔记搬入（§七 / §八 / §九 / §十一 为主体） |
| `proposal.md` | 三块裁决 + 五方不一致表 + 硬前提四条 + 不含项（L4 / case B 静默陷阱） |
| `design.md` | 「kind × path」判据表、落点三选与选 ① 的理由、L2↔L3 批次耦合、文案逐处改写 |
| `tasks.md` | 见 §12.4 |
| `specs/desktop-archi-model/spec.md` | L56 值域句 + 新 Scenario（package 2 条 ⇒ invalidValue） |
| `specs/desktop-archi-drift/spec.md` | L39 的 `noPath` 判据改写 |
| `specs/desktop-archi-agent/spec.md` | L386 kind 语义补「至多一条包根」 |
| `specs/repo-archi-model/spec.md` | L31 / L258 / L279 / L432 四处（baseline 契约） |

ADR `0001-model-granularity.md` 的 §2 / §3 改写不属 spec delta（ADR 是裁定记录，
随变更同批改，不进 `specs/`）。

### 12.4 任务分解（按依赖序，可执行粒度）

**阶段一：判据与代码（不动模型数据）**

1. `parse/toml.rs::to_model` 加 package 的 `path` 上界校验——≥2 条出 `invalidValue`，
   诊断指向第 2 条 `Spanned` 位置；文案点名「一个包至多一条包根」。
2. `parse/toml_test.rs` 加 4 条：package 2 条拒收（带行号）/ package 0 条零诊断 /
   module 2 条零诊断 / composite 2 条零诊断。
3. 跨面锚：带该诊断时 `graph_view` 元素数 0（钉住停摆语义，防被改成「跳过元素」）。

**阶段二：kind 迁移（L2 的数据面）**

4. `02-backend-core.toml` / `03-backend-infra.toml` / `05-backend-tools.toml` 各改
   1 行 `kind = "composite"`。
5. 跑 `archi_module` 确认零结构诊断、元素 55 / 关系 129 不变。

**阶段三：noPath 豁免（L3 的代码面）**

6. `drift/mod.rs` 的 `path.is_empty()` 分支加 kind 判据（`Package | Module` 才报），
   补 `use crate::dto::ElementKind;`。
7. `drift/report.rs` 的 `NoPath` 变体 doc comment 改写。
8. `drift/mod_test.rs` 加 3 条：`composite` / `domain` 空 path 零警告、`package` 空 path 仍报。
9. 重跑 `bindings:export`，核 `git diff`（doc comment 经 specta 落到 `bindings.ts`）。

**阶段四：文案面（五处说法收敛）**

10. `archi-knowledge.md` 字段纪律首条 + 元素类型段（composite 补典型用例）；
    同批实测更新两个体量锚（2459 / 2602 B）。
11. `SCAFFOLD_TEMPLATE` 的 `kind` / `path` 两行（kind 四值齐列）。
12. `sdk/archi_tool_test.rs` 加「模板 kind 覆盖 `ElementKind::ALL` 四值」断言。
13. ADR `0001-model-granularity.md` §2 / §3 改写（消除自相矛盾）。

**阶段五：契约面与收口**

14. 四处 spec delta 落笔（§12.3 表）。
15. `packages/desktop/package.json` version patch +1。
16. 全量验收：`archi_drift` 全量 → `model.warnings` **0 条**、`status = clean`、
    `passed = true`；`pnpm api:check` 零 diff；`vp check` / `vp test` 绿。

**验收判据（机械可判）**

```
元素 55 / 关系 129 / 诊断 0 / status clean / passed true      ← 全部不变
model.warnings  6 → 0                                          ← 唯一变化
package 元素数  22 → 19；composite 元素数 0 → 3                 ← 唯一分布变化
api:check 零 diff；bindings diff 仅 NoPath doc comment 一行
```

### 12.5 尚未纳入的两块（明确「不含」）

- **L4**（`module` 多 path × 契约面只认 `path.first()`）：暂不处理；当前由
  `Desktop.Shell` 的 `status = "absent"` 兜住，零损失。
- **case B**（「静默陷阱」理由不成立，§二）：与本变更无关，修法逐字不变，仍需独立变更。
