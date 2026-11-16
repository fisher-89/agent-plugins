# desktop-archi-agent Specification (Delta)

## MODIFIED Requirements

### Requirement: archi 工具面注入与 (phase, role) 矩阵

archi 专用工具的**下发面** SHALL 由 `agent-runtime` 组合根以**工具名字清单**表达（`SessionInjections.tools: Option<Vec<String>>`；`None` = 七内建全量），MUST NOT 依赖已退役的枚举式 toolset 概念。清单内每个名字 SHALL 经 `mount::lookup` 成员校验（未知名显式 `Err` 携名与来源，不静默滤过）；自由字符串工具名 MUST NOT 进执行面。

会话装配（`crates/infra/agent/src/compose.rs`）SHALL 按 `(source, phase, role)` 决定下发面：

| phase | role | `archi_module` | `archi_search` | `archi_api` |
|---|---|---|---|---|
| `proposal` | executor | ❌ | ✅ | ❌ |
| `dev-design` | executor / evaluator | ✅ | ✅ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ |
| `code-review` | evaluator | ✅ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ | ❌ |

- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份同名映射。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ 三个 archi 工具名（`archi_module` / `archi_search` / `archi_api`）+ `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）。该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`），段数 < 4 或末段非数字即 `None`。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒三工具全开）。
- `source = "explore"` 臂的 **archi 面 SHALL 恒为空**（零 archi 工具，逐字沿用）；该臂的**完整**名字清单（六工具收窄 + `create_explore_note` + 只读 DB 工具 `db_tables` / `db_query` 两件）单源见 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」与 `desktop-agent-db-tools`，本能力 MUST NOT 另写一份；其余非 change 来源（debug 等）SHALL 恒「七内建全量 + 零 archi 工具 + `ask = Generic`」的显式构造（MUST NOT 以默认构造兜底——默认值的 `ask = None` 即静默禁用）。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（当前唯一生产者 = archi 臂），前端 / 自由字符串 MUST NOT 直接供给。
- **`archi_api` 只给 implement/executor 与 archi 臂**：其余相位 MUST NOT 下发（`code-review` / `acceptance` 判「改动是否越界」只读模型面，不自检生成物新鲜度——新鲜度由门禁与契约面板承担）。

#### Scenario: 逐相位矩阵命中

- **WHEN** 分别以 7 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 只 `archi_search`；dev-design 两工具；implement 三工具；code-review / acceptance 只 `archi_module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落七内建全量），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为七内建全量（含 `write`）另加三个 archi 工具名、`preamble` 为架构会话知识文本；definitions 含 `archi_module` / `archi_search` / `archi_api` 三定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：三工具全开 + 七内建全量 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零 archi 工具

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note` + `db_tables` / `db_query`（只读 DB 工具两件，清单单源见 desktop-agent-db-tools），debug 为七内建（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `openspec/specs/desktop-archi-agent`（表述对齐） | explore 臂口径 | 仅改「explore 臂逐字不变」的两处表述（矩阵 bullet 与 `explore 与 debug 来源零 archi 工具` scenario）：archi 面恒空不变，完整清单单源指向 desktop-agent-execution / desktop-agent-db-tools |
| `crates/infra/agent/src/compose.rs`（零 archi 语义改动） | 矩阵与两臂 | `archi_matrix` 7 行逐字不变；archi 臂三工具 + 知识 preamble 逐字不变；change 臂矩阵语义不变（DB 工具两名的恒 push 属 desktop-agent-execution 的装配面，不参与矩阵判定） |
| `crates/infra/agent/src/sdk/mount.rs`（零 archi 语义改动） | 档位与成员校验 | 三 archi 工具仍 `ToolTier::Read`；成员校验对新增两 DB 工具名同样生效（同一条判据，无第二份名单） |
