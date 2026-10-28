# workspace-layout-resolution Specification（delta: archi-page-agent-window）

## ADDED Requirements

### Requirement: 架构域 repo 相对路径派生（会话知识用）

foundation crate SHALL 提供架构域的 **repo 相对路径**派生函数（与服务绝对路径族并列，纯路径推导、零文件系统访问）：

```
archi_rel_dir()          → 架构域根的 repo 相对路径（`/` 分隔）
archi_models_rel_dir()   → <archi_rel_dir>/models
archi_decisions_rel_dir()→ <archi_rel_dir>/decisions
```

- 三函数 SHALL 全部从 layout/mod.rs 的目录名常量组派生，MUST NOT 行内字面量；`archi_models_rel_dir` / `archi_decisions_rel_dir` SHALL 从 `archi_rel_dir` 派生（架构域段名不二次出现）。
- 返回形态 SHALL 为 `String`（或等价的 `&'static str` 派生形态，取值形态由 design 定型），内容 MUST 为 `/` 分隔、无前导 `./`、无反斜杠——该形态直接用于模型可见文本（提示词 / 知识文本），MUST NOT 依赖调用方再归一。
- 唯一消费者 SHALL 为架构会话知识文本（`desktop-archi-agent`「架构会话的系统知识注入」），MUST NOT 在产品源码手写 `openspec/architecture` 一类相对路径字面量。
- 既有绝对路径族（`archi_dir` / `archi_models_dir` / `archi_decisions_dir`）、`is_archi_rel_path` 与 `Layout` 三字段 SHALL 语义零变化；`archi_config_path` 继续不在射程内（不因本变更预建）。

#### Scenario: 三函数同源且前缀一致

- **WHEN** 分别调用 `archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()`
- **THEN** 结果依次为架构域根、`<根>/models`、`<根>/decisions`（后两者以第一者为前缀），全部 `/` 分隔、零反斜杠、零前导 `./`

#### Scenario: 与绝对路径族尾部一致

- **WHEN** 以某 workspace 根调用绝对族（`archi_models_dir(root)`）并与 `archi_models_rel_dir()` 比对
- **THEN** 绝对路径剥去 `root` 前缀后的相对段与相对族返回值逐字相等（同源可验：改名只动常量组，两侧同时变化）

#### Scenario: 零 IO 与零字面量

- **WHEN** 审查实现与产品源码字面量
- **THEN** 三函数无任何文件系统访问（对不存在的 root 无关，因为不接 root 入参）；产品 `.rs`（`*_test.rs` 除）无 `architecture` 段字面量（唯一触点仍为 `foundation/src/layout/mod.rs`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `foundation::layout::archi_rel_dir` | 架构域根相对路径 | 由目录名常量组派生；`/` 分隔、零前导 `./`；零 IO |
| `foundation::layout::archi_models_rel_dir` / `archi_decisions_rel_dir` | 子目录相对路径 | 全从 `archi_rel_dir` 派生；供架构会话知识文本注入（唯一消费者） |
| `crates/infra/agent/src/archi_knowledge.rs`（消费点，见 desktop-archi-agent） | 知识拼装 | 以三函数注入目录路径，MUST NOT 手写相对路径字面量 |
