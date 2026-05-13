## Context

当前插件有两个外部 CLI 依赖，均需用户手动安装：
- `@fission-ai/openspec` (~9MB): 技能流程的核心 CLI，propose/apply/archive 技能均直接调用
- `likec4` (~99MB): 仅 `archi-model.py` 的 `_validate_via_cli()` 中通过 `npx likec4 build` 调用，失败后自动回退到 Python 解析器

SessionStart hook `session-start-ensure-openspec.py` 在会话启动时检查两个 CLI 是否可用，不可用时输出安装提示——但不能自动修复。

`archi-model.py` 和 `archi-validate.py` 各自维护了完整的纯 Python DSL 解析器（包括语法校验、元素提取、关系解析、metadata.path 映射），在实际使用中 likec4 调用往往是冗余的。

## Goals / Non-Goals

**Goals:**
- 用户安装插件后无需任何手动步骤即可使用所有功能
- openspec CLI 随插件自动就绪
- 完全移除 likec4 依赖，消除 ~99MB 的安装负担
- 保持所有现有功能和行为不变

**Non-Goals:**
- 不修改 OpenSpec 工作流的任何语义（propose/apply/archive 流程不变）
- 不修改 model.c4 DSL 语法
- 不移除 Python DSL 解析器（反而是加强其作为唯一路径）
- 不改变报告格式或 hook gate 逻辑

## Decisions

### Decision 1: npm install 到 plugin/bin + wrapper，而非 pkg 打包二进制

**选择**: 在 `plugins/dev-team/bin/` 下执行 `npm install @fission-ai/openspec@<version>`，创建 `bin/openspec` wrapper 脚本。

**备选方案**:
- pkg/nexe 打包成独立可执行文件：跨平台需维护 3 个二进制，约 30-40MB 每个，reproducible build 复杂
- 保持 npx 调用：仍需网络，首次慢

**理由**: npm 包体积 ~9MB（合理），跨平台（Node.js 脚本），版本锁定在 plugin.json 中，更新方便。wrapper 脚本负责转发 CLI 参数。

### Decision 2: SessionStart hook 自举安装

**选择**: `session-start-ensure-openspec.py` 检测 `plugins/dev-team/bin/node_modules/.bin/openspec` 不存在时，自动执行 `npm install` 到 bin 目录。

**理由**: 用户感知为零——安装插件即安装 openspec。首次会话可能会多等待几秒（npm install），后续即时可用。

### Decision 3: 删除 _validate_via_cli()，_validate_structure() 为主路径

**选择**: 移除 `archi-model.py` 中对 `npx likec4 build` 的调用，`validate_dsl()` 直接使用 `_validate_structure()`。

**理由**: `_validate_structure()` 已覆盖所需校验：括号平衡、specification/model 块存在。`archi-validate.py` 完全不调用 likec4——它自己解析 DSL。likec4 在此架构中已是空壳依赖。

### Decision 4: 移除 likec4 检查，不提供回退

**选择**: SessionStart hook 不再检查 likec4，不提供 likec4 安装选项。

**理由**: Python DSL 解析器是功能完备的——它支持元素解析、关系解析、metadata.path 映射、路径匹配。删除 likec4 不损失任何功能。

## Risks / Trade-offs

- **[Risk] openspec CLI 大版本更新导致行为不兼容** → 在 plugin.json 或 package.json 中锁定 `@fission-ai/openspec` 版本，升级需经过测试
- **[Risk] 企业网络环境 npm install 可能失败** → SessionStart hook 中 install 失败时给出明确错误信息，不阻塞会话
- **[Risk] Python DSL 解析器可能遗漏边缘语法** → 现有解析器已在 prod 使用（via archi-validate.py 的 import 解析），风险低；如有遗漏，补充 parser 即可

## Architecture

```
Before:
  plugins/dev-team/
  ├── hooks/session-start-ensure-openspec.py  → 检查全局 openspec、likec4
  ├── utils/archi-model.py                    → npx likec4 build → 回退 Python
  ├── utils/archi-validate.py                 → 纯 Python DSL 解析
  └── (无 bin/)

After:
  plugins/dev-team/
  ├── bin/
  │   ├── node_modules/        (npm install @fission-ai/openspec)
  │   └── openspec             (wrapper: node_modules/.bin/openspec "$@")
  ├── hooks/session-start-ensure-openspec.py  → 检查+自举 bin/openspec
  ├── utils/archi-model.py                    → 纯 Python _validate_structure()
  ├── utils/archi-validate.py                 → 纯 Python DSL 解析 (不变)
  └── (不再引用 likec4)
```

## Open Questions

- openspec 版本锁定在 plugin.json 的哪个字段？建议新增 `"dependencies"` 或 `"openspecVersion"` 字段
- Windows 下 wrapper 脚本用 `.cmd` 还是 `. ps1`？
