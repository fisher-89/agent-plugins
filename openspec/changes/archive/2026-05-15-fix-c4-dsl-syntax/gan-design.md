# Architecture Subagent 优化建议

## A. 结构性优化

### A1. DSL 解析逻辑重复——提取共享模块

`archi-model.py` 和 `archi-validate.py` 各自实现了一套 DSL 解析器（`_parse_dsl` vs `_parse_model_dsl`）、metadata 解析器、path array 解析器。它们的实现几乎相同但又有微妙差异（例如 `_parse_metadata_kv` 在两个文件中签名不同）。

**建议**：提取 `archi-parser.py` 共享模块，消除 ~150 行重复代码，也消除两个解析器行为不一致的风险。

### A2. `extend` 块内的 relationship 解析有 bug

两个解析器中都有这个条件：

```python
if "->" in stripped and current_element is None and not extend_stack:
```

`not extend_stack` 意味着在 `extend` 块内部声明的 relationship 会被**静默忽略**。但实际上用户完全可能在 `extend` 块内写 relationship（例如在同一个 domain 内声明 module 间的依赖）。这是一个功能缺陷。

### A3. Validation 过于浅层

`_validate_structure` 只检查了：
- 括号平衡
- 是否存在 `specification` 和 `model` 关键字（字符串搜索，不是块解析）
- Colon syntax 和 flat metadata 两种已知错误

缺少的验证：
- element kind 是否在 specification 中声明过
- FQN 引用的 element 是否存在（如 `extend Foo.Bar` 但 `Foo.Bar` 从未定义）
- relationship 的 source/target 是否都是已声明的 element
- 同一个 FQN 是否被重复定义

**建议**：增加语义验证层（在语法验证之上），这能显著减少 Agent 需要用 LLM 推理来发现的错误数量。

## B. Agent Prompt 优化

### B1. 缺少模式路由指令

Prompt 定义了四种模式，但没有告诉 Agent **如何判断用户意图属于哪种模式**。当用户说"检查一下架构"时，是 VALIDATE 还是 REVIEW？当用户说"帮我加个 ADR"时，Agent 需要自己推断是 DECIDE 模式。

**建议**：增加一个 "Mode Selection" 段落，给出意图→模式的映射规则，例如：
- "validate / check code / verify imports" → VALIDATE
- "review / critique / audit model" → REVIEW
- "add / update / propose / change model" → PROPOSE
- "ADR / decision / record" → DECIDE

### B2. REVIEW 模式完全依赖 LLM 推理——可工具化

REVIEW 模式要求 Agent 读模型并自行评估 completeness/hierarchy/coupling。但很多检查可以用 Python 确定性完成：
- 孤立 element（无 relationship）→ 遍历 elements + relationships
- 悬空 relationship（source/target 不存在）→ 集合差运算
- 没有 `metadata.path` 的 element → 字段检查
- 重复 "contains" relationship → 对比 extend 嵌套 vs relationship

**建议**：在 `archi-model.py` 增加 `--command review` 子命令，输出结构化的 quality report，让 LLM 只负责解释和建议，而不是自己数 relationship。

### B3. PROPOSE 模式的 validate 步骤有歧义

步骤 4 说：

> validate the aggregated model if changes span files

但没有说明**如何**聚合。Agent 只能 validate 单个 `--source` 输入。如果用户在 `02-hooks.c4` 里增加元素，但 specification 在 `01-core.c4` 里，单独 validate `02-hooks.c4` 会因为 "Missing specification block" 而失败。

**建议**：增加 `archi-model.py --command validate --file models/02-hooks.c4`（先聚合再验证指定文件的增量）或者文档明确说明 Agent 应该把所有文件拼接后整体 validate。

## C. 集成与运维优化

### C1. Commit Gate 报告检查过于宽松

`architecture.py` hook 只检查是否有 `validate-*.json` 文件被 staged，但不检查：
- 报告是否 pass（可能报告本身有 violations）
- 报告的 `commit_diff_hash` 是否匹配当前 staged diff（可能是旧报告）

这意味着用户可以 stage 一个旧的/失败的报告来绕过 gate。

**建议**：检查报告的 `status` 字段，至少拒绝 `violations_found` 状态的报告。理想情况下还应验证 `commit_diff_hash` 匹配。

### C2. `update-architecture` Skill 只暴露 PROPOSE 模式

当前 skill 定义明确限制：

> This skill only supports PROPOSE mode

用户无法通过 slash command 触发 VALIDATE / REVIEW / DECIDE。要么扩展这个 skill 支持 mode 参数，要么增加额外的 skill（如 `/dev-team:architecture-review`, `/dev-team:architecture-validate`）。

### C3. `_PYTHON_STDLIB` 列表硬编码且不完整

`archi-validate.py` 中硬编码了 stdlib 模块列表，缺少 `dataclasses`, `contextlib`, `glob`, `fnmatch`, `pprint`, `signal`, `ctypes` 等常用模块。Python 3.10+ 有 `sys.stdlib_module_names` 可以动态获取。

## D. 优先级建议

| 优先级 | 建议 | 影响 |
|---|---|---|
| **P0** | A2: 修复 extend 块内 relationship 解析 | 功能正确性 |
| **P0** | C1: Commit gate 检查报告状态 + diff hash | 安全性 |
| **P1** | A1: 提取共享 DSL 解析模块 | 可维护性 |
| **P1** | B1: 增加模式路由指令 | Agent 准确性 |
| **P1** | B3: 修复跨文件 validate 歧义 | 用户体验 |
| **P2** | B2: REVIEW 模式工具化 | 质量 + 速度 |
| **P2** | A3: 增加语义验证 | 减少 LLM 负担 |
| **P2** | C2: 扩展 skill 暴露更多模式 | 可用性 |
| **P3** | C3: 动态获取 stdlib 列表 | 准确性 |
