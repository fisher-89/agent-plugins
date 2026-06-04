# 任务: code-driven-test-phases

> **变更**: code-driven-test-phases
> **日期**: 2026-06-03
> **基于**: design.md

---

## 阶段 1: 模板更新

此阶段更新 test-design.md.template 的结构以支持正向/反向 AC 分类，并移除参数类型/风险标记章节。

- [x] 1.1 修改 `plugins/dev-team/templates/artifacts/test-design.md.template`
  - 在覆盖映射章节之后新增「正向 AC」和「反向 AC」两个独立章节
  - 正向 AC 章节列出快乐路径业务场景，每项包含：AC ID、需求描述、测试层级
  - 反向 AC 章节列出异常路径业务场景，每项包含：AC ID、场景描述（错误处理、无效输入、边界条件）、测试层级
  - 移除所有参数类型表相关占位符
  - 确保反向 AC 章节强调从业务场景角度描述异常路径，不涉及具体参数类型

- [x] 1.2 验证 `test-design.md.template` 的章节结构
  - 确认 Forward ACs 和 Reverse ACs 章节存在且独立
  - 确认无参数类型表或参数类型相关字段
  - 确认无风险标记相关字段

## 阶段 2: test-design-planner 代理定义更新

此阶段修改 test-design-planner.md 以支持源码 Grep 补充输入、输出正向/反向 ACs、禁止输出参数类型和风险标记。

- [x] 2.1 修改 `plugins/dev-team/agents/test-design-planner.md`
  - 在 Input 部分新增：「Grep 源码提取变更涉及的真实 API 签名（函数名、参数类型、返回类型）作为补充输入」
  - 在 Process 部分新增：「2.5 分类输出正向 AC（Forward ACs）和反向 AC（Reverse ACs）」
  - 添加输出约束：「SHALL NOT output parameter type tables or risk markers」
  - 添加约束：「参数类型和风险标记不得出现在 test-design.md 的任何章节中」
  - 更新 Output 部分描述，明确新增正向/反向 AC 章节，强调不包含参数类型和风险标记

- [x] 2.2 验证 `test-design-planner.md` 的约束完整性
  - 确认禁止输出参数类型的约束已明确写入
  - 确认禁止输出风险标记的约束已明确写入
  - 确认 Grep 源码指令已添加
  - 确认 Forward/Reverse AC 分类指令已添加
  - 确认 tests/ 目录路径约束已移除（test-design 不再输出具体测试文件路径）

## 阶段 3: test-design-evaluator 代理定义更新

此阶段修改 test-design-evaluator.md 的 T2 检查项从 tests/ 路径检查改为 AC 覆盖检查。

- [x] 3.1 修改 `plugins/dev-team/agents/test-design-evaluator.md`
  - 更新 T2 检查项从「coverage map 条目包含 openspec/changes/<change-name>/tests/ 下的测试文件路径」改为「每个正向 AC（Forward AC）和反向 AC（Reverse AC）在 coverage map 中有对应条目」
  - 更新 T2 判断依据从「每行必须有 change 的 tests/ 目录下的具体文件路径」改为「逐项交叉验证正向 ACs 和反向 ACs 章节中每个 AC ID 与 coverage map 表格」
  - 更新静态检核表章节中 T2 的描述

- [x] 3.2 验证 `test-design-evaluator.md` 的 T2 检查项
  - 确认 T2 文本不再引用 `openspec/changes/<name>/tests/` 路径
  - 确认 T2 明确要求验证每个 Forward/Reverse AC 在 coverage map 中有对应条目
  - 确认其他检查项（T1, T3-T8）未受影响

## 阶段 4: test-gen-generator 代理定义更新

此阶段修改 test-gen-generator.md 以移除文件类型黑名单、添加源码读取能力、支持共存文件输出、集成参数类型→边界场景系统映射。

- [x] 4.1 修改 `plugins/dev-team/agents/test-gen-generator.md`
  - 删除 Constraints 中的「文件类型黑名单」约束（包括所有禁止读取的扩展名列表和违反惩罚描述）
  - 替换输出路径约束：从「所有测试文件写入 `openspec/changes/<change-name>/tests/`」改为根据源码语言写入同一目录
  - 添加语言特定的测试文件命名规范映射表到 Process 部分：
    - `.py` → `test_<module>.py`（同目录）
    - `.ts` / `.tsx` → `<module>.test.ts` / `<module>.test.tsx`（同目录）
    - `.rs` → `<module>_test.rs` 或 inline `#[cfg(test)] mod tests`（同目录）
    - `.go` → `<module>_test.go`（同目录）
  - 添加参数类型→边界场景系统映射表到 Process 部分（int→0/-1/MAX, str→空/超长/特殊字符, bool→True/False/None, list→[]/单元素/超大, dict→{}/缺失/多余, Optional[T]→None, Enum→枚举值/非法值, float→0.0/-0.0/NaN/Inf）
  - 添加指令：从源码读取方法签名和参数类型用于边界测试推导
  - 添加指令：对无类型文件的参数使用参数名推断类型（username→str, count→int, flags→boolean），结果标记为 P2 + TODO
  - 添加指令：测试骨架包含 TODO 或 skip 标记以防止被框架自动执行
  - 更新 Input 部分：移除 spec.md 文件作为唯一签名来源的描述，添加直接读取源码文件的描述
  - 更新 Process 部分：移除「4. 对于 coverage map 中每项在 `openspec/changes/<name>/tests/` 下创建测试文件」改为「在源码文件所在目录创建共存测试文件」

- [x] 4.2 验证 `test-gen-generator.md` 的修改完整性
  - 确认文件类型黑名单已被完全移除（无 `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.rs`, `.go`, `.java` 等限制）
  - 确认包含每种语言的测试文件命名规范映射
  - 确认包含参数类型→边界场景系统映射表
  - 确认包含对无类型文件的参数名推断指令及 P2 + TODO 标记规则
  - 确认输出路径不再指向 `openspec/changes/<name>/tests/`
  - 确认 tests/ 目录相关约束已被全部替换

## 阶段 5: test-gen-evaluator 代理定义更新

此阶段修改 test-gen-evaluator.md 的 G1 和 G2 检查项以适配共存文件模式。

- [x] 5.1 修改 `plugins/dev-team/agents/test-gen-evaluator.md`
  - 更新 G1 检查项：
    - 原：「test-design.md 中每个 coverage map 条目在 `openspec/changes/<change-name>/tests/` 下都有对应的测试文件」
    - 改为：「源码中每个公开方法在源码目录中有对应的测试文件」
    - 更新判断依据：从「逐项交叉验证 coverage map 每行与 git diff 中 tests/ 目录下的文件」改为「逐项交叉验证源码目录中每个受影响的公开方法与对应的共存测试文件」
  - 更新 G2 检查项：
    - 原：「测试文件遵循项目命名规范且位于 `openspec/changes/<change-name>/tests/`」
    - 改为：「测试文件命名遵循语言规范且与源码共存于同一目录」
    - 更新判断依据：从「检查文件名匹配现有模式（test_*.py、*.test.ts 等）且位于 change 的 tests/ 目录下」改为「检查文件名匹配语言规范（test_*.py、*.test.ts、*_test.rs、*_test.go）且存在于源码文件的同一目录」
  - 更新 G5 检查项判断依据：从「每个边界情况必须有对应的测试骨架」保持基本语义但添加「至少覆盖类型映射表中每种参数类型的 2 个边界值」的补充说明

- [x] 5.2 验证 `test-gen-evaluator.md` 的修改完整性
  - 确认 G1 不再引用 `openspec/changes/<name>/tests/` 路径
  - 确认 G2 不再引用 `openspec/changes/<name>/tests/` 路径
  - 确认 G1 和 G2 明确要求与源码共存且命名符合语言规范
  - 确认 G5 包含边界场景的覆盖要求
  - 确认其他检查项（G3, G4, G6, G7）未受影响

## 阶段 6: 验证与插件版本更新

此阶段验证所有修改的完整性并更新插件版本号。

- [x] 6.1 确认 `plugins/dev-team/skills/phase-test-design/SKILL.md` 无需修改
  - 对比 spec 要求：技能保持单行 prompt + gate check + verdict 循环的编排模式
  - 确认 prompt 文本不包含 test-design 阶段的输出约束或路径信息

- [x] 6.2 确认 `plugins/dev-team/skills/phase-test-gen/SKILL.md` 无需修改
  - 对比 spec 要求：技能保持单行 prompt + gate check + verdict 循环的编排模式
  - 确认 prompt 文本不指定输出目录或文件黑名单

- [x] 6.3 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号
  - 从 `2.5.5` 升级到 `2.6.0`（次版本升级，新增源码驱动能力和共存文件模式）

- [x] 6.4 对所有修改文件执行最终完整性检查
  - 确认 4 个 agent.md 文件格式正确（YAML frontmatter + Markdown 主体）
  - 确认 1 个模板文件格式正确（Markdown table 对齐、占位符完整）
  - 确认 2 个 skill 文件无变更
  - 确认 git diff 只包含预期修改的 5 个文件 + 1 个版本文件
