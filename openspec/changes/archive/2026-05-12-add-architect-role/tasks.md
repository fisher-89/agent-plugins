## 1. 基础设施

- [x] 1.1 创建 `openspec/architecture/` 目录结构（model.c4, views/, decisions/, reports/）
- [x] 1.2 创建 `plugins/dev-team/templates/adr.md` ADR 模板
- [x] 1.3 创建 `plugins/dev-team/templates/validate-report.json` 验证报告 schema 模板
- [x] 1.4 创建 `plugins/dev-team/templates/model.c4` likec4 模型初始化模板
- [x] 1.5 确保 SessionStart 钩子检测 likec4 CLI 可用性（`npx likec4 --version`）

## 2. archi-model 子代理

- [x] 2.1 创建 `plugins/dev-team/agents/archi-model.md` 子代理定义（读: likec4 API; 写: DSL 编辑 + fromSource 验证）
- [x] 2.2 创建 `plugins/dev-team/utils/archi-model.py` 模型操作工具（读取模型结构、验证 DSL 语法、写入 DSL）
- [x] 2.3 实现 `fromSource()` 验证流程：修改前解析 → 编辑 DSL → 重新解析 → 通过后写入

## 3. archi-validate 子代理

- [x] 3.1 创建 `plugins/dev-team/agents/archi-validate.md` 子代理定义
- [x] 3.2 创建 `plugins/dev-team/utils/archi-validate.py` 验证引擎
- [x] 3.3 实现 metadata.path 匹配逻辑（目录递归匹配所有子文件）
- [x] 3.4 实现 import 解析（支持 TypeScript/JavaScript/Python import 语句）
- [x] 3.5 实现依赖对账：将 import 目标映射到模型元素，交叉比对 relationships
- [x] 3.6 实现未建模依赖检测（代码 import 了未映射到任何元素的模块）
- [x] 3.7 实现未使用关系检测（模型声明了关系但代码中无对应 import）
- [x] 3.8 实现 JSON 报告输出（`reports/validate-<timestamp>.json`）
- [x] 3.9 实现模型变更摘要（当 architecture/ 也在变更中时）

## 4. archi-decide 子代理

- [x] 4.1 创建 `plugins/dev-team/agents/archi-decide.md` 子代理定义
- [x] 4.2 创建 `plugins/dev-team/utils/archi-decide.py` ADR 管理工具
- [x] 4.3 实现 ADR 创建（按日期命名，含所有必要字段和影响范围）
- [x] 4.4 实现 ADR 查询（列表、按状态筛选）
- [x] 4.5 实现 ADR 状态更新（proposed → accepted → deprecated → superseded）

## 5. 提交验证钩子

- [x] 5.1 创建 `plugins/dev-team/hooks/pre-tool-architecture-gate.py` 钩子脚本
- [x] 5.2 实现 git commit 检测（PreToolUse Bash matcher）
- [x] 5.3 实现变更文件扫描（全部文件，非仅匹配项）
- [x] 5.4 实现验证报告检查（staged 中是否存在 validate-*.json）
- [x] 5.5 实现 deny 输出（引导代理运行 archi-validate）
- [x] 5.6 更新 `plugins/dev-team/hooks/hooks.json` 注册新钩子

## 6. 端到端验证

- [x] 6.1 在 demo-project 中创建示例 model.c4 并验证 archi-model 读写流程
- [x] 6.2 模拟代码变更，验证 archi-validate 的依赖检测和报告生成
- [x] 6.3 模拟提交，验证钩子拦截和放行逻辑
