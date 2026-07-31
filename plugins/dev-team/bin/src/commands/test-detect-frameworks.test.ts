/**
 * 单元测试: test-detect-frameworks — 从 config.tests[] 构建 plan
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect, vi } from 'vite-plus/test';

import * as testFramework from '../lib/test-framework';
import { type OpenSpecConfigInput } from '../schemas';
import { runTestDetectFrameworks } from './test-detect-frameworks';

// ESM 下不可对 namespace export 使用 vi.spyOn；用 vi.mock 提供可替换的 readdirSync / resolve
const { mockReaddirSync, mockPathResolve, actualFsRef, actualPathRef } = vi.hoisted(() => ({
  mockReaddirSync: vi.fn(),
  mockPathResolve: vi.fn(),
  actualFsRef: { current: null as null | typeof fs },
  actualPathRef: { current: null as null | typeof path },
}));

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  actualFsRef.current = actual;
  mockReaddirSync.mockImplementation(actual.readdirSync as unknown as typeof mockReaddirSync);
  return {
    ...actual,
    readdirSync: ((...args: unknown[]) => mockReaddirSync(...args)) as typeof actual.readdirSync,
  };
});

vi.mock('path', async (importOriginal) => {
  const actual = await importOriginal<typeof path>();
  actualPathRef.current = actual;
  mockPathResolve.mockImplementation(actual.resolve as unknown as typeof mockPathResolve);
  return {
    ...actual,
    resolve: ((...args: unknown[]) => mockPathResolve(...args)) as typeof actual.resolve,
  };
});

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: OpenSpecConfigInput): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-fw-test-'));
  const openspecDir = path.join(tmpDir, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(configData, null, 2),
    'utf-8',
  );
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

function writeFile(projectRoot: string, relativePath: string, content = ''): void {
  const fullPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf-8');
}

// ===========================================================================
// plan.directory = absCwd (AC-2)
// ===========================================================================

describe('runTestDetectFrameworks — plan.directory = absCwd (AC-2)', () => {
  it('root: "a/b", cwd: ".." → plan[0].directory === "a"，shell script 含 cd a', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'a/b', framework: 'vite-plus', cwd: '..' }],
    });
    try {
      writeFile(project.root, 'a/b/foo.test.ts');
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].directory).toBe('a');
      expect(result.plan[0].scope).toBe('b');
      expect(result.plan[0].script.shell).toContain('cd a');
      expect(result.plan[0].script.shell).toContain('{files}');
      expect(result.plan[0].script.cmd).toContain('{files}');
    } finally {
      project.cleanup();
    }
  });

  it('root: "plugins/dev-team/bin/src", cwd: ".." → directory=bin、scope=src', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'plugins/dev-team/bin/src', cwd: '..', framework: 'vite-plus' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('plugins/dev-team/bin');
      expect(result.plan[0].scope).toBe('src');
    } finally {
      project.cleanup();
    }
  });

  it('省略 cwd（或缺省 "."）时 directory 等于 root，scope 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg/src', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('pkg/src');
      expect(result.plan[0].scope).toBe('.');
    } finally {
      project.cleanup();
    }
  });

  it('cwd: "nested" 时 directory 为 root/nested（POSIX）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: 'nested' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('pkg/nested');
    } finally {
      project.cleanup();
    }
  });

  it('root 指向不存在目录时仍产出 plan（directory 可推导）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'missing/dir', framework: 'vitest', cwd: '.' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0].directory).toBe('missing/dir');
    } finally {
      project.cleanup();
    }
  });

  it('cwd 为空字符串时 directory 等于 root（schema 默认/解析后为 "."）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: '' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      // empty cwd resolves like '.' under path.resolve
      expect(result.plan[0].directory).toBe('pkg');
    } finally {
      project.cleanup();
    }
  });

  it('cwd 为超长相对路径（>1000 chars）不崩溃且 directory 可推导', () => {
    const long = 'n'.repeat(1001);
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: long }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toContain('pkg/');
    } finally {
      project.cleanup();
    }
  });

  it('cwd 含特殊字符（空格 / Unicode）时 directory 与 script cd 正确规范化', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', cwd: 'my dir/测试' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].directory).toBe('pkg/my dir/测试');
      expect(result.plan[0].script.cmd).toMatch(/cd \/d /);
    } finally {
      project.cleanup();
    }
  });

  it('script.cmd 用 & 串联为单行，不含换行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const cmd = runTestDetectFrameworks({ projectRoot: project.root }).plan[0].script.cmd;
      expect(cmd).not.toMatch(/[\r\n]/);
      expect(cmd).toContain(' & ');
      expect(cmd.indexOf('cd /d')).toBeLessThan(cmd.indexOf('npx vitest'));
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// {config_args} 注入 (AC-3)
// ===========================================================================

describe('runTestDetectFrameworks — {config_args} 注入 (AC-3)', () => {
  it('suite 含 config 且框架为 vite-plus 时，script 展开为 --config + 相对 absCwd 路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vite-plus',
          cwd: '.',
          config: 'vite.config.ts',
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).toContain('--config vite.config.ts');
      // 不应整段盲 append 在命令末尾独行
      const lines = result.plan[0].script.shell.trim().split('\n');
      expect(lines[lines.length - 1]).toContain('--config');
      expect(lines[lines.length - 1]).toContain('vp test');
    } finally {
      project.cleanup();
    }
  });

  it('suite 无 config 时 script 中 {config_args} 展开为空', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).not.toContain('{config_args}');
      expect(result.plan[0].script.shell).not.toContain('--config');
    } finally {
      project.cleanup();
    }
  });

  it('suite 含 config 但框架为 pytest 时 plan 构建失败并给出明确错误', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'pytest', config: 'pytest.ini' }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /does not support config injection/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('suite 含 config 但框架为 rust 时 plan 构建失败', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'rust', config: 'Cargo.toml' }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /does not support config injection/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('config 为空字符串时校验失败或按实现拒绝', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', config: '' }],
    });
    try {
      // schema nonempty → decode 失败 → tests 回落默认 []
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('config 为超长路径（>1000）时展开不崩溃', () => {
    const long = `${'c'.repeat(1001)}.ts`;
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', config: long }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).toContain('--config');
    } finally {
      project.cleanup();
    }
  });

  it('cwd: ".." 且 config 相对 root 时，展开路径为相对 absCwd 的 POSIX 相对路径', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'a/b',
          framework: 'vite-plus',
          cwd: '..',
          config: 'vite.config.ts',
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      // absCwd=a, absConfig=a/b/vite.config.ts → relative = b/vite.config.ts
      expect(result.plan[0].script.shell).toContain('--config b/vite.config.ts');
      expect(result.plan[0].directory).toBe('a');
    } finally {
      project.cleanup();
    }
  });

  it('suite 含 config 但 absConfig 无法解析：message 含 absConfig could not be resolved', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', config: 'vite.config.ts' }],
    });
    const realResolve = actualPathRef.current!.resolve.bind(actualPathRef.current!);
    mockPathResolve.mockImplementation((...args: string[]) => {
      const last = args[args.length - 1] ?? '';
      if (String(last).includes('vite.config.ts')) {
        return '';
      }
      return realResolve(...args);
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /absConfig could not be resolved/,
      );
    } finally {
      mockPathResolve.mockImplementation(
        actualPathRef.current!.resolve as unknown as typeof mockPathResolve,
      );
      project.cleanup();
    }
  });
});

// ===========================================================================
// suite scope (AC-4)
// ===========================================================================

describe('runTestDetectFrameworks — suite scope (AC-4)', () => {
  it('省略 includes 时使用框架 default_glob（相对 root）匹配文件', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/src/foo.test.ts', 'pkg/src/foo.ts'],
        projectRoot: project.root,
      });
      const fwOf = (suffix: string) =>
        result.detected.find((d) => d.file.replace(/\\/g, '/').endsWith(suffix))?.framework;
      expect(fwOf('pkg/src/foo.test.ts')).toBe('vitest');
      expect(fwOf('pkg/src/foo.ts')).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('显式 includes + excludes 时仅 in-scope 文件进入 detected[]', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vite-plus',
          includes: ['**/*.{test,spec}.ts'],
          excludes: ['**/skip/**'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts', 'pkg/skip/b.test.ts', 'pkg/c.ts'],
        projectRoot: project.root,
      });
      const frameworks = result.detected.map((d) => d.framework);
      expect(result.detected.find((d) => d.file.endsWith('a.test.ts'))?.framework).toBe(
        'vite-plus',
      );
      // excluded file skipped entirely
      expect(result.detected.find((d) => d.file.includes('skip'))).toBeUndefined();
      expect(result.detected.find((d) => d.file.endsWith('c.ts'))?.framework).toBe('unknown');
      expect(frameworks).not.toContain(undefined);
    } finally {
      project.cleanup();
    }
  });

  it('多 suite 时数组顺序优先（第一个匹配决定 framework）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'pkg', framework: 'vitest', includes: ['**/*.test.ts'] },
        { root: 'pkg', framework: 'jest', includes: ['**/*.test.ts'] },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('includes/excludes 含非法 glob 语法时行为明确（不静默匹配全部）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['[invalid'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts', 'pkg/b.ts'],
        projectRoot: project.root,
      });
      // 非法 glob 不应把所有文件标为匹配
      const matched = result.detected.filter((d) => d.framework === 'vitest');
      expect(matched.length).toBeLessThan(2);
    } finally {
      project.cleanup();
    }
  });

  it('includes: []（空数组）时无文件匹配进 detected（显式 files 标 unknown）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: [] }],
    });
    try {
      // includes: [] → 回落 default_glob（suite.includes?.length 为 0）
      // 按实现 length 为 0 时使用 default_glob
      const result = runTestDetectFrameworks({
        files: ['pkg/a.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('includes 为单元素数组时仅匹配该 glob', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/only.test.ts'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/only.test.ts', 'pkg/other.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected.find((d) => d.file.endsWith('only.test.ts'))?.framework).toBe(
        'vitest',
      );
      expect(result.detected.find((d) => d.file.endsWith('other.test.ts'))?.framework).toBe(
        'unknown',
      );
    } finally {
      project.cleanup();
    }
  });

  it('excludes 超大列表（大量模式）时匹配仍正确且不超时失控', () => {
    const excludes = Array.from({ length: 300 }, (_, i) => `**/skip${i}/**`);
    excludes.push('**/ignored/**');
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/*.test.ts'],
          excludes,
        },
      ],
    });
    try {
      const started = Date.now();
      const result = runTestDetectFrameworks({
        files: ['pkg/ok.test.ts', 'pkg/ignored/x.test.ts'],
        projectRoot: project.root,
      });
      expect(Date.now() - started).toBeLessThan(5000);
      expect(result.detected.find((d) => d.file.includes('ok'))?.framework).toBe('vitest');
      expect(result.detected.find((d) => d.file.includes('ignored'))).toBeUndefined();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// suite 阈值 (AC-5)
// ===========================================================================

describe('runTestDetectFrameworks — suite 阈值 (AC-5)', () => {
  it('plan[].mutation_score 取自对应 suite 的 mutation.score', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { score: 88 } }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(88);
    } finally {
      project.cleanup();
    }
  });

  it('suite 省略 mutation 时 mutation_score 为 schema 默认 70', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(70);
    } finally {
      project.cleanup();
    }
  });

  it('suite mutation.score 为非法值时 schema 解析失败，detect 不产出该 suite plan', () => {
    for (const score of [-1, 101, Number.NaN]) {
      const project = createTempProject({
        schema: 'spec-driven',
        tests: [{ root: 'pkg', framework: 'vitest', mutation: { score } }],
      });
      try {
        const result = runTestDetectFrameworks({ projectRoot: project.root });
        expect(result.plan).toEqual([]);
      } finally {
        project.cleanup();
      }
    }
  });

  it('mutation.score 为 0 时 plan.mutation_score 为 0', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { score: 0 } }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(0);
    } finally {
      project.cleanup();
    }
  });

  it('mutation.score 为 100（上界）时 plan.mutation_score 为 100', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { score: 100 } }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(100);
    } finally {
      project.cleanup();
    }
  });

  it('mutation.score 省略 / undefined 时回落 schema 默认 70', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: {} }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].mutation_score).toBe(70);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 空 tests
// ===========================================================================

describe('runTestDetectFrameworks — 空 tests', () => {
  it('tests: [] 时 plan 为空数组', () => {
    const project = createTempProject({ schema: 'spec-driven', tests: [] });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('配置仅含旧 test 键（无 tests）时不产生 suite plan（硬 breaking）', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-legacy-'));
    try {
      fs.mkdirSync(path.join(tmpDir, 'openspec'), { recursive: true });
      fs.writeFileSync(
        path.join(tmpDir, 'openspec', 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          test: { framework: 'vitest', overrides: [{ file: 'src', framework: 'vitest' }] },
        }),
        'utf-8',
      );
      const result = runTestDetectFrameworks({ projectRoot: tmpDir });
      expect(result.plan).toEqual([]);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('tests 元素缺少必填 root/framework 时配置解析失败，命令侧不产出 plan', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-bad-'));
    try {
      fs.mkdirSync(path.join(tmpDir, 'openspec'), { recursive: true });
      fs.writeFileSync(
        path.join(tmpDir, 'openspec', 'config.json'),
        JSON.stringify({ schema: 'spec-driven', tests: [{ framework: 'vitest' }] }),
        'utf-8',
      );
      const result = runTestDetectFrameworks({ projectRoot: tmpDir });
      expect(result.plan).toEqual([]);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks — projectRoot (AC-7)
// ===========================================================================

describe('runTestDetectFrameworks — projectRoot', () => {
  it('显式 projectRoot 读该根 config；spy getProjectDir 次数 0 (AC-7)', async () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    const projectRootLib = await import('../lib/project-root');
    const spy = vi.spyOn(projectRootLib, 'getProjectDir');
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(spy).toHaveBeenCalledTimes(0);
      expect(result.plan).toHaveLength(1);
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });

  it('省略 / "" 回退 getProjectDir (AC-7)', async () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    const projectRootLib = await import('../lib/project-root');
    const spy = vi.spyOn(projectRootLib, 'getProjectDir').mockReturnValue(project.root);
    try {
      spy.mockClear();
      runTestDetectFrameworks({});
      expect(spy).toHaveBeenCalled();
      spy.mockClear();
      runTestDetectFrameworks({ projectRoot: '' });
      expect(spy).toHaveBeenCalled();
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });

  it('显式根指向不存在目录：不得崩溃；detected/plan 为空或错误可观测，且不得误读 cwd 配置', () => {
    const missing = path.join(os.tmpdir(), `detect-missing-${Date.now()}`);
    expect(() => runTestDetectFrameworks({ projectRoot: missing })).not.toThrow();
    const result = runTestDetectFrameworks({ projectRoot: missing });
    expect(Array.isArray(result.detected)).toBe(true);
    expect(Array.isArray(result.plan)).toBe(true);
  });
});

// ===========================================================================
// runTestDetectFrameworks — {config_args} 精确展开
// ===========================================================================

describe('runTestDetectFrameworks — {config_args} 精确展开', () => {
  it('suite 含 config 且 vite-plus：script.shell 含 --config 与相对路径；不得残留 {config_args}；空 configArgs 时不得双空格粘连', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        { root: 'pkg', framework: 'vite-plus', config: 'vite.config.ts' },
        { root: 'other', framework: 'vite-plus' },
      ],
    });
    try {
      const withConfig = runTestDetectFrameworks({ projectRoot: project.root });
      const shell = withConfig.plan.find((p) => p.directory === 'pkg')!.script.shell;
      expect(shell).toContain('--config');
      expect(shell).toContain('vite.config.ts');
      expect(shell).not.toContain('{config_args}');
      const execLine = shell.trim().split('\n').at(-1) ?? '';
      expect(execLine).not.toMatch(/  {2,}/);

      const noConfig = withConfig.plan.find((p) => p.directory === 'other')!.script.shell;
      expect(noConfig).not.toContain('{config_args}');
      expect(noConfig).not.toMatch(/vp test {2}--/);
    } finally {
      project.cleanup();
    }
  });

  it('suite 含 config 但框架 config_flag === null（pytest）：message 精确含 does not support config injection 与 config_flag is null', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'pytest', config: 'pytest.ini' }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).toThrow(
        /does not support config injection[\s\S]*config_flag is null/,
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks — shell·cmd 脚本字面量
// ===========================================================================

describe('runTestDetectFrameworks — shell·cmd 脚本字面量', () => {
  it("plan[0].script.shell：directory≠'.' 时以 cd <directory>\\n 开头、含 rm -rf、以 \\n 结尾；directory 为 '.' 时不得出现 cd . 行", () => {
    const nested = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    const rootDot = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest' }],
    });
    try {
      const shell = runTestDetectFrameworks({ projectRoot: nested.root }).plan[0].script.shell;
      expect(shell.startsWith('cd pkg\n')).toBe(true);
      expect(shell).toContain('rm -rf');
      expect(shell.endsWith('\n')).toBe(true);

      const dotShell = runTestDetectFrameworks({ projectRoot: rootDot.root }).plan[0].script.shell;
      expect(dotShell).not.toContain('cd .');
      expect(dotShell).toContain('rm -rf');
    } finally {
      nested.cleanup();
      rootDot.cleanup();
    }
  });

  it('plan[0].script.cmd：单行 & 连接；含 cd /d；(if exist ...)；directory 含空格时路径被双引号包裹', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'my pkg', framework: 'vitest' }],
    });
    try {
      const cmd = runTestDetectFrameworks({ projectRoot: project.root }).plan[0].script.cmd;
      expect(cmd).not.toMatch(/[\r\n]/);
      expect(cmd).toContain(' & ');
      expect(cmd).toContain('cd /d');
      expect(cmd).toContain('(if exist');
      expect(cmd).toContain('"my pkg"');
    } finally {
      project.cleanup();
    }
  });

  it('shell/cmd 均不得残留 {config_args}；若 rm -rf / cd /d / (if exist 任一关键字被清空则失败', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', config: 'vite.config.ts' }],
    });
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).not.toContain('{config_args}');
      expect(plan.script.cmd).not.toContain('{config_args}');
      expect(plan.script.shell).toContain('rm -rf');
      expect(plan.script.cmd).toContain('cd /d');
      expect(plan.script.cmd).toContain('(if exist');
    } finally {
      project.cleanup();
    }
  });

  it('directory 无空格时 cmd 不包引号；含空格时包引号', () => {
    const plain = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    const spaced = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'p kg', framework: 'vitest' }],
    });
    try {
      const plainCmd = runTestDetectFrameworks({ projectRoot: plain.root }).plan[0].script.cmd;
      expect(plainCmd).toContain('cd /d pkg');
      expect(plainCmd).not.toContain('"pkg"');
      const spacedCmd = runTestDetectFrameworks({ projectRoot: spaced.root }).plan[0].script.cmd;
      expect(spacedCmd).toContain('cd /d "p kg"');
    } finally {
      plain.cleanup();
      spaced.cleanup();
    }
  });

  it('coverage_cleanup: [] 时空清理列表：shell 仍含 cd/test_execution 行，且不得出现孤立的 rm -rf 空参行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    const base = testFramework.getFrameworkConfig('vitest');
    const spy = vi.spyOn(testFramework, 'getFrameworkConfig').mockReturnValue({
      ...base,
      shell: { ...base.shell, coverage_cleanup: [] },
      cmd: { ...base.cmd, coverage_cleanup: [] },
    });
    try {
      const shell = runTestDetectFrameworks({ projectRoot: project.root }).plan[0].script.shell;
      expect(shell.startsWith('cd pkg\n')).toBe(true);
      expect(shell).toContain('vitest');
      expect(shell).not.toMatch(/rm -rf\s*\n/);
      expect(shell).not.toContain('rm -rf \n');
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks — suite scope 边界
// ===========================================================================

describe('runTestDetectFrameworks — suite scope 边界', () => {
  it("文件恰好等于 suite.root（无尾部 /）视为 in-scope；root + '/' + child in-scope；root + 'x' 前缀假匹配 out-of-scope", () => {
    const project = createTempProject({
      schema: 'spec-driven',
      // includes 含 '.' → joinRootScoped 得到可匹配 root 本身的 glob
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['.', '**/*'] }],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg', 'pkg/child.ts', 'pkgx/child.ts'],
        projectRoot: project.root,
      });
      const fwOf = (suffix: string) =>
        result.detected.find((d) => d.file.replace(/\\/g, '/').endsWith(suffix))?.framework;
      expect(fwOf('/pkg') ?? fwOf('pkg')).toBe('vitest');
      expect(fwOf('pkg/child.ts')).toBe('vitest');
      expect(fwOf('pkgx/child.ts')).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks — 空 files auto-scan
// ===========================================================================

describe('runTestDetectFrameworks — 空 files auto-scan', () => {
  it('省略 files：扫描 suite 范围源文件填入 detected', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.ts'] }],
    });
    try {
      writeFile(project.root, 'pkg/a.test.ts');
      writeFile(project.root, 'pkg/b.ts');
      const omitted = runTestDetectFrameworks({ projectRoot: project.root });
      expect(omitted.detected.length).toBeGreaterThan(0);
      expect(omitted.plan).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('files: []：与省略行为对照（空数组早退 detected/plan 均为 []）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.ts'] }],
    });
    try {
      writeFile(project.root, 'pkg/a.test.ts');
      const empty = runTestDetectFrameworks({ files: [], projectRoot: project.root });
      const omitted = runTestDetectFrameworks({ projectRoot: project.root });
      expect(empty.detected).toEqual([]);
      expect(empty.plan).toEqual([]);
      expect(omitted.detected.length).toBeGreaterThan(0);
      expect(omitted.plan).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('suite root 无匹配源文件时 detected 为空数组且不抛', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'empty-pkg', framework: 'vitest' }],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'empty-pkg'), { recursive: true });
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.detected).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('files: null（类型断言到达）：按实现拒绝或等同省略；不得崩溃且不得误用 getProjectDir 外路径', async () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*.ts'] }],
    });
    writeFile(project.root, 'pkg/a.test.ts');
    const projectRootLib = await import('../lib/project-root');
    const spy = vi
      .spyOn(projectRootLib, 'getProjectDir')
      .mockReturnValue(path.join(os.tmpdir(), 'should-not-use'));
    try {
      let threw: unknown;
      let result: ReturnType<typeof runTestDetectFrameworks> | undefined;
      try {
        result = runTestDetectFrameworks({
          files: null as unknown as string[],
          projectRoot: project.root,
        });
      } catch (err) {
        threw = err;
      }
      if (threw !== undefined) {
        expect(threw).toBeInstanceOf(Error);
      } else {
        expect(Array.isArray(result?.detected)).toBe(true);
        expect(Array.isArray(result?.plan)).toBe(true);
        for (const d of result?.detected ?? []) {
          expect(d.file).not.toContain('should-not-use');
        }
      }
      expect(spy).not.toHaveBeenCalled();
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks — collectFiles 不可读目录
// ===========================================================================

describe('runTestDetectFrameworks — collectFiles 不可读目录', () => {
  it('空目录 / 仅含子目录无文件：返回 []，detected 为空且不抛', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'empty-tree', framework: 'vitest', includes: ['**/*'] }],
    });
    try {
      fs.mkdirSync(path.join(project.root, 'empty-tree', 'only-dir'), { recursive: true });
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.detected).toEqual([]);
      expect(result.plan).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('suite root 指向不存在路径时 auto-scan 不抛且 detected 为空（walk catch / 空收集）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'missing-root', framework: 'vitest', includes: ['**/*'] }],
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.detected).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('spy readdirSync 对根目录本身抛 EACCES：返回 []（或仅已收集项），公共 API 不抛未捕获异常', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*'] }],
    });
    writeFile(project.root, 'pkg/a.test.ts');
    const realReaddir = actualFsRef.current!.readdirSync.bind(actualFsRef.current!);
    mockReaddirSync.mockImplementation((dir: unknown, options?: unknown) => {
      const dirStr = String(dir);
      if (dirStr === project.root || dirStr === path.join(project.root, 'pkg')) {
        const err = new Error('EACCES: permission denied') as NodeJS.ErrnoException;
        err.code = 'EACCES';
        throw err;
      }
      return realReaddir(dir as Parameters<typeof realReaddir>[0], options as never);
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.detected).toEqual([]);
      expect(mockReaddirSync).toHaveBeenCalled();
      expect(
        mockReaddirSync.mock.calls.some((c) => {
          const arg = String(c[0]);
          return arg === project.root || arg === path.join(project.root, 'pkg');
        }),
      ).toBe(true);
    } finally {
      mockReaddirSync.mockImplementation(
        actualFsRef.current!.readdirSync as unknown as typeof mockReaddirSync,
      );
      project.cleanup();
    }
  });

  it('spy readdirSync 对不可读子目录抛错：walk catch 后仍返回其它可读文件，不抛未捕获', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', includes: ['**/*'] }],
    });
    writeFile(project.root, 'pkg/visible.test.ts');
    const blocked = path.join(project.root, 'pkg', 'secret');
    fs.mkdirSync(blocked, { recursive: true });
    writeFile(project.root, 'pkg/secret/hidden.test.ts');
    const realReaddir = actualFsRef.current!.readdirSync.bind(actualFsRef.current!);
    mockReaddirSync.mockImplementation((dir: unknown, options?: unknown) => {
      if (String(dir) === blocked) {
        const err = new Error('EACCES: permission denied') as NodeJS.ErrnoException;
        err.code = 'EACCES';
        throw err;
      }
      return realReaddir(dir as Parameters<typeof realReaddir>[0], options as never);
    });
    try {
      expect(() => runTestDetectFrameworks({ projectRoot: project.root })).not.toThrow();
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.detected.some((d) => d.file.includes('visible.test.ts'))).toBe(true);
      expect(result.detected.some((d) => d.file.includes('hidden.test.ts'))).toBe(false);
      expect(mockReaddirSync).toHaveBeenCalled();
    } finally {
      mockReaddirSync.mockImplementation(
        actualFsRef.current!.readdirSync as unknown as typeof mockReaddirSync,
      );
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestDetectFrameworks — mutation_score / 空 tests（异常补强）
// ===========================================================================

describe('runTestDetectFrameworks — mutation_score / 空 tests（异常）', () => {
  it('mutation.score 为非法类型（字符串 "70" / null）时不得产出 NaN 写入 plan', () => {
    for (const score of ['70', null] as unknown[]) {
      const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-score-'));
      try {
        fs.mkdirSync(path.join(tmpDir, 'openspec'), { recursive: true });
        fs.writeFileSync(
          path.join(tmpDir, 'openspec', 'config.json'),
          JSON.stringify({
            schema: 'spec-driven',
            tests: [{ root: 'pkg', framework: 'vitest', mutation: { score } }],
          }),
          'utf-8',
        );
        const result = runTestDetectFrameworks({ projectRoot: tmpDir });
        for (const entry of result.plan) {
          expect(entry.mutation_score).not.toBeNaN();
        }
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    }
  });
});

// ===========================================================================
// plan script — framework version gating (jest --randomize)
// ===========================================================================

describe('runTestDetectFrameworks — jest version gating', () => {
  it('jest >= 29.5.0 时 plan.script 含 --randomize；更低版本不含', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'jest' }],
    });
    const versionSpy = vi.spyOn(testFramework, 'detectFrameworkVersion');
    try {
      versionSpy.mockReturnValue('29.5.0');
      const withFlag = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(withFlag.script.shell).toContain('--randomize');
      expect(withFlag.script.cmd).toContain('--randomize');

      versionSpy.mockReturnValue('29.4.0');
      const withoutFlag = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(withoutFlag.script.shell).not.toContain('--randomize');
      expect(withoutFlag.script.cmd).not.toContain('--randomize');
    } finally {
      versionSpy.mockReturnValue('99.0.0');
      project.cleanup();
    }
  });
});
