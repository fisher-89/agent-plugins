---
name: js-unit-test
description: 为已有的 JS/TS/JSX/TSX 代码生成 colocated 单元测试，跑到通过、核对覆盖率，并产出突变测试配置。适用于给存量代码补测试。
version: 1.0.1
---

# js-unit-test

给**已有**的 JS/TS 代码补 colocated 单元测试：反构设计 → 设计用例 → **用户确认** → 逐文件生成 → 跑到通过 → **覆盖率核对** → 产出突变配置。

## 硬约束

| 约束 | 说明 |
|---|---|
| 不改生产代码 | 撞见被测代码的 bug 时停止并报告，不得自行修复 |
| 只写四类文件 | 项目级画像 `openspec/test-profile.md`、`openspec/changes/<name>/` 下的设计产物、colocated 测试文件、突变配置；覆盖率报告目录由测试工具生成，不算产物 |
| 不探测框架版本 | 直接使用项目自己的测试命令 |
| 无快速通道 | 阶段 1–3 不可跳过 |
| 设计必确认 | `test-design.md` 写完后由用户确认，确认前不得生成测试文件 |
| 覆盖率有门 | 能收集就收集，低于 `lines 80 / branches 70 / functions 80` 必须归因，不得静默略过 |
| 回补有界 | 覆盖率不足只回补流程内问题（阶段 2–5）；环境类问题只写报告，不动源码 |
| 突变不代跑 | 只产出配置，由用户手动执行 |

`references/` 下的文件按需读取，路径为 `<本 skill 的 base directory>/references/<name>.md`。分支判定按主题分散在各 reference 里，走到对应判断点再读，不要预先通读。

## 输入

用户给出其一：

- **change 名**（kebab-case）+ 要补测试的范围描述
- 或仅有范围描述，由你推导 change 名并告知用户

范围描述可以是模块路径、目录、或功能描述。**范围决定边界，不决定是否扫描仓库。**

## 项目根

下文所有 `openspec/...` 路径均相对**项目根**。项目根取当前工作目录（cwd）；若 cwd 深处某个子目录，向上查找最近的 `package.json` 或 `.git` 作为项目根，并在开始前告知用户你认定的根路径。

`openspec/` 目录不存在时按需创建（`mkdir -p` 语义），不要求预先存在。

## 阶段 0：测试环境画像

读取 `openspec/test-profile.md`（相对项目根）。

**存在** → 按 `references/project-detection.md` 的「过期判定」抽查，通过则直接用，进入阶段 1。

**不存在** → 执行首轮扫描（步骤见 `references/project-detection.md`），写入 `openspec/test-profile.md`，**展示给用户确认**后再继续。

多测试域时按范围做路径前缀匹配，判据见同文件的「多包时的目标选择」。

## 阶段 1：定范围

产出**模块清单**：本次要补测试的源文件列表（相对项目根，POSIX 风格）。

- 边界由用户输入决定；范围外的文件不纳入
- 清单只含**源文件**，不含测试文件
- 若清单为空或与用户描述不符，回到用户澄清

清单暂存于当前对话，不落盘——它会在阶段 3 转为 `test-design.md` 的章节。

## 阶段 2：反构

写入 `openspec/changes/<name>/design.md`，结构见 `references/design-template.md`。

要点：

- 描述**已存在**的架构，不提出新设计
- `变更清单` 以本次模块清单为界
- 逐文件列出公共导出（函数 / 类及其公开方法），签名从源码读取
- 中文叙述；标识符、路径、命令保持英文
- 不含测试策略、测试架构、用例设计（那是阶段 3）

若 `design.md` 已存在：向用户确认是覆盖还是沿用（处置见 `references/design-template.md` 的「已存在时」）。

## 阶段 3：设计

写入 `openspec/changes/<name>/test-design.md`，结构见 `references/test-design-template.md`。

要点：

- `## 单元测试` 下**按源文件分节**：一个源文件一节，标题形如 `### <源文件> -> <测试文件>`
- 每节的 `用例` 表覆盖该源文件的全部公共导出，含正向 / 异常 / 边界三类
- 边界用例按 `references/edge-cases.md` 的类型映射系统推导
- `Mock策略` 表遵守 `references/mock-rules.md` 的最小 mock 原则
- 测试文件路径按 `references/test-file-conventions.md` 的「命名规范」推导
- 中文叙述

**这份文件是阶段 4 的续跑锚**：其分节与 colocated 测试文件一一对应。分节标题必须精确，否则中断后无法比对进度。

### 用户确认（阶段 3 出口）

写完后**停在这里**，用 `AskUserQuestion` 请用户确认 `test-design.md`。确认结果的四种去向见 `references/test-design-template.md` 的「确认出口」。

## 阶段 4：生成

按 `test-design.md` 的章节逐节生成测试文件。**分批执行**，每批处理一个源文件（同目录内相关性高时，单批可放宽至 ~10 个源文件或估算低于上下文 40%）。

单批流程：

```
Read test-design.md 的该节
Read 该源文件（拿到真实签名、参数类型、返回类型、错误处理）
Read 同目录已有测试文件（学习命名、导入、断言风格）
Write 对应的 colocated 测试文件
```

测试代码要求见 `references/test-file-conventions.md` 与 `references/frameworks.md`，不引入新依赖。

**续跑**：每批开始前，检查该节对应的测试文件是否已存在且非空；是则跳过该节。其余判据见 `references/resume.md`。

## 阶段 5：跑绿与覆盖率

### 5a 跑绿

先按 `references/coverage.md` 的前置判断确认覆盖率能否采集，据此决定命令：

```
Bash: <画像中的测试命令> --coverage     # 可采集
Bash: <画像中的测试命令>               # 不可采集
```

可采集时**一次执行同时拿到通过结果和覆盖率**，不要为了覆盖率多跑一遍。

退出码为 0 直接进 5b。非零时按 `references/frameworks.md` 的「失败时的读法」读输出尾部，再按 `references/coverage.md` 的「退出码」分叉：用例失败改测试重跑（上限 3 轮）；撞见产码 bug 则停止并交还用户。

**这条分叉不可越过。** 本 skill 的职责是补测试，不是修业务逻辑。撞见产码 bug 时交还用户，同时说明已完成的测试文件仍然有效。

### 5b 覆盖率判定

门禁（统计范围是阶段 1 的模块清单，不是全仓库）：

| 维度 | 阈值 |
|---|---|
| lines | 80 |
| branches | 70 |
| functions | 80 |

`references/coverage.md` 的「读法与判定」给出摘要表读法；三个维度全部达标 → 阶段 5 完成，进入阶段 6。采集不可用或任一维度低于阈值 → 按 `references/coverage.md` 的处置走：不可采集记原因直接进阶段 6；未达标进入 5c 归因。

### 5c 覆盖率归因

逐条读未覆盖行 / 分支，分成「流程内问题」与「环境问题」两类，分类规则、回补目标阶段与回补上限见 `references/coverage.md` 的「归因与回补」。环境类不改源码，带文件与行号记入最终报告。

## 阶段 6：突变配置

先查项目是否已有 `stryker.config.{js,ts,mjs,cjs,json}`：有则记录路径、不再生成；无则生成 `openspec/changes/<name>/mutation/stryker.config.json`。查找范围、模板、字段填法与依赖检查见 `references/mutation-config.md`。

产出后**不执行**。阶段 7 只给出一行运行命令（见同一 reference 的「运行命令」）。

## 阶段 7：简报

输出：

- change 名与产物路径
- 生成的测试文件清单（路径 + 用例数）
- 覆盖的源文件数 / 模块清单中的总数
- 未覆盖或特意跳过的部分及原因
- 测试执行结果（通过数 / 总数）
- 覆盖率：`lines` / `branches` / `functions` 三维度实测值与阈值对比，附采集命令
- 覆盖率未达标时的归因：已回补的轮次与改动，以及留给用户的环境类缺口（源码 / 接口 / 文档 / 环境限制，逐条带文件与行号）
- 突变配置位置与运行命令
- 下一步建议（如"需要调整用例可重跑阶段 3"）

## 中断与恢复

进度判据全部来自磁盘，不维护状态文件：见 `references/resume.md`。新会话再次调用时从阶段 0 重新进入，据该表自动推进到未完成的阶段。
