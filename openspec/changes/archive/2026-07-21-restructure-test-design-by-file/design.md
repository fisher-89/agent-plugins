# 设计: restructure-test-design-by-file

> **变更**: restructure-test-design-by-file
> **日期**: 2026-07-21

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| test-design 模板 | 定义 test-design.md 的结构格式，包含 per-file 单元测试章节和自由命名关系标题的集成测试章节 | `plugins/dev-team/templates/artifacts/test-design.md.template` | — | Markdown (模板) |
| test-design-planner agent | 读取 proposal.md/design.md，调用 test_resolve_paths 和 test_detect_frameworks，按新模板结构生成 test-design.md | `plugins/dev-team/agents/test-design-planner.md` | test-design.md.template, test_resolve_paths, test_detect_frameworks | Markdown (agent spec) |
| test-design-evaluator agent | 根据 checklist 评估 test-design.md 的完整性和格式合规性，T8 格式检查适配新结构 | `plugins/dev-team/agents/test-design-evaluator.md` | test-design.md.template, proposal.md | Markdown (agent spec) |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
-->

### 新增文件

<!-- 本次变更为纯模板和 agent prompt 调整，无新增文件 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/templates/artifacts/test-design.md.template` | **全结构重写**。单元测试从两层扁平表格（`### 用例` + `### Mock策略`）重组为 per-file 分组结构；集成测试从两层扁平表格（`### 用例` + `### Mock策略`）重组为自由命名关系标题 + 涉及模块 + 场景级结构 | 模板结构调整，满足 AC-1 ~ AC-5 |
| `plugins/dev-team/agents/test-design-planner.md` | 更新 Process 步骤中集成测试场景生成逻辑（从分类关系改为自由命名关系标题）、单元测试输出映射逻辑（从全局表格映射改为 per-file 章节映射）；更新 Output 指令 | 生成逻辑与模板一致，满足 AC-6 |
| `plugins/dev-team/agents/test-design-evaluator.md` | 重写 T8 格式检查项：新增 per-file 分组标题模式检查、关系标题 `→` 模式检查、涉及模块表格 ≥2 行检查、关联AC 非空检查、关系描述非空检查、场景标题格式和用例表列名检查；更新 T2 交叉引用检查为新结构 | 检查逻辑适配新模板，满足 AC-7 |

#### 模板修改明细

**单元测试章节 (AC-1, AC-3, AC-4):**

```
BEFORE:                               AFTER:
## 单元测试                            ## 单元测试
### 用例                              ### {{源文件}} -> {{测试文件}}
  | 测试文件 | 测试对象 | 路径类型 | ...   #### 待测功能 [列表]
### Mock策略                           #### 用例 [表: 测试对象|路径类型|测试条件|迭代类型]
  | 测试文件 | Mock主体 | Mock方案 | ...   #### Mock策略 [表: Mock主体|Mock方案|应用场景]
                                        ### {{源文件2}} -> {{测试文件2}}
                                          #### 待测功能
                                          #### 用例
                                          #### Mock策略
```

- `测试文件` 列从 `#### 用例` 和 `#### Mock策略` 表中移除（已在 heading 中声明）
- 新增 `#### 待测功能` 列表小节

**集成测试章节 (AC-2, AC-3, AC-4, AC-5):**

```
BEFORE:                               AFTER:
## 集成测试                            ## 集成测试
### 用例                              ### {{关系标题}} → {{测试文件}}
  | AC ID | 测试文件 | 测试场景 | ...   **涉及模块** [表: 模块|角色]
### Mock策略                           **关联AC**: AC-1, AC-2
  | 测试文件 | Mock主体 | Mock方案 | ...   **关系描述** [段落]
                                        #### 场景: {{场景名称}}
                                          [场景描述段落]
                                          ##### 用例 [表: 路径类型|测试条件|迭代类型]
                                          ##### Mock策略 [表: Mock主体|Mock方案|应用场景]
                                        #### 场景: {{场景名称2}}
                                          ...
```

- 关系标题自由命名，建议使用箭头链路格式（如 `CLI参数 → eval.json持久化`）
- 新增 `**涉及模块**` 表格（强制显式声明跨模块性）
- 新增 `**关联AC**` 元数据
- 新增 `**关系描述**` 叙事段落
- 按场景组织（`#### 场景:`），场景包含叙事描述 + `##### 用例` 表 + 可选 `##### Mock策略`
- 用例表列简化（去掉 `AC ID`、`测试文件`、`测试场景`，改为 `路径类型 | 测试条件 | 迭代类型`）
- Mock 策略仅限跨进程边界

**验收范围 (保持不变):**

`## 验收范围` 表保持现有 5 列：`AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景`。本变更不修改此章节。

#### Planner 修改明细

**Process 步骤变更 (AC-6):**

| 步骤 | 当前内容 | 修改后内容 |
|------|----------|-----------|
| Step 6 | 从 proposal.md/design.md 汇总精确模块列表 | **不变**（模块列表汇总逻辑不受影响） |
| Step 7 | 调用 test_detect_frameworks 识别测试框架 | **不变** |
| Step 8 | "从 proposal.md / design.md 识别需要集成测试覆盖的场景。对每个场景，在受测模块最近的 `__tests__/<scenario>/` 目录下创建集成测试用例" | 改为：从 proposal.md / design.md 识别跨模块交互，为每个交互生成自由命名的关系标题（使用 `→` 箭头链路格式）。对每个关系，确定涉及模块（≥2个）及其角色，生成 `**涉及模块**` 表格、`**关联AC**`、`**关系描述**` 段落。对每个关系生成一个或多个 `#### 场景:` 子章节，每场景包含叙事描述 + `##### 用例` 表 + 可选 `##### Mock策略`（仅跨进程边界） |
| Step 9 | 调用 test_detect_frameworks | 移至第 8 步之后，用于确定集成测试文件扩展名 |
| Step 10 | 调用 test_resolve_paths 获取单元测试路径 | **调整输出映射**：不再填充全局 `### 用例` 表，而是为每个 `source -> test_file` 对创建 `### <source> -> <test_file>` 章节 |
| Step 11 | 将 unit_tests 映射到验收范围与单元测试 > 用例 | 改为：将 `unit_tests` 映射到验收范围，并为每个 `source -> test_file` 对生成独立的 `###` 章节。每个章节包含 `#### 待测功能`（从源码 grep 导出函数列表）、`#### 用例` 表（无 `测试文件` 列）、`#### Mock策略` 表（无 `测试文件` 列） |
| Step 12 | 若 test_resolve_paths 有 error 则记录不可测试项 | **不变** |
| Step 13 | 读取 test-design.md.template 确认占位符 | **调整**：确认模板结构与新格式一致 |
| Step 14 | 写入 test-design.md | **调整**：改为按 per-file 章节分段写入单元测试，按 per-relationship 章节分段写入集成测试 |

#### Evaluator 修改明细

**T8 格式检查重写 (AC-7):**

| 检查维度 | 当前 T8 逻辑 | 修改后 T8 逻辑 |
|---------|-------------|---------------|
| 单元测试分组 | 检查 `### 用例` / `### Mock策略` 存在性 | 检查每个 `###` 标题匹配 `{{source}} -> {{test_file}}` 模式；检查每章节包含 `#### 待测功能`、`#### 用例`、`#### Mock策略` |
| 单元测试表格列 | 检查 `### 用例` 表列: `测试文件\|测试对象\|路径类型\|测试条件\|迭代类型` | 检查 `#### 用例` 表列: `测试对象\|路径类型\|测试条件\|迭代类型`（无 `测试文件` 列） |
| 单元测试 Mock 表列 | 检查 `### Mock策略` 表列: `测试文件\|Mock主体\|Mock方案\|应用场景` | 检查 `#### Mock策略` 表列: `Mock主体\|Mock方案\|应用场景`（无 `测试文件` 列） |
| 待测功能格式 | 无检查 | 检查 `#### 待测功能` 使用无序列表格式（`- func(): desc`），非表格 |
| 集成测试分组 | 检查 `### 用例` / `### Mock策略` 存在性 | 检查每个 `###` 标题包含 `→ test_file` 模式 |
| 涉及模块 | 无检查 | 检查 `**涉及模块**` 表存在，列名 `模块\|角色`，≥2 行数据 |
| 关联AC | 无检查 | 检查 `**关联AC**` 非空 |
| 关系描述 | 无检查 | 检查 `**关系描述**` 段落非空 |
| 场景存在性 | 无检查 | 检查每个关系至少一个 `#### 场景:` |
| 场景描述 | 无检查 | 检查场景标题后跟叙事描述段落（非空） |
| 场景用例表列 | 无检查 | 检查 `##### 用例` 表列: `路径类型\|测试条件\|迭代类型` |
| 场景 Mock 表列 | 无检查 | 检查 `##### Mock策略` 表列（如存在）：`Mock主体\|Mock方案\|应用场景` |
| 验收范围 | 检查模板一致 | 检查 5 列不变：`AC ID\|验收条件\|测试类型\|测试文件\|测试对象/测试场景` |

**T2 交叉引用检查更新:**

| 检查维度 | 当前 T2 逻辑 | 修改后 T2 逻辑 |
|---------|-------------|---------------|
| 单元测试 | 检查验收范围中每个 单元测试 条目在 `### 用例` 表中存在 | 检查验收范围中每个 单元测试 条目对应的 `测试文件` 出现在 `### <source> -> <test_file>` heading 中 |
| 集成测试 | 检查验收范围中每个 集成测试 条目在 `### 用例` 表中存在 | 检查验收范围中每个 `AC ID`（测试类型=集成测试）在至少一个关系的 `**关联AC**` 中出现；检查每个关系 `**关联AC**` 中所有 AC ID 在验收范围中存在 |

### 公共函数 / API

<!-- 本次变更为纯模板和 agent prompt 调整，无运行时源码变更，无公共函数/API 新增或修改 -->

### 类型定义

<!-- 本次变更为纯模板和 agent prompt 调整，无运行时源码变更，无类型定义新增或修改 -->

### 配置

<!-- 本次变更为纯模板和 agent prompt 调整，无运行时代码变更，不涉及配置文件键值新增或修改 -->

---

## 数据模型

### 模板数据结构

模板本身定义的是文档结构，不是编程数据模型。以下描述的是 test-design.md 产物的逻辑数据模型：

**单元测试章节（per-file）:**

```
UnitTestSection:
  - source: string        // 源文件路径
  - test_file: string     // 测试文件路径
  - 待测功能: Function[]   // 列表
      - name: string      // 函数名
      - description: string // 简短描述
  - 用例: TestCase[]      // 表
      - 测试对象: string   // describe 标题
      - 路径类型: "正向" | "异常" | "边界"
      - 测试条件: string   // it 标题
      - 迭代类型: "新增" | "废弃"
  - Mock策略: MockEntry[] // 表
      - Mock主体: string   // 文件/接口/全局变量/运行环境
      - Mock方案: string   // 具体 mock 路径、输入输出
      - 应用场景: string   // 需要此 mock 的 describe 列表
```

**集成测试章节（per-relationship）:**

```
IntegrationSection:
  - 关系标题: string       // 自由命名，如 "CLI参数 → eval.json持久化"
  - test_file: string     // 测试文件路径
  - 涉及模块: ModuleRole[] // 表
      - 模块: string       // 模块路径
      - 角色: string       // 如 写入方/读取方/中间件/触发方
  - 关联AC: string[]       // AC-ID 列表
  - 关系描述: string       // 叙事段落
  - 场景: Scenario[]       // 一个或多个
      - 场景名称: string
      - 场景描述: string    // 叙事段落（验证什么+前置条件+输入+预期输出）
      - 用例: TestCase[]   // 表
          - 路径类型: "正向" | "异常" | "边界"
          - 测试条件: string
          - 迭代类型: "新增" | "废弃"
      - Mock策略?: MockEntry[] // 可选，仅跨进程边界
```

**验收范围（unchanged）:**

```
AcceptanceScope:
  - AC ID: string
  - 验收条件: string
  - 测试类型: "单元测试" | "集成测试"
  - 测试文件: string
  - 测试对象/测试场景: string
```

### 数据流

```
修改前:
  proposal.md + design.md
    → test-design-planner
      → 生成全局 ### 用例 表（所有文件混合）
      → 生成全局 ### Mock策略 表（所有文件混合）
      → 生成 ### 用例 + ### Mock策略 集成测试（所有场景混合）

修改后:
  proposal.md + design.md
    → test-design-planner
      → test_resolve_paths 获取 unit_tests pairs
      → 遍历每个 pair，生成 ### <source> -> <test_file>
          → grep 源码获取导出函数 → #### 待测功能（列表）
          → 测试用例设计 → #### 用例（表，无测试文件列）
          → Mock 策略 → #### Mock策略（表，无测试文件列）
      → 从 proposal/design 识别跨模块交互
      → 对每个交互，生成 ### <关系标题> → <test_file>
          → 确定涉及模块 → **涉及模块**（表，≥2行）
          → 关联 AC → **关联AC**
          → 交互描述 → **关系描述**（段落）
          → 对每个场景 → #### 场景:
              → 场景描述（段落）
              → 用例 → ##### 用例（表）
              → Mock → ##### Mock策略（可选）
```

---

## 路由/API 设计

<!-- 本次变更不涉及 HTTP API 或 MCP 工具变更 -->

---

## 依赖

### 运行时依赖

- 无新增运行时依赖（纯模板和 agent prompt 调整）

### 构建/测试依赖

- 无新增构建/测试依赖

### 逻辑依赖（只读，不修改）

- `plugins/dev-team/agents/test-gen-generator.md` — 下游消费者，仅消费表格行数据，不受章节重组影响
- `plugins/dev-team/agents/test-gen-evaluator.md` — 同上
- `mcp__plugin_dev-team_dev-team__test_resolve_paths` — test-design-planner 用于获取 unit_tests pairs（已有，不修改）
- `mcp__plugin_dev-team_dev-team__test_detect_frameworks` — test-design-planner 用于识别测试框架（已有，不修改）

---

## 待决问题

1. **插件版本号是否需要递增。** 本次变更为纯模板和 agent prompt 调整，无 `plugins/dev-team/bin/` 目录下的运行时代码变更。根据语义化版本惯例，仅修改模板和 agent prompt（Markdown 文档）不构成功能增强或修复，可能无需版本递增。待与用户确认。

2. **验收范围表格结构与 per-file 章节的关系。** 新模板中 `## 验收范围` 的 `测试文件` 列仍保持 flat 结构（每行一个测试文件），而 `## 单元测试` 改为 per-file 分组。Per-file 章节的 `#### 用例` 表不再有 `测试文件` 列，但需要保证 `测试对象` 列与验收范围中 `测试对象/测试场景` 列一致，以维持 T2 交叉引用的可追溯性。planner 需在生成时确保一致性。

3. **关系标题与验收范围的映射关系。** 一个关系标题可能关联多个 AC（如 AC-1, AC-2），一个 AC 也可能出现在多个关系中（如 AC-1 同时被单元测试和集成测试覆盖）。T2 检查需要处理多对多映射——验收范围中标记为集成测试的 AC 必须出现在至少一个关系的 `**关联AC**` 中，但不需要所有出现该 AC 的关系被联动。当前设计中这是一个隐式约束，无需额外建模。

4. **集成测试场景标题的去重约束。** 不同关系下可能出现同名的场景标题（如 "正常传递"），planner 应按关系范围确定唯一性——同关系下场景名唯一，跨关系不要求唯一。此约束在模板注释和 planner 指令中体现即可，不需要架构层面的强制。
