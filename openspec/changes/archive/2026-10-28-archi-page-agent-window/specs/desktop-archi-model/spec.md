# desktop-archi-model Specification（delta: archi-page-agent-window）

## MODIFIED Requirements

### Requirement: 消费面词汇集一致性对拍

四个消费面（进程内诊断集、`archi_module` 工具响应、IPC DTO、**架构会话知识文本**）SHALL 对同一份模型语料覆盖**相同的词汇集**。需全覆盖的封闭词汇：

```
element kind   package / domain / module / component
gate 档        off / warn / strict
结构诊断       syntaxError / missingField / invalidValue / unknownField /
               duplicateElement / cycle / dependsOnAncestor /
               unknownReference / orphanHierarchy
文档诊断       docNotFound
边标记         none / unfulfilled / crossKind / exempt
节点标记       none / unfulfilled
```

对拍 SHALL 以一份**含全部词汇**的模型语料驱动四面投影，断言四面的词汇集相等；新增或删除任一词汇而语料 / 投影未同步时对拍 MUST 变红（防「新增一个诊断类型、UI 静默不显示」）。对拍 SHALL 为机械测试（形态沿用 `src/views/changes/flow/run-state.test.ts` 的全词汇出图先例：一份语料走全投影、逐词汇断言在场）。

- **`ALL` 全量常量为唯一派生源**：`element kind` / `gate` 档 / 结构诊断三类封闭词汇 SHALL 各自提供全量常量（`ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL`；其中 `DiagnosticKind::ALL` 由本变更补齐为九值），且 SHALL 与各自的线格式词函数（`as_str`）逐值对拍。派生型消费者（当前唯一 = 架构会话知识文本，见 `desktop-archi-agent`「架构会话的系统知识注入」）MUST 从这些常量取词，MUST NOT 在产品源码或静态文本里手抄一份词表。
- **知识文本纳入射程**：架构会话知识文本出现在四面之列——它必须含全部 `kind` / `gate` / 结构诊断词；新增词汇而知识文本未同步（因为绕过了 `ALL`）MUST 由对拍或 `ALL` 对拍变红。

可选字段的**线形态**（缺字段 vs `null`）不在本对拍射程内（见 proposal R-3）；本对拍只锚定词汇集。

#### Scenario: 语料含全词汇

- **WHEN** 检查对拍语料
- **THEN** 上述每一词汇至少各一条在场（`kind` 四类元素、`gate` 三档、九类结构诊断各一坏点、`docNotFound` 一例、边标记四值、节点标记两值）

#### Scenario: 三面（诊断 / 工具 / IPC）词汇集相等

- **WHEN** 同一语料分别经进程内诊断集、`archi_module` 响应与 IPC DTO 投影
- **THEN** 三面枚举出的词汇集完全相等；任一面的缺失项使测试失败并点名缺失词汇

#### Scenario: 新增词汇未同步即红

- **WHEN** 代码新增一个诊断类型或标记值而语料 / 对拍未同步
- **THEN** 对拍失败（词汇集不等），MUST NOT 静默通过

#### Scenario: 知识文本与三面同词汇集

- **WHEN** 取架构会话知识文本（`source = "archi"` 的 `preamble` 内容）并枚举其词汇
- **THEN** 其中含 `kind` 四值、`gate` 三值（含缺省 `off`）、结构诊断九词，与另三面词汇集相等；把某个词从 `ALL` 删掉而知识侧仍能拼出该词（手抄）时测试红

#### Scenario: ALL 与线格式词逐值对拍

- **WHEN** 遍历 `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL`
- **THEN** 每一项的 `as_str` 产出唯一且与语料词一致；新增变体未补入对应 `ALL`（长度断言）时测试红

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/dto.rs` | 封闭词汇单点（补全量常量） | `ArchiDiagnosticKind::ALL: [Self; 9]`（与 `as_str` 同源；长度与词面双向对拍）；既有 `ElementKind::ALL` / `GateTier::ALL` 语义零变 |
| `crates/core/archi/tests/vocabulary_test.rs`（扩展） | 四面词汇对拍 | 语料 → 进程内诊断集 / `archi_module` 串 / IPC DTO / 架构会话知识文本四面词汇集相等；`ALL` 与 `as_str` 逐值对拍 |
