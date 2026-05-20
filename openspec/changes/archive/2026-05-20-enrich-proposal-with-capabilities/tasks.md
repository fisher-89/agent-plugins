# 任务: enrich-proposal-with-capabilities

> **变更**: enrich-proposal-with-capabilities
> **日期**: 2026-05-20

---

## Phase 1: CLI Wrapper 增加 spec list 函数

此阶段为 `openspec-cli.sh` 新增 `openspec_spec_list()` 函数，该函数是后续 agent 获取已有 capability 列表的基础设施。

- [x] **1.1 在 openspec-cli.sh 中新增 `openspec_spec_list()` 函数实现**
  - 在 `D:\Projects\wps-claude-plugin\plugins\dev-team\utils\openspec-cli.sh` 中新增函数
  - 调用 `openspec spec list --json` 获取 capability 列表
  - 对输出进行 JSON 数组校验：使用 `echo "$output" | python3 -c "import json,sys; data=json.load(sys.stdin); assert isinstance(data, list), 'not a list'; print(json.dumps(data))"` 验证格式
  - CLI 不可用（command not found / 非零退出码）时返回 `[]`
  - 输出格式非合法 JSON 数组时返回 `[]` 并输出 `echo "Warning: openspec spec list returned non-array output" >&2`
  - 正常时直接透传 CLI 输出的 JSON 数组
  - 遵循现有函数风格（set -euo pipefail、参数校验、错误处理）

- [x] **1.2 在 `openspec_cli_cache` 中注册 `spec_list` 命令支持**
  - 在 `openspec-cli.sh` 的 `openspec_cli_cache()` 函数的 `case "$cmd" in` 块中新增 `spec_list)` 分支
  - 调用 `openspec_spec_list "$name"` 获取输出
  - 使用统一的缓存键机制缓存结果

- [x] **1.3 在函数文档注释中更新函数列表**
  - 在 `openspec-cli.sh` 文件顶部的函数注释块中添加 `openspec_spec_list(name)` 条目
  - 更新描述说明其返回 JSON 数组，CLI 故障时返回 `[]`

---

## Phase 2: 静态模板增加 Capabilities 章节

此阶段修改 `proposal.md.template`，在模板中新增 `New Capabilities` 和 `Modified Capabilities` 子章节，引导 agent 输出能力清单。

- [x] **2.1 在 proposal.md.template 中新增 Capabilities 章节**
  - 编辑 `D:\Projects\wps-claude-plugin\plugins\dev-team\templates\artifacts\proposal.md.template`
  - 在 `## 风险` 章节之后、文件末尾之前插入以下内容：
    ```markdown
    ---

    ## Capabilities

    本变更涉及的能力清单。每个 capability 后续需要对应一份 `specs/<capability>/spec.md` 文件。

    ### New Capabilities

    本次变更新增的能力：

    - {{capability_name}} — {{capability_description}}

    ### Modified Capabilities

    本次变更修改的已有能力：

    - {{capability_name}} — {{modification_description}}
    ```
  - 确保分隔线 `---` 与前后章节风格一致（参考风险章节前后的格式）
  - 保留 `{{template_variables}}` 作为 agent 填充指南

---

## Phase 3: SKILL.md 移除 template 字段注入

此阶段修改 `SKILL.md`，移除从 CLI `instructions` 响应中注入 `template` 字段的逻辑，消除模板注入冲突。

- [x] **3.1 修改 Step 3a 的 prompt 构造注释：移除 template 字段引用**
  - 编辑 `D:\Projects\wps-claude-plugin\plugins\dev-team\skills\phase-requirements\SKILL.md`
  - 在 Step 3a 的 prompt 构造说明中（第 163 行附近），将：
    ```
    3. **Dynamic CLI instructions** (if available): If `INSTRUCTIONS_JSON` is not `{}`, append the `rules`, `context`, and `template` fields from the instructions output.
    ```
    修改为：
    ```
    3. **Dynamic CLI instructions** (if available): If `INSTRUCTIONS_JSON` is not `{}`, append the `rules` and `context` fields from the instructions output. The `template` field is intentionally not injected — the static template path provides the structure.
    ```

- [x] **3.2 移除 prompt 构造代码中 template 字段的注入**
  - 在 `SKILL.md` 的 Step 3a 中，找到实际注入 `INSTRUCTIONS_JSON` 字段到 prompt 的代码段
  - 移除对 `template` 字段的引用（注入 prompt 时只包含 `rules` 和 `context`）
  - 如果存在类似 `"Template: ${template}"` 或 `"template": ...` 的注入代码，删除对应行
  - 保留 `rules` 和 `context` 字段的注入逻辑不变

- [x] **3.3 验证 prompt 模板示例中不包含 template 字段**
  - 在 `SKILL.md` 的第 179-201 行的 prompt 示例代码块中
  - 确认示例 prompt 中不包含 `template` 字段的占位符或引用
  - 如果需要，更新示例 prompt 使其只展示 `rules` 和 `context` 字段

---

## Phase 4: Planner Agent 增加 Capability 查询与分类逻辑

此阶段修改 `requirements-planner.md`，在 Process 中增加 spec list 调用步骤和 New/Modified 分类指令。

- [x] **4.1 在 Process 中增加 spec list 查询步骤**
  - 编辑 `D:\Projects\wps-claude-plugin\plugins\dev-team\agents\requirements-planner.md`
  - 在现有的 Process 第 1-2 步之后、第 3 步（Write proposal）之前，插入新步骤：
    ```
    2.5. Query existing capabilities:
       - Run `source plugins/dev-team/utils/openspec-cli.sh && openspec_spec_list "<change-name>"`
       - Parse the JSON array output to get the list of existing capability IDs
       - If the call fails or returns empty array `[]`, assume no existing capabilities (all entries will be marked as New)
    ```
  - 重新编号后续步骤（原 3 → 4，原 Output → 调整引用）

- [x] **4.2 在 Output 说明中增加 Capabilities 章节编写指南**
  - 在 `requirements-planner.md` 的 Output 部分（第 32-35 行），增加 Capabilities 章节的编写说明：
    ```
    - **Capabilities**：列出本次变更涉及的所有能力
      - 使用步骤 2.5 获取的已有 capability ID 列表来区分 New 和 Modified
      - 如果 capability ID 在列表中 → 归入 `Modified Capabilities`，说明修改内容
      - 如果 capability ID 不在列表中 → 归入 `New Capabilities`，说明新增能力
      - 如果 `openspec_spec_list()` 返回空列表，所有条目标记为 `New`
      - 每个 capability 需给出名称和简要说明，格式为 `- {{名称}} — {{说明}}`
    ```

- [x] **4.3 在 Language 或 Constraints 中增加 Capabilities 分类约束**
  - 在 `requirements-planner.md` 的 Constraints 部分增加约束：
    ```
    - Capabilities 章节必须同时包含 New Capabilities 和 Modified Capabilities 两个子章节（即使某个子章节为空，也要保留标题）
    - 根据 `openspec_spec_list()` 的返回值准确区分 New 和 Modified，不允许将所有 capability 都标记为 New（除非 CLI 不可用返回空列表）
    ```

---

## Phase 5: Evaluator Agent 增加 Capabilities 检查项

此阶段修改 `requirements-evaluator.md`，在静态 checklist 中新增 Capabilities 章节检查，并将 R7 调整为其原本意图。

- [x] **5.1 在 checklist 中新增 R8 Capabilities 章节检查项**
  - 编辑 `D:\Projects\wps-claude-plugin\plugins\dev-team\agents\requirements-evaluator.md`
  - 在 checklist 表格末尾新增一行：
    ```
    | R8 | Capabilities 章节存在且至少有一个 New 或 Modified 条目 | true | proposal 必须包含 `## Capabilities` 章节，其下 `### New Capabilities` 或 `### Modified Capabilities` 子章节至少有一个非空列表 |
    ```

- [x] **5.2 调整 R7 检查项描述**
  - 将原 R7 的描述从"所有模板章节已填写实质性内容"调整为更明确的表述
  - 修改后应为：
    ```
    | R7 | 所有非 Capabilities 的模板章节已填写实质性内容 | false | 模板中除 Capabilities 外的其他章节（问题、提案、变更范围、验收标准、风险）需有实质性内容，无占位符 |
    ```
  - 保持 R7 的 `必须` 字段为 `false`（非必须项）

- [x] **5.3 更新 Process 中的检查项引用**
  - 检查 `requirements-evaluator.md` 的 Process 部分是否有对检查项数量的硬编码引用（如"R1-R7"）
  - 如果有，更新为正确的范围（如"R1-R8"）
  - 确保 verdict 判定逻辑（"ALL required items pass"）不需要修改（已基于 `必须` 字段为 true 的项做判定）

---

## Phase 6: 测试辅助库与 Fixtures

此阶段创建测试基础设施：扩展共享测试辅助库、创建测试数据 Fixtures。

- [x] **6.1 创建测试目录结构**
  - 创建 `D:\Projects\wps-claude-plugin\openspec\changes\enrich-proposal-with-capabilities\tests\` 目录
  - 创建 `tests\helpers\` 子目录
  - 创建 `tests\fixtures\` 子目录
  - 创建 `tests\reports\` 子目录

- [x] **6.2 创建共享测试辅助库 `setup_test_env.sh`**
  - 新建 `tests/helpers/setup_test_env.sh`
  - 基于 `openspec/changes/archive/2026-05-19-enhance-phase-requirements-skill/tests/helpers/setup_test_env.sh` 扩展
  - 保留现有函数：`setup_sandbox()`、`teardown_sandbox()`、`mock_openspec()`、`assert_dir_exists()`、`assert_file_exists()`、`assert_json_field()`、`assert_exit_code()`、`assert_string_contains()`、`assert_string_not_contains()`
  - 新增 `mock_spec_list(output_mode)` — 模拟 `openspec spec list --json` 命令，支持三种模式：
    - `normal`：输出 `["auth", "storage"]`
    - `empty`：输出 `[]`
    - `error`：返回非零退出码并输出空
  - 新增 `mock_specs_dir(sandbox_path, spec_ids...)` — 在 sandbox 中创建 `openspec/specs/<id>/spec.md` 目录结构
  - 新增 `assert_template_section(file_path, section_title)` — 断言文件中存在指定 Markdown 章节标题
  - 新增 `assert_capability_classified(file_path, capability_name, expected_section)` — 断言 Capabilities 章节中 capability 被正确分类

- [x] **6.3 创建测试 Fixture 文件**
  - 创建 `tests/fixtures/specs/auth/spec.md` — 模拟已存在的 auth capability
  - 创建 `tests/fixtures/specs/storage/spec.md` — 模拟已存在的 storage capability
  - 创建 `tests/fixtures/specs_empty/.gitkeep` — 空 spec 目录占位
  - 创建 `tests/fixtures/openspec_spec_list_output.json` — 内容：`["auth", "storage"]`
  - 创建 `tests/fixtures/openspec_spec_list_empty.json` — 内容：`[]`
  - 创建 `tests/fixtures/openspec_spec_list_invalid.json` — 内容：`{"error": "internal error"}`
  - 创建 `tests/fixtures/proposal_with_capabilities.md` — 包含 `## Capabilities` 章节及 New/Modified 子章节的完整 proposal fixture
  - 创建 `tests/fixtures/proposal_without_capabilities.md` — 不含 Capabilities 章节的 proposal fixture
  - 创建 `tests/fixtures/proposal_empty_capabilities.md` — 包含 Capabilities 标题但子章节为空的 proposal fixture
  - 创建 `tests/fixtures/openspec_instructions.json` — 包含 `rules`、`context`、`template` 三个字段的完整 instructions fixture
  - 创建 `tests/fixtures/openspec_status.json` — 标准 status fixture

---

## Phase 7: 单元测试 — openspec_spec_list()

此阶段创建针对 `openspec-cli.sh` 中新增函数的 bats 单元测试。

- [x] **7.1 创建 `test_openspec_spec_list.bats` 测试文件**
  - 新建 `tests/test_openspec_spec_list.bats`
  - 编写以下测试用例：
    - [x] **7.1.1** `@test "openspec_spec_list normal call returns JSON array"` — mock openspec 返回 `["auth","storage"]`，验证函数输出为 `["auth","storage"]`
    - [x] **7.1.2** `@test "openspec_spec_list empty spec directory returns empty array"` — mock openspec 返回 `[]`，验证函数输出为 `[]`
    - [x] **7.1.3** `@test "openspec_spec_list CLI not found returns empty array"` — 移除 PATH 中的 openspec，验证函数输出为 `[]`
    - [x] **7.1.4** `@test "openspec_spec_list CLI non-zero exit returns empty array"` — mock openspec 退出码 1，验证函数输出为 `[]`
    - [x] **7.1.5** `@test "openspec_spec_list non-array JSON output returns empty array with warning"` — mock openspec 返回 `{"error":"..."}`，验证函数输出为 `[]` 且 stderr 包含警告信息
    - [x] **7.1.6** `@test "openspec_spec_list returns capability IDs extracted from array"` — mock openspec 返回 `["auth","storage","logging"]`，验证函数正确透传

---

## Phase 8: 集成测试

此阶段创建 5 个集成测试脚本，覆盖每个变更点及其交互。

- [x] **8.1 创建 `test_proposal_template_structure.sh`**
  - 新建 `tests/test_proposal_template_structure.sh`
  - 测试步骤：
    1. 读取 `plugins/dev-team/templates/artifacts/proposal.md.template`
    2. 使用 `assert_template_section()` 验证包含 `## Capabilities` 章节标题
    3. 验证包含 `### New Capabilities` 子章节标题
    4. 验证包含 `### Modified Capabilities` 子章节标题
    5. 验证子章节下方有模板引导注释（`{{capability_name}}` 占位符或描述性注释）

- [x] **8.2 创建 `test_prompt_no_template_injection.sh`**
  - 新建 `tests/test_prompt_no_template_injection.sh`
  - 测试步骤：
    1. 使用 `openspec_instructions.json` fixture 模拟 CLI 返回
    2. 模拟 SKILL.md 中 Step 3a 的 prompt 构造逻辑
    3. 使用 `assert_string_not_contains()` 验证构造的 prompt 中不包含 `template` 字段及其内容
    4. 使用 `assert_string_contains()` 验证 prompt 中仍包含 `rules` 字段
    5. 使用 `assert_string_contains()` 验证 prompt 中仍包含 `context` 字段
    6. 场景：`INSTRUCTIONS_JSON` 为 `{}` 时 prompt 回退到基本格式

- [x] **8.3 创建 `test_planner_capabilities_flow.sh`**
  - 新建 `tests/test_planner_capabilities_flow.sh`
  - 测试步骤：
    1. 在 sandbox 中创建 mock spec 目录（auth, storage）
    2. mock `openspec spec list --json` 返回 `["auth", "storage"]`
    3. 模拟 agent 的 Capabilities 分类逻辑：给定 capability 列表 `["auth", "storage", "logging"]`，auth 和 storage 应归入 Modified，logging 归入 New
    4. 验证生成的 capabilities 输出中 auth 和 storage 在 Modified 子章节
    5. 验证 logging 在 New 子章节
    6. 场景：无全局 spec 时，所有条目标记为 New
    7. 场景：`openspec_spec_list()` 返回空数组时，所有条目标记为 New

- [x] **8.4 创建 `test_evaluator_capabilities_check.sh`**
  - 新建 `tests/test_evaluator_capabilities_check.sh`
  - 测试步骤：
    1. 使用 `proposal_with_capabilities.md` fixture 作为输入，运行 evaluator 检查逻辑
    2. 验证 R8 检查项通过，整体 verdict 为 pass
    3. 使用 `proposal_without_capabilities.md` fixture 作为输入，运行 evaluator 检查逻辑
    4. 验证 R8 检查项失败，整体 verdict 为 fail
    5. 使用 `proposal_empty_capabilities.md` fixture 作为输入，运行 evaluator 检查逻辑
    6. 验证 R8 检查项失败（"至少有一个 New 或 Modified 条目"），verdict 为 fail
    7. 验证失败证据中包含具体提示（如"能力列表为空"）

- [x] **8.5 创建 `test_cli_fail_fallback.sh`**
  - 新建 `tests/test_cli_fail_fallback.sh`
  - 测试步骤：
    1. 场景：`openspec` CLI 完全不可用（PATH 中无 openspec），执行 planner 的 spec list 步骤
    2. 验证 `openspec_spec_list()` 返回 `[]`
    3. 验证生成的 Capabilities 章节中所有条目标记为 New（无 Modified 条目）
    4. 场景：`openspec spec list --json` 返回非 JSON 输出（如纯文本错误）
    5. 验证 `openspec_spec_list()` 返回 `[]`
    6. 验证 stderr 中包含格式警告信息
    7. 验证 agent 仍能正常生成包含 Capabilities 章节的 proposal

---

## Phase 9: 端到端测试

此阶段创建完整的 E2E 测试，验证 P→E 工作流在引入 Capabilities 章节后仍然正确运行。

- [x] **9.1 创建 `test_e2e_full_flow.sh`**
  - 新建 `tests/test_e2e_full_flow.sh`
  - 测试步骤：
    1. 在 sandbox 中初始化完整的 OpenSpec 变更目录结构（含 `.openspec.yaml`、`phases/`）
    2. 模拟执行 phase-requirements 技能的前置步骤：设置 `STATUS_JSON` 和 `INSTRUCTIONS_JSON`
    3. 使用 mock CLI 返回包含 `["auth"]` 的 capability 列表
    4. 模拟 Planner 执行：调用 `openspec_spec_list()`、读取模板、生成带 Capabilities 章节的 proposal.md
    5. 模拟 Evaluator 执行：读取生成的 proposal.md、运行静态 checklist（含 R8）、写入 eval.json
    6. 验证生成的 proposal.md 包含 Capabilities 章节且分类正确（auth → Modified，新增的 → New）
    7. 验证 eval.json 中 R8 条目 pass、整体 verdict 为 pass
    8. 场景：Planner 输出空 Capabilities 章节（只有标题无条目），验证 Evaluator 的 R8 判 fail
    9. 验证 eval.json 格式与 `eval.schema.json` 一致（字段完整性：`phase`、`timestamp`、`attempt`、`verdict`、`report`、`items[]`、`backtrack_to`、`schema_version`）

---

## Phase 10: 验证与清理

此阶段运行所有测试，确保变更的正确性和完整性。

- [x] **10.1 运行单元测试并确认全部通过**
  - 运行 `bats tests/test_openspec_spec_list.bats`
  - 确认 6 个测试用例全部通过
  - 如果存在失败项，定位原因并修复对应实现代码

- [x] **10.2 运行集成测试并确认全部通过**
  - 依次运行 5 个集成测试脚本（或使用聚合 runner）
  - 确认所有脚本退出码为 0
  - 如果存在失败项，定位原因并修复

- [x] **10.3 运行端到端测试并确认通过**
  - 运行 `bash tests/test_e2e_full_flow.sh`
  - 确认最终 verdict 为 pass
  - 确认生成的 eval.json 符合 schema

- [x] **10.4 最终人工审查：确认五个修改文件的一致性**
  - 审查 `openspec-cli.sh`：`openspec_spec_list()` 输出格式、错误处理、缓存注册
  - 审查 `proposal.md.template`：Capabilities 章节的结构和注释
  - 审查 `SKILL.md`：确认 template 注入已移除且 rules/context 仍保留
  - 审查 `requirements-planner.md`：spec list 调用步骤和 New/Modified 分类逻辑
  - 审查 `requirements-evaluator.md`：R8 检查项和 R7 调整的正确性
