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
    const result = resolveStrykerConfig(project.root, ['src/foo.ts'], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.jsonReporter.fileName).toBe(absPosix(reportDir, 'mutation.json'));
    expect(config.jsonReporter.fileName).not.toContain('reports/mutation/');
    fs.unlinkSync(result.configPath);
  });

  it('临时配置落在 sandbox root，不改用户长期 config', () => {
    const userConfig = path.join(project.root, 'stryker.config.json');
    fs.writeFileSync(userConfig, JSON.stringify({ mutate: ['keep.ts'] }), 'utf-8');
    const before = fs.readFileSync(userConfig, 'utf-8');
    const result = resolveStrykerConfig(project.root, ['src/foo.ts'], 'vitest', reportDir);
    expect(result.configPath).not.toBe(userConfig);
    expect(path.dirname(result.configPath)).toBe(path.resolve(project.root));
    expect(fs.readFileSync(userConfig, 'utf-8')).toBe(before);
    fs.unlinkSync(result.configPath);
  });

  it('不支持的 framework 抛错', () => {
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], 'bun', reportDir)).toThrow(
      /Unsupported/,
    );
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], 'pytest', reportDir)).toThrow(
      /Unsupported/,
    );
  });

  it('sourceFiles=[] 仍生成配置', () => {
    const result = resolveStrykerConfig(project.root, [], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([]);
    fs.unlinkSync(result.configPath);
  });

  it('reportDir 为相对/绝对路径均可落盘到目标 planDir', () => {
    const absReport = path.resolve(reportDir);
    const resultAbs = resolveStrykerConfig(project.root, ['a.ts'], 'jest', absReport);
    const cfgAbs = JSON.parse(fs.readFileSync(resultAbs.configPath, 'utf-8'));
    expect(cfgAbs.jsonReporter.fileName).toBe(absPosix(absReport, 'mutation.json'));
    fs.unlinkSync(resultAbs.configPath);

    const relReport = path.relative(project.root, reportDir) || '.';
    const resultRel = resolveStrykerConfig(project.root, ['a.ts'], 'vite-plus', relReport);
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
      const result = resolveStrykerConfig(project.root, ['src/t.ts'], fw, reportDir);
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
    const jestCfg = resolveStrykerConfig(project.root, ['a.ts'], 'jest', reportDir);
    const jestJson = JSON.parse(fs.readFileSync(jestCfg.configPath, 'utf-8'));
    expect(jestJson.testRunner).toBe('jest');
    expect(jestJson.plugins).toEqual(['@stryker-mutator/jest-runner']);
    fs.unlinkSync(jestCfg.configPath);

    for (const fw of ['vitest', 'vite-plus'] as const) {
      const r = resolveStrykerConfig(project.root, ['a.ts'], fw, reportDir);
      const j = JSON.parse(fs.readFileSync(r.configPath, 'utf-8'));
      expect(j.testRunner).toBe('vitest');
      expect(j.plugins).toEqual(['@stryker-mutator/vitest-runner']);
      fs.unlinkSync(r.configPath);
    }
  });

  it('写入 JSON 含 $schema 后缀、ignoreStatic=true、reporters、timeoutMS=10000、tempDirPath=.stryker-tmp', () => {
    const result = resolveStrykerConfig(project.root, ['src/foo.ts'], 'vitest', reportDir);
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

  it('相对路径 src/foo.ts → mutate 为正斜杠绝对路径', () => {
    const result = resolveStrykerConfig(project.root, ['src/foo.ts'], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([absPosix(project.root, 'src/foo.ts')]);
    fs.unlinkSync(result.configPath);
  });

  it('绝对路径位于 root 下 → 绝对 POSIX；Windows 反斜杠输入 → 输出仅 /', () => {
    const abs = path.join(project.root, 'src', 'bar.ts');
    const result = resolveStrykerConfig(project.root, [abs, 'src\\baz.ts'], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate[0]).toBe(absPosix(project.root, 'src', 'bar.ts'));
    expect(config.mutate[1]).toBe(absPosix(project.root, 'src/baz.ts'));
    for (const m of config.mutate as string[]) {
      expect(m).not.toContain('\\');
      expect(path.isAbsolute(m) || /^[A-Za-z]:/.test(m)).toBe(true);
    }
    fs.unlinkSync(result.configPath);
  });

  it('绝对路径等于 root → mutate 条目为 root 绝对路径', () => {
    const result = resolveStrykerConfig(project.root, [project.root], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([absPosix(project.root)]);
    fs.unlinkSync(result.configPath);
  });

  it('盘符绝对路径保持绝对 POSIX；落在 root 外亦为绝对路径', () => {
    const inside = path.resolve(project.root, 'src', 'in.ts');
    const outside = path.resolve(project.root, '..', 'outside-only.ts');
    const result = resolveStrykerConfig(project.root, [inside, outside], 'jest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate[0]).toBe(absPosix(inside));
    expect(config.mutate[1]).toBe(absPosix(outside));
    expect(config.mutate[1]).not.toContain('\\');
    fs.unlinkSync(result.configPath);
  });

  it('相对路径误带 root 绝对前缀字符串 → 仍归一为绝对路径；空字符串路径仍写出配置不抛', () => {
    const normalizedRoot = path.resolve(project.root).replace(/\\/g, '/');
    const prefixed = `${normalizedRoot}/src/accidental.ts`;
    const result = resolveStrykerConfig(project.root, [prefixed, ''], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate[0]).toBe(absPosix(project.root, 'src/accidental.ts'));
    expect(config.mutate[1]).toBe('');
    fs.unlinkSync(result.configPath);
  });

  it('jsonReporter.fileName 绝对路径等于 reportDir/mutation.json；不含 reports/mutation/', () => {
    const result = resolveStrykerConfig(project.root, ['a.ts'], 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.jsonReporter.fileName).toBe(absPosix(reportDir, 'mutation.json'));
    expect(path.isAbsolute(config.jsonReporter.fileName) || /^[A-Za-z]:/.test(config.jsonReporter.fileName)).toBe(
      true,
    );
    expect(config.jsonReporter.fileName).not.toContain('\\');
    expect(config.jsonReporter.fileName).not.toContain('reports/mutation/');
    fs.unlinkSync(result.configPath);
  });

  it("framework=bun/pytest/go/'' 抛错，message 匹配 Unsupported 且列出 jest, vitest, vite-plus", () => {
    for (const fw of ['bun', 'pytest', 'go', ''] as const) {
      expect(() => resolveStrykerConfig(project.root, ['a.ts'], fw, reportDir)).toThrow(
        /Unsupported/,
      );
      try {
        resolveStrykerConfig(project.root, ['a.ts'], fw, reportDir);
      } catch (e) {
        const msg = (e as Error).message;
        expect(msg).toContain('jest, vitest, vite-plus');
      }
    }
  });

  it('多 sourceFiles（单元素、>100）顺序保留且全部归一为绝对路径', () => {
    const many = Array.from({ length: 120 }, (_, i) => `src/f${i}.ts`);
    const result = resolveStrykerConfig(project.root, many, 'vitest', reportDir);
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toHaveLength(120);
    expect(config.mutate[0]).toBe(absPosix(project.root, 'src/f0.ts'));
    expect(config.mutate[119]).toBe(absPosix(project.root, 'src/f119.ts'));
    fs.unlinkSync(result.configPath);

    const one = resolveStrykerConfig(project.root, ['only.ts'], 'jest', reportDir);
    expect(JSON.parse(fs.readFileSync(one.configPath, 'utf-8')).mutate).toEqual([
      absPosix(project.root, 'only.ts'),
    ]);
    fs.unlinkSync(one.configPath);
  });

  it('传入 frameworkConfigPath 时按 runner 写入绝对 configFile；未传则无 jest/vitest 块', () => {
    const vitestCfg = path.join(project.root, 'vitest.config.ts');
    const jestCfg = path.join(project.root, 'jest.config.js');

    const vitestResult = resolveStrykerConfig(
      project.root,
      ['a.ts'],
      'vitest',
      reportDir,
      vitestCfg,
    );
    const vitestJson = JSON.parse(fs.readFileSync(vitestResult.configPath, 'utf-8'));
    expect(vitestJson.vitest).toEqual({ configFile: absPosix(vitestCfg) });
    expect(vitestJson.jest).toBeUndefined();
    fs.unlinkSync(vitestResult.configPath);

    const vitePlusResult = resolveStrykerConfig(
      project.root,
      ['a.ts'],
      'vite-plus',
      reportDir,
      'vite.config.ts',
    );
    const vitePlusJson = JSON.parse(fs.readFileSync(vitePlusResult.configPath, 'utf-8'));
    expect(vitePlusJson.vitest).toEqual({
      configFile: absPosix(project.root, 'vite.config.ts'),
    });
    fs.unlinkSync(vitePlusResult.configPath);

    const jestResult = resolveStrykerConfig(project.root, ['a.ts'], 'jest', reportDir, jestCfg);
    const jestJson = JSON.parse(fs.readFileSync(jestResult.configPath, 'utf-8'));
    expect(jestJson.jest).toEqual({ configFile: absPosix(jestCfg) });
    expect(jestJson.vitest).toBeUndefined();
    fs.unlinkSync(jestResult.configPath);

    const none = resolveStrykerConfig(project.root, ['a.ts'], 'vitest', reportDir, null);
    const noneJson = JSON.parse(fs.readFileSync(none.configPath, 'utf-8'));
    expect(noneJson.vitest).toBeUndefined();
    expect(noneJson.jest).toBeUndefined();
    fs.unlinkSync(none.configPath);
  });
});
