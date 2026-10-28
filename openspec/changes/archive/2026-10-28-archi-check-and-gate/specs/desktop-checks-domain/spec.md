# desktop-checks-domain Specification（delta: archi-check-and-gate）

## MODIFIED Requirements

### Requirement: baseline 报告槽位参数化

`crates/infra/checks` 两 runner（`static_check.rs` 的 `ProcessStaticCheck` 与 `testexec/` 的 test-execution runner）SHALL 支持报告槽位参数化：槽位 SHALL 为**封闭三值**——`test`（缺省——既有 `reports/test` 锚点，语义零改动）、`baseline`（initialize 相位基线检查——`reports/baseline` 锚点）与 `archi`（架构漂移门禁报告——`reports/archi` 锚点，消费者为 `desktop-archi-gate` 的 runner）。本域两 runner MUST NOT 接受 `archi` 槽位（传入即显式 `Err`——该槽位的报告体是漂移报告 JSON，非 checks 报告 schema，混入会让两套报告语义互相污染）。槽位 SHALL 仅参数化报告落点（summary / 子报告 / exec-outputs 同锚派生），执行语义（无配置直过 / 程序不可达显式 Err / 退出码映射 / 框架矩阵 / 复用门判定规则）MUST NOT 因槽位分叉；三槽位的目录派生 SHALL 经 foundation layout 单点（`reports/test` / `reports/baseline` / `reports/archi` 同源变体），runner 内 MUST NOT 出现第二份路径派生。walker 调用面（`StaticCheckRunner` / `TestExecutionRunner` port 签名）的槽位透传形态由 design 定稿；`read_exec_output` 回查面 SHALL 覆盖 test / baseline 两槽位的 exec-outputs（exec_id 跨槽位唯一）；`archi` 槽位零 exec-outputs（报告为 JSON 文件，非 exec 输出，回查走报告路径）。

#### Scenario: 缺省槽位零改动

- **WHEN** 以缺省调用形态执行 static-check / test-execution（implement / test-gen / test-execution 相位的门禁步）
- **THEN** 报告与 exec-outputs 落 `reports/test` 锚点，与本变更前逐字节同路径；执行语义零分叉

#### Scenario: baseline 槽位落点与语义不变

- **WHEN** 以 baseline 槽位执行两 runner（initialize 相位基线检查）
- **THEN** 报告与 exec-outputs 落 `reports/baseline` 锚点（layout 单点派生）；static-check 无配置直过 / 程序不可达显式 Err、test-execution 框架矩阵与不支持框架显式 Err 等语义与缺省槽位一致

#### Scenario: 路径派生单点

- **WHEN** 审查 infra/checks 与 infra/exec 源码的报告目录派生
- **THEN** `reports/test` / `reports/baseline` / `reports/archi` 三变体仅驻 foundation layout 单点；runner 与 exec 层消费该单点，无逐 runner 私有后缀拼接

#### Scenario: archi 槽位归架构门禁

- **WHEN** 以 `archi` 槽位调用 checks 域任一 runner，并以该槽位调用架构门禁 runner
- **THEN** 前者显式 `Err`（checks 域不接受该槽位）；后者报告落 `reports/archi`（layout 单点派生）、零 exec-outputs 产出，两域报告互不混居

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `foundation::layout::ReportSlot` | 槽位封闭集 | 三值 `Test` / `Baseline` / `Archi`（纯进程内枚举，零 serde / specta）；`report_dir_name` 三分臂 → `test` / `baseline` / `archi` |
| `foundation::layout::change_reports` | 槽位驱动报告目录单点 | `change_reports(root, change, slot)` 对三值同源派生；`reports/archi` 不另起第二套定位 |
| `crates/infra/checks`（两 runner） | 槽位消费面不变 | 只消费 `Test` / `Baseline`；`Archi` 传入即显式 `Err`；报告 schema、执行语义、复用门规则零变化 |

