# desktop-archi-file-tree Specification（delta: frontend-barrel-entry-migration）

## Purpose

本 delta 只改**一处叙述实例**，归属判据（模块根表 / 五值定档 / 判据顺序 / 三候选剥离 / 判定单源）**逐字不动**（本变更**零 Rust 改动**）：

1. **实例翻新**：`App.Frontend.Components.Agent` 已由「`kind = domain` 但持 barrel 文件型 `path`」翻转为「`kind = module` 持目录型 `path`（`components/agent/`）」。故「元素根判据是 `path` 非空而非 `kind`」这条**判据措辞保留**，举例改为「本仓现基线无一例」——判据只读 `path`、`kind` 不参与定档这一结论不变。
2. **容器零节点的前提仍成立**：ADR `§3` 的「容器 `path` 恒空」由「唯一例外」改为**零例外**，故「`path` 恒空的容器 MUST NOT 出现在文件树上」逐字有效。
3. **新增一条实例化 Scenario**：`components/agent/` 目录节点为 `elementRoot`（owners = 目录模块），其 7 个子元素文件为各自元素的 `elementRoot`、`index.tsx` 为 `member`——把「目录型 `path` + 子元素文件型 `path`」的最长前缀分工钉成机械判据（`desktop-frontend-module-entry` 的 7 条端点边归属即依赖同一分工）。

## MODIFIED Requirements

### Requirement: 归属判定与判定单源

文件树的每个节点 SHALL 按**与架构漂移同一实现**的归属判据定档，树侧 MUST NOT 自建第二套归属 / 前缀匹配 / 生成物剥离：

- **模块根表**：SHALL 由全部 `path` 非空元素展开为 `norm_path → Vec<fqn>`，归一 SHALL 复用 `OwnerIndex::normalize` 同一实现（`./` 前缀剥除、尾 `/` 剥除、`\` → `/`）；MUST NOT 另写一份归一。
- **判据顺序**（逐节点）：
  ```
  ① 节点 path ∈ 模块根表：
       该表项恰 1 个 FQN              → elementRoot，owners = [该 FQN]
       该表项 ≥ 2 个 FQN（同路径多元素声明）→ ambiguous， owners = 全部 FQN（字典序）
  ② 否则 OwnerIndex::owner_of(path)：
       Element { fqn }                → member，      owners = [fqn]
       Ambiguous(fqns)                → ambiguous，   owners = fqns
       None                           → contains > 0 ? containsElements : none
  ③ contains(p) = 模块根表中 path 严格以 "p/" 开头的元素数（按 FQN 去重，不含自身——自身 path 恰为 p，不满足严格前缀）
  ```
- **元素根判据是 `path` 非空而非 `kind`**：定档只读 `path`，`kind` 不参与（`domain` / `package` 元素只要 `path` 非空即照常判 `elementRoot`）。本仓现基线**无「`domain` 但 `path` 非空」实例**——原 `App.Frontend.Components.Agent`（持 barrel 文件型 `path`）已随 `frontend-barrel-entry-migration` 翻转为目录型 `module`，其目录 `path` 与 12 个目录模块入口同形。
- **多 `path` 元素**：一个元素的每条 `path` 条目 SHALL 各出 `elementRoot` 节点（每个节点 `owners` 恰一条该 FQN）；`contains` 按 FQN 去重，同一元素 MUST NOT 被重复计数。
- **`elementRoot` 的 owners 恰 1**：同一归一路径被 ≥ 2 个元素声明时该节点 SHALL 判 `ambiguous`（`owners` ≥ 2），MUST NOT 从中任选一个给出 `elementRoot`——该形态与「同长不同前缀」同为归属不确定，两者都不得任选。
- **嵌套最长前缀**：父子 `path` 前缀重叠时，内层文件 SHALL 归最深前缀者（既有最长前缀语义），父子各出各自 `elementRoot`。
- **容器元素零节点**：`path` 恒空的容器元素 MUST NOT 出现在文件树上（这是 `decisions/0001-model-granularity.md` §3 的直接后果，不是遗漏；§3 的 barrel 例外已退场、现为零例外）；容器只在元素树（FQN 前缀派生）上可见。
- **`ambiguous` 必须实现**：同长多命中 SHALL 记 `ambiguous` 并保留全量候选于 `owners`，MUST NOT 任选其一；本仓预期恒 0，但分支 MUST 实现并由合成用例覆盖（不实现即退化成「任选其一」，与判定单源纪律冲突）。
- **生成物归属沿用三候选并集判据**：`.i.<ext>` 生成物的归属 SHALL 由 `OwnerIndex` 的「整组剥离 ∪ 替换扩展名 ∪ **主干对齐**（与文件型 `path` 的去扩展名形态全等）」三候选并集判据给出（定义与断言面见 `desktop-archi-api`「生成物落点与入口查找」与 `desktop-archi-drift`「文件归属」），树侧 MUST NOT 另加剥离规则（否则树与漂移对同一文件给出不同归属）。生成物落点已改为**入口同级**：目录型根下仍在元素根之内（`crates/infra/archi/src/lib.i.rs`、目录模块入口 `components/agent/index.i.ts`），文件型根下是根的兄弟文件（`views/welcome-view.i.ts`），后者只有主干对齐候选能命中。

#### Scenario: 目录型元素根与其内部文件

- **WHEN** 元素 `App.Core.Archi` 的 `path = ["./crates/core/archi/"]`，盘上该目录含 `src/lib.rs` 与 `src/dto.rs`
- **THEN** `crates/core/archi` 节点 `mark = elementRoot`、`owners = ["App.Core.Archi"]`；`crates/core/archi/src` 与两个 `.rs` 文件 `mark = member`、`owners = ["App.Core.Archi"]`

#### Scenario: 目录模块根与其子元素的文件根并存

- **WHEN** 目录模块 `App.Frontend.Components.Agent` 的 `path = ["./packages/desktop/src/components/agent/"]`，同目录下 7 个组件文件各是一个子元素的文件型 `path`，另有入口 `index.tsx`
- **THEN** `packages/desktop/src/components/agent` 目录节点 `mark = elementRoot`、`owners = ["App.Frontend.Components.Agent"]`（`contains` 计 7 个子元素）；7 个组件文件各判 `elementRoot`、`owners` 为其子元素；`index.tsx` 判 `member`、`owners = ["App.Frontend.Components.Agent"]`（无子元素认领它，最长前缀归目录模块）——该分工与 `desktop-frontend-module-entry` 的 7 条 barrel 端点边归属同源

#### Scenario: 单文件型元素根

- **WHEN** 元素 `App.Frontend.Views.Welcome` 的 `path = ["./packages/desktop/src/views/welcome-view.tsx"]`
- **THEN** 该文件节点 `mark = elementRoot`、`owners = ["App.Frontend.Views.Welcome"]`；同目录其余文件为 `member` 或按各自归属定档

#### Scenario: 多 path 元素与 contains 去重

- **WHEN** 元素 `App.Frontend.Shell` 声明四条文件形 `path`，且其中一条位于某祖先目录下
- **THEN** 四条各出 `elementRoot` 节点（`owners` 均为该 FQN）；该祖先目录的 `contains` 只计 1（按 FQN 去重），MUST NOT 计 4

#### Scenario: 嵌套父子各出根且内层文件归最深

- **WHEN** 父元素 `path = ["./crates/core/archi/"]`、子元素 `path = ["./crates/core/archi/src/scan/"]`，盘上 `crates/core/archi/src/scan/owner.rs` 在场
- **THEN** 两个目录节点各为 `elementRoot`（各有其 `owners`）；`owner.rs` 归子元素（最深前缀），父元素目录的 `contains = 1`

#### Scenario: 容器元素零节点

- **WHEN** 树构建输入含 `path` 为空数组的容器元素（`domain` / `package`）
- **THEN** 该元素在文件树上零节点、不产生任何 `owners` 引用；它只在元素树上以 FQN 节点呈现（本仓 `06` 段的 3 个容器 `Views` / `Components` / `Standard` 即此形态，`Components.Agent` 已翻转为目录模块、有节点）

#### Scenario: 同长多命中判 ambiguous 不任选

- **WHEN** 两个不同元素以同长度 `path` 前缀同时覆盖某文件（合成语料）
- **THEN** 该文件 `mark = ambiguous`、`owners` 为两个 FQN（字典序），MUST NOT 只给其一

#### Scenario: 同一归一路径被多元素声明判 ambiguous

- **WHEN** 两个元素声明同一个 `path`（如均写 `./src/shared/`，含 `./` 前缀与尾 `/` 两种拼法归一后相同）
- **THEN** 该路径节点 `mark = ambiguous`、`owners` 为两个 FQN；MUST NOT 判 `elementRoot`（`owners` 恰 1 才是 `elementRoot`）

#### Scenario: 生成物按三候选并集归元素

- **WHEN** 盘上分别存在入口同级生成物 `crates/infra/archi/src/lib.i.rs`（元素根目录形 `path = ["./crates/infra/archi/"]`）与 `packages/desktop/src/views/welcome-view.i.ts`（元素根文件形 `path = ["./packages/desktop/src/views/welcome-view.tsx"]`）
- **THEN** 两者各自 `mark = member`、`owners` 为对应元素（不落 `none`）；文件型根一例只有「主干对齐」候选能命中；树侧源码零第二份剥离 / 归一 / 前缀匹配实现

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/tree.rs` | 文件树投影（归属与定档） | **零改动**：模块根表由 `path` 非空元素展开、`owner_of` 复用、五值定档与 `contains` 语义逐字不变；本变更只让目录型 `path` 的元素多一个（`Components.Agent` 由文件型根变目录型根） |
| `crates/core/archi/src/scan/owner.rs` | 归属单点（树与漂移共用） | **零改动**：三候选并集、最长前缀、`Ambiguous` 不任选逐字不变；`components/agent/index.tsx` 仍归 `Components.Agent`（子元素文件型根优先、入口归父目录） |
| `.dev-team/architecture/models/06-frontend.toml` | 模块根表数据来源 | `Components.Agent` 的 `path` 由文件改目录 ⇒ 树侧该目录节点变 `elementRoot`、`index.tsx` 变 `member`；元素 / 关系计数不变（30 / 71） |

### Function: 无

本能力不新增函数：`tree` 的定档与 `contains` 计算零签名 / 语义变化。

### API: 无

`archi_file_tree` 的响应形状、五值 `FileTreeMark` 词汇与 `ALL` 对拍逐字不变。

### CLI: 无

无脚本面变化。

### Component: 无

左栏文件树呈现（分段切换 / 双向联动 / 五值标记）零改动。
