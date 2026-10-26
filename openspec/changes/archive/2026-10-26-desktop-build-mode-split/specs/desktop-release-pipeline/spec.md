# desktop-release-pipeline Specification（delta: desktop-build-mode-split）

## ADDED Requirements

### Requirement: CI 构建恒生产模式（不经 pnpm 脚本）

desktop 发版流水线的 build matrix SHALL 恒产**生产模式**产物，且该性质 MUST NOT 依赖任何脚本默认值：

- build 腿 SHALL 经 tauri-action 直接驱动 tauri CLI（`tauri build`，`args` 仅 `--bundles …`），MUST NOT 改为调用 `pnpm build` / `pnpm run build` / 任何仓内 npm 脚本——`packages/desktop/package.json` 的 `build` 已倒置为 **dev**（`--debug --no-bundle`），改走脚本会让 CI 悄悄产出 dev 构建；
- workflow MUST NOT 出现 `--debug`、`--no-bundle`、`CARGO_TARGET_DIR`、`DEV_TEAM_DATA_ROOT` 任何一项（dev 侧参数与验收用环境变量 MUST NOT 进入发版链路）；
- CI 产物 SHALL 落 prod 侧全部取值：cargo release profile、单实例插件注册、数据根 `<home>/.dev-team`、`--self-check` 不可用（prod 侧拒绝）；判别源与取值的定义见 desktop-build-modes「构建模式判别源单点」与「数据根三分优先级与路径校验」；
- `build.beforeBuildCommand` SHALL 保持 `pnpm client:build`（仅前端构建），MUST NOT 被替换为含 server 构建的整链脚本；
- 发版侧的签名前置（`TAURI_SIGNING_PRIVATE_KEY` + 空密码）MUST NOT 因本变更松动——`--debug --no-bundle` 免私钥的便利**只属本地 dev 构建**。

#### Scenario: CI 产物落生产侧

- **WHEN** 一次发版由 build matrix 双腿构建完成并安装产物
- **THEN** 产物为 release profile、单实例注册生效、数据根为 `<home>/.dev-team`；带 `--self-check` 启动会被拒绝（退出码 2），不出现 `DEV` 徽标

#### Scenario: 脚本默认值不得渗入 CI

- **WHEN** 审阅 `.github/workflows/desktop-release.yml` 的 build 腿
- **THEN** 构建入口为 tauri-action（未改为 `pnpm build`），`args` 无 `--debug` / `--no-bundle`，env 段无 `CARGO_TARGET_DIR` / `DEV_TEAM_DATA_ROOT`

#### Scenario: 签名前置不削弱

- **WHEN** Windows 腿构建（私钥 secret 未配置或失效）
- **THEN** 构建如现状失败；配齐私钥后产出带签名的 NSIS 安装包与 `.sig`（与既有更新链路同形态）

## Module Contract

| 模块 | 面 | 关键契约 |
|---|---|---|
| `.github/workflows/desktop-release.yml`（build matrix） | CLI | 经 tauri-action 直调 `tauri build`；`args` 仅 bundles 项；零 `pnpm build`、零 dev 侧参数、零 `DEV_TEAM_DATA_ROOT`；本变更对该文件零改动 |
| `packages/desktop/package.json`（`build:prod` / `server:build`） | CLI | 生产构建的唯一脚本入口（bundle + 签名 + release profile）；CI 不消费但与之同语义，避免两条产线口径分叉 |
