# 任务: dual-platform-plugin-build

> **变更**: dual-platform-plugin-build
> **日期**: 2026-07-27

---

## 阶段 1: 源码布局与版本权威

- [x] 将 `plugins/dev-team/bin/package.json` 上移为 `plugins/dev-team/package.json`，同步上移 `pnpm-lock.yaml`（若存在）
- [x] 将上移后 `package.json` 的 `version` 设为 `2.10.3`（不得保留 `1.0.0`）；保留 `scripts.build` 为 `vp pack`；`name` 暂可保留 `dev-team-cli`
- [x] 删除 `plugins/dev-team/bin/package.json`
- [x] 删除 `plugins/dev-team/.claude-plugin/`（含 `plugin.json`），确保源码树无 `.claude-plugin/` / `.cursor-plugin/`
- [x] 将 `plugins/dev-team/bin/vite.config.ts` 上移到 `plugins/dev-team/vite.config.ts`，把 pack 入口/输出、lint/fmt ignore、test include 及 schema 写入路径改为相对源码根下的 `bin/`（例如 `bin/src/mcp.ts` → `bin/dev-team-mcp.cjs`）
- [x] 按需调整 `bin/tsconfig.json` / `bin/knip.json` / `bin/stryker.config.json` 与根 `package.json` scripts，使在 `plugins/dev-team` 下可执行 `vp pack`（及既有 check/test 入口不因路径断裂）
- [x] 在 `plugins/dev-team` 执行一次 `vp pack`，确认 `bin/dev-team-mcp.cjs`、`bin/dev-team-cli.cjs`、`bin/dev-team-hooks.cjs`（及 schema）正常生成

## 阶段 2: 根构建脚本与双产物组装

- [x] 新增 `scripts/build-plugins.mjs`：读取 `plugins/dev-team/package.json` 的 `version`
- [x] 在脚本中以 cwd `plugins/dev-team` 调用 `vp pack`（vite-plus）；失败时非零退出；不引入第二套 bundler
- [x] 实现产物组装：将 `agents/`、`skills/`、`hooks/`、`utils/`、`templates/`、`.mcp.json` 以及 `bin/` 运行时文件（`dev-team-*.cjs`、`dev-team-config.schema.json`、`openspec`、`openspec.cmd`、`openspec-bundled.js`；排除 `src/`、`__tests__/`、开发配置、`.map`、`node_modules`）同步到 `claude-plugins/dev-team` 与 `cursor-plugins/dev-team`（清空后重建或等价完整同步）
- [x] 写入 `claude-plugins/dev-team/.claude-plugin/plugin.json`：`name`=`dev-team`，`version`=源 `package.json` 的 `version`，其余字段沿用现有 Claude manifest 约定（`description` / `bin` / `openspecVersion` / `author`）
- [x] 写入 `cursor-plugins/dev-team/.cursor-plugin/plugin.json`：至少含 `name`=`dev-team`、`version`（与 Claude 相同）、`description`；本迭代允许与 Claude 元数据同构
- [x] 确保构建不会在 `plugins/dev-team` 下生成平台 manifest 目录；可为 Cursor 额外文件预留空扩展点（不强制写专用 `mcp.json`）
- [x] 在仓库根执行 `node scripts/build-plugins.mjs`，确认双产物目录与两边 `plugin.json` 的 `version` 一致

## 阶段 3: Marketplace、gitignore 与文档

- [x] 修改 `.claude-plugin/marketplace.json`：将 `dev-team` 的 `source` 改为 `./claude-plugins/dev-team`；description 与产物 manifest 对齐
- [x] 新增 `.cursor-plugin/marketplace.json`：注册 `dev-team`，`source` 为 `./cursor-plugins/dev-team`，description 与 Cursor 产物 manifest 对齐
- [x] 更新 `.gitignore`：增加 `!.cursor-plugin`；确认未忽略 `claude-plugins/`、`cursor-plugins/`
- [x] 更新 `CLAUDE.md` 升版规则：改插件代码后升 `plugins/<name>/package.json` 并重建双产物（根构建脚本）
- [x] 将首轮构建出的 `claude-plugins/`、`cursor-plugins/` 与 `.cursor-plugin/` 纳入可跟踪文件（准备 commit；本任务不执行 git commit，除非另行要求）

## 阶段 4: 实现自检（不含写测试）

- [x] AC-1：`plugins/dev-team` 下不存在 `.claude-plugin/` 或 `.cursor-plugin/`
- [x] AC-2：存在 `plugins/dev-team/package.json`；无 `bin/package.json`；`version` 为 `2.10.3`（非 `1.0.0`）
- [x] AC-3：存在可执行的 `scripts/build-plugins.mjs`；运行后调用 `vp pack` 并刷新双产物
- [x] AC-4 / AC-5：两边产物 `plugin.json` 存在且 `version` 等于源 `package.json`
- [x] AC-6：双 marketplace 的 `source` 分别指向 `./claude-plugins/dev-team` 与 `./cursor-plugins/dev-team`
- [x] AC-7：`.gitignore` 含 `!.cursor-plugin`；产物目录可被 git 跟踪
- [x] AC-8：`CLAUDE.md` 已更新升版与重建双产物规则
- [x] AC-9：产物树含安装所需内容（manifest + hooks/skills/agents/bin 运行时等）；两边可同构

## 阶段 5: 回溯修复 — openspec 测试根与路径

- [x] 更新 `openspec/config.json`：`tests[].root` 改为 `plugins/dev-team`（随 `vite.config.ts` / `package.json` 上移），`includes`/`excludes` 改为相对新根的 `bin/src/**` 路径
- [x] 更新 `static_analysis` 为 `pnpm run -C ./plugins/dev-team check`；`write_protection` 的 knip 路径保持 `plugins/dev-team/bin/knip.json`（knip 仍在 bin/）
- [x] 新增 `scripts` 的 `node-test` suite（`includes: ["build-plugins.mjs"]`），使 `scripts/build-plugins.test.mjs` 可被 test-execution 发现并执行
