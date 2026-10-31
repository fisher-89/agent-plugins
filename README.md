# WPS AI 桌面应用（desktop）

本仓库是 WPS AI 桌面应用（`packages/desktop`）的**唯一源码仓**：Tauri 2 后端 + React 前端，
承载 change 工作流的知识（相位角色 prompt）与相位编排。仓库内只有一个可分发包，发布走
GitHub Releases + 应用内自动更新（Windows 全链路；macOS 手动装 dmg）。

## 安装

从 [Releases](https://github.com/fisher-89/agent-plugins/releases) 下载对应平台安装包：

| 平台 | 产物 | 说明 |
|------|------|------|
| Windows | `dev-team_<version>_x64-setup.exe` | NSIS 安装版，安装后启动即走自动更新链路（首次安装会有 SmartScreen 提示，属无 Authenticode 签名的预期行为） |
| macOS（Apple Silicon） | `dev-team_<version>_aarch64.dmg` | 打开 dmg 把 App 拖进「应用程序」；首次启动被 Gatekeeper 拦下时，在访达中 **右键 App 图标 → 「打开」→ 再点「打开」** 放行 |

安装包形态、签名密钥与更新链路的细节见 [`packages/desktop/RELEASE.md`](packages/desktop/RELEASE.md)。

## 开发

前置：Node.js + pnpm、Rust 工具链（`rustup`），以及平台对应的 Tauri 依赖
（Windows：WebView2 + MSVC 构建工具；macOS：Xcode Command Line Tools）。

```bash
pnpm -C packages/desktop install      # 安装前端依赖
pnpm -C packages/desktop run dev      # tauri dev：前端 + Rust 全链（含热重载）
pnpm -C packages/desktop run client:dev   # 仅前端单跑（不启动 Rust 侧）
```

常用门禁（提交前跑一遍）：

```bash
pnpm -C packages/desktop run check    # cargo fmt + clippy + bindings 对拍 + eslint/prettier + knip
pnpm -C packages/desktop run test     # 前端 vp test（shuffle）+ cargo test --workspace
```

## 构建

```bash
pnpm -C packages/desktop run build
```

`tauri.conf.json` 启用了 updater 工件（`createUpdaterArtifacts` + `pubkey`），因此**本地打包必须先设
两个签名环境变量**，否则构建报错或在打包后卡住等 stdin 输入：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\desktop-terminal.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
pnpm -C packages/desktop run build
```

（macOS 上把 `$env:` 换成 `export`；`tauri dev` 不打包，不受影响。密钥生成、公钥落位与 CI secrets
配置见 [`packages/desktop/RELEASE.md`](packages/desktop/RELEASE.md)「一次性准备」与「本地构建须知」。）

## 发版

发版全程由 `.github/workflows/desktop-release.yml` 承接，人只做两步：

1. **改版本号**：权威是 `packages/desktop/package.json` 的 `version`，每次桌面源码变更 **patch +1**
   （`src-tauri/tauri.conf.json` 以 `"version": "../package.json"` 自动跟随；`src-tauri/Cargo.toml`
   的版本是 crate 自身版本线，MUST NOT 随应用版本 bump）。
2. **`git push origin master`**：workflow 的 detect 步比对 package.json 版本与仓库最新 release 版本，
   更高则自动在该 commit 上打 tag，双平台并行构建，finalize 合并 `latest.json` 后发布到 Releases。
   版本未升高（未 bump / 回退）则 build 与 finalize 均 skipped——**不 bump 就不会发版**。

重发同版本走 Actions → desktop-release → Run workflow → 勾选 `force`。**绝不 `git push --tags`**：
workflow 不监听 tag 推送，手动打 tag 无发版效果，全量推 tag 还会污染远端仓库。

## 仓库结构

```
packages/desktop/                  唯一可分发包
├── src/                           React 前端（视图 / 组件 / hooks / 生成类型）
└── src-tauri/
    ├── src/                       Tauri 应用壳（commands / bindings / 入口）
    └── crates/
        ├── core/                  领域核心：foundation / config / workflow / orchestration /
        │                          agent / checks（相位表、角色 prompt、报告语义）
        ├── infra/                 基础设施适配：agent / checks / exec / store / vcs / watch
        └── tools/export-bindings  前端类型生成（bindings:export）
openspec/
├── specs/                         存活能力规格（只放仍在维护的能力）
├── changes/                       进行中的 change（archive/ 为历史存档）
└── explores/                      探索笔记
```

## 项目配置（`openspec/config.json`）

仓库级配置（测试套件、静态分析命令、写保护）在 `openspec/config.json`，schema 在
`packages/desktop/src-tauri/crates/core/config/config.schema.json`，文件顶部 `$schema` 指其 raw URL
即可获得编辑器补全：

```json
{
  "$schema": "https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json"
}
```

| 字段 | 作用 | 本仓取值 |
|------|------|----------|
| `tests` | 测试套件：root / 框架 / 范围 / 覆盖率与变异门槛 | 两个 suite：`packages/desktop`（`vite-plus`）、`packages/desktop/src-tauri`（`rust`） |
| `static_analysis` | 静态检查命令 | `pnpm run -C ./packages/desktop check` |
| `write_protection` | 禁止 Agent 改写的文件（glob） | `packages/desktop/knip.json`（另内置锁定 `openspec/config.json`） |

## 历史

插件层（早期为 Claude Code / Cursor 发布的 workflow 插件）已退役：其知识与编排由桌面端原生承载，
源码、交付产物与对外市场均已删除；历史变更与决策见 `openspec/changes/archive/`。
