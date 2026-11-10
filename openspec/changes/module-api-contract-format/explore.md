# 模块公共 API 的统一格式与同步机制

workflow_type: requirement

目标：架构的模块用**统一格式**定义公共 API，并保障代码与 API 同步；未来基于模块 API 生成单元测试，替代「逐个文件测试」。桌面端最终要接管自己与其他项目的开发工作，因此格式不得绑定单一语言生态。

---

## 1. 定位

### 1.1 模块 = archi 模型元素

API 的键是 **element FQN**，既不是 crate 也不是文件。粒度由模型作者裁定：元素划到哪一层，契约就覆盖到哪一层。

- 覆盖范围由元素 `path` 经 `OwnerIndex::owner_of` 最长前缀唯一裁定（既有能力）；
- 一个 API 可横跨多文件、多 crate；容器元素 `path` 为空 → 天然无契约；
- ADR 0001 §2「crate 内部模块不建元素」约束的是**基线划分**，不约束契约粒度。真需要给子域定契约，那是「模型新增元素」的架构变更。

### 1.2 标准流程：声明先行，代码遵守

```
        Rust 侧                        TS 侧
契约 md（手写，唯一真相）      契约 md（手写，唯一真相）
   │ generate                     │ generate
   ▼                              ▼
api_gen/<module>.rs            <element>/<module>.i.ts
   │                              │
   ├─ impl'd by ─► _ext.rs        ├─ imported by ─► 手写实现
   └─ used by ───► 其余代码       └─ 形状校验

verify ≡ generate + cargo check + git diff --exit-code
verify ≡ generate + tsc --noEmit + git diff --exit-code
```

**两侧对等**：`verify` 不是独立模式，是「声明与代码是否一致」的机械判定，塌缩为「生成 + 编译器 + 零 diff」。因此不需要归一函数、双层锁、语义 diff 比对器。

两侧的编译器验证点不同：Rust 侧验**签名与实现**（`impl` 必须匹配 trait、函数体必须匹配签名），TS 侧验**形状**（返回类型标注必须匹配生成接口）。TS 无法验「抽象承诺」（结构化类型不强制 `implements`），这一点记 `x-common.degraded`。

`verify` 覆盖不到的三件事：bootstrap 方向（代码 → schema，属迁移工具）、跨模块语义不变量（散文 + 专项测试）、生成器自身正确性（生成器自己的单测）。

### 1.3 第一步：从存量代码生成草稿

bootstrap 生成器产出候选（`symbol` / `kind` / 定义位置 / 归一签名 / `visibility` / `used_by`），再由 Agent 或人工删除伪公共面，形成初版契约。输出 `visibility` 封闭四值：

| 值 | 判据 | 剪枝含义 |
|---|---|---|
| `external` | 模块外可达 | 真承诺，保留 |
| `crateOnly` | 仅同 crate / 同语言模块内可达 | 内部契约，保留但降格 |
| `unused` | 全仓无引用者 | **伪公共首选删除对象（机械判据）** |
| `testOnly` | 仅测试树内被引用 | 不进 API |

**抽取器按语言分派。两个语言的可达性语义不同，不可套用同一套规则。**

#### Rust 侧（`syn`，已是 `core/archi` 直接依赖）

| 抽取项 | 来源 | 映射 |
|---|---|---|
| 类型 / trait / 枚举 | `pub struct` / `pub trait` / `pub enum` | `$defs` 条目 |
| 操作 | `pub fn`（自由函数 + `impl` 方法） | `x-function` |
| 签名 | AST 归一为单行 | `x-function.params` / `returns` |

签名必须走 AST：多行签名占 `pub fn` 的 30–62%（`core/checks` 13/21、`core/archi` 16/27），`grep` 单行抽取会截半。

`visibility` 判据：

| 值 | Rust 判据 |
|---|---|
| `external` | 经 `pub mod` 链或 `pub use` 白名单对外可达 |
| `crateOnly` | `pub(crate)`，或 `pub` 但位于私有 `mod` 内 |
| `unused` | 全仓 `use` 事实中无引用者 |
| `testOnly` | 仅出现在 `#[cfg(test)]` 树内 |

近似误差源：glob 导入（`use a::*`）、同名符号、宏生成引用。

#### TS / JS 侧（`oxc`，`oxc_ast` / `oxc_parser` / `oxc_syntax` 已是直接依赖）

`scan/ts.rs` 现只抽 import / `export … from` 的 specifier 供依赖图使用；bootstrap 需**新增声明抽取**——同 AST、同 crate，零新依赖。

导出形态实测分布与映射：

| TS 形态 | 实测 | 映射 | 依据 |
|---|---|---|---|
| `export function` | 120（其中 26 返回 JSX） | `x-function` / 组件 | 唯一可靠的「操作」信号 |
| `export const` = `memo(...)` | 多数 | **组件**（本期跳过） | React 组件包装 |
| `export const` = 对象 / 常量 / Proxy | 其余 | 数据或**不进契约** | §7.1「模块不导出实例 / 不为标量常量设计公共面」 |
| `export type` / `export interface` | 138 / 66 | `$defs`（类型） | 直接转 JSON Schema |
| `export class` | **1** | `$defs` + `x-class` | 量级可忽略 |
| `export { … }` / `export * from` | **0 / 0** | — | 仓库零聚合重导出，首层 properties 免解析间接层 |

**关键结论：`export const` 里零个箭头函数（实测 0/132）。** 因此 TS 侧不需要「从初始化表达式推断函数」的启发式——**只有 `export function` 是操作**，判定比 Rust 简单得多。

`const` 三类须按初始化表达式区分，不能只看关键字：

| 初始化表达式 | 判定 |
|---|---|
| `memo(...)` | 组件 → `api = "none"`（本期留白） |
| 对象字面量 / `new Proxy(...)` / 标量 | 按 §7.1 排除 |
| 箭头函数 `=>` | 实测 0 例；若出现则按 `x-function` 处理 |

`visibility` 判据——**复用既有工具，不自造**：

| 值 | TS 判据 |
|---|---|
| `external` | 跨元素 import 引用 |
| `crateOnly` | 仅同元素目录内引用（TS 无 crate，以元素 `path` 边界替代） |
| `unused` | **`knip` 报告未使用**（`client:check` 已含） |
| `testOnly` | 仅 `*.test.ts(x)` 内引用 |

`knip.json` 的 `ignore` 面（`src/components/ui/**`、`src/types/generated/**`）与 archi 模型排除面高度一致，直接沿用其判定作为 `unused` 输入——不为 TS 侧造第二套死代码扫描。

#### TS 侧也生成，产物后缀 `*.i.ts`

TS 侧的强制边界：结构化类型下 `implements` 可选，故**抽象承诺强制不了**；但**具体形状可以强制**——实现侧 `import type` 引用生成的接口并标注返回类型，形状不符即 `tsc` 报错。

实测（`--strict --isolatedModules --moduleResolution bundler`）：

```ts
// src/checks.i.ts（生成）
export type Conclusion = "pass" | "fail" | "error";
export interface SummaryReport { total: number; conclusion: Conclusion }
```
```ts
// src/broken.ts（手写实现）
import type { SummaryReport } from "./checks.i";
const bad = (): SummaryReport => ({ total: 1, conclusion: "nope" });
// → TS2322: Type '"nope"' is not assignable to type 'Conclusion'.
```

形状校验在**编译期**成立，与 Rust 侧对等。而「导出集比对」只能到门禁期，弱一档。

##### 为什么是 `.i.ts` 而不是 `.d.ts`

| | `.d.ts` | `.i.ts` |
|---|---|---|
| 语义 | **ambient** 声明文件（TS 特殊类目） | 普通 TS 模块 |
| 内容限制 | 不能有初始化器 / 实现 | 无限制 |
| 工具行为 | 部分工具按「外部声明」处理，可能跳过 | 等同普通源文件 |
| 命名意图 | TypeScript 官方「已编译产物的类型」 | 本项目的「interface 生成物」 |

`.i.ts` 不触发 ambient 语义，也不与「Rust 编译产物」的 `.d.ts` 惯例混淆。仓库当前 `*.d.ts` / `*.i.ts` 均为 **0**，命名空间干净。

##### 落地三条

1. **`.i.ts` 必须 commit 并进 `git diff --exit-code`**——与 Rust 侧 `api_gen/` 同款。
2. **`knip.json` 的 `ignore` 必须加生成物路径**。实测未 import 的 `.i.ts` 会被报为 unused file（knip 的 `project` glob 覆盖 `src/**/*.ts`）。与既有 `src/types/generated/**` 同处理。
3. **实现侧必须 `import type` 引用生成物**，否则校验不发生——这是「伴随型」问题的落点：不用人记得同步，只要编译器看得见。

TS 侧仍不生成的三项（与 Rust 的差异保持）：

| Rust 生成项 | TS 侧 | 原因 |
|---|---|---|
| newtype struct | 别名 | 不做品牌类型（§5.3），按设计退化 |
| `derive(Deserialize)` | 无 | TS 类型是纯声明 |
| 工厂函数 `new()` | 无 | TS 无构造器派生需求 |

于是两侧对等：

```
verify ≡ generate + tsc/cargo + git diff --exit-code
```

`bindings.ts`（Rust → TS 的 IPC 命令面）与本能力生成的 `.i.ts`（模块 API 面）是**两个层次**，互不替代。

---

## 2. 资产形态

### 2.1 两层结构

```
<archi_dir>/api/<Domain>/<FQN>.md       手写唯一真相：职能 + 协议散文 + 分段的 fenced json
<archi_dir>/api/<Domain>/<FQN>.schema.json  生成：从 md 提取合成，供生态工具/校验器消费
<archi_dir>/api/INDEX.toml              生成：状态 / 类型数 / 操作数 / 覆盖率 / 降级计数

Rust 元素（<root> = 元素根路径「无后缀」，<name> = FQN 末段小写；详见 §2.6 / §2.7）
<root>/src/lib.rs | <root>/mod.rs | <root>.rs                  入口（按序查找）
<root>.i.rs                             生成：类型 / trait / 枚举 / 常量
<root>/**/*.rs                          手写：impl 块 + 实现体（不再有 _ext.rs）

TS 元素
<root>/index.ts | <root>/index.tsx | <root>.ts | <root>.tsx     入口（按序查找）
<root>.i.ts                             生成：类型别名 + interface（ambient-free）
<root>/**/*.ts(x)                       手写：实现，`import type` 引用 .i.ts
```

`<root>` 无后缀是硬约束（见 §2.6）：`views/architecture` 而非 `views/architecture/`，`crates/core/archi` 而非 `./crates/core/archi/`。实测 44 个 path 全部需要归一化改写。

**手写资产只有一份 `<FQN>.md`**（合写，理由见 §2.5）。`.schema.json` 与 `INDEX.toml` 都是生成物——它们不是第二份手写真相，是给工具消费的投影，漂移由 `git diff --exit-code` 拦。

**两侧都有生成物，形态不同**：Rust 生成真实类型定义（`impl` 可挂载其上），TS 生成接口声明（实现侧靠 `import type` 建立形状校验）。两者都 commit、都进零 diff 门。

TS 元素只新增 `.i.ts` 一处，实现侧仍是现有源码——门禁管「`.i.ts` 是否为契约的最新投影」，编译器管「实现是否匹配 `.i.ts`」。

`<archi_dir>` 由 `foundation::layout::archi_dir` 派生，新增 `archi_api_dir` 沿 `archi_models_dir` / `archi_decisions_dir` 同一模式。

### 2.2 为什么统一目录（而非跟代码走）

两种 review 模式答案相反：

| 模式 | 提问 | 相邻（代码侧） | 统一（archi 侧） |
|---|---|---|---|
| **审计型** | 这批改动里哪些模块的契约被动了？ | 差（散在数百改动文件里） | **好：`git diff --stat <archi>/api/` 即列出** |
| **伴随型** | 我改的这行，契约同步了吗？ | 好 | 差 |

伴随型问题已由门禁机械回答，**不需要靠文件相邻**；审计型问题只有统一目录能答，且无替代物。同源先例：`models/` 与 `decisions/` 同样不跟代码走。

**代价（明确留痕）**：人在 crate 里读代码时看不到契约文件，只能经画布 / `archi_module` / 门禁失败提示 / 全局搜索四条路之一。

### 2.3 与既有资产的关系

```
              element FQN（唯一键）
     ┌───────────────┼───────────────┐
     ▼               ▼               ▼
models/*.toml     api/<FQN>.json   docs 三源
元素/关系/gate     契约             叙事
     │               │               │
     └─ owner_of ────┘    docs::element_docs(FQN)
       「源文件属哪个模块」
```

**API 不接入 `docs` 三源**（`desktop-archi-module-docs` 已封死为恰三种来源）。改为 `archi_module` 投影新增并列字段，按 `archi_api_dir + FQN` 约定定位：文件不存在即无契约，零配置，且「有契约 / 无契约」成为可枚举事实。

### 2.4 语言无关性

`crate` 是 Rust 工具链概念，架构层不得依赖。分层：

```
语言无关    models · api 契约 · docs · graph · drift 判定 · FQN · gate
────────────────────────────────────────────────────────
语言特定    scan/rust.rs · scan/ts.rs（取数器）· 生成物投影
```

`core/archi/src/dto.rs` 的 `ElementKind` 已无 Lang 字段，原则已被遵守；但 models 注释未跟上（`02-backend-core.toml:2` 写「Cargo [dependencies]」、`06-frontend.toml:3` 写「import 证据」）。契约格式 MUST NOT 重复此错误。

### 2.5 为什么 api 与 doc 合写成一份

早期草案是 `<FQN>.json` + `<FQN>.md` 两份手写文件。这**与 §8.1 的裁决自相矛盾**——那里已用 `AGENT.md` / `CLAUDE.md` 证明「靠约定配对的两份手写文件必然漂移」（同源复制，`diff` 8 行后标题已分化）。

合写的收益不是省文件名，是**「改契约必须同时改理由」从纪律变成编辑动作**：改了 op 签名却不改不变量，在同一文件里是显眼的遗漏；在两文件里是一次无人察觉的漏改。

#### 载体选择：三个方案实测

| 方案 | 单文件 | 散文 diff | 跨语言一致 | 生态工具消费 | 解析成本 |
|---|---|---|---|---|---|
| **A. JSON 内嵌 `description`** | ✓ | ✗ 整行 | ✓ | ✓ 标准 | 零 |
| **B. YAML block scalar** | ✓ | ✓ | **✗**（见下） | ✓ | 低 |
| **C. md 为主 + fenced json** | ✓ | ✓ | ✓ | ✗ 需生成物 | 中 |

方案 A 实测：`"description": "…

- 不得因缺覆盖数据判 fail"` 是**一整行超长字符串**，改一个词 → 整行替换。人审体验差。

方案 B 外观最好（逐行 diff），但踩到致命陷阱：

```yaml
enum:
  - off      # YAML 1.1 解析为 False
  - warn
  - strict
```

实测 PyYAML（YAML 1.1）把 `off` / `on` / `yes` / `no` 解析成布尔值：

```
GateTier enum: [False, 'warn', 'strict']
GateTier default: False
```

而 **`off` 恰是本仓库 `GateTier` 的真实枚举值**（`off` / `warn` / `strict`）。YAML 1.2 修正了该问题，但各语言解析器版本不一致（PyYAML 1.1 vs serde_yaml 0.9 1.2），**跨语言一致性被破坏**——与「语言无关」的核心目标正面对撞。**方案 B 否决。**

#### 采纳方案 C

````markdown
# App.Backend.Core.Arch::GateTier

## 职能

gate 档位。缺省 `off` 是 ratchet 起点（存量全 off，逐个收紧）。

## 操作

### set_gate

改写单个元素的 gate 档位，返回改写后的源文本。

```json api-op
{
  "params": [
    { "name": "source", "schema": { "type": "string" } },
    { "name": "fqn",    "schema": { "type": "string" } },
    { "name": "gate",   "schema": { "$ref": "#/$defs/GateTier" } }
  ],
  "returns": { "type": "string" }
}
```

### plan_set_gate

```json api-op
{ ... }
```
````

实测性质：

- 散文逐行 diff（改一个词只红一行）；
- **一 op 一个 fence，diff 粒度 = op**（不是整个模块）；
- fence 内是标准 JSON，**零 YAML 歧义**，且可 pretty-print 多行；
- md 由 GitHub / VSCode / 编辑器原生渲染，人读体验最好。

#### 结构约定

| 位置 | 承载 |
|---|---|
| H1 标题 | `<FQN>`（与文件名一致，作锚点校验） |
| `## 职能` 等二维标题 | 散文区（人审主体） |
| `### <op-id>` | 操作锚点，后跟一个 ` ```json api-op ` fence |
| ` ```json api-types ` fence | `$defs` 类型定义 |

双向 gate（可机械校验）：`### <op-id>` heading 集合 **≡** `api-op` fence 内的 op 身份集合；`api-types` fence 内的键集 ≡ `.schema.json` 的 `$defs` 键集。任一缺失即契约错误。

#### 代价（明确留痕）

1. **需要 md fence 提取器**——新代码，但 `archi::docs` 已具备 markdown 读取能力（读 `decisions/*.md` 取标题），基础设施在。
2. **JSON Schema 生态工具读不到 md 里的 fence** → 用生成的 `<FQN>.schema.json` 补齐（同 `bindings.ts` 的既有模式：生成物供工具消费，人不审）。
3. **op 身份分散在 heading 与 fence 两处** → 由上面的双向 gate 兜住，不构成第二真相（heading 只提供名字，fence 只提供签名，无重叠字段）。


### 2.6 模块根路径

每个元素 SHALL 有**唯一根路径**：无 `./` 前缀、无尾 `/`、无文件扩展名。根路径是**模块身份**，不是文件路径，同时决定生成物落点、实现入口位置与 `#[path]` 写法。

现状 44 个 path 条目**全部需要归一化**（带 `./` 前缀 + 尾 `/`）。归一在模型解析层完成（`OwnerIndex::normalize` 已处理这些变体），不改存量 models。

`<name>` 取 FQN 末段小写（`App.Backend.Core.Archi.Scan` → `scan`）。

#### `#[path]` 的解析基准随入口形态变化（实测）

| 入口形态 | `#[path = "x.i.rs"]` 基准 |
|---|---|
| `<dir>/mod.rs` | `<dir>/` |
| `<dir>.rs` | `<dir>/../`（父目录） |

仓库现状：12 个子目录模块全部 `mod.rs` 形态，0 个 `dir.rs`。

#### 入口文件的固定编排（实测编译通过）

```rust
//! <模块 doc 保持原样>

#[path = "scan.i.rs"]
mod api_gen;
pub use api_gen::*;

pub mod owner;   // 既有声明不变
```

生成类型经 `pub use` 对外可引用；与既有同名类型冲突时编译器直接报错（E0255），不会静默覆盖。

#### 新判据 apiEntryMissing

```
apiEntryMissing   元素根路径下既无生成物（.i.rs / .i.ts）也无实现入口
                  → 该元素不是真模块，或已被删除
```

它比 `apiMissing` 更基础——**在契约生成之前就失效**，防止给不存在的模块写契约。

完整闭环：

```
models 声明元素 + 无后缀根路径
   ↓ 路径归一化
根路径 → 生成物路径（<root>/src/<name>.i.rs | <root>/<name>.i.ts）
   ↓ apiEntryMissing：实现入口必须存在
   ↓ git diff：生成物必须是契约的最新投影
   ↓ 编译器：实现必须匹配生成物
```

#### 父子路径重叠

子元素根路径（`.../archi/src/scan`）是父元素根路径（`.../archi`）的前缀。`OwnerIndex::owner_of` 的最长前缀匹配已正确处理：`.../src/scan/x.rs` 归 `Scan`，`.../src/dto.rs` 归 `Archi`。父元素契约不因存在子元素而缩窄。

### 2.7 生成物落点

#### 归属：生成物放根同级需一条剥离特例（已实测）

`owner.rs` 的 `prefix_matches` 判据是 `rel == prefix || rel.startsWith(prefix + "/")`，故 `<root>.i.<ext>` 不匹配 `<root>`。解法是归属规则加一条：**`.i.<ext>` 生成物按去掉 `.i` 后的路径判定**。

实测四种形态全部正确：

| 路径 | 元素前缀 | 剥离后 |
|---|---|---|
| `views/architecture.i.ts` | `views/architecture` | ✓ |
| `views/welcome-view.i.ts` | `views/welcome-view` | ✓ |
| `.../crates/core/archi.i.rs` | `.../crates/core/archi` | ✓ |
| `.../crates/core/archi/src/scan.i.rs` | `.../crates/core/archi/src/scan` | ✓ |

#### 方案对比：按文件（根同级）vs 强制目录

| | **方案 1：按文件（采纳）** | 方案 2：强制模块是目录 |
|---|---|---|
| 生成物路径 | `<root>.i.<ext>` | `<root>/<name>.i.<ext>` |
| 归属 | 需 `.i` 剥离特例（1 处代码） | 天然成立 |
| 单文件模块 | 零改动 | 12 个前端元素要改成目录 + 全部引用点 |
| Rust `#[path]` | 目录模块：`"../<name>.i.rs"`；单文件：`"<name>.i.rs"` | 恒为 `"<name>.i.rs"` |
| TS `import` | 目录模块：`"../<name>.i"`；单文件：`"./<name>.i"` | 恒为 `"./<name>.i"` |
| 规则条数 | **1 条**（根路径 + `.i` + 扩展） | 2 条（目录内 + 形态判定） |

采纳方案 1：规则只有一条无形态分叉；12 个单文件元素零改动；归属特例成本极低。

**代价留痕**：目录形态的引用路径带 `../`，生成物与模块文件在 `git` 视角不聚拢。

#### 不做 `_ext.rs`

生成物只含类型定义，`impl` 块写在模块内任意手写文件里（Rust 允许同 crate 内 `impl` 分散）。生成/手写分离靠**文件命名**（`.i.rs` 后缀）而非目录分离。

### 2.8 入口查找顺序与现状缺口

#### 最终顺序（已定）

```
Rust      <dir>/src/lib.rs     crate 形态（实测 14/14）
          <dir>/mod.rs         子目录模块形态（实测 12 个存量子模块全部此形态）
          <dir>.rs             单文件模块形态

前端      <dir>/index.ts
          <dir>/index.tsx
          <dir>.ts
          <dir>.tsx
```

`<dir>/lib.rs` **不在列表内**——crate 的 `lib.rs` 恒在 `src/` 下。三分支各有真实存量支撑，无冗余分支。

#### 实测命中

| 分支 | 命中 |
|---|---|
| `<dir>/src/lib.rs` | **14**（全部 crate） |
| `<dir>/mod.rs` | 0（12 个子目录模块尚未建元素） |
| `<dir>.rs` | 0（无单文件模块元素） |
| 前端四分支 | **0** |

前端 0 命中的三类原因：视图/子域入口是 `*-view.tsx`（非 `index.tsx`）、16 个单文件元素根路径带扩展名、`components/ui` 无 index。

**这些债务不消解**（§2.10）——`apiEntryMissing` 报出即达目的，报告数即迁移 backlog。

### 2.9 `<FQN>.schema.json` 不可手工修改

**结论：不可。** 它是 `<FQN>.md` 的纯派生投影，零手写自由度。

#### 派生完备性（已验证）

`schema.json` 的每一项都有 markdown 出处：

| schema.json 字段 | 来源 |
|---|---|
| `$schema` / `$id` | 常量 + FQN |
| `title` | H1 标题 |
| `$defs` 本地定义 | `api-types` fence |
| `$defs` 透传 alias | `api-reexport` fence（值 → `./<FQN>.schema.json#/$defs/<name>`） |
| `properties` 操作项 | `api-op` fence 的键 + `x-function` |
| `properties` 类型项 | `$defs` 键集 → `{"$ref": "#/$defs/<name>"}` |
| `$comment` | 常量警示文本 |

实测按上述规则组装，产物与预期逐项一致——**不存在「只有手改才能表达」的内容**。

关键原因：`api-types` / `api-op` 的 fence 内容是**原生 JSON Schema**，不是自造的协议记法。因此 `minLength` / `pattern` / `enum` / `anyOf` / `x-*` 全部可在 markdown 内表达。生成器只做**装配**（拼 `$defs` + `properties` + `$id` + 常量），不做**翻译**。

若某天确实需要 schema 里存在 markdown 表达不了的东西，那是生成器能力不足，应修生成器，而非开口子手改。

#### 违反时的行为（实测）

| 情形 | 行为 |
|---|---|
| 手改**已提交** | 零 diff 门持久失败，直到回退 —— **响**，可发现 |
| 手改**未提交** | 下一次 `generate` 静默覆盖，零报错零痕迹 —— **哑**，与今日 `bindings.ts` 同款已知行为 |

生成物头部须带警示，沿既有惯例：

```json
{
  "$comment": "由 <FQN>.md 生成，勿手改。手改将在下次 generate 时被覆盖，且零 diff 门会失败。"
}
```

对照：`bindings.ts` 首行为 `// This file has been generated by Tauri Specta. Do not edit this file manually.`；`bindings.zod.ts` 首行 `// 本文件由 cargo run -p export-bindings 生成…勿手改`。

#### 与 `config.schema.json` 的区别

仓库另有一份**手工维护**的 `config.schema.json`（`AGENT.md` 明文「静态 JSON 镜像」「双向一致性测试强制执行」）。两者纪律相反，原因明确：

| | `config.schema.json` | `<FQN>.schema.json` |
|---|---|---|
| 上游真相 | 无其他载体（`KNOWN_KEYS` / `TestFramework` 在 Rust 代码里，无法反向生成） | `<FQN>.md`（专门为它写的真相） |
| 维护方式 | 手工 + 双向一致性测试 | 生成 + 零 diff 门 |
| 手改 | 合法且必需 | **禁止** |

`AGENT.md` 记录这份手维护 schema 是已知负担——本能力的设计正是为了不重演：给 schema 一个专门的上游真相，让它可被生成。

#### 提交纪律

`schema.json` **必须提交**（不 `.gitignore`），沿 `bindings.ts` 既有选择。三个理由：

1. 零 diff 门需要 git 里有基线可比；
2. 工具可离线消费，不必先跑生成器；
3. 审计型 review 可 `git diff --stat api/` 一眼看全。

**代价**：每次改契约产生两份 diff（`.md` + `.schema.json`）。但因 schema 零手改自由度，**review 只读 `.md`，`.schema.json` 的 diff 视为机械噪声**。这条要写进 `api/` 的 README 或 INDEX 说明。

#### 派生判据：投影可替代真相的条件

由此提炼一条可复用的判断法则：

> **投影可以安全替代原始真相，当且仅当「投影 ← 真相」方向全部由机械门禁守住。**

`schema.json` 满足：

```
md 的 api-types / api-reexport fence   ← 手写真相
        ↓ apiReexportDrift（fence ≡ 散文列表）
        ↓ 生成器（单一实现）
schema.json                            ← 投影
        ↓ git diff --exit-code（防手改）
        ↓ $comment 警示
```

因此消费 schema.json 的判据（如 `apiUndeclared`）不必回溯到 markdown——投影与真相之间的每一步都有门禁。反例：`config.schema.json` 不满足此条件（无专门的上游真相、靠双向一致性测试手补），故它**不能**被当作派生性的、可信的投影。

---

### 2.10 存量债务不消解：债务即检查器的验证语料

用户裁定：**所有代码存量债务先不消解，确保检查能否正确提示即可。**

这条把债务的定位从「要清的负担」翻转为「验证语料」，并连带改变三处设计。

#### 一、gate 档位：一律 `warn`，唯完备性判据 `off`

若判据设 `off`，就无法验证检查器是否正确工作；而债务恰恰是最真实的测试输入。故一致性判据一律 `warn`。

| 判据类 | 档 | 理由 |
|---|---|---|
| 一致性判据（`apiEntryMissing` / `apiMissing` / `apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift`） | **`warn`** | 报的是「债」，可枚举可消除 |
| 完备性判据（`apiUndeclared`） | **`off`** | 报的是「海」——要求代码每个导出都有契约声明，迁移期必然爆炸 |

分界线：**一致性判据（契约说 A、代码是 B）可即开 `warn`；完备性判据（代码有的契约都得有）必须后置。**

#### 二、不消解的债务清单（均作为预期报告项）

| 债务 | 数量 | 检查器应报 |
|---|---|---|
| 元素无实现入口 | **27**（实测） | `apiEntryMissing` 27 条 |
| `infra/agent/sdk` 子→父类型依赖 | 3 处 | `UndeclaredDependency`（既有判据，`gate ≠ off` 时） |
| 父子 `$defs` 同名风险 | 0（尚无嵌套元素持契约） | `apiShadowing` 0 条 |
| 前端多入口（`changes` / `explores` 各 2 个） | 2 | 归入「无入口」（无一命中四分支） |
| 壳元素根路径指向 `src/` | 1 | 归入 `apiEntryMissing` |
| 前端单文件元素根路径带扩展名 | 16 | 归入 `apiEntryMissing` |

##### 27 条 apiEntryMissing 的完整清单（可作金样）

```
App.Backend.Shell                              packages/desktop/src-tauri/src
App.Backend.Tools.ExportBindings               crates/tools/export-bindings
App.Frontend.Shell                             src/app.tsx
App.Frontend.Views.AgentDebug                  src/views/agent
App.Frontend.Views.Agents                      src/views/agents
App.Frontend.Views.Architecture                src/views/architecture
App.Frontend.Views.Changes                     src/views/changes
App.Frontend.Views.Config                      src/views/config
App.Frontend.Views.DbInspector                 src/views/db
App.Frontend.Views.Explores                    src/views/explores
App.Frontend.Views.Info                        src/views/info
App.Frontend.Views.Welcome                     src/views/welcome-view.tsx
App.Frontend.Views.Changes.Flow                src/views/changes/flow
App.Frontend.Views.Changes.Renderers           src/views/changes/renderers
App.Frontend.Views.Changes.Agent               src/views/changes/agent
App.Frontend.Components.Agent                  src/components/agent/index.tsx
App.Frontend.Components.Agent.AgentBlocks      src/components/agent/agent-blocks.tsx
App.Frontend.Components.Agent.AgentError       src/components/agent/agent-error.ts
App.Frontend.Components.Agent.AgentInput       src/components/agent/agent-input.tsx
App.Frontend.Components.Agent.AgentMessages    src/components/agent/agent-messages.tsx
App.Frontend.Components.Agent.AgentSessionView src/components/agent/agent-session-view.tsx
App.Frontend.Components.Agent.ToolCards        src/components/agent/tool-cards.tsx
App.Frontend.Components.Standard.Dailog        src/components/standard/dailog.tsx
App.Frontend.Components.Standard.Drawer        src/components/standard/drawer.tsx
App.Frontend.Components.Standard.Pagination    src/components/standard/pagination.tsx
App.Frontend.Components.Standard.Select        src/components/standard/select.tsx
App.Frontend.Components.Ui                     src/components/ui
```

按 kind 分布：`module` 16 / `component` 10 / `domain` 1。

`App.Frontend.Components.Agent` 也在列——它的 `path` 是 `components/agent/index.tsx`，按 `<d>.tsx` 分支会拼成 `index.tsx.tsx`，故未命中。这正是「根路径须去扩展名」这条债的体现。

#### 三、验收判据：报告数逐项对拍

提案的验收标准由此可机械化：

| 判据 | 预期报告数 | 容差 |
|---|---|---|
| `apiEntryMissing` | **27** | 0 |
| `apiMissing` | 40（41 非容器 − 1 试点） | 0 |
| `apiShadowing` | 0 | 0 |
| `apiRefMissing` | 0 | 0 |
| `apiReexportDrift` | 0 | 0 |
| `apiNestedCrossCrate` | 0 | 0 |
| `apiUndeclared` | 不适用（`off`） | — |

**「检查能否正确提示」的判据 = 报告数与上表逐项相等**。多于预期说明判据误报（假阳），少于预期说明判据漏报（假阴），两者都是检查器缺陷。

#### 四、副作用：债务清单即迁移 backlog

`warn` 模式下的报告输出天然成为迁移待办表——27 条 `apiEntryMissing` 就是 27 个待建 barrel / 改名的任务。这比手写 todo 更可靠（它直接来自代码事实）。

于是「消解债务」与「验证检查器」成为同一件事的两个阶段：先看报告是否准确，再按报告逐项消除。每消一条，报告数减一，可单调验证。
---

## 3. 关键字集

### 3.1 全貌

```
标准关键字    object / enum / const / default / oneOf / $ref
x-class       声明为 trait（仅允许方法）
x-function    函数的 params / returns / async
x-common      自由注记（degraded / unit / note / since / owner …）
```

**只有 2 个扩展构造**，其余用标准关键字承载。

### 3.2 为什么必须保留 x-function

JSON Schema 没有表达「函数」的标准方式。且它是唯一能区分两种同形意图的标记：

```rust
pub fn parse(source: &str) -> ParseOutcome                    // 模块操作
pub struct Helper { pub parse: fn(&str) -> ParseOutcome }     // 函数指针字段
```

仓库当前 0 例函数指针字段，但接管其他项目时必然出现。

### 3.3 为什么必须保留 x-class

形状推导（「properties 全是函数 → trait」）无法区分 `Port`（本意 trait）与 `Helper`（本意函数指针结构体），会静默误判。显式标记 + 强校验可消灭该静默故障：

| `$defs` 内容 | 生成 | 校验规则 |
|---|---|---|
| `x-class` | **`pub trait`** | **仅允许方法**；出现属性 → **契约错误** |
| `object` + `properties` | `pub struct` | 属性、方法均允许 |
| `enum` | `pub enum` | — |
| 标量（`"type":"integer"`） | `newtype struct` | 见 §3.4 |

普通 struct **不限制属性和方法**：契约描述「对外承诺什么」，方法承诺本就在其中，实现落手写文件，由编译器验签名。

**`x-class` 只描述「本元素提供的 trait」，不表达「本元素实现了谁的 trait」。** 后者是跨元素依赖，由模型 relation + 编译器保证（§4.6）。

反向补一条**警告**（非错误）：`properties` 全部为函数却未标 `x-class` → 提示可能意图为 trait。因为函数指针结构体是合法意图，不能一刀切拒绝。

### 3.4 标量 → newtype，不能生成 trait

```rust
pub trait TimestampMs { }        // 名义性为零：任何类型都能 impl
pub struct TimestampMs(pub i64); // 正确：零开销名义类型
```

名义性不设独立构造，由「标量 vs object」形状推导（见上表）。

### 3.5 x-common：自由注记

不驱动生成，只解释与补述。实测需要承载两类：

- `degraded`：语言能力缺失的显式留痕（当前仅两类——错误通道走 `Promise` reject 不在类型系统内；TS 结构化类型无法强制实现）；
- `unit` / `note` / `since` / `owner`：单位与来源说明。仓库现有 `/// UTC unix 毫秒` 这类单位注记散在文档注释里（`started_at` / `timestamp` / `duration_ms`），需要机械载体。

注记键**自由扩展**、不加强校验——它是人的表达，不该每次改生成指令的 schema。

### 3.6 为什么不用 Zod

Zod 是 TS 生态产物，当通用载体等于把「语言无关」偷换成「TS 优先」。语言无关的注解载体是 JSON Schema 原生机制。

关键安全性事实：**规范要求校验器 MUST 忽略未知关键字**，故 `x-*` 扩展不破坏生态工具、IDE、校验器——这是 Zod `meta` 给不了的跨语言保证。

---

## 4. 引用与命名规则

### 4.1 三条硬规则

| # | 规则 | 违反后果 |
|---|---|---|
| 1 | **身份 = `$defs` 键名**，不是 pointer 路径 | 同名两处定义 → 契约错误 |
| 2 | 每个具名类型在顶层 `$defs` 定义恰一次；嵌套 `$defs` 一律提升，**原名保留** | 提升名碰撞 → 契约错误，禁静默改名 |
| 3 | 所有 `$ref` target 必须是 `$defs` 顶层条目 | 指向 `properties` / `items` / `allOf/0` → 契约错误 |

规则 2 的命名选择理由：作者写的是 `B` 就还叫 `B`；加父前缀 `AB` 会让类型名取决于它「恰好被谁嵌套」，换个位置就改名，是静默漂移。**嵌套 `$defs` 只是书写分组，不构成作用域。**

规则 3 的理由：`properties/attr`、`items`、`allOf/0` 这些位置**没有稳定名字**——名字取决于父级结构，父级改一个属性名就变了。

### 4.2 多 `$ref` 指向同一类型：只生成一个

规范化后「深度」概念消失：

```
#/$defs/A                     →  $defs.A            （唯一定义）
#/$defs/Parent/$defs/B        →  $defs.B            （提升，原名保留）
                               $ref: #/$defs/A      （若与既有同类型，原位置改写）
```

多个 `$ref` 指向同一顶层定义是正常形态（`ArchiModuleDetail` 一个类型引用 3 个兄弟类型），生成时只定义一次。

### 4.3 严禁按结构去重

实测 17 组「字段类型序列完全相同」的类型：

| 签名 | 同形类型 |
|---|---|
| `(String, Option<String>)` | `ArchiDependsOn` / `ArchiDependedBy` ← **出入边，语义相反** |
| `(DiagnosticKind, String, String, u32, Option<u32>)` | `Diagnostic` / `ArchiDiagnostic` ← 进程内 vs 线面 |
| `(String,)` | `TurnQuestion` / `ProjectConfig` / `StoreSnapshot` / `DecisionLogOutcome` |
| `(f64,f64,f64)` | `CoverageThresholds` ← 跨 `core/config` 与 `core/checks` 两处独立定义 |

合并 `ArchiDependsOn` 与 `ArchiDependedBy` 等于把「出边」「入边」变成同一个类型。**身份靠名字，去重靠名字。**

### 4.4 位置语义

```
第一层 properties    模块导出成员（x-function 操作 / $ref 到具名类型）
$defs                本模块自有类型（本地定义）+ 透传项的纯 $ref alias
api-reexport fence   透传清单（人读 + 生成器输入，与 $defs 透传键集相等）
```

公共面 = **第一层 + 第一层经 `$ref` 可达的闭包**。此约定自动排除不出现在任何函数签名里的纯内部类型。

`$ref` 的**指向**天然表达 trait 性质：对定义它的模块是「我提供的类型」，对引用它的模块是「我要求外部提供的契约」。跨模块 `$ref` 无需任何标记。

---

### 4.5 模块嵌套：父元素透传子元素

真实案例已存在（非假设）：**9 对父子元素全部在前端**。

```
App.Frontend.Components.Agent ⊃ AgentBlocks / AgentError / AgentInput
                                / AgentMessages / AgentSessionView / ToolCards
App.Frontend.Views.Changes   ⊃ Flow / Renderers / Agent
```

`App.Frontend.Components.Agent` 的 path 正是 `components/agent/index.tsx` —— 一个 barrel 文件，实测内容：

```tsx
export { AgentInput } from './agent-input';
export { AgentSessionView } from './agent-session-view';
export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view';
export { SessionPanel } from './session-panel';
export type { SessionPanelState } from './session-panel';
```

##### 事实一：父的 API 面 ≠ 子元素面的并集

父元素有 6 个子元素，但只透传 **2 个**（`AgentInput` / `AgentSessionView`）。`AgentBlocks` / `AgentError` / `AgentMessages` / `ToolCards` 是子元素却**未被父导出**。

且父还导出了 `SessionPanel`（来自 `session-panel.tsx`）—— **该文件不是元素**。

所以父契约的构成是三类混合：

| 来源 | 例 | 契约表达 | 生成 |
|---|---|---|---|
| 透传自子元素 | `AgentInput` | `api-reexport` fence + `$defs` 纯 `$ref` 条目（见「三处落位」） | `export { AgentInput } from '...'` |
| 自有（非元素文件） | `SessionPanel` | `$defs` 内本地定义 | `export interface SessionPanel` |
| 子元素但不导出 | `AgentBlocks` | **不出现** | — |

**结论：透传是选择性的，父契约必须逐项列出透传目标。** 不能写成「父的 API = 子元素并集」这种自动推导。

##### 事实二：Rust 侧透传机制实测

| 场景 | 结果 |
|---|---|
| 父显式 `pub use drift::CheckConfig` | ✓ 通过 |
| 父 glob `pub use drift::*` | ✓ 通过 |
| 父重复定义（显式 `use` + `struct`） | ✗ **E0255 编译错误** |
| 父重复定义（glob `use` + `struct`） | ⚠ **仅 warning** `ambiguous glob re-exports` |
| 生成物内 `pub use crate::drift::X`（`mod` 声明在手写侧） | ✓ 通过 |
| 外部经父路径 `nest5::CheckConfig` 与子路径 `nest5::drift::CheckConfig` | ✓ 均可达 |

**第四条是核心风险**：glob 形态的重复定义编译器只给警告，不报错。所以「避免重复定义」MUST 由契约层自己保证。

#### 跨模块引用：相对文件路径

四种写法实测（ajv / Ajv2020）：

| 写法 | 结果 |
|---|---|
| 自定义 scheme `module://X#/$defs/Y` | **✗** `can't resolve reference`，ajv 只认标准 URI |
| 合成 https URI `https://archi.invalid/X` | ✓ 但需 `$id` 同构注册，可读性差 |
| **相对文件路径 `./<FQN>.schema.json#/$defs/Y`（采纳）** | ✓ |
| 裸文件名 `<FQN>.schema.json#/$defs/Y` | ✓ |

采纳相对路径，`$id` 也取文件名（`<FQN>.schema.json`），不做任何合成 URI。同目录用 `./`，跨 Domain 目录用 `../`。

两个前置条件：

1. 消费方 MUST 用 **draft 2020-12 校验器**（ajv 需 `require("ajv/dist/2020")`；默认的 `ajv` 是 draft-07，会报 `no schema with key or ref ".../2020-12/schema"`）。
2. 每个被引用 schema MUST 先 `addSchema` 注册。

#### 校验器的 strict 模式必须关

实测 ajv：`strict: true` 时 `unknown keyword: "x-function"` 编译失败；`strict: "log"` 与 `strict: false` 通过。

消费 `<FQN>.schema.json` 的工具须配 `strict: "log"` 或 `false`——`x-function` / `x-common` / `x-class` 都是未知关键字，strict 模式必然报错。

#### 禁断子→父依赖

**子模块 MUST NOT 依赖父模块（含类型）**，从设计上断绝这条路径。

界定：`dto` 与 `scan` 在 `lib.rs` 里同为子模块（`pub mod dto;` / `pub mod scan;`），二者互引是**兄弟依赖**，不在禁断范围。

实测「子 → 父」依赖只有一处 crate：

| crate | 子 → 父类型引用 | 具体 |
|---|---|---|
| `infra/agent` | **3 处** | `sdk/` 用 `lib.rs` 直接定义的 `EngineFacade` / `EngineKind` / `ResumeTranscript` |

其余 13 个 crate **零**子→父类型依赖。存量代价极小，且本期不消解（§2.10）。

##### 架构含义

依赖方向只有一条：**父 → 子**（父透传子）。

```
允许   父 → 子        父契约 $ref 子契约（透传）
允许   兄弟 → 兄弟     Scan → Dto（同层，实测主流形态）
禁断   子 → 父         Scan → Archi（crate 根直接定义的类型）
```

**共享类型因此不得留在父元素**，三条出路：

| 出路 | 形态 | 代价 |
|---|---|---|
| **下沉到兄弟元素** | 父下新增 `Shared` 子元素承载公用类型 | 需模型新增元素；兄弟依赖需显式 relation |
| 合入父元素 | 不把该子目录提升为元素 | 失去该子域的契约独立性 |
| 消解依赖 | 类型改为泛型参数 / 由入参传入 | 需重构 |

**这条规则的最大价值不在禁断本身，而在它让「把子目录提升为元素」变成一次真实的解耦决策**——不能白贴标签。以 `infra/agent` 为例：要把 `sdk/` 提升为元素，先得把 3 个类型从 `lib.rs` 挪出来。

##### 环依赖从此不可能

跨元素 `$ref` 只剩「父→子」与「兄弟→兄弟」两类边，必然可拓扑排序（叶子在前、根在后）。Rust 侧的 crate 环由 cargo 直接拒绝（实测 `cyclic package dependency`），模型侧的关系环由既有 `cycle` 诊断拒绝。

##### 为什么不需要独立判据 `apiLeakedDependency`

契约不是依赖事实源，事实链是单向的：

```
<FQN>.md 契约（人写）→ 生成物 → 代码必然 import / pub use
                                      ↓ scan 抽 use/import 事实
                                      ↓ OwnerIndex 归属到元素 FQN
                                      ↓ drift::check 与 relation 声明对账
                       既有 UndeclaredDependency（已实现）
```

| 情形 | 谁抓 |
|---|---|
| 契约 `$ref` 了模型未声明的元素 | 生成物 → 代码 import → **`UndeclaredDependency`** |
| 契约 `$ref` 指向不存在的目标 | 生成器 / `apiRefMissing` |
| 子元素依赖父元素类型 | 代码 import `Scan → Archi` → **`UndeclaredDependency`**（父子关系不进 relation 图，故必然报） |

**判据建在事实源（代码）上，不在投影（契约）上。** 新增判据只在「契约自身内部矛盾」时才成立。

##### 嵌套元素必须同属一个编译单元

父透传子类型 + 子引用父类型 → 若父子分属不同 crate 即 crate 循环依赖。实测 cargo：

```
error: cyclic package dependency: package `a` depends on itself
```

故 **嵌套元素 MUST 落在同一 crate 内**（父 = crate，子 = 其内子模块）。跨 crate 父子在 Rust 侧不可表达，模型层拒绝（判据 `apiNestedCrossCrate`）。TS 侧无此约束。

#### 两种载体的分工

用户裁决：markdown 保持结构化章节（阅读），`schema.json` 同时生成 `$ref` 关联（检查），复杂度只落在转换过程。

##### 为什么 schema.json 必须含跨文件 $ref

即便禁断了子→父，**父→子（透传）与兄弟→兄弟仍要跨文件**：

| 边类型 | 例 | 是否禁断 |
|---|---|---|
| 父 → 子（透传） | `Archi` 契约 `$ref` `Archi.Scan` 的类型 | 允许 |
| 兄弟 → 兄弟 | `Archi.Scan` 引用 `Archi.Dto` 的类型（若 `dto` 也成元素） | 允许 |
| 子 → 父 | `Archi.Scan` 引用 `lib.rs` 直接定义的类型 | **禁断** |

「schema.json 自包含」这个优点因此不成立——跨文件 `$ref` 是结构性的。既然不成立，把透传排除在 schema.json 之外只换来「少几个 `$defs` 条目」，却要 `apiUndeclared` 从单文件读取退化为并读 markdown，成本反而更高。

##### 实测：ajv 对两类 $ref 的态度不同

| `$ref` 位置 | ajv 是否解析 |
|---|---|
| schema 层（`properties` / `$defs`） | **✓ 解析并校验**；悬空引用编译期报 `can't resolve reference` |
| `x-function` 内部（`params[].schema` / `returns`） | **✗ 不解析**——`x-function` 是未知关键字，ajv 不下沉 |

对照实测：同一个悬空 `$ref` 放 schema 层报错、放 `x-function` 内静默通过。

**这限定了 schema.json 的校验价值边界**：

- 类型级跨模块引用 → 外部校验器**能验**，值得进 schema.json
- 函数签名里的跨模块引用 → **验不了**，由生成器 + 编译器保证

所以 schema.json 的定位是「**类型层的可校验投影**」。透传恰好是类型层的事，进 `$defs` 正中 ajv 能验的范围。

##### 回环不是障碍（实测）

三方环（父透传子 + 子参数引用父类型）在 ajv 下**两种加载顺序均通过**。

##### 三处落位

| 载体 | 内容 | 用途 |
|---|---|---|
| `<FQN>.md` 的 `api-reexport` fence | 透传清单（键 = 导出名，值 = 直接子元素 FQN） | **人读** + 生成器输入 |
| `<FQN>.md` 散文 | 透传意图 | 人读 |
| `<FQN>.schema.json` 的 `$defs` | 本地定义 **+ 透传项（纯 `$ref`）** | **机器检查**（悬空引用编译期即报） |

`$defs` 内两类条目：

```json
"$defs": {
  "AgentInput":   { "$ref": "./App.Frontend.Components.Agent.AgentInput.schema.json#/$defs/AgentInput" },
  "SessionPanel": { "type": "object", "properties": { "open": { "type": "boolean" } } }
}
```

生成器据此产出 `pub use` / `export { } from`（纯 `$ref` 条目）或 `struct` / `interface`（其余）。

##### 代价：读者须预加载依赖闭包

schema.json 不自包含，消费方须按 `$id` 逐个 `addSchema` 预加载引用闭包。与 `bindings.ts` 处境一致（同为生成物、同需完整类型图）。复杂度集中在转换过程，正是预期的形态。

##### 判据落点

| 判据 | 读取面 | 备注 |
|---|---|---|
| `apiUndeclared` | 代码导出集 **vs** schema.json 的 `$defs` 全集 | 单文件足够，不必并读 markdown |
| `apiShadowing` | 父子 schema.json 的 `$defs` 键集 | — |
| `apiRefMissing` | schema.json 的悬空 `$ref` | ajv 编译期即捕获，不必自写判据 |
| `apiReexportDrift` | markdown 散文列表 vs `api-reexport` fence | 防两处不一致 |
| `apiNestedCrossCrate` | 父子元素的 crate 归属 | Rust 侧不可表达跨 crate 嵌套 |

##### `apiUndeclared` 为何可以只读 schema.json

判据两侧恰是「事实源 vs 契约」：

| 侧 | 事实来源 | 抽取方式 |
|---|---|---|
| 代码导出集 | 源码 | `scan/rust.rs`（`syn`）/ `scan/ts.rs`（`oxc`） |
| 契约声明集 | schema.json 的 `$defs` 键集 | 生成物，读文件即可 |

安全性由前序门禁保证：

```
$defs 键集 = api-types fence ∪ api-reexport fence
              ↓ apiReexportDrift 保证 fence 与散文列表一致
              ↓ schema.json 零手改自由度（§2.9）
```

**普适形式**：投影可以安全替代原始真相，当且仅当「投影 ← 真相」方向全部由机械门禁守住。

#### 契约规则

| # | 规则 | 违反后果 |
|---|---|---|
| 1 | 透传 SHALL 同时表达于两处：`<FQN>.md` 的 `api-reexport` fence（人读 + 生成器输入）与 `<FQN>.schema.json` 的 `$defs` 纯 `$ref` 条目（机器检查）。两处键集 MUST 相等 | `apiReexportDrift` / 生成物零 diff 门失败 |
| 2 | 父 `$defs` 的透传条目 MUST NOT 携带结构关键字（`type` / `properties` / `enum` / `const` / `items` / `oneOf`）——它是纯 `$ref` alias | 沦为重复定义 → `apiShadowing` |
| 3 | 父 `$defs` 与任一子元素 `$defs` 的**类型名交集必须为空** | `apiShadowing` 契约错误。**不能依赖编译器**（glob 透传只警告，实测） |
| 4 | 透传目标与跨模块 `$ref` 必须是**直接子元素**（FQN 前缀相邻）且其契约存在、`api = declared` | `apiRefMissing` |
| 5 | 嵌套元素的父子 MUST 同属一个 crate | `apiNestedCrossCrate`（cargo 报循环依赖） |
| 6 | 子元素 MUST NOT 依赖父元素（含类型）；共享类型须下沉到兄弟元素或消解 | 代码层由既有 `UndeclaredDependency` 报（见 §4.5 末「为什么不需要新判据」） |

生成侧：

```rust
// <root>.i.rs（父，生成）—— 透传出 pub use，自有出 struct，无重复定义
pub use crate::drift::CheckConfig;    // 来自纯 $ref 条目
pub struct OwnType { .. }             // 来自本地定义条目
```

```ts
// <root>.i.ts（父，生成）
export { AgentInput } from "@element/agent-input";   // 透传
export interface SessionPanel { .. }                 // 自有
```

依赖顺序无关（Rust 模块级项顺序无关、TS 类型导出顺序无关），实测通过。

#### 递归透传

若 A ⊃ B ⊃ C，A 只声明透传 B（MUST NOT 展开 C）。生成物里 A 写 `pub use crate::b::X`，B 内部已透传 C 则自动带出。

契约校验时跨模块 `$ref` 解析 SHALL 只在**直接子元素**（FQN 前缀相邻）范围内查找，跨层引用报 `apiRefMissing` —— 防止父模块绕过中间层直连孙辈（层级语义被架空的信号）。

### 4.6 契约记录「提供什么」，不记录「实现了什么」

这是本能力最容易被误解的一条，必须单独立节。

#### 两个不同的东西

| 形态 | 例 | 归属 | 由谁记录 |
|---|---|---|---|
| **本元素提供的 API** | `LocalArchiCheck::new()`（inherent 方法） | 本元素 | **契约** |
| **本元素实现了某 trait** | `impl ArchiCheckRunner for LocalArchiCheck` | 跨元素依赖 | **模型 relation** + 编译器 |

`ArchiCheckRunner` 定义在 `App.Backend.Core.Orchestration`（`port.rs:420`），`LocalArchiCheck` 在 `App.Backend.Infra.ArchiRuntime`——两个元素。这条依赖**模型里已经声明**：

```toml
[[relation]]
from = "App.Backend.Infra.ArchiRuntime"
to   = "App.Backend.Core.Orchestration"
desc = "门禁 runner 实现编排的步骤端口"
```

由既有 `UndeclaredDependency` 判据管辖（代码有跨元素依赖但模型未声明即报）。

#### 因此不需要 `implements` 注记

曾考虑在 `$defs` 条目上加：

```json
"LocalArchiCheck": { "x-common": { "implements": "./App.Backend.Core.Orchestration.schema.json#/$defs/ArchiCheckRunner" } }
```

**否决。** 理由：

1. **重复表达**——模型的 relation 已说「谁依赖谁」，注记再说一遍即第二份真相。
2. **契约语义被污染**——契约是「对外承诺」，`implements` 是「对内实现方式」，消费者不关心。
3. **判据冗余**——`apiImplementsMissing` 能查的，编译器（`impl` 不完整即报错）与 `UndeclaredDependency`（relation 未声明即报）已双重覆盖。

#### 推论：三个 runner 的契约面就是 `new()`

实测三种 runner 同一模式：

| runner | 元素 | 公开面 |
|---|---|---|
| `LocalArchiCheck` | `Infra.ArchiRuntime` | `new()` |
| `ProcessTestExecution` | `Infra.ChecksRuntime` | `new()` |
| `ProcessStaticCheck` | `Infra.ChecksRuntime` | `new()` |

**都只有零参构造器。** 契约如实记录 `new()` 不是缺陷，是代码形状的事实。

#### 策略：契约变薄时改代码，不改格式

若某元素的契约面过薄（无法支撑未来测试生成），处置是**改代码暴露真实逻辑**，不是放宽格式。

`Infra.ArchiRuntime` 的实例：真正可测的逻辑全在私有函数里

```
runner.rs   compose_text / entry_line / location / report_suffix / clip
report.rs   write_report (pub(crate)) / file_stem
```

而 `runner_test.rs` + `report_test.rs` 已有 **44 个测试**在测它们——说明这些逻辑的价值已被承认，只是没暴露成 API。做法：

```rust
impl LocalArchiCheck {
    pub fn new() -> Self { Self }

    /// 检查结果 → 人读文案（状态分支 + 违例明细 + 报告路径）
    pub fn compose_text(report: &ArchiDriftReport, report_ref: Option<&str>) -> String
}
```

契约面因此从「一个零参构造器」变成「`new()` + `compose_text()`」——后者有真实入参类型与返回，测试生成才有抓手。**不动实现逻辑，只改可见性 + 挪进 `impl` 块。**

这条与「存量债务不消解」（§2.10）不矛盾：后者指**不为合规而重构**（建 barrel、改根路径）；前者指**为契约有效性而暴露已存在的逻辑**。两者判据不同——前者是「格式追代码」，后者是「代码追上契约应有内容」。

---

## 5. 类型映射

### 5.1 语言默认转换策略（参考 JSON Schema → Rust 工业实践）

| 通用结构 | Rust | 转换逻辑 |
|---|---|---|
| `object`（具名） | `struct` | 每个具名 schema 一个独立 struct，建立名义性 |
| `enum` | `enum` | 不直接用 String；派生 `Deserialize` |
| `oneOf` / `anyOf` | 带负载 `enum` | `#[serde(tag = "...")]` 或 `untagged` |
| 非 `required` 字段 | `Option<T>` | 编译期空安全 |
| 字段位一般类型 | owned（`String` / `Vec<T>`） | 不引入生命周期参数 |

本仓实测印证最后一条：struct 字段 `String` : `&str` = **283 : 1**，`Deserialize<'de>` / `#[serde(borrow)]` **0 处**（零借用 DTO，生命周期不传染）。

### 5.2 位置分派：函数签名位按「对调用方要求最少」定

S→L 方向不成立（`string` 反推不出 `&str` 还是 `String`），故「宽松」不是类型属性，是位置化判据：

| 位置 | 规则形态 | 实测符合 / 偏离 | 保真度 |
|---|---|---|---|
| 入参位 `string` | `&str` | 95 / 3 | **97%** |
| 返回位 `string` | `String` | 64 / 28（剔除 25 处 `as_str` 后） | **96%** |
| 字段位 `string` | `String` | 283 / 1 | **99.6%** |
| 入参位 `list<T>` | `&[T]` | 15 / 1 | **94%** |

**偏离携带语义，不是噪声**：入参位那 3 例 `String`（`Store::new` / `agent::new` / `answer`）全部是**存储型**——借用形态本身是信号（`&str` = 用完即弃，`String` = 会被持有）。偏离须在契约登记理由。

`as_str(self) -> &'static str` 的 25 例可机械识别（`self` by value + `&'static str` + 名 `as_str`）。

位置分派规则是**生成器的内部表**，不是对外门禁。

### 5.3 TS 侧映射

生成物落在 `<element_path>/<module>.i.ts`（理由见 §1.3）。**只出类型与接口声明，不出实现**——实现侧手写并 `import type` 引用。

| 协议 | `.i.ts` 生成 |
|---|---|
| `object` | `export interface X { ... }` |
| `enum` | `export const X = [...] as const;`<br>`export type X = (typeof X)[number];` |
| `x-class`（trait） | `export interface X { ... }`（不强制实现 → 记 `x-common.degraded`） |
| 标量 newtype | `export type TimestampMs = number;`（**按设计退化，不记 degraded**） |
| `x-function` | `export type Fn = (...) => R` / `Promise<R>`（`async`） |
| 错误面 | `Promise` reject —— **不在类型系统内**（记 degraded） |

实现侧：

```ts
// <element_path>/checks.ts（手写）
import type { SummaryReport, Conclusion } from "./checks.i";

export function parseCargoTest(input: string): SummaryReport { /* ... */ }
export function allConclusions(): readonly Conclusion[] { return Conclusion; }
```

`Conclusion` 的 `const` 与 `type` 同名，`import type` 取类型、`import` 取值——两者都可用同一个模块路径。

**同名值/类型声明已实测通过**（`--strict --isolatedModules`）：

```ts
export const GateTier = ["off", "warn", "strict"] as const;
export type GateTier = (typeof GateTier)[number];
// const bad: GateTier = "nope";
//   → TS2322: '"nope"' is not assignable to type '"off" | "warn" | "strict"'
```

TS 有分离的值/类型命名空间，`typeof GateTier` 在类型位解析到值空间，不自引用成环。**数组本身就是枚举列举，无需额外 `_ALL` 名字。**

这条直接解决前端实测痛点：`run-step-node.test.tsx:228` 现手抄 22 个步词，可改为 `const allKinds = RunStepKind`。`Record<ChangeStepKind, string>` 只能保编译期穷举，运行期枚举必须手抄第二份。

注意事项：`isolatedModules` 下重导出须写 `export type { GateTier }`；`GateTier.Off` 变体名访问不可用（数组形态代价，生成物只关心 wire 值）。

**TS 不做品牌类型**：`number & { __brand }` 会导致 cast 传染整条调用链（`JSON.parse` 出裸 `number`），收益只是防误传，代价大于收益。TS 侧「实质同类型即可」。

### 5.4 类型集

| 协议 | Rust | TS |
|---|---|---|
| `string` | `&str` / `String` / `Cow<'_,str>` | `string` |
| `int` | `i8..i64` `u8..u64` `usize` `isize` | `number` |
| `float` | `f32` `f64` | `number` |
| `bool` | `bool` | `boolean` |
| `list<T>` | `&[T]` / `Vec<T>` / `[T;N]` | `T[]` |
| `dict<V>` | `BTreeMap` / `HashMap` | `Record<string,V>` |
| `optional<T>` | `Option<T>` | `T \| null` |
| `result<T,E>` | `Result<T,E>` | 见 §6 |
| `ref<X>` | `&X` / `Arc<X>` / `X` / `&dyn Trait` | `X` |
| `callback<P,R>` | 单方法具名 trait / 闭包 | 函数类型 |

**泛型明确不支持**（Rust 侧零泛型公开面、TS 侧无真泛型组件；500 模块规模下会让协议退化成需要类型推导）。

---

## 6. 错误面

不区分预期内 / 异常。错误是**返回值 schema 的一部分**，用约定格式：

```json
"Outcome": {
  "oneOf": [
    { "type":"object", "required":["ok"],  "properties":{"ok":{}},                   "x-common":{"tag":true} },
    { "type":"object", "required":["err"], "properties":{"err":{"type":"string"}},   "x-common":{"tag":true} }
  ]
}
```

| 语言 | 映射 |
|---|---|
| Rust | `Result<T, String>`（std 类型，不生成新类型） |
| TS | `{ ok: T } \| { err: string }`，或 `Promise` reject（须记 degraded） |
| Go | `(T, error)` |
| Python | 返回 union 或 raise（需 ADR 二选一） |

`common.json` 随工具发布，落地形态 `include_str!` 编译期内嵌（仓库 16 个文件在用该模式）。

**关键性质**：当前 `common.json` 只有 `Outcome` 信封，属**映射规则**而非**类型库**——生成时就地展开为各语言标准类型，**零生成物、零 vendor 文件、零版本声明**。风险只在它长出「带生成产物的类型」时出现：

> 一旦有人想往 `common.json` 加带生成产物的类型（而非映射规则），必须回炉重裁分发方案。

「工具升级 → 无预警批量重生成」的担忧已被 `git diff --exit-code` 消解（变更可见）。生成文件头带工具版本可分辨变更来源：

```rust
// 生成于 archi-api 0.4.2 · common.json@sha256:1a2b…
```

---

## 7. 留白与门禁

### 7.1 显式留白

`api` 为模型元素的**显式字段**（非 `metadata`），封闭二值（沿 `GateTier` 纪律，带 `ALL`）：

| 值 | 含义 | 门禁行为 |
|---|---|---|
| `declared`（缺省） | 有契约，契约文件必须存在 | 缺文件 → `apiMissing` |
| `none` | 显式声明本模块无 API | 有契约文件 → `apiUnexpected` |

**完备性判据**：每个非容器元素 ∈ {`none`} ∪ {契约文件存在}。不存在「忘了建契约」这第三种状态。

不用 `metadata` 的理由：显式字段买到 `deny_unknown_fields` 的拼写校验（`api = "non"` 报 `invalidValue` 并列全合法值；`metadata = { api = "non" }` 会静默变成「未声明」）。

### 7.2 十条判据与默认档

| 判据 | 方向 | 默认档 |
|---|---|---|
| `apiEntryMissing` | 元素无生成物 / 实现入口（§2.10 实测 27 条） | **`warn`** |
| `apiMissing` | 契约 → 代码（声明的 op 不存在） | **`warn`** |
| `apiUndeclared` | 代码 → 契约（有 external 面未声明） | **`off`**（唯一完备性判据，迁移期必须关） |
| `apiSignatureMismatch` | 双向 | **`warn`** |
| `apiUnexpected` | 留白 ↔ 文件矛盾 | **`warn`** |
| `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空（§4.5） | **`warn`**（编译器对 glob 形态只警告，必须自校验） |
| `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 未 declared / 非直接子元素（schema 层由 ajv 编译期捕获） | **`warn`** |
| `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate（Rust 侧不可表达） | **`warn`** |
| `apiReexportDrift` | markdown 散文列表与 `api-reexport` fence 键集不等 | **`warn`** |
| `apiConstructorDeclared` | 契约 `### 操作` 声明了构造器（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`）——构造器由生成器自动产出，不进契约（§10.2.1） | **`warn`** |

十条判据统一归入新枚举 `ArchiApiFindingKind`（与既有 `ArchiDriftKind` 分立，理由见 §7.4）。

**分界线**：一致性判据（契约说 A、代码是 B）可即开 `warn`；完备性判据（代码有的契约都得有）必须后置——`apiUndeclared` 是唯一后者。存量债务不消解的前提下，前者报的是「债」，后者报的是「海」。（§2.10）

**两个考虑过但删除的判据**（理由见 §4.5）：`apiChildToParent`（由既有 `UndeclaredDependency` 覆盖）、`apiLeakedDependency`（契约不是依赖事实源）。判据建在事实源（代码）上，不在投影（契约）上；新增判据只在「契约自身内部矛盾」时成立。

`apiMissing` 天然支持渐进——契约里只写已确定的 op 即可。`apiUndeclared` 是要求完备的那一档，长期 `off` 直到某元素真要收紧，这是「逐个收紧」的机械载体。

`gate` 基线 50/50 全 `off`。**「统一目录」与「gate 全 off」不能同时成立**：相邻 + off 尚有软提醒，统一 + off 是零提醒零校验 → 契约静默腐烂。持契约元素起点应为 `warn`。

### 7.3 生成能消灭的一类测试

封闭词表单源（`ALL` 常量）在生成模式下由 `enum` 派生 → 长度一致性**由构造保证**，对应 corpus 测试不再必要。

实测量：`pub enum` **78** 个，带 `pub const ALL` 仅 **7** 个，带 `as_str` **31** 个，两者都无 **47** 个。

**`singleSource` 构造已删除**：真正的完备性执行者是 `as_str` 的穷举 `match`（加变体必编译失败），枚举列举（`ALL`）只是 7 处的特例需求。函数式编程的既有答案一致——Haskell `deriving Enum`、Rust `strum::EnumIter` 都是**函数**；和类型不需要运行期列举，需要列举时由显式函数承担。

落地：需要列举时声明普通函数 `all() -> list<LocalEnum>`，**函数体由枚举派生**（语义条件，非标记）。迁移：7 处 `pub const ALL` → `pub fn all()`，消费点 `X::ALL` → `X::all()`。

---

### 7.4 分类：`api` 判据的词汇归属与扩展性

用户裁决：**先增加分类，预留扩展性。**

#### 分类维度：落在 `ArchiDriftKind` 还是新枚举？

`ArchiDriftKind` 是既有封闭四值（`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`），带 `ALL` 常量 + corpus 金样 + `vocabulary_test` 全量对拍。api 判据若要加入，两条路：

| 方案 | 形态 | 影响 |
|---|---|---|
| **A. 并入 `ArchiDriftKind`（4→12）** | 一个枚举承载全部漂移 | 现有 `match` 臂、corpus 金样、前端 `driftMarks` 全部要扩；且「模型漂移」与「契约漂移」混为一类，语义糊 |
| **B. 新增 `ArchiApiFindingKind` 平行枚举** | 两套封闭词汇并列 | 各自 `ALL` / 语料 / 判据独立演进；报告顶层新增 `apiFindings` 槽位 |

**采纳 B。** 理由：

1. **语义分层**——`ArchiDriftKind` 是「模型 vs 代码」的漂移；api 判据是「契约 vs 代码」的漂移。两者事实源不同（relation 图 vs 契约文件），混在一个枚举里会让 `match` 分支被迫处理不相关变体。
2. **扩展性**——用户要求「预留扩展性」。api 契约是新生能力，判据大概率还会长（组件 API、TS 侧校验等）。独立枚举意味着后续加变体不动既有 `ArchiDriftKind` 的任何 `match` / 金样 / 前端渲染。
3. **定级分离**——`ArchiDriftKind` 的定级看元素 `gate`（`off`/`warn`/`strict`）；api 判据的定级在迁移期一律 `warn`，将来才可能接 `gate`。两套定级语义不宜共用一个字段。

#### 新枚举定义

```rust
/// 契约判据种类（封闭十值；线格式 camelCase）。
///
/// 与 [`ArchiDriftKind`] 分立：那边是「模型 vs 代码」，这边是「契约 vs 代码」。
/// 两套 `ALL` 各自对拍，新增变体不同步即语料红。
pub enum ArchiApiFindingKind {
    /// 元素有契约声明但契约文件缺失
    ApiMissing,
    /// 元素根路径下无生成物 / 实现入口
    ApiEntryMissing,
    /// 代码有 external 面但契约未声明（唯一完备性判据）
    ApiUndeclared,
    /// 契约签名与代码签名不一致
    ApiSignatureMismatch,
    /// 留白（`api = none`）与契约文件存在矛盾
    ApiUnexpected,
    /// 父 `$defs` 与子元素 `$defs` 类型名交集非空
    ApiShadowing,
    /// 跨模块 `$ref` 目标缺失 / 未 declared / 非直接子元素
    ApiRefMissing,
    /// 嵌套元素父子分属不同 crate
    ApiNestedCrossCrate,
    /// markdown 散文列表与 `api-reexport` fence 键集不等
    ApiReexportDrift,
    /// 契约声明了构造器（应留空，由生成器自动产出）
    ApiConstructorDeclared,
}

impl ArchiApiFindingKind {
    pub fn as_str(self) -> &'static str { /* camelCase 词 */ }
    pub const ALL: [ArchiApiFindingKind; 10] = [..];
}
```

#### 扩展预留的三条设计

**1. `category` 为开值集，不设 `ALL`**

「分类」用于报告分组（§2.10 的 27+40 条按类聚合），但它不同于判据 `kind`：

| | `kind` | `category` |
|---|---|---|
| 值域 | **封闭**（`ALL` + 语料对拍） | **开放**（字符串，无 `ALL`） |
| 谁定义 | 本能力 | 各判据自行标注（可随时新增分类而不改枚举） |
| 消费 | `match` 分支 | 报告渲染分组键 |

```rust
pub struct ArchiApiFinding {
    pub kind: ArchiApiFindingKind,
    /// 分组键（开放值集：`entry` / `contract` / `signature` / `reexport` / `nesting` …）
    /// 渲染层按本字段聚合；新增分类 MUST NOT 要求改枚举
    pub category: String,
    pub element: Option<String>,   // 目标元素 FQN（对应既有 from）
    pub target: Option<String>,    // 关联元素 FQN（对应既有 to）
    pub message: String,
    pub occurrences: Vec<ArchiDriftOccurrence>,
}
```

**`category` 用 `String` 而非枚举**，是「预留扩展性」的直接兑现：加一个分类只改生成侧的打标，不动类型、不动 `ALL`、不动金样的词汇对拍。代价是拼错不报错——由生成器单点产出 + 契约测试覆盖已知分类来控。

**2. `ArchiDriftReport` 新增并列槽位，不改既有字段**

```rust
pub struct ArchiDriftReport {
    // …既有 14 字段全部不动…
    /// 契约判据结果（与模型漂移并列；空数组 = 无契约问题）
    pub api_findings: Vec<ArchiApiFinding>,
}
```

新增字段而非改造 `errors` / `warnings`，好处：

- 既有 corpus 金样的 `errors` / `warnings` / `suppressed` / `observations` 逐字不变，金样只需**追加** `apiFindings: []`；
- 既有前端 `driftMarks` 只读 `observations` / `suppressed`，零改动；
- 报告 `version` 递增（`n → n+1`）并留兼容说明。

**3. 定级目前不进 `status`**

首期 `api_findings` **不影响 `status` / `passed`**——与 `observations` 同款（「永不影响 status」的既有先例）。原因：迁移期 67 条 warn 若进 `status`，门禁会立刻变红，违背「不消解债务」的前提。

将来收紧时，`api_findings` 的严重度由 `ArchiApiFindingKind` 自带的 `severity()` 决定（而非元素 `gate`），这样契约判据与模型判据的收紧节奏解耦。

#### 报告分组形态

```json
{
  "apiFindings": [
    { "kind": "apiEntryMissing", "category": "entry",    "element": "App.Frontend.Views.AgentDebug", … },
    { "kind": "apiEntryMissing", "category": "entry",    "element": "App.Frontend.Views.Agents",   … },
    { "kind": "apiMissing",      "category": "contract", "element": "App.Backend.Infra.Exec",      … }
  ]
}
```

渲染层按 `category` 聚合，得到「同一类债 N 条」的分组视图，而不是 67 行平铺。`category` 的初始取值集（实现时定，非类型约束）：

| category | 含义 | 归属判据 |
|---|---|---|
| `entry` | 模块根路径 / 入口 / 留白 | `apiEntryMissing` / `apiUnexpected` |
| `contract` | 契约文件与声明 | `apiMissing` / `apiUndeclared` |
| `signature` | 签名一致性 | `apiSignatureMismatch` |
| `reexport` | 透传 | `apiReexportDrift` / `apiShadowing` / `apiRefMissing` |
| `nesting` | 嵌套结构 | `apiNestedCrossCrate` |

#### 连带改动面

| # | 位置 | 改动 |
|---|---|---|
| 1 | `core/archi/src/drift/report.rs` | 新 `ArchiApiFindingKind`（9 值 + `ALL` + `as_str`）+ `ArchiApiFinding` + `ArchiDriftReport.api_findings` |
| 2 | 同上 | `ArchiDriftReport.version` 递增 |
| 3 | `core/archi/tests/vocabulary_test.rs` | 新词汇组对拍（9 值） |
| 4 | `core/archi/tests/corpus/drift/expected/*.json` | 追加 `"apiFindings": []`（现有语料无契约，恒空） |
| 5 | 新增 corpus | 至少一个含 `apiFindings` 的非空金样 |
| 6 | `src/types/generated/bindings.ts` | 再生成 |
| 7 | `infra/agent/src/archi_knowledge.rs` | 派生段增 api 契约相关词表（若纳入） |

既有 `errors` / `warnings` / `suppressed` / `observations` 与 `driftMarks` **零改动**，这是方案 B 的直接收益。
---

## 8. 存量实测数据

### 8.1 表面规模

| crate | pub 类型 | pub 函数 | 类型占比 |
|---|---|---|---|
| `core/foundation` | 4 | 24 | 14% |
| `core/config` | 13 | 1 | 93% |
| `core/workflow` | 73 | 32 | 70% |
| `core/archi` | 67 | 48 | 58% |
| `core/orchestration` | 55 | 43 | 56% |
| `core/agent` | 41 | 25 | 62% |
| `core/checks` | 21 | 28 | 43% |
| `infra/agent` | 10 | 31 | 24% |
| `infra/checks` | 3 | 4 | 43% |
| `infra/exec` | 11 | 28 | 28% |
| `infra/store` | 26 | 80 | 25% |
| `infra/vcs` | 2 | 3 | 40% |
| `infra/watch` | 3 | 1 | 75% |
| `infra/archi` | 1 | 1 | 50% |
| **合计** | **330** | **349** | **49%** |

**契约覆盖率随模块性质剧烈波动**：`core/foundation` 的 API 几乎全是函数（schema 只捕获 4 个类型），`core/config` 93% 是类型。不是「统一变简单」，而是数据型模块受益、行为型模块失血。

### 8.2 三条裁剪

- **349 `pub fn` − 51 构造器 = 298 真实操作**。构造器是类型的推论（有 `SummaryReport` 就有 `::new`），生成器自动产出，不进契约。
- **5 个 `pub type` 全是 `Pin<Box<dyn Future>>` 管道**，不进契约。
- **`pub static` = 0**，不需要 static 构造子。

契约面比 `pub` 面小约 15%，且这 15% 由生成器接管。

### 8.3 结构体形态

| 形态 | 数量 |
|---|---|
| 纯数据 struct（仅属性） | **170** |
| 纯行为 struct（字段私有，仅方法） | **23** |
| 混合（属性 + 方法） | **20** |

混合体的 69 个方法里 **36 个（52%）是构造器**，另 33 个是行为方法，其中 9 个 `node_seq`/`node_id`/`edge_seq` 是 db 键派生、3 个是字段透传 —— **全是机械访问器**。

「struct 限制」约束的是**生成物**，不是手写代码：方法从来不在 schema 里，一直在 `_ext.rs`。**零重构。**

### 8.4 trait

18 个 `pub trait`：**全部只有方法，零关联类型、零关联常量** →「仅允许公共方法」100% 满足。

方法带 `&self` 的比例：`ChangeStateStore` 14/17、`ArchiveVcsPort` 14/14、`WorkflowSnapshotPort` 1/1、`SourceParser` 0/1（静态）。

跨模块引用密度：`WorkflowSnapshotPort` 176 次、`ChangeStateStore` 85 次、`ArchiveVcsPort` 44 次。

### 8.5 私有字段 class

14 个，其中 **13 个未实现任何 trait**（仅 `Store` 实现了 `ChangeStateStore`）。

真实跨模块使用（排除注释误报）：

| 类 | 使用范围 | 是否模块 API |
|---|---|---|
| `SystemCommand` | infra/agent, checks, exec, vcs | **是** |
| `RunHandle` | core/agent, infra/agent | **是** |
| `ModelGraph` / `RunGuard` / `OwnerIndex` / `ChangeFlowControl` | 各自模块内 | 否 |

7 个是模块内部实现细节，按 §4.4 的闭包约定**不进契约**。剩余 2 个单实现跨模块类，不为它们造 trait —— 违反 `AGENT.md` 编码准则「No abstractions for single-use code」，且 `SystemCommand` 在热路径。

### 8.6 名义类型需求

`i64`/`u64` **字段 162 处**，按字段名聚合：

| 语义簇 | 字段 | 合计 |
|---|---|---|
| **时间戳（毫秒）** | `started_at` 15、`timestamp` 11、`created_at` 10、`finished_at` 7、`updated_at` 7、`start_at` 7、`ended_at` 5 | **62** |
| db 记录 id | `id` 15、`turn_id` 3 | 18+ |
| 计数 / token | `total` 7、`token_input` 5、`token_output` 5、`num_turns` 4 | 21+ |
| 时长 | `duration_ms` 4 | 4 |

标量 newtype 的真实价值在此（让毫秒时间戳与 db id 不可互换），而非字符串 Newtype（仓库 **0 个 newtype**、具名类型字段 803 处——名义性已饱和）。这些应放**各模块 `$defs`**，不进 `common.json`（否则触发 §6 的再评估点）。单位须进名称或 `x-common.unit`（`started_at` 未标单位是隐患）。

### 8.7 模块可见性：四种策略混用

| 策略 | crate | 对外可达 pub | 仅 crate 内 |
|---|---|---|---|
| `pub mod` 全开 | `core/foundation` | 30 | 0 |
| `pub mod` + 精选 `pub use` | `core/archi`·`orchestration`·`checks`·`workflow`·`infra/checks` | **383** | 40 |
| 私有 `mod` + 白名单 `pub use` | `core/agent`·`infra/store`·`exec`·`watch`·`archi` | 20 | **145** |
| 直接定义在 `lib.rs` | `core/config`·`infra/vcs` | 20 | 0 |
| **合计** | | **433** | **251** |

`core/archi` 的 `pub mod scan;` + `scan/mod.rs: pub mod rust;` 使 `RustResolver` 等 123 个 pub 项全部是 crate 对外 API，含纯内部工序。**没有任何 spec 约束这层策略**，四种风格是历史沉积。bootstrap 剪枝的主战场在此。

### 8.8 与测试的关系

146 个 `*_test.rs` / 174 个非测试 `.rs`；源码内零 `#[cfg(test)] mod tests`（测试一律 `#[cfg(test)] mod x_test;` 挂在 crate 根，兄弟于被测模块）。

因测试挂在 crate 根，**只能触 `pub` / `pub(crate)`** —— 今天的 per-file 测试实质上已是「文件级 API 测试」。换成模块 API 测试掉的不是覆盖率，而是 `test-design.md` 里「源文件 → 测试文件」的定位映射。

`infra/agent`（44 `pub` / **59 `pub(crate)`**）、`infra/checks`（14 / **42**）、`infra/vcs`（6 / **25**）三处 `pub(crate)` 远多于 `pub` —— `visibility = crateOnly` 这一档正是为此存在。

---

## 9. 完整示例

````markdown
# App.Backend.Core.Checks

FQN: `App.Backend.Core.Checks` · 由 `models/02-backend-core.toml` 声明

## 职能

测试报告语义与检查结论的领域规则。

**边界**：不 spawn、不读盘、不写盘 —— 进程侧在 `App.Backend.Infra.Checks`。

## 操作

### parse_cargo_test

cargo test 文本输出 → 结构化用例集；多个 `running` 块累加。

- 入参 `input`：UTF-8 文本；非 UTF-8 即 `Err`
- 错误面：`running` 头格式非法时 `Err`
- 不变量：`skipped` 不计入 `total`

```json api-op
{
  "params": [{ "name": "input", "schema": { "type": "string" } }],
  "returns": { "$ref": "#/$defs/SummaryReport" }
}
```

### all_conclusions

结论枚举的穷举清单；**函数体由 `Conclusion` 枚举派生**，加变体必编译失败。

```json api-op
{
  "params": [],
  "returns": { "type": "array", "items": { "$ref": "#/$defs/Conclusion" } }
}
```

## 类型

```json api-types
{
  "Conclusion": {
    "type": "string",
    "enum": ["pass", "fail", "error"],
    "x-common": { "note": "定型优先级：execution_error > failed > 阈值比较" }
  },
  "SummaryReport": {
    "type": "object",
    "properties": {
      "total":      { "type": "integer" },
      "conclusion": { "$ref": "#/$defs/Conclusion" }
    }
  }
}
```
````

生成的 `<FQN>.schema.json`（供生态工具与校验器消费，不人审）与 Rust 侧：

```rust
// api_gen/checks.rs（生成）
pub fn parse_cargo_test(input: &str) -> Result<SummaryReport, String>;
pub fn all_conclusions() -> &'static [Conclusion] {
    &[Conclusion::Pass, Conclusion::Fail, Conclusion::Error]
}

pub enum Conclusion { Pass, Fail, Error }
pub struct SummaryReport { pub total: u64, pub conclusion: Conclusion }
```

```rust
// checks_ext.rs（手写，编译器验签名）
impl Conclusion {
    pub fn as_str(self) -> &'static str { /* 穷举 match */ }
}
```

---

## 10. 待决与边界

### 10.1 spec 双写过渡

`## Module Contract` 暂不处理，双写过渡。

`## Module Contract` 表长这样（`desktop-checks-domain` 实测）：

```markdown
| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/checks`（新，裸名 `checks`） | 检查域纯层 | model / parser / aggregate / diagnose / reuse 纯函数；零 spawn 零 tokio process 零 Tauri；依赖 config + foundation；fixtures 直驱 + corpus 黄金可测 |
```

三列、文件级粒度、无签名。与 api 契约（FQN 粒度、有签名、语言无关）**不是同一件事**，双写不构成「第二份手写真相」——它们是两个不同粒度的视图：

| | spec `## Module Contract` | api `<FQN>.md` |
|---|---|---|
| 粒度 | 文件路径 | 元素 FQN |
| 内容 | 职责 + 边界 + 关键契约（散文） | 操作签名 + 类型 + 不变量 |
| 生命周期 | 能力级，归档合并 | 模块级，随代码 |
| 消费者 | proposal-planner 强制产出 | 生成器 + 门禁 |

双写期的**唯一纪律**：不要求两处逐字一致（粒度和内容本就不同），但要求**语义不冲突**——spec 说「零 spawn」时，api 契约里不得出现 `std::process`。这条靠 review 保证，不机械校验。

双写退出条件：待 §10 第 3 项（46 份 spec 处置）另案裁决。退出时 spec 用 `REMOVED Requirements` 或 `MODIFIED` 指向 api 资产。

### 10.2 试点元素：`App.Backend.Infra.ArchiRuntime`

14 个 Rust crate 元素全部命中 `<dir>/src/lib.rs`。按公开面规模排序后选最小的：

| 候选 | pub 项 | pub(crate) | 形态覆盖 |
|---|---|---|---|
| **`infra/archi`（archi-runtime）** | **2** | 1 | ✓ 类型 + 函数 + trait impl + 私有字段 class |
| `infra/watch` | 4 | 0 | 类型 + 函数，无 trait impl |
| `infra/vcs` | 5 | 25 | 大量 pub(crate)，不适合首批 |
| `core/foundation` | 9 | 0 | 纯函数为主，形态偏窄 |

`App.Backend.Infra.ArchiRuntime` 的公开面全貌：

```rust
// lib.rs（1 行 pub use）
pub use runner::LocalArchiCheck;

// runner.rs（2 项）
pub struct LocalArchiCheck;
impl LocalArchiCheck { pub fn new() -> Self }
impl Default for LocalArchiCheck { .. }
impl ArchiCheckRunner for LocalArchiCheck { fn run(..) -> BoxToolFuture }

// report.rs（1 项 pub(crate)）
pub(crate) fn write_report(..) -> Result<String, String>
```

**选它的四条理由**：

1. **规模最小**（2 个 pub 项），试点暴露的格式问题不会被数量淹没。
2. **形态齐全**——含 `pub struct`（零字段）、`pub fn`（构造器）、`trait impl`（`ArchiCheckRunner`）、`pub(crate) fn`（不进契约）。四种判定都能跑一遍。
3. **构造函数返回 `Self`**，可验证「构造器由生成器自动产出，不进契约」这条裁剪规则。
4. **`impl ArchiCheckRunner for LocalArchiCheck`** 是「跨模块 trait 作参数类型」的真实案例（`ArchiCheckRunner` 定义在 `core/orchestration`），可验证 §4.5 的跨模块 `$ref` 指向规则。

**预期试点产出**：

```
.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md      契约（手写，bootstrap 起草）
.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json  生成
packages/desktop/src-tauri/crates/infra/archi.i.rs                      生成（<root>.i.rs）
```

以及 `archi_module` 投影新增 `api` 字段、`app.backend.infra.archiruntime` 元素 `api = "declared"`。

**试点可能的代码改动**：若契约面仅剩 `new()`（零参构造器，测试价值低），须按 §4.6 暴露已存在的私有纯函数（`compose_text` 等）——只改可见性，不动实现逻辑，也不动格式。

**试点要回答的三个问题**（决定后续铺开是否可行）：

1. `BoxToolFuture = Pin<Box<dyn Future<Output = Result<ToolStepOutput, String>> + Send>>` 这类**类型别名**如何表达？（§8.2 裁决「5 个 pub type 不进契约」，但它是 `ArchiCheckRunner::run` 的返回类型——不进契约则签名表达不完整）
2. `impl Default` 与 `impl ArchiCheckRunner` 两个 impl 块的归属：前者由生成器产出、后者手写，`#[path]` 引入后是否冲突？
3. `pub(crate) fn write_report` 的 `visibility = crateOnly` 是否应进契约（本 crate 内它被 `runner.rs` 调用，但外部不可达）？


### 10.2.1 试点纸面验证结果

用 `App.Backend.Infra.ArchiRuntime` 实写契约草案并按格式全链路跑了一遍。发现 **1 个格式缺口 + 3 项确认**。

#### 缺口：构造器在契约里的位置（已修正草案）

首版草案把 `new` 写成 `### new` + `api-op`。跑 `apiUndeclared` / `apiMissing` 对拍时报出：

```
契约声明集: ['LocalArchiCheck', 'compose_text', 'new']
代码导出集: ['LocalArchiCheck', 'compose_text']
apiMissing: ['new']        ← 误报
```

根因：§8.2 已裁定「构造器由生成器自动产出，不进契约」，但格式里没有阻止它被写进 `### 操作` 的机制。

**修正**：从草案删掉 `### new` 段，只保留 `compose_text`。生成器从 `$defs.LocalArchiCheck`（零字段 struct）自动产出 `new()`。

**需补的格式规则**：契约 `### 操作` MUST NOT 声明构造器（`new` / `default` / `from_*` / `build` / `with_*` / `empty` / `never` / `open`）。违反者报 `apiConstructorDeclared`（契约自身矛盾，属新增判据的合法范围）。

构造器因此**双向都不参与判据比对**：既不在契约声明集，也不计入 `apiUndeclared` 的代码导出集——它是生成器的固有产物。

#### 确认一：双向 gate 可机械执行

按 `### <op-id>` 分段提取 `api-op` fence，heading 集合与 fence 归属一一对应：

```
heading 提取: ['new', 'compose_text']  →  修正后 ['compose_text']
fence 提取:   ['compose_text']
✓ 双向 gate 通过
```

`api-op` fence 是**归属于前面 heading 的**，fence 内不自包含 op 名（不重复）。提取器按 heading 分段即可，实现简单。

#### 确认二：schema.json 生成与校验行为符合预期

生成产物用 Ajv2020 实测：

| 检查 | 结果 |
|---|---|
| 直接编译（子模块 schema 不存在） | ✓ 通过——`x-function` 内的跨模块 `$ref` 未被解析 |
| `strict: true` | ✗ `unknown keyword: "x-function"`（须 `strict: "log"` / `false`） |
| `x-function` 内悬空 `$ref` | 未被解析（符合 §4.5 的边界结论） |

#### 确认三：生成的 Rust 与手写 impl 共存编译

```rust
// archi.i.rs（生成）
pub struct LocalArchiCheck;
impl LocalArchiCheck {
    pub fn new() -> Self { Self }
    pub fn compose_text(report: &str, report_ref: Option<&str>) -> String { .. }
}

// lib.rs（手写）
#[path = "archi.i.rs"]
mod api_gen;
pub use api_gen::LocalArchiCheck;
impl Default for LocalArchiCheck { fn default() -> Self { Self::new() } }
```

编译通过（`rustc --edition 2021`），外部可 `LocalArchiCheck::new()` / `compose_text(..)` 引用且签名受约束。

#### 判据对拍结果

| 判据 | 试点实测 |
|---|---|
| `apiEntryMissing` | 不报 ✓（`<dir>/src/lib.rs` 命中） |
| `apiUndeclared` | 无 ✓ |
| `apiMissing` | 无 ✓（删掉 `new` 后） |
| `apiShadowing` | 不适用（无子元素） |
| `apiNestedCrossCrate` | 不适用（无子元素） |
| `apiReexportDrift` | 不适用（无透传） |

#### 试点要回答的三个原问题

| # | 问题 | 验证结论 |
|---|---|---|
| 1 | `BoxToolFuture` 类型别名如何表达？ | **不需要表达**——`ArchiCheckRunner::run` 的签名归 `Core.Orchestration` 的契约，本元素不重复（§4.6） |
| 2 | `impl Default` 与 `impl ArchiCheckRunner` 归属？ | `impl Default` 手写、与生成物共存编译通过；`impl ArchiCheckRunner` 是跨元素依赖，进模型 relation 不进契约 |
| 3 | `pub(crate) fn write_report` 进不进契约？ | **不进**——`visibility = crateOnly` 且非跨模块承诺 |

### 10.3 未决（真未定项）

1. **`common.json` 不兼容升级**——随工具发布，遇不兼容时再处理（届时需版本声明与语义冻结规则）。
2. **封闭词表测试删除**——生成派生后理论可删，暂缓决策。
3. **非组件前端元素是否同期生成 `.i.ts`**——组件形态元素本期 `api = "none"`；hooks / lib 导出函数（实测 `export function` 21 处）是否纳入待定。
4. **`degraded` 是否收紧为封闭 reason 枚举**——自由文本无法聚合，INDEX 的「降级计数」会失效；但封闭枚举与「注记自由扩展」冲突。
5. **`knip.json` 需增生成物 ignore**——实测未被 import 的 `.i.ts` 会被报为 unused file（`project` glob 覆盖 `src/**/*.ts`）。生成物路径须入 `ignore`，与既有 `src/types/generated/**` 同处理。
6. **`api` 字段规格落地的 9 处改动**——`dto.rs`（`ApiStatus` + `ArchiElement.api`）、`parse/toml.rs`（`RawElement.api`）、`model.rs`（`Element.api`）、`archi_knowledge.rs`（派生段）、`archi-knowledge.md`（字段面 + 契约权威表述）、corpus 语料、`desktop-archi-model` MODIFIED delta、`bindings.ts` 再生成。

### 10.4 已定（不再作为开口）

| # | 裁定 | 落点 |
|---|---|---|
| 1 | 46 份 spec `## Module Contract` 暂不处理，**双写过渡** | §10.1 |
| 2 | bootstrap **先挑一个元素试点**：`App.Backend.Infra.ArchiRuntime` | §10.1 |
| 3 | **存量代码债务不消解**，判据全 `warn` 以验证检查器 | §2.10 |
| 4 | 前端代码漂移（多入口 / 无 barrel / 根路径带扩展名）不处理 | §2.10 |
| 5 | `infra/agent` 3 处子→父依赖不消解（`sdk/` 不提升为元素） | §4.5 |
| 6 | 壳元素根路径不上移 | §2.10 |
| 7 | 前端单文件元素根路径不去扩展名 | §2.10 |

### 10.5 明确不做

- **存量代码债务消解**：不重构、不改名、不建 barrel、不挪类型（用户裁定）。债务只作检查器的验证语料（§2.10），报告准确即达目的。
  - 例外（非债务消解）：为让契约有效而**暴露已存在的私有逻辑**（改可见性 + 挪进 `impl`），不是重构。判据见 §4.6 末段。
- **组件 API**：本期留白，另起探索。组件形态元素标 `api = "none"`。协议类型集因此暂不含 `union` / `element` / `extends` 等组件专属构造子。
- **基于 API 生成单测**：本期不做。但格式必须保住输入：错误面（`Outcome` envelope）若不显式，生成器会以为没有错误分支，漏掉整条异常路径。
- **泛型**：明确不支持。

### 10.6 与既有 spec 的冲突（须在提案中正面回答）

`desktop-archi-model`「架构模型文件格式与字段封闭」有防误加字段的边界：

> **协议不经模型**：元素上 MUST NOT 出现接口 / 导出符号 / 签名一类协议字段。契约权威在 `openspec/specs/` 与 ADR；模型内再存一份必然漂移，且需要 Rust / TS 两套提取器对齐——该方案已被否决，重提须独立变更并先回答「契约权威在哪」与「两套提取器怎么对齐」两问。

本设计的回答：

| 问题 | 回答 |
|---|---|
| 契约权威在哪 | 在 `<archi_dir>/api/<FQN>.json`。模型只存**状态指针** `api = declared｜none`，不含签名 / 符号 / 接口正文 |
| 两套提取器怎么对齐 | **不提两套**。单一 schema + 生成器；`scan/rust.rs` / `scan/ts.rs` 是既有取数器（非权威），只供 bootstrap |

### 10.7 设计禁区

列出这些是为了让后来者不必重新论证；不是历史记录，是**约束**。

| 禁区 | 理由 |
|---|---|
| 自造协议记法（`[op.xxx]` 信封 / `module://` scheme） | JSON Schema 有跨语言验证器 / IDE / diff / 生成器；自造记法只有自己的解析器。且 ajv 实测不认自定义 scheme |
| L/S 双层锁 + 归一函数 + 偏离登记表 | `verify ≡ generate + 类型检查`；比较就是 `git diff`，不需要第三层比对机制 |
| Zod 作通用载体 | TS 生态绑定；JSON Schema 原生 `x-*` 规范保证被未知关键字忽略 |
| `x-interface` / `x-nominal` / `x-singleSource` 独立构造 | 分别由 `x-class`、标量形状、函数返回消解（构造从 9 降到 2） |
| 生成物按 crate 聚合 | 架构层不得依赖语言概念；按模块落盘，diff 粒度不串味 |
| 按结构去重类型 | 17 组同形不同义（`ArchiDependsOn` vs `ArchiDependedBy`），合并即语义错 |
| api 与 doc 分成两个文件配对 | 配对靠纪律，必然漂移（`AGENT.md`/`CLAUDE.md` 已分化）。合写成一份 |
| 契约跟代码走（元素主目录） | 审计型 review 不可替代；伴随型已由门禁接管 |
| TS 品牌类型 | cast 传染整条调用链，收益只是防误传；TS 侧「实质同类型即可」 |
| 禁止匿名回调 | 改为「回调须具名」——匿名 `Fn` 无法枚举输入域，测试生成无抓手 |
| 全局禁止 Fn 字段（推论：`x-function` 可删） | 函数指针结构体是合法意图，与 trait 形状相同，故 `x-function` / `x-class` 须保留 |
| 契约层新增依赖检查判据 | 契约不是依赖事实源；既有 `UndeclaredDependency` 已覆盖（§4.5） |
| 在契约里记录「实现了谁的 trait」（`implements` 注记） | 契约记录「提供什么」不记录「实现了什么」；relation + 编译器双重覆盖（§4.6） |
