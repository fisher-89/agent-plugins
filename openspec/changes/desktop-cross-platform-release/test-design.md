# 测试设计: desktop-cross-platform-release

> **变更**: desktop-cross-platform-release
> **日期**: 2026-10-10
> **基于**: proposal.md, design.md（D1–D8 决策编号见 design）

---

## 0. 测试范围结论（先行）

本变更的变更面为「改 2 文件（workflow / tauri.conf.json）+ 补 1 套图标 + 改 1 文档 + 版本 bump」，**零 Rust/TS 应用源码、零新增仓库脚本模块**（design「公共函数 / API」「类型定义」两子节均明确省略）。因此：

1. **不存在传统应用单元测试面**：`packages/desktop` 的 `vp test` / `client:check` / `server:check` 均不覆盖 CI workflow、tauri 打包配置、图标资产与 Markdown 文档（proposal「测试文件」已定稿为「无」）。
2. **唯一承载真实风险逻辑的单元**是 workflow 内联脚本中的 **latest.json 合并算法**（finalize，D3），对应 R1/R2/R8 与 AC-2/AC-3。其当前形态为 heredoc 内联 node，无模块边界、无导出，故按现状**不可直接单元测试**。
3. 因此本设计的测试分层为：**① 纯逻辑单元测试（仅 latest.json 合并 + detect 版本比较两个纯函数，需最小提取才可单测）→ ② 静态验证（workflow 结构 / 配置 / 图标 / 文档 / 版本，占 AC 主体）→ ③ CI 验收（release 实际产出 dmg 与权威 latest.json，为 proposal 定稿的验收线）**。

> 关键权衡（需 design 侧签字）：合并算法若要获得确定性单元覆盖，需把纯函数从 heredoc 提取为一个**被 finalize job 实际 invoke 的模块**（导出为生产入口而非 test-only 导出，符合 AGENT.md「不得仅为测试加 export」约束），但这会新增 1 个仓库脚本文件，打破 D1「不新增仓库脚本文件」与 proposal 实现文件清单。默认方案（本设计推荐）为**不提取**：合并算法通过「静态语义对账 + CI 验收行为验证」覆盖；提取方案作为可选加固，签名与用例见 §2/§3，实施前须回写 design D1。

---

## 1. 验收范围（AC → 测试类型 → 测试对象）

| AC ID | 验收条件 | 测试类型 | 测试对象 / 验证手段 |
|--------|---------|---------|----------|
| AC-1 | macOS 产物（`*_aarch64.dmg` + `.app.tar.gz` + `.sig`，版本与 package.json 一致） | CI 验收 + 静态验证 | `build` matrix mac 腿 `args: --bundles dmg`、`createUpdaterArtifacts: true` 未动（静态）；release 实际产出 dmg（验收） |
| AC-2 | latest.json 双平台条目；Windows 条目形态与现状一致（回归红线） | 单元测试（合并算法）+ CI 验收 | `mergeLatestJson` 双腿成功分支（单测）；`releases/latest/download/latest.json` 含双条目（验收） |
| AC-3 | 合并幂等 + 容忍缺腿（mac 腿失败仍产出含现存平台） | 单元测试（合并算法）+ CI 验收 | `mergeLatestJson` 幂等 / 缺腿分支（单测）；force 重发与缺腿场景（验收） |
| AC-4 | tag 前移（detect 单点创建，双腿无竞态） | 静态验证 + CI 验收 | detect 内存在「创建 tag（已存在则跳过）」step、`build`/`release` 内无 tag 创建（静态）；一次发版仅一个 tag（验收） |
| AC-5 | Windows 腿不回退（`--bundles nsis` / 签名 env / `releaseDraft`/`prerelease` 等） | 静态验证 | workflow YAML：Windows 腿 `args`、`TAURI_SIGNING_*` env、tauri-action `with` 参数逐项比对 |
| AC-6 | mac 无 `APPLE_*` env 可产出 | 静态验证 | workflow YAML：mac 腿 env 只含 `GITHUB_TOKEN` + `TAURI_SIGNING_*`，无任何 `APPLE_*` |
| AC-7 | 图标成套（ico / icns / 多尺寸 png，`bundle.icon` 列全套） | 静态验证 | `src-tauri/icons/` 文件存在性 + `tauri.conf.json` `bundle.icon` 五元列表逐字对齐 |
| AC-8 | RELEASE.md 双平台文档（右键打开 + 手动升级，Windows 链路保持） | 静态验证 | RELEASE.md 关键字 grep + 产物名校正核对 |
| AC-9 | 版本 0.4.30 + `plugins/dev-team` 零改动 + 零 Rust/TS 回归 | 静态验证 + 回归确认 | `package.json` version、`plugins/dev-team` 版本与交付产物；`server:check`/`client:check`/`bindings:check` 预期全绿（回归确认，非本变更新增用例） |

---

## 2. 公共 API 签名（唯一可单测逻辑单元）

> design「公共函数 / API」子节因零源码变更而省略；以下签名是对 workflow 内联脚本中**唯一承载业务风险**的两个纯逻辑单元的推导。当前二者均为 heredoc 内联、无导出；若按 §0 可选加固路径提取，签名即生产模块公共 API（非 test-only 导出）；若不提取，则作为「逻辑语义契约」供静态对账与行为验收参照。

### 2.1 `mergeLatestJson`（finalize 合并，D3 / spec「latest.json 权威合并」）

```ts
// 目标模块（若提取）：packages/desktop/scripts/merge-latest.mjs
export type PlatformId = "windows-x86_64" | "darwin-aarch64";
export interface PlatformEntry { signature: string; url: string; }
export interface LatestJson {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Partial<Record<PlatformId, PlatformEntry>>;
}
export type MergeOutcome =
  | { ok: true; merged: LatestJson }
  | { ok: false; error: "missing-windows-base" | "version-mismatch" };

export function mergeLatestJson(
  base: LatestJson,          // Windows 腿产物，必存在（缺则 missing-windows-base）
  mac: LatestJson | null     // mac 腿产物，缺腿时为 null（容忍）
): MergeOutcome;
```

**语义契约（= design D3 逐条）**：

- **base 必存在**：`base` 为 Windows 腿 `latest.json`；缺（工件未下载到）→ `{ ok: false, error: "missing-windows-base" }`，调用方非零退出（Windows 是红线）。
- **缺腿容忍**：`mac === null` → 不校验版本、不报错，`merged = { ...base }`，`platforms` 仅含 Windows 条目。
- **版本逐字校验**：`mac !== null && mac.version !== base.version` → `{ ok: false, error: "version-mismatch" }`。
- **键合并**：`merged = { ...base }`；`merged.platforms = { ...(mac?.platforms ?? {}), ...base.platforms }`。base 后展开 → `windows-x86_64` 条目逐字保留（AC-2 红线），`darwin-aarch64` 条目补入。
- **幂等**：`platforms` 为键控对象（`windows-x86_64` / `darwin-aarch64` 键唯一），同键覆盖不产生重复条目（AC-3）。
- **version / notes / pub_date 取 base**（Windows 权威）。

### 2.2 `isVersionNewer`（detect 版本比较，既有逻辑，未改动）

```ts
// detect job 内联 node（现状既有，本变更不修改，仅作三段契约的一部分纳入对账）
export function isVersionNewer(current: string, latest: string): boolean;
```

**语义契约（= 现状 node 内联脚本逐条）**：

- 输入形如 `"0.4.30"`；`split('.').map(Number)` 后逐位比较**前 3 位**。
- 严格大于返回 `true`；相等或更低（含位数不同的退化比较）返回 `false`。
- `detect` 中：`SHOULD = force==='true' || isVersionNewer(version, latest)`。

### 2.3 tag 存在性判定（D5，非纯函数，不纳入单元测试）

`detect` 内「创建 tag（已存在则跳过）」step 的判定为 `git ls-remote --exit-code origin "refs/tags/desktop-v${VERSION}"`（存在→跳过，不存在→创建+push）。属 git 外部命令行为，非纯函数，仅作静态存在性核对（§4 AC-4），不设单元测试。

---

## 3. 单元测试（纯逻辑）

### 3.1 用例（`mergeLatestJson`，AC-2 / AC-3）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `merge-latest.test.ts` | `mergeLatestJson` -- 双腿成功合并 | 正向 | `base` 含 `windows-x86_64`、`mac` 含 `darwin-aarch64` 且版本一致 → 返回 `ok: true`，`merged.platforms` 同时含两条目（AC-2） | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- Windows 条目形态保持 | 正向 | 合并后 `merged.platforms["windows-x86_64"]` 与 `base` 中该条目**逐字段引用相等**（`signature` / `url` 不变，AC-2 红线） | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 权威字段取 base | 正向 | `merged.version` / `notes` / `pub_date` 与 `base` 完全一致（即使 mac 的 pub_date 不同） | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 缺腿容忍 | 边界 | `mac === null` → `ok: true`，`merged.platforms` 仅含 `windows-x86_64`，不报错（AC-3） | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 缺 base | 异常 | `base` 缺失（调用方以 null/undefined 传入）→ `ok: false, error: "missing-windows-base"` | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 版本不一致 | 异常 | `mac.version !== base.version` → `ok: false, error: "version-mismatch"` | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 幂等（同版本重发） | 边界 | 对同一对 `(base, mac)` 调用两次 → 结果逐字段一致，`platforms` 每平台恰一条目，无重复键（AC-3） | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 平台键覆盖不重复 | 边界 | `mac.platforms` 意外含 `windows-x86_64` 键时，合并后取 base 值（后展开覆盖），`Object.keys(merged.platforms)` 无重复 | 新增 |
| `merge-latest.test.ts` | `mergeLatestJson` -- 空 platforms | 边界 | `mac` 存在但 `mac.platforms` 为空对象 → 合并后仍含 base 的 Windows 条目，不报错 | 新增 |

### 3.2 用例（`isVersionNewer`，既有逻辑回归）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `merge-latest.test.ts`（或独立 detect 测试） | `isVersionNewer` -- 更高 | 正向 | `("0.4.30", "0.4.29")` → `true` | 新增 |
| `merge-latest.test.ts` | `isVersionNewer` -- 相等 | 边界 | `("0.4.30", "0.4.30")` → `false` | 新增 |
| `merge-latest.test.ts` | `isVersionNewer` -- 更低 | 边界 | `("0.4.29", "0.4.30")` → `false` | 新增 |
| `merge-latest.test.ts` | `isVersionNewer` -- 首发归空（latest 0.0.0） | 边界 | `("0.4.30", "0.0.0")` → `true` | 新增 |
| `merge-latest.test.ts` | `isVersionNewer` -- 前导/多位退化 | 边界 | `("0.4.30", "0.5")` → 按前 3 位逐位比较的确定性结果（非 NaN 崩溃） | 新增 |

### 3.3 Mock 策略（单元）

| 测试对象 | Mock 主体 | 方案 |
|---------|----------|------|
| `mergeLatestJson` | 无 | 纯函数，输入为内存构造的 `LatestJson` 对象，无外部依赖，无需 mock |
| `isVersionNewer` | 无 | 纯函数，输入为字符串，无需 mock |

> 注：若采纳 §0 提取方案，`merge-latest.mjs` 的导出即生产入口（finalize job 直接 invoke），不属于「仅为测试加 export」；若维持 heredoc 内联，则本节约降级为「语义契约对账清单」，由 test-execution 阶段以等价内联脚本行为验证替代（存在逻辑漂移风险，故提取为推荐加固项）。

---

## 4. 静态验证（workflow / 配置 / 图标 / 文档 / 版本）

> 本层无运行时测试框架，采用「文件解析 + grep + 逐项对账」的结构/内容断言（沿用 mcp-hierarchical-commands 等 CI/重命名类变更的静态验证先例）。

| AC ID | 验证对象 | 验证手段 | 断言要点 |
|--------|---------|----------|----------|
| AC-4 | `.github/workflows/desktop-release.yml` `detect` job | grep / 结构对账 | 存在「创建 tag（已存在则跳过）」step，含 `git ls-remote --exit-code origin "refs/tags/desktop-v$VERSION"` 判存与 `git push origin "refs/tags/desktop-v$VERSION"` |
| AC-4 | workflow `build` job | 结构对账 | `build`（原 `release`）内**无** tag 创建 step（已前移 detect）；`needs: detect`、`if: needs.detect.outputs.should == 'true'` |
| AC-5 | workflow `build` matrix Windows 腿 | YAML 解析 + 逐项比对 | `args: --bundles nsis`；env 含 `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ''`；`releaseDraft: false` / `prerelease: false` / `projectPath` / `tagName` / `releaseName` 语义不变 |
| AC-6 | workflow `build` matrix mac 腿 | grep | mac 腿 env 不含任何 `APPLE_*`；`args: --bundles dmg`；`runs-on: macos-latest` |
| AC-2/AC-3 | workflow `finalize` job | 结构对账 | `needs: [detect, build]`、`if: ... && (success() \|\| failure())`；Windows 工件下载不 `continue-on-error`、mac 工件下载 `continue-on-error: true`；合并 node 脚本语义与 §2.1 契约一致；`gh release upload ... --clobber` |
| AC-7 | `src-tauri/icons/` | 文件存在性 | 含 `icon.ico` / `icon.icns` / `icon.png` / `32x32.png` / `128x128.png` / `128x128@2x.png` / `icon-source.png` |
| AC-7 | `icon-source.png` | PNG 头解析 | 魔数正确、尺寸 1024×1024、8-bit RGB 纯色 `#2563eb`（不透明） |
| AC-7 | `tauri.conf.json` | JSON 解析 | `bundle.icon` 逐字等于 `["icons/32x32.png","icons/128x128.png","icons/128x128@2x.png","icons/icon.icns","icons/icon.ico"]`；`version`/`targets`/`createUpdaterArtifacts`/`plugins.updater` 未动 |
| AC-8 | `packages/desktop/RELEASE.md` | grep + 对账 | 含 mac 首次安装 Gatekeeper 右键打开说明；含「mac 权威升级 = 手动下载 dmg」；Windows 更新链路（minisign / NSIS `/UPDATE` / 密钥管理）未丢失；Windows 产物名已校正为 `dev-team_<version>_x64-setup.exe`（无 `desktop-terminal_…` 残留） |
| AC-9 | `packages/desktop/package.json` | JSON 解析 | `version === "0.4.30"` |
| AC-9 | `plugins/dev-team/**` | git diff 对账 | 零改动（版本 2.10.44、三类交付产物未动） |

---

## 5. CI 验收（行为层，proposal 定稿的验收线）

> 本层不产生仓库内测试文件；以「一次真实（或 force）发版的实际 release 产出」为验收判定，由 test-execution / 验收阶段执行。

| AC ID | 验收场景 | 判定标准 |
|--------|---------|----------|
| AC-1 | mac 腿构建成功 | release 资产含 `dev-team_<version>_aarch64.dmg`、`dev-team_<version>_aarch64.app.tar.gz` + `.sig`，版本 = package.json |
| AC-2 | 双腿成功 | `releases/latest/download/latest.json` 同时含 `windows-x86_64` 与 `darwin-aarch64`，Windows 条目 `signature`/`url`/`notes` 与历史单平台产物形态一致 |
| AC-3 | force 重发同版本 | 重发后 latest.json 每平台恰一条目，无重复键；mac 腿失败（或跳过）时仍产出含 Windows 条目的权威 latest.json |
| AC-4 | 一次发版 | 远端仅出现一个 `desktop-v<version>` tag；双腿并行未产生 release 创建竞态（或按 D5 备选 detect 预建 release 规避） |
| AC-6 | mac 无 `APPLE_*` | dmg 与 updater 工件正常产出，无 OS 级签名（二进制带 ad-hoc 签名） |

---

## 6. 测试策略

### 6.1 方法

本变更零 Rust/TS 源码，**不新增应用单元测试**（proposal 已定稿「测试文件：无」），采用「纯逻辑单测（可选加固）+ 静态结构/内容对账（主体）+ CI 行为验收（验收线）」三层：

1. **纯逻辑单元测试（可选）**：仅针对 latest.json 合并与版本比较两个纯函数（§2/§3）。默认不提取、不单测（尊重 D1）；提取则覆盖 AC-2/AC-3 的确定性分支。
2. **静态验证（主体）**：workflow YAML 结构、tauri.conf.json、图标资产、RELEASE.md、package.json 的结构/内容断言，覆盖 AC-4～AC-9 与 AC-1/AC-6 的静态部分。
3. **CI 验收（红线）**：release 实际产出 dmg + 权威 latest.json，覆盖 AC-1～AC-4、AC-6 的行为部分（与 proposal「验收以 CI 产出为准」一致）。

### 6.2 测试分类

- **纯逻辑单元测试**：内存构造对象，无文件系统 / 网络依赖。
- **静态验证**：对已检入文件做解析 + grep，无运行时依赖（`git ls-remote` / `gh` 等仅在 CI 验收阶段出现，不属静态验证）。
- **CI 验收**：依赖 GitHub 托管 runner 与 `fisher-89/agent-plugins` Releases，非本变更可本地复现的确定性测试。

### 6.3 模拟策略

| 层 | 模拟对象 | 方案 |
|----|---------|------|
| 纯逻辑单测 | 无 | 全内存，无需 mock |
| 静态验证 | 无 | 直接读工作区文件，无需 mock |
| CI 验收 | tauri-action / GitHub Releases / 签名 env | 不 mock，走真实 CI；force 重发与缺腿场景以「人为跳过 mac 腿 / 重复 dispatch」构造 |

---

## 7. 边界场景

| 场景 | 输入 / 条件 | 预期行为 | 归属 |
|------|------------|----------|------|
| mac 腿失败 / 未产出工件 | `mac === null`（download-artifact mac 缺） | finalize 仍合并 Windows-only latest.json 并上传，不因缺腿失败（AC-3） | 单元 + CI 验收 |
| Windows 腿失败 / 工件缺 | `base` 缺失 | finalize 非零退出，显式暴露（Windows 是红线） | 单元 + 静态 |
| 双腿 `version` 逐字不一致 | `mac.version !== base.version` | 合并非零退出，不产出错误 latest.json | 单元 |
| force 重发同版本 | 双腿再次产出同键条目 | `platforms` 键覆盖、无重复（幂等） | 单元 + CI 验收 |
| 首发无历史 release | detect `LATEST` 归 `0.0.0` | `should=true`，正常发版 | 单元（isVersionNewer）+ CI 验收 |
| tag 已存在 | `git ls-remote --exit-code` 命中 | 跳过创建，双腿复用既有 release 只上传 | 静态 + CI 验收 |
| `mac.platforms` 为空对象 | mac 存在但无平台条目 | 合并后仍含 Windows 条目，不报错 | 单元 |
| `mac.platforms` 意外含 Windows 键 | 键冲突 | 合并后取 base（后展开覆盖），无重复键 | 单元 |
| RELEASE.md 残留旧产物名 | 含 `desktop-terminal_X.Y.Z_x64-setup.exe` | 静态对账失败（应已校正为 `dev-team_<version>_x64-setup.exe`） | 静态 |

---

## 8. 不可测试项

- **CI workflow 的运行时正确性（三段 job 依赖 / gate / concurrency）** —— 原因：GitHub Actions 的 job 编排与 `needs`/`if` gate 无本地可复现的确定性测试手段，只能靠「一次真实发版」的 CI 验收验证；静态层仅能断言结构存在与关键参数，不能断言运行时调度。
- **tauri-action 对 matrix + updater 的 `latest.json` 合并 / 「release 已存在则只上传」行为** —— 原因：为第三方 action 的内部行为（design 待决问题 1/2），不在本变更代码面内，属实现期验证项；其结果决定 finalize 是「自行合并」（主路径）还是「退化为校验」，本测试设计按主路径签名（§2.1）撰写，退化为校验时对应断言相应降级。
- **mac 无 `APPLE_*` 时 updater 工件能否产出 / ad-hoc 签名的可运行性** —— 原因：依赖 tauri bundler 对无签名 env 的行为与 Apple 平台工具链，属实现期验证项（design 待决问题 3）；仅以 CI 验收的「产出 dmg 即验收」判定，不做 mac 运行时冒烟。
- **图标视觉观感 / icns 在 dmg·app 上的实际落位** —— 原因：R7 已拍板「先用纯色源、观感变化接受」；AC-7 只断言文件在案与 `bundle.icon` 列表，不验收视觉与打包后落位（design 待决问题 4）。
- **`tauri icon` 生成的二进制资产（ico/icns/png）逐字节确定性** —— 原因：`tauri icon` 为第三方 CLI，产物二进制不可在单测中逐字节预言；以「文件存在 + 源图可重跑一致（AC-7 Scenario 源图可再生成）」的流程性断言代替逐字节比对。
- **RELEASE.md 的语义可读性与版本历史措辞** —— 原因：文档正确性以关键字与产物名对账（AC-8）为准，不设自然语言语义断言；design 勘误注记 1 明确不扩及文档其余历史措辞。
- **Windows 更新链路的端到端（minisign 校验 / NSIS `/UPDATE` 静默安装）** —— 原因：属既有单平台链路，本变更零改应用侧（proposal「引用沿用」），以 AC-2「Windows 条目形态不变」间接保障，不做端到端重测。
