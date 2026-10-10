# desktop-release-pipeline Specification

## ADDED Requirements

### Requirement: 发版流水线三段结构

desktop 发版流水线 SHALL 由三段 job 组成，并 SHALL 在同一 workflow 内闭合（GITHUB_TOKEN 推的 tag 不触发其他 workflow，检测与发布 MUST 同 workflow）：

1. **detect**（ubuntu）：读取 `packages/desktop/package.json` 的 `version`，经 GitHub API 与仓库最新非 draft / 非 prerelease release 的版本比对（首发无 release 时 API 404 归空、版本按 0.0.0 计），更高则 `should=true`；`workflow_dispatch` 的 `force` 输入 SHALL 跳过版本比较强制发版。**tag `desktop-v<version>` 的创建 SHALL 前移至 detect**（版本比较之后、build 之前），已存在则跳过创建。
2. **build**（matrix）：以 detect 的 `version` 为输入，`windows-latest` 与 `macos-latest` 双腿并行构建（`if: needs.detect.outputs.should == 'true'` 由上游 gate）。
3. **finalize**（ubuntu）：`latest.json` 权威归属方（见「latest.json 权威合并」）。

release 创建与产物上传 SHALL 经 tauri-action 发布到 `fisher-89/agent-plugins` 的 Releases；`releaseDraft: false`（draft 会使 `releases/latest/download/latest.json` 端点 404）、`prerelease: false` SHALL 保持。

#### Scenario: 版本更高自动发版

- **WHEN** master 推送触及 `packages/desktop/**` 且 package.json 版本高于仓库最新 release 版本
- **THEN** detect `should=true`，tag `desktop-v<version>` 建于该 commit，build 双腿并行构建，finalize 上传权威 latest.json

#### Scenario: 版本不高跳过构建

- **WHEN** package.json 版本不高（未升版 / 回退）且非 force 触发
- **THEN** detect `should=false`，build 与 finalize 均跳过（Actions 表现为 release skipped），不建 tag、不发版

#### Scenario: force 跳过版本比较

- **WHEN** `workflow_dispatch` 以 `force=true` 触发
- **THEN** 跳过版本比较按 package.json 版本强制发版；tag 已存在则跳过创建（补跑 / 重发同版本）

#### Scenario: tag 前移单点创建

- **WHEN** 一次发版中 tag 尚不存在
- **THEN** tag `desktop-v<version>` 在 detect 内创建并推送（build 双腿开始前已在远端）；双腿并行时无 tag 创建竞态

### Requirement: 双平台产物矩阵

build matrix SHALL 产出双平台应用：

- **windows-latest**：`--bundles nsis`，产物 `*_x64-setup.exe` + `.sig`（updater 签名工件）。
- **macos-latest**：`--bundles dmg`，产物 `*_aarch64.dmg`；aarch64（Apple Silicon 原生）SHALL 为唯一 mac 架构，MUST NOT 构建 universal / x86_64。

版本 SHALL 单一来源自 `packages/desktop/package.json`（`tauri.conf.json` `version: "../package.json"` + workflow 读 package.json 打 tag），双腿 SHALL 自动同版本（matrix 单版本源，无独立 mac 版本）。`tauri.conf.json` 的 `bundle.targets: "all"` SHALL 保持（仅影响本地构建，CI 经 `--bundles` args 覆盖）、`createUpdaterArtifacts: true` SHALL 保持。

#### Scenario: Windows 腿产物

- **WHEN** Windows 腿构建成功
- **THEN** release 含 `*_x64-setup.exe` 与 `.sig`，`latest.json` 的 `windows-x86_64` 条目由该腿产出（签名形态与现状一致）

#### Scenario: macOS 腿产物

- **WHEN** macOS 腿构建成功
- **THEN** release 含 `*_aarch64.dmg`，且 updater 工件（`.app.tar.gz` + `.sig`）随 `createUpdaterArtifacts` 自动产出；不产出 universal / x86_64 产物

#### Scenario: 双腿同版本

- **WHEN** 一次发版的双腿均成功
- **THEN** Windows 与 macOS 产物版本逐字一致，均等于 package.json 版本

### Requirement: latest.json 权威合并

`finalize`（ubuntu）SHALL 为 `latest.json` 的**唯一权威归属方**：合并双腿产出的 `platforms` 条目后上传权威版本，`releases/latest/download/latest.json` SHALL 同时含 `windows-x86_64` 与 `darwin-aarch64` 两条目。合并 SHALL：

- **幂等**：同版本条目覆盖而非重复（force 重发同版本不产生重复条目）；
- **容忍缺失平台**：某腿失败（如 mac 腿失败）时 SHALL 合并现存平台条目、照常上传，MUST NOT 因缺腿失败（首发组合空窗时 release 仅含 Windows 产物，Windows 条目 SHALL 仍在且形态不变）；
- **回归红线**：`windows-x86_64` 条目的签名与字段形态 SHALL 与单平台现状一致（Windows 更新链路不可坏）。

若实现期验证 tauri-action 对 matrix + updater 场景已内置 latest.json 合并，`finalize` SHALL 退化为「校验权威 latest.json 含双平台条目」（权威归属语义不变，仅实现载体不同）。

#### Scenario: 双腿成功合并双平台

- **WHEN** Windows 与 macOS 双腿均成功，finalize 运行
- **THEN** 上传的 `latest.json` 含 `windows-x86_64` 与 `darwin-aarch64` 两条目，版本一致

#### Scenario: 缺腿容忍

- **WHEN** mac 腿失败（或未产出），finalize 运行
- **THEN** `latest.json` 含现存平台（`windows-x86_64`）条目并照常上传，不报错；Windows 条目签名与形态不变

#### Scenario: 重发幂等

- **WHEN** force 重发同版本（双腿再次产出同版本条目），finalize 运行
- **THEN** `latest.json` 每平台恰一条目（同版本覆盖），无重复条目

### Requirement: 更新工件与签名

更新链路签名 SHALL 保持：Windows 腿以 `TAURI_SIGNING_PRIVATE_KEY`（空密码，`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 定义为空）对 NSIS 安装包做 minisign 签名（pubkey 编译进应用，`tauri.conf.json` `plugins.updater.pubkey` 不变）。macOS 腿 SHALL NOT 配置任何 `APPLE_*` 签名环境变量：mac 版无 OS 级代码签名（Apple Silicon 二进制天然带链接器 ad-hoc 签名，可运行）；mac updater 工件（`.app.tar.gz` + `.sig`）SHALL 照常产出（仅无 OS 级签名），若 tauri 对无签名 env 的 mac updater 报错，按实现期验证结果调整（不改变「无 Apple 账号、无 OS 级签名」的前提）。

#### Scenario: Windows 签名不变

- **WHEN** Windows 腿构建
- **THEN** 以 `TAURI_SIGNING_PRIVATE_KEY` 签名，`latest.json` 的 `windows-x86_64` 条目 signature 由同一 pubkey 校验通过（与现状同形态）

#### Scenario: mac 无 OS 级签名产出

- **WHEN** macOS 腿在无任何 `APPLE_*` env 下构建
- **THEN** dmg 与 updater 工件（`.app.tar.gz` + `.sig`）产出，二进制带链接器 ad-hoc 签名可运行；不引入 `APPLE_*` secrets

### Requirement: 图标成套

`src-tauri/icons/` SHALL 含 Windows 与 macOS 打包所需全套图标：`icon.ico`、`icon.icns` 与多尺寸 `png`。图标 SHALL 自程序生成的纯色 1024×1024 PNG 源经 `pnpm tauri icon` 生成（无原始设计稿的落位手段；后续有设计稿仅替换源图重跑）。`tauri.conf.json` 的 `bundle.icon` SHALL 列出 ico / icns / png 全套（MUST NOT 仅剩 `icon.ico`）。

#### Scenario: 全套图标落位

- **WHEN** 检查 `src-tauri/icons/` 与 `tauri.conf.json`
- **THEN** 目录含 `icon.ico` / `icon.icns` 与多尺寸 png；`bundle.icon` 列出 icns 与 png（ico 保留），dmg/app 打包时 icns 生效

#### Scenario: 源图可再生成

- **WHEN** 以纯色源 PNG 重跑 `pnpm tauri icon`
- **THEN** 生成的全套图标与已检入的一致（确定性）；后续替换源图仅需重跑该命令

### Requirement: RELEASE.md 双平台文档

RELEASE.md SHALL 更新为双平台分发说明：mac 首次安装 SHALL 记录 Gatekeeper 右键打开（无 OS 级签名的预期行为）；mac 权威升级路径 SHALL 为手动下载 dmg（updater 尽力而为、不承诺自动更新）。Windows 更新链路说明（minisign 校验 / NSIS `/UPDATE` 静默安装 / 密钥管理）SHALL 保持。

#### Scenario: mac 安装与升级说明在案

- **WHEN** 查阅 RELEASE.md
- **THEN** 含 mac 首次安装右键打开说明与手动升级路径；Windows 更新链路说明未丢失

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.29` 升级为 `0.4.30`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 2.10.44 与三类交付产物）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.30，`plugins/dev-team/package.json` version 仍为 2.10.44

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.github/workflows/desktop-release.yml` | 发版流水线载体 | detect（版本比较 + tag 前移）→ build matrix（win nsis / mac dmg）→ finalize（latest.json 权威合并）；单 workflow 闭合；`permissions: contents: write` 与 `concurrency: group desktop-release` 保持 |
| `detect` job | 版本 gate + tag 创建 | 读 package.json 版本 → 与 `releases/latest` tag 比对（`desktop-v` 前缀剥离 + 格式过滤）→ `should`；`force` 跳过比较；tag 已存在跳过创建 |
| `build` job（matrix） | 双平台产物 | `windows-latest --bundles nsis` / `macos-latest --bundles dmg`；tauri-action `tagName` / `releaseName` / `projectPath` 语义不变；签名 env 按腿分流 |
| `finalize` job | latest.json 权威归属 | 合并双腿 `platforms`（幂等覆盖 / 容忍缺腿）→ 上传权威版本；tauri-action 内置合并时退化为校验 |
| `src-tauri/tauri.conf.json` | 打包配置 | `bundle.icon` 补 icns/png；`version` / `targets` / `createUpdaterArtifacts` / `plugins.updater` 不变 |
| `src-tauri/icons/` | 图标资产 | 纯色源 PNG → `pnpm tauri icon` 生成 ico/icns/png 全套 |
| `RELEASE.md` | 双平台发版文档 | mac 右键打开 + 手动升级路径；Windows 更新链路说明保持 |
