/**
 * 单元测试: lib/test-parser/stryker-config -- StrykerJS 配置检测与生成
 *
 * 覆盖范围:
 * - 正向: 项目根目录存在 stryker.config.json 时直接返回该路径，cleanup=false
 * - 正向: 项目根目录存在 stryker.config.mjs 时直接返回该路径，cleanup=false
 * - 正向: 项目根目录存在 stryker.config.cjs 时直接返回该路径，cleanup=false
 * - 正向: 无自定义配置时生成临时配置文件，返回路径和 cleanup=true
 * - 正向: 生成临时配置时 mutate 限定为 sourceFiles 路径列表
 * - 正向: 生成临时配置时 testRunner 根据 framework 选择 "jest-runner" 或 "vitest-runner"
 * - 正向: 生成临时配置时 reporters 设置为 ["json"]
 * - 正向: 生成临时配置时 thresholds 从传入参数读取
 * - 异常: sourceFiles 为空数组时 mutate 为空列表
 * - 边界: 临时配置文件名使用随机后缀避免冲突
 *
 * @see openspec/changes/unit-test-mutation-testing/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';

import { resolveStrykerConfig } from './stryker-config';

// ===========================================================================
// Helpers
// ===========================================================================

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

// ===========================================================================
// 正向测试: 自定义配置检测
// ===========================================================================

describe('resolveStrykerConfig -- 自定义配置检测', () => {
  let project: TempProject;

  beforeEach(() => {
    project = createTempProject();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('项目根目录存在 stryker.config.json 时直接返回该路径，cleanup=false', () => {
    const configPath = path.join(project.root, 'stryker.config.json');
    fs.writeFileSync(configPath, JSON.stringify({}), 'utf-8');

    const result = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    expect(result.configPath).toBe(configPath);
    expect(result.cleanup).toBe(false);
  });

  it('项目根目录存在 stryker.config.mjs 时直接返回该路径，cleanup=false', () => {
    const configPath = path.join(project.root, 'stryker.config.mjs');
    fs.writeFileSync(configPath, 'export default {}', 'utf-8');

    const result = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    expect(result.configPath).toBe(configPath);
    expect(result.cleanup).toBe(false);
  });

  it('项目根目录存在 stryker.config.cjs 时直接返回该路径，cleanup=false', () => {
    const configPath = path.join(project.root, 'stryker.config.cjs');
    fs.writeFileSync(configPath, 'module.exports = {}', 'utf-8');

    const result = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    expect(result.configPath).toBe(configPath);
    expect(result.cleanup).toBe(false);
  });
});

// ===========================================================================
// 正向测试: 临时配置生成
// ===========================================================================

describe('resolveStrykerConfig -- 临时配置生成', () => {
  let project: TempProject;

  beforeEach(() => {
    project = createTempProject();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('无自定义配置时生成临时配置文件，返回路径和 cleanup=true', () => {
    const result = resolveStrykerConfig(project.root, ['src/foo.ts', 'src/bar.ts'], [], 'vitest');
    expect(result.cleanup).toBe(true);
    expect(fs.existsSync(result.configPath)).toBe(true);

    // 清理临时文件
    fs.unlinkSync(result.configPath);
  });

  it('生成临时配置时 mutate 限定为 sourceFiles 路径列表', () => {
    const sourceFiles = ['src/foo.ts', 'src/bar.ts'];
    const result = resolveStrykerConfig(project.root, sourceFiles, [], 'vitest');
    expect(result.cleanup).toBe(true);

    const configContent = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(configContent.mutate).toEqual(sourceFiles);

    fs.unlinkSync(result.configPath);
  });

  it('生成临时配置时 testRunner 根据 framework 选择 "jest-runner" 或 "vitest-runner"', () => {
    // vitest -> vitest-runner
    const result1 = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    const config1 = JSON.parse(fs.readFileSync(result1.configPath, 'utf-8'));
    expect(config1.testRunner).toBe('vitest-runner');
    fs.unlinkSync(result1.configPath);

    // jest -> jest-runner
    const result2 = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'jest');
    const config2 = JSON.parse(fs.readFileSync(result2.configPath, 'utf-8'));
    expect(config2.testRunner).toBe('jest-runner');
    fs.unlinkSync(result2.configPath);

    // vite-plus -> vitest-runner
    const result3 = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vite-plus');
    const config3 = JSON.parse(fs.readFileSync(result3.configPath, 'utf-8'));
    expect(config3.testRunner).toBe('vitest-runner');
    fs.unlinkSync(result3.configPath);
  });

  it('生成临时配置时 reporters 设置为 ["json"]', () => {
    const result = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.reporters).toEqual(['json']);
    fs.unlinkSync(result.configPath);
  });

  it('生成临时配置时 thresholds 从传入参数读取', () => {
    const result = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.thresholds).toBeDefined();
    expect(config.thresholds.high).toBe(80);
    expect(config.thresholds.low).toBe(60);
    expect(config.thresholds.break).toBeNull();
    fs.unlinkSync(result.configPath);
  });
});

// ===========================================================================
// 异常测试
// ===========================================================================

describe('resolveStrykerConfig -- 异常', () => {
  let project: TempProject;

  beforeEach(() => {
    project = createTempProject();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('sourceFiles 为空数组时 mutate 为空列表', () => {
    const result = resolveStrykerConfig(project.root, [], [], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([]);
    fs.unlinkSync(result.configPath);
  });

  it('不支持框架时抛出错误', () => {
    expect(() => resolveStrykerConfig(project.root, ['src/test.ts'], [], 'bun')).toThrow();
  });
});

// ===========================================================================
// 边界测试
// ===========================================================================

describe('resolveStrykerConfig -- 边界', () => {
  let project: TempProject;

  beforeEach(() => {
    project = createTempProject();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('临时配置文件名使用随机后缀避免冲突', () => {
    const result1 = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');
    const result2 = resolveStrykerConfig(project.root, ['src/test.ts'], [], 'vitest');

    // 文件名应不同（随机后缀）
    expect(result1.configPath).not.toBe(result2.configPath);

    // 两个文件都是有效 JSON 配置
    const config1 = JSON.parse(fs.readFileSync(result1.configPath, 'utf-8'));
    const config2 = JSON.parse(fs.readFileSync(result2.configPath, 'utf-8'));
    expect(config1.testRunner).toBe('vitest-runner');
    expect(config2.testRunner).toBe('vitest-runner');

    fs.unlinkSync(result1.configPath);
    fs.unlinkSync(result2.configPath);
  });
});
