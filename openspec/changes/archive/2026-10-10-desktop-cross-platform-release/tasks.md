# 任务: desktop-cross-platform-release

> **变更**: desktop-cross-platform-release
> **依据**: proposal.md + design.md（D1–D8 决策编号见 design）

任务边界：本列表只含实现任务；测试编写与测试执行由 test-design / test-gen / test-execution 阶段承接（本变更零 Rust/TS 源码，验收以 CI 实际产出为准）。任务内引用的文件均在 design.md 变更清单内；`plugins/dev-team` 全部零改动。

## 阶段一：图标成套（无依赖，先行）

- [x] 生成 1024×1024 纯色源 PNG `packages/desktop/src-tauri/icons/icon-source.png`：在 `packages/desktop` 下运行内联 node 脚本（Node 内置 `zlib` 手写 PNG chunk，颜色 `#2563eb`、8-bit RGB、零 filter，design D6），确定性写出该文件；核验 PNG 头魔数与 1024×1024 尺寸
- [x] 运行 `pnpm -C packages/desktop tauri icon src-tauri/icons/icon-source.png`，确认在 `packages/desktop/src-tauri/icons/` 生成 `icon.icns` / `icon.png` / `32x32.png` / `128x128.png` / `128x128@2x.png` 并重新生成 `icon.ico`（design D6/D7；`tauri icon` 额外产出的 `Square*.png` / `android/` / `ios/` 桌面不消费，删除或保留均可，AC-7 只断言 icns + 多尺寸 png + ico 在案）
- [x] 修改 `packages/desktop/src-tauri/tauri.conf.json`：`bundle.icon` 由 `["icons/icon.ico"]` 改为 `["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]`（design D7；`version` / `bundle.targets` / `createUpdaterArtifacts` / `plugins.updater` 保持不动）

## 阶段二：workflow 三段重构（detect tag 前移 + build matrix + 工件捕获）

- [x] `.github/workflows/desktop-release.yml` 的 `detect` job：在版本比较 step 之后追加「创建 tag（已存在则跳过）」step（`shell: bash`，`env.VERSION = ${{ needs.detect.outputs.version }}` 语义不变，内容自原 `release` job 逐字前移：`git ls-remote --exit-code origin "refs/tags/desktop-v$VERSION"` 判存，不存在则 `git tag "desktop-v$VERSION"` + `git push origin "refs/tags/desktop-v$VERSION"`）（design D5）
- [x] 将原 `release` job 重构为 `build` job：`needs: detect`、`if: needs.detect.outputs.should == 'true'`、`strategy.matrix.include` 两条（`{platform: windows-latest, args: --bundles nsis, artifact: latest-json-windows}` / `{platform: macos-latest, args: --bundles dmg, artifact: latest-json-macos}`）、`runs-on: ${{ matrix.platform }}`；删除原 job 内的 tag 创建 step（已前移 detect）
- [x] `build` 双腿保持 checkout / pnpm-action-setup(11.22.0) / setup-node(24, cache pnpm, cache-dependency-path packages/desktop/pnpm-lock.yaml) / dtolnay-rust-toolchain / Swatinem-rust-cache(workspaces packages/desktop/src-tauri) / `pnpm install --frozen-lockfile` / tauri-action@v0；tauri-action `with` 参数化 `args: ${{ matrix.args }}`，其余 `projectPath: packages/desktop` / `tagName: desktop-v${{ needs.detect.outputs.version }}` / `releaseName: 'Dev Team v${{ needs.detect.outputs.version }}'` / `releaseDraft: false` / `prerelease: false` 双腿一致；env 保持 `GITHUB_TOKEN` + `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ''`（mac 腿不配任何 `APPLE_*`，design AC-6）
- [x] `build` 双腿在 tauri-action 之后追加「定位并上传 latest.json 工件」：`shell: bash`，`find "$GITHUB_WORKSPACE/packages/desktop/src-tauri/target" -name latest.json -print -quit` 取唯一命中（缺则 `exit 1`），`cp` 到 `"$GITHUB_WORKSPACE/_updater/latest.json"`；随后 `actions/upload-artifact@v4`（`name: ${{ matrix.artifact }}`、`path: _updater/latest.json`、`if-no-files-found: error`、`if: success()`）（design D2）

## 阶段三：finalize 合并与上传

- [x] 新增 `finalize` job：`needs: [detect, build]`、`if: needs.detect.outputs.should == 'true' && (success() || failure())`、`runs-on: ubuntu-latest`（design D4）
- [x] `finalize` 下载双腿工件：`actions/checkout@v7` 后 `actions/download-artifact@v4` 两步——Windows（`name: latest-json-windows`、`path: _updater/windows`、不设 `continue-on-error`）与 mac（`name: latest-json-macos`、`path: _updater/macos`、`continue-on-error: true` 容忍缺腿）（design D4）
- [x] `finalize` 合并与上传 step（`shell: bash`，`working-directory: ${{ github.workspace }}`，`env: GH_TOKEN / VERSION`）：内联 node 脚本读 `_updater/windows/latest.json`（必存在，缺则非零退出）与 `_updater/macos/latest.json`（存在才读），校验双腿 `version` 逐字一致，`merged = { ...base }` 且 `merged.platforms = { ...(mac?.platforms ?? {}), ...base.platforms }`，`JSON.stringify(merged, null, 2)` 写 `latest.json`（design D3）；随后 `gh release upload "desktop-v$VERSION" latest.json --clobber` 覆盖双腿遗留同名资产为权威版本（design D1/D3）

## 阶段四：文档与版本交付

- [x] 更新 `packages/desktop/RELEASE.md`：`每次发版` 产物核对清单补 mac 产物（`dev-team_<version>_aarch64.dmg`、`dev-team_<version>_aarch64.app.tar.gz` + `.sig`），并将 Windows 产物名 `desktop-terminal_X.Y.Z_x64-setup.exe` 校正为 `dev-team_<version>_x64-setup.exe`（productName 为 `dev-team`，design 勘误注记 1）
- [x] `packages/desktop/RELEASE.md` 新增/更新双平台说明：mac 首次安装 Gatekeeper 右键打开（无 OS 级签名的预期行为）；mac 权威升级路径 = 手动下载 dmg（updater 尽力而为、不承诺自动更新）；`约束与已知行为` 中「仅 Windows 分发」措辞改为双平台；Windows 更新链路说明（minisign 校验 / NSIS `/UPDATE` 静默安装 / 密钥管理）保持（design AC-8）
- [x] 修改 `packages/desktop/package.json`：`version` 0.4.29 → 0.4.30（design D8；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）

## 阶段五：守线收口（静态对账，不含测试执行）

- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（新增 `icon-source.png` / `icon.icns` / `icon.png` / `32x32.png` / `128x128.png` / `128x128@2x.png`；修改 workflow / tauri.conf.json / icon.ico / RELEASE.md / package.json），无清单外改动、无清单内遗漏
- [x] 双平台语义自查：grep 确认 mac 腿 env 不含 `APPLE_*`；Windows 腿 `args: --bundles nsis` 与签名 env 未回退；`bundle.icon` 五元列表在案；`plugins/dev-team` 零改动复核（版本 2.10.44 与三类交付产物未动）
- [x] 静态回归确认（零 Rust/TS 源码变更，预期绿）：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）与 `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零错误；`pnpm -C packages/desktop run bindings:check` 绿（本变更不触 bindings，属一致性守卫）
