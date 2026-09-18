# 提案: workflow-files-query-api

> **变更**: workflow-files-query-api
> **日期**: 2026-09-17
> **状态**: draft

---

## 问题

change 文件清单（`workflow.json` 的 `files` 净状态）目前只有写通道：`change_files` MCP 工具（append/set）与 PostToolUse 记录器。读侧没有 API——evaluator agent（implementation-evaluator、code-review-evaluator、acceptance-evaluator）在圈定审查/对账范围时被提示词指示**直接 Read `workflow.json`**：

- `plugins/dev-team/agents/implementation-evaluator.md`：「Read `workflow.json` 的 `files` 清单；运行 `git diff` 观察修改内容」
- `plugins/dev-team/agents/code-review-evaluator.md`：「Read `workflow.json` 的 `files` 清单圈定审查范围」
- `plugins/dev-team/agents/acceptance-evaluator.md`：范围权威表述引用 `workflow.json.files`

直接读文件带来三个问题：

1. agent 绕过 schema 校验读取，`workflow.json` 格式异常时得到的是原始 JSON 而非带重建指引的硬报错；
2. agent 可能读到并依赖 `source` 审计映射——`workflow-file-inventory` 规格明确规定该字段仅审计用途、消费方 MUST NOT 读取；
3. 读取 `workflow.json` 的逻辑分散在各 agent 提示词里，缺少统一的程序化入口。

同时，操作 `workflow.json` 的代码逻辑目前散落在 `lib/file-inventory.ts`（读/折叠/写）与 `commands/*`（eval、创建、回溯）中，没有统一的模块边界。用户方向：新建 `plugins/dev-team/bin/src/modules/workflow/` 目录，后续操作 `workflow.json` 的逻辑统一从 `workflow/index.ts` 导出。

---

## 提案

1. **新增只读 MCP 工具 `workflow_files`**：输入 `{ change, project_root? }`，读取 `<projectRoot>/openspec/changes/<change>/workflow.json`，聚合 `files` 净状态，输出 `{ written, deleted }`。工具严格只读（无任何写路径），输出不含 `source` 审计映射；硬报错语义与既有消费方一致（文件缺失 / JSON 非法 / schema 不通过 / 缺 `files` 字段 → 报错并附重建指引，不回退 git diff）。命名遵循 `mcp-tool-namespace` 的 `xx_yy` 规则，domain 取 `workflow`，与新模块对齐。

2. **新建 `plugins/dev-team/bin/src/modules/workflow/` 模块**，`workflow/index.ts` 作为公共出口：
   - `lib/file-inventory.ts` 整体迁入 `modules/workflow/file-inventory.ts`（`readFileInventory` / `foldFileOps` / `writeFileInventory` + `FileInventory` / `FileOp` 类型）——这正是本次增量直接触及的相似代码片段，一次迁移避免「逻辑在 lib/、出口在 modules/」的双出口状态；原文件删除，不做 re-export shim（避免 knip 判 dead export）；
   - 新增 `modules/workflow/files-query.ts` 承载 `runWorkflowFiles` 查询命令；
   - 约 10 个既有导入方（`commands/change-files.ts`、`commands/record-files.ts`、`commands/test-execution.ts`、`commands/test-resolve-paths.ts`、`lib/c4-cross-ref.ts`、`lib/shell-file-ops.ts` 及相关测试）改从 `modules/workflow` 导入；
   - `phase_next` / `phase_log` / `backtrack` / `change_create` 的 eval 与元数据逻辑**本次不迁移**——当前 change 仅处理增量功能，后续 change 再逐块搬入。

3. **agent 提示词切换**：三个 evaluator 的范围圈定步骤由「直接 Read `workflow.json`」改为调用 `__MCP:workflow_files__`；三态对账判定语义不变（仍按 `workflow-file-inventory` 规格「消费方对账三态」执行）。

4. **版本与产物**：按 CLAUDE.md 规则提升 `plugins/dev-team/package.json` `version` 并执行 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 产物。

---

## 能力

### 新增能力

- 无（查询 API 并入既有 `change-files` 能力：该能力由此扩展为 change 文件清单的 MCP 读写双通道）

### 修改的能力

- `change-files` — 新增 `workflow_files` 只读查询工具、查询语义（净状态聚合、无 source、硬报错一致）、`modules/workflow` 模块化导出约定
- `phase-agents` — 新增「evaluator 经查询 API 获取文件清单」要求：范围圈定改走 `__MCP:workflow_files__`，禁止提示词直接读 `workflow.json`

---

## 变更范围

### 实现文件

- 新建 `plugins/dev-team/bin/src/modules/workflow/index.ts` — 模块公共出口（barrel）
- 新建 `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` — 自 `lib/file-inventory.ts` 迁移（`readFileInventory` / `foldFileOps` / `writeFileInventory` / `FileInventory` / `FileOp`）
- 新建 `plugins/dev-team/bin/src/modules/workflow/files-query.ts` — `runWorkflowFiles` 只读查询
- 删除 `plugins/dev-team/bin/src/lib/file-inventory.ts`
- 新建 `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` — `workflowFilesInputSchema` / `workflowFilesOutputSchema`
- 修改 `plugins/dev-team/bin/src/schemas/index.ts` — 导出新 schema
- 修改 `plugins/dev-team/bin/src/mcp.ts` — 注册 `workflow_files` 工具（只读 description）
- 修改导入方：`commands/change-files.ts`、`commands/record-files.ts`、`commands/test-execution.ts`、`commands/test-resolve-paths.ts`、`lib/c4-cross-ref.ts`、`lib/shell-file-ops.ts`
- 修改 agent 提示：`agents/implementation-evaluator.md`、`agents/code-review-evaluator.md`、`agents/acceptance-evaluator.md` — 范围圈定改用 `__MCP:workflow_files__`
- `plugins/dev-team/package.json` — version 提升，`pnpm -C plugins/dev-team run build` 刷新产物

### 测试文件

- 迁移 `plugins/dev-team/bin/src/lib/file-inventory.test.ts` → `plugins/dev-team/bin/src/modules/workflow/file-inventory.test.ts`
- 新建 `plugins/dev-team/bin/src/modules/workflow/files-query.test.ts` — 净状态输出、无 source、硬报错四态、只读不落盘
- 修改 `plugins/dev-team/bin/src/mcp.test.ts` — listTools 断言含 `workflow_files`
- 修改旧路径测试导入：`commands/backtrack.test.ts`、`commands/test-resolve-paths.test.ts`、`bin/__tests__/archi-check-inventory/archi-check-inventory.test.ts`、`bin/__tests__/inventory-backtrack-preserve/inventory-backtrack-preserve.test.ts`

### 删除文件

- `plugins/dev-team/bin/src/lib/file-inventory.ts`（迁入 `modules/workflow/` 后删除，不留 shim）

### 不要修改

- `plugins/dev-team/bin/src/lib/workflow.ts` — PGE phase 配置，与 `workflow.json` 文件操作无关
- `phase_next` / `phase_log` / `backtrack` / `change_create` 的 eval 与元数据逻辑 — 后续 change 再迁移
- `plugins/dev-team/bin/src/schemas/workflow.schema.ts` — `workflowFileSchema` 数据模型不变
- 写保护（PreToolUse）与 protect-files-hook 规则
- `agents/architecture.md` — 已指示调用 `archi_check`，非直接读取
- `skills/phase-proposal/SKILL.md` 的 `workflow.json` 存在性检查 — 前置检查，非清单查询
- `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 产物 — 由 build 刷新，不手改

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `mcp.ts` 注册 `workflow_files` | MCP tools/list 含 `workflow_files`，inputSchema 含必填 `change`；description 表述为只读查询（`mcp.test.ts` 断言） |
| AC-2 | `modules/workflow/files-query.ts` 查询实现 | 对含 `files` 的 change 返回 `{ written, deleted }` 净状态；输出不含 `source`；调用前后 `workflow.json` 逐字节不变（`files-query.test.ts`） |
| AC-3 | 硬报错语义 | `workflow.json` 缺失 / JSON 非法 / schema 不通过 / 缺 `files` 字段时返回错误且文案含重建指引，MUST NOT 回退 git diff（`files-query.test.ts` 四态覆盖） |
| AC-4 | `modules/workflow` 模块迁移 | `workflow/index.ts` 导出 `readFileInventory` / `foldFileOps` / `writeFileInventory` / `runWorkflowFiles` 及类型；仓库内 grep 无 `lib/file-inventory` 导入残留；`lib/file-inventory.ts` 已删除 |
| AC-5 | agent 提示切换 | 三个 evaluator md 的范围圈定步骤指向 `__MCP:workflow_files__`，不再含「Read `workflow.json`」清单读取指令（grep 验证） |
| AC-6 | 版本与产物 | `plugins/dev-team/package.json` `version` 提升，build 成功且产物目录刷新（build 输出 + git status） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| agent 提示占位符 `__MCP:workflow_files__` 与实际工具名不一致 | evaluator 调不到工具，流程中断 | 低 | `mcp.test.ts` listTools 名称断言 + md 占位符 grep 校验同入 AC |
| 迁移触碰 10+ 导入点，回归面广 | 编译或既有测试失败 | 低 | tsc 全量编译 + 既有单测与 2 个集成测试（archi-check-inventory、inventory-backtrack-preserve）回归 |
| `workflow_files` 与 `change_files` 语义混淆 | agent 误调用写工具补录路径 | 低 | description 明确「只读查询」并写明与写通道的分工 |
| 迁移残留导致 knip 判 dead export / Stryker 逃逸 | 质量门禁失败 | 中 | 删除旧文件、不做 re-export shim，全量更新导入 |
| eval 逻辑未同步迁移 | 后续 change 误把 `workflow.json` 新逻辑继续加在 `lib/` 或 `commands/` | 中 | spec 明确「后续逻辑从 `workflow/index.ts` 导出」约定；待决问题记录 `lib/workflow.ts` 命名澄清 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 独立只读工具 vs `change_files` 增加 `op: "query"` | 独立工具 `workflow_files` | 读写分离符合 `change_list`/`change_create`、`archi_query`/`archi_write` 既有惯例；读入口对 agent 可发现；避免写工具 refine（至少一桶非空）复杂化 | `change_files` 增 `op: "query"` |
| 查询输出是否含 `source` 审计映射 | 不含 | `workflow-file-inventory` 规格规定来源字段仅审计、消费方 MUST NOT 读取，而本工具消费方恰为该禁令约束的 agent | 输出可选携带 `source` |
| `lib/file-inventory.ts` 是否迁入 `modules/workflow` | 迁入并删除原文件 | 用户指定「操作 `workflow.json` 的逻辑从 `workflow/index.ts` 导出」；读/折叠/写正是本次增量直接触及的相似片段；避免双出口 | 保留 `lib/` 原文件，`modules/workflow` 仅 re-export |
| eval / backtrack / change-create 逻辑是否本次迁移 | 否 | 当前 change 仅处理增量功能，整体迁移超范围且回归面大 | 一并迁移 |
| 工具命名 | `workflow_files` | `xx_yy` 两段规则；domain=workflow 与新模块对齐，后续 `workflow.json` 工具可成族 | `change_files_query`（三段不符合既有两段惯例）、`files_query` |

### 待决问题

- `lib/workflow.ts`（PGE phase 配置）与 `modules/workflow/`（`workflow.json` 操作）命名并存，后续 change 是否重命名澄清（如 `lib/phase-config.ts`）。
- 未来是否出现需向 agent 暴露 `source` 审计映射的运维场景（当前契约禁止，若放开需修订 `workflow-file-inventory` 规格）。
- 其余 `workflow.json` 操作逻辑（eval 写入、创建、回溯）迁入 `modules/workflow` 的切分节奏。

---
