# desktop-crate-layout Specification（delta: archi-check-and-gate）

## ADDED Requirements

### Requirement: infra/archi crate 落地登记与 archi 依赖扩展

架构漂移判定域的执行面 SHALL 落位 `crates/infra/archi`（crate 裸名 `archi-runtime`），并在本能力登记其落位、依赖与边界归类：

- **落位与职责**：承载 `ArchiCheckRunner` 实现（取输入 → 调 `archi` 判定纯函数 → 落 `reports/archi` 报告 → 派生 outcome）与报告落盘单点。领域判定（扫描 / 归属 / 定级 / 报告结构 / 配置读取）MUST NOT 在此重复——全部在 `crates/core/archi`（见 `desktop-archi-drift`）。
- **workspace 内依赖**：`archi`（判定与报告结构）+ `orchestration`（port 与产出类型，与 `checks-runtime` 依赖 `orchestration` 同构）+ `foundation`（报告目录派生）。依赖方向 SHALL 为 infra → core 单向，MUST NOT 出现 core → infra。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `vcs-runtime`（变更文件清单经 port 注入，见 `desktop-archi-gate`「门禁输入口径与观察根」）、MUST NOT 依赖 `config`、MUST NOT 依赖 `workflow`、MUST NOT 创建任何进程（零 `std::process` / 零 `tokio::process` / 零 shell）。
- **新增入边**：`desktop-app → archi-runtime`（壳层装配注入 runner），MUST 在 `Cargo.toml` 处带用途注释；`archi-runtime` MUST NOT 被 `workflow` / `archi` / `agent-runtime` / `checks-runtime` 依赖。
- **`archi` crate 依赖扩展**：`crates/core/archi` SHALL 新增外部依赖 `oxc_parser` / `oxc_allocator` / `oxc_resolver`（TS / JS 词法与模块解析）与 `syn`（Rust 语法与 `use` 树），并把 `globset` 由传递依赖提为直接依赖（排除匹配）；这些依赖 MUST NOT 引入进程、Tauri、`config` 或 `workflow` 触点，`archi` 仍保持零 `workflow` 依赖（`workflow` ⇄ `archi` 互不依赖为硬红线，双向机械可验）。
- **边界分类学不增类**：本执行面零新外部依赖（`serde_json` 为锁内既有），归既有「文件边界」的读盘面 + 检查结论落盘（报告树），MUST NOT 新增第八类边界；`infra/` 目录不新增除 `archi` 外的成员。
- **依赖成本护栏**：`Cargo.lock` 净新增包数 SHALL ≤ 40（软线 25）；超出即视为特性开关引入面失控，先收紧 `oxc` 特性再复测（成本实测与回退序见 `desktop-archi-drift`「运行期预算与语料」）。

#### Scenario: 依赖面与方向审查

- **WHEN** 审查 `crates/infra/archi/Cargo.toml`、`crates/core/archi/Cargo.toml` 与壳 `Cargo.toml`
- **THEN** `archi-runtime` 的 workspace 内依赖恰为 `archi` / `orchestration` / `foundation`；`archi` 的 workspace 内依赖仍仅 `foundation`；壳含 `archi-runtime = { workspace = true }` 且带用途注释；`workflow` 依赖中不出现 `archi`，`archi` 依赖中不出现 `workflow`

#### Scenario: 零进程零 Tauri 审查

- **WHEN** 审查 `crates/infra/archi` 与 `crates/core/archi` 的进程触点与框架依赖
- **THEN** 两者均无进程创建、无 shell 调用、无 Tauri 依赖；`archi-runtime` 零 `store` / 零 `config` / 零 `vcs-runtime` 依赖

#### Scenario: 依赖成本不越硬线

- **WHEN** 完成 `oxc_*` / `syn` 引入并执行成本实测（锁包数 / 安装包 / 主二进制 / 编译用时）
- **THEN** `Cargo.lock` 净增 ≤ 40 包；任一指标越硬线时按回退序（减特性 → 自算路径 → 正则 import）处置并留痕，MUST NOT 直接放宽阈值

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/archi`（新，裸名 `archi-runtime`） | 架构门禁执行面 + 报告落盘 | workspace 内依赖 `archi` + `orchestration` + `foundation`；外部依赖 `serde_json`；零 Tauri / 零 spawn / 零 store / 零 vcs / 零 config / 零 workflow |
| `crates/core/archi`（依赖扩展） | TS / JS 与 Rust 解析面 | 外部依赖新增 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`（+ `globset` 提为直接依赖）；`foundation` 仍为唯一 workspace 内依赖；零 spawn / 零 Tauri / 零 store / 零 workflow |
| `crates/desktop-app`（依赖登记） | 装配注入 | `archi-runtime = { workspace = true }` + 用途注释；装配层零判定（runner 构造与注入） |
| `src-tauri/Cargo.toml`（workspace 清单） | 成员与版本收敛 | `members` 增 `crates/infra/archi`；`[workspace.dependencies]` 增 `archi-runtime` 与 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn` 收敛项（版本取实测选定值，MUST NOT 各 crate 分散声明） |
| 边界分类学 | 七类不增类 | 零新外部依赖 ⇒ 归既有文件边界读盘面 + 报告落盘；`infra/` 仅新增 `archi` 一个成员 |
