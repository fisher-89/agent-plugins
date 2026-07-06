# 提案: Write Protection Config

> **变更**: write-protection-config
> **日期**: 2026-07-06
> **状态**: 草稿

---

## 问题

当前插件仅通过硬编码的正则表达式保护 `eval.json` 文件（protect-eval.mjs），存在以下问题：

1. **保护范围有限**：仅有 `eval.json` 受保护，`config.json` 等重要配置文件无保护机制。
2. **不可配置**：用户无法自定义需要保护的文件或 glob 模式，无法针对项目特定的关键文件设置保护。
3. **拒绝文案不可定制**：硬编码的中文拒绝原因无法根据场景调整。
4. **MCP 工具冗余**：`config_set`、`config_unset`、`config_context` 三个 MCP 工具未被任何 agent 或 skill 使用，增加维护负担和工具面复杂度。

---

## 提案

在 `openspec/config.json` 中新增 `write_protection` 配置项，实现可配置的全局写入保护机制：

1. **配置驱动**：通过 `write_protection.files` 数组定义 glob 模式，匹配的文件将受到写入保护。
2. **内置默认保护**：`eval.json` 和 `config.json` 始终受保护，无需显式配置。
3. **可定制拒绝文案**：每个文件规则可指定 `reason` 字段，覆盖默认拒绝提示。
4. **Hook 升级**：将 `protect-eval.mjs` 升级为 `protect-files.mjs`，读取 `config.json` 驱动保护逻辑。
5. **工具清理**：移除 `config_set`、`config_unset`、`config_context` 三个 MCP 工具及其相关命令和 schema 文件。

---

## 能力

### 新增能力

- **write-protection-config** — 在 `openspec/config.json` 中定义 `write_protection` 配置项的结构和语义
- **protect-files-hook** — 将 protect-eval.mjs 升级为 config 驱动的 protect-files.mjs PreToolUse hook，支持可配置的写入保护

### 修改的能力

- **config-schema** — 在 configSchema 中新增 `write_protection` 字段定义
- **eval-json-protection** — 从硬编码 eval.json 保护扩展为 config 驱动的通用文件保护机制
- **config-context** — 删除（MCP 工具移除）
- **config-set** — 删除（MCP 工具移除）
- **config-unset** — 删除（MCP 工具移除）

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — 新增 write_protection 类型定义
- `plugins/dev-team/bin/src/mcp.ts` — 移除 config_set、config_unset、config_context 注册
- `plugins/dev-team/hooks/scripts/protect-eval.mjs` → `protect-files.mjs` — 升级为 config 驱动的通用文件保护
- `plugins/dev-team/hooks/hooks.json` — 更新 PreToolUse hook 脚本引用路径
- `plugins/dev-team/bin/src/commands/config-set.ts` — 删除
- `plugins/dev-team/bin/src/commands/config-unset.ts` — 删除
- `plugins/dev-team/bin/src/commands/config-context.ts` — 删除
- `plugins/dev-team/bin/src/schemas/config-set.schema.ts` — 删除
- `plugins/dev-team/bin/src/schemas/config-unset.schema.ts` — 删除
- `plugins/dev-team/bin/src/schemas/config-context.schema.ts` — 删除
- `plugins/dev-team/bin/src/schemas/index.ts` — 移除 config-set.schema、config-unset.schema、config-context.schema 导出
- `plugins/dev-team/.claude-plugin/plugin.json` — 版本号升级

### 测试文件

- `plugins/dev-team/bin/__tests__/protect-eval-regression/protect-eval-regression.test.ts` — 扩展为 protect-files 测试，覆盖 config 驱动场景

### 不要修改

- `plugins/dev-team/bin/src/commands/config-get.ts` — config_get 工具保留
- `plugins/dev-team/bin/src/lib/config.ts` — 基础配置文件读写逻辑不变
- `plugins/dev-team/bin/src/schemas/config-get.schema.ts` — config_get schema 保留
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 中的 `.passthrough()` 行为 — 保持向后兼容

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | config schema 新增 write_protection 字段 | Zod schema 接受并验证包含 write_protection.files[].glob 和 reason 的 config.json |
| AC-2 | protect-files.mjs 读取 config.json | Hook 启动时读取 openspec/config.json，提取 write_protection 配置 |
| AC-3 | 内置默认保护 | 不配置 write_protection 时，protect-files.mjs 仍然阻止对 eval.json 和 config.json 的直接写入 |
| AC-4 | 用户自定义 glob 保护 | 配置 `write_protection.files` 后，匹配 glob 模式的文件写入被拒绝 |
| AC-5 | 自定义拒绝文案 | 配置 `reason` 后，拒绝时返回该自定义文案（支持 %s/%t 占位符） |
| AC-6 | MCP 工具移除 | config_set、config_unset、config_context 在 MCP 工具列表中不再出现 |
| AC-7 | 命令文件删除 | config-set.ts、config-unset.ts、config-context.ts 及其 schema 文件被删除 |
| AC-8 | Hook fail-open 行为保持 | 输入异常时 protect-files.mjs 默认返回 allow |
| AC-9 | 版本升级 | plugin.json 版本号更新 |
| AC-10 | 现有 eval.json 保护向后兼容 | 未配置 write_protection 时，对 eval.json 的写入仍被拒绝 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| config.json 解析失败导致保护失效 | 受保护文件可能被意外写入 | 低 | Hook 在解析失败时回退到内置默认保护（eval.json + config.json） |
| 用户配置的 glob 过宽导致正常写入被拒 | 开发者工作流受阻 | 中 | 拒绝文案中提示冲突的 glob 规则，方便用户调试 |
| 删除的 MCP 工具被遗留脚本引用 | 引用处报错 | 低 | 已验证无任何 agent/skill 使用这三个工具 |
| Windows 路径分隔符不匹配 glob 模式 | 保护规则在 Windows 上失效 | 中 | Hook 中对文件路径统一做 `\\` → `/` 归一化处理 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 保护模式 | glob 模式 + 内置默认保护 | 灵活性与安全性兼顾 | 仅内置固定列表（不够灵活）；仅用户配置（默认无保护） |
| 配置位置 | openspec/config.json | 统一配置入口，复用现有 schema 验证机制 | 独立配置文件（增加复杂度） |
| 违规行为 | 拒绝写入 + 定制提示 | 明确阻断并提供可操作的反馈 | 仅记录日志不阻断（不安全） |

### 待决问题

- 无
