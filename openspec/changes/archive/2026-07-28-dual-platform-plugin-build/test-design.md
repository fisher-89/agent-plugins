# 测试设计: dual-platform-plugin-build

> **日期**: 2026-07-27

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team` 下不存在 `.claude-plugin/` 或 `.cursor-plugin/` | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | 构建后源码无平台 manifest |
| AC-1 | `plugins/dev-team` 下不存在 `.claude-plugin/` 或 `.cursor-plugin/` | 单元测试 | `scripts/build-plugins.test.mjs` | `ensureSourceCleanOfPlatformManifests` |
| AC-2 | 存在 `plugins/dev-team/package.json`；无 `plugins/dev-team/bin/package.json`；`version` 为 `2.10.3`（或约定下一 semver），非 `1.0.0` | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | 版本权威上移布局 |
| AC-3 | 仓库根存在可执行的 `scripts/build-plugins.*`；运行后在 `plugins/dev-team` 调用 `vp pack`（vite-plus）并刷新双产物 | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | 根构建入口 → vp pack → 双产物 |
| AC-3 | 仓库根存在可执行的 `scripts/build-plugins.*`；运行后在 `plugins/dev-team` 调用 `vp pack`（vite-plus）并刷新双产物 | 单元测试 | `scripts/build-plugins.test.mjs` | `runVpPack` |
| AC-3 | 仓库根存在可执行的 `scripts/build-plugins.*`；运行后在 `plugins/dev-team` 调用 `vp pack`（vite-plus）并刷新双产物 | 单元测试 | `plugins/dev-team/vite.config.test.ts` | `pack` 入口/输出相对 `bin/` |
| AC-4 | `claude-plugins/dev-team/.claude-plugin/plugin.json` 存在，且 `version` 等于源 `package.json` 的 `version` | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | package.json version → Claude plugin.json |
| AC-4 | `claude-plugins/dev-team/.claude-plugin/plugin.json` 存在，且 `version` 等于源 `package.json` 的 `version` | 单元测试 | `scripts/build-plugins.test.mjs` | `writePlatformManifest`（claude） |
| AC-5 | `cursor-plugins/dev-team/.cursor-plugin/plugin.json` 存在，且 `version` 等于源 `package.json` 的 `version` | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | package.json version → Cursor plugin.json |
| AC-5 | `cursor-plugins/dev-team/.cursor-plugin/plugin.json` 存在，且 `version` 等于源 `package.json` 的 `version` | 单元测试 | `scripts/build-plugins.test.mjs` | `writePlatformManifest`（cursor） |
| AC-6 | `.claude-plugin/marketplace.json` 的 dev-team `source` 为 `./claude-plugins/dev-team`；`.cursor-plugin/marketplace.json` 的 `source` 为 `./cursor-plugins/dev-team` | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | marketplace source → 产物目录 |
| AC-7 | `.gitignore` 含 `!.cursor-plugin`；`claude-plugins/`、`cursor-plugins/` 未被 ignore，可被 commit | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | gitignore 放行 Cursor 清单与产物 |
| AC-8 | `CLAUDE.md` 写明：改插件代码后升 `plugins/<name>/package.json` 并重建双产物 | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | CLAUDE.md 升版规则文案 |
| AC-9 | 本迭代两边产物可按 marketplace 路径安装；允许 skills/agents/hooks 内容暂同构 | 集成测试 | `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts` | 双产物最小可装树同构 |
| AC-9 | 本迭代两边产物可按 marketplace 路径安装；允许 skills/agents/hooks 内容暂同构 | 单元测试 | `scripts/build-plugins.test.mjs` | `assembleProductTree` / `shouldCopyPath` |

---

## 单元测试

> **说明**: `test_resolve_paths` 将 `scripts/build-plugins.mjs` 映射到 `scripts/build-plugins.test.mjs`，将上移后的 `plugins/dev-team/vite.config.ts` 映射到 `plugins/dev-team/vite.config.test.ts`。`test_detect_frameworks` 对上述两文件返回 `framework: unknown`（当前 `openspec/config.json` 的 suite 仅覆盖 `plugins/dev-team/bin` + `src/**/*.{ts,tsx}`）。实现侧为便于单测，应将构建辅助函数以命名 export 暴露（CLI 入口仍可 `import.meta.url === …` 时执行）；`vite.config` 单测可在扩展 suite 或临时以 vite-plus/`node --test` 运行。设计文档中的「无模块级 export 要求」不阻止测试导出。

### `scripts/build-plugins.mjs` -> `scripts/build-plugins.test.mjs`

#### 待测功能

- `readPackageVersion(packageJsonPath: string): string`: 读取插件源 `package.json` 的权威 `version`
- `runVpPack(pluginRoot: string): Promise<void>`: 在插件源码根以 cwd 执行 `vp pack`；失败时抛错或非零退出
- `shouldCopyPath(relativePath: string): boolean`: 判断相对路径是否应复制到产物（排除 `bin/src`、`bin/__tests__`、开发配置、`.map`、`node_modules` 等）
- `assembleProductTree(srcRoot: string, destRoot: string): Promise<void>`: 清空/同步可安装内容到产物目录
- `writePlatformManifest(productRoot: string, platform: 'claude' | 'cursor', version: string, meta?: object): Promise<void>`: 写入 `.claude-plugin/plugin.json` 或 `.cursor-plugin/plugin.json` 并注入 `version`
- `ensureSourceCleanOfPlatformManifests(pluginRoot: string): void`: 断言或清理源码树不得残留平台 manifest 目录
- `buildPlugins(options?: { repoRoot?: string }): Promise<void>`: 编排读 version → `vp pack` → 组装双产物 → 写双边 manifest

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `readPackageVersion` | 正向 | 给定含 `"version": "2.10.3"` 的临时 `package.json`，返回 `"2.10.3"` | 新增 |
| `readPackageVersion` | 异常 | `packageJsonPath` 指向不存在的文件时抛错（或非零失败语义） | 新增 |
| `readPackageVersion` | 异常 | `packageJsonPath` 为 `null`/`undefined` 时抛错 | 新增 |
| `readPackageVersion` | 异常 | JSON 非法（无法 parse）时抛错 | 新增 |
| `readPackageVersion` | 异常 | JSON 为 `null` 时抛错 | 新增 |
| `readPackageVersion` | 异常 | JSON 为数组（非 object）时抛错 | 新增 |
| `readPackageVersion` | 边界 | `packageJsonPath` 为 `""` 时抛错或拒绝执行 | 新增 |
| `readPackageVersion` | 边界 | `packageJsonPath` 为超长字符串（>1000 chars）时行为确定性（抛错或拒绝，且不崩溃） | 新增 |
| `readPackageVersion` | 边界 | `packageJsonPath` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `readPackageVersion` | 边界 | `version` 缺失时抛错 | 新增 |
| `readPackageVersion` | 边界 | `version` 为 `""`（空字符串）时抛错或按约定拒绝 | 新增 |
| `readPackageVersion` | 边界 | `version` 为超长字符串（>1000 chars）时仍按字面返回或按 semver 校验拒绝（与实现约定一致且确定性） | 新增 |
| `readPackageVersion` | 边界 | `version` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `readPackageVersion` | 边界 | `package.json` 为 `{}`（空对象）时因缺 `version` 抛错 | 新增 |
| `readPackageVersion` | 边界 | `package.json` 含多余字段时仍正确读取 `version` | 新增 |
| `readPackageVersion` | 边界 | `version` 为 `"1.0.0"` 时函数仍返回字面值（权威校验留给集成/编排层，不在读取层静默改写） | 新增 |
| `runVpPack` | 正向 | mock `spawn`/`execFile`：调用参数 command 为 `vp`（或 `vp pack` 等价），`cwd` 为传入的 `pluginRoot` | 新增 |
| `runVpPack` | 正向 | mock 子进程 exit code `0` 时 Promise resolve / 不抛错 | 新增 |
| `runVpPack` | 异常 | mock 子进程 exit code 非 `0` 时抛错或向上传播失败 | 新增 |
| `runVpPack` | 异常 | mock 子进程 `ENOENT`（`vp` 不存在）时抛错 | 新增 |
| `runVpPack` | 异常 | `pluginRoot` 为 `null`/`undefined` 时抛错或拒绝执行 | 新增 |
| `runVpPack` | 边界 | `pluginRoot` 为 `""` 时抛错或拒绝执行 | 新增 |
| `runVpPack` | 边界 | `pluginRoot` 为超长字符串（>1000 chars）时行为确定性（抛错或拒绝，且不崩溃） | 新增 |
| `runVpPack` | 边界 | `pluginRoot` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `runVpPack` | 边界 | 断言未调用 webpack / esbuild / rollup / tsup 等替代 bundler 命令 | 新增 |
| `shouldCopyPath` | 正向 | `agents/foo.md`、`skills/bar/SKILL.md`、`hooks/hooks.json`、`utils/x.py`、`templates/a.md`、`.mcp.json` 返回 `true` | 新增 |
| `shouldCopyPath` | 正向 | `bin/dev-team-mcp.cjs`、`bin/dev-team-cli.cjs`、`bin/dev-team-hooks.cjs`、`bin/dev-team-config.schema.json`、`bin/openspec`、`bin/openspec.cmd`、`bin/openspec-bundled.js` 返回 `true` | 新增 |
| `shouldCopyPath` | 异常 | `bin/src/mcp.ts`、`bin/__tests__/x/x.test.ts` 返回 `false` | 新增 |
| `shouldCopyPath` | 异常 | `bin/node_modules/pkg/index.js`、`bin/dev-team-mcp.cjs.map` 返回 `false` | 新增 |
| `shouldCopyPath` | 异常 | `relativePath` 为 `null`/`undefined` 时抛错或返回 `false`（行为确定性） | 新增 |
| `shouldCopyPath` | 边界 | `relativePath` 为 `""` 返回 `false` | 新增 |
| `shouldCopyPath` | 边界 | `relativePath` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃（返回 `false` 或按约定拒绝） | 新增 |
| `shouldCopyPath` | 边界 | `relativePath` 为 `bin/package.json`（源码侧已删除，不应进入产物）返回 `false` | 新增 |
| `shouldCopyPath` | 边界 | `relativePath` 为 `.claude-plugin/plugin.json` 或 `.cursor-plugin/plugin.json` 返回 `false`（manifest 由写入步骤生成，非盲拷） | 新增 |
| `shouldCopyPath` | 边界 | Windows 风格 `bin\\src\\mcp.ts` 与 POSIX `bin/src/mcp.ts` 均返回 `false` | 新增 |
| `shouldCopyPath` | 边界 | 超长相对路径（>1000 chars）以 `agents/` 前缀时仍返回 `true` 且不崩溃 | 新增 |
| `assembleProductTree` | 正向 | 临时源树含 `agents/a.md` 与 `bin/dev-team-mcp.cjs`，目标为空目录；组装后目标含对应文件 | 新增 |
| `assembleProductTree` | 正向 | 目标已有陈旧文件 `stale.txt`；组装后该文件不存在（清空后重建或等价完整同步） | 新增 |
| `assembleProductTree` | 异常 | `srcRoot` 不存在时抛错 | 新增 |
| `assembleProductTree` | 异常 | `srcRoot` 为 `null`/`undefined` 时抛错 | 新增 |
| `assembleProductTree` | 异常 | `destRoot` 为 `null`/`undefined` 时抛错 | 新增 |
| `assembleProductTree` | 边界 | `srcRoot` 为 `""` 时抛错或拒绝执行 | 新增 |
| `assembleProductTree` | 边界 | `destRoot` 为 `""` 时抛错或拒绝执行 | 新增 |
| `assembleProductTree` | 边界 | `srcRoot` 为超长字符串（>1000 chars）时行为确定性且不崩溃 | 新增 |
| `assembleProductTree` | 边界 | `destRoot` 为超长字符串（>1000 chars）时行为确定性且不崩溃 | 新增 |
| `assembleProductTree` | 边界 | `srcRoot` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `assembleProductTree` | 边界 | `destRoot` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `assembleProductTree` | 边界 | 源树无可复制文件（空插件内容）时目标被清空且不抛错 | 新增 |
| `assembleProductTree` | 边界 | 源树仅含应排除路径（如仅 `bin/src`）时目标不含这些路径 | 新增 |
| `writePlatformManifest` | 正向 | `platform='claude'` 时写入 `productRoot/.claude-plugin/plugin.json`，`name`=`"dev-team"`，`version` 等于入参 | 新增 |
| `writePlatformManifest` | 正向 | `platform='cursor'` 时写入 `productRoot/.cursor-plugin/plugin.json`，`name`=`"dev-team"`，`version` 等于入参 | 新增 |
| `writePlatformManifest` | 正向 | Claude/Cursor 两边以同一 `version` 调用后两边 JSON 的 `version` 字段相同 | 新增 |
| `writePlatformManifest` | 异常 | `version` 为 `undefined`/`null` 时抛错 | 新增 |
| `writePlatformManifest` | 异常 | `productRoot` 为 `null`/`undefined` 时抛错 | 新增 |
| `writePlatformManifest` | 异常 | `platform` 为 `null`/`undefined` 时抛错 | 新增 |
| `writePlatformManifest` | 异常 | `platform` 为非法枚举值（如 `'vscode'`）时抛错 | 新增 |
| `writePlatformManifest` | 边界 | `platform` 为 `""` 时抛错或按约定拒绝 | 新增 |
| `writePlatformManifest` | 边界 | `version` 为 `""` 时抛错或按约定拒绝 | 新增 |
| `writePlatformManifest` | 边界 | `version` 为超长字符串（>1000 chars）时行为确定性（写入字面值或按约定拒绝，且不崩溃） | 新增 |
| `writePlatformManifest` | 边界 | `version` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `writePlatformManifest` | 边界 | `productRoot` 为 `""` 时抛错或拒绝执行 | 新增 |
| `writePlatformManifest` | 边界 | `productRoot` 为超长字符串（>1000 chars）时行为确定性且不崩溃 | 新增 |
| `writePlatformManifest` | 边界 | `productRoot` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `writePlatformManifest` | 边界 | `meta` 为 `{}` 时仍写出最小必填字段（`name`/`version`/`description`） | 新增 |
| `writePlatformManifest` | 边界 | `meta` 含多余字段时合并写入且不丢失 `version` | 新增 |
| `writePlatformManifest` | 边界 | `meta` 为 `null`/`undefined` 时使用默认 description 等元数据 | 新增 |
| `writePlatformManifest` | 边界 | `productRoot` 尚不存在时自动创建平台目录再写入 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 正向 | 源码根无 `.claude-plugin`/`.cursor-plugin` 时不抛错 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 异常 | 源码根存在 `.claude-plugin/` 时抛错（或构建后校验失败） | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 异常 | 源码根存在 `.cursor-plugin/` 时抛错 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 异常 | `pluginRoot` 为 `null`/`undefined` 时抛错 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 边界 | `pluginRoot` 为 `""` 时抛错或拒绝执行 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 边界 | `pluginRoot` 为超长字符串（>1000 chars）时行为确定性且不崩溃 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 边界 | `pluginRoot` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `ensureSourceCleanOfPlatformManifests` | 边界 | `pluginRoot` 指向不存在的路径时行为确定性（抛错或按约定视为已干净） | 新增 |
| `buildPlugins` | 正向 | mock `readPackageVersion`→`"2.10.3"`、`runVpPack`、`assembleProductTree`×2、`writePlatformManifest`×2 均被按序调用 | 新增 |
| `buildPlugins` | 异常 | `runVpPack` 失败时后续 assemble/write 不被调用 | 新增 |
| `buildPlugins` | 异常 | `options.repoRoot` 为 `null`/`undefined`（显式传入）时抛错或回退到默认仓库根（行为确定性） | 新增 |
| `buildPlugins` | 边界 | `options.repoRoot` 为 `""` 时抛错或拒绝执行 | 新增 |
| `buildPlugins` | 边界 | `options.repoRoot` 为超长字符串（>1000 chars）时行为确定性且不崩溃 | 新增 |
| `buildPlugins` | 边界 | `options.repoRoot` 含特殊字符（`\n`、`\0`、emoji）时行为确定性且不崩溃 | 新增 |
| `buildPlugins` | 边界 | 编排层检测到源 `version==="1.0.0"` 时可选择警告或失败（与实现约定一致；至少不得静默当作成功发布版） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统 | `fs.mkdtempSync` 创建临时 `package.json`、源树与产物目录；测试后 `fs.rmSync` | `readPackageVersion`、`assembleProductTree`、`writePlatformManifest`、`ensureSourceCleanOfPlatformManifests` |
| `child_process.spawn` / `execFile` / `spawnSync` | `vi.mock` 或手动注入 stub，记录 `command`/`args`/`cwd`，可控 exit code | `runVpPack`、`buildPlugins` |
| `runVpPack` / `assembleProductTree` / `writePlatformManifest` | 在 `buildPlugins` 编排测试中 spy 或依赖注入替换为 stub | `buildPlugins` 调用序与失败短路 |

---

### `plugins/dev-team/vite.config.ts` -> `plugins/dev-team/vite.config.test.ts`

#### 待测功能

- `default`（`defineConfig` 导出）: vite-plus 配置；`pack` 条目的 `entry` / `outputOptions.file` 相对源码根下的 `bin/`；`test` include 等路径同步迁到源码根视角

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `pack` 入口/输出相对 `bin/` | 正向 | `pack` 含 mcp / cli / hooks 三项（或与现网等价的命名集合） | 新增 |
| `pack` 入口/输出相对 `bin/` | 正向 | mcp `entry` 为 `bin/src/mcp.ts`（或等价相对源码根路径），`outputOptions.file` 指向 `bin/dev-team-mcp.cjs` | 新增 |
| `pack` 入口/输出相对 `bin/` | 正向 | cli `entry` 为 `bin/src/cli.ts`，输出 `bin/dev-team-cli.cjs` | 新增 |
| `pack` 入口/输出相对 `bin/` | 正向 | hooks `entry` 为 `bin/src/hooks.ts`，输出 `bin/dev-team-hooks.cjs` | 新增 |
| `pack` 入口/输出相对 `bin/` | 正向 | schema 写入路径为 `bin/dev-team-config.schema.json`（若仍在 `build:done` hook 中） | 新增 |
| `pack` 入口/输出相对 `bin/` | 异常 | `pack` 不得仍使用旧的 `src/mcp.ts`（相对 `bin/` cwd）而不带 `bin/` 前缀——在配置已上移到源码根的前提下 | 新增 |
| `pack` 入口/输出相对 `bin/` | 边界 | `format` 为 `'cjs'` 且 `platform` 为 `'node'` | 新增 |
| `pack` 入口/输出相对 `bin/` | 边界 | `OUTPUT_FILE_NAMES`（或等价 ignore 列表）包含三个 `dev-team-*.cjs` 与 schema 文件名 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| vite-plus `defineConfig` | 直接 `import` 配置模块并断言返回对象属性；不启动真实 pack | 全部 `pack` 路径用例 |
| `node:fs.writeFileSync`（schema hook） | 若测试触发 hook，则 spy；默认只断言 hook 闭包内路径字符串或配置静态字段 | schema 路径用例 |

---

## 集成测试

> **框架**: 仓库现有 suite 为 **vite-plus**（`plugins/dev-team/bin`）。集成测试放在 `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts`，扩展名 `.test.ts`，与 `test_detect_frameworks` 的 plan 一致。对根构建脚本与产物布局的断言通过读取仓库根相对路径（或 `execFile` 调用 `node scripts/build-plugins.mjs`）完成；真实 `vp pack` 可在冒烟场景执行，或在临时 fixture 中 mock 子进程后只验证组装与清单写入。

### `package.json version → 双产物 plugin.json` → `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/package.json` | 权威 version 提供方 |
| `scripts/build-plugins.mjs` | 读取 version 并注入双边 manifest |
| `claude-plugins/dev-team/.claude-plugin/plugin.json` | Claude 产物 version 接收方 |
| `cursor-plugins/dev-team/.cursor-plugin/plugin.json` | Cursor 产物 version 接收方 |

**关联AC**: AC-2, AC-4, AC-5

**关系描述**:

根构建从插件源码根 `package.json` 读取单一权威 `version`，写入 Claude 与 Cursor 各自平台 `plugin.json`。该链路是版本回退风险的核心：若误保留旧 `bin/package.json` 的 `1.0.0`，或只写一侧产物，将导致对外版本不一致。集成测试验证布局迁移后的权威文件位置，以及一次成功构建后两边产物 `version` 与源完全一致。

#### 场景: 版本权威上移布局

验证迁移后源码布局：存在 `plugins/dev-team/package.json`，不存在 `plugins/dev-team/bin/package.json`，且源 `version` 为 `2.10.3`（或约定的下一 semver）而非 `1.0.0`。前置条件为变更实现已完成文件上移。输入为仓库当前文件树；预期上述路径与字段断言全部成立。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `plugins/dev-team/package.json` 存在且可 JSON.parse | 新增 |
| 正向 | 解析后 `version` 等于 `2.10.3`（或约定下一 semver） | 新增 |
| 异常 | `plugins/dev-team/bin/package.json` 不存在 | 新增 |
| 边界 | 源 `version` 不等于 `1.0.0` | 新增 |

#### 场景: 构建注入双边相同 version

验证在源 `version` 为 `2.10.3` 时运行根构建（或对已构建产物做只读断言）后，Claude 与 Cursor 的 `plugin.json` 均存在且 `version` 同为源值，`name` 均为 `"dev-team"`。前置条件为根脚本可执行且产物目录可写；输入为 `node scripts/build-plugins.mjs`；预期两边 manifest 版本一致。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `claude-plugins/dev-team/.claude-plugin/plugin.json` 存在且 `version` 等于源 `package.json` 的 `version` | 新增 |
| 正向 | `cursor-plugins/dev-team/.cursor-plugin/plugin.json` 存在且 `version` 等于源 `package.json` 的 `version` | 新增 |
| 正向 | 两边 `name` 均为 `"dev-team"` | 新增 |
| 边界 | 两边 `version` 字段彼此相等 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `vp pack` 子进程 | 冒烟可用真实 `vp pack`；若需加速，可在隔离临时仓库 fixture 中 stub `vp` 为 exit 0 空实现，仅验证 version 注入与文件布局 | 构建注入双边相同 version |

---

### `根构建脚本 → vp pack → 双产物组装` → `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `scripts/build-plugins.mjs` | 编排触发方 |
| `plugins/dev-team/`（含 `vite.config.ts` / `vp pack`） | JS 打包执行方 |
| `claude-plugins/dev-team/` | Claude 产物接收方 |
| `cursor-plugins/dev-team/` | Cursor 产物接收方 |

**关联AC**: AC-1, AC-3, AC-9

**关系描述**:

根脚本在 `plugins/dev-team` 调用 `vp pack`，再将可安装内容同步到双产物目录，且不得污染源码树平台 manifest。出错模式包括：cwd 仍停在 `bin/` 导致 pack 失败、引入第二套 bundler、组装残留旧文件、或在源码下误写 `.claude-plugin`。集成测试覆盖入口存在性、成功构建后的产物树与源码净化。

#### 场景: 根构建入口存在且刷新双产物

验证仓库根存在 `scripts/build-plugins.mjs`（或约定的 `scripts/build-plugins.*`），执行成功后 `claude-plugins/dev-team` 与 `cursor-plugins/dev-team` 存在，并包含打包后的运行时 JS（如 `bin/dev-team-mcp.cjs` 等）。前置条件为依赖与 vite-plus 可用；输入为在仓库根执行 `node scripts/build-plugins.mjs`；预期 exit code 0 且双产物刷新。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `scripts/build-plugins.mjs`（或等价 `scripts/build-plugins.*`）文件存在 | 新增 |
| 正向 | 根构建 exit code 为 `0` | 新增 |
| 正向 | 构建后 `claude-plugins/dev-team` 与 `cursor-plugins/dev-team` 目录存在 | 新增 |
| 正向 | 两边产物含 `bin/dev-team-mcp.cjs`（及 cli/hooks 等价产物） | 新增 |
| 异常 | 故意使 `vp pack` 失败时根脚本以非零码退出 | 新增 |
| 边界 | 构建脚本源码/调用轨迹表明打包命令为 `vp pack`，未调用 webpack/esbuild/rollup/tsup 作为替代打包入口 | 新增 |

#### 场景: 构建后源码无平台 manifest

验证成功构建后 `plugins/dev-team` 下不存在 `.claude-plugin/` 或 `.cursor-plugin/`。前置条件为刚完成一次成功（或 dry-run 组装）构建；输入为源码目录列表；预期两平台目录均不存在。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `plugins/dev-team/.claude-plugin` 不存在 | 新增 |
| 正向 | `plugins/dev-team/.cursor-plugin` 不存在 | 新增 |

#### 场景: 双产物最小可装树同构

验证两边产物均含安装所需内容（平台 manifest、`hooks/`、`skills/`、`agents/`、`bin` 运行时文件等），且本迭代允许 skills/agents/hooks 内容同构（抽样文件内容一致即可）。前置条件为构建成功；输入为两边目录树；预期关键路径存在且抽样同构。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | Claude 产物含 `.claude-plugin/plugin.json`、`hooks/`、`skills/`、`agents/`、运行时 `bin/dev-team-*.cjs` | 新增 |
| 正向 | Cursor 产物含 `.cursor-plugin/plugin.json` 及同等内容目录 | 新增 |
| 正向 | 抽样 `hooks/hooks.json`（或约定代表性文件）在两边内容一致（同构） | 新增 |
| 边界 | 产物 `bin/` 不含 `src/`、`__tests__/`、`.map`、`node_modules` | 新增 |
| 边界 | 构建前在产物放入 `stale.txt` 后再次构建，陈旧文件消失 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `vp pack` / PATH 上的 `vp` | 完整冒烟用真实命令；失败路径可临时将 `PATH` 指向只含失败 stub 的目录 | 根构建入口存在且刷新双产物 |
| 仓库工作区 | 优先在临时 clone/fixture 上跑组装，避免污染开发者未提交改动；若测真实仓库产物则限定断言只读路径 | 双产物最小可装树同构 |

---

### `marketplace source → 产物目录` → `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `.claude-plugin/marketplace.json` | Claude marketplace 配置方 |
| `.cursor-plugin/marketplace.json` | Cursor marketplace 配置方 |
| `claude-plugins/dev-team/` | Claude `source` 目标 |
| `cursor-plugins/dev-team/` | Cursor `source` 目标 |

**关联AC**: AC-6, AC-9

**关系描述**:

手写双 marketplace 必须指向产物目录而非 `plugins/` 源码树，否则宿主会直接安装未净化源码。集成测试读取两边清单的 `plugins[].source`，并确认目标目录存在且与对应产物 manifest 的 `name`/`description` 可对齐，保证最小可装路径闭环。

#### 场景: 双 marketplace 指向产物

验证 `.claude-plugin/marketplace.json` 中 `dev-team` 的 `source` 为 `./claude-plugins/dev-team`，`.cursor-plugin/marketplace.json` 中 `source` 为 `./cursor-plugins/dev-team`；两边均注册 `name: "dev-team"`；`description` 与各自产物 `plugin.json` 对齐。前置条件为清单与产物已落地；输入为 JSON 文件；预期字段与路径断言通过。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | Claude marketplace 存在且 JSON 合法 | 新增 |
| 正向 | Cursor marketplace 存在且 JSON 合法 | 新增 |
| 正向 | Claude 侧 `dev-team.source === "./claude-plugins/dev-team"` | 新增 |
| 正向 | Cursor 侧 `dev-team.source === "./cursor-plugins/dev-team"` | 新增 |
| 正向 | 两边 marketplace 的 `name`/`description` 与对应产物 `plugin.json` 一致 | 新增 |
| 异常 | Claude marketplace 的 `source` 不得仍为 `./plugins/dev-team` | 新增 |
| 边界 | marketplace 中 `source` 指向的目录在仓库内真实存在 | 新增 |

##### Mock策略

<!-- 纯文件读断言，无跨进程 Mock 需求 -->

---

### `gitignore 例外 → Cursor 清单与产物可跟踪` → `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `.gitignore` | ignore 规则提供方 |
| `.cursor-plugin/marketplace.json` | 需被放行的点目录内容 |
| `claude-plugins/` / `cursor-plugins/` | 需可 commit 的产物目录 |

**关联AC**: AC-7

**关系描述**:

仓库已有 `.*` 忽略规则会吞掉 `.cursor-plugin`；若未显式 `!.cursor-plugin`，Cursor marketplace 无法入库。同时产物目录不得被 ignore。集成测试检查 `.gitignore` 文本并（在 git 可用时）用 `git check-ignore` 验证关键路径不被忽略。

#### 场景: gitignore 放行 Cursor 清单与产物

验证 `.gitignore` 含 `!.cursor-plugin`；`claude-plugins/`、`cursor-plugins/`、`.cursor-plugin/marketplace.json` 不被 ignore。前置条件为实现已更新 `.gitignore`；输入为 ignore 文件与 `git check-ignore -v`（若环境有 git）；预期例外生效且产物可跟踪。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `.gitignore` 文本包含 `!.cursor-plugin` | 新增 |
| 正向 | `git check-ignore -q .cursor-plugin/marketplace.json` 退出非 0（表示未被 ignore），在 git 可用时 | 新增 |
| 正向 | `git check-ignore -q claude-plugins/dev-team` 与 `cursor-plugins/dev-team` 均显示未被 ignore | 新增 |
| 边界 | `.gitignore` 中不存在单独忽略 `claude-plugins/` 或 `cursor-plugins/` 的规则 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `git` CLI | 通过 `execFile('git', ['check-ignore', …])` 查询；若无 git 则跳过 check-ignore 用例并仅做文本断言 | gitignore 放行 Cursor 清单与产物 |

---

### `CLAUDE.md 升版规则 → package.json 权威` → `plugins/dev-team/bin/__tests__/dual-platform-build/dual-platform-build.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `CLAUDE.md` | 文档规则陈述方 |
| `plugins/<name>/package.json` | 文档所指向的版本权威 |

**关联AC**: AC-8

**关系描述**:

文档若仍要求升 `.claude-plugin/plugin.json`，贡献者会继续改错位置。集成测试对 `CLAUDE.md` 做关键字/路径断言，确认升版规则指向 `plugins/<name>/package.json` 并要求重建双产物。

#### 场景: CLAUDE.md 升版规则文案

验证 `CLAUDE.md` 写明改插件代码后升级 `plugins/<name>/package.json`（或等价表述）并重建双产物（提及根构建 / `claude-plugins` / `cursor-plugins` 之一即可）。前置条件为文档已更新；输入为文件文本；预期不再将源码树内 `.claude-plugin/plugin.json` 作为升版权威。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `CLAUDE.md` 包含 `plugins/<name>/package.json` 或 `plugins/` + `package.json` 升版指引 | 新增 |
| 正向 | `CLAUDE.md` 提及重建双产物或根构建脚本（`build-plugins` / `claude-plugins` / `cursor-plugins`） | 新增 |
| 异常 | 升版规则不得仍仅指向 `<plugin_name>/.claude-plugin/plugin.json` 作为唯一权威 | 新增 |

##### Mock策略

<!-- 纯文件读断言，无跨进程 Mock 需求 -->

---

## 不可测试项

- `plugins/dev-team/package.json`、`.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json`、`.gitignore`、`CLAUDE.md` — **原因**: `test_resolve_paths` 将 JSON/配置/文档标为 `Not a testable source file`（对目录 `plugins/dev-team/`、`scripts/` 同理）。其正确性通过集成测试的文件读断言覆盖，不单独建立单元测试章节。
- AC-9 真实宿主安装 — **原因**: 在 Claude Code / Cursor 内实际点击安装并验证 MCP/hooks 运行依赖本机宿主与人工操作；自动化仅验证 marketplace 路径与产物树最小可装内容。工具名/hooks 反参差异明确不在本迭代范围。
- Cursor 专用 `mcp.json` 是否首期写入 — **原因**: design 待决项；本迭代不强制，无稳定验收断言可绑定。
- 强制「改源后必须跑根构建再 commit」的 CI 闸门 — **原因**: proposal/design 明确本迭代不强制 CI；属后续开放项。
- `scripts/build-plugins.test.mjs` 与 `plugins/dev-team/vite.config.test.ts` 纳入现有 `vp test` suite — **原因**: 当前 `openspec/config.json` 的 `tests[].root` 为 `plugins/dev-team/bin` 且 `includes` 为 `src/**/*.{ts,tsx}`，`test_detect_frameworks` 对上述文件返回 `unknown`。测试生成后需扩展 suite 或改用 `node --test`/临时配置才能在 CI 中执行；路径映射本身以 `test_resolve_paths` 为准。
- 既有 `plugins/dev-team/bin` 业务单测行为逻辑 — **原因**: proposal「不要修改」业务 MCP/CLI/hooks；若仅因 `package.json`/vite 工作目录上移导致个别测试的路径假设断裂，属实现阶段回归修复，不在本变更新增用例范围内（除非发现硬编码 `bin/package.json` 的断言需同步更新）。
