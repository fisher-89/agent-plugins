# Desktop 发版与自动更新

更新链路:`tauri build` 产出带签名的 NSIS 安装包 → master 推送且 package.json
版本高于远端最新 `desktop-v*` tag 时,GitHub Actions
(`.github/workflows/desktop-release.yml`)自动打 tag 并由 tauri-action 发布到
[fisher-89/agent-plugins](https://github.com/fisher-89/agent-plugins) 的 Releases →
已安装应用启动时拉取 `releases/latest/download/latest.json` 比对版本 →
顶栏出现「更新到 vX.Y.Z」→ 点击下载(带进度)→ NSIS 安装器静默安装并自动重启。

仅 Windows 分发,无 OS 级代码签名(无 Authenticode);Tauri updater 内置的
minisign 签名校验不可关闭,使用下方本地密钥对。

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

(`tauri dev` 不打包,不受影响。)

## 每次发版

1. 改版本号——权威是 `package.json` 的 `version`(`tauri.conf.json` 以
   `"version": "../package.json"` 引用;`src-tauri/Cargo.toml` 的版本是 crate
   自身版本,与应用版本无关)。
2. commit 后 `git push origin master` 即可:workflow 触发时读取 package.json 版本,
   经 `git ls-remote` 与远端最新 `desktop-v*` tag 比对,更高则在该 commit 上自动
   创建 tag 并发版;不高(未升版/回退)则 release job 跳过,Actions 里表现为
   detect 通过、release skipped。
3. 补跑/重发同版本:Actions → desktop-release → Run workflow,勾选 `force`
   (跳过版本比较;tag 已存在时自动跳过创建)。

   > **绝不 `git push --tags`**:本地仓库带有 276 个内部 gitlab(kso)历史遗留 tag,
   > 全量推送会污染 GitHub 仓库。workflow 不监听 tag 推送,手动打 tag 无发版效果,
   > 重发一律走上面的 force 入口。

4. Actions 跑完后核对 release 产物:
   - `desktop-terminal_X.Y.Z_x64-setup.exe`(NSIS 安装包)
   - `desktop-terminal_X.Y.Z_x64-setup.exe.sig`(签名)
   - `latest.json`(`version` 为 X.Y.Z、`platforms` 含 `windows-x86_64`)

## 约束与已知行为

- **本仓库只能发 desktop release**:`releases/latest/download/latest.json` 端点跟随
  仓库最新非 draft、非 prerelease 的 release。若发布任何不带 `latest.json` 的其他
  release,已安装应用的更新检查会静默失败(hook 设计为启动期失败不报错)。
- workflow 固定 `--bundles nsis`:conf 里 `targets: "all"` 只影响本地构建,
  CI 只打 NSIS,保证 `latest.json` 的 `windows-x86_64` 条目确定。
- 更新交互:启动后台检查(失败静默),有新版本才在顶栏显示入口;下载完成后
  NSIS 安装器以 `/UPDATE` 静默安装并自动重启应用,无「重启生效」按钮。
- `tauri dev` 不检查更新(`import.meta.env.DEV` 守卫),更新路径只有安装版能走到。
- 首次从网页下载安装会有 SmartScreen 警告(无 Authenticode 的预期行为);
  应用内自动更新不经过浏览器,无此提示。
- 密钥生成与 secrets 配置详见上节;私钥遗失 = 更新通道报废。
