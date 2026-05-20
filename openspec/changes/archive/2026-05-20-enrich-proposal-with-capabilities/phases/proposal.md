# 提案: enrich-proposal-with-capabilities

> **变更**: enrich-proposal-with-capabilities
> **日期**: 2026-05-20
> **状态**: 提案

---

## 问题

当前 proposal.md 静态模板（`plugins/dev-team/templates/artifacts/proposal.md.template`）缺少 Capabilities 章节，导致 proposal 到 spec 的链路断裂。

具体表现为：

1. **模板缺失 Capabilities 章节**：`spec-driven` schema 的提案应当列出本次变更涉及的能力（Capabilities），每个 capability 后续需要对应一份 `specs/<capability>/spec.md` 文件。当前模板没有这个章节，Planner agent 不会输出能力清单。

2. **SKILL.md 注入的 template 字段被忽略**：`phase-requirements/SKILL.md` 在 Step 3a 从 `openspec instructions` 获取 `template` 字段并注入到 Planner prompt 中，但 `requirements-planner.md` agent 定义中只引用了 `plugins/dev-team/templates/artifacts/proposal.md.template` 这个静态模板路径。动态注入的 `template` 字段与静态模板引用之间存在冲突，agent 实际忽略 CLI 传入的 template 内容。

3. **Agent 无法区分 New/Modified Capabilities**：当 agent 编写 proposal 时，如果不知道哪些 capability 已在项目中存在（即在 `openspec/specs/` 下已有 spec.md），就无法区分"新增能力"和"修改已有能力"。这会导致 spec 创建时的命名冲突或重复。

4. **Evaluator checklist 未覆盖 Capabilities**：`requirements-evaluator.md` 的静态检查项（R1-R7）没有针对 Capabilities 章节的检查，即使模板增加了该章节，也无法通过 evaluator 确保其被填写。

## 提案

对 proposal 编写流程进行四项改造，补齐 Capabilities 章节并修复模板注入冲突：

### 方案一：静态模板 + CLI 能力查询（选定方案）

在静态模板中直接增加 Capabilities 章节，同时让 agent 调用 `openspec spec list --json` 获取全局已有 capability ID 列表，从而在 proposal 中区分 New Capabilities 和 Modified Capabilities。

**优点：**
- 静态模板是 agent 行为的主驱动，修改模板对 agent 行为的影响最直接
- `openspec spec list --json` 提供权威的已有 capability 清单，agent 可据此做准确判断
- 消除 CLI template 注入与静态模板的冲突风险

**缺点：**
- 需要同时修改 template、SKILL.md、agent、CLI wrapper、evaluator 五个文件，涉及面较广
- 对已有 proposal（无 Capabilities 章节）需要 evaluator 做兼容处理

### 方案二：完全依赖 CLI 动态注入（已排除）

完全依赖 `openspec instructions` 的 `template` 字段来驱动 agent，移除静态模板引用。

**排除理由：**
- 当前 `openspec instructions` 在 CLI 端返回的 template 内容不稳定（返回空 `{}` 时回退到静态模板）
- agent 定义中硬编码了静态模板路径，改变 agent 行为需修改 agent 定义
- 动态注入内容无法被 evaluator 的静态 checklist 可靠引用

### 方案三：仅在 evaluator 侧增加检查（已排除）

不修改模板，仅在 evaluator 侧增加关于 Capabilities 的检查项。

**排除理由：**
- Planner agent 没有 Capabilities 章节的模板引导，不会主动输出该章节
- 仅仅检查而不提供模板引导，会导致循环失败
- 与 `spec-driven` schema 的设计意图矛盾（proposal 的 Capabilities 是 spec 的输入）

---

## 变更范围

### 实现以下特性

1. **静态模板增加 Capabilities 章节**：在 `plugins/dev-team/templates/artifacts/proposal.md.template` 中新增 `New Capabilities` 和 `Modified Capabilities` 两个子章节，引导 agent 列出本次变更新增或修改的能力名称和简要说明。

2. **SKILL.md 移除 template 注入**：`plugins/dev-team/skills/phase-requirements/SKILL.md` 中 Step 3a 的 prompt 构造逻辑移除对 CLI `template` 字段的注入，保留 `context` 和 `rules` 字段的注入。

3. **CLI wrapper 增加 spec list 函数**：在 `plugins/dev-team/utils/openspec-cli.sh` 中新增 `openspec_spec_list()` 函数，调用 `openspec spec list --json` 获取全局 capability 列表，并在 CLI 调用失败时 fail-safe 返回 `[]`。

4. **Agent 增加 capability 查询与填充逻辑**：`plugins/dev-team/agents/requirements-planner.md` 在 Process 中增加步骤：在编写 proposal 前先调用 `openspec_spec_list()` 获取已有 capability ID 列表；在编写 Capabilities 章节时，将已在列表中的标记为 `Modified`，不在列表中的标记为 `New`。

5. **Evaluator checklist 适配**：`plugins/dev-team/agents/requirements-evaluator.md` 在静态 checklist 中增加一项 Capabilities 章节检查（至少有一个 New 或 Modified 条目），并将原有 R7（所有模板章节已填写实质性内容）的权重调整为必须通过项。

### 不要修改

- 不修改 OpenSpec CLI 本身的 `spec list` 命令行为或输出格式
- 不修改已有的 proposal.md 文件或 eval.json 文件格式
- 不修改 spec 文件格式（`specs/<capability>/spec.md` 的格式和结构保持不变）
- 不修改除 requirements-planner 之外的其他 agent 定义
- 不修改除 phase-requirements 之外的其他 skill 定义
- 不引入新的 CLI 命令或新的模板文件

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | `plugins/dev-team/templates/artifacts/proposal.md.template` 包含 Capabilities 章节，包含 `New Capabilities` 和 `Modified Capabilities` 两个子章节 | 手动审查模板文件，确认两个子章节存在且结构清晰 | P0 |
| AC-2 | `plugins/dev-team/skills/phase-requirements/SKILL.md` 不再从 CLI 注入 `template` 字段到 Planner prompt | 审查 SKILL.md 中 prompt 构造代码，确认 `template` 字段未被引用；执行完整的 phase-requirements 流程，确认 prompt 中不包含 CLI 注入的模板内容 | P0 |
| AC-3 | `plugins/dev-team/utils/openspec-cli.sh` 新增 `openspec_spec_list()` 函数，在 CLI 可用时返回 JSON 数组，CLI 不可用时返回 `[]` | 单元测试：分别在有 spec 目录和无 spec 目录时调用该函数，确认输出格式正确；模拟 CLI 故障场景，确认返回 `[]` | P0 |
| AC-4 | `plugins/dev-team/agents/requirements-planner.md` 新增步骤：在编写 proposal 前调用 `openspec_spec_list()`，并在 Capabilities 章节中区分 New 和 Modified | 审查 agent 定义中 Process 部分，确认包含 spec list 调用逻辑和 New/Modified 区分逻辑；执行 phase-requirements 流程，验证生成的 proposal.md 包含 Capabilities 章节且分类正确 | P0 |
| AC-5 | `plugins/dev-team/agents/requirements-evaluator.md` 的 checklist 新增 Capabilities 章节检查项（标记为必须），R7 调整为对 Capabilities 章节的实质性内容检查 | 审查 evaluator checklist，确认新检查项存在且标记为 `true`；在已有 Capabilities 和有缺失 Capabilities 的 proposal 上分别运行 evaluator，确认 verdict 正确 | P0 |
| AC-6 | 当 `openspec` CLI 不可用或 `spec list` 失败时，agent 仍能正常生成 proposal（capability 列表为空，所有条目标记为 New） | 模拟 CLI 故障场景执行 phase-requirements，确认 proposal 正常生成且 Capabilities 章节存在但不含 Modified 条目 | P1 |
| AC-7 | 执行完整 phase-requirements 流程（P→E 循环），生成的 proposal.md 通过 evaluator 全部检查项 | 对任意 OpenSpec change 运行 `/dev-team:phase-requirements`，确认最终 eval.json 中 verdict 为 `pass` | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 已有 proposal（无 Capabilities 章节）在 evaluator 升级后首次检查 fail | 中等：已有变更的 proposal 突然不通过 evaluator，需要手动补填 | 中 | evaluator 的 Capabilities 检查项仅在模板包含该章节时触发；对已有 proposal 增加过渡期，首次检查仅记录 warning 不导致 fail |
| `openspec spec list --json` 输出格式与预期不一致（如字段名变更） | 高：agent 解析失败，proposal 生成中断 | 低 | `openspec_spec_list()` 增加输出校验：检查是否为 JSON 数组；若不是，返回 `[]` 并记录 stderr 警告 |
| agent 将 global specs（`openspec/specs/`）与 change-local specs（`openspec/changes/<name>/specs/`）混淆 | 中等：agent 错误地将其他 change 的 spec 标记为 Modified | 中 | agent 定义中明确限定只查询 `openspec spec list --json`（global），并在 prompt 中说明该命令返回的是"项目中已存在的能力清单"；在 evaluator 检查项中验证 Modified 条目确实对应 global 目录下的 spec |
| SKILL.md 移除 template 注入后，依赖 CLI template 字段的自定义 CLI 扩展停止工作 | 高：自定义 CLI 集成的团队 workflow 中断 | 低 | CLI 的 `openspec instructions` 输出仍保留 `template` 字段，仅 SKILL.md 不再读取它；如果未来需要恢复模板注入，通过 agent 定义中的 context 字段传递而非 prompt 注入 |
| template 新增 Capabilities 章节后，旧版 agent（未更新）生成的 proposal 缺少该章节 | 中等：新旧 agent 产出格式不一致 | 中 | evaluator 的新检查项会捕获缺失 Capabilities 的 proposal，导致 P→E 循环 fail 并触发重新生成；version 字段标记 proposal 模板版本 |