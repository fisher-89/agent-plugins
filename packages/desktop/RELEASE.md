# Desktop 发版与自动更新

发版链路:`tauri build` 产出带签名的安装包 → master 推送且 package.json
版本高于仓库最新 release 的版本时,GitHub Actions
(`.github/workflows/desktop-release.yml`)自动打 tag(三段:detect 建 tag →
build matrix 双平台并行构建 → finalize 合并 `latest.json` 后上传权威版本)并由
tauri-action 发布到
[fisher-89/agent-plugins](https://github.com/fisher-89/agent-plugins) 的 Releases →
已安装的 Windows 应用启动时拉取 `releases/latest/download/latest.json` 比对版本 →
顶栏出现「更新到 vX.Y.Z」→ 点击下载(带进度)→ NSIS 安装器静默安装并自动重启。

分发两个平台:

- **Windows**:安装版,自动更新链路完整(见上)。
- **macOS**(Apple Silicon / aarch64):手动安装 dmg;升级的权威路径是手动下载新版
  dmg,应用内更新「尽力而为、不承诺」(见「macOS 安装与升级」)。

两个平台共用一份版本号(权威是 `packages/desktop/package.json`)与一把 minisign
更新签名密钥:Tauri updater 内置的 minisign 签名校验不可关闭,使用下方本地密钥对。
macOS 侧不配置任何 Apple 签名(`APPLE_*`)secret——mac 版无 OS 级代码签名,Apple
Silicon 二进制带链接器 ad-hoc 签名,可运行(首次打开需右键放行,见下)。

## 一次性准备(已完成则跳过)

1. 生成签名密钥对(密码为空):

   ```powershell
   pnpm -C packages/desktop exec tauri signer generate -w "$env:USERPROFILE\.tauri\desktop-terminal.key" --ci
   ```

2. 公钥已写入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。
3. 在 GitHub 仓库 **Settings → Secrets and variables → Actions** 添加
   `TAURI_SIGNING_PRIVATE_KEY` = 私钥文件(`~/.tauri/desktop-terminal.key`)的完整内容。

   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` **无需创建**:密钥密码为空,而 GitHub
   secret 不接受空串;workflow 引用未定义 secret 会解析为空字符串,env 变量以
   「已定义、值为空」传给 tauri CLI,即等价于显式空密码(本地已验证不触发密码
   提示)。若将来换成带密码的密钥,再创建该 secret 即可,workflow 无需改动。

   Windows 与 macOS 双腿都用这把密钥签名各自平台的 updater 工件(NSIS 安装包 /
   `.app.tar.gz`),不额外配置 Apple 签名相关 secret。

> **私钥必须离机备份。** 公钥编译进应用,私钥丢失后现有安装的更新通道即报废,
> 只能发新安装包重建。密码同样遗失则密钥作废。

## 本地构建须知

`tauri.conf.json` 配置 `createUpdaterArtifacts` + `pubkey` 后,本地
`pnpm -C packages/desktop run build` **必须**先设两个签名环境变量:缺私钥构建直接
报错;只设私钥不设密码变量,CLI 会在打包完成后卡在 `Decrypting updater signing
key, expect a prompt for password` 等 stdin 输入(密钥密码为空也要显式设空):

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\desktop-terminal.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
pnpm -C packages/desktop run build
```

(`tauri dev` 不打包,不受影响。在 mac 上本地打包同理,只是 `$env:` 换成 `export`。)

## 每次发版

1. 改版本号——权威是 `package.json` 的 `version`(`tauri.conf.json` 以
   `"version": "../package.json"` 引用;`src-tauri/Cargo.toml` 的版本是 crate
   自身版本,与应用版本无关)。双腿都读这份版本,自动同版本。
2. commit 后 `git push origin master` 即可:workflow 触发时读取 package.json 版本,
   经 GitHub API 与仓库最新 release 的版本比对,更高则在该 commit 上自动
   创建 tag,双腿(Windows / macOS)并行构建,finalize 合并双平台 `latest.json`;
   不高(未升版/回退)则 build 与 finalize 都跳过,Actions 里表现为 detect 通过、
   build/finalize skipped。
3. 补跑/重发同版本:Actions → desktop-release → Run workflow,勾选 `force`
   (跳过版本比较;tag 已存在时自动跳过创建)。

   > **绝不 `git push --tags`**:本地仓库带有 276 个内部 gitlab(kso)历史遗留 tag,
   > 全量推送会污染 GitHub 仓库。workflow 不监听 tag 推送,手动打 tag 无发版效果,
   > 重发一律走上面的 force 入口。

4. Actions 跑完后核对 release 产物:
   - `dev-team_X.Y.Z_x64-setup.exe`(Windows NSIS 安装包)+ `.sig`(签名)
   - `dev-team_X.Y.Z_aarch64.dmg`(macOS 安装包)
   - `dev-team_aarch64.app.tar.gz`(macOS updater 工件)+ `.sig`(签名)
   - `latest.json`(`version` 为 X.Y.Z、`platforms` 含 `windows-x86_64` 与
     `darwin-aarch64`)

   mac 腿失败时不会有后两项里的 mac 产物,`latest.json` 只含 `windows-x86_64`
   (finalize 容忍缺腿,Windows 更新链路照常)。

## macOS 安装与升级

- **首次安装**:从 Releases 下载 `dev-team_X.Y.Z_aarch64.dmg` → 打开并把 App
  拖进「应用程序」→ 双击启动会被 Gatekeeper 拦下(「无法打开,因为 Apple 无法
  检查其是否包含恶意软件」)→ 在访达中 **右键 App 图标 → 「打开」→ 再点「打开」**
  放行,之后正常双击启动即可。
  原因:团队无 Apple 开发者账号,mac 版不做 OS 级签名与公证——这是预期行为,
  不是构建故障。
- **升级**:权威路径 = 手动下载新版 dmg,退出应用后覆盖安装。
  应用内更新对 mac 是「尽力而为、不承诺」:启动检查若有新版本会显示入口,但 mac
  更新路径未纳入验收,失败静默;不要把应用内更新当作 mac 的升级依据。
- 仅构 aarch64(Apple Silicon);不产 universal / x86_64 产物。

## 约束与已知行为

- **本仓库只能发 desktop release**:`releases/latest/download/latest.json` 端点跟随
  仓库最新非 draft、非 prerelease 的 release。若发布任何不带 `latest.json` 的其他
  release,已安装应用的更新检查会静默失败(hook 设计为启动期失败不报错)。
- workflow 双腿各固定 bundle:Windows 腿 `--bundles nsis`,mac 腿
  `--bundles app,dmg`(mac 的 `app` 目标是 updater 工件 `.app.tar.gz` + `.sig` 的
  产出前提,`dmg` 是安装包);conf 里 `targets: "all"` 只影响本地构建,CI 只打这两类,
  保证 `latest.json` 的平台条目确定。
- `latest.json` 的权威归属是 `finalize` job:它合并双腿产物后
  `gh release upload --clobber` 覆盖双腿各自上传的同名资产(否则后结束的腿会把另一
  平台条目覆盖掉)。合并幂等(同版本条目覆盖、不重复)、容忍缺腿,`windows-x86_64`
  条目由 Windows 腿逐字提供。
- 更新交互(Windows):启动后台检查(失败静默),有新版本才在顶栏显示入口;下载完成后
  NSIS 安装器以 `/UPDATE` 静默安装并自动重启应用,无「重启生效」按钮。
- `tauri dev` 不检查更新(`import.meta.env.DEV` 守卫),更新路径只有安装版能走到。
- 首次从网页下载安装会有 SmartScreen 警告(Windows,无 Authenticode 的预期行为);
  mac 侧对应的是上面的 Gatekeeper 右键放行。应用内自动更新不经过浏览器,无此提示。
- 密钥生成与 secrets 配置详见上节;私钥遗失 = 更新通道报废。
