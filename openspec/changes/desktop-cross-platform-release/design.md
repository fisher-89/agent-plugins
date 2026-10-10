# 设计: desktop-cross-platform-release

> **变更**: desktop-cross-platform-release
> **日期**: 2026-10-10

---

## 提案与规格同步状态

`proposal.md` 与 `openspec/changes/desktop-cross-platform-release/specs/desktop-release-pipeline/spec.md`（7 个 ADDED requirement）已由提案阶段定稿，explore 决策（用户面 / Apple 签名 / 验收线 / 版本号 / mac 架构 / latest.json 归属 / 图标源 / mac 更新口径 / tag 前移 / 版本交付）用户拍板在案；本设计不重复其内容、不将其列为待办，只在其五项「待决问题（实现期验证）」之上定稿（见「关键设计决策」）。

两点实现期基准勘误注记：

1. **RELEASE.md 产物名残留**：现有 RELEASE.md「每次发版」核对清单写的是 `desktop-terminal_X.Y.Z_x64-setup.exe`，但 `tauri.conf.json` 的 `productName` 为 `dev-team`（`src-tauri/Cargo.toml` 包名亦为 `dev-team`），tauri 实际 NSIS 产物名为 `dev-team_<version>_x64-setup.exe`。本变更重写该产物清单段时一并校正为 `dev-team_<version>_x64-setup.exe`（不扩及文档其余历史措辞），避免双平台清单复制错误产物名。
2. **现状图标几何**：`src-tauri/icons/icon.ico`（32×32，BGRA）实测为暗底 `#1f2430` + 居中正方 `#2563eb`（75% 边长 inset），非字面单色。图标源决策见 D6——仍遵守「纯色」定稿（单色 `#2563eb`），R7 已拍板接受观感变化。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `detect` job（ubuntu） | 版本 gate + tag 前移单点创建：读 `package.json` 版本与 `releases/latest` tag 比对（`desktop-v` 前缀剥离 + 格式过滤）→ `should`；`force` 跳过比较；`desktop-v<version>` 已存在则跳过创建 | `.github/workflows/desktop-release.yml` | GitHub API（`gh`）、`node` | bash + node 内联脚本 |
| `build` job（matrix） | 双平台产物矩阵：`windows-latest --bundles nsis` / `macos-latest --bundles dmg`，tauri-action 发布到 `fisher-89/agent-plugins` Releases；每腿捕获自身 `latest.json` 为独立工件 | `.github/workflows/desktop-release.yml` | tauri-action、pnpm、Rust toolchain | GitHub Actions matrix + tauri-action@v0 |
| `finalize` job（ubuntu） | `latest.json` 权威归属方：下载双腿工件 → 以 Windows 为 base 键合并 `platforms`（幂等 / 容忍缺腿）→ `gh release upload --clobber` 覆盖为权威版本 | `.github/workflows/desktop-release.yml` | `actions/download-artifact`、`gh`、`node` | bash + node 内联合并脚本 |
| 打包配置 | `bundle.icon` 补 icns / 多尺寸 png（`version` / `targets: "all"` / `createUpdaterArtifacts` / `plugins.updater` 不变） | `packages/desktop/src-tauri/tauri.conf.json` | Tauri v2 bundler | JSON |
| 图标资产 | 纯色源 PNG + `pnpm tauri icon` 生成的全套（ico / icns / png） | `packages/desktop/src-tauri/icons/` | `@tauri-apps/cli`（`tauri icon`） | PNG / ICO / ICNS 二进制 |
| 发版文档 | 双平台分发 + mac 首次安装右键打开 + mac 手动升级路径；Windows 更新链路说明保持 | `packages/desktop/RELEASE.md` | 无 | Markdown |
| 版本权威 | `version` 0.4.29 → 0.4.30（单一版本源，`tauri.conf.json` 经 `../package.json` 自动跟随） | `packages/desktop/package.json` | 无 | semver |

### 工作流三段结构（目标形态）

```
detect (ubuntu)
  ├─ 版本比较(不变,输出 version / should)
  └─ 创建 tag desktop-v<version> ◀── 自 release job 前移(已存在跳过)
        ▼
build (matrix, if: needs.detect.outputs.should == 'true')
  ├─ windows-latest → tauri-action(--bundles nsis) → 捕获 latest.json 工件
  └─ macos-latest   → tauri-action(--bundles dmg)  → 捕获 latest.json 工件
        ▼
finalize (ubuntu, if: should=='true' && (success() || failure()))
  下载双腿工件 → node 合并 platforms → gh release upload --clobber 权威 latest.json
```

矩阵双腿 `if` 由上游 `detect.outputs.should` 单点 gate（job 级 `if` 一行）；`finalize` 的 `(success() || failure())` 使单腿失败（mac 腿失败）仍照常合并现存平台，MUST NOT 因缺腿跳过。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | finalize 合并 latest.json 的实现载体 | **workflow 内联 node 脚本**（heredoc，不新增仓库脚本文件、不依赖 tauri-action 内置合并）。先实现期验证 tauri-action 是否已内置合并：内置则 finalize 退化为「校验双平台条目存在」，未内置（预期）则自行合并 | 变更面守「改 2 文件 + 补图标 + 改文档、0 行应用代码」；detect 已用 node 内联先例，ubuntu 自带 node，零新依赖；不新增仓库脚本避免打破提案实现文件清单 |
| D2 | 双腿 latest.json 如何不被互覆盖丢失 | 双腿各自经 `actions/upload-artifact@v4` 上传**自身** `latest.json`（工件名 `latest-json-windows` / `latest-json-macos`，`if: success()`），finalize 经 `actions/download-artifact@v4` 取回。**不能**从 release 资产取回：双腿都上传同名 `latest.json` 资产，后结束的腿覆盖先者，finalize 时只能看到一份 | tauri 生成的 `latest.json` 是 `signature` / `url` 的唯一权威（签名逐字不可重构）；工件通道在 release 资产覆盖前保住双腿版本，且免去解析安装包命名 |
| D3 | 合并语义与幂等 / 缺腿 | **以 Windows `latest.json` 为 base**：`merged = { ...base }`，`merged.platforms = { ...(mac?.platforms ?? {}), ...base.platforms }`；校验双腿 `version` 逐字一致（缺腿不校验）。幂等天然成立：`platforms` 是键控 map（`windows-x86_64` / `darwin-aarch64`），同版本重发只覆盖键值、不可能出现重复条目。缺腿容忍：mac 工件缺失时 `mac = null`，仅保留 base 的 Windows 条目 | base 取 Windows 保证 `windows-x86_64` 条目的 `signature` / `url` / `notes` 逐字不变（AC-2 回归红线）；键合并 + 覆盖语义同时满足 AC-3 幂等与缺腿 |
| D4 | finalize 触发门与缺腿下载 | `finalize.needs = [detect, build]`，`if: needs.detect.outputs.should == 'true' && (success() \|\| failure())`。Windows 工件下载 step **不** `continue-on-error`（缺即 fail——Windows 是红线）；mac 工件下载 step `continue-on-error: true`（缺腿容忍） | `success() \|\| failure()` = 除取消/跳过外总运行，覆盖「mac 腿失败仍产出 Windows-only latest.json」；Windows 缺失属真失败应显式暴露 |
| D5 | 双腿并行 release 创建竞态 | **主路径：tag 前移至 detect 单点创建**，双腿 tauri-action 经 `getReleaseByTag` 复用既有 release（只上传资产，`releaseName` / `releaseDraft: false` / `prerelease: false` 双腿一致故创建语义确定）。实现期待验证 tauri-action「release 已存在则只上传」行为；若验证发现仍有竞态窗口，**备选** detect 预建 release（`gh release create desktop-v<version> --title "Dev Team v<version>"`，不设 draft/prerelease） | tag 前移消除「双腿各自带 tagName 并发 create」竞态；单点建 tag + 复用语义使 release 只建一次（AC-4） |
| D6 | 图标源颜色 | **1024×1024 纯色 PNG，颜色 `#2563eb`**（取现状 icon 的唯一强调蓝），经内联 node 脚本（zlib PNG 写入器）确定性生成 `icon-source.png`，再 `pnpm tauri icon` 生成 ico / icns / png 全套。**不**复刻现状暗底两色几何 | 遵守 explore 定稿「程序生成纯色 1024×1024 PNG」；`#2563eb` 是现状 icon 唯一有识别度的色、纯蓝方块在 dock / 应用列表可见（暗底 `#1f2430` 会隐形）；观感变化已由 R7 拍板接受，后续有稿仅换源图重跑 |
| D7 | `bundle.icon` 权威列表 | `["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]`（Tauri v2 桌面标准集，ico 保留） | 覆盖 Windows（ico）、macOS（icns）、Linux/回退（png）；与 `tauri icon` 默认产物名逐字对齐（AC-7） |
| D8 | 版本交付 | `packages/desktop/package.json` `version` 0.4.29 → **0.4.30**（spec「版本交付」已定，无漂移）；`plugins/dev-team` 零改动（版本 2.10.44 与三类交付产物） | 用户可见发版面变更沿 0.4.x patch 先例；单版本源使矩阵双腿自动同版本 |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/icons/icon-source.png` | 1024×1024 纯色 `#2563eb` 源 PNG（D6 内联 node 脚本生成，确定性；后续换设计稿仅替换此图重跑 `tauri icon`） |
| `packages/desktop/src-tauri/icons/icon.icns` | macOS 打包所需（`pnpm tauri icon` 自 `icon-source.png` 生成） |
| `packages/desktop/src-tauri/icons/icon.png` | 512×512 主 png（`pnpm tauri icon` 生成） |
| `packages/desktop/src-tauri/icons/32x32.png` | `pnpm tauri icon` 生成（`bundle.icon` 引用） |
| `packages/desktop/src-tauri/icons/128x128.png` | `pnpm tauri icon` 生成（`bundle.icon` 引用） |
| `packages/desktop/src-tauri/icons/128x128@2x.png` | `pnpm tauri icon` 生成（`bundle.icon` 引用） |

> `pnpm tauri icon` 可能额外生成 iOS / Android（`Square*.png` / `android/` / `ios/`）资产；本变更桌面应用不消费，可删除不检入，亦可在实现期保留（无害）。AC-7 只断言 icns + 多尺寸 png + ico 在案。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `.github/workflows/desktop-release.yml` | ① `detect` 末追加「创建 tag（已存在则跳过）」step（自原 `release` job 前移，`git ls-remote --exit-code origin refs/tags/desktop-v$VERSION` 判存）；② 原 `release` job 重构为 `build` matrix（`strategy.matrix.include`：`windows-latest` → `--bundles nsis` / `macos-latest` → `--bundles dmg`，`args` 参数化，`runs-on` 参数化），移除原 job 内 tag 创建 step；③ 双腿在 tauri-action 后追加「定位 `src-tauri/target/**/latest.json` 并上传工件」step（`actions/upload-artifact@v4`，工件名按 `matrix.artifact`）；④ 新增 `finalize` job（下载双腿工件 + node 合并 + `gh release upload --clobber`）。`permissions: contents: write`、`concurrency: group desktop-release`、`defaults.run.working-directory: packages/desktop`、tauri-action `projectPath` / `tagName` / `releaseName` / `releaseDraft: false` / `prerelease: false`、签名 env（`TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ''`，mac 腿不配任何 `APPLE_*`）全部保持 | 三段结构落位（D1–D5）；Windows 腿构建参数与签名 env 零回退（AC-5） |
| `packages/desktop/src-tauri/tauri.conf.json` | `bundle.icon` 由 `["icons/icon.ico"]` 改为 D7 权威列表（ico / icns / 多尺寸 png） | `version` / `targets: "all"` / `createUpdaterArtifacts: true` / `plugins.updater`（endpoint 与 pubkey）不变 |
| `packages/desktop/src-tauri/icons/icon.ico` | 被 `pnpm tauri icon` 重新生成（多尺寸 ico，外观 = 纯蓝，观感变化已由 R7 接受） | 文件「保留在案」非删除（AC-7） |
| `packages/desktop/RELEASE.md` | 更新为双平台分发说明：`每次发版` 产物核对清单补 mac 产物（`dev-team_<version>_aarch64.dmg` + `.app.tar.gz` + `.sig`）并校正 Windows 产物名 `desktop-terminal_…` → `dev-team_<version>_x64-setup.exe`；新增 mac 首次安装 Gatekeeper 右键打开说明与「mac 权威升级 = 手动下载 dmg，updater 尽力而为不承诺」；`约束与已知行为` 更新「仅 Windows 分发」相关措辞；Windows 更新链路说明（minisign 校验 / NSIS `/UPDATE` 静默安装 / 密钥管理）保持 | AC-8；双平台文档 |
| `packages/desktop/package.json` | `version` 0.4.29 → 0.4.30 | 版本交付（D8）；`tauri.conf.json` 自动跟随 |

### 删除文件

无。

### 公共函数 / API

<!-- 本变更零 Rust/TS 源码与零新增仓库脚本模块：finalize 合并逻辑为 workflow 内联 node 脚本（无模块级导出、无 CLI 子命令、无 HTTP 端点），故省略本子节。 -->

### 类型定义

<!-- 无 interface / type alias / enum / 公共 API class 变更，省略本子节。 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `bundle.icon` | `packages/desktop/src-tauri/tauri.conf.json` | 修改 | `["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]` | 自 `["icons/icon.ico"]` 扩为全套；供 Windows / macOS / 回退打包（D7）。`version` / `targets` / `createUpdaterArtifacts` / `plugins.updater` 不变 |
| `build.strategy.matrix` | `.github/workflows/desktop-release.yml` | 修改 | `include: [{platform: windows-latest, args: --bundles nsis, artifact: latest-json-windows}, {platform: macos-latest, args: --bundles dmg, artifact: latest-json-macos}]` | 单平台 `release` job 扩为双平台矩阵（D5 上下文） |

---

## 数据模型

### latest.json（Tauri v2 updater manifest，finalize 合并的权威对象）

```json
{
  "version": "0.4.30",
  "notes": "<release notes，取 base（Windows 腿）>",
  "pub_date": "<ISO8601，取 base（双腿各自生成可能差秒级，权威取 base）>",
  "platforms": {
    "windows-x86_64": { "signature": "<minisign 签名>", "url": "<nsis 安装包资产 URL>" },
    "darwin-aarch64": { "signature": "<minisign 签名>", "url": "<.app.tar.gz 资产 URL>" }
  }
}
```

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `latest.json`（合并后权威） | `version: string`（= package.json 版本，单版本源）；`notes: string`；`pub_date: string`；`platforms: { [platformId]: PlatformEntry }` | `platforms` 每平台恰一条目；`platformId ∈ { windows-x86_64, darwin-aarch64 }` | GitHub Release 资产 `latest.json`（`releases/latest/download/latest.json` 端点消费） |
| `PlatformEntry` | `signature: string`（minisign 签名，双腿共用同一 pubkey）；`url: string`（指向该平台 updater/安装资产） | 隶属 `platforms` 键 | 内嵌于 `latest.json` |
| `build` matrix 参数 | `platform`（runner）、`args`（`--bundles nsis` / `--bundles dmg`）、`artifact`（工件名） | 一腿产出一份 `latest.json` 工件 + 一套安装/更新资产 | GitHub Actions matrix / 工件通道（非持久，随 run 保留） |
| 图标源几何 | 1024×1024、单色 `#2563eb`（RGBA 不透明） | 源图 → `tauri icon` → ico / icns / 多尺寸 png | `src-tauri/icons/icon-source.png`（检入） |

### finalize 合并算法（内联 node，幂等 / 缺腿容忍）

```text
输入: base = 解析 _updater/windows/latest.json（必存在，缺则 fail）
      mac  = _updater/macos/latest.json 存在 ? 解析 : null
校验: mac != null 且 mac.version !== base.version → 非零退出
合并: merged = { ...base }                              // version/notes/pub_date 取 Windows base
      merged.platforms = { ...(mac?.platforms ?? {}), ...base.platforms }
      // base 后展开 → 同键覆盖取 base（Windows 条目逐字保留）；缺 mac 时仅 Windows
输出: 写 latest.json（cwd）
上传: gh release upload "desktop-v<version>" latest.json --clobber   // 覆盖双腿遗留的同名资产
```

幂等说明：`platforms` 为键控对象，`windows-x86_64` / `darwin-aarch64` 键唯一；force 重发同版本时 tauri-action 双腿重新上传同键条目、finalize 以 base 覆盖合并，无重复条目可能。

---

## 依赖

### 运行时依赖

- 无新增应用运行时依赖（零 Rust/TS 源码变更）。

### 构建/测试依赖

- `tauri-apps/tauri-action@v0`（已有）— 双腿构建与 release 资产上传，复用既有发布语义。
- `actions/upload-artifact@v4` / `actions/download-artifact@v4`（新增于 workflow）— 双腿 `latest.json` 工件的捕获与取回（D2）。
- `gh` CLI + `node`（GitHub 托管 runner 预装）— finalize 合并脚本与 `gh release upload --clobber`（D1）。
- `@tauri-apps/cli`（已有 devDependency，`pnpm tauri icon`）— 纯色源 PNG 生成 ico / icns / png 全套（D6）。
- Node 内置 `zlib`（内联 PNG 写入器）— 确定性生成 1024×1024 纯色 `icon-source.png`，零新依赖。

---

## 验收标准覆盖

| 验收 | 设计落点 |
|------|----------|
| AC-1 macOS 产物 | `build` matrix 新增 `macos-latest --bundles dmg` 腿；`createUpdaterArtifacts: true` 自动产出 `.app.tar.gz` + `.sig`；版本单源（`version: "../package.json"` + workflow 读 package.json） |
| AC-2 latest.json 双平台 | `finalize` D3 合并（base=Windows，键合并 `darwin-aarch64`）；Windows 条目逐字保留 |
| AC-3 幂等与缺腿 | D3 键覆盖幂等 + D4 `continue-on-error` 缺腿下载、`(success() \|\| failure())` 门 |
| AC-4 tag 前移 | D5：detect 内单点建 tag，双腿复用既有 release（竞态备选 detect 预建 release） |
| AC-5 Windows 腿不回退 | 修改文件表：Windows 腿 `args: --bundles nsis`、签名 env、`releaseDraft` / `prerelease` / `projectPath` / `tagName` / `releaseName` 保持 |
| AC-6 mac 无签名产出 | `build` mac 腿不配任何 `APPLE_*` env，仅 `GITHUB_TOKEN` + `TAURI_SIGNING_*`（updater minisign，与 Windows 同 key） |
| AC-7 图标成套 | 新增文件表（`icon.icns` + 多尺寸 png + `icon.ico`）+ D6/D7 生成与 `bundle.icon` 列表 |
| AC-8 文档 | RELEASE.md 修改行（双平台 + 右键打开 + 手动升级） |
| AC-9 版本与零回归 | D8（0.4.30）+ `plugins/dev-team` 零改动；零 Rust/TS 源码变更 |

---

## 待决问题

以下为**实现期验证项**，不阻塞本设计（已给定稿主路径与备选），实现阶段按验证结果落主路径或备选：

- tauri-action 对 matrix + updater 的 `latest.json` 是否内置合并——决定 `finalize` 是「自行合并」（主路径，D1）还是「退化为校验」。
- tauri-action「release 已存在则只上传」行为确认——若仍存在并发 create 竞态窗口，落 D5 备选「detect 预建 release」。
- 无 `APPLE_*` env 时 mac updater 工件（`.app.tar.gz` + `.sig`）是否照常产出——预期能（仅无 OS 级签名）；若 tauri 报错按 R6 调整（不改变「无 Apple 账号」前提）。
- `tauri icon` 纯色源 PNG 产出的 icns 在 dmg/app 上的落位确认（AC-7 只断言文件在案与 `bundle.icon` 列表）。
- 双腿 `latest.json` 工件的本地定位路径（`src-tauri/target/**/latest.json` 的 `find -print -quit`）在 Windows / macOS runner 上的实际命中确认。
