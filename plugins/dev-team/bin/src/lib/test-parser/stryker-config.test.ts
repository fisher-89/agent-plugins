/**
 * 单元测试: lib/test-parser/stryker-config -- StrykerJS 配置检测与生成
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

  it('无自定义配置时生成临时配置文件，返回路径', () => {
    const result = resolveStrykerConfig(project.root, ['src/foo.ts', 'src/bar.ts'], 'vitest');
    expect(fs.existsSync(result.configPath)).toBe(true);

    // 清理临时文件
    fs.unlinkSync(result.configPath);
  });

  it('生成临时配置时 mutate 限定为 sourceFiles 路径列表', () => {
    const sourceFiles = ['src/foo.ts', 'src/bar.ts'];
    const result = resolveStrykerConfig(project.root, sourceFiles, 'vitest');

    const configContent = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(configContent.mutate).toEqual(sourceFiles);

    fs.unlinkSync(result.configPath);
  });

  it('生成临时配置时 testRunner 根据 framework 选择 "jest" 或 "vitest"', () => {
    // vitest -> vitest
    const result1 = resolveStrykerConfig(project.root, ['src/test.ts'], 'vitest');
    const config1 = JSON.parse(fs.readFileSync(result1.configPath, 'utf-8'));
    expect(config1.testRunner).toBe('vitest');
    fs.unlinkSync(result1.configPath);

    // jest -> jest
    const result2 = resolveStrykerConfig(project.root, ['src/test.ts'], 'jest');
    const config2 = JSON.parse(fs.readFileSync(result2.configPath, 'utf-8'));
    expect(config2.testRunner).toBe('jest');
    fs.unlinkSync(result2.configPath);

    // vite-plus -> vitest
    const result3 = resolveStrykerConfig(project.root, ['src/test.ts'], 'vite-plus');
    const config3 = JSON.parse(fs.readFileSync(result3.configPath, 'utf-8'));
    expect(config3.testRunner).toBe('vitest');
    fs.unlinkSync(result3.configPath);
  });

  it('生成临时配置时 reporters 设置为 ["json", "html"]', () => {
    const result = resolveStrykerConfig(project.root, ['src/test.ts'], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.reporters).toEqual(['json', 'html']);
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
    const result = resolveStrykerConfig(project.root, [], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([]);
    fs.unlinkSync(result.configPath);
  });

  it('不支持框架时抛出错误', () => {
    expect(() => resolveStrykerConfig(project.root, ['src/test.ts'], 'bun')).toThrow();
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
    const result1 = resolveStrykerConfig(project.root, ['src/test.ts'], 'vitest');
    const result2 = resolveStrykerConfig(project.root, ['src/test.ts'], 'vitest');

    // 文件名应不同（随机后缀）
    expect(result1.configPath).not.toBe(result2.configPath);

    // 两个文件都是有效 JSON 配置
    const config1 = JSON.parse(fs.readFileSync(result1.configPath, 'utf-8'));
    const config2 = JSON.parse(fs.readFileSync(result2.configPath, 'utf-8'));
    expect(config1.testRunner).toBe('vitest');
    expect(config2.testRunner).toBe('vitest');

    fs.unlinkSync(result1.configPath);
    fs.unlinkSync(result2.configPath);
  });
});

// ===========================================================================
// resolveStrykerConfig — absCwd rootPath (AC-2)
// ===========================================================================

describe('resolveStrykerConfig — absCwd rootPath (AC-2)', () => {
  let project: TempProject;

  beforeEach(() => {
    project = createTempProject();
  });

  afterEach(() => {
    project.cleanup();
  });

  it('rootPath 为 absCwd 时，生成的 config 位于该目录，tempDirPath 为该目录下 .stryker-tmp', () => {
    const absCwd = fs.mkdtempSync(path.join(project.root, 'cwd-'));
    const result = resolveStrykerConfig(absCwd, ['src/foo.ts'], 'vitest');
    expect(result.configPath.startsWith(absCwd)).toBe(true);
    expect(result.tempDirPath).toBe(path.resolve(absCwd, '.stryker-tmp'));
    fs.unlinkSync(result.configPath);
  });

  it('sourceFiles 含相对 projectRoot 的路径时，mutate 条目被规范为相对 rootPath', () => {
    const absCwd = path.join(project.root, 'pkg');
    fs.mkdirSync(absCwd, { recursive: true });
    const absSource = path.join(project.root, 'pkg', 'src', 'foo.ts');
    const result = resolveStrykerConfig(absCwd, [absSource], 'vite-plus');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual(['src/foo.ts']);
    fs.unlinkSync(result.configPath);
  });

  it('不支持的 framework（如 pytest / 未知名）抛出 Error', () => {
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], 'pytest')).toThrow(/Unsupported/);
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], 'unknown')).toThrow(/Unsupported/);
  });

  it('rootPath 不存在或不可写时抛文件系统错误', () => {
    const missing = path.join(project.root, 'no-such-dir');
    expect(() => resolveStrykerConfig(missing, ['a.ts'], 'vitest')).toThrow();
  });

  it('framework 为 undefined/null 时抛错', () => {
    expect(() => {
      // @ts-expect-error intentional invalid runtime input
      resolveStrykerConfig(project.root, ['a.ts'], undefined);
    }).toThrow();
    expect(() => {
      // @ts-expect-error intentional invalid runtime input
      resolveStrykerConfig(project.root, ['a.ts'], null);
    }).toThrow();
  });

  it('sourceFiles: [] 时仍生成合法临时配置（mutate 为空数组）', () => {
    const result = resolveStrykerConfig(project.root, [], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual([]);
    fs.unlinkSync(result.configPath);
  });

  it('sourceFiles 为单元素列表时 mutate 仅一条且相对 rootPath', () => {
    const result = resolveStrykerConfig(project.root, ['only.ts'], 'jest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toEqual(['only.ts']);
    fs.unlinkSync(result.configPath);
  });

  it('超大 sourceFiles 列表均可写入且路径均为相对 rootPath', () => {
    const files = Array.from({ length: 200 }, (_, i) => `src/f${i}.ts`);
    const result = resolveStrykerConfig(project.root, files, 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(config.mutate).toHaveLength(200);
    expect(config.mutate.every((m: string) => !path.isAbsolute(m))).toBe(true);
    fs.unlinkSync(result.configPath);
  });

  it('rootPath 为空字符串时行为明确（回落 cwd 仍可写配置）', () => {
    const result = resolveStrykerConfig('', ['a.ts'], 'vitest');
    expect(fs.existsSync(result.configPath)).toBe(true);
    fs.unlinkSync(result.configPath);
  });

  it('framework 为空字符串 / 超长字符串 / 含特殊字符时抛 Unsupported 错误', () => {
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], '')).toThrow(/Unsupported/);
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], 'x'.repeat(1001))).toThrow(
      /Unsupported/,
    );
    expect(() => resolveStrykerConfig(project.root, ['a.ts'], 'vit\nest')).toThrow(/Unsupported/);
  });

  it('sourceFiles 条目含空字符串或特殊字符路径时 mutate 规范化或过滤行为明确', () => {
    const result = resolveStrykerConfig(project.root, ['', 'src/有 空格.ts'], 'vitest');
    const config = JSON.parse(fs.readFileSync(result.configPath, 'utf-8'));
    expect(Array.isArray(config.mutate)).toBe(true);
    expect(config.mutate).toContain('src/有 空格.ts');
    fs.unlinkSync(result.configPath);
  });
});
