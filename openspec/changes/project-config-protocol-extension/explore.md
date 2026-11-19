# 切片 1：project 协议扩展（language / depends_on）+ 拓扑序安装

workflow_type: requirement

> 出处：`openspec/todo.md` 待办原文「扩充project配置协议，可以指定编码语言，依赖project（仅列出project path即可），配置页面支持Agent检测生成配置。（未来基于project划分架构模块、编码/测试按语言使用不同prompt、编码/测试并行）」
> 本文只承载**切片 1：协议字段 + 拓扑序安装**。切片 2 是另一条独立议题，见 `openspec/explores/config-agent-session.md`——本文不涉及它的任何实现细节。

---

## 0. 切片边界

已裁决的排期：

| 切片 | 内容 | 状态 |
|---|---|---|
| 1（本文） | `projects[].language` + `projects[].depends_on` 协议字段；依赖引导改拓扑序安装 | 排期在即 |
| 2（另篇） | 见 `openspec/explores/config-agent-session.md` | 已设计，可独立开工 |
| 3（未排期） | `language_key()` 归一单点 + 按语言 prompt 附录；架构模块划分边界裁决；影响面传播；编码/测试并行 | 路标 |

切片 1 的核心价值：**两个新字段一出生就有真实消费者**（拓扑序安装），不重演 `context` / `rules` 的「已声明、零消费」结局。

**文档纪律**：本文只写切片 1 可执行的内容。切片 2 的任何实现细节 MUST NOT 出现在本文——需要时只给上方文档指针，避免后续按本文施工的 agent 误做切片 2 的工作。

---

## 1. 现状地图：字段 → 消费点

```
<root>/.dev-team/config.json            （foundation::layout::config_path, layout/mod.rs:94）
  │  config::load(root) -> { config, diagnostics }     core/config/src/lib.rs:912
  │  KNOWN_KEYS = 8 项                                  lib.rs:30
  ├─ $schema / schema ───────────▶ 纯展示（schema 恒 spec-driven 字面量）
  ├─ context / rules ────────────▶ ∅ 零消费 ★ 纯展示字段（“已声明未接通”前车）
  ├─ static_analysis ────────────▶ checks::static_check   单条命令，cwd = root
  │                                  infra/checks/src/static_check.rs:72
  ├─ projects[].root ────────────▶ write/create bootstrap 逐项目 install（声明序）
  │                                  commands/changes/mod.rs:302 / explores/mod.rs:329
  │                                  core/workflow/src/write/create.rs:355 bootstrap_project
  ├─ tests[] ────────────────────▶ testexec / mutation / aggregate
  │                                  infra/checks/src/testexec/mod.rs:131、mutation/mod.rs:194
  └─ write_protection.files[] ───▶ agent protect 规则（+ 内置锁 config.json 本身）
                                     infra/agent/src/sdk/protect/rules.rs:77
```

三个必须记住的事实：

1. **项目元素丢弃是整条剔除**：`assemble_project` 在非对象 / `root` 非法时整条剔除 + `InvalidValue`；合法元素的**未知子键被静默忽略**（`lib.rs` 约 700–760，注释「对齐 suite 元素处置」）。新增字段若不改这里，就是「配了等于没配」且零诊断——最恶心的失败模式。
2. **`tests[].root` 的实际基准是 worktree 根，不是 project root**（`detect.rs:257` `root.join(&suite.root)`），而 `config.schema.json` 描述写的是「相对 projectRoot」——文案与实现已不一致；`projects[].root` 校验很严（禁绝对 / `..` / `:` / 通配符），`tests[].root` 只禁通配符。
3. **`language` 的一半真实信息已被 `tests[].framework` 编码**：`mutation_tool(framework)` 已按框架分派（rust → cargo-mutants、jest/vitest/vite-plus → stryker、其余 None，`mutation/mod.rs:512`）。两者是同一事实的两种表达，但本次裁决**不做交叉校验**（软规则）。

---

## 2. 协议形状（已裁决：软规则）

### 2.1 `language`：开放字符串、可选、零校验

```jsonc
{ "root": "packages/desktop/src-tauri", "language": "rust" }
```

裁决依据：project 数量少、可人工看护，先纯文本无检查。

- 类型 `Option<String>`；缺失 / 显式 null → `None` **零诊断**（对齐 `projects` 整体的静默风格）；非字符串 → `None` + `InvalidValue`（走现成 `opt_string`）。
- **不做**：白名单、大小写归一、trim、`language` ↔ `tests[].framework` 交叉校验。`language: rust` 配 `framework: jest` 不会被拦。
- 可选而非必填：当前仓库自己的 config 就没有 language，必填会立刻制造一屏诊断噪音。
- **后果要接住**：将来「按语言分派 prompt」必须有**归一单点**（建议 `language_key(&str) -> String`，落 `core/config` 导出、消费侧唯一入口）。协议不归一、消费侧归一——这是软规则协议唯一能长期保持可用的姿态；否则 `TypeScript` / `ts` / `typescript` 三分，prompt 分派必然漏。

现有框架八值已经隐含切出四个语言族（仅作背景，本次不引入枚举）：

| 语言族 | 现有 `TestFramework` 值 | 突变工具 |
|---|---|---|
| TS / JS | `jest` `vitest` `vite-plus` `bun` `node-test` | stryker |
| Rust | `rust` | cargo-mutants |
| Go | `go` | ∅ |
| Python | `pytest` | ∅ |

### 2.2 `depends_on`：字符串数组、零引用检查

```jsonc
{
  "projects": [
    { "root": "packages/desktop",           "language": "typescript" },
    { "root": "packages/desktop/src-tauri", "language": "rust",
      "depends_on": ["packages/desktop"] }
  ]
}
```

- 类型 `Option<Vec<String>>`；缺失 / 显式 null → `None` 零诊断；非数组 / 混入非字符串 → `None` + `InvalidValue`（走现成 `opt_string_list`，与 `rules.proposal` 同一条路径）。
- 值**约定**等于某个 `projects[].root`，但**不检查**。四类违例全部不判：

| 违例 | 处置 |
|---|---|
| 悬空引用（指向未声明的 root） | 不判定，消费者跳过 |
| 自引用 | 不判定，消费者按 visited 集合天然免疫 |
| 成环 A→B→A | 不判定，消费者按声明序落定 |
| 重复项 / 顺序 | 不判定，原样保留（要语义的消费者自己 Set 去重） |

责任转移是这条裁决的关键：**校验面不做，消费面必须对脏数据天然免疫**。好消息是唯一消费者（拓扑序安装）用 DFS + `visited` 自动具备该性质，**一行校验代码都不用写**。

- `assemble` 仍是**单遍、无跨元素视野**，自洽不变量（`to_value(assemble(x))` 再过一遍诊断必空）**不受影响**——不引入第二遍，这是软规则路线的附带收益。
- schema 镜像的 `description` 可写「值宜为某个 `projects[].root`」——**只是提示文案，不是断言**。

### 2.3 模块层级**不并入** `depends_on`（建议，已有强依据）

`depends_on` 是**平级边**（A 依赖 B）；架构模块的父子层级在现有实现里**纯由 FQN 最长前缀推导**：

- `OwnerIndex::build(&graph.elements)` 做最长前缀归属（`core/archi/src/scan/owner.rs`，树投影与漂移判定共用同一份）；
- 模型声明纪律：「父子层级由 FQN 最长前缀推导；子元素必须先声明祖先」（`.dev-team/architecture/graph/model/`）。

若 `depends_on` 也充当层级来源，就等于给层级开第二个真相源，让「最长前缀」与「depends_on」可能互相矛盾。分工：

```
depends_on  ──▶ 平级边：安装顺序 + 将来影响面传播（同一张图，两种用法）
FQN / path  ──▶ 包含关系：模块层级（唯一真相源，不动）
```

将来做影响面传播时需要的「改动文件 → 所属 project」= 路径前缀归属，**正是 `OwnerIndex` 已经在做的事**，只是归属目标从元素 FQN 换成 project root——真实可行，不必另造映射。

### 2.4 活消费点：拓扑序安装

`depends_on` 的唯一真实消费者是依赖引导。现状是**纯声明序**：

```rust
// core/workflow/src/write/create.rs:320-326（现状）
if project_roots.is_empty() {
    bootstrap_worktree_root(worktree, vcs, warnings);
}
for root in project_roots {
    bootstrap_project(worktree, root, vcs, warnings);
}
```

改成后序 DFS（依赖先装），语义等价于「声明序 = 拓扑序」的退化情形——所以既有测试的断言结构不用改。

诚实提示：**`language` 在切片 1 里基本是声明型字段**（真实工程消费者在切片 3：按语言 prompt / 按语言静态检查）。区别在于它有明确的已规划消费者。接受「先立协议、后接消费」，还是把切片 3 的语言 prompt 附录提前合并——这是排期判断，已裁决接受前者。

### 2.5 契约纪律（机械闭环，不能忘）

- `KNOWN_KEYS` **不变**（`projects` 已在列）；`ProjectConfig` 结构变 → `bindings:export` 后 `src/types/generated/*` 必须刷新（前端 `basic-config-section.tsx` 的 `ProjectsGroup` 当前只渲染 `project.root`）。
- `config.schema.json` 是**手工镜像**，两向一致性断言在 `lib_test.rs`（键集 + framework 枚举）。软规则裁决后 `language` 只是 `type: string`、`depends_on` 只是 `array<string>` ⇒ **两条既有断言不用动**；只需在 `projects.items.properties` 补两个字段 + description。这是软规则路线的另一项附带收益：机械闭环零新增。
- 版本号纪律：`packages/desktop/package.json` patch +1；`src-tauri/tauri.conf.json` 跟随、`Cargo.toml` 不跟。
- `core/archi/src/api` 契约面：`config` / `workflow` 两模块的 `<FQN>.md` 若声明了这两个类型，需同步 `api:export`（零 diff 门会拦）。

---

## 3. 可执行草案

### 3.1 第一件事不是写代码，是改判 spec 硬约束

| 条款 | 原文 | 冲突 |
|---|---|---|
| `desktop-change-worktree`（确定性依赖 bootstrap） | 「项目根仅承载扫描 / 安装锚点，**MUST NOT 承载依赖引导之外的语义**」 | `language` 就是「依赖引导之外」的语义；禁令在案，实现即违规 |
| 同上 | 「**按配置顺序**逐个解析 `<worktree>/<root>`」 | 拓扑序会改变顺序 |
| 同上 scenario「多项目逐个扫描安装」 | 「create **按配置顺序**执行两次安装」 | 断言写死在声明序（`create_test.rs:1441` `projects逐个扫描按项目根执行安装_多安装按序` 同源） |
| `desktop-workspace-config` | 「`projects` SHALL 为顶层数组（元素 `{ root: string }`…）」 | 元素形状要扩 |

改判建议文案（三条）：

1. 「项目根承载扫描 / 安装锚点、编码语言标注与项目间依赖声明；这三项之外 MUST NOT 承载语义」——把禁令从「只允许一件事」收窄成「不许变成通用元数据口袋」，保留原意（防 projects 沦为任意 KV）。
2. 安装顺序：「按 `depends_on` 的拓扑序（依赖先于依赖方），无依赖或同层者保持配置声明序」。
3. scenario 改写为「多项目按拓扑序安装」+ 新增「脏依赖不阻断」scenario（悬空 / 自引用 / 成环各一例，断言零 `Err`、顺序确定）。

### 3.2 数据结构与签名

```rust
// core/config/src/lib.rs
pub struct ProjectConfig {
    pub root: String,
    /// 编码语言标注（自由文本，零白名单；未设为 None）
    pub language: Option<String>,
    /// 依赖的项目根（字符串数组，零引用校验；未设为 None）
    pub depends_on: Option<Vec<String>>,
}
```

**`assemble_project` 逐字段处置**（不更新这里 = 新字段被当未知子键静默忽略）：

| 字段 | 缺失 / null | 合法 | 非法 |
|---|---|---|---|
| `root` | 整条剔除 + `InvalidValue`（现状不动） | 原样 | 整条剔除 + `InvalidValue`（现状不动） |
| `language` | `None`，零诊断 | 字符串 → `Some`（**不 trim、不归一、不校验**） | 非字符串 → `None` + `InvalidValue`（`opt_string`） |
| `depends_on` | `None`，零诊断 | 全字符串数组 → `Some`（保序、不去重、不校验值） | 非数组 / 混入非字符串 → `None` + `InvalidValue`（`opt_string_list`） |

两个字段都**不影响元素去留**——元素身份只由 `root` 决定，保住引导扫描的连续性。

**create 入参**（避免 `workflow → config` 新依赖，保持 `workflow::write` 收纯数据、与 `&[String]` 同风格）：

```rust
// core/workflow/src/write/create.rs
pub struct BootstrapProject {
    pub root: String,
    pub depends_on: Vec<String>,   // 缺省空
}
// create / promote_explore 的 project_roots: &[String] → projects: &[BootstrapProject]
```

> 替代方案：直接收 `&[config::ProjectConfig]`（先例：`core/checks/src/aggregate.rs` 已收 `config::TestSuite`）。方向安全（config 只依赖 foundation），但会为一段纯排序逻辑引入 crate 依赖；除非将来 `workflow::write` 还要读配置其它字段，否则不划算。

### 3.3 拓扑序：零校验路径的五条边界

```rust
/// 安装序 = 后序 DFS（依赖先于依赖方）；声明序作 tie-break。
/// 脏数据零校验、零 Err：找不到的依赖跳过、环按当前栈停、重复按 visited 去重。
fn install_order(projects: &[BootstrapProject]) -> Vec<&str>
```

1. **同名 root 重复**：以**首次出现**为准，后续重复项忽略（前端 `ProjectsGroup` 早有注释「重复 root 允许」，此处给语义；不新造诊断）。
2. **悬空依赖**：`depends_on` 里的串不在本项目表内 → 直接跳过（不注记、不 `Err`）。
3. **自引用 / 成环**：`visited` 含则停，结果仍是确定序列，不 `Err`。
4. **不改两条既有语义**：「每项目首匹配单安装」「失败继续下一个项目」原样保留；拓扑序只改变遍历顺序。
5. **空 `projects`**：仍走 `bootstrap_worktree_root`（worktree 根扫描），`install_order` 不介入。

### 3.4 触点清单（签名级）

| # | 文件 | 改动 |
|---|---|---|
| 1 | `crates/core/config/src/lib.rs` | `ProjectConfig` 两字段 + `assemble_project` 两字段处置 |
| 2 | `crates/core/config/config.schema.json` | `projects.items.properties` 加 `language`（`type: string`）/ `depends_on`（`array<string>`）+ description；两条一致性断言**不动** |
| 3 | `crates/core/config/src/lib_test.rs` | 新增：两字段缺失零诊断 / 合法原样 / 非字符串落 `None`+`InvalidValue` / 自洽不变量（回写后再 assemble 诊断空） |
| 4 | `crates/core/workflow/src/write/create.rs` | `BootstrapProject` + `install_order` + `bootstrap` 改遍历序；`create` / `promote_explore` 签名 |
| 5 | 同上 `create_test.rs` | `create_with_projects` 夹具扩形；既有六个场景断言不变（退化情形）；新增拓扑 / 悬空 / 成环 / 自引用四例 |
| 6 | `commands/changes/mod.rs:302`、`commands/explores/mod.rs:329` | `projects` → `Vec<BootstrapProject>` 映射（不再是 `map(p.root)`） |
| 7 | `src/types/generated/*` | `bindings:export` 刷新（`ProjectConfig` 出线变宽） |
| 8 | `src/views/config/components/basic-config-section.tsx` | `ProjectsGroup` 呈现 `language` 与 `depends_on`（当前只渲染 `root`）+ 组件测试两条 |
| 9 | `packages/desktop/package.json` | patch +1（版本纪律） |
| 10 | spec | `desktop-workspace-config` 的 `projects` 元素条款；`desktop-change-worktree` 的「MUST NOT 承载…之外的语义」+ 顺序/scenario 三处 |

### 3.5 待定小口径（给默认值，可被推翻）

- `language` 值是否 trim？建议**不 trim、原样保留**（协议零智能），归一只在消费侧 `language_key()`。
- `depends_on` 是否去重落盘？建议**不去重**（写盘保真），消费侧 Set 化。
- 拓扑序里「同层 tie-break = 声明序」是唯一确定性来源，别改成字典序（否则用户的配置顺序失去意义）。
- `BootstrapProject` 收 `depends_on: Vec<String>` 还是 `Option<Vec<String>>`？建议 `Vec`（`workflow::write` 不需要区分「未设」与「空」）。

---

## 4. 后续切片路标（不属于本文范围）

### 4.1 按语言使用不同 prompt（切片 3）

14 份 `include_str!` 静态 md（6 executor + 8 evaluator；**AGENT.md 写的「16 份（7+9）」已漂移**，`phase_table_test.rs:3367` 与 `desktop-phase-prompts` spec 均为 14）。`PhaseAgentSpec.prompt: &'static str`，`PhaseNextOutcome` 只带一个 `executor` + 一个 `evaluator`，`step_executor` 直接用 `ctx.next.executor.prompt`（`walker.rs:876`）。三种做法：

| 做法 | 代价 |
|---|---|
| (i) 语言**附录**拼在基础 prompt 后（`archi_knowledge` 式动态拼接，零改类型） | 便宜，首选 |
| (ii) 每种语言一份完整 prompt（14 × N 份 md，`&'static str` 仍成立，靠 match 选） | 线性膨胀但机械 |
| (iii) 相位内按项目展开多个实例（动 `PhaseNextOutcome` 结构） | 贵 |

前提是 §2.1 的 `language_key()` 归一单点已就位。

### 4.2 编码 / 测试并行（切片 3 或独立待办）

接缝现成：节点身份 = `(phase, attempt, step, instance_no)`（`walker.rs:435` `node_key`），`instance_no` 已支持同 step 多实例；`phase_next` 返回的 steps 是 `Vec<StepDefinition>`（**克隆**，非 `&'static`），动态展开天然可行。阻塞点：walker 步循环是串行 `await`；`EdgeTrace.tail` 是「链尾节点」单指针，并行会打破单链假设。

**建议由 todo 里「step 增加是否支持并行 + 高消耗步骤共享运行队列」那条待办承载**，本文只留数据（project 边界）。

### 4.3 基于 project 划分架构模块（切片 3）

archi 模型的元素 `path = ["./packages/desktop/src-tauri/crates/core/foundation/"]` 与 project root 天然同前缀，看起来可对接。但 `core/archi/src/drift/config.rs` 头注释明令：「MUST NOT 把 `architecture` 键塞进工作区配置」「MUST NOT 出现第三个配置文件」。project 声明属于工作区配置，archi 又要读它 → 只能走「archi 经 foundation 派生 + projects 只提供事实」的路，不能让 archi 去解析 config.json。**需要一次显式的架构边界裁决**（谁依赖谁）。

---

## 5. 顺手发现的既有漂移（相邻，改到就该一起修）

- `AGENT.md` 写「16 static markdown files（7 executor + 9 evaluator）」，实际 14 份（6 + 8）。
- `config.schema.json` 里 `tests[].root` 描述「相对 projectRoot」，实现基准是 worktree 根（`detect.rs:257`）。
- `tests[].root` 只禁通配符（允许绝对路径），与 `projects[].root` 的严格校验（禁绝对 / `..` / `:` / 通配符）口径不一致。
- `context` / `rules` 两个字段零消费——本次切片 1 加字段时值得在 spec 里给一条「新增字段须有消费点或显式标注为声明型」的纪律，防止第三次重演。
