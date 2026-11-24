# desktop-archi-model Specification（delta: archi-scaffold-field-dictionary）

## Purpose

本 delta 给元素字段面补一份**机器可读清单** `ELEMENT_FIELD_NAMES`（`parse/toml.rs`，`RawElement` 定义旁），使消费面（`archi_scaffold` 的字段词典断言，见 `desktop-archi-agent`）能遍历字段名而不再手抄——字段面新增 / 改名时，词典缺行由测试红而不是靠「人记得同批改」。

字段面本身零变化：`RawElement` 仍八项、`Element` 仍十一项、`deny_unknown_fields` / 值域封闭 / `apis` 五拒 / `path` 基数逐字不动；常量无 serde / specta，不进 IPC、bindings 与任何生成物。

## ADDED Requirements

### Requirement: 元素字段面的机器可读清单

`crates/core/archi/src/parse/toml.rs` SHALL 在 `RawElement` 定义旁定义 `pub const ELEMENT_FIELD_NAMES: [&str; 8]`（顺序 = 字段声明序：`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`），并经 `parse/mod.rs` 再导出为 `archi::parse::ELEMENT_FIELD_NAMES`（沿 `TomlParser` 的既有重导出形态）。

- **单点不是第二份事实源**：常量是字段白名单的**机器可读副本**，权威仍是 `RawElement` 的字段定义。两者一致性 SHALL 由同 crate 内的源码自证测试机械对拍（`include_str!("toml.rs")` 取 `struct RawElement { … }` 块内的字段名，滤掉 `///` 与 `#[…]` 行），断言**同序同集**——新增 / 改名 / 删除字段而常量未同批更新即红（范式同 `model_test.rs` 的 `element字段数恰十一个` 与 `toml_test.rs` 的 `解析doc白名单源码自证`）。
- **消费侧零手抄**：消费面（`archi_scaffold` 草稿的字段全集断言）SHALL 遍历本常量，MUST NOT 在测试或产品代码里另抄八个字段名字面量。
- **不是值域词表**：本清单是字段**名**的封闭白名单，MUST NOT 混入 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` 一族封闭**值**词表，亦 MUST NOT 进「消费面词汇集一致性对拍」（那几组词表的域是值，不是字段名）。
- **零线面**：常量 MUST NOT 派生 `Serialize` / `Deserialize` / `specta::Type`，MUST NOT 进 IPC 契约、TS bindings、`INDEX.toml` 的 `types` / `ops` 计数与任何生成物。
- **零字段面扩张**：本常量 MUST NOT 被用来放宽字段白名单——`RawElement` 仍八项、`Element` 仍十一项，`deny_unknown_fields` 语义逐字不变；`docs` 类退役键仍 `unknownField`。

#### Scenario: 常量与字段面同序同集

- **WHEN** 读取 `archi::parse::ELEMENT_FIELD_NAMES` 与 `parse/toml.rs` 的 `struct RawElement { … }` 块（字段行取 `///` 与 `#[…]` 之外以 `,` 收尾的行）
- **THEN** 常量恰八项且逐项同序相等（`kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）；`docs` 在两侧均零命中；测试内不出现另一份手抄的八字面量

#### Scenario: 字段增删未同批更新即红

- **WHEN** 只在 `RawElement` 增加 / 删除 / 改名一个字段，而 `ELEMENT_FIELD_NAMES` 未同批更新
- **THEN** 源码自证测试红，并按集合差指出缺失或多出的字段名——「新增字段未补词典」不再可能以全绿形态漏过

#### Scenario: 常量不进线面与生成物

- **WHEN** 运行 `api:export` 与 `bindings:export` 并与盘上产物比对
- **THEN** `INDEX.toml` / `.schema.json` / `.i.rs` / `.i.ts` / `bindings.ts` 零 diff（`Backend.Core.Archi` 与 `Backend.Infra.AgentRuntime` 的契约状态均为 `absent`，常量不计入 `types` / `ops`）；`ELEMENT_FIELD_NAMES` 在生成物内零命中

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/parse/toml.rs` | 字段面单点（本变更只加一条常量） | `pub const ELEMENT_FIELD_NAMES: [&str; 8]`（声明序）；`RawElement` / `to_model` / `validate_apis` / `validate_path` / `classify` 零改动 |
| `crates/core/archi/src/parse/mod.rs` | 模块出口 | `pub use toml::{TomlParser, ELEMENT_FIELD_NAMES};`（一行；括号内取 fmt 实序，`cargo fmt --all --check` 零退出；`SourceParser` / `parse_dir` 不动） |
| `crates/core/archi/src/parse/toml_test.rs` | 字段面断言面 | 新增 `字段面清单与RawElement同序同集`（`include_str!("toml.rs")` 源码自证）；既有解析 / 值域 / `apis` 五拒 / `path` 基数用例逐字保留 |
| `crates/core/archi/src/lib.rs` | crate 根出口 | 零改动——常量经 `parse` 再导出，MUST NOT 提成 crate 根 `pub use` 清单项（字段面清单不是跨 crate 通用词汇） |

### API: `archi` crate 的 `parse` 出口

| 项 | 变化 | 描述 |
|---|---|---|
| `archi::parse::ELEMENT_FIELD_NAMES` | 新增 | `[&str; 8]`，顺序 = `RawElement` 声明序；零 serde / 零 specta |
| `archi::parse::TomlParser` / `SourceParser` / `parse_dir` | 零改动 | 解析入口、诊断分类与聚合语义逐字不变 |
| `archi` crate 根出口（`lib.rs`） | 零改动 | 常量不进根 `pub use` 清单；`dto` 词汇出口不变 |
| `RawElement` / `Element` / `Relation` 字段面 | 零改动 | 八项 / 十一项 / 关系四键各自逐字不变（本常量只是字段名的机器可读副本） |

### Function: 解析与校验面

| 项 | 变化 | 描述 |
|---|---|---|
| `to_model` / `validate_apis` / `validate_path` / `classify` | 零改动 | `apis` 五拒、`package` 的 `path` 基数、`invalid type → invalidValue` 分类逐字不变 |

### CLI / Component: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| IPC 命令面 / 前端组件 | 零改动 | 常量无 serde，无 DTO / bindings / UI 变化 |
| `.dev-team/architecture/**` 生成物 | 零改动 | `INDEX.toml` / `.schema.json` / `.i.*` 零 diff（两元素 `status = "absent"`） |
