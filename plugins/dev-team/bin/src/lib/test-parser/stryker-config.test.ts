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
    const expectedRel = path
      .relative(project.root, path.join(reportDir, 'mutation.json'))
      .replace(/\\/g, '/');
    expect(config.jsonReporter.fileName).toBe(expectedRel);
    expect(config.jsonReporter.fileName).toContain('mutation.json');
    expect(config.jsonReporter.fileName).not.toContain('reports/mutation/');
    fs.unlinkSync(result.configPath);
  });

  it('临时配置落在 absCwd，不改用户长期 config', () => {
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
    expect(path.resolve(project.root, cfgAbs.jsonReporter.fileName)).toBe(
      path.resolve(absReport, 'mutation.json'),
    );
    fs.unlinkSync(resultAbs.configPath);

    const relReport = path.relative(project.root, reportDir) || '.';
    const resultRel = resolveStrykerConfig(project.root, ['a.ts'], 'vite-plus', relReport);
    const cfgRel = JSON.parse(fs.readFileSync(resultRel.configPath, 'utf-8'));
    expect(cfgRel.jsonReporter.fileName.replace(/\\/g, '/')).toContain('mutation.json');
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
