# desktop-config-agent Specification（delta: archi-edit-engine-primitives）

## Purpose

本 delta 只有一处**引用追改**：写面写入器要求里「`ConfigWrite` 沿 `archi::edit::GateWrite` 的 `{ file, content }` 形状」这句引用的类型在 `archi-edit-engine-primitives` 里改名（`GateWrite` → `ModelWrite`，并新增写后哈希字段）。句子里的形状承诺（`{ file, content }`）本身不变——`ConfigWrite` 仍是两字段、配置写面仍不经 `hash`：

- 追改面最小：仅该 requirement 的「公开面配套」条一处括注；该 requirement 的其余内容、`to_file_value` 的四面一致性锚（P1–P4）、`ConfigWrite` 形状与 `crates/core/config` 的实现**逐字不变**。
- 零实现改动：`plan_apply_config` / `ConfigWrite` / 配置写面行为零变化；本 delta 不引入 `hash` 字段到 `ConfigWrite`（写面乐观锁不在本能力射程）。

## MODIFIED Requirements

### Requirement: 文件面写入器 `to_file_value`（模板驱动）

写面 SHALL 经 `core/config` 的纯函数 `to_file_value(config: &WorkspaceConfig, template: &Value) -> Value` 产出 file face；`serde_json::to_value(&WorkspaceConfig)` MUST NOT 用于落盘：

- **模板驱动**：输出键集 SHALL 按 `template`（候选原文）出现过的键裁剪，值取规范化结果；`WorkspaceConfig` 不记录 provenance（`cwd: "."` 无法区分「文件写了 `.`」与「吃了默认」），故「写全 / 写最小」不是 flag 而是**入参类型**；「写全字段」= 模板取全键集的同一特例，MUST NOT 设第二形态。
- **结构变换而非手拼**：SHALL 为 `to_value` → 三处递归 `rename_in_place`（`staticAnalysis`→`static_analysis`、`writeProtection`→`write_protection`、`dependsOn`→`depends_on`，`dependsOn` 在 `projects[i]` 内故须递归全层级）→ `extra` 展开回顶层 → 数值整数化 → 模板裁剪；MUST NOT 手拼字段名清单（新增字段必漏）。
- **数值整数化**：`fract() == 0.0` 且绝对值不超过 `2^53` 时出 `i64`，否则保留浮点（serde_json 的 `PosInt` 与 `Float` 不相等，阈值缺省值来自 `f64`，不整数化会让用户写的 `90` 变成 `90.0`）。
- **`extra` 三坑**：① null 值条目 SHALL 保留（丢 null 规则只作用于已知 Option 字段，MUST NOT 作用于 `extra`，否则回读少一条）；② 键序按字母序重排（不可保真，接受）；③ 键冲突 SHALL 显式 `Err`（若将来给结构体加字段却忘进 `KNOWN_KEYS`，字段与 `extra` 副本会同键，`Map::insert` 会静默覆盖）。
- **四面一致性锚**：写入器 SHALL 由四条机械锚守（缺一即漂移无人拦）——**P1** 文件面输出中 `extra` 值内部以外任何键名不含大写字母（自证，零维护）；**P2** 满配 fixture 的顶层键集 == `known_keys()` 且无 `extra`（使 `KNOWN_KEYS` 成为写入器键集的唯一持有者，并继承其与 `config.schema.json` 的既有一致性断言）；**P3** `assemble(to_file_value(cfg, 模板))` 与 `cfg` 逐字段相等（线面值比较，`WorkspaceConfig` 无 `PartialEq`）；**P4** 对「全字段显式」字面量 `L`（字母序、数值写整数）：`to_file_value(assemble(L).config, L) == L`。
- **公开面配套**：`core/config` SHALL 导出 `known_keys()` 与 `TestFramework::ALL` / `as_str()`（与 `parse_framework` 双向互证），供知识词表与 P2 锚共用；`plan_apply_config` SHALL 返回 `ConfigWrite { file: PathBuf, content: String }`（沿 `archi::edit::ModelWrite` 的 `{ file, content }` 形状——同型另有写后哈希字段 `hash`，配置写面 MUST NOT 引用它），路径经 `foundation::layout::config_path`（crate 内 MUST NOT 出现 `config.json` 字面量）。
- **规范化明示（不做保序）**：写回会重排键序并格式化（字母序 + pretty 展开，行内数组拆行），`projects[i]` / `tests[i]` / `write_protection.files[i]` 内的未知子键在组装路径上被静默丢弃——该后果 SHALL 在应用确认面明示；MUST NOT 引入 `serde_json/preserve_order`（会把全仓 `Map` 换 `IndexMap`，波及所有 JSON 顺序断言）。

#### Scenario: wire face 绝不用于落盘

- **WHEN** 审查写面实现与 `Ok` 输出形态
- **THEN** 落盘内容与 `propose_config` 输出只经 `to_file_value`；`serde_json::to_value(&WorkspaceConfig)` 未出现在任何落盘路径（出现即判不合格）

#### Scenario: 四条机械锚

- **WHEN** 运行 `core/config` 测试
- **THEN** P1 / P2 / P3 / P4 各自成立：无大写键名（`extra` 值内除外）、满配顶层键集与 `known_keys()` 逐字相同、`assemble ∘ to_file_value` 幂等、全字段字面量往返逐字相等

#### Scenario: 整数化与 extra 保真

- **WHEN** 候选含 `mutation.score: 90`、`coverage.lines: 90.5`，并含一个值为 `null` 的 `extra` 条目
- **THEN** 落盘内容为 `90`（非 `90.0`）与 `90.5`；`extra` 的 null 条目仍在盘上，重读后条目数不减

#### Scenario: 键冲突显式失败

- **WHEN** 构造「结构体字段与 `extra` 键同名」的输入（例如给结构体加字段却忘进 `KNOWN_KEYS`）
- **THEN** `to_file_value` 返回显式 `Err`（附冲突键），MUST NOT 静默覆盖任一侧

#### Scenario: 文件面键形全小写

- **WHEN** 检查任一满配 fixture 的落盘输出
- **THEN** `static_analysis` / `write_protection` / `depends_on` 三处为 snake_case，`extra` 条目展开在顶层，无 `staticAnalysis` / `writeProtection` / `dependsOn` 残留

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/config/src/lib.rs` | 写面纯函数（零改动） | `to_file_value` / `plan_apply_config` / `ConfigWrite { file: PathBuf, content: String }` 逐字不变；**零 `hash` 字段**（写面乐观锁归 `archi` 的写回原语族，配置写面不引用） |
| `crates/core/config/src/lib_test.rs` | 断言面（零改动） | P1–P4 四条机械锚逐条不变 |

### API: 对外面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `ConfigWrite` | 零改动 | 仍两字段 `{ file, content }`；本 delta 只追改 spec 里引用的上游类型名 |
| `workspace_config_apply` 命令面 | 零改动 | 签名 / 错误面 / 写后重读语义逐字不变（`bindings.ts` 零 diff） |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| 配置页应用确认面 | 零 UI 变化；「重排键序与格式化」的明示句逐字保留 |
| `archi::edit` 写回原语族 | 只作形状引用源（`ModelWrite { file, content, hash }`）；本能力不消费其 `hash`，也不引入写面乐观锁 |
