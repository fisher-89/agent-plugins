# desktop-crate-layout Specification（delta: desktop-write-protection）

## ADDED Requirements

### Requirement: glob 方言单点复用与 agent-runtime → checks 依赖边

`agent-runtime` SHALL 以 workspace 依赖 `checks = { workspace = true }` 引入一条 infra → core 纯层的依赖边，其**唯一用途**是复用 `checks::globmatch::match_glob` 作写入保护的规则匹配：

- `agent-runtime`（写入保护判定面）MUST NOT 自造第二份 glob 匹配实现——花括号展开、无通配模式的目录前缀语义、`**/` 零段可命中、反斜杠归一四项语义 SHALL 只存在于 `checks::globmatch` 一处（MUST NOT 在 protect 模块内直接以 `glob::Pattern` + 自定 `MatchOptions` 拼第二套方言）。
- `checks::globmatch` 的公开签名与语义 MUST NOT 变化：`match_glob(path, pattern) -> bool` 与 `try_expand_braces(pattern) -> Result<Vec<String>, String>` 保持不变，既有两个消费点（`core/checks/src/aggregate.rs`、`infra/checks/src/testexec/detect.rs`）零改动。
- **边方向合法性**：infra → core 已有先例（`agent-runtime → agent` / `config` / `orchestration` / `foundation` / `exec` / `store`）；`checks` MUST NOT 反向依赖 `agent-runtime`（crate 图机械保证无环）；`checks` 的纯层性质 MUST NOT 变化（零进程 spawn、零 Tauri，边界归类仍为 checks 边界的纯层半边）；`agent-runtime` 仍 MUST NOT 依赖 Tauri。
- **替代方案被否留痕**：把 `match_glob` 下沉 `foundation` 被否——`foundation` 的「零其他 crate 依赖（含零 tokio）」是刻意不变量（取消原语 `cancel` 即为此以 std-only 实现），引入 `glob` crate 会破该不变量；故取「登记新增依赖边 + 方言单点」而非「下沉到最小 crate」。
- **现行口径声明**：本条款 SHALL 为 `agent-runtime` workspace 内依赖集合的现行登记。既有「`agent-runtime` SHALL 仅依赖 `agent`」表述已随多轮演进（config / exec / orchestration / foundation / store 逐次引入）陈旧，本条款只登记新增边与方言理由，MUST NOT 被解读为放宽任何分层结论：core 各 crate 仍不得依赖 infra、`config` 仍仅依赖 `foundation`、`store` 仍不依赖 core 行为、`foundation` 仍零 workspace 内依赖。

#### Scenario: 依赖边审查

- **WHEN** 审查 `crates/infra/agent/Cargo.toml` 与 `crates/core/checks/Cargo.toml`
- **THEN** `agent-runtime` 的 `[dependencies]` 含 `checks = { workspace = true }` 且有用途注释（glob 方言单点复用）；`checks` 的依赖中不出现 `agent-runtime` 或任何 infra crate；`agent-runtime` 无 Tauri 依赖

#### Scenario: 方言单点生效

- **WHEN** 审查写入保护的规则匹配实现
- **THEN** 匹配只经 `checks::globmatch::match_glob`；protect 模块内无第二份花括号展开、无自建 `glob::Pattern` + 自定义 `MatchOptions` 的匹配路径（`glob` crate 在 `agent-runtime` 内的既有用途仅为 glob 工具的目录扫描，不参与保护判定）

#### Scenario: foundation 不变量守住

- **WHEN** 审查 `crates/core/foundation/Cargo.toml`
- **THEN** 依赖面与本变更前一致（无 `glob` crate、无任何 workspace 内依赖）——方言未下沉，极小 crate 的零依赖不变量不变

#### Scenario: 分层结论不变

- **WHEN** 复核既有「依赖方向符合分层」「core 零 Tauri」「store 不依赖 core 行为」断言
- **THEN** 全部结论原样成立；新增边只发生在 infra → core 纯层方向，checks 边界的成员（进程执行）仍不出现在 `infra/agent`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/Cargo.toml` | 新增 infra → core 纯层依赖边 | `checks = { workspace = true }`（唯一用途：`checks::globmatch::match_glob`；带用途注释） |
| `crates/core/checks/src/globmatch.rs`（既有，零改） | 唯一 glob 方言实现 | `match_glob` / `try_expand_braces` 签名与语义不变；花括号展开上限 `EXPANSION_LIMIT`、`MatchOptions{ require_literal_separator: true, require_literal_leading_dot: false }` 单点持有 |
| `crates/infra/agent/src/sdk/protect/**` | 依赖边消费方 | 匹配只调 `checks::globmatch::match_glob`；MUST NOT 自建第二套方言（含自定 `MatchOptions`） |
| `crates/core/foundation/**` | 本变更零改动 | 依赖面零变化（零 workspace 内依赖、零 `glob` crate）；`layout::config_path` 仅为内置 glob 派生源，不承载匹配语义 |
| `crates/core/checks/src/aggregate.rs`、`crates/infra/checks/src/testexec/detect.rs` | 本变更零改动（既有两消费点） | 调用点与断言不变，方言演进仍单点 |
