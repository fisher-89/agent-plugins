## ADDED Requirements

### Requirement: requirements 阶段识别受影响的模块目录
requirements-planner SHALL 在生成 proposal.md 和 specs/ 时，分析当前变更涉及的文件变更集合，识别出所有受影响的模块目录。
每个模块目录 SHALL 被分配一个唯一的模块标识符（如 `module:user-auth`、`module:data-pipeline`）。
模块目录的识别规则 SHALL 以项目根目录下的 CLAUDE.md 中的模块定义为依据；如果 CLAUDE.md 中无模块定义，以 `src/` 或 `packages/` 等源码顶层目录的子目录为模块单位。

#### Scenario: requirements 阶段识别变更涉及的模块
- **WHEN** requirements-planner 分析当前变更涉及的文件变更集合
- **THEN** 它遍历变更路径列表，提取所有非重复的顶层模块目录路径
- **AND** 为每个模块目录生成唯一模块标识符

### Requirement: requirements 阶段定义模块的 public/export 函数签名
对于每个受影响的模块，requirements-planner SHALL 定义该模块的 public/export 函数的接口契约。
每个函数条目 SHALL 包含：函数名、参数列表（名称 + 类型 + 是否必需 + 默认值）、返回值类型、简要功能描述。
参数和返回值的类型 SHALL 使用 TypeScript 风格的类型注解或语言对应的类型表示法。
函数签名条目 SHALL 以结构化表格形式写入 specs/ 目录下对应 capability 的 spec.md 文件中。

#### Scenario: 函数签名契约写入 spec 文件
- **WHEN** requirements-planner 定义了模块的 public 函数签名
- **THEN** 它在 `openspec/changes/<name>/specs/<capability-name>/spec.md` 的 `## ADDED Requirements` 下插入函数签名契约表格
- **AND** 表格包含列：函数名、参数（名称:类型）、返回值、描述

#### Scenario: 无 public export 的模块
- **WHEN** 受影响的模块没有 public/export 函数（如纯数据配置变更）
- **THEN** 模块边界契约中注明 "此模块无可导出函数"，不生成空表格

### Requirement: requirements 阶段定义模块的 API 接口
对于每个受影响的模块，requirements-planner SHALL 定义该模块对外暴露的 API 接口契约。
每个 API 条目 SHALL 包含：method（GET/POST/PUT/DELETE/PATCH）、path（相对于 API 根路径）、request schema（参数名 + 类型 + 位置）、response schema（状态码 + 返回体结构）。
API 接口条目 SHALL 以结构化表格形式写入 specs/ 目录下对应 capability 的 spec.md 文件中。

#### Scenario: API 接口契约写入 spec 文件
- **WHEN** requirements-planner 定义了模块的 API 接口
- **THEN** 它在 `openspec/changes/<name>/specs/<capability-name>/spec.md` 中插入 API 接口契约表格
- **AND** 表格包含列：方法、路径、请求参数、响应状态码、响应体结构

#### Scenario: 无 API 接口的模块
- **WHEN** 受影响的模块不暴露 API 接口（如纯后端工具函数模块）
- **THEN** 模块边界契约中注明 "此模块无 API 接口"，不生成空表格

### Requirement: requirements 阶段定义模块的 CLI 命令
对于每个受影响的模块，requirements-planner SHALL 定义该模块对外暴露的 CLI 命令契约。
每个 CLI 条目 SHALL 包含：命令名、参数列表（名称 + 类型 + 必需）、flags（名称 + 缩写 + 类型 + 默认值）、使用示例。
CLI 命令条目 SHALL 以结构化表格形式写入 specs/ 目录下对应 capability 的 spec.md 文件中。

#### Scenario: CLI 命令契约写入 spec 文件
- **WHEN** requirements-planner 定义了模块的 CLI 命令
- **THEN** 它在 `openspec/changes/<name>/specs/<capability-name>/spec.md` 中插入 CLI 命令契约表格
- **AND** 表格包含列：命令名、参数、flags、示例

#### Scenario: 无 CLI 命令的模块
- **WHEN** 受影响的模块不暴露 CLI 命令
- **THEN** 模块边界契约中注明 "此模块无 CLI 命令"，不生成空表格

### Requirement: requirements 阶段定义前端组件的 props & events
对于前端项目中的受影响模块，requirements-planner SHALL 定义该模块的 Vue/React 组件接口契约。
每个组件条目 SHALL 包含：组件名、props（名称 + 类型 + 是否必需 + 默认值）、events（事件名 + payload 类型）。
组件接口条目 SHALL 以结构化表格形式写入 specs/ 目录下对应 capability 的 spec.md 文件中。
如果当前项目不是前端项目，此条目不适用。

#### Scenario: 前端组件契约写入 spec 文件
- **WHEN** requirements-planner 确认当前项目包含前端代码（检测到 `.vue`、`.tsx`、`.jsx` 文件）
- **THEN** 它为受影响的前端模块生成组件 props & events 契约表格
- **AND** 表格包含列：组件名、props（名称:类型:必需）、events（事件名:payload）

#### Scenario: 非前端项目跳过组件契约
- **WHEN** requirements-planner 确认当前项目无前端代码
- **THEN** 模块边界契约中注明 "此项目无前端组件定义"，不生成组件表格
