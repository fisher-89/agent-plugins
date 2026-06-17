# 实施任务: add-integration-root-param

---

## 阶段 1: Schema 扩展

- [x] 修改 `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts`：
  - 在 `testResolvePathsInputSchema` 增加 `integration_root: z.string().optional()`，describe 为 `__tests__/` 父目录（相对 `project_root`）
- [x] 确认 `plugins/dev-team/bin/src/schemas/index.ts` 导出无需额外变更（已有 re-export）

## 阶段 2: 核心实现

- [x] 修改 `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` 类型定义：
  - `ResolveTestPathsParams` 增加 `integrationRoot?: string`
  - `TestResolvePathsInput` 增加 `integration_root?: string`
- [x] 更新 `deriveIntegrationTestPath(scenario, ext, integrationRoot?)`：
  - 未提供或 `integrationRoot === "."` → `__tests__/<scenario>/<scenario>.test.<ext>`
  - 否则：POSIX 规范化、去除尾部 `/`，返回 `<integrationRoot>/__tests__/...`
  - 导出纯函数供单元测试直接调用（AC-4）
- [x] 更新 `resolveTestPaths()`：
  - 将 `params.integrationRoot` 传入 `deriveIntegrationTestPath()`
  - 确认 `unit_tests` 推导路径**不**读取 `integrationRoot`（AC-6）
- [x] 更新 `runTestResolvePaths()`：
  - 映射 `args.integration_root` → `integrationRoot` 并传给 `resolveTestPaths()`

## 阶段 3: MCP 注册

- [x] 检查 `plugins/dev-team/bin/src/mcp.ts` 中 `test_resolve_paths` 注册：
  - handler 已通过 `{ ...args }` 透传 `integration_root`（AC-8）
  - 可选：更新 tool `description`，说明集成测试路径可相对框架工作目录前缀
- [x] 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号

## 阶段 4: Agent 集成

- [x] 修改 `plugins/dev-team/agents/test-design-planner.md` Process 章节：
  1. 汇总 `modules` 与 `integration_scenarios`（不变）
  2. **单元测试**：调用一次 `mcp__plugin_dev-team_dev-team__test_resolve_paths`（`modules`、可选 `extension`；不传 `integration_root`）
  3. **集成测试**（若存在场景）：先调用 `mcp__plugin_dev-team_dev-team__test_detect_frameworks`；对每个 plan 条目再调用 `test_resolve_paths`，传入相同 `modules` / `integration_scenarios` / `extension`，并将 `directory` 作为 `integration_root`
  4. 合并各次 `integration_tests` 映射到 `集成测试 > 用例` 表格
  5. 单框架且 `directory === "."` 时 MAY 仅调用一次（不传 `integration_root`）
  6. 合并/记录各次调用的 `errors`（不变）
- [x] 在 Constraints 中保留禁止手工拼接路径；补充 `test_detect_frameworks` 工具引用
- [x] 确认 agent 工具权限包含 `test_detect_frameworks`（若 frontmatter 或 plugin 配置需更新则一并修改）

## 阶段 5: 单元测试 — `deriveIntegrationTestPath` 与 `resolveTestPaths`

在 `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` 中新增或扩展测试：

### 向后兼容（AC-1、AC-2）

- [x] AC-1：未传 `integrationRoot` / `integration_root` 时，`integration_scenarios: ["api-flow"]` → `__tests__/api-flow/api-flow.test.ts`
- [x] AC-2：`integration_root: "."` 时行为与 AC-1 相同

### integration_root 前缀（AC-3、AC-4、AC-5）

- [x] AC-3：`integration_root: "plugins/dev-team/bin"` → `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts`
- [x] AC-4：`deriveIntegrationTestPath("api-flow", "ts", "plugins/dev-team/bin")` 返回带前缀的 POSIX 路径
- [x] AC-5：`integration_root: "plugins/dev-team/bin/"` 规范化后无重复斜杠

### 隔离性（AC-6）

- [x] AC-6：`integration_root: "plugins/dev-team/bin"` 时 `unit_tests` 仍为 colocated 路径（如 `src/config.test.ts`），不受影响

### runTestResolvePaths 端到端

- [x] `runTestResolvePaths({ modules, integration_scenarios, integration_root })` 与 `resolveTestPaths({ integrationRoot })` 结果一致

## 阶段 6: Schema 单元测试（AC-7）

在 `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` 中：

- [x] AC-7：含 `integration_root: "plugins/dev-team/bin"` 的有效输入通过 `testResolvePathsInputSchema`
- [x] 不含 `integration_root` 的最小输入仍通过（向后兼容）

## 阶段 7: Agent 静态检查（AC-9）

- [x] 新建或扩展 `openspec/changes/add-integration-root-param/tests/test_agent_planner_integration_root.py`（或更新既有 planner 静态测试），验证 `test-design-planner.md`：
  - Process 描述多框架下调用 `test_detect_frameworks`
  - 将 plan 条目 `directory` 作为 `integration_root` 传入 `test_resolve_paths`
  - 合并 `integration_tests` 的说明
  - 禁止手工拼接测试路径的约束仍存在

## 阶段 8: 构建与验证（AC-10）

- [x] 在 `plugins/dev-team/bin` 下运行 `npm run test`（`vp test`），全部通过
- [x] 在 `plugins/dev-team/bin` 下运行 `npm run build`（`vp pack`），无错误（AC-10）
- [x] 确认 `test_detect_frameworks` 与现有 `test-resolve-paths` 测试无回归
