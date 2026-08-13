/**
 * 单元测试: lib/test-parser/stryker-config — reportDir/mutation.json
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';

import { resolveStrykerConfig } from './stryker-config';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'stryker-config-'));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function absPosix(...parts: string[]): string {
  return path.resolve(...parts).replace(/\\/g, '/');
}

function relPosix(root: string, ...parts: string[]): string {
  return path.relative(root, path.resolve(root, ...parts)).replace(/\\/g, '/');
}

const PLAN_ROOT = 'src';

describe('resolveStrykerConfig', () => {
  let project: TempProject;
  let reportDir: string;

  beforeEach(() => {
    project = createTempProject();
    reportDir = path.join(project.root, 'reports', 'test', 'vitest');
    fs.mkdirSync(reportDir, { recursive: true });
  });

  afterEach(() => {
    project.cleanup();
  });

  it('传入 reportDir 后临时配置中 jsonReporter.fileName 指向该目录下 mutation.json', () => {
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['src/foo.ts'],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.jsonReporter.fileName).toBe(absPosix(reportDir, 'mutation.json'));
    expect(config.jsonReporter.fileName).not.toContain('reports/mutation/');
    fs.unlinkSync(result.configPath);
  });

  it('临时配置落在 sandbox root，不改用户长期 config', () => {
    const userConfig = path.join(project.root, 'stryker.config.json');
    fs.writeFileSync(userConfig, JSON.stringify({ mutate: ['keep.ts'] }), 'utf-8');
    const before = fs.readFileSync(userConfig, 'utf-8');
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['src/foo.ts'],
      'vitest',
      reportDir,
    );
    expect(result.configPath).not.toBe(userConfig);
    expect(path.dirname(result.configPath)).toBe(path.resolve(project.root));
    expect(fs.readFileSync(userConfig, 'utf-8')).toBe(before);
    fs.unlinkSync(result.configPath);
  });

  it('不支持的 framework 抛错', () => {
    expect(() => resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], 'bun', reportDir)).toThrow(
      /Unsupported/,
    );
    expect(() =>
      resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], 'pytest', reportDir),
    ).toThrow(/Unsupported/);
  });

  it('sourceFiles=[] 仍生成配置', () => {
    const result = resolveStrykerConfig(project.root, PLAN_ROOT, [], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([]);
    fs.unlinkSync(result.configPath);
  });

  it('reportDir 为相对/绝对路径均可落盘到目标 planDir', () => {
    const absReport = path.resolve(reportDir);
    const resultAbs = resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], 'jest', absReport);
    const cfgAbs = JSON.parse(fs.readFileSync(resultAbs.configPath, 'utf-8'));
    expect(cfgAbs.jsonReporter.fileName).toBe(absPosix(absReport, 'mutation.json'));
    fs.unlinkSync(resultAbs.configPath);

    const relReport = path.relative(project.root, reportDir) || '.';
    const resultRel = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['a.ts'],
      'vite-plus',
      relReport,
    );
    const cfgRel = JSON.parse(fs.readFileSync(resultRel.configPath, 'utf-8'));
    expect(cfgRel.jsonReporter.fileName).toBe(absPosix(relReport, 'mutation.json'));
    fs.unlinkSync(resultRel.configPath);
  });

  it('testRunner 按 framework 映射 jest/vitest', () => {
    for (const [fw, runner] of [
      ['vitest', 'vitest'],
      ['jest', 'jest'],
      ['vite-plus', 'vitest'],
    ] as const) {
      const result = resolveStrykerConfig(project.root, PLAN_ROOT, ['src/t.ts'], fw, reportDir);
      const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
      expect(config.testRunner).toBe(runner);
      fs.unlinkSync(result.configPath);
    }
  });
});

describe('resolveStrykerConfig -- mutation-score 补强', () => {
  let project: TempProject;
  let reportDir: string;

  beforeEach(() => {
    project = createTempProject();
    reportDir = path.join(project.root, 'reports', 'test', 'vitest');
    fs.mkdirSync(reportDir, { recursive: true });
  });

  afterEach(() => {
    project.cleanup();
  });

  it('jest → plugins=@stryker-mutator/jest-runner；vitest/vite-plus → vitest-runner', () => {
    const jestCfg = resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], 'jest', reportDir);
    const jestJson = JSON.parse(fs.readFileSync(jestCfg.configPath, 'utf-8'));
    expect(jestJson.testRunner).toBe('jest');
    expect(jestJson.plugins).toEqual(['@stryker-mutator/jest-runner']);
    fs.unlinkSync(jestCfg.configPath);

    for (const fw of ['vitest', 'vite-plus'] as const) {
      const r = resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], fw, reportDir);
      const j = JSON.parse(fs.readFileSync(r.configPath, 'utf-8'));
      expect(j.testRunner).toBe('vitest');
      expect(j.plugins).toEqual(['@stryker-mutator/vitest-runner']);
      fs.unlinkSync(r.configPath);
    }
  });

  it('写入 JSON 含 $schema 后缀、ignoreStatic=true、reporters、timeoutMS=10000、tempDirPath=.stryker-tmp', () => {
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['src/foo.ts'],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.$schema).toMatch(/stryker-schema\.json$/);
    expect(config.ignoreStatic).toBe(true);
    expect(config.reporters).toEqual(expect.arrayContaining(['json', 'html']));
    expect(config.reporters).toHaveLength(2);
    expect(config.timeoutMS).toBe(10000);
    expect(result.tempDirPath).toBe(path.resolve(project.root, '.stryker-tmp'));
    expect(path.basename(result.configPath)).toMatch(/^stryker\.config\.[a-f0-9]+\.json$/);
    fs.unlinkSync(result.configPath);
  });

  it('相对路径 src/foo.ts → mutate 为正斜杠相对路径', () => {
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['src/foo.ts'],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([relPosix(project.root, 'src/foo.ts')]);
    fs.unlinkSync(result.configPath);
  });

  it('绝对路径位于 root 下 → 相对 POSIX；Windows 反斜杠输入 → 输出仅 /', () => {
    const abs = path.join(project.root, 'src', 'bar.ts');
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      [abs, 'src\\baz.ts'],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate[0]).toBe(relPosix(project.root, 'src', 'bar.ts'));
    expect(config.mutate[1]).toBe(relPosix(project.root, 'src/baz.ts'));
    for (const m of config.mutate as string[]) {
      expect(m).not.toContain('\\');
      expect(path.isAbsolute(m) || /^[A-Za-z]:/.test(m)).toBe(false);
    }
    fs.unlinkSync(result.configPath);
  });

  it('绝对路径等于 root → mutate 条目为空相对路径', () => {
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      [project.root],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual(['']);
    fs.unlinkSync(result.configPath);
  });

  it('落在 root 外 → 相对路径含 ..；盘符路径无反斜杠', () => {
    const inside = path.resolve(project.root, 'src', 'in.ts');
    const outside = path.resolve(project.root, '..', 'outside-only.ts');
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      [inside, outside],
      'jest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate[0]).toBe(relPosix(project.root, 'src', 'in.ts'));
    expect(config.mutate[1]).toBe(path.relative(project.root, outside).replace(/\\/g, '/'));
    expect(config.mutate[1]).toMatch(/^\.\.\//);
    expect(config.mutate[1]).not.toContain('\\');
    fs.unlinkSync(result.configPath);
  });

  it('相对路径误带 root 绝对前缀字符串 → 仍归一为相对路径；空字符串路径仍写出配置不抛', () => {
    const normalizedRoot = path.resolve(project.root).replace(/\\/g, '/');
    const prefixed = `${normalizedRoot}/src/accidental.ts`;
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      [prefixed, ''],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate[0]).toBe(relPosix(project.root, 'src/accidental.ts'));
    expect(config.mutate[1]).toBe('');
    fs.unlinkSync(result.configPath);
  });

  it('jsonReporter.fileName 绝对路径等于 reportDir/mutation.json；不含 reports/mutation/', () => {
    const result = resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.jsonReporter.fileName).toBe(absPosix(reportDir, 'mutation.json'));
    expect(
      path.isAbsolute(config.jsonReporter.fileName) ||
        /^[A-Za-z]:/.test(config.jsonReporter.fileName),
    ).toBe(true);
    expect(config.jsonReporter.fileName).not.toContain('\\');
    expect(config.jsonReporter.fileName).not.toContain('reports/mutation/');
    fs.unlinkSync(result.configPath);
  });

  it("framework=bun/pytest/go/'' 抛错，message 匹配 Unsupported 且列出 jest, vitest, vite-plus", () => {
    for (const fw of ['bun', 'pytest', 'go', ''] as const) {
      expect(() => resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], fw, reportDir)).toThrow(
        /Unsupported/,
      );
      try {
        resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], fw, reportDir);
      } catch (e) {
        const msg = (e as Error).message;
        expect(msg).toContain('jest, vitest, vite-plus');
      }
    }
  });

  it('多 sourceFiles（单元素、>100）顺序保留且全部归一为相对路径', () => {
    const many = Array.from({ length: 120 }, (_, i) => `src/f${i}.ts`);
    const result = resolveStrykerConfig(project.root, PLAN_ROOT, many, 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toHaveLength(120);
    expect(config.mutate[0]).toBe(relPosix(project.root, 'src/f0.ts'));
    expect(config.mutate[119]).toBe(relPosix(project.root, 'src/f119.ts'));
    fs.unlinkSync(result.configPath);

    const one = resolveStrykerConfig(project.root, PLAN_ROOT, ['only.ts'], 'jest', reportDir);
    expect(JSON.parse(fs.readFileSync(one.configPath, 'utf-8')).mutate).toEqual([
      relPosix(project.root, 'only.ts'),
    ]);
    fs.unlinkSync(one.configPath);
  });

  it('传入 frameworkConfigPath 时 vitest 含 configFile+dir；jest 另写 testMatch；未传则无块', () => {
    const vitestCfg = path.join(project.root, 'vitest.config.ts');
    const jestCfg = path.join(project.root, 'jest.config.js');
    const testMatch = [`**/${PLAN_ROOT}/**/*.test.ts?(x)`];

    const vitestResult = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['a.ts'],
      'vitest',
      reportDir,
      vitestCfg,
    );
    const vitestJson = JSON.parse(fs.readFileSync(vitestResult.configPath, 'utf-8'));
    expect(vitestJson.vitest).toEqual({
      configFile: relPosix(project.root, 'vitest.config.ts'),
      dir: '.',
    });
    expect(vitestJson.vitest.config).toBeUndefined();
    expect(vitestJson.jest).toBeUndefined();
    fs.unlinkSync(vitestResult.configPath);

    const nestedCfg = path.join(project.root, 'plugins', 'dev-team', 'vite.config.ts');
    fs.mkdirSync(path.dirname(nestedCfg), { recursive: true });
    fs.writeFileSync(nestedCfg, 'export default {}', 'utf-8');
    const nestedResult = resolveStrykerConfig(
      project.root,
      'plugins/dev-team',
      ['a.ts'],
      'vitest',
      reportDir,
      nestedCfg,
    );
    const nestedJson = JSON.parse(fs.readFileSync(nestedResult.configPath, 'utf-8'));
    expect(nestedJson.vitest).toEqual({
      configFile: relPosix(project.root, 'plugins/dev-team/vite.config.ts'),
      dir: 'plugins/dev-team',
    });
    expect(nestedJson.vitest.config).toBeUndefined();
    fs.unlinkSync(nestedResult.configPath);

    const vitePlusResult = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['a.ts'],
      'vite-plus',
      reportDir,
      'vite.config.ts',
    );
    const vitePlusJson = JSON.parse(fs.readFileSync(vitePlusResult.configPath, 'utf-8'));
    expect(vitePlusJson.vitest).toEqual({
      configFile: relPosix(project.root, 'vite.config.ts'),
      dir: '.',
    });
    expect(vitePlusJson.vitest.config).toBeUndefined();
    fs.unlinkSync(vitePlusResult.configPath);

    const jestResult = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['a.ts'],
      'jest',
      reportDir,
      jestCfg,
    );
    const jestJson = JSON.parse(fs.readFileSync(jestResult.configPath, 'utf-8'));
    expect(jestJson.jest).toEqual({
      configFile: relPosix(project.root, 'jest.config.js'),
      enableFindRelatedTests: false,
      config: { rootDir: '.', testMatch },
    });
    expect(jestJson.vitest).toBeUndefined();
    fs.unlinkSync(jestResult.configPath);

    const none = resolveStrykerConfig(project.root, PLAN_ROOT, ['a.ts'], 'vitest', reportDir, null);
    const noneJson = JSON.parse(fs.readFileSync(none.configPath, 'utf-8'));
    expect(noneJson.vitest).toBeUndefined();
    expect(noneJson.jest).toBeUndefined();
    fs.unlinkSync(none.configPath);
  });
});

describe('resolveStrykerConfig -- 盘符 / 空路径 / overlay（突变补强）', () => {
  let project: TempProject;
  let reportDir: string;

  beforeEach(() => {
    project = createTempProject();
    reportDir = path.join(project.root, 'reports', 'test', 'vitest');
    fs.mkdirSync(reportDir, { recursive: true });
  });

  afterEach(() => {
    project.cleanup();
  });

  it("sourceFiles 含 '' 时 mutate 保留空串条目且不抛", () => {
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['src/a.ts', '', 'src/b.ts'],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([
      relPosix(project.root, 'src/a.ts'),
      '',
      relPosix(project.root, 'src/b.ts'),
    ]);
    fs.unlinkSync(result.configPath);
  });

  it('形如 C:/abs/file.ts 的盘符路径走盘符分支并相对化', () => {
    // 在非 win 上 path.isAbsolute('C:/…') 可能为 false，依赖 /^[A-Za-z]:/ 分支
    const driveAbs = `C:/abs/under-root/file.ts`;
    const rooted = path.resolve(project.root, 'src', 'drive.ts').replace(/\\/g, '/');
    // 若当前平台是 win，用真实盘符绝对路径；否则用 C: 前缀字符串覆盖 || 分支
    const driveLike =
      process.platform === 'win32'
        ? path.resolve(project.root, 'src', 'win-drive.ts').replace(/\\/g, '/')
        : driveAbs;

    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      [driveLike, rooted],
      'vitest',
      reportDir,
    );
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toHaveLength(2);
    for (const m of config.mutate as string[]) {
      expect(m).not.toContain('\\');
    }
    // 位于 root 下的路径应相对化
    expect(config.mutate[1]).toBe(relPosix(project.root, 'src', 'drive.ts'));
    if (process.platform === 'win32') {
      expect(config.mutate[0]).toBe(relPosix(project.root, 'src', 'win-drive.ts'));
    } else {
      // 非 win：C:/abs/... 相对化后含 .. 或保留相对片段，且不得原样抛错
      expect(typeof config.mutate[0]).toBe('string');
      expect(config.mutate[0]).not.toMatch(/\\/);
    }
    fs.unlinkSync(result.configPath);
  });

  it('frameworkConfigPath 为盘符绝对路径时 vitest overlay 含 configFile+dir（相对 root、正斜杠）', () => {
    fs.writeFileSync(path.join(project.root, 'vitest.config.ts'), 'export default {}', 'utf-8');
    const absCfg = path.resolve(project.root, 'vitest.config.ts');
    const result = resolveStrykerConfig(
      project.root,
      PLAN_ROOT,
      ['a.ts'],
      'vitest',
      reportDir,
      absCfg,
    );
    const json = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(json.vitest).toEqual({
      configFile: relPosix(project.root, 'vitest.config.ts'),
      dir: '.',
    });
    expect(json.vitest.config).toBeUndefined();
    expect(json.vitest.configFile).not.toContain('\\');
    expect(json.vitest.dir).not.toContain('\\');
    fs.unlinkSync(result.configPath);

    // 非 win：C: 前缀在 path.isAbsolute 为 false 时仍走盘符分支
    if (process.platform !== 'win32') {
      const drivePath = `C:${absCfg.replace(/\\/g, '/')}`;
      const driveResult = resolveStrykerConfig(
        project.root,
        PLAN_ROOT,
        ['a.ts'],
        'vitest',
        reportDir,
        drivePath,
      );
      const driveJson = JSON.parse(fs.readFileSync(driveResult.configPath, 'utf-8'));
      expect(driveJson.vitest).toHaveProperty('configFile');
      expect(driveJson.vitest.configFile).not.toContain('\\');
      fs.unlinkSync(driveResult.configPath);
    }
  });

  it("jest + planRoot '.' → **/*.test.ts?(x)；'src' → **/src/**；'./pkg' 先剥 ./", () => {
    const jestCfg = path.join(project.root, 'jest.config.js');
    fs.writeFileSync(jestCfg, 'module.exports={}', 'utf-8');

    const cases: Array<{ planRoot: string; testMatch: string }> = [
      { planRoot: '.', testMatch: '**/*.test.ts?(x)' },
      { planRoot: 'src', testMatch: '**/src/**/*.test.ts?(x)' },
      { planRoot: './pkg', testMatch: '**/pkg/**/*.test.ts?(x)' },
    ];
    for (const c of cases) {
      const result = resolveStrykerConfig(
        project.root,
        c.planRoot,
        ['a.ts'],
        'jest',
        reportDir,
        jestCfg,
      );
      const json = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
      expect(json.jest.config.testMatch).toEqual([c.testMatch]);
      expect(json.jest.enableFindRelatedTests).toBe(false);
      fs.unlinkSync(result.configPath);
    }
  });

  it('未传 frameworkConfigPath → 无 vitest/jest 块；ignoreStatic/timeoutMS/jsonReporter 精确', () => {
    const result = resolveStrykerConfig(project.root, '.', ['src/foo.ts'], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.vitest).toBeUndefined();
    expect(config.jest).toBeUndefined();
    expect(config.ignoreStatic).toBe(true);
    expect(config.timeoutMS).toBe(10000);
    expect(config.jsonReporter.fileName).toBe(absPosix(reportDir, 'mutation.json'));
    const roundTrip = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(roundTrip.mutate).toEqual([relPosix(project.root, 'src/foo.ts')]);
    fs.unlinkSync(result.configPath);
  });
});
