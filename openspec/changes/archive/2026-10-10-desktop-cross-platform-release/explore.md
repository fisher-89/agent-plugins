# desktop 跨平台发版探索(macOS 产物)

日期:2026-10-08 · 状态:探索完成,待开 change

## 动机

desktop(`packages/desktop`,Tauri 2)目前仅分发 Windows。需要构建链路同时产出
Windows 与 macOS 应用,mac 侧供团队几个人手动安装使用。

## 现状

- 发版链路:`.github/workflows/desktop-release.yml` —— detect(ubuntu,版本比较)
  → release(**仅 windows-latest**),`--bundles nsis`,tauri-action 发布到
  fisher-89/agent-plugins(公开仓库)的 Releases,tag `desktop-v<version>` 在
  release job 内创建。
- 更新链路:安装版启动拉 `releases/latest/download/latest.json`,minisign 校验
  (pubkey 编译进应用),NSIS `/UPDATE` 静默安装。本地私钥 secret:
  `TAURI_SIGNING_PRIVATE_KEY`(空密码)。
- `tauri.conf.json`:`bundle.targets: "all"`(只影响本地构建,CI 用 args 覆盖)、
  `createUpdaterArtifacts: true`、`version: "../package.json"`(单一版本源)。
- 图标:`src-tauri/icons/` 下仅 `icon.ico`,无 PNG/SVG 源,无 `.icns`。
- RELEASE.md 明确记录「仅 Windows 分发」。

## 硬约束与前提

- **Tauri 无法在 Windows 上交叉编译 macOS 产物**(需 Apple 工具链),双平台产出
  只能落在 CI(或真 mac)。本变更走 CI。
- 发布仓库公开 → GitHub 托管 `macos-latest` runner 免费(且为 ARM 机型,
  Apple Silicon 原生构建,无交叉)。
- 团队无 Apple 开发者账号 → 不引入任何 `APPLE_*` 签名环境变量;mac 版无
  OS 级代码签名(Apple Silicon 二进制天然带链接器 ad-hoc 签名,可运行)。

## 已定决策(2026-10-08 与维护者确认)

| 决策点 | 结论 |
|--------|------|
| 用户面 | 团队几人,接受首次安装 Gatekeeper 右键打开 |
| Apple 签名 | 无账号,不配 APPLE_* secrets |
| 验收线 | **CI 产出 dmg 即验收**,不做 mac 运行时冒烟 |
| 版本号 | 与 Windows 同一版本 —— 已有机制零成本(`version: "../package.json"`,workflow 读它打 tag,matrix 两腿自动同版本) |
| mac 架构 | 仅 aarch64(Apple Silicon),不构建 universal / x86_64 |
| latest.json 归属 | 按推荐:**新增 finalize 步骤作为 latest.json 权威归属方**,合并双平台 `platforms` 后上传;实现期顺带验证 tauri-action 是否已内置合并,已内置则 finalize 退化为校验 |
| 图标源 | 无原始设计稿,**先用程序生成纯色 1024×1024 PNG**,跑 `pnpm tauri icon` 生成全套(ico/icns/png) |
| mac 更新口径 | **尽力而为,不验收**;mac 用户权威升级路径 = 手动下载 dmg;RELEASE.md 补首次安装右键打开说明 |

## 目标形态

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

## 变更面

```
改 1 个 workflow   desktop-release.yml:tag 前移 + matrix + finalize
改 1 个 conf      tauri.conf.json:bundle.icon 补 icns/png
补 1 套图标       纯色源 PNG → tauri icon 生成全套
改 1 个文档       RELEASE.md:双平台分发 + mac 安装/升级注意事项
新增 0 行应用代码  Rust/前端零改动(验收线为「产出即可」的红利)
```

## 风险与红线

- **回归红线:Windows 更新链路不能坏。** 即使 mac 更新不验收,两腿各自上传的
  latest.json 互覆盖也会伤及现有 Windows 装机(mac 腿后结束则 latest.json 只剩
  darwin 条目,Windows 更新检查静默失效)——finalize 步骤无论如何必须有。
- 首发组合的空窗:mac 腿失败时 release 只含 Windows 产物,finalize 需容忍缺失
  平台(合并现存条目,不因缺腿失败)。
- force 重发同版本:finalize 合并需幂等(同版本条目覆盖而非重复)。

## 实现期待验证项

1. tauri-action 对 matrix + updater 场景的 latest.json 是否自动合并(README 有
   专门讨论);不合并则 finalize 自行合并。
2. 双腿并行调用 tauri-action 的 **release 创建竞态**(双方均带 tagName):确认
   其「release 已存在则只上传」行为;若有竞态窗口,备选方案是 detect 预建 release。
3. 无 APPLE_* env 时 mac updater 工件(.app.tar.gz + sig)能否正常产出
   (预期:能,仅无 OS 级签名;若 tauri 报错则按需调整)。
4. `tauri icon` 纯色源 PNG 产出的 icns 在 dmg/app 上的落位确认。

## 验收草案(供 proposal 参考)

- AC-1 release 产物含 `*_aarch64.dmg`(及 .app.tar.gz + .sig),版本与
  package.json 一致。
- AC-2 latest.json 同时含 `windows-x86_64` 与 `darwin-aarch64` 条目,Windows
  条目签名与现状形态一致(回归红线)。
- AC-3 Windows 腿构建参数与流程不回退(nsis bundle、updater 签名 env)。
- AC-4 RELEASE.md 更新为双平台说明,含 mac 首次安装右键打开与手动升级路径。

## 下一步

开 change(`desktop-cross-platform-release`),workflow_type: requirement。
本笔记由 phase-proposal 自动提拔为 `openspec/changes/<name>/explore.md`。