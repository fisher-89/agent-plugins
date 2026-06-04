# 测试设计: code-driven-test-phases

> **变更**: code-driven-test-phases
> **日期**: 2026-06-03
> **基于**: proposal.md, design.md, specs/phase-agents/spec.md, specs/phase-skills/spec.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | 逐个验证 agent 定义文件（test-design-planner.md、test-design-evaluator.md、test-gen-generator.md、test-gen-evaluator.md）和模板文件（test-design.md.template）是否包含正确的指令、约束和检查清单项。验证 skill 文件（phase-test-design/SKILL.md、phase-test-gen/SKILL.md）是否未发生变更。 | Python 脚本（re / ast / yaml 解析）+ shell 断言 | 100% 覆盖每份文件的关键指令存在性、约束移除/添加、检查项语义变更 |
| 集成测试 | 模拟完整 P→E 和 G→E 工作流执行：通过 dev-team MCP 工具链调用代理，验证 test-design.md 输出包含正向 AC 和反向 AC 但不含参数类型/风险标记；验证 test-gen 输出与源码文件共存且命名规范；验证边界场景推导完整；验证重复执行工作流两次的幂等性（输出一致无累计副作用）。 | Python 脚本（调用 MCP 工具的集成包装器 + git diff 分析 + SHA-256 文件校验） | 验证代理间协同、工作流编排器兼容性、输出产物完整性、正确性和幂等性 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `openspec/changes/code-driven-test-phases/tests/test_agent_planner_ac_categorization.py` | 单元测试 | 验证 test-design-planner.md 中包含 Forward ACs / Reverse ACs 分类指令、Grep 源码补充输入指令、禁止输出参数类型和风险标记 |
| AC-2 | `openspec/changes/code-driven-test-phases/tests/test_agent_generator_blacklist_removed.py` | 单元测试 | 验证 test-gen-generator.md 中不存在文件类型黑名单约束，确认 .ts/.tsx/.js/.py/.rs/.go/.java 等类型不再被禁止 |
| AC-3 | `openspec/changes/code-driven-test-phases/tests/test_agent_generator_colocated_output.py` | 单元测试 | 验证 test-gen-generator.md 中包含共存文件输出指令，输出路径从 `openspec/changes/<name>/tests/` 改为源码同目录 |
| AC-4 | `openspec/changes/code-driven-test-phases/tests/test_agent_generator_naming_convention.py` | 单元测试 | 验证 test-gen-generator.md 中包含各语言命名规范指令（py→test_*.py、ts→*.test.ts、rs→*_test.rs、go→*_test.go） |
| AC-5 | `openspec/changes/code-driven-test-phases/tests/test_agent_generator_boundary_mapping.py` | 单元测试 | 验证 test-gen-generator.md 中包含参数类型→边界场景系统映射表（int/str/bool/list/dict/Optional/Enum/float 等类型的完整边界值列表） |
| AC-6 | `openspec/changes/code-driven-test-phases/tests/test_agent_generator_reverse_ac_test.py` | 单元测试 | 验证 test-gen-generator.md 中包含从 Reverse ACs 推导异常路径测试的指令 |
| AC-7 | `openspec/changes/code-driven-test-phases/tests/test_evaluator_t2_ac_coverage.py` | 单元测试 | 验证 test-design-evaluator.md 中 T2 检查项已从「tests/ 目录路径检查」更新为「coverage map 覆盖所有正向 AC 和反向 AC」 |
| AC-8 | `openspec/changes/code-driven-test-phases/tests/test_evaluator_g1_g2_colocation.py` | 单元测试 | 验证 test-gen-evaluator.md 中 G1 从「tests/ 目录文件检查」更新为「源码共存文件存在性检查」；G2 从「tests/ 位置检查」更新为「命名规范 + 共存位置检查」 |
| AC-9 | `openspec/changes/code-driven-test-phases/tests/test_agent_generator_type_inference.py` | 单元测试 | 验证 test-gen-generator.md 中包含无类型文件的参数名→类型推断指令（username→str、count→int、flags→boolean 等）及 P2+TODO 标记策略 |
| AC-10 | `openspec/changes/code-driven-test-phases/tests/test_skills_unchanged.py` | 单元测试 | 验证 phase-test-design/SKILL.md 和 phase-test-gen/SKILL.md 的文件内容与基线版本一致，无任何新增/修改的领域逻辑指令 |
| AC-1 ~ AC-10 端到端 | `openspec/changes/code-driven-test-phases/tests/test_workflow_integration.py` | 集成测试 | 模拟一个完整变更（含 mock 源码文件），执行 test-design 和 test-gen 两个阶段，验证：输出 test-design.md 含正反向 AC、不含参数类型/风险标记；生成的测试文件共存于 mock 源码目录且命名规范；边界场景完整 |
| AC-3/AC-4/AC-5/AC-6 (幂等性 T8) | `openspec/changes/code-driven-test-phases/tests/test_workflow_idempotency.py` | 集成测试 | 对同一 mock 变更重复执行两轮完整 test-gen 工作流，验证：不生成重复测试文件（无 test_auth_2.py）、不追加重复测试用例（文件内无重复函数定义）、测试文件内容逐字节一致（SHA-256 校验）、无残留临时文件（.bak/.tmp）；同时验证 test-design 阶段重复执行后 test-design.md 内容一致 |
| AC-5 细化 | `openspec/changes/code-driven-test-phases/tests/test_boundary_value_completeness.py` | 单元测试 | 逐类型验证边界映射的完整性：int(0/-1/MAX/None)、str(空/超长/特殊字符/None)、bool(True/False/None)、list([]/单元素/超大/None)、dict({}/缺字段/多余字段/None)、Optional(None)、Enum(每值+非法)、float(0.0/-0.0/NaN/Inf/None) |

---

## 3. 测试策略

### 3.1 方法

本变更为元变更（meta-change）：修改的是开发工作流中的 agent 定义、模板和评估器检查清单，而非应用程序代码。因此测试方法分为两层：

1. **静态分析层**：通过 Python 脚本解析 agent .md 文件、模板文件和 skill 文件的文本内容，使用正则匹配、YAML frontmatter 解析、关键字符串存在性检测等手段，验证每份文件是否包含（或不包含）spec.md 中规定的指令和约束。无需执行任何工作流，快速验证文件内容正确性。

2. **集成执行层**：通过 dev-team MCP 工具链真实调用 phase-test-design 和 phase-test-gen 技能，使用一个临时 mock 变更（包含 proposal.md、design.md 和模拟源码文件），验证完整工作流的输出产物是否符合预期。执行两轮完整工作流并逐字节对比输出产物（SHA-256 校验），验证幂等性。验证完成后清理 mock 数据。

### 3.2 测试分类

- **单元测试**: 不依赖 MCP 工具或工作流编排的独立验证脚本。每个脚本针对一个或多个 agent 定义文件/模板文件进行文本级验证。使用 Python re 模块和 yaml 解析库检查文件内容。无需 mock，直接读取文件系统中的目标文件。

- **集成测试**: 依赖 dev-team MCP 工具（phase_check、phase_log）和 agent 代理调用的验证脚本。需要在一个隔离的 mock 变更目录中创建 proposal.md、design.md 和模拟源码文件，然后通过 skill 描述手动触发 P→E 和 G→E 循环。循环执行两轮完整工作流，通过 SHA-256 校验对比轮次之间的产物一致性以验证幂等性。验证完成后自动清理 mock 数据。

#### 测试分类说明

- **单元测试**: 不依赖外部服务/数据库的测试，测试独立文件内容是否符合 spec 规定的指令和约束。使用 Python 标准库 re、yaml 进行文本分析。
- **集成测试**: 依赖 dev-team MCP 工具链的测试，验证多个 agent 组件在工作流编排下的协同输出。需要创建临时 mock 变更目录并在其中执行完整工作流。

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock 变更，不依赖真实项目变更数据。

### 3.3 模拟策略

| 依赖 | 模拟方式 | 适用场景 |
|------|----------|----------|
| dev-team MCP 工具（phase_check、phase_log） | 直接调用真实 MCP 工具。集成测试中通过 MCP 协议调用 dev-team 插件提供的 tool，不 mock 工具本身 | 集成测试、验收测试 |
| proposal.md / design.md | 在 mock 变更目录中创建精简但结构完整的 proposal.md 和 design.md，包含可被提取的 API 签名 | 集成测试 |
| 模拟源码文件 | 在 mock 变更目录所在项目的源码区域创建临时 .py / .ts / .rs / .go 文件，包含函数签名和类型注解，集成测试完成后删除 | 集成测试（验证共存输出和命名规范） |
| agent 代理调用 | 不做 mock — 集成测试真实调用 Agent 代理。单元测试直接读取 agent .md 文件内容，不触发代理 | 全部 |
| git diff | 集成测试通过 Bash 执行 `git diff` 检查已生成的测试文件状态 | 集成测试 |
| 文件系统 | 直接读取文件：单元测试读 agent 定义文件；集成测试读写 mock 变更目录中的文件 | 全部 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| test-design-planner 同时收到旧格式和新格式指令 | 文件同时包含"参数类型表"指令和"禁止输出参数类型"指令 | 验证脚本 SHALL 报告冲突，测试设计要求新指令完全覆盖旧指令（旧指令不存在或新指令在旧之后覆盖） | `tests/test_agent_planner_ac_categorization.py` |
| test-gen-generator 黑名单被移除后无其他隐含类型限制 | 文件中不存在"禁止读取"或"黑名单"关键字指向 .ts/.py/.rs 等类型 | 验证脚本 SHALL 确认无任何 glob 模式或扩展名列表被标注为禁止读取 | `tests/test_agent_generator_blacklist_removed.py` |
| Python 文件无类型注解（无类型提示） | `def add_user(username, age):` — 无类型注解 | generator 应根据参数名推断：username→str、age→int；生成的边界测试含 P2/TODO 标记 | `tests/test_agent_generator_type_inference.py` |
| Python 文件部分参数有类型注解 | `def process(name: str, age)` — age 无类型 | 对 name 使用 str 映射表；对 age 使用参数名推断；生成的测试中 age 的边界用例标记 P2+TODO | `tests/test_agent_generator_type_inference.py` |
| 源文件位于多级子目录 | `src/api/v2/handler.py` | 测试文件应生成在 `src/api/v2/test_handler.py`，而非 `src/test_handler.py` 或 `openspec/changes/.../tests/` | `tests/test_agent_generator_colocated_output.py` |
| 多个源文件同名但不同目录 | `src/auth.py` 和 `src/api/auth.py` | 各自生成 `src/test_auth.py` 和 `src/api/test_auth.py`，不会冲突 | `tests/test_agent_generator_colocated_output.py` |
| 单个参数为 Union 类型 | `def score(value: int \| None)` 或 `value: Optional[int]` | 边界场景应合并 int 的边界值和 None：0, -1, MAX_INT, None | `tests/test_boundary_value_completeness.py` |
| 参数为嵌套泛型类型 | `def process(data: List[Dict[str, int]])` | 边界场景应考虑外层的 list 边界（[]、单元素、超大、None）及内层 dict 的边界 | `tests/test_boundary_value_completeness.py` |
| 参数类型为自定义类 | `def handle(event: UserEvent)` | 不做深度递归推导；至少生成 None 测试用例，并标注 TODO 提示开发者补充字段级边界 | `tests/test_agent_generator_boundary_mapping.py` |
| JSDoc 类型标注（TypeScript） | `/** @param {number} count */` JSDoc | generator 应能提取 `count: number` 类型并应用 int 边界映射 | `tests/test_agent_generator_type_inference.py` |
| evaluator G2 检测到共存但命名错误 | 测试文件名为 `auth_test.py`（Python）而非 `test_auth.py` | G2 SHALL fail，提示命名应遵循 `test_*.py` 规范 | `tests/test_evaluator_g1_g2_colocation.py` |
| evaluator G2 检测到命名正确但位置错误 | `openspec/changes/x/tests/test_auth.py` 而非 `src/test_auth.py` | G2 SHALL fail，提示应共存于源文件所在目录 | `tests/test_evaluator_g1_g2_colocation.py` |
| 技能文件被意外修改 | phase-test-design/SKILL.md 中出现领域逻辑指令（如"区分正向反向 AC"） | 验证脚本 SHALL 报告：技能文件不应包含领域逻辑，应由 agent 定义承载 | `tests/test_skills_unchanged.py` |
| test-design.md 中 Reverse ACs 为空 | 某变更无明确的异常路径场景 | test-design-planner 也应输出空 Reverse ACs 章节（占位），不应跳过章节 | `tests/test_agent_planner_ac_categorization.py` |
| 生成测试文件包含 TODO/skip 标记 | 新生成的骨架测试被写入源码目录 | 验证每个生成的测试文件包含 `TODO` 或 `@pytest.mark.skip` 等标记，确保不干扰现有 CI/CD | `tests/test_workflow_integration.py` |
| test-gen 重复执行写入已存在的测试文件 | 首次运行已生成 src/test_auth.py，再次运行同一 generator | 不应创建 src/test_auth_2.py 或 src/test_auth(1).py 等重复文件；已有文件被覆盖写入（内容一致）或跳过写入 | `tests/test_workflow_idempotency.py` |
| test-gen 重复执行不追加重复测试用例 | 首次运行生成 test_register_success 和 test_register_invalid_email 两个函数；再次运行后 | 测试文件中不应出现两个 test_register_success 或两个 test_register_invalid_email 定义；总测试函数数量不变 | `tests/test_workflow_idempotency.py` |
| 两轮执行后测试文件内容的 SHA-256 校验 | 首次运行后记录所有生成测试文件的 SHA-256，清理后再次运行并重新计算 | 每份文件两次的 SHA-256 完全一致；全量文件列表一致（无新增/遗漏文件） | `tests/test_workflow_idempotency.py` |
| 重复执行不产生残留临时文件 | 首次运行后源码目录中可能出现 .bak、.tmp、缓存文件等；再次运行后 | 源码目录中无新增非预期文件（仅 test_*.py / *.test.ts 等规范命名的测试文件）；生成前后只有测试文件数量差异 | `tests/test_workflow_idempotency.py` |
| test-design 阶段重复执行输出一致性 | 两次执行 phase-test-design 工作流 | 输出的 test-design.md 中 Forward ACs 列表、Reverse ACs 列表、coverage map 映射表的结构和内容一致 | `tests/test_workflow_idempotency.py` |
| 先生成后手动删除再生成的恢复能力 | 首次生成测试文件后手动删除，再次运行 generator | 第二次运行应重新生成缺失的测试文件，内容与首次一致；不因文件已存在而跳过导致永久性丢失 | `tests/test_workflow_idempotency.py` |

---

## 5. 测试数据

### 5.1 Mock 变更目录结构（集成测试）

```
openspec/changes/_mock_test_change/
├── proposal.md              # 精简但完整的提案，含 2 个 Forward AC、2 个 Reverse AC
├── design.md                # 精简设计，定义 1 个函数签名
└── specs/
    └── mock-capability/
        └── spec.md          # 模块边界契约，含函数签名

mock_project_src/            # 模拟项目源码目录（与变更目录分离）
├── auth.py                  # Python: 含类型注解的函数
├── utils.py                 # Python: 无类型注解的函数
├── api/
│   └── user.ts              # TypeScript: 含接口定义和 async 函数
├── processor.rs             # Rust: 含 Result 返回类型的函数
└── handler.go               # Go: 含导出函数的包
```

### 5.2 Mock 测试数据示例

**mock_proposal.md 内容概要**：
- 功能：用户注册与登陆模块
- AC-1（Forward）：有效邮箱和密码可成功注册
- AC-2（Forward）：注册后可用凭据成功登陆
- AC-3（Reverse）：无效邮箱格式返回验证错误
- AC-4（Reverse）：重复注册返回冲突错误

**mock_design.md 内容概要**：
- `src/auth.py` 中 `register(email: str, password: str) -> User` 和 `login(email: str, password: str) -> Token`
- `src/api/user.ts` 中 `createUser(data: CreateUserInput) -> Promise<User>`

**mock 源码函数签名**：
- `src/auth.py`: `def register(email: str, password: str) -> User | None`, `def login(email: str, password: str) -> Token`
- `src/utils.py`: `def validate_email(value)` — 无类型注解
- `src/api/user.ts`: `async function createUser(data: CreateUserInput): Promise<User>`
- `src/processor.rs`: `pub fn process(config: Config) -> Result<Output, AppError>`
- `src/handler.go`: `func HandleRequest(w http.ResponseWriter, r *http.Request)`

---

## 6. 不可测试项

- **代理运行时行为（LLM 输出质量）** — 无法通过静态脚本验证 LLM 生成的测试代码质量或业务逻辑正确性。所有代理定义测试仅验证指令存在性和约束正确性，不验证 LLM 代理在真实执行时的推理质量。
  - **原因**: LLM 输出具有概率性，静态脚本无法判定语义正确性。质量验证通过工作流中的评估器（E 步骤）在运行时完成，不属于本次变更的测试范畴。

- **与外部 CI/CD 流水线的兼容性** — 无法在当前测试环境中验证生成的共存测试文件是否与项目的 GitHub Actions / GitLab CI 等流水线正确集成。
  - **原因**: CI/CD 配置在项目根目录，不在本变更的测试范围内。兼容性缓解措施（TODO/skip 标记）已在 agent 定义中约束。

- **跨语言测试框架的运行时正确性** — 不验证生成的 pytest / jest / cargo test / go test 文件是否能通过对应框架的编译或运行。
  - **原因**: 需要目标语言运行时环境和依赖安装。验证脚本仅检查文件命名、结构和导入语句的语法正确性，不执行测试框架。
