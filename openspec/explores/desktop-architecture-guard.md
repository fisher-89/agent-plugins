# 基于 Rust 生态的本地 Agent 架构看护系统自研方案
## (Local Architecture Guardrails & Evolution System)

---

## 1. 方案愿景与设计哲学

在现代多工程、Monorepo 及全栈项目研发中，架构设计图、技术文档与真实源代码极易发生脱节（即“架构漂移”）。本方案旨在利用 **Rust 生态的高性能工具链**，打造一款纯本地运行的 Agent 架构看护核心引擎。

### 核心设计哲学：代码与契约即唯一真理 (Code & Contract as Single Source of Truth)
* **声明式驱动**：架构拓扑、模块划分与 API 定义完全由本地的声明式文件（Markdown / YAML）决定，对 Git 版本控制天然友好。
* **三位一体联动**：彻底打通 **图（关系）- 文档（契约）- 代码（源码 AST）** 的双向闭环。
* **刚柔并济看护**：局部利用成熟的 Rust 静态拓扑算法进行“刚性拦截”（如循环依赖、分层违规），结合 LLM 大模型的柔性理解进行“架构重构与用例补全”。

---

## 2. 全局概念实体模型 (Global Entity Model)

系统在内存中将工作区（Workspace）抽象为四层级映射网络，初版专注于 **Rust 生态专区** 与 **JS/TS 生态专区** 的独立语言内闭环，暂不引入跨语言依赖的复杂性。

```
【 架构拓扑层 (图) 】       LogicalModule (逻辑模块节点) ───[ DependsOn / Contains 边 ]
│
▼ 1 : 1 物理绑定
【 属性与契约层 (文档) 】   ModuleDocument (Markdown 文档 + 语言无关 API Schema)
│
┌─────────────────────┴─────────────────────┐
▼ 1 : N 物理映射                            ▼ 1 : N 语义映射
【 源码文件层 (代码) 】    FileMapping (磁盘路径/包边界)    ApiMapping (接口契约与源码AST)
```

### 2.1 四大核心映射定义

#### ① 图（关系拓扑 - Topology Graph）
* **底层引擎**：基于 Rust 生态最成熟的 [`petgraph::graph::DiGraph`](https://github.com "petgraph") 构建。
* **节点 (Node - LogicalModule)**：代表一个逻辑模块。包含 `id`（全局唯一标识）与 `lang_env`（所属语言环境：Rust 或 JavaScript）。
* **边 (Edge - Relation)**：
  * `Contains`：父子关系。用于表达模块的嵌套与边界划分。
  * `DependsOn`：依赖关系。用于表达模块间的水平调用与引入。

#### ② 模块文档（契约载体 - Module Document）
* **物理表现**：每个 `LogicalModule` 强绑定一个本地物理 Markdown 文档（例如存放于模块目录下的 `docs/arch.md`）。
* **元数据区 (Front-matter)**：声明模块的基本属性与绑定的代码物理边界。
* **API 契约区 (Language-Agnostic Schema)**：引入**语言无关的 YAML/JSON Schema** 声明该模块对外暴露的公共 API 契约，抹平具体编程语言的语法噪点。

#### ③ 文件映射（物理锚点 - File Mapping）
* **映射机制**：解决“逻辑模块”在不同语言生态下落地的物理边界问题，防止架构浮空。
* **Rust 生态映射**：支持包级别（映射到独立 `Cargo Package` 目录）或文件级别（映射到特定的 `mod.rs` 或源文件）。
* **JS/TS 生态映射**：支持 Monorepo 包级别（映射到 `pnpm workspace` 的子包目录）或文件夹级别（包含 `index.ts` 的物理功能文件夹）。

#### ④ API 映射（源码契约核对 - API & AST Mapping）
* **映射桥梁**：将文档层语言无关的 `Schema` 与源码中真正的符号签名（AST）进行刚性绑定。
* **Rust 提取核**：基于 [`syn`](https://github.com "syn") 与 [`cargo_metadata`](https://github.com "cargo_metadata")，在对应物理文件中核对带有特异性标记（如属性宏 `#[arch_api(id)]` 或同名函数）的 `pub fn`。
* **JS/TS 提取核**：基于 [`oxc`](https://github.com "oxc") 家族（`oxc_parser` + `oxc_resolver`），在导出的抽象语法树中，核对是否存在对应的 `export function` 且参数契约完全对齐。

---

## 3. 核心运作管线与数据流转 (Pipeline & Data Flow)

当用户通过**直接编辑文档**或通过 **Agent 交互式引导修改** 导致设计（依赖关系、模块划分、API 定义）发生变更时，系统触发以下自动化漏斗模型：

```
1. 扫描与提取 (Parse) ──> 2. 拓扑图构建 (Build) ──> 3. 刚性防漂移核对 (Lint) ──> 4. 自动化使能 (Act)
• 读文档提 YAML Schema     • 将模块装载进 petgraph     • 刚性算法: 环路/分层校验     • 差异化生成测试桩代码
• 调 cargo_metadata / oxc • 建立 Contains/DependsOn   • 源码解 AST 强比对契约     • 喂给 AI Agent 补全用例
```

### 3.1 变更场景与数据流转细节

#### 场景 A：修改“模块划分”（新增/删除/合并模块）
1. **物理变更**：用户或 Agent 在工作区新增 `billing.md`，或修改 `order.md` 中的父级目录声明。
2. **图重建与差分**：Rust 本地守护进程（基于 [`notify`](https://github.com "notify-rs")）捕捉变更，重构 `petgraph`，并与 Git 基线（基于 [`git2-rs`](https://github.com "git2-rs")）比对计算出结构化的 **Topology Diff**。
3. **脚手架下沉**：Rust 引擎检测到新模块，自动在磁盘对应位置创建 Crate 骨架或 JS/TS 物理文件夹，完成前向代码生成。

#### 场景 B：修改“依赖关系”（改变调用方向）
1. **拓扑合规检查**：引擎将新边插入 `petgraph`，立即运行环路检测算法（如 `kosaraju_scc`）。若构成循环依赖或违反分层原则，**立刻熔断流转并抛出刚性报错**。
2. **代码侵入性检查**：若拓扑合规，引擎调用 `cargo_metadata` 或 `oxc_resolver` 反向扫描真实代码。若代码中尚未写入对应的 `use` 或 `import`，在本地提交或 CI 时拦截并提示：“代码落后于设计，是否一键同步构建配置文件？”

#### 场景 C：修改“API 定义”（新增接口、变更字段）
1. **契约提取**：引擎将更新后的语言无关 YAML 契约对象转化为通用中间表示（IR）。
2. **AST 碰撞（检测漂移）**：适配器调用 `syn` 或 `oxc_parser` 抓取源码中真实函数的 AST。比对发现源码未及时修改（如字段名对不上），系统判定**“代码与契约发生漂移”**。
3. **自动化测试使能 (Test Gen)**：测试生成引擎根据新 Schema，**强行重写或更新本地对应的测试文件数据桩（Test Stubs & Mocks）**（如 Rust 的 `#[test]` 或 TS 的 `Vitest` 用例）。由于测试桩由 Schema 刚性驱动，运行本地测试时将直接报错，倒逼开发人员重构源码；同时该 Schema Diff 作为黄金 Context 喂给 AI Agent，由 AI 辅助完成业务源码的柔性重构。

---

## 4. 系统 UI 与交互设计 (UI & UX Architecture)

针对本地 Agent 工具的轻量化定位，推荐采用 **“左树右图、双屏联动”的非对称画布架构**（可通过 Tauri 构建独立 App 或嵌入 IDE 插件视图）。

```
┌────────────────────────────────────────────────────────────────────────┐
│ 🛠️ Project: My_Monorepo (Rust + JS)              [ 基线版本 🟢 | 演进设计 🟡 ]│
├──────────────┬──────────────────────────────────────────────┬──────────┤
│ 📂 模块大纲树 │ 🗺️ 架构拓扑主画布 (The Canvas)               │ 📑 详情    │
│ ├─ 🟢 auth   │                                              │    抽屉  │
│ ├─ 🟡 order  │     ┌──────────────┐     ┌──────────────┐    │ (Drawer) │
│ └─ 🟢 billing│     │  auth_service│     │ order_service│    │          │
│              │     │    (Rust)    │────>│     (TS)     │    │ 🟡 order │
│              │     └──────────────┘     └──────────────┘    │          │
│              │            │                      │          │ • 类型:TS│
│              │            ▼                      ▼          │ • 父级:..│
│              │     ┌──────────────┐     ┌──────────────┐    │          │
│              │     │  db_adapter  │<────│ billing_serv │    │ [上下游] │
│              │     └──────────────┘     └──────────────┘    │          │
└──────────────┴──────────────────────────────────────────────┴──────────┘
```

### 4.1 核心视图交互组件

* **左侧：模块大纲树 (Hierarchy Tree)**
  以树状图清晰展示模块的 **Contains（父子包含）** 关系。节点前置状态圆点（🟢 对齐、🟡 演进中/漂移、🔴 违规环路）。
* **中间：渐进式拓扑画布 (Interactive Canvas)**
  展示模块的 **DependsOn（依赖调用）** 关系。引入**无级缩放（Semantic Zooming）**机制：远处视角只显示大模块域及粗线，近处视角自动展开节点露出子模块细节。画布默认只渲染当前聚焦模块的一阶（1-hop）上下游连线，防止界面过载。
* **右侧：全屏/半屏详情抽屉 (Sliding Drawer)**
  点击任意节点滑出，提供三大核心视图：
  1. *模块概览*：直接渲染绑定的物理 Markdown 架构文档，支持一键在 IDE 中定位。
  2. *契约核对*：结构化展示通用 API Schema。发生漂移时红字高亮，并排面对面展示【文档契约】与【源码 AST】的 Diff，提供一键同步按钮。
  3. *用例管线*：展示当前模块的测试覆盖率状态，提供“一键补充缺失测试桩”的功能入口。
* **双轨对比视图（Design Review Mode）**
  演进设计时，图上呈现视觉差分：🟢 绿色节点/连线代表新增，🔴 红色虚线代表删除，🟡 黄色闪烁节点代表 API 发生变更且源码尚未对齐。

---

## 5. 核心技术选型与实验探索 (Tech Stack & Spike)

为了在快速实现产品的同时保证系统刚性，推荐局部拼装以下 Rust 生态的顶级基础库：

| 维度 / 功能领域 | 推荐 Rust 生态依赖库 | 在本自研系统中的具体核心作用 |
| :--- | :--- | :--- |
| **内存图计算与图算法** | [`petgraph`](https://github.com "petgraph") | 构建多工程拓扑图，负责环路检测（SCC）及上下游影响半径（Blast Radius）遍历。 |
| **本地文件监听与同步** | [`notify`](https://github.com "notify-rs") | 监听本地 `.md` 文件或源码变更，实现防抖的增量图重构与数据刷新。 |
| **Rust 源码解析域** | [`syn`](https://github.com "syn") / [`cargo_metadata`](https://github.com "cargo_metadata") | 提取本地 Crate 硬依赖关系；将函数签名字符串转化为 AST 节点用于碰撞。 |
| **JS/TS 源码解析域** | [`oxc_parser`](https://github.com "oxc") / `oxc_resolver` | 纯 Rust 编写的极速 JS/TS 解析器。模拟 Node.js 模块路径解析，提取 ESM 真实依赖链。 |
| **本地数据与元数据索引** | [`rusqlite`](https://github.com "rusqlite") | 内嵌嵌入式 SQLite，维护“逻辑模块-物理文件-API 契约符号”的映射索引表。 |
| **无代码中间类型抽象参考** | [`ts-rs`](https://github.com "ts-rs") / `specta` | 参考其跨语言类型 IR 映射机制，用于将通用契约转换为具体编程语言的语言类型桩。 |

---

## 6. 第一阶段最小可行性验证 (Spike Plan)

建议自研团队优先通过以下三个独立的超轻量脚本进行技术可行性破局：

1. **Spike 1（拓扑生成）**：调用 `cargo_metadata` 抓取工作区本地 Crate 列表，利用 `petgraph` 构建内存图并成功打印出拓扑排序结果。
2. **Spike 2（契约解析）**：使用 `serde_yaml` 读取 Markdown 代码块中的自定义 API 契约，使用 `syn::parse_str` 解析一段 Rust 函数签名字符串，并在内存中完成二者的字段类型核对。
3. **Spike 3（路径解构）**：调用 `oxc_resolver` 传入带有 `tsconfig` 路径别名（如 `@/components`）的 TS 文件，验证其能否在几微秒内正确输出本地物理磁盘的绝对路径。