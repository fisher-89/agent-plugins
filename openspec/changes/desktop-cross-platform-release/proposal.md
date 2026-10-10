# 提案: desktop-cross-platform-release

> **变更**: desktop-cross-platform-release
> **日期**: 2026-10-08
> **状态**: draft
> **探索**: `openspec/changes/desktop-cross-platform-release/explore.md`（决策已与维护者确认在案）

---

## 问题

desktop（`packages/desktop`，Tauri 2）当前只分发 Windows，发版链路是一条单平台流水线：

- 发版：`.github/workflows/desktop-release.yml` —— `detect`（ubuntu，读 package.json 版本与仓库最新 release 比对）→ `release`（**仅 windows-latest**，`--bundles nsis`），tauri-action 发布到 `fisher-89/agent-plugins` 的 Releases，tag `desktop-v<version>` 在 `release` job 内创建。
- 更新：安装版启动拉 `releases/latest/download/latest.json`，minisign 校验（pubkey 编译进应用），NSIS `/UPDATE` 静默安装。
- 图标：`src-tauri/icons/` 仅 `icon.ico`，无 PNG/SVG 源、无 `.icns`。
- 文档：RELEASE.md 明确记录「仅 Windows 分发」。

需要让构建链路同时产出 Windows 与 macOS 应用，mac 侧供团队几个人手动安装使用。硬约束（explore 已核实）：

1. **Tauri 无法在 Windows 上交叉编译 macOS 产物**（需 Apple 工具链）→ 双平台产出只能落在 CI（或真 mac）；本变更走 CI，GitHub 托管 `macos-latest` 免费且为 ARM 机型（Apple Silicon 原生构建，无交叉）。
2. **团队无 Apple 开发者账号** → 不引入任何 `APPLE_*` 签名环境变量；mac 版无 OS 级代码签名（Apple Silicon 二进制天然带链接器 ad-hoc 签名，可运行）。
3. **验收线 = CI 产出 dmg 即验收**，不做 mac 运行时冒烟。

把单平台流水线扩成双平台，真正的风险面不在「多一条 mac 腿」，而在 `latest.json` 的归属：双腿各自上传 `latest.json` 会互相覆盖（后结束的腿只剩自己平台条目），直接打穿现有 Windows 更新链路。

---

## 提案

把发版流水线重构为「detect（前移 tag）→ build matrix（双平台）→ finalize（latest.json 权威合并）」三段，mac 腿作为矩阵新腿加入、Windows 腿原样保留：

```
detect (ubuntu)
  ├─ 版本比较(不变)
  ├─ 创建 tag ◀── 前移(现于 release job 内,matrix 双腿并行会竞态)
  ▼
build (matrix)
  ├─ windows-latest → --bundles nsis(整腿原样保留)
  └─ macos-latest   → --bundles dmg(aarch64 原生,无 APPLE_* env,
       updater 工件随 createUpdaterArtifacts 自动产出)
  ▼
finalize (ubuntu)
  合并双平台 latest.json → 上传权威版本
```

变更面（explore 已收口为「改 2 文件 + 补 1 套图标 + 改 1 文档，0 行应用代码」）：

1. **`.github/workflows/desktop-release.yml`**：tag 创建前移至 `detect`（规避 matrix 双腿并行触发 tauri-action release 创建竞态）；`release` 拆成 `build` matrix（windows-latest → nsis / macos-latest → dmg）；新增 `finalize`（ubuntu）作为 `latest.json` 权威归属方——合并双腿 `platforms` 后上传权威版本。
2. **`tauri.conf.json`**：`bundle.icon` 补 `icns` / `png`（现仅 `icon.ico`）；`targets: "all"` / `createUpdaterArtifacts: true` / `version: "../package.json"` 不变（单一版本源，matrix 两腿自动同版本）。
3. **图标**：无原始设计稿，程序生成纯色 1024×1024 PNG 源 → `pnpm tauri icon` 生成全套（ico / icns / png）落 `src-tauri/icons/`。
4. **`RELEASE.md`**：更新为双平台分发说明 + mac 首次安装 Gatekeeper 右键打开 + 手动升级路径（mac 更新「尽力而为、不验收」，权威升级路径 = 手动下载 dmg）。

回归红线：**Windows 更新链路不能坏**——`finalize` 无论 tauri-action 是否已内置合并都必须存在（已内置则退化为校验）；`finalize` 合并需幂等（同版本条目覆盖而非重复）且容忍缺失平台（mac 腿失败时 release 只含 Windows 产物，合并现存条目、不因缺腿失败）。

---

## 能力

### 新增能力

- **desktop-release-pipeline** — desktop 发版流水线契约（本变更首次将流水线从「workflow + RELEASE.md 口头契约」升格为 spec 能力）：detect → build matrix → finalize 三段结构、双平台产物矩阵（Windows NSIS / macOS aarch64 dmg）、`latest.json` 权威合并（幂等 / 容忍缺失平台 / Windows 条目签名形态不变）、tag 前移与竞态规避、更新工件与签名（Windows minisign / mac 无 APPLE_* 签名）、图标成套、RELEASE.md 双平台文档、版本交付。

### 修改的能力

- 无（release 流水线此前无对应 spec，本变更为全新能力基线；`desktop-app-shell` 中 updater 应用侧行为——UpdateIndicator / 「重试更新」按钮 / 启动期静默检查——零改动，见「引用沿用」）。

### 引用沿用（零 delta）

- desktop-app-shell — updater 应用侧行为（启动后台检查失败静默、有新版本才显顶栏入口、`UpdateIndicator` 含「重试更新」按钮、updater 错误呈现）零改动；`tauri.conf.json` 的 updater `endpoint` 与 `pubkey` 不变（本变更仅动 `bundle.icon`）。

---

## 变更范围

### 实现文件

- `.github/workflows/desktop-release.yml` — tag 前移至 `detect`；`release` → `build` matrix（windows-latest → nsis / macos-latest → dmg）；新增 `finalize`（ubuntu）latest.json 权威合并
- `packages/desktop/src-tauri/tauri.conf.json` — `bundle.icon` 由 `["icons/icon.ico"]` 扩为含 `icon.ico` / `icon.icns` / 多尺寸 `png`
- `packages/desktop/src-tauri/icons/` — 新增纯色源 PNG 与 `pnpm tauri icon` 生成的全套图标（ico / icns / png）
- `packages/desktop/RELEASE.md` — 双平台分发 + mac 首次安装右键打开 + 手动升级路径
- `packages/desktop/package.json` — `version` 0.4.29 → 0.4.30

### 测试文件

- 无（本变更只触及 CI workflow、tauri 打包配置、图标资产与文档，零 Rust/TS 应用代码；`packages/desktop` 的 `vp test` / `client:check` / `server:check` 均不覆盖这些面，验收以「release 实际产出 dmg + latest.json 双平台条目」为准——见验收标准）。

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（版本 2.10.44、三类交付产物）
- `packages/desktop/src/` 与 `packages/desktop/src-tauri/crates/`、`src-tauri/src/` 全部 Rust/前端源码（零改动）
- `tauri.conf.json` 的 `version` / `bundle.targets` / `createUpdaterArtifacts` / `plugins.updater`（endpoint 与 pubkey 不变）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | macOS 产物 | CI release 产物含 `*_aarch64.dmg`（及 updater 工件 `.app.tar.gz` + `.sig`），版本与 `packages/desktop/package.json` 一致（单一版本源零成本达成） |
| AC-2 | latest.json 双平台 | `finalize` 后 `releases/latest/download/latest.json` 同时含 `windows-x86_64` 与 `darwin-aarch64` 两条目；Windows 条目形态（`version` / `signature` / `url` / `notes` 等）与现状一致（回归红线） |
| AC-3 | latest.json 幂等与容忍缺失平台 | force 重发同版本时 `finalize` 合并幂等（同版本条目覆盖而非重复）；仅 Windows 腿成功时（mac 腿失败 / 缺腿）`finalize` 照常产出含现存平台的权威 `latest.json`，不因缺腿失败 |
| AC-4 | tag 前移 | tag `desktop-v<version>` 在 `detect`（build matrix 之前）创建；双腿并行触发时无 release 创建竞态（tauri-action「release 已存在则只上传」或 detect 预建 release，按实现期验证结果二选一） |
| AC-5 | Windows 腿不回退 | Windows 腿构建参数与流程不回退（`--bundles nsis`、`TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD` 签名 env、`releaseDraft: false` / `prerelease: false`、`projectPath` / `tagName` / `releaseName` 语义不变） |
| AC-6 | mac 无签名可产出 | 不配置任何 `APPLE_*` env 时 mac updater 工件正常产出（无 OS 级签名，二进制带链接器 ad-hoc 签名）；`--bundles dmg`、aarch64（不构建 universal / x86_64） |
| AC-7 | 图标成套 | `src-tauri/icons/` 含 `icon.icns` 与多尺寸 png，`icon.ico` 保留；`pnpm tauri icon` 自纯色 1024×1024 PNG 源生成；`tauri.conf.json` `bundle.icon` 列出 ico / icns / png 全套 |
| AC-8 | 文档 | RELEASE.md 更新为双平台分发说明，含 mac 首次安装 Gatekeeper 右键打开、mac 权威升级路径 = 手动下载 dmg（updater 尽力而为不承诺） |
| AC-9 | 版本与零回归 | `packages/desktop/package.json` version 0.4.30；`plugins/dev-team` 零改动；`packages/desktop` 的 `vp test` / `client:check` / `server:check`（含 bindings:check）全绿（本变更零 Rust/TS 源码，属回归确认） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 Windows 更新链路回归（latest.json 互覆盖） | mac 腿后结束则 latest.json 只剩 darwin 条目，现有 Windows 装机更新检查静默失效 | 高 | finalize 步骤无论如何必有（回归红线）；AC-2 断言 Windows 条目形态不变；finalize 作为 latest.json 唯一权威归属方 |
| R2 tauri-action matrix + updater 的 latest.json 合并行为未定 | finalize 若重复实现或误信内置合并，产物错误 | 中 | 实现期待验证项 1：先验证 README 是否内置合并；内置则 finalize 退化为校验，未内置则自行合并；AC-2/AC-3 覆盖 |
| R3 双腿并行 release 创建竞态 | 双方均带 tagName，release 创建竞争、工件互踩 | 中 | tag 前移至 detect（AC-4）；实现期验证 tauri-action「release 已存在则只上传」行为，若有竞态窗口备选 detect 预建 release |
| R4 首发组合空窗（mac 腿失败） | 首发时 release 只有 Windows 产物 | 中 | finalize 容忍缺失平台（合并现存条目、不因缺腿失败）；AC-3 覆盖 |
| R5 mac 无签名可运行性 | 团队首次安装需 Gatekeeper 右键打开，体验摩擦 | 高（确定发生） | 用户已拍板接受（团队几人、手动安装）；RELEASE.md 补右键打开说明（AC-8）；不做 mac 运行时冒烟（验收线 = 产出 dmg） |
| R6 无 APPLE_* env 时 updater 工件产出不确定性 | tauri 若对 mac 强校验签名环境则构建失败 | 低 | 实现期待验证项 3：预期能产出（仅无 OS 级签名）；若 tauri 报错按需调整（如显式跳过 mac updater 签名）；AC-6 覆盖 |
| R7 纯色图标观感 | 无原始设计稿，程序生成纯色图标缺乏品牌识别 | 中 | 用户已拍板先用纯色源 PNG（后续有设计稿再替换，仅换源图重跑 `tauri icon`）；AC-7 覆盖落位 |
| R8 force 重发幂等 | 同版本重发时 latest.json 条目重复或覆盖错乱 | 低 | finalize 合并幂等（同版本条目覆盖）；AC-3 覆盖 |
| R9 mac 更新链路「尽力而为」的隐性承诺 | 用户误以为 mac 也支持自动更新 | 低 | RELEASE.md 明确 mac 权威升级 = 手动下载 dmg、updater 不承诺（AC-8）；mac 更新不验收 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 用户面 | 团队几人，接受首次安装 Gatekeeper 右键打开 | 内部工具、无 Apple 开发者账号；manual install 可接受 | 申请 Apple 账号 + 公证（成本与周期超范围） |
| Apple 签名 | 无账号，不配 APPLE_* secrets | mac 版无 OS 级签名；Apple Silicon 二进制 ad-hoc 签名可运行 | 引入公证（无账号不可行） |
| 验收线 | CI 产出 dmg 即验收，不做 mac 运行时冒烟 | mac 无 CI 冒烟基建；团队手动安装即为冒烟 | 增加 mac 运行时冒烟（超范围） |
| 版本号 | 与 Windows 同一版本 | 已有机制零成本（`version: "../package.json"` + workflow 读它打 tag，matrix 两腿自动同版本） | mac 独立版本源（双源漂移风险） |
| mac 架构 | 仅 aarch64（Apple Silicon），不构建 universal / x86_64 | macos-latest 为 ARM 机型原生构建；团队设备均为 Apple Silicon | universal / x86_64（无诉求，构建成本翻倍） |
| latest.json 归属 | 新增 finalize 步骤作为权威归属方，合并双平台 platforms 后上传；实现期验证 tauri-action 是否已内置合并（内置则 finalize 退化为校验） | latest.json 互覆盖会打穿 Windows 更新链路；权威单点保证双平台条目同现 | 依赖 tauri-action 自动合并（未验证，风险高）；双腿各自上传（会互覆盖，否决） |
| 图标源 | 程序生成纯色 1024×1024 PNG，跑 `pnpm tauri icon` 生成全套 | 无原始设计稿；纯色源可快速落位，后续有稿替换源图即可 | 手工制作 icns（无源、不可维护） |
| mac 更新口径 | 尽力而为、不验收；权威升级 = 手动下载 dmg；RELEASE.md 补右键打开 | 更新链路的完整验证成本高；团队规模小，手动升级可接受 | 承诺 mac 自动更新（不可验收，风险高） |
| tag 前移 | detect 内创建 tag（版本比较后、build 前） | matrix 双腿并行各自带 tagName 触发 release 创建竞态；前移后单点建 tag、双腿只上传 | 保留 release job 内建 tag（竞态窗口） |
| 版本交付 | desktop 0.4.29 → 0.4.30；dev-team 零改动 | 用户可见发版面变更沿 0.4.x patch 先例 | minor bump（无 breaking，过度） |

### 待决问题（实现期验证，不阻塞本 proposal）

- tauri-action 对 matrix + updater 场景的 latest.json 是否自动合并（README 有专门讨论）→ 决定 finalize 是「自行合并」还是「退化为校验」。
- 双腿并行调用 tauri-action 的 release 创建竞态：确认其「release 已存在则只上传」行为；若有竞态窗口，备选 detect 预建 release。
- 无 APPLE_* env 时 mac updater 工件（`.app.tar.gz` + `.sig`）能否正常产出（预期：能，仅无 OS 级签名；若 tauri 报错则按需调整）。
- `tauri icon` 纯色源 PNG 产出的 icns 在 dmg/app 上的落位确认。
- finalize 合并 latest.json 的实现载体（workflow 内 node/shell 脚本 vs tauri-action 内置能力）与合并语义（同版本覆盖 / platforms 键合并）——design 定稿。
