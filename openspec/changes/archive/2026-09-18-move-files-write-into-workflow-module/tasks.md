# 任务: move-files-write-into-workflow-module

> 任务只覆盖 design.md 变更清单中的实现文件；测试文件由 test-design / test-gen 阶段承接，此处不建任务。

## 阶段一：依赖与 gitignore 过滤器

- [x] 在 `plugins/dev-team/package.json` 的 `dependencies` 中新增 `"ignore": "^7.0.0"` 并安装（更新 lockfile）
- [x] 新建 `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts`：导出 `GitignoreFilter`（不透明句柄）、`loadGitignoreFilter(projectRoot, onWarn?)`、`isGitIgnored(filter, relPath)`；根 `.gitignore` 即刻解析，祖先链各层按需惰性解析并缓存于句柄内；层级求值实现目录短路（任一层判定目录被忽略即返回 `true`，不再下探、其内 `.gitignore` 不参与判定）与深层优先（最后产生匹配的层定结论）；某层缺失视为该层无规则；任一层读取/解析/判定异常经 `onWarn` 留诊断并 fail-open（按不忽略处理），MUST NOT 抛错

## 阶段二：写路径语义迁入模块

- [x] 修改 `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts`：自 `commands/change-files.ts` 迁入 `appendFileOps`（原 `applyAppend` + `dedupe` 语义：读改写、桶内折叠合并去重、保留既有 `source`）与 `setFileBuckets`（原 `applySet` 语义：所提供桶整桶覆写、覆写条目清 `source`、消失路径的 `source` 条目删除），持久化经 `readFileInventory` / `writeFileInventory`；`appendFileOps` 不复用 `foldFileOps`（source 清除语义不同）；本文件不引入 gitignore 过滤
- [x] 新建 `plugins/dev-team/bin/src/modules/workflow/files/record.ts`：导出 `recordFileOps(changeDir, ops, context)`（`context: { projectRoot: string; agentType?: string }`）；自 `commands/record-files.ts` 迁入 `normalizeRecordedPath` / `isExcludedFromInventory` 并转为私有；按序实现「规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘」；`GitignoreFilter` 每次调用惰性构建一次（仅存在通过规范化与排除的候选路径时），`onWarn` 诊断经 `process.stderr.write` 输出（前缀 `record-files:`）；raw ops 为空时提前返回
- [x] 修改 `plugins/dev-team/bin/src/modules/workflow/index.ts`：barrel 扩充 `recordFileOps`（自 `files/record.ts`）与 `appendFileOps` / `setFileBuckets`（并入 `files/file-inventory.ts` 导出集）；gitignore API 不进 barrel（仅供管线内部消费，避免 knip dead export）

## 阶段三：命令层瘦身

- [x] 修改 `plugins/dev-team/bin/src/commands/record-files.ts`：删除 `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` 与读折叠写步骤，事件流程改为「提取 raw ops → `resolveChangeDir` → `recordFileOps(changeDir, ops, { projectRoot, agentType })`」；保留 stdin 解析、`isPhaseNextCall` 绑定识别、`bindFromPhaseNextCall`、`extractOpsFromToolInput` 与 `runRecordFiles` 的 exit-0 错误吞并策略；不留 re-export shim
- [x] 修改 `plugins/dev-team/bin/src/commands/change-files.ts`：删除 `applyAppend` / `applySet` / `dedupe`，`runChangeFiles` 校验后委托 `setFileBuckets` / `appendFileOps` 并透传返回投影；不应用 gitignore 过滤；`ChangeFilesOptions` 与 `runChangeFiles` 签名不变；不留 re-export shim

## 阶段四：版本与产物

- [x] 提升 `plugins/dev-team/package.json` 的 `version` 至 `2.10.39`
- [x] 执行 `pnpm -C plugins/dev-team run build`，确认 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 产物刷新（git status 可见产物更新）
