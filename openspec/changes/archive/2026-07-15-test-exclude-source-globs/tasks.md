# 任务: test-exclude-source-globs

## 阶段 1: Schema 扩展

- [x] 在 `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 中 `test` Zod object 内增加 `exclude: z.array(z.string()).optional().describe('排除的源文件 glob 模式列表')`
- [x] 在 `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 中 `test.overrides` 条目的 Zod object 内增加 `exclude: z.array(z.string()).optional().describe('排除的源文件 glob 模式列表')`
- [x] 在 `plugins/dev-team/bin/dev-team-config.schema.json` 中 `test.properties` 下增加 `exclude` 属性（type: array, items: { type: string }, 可选）
- [x] 在 `plugins/dev-team/bin/dev-team-config.schema.json` 中 `test.properties.overrides.items.properties` 下增加 `exclude` 属性（type: array, items: { type: string }, 可选）

## 阶段 2: 共享排除工具函数

- [x] 新建 `plugins/dev-team/bin/src/lib/test-exclude.ts`，实现 `isFileExcluded(filePath, config)` 函数
- [x] 在 `test-exclude.ts` 中实现 `getExcludeGlobs(config)` 函数
- [x] `isFileExcluded` 逻辑：全局 `test.exclude` glob 直接匹配；override-level exclude 仅当文件在该 override 的 `file` glob 范围内时匹配；任一匹配则返回 `true`

## 阶段 3: test-detect-frameworks 集成

- [x] 在 `test-detect-frameworks.ts` 的 `detectFrameworksForFiles` 中，获取 `config` 引用，在文件-框架匹配循环前调用 `isFileExcluded` 过滤排除的文件
- [x] 确保排除的文件完全不进入 `detected[]` 数组（不触发 framework 匹配，也不落入 "unknown" 回退）

## 阶段 4: test-resolve-paths 集成

- [x] 在 `test-resolve-paths.ts` 的 `processNonEmptyModules` 中，在 `detectedSet` 检查之后、`isTestFile` 检查之前，调用 `isFileExcluded` 过滤排除的文件（不添加错误信息，静默跳过）
- [x] 在 `test-resolve-paths.ts` 的 `processEmptyModules` 中，在收集 `sourceFiles` 之后、调用 `addUnitTest` 之前，调用 `isFileExcluded` 过滤排除的文件

## 阶段 5: test-runner 变异测试集成

- [x] 在 `test-runner.ts` 的 `runMutationPhase` 中，在调用 `resolveStrykerConfig` 之前，读取配置并调用 `isFileExcluded` 过滤源文件
- [x] 若过滤后的源文件列表为空，跳过变异测试（返回 `null`）

## 阶段 6: 插件版本号更新

- [x] 在 `plugins/dev-team/.claude-plugin/plugin.json` 中升级版本号
