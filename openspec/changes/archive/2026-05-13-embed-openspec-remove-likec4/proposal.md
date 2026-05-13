## Why

当前用户安装此插件后，必须手动执行 `npm install -g @fission-ai/openspec` 和 `npm install -g likec4` 两个全局包才能正常使用。SessionStart hook 检测到缺失后仅显示安装提示，无法自动修复。这增加了上手摩擦，且 likec4 安装体积达 ~99MB，而插件实际只用到其 DSL 语法校验能力——该能力已在 Python 端完整实现。

## What Changes

- **Embed openspec CLI**: 将 `@fission-ai/openspec` 自动安装到 `plugins/dev-team/bin/` 目录，SessionStart hook 不再提示手动安装
- **Remove likec4 dependency**: 删除所有对 likec4 CLI 的调用和检查，Python DSL 解析器成为唯一验证路径
- **SessionStart hook 简化**: 检查逻辑从「检测并提示安装两个全局包」变为「确保嵌入式 openspec 就绪」，移除 likec4 检查
- **archi-model.py 简化**: 删除 `_validate_via_cli()` 中的 `npx likec4 build` 调用，直接使用 `_validate_structure()` 纯 Python 验证

## Capabilities

### New Capabilities
- `embedded-cli`: 将 openspec CLI 嵌入插件 bin 目录，插件安装时自动就绪，无需用户手动安装 npm 全局包

### Modified Capabilities
- `architecture-model`: **BREAKING** 移除 likec4 API 依赖（`LikeC4.fromWorkspace()`, `fromSource()`），DSL 解析和校验完全由 Python 实现承担；行为等价，不影响功能
- `architecture-validation`: 移除规范中对 likec4 的引用，明确 DSL 解析由 Python 解析器完成；其余行为不变

## Impact

- **hooks/session-start-ensure-openspec.py**: 重写检查逻辑，移除 likec4 检查，新增嵌入式 openspec 自举逻辑
- **utils/archi-model.py**: 删除 `_validate_via_cli()`，`validate_dsl()` 直接走 `_validate_structure()`
- **skills/openspec-* (propose/apply/archive)**: openspec CLI 路径可能需指向嵌入式版本
- **openspec/specs/architecture-model/**: delta spec 更新 likec4 相关 requirement
- **openspec/specs/architecture-validation/**: delta spec 移除 likec4 引用
