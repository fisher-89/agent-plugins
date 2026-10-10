/**
 * desktop 发布链路静态验证 — workflow 结构 / 打包配置 / 图标资产 / 发版文档 / 版本
 *
 * 本变更零 Rust/TS 应用源码，主体验收面是「已检入产物的结构对账」：直接读
 * `.github/workflows/desktop-release.yml`、`src-tauri/tauri.conf.json`、
 * `src-tauri/icons/**`、`RELEASE.md`、`package.json` 原文做解析 + 断言。
 *
 * 覆盖 test-design §4（AC-1 / AC-4～AC-9 的静态面）。finalize 合并算法的行为测试
 * 见 `merge-latest.test.ts`；CI 行为层（真实 release 产出）不在本文件范围。
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as zlib from 'node:zlib';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 仓库定位与读取
// ---------------------------------------------------------------------------

const WORKFLOW_PATH = '.github/workflows/desktop-release.yml';
const TAURI_CONF_PATH = 'packages/desktop/src-tauri/tauri.conf.json';
const ICONS_DIR = 'packages/desktop/src-tauri/icons';
const RELEASE_DOC_PATH = 'packages/desktop/RELEASE.md';
const DESKTOP_PACKAGE_PATH = 'packages/desktop/package.json';

/** 自测试文件位置向上定位仓库根（含 workflow 的最近祖先目录）。 */
function findRepoRoot(): string {
  let dir = path.dirname(fileURLToPath(import.meta.url));
  for (;;) {
    if (fs.existsSync(path.join(dir, WORKFLOW_PATH))) return dir;
    const parent = path.dirname(dir);
    if (parent === dir) throw new Error(`未找到仓库根（缺少 ${WORKFLOW_PATH}）`);
    dir = parent;
  }
}

const repoRoot = findRepoRoot();

function readText(relativePath: string): string {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function readBytes(relativePath: string): Buffer {
  return fs.readFileSync(path.join(repoRoot, relativePath));
}

const workflow = readText(WORKFLOW_PATH);
const releaseDoc = readText(RELEASE_DOC_PATH);

/** 取 `jobs:` 下某个顶层 job 的原文块，用于按 job 而非全文断言。 */
function jobBlock(name: string): string {
  const lines = workflow.split('\n');
  const start = lines.findIndex((line) => line === `  ${name}:`);
  if (start === -1) throw new Error(`workflow 中找不到 job: ${name}`);
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => /^ {2}\S/.test(line));
  return [lines[start], ...(end === -1 ? rest : rest.slice(0, end))].join('\n');
}

const detectJob = jobBlock('detect');
const buildJob = jobBlock('build');
const finalizeJob = jobBlock('finalize');

// ---------------------------------------------------------------------------
// 打包配置解析
// ---------------------------------------------------------------------------

interface TauriConfig {
  productName: string;
  version: string;
  bundle: {
    targets: string;
    icon: string[];
    createUpdaterArtifacts: boolean;
  };
  plugins: {
    updater: {
      endpoints: string[];
      pubkey: string;
    };
  };
}

const tauriConfig = JSON.parse(readText(TAURI_CONF_PATH)) as TauriConfig;

interface DesktopPackage {
  version: string;
}

const desktopPackage = JSON.parse(readText(DESKTOP_PACKAGE_PATH)) as DesktopPackage;

// ---------------------------------------------------------------------------
// PNG / ICO / ICNS 头解析（仅解析自研产物的容器头与像素，不涉库语义）
// ---------------------------------------------------------------------------

interface PngHeader {
  width: number;
  height: number;
  bitDepth: number;
  colorType: number;
}

function readPngHeader(fileName: string): PngHeader {
  const bytes = readBytes(`${ICONS_DIR}/${fileName}`);
  expect(bytes.subarray(0, 8)).toEqual(
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  );
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
    bitDepth: bytes.readUInt8(24),
    colorType: bytes.readUInt8(25),
  };
}

/** 收集 IDAT 数据并 inflate（源图为零 filter 的裸 RGB 行）。 */
function inflatePngScanlines(fileName: string): Buffer {
  const bytes = readBytes(`${ICONS_DIR}/${fileName}`);
  const chunks: Buffer[] = [];
  for (let offset = 8; offset + 12 <= bytes.length;) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.subarray(offset + 4, offset + 8).toString('latin1');
    if (type === 'IDAT') chunks.push(bytes.subarray(offset + 8, offset + 8 + length));
    offset += 12 + length;
  }
  return zlib.inflateSync(Buffer.concat(chunks));
}

/** 逐位比较前 3 位版本号：a > b → 1，相等 → 0，a < b → -1。 */
function compareSemver(a: string, b: string): number {
  const [left, right] = [a, b].map((value) => value.split('.').map(Number));
  for (let i = 0; i < 3; i++) {
    const [x, y] = [left[i] ?? 0, right[i] ?? 0];
    if (x !== y) return x > y ? 1 : -1;
  }
  return 0;
}

// ===========================================================================
// workflow 三段结构（AC-4 / AC-5 / AC-6）
// ===========================================================================

describe('desktop-release workflow 结构与 Windows 腿守线', () => {
  it('正向: detect 单点创建 tag（ls-remote 判存 + tag/push），且受 should gate 约束（AC-4）', () => {
    expect(detectJob).toContain('name: 创建 tag(已存在则跳过)');
    expect(detectJob).toContain("if: steps.version.outputs.should == 'true'");
    expect(detectJob).toContain('git ls-remote --exit-code origin "refs/tags/desktop-v$VERSION"');
    expect(detectJob).toContain('git tag "desktop-v$VERSION"');
    expect(detectJob).toContain('git push origin "refs/tags/desktop-v$VERSION"');
  });

  it('正向: build 由 detect 单点 gate，且未残留 tag 创建（前移回归，AC-4）', () => {
    expect(buildJob).toContain('needs: detect');
    expect(buildJob).toContain("if: needs.detect.outputs.should == 'true'");
    expect(buildJob).not.toContain('git tag');
    expect(buildJob).not.toContain('git push');
  });

  it('正向: build matrix 双腿（windows-latest/nsis、macos-latest/app,dmg）与工件名齐备（AC-1）', () => {
    expect(buildJob).toContain('fail-fast: false');
    expect(buildJob).toMatch(
      /- platform: windows-latest\s*\n\s+args: --bundles nsis\s*\n\s+artifact: latest-json-windows/,
    );
    expect(buildJob).toMatch(
      /- platform: macos-latest\s*\n\s+args: --bundles app,dmg\s*\n\s+artifact: latest-json-macos/,
    );
    expect(buildJob).toContain('runs-on: ${{ matrix.platform }}');
  });

  it('正向: Windows 腿构建参数与签名 env 零回退（AC-5）', () => {
    expect(buildJob).toContain('args: ${{ matrix.args }}');
    expect(buildJob).toContain(
      'TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}',
    );
    expect(buildJob).toContain("TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ''");
    expect(buildJob).toContain('GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}');
  });

  it('正向: macOS 腿不配任何 Apple 签名 secret（AC-6）', () => {
    expect(buildJob).not.toMatch(/APPLE_/);
    expect(workflow).not.toMatch(/APPLE_/);
  });

  it('正向: tauri-action 发布语义不变（projectPath/tagName/releaseName/releaseDraft/prerelease）', () => {
    expect(buildJob).toContain('uses: tauri-apps/tauri-action@v0');
    expect(buildJob).toContain('projectPath: packages/desktop');
    expect(buildJob).toContain('tagName: desktop-v${{ needs.detect.outputs.version }}');
    expect(buildJob).toContain("releaseName: 'Dev Team v${{ needs.detect.outputs.version }}'");
    expect(buildJob).toContain('releaseDraft: false');
    expect(buildJob).toContain('prerelease: false');
  });

  it('正向: 双腿捕获本腿 latest.json 工件（缺失即失败 + if: success()）（AC-2/AC-3）', () => {
    expect(buildJob).toContain('name: 捕获本腿 latest.json(缺失即失败)');
    expect(buildJob).toContain('if [ ! -f latest.json ]');
    expect(buildJob).toContain('uses: actions/upload-artifact@v4');
    expect(buildJob).toContain('name: ${{ matrix.artifact }}');
    expect(buildJob).toContain('path: latest.json');
    expect(buildJob).toContain('if-no-files-found: error');
    expect(buildJob).toContain('if: success()');
  });

  it('正向: finalize 门为 should && (success() || failure())，缺腿也照常合并（AC-3）', () => {
    expect(finalizeJob).toContain('needs: [detect, build]');
    expect(finalizeJob).toContain(
      "if: needs.detect.outputs.should == 'true' && (success() || failure())",
    );
  });

  it('正向: Windows 工件下载不 continue-on-error，mac 工件下载 continue-on-error（AC-3）', () => {
    expect(finalizeJob).toMatch(
      /- uses: actions\/download-artifact@v4\s*\n\s+with:\s*\n\s+name: latest-json-windows/,
    );
    expect(finalizeJob).toMatch(
      /- uses: actions\/download-artifact@v4\s*\n\s+continue-on-error: true\s*\n\s+with:\s*\n\s+name: latest-json-macos/,
    );
    expect(finalizeJob.match(/continue-on-error/g)).toHaveLength(1);
  });

  it('正向: finalize 以 gh release upload --clobber 回传权威 latest.json', () => {
    expect(finalizeJob).toContain('gh release upload "desktop-v$VERSION" latest.json --clobber');
  });

  it('正向: 发布权限 / 并发 / 工作目录 / 触发面保持', () => {
    expect(workflow).toContain('permissions:\n  contents: write');
    expect(workflow).toContain('concurrency:\n  group: desktop-release');
    expect(workflow).toContain('working-directory: packages/desktop');
    expect(workflow).toContain(
      "paths: ['packages/desktop/**', '.github/workflows/desktop-release.yml']",
    );
    expect(workflow).toContain('workflow_dispatch:');
  });
});

// ===========================================================================
// 打包配置（AC-7 / AC-1）
// ===========================================================================

describe('tauri 打包配置', () => {
  it('正向: bundle.icon 逐字等于桌面全套（ico / icns / 多尺寸 png，AC-7）', () => {
    expect(tauriConfig.bundle.icon).toEqual([
      'icons/32x32.png',
      'icons/128x128.png',
      'icons/128x128@2x.png',
      'icons/icon.icns',
      'icons/icon.ico',
    ]);
  });

  it('正向: 版本单源与 updater 打包开关未动（AC-1）', () => {
    expect(tauriConfig.productName).toBe('dev-team');
    expect(tauriConfig.version).toBe('../package.json');
    expect(tauriConfig.bundle.targets).toBe('all');
    expect(tauriConfig.bundle.createUpdaterArtifacts).toBe(true);
  });

  it('正向: updater 端点与公钥未动（latest.json 权威来源）', () => {
    expect(tauriConfig.plugins.updater.endpoints).toEqual([
      'https://github.com/fisher-89/agent-plugins/releases/latest/download/latest.json',
    ]);
    // minisign 公钥以 "untrusted comment:" 的 base64 头开头
    expect(tauriConfig.plugins.updater.pubkey.startsWith('dW50cnVzdGVk')).toBe(true);
  });
});

// ===========================================================================
// 图标资产（AC-7）
// ===========================================================================

describe('图标资产成套', () => {
  it('正向: 桌面消费的图标文件全部在案', () => {
    for (const fileName of [
      'icon.ico',
      'icon.icns',
      'icon.png',
      '32x32.png',
      '128x128.png',
      '128x128@2x.png',
      'icon-source.png',
    ]) {
      expect(fs.existsSync(path.join(repoRoot, ICONS_DIR, fileName)), fileName).toBe(true);
    }
  });

  it('正向: ico / icns 容器魔数正确（Windows / macOS 打包可消费）', () => {
    expect(readBytes(`${ICONS_DIR}/icon.ico`).subarray(0, 4).toString('hex')).toBe('00000100');
    expect(readBytes(`${ICONS_DIR}/icon.icns`).subarray(0, 4).toString('latin1')).toBe('icns');
  });

  it('正向: png 尺寸符合 Tauri 约定（32 / 128 / 256 / 512）', () => {
    expect(readPngHeader('32x32.png')).toMatchObject({ width: 32, height: 32, bitDepth: 8 });
    expect(readPngHeader('128x128.png')).toMatchObject({ width: 128, height: 128, bitDepth: 8 });
    expect(readPngHeader('128x128@2x.png')).toMatchObject({ width: 256, height: 256, bitDepth: 8 });
    expect(readPngHeader('icon.png')).toMatchObject({ width: 512, height: 512, bitDepth: 8 });
  });

  it('正向: 源图 1024×1024 8-bit 真彩、纯色 #2563eb（可重跑 tauri icon）', () => {
    const header = readPngHeader('icon-source.png');
    expect(header).toMatchObject({ width: 1024, height: 1024, bitDepth: 8, colorType: 2 });

    const scanlines = inflatePngScanlines('icon-source.png');
    // 1024 行 × (1 字节 filter + 1024 像素 × 3 字节 RGB)
    expect(scanlines.length).toBe(1024 * (1 + 1024 * 3));
    expect(scanlines.readUInt8(0)).toBe(0); // 零 filter，裸 RGB
    expect([scanlines.readUInt8(1), scanlines.readUInt8(2), scanlines.readUInt8(3)]).toEqual([
      0x25, 0x63, 0xeb,
    ]);
  });
});

// ===========================================================================
// 发版文档（AC-8）
// ===========================================================================

describe('RELEASE.md 双平台文档', () => {
  it('正向: macOS 首次安装的 Gatekeeper 右键放行说明在案', () => {
    expect(releaseDoc).toContain('Gatekeeper');
    expect(releaseDoc).toContain('右键');
  });

  it('正向: macOS 权威升级路径 = 手动下载 dmg，应用内更新不承诺', () => {
    expect(releaseDoc).toMatch(/手动下载(新版 )?dmg/);
    expect(releaseDoc).toContain('不承诺');
  });

  it('正向: Windows 更新链路说明保持（minisign / NSIS /UPDATE / 密钥管理）', () => {
    expect(releaseDoc).toContain('minisign');
    expect(releaseDoc).toContain('/UPDATE');
    expect(releaseDoc).toContain('TAURI_SIGNING_PRIVATE_KEY');
    expect(releaseDoc).toContain('私钥必须离机备份');
  });

  it('正向: 分发口径为双平台，且 Windows 产物名已校正（无旧名残留）', () => {
    expect(releaseDoc).toContain('dev-team_X.Y.Z_x64-setup.exe');
    expect(releaseDoc).toContain('dev-team_X.Y.Z_aarch64.dmg');
    expect(releaseDoc).not.toMatch(/desktop-terminal_[0-9.]+/);
    expect(releaseDoc).not.toMatch(/仅 Windows 分发/);
  });
});

// ===========================================================================
// 版本交付（AC-9）
// ===========================================================================

describe('版本交付', () => {
  it('正向: package.json 为合法 semver 且不低于本变更交付的 0.4.30', () => {
    // 变更交付值为 0.4.30；后续发版只会前进，故断言「形态合法 + 不低于」以保持长期可用。
    expect(desktopPackage.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(compareSemver(desktopPackage.version, '0.4.30')).toBeGreaterThanOrEqual(0);
  });
});
