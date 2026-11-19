# desktop-change-flow-view Specification（delta: init-check-archi）

## MODIFIED Requirements

### Requirement: initialize 相位列与 clean 步词呈现

initialize 相位（表头一次性基线检查相位，见 desktop-change-orchestration「initialize 基线检查相位执行体」）SHALL 以**零结构改动**方式呈现：initialize 相位列随 `detail.pipeline` 出线自动成为首列（列头相位名文案补 initialize 人读标签），Clean / StaticCheck / ArchiCheck / TestExecution 步节点以既有 runtime step 节点渲染（四类分组徽章可辨、running / failed / stopped / passed 状态如实）；基线**检查域**输出经节点 `outputRef` 的既有 stdout 面板回查（`exec_output_path` 命中 baseline 树 + `read_file` / `read_exec_output_delta` 整读）；基线**架构**检查输出不经 `outputRef`（架构报告是 `reports/archi/<timestamp>.json` 的 JSON 文件、非 exec 输出，节点 `outputRef` 恒 `null`），其回查走节点 detail 中的报告路径与 `reports/archi` 目录——MUST NOT 为其新增面板或第二套读取链；initialize 失败的 run 终态 reason（Error / Fail 分文案 + 报告目录指引，以及**基线架构未通过**一类文案 + `reports/archi` 目录指引）经既有终态呈现面直达。前端步词呈现 SHALL 补 `clean` 与 `archi_check` 两线词：`STEP_LABEL` 穷尽 Record 补 `clean` 键（人读文案）与 `archiCheck` 键（人读文案「架构检查」）、`normalizeKind` / `stepWord` 单点补 `clean` ↔ `clean` 与 `archi_check` ↔ `archiCheck` 两组映射、分类徽章覆盖（`RunStepGroup` / `GROUP_LABEL`：`clean` 归 git、`archiCheck` 归 toolStep），现有步骤渲染零改动直接吃。MUST NOT 为 initialize 引入新节点类型、新面板或独立交互入口。辅助步退场后 initialize 相位的新 run 恰为 Clean / StaticCheck / ArchiCheck / TestExecution 四节点（无 `phaseStart` / `phaseLog`），MUST NOT 为缺失的两节点补占位或补空态；`archi_check` 在两处挂载（initialize / implement）共用同一渲染面与同一线词，前端 MUST NOT 按相位区分其渲染分支。

#### Scenario: initialize 列与步节点零结构呈现

- **WHEN** 打开一个已跑过 initialize 相位（绿或红）的 change 详情
- **THEN** initialize 列为首列（相位名人读文案在案），Clean（git 分类徽章）/ StaticCheck / ArchiCheck / TestExecution 步节点按序呈现、状态如实；ArchiCheck 呈 toolStep 分类徽章与 detail 摘要；无 `phaseStart` / `phaseLog` 占位、无新节点类型、无独立面板

#### Scenario: 基线失败终态与回查

- **WHEN** initialize 相位以 Terminal 失败（static-check 红、archi-check 违例或 test-execution Error / Fail）收口的 run 详情被打开
- **THEN** run 终态 reason 呈现对应文案（Error / Fail 可辨、含 baseline 报告目录指引；archi 一类另携 `reports/archi` 目录指引且与另两类文案相异）；点击 StaticCheck / TestExecution 节点经既有 stdout 面板回查 baseline 树输出；点击 ArchiCheck 节点不呈现 stdout 面板入口（零 `outputRef`），其 detail 与报告目录即为回查面

#### Scenario: 架构基线节点不因挂载点而分叉

- **WHEN** 同一 change 的图里同时存在 initialize 的 `archi_check` 节点与 implement 的 `archi_check` 节点
- **THEN** 两节点以同一组件、同一标签「架构检查」、同一 toolStep 分组徽章渲染，差异只来自 `phase` / `attempt` / `status` / `detail` 字段；前端零相位名判断分支

#### Scenario: clean 步词穷尽补齐

- **WHEN** bindings 再生成后审查 `STEP_LABEL` / `normalizeKind` / `stepWord` / `GROUP_LABEL`
- **THEN** `clean` 与 `archiCheck` 两键在案（人读文案非空）、snake ↔ camel 归一在单点可考、分别落 git / toolStep 分类；穷尽 Record 键集与二十二词全词汇一致（编译期兜底），其中三辅助步与三门 gate 五键为存量解码词不删

## Module Contract

| 模块 | 职责 | 关键契约（Component / 类型面） |
|------|------|------|
| `packages/desktop/src/views/changes/flow/run-state.ts`（零改动） | 步词归一单点 | `archi_check` ↔ `archiCheck` 映射已在案，新节点直接吃；`RunStepGroup` 分类不变 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx`（零改动） | 人读文案穷尽表 | `STEP_LABEL.archiCheck = '架构检查'` 已在案；`GROUP_LABEL` 四类不变 |
| `packages/desktop/src/views/changes/flow/*`（零改动） | 节点渲染与终态呈现 | 新节点由后端 emit 序驱动上图；无新组件、无新 data-testid 约定、无按相位分支 |
| `packages/desktop/src/types/generated/bindings.ts`（零 diff） | 词面 | `RunStepKind` 二十二值已在案，本变更零生成物变化 |
