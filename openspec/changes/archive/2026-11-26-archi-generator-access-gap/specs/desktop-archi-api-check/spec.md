# desktop-archi-api-check Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 只补一条**语义澄清**，不改任何判据、档位或计数：`error` 档（`apiMissing` / `apiSignatureMismatch`）从此只有一条合法修复路径——**改代码追上契约**。判据本体（十值词汇、`error` 恰两项、`docMissing` 恒 `warn` 不纳管、`passed` 三合取）逐字不变。

## ADDED Requirements

### Requirement: error 档的修复路径唯一化

`apiMissing` / `apiSignatureMismatch` 是唯二 `error` 档判据（能阻断归档：`passed = errors 空 ∧ model.structuralErrors 空 ∧ error 档 apiFindings 空`）。本变更 SHALL 把「两者怎么才能消失」写成唯一路径，使 `error` 档名副其实：

- **唯一合法修复 = 改代码追上契约**：两条判据的对象都是「接口定义已声明而代码未兑现」。故其修复动作 SHALL 恰为「改实现（导出 / 签名）使代码满足 `<叶段>.schema.json`」。该路径 MUST NOT 需要改契约、MUST NOT 需要改本能力的任何判据。
- **改契约不再是修复路径**：实现相位对 `<archi_models_dir>/**` 的手改由**会话级相位锁**拒绝（锁面与拒串归 `desktop-write-protection`「会话级写入禁区（相位锁）」）⇒「改 schema 迎合代码」在结构上不可达。若实现方判定契约写错或不可实现，正确动作是 `backtrack` 回 `dev-design` 修订契约（或请用户改用架构页会话），此时该判据的消失属**契约变更的结果**，不属实现相位的修复动作。
- **判据面零变化**：`ArchiApiFindingKind` 十值、`error` 恰两项、`docMissing` 恒 `warn` 且永不纳管、`status == errors ⟺ passed == false` 的不变量、报告并列槽位 `apiFindings` 与基线读数——全部逐字不改。本 requirement MUST NOT 引入新 kind、新 category、新档位或新的求值顺序。
- **不得以「实现相位不能改契约」为由放松判据**：MUST NOT 把两条判据降档、MUST NOT 把 `error` 档移出 `passed`、MUST NOT 为「起草期生成的 `.i.rs` 在 implement 补齐之前编译不过」加白名单——`error` 档在 implement 期变红是**设计意图**（契约先行、实现跟上）。

#### Scenario: 实现未兑现时红且修复路径唯一

- **WHEN** 某元素 `api = declared`、schema 在盘并声明了 `foo`，代码未导出 `foo`，在 `implement/executor` 相位取报告
- **THEN** 报告含一条 `apiMissing`（`error` 档 ⇒ `status = errors` 且 `passed = false`）；该相位对 `<archi_dir>/models/**` 的 `write` / `edit` 被相位锁拒绝（取契约串）；删掉 schema 里的 `foo` 声明不会发生（被锁挡）——使判据转绿的唯一动作是代码导出 `foo`

#### Scenario: 契约确有错时走 backtrack 而非就地改

- **WHEN** 实现方发现 schema 声明的签名不可实现，先在 `implement` 相位尝试 `write` 该 `.schema.json`
- **THEN** 写入被拒绝（契约串给出三条出路）；处置路径为「停下说明冲突 → 用户决定 `backtrack` 回 `dev-design` 修订契约」；修订后重跑两相位，判据按新契约求值——判据面零新增 kind、零档位变化

#### Scenario: 判据与读数逐字不变

- **WHEN** 对照变更前后的 `ArchiApiFindingKind::ALL`、`error` 档成员、`category` 七值、`passed` 三合取与基线读数
- **THEN** 全部逐值相等（`error` 仍恰 `apiMissing` / `apiSignatureMismatch` 两项；`docMissing` 仍 `warn` 且不纳管；基线 `0 / 38`、分母 49 的口径不变）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: 判据面（零改动）

| 项 | 描述 |
|---|---|
| `crates/core/archi/src/api/**` 的判据求值 / 档位 / 报告结构 | 零改动：本 requirement 只约束「修复动作」，不改判据实现 |
| `crates/infra/archi/src/**` 的 runner 与 `passed` 折算 | 零改动：`passed` 三合取逐字不变 |

### Component: 纪律面

| 项 | 描述 |
|---|---|
| `crates/core/workflow/src/prompts/implementation-generator.md` | 纪律反转的载体（不改契约 + 两条出路），断言归 `desktop-phase-prompts` |
| `crates/infra/agent/src/sdk/protect/reason.rs` | 契约串给出三条出路（落位归 `desktop-write-protection`） |
