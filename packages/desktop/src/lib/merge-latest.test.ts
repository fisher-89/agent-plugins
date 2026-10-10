/**
 * finalize `latest.json` 合并 + detect 版本比较 — workflow 内联脚本行为测试
 *
 * 被测逻辑是 `.github/workflows/desktop-release.yml` 里的两段内联 node 脚本
 * （finalize 的 heredoc 合并脚本 / detect 的 `node -e` 版本比较脚本）。为遵守
 * design D1「不新增仓库脚本文件」，本用例不在测试里复制一份算法，而是执行期
 * 从 YAML 原文提取脚本、以真实 node 子进程跑真实脚本（零逻辑复制、零源码改动）。
 *
 * 覆盖 test-design §3.1 / §3.2 与 AC-2（双平台条目 + Windows 条目逐字保留）、
 * AC-3（幂等 + 缺腿容忍）。CI 行为层（真实 release 产出）不在本文件范围。
 */

import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 仓库定位与脚本提取
// ---------------------------------------------------------------------------

const WORKFLOW_PATH = '.github/workflows/desktop-release.yml';

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
const workflow = fs.readFileSync(path.join(repoRoot, WORKFLOW_PATH), 'utf8');

/** 提取 finalize job 的 heredoc 合并脚本（`node - <<'NODE' … NODE`）。 */
function extractFinalizeMergeScript(): string {
  const match = /node - <<'NODE'\r?\n([\s\S]*?)\r?\n\s*NODE\r?\n/.exec(workflow);
  if (match === null) {
    throw new Error("未在 workflow 中提取到 finalize 合并脚本（node - <<'NODE' 形态已变更）");
  }
  return match[1];
}

/** 提取 detect job 的版本比较脚本（`node -e "…" "$VERSION" "$LATEST"`）。 */
function extractDetectVersionScript(): string {
  const match = /node -e "([\s\S]*?)"\s*"\$VERSION"\s*"\$LATEST"/.exec(workflow);
  if (match === null) {
    throw new Error('未在 workflow 中提取到 detect 版本比较脚本（node -e 形态已变更）');
  }
  return match[1];
}

// ---------------------------------------------------------------------------
// fixture 类型（= test-design §2.1 的 LatestJson 语义契约）
// ---------------------------------------------------------------------------

type PlatformId = 'windows-x86_64' | 'darwin-aarch64';

interface PlatformEntry {
  signature: string;
  url: string;
}

interface LatestJson {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Partial<Record<PlatformId, PlatformEntry>>;
}

/** Windows 腿产物（权威 base）：version / notes / pub_date 均取此腿。 */
function windowsBase(overrides: Partial<LatestJson> = {}): LatestJson {
  return {
    version: '0.4.30',
    notes: 'Windows 腿 release notes',
    pub_date: '2026-10-10T00:00:00Z',
    platforms: {
      'windows-x86_64': {
        signature: 'windows-minisign-sig',
        url: 'https://example.com/dev-team_0.4.30_x64-setup.exe',
      },
    },
    ...overrides,
  };
}

/** macOS 腿产物：notes / pub_date 与 base 刻意不同，用于验证权威字段归属。 */
function macLeg(overrides: Partial<LatestJson> = {}): LatestJson {
  return {
    version: '0.4.30',
    notes: 'macOS 腿 release notes',
    pub_date: '2026-10-10T00:00:07Z',
    platforms: {
      'darwin-aarch64': {
        signature: 'darwin-minisign-sig',
        url: 'https://example.com/dev-team_aarch64.app.tar.gz',
      },
    },
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// 合并脚本执行载体：临时目录（真盘写读，无 process.chdir）
// ---------------------------------------------------------------------------

const tempDirs: string[] = [];

function makeTempDir(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'merge-latest-'));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir !== undefined) fs.rmSync(dir, { recursive: true, force: true });
  }
});

interface MergeRun {
  status: number | null;
  stdout: string;
  stderr: string;
  /** 产出的权威 latest.json 原文；未产出为 null。 */
  merged: string | null;
}

/** 写入某腿工件；artifact 为 undefined 表示该腿工件缺失（缺腿场景）。 */
function writeLegArtifact(dir: string, leg: 'windows' | 'macos', artifact?: LatestJson): void {
  if (artifact === undefined) return;
  const legDir = path.join(dir, '_updater', leg);
  fs.mkdirSync(legDir, { recursive: true });
  fs.writeFileSync(path.join(legDir, 'latest.json'), JSON.stringify(artifact, null, 2));
}

/** 以真实 node 子进程执行 finalize 合并脚本（cwd = 临时工作区根）。 */
function runFinalizeMerge(fixtures: { windows?: LatestJson; macos?: LatestJson }): MergeRun {
  const dir = makeTempDir();
  const scriptPath = path.join(dir, 'finalize-merge.cjs');
  fs.writeFileSync(scriptPath, extractFinalizeMergeScript());
  writeLegArtifact(dir, 'windows', fixtures.windows);
  writeLegArtifact(dir, 'macos', fixtures.macos);

  const result = spawnSync(process.execPath, [scriptPath], { cwd: dir, encoding: 'utf8' });
  const mergedPath = path.join(dir, 'latest.json');
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
    merged: fs.existsSync(mergedPath) ? fs.readFileSync(mergedPath, 'utf8') : null,
  };
}

/** 断言合并成功并返回权威 latest.json（避免各用例重复空值分支）。 */
function mergedJson(run: MergeRun): LatestJson {
  expect(run.status).toBe(0);
  if (run.merged === null) throw new Error('finalize 成功退出但未产出 latest.json');
  return JSON.parse(run.merged) as LatestJson;
}

// ---------------------------------------------------------------------------
// detect 版本比较脚本执行载体
// ---------------------------------------------------------------------------

/** 复现 detect 的 SHOULD 判定：node 脚本 exit 0 = 更高（发版），1 = 不高。 */
function isVersionNewer(current: string, latest: string): boolean {
  const result = spawnSync(
    process.execPath,
    ['-e', extractDetectVersionScript(), current, latest],
    { encoding: 'utf8' },
  );
  if (result.status === 0) return true;
  if (result.status === 1) return false;
  throw new Error(`版本比较脚本异常退出（status=${String(result.status)}）: ${result.stderr}`);
}

// ===========================================================================
// finalize latest.json 合并（AC-2 / AC-3）
// ===========================================================================

describe('finalize latest.json 合并（workflow 内联脚本）', () => {
  it('正向: 提取到的合并脚本读双腿工件并写权威 latest.json（脚本形态守卫）', () => {
    const script = extractFinalizeMergeScript();

    expect(script).toContain('_updater/windows/latest.json');
    expect(script).toContain('_updater/macos/latest.json');
    expect(script).toContain("fs.writeFileSync('latest.json'");
  });

  it('正向: 双腿成功 → platforms 同时含 windows-x86_64 与 darwin-aarch64 条目（AC-2）', () => {
    const run = runFinalizeMerge({ windows: windowsBase(), macos: macLeg() });
    const merged = mergedJson(run);

    expect(Object.keys(merged.platforms).sort()).toEqual(['darwin-aarch64', 'windows-x86_64']);
    expect(merged.platforms['darwin-aarch64']).toEqual({
      signature: 'darwin-minisign-sig',
      url: 'https://example.com/dev-team_aarch64.app.tar.gz',
    });
    expect(run.stdout).toContain('darwin-aarch64');
  });

  it('正向: Windows 条目逐字保留（signature / url 与 base 完全一致，AC-2 回归红线）', () => {
    const base = windowsBase();
    const merged = mergedJson(runFinalizeMerge({ windows: base, macos: macLeg() }));

    expect(merged.platforms['windows-x86_64']).toEqual(base.platforms['windows-x86_64']);
    expect(merged.platforms['windows-x86_64']?.signature).toBe('windows-minisign-sig');
  });

  it('正向: 权威字段取 Windows base（mac 腿的 notes / pub_date 不覆盖）', () => {
    const base = windowsBase();
    const merged = mergedJson(runFinalizeMerge({ windows: base, macos: macLeg() }));

    expect(merged.version).toBe(base.version);
    expect(merged.notes).toBe(base.notes);
    expect(merged.pub_date).toBe(base.pub_date);
  });

  it('边界: mac 工件缺失（缺腿容忍）→ 仍产出仅含 windows-x86_64 的权威 latest.json（AC-3）', () => {
    const run = runFinalizeMerge({ windows: windowsBase() });
    const merged = mergedJson(run);

    expect(Object.keys(merged.platforms)).toEqual(['windows-x86_64']);
    expect(run.stderr).toBe('');
  });

  it('边界: mac 腿 platforms 为空对象 → 仍保留 base 的 windows-x86_64 条目', () => {
    const merged = mergedJson(
      runFinalizeMerge({ windows: windowsBase(), macos: macLeg({ platforms: {} }) }),
    );

    expect(Object.keys(merged.platforms)).toEqual(['windows-x86_64']);
  });

  it('边界: mac 腿意外带 windows-x86_64 键（force 重发残留）→ 取 base 值且键不重复', () => {
    const base = windowsBase();
    const merged = mergedJson(
      runFinalizeMerge({
        windows: base,
        macos: macLeg({
          platforms: {
            'windows-x86_64': { signature: '上一轮旧签名', url: 'https://example.com/旧包.exe' },
            'darwin-aarch64': { signature: 'darwin-minisign-sig', url: 'https://example.com/mac' },
          },
        }),
      }),
    );

    expect(Object.keys(merged.platforms).sort()).toEqual(['darwin-aarch64', 'windows-x86_64']);
    expect(merged.platforms['windows-x86_64']).toEqual(base.platforms['windows-x86_64']);
  });

  it('边界: 同一对工件重跑两次 → 结果逐字节一致、每平台恰一条目（幂等，AC-3）', () => {
    const first = runFinalizeMerge({ windows: windowsBase(), macos: macLeg() });
    const second = runFinalizeMerge({ windows: windowsBase(), macos: macLeg() });

    expect(second.merged).toBe(first.merged);
    const merged = mergedJson(second);
    expect(Object.keys(merged.platforms)).toHaveLength(2);
  });

  it('异常: Windows 工件缺失 → 非零退出、stderr 显式报错、不产出 latest.json（Windows 是红线）', () => {
    const run = runFinalizeMerge({ macos: macLeg() });

    expect(run.status).not.toBe(0);
    expect(run.merged).toBeNull();
    expect(run.stderr).toContain('缺少 Windows 腿');
  });

  it('异常: 双腿 version 不一致 → 非零退出、stderr 报明两腿版本、不产出 latest.json', () => {
    const run = runFinalizeMerge({
      windows: windowsBase(),
      macos: macLeg({ version: '0.4.31' }),
    });

    expect(run.status).not.toBe(0);
    expect(run.merged).toBeNull();
    expect(run.stderr).toContain('版本不一致');
    expect(run.stderr).toContain('0.4.30');
    expect(run.stderr).toContain('0.4.31');
  });
});

// ===========================================================================
// detect 版本比较（既有逻辑回归）
// ===========================================================================

describe('detect 版本比较（workflow 内联脚本）', () => {
  it('正向: 当前版本更高 → true（0.4.30 > 0.4.29）', () => {
    expect(isVersionNewer('0.4.30', '0.4.29')).toBe(true);
  });

  it('边界: 版本相同 → false（未升版不发版）', () => {
    expect(isVersionNewer('0.4.30', '0.4.30')).toBe(false);
  });

  it('边界: 当前版本更低 → false（回退不发版）', () => {
    expect(isVersionNewer('0.4.29', '0.4.30')).toBe(false);
  });

  it('边界: 首发无历史 release（LATEST 归 0.0.0）→ true', () => {
    expect(isVersionNewer('0.4.30', '0.0.0')).toBe(true);
  });

  it('边界: 位数不同的退化比较 → 按前 3 位逐位判定且不崩溃（0.4.30 vs 0.5 → false）', () => {
    expect(isVersionNewer('0.4.30', '0.5')).toBe(false);
    expect(isVersionNewer('0.5.0', '0.4.30')).toBe(true);
  });
});
