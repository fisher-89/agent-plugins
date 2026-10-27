# 提案: archi-graph-doc-agent

> **变更**: archi-graph-doc-agent
> **日期**: 2026-10-26
> **状态**: draft
> **来源**: `openspec/changes/archi-graph-doc-agent/explore.md`（requirement 类型；模型格式 / 结构诊断分工 / 文档三源 / 画布两挂载点 / agent 注入矩阵 / 归档纪律均已成文；模块接口协议为唯一未决项）
> **范围接缝**: 代码扫描、依赖归属、漂移判定、门禁挂载、报告结构、产物标准归 `archi-check-and-gate`（另一个 explore）。本变更只产出 `ModelGraph` 与「漂移标记的**呈现契约**」，MUST NOT 产出标记本身。
> **前置依赖**: 无阻塞项。spike 环境（`toml` 1.1.6 / `toml_edit` 0.25 / `petgraph` 0.8.3 / `ignore` 0.4.33）全部已在 `src-tauri/Cargo.lock` 内 ⇒ 零新增包；`git.rs` 已有 `unmerged_files()`、`ArchiveVcsPort` 已有 `worktree_snapshot()`；「专用工具经 `toolset` 枚举注入」的先例（`create_explore_note`）已在位；change 侧执行根口径（`detail.worktree` / 归档回落主仓根）已由 `use-change-chat.ts` 钉死。

---

## 问题

### 现象一：架构知识没有落点

模块边界、依赖方向、「谁能依赖谁」目前只存在于人的记忆与提交历史里。三个可见后果：

| 消费面 | 现状 | 后果 |
|---|---|---|
| 人 | 只能从目录名推断模块边界 | 重构时「这个目录算不算一个模块」每次重新吵 |
| agent | 只有 `read` / `grep` / `glob` / `ls` | 提案阶段先 grep 猜归属，猜错则整份提案落错能力域 |
| 门禁 | 无机器可读的声明 | 「声明了但代码未实现」「实现了但没声明」两种漂移**都无法表达**（这是"目录约定"方案的致命缺陷：声明必须显式存在） |

### 现象二：模块文档与代码不同生命周期

文档散在 `docs/`、wiki、ADR 里，与它描述的代码**不共享生命周期**：代码搬走文档不跟着搬，代码删了没人发现文档过期。反过来，agent 即便读到了文档也不知道「这份文档描述哪个模块」。

### 现象三：把模型加进来之后，写路径才是难点

新增一个模型文件必然引出三个新问题：谁写、写坏了怎么办、和别人改冲突了怎么办。若不给答案，结果只有两种：模型进不了主仓（没人写），或某天有人往主仓塞了个模型文件（无人评审）。

### 症结一：格式选择会决定后面 80% 的实现量

手工 parser + 自建 span + 多文件聚合 + 全部校验自写，是 c4/DSL 类方案的固定账单（explore 实测估算 400–600 行 parser + 80 行 span + 50 行聚合）。spike 已实测 `toml` 1.1.6 + `toml_edit` 0.25 的组合：**类型化 AST 与字节区间同时可得**（`serde` + `serde_spanned`）、注释与空行在写入时可保留（`toml_edit` 编辑 AST 后渲染）、错误消息自带 `line, column` + 源码片段 + `^^^^` 指示（`kind = "pakage"` 直接列出全部合法值）、重复 key 由 map 语义天然拒绝。**结论：格式侧零自研**，自研范围收敛到图规则。

### 症结二：explore 把「结构诊断自实现」低估了一类

explore §2.4 断言「`duplicateElement` parser 免费、`orphanHierarchy` 不需要」，其前提是「层级由表头结构表达」。但 §2.2 的实测输出显示键字面量是 `"App.Auth"`（**带引号的单键**），即 `element` 是一张以 FQN 字符串为键的**扁平表**，不是真嵌套表。由此：

- 同一 FQN 重复声明仍免费（map 键唯一，含跨文件时**不**免费，见下）；
- 层级只能由 **FQN 最长前缀**推导 ⇒ **`orphanHierarchy` 不是"不需要"，而是必须自实现**（`App.Auth` 声明了而 `App` 没声明）；
- 多文件聚合（`models/*.toml`）的**跨文件重复 FQN** 也不在 parser 免费射程内（每文件各自一张 map）。

本提案据此把自实现诊断从 3 类修正为 **4 类**（`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`），并把「键为带引号的完整 FQN 字符串」写成硬约定（否则同一层级存在引号 / 未引号两种写法，产生歧义）。

### 症结三：「模型 = 源码」缺三件配套

worktree 隔离已经给了「改动先落在 change 私有目录」这个天然屏障，但缺：① 写模型不设专门工具（否则要回答"给哪个相位发写工具"）；② 模型 diff 在归档确认面必须**显眼**（否则混在几十个代码文件里被橡皮图章）；③ 模型冲突必须**不进 agent 解算**（让 LLM 替人裁决"哪个模块该依赖哪个"是把架构决策外包）。

### 症结四：同一份图被三个面消费，损耗不对等

进程内消费零序列化，`archi_module` 走 `serde_json`，IPC 走 `specta` → `bindings.ts`。两条 JSON 链的形状不可能靠"小心点写"保持同源 ⇒ 只能靠**单点构建 + 零改写投影 + 词表对拍**三条机械纪律。

## 提案

新增一个纯读 core crate 承载模型与索引，四个消费面（进程内 / agent 工具 / IPC / 画布）**全部从同一份 `ModelGraph` 裁剪**；写路径不新增工具，直接复用 `write` / `edit`，纪律落在归档（可见性 + 冲突分流）。

```
                       ┌───────────────────────────────────────────┐
                       │  crates/core/archi（纯逻辑：零 Tauri /     │
                       │  零 store / 零 spawn；读盘但不写盘）        │
                       │  model.rs → parse/toml.rs → graph.rs      │
                       │        ↓                    ↓              │
                       │     docs.rs / search.rs    dto.rs（投影）  │
                       └────┬───────────┬──────────────┬───────────┘
                            │           │              │
                  ┌─────────▼──┐  ┌─────▼──────┐  ┌────▼─────────────┐
                  │ agent 工具 │  │ IPC 命令面 │  │ 进程内结构校验    │
                  │ archi_*    │  │ archi_*    │  │ （commands 写前）  │
                  └─────────┬──┘  └─────┬──────┘  └────┬─────────────┘
                            │           │              │
                     注入矩阵（§6）  画布（§5）     gate 写回（§5）
```

### 1. crate 落位与依赖

新建 `crates/core/archi`（裸名 `archi`）。依赖：`foundation`（路径） + `serde` / `serde_json` / `specta`（DTO） + `toml` / `toml_edit`（格式与写入渲染） + `petgraph`（图算法） + `ignore`（目录遍历，自动尊重 gitignore）。零 Tauri、零 store、零 spawn、零 `workflow` 依赖（`workflow` ⇄ `archi` 互不依赖为硬红线）。

两条新增边：`desktop-app → archi`（命令面）、`agent-runtime → archi`（工具执行体复用同一投影）。后者是必需项——若工具执行体自己再拼一份 JSON，三方一致性立刻退化为「三方各写一份」，与 §8 纪律直接冲突。

`foundation::layout` 新增三个派生函数（不动 `Layout` struct）：`archi_dir(root)` → `<domain>/architecture`、`archi_models_dir(root)` → `<archi_dir>/models`、`archi_decisions_dir(root)` → `<archi_dir>/decisions`；另加一个**相对路径判定** `is_archi_rel_path(rel)`，供归档冲突分流（core/orchestration 不认识 `architecture` 字面量）。`archi_config_path` **本期不落**——它的首个消费者（扫描排除 / 豁免键集）属 `archi-check-and-gate`，按 foundation 的「第二个消费者到场才下沉」纪律留待彼时。

### 2. 模型与存储格式

`<archi_dir>/models/*.toml`，按文件名字典序聚合为一个 `Model`。写法固定为「一元素一表头、键为带引号的完整 FQN」：

```toml
[element."App"]
kind = "package"

[element."App.Auth"]
kind = "domain"
path = ["./src/auth/", "./src/auth-legacy/"]
gate = "strict"
summary = "认证与令牌签发"
docs = ["./src/auth/README.md"]
metadata = { owner = "platform" }        # 内联表，非子表

[[relation]]
from = "App.Auth"
to   = "App.Shared"
desc = "复用错误类型"
```

四条格式裁决：

| # | 裁决 | 理由 |
|---|---|---|
| 1 | 键 SHALL 为带引号的完整 FQN 单键（`[element."App.Auth"]`），MUST NOT 用未引号点分形式声明层级 | 否则 `element.App.Auth`（真嵌套）与 `element."App.Auth"`（扁平键）两种写法并存 ⇒ 同一层级两种拼法、歧义不可判 |
| 2 | 元素字段 SHALL 全为标量 / 数组 / **内联表**；`metadata` MUST NOT 用子表（`[element."X".metadata]`） | 子表与「子元素表头」同构，`App.Auth.metadata` 会被读成元素 `App.Auth.metadata` |
| 3 | `kind` 必填、封闭四值（`package` / `domain` / `module` / `component`）；`gate` 可选三值（`off` / `warn` / `strict`，缺省 `off`） | 未知值即错（`invalidValue` 列出全部合法值）；`gate` 是模型属性，其**语义消费**（违例定级 / 门禁）归 check 侧 |
| 4 | `metadata` 为字符串→字符串的自由表，未知键忽略；未知**字段**（如 `gatte`）拒绝 | 与 explore §9.3 的容错裁定一致（元数据可自由演进，模型骨架严格） |

### 3. 结构诊断与「全或无」停摆

诊断分两类，接口一致（`kind` + `message` + `file` + `line` + `column?`）：

```
parser / serde 免费（文案与 caret 不由本项目撰写）
├── syntaxError          漏引号 / 漏闭合 ]
├── missingField         缺 kind
├── invalidValue         kind / gate 非法值（错误里列出全部合法值）
├── unknownField         gatte 之类拼错的键
└── duplicateElement     同一文件内重复 key（TOML map 语义）

archi 自实现（图规则，4 类）
├── cycle                成环（SCC 输出环路径）
├── dependsOnAncestor    元素依赖自己的祖先
├── unknownReference     关系引用不存在的元素
└── orphanHierarchy      子元素声明了而祖先未声明（本提案修正 explore 的 3 类）
```

外加**跨文件重复 FQN**（`duplicateElement`，`file` / `line` 指后出现的那条声明）——两文件各自合法，聚合时才冲突。

三条策略各管一段（互不冲突）：**读取**照常加载、返回结构化 errors、绝不 panic；**渲染**停摆（全或无，见本节停摆纪律）；**门禁**判 `structuralErrors`（归 check 侧）。写入侧**不校验**——写坏模型的表现是"读取报结构错"，与"写坏 Rust 代码编译不过"同构；唯一的写前校验在命令面（见 §5 的「能力差异」表），那是「UI 编辑动作」而不是「agent 写盘拦截」。

### 4. 模块文档：只做关联，不碰正文

`archi` 返回**索引**，正文由 agent / 前端按需取（agent 已有 `read`，重造一份正文检索只会更差且烧 token）：

| 来源 | 机制 | 强度 | 强度次序 |
|---|---|---|---|
| 显式 | `docs = [...]` | 强（作者亲口指定） | 1 |
| 邻近 | 元素 `path` 覆盖目录内的 `*.md` | 中（按位置推断） | 2 |
| ADR 正文 | `decisions/*.md` 正文出现该元素 FQN | 弱（检索命中，非承诺） | 3 |

三者合并去重（同文件取最强来源），契约恒为 `{ path, title, kind, source }`——**没有摘要、没有正文、没有 schema**，将来加新形态只需加一条索引来源而不动工具契约。`docs` 指向不存在的文件 → `warnings` 的 `docNotFound`，只报不拦（文档缺失不影响代码正确性）。

文档**不进** `architecture/` 目录（跟代码走：`src/auth/README.md` 或被 `docs` 显式指向），否则文档与实现分离、更容易过期。邻近扫描的排除集本期为**内置谓词**（测试 / 构建 / VCS 目录），用户可配排除键随 `archi-check-and-gate` 落地。

### 5. 画布：一套组件、两处挂载、两态渲染

```
ArchitectureCanvas（共享组件，props = { root, mode: 'full' | 'incremental' }）
├─ 左：元素树（层级由 FQN 前缀派生；节点状态点 🟡 未兑现 / ⚪ 正常）
├─ 中：xyflow 画布（节点 = element、边 = relation；缺省只渲染聚焦元素 1-hop）
└─ 右：抽屉（元素概览 kind / path / gate / modelFile:line / docs；ADR 条目）
```

| 挂载点 | 路由 / 位置 | `root` | `mode` | 改 `gate` |
|---|---|---|---|---|
| A | `/architecture`（侧栏「架构」） | 主仓根 | `full` | ❌ 只读（提示"改档需要开一个 change"） |
| B | change 详情「架构」页签 | `detail.worktree`（归档 change 回落主仓根） | `incremental` | ✅ 写 worktree 模型，随归档合回 |

**观察根就是当前工作根**，不引入「当前选中 change」这类隐式上下文：命令签名一律带 `root`，与会话 `cwd` 同口径。

**停摆：全或无**：模型有**任何**结构错时，画布与元素树一起停摆，改显示错误面板（逐条 `kind` + `file:line` + 「修好后再看」），零图数据、零降级样式、零幽灵位置。理由：残缺图的读法本身歧义（环画成什么样？悬空边画在哪？），每种处理都要新的视觉约定 + 新的交互解释；而结构错都有精确行号，读文本比读畸形图快。

**标记：只认呈现**：画布接受一组标记并按标记渲染边样式与节点状态点；**什么算漂移不在本变更**。本期标记生产者为空（恒 `none` ⇒ 实线 / ⚪ 正常），词汇与渲染契约先立，check 侧接入时 UI 零改动。

**显式取数**：画布零 watch、零轮询（挂载 / 根切换 / 手动刷新触发）。

### 6. agent 面：两个只读工具 + 相位注入矩阵

```jsonc
// 定位：已知在看哪个模块
archi_module { "element": "App.Auth" }            // 省略 element → 全元素薄概览
// 发现：还不知道这功能归谁管
archi_search { "query": "auth", "scope": "all" }  // elements | docs | all
```

- `archi_module` 返回 `element`（含 `modelFile` + `modelLine`，来自解析器已有 span）+ `dependsOn` / `dependedBy` / `children` + `docs`。`modelFile` / `modelLine` 让 agent 直接定位到声明行再 `edit`，不必 grep 猜。
- `archi_search` 只搜 **FQN / summary / metadata 值 / 文档路径与文件名**，ASCII 大小写不敏感子串 + 驼峰分词（`AuthToken` → `auth` / `token`）；**不搜正文、不碰 `openspec/specs/`**、无相关度打分（按来源强度排序）。
- `SessionToolset` 增 `Archi { module: bool, search: bool }`（两工具封闭集上的子集表达；**没有第三个工具**，新增工具须同改词汇与语料）。
- 注入按 `(phase, role)` 定矩阵（`role` / `phase` 取自既有 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`；不可解析即 fail-closed 零 archi 工具）：

| phase | role | `archi_module` | `archi_search` |
|---|---|---|---|
| proposal | executor | ❌ | ✅ |
| dev-design | executor / evaluator | ✅ | ✅ |
| test-design | executor | ❌ | ✅ |
| implement | executor | ✅ | ❌ |
| code-review | evaluator | ✅ | ❌ |
| acceptance | evaluator | ✅ | ❌ |
| 其余（含 test-gen / test-execution / archive SpecSync / code-analyze） | — | ❌ | ❌ |

- 两个工具都是只读 ⇒ `policy` 只读面 += 两名（`default` 档亦放行）；成员校验（`tool_unavailable`）语义不变，矩阵外相位幻觉调用即拒。

### 7. 模型 = 源码：写路径复用 `write` / `edit`

```
change 的 worktree                             归档             主仓
├─ 源码 *.rs / *.ts                            ↓                ↓
└─ <archi_dir>/models/*.toml ───────▶ 人工确认（专用区块）──▶ ff_merge
   agent 用 write / edit 直接改           看模型 diff + 新图预览     权威
```

- **不给专用写工具**：模型文件与源码同权，于是"专门写模型的工具该不该发给某相位"这个问题不存在。
- **写入不校验、读取报错**：`archi_*` 读取面遇坏模型返回结构化 errors 且绝不 panic。
- **不依赖运行时审批**：隔离层是 worktree，确认发生在归档，不是写盘那一刻（`loop.rs` 本就只有 `deny`，没有"挂起等确认"）。
- **`write_protection` 不纳入**：`models/` 与 `archi config` MUST NOT 进内置锁面（规则是绝对路径 glob，worktree 内同样命中，加了 agent 就改不了模型）。真正的护栏是归档时的人工确认。
- **冷启动也是 change**：模型为空时起一个 change，dev-design 让 agent 生成骨架（新元素缺省 `gate = "off"`，符合 ratchet 逐块收紧）。

### 8. 归档纪律三件套

| # | 纪律 | 落点 |
|---|---|---|
| 1 | 归档确认面 MUST 单列「架构变更」区块（元素 / 关系增删计数 + `gate` 变更 + 查看 diff + 新图预览；模型无改动时零占位） | `desktop-change-archive`（MODIFIED 前置确认面）+ `run-control-panel.tsx` |
| 2 | worktree 模型与主仓冲突 ⇒ **不进 agent 解算**，`ResolveConflicts` 按路径分流 → `Terminal::failed`（文案含 worktree 绝对路径与手动裁决引导） | `desktop-change-archive`（MODIFIED 合入面）+ `walker.rs` |
| 3 | 归档 `Preflight` MUST 检出 **rebase 半程**并挡回（两条出路：`rebase --continue` 后重跑 / `rebase --abort` 后重跑） | `desktop-change-archive`（MODIFIED 编排链）+ `steps.rs` |

第 3 条是第 2 条的必然推论：`ResolveConflicts` 失败**保留 rebase 半程**（既有语义必须保留，MUST NOT 自动 abort），于是"用户直接重跑归档"会在 rebase 中间态被 `Commit` 步 `git add -A && git commit` 污染。`Preflight` 用既有 `worktree_snapshot().rebase_head` 检出（零新 git 能力）。代价：半程 change 持续占 active 名额（上限 5）且 worktree 不可被 Cleanup 删——前端失败面 MUST 显示 worktree 绝对路径 + 两条处置命令。

### 9. 三方一致性：同源 + 三条纪律

| # | 纪律 | 违反后果 |
|---|---|---|
| 1 | **单点构建**：`ModelGraph` 只在 `archi::graph` 建一次，三个消费面都不自己再建图 | 出现第二份图算法，环检测结果可能不一致 |
| 2 | **投影零改写**：投影只裁剪（1-hop / 字段子集），不重算、不推导、不补默认值 | 投影层变成第二事实源 |
| 3 | **词汇全量对拍**：一份含全部词汇的模型语料，断言三个消费面覆盖同一词汇集 | 新增诊断 / 标记类型后 UI 静默不显示 |

需全覆盖的词汇：`kind` ×4、`gate` ×3、结构诊断 ×9（免费 5 + 自实现 4）、文档诊断 `docNotFound`、边标记 ×4、节点标记 ×2。对拍形态照抄 `src/views/changes/flow/run-state.test.ts` 的「全词汇经投影全量出图」先例。

### 范围裁决（explore 决议的落地口径）

- **D1 格式**：TOML 嵌套表头（实为"扁平 FQN 键 + 每元素一表头"），`toml` + `toml_edit` + `petgraph`；零新增包。
- **D2 诊断分工**：免费 5 类不自研；自实现 4 类（较 explore 增 `orphanHierarchy`，见症结二）。
- **D3 跨文件重复 FQN**：`duplicateElement`，聚合层自实现（explore 未覆盖，属规划期发现）。
- **D4 文档**：三源关联、契约四字段、正文不返回；文档不进 `architecture/`。
- **D5 停摆**：全或无（画布与元素树一起停），无降级样式。
- **D6 标记**：本期只落呈现契约与词表，生产者归 `archi-check-and-gate`；**节点标记不收 `structuralError`**（结构错一律停摆，节点无从呈现——对 explore §4.3 的一处收敛，理由与 §4.2 同源）。
- **D7 工具面**：恰两个只读工具、矩阵 7 行、矩阵外 fail-closed、无专用写工具。
- **D8 写路径**：模型 = 源码；`write_protection` 不纳入；gate 写回只在命令面（change 模式）。
- **D9 归档**：三件套（单列区块 / 冲突不进解算 / Preflight 挡半程）。
- **D10 三方一致性**：单点 + 零改写 + 词表对拍三条机械纪律。
- **本期不做**：模块接口协议（explore §1.3 三候选全部留下，见 R-1）；代码扫描 / 漂移判定 / 门禁 / 报告 / 归档「相邻报告」抽屉区块（check 侧）；用户可配扫描排除（`archi_config_path` 随 check 侧落地）；`specs` ↔ element 映射与「模块文档替代 specs」迁移；跨语言依赖、符号级契约核对；画布自动刷新。
- **已知摩擦（已计入工量）**：xyflow 在 jsdom 无布局引擎，节点度量需与 `change-detail-view.test.tsx` / `change-flow-graph.test.tsx` 同款垫片（`offsetWidth` / `getBBox` / viewport transform）；`app.test.tsx` 的 invoke 序列断言会因侧栏新增入口与详情页新增取数而需要按 `filter` 口径核对。

## 能力

### 新增的能力

- **desktop-archi-model** — 模型与图：① 模型文件格式与多文件聚合（FQN 键、字段封闭、内联 metadata、元素级 `modelFile` / `modelLine`）；② 结构诊断九类分工（免费五 / 自实现四）与跨文件重复 FQN；③ `ModelGraph` 单点构建与 1-hop / 邻接投影；④ 三方一致性三条机械纪律与全词汇对拍；⑤ gate 写回纯函数（`toml_edit` 保留注释 + 自证可解析）；⑥ 模型结构 diff（归档确认面数据源）。
- **desktop-archi-module-docs** — 模块文档关联：① 三源关联与强度排序去重；② 契约恒 `{ path, title, kind, source }`、正文不返回；③ `docNotFound` 只报不拦；④ 文档不进架构域目录 + 邻近扫描排除；⑤ 四条「看护不读 specs」边界。
- **desktop-archi-canvas** — 画布：① 共享组件与两处挂载点 + 观察根口径；② 停摆态全或无（错误面板、零图数据）；③ 漂移标记呈现契约（边 4 值 / 节点 2 值，封闭词汇）；④ 全量只读 / 增量可改 `gate` 的能力差异；⑤ 显式取数（零 watch 零轮询）。
- **desktop-archi-agent** — agent 面：① `archi_module` 契约（含 `modelFile` / `modelLine`）；② `archi_search` 语义（分词、不搜正文、不碰 specs、空 query 拒绝）；③ `SessionToolset::Archi` 与 `(phase, role)` 注入矩阵（矩阵外 fail-closed）；④ 只读工具在 `policy` 全档放行 + 成员校验语义不变；⑤ 模型 = 源码（不设写工具、写不校验读报错、`write_protection` 不纳入）。

### 修改的能力

- **workspace-layout-resolution** — **MODIFIED**「resolve 函数返回 Layout 结构」：常量组扩一项架构域段名，并钉死「`Layout` 三字段不变、架构域经派生函数取得」；**ADDED**「架构域路径派生（`archi_dir` 族）」：三个派生函数 + `is_archi_rel_path` 相对路径判定，纯推导零 IO，`archi_config_path` 显式不在本期射程。
- **desktop-crate-layout** — **ADDED**「core/archi crate 落地登记」：`archi → foundation` 唯一边 + `desktop-app → archi` + `agent-runtime → archi` 两条新增边；零 Tauri / 零 store / 零 spawn；`workflow` ⇄ `archi` 互不依赖由本条正面确认；七类边界分类学不增类（架构模型读盘属既有「文件边界读面」）。
- **desktop-agent-execution** — **MODIFIED**「会话级工具面注入与 explore 笔记工具」：`toolset` 枚举增 `Archi { module, search }` 变体；注入装配由「仅按 `source`」扩为「按 `(source, phase, role)`」（explore 行为零变化、矩阵外来源恒 default）；只读面在 `policy` 扩两名；成员校验 / 硬禁用 / `RunStarted.tools` 上报语义不变。
- **desktop-phase-prompts** — **MODIFIED**「桌面端特性适配（迁移是改写非搬运）」：注入矩阵命中的 7 份角色 md 允许并 SHALL 引用其勾选的 archi 只读工具（其余 **9** 份 md 仍只引七工具，`prompts/` 共 16 份），「静态脚本外置」与「零 MCP」两条护栏不变。
- **desktop-page-routing** — **MODIFIED**「路由表与 Router 选型」：增 `/architecture` 一行与对应 scenario；**MODIFIED**「Sidebar 页面导航 NavLink 化」：页面组增「架构」一项（`data-testid="nav-architecture"`），既有 testid 与组语义不变。
- **desktop-change-archive** — **MODIFIED**「归档入口与前置确认面」：确认面增「架构变更」区块（数据源为独立取数命令，非 `ArchivePreflight` 字段）；**MODIFIED**「归档编排链与阶段幂等」：`Preflight` 增 rebase 半程检出与两条处置出路；**MODIFIED**「worktree 提交与主仓合入」：`ResolveConflicts` 增模型路径分流（模型冲突 `Terminal`、零 agent 会话、不 abort）。
- **desktop-write-protection** — **ADDED**「架构域路径不在内置锁面」：`models/` 与架构域配置 MUST NOT 纳入内置锁文件；用户显式 glob 仍生效（护栏非安全边界）；真正护栏是归档人工确认。

### 零改动能力与理由

- **desktop-app-shell**：新命令轨道 `commands/archi/` 是「轨道枚举」的又一成员——该 requirement 现有文本只列 `changes` / `exec` / `db` 三轨（`agents` / `stats` / `config` / `explores` / `watch` / `env` 历来以零 delta 追加），枚举非穷尽且非规范性；命令契约由本变更新能力承载。
- **desktop-ipc-type-bindings**：「生成物入库与一致性守卫」已覆盖任意新命令与 DTO（`bindings:check` 重导出后零 diff 属既有门禁），新增五个命令与全部 `Archi*` 类型不需要新条款。
- **desktop-file-watch**：「显式取数、无 watch 无轮询」已是既有红线，本变更的画布**遵守**它（`desktop-archi-canvas` 内重申一句），不构成 delta。
- **desktop-workspace-config**：架构域配置不是工作区配置——工作区 `config.json` 的 `KNOWN_KEYS` 七项零变化、`config.schema.json` 镜像零变化；且本变更**不产出**架构域配置读取面（见 D5 与 R-4）。
- **desktop-workspace-store / desktop-change-queries / desktop-change-state-store**：模型不落库（无新表、无新字段）；画布需要的执行根 `detail.worktree` 与会话 `cwd` 口径已在线。
- **desktop-checks-domain / desktop-test-execution / desktop-mutation-testing / desktop-release-pipeline / desktop-corpus-regression**：spawn 面、门禁面、发版面、workflow 语料面全不触碰；`archi` 自带语料与 golden 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 惯例（沿用约定，非新条款）。
- **repo-surface-scope**：四个新增 capability 均对应本变更落地的可指认实现（`crates/core/archi` / 命令轨 `commands/archi/` / `views/architecture/`），不退役任何资产、不留空壳；活跃面（产品源码 / CI / `openspec/config.json` / 根文档 / `.gitignore`）不新增指向已删资产的路径引用——纪律被遵守而非被改动。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/core/archi/Cargo.toml`（新） | crate 声明：依赖 `foundation` + serde 系 + `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`；零 Tauri / 零 store / 零 spawn |
| `crates/core/archi/src/lib.rs`（新） | 模块装配与公开出口（`model` / `parse` / `graph` / `docs` / `search` / `diff` / `edit` / `dto`） |
| `crates/core/archi/src/model.rs`（新） | 格式无关 AST：`Model` / `Element` / `Relation` / `SourceSpan` / `Diagnostic`；`derive PartialEq`（为将来 UI diff 预留）；关系级 `Option<SourceSpan>` 占位 |
| `crates/core/archi/src/parse/mod.rs`（新） | `trait SourceParser`；`parse_dir(models_dir) -> ParseOutcome`：字典序聚合、跨文件重复 FQN 检出、file→line 回填 |
| `crates/core/archi/src/parse/toml.rs`（新） | `toml` + span 反序列化（`deny_unknown_fields`）；`kind` / `gate` 封闭枚举；错误映射为结构化 `Diagnostic`（复用 parser 的 line/column 与文案） |
| `crates/core/archi/src/graph.rs`（新） | `ModelGraph` 单点构建（petgraph）；SCC 成环 / 依赖祖先 / 未定义引用 / 孤儿层级；`dependsOn` / `dependedBy` / `children` / 1-hop 投影 |
| `crates/core/archi/src/docs.rs`（新） | 三源文档关联 + 强度排序去重 + `docNotFound` 警告 + 邻近扫描排除谓词 + `decisions/*.md` FQN 命中 |
| `crates/core/archi/src/search.rs`（新） | 轻量检索（子串 + 驼峰分词；scope 三值；不读正文、不扫 `specs`） |
| `crates/core/archi/src/diff.rs`（新） | 两模型结构 diff：元素 / 关系增删计数 + `gate` 变更清单 |
| `crates/core/archi/src/edit.rs`（新） | gate 写回纯函数（`toml_edit` 解析→改 AST→渲染，保留注释空行；渲染后自证可解析） |
| `crates/core/archi/src/dto.rs`（新） | 三消费面共用投影 + 封闭词汇枚举（`ElementKind` / `GateTier` / `DiagnosticKind` / `EdgeMark` / `NodeMark`）+ specta `Type` 派生 |
| `crates/core/foundation/src/layout/mod.rs` | 增 `ARCHITECTURE_DIR_NAME` / `MODELS_DIR_NAME` / `DECISIONS_DIR_NAME` 常量与 `archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`；`Layout` struct 零变化 |
| `src-tauri/src/commands/archi/mod.rs`（新） | 五命令：`archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` / `archi_set_gate`（薄包装 + `*_inner` 微形态；`archi_set_gate` 写前校验 + 原子写回） |
| `src-tauri/src/commands/mod.rs` | `pub mod archi;` + `all_commands!` 五条 |
| `src-tauri/Cargo.toml` | workspace `members` + `[workspace.dependencies]` 增 `archi` / `toml` / `toml_edit` / `petgraph` / `ignore`（版本取锁内既有值）；壳 `[dependencies]` 增 `archi` |
| `crates/core/archi` 之上的 workspace 清单 | `crates/infra/agent/Cargo.toml` 增 `archi`（工具执行体复用投影） |
| `crates/core/agent/src/runner.rs` | `SessionToolset` 增 `Archi { module: bool, search: bool }`（serde camelCase additive；缺省仍 `None`） |
| `crates/infra/agent/src/compose.rs` | `assemble_injections` 改收 `provenance`：explore 臂逐字不变；change 臂按 `source_ref` 解析 `(phase, role)` 查矩阵；不可解析 → default |
| `crates/infra/agent/src/sdk/tools.rs` | 两名常量 + 两工具定义（schema：`element` / `query` + `scope`）；`definitions` 按 `toolset` 追加；`input_path` 对两名返回 `None`（不进路径沙箱） |
| `crates/infra/agent/src/sdk/archi_tool.rs`（新） | 工具执行体：经 `LoopTurn` 上下文取 root → `archi::dto` 投影；含 L1 单结果上限收口 |
| `crates/infra/agent/src/sdk/loop.rs` | 执行臂路由增两 archi 名（与 `EXPLORE_NOTE_TOOL` 同式，上下文缺席即 `is_error` 回灌） |
| `crates/infra/agent/src/sdk/policy.rs` | `READONLY_TOOLS` += `archi_module` / `archi_search`（三档全放行） |
| `crates/core/orchestration/src/steps.rs` | `ArchivePreflight` 前增 rebase 半程检出（`worktree_snapshot().rebase_head`），命中即 `Terminal`（文案含 worktree 绝对路径 + 两条处置命令） |
| `crates/core/orchestration/src/walker.rs` | `step_resolve_conflicts` 冲突清单按 `foundation::layout::is_archi_rel_path` 分流：命中即 `Terminal::failed`（不 abort、零 agent 会话） |
| `packages/desktop/src/routes.tsx` | `/architecture` 路由 |
| `src/components/app-sidebar.tsx` | 页面组增「架构」项（`nav-architecture`） |
| `src/views/architecture/architecture-view.tsx`（新） | 挂载点 A（全量只读） |
| `src/views/architecture/components/architecture-canvas.tsx`（新） | 共享画布（`{ root, mode }`）：xyflow 节点 / 边、1-hop 聚焦、两态切换 |
| `src/views/architecture/components/element-tree.tsx`（新） | 左元素树（层级派生 + 状态点） |
| `src/views/architecture/components/element-drawer.tsx`（新） | 右抽屉（概览 / `modelFile:line` / docs / ADR 条目，markdown 渲染复用既有 `react-markdown` 装配） |
| `src/views/architecture/components/structure-error-panel.tsx`（新） | 停摆态错误面板 |
| `src/views/architecture/lib/marks.ts`（新） | 标记 → 边样式 / 节点状态点映射（封闭词汇，未知值 → 中性样式） |
| `src/views/architecture/hooks/use-archi-graph.ts`（新） | 取数（挂载 / 根切换 / 手动刷新；零 watch 零轮询） |
| `src/views/architecture/hooks/use-archi-module.ts`（新） | 抽屉取数（`archi_module`）与 ADR 正文取数（`archi_read_doc`） |
| `src/views/changes/change-detail-view.tsx` | 详情页增「架构」页签（挂载点 B，`root = detail.worktree`、归档回落主仓根） |
| `src/views/changes/flow/run-control-panel.tsx` | `ArchiveConfirmCard` 增「架构变更」区块（独立取数 `archi_change_summary`；无改动零占位；结构错红字） |
| `src/types/generated/bindings.ts` | `bindings:export` 重导出（五命令 + `Archi*` 类型 + `SessionToolset` 新变体） |
| `packages/desktop/package.json` | `version` 0.5.32 → 0.6.0 |
| `crates/core/workflow/src/prompts/*.md`（7 份） | 按注入矩阵在 proposal / dev-design（×2）/ test-design / implement / code-review / acceptance 的角色 md 内补一句 archi 工具用法（只引工具名与用法，不带路径模板、不带占位符） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 模型语料：注释 / 多文件字典序 / 数组值 / `gate` + `docs` + `metadata` / 空模型 / 缺 `kind` / 非法 `kind` / 非法 `gate` / 拼错键 / 漏引号 / 漏 `]` / 同文件重复 key / **跨文件重复 FQN** | `crates/core/archi/tests/corpus/dsl/<case>/*.toml` + `tests/golden_test.rs` | 新增（golden 沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写） |
| 结构诊断：环（SCC 路径）/ 依赖祖先 / 未定义引用 / 孤儿层级 / clean 无环图 / 祖先合法 | `crates/core/archi/src/graph_test.rs` | 新增（直接造 AST，不经解析） |
| span 精确性：元素表头行 / `path` 每项 / `docs` 每项 / relation 的 `from` 行 | `crates/core/archi/src/parse/toml_test.rs` | 新增 |
| 文档关联：显式命中 / `docNotFound` / 邻近命中 / `__tests__` 内 md 不出现 / ADR 正文命中 / 同文件双源合并取强 / 无文档空数组 | `crates/core/archi/src/docs_test.rs` | 新增（tempdir） |
| 检索：`auth` / 驼峰 `AuthToken` / 正文命中不算 / `specs` 内不算 / `scope=elements` → docs 恒空 / 空串 `Err` | `crates/core/archi/src/search_test.rs` | 新增 |
| gate 写回：保留注释与空行 / 目标元素缺席 `Err` / 渲染后不可解析即 `Err` / 非法 `gate` 值 `Err` | `crates/core/archi/src/edit_test.rs` | 新增 |
| 三方一致性词表对拍：全词汇语料 → 进程内诊断集 / `archi_module` 串 / IPC DTO 词汇集三面相等；新增词汇未同步即红 | `crates/core/archi/tests/vocabulary_test.rs` | 新增（照抄 `run-state.test.ts` 全词汇先例的 Rust 形态） |
| diff：元素增删计数 / 关系增删计数 / `gate` 变更 / 两模型等价零差异 | `crates/core/archi/src/diff_test.rs` | 新增 |
| foundation：三个派生函数路径正确且同源 / `is_archi_rel_path` 正负样本（`openspec/architecture/...` 命中、`openspec/changes/...` 不命中、Windows 反斜杠归一） | `crates/core/foundation/src/layout/mod_test.rs` | 扩展 |
| 命令面：`archi_graph` 停摆 / 正常两态；`archi_read_doc` 越界拒绝；`archi_set_gate` 写盘生效与失败零写入；`archi_change_summary` 无模型目录 → `null` | `src-tauri/src/commands/archi/mod_test.rs` | 新增（tempdir，沿 `commands/config/mod_test.rs` 直调先例） |
| 注入矩阵：7 行逐项 + 矩阵外（test-gen / test-execution / spec_sync / code-analyze / 畸形 `source_ref`）恒 default；explore 臂逐字不变 | `crates/infra/agent/src/compose_test.rs` | 扩展 |
| 工具定义与执行：两名在 `toolset` 命中时出现在 definitions、`RunStarted.tools` 一致上报；`archi_module` 返回含 `modelFile` / `modelLine`；坏模型返回 `is_error` 不 panic | `crates/infra/agent/src/sdk/tools_test.rs` + `archi_tool_test.rs`（新） | 新增 |
| 成员校验：矩阵外相位调用 `archi_module` → `tool_unavailable` 回灌、run 不中断 | `crates/infra/agent/src/sdk/loop_test.rs` | 扩展 |
| policy：三档对两 archi 工具全放行；未知名仍拒 | `crates/infra/agent/src/sdk/policy_test.rs` | 扩展 |
| `SessionToolset` 新变体：序列化线值、旧线缺省读 `None`、非法线值 `Err` | `crates/core/agent/src/lib_test.rs` | 扩展 |
| 归档 Preflight 半程：`rebase_head` 在场 → `Terminal` 且文案含 worktree 路径与两条命令；不在场 → 放行 | `crates/core/orchestration/src/steps_test.rs` | 扩展 |
| 冲突分流：冲突清单含 `openspec/architecture/models/x.toml` → `Terminal`、零 agent 会话、`rebase --abort` 零调用；纯代码路径 → 既有解算流程 | `crates/core/orchestration/src/walker_test.rs` | 扩展 |
| 画布：停摆态零图数据 / 正常态节点边齐全 / 1-hop 聚焦 / 元素树状态点 / 抽屉 `modelFile:line` / `nav-architecture` active | `src/views/architecture/**/*.test.tsx`（新） | 新增（jsdom 度量垫片照抄 `change-flow-graph.test.tsx`） |
| 标记映射：边标记四值各自样式 / 节点标记两值各自状态点 / 词汇外值退化中性且不抛错 / 本期零标记时全边实线 + 全节点 ⚪ | `src/views/architecture/lib/marks.test.ts`（新） | 新增（纯函数直调，无渲染、无 jsdom 垫片） |
| 详情页页签：架构页签在场、`root` 取 `detail.worktree`、归档回落主仓根、全量模式 gate 只读 / 增量模式可提交 | `src/views/changes/change-detail-view.test.tsx` | 扩展 |
| 归档确认区块：模型有改动 → 区块在场（计数逐项）；无改动 → 零占位；结构错 → 红字错误行 | `src/views/changes/flow/run-control-panel.test.tsx` | 扩展（`preflight()` 构造器补 `archi` 形状） |
| prompt 文本面：7 份命中 md 各含其矩阵勾选的 archi 工具名 +「何时用 / 返回什么」表述 / 其余 9 份零 archi 字样 / 16 份零占位符与零 `__MCP:*` / 16 条 `include_str!` 字面路径与角色表逐字不变 | `crates/core/workflow/src/write/phase_table_test.rs` | 扩展（逐文件扫描，未命中文件断言零 archi 字样） |

### 不要修改

- `openspec/specs/**` 主基线（落主基线只在归档 `SpecSync` 步）。
- `crates/core/workflow/**`——两处例外且仅此两处：① §实现文件的 7 份 prompt md 允许补充其矩阵勾选的 archi 工具用法；② `src/write/phase_table_test.rs` 允许扩展 prompt 文本面断言（7 份命中 / 9 份未命中的逐文件扫描、零占位、零 `__MCP:*`）。除此之外相位表 / 写面 / 查询面零改动：`src/write/phase_table.rs` 的 16 条 `include_str!` 字面路径与角色表**逐字不变**；模型不进相位表、不进 change 状态机；`workflow` ⇄ `archi` 零依赖。
- `crates/infra/store/**`：模型不落库（无新表、无新字段、无迁移）。
- `crates/core/config/**` 与 `crates/core/config/config.schema.json`：工作区 `KNOWN_KEYS` 与 schema 镜像零变化（架构域配置不是工作区配置）。
- `crates/infra/watch/**` 与任何目录监听接入：画布显式取数。
- `crates/infra/vcs/**`：`unmerged_files()` / `worktree_snapshot()` 既有能力足够，本轮零新 git 能力。
- `src-tauri/tauri.conf.json`（版本经 `../package.json` 跟随）、`src-tauri/capabilities/default.json`（权限面零扩张）、`.github/workflows/**`。
- `src/views/changes/flow/**` 除 `run-control-panel.tsx` 的归档确认区块与 `run-control-panel.test.tsx` 的对应扩展用例；`src/components/ui/**`；`src/views/explores/**`；`src/hooks/use-agent-session.ts` 与 `agent-transport`（会话注入面在后端装配，前端零改动）。
- `openspec/changes/archi-graph-doc-agent/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 模型可解析且定位精确：多文件字典序聚合后，`archi_module` 回报的 `modelFile` / `modelLine` 指向该元素表头所在文件与行，可直接用于 `edit` | `parse/toml_test` + `archi_tool_test` 逐元素断言 |
| AC-2 | 结构诊断九类齐备：免费五类（`syntaxError` / `missingField` / `invalidValue` / `unknownField` / `duplicateElement`）保留 parser 原文案与 line/column；自实现四类（`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`）各带 `file` + `line` | 语料逐 case 对拍 + `graph_test` |
| AC-3 | 跨文件重复 FQN 被拒：两文件各声明 `App.Auth` → `duplicateElement`，`file` / `line` 指后出现者 | 语料 `dup-cross-file` case |
| AC-4 | 停摆全或无：任一结构错时 `archi_graph` 返回停摆态 + 错误清单且 `elements` / `relations` 为空；无结构错时返回完整图 | 命令面两态用例 + 画布组件用例 |
| AC-5 | 文档三源：显式排首（`source=explicit`）、邻近次之（`adjacent`）、ADR 正文再次（`text`）；同文件双源合并取强；`docs` 指向不存在文件 → `warnings.docNotFound` 且该条不出现在 `docs`；无任何关联 → `docs: []` 不告警 | `docs_test` 七条用例 |
| AC-6 | 文档正文不经关联面返回：关联结果不含正文；`archi_read_doc` 仅接受关联集内路径，越界显式 `Err` | `docs_test` + `commands/archi/mod_test` |
| AC-7 | 三方一致性：一份含全部词汇（`kind` ×4 / `gate` ×3 / 诊断 ×9 / `docNotFound` / 边标记 ×4 / 节点标记 ×2）的语料，进程内诊断集、`archi_module` 串、IPC DTO 词汇集三面相等；任一词汇新增未同步对拍即红 | `tests/vocabulary_test.rs` |
| AC-8 | 分层与依赖：`archi` 的 workspace 内依赖仅 `foundation`；`workflow` 与 `archi` 互不依赖；两 crate 均零 Tauri；`desktop-app → archi`、`agent-runtime → archi` 在案且有用途注释；`archi` 零 store / 零 spawn | `Cargo.toml` 审查 + 既有依赖方向断言复核 |
| AC-9 | 路径字面量单点：产品 `.rs`（`*_test.rs` 除）无 `architecture` 段字面量、无 `openspec` 字面量、无 `config.json` 字面量；四个派生 / 判定函数全部经 `foundation::layout` | 既有命名隔离扫描扩展通过 |
| AC-10 | `archi_module` 契约：指定元素返回 `element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine`）+ `dependsOn` / `dependedBy` / `children` / `docs`；省略 `element` 返回全元素薄概览并受既有单结果上限截断留痕 | `archi_tool_test` 两形态用例 |
| AC-11 | `archi_search` 语义：`auth` 命中 FQN 与文件名；`AuthToken` 经分词命中 `App.Auth.Token`；仅正文命中不算；`specs` 内不算；`scope=elements` 时 `docs` 恒空数组；空 `query` 显式 `Err` | `search_test` 六条用例 |
| AC-12 | 注入矩阵：7 行逐行命中（proposal=search / dev-design=both / test-design=search / implement=module / code-review=module / acceptance=module）；矩阵外相位与畸形 `source_ref` 恒零 archi 工具；explore 会话注入面逐字不变；`RunStarted.tools` 与实际 definitions 一致 | `compose_test` + `tools_test` |
| AC-13 | 硬禁用与档位：矩阵外相位调用 `archi_module` → `tool_unavailable` + `is_error` 回灌、run 不中断、不触执行体；`default` 档调用两工具均放行（只读面语义） | `loop_test` + `policy_test` |
| AC-14 | 模型 = 源码：worktree 内 `write` / `edit` 可直改 `models/*.toml`（内置锁面零命中）；写坏模型写入成功、`archi_graph` 返回结构错且不 panic；`write_protection` 的内置规则清单不含架构域路径 | 命令面 + `protect` 用例 + 源码审查 |
| AC-15 | 归档确认面：模型有改动 → 「架构变更」区块呈现（元素 ± / 关系 ± / `gate` 变更清单）+ 查看 diff + 新图预览入口；模型无改动 → 区块零占位；模型结构错 → 区块内红字列出错误且不阻断确认 | `run-control-panel.test` + 实跑一次归档停等面 |
| AC-16 | 模型冲突分流：`ResolveConflicts` 冲突清单含架构域路径 → `Terminal::failed`（文案含 worktree 绝对路径 +「解完模型冲突再重跑」引导）、零 agent 会话、零 `rebase --abort`；纯代码冲突路径行为逐字不变 | `walker_test` 两臂用例 |
| AC-17 | Preflight 挡半程：`worktree_snapshot().rebase_head` 在场 → `Terminal`，文案含 worktree 绝对路径与 `git rebase --continue` / `git rebase --abort` 两条出路；不在场 → 放行；已 continue / 已 abort 两种处置后重跑均放行 | `steps_test` + 实跑一次重跑 |
| AC-18 | 画布与挂载点：`/architecture` 全量只读（`gate` 无编辑入口、文案提示"改档需要开一个 change"）；change 详情「架构」页签以 `detail.worktree` 为根、归档 change 回落主仓根、增量模式可提交 `gate`；缺省只渲染聚焦元素 1-hop；零 watch 零轮询（挂载 / 根切换 / 手动刷新三触发）；侧栏页面组呈现「架构」入口（`data-testid="nav-architecture"`），其 active 态由当前 URL 派生（在 `/architecture` 时 active、切到其它页面后不 active） | 前端组件用例（画布 / 详情页页签 / 侧栏导航）+ 源码扫描（无 `watch` 调用） |
| AC-19 | gate 写回：`archi_set_gate` 写前校验（元素存在 / 值合法 / 渲染后可解析），成功时仅改目标行且保留注释与空行（`toml_edit` 渲染）；任一校验失败零写入 | `edit_test` + `commands/archi/mod_test` |
| AC-20 | 交付面：`bindings:check` 重导出后零 diff（五命令 + `Archi*` 类型 + `SessionToolset` 新变体在案）；`packages/desktop` version 0.6.0、`src-tauri/Cargo.toml` 版本不随动；`vp check` / `knip` / `cargo clippy -D warnings` 通过 | 机械门面 |
| AC-21 | prompt 文本面：7 份矩阵命中 md（`proposal-planner` / `dev-design-planner` / `dev-design-evaluator` / `test-design-planner` / `implementation-generator` / `code-review-evaluator` / `acceptance-evaluator`）各含其矩阵勾选的 archi 工具名（`archi_module` / `archi_search` 两名在 7 份并集内齐备，单份只提矩阵勾选者）与「何时用 / 返回什么」表述，且零路径模板、零写模型指令；其余 9 份 md 零 archi 字样；16 份 md 零占位符（`<change>` / `<phase>` / `{{ }}` / `__INCLUDE:*` / `__MODEL_*__` / `__DEV_TEAM_ROOT__`）与零 `__MCP:*` 扫描仍绿；`phase_table.rs` 的 16 条 `include_str!` 字面路径与角色表逐字不变 | `phase_table_test` 扩展（逐文件命中 / 未命中扫描 + 零占位 + 零 MCP）+ `cargo test -p workflow` |
| AC-22 | 标记映射封闭且退化安全：`marks.ts` 对**边标记四值**（`none` 实线 / `unfulfilled` 灰虚线 / `crossKind` 橙虚线 / `exempt` 灰实线）与**节点标记两值**（`none` ⚪ 正常 / `unfulfilled` 🟡 未兑现）逐一给出确定性样式，词汇外值退化为中性（与 `none` 同款）且不抛错、不空白；本期图数据恒 `none` ⇒ 全部边实线、全部节点 ⚪ 正常；该词汇与 `desktop-archi-canvas`「漂移标记呈现契约」及三方对拍语料逐值一致（边 4 / 节点 2），任一处新增未同步即红 | `src/views/architecture/lib/marks.test.ts` 全词汇 / 未知值 / 零标记三条用例 + `tests/vocabulary_test.rs` |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **模块接口协议仍未裁决**（explore §1.3）：若本变更顺手落「符号名声明」或「签名级契约」，会重蹈「契约权威在哪 / 两套提取器怎么对齐」两条否决理由 | 引入一份与 specs / ADR 必然漂移的第二契约源 | 中（有天然引力） | 本期**明令不做**：`element` 上的协议相关字段一个不加；协议只经 `docs` 引用（方案 A 的零成本形态）已由文档关联承载；B / C 需独立 change 且必须先回答两条否决理由——写进 `desktop-archi-model` 的边界条款，防后人误加字段 |
| R-2 | **诊断分类被 explore 低估一类**（`orphanHierarchy`）+ 跨文件重复 FQN | 若按 explore 的 3 类直接实现，语料会漏测这两类，模型坏着也能"通过校验" | 已发生（规划期发现） | 提案已把分类修正为 4 类 + 跨文件重复 FQN，语料显式含 `orphan` 与 `dup-cross-file` 两 case；AC-2 / AC-3 钉死 |
| R-3 | **三方一致性依赖既有 todo 项**：「移除生成类型中的 null，使用 undefined 代替」（`openspec/todo.md`）未落地 ⇒ `Option::None` 在 `archi_module`（serde_json）与 IPC（specta）两侧可能是「缺字段」与 `null` 两种形状 | agent 解析同一字段要写两种兜底；词表对拍只覆盖词汇集、不覆盖 null 形态 | 中 | 本期以「词汇集相等」为对拍口径（不含 null/undefined 形态）；`archi_module` 响应把可选字段显式序列化为 `null`（不省字段）以与 IPC 对齐；形态统一留待 todo 项落地后加一条对拍 |
| R-4 | **扫描排除配置面延后**：用户可配排除键集归 `archi-check-and-gate`，本期只有内置排除谓词 | 用户想把某目录的 md 排除出邻近关联时无入口 | 中 | 内置谓词（测试 / 构建 / VCS 目录 + gitignore）覆盖绝大多数情形；`archi_config_path` 与键集在设计侧留出接缝（`foundation` 增函数即接入，零重构）；spec 明写「本期不预建配置面」以免被当成缺口遗漏 |
| R-5 | **画布复用度高估**：change flow 画布是「分层泳道 + 步节点」，archi 图是「拓扑 + 1-hop」，布局算法不可复用 | 若按"复用既有画布"排工，实际要重写布局 | 中 | 只复用 xyflow 与 `ui/*` 组件、不复用 `flow/` 的布局与节点组件；jsdom 度量垫片照抄既有三处先例；1-hop 缺省已把布局复杂度压到"聚焦邻域" |
| R-6 | **归档确认面信息不足**（explore 未决 2）：三层（摘要 / diff / 新图预览）是否够 | 模型改动仍可能被橡皮图章 | 中 | 本期落三层 + 计数逐项；G3 等价实测（AC-15 实跑一次归档停等面）后若不足，加"哪些代码文件落在改动元素覆盖路径内"的交叉信息属纯增量（不改 DTO 契约）；该项写进 spec 的观察项 |
| R-7 | **rebase 半程的连带成本**：挡回后 change 持续占 active 名额（上限 5）、worktree 不可被 Cleanup 删，用户可能卡住 | 用户以为流程坏了 | 中 | 失败文案 MUST 含 worktree 绝对路径 + 两条可照抄命令（AC-17）；归档失败面同款呈现；`desktop-change-create` 的上限文案已引导"归档或手动清理"，此处补一条处置出路即可 |
| R-8 | **注入矩阵依赖 `source_ref` 定式的隐式契约**：矩阵按 `{change_id}/{phase}/{role}/{attempt}` 解析 | 将来 `source_ref` 定式变更 → 矩阵静默全 miss（archi 工具消失，无报错） | 低 | 解析失败 fail-closed（零 archi 工具，安全侧）+ `compose_test` 锚定该定式；若定式要改，测试先红 |
| R-9 | **模型文件进入 worktree 后的合并面**：模型是"多文件 + FQN 跨文件不重复"的结构，git 合并粒度是文件级 | 两 change 各加一个元素文件 → 无冲突但可能跨文件撞 FQN；同文件改动 → 必然文本冲突（已由 R-8/AC-16 分流交人） | 中 | 跨文件撞 FQN 属**读取期**结构错（`duplicateElement`），合入后立即在 `/architecture` 停摆态可见，不会静默；模型分文件（按域命名）是文档化建议，`05-*.toml` 划分由使用方按域拆 |
| R-10 | **单次现算的性能**：不缓存、不建倒排，大仓（元素数百 / 目录数千）每次取数都全扫 | 画布首屏或 `archi_search` 慢 | 中 | 1-hop 缺省 + 文档邻近扫描限定元素 `path` 覆盖范围 + `ignore` 跳过 `node_modules` / `target` / gitignore；若实测超阈值，缓存属纯增量（不起第二事实源：缓存键 = mtime 集合）；实测项写进 acceptance |
| R-11 | **prompt 改动面**：7 份角色 md 补 archi 工具用法，既有测试对 prompt 有逐字断言 | 文本改动触发既有断言红，被误当回归 | 高（必然发生） | 逐字断言测试的期望值随本次文本更新（落点为 `workflow/src/write/phase_table_test.rs` 的扩展，`phase_table.rs` 的 `include_str!` 路径与角色表不动），`desktop-phase-prompts` 的「全静态零占位」与「零 MCP」两条扫描必须仍通过（AC-21）；只在矩阵命中角色内提工具名，未命中 9 份 md 零改动 |
| R-12 | **`SessionToolset` 变体形状的争议**：`Archi { module, search }` 用两布尔位表达子集，被质疑"为什么不拆成三个单元变体" | 词汇层审查卡壳 | 低 | 两工具是**封闭二元集**，布尔位即子集表达；三个单元变体（`Archi` / `ArchiModule` / `ArchiSearch`）在第三个工具出现时会退化为组合爆炸；spec 明写"新增第三工具须同改词汇与语料"，把演进路径留在眼下 |
| R-13 | **`agent-runtime → archi` 新依赖边**：infra → core 已有先例，但"工具执行体直连纯读 core crate"仍可能被要求改走 port | 若被推翻，需在 core 侧加 port + 两处装配，工期增加 | 低 | 复用同一投影是三方一致性的**机械前提**（工具另写一份 JSON 则纪律失效）；理由与既有 `agent-runtime → checks`（glob 方言单点）同构，按同款"登记新增边 + 单点理由"落 spec |
| R-14 | **画布与详情的取数次数**：`app.test.tsx` 的 invoke 序列断言与详情页新增取数相互影响 | 用例改造被低估 | 中 | 画布取数挂在画布组件（非 App 根），详情页取数只在「架构」页签激活时发起；按既有先例改 `filter` 口径或前置 mock |
