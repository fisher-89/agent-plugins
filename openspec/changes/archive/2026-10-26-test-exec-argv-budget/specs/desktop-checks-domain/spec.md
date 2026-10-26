# desktop-checks-domain Specification（delta: test-exec-argv-budget）

## ADDED Requirements

### Requirement: 命令行超限归因文案单点装配

检查域 SHALL 为「展开后命令行超平台长度上限」这一环境成因提供两臂文案装配，落在既有单点模块 `crates/infra/checks/src/environment.rs`（「检查域环境归因装配单点，MUST NOT 两写」），MUST NOT 在 runner / mod 侧就地拼串：`argv_budget_text(budget: usize, expanded_len: usize, files: usize) -> String`（拒绝降级时的 `RunFault::Environment` 摘要）与 `argv_degrade_note(budget: usize, expanded_len: usize, files: usize) -> String`（安全降级时的记录 info 行）。

- **文案内容三项（MUST 齐备）**：① 定性——「命令行长度超平台上限，属 harness 限制，非被测代码问题」（MUST 明确否定代码归因，避免把修复引向被测仓库）；② 数字——展开长度 / 平台预算 / 文件数三个数逐一给出；③ 动作——「收窄 suite `includes` 或拆分 suite」的可执行建议。
- **MUST NOT 复用安全软件指引**：`argv_budget_text` 与 `argv_degrade_note` MUST NOT 含 `ENV_GUIDANCE`（「在火绒企业版 / 云枢中排除 workspace 父目录…」）或任何占用者 / 被锁文件 / 重试次数措辞——成因是命令长度而非文件锁，误引会让用户去改安全软件配置。
- **两臂措辞分工**：`argv_budget_text` MUST 给可执行动作（收窄 `includes` / 拆分 suite）；`argv_degrade_note` MUST 陈述「已降级为框架自发现（N 个测试文件）」而不给动作（降级已发生，此事只需可查）。
- **既有文案零变**：`fault_text`（瞬态耗尽 / 超时归因）与 `attempts_log`（尝试分隔头）的产出 MUST 逐字不变，两者是命令行超限面的**回归锚**（同模块不同成因，各自可辨）。
- **不出线**：两臂文案均在进程内消费——`argv_budget_text` 随 `RunFault::Environment(String)` 交 walker 终局文案，`argv_degrade_note` 只进输出记录的 plan 头；两者 MUST NOT 进 IPC / `RunUpdate` / `RunNodeRecord` / `src/types/generated/bindings.ts`。

#### Scenario: 三要点齐备且不误引环境指引

- **WHEN** runner 因展开长度超预算且不可安全降级而装配 `Environment` 文案
- **THEN** 文本含「harness 限制 / 非被测代码问题」定性、展开长度与平台预算与文件数三个数字、「收窄 includes 或拆分 suite」动作；MUST NOT 含「排除 workspace 父目录」「火绒」「云枢」等安全软件指引，MUST NOT 含占用者点名或「已自动重试 N 次」

#### Scenario: 降级说明措辞不带动作也不带环境指引

- **WHEN** runner 因展开长度超预算但可安全降级而装配 `argv_degrade_note`
- **THEN** 文本陈述「展开 N 字符超平台预算 M，已降级为框架自发现（K 个测试文件）」；MUST NOT 给「收窄 includes / 拆分 suite」动作建议（降级已发生，无需动作）、MUST NOT 含安全软件 / 占用者 / 重试次数措辞；该文本只进输出记录 plan 头，MUST NOT 进 `SubReport.findings`

#### Scenario: 既有归因文案逐字不变

- **WHEN** 以既有瞬态耗尽场景（锁文本 + 5 次尝试）驱动 `fault_text`
- **THEN** 输出与变更前逐字一致（归因段 / 「已自动重试 N 次」/ 重试进展行 / 末次诊断摘要 / 环境侧处理指引齐备）；`attempts_log` 的 attempt 分隔头亦零变

#### Scenario: 文案不出线

- **WHEN** 审查生成绑定与 IPC 载荷
- **THEN** `argv_budget_text` / `argv_degrade_note` 的产物均未出现在 `bindings.ts` / `RunUpdate` / `RunNodeRecord` 中，`bindings:check` 零 diff

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/checks/src/environment.rs` | 环境归因文案装配（新增成因臂 ×2） | 新增 `argv_budget_text(budget, expanded_len, files) -> String`（拒绝面：定性 + 三数字 + 收窄 includes / 拆分 suite 动作）与 `argv_degrade_note(budget, expanded_len, files) -> String`（降级面：三数字 + 「已降级为框架自发现」，零动作建议）；两者 MUST NOT 复用 `ENV_GUIDANCE`；`fault_text` / `attempts_log` / `progress_line` 逐字零改（同模块另一成因臂） |
