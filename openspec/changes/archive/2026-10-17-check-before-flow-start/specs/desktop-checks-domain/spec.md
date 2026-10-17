# desktop-checks-domain Specification (Delta)

## ADDED Requirements

### Requirement: baseline 报告槽位参数化

`crates/infra/checks` 两 runner（`static_check.rs` 的 `ProcessStaticCheck` 与 `testexec/` 的 test-execution runner）SHALL 支持报告槽位参数化：槽位为封闭二值 `test`（缺省——既有 `reports/test` 锚点，语义零改动）与 `baseline`（initialize 相位基线检查——`reports/baseline` 锚点）。槽位 SHALL 仅参数化报告落点（summary / 子报告 / exec-outputs 同锚派生），执行语义（无配置直过 / 程序不可达显式 Err / 退出码映射 / 框架矩阵 / 复用门判定规则）MUST NOT 因槽位分叉；两槽位的目录派生 SHALL 经 foundation layout 单点（`reports/test` 与 `reports/baseline` 同源变体），runner 内 MUST NOT 出现第二份路径派生。walker 调用面（`StaticCheckRunner` / `TestExecutionRunner` port 签名）的槽位透传形态由 design 定稿；`read_exec_output` 回查面 SHALL 覆盖两槽位的 exec-outputs（exec_id 跨槽位唯一）。

#### Scenario: 缺省槽位零改动

- **WHEN** 以缺省调用形态执行 static-check / test-execution（implement / test-gen / test-execution 相位的门禁步）
- **THEN** 报告与 exec-outputs 落 `reports/test` 锚点，与本变更前逐字节同路径；执行语义零分叉

#### Scenario: baseline 槽位落点与语义不变

- **WHEN** 以 baseline 槽位执行两 runner（initialize 相位基线检查）
- **THEN** 报告与 exec-outputs 落 `reports/baseline` 锚点（layout 单点派生）；static-check 无配置直过 / 程序不可达显式 Err、test-execution 框架矩阵与不支持框架显式 Err 等语义与缺省槽位一致

#### Scenario: 路径派生单点

- **WHEN** 审查 infra/checks 与 infra/exec 源码的报告目录派生
- **THEN** `reports/test` / `reports/baseline` 两变体仅驻 foundation layout 单点；runner 与 exec 层消费该单点，无逐 runner 私有后缀拼接

### Requirement: 依赖与工具产物树名单共享单源

依赖与工具产物树名单（`node_modules` / `dist` / `build` / `target` / `coverage` / `.nyc_output` + dot 目录剪枝——语义为「依赖与工具产物树，探测与清理均应跳过」）SHALL 自 `crates/infra/checks/src/testexec/detect.rs` 的私有 `PRUNED_DIR_NAMES` 上移 foundation 公开共享单点：testexec detect 剪枝与 infra/vcs `restore_baseline` 的 clean 排除面两消费点 SHALL 同源消费该单点，MUST NOT 保留第二份名单（detect 私有常量退役）。名单本体 SHALL 仅承载目录名封闭集与 dot 剪枝语义；`restore_baseline` 消费点的排除面 SHALL 为名单 + `openspec/`（工件与报告树为未跟踪面，MUST NOT 被 clean 删除——增补属 vcs 消费点语义，不入名单本体）。名单扩展 SHALL 经 foundation 单点一处修改两消费点同时生效。

#### Scenario: 两消费点同源

- **WHEN** 审查 testexec detect 的剪枝实现与 infra/vcs restore_baseline 的排除面实现
- **THEN** 两者均消费 foundation 公开名单单点；`detect.rs` 无私有 `PRUNED_DIR_NAMES` 残留，infra/vcs 无第二份名单

#### Scenario: 名单语义仅目录名与 dot 剪枝

- **WHEN** 以名单驱动的探测 / 清理遇到 `node_modules/`、`target/`、`.venv/`（dot 目录）与普通源码目录
- **THEN** 名单内目录与 dot 目录被剪枝 / 保留（按消费点语义），普通源码目录不受名单影响；名单本体不含 `openspec/` 等工件路径

#### Scenario: 单点扩展双点生效

- **WHEN** 向 foundation 名单新增一个依赖产物目录名
- **THEN** detect 剪枝与 restore_baseline 排除面同步生效，零第二处修改

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/foundation/src/`（名单单点，新） | 依赖与工具产物树名单 | 封闭目录名集 + dot 剪枝判定纯函数；公开共享；detect 与 vcs 两消费点同源 |
| `crates/infra/checks/src/static_check.rs` | static-check 槽位 | 报告锚点按槽位参数化（test 缺省 / baseline）；执行语义零分叉 |
| `crates/infra/checks/src/testexec/`（runner / detect） | test-execution 槽位与剪枝 | 报告锚点按槽位参数化；detect 剪枝改引 foundation 名单（私有 `PRUNED_DIR_NAMES` 退役） |
| `crates/infra/vcs`（restore_baseline，新） | 基线恢复排除面 | 排除面 = foundation 名单 + `openspec/`；实现驻 infra（core 零 spawn 红线），经 `ArchiveVcsPort` 缝消费 |
