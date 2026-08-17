/**
 * Unit tests for lib/test-plan — planId, pathFilter, file scoping, suite scope.
 */

import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { configSchema, type OpenSpecConfig, type TestSuite } from '../schemas';
import {
  derivePlanId,
  findSuite,
  isInSuiteScope,
  pathFilterFromPlan,
  resolveAllSuites,
  resolvePlanFiles,
} from './test-plan';

function suite(overrides: Partial<TestSuite> & Pick<TestSuite, 'root' | 'framework'>): TestSuite {
  return {
    cwd: '.',
    coverage: { lines: 80, branches: 70, functions: 75 },
    mutation: { score: 70 },
    ...overrides,
  };
}

function config(tests: TestSuite[]): OpenSpecConfig {
  return configSchema.parse({ schema: 'spec-driven', tests });
}

describe('derivePlanId', () => {
  it("root='.' → framework only", () => {
    expect(derivePlanId('.', 'vitest')).toBe('vitest');
  });

  it('nested root sanitizes separators', () => {
    expect(derivePlanId('plugins/dev-team/bin/src', 'vite-plus')).toBe(
      'plugins_dev-team_bin_src_vite-plus',
    );
  });

  it('backslash root normalizes like forward slash', () => {
    expect(derivePlanId('plugins\\dev-team\\bin', 'vitest')).toBe('plugins_dev-team_bin_vitest');
  });
});

describe('pathFilterFromPlan', () => {
  it('cwd === root → "."', () => {
    expect(pathFilterFromPlan({ cwd: 'pkg', root: 'pkg' })).toBe('.');
  });

  it('cwd parent of root → relative child path', () => {
    expect(
      pathFilterFromPlan({ cwd: 'plugins/dev-team/bin', root: 'plugins/dev-team/bin/src' }),
    ).toBe('src');
  });
});

describe('resolvePlanFiles', () => {
  it('keeps files under root and rewrites relative to cwd', () => {
    expect(resolvePlanFiles(['a/b/x.test.ts', 'a/c/y.test.ts'], { cwd: 'a', root: 'a/b' })).toEqual(
      ['b/x.test.ts'],
    );
  });

  it('undefined files stays undefined', () => {
    expect(resolvePlanFiles(undefined, { cwd: '.', root: '.' })).toBeUndefined();
  });
});

describe('resolveAllSuites / findSuite', () => {
  const projectRoot = path.resolve('/virtual-project-root');

  it('resolveAllSuites maps cwd parent of root', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: 'plugins/dev-team/bin/src', cwd: '..', framework: 'vite-plus' })],
      projectRoot,
    );
    expect(resolved.root).toBe('plugins/dev-team/bin/src');
    expect(resolved.cwd).toBe('plugins/dev-team/bin');
    expect(resolved.frameworkConfig.framework).toBe('vite-plus');
  });

  it('preserves order; findSuite matches root then framework', () => {
    const a = suite({ root: 'pkg-a', framework: 'vitest' });
    const b = suite({ root: 'pkg-b', framework: 'jest' });
    const cfg = config([a, b]);
    const resolved = resolveAllSuites(cfg.tests ?? [], projectRoot);
    expect(resolved).toHaveLength(2);
    expect(findSuite(cfg, 'vitest', 'pkg-a')?.root).toBe('pkg-a');
    expect(findSuite(cfg, 'jest')?.root).toBe('pkg-b');
    expect(findSuite(cfg)?.root).toBe('pkg-a');
  });
});

describe('resolveAllSuites -- 缺省 LCA', () => {
  const projectRoot = path.resolve('/virtual-project-root');

  it('root=pkg、cwd=.、config=vite.config.ts、未设 mutation.cwd → mutationCwd 为 pkg（AC-1）', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg',
          cwd: '.',
          config: 'vite.config.ts',
          framework: 'vitest',
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg');
    expect(resolved.cwd).toBe('pkg');
  });

  it('root=pkg/src、cwd=.、config=../vitest.config.ts → mutationCwd 为 pkg（AC-2）', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg/src',
          cwd: '.',
          config: '../vitest.config.ts',
          framework: 'vitest',
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg');
  });

  it('root=pkg/src、cwd=..、无 config → mutationCwd 为 pkg（AC-3 / AC-5）', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: 'pkg/src', cwd: '..', framework: 'vitest' })],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg');
    expect(resolved.mutationCwd).not.toBe('pkg/src');
  });

  it('两条 suite 各自 cwd=.. → 均 mutationCwd 为 pkg，互不合并', () => {
    const resolved = resolveAllSuites(
      [
        suite({ root: 'pkg/a', cwd: '..', framework: 'vitest' }),
        suite({ root: 'pkg/b', cwd: '..', framework: 'jest' }),
      ],
      projectRoot,
    );
    expect(resolved[0].mutationCwd).toBe('pkg');
    expect(resolved[1].mutationCwd).toBe('pkg');
  });

  it('同一输入连续调用两次 → mutationCwd 相同（纯函数）', () => {
    const suites = [suite({ root: 'pkg/src', cwd: '..', framework: 'vitest' })];
    const first = resolveAllSuites(suites, projectRoot);
    const second = resolveAllSuites(suites, projectRoot);
    expect(first[0].mutationCwd).toBe(second[0].mutationCwd);
  });

  it('framework 非法 → 抛错含 Unknown framework', () => {
    for (const bad of ['unknown', '', 'jest '] as const) {
      expect(() =>
        resolveAllSuites(
          [suite({ root: 'pkg', framework: bad as TestSuite['framework'] })],
          projectRoot,
        ),
      ).toThrow(/Unknown framework/);
    }
  });

  it('suites 为 undefined / null → 抛 TypeError', () => {
    expect(() => resolveAllSuites(undefined as unknown as TestSuite[], projectRoot)).toThrow();
    expect(() => resolveAllSuites(null as unknown as TestSuite[], projectRoot)).toThrow();
  });

  it('suites=[] → 返回 []', () => {
    expect(resolveAllSuites([], projectRoot)).toEqual([]);
  });

  it('单元素 root=pkg、cwd=.、无 config → mutationCwd 为 pkg', () => {
    const [resolved] = resolveAllSuites([suite({ root: 'pkg', framework: 'vitest' })], projectRoot);
    expect(resolved.mutationCwd).toBe('pkg');
  });

  it('超大列表（>100 条）→ 每条 mutationCwd 独立正确', () => {
    const suites = Array.from({ length: 101 }, (_, i) =>
      suite({ root: `pkg/s${i}`, cwd: '..', framework: 'vitest' }),
    );
    const resolved = resolveAllSuites(suites, projectRoot);
    expect(resolved).toHaveLength(101);
    for (const r of resolved) {
      expect(r.mutationCwd).toBe('pkg');
    }
  });

  it('projectRoot 为空串 → 不崩溃且 mutationCwd 为 POSIX 相对路径', () => {
    const [resolved] = resolveAllSuites([suite({ root: 'pkg', framework: 'vitest' })], '');
    expect(resolved.mutationCwd).toBe('pkg');
    expect(resolved.mutationCwd).not.toMatch(/^[A-Za-z]:\\$/);
  });

  it('projectRoot 超长或含 emoji / \\n → 不崩溃', () => {
    const longRoot = path.resolve(`/tmp/${'a'.repeat(1001)}`);
    const [longResolved] = resolveAllSuites(
      [suite({ root: 'pkg', framework: 'vitest' })],
      longRoot,
    );
    expect(longResolved.mutationCwd).toBe('pkg');

    const emojiRoot = path.resolve('/tmp/emoji\n🧪');
    const [emojiResolved] = resolveAllSuites(
      [suite({ root: 'pkg', framework: 'vitest' })],
      emojiRoot,
    );
    expect(emojiResolved.mutationCwd).toBe('pkg');
  });

  it('suite.config 为 undefined → 第三元不参与 LCA（AC-5）', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: 'pkg/src', cwd: '..', framework: 'vitest' })],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg');
  });

  it('root=pkg/a/b、cwd=../..、config=../x.ts → mutationCwd 为 pkg（三者 LCA）', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg/a/b',
          cwd: '../..',
          config: '../x.ts',
          framework: 'vitest',
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg');
  });

  it('cwd 含反斜杠 → LCA 与正斜杠等价', () => {
    const [forward] = resolveAllSuites(
      [suite({ root: 'pkg/src', cwd: '../..', framework: 'vitest' })],
      projectRoot,
    );
    const [backslash] = resolveAllSuites(
      [suite({ root: 'pkg/src', cwd: '..\\..', framework: 'vitest' })],
      projectRoot,
    );
    expect(backslash.mutationCwd).toBe(forward.mutationCwd);
  });
});

describe('resolveAllSuites -- 显式 mutation.cwd 覆盖', () => {
  const projectRoot = path.resolve('/virtual-project-root');

  it('mutation.cwd=. → mutationCwd 为 pkg/src，不是 LCA pkg（AC-4）', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg/src',
          cwd: '..',
          framework: 'vitest',
          mutation: { cwd: '.', score: 70 },
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg/src');
    expect(resolved.mutationCwd).not.toBe('pkg');
  });

  it('mutation.cwd=../.. → 按 path.resolve 解析，不被改写为 LCA（AC-4）', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg/src',
          cwd: '.',
          framework: 'vitest',
          mutation: { cwd: '../..', score: 70 },
        }),
      ],
      projectRoot,
    );
    const expected = path
      .relative(projectRoot, path.resolve(projectRoot, 'pkg/src', '../..'))
      .replace(/\\/g, '/');
    expect(resolved.mutationCwd).toBe(expected === '' ? '.' : expected);
  });

  it('显式覆盖场景下 framework 非法 → 仍先抛 Unknown framework', () => {
    expect(() =>
      resolveAllSuites(
        [
          suite({
            root: 'pkg/src',
            framework: 'unknown' as TestSuite['framework'],
            mutation: { cwd: '.', score: 70 },
          }),
        ],
        projectRoot,
      ),
    ).toThrow(/Unknown framework/);
  });

  it('mutation.cwd 为空串 → absRoot，跳过 LCA', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg/src',
          cwd: '..',
          framework: 'vitest',
          mutation: { cwd: '', score: 70 },
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg/src');
  });

  it('mutation.cwd 超长 → 按 path.resolve 解析', () => {
    const deep = '../'.repeat(600);
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: 'pkg/src',
          cwd: '.',
          framework: 'vitest',
          mutation: { cwd: deep, score: 70 },
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).not.toBe('pkg');
  });

  it('mutation.cwd 含特殊字符 → path.resolve 后不抛未捕获异常', () => {
    expect(() =>
      resolveAllSuites(
        [
          suite({
            root: 'pkg',
            framework: 'vitest',
            mutation: { cwd: 'sub\n🧪', score: 70 },
          }),
        ],
        projectRoot,
      ),
    ).not.toThrow();
  });

  it('mutation.cwd 为 undefined → 走缺省 LCA', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: 'pkg/src', cwd: '..', framework: 'vitest', mutation: { score: 70 } })],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('pkg');
  });

  it('显式 mutation.cwd=.. 使结果高于 projectRoot → 不 clamp', () => {
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: '.',
          framework: 'vitest',
          mutation: { cwd: '..', score: 70 },
        }),
      ],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('..');
  });
});

describe('resolveAllSuites -- LCA 上沿 clamp', () => {
  const projectRoot = path.resolve('/virtual-project-root');

  it('root=.、cwd=..、未设 mutation.cwd → mutationCwd 为 .（AC-10）', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: '.', cwd: '..', framework: 'vitest' })],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('.');
    expect(resolved.mutationCwd).not.toBe('..');
  });

  it('缺省 LCA 绝对路径为 projectRoot 自身 → mutationCwd 为 .', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: '.', cwd: '.', framework: 'vitest' })],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('.');
  });

  it('cwd=../.. 使 LCA 高于仓库根 → clamp 为 .', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: '.', cwd: '../..', framework: 'vitest' })],
      projectRoot,
    );
    expect(resolved.mutationCwd).toBe('.');
    expect(resolved.mutationCwd).not.toBe('../..');
  });

  it('projectRoot 为 undefined / null → 抛错', () => {
    const suites = [suite({ root: 'pkg', framework: 'vitest' })];
    expect(() => resolveAllSuites(suites, undefined as unknown as string)).toThrow();
    expect(() => resolveAllSuites(suites, null as unknown as string)).toThrow();
  });

  it('cwd=0（非字符串强转）→ path.resolve 抛错，不得当作合法覆盖', () => {
    expect(() =>
      resolveAllSuites(
        [
          suite({
            root: '.',
            cwd: 0 as unknown as TestSuite['cwd'],
            framework: 'vitest',
          }),
        ],
        projectRoot,
      ),
    ).toThrow();
  });
});

describe('resolveAllSuites -- 跨盘抛错', () => {
  const projectRoot = path.resolve('/virtual-project-root');

  function expectNoDriveRoot(mutationCwd: string): void {
    expect(mutationCwd).not.toBe('C:\\');
    expect(mutationCwd).not.toBe('D:\\');
    expect(mutationCwd).not.toBe('/');
  }

  it('无共同祖先且未设 mutation.cwd → 抛错（AC-11）', () => {
    if (process.platform === 'win32') {
      const winRoot = path.resolve('C:\\virtual-project-root');
      expect(() =>
        resolveAllSuites(
          [suite({ root: '.', cwd: 'D:\\no-common', framework: 'vitest' })],
          winRoot,
        ),
      ).toThrow(/mutation_cwd|common ancestor|drive/i);
    } else {
      expect(() =>
        resolveAllSuites([suite({ root: '.', cwd: '/', framework: 'vitest' })], projectRoot),
      ).toThrow(/mutation_cwd|common ancestor/i);
    }
  });

  it('跨盘抛错后 message 含 mutation_cwd，不得返回盘符根', () => {
    try {
      if (process.platform === 'win32') {
        resolveAllSuites(
          [suite({ root: '.', cwd: 'D:\\no-common', framework: 'vitest' })],
          path.resolve('C:\\virtual-project-root'),
        );
      } else {
        resolveAllSuites([suite({ root: '.', cwd: '/', framework: 'vitest' })], projectRoot);
      }
      expect.fail('应抛错');
    } catch (e) {
      const msg = (e as Error).message;
      expect(msg).toMatch(/mutation_cwd|common ancestor|drive/i);
      expectNoDriveRoot(msg);
    }
  });

  it('config 在另一盘符导致无共同祖先 → 同样抛错', () => {
    if (process.platform !== 'win32') return;
    expect(() =>
      resolveAllSuites(
        [
          suite({
            root: 'pkg',
            cwd: '.',
            config: 'D:\\other\\vitest.config.ts',
            framework: 'vitest',
          }),
        ],
        path.resolve('C:\\virtual-project-root'),
      ),
    ).toThrow(/mutation_cwd|common ancestor|drive/i);
  });

  it('已设 mutation.cwd 时跨盘 cwd → 不走 LCA，不因跨盘抛错', () => {
    if (process.platform !== 'win32') return;
    const [resolved] = resolveAllSuites(
      [
        suite({
          root: '.',
          cwd: 'D:\\no-common',
          framework: 'vitest',
          mutation: { cwd: '.', score: 70 },
        }),
      ],
      path.resolve('C:\\virtual-project-root'),
    );
    expect(resolved.mutationCwd).toBe('.');
  });
});

describe('configSchema -- mutation.cwd 省略与文档', () => {
  it('省略 mutation.cwd → parse 后为 undefined；resolveAllSuites 走 LCA（AC-8）', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    expect(parsed.tests[0].mutation.cwd).toBeUndefined();
    const [resolved] = resolveAllSuites(parsed.tests, path.resolve('/virtual-project-root'));
    expect(resolved.mutationCwd).toBe('pkg');
  });

  it('toJSONSchema 中 mutation.cwd description 表明覆盖自动 LCA（AC-8）', () => {
    const jsonSchema = configSchema.toJSONSchema({ io: 'input' }) as {
      properties?: {
        tests?: {
          items?: {
            properties?: {
              mutation?: {
                properties?: { cwd?: { description?: string } };
              };
            };
          };
        };
      };
    };
    const desc =
      jsonSchema.properties?.tests?.items?.properties?.mutation?.properties?.cwd?.description ?? '';
    expect(desc).toMatch(/LCA|自动/i);
    expect(desc).not.toMatch(/默认等于\s*cwd/i);
  });

  it('parse 显式 mutation:{ cwd:., score:50 } → 字段保留', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { cwd: '.', score: 50 } }],
    });
    expect(parsed.tests[0].mutation.cwd).toBe('.');
    expect(parsed.tests[0].mutation.score).toBe(50);
  });

  it('mutation.cwd 为 null / 123 → configSchema.parse 抛 ZodError', () => {
    for (const bad of [null, 123] as const) {
      expect(() =>
        configSchema.parse({
          schema: 'spec-driven',
          tests: [{ root: 'pkg', framework: 'vitest', mutation: { cwd: bad } }],
        }),
      ).toThrow();
    }
  });

  it('mutation:{} → cwd 仍为 undefined', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: {} }],
    });
    expect(parsed.tests[0].mutation.cwd).toBeUndefined();
  });

  it('mutation 省略 → mutation 存在但 cwd 为 undefined', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    expect(parsed.tests[0].mutation).toBeDefined();
    expect(parsed.tests[0].mutation.cwd).toBeUndefined();
  });

  it('mutation.cwd 为空串 → parse 成功且值为空串', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest', mutation: { cwd: '' } }],
    });
    expect(parsed.tests[0].mutation.cwd).toBe('');
  });

  it('多余未知字段（passthrough）不影响 mutation.cwd 省略语义', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      extraField: 'ignored',
      tests: [{ root: 'pkg', framework: 'vitest', custom: true }],
    });
    expect(parsed.tests[0].mutation.cwd).toBeUndefined();
  });
});

describe('isInSuiteScope', () => {
  it('matches under root + default_glob; suite-local exclude without config', () => {
    const s = suite({
      root: 'pkg',
      framework: 'vitest',
      excludes: ['**/skip/**'],
    });
    expect(isInSuiteScope('pkg/foo.test.ts', s)).toBe(true);
    expect(isInSuiteScope('pkg/skip/x.test.ts', s)).toBe(false);
    expect(isInSuiteScope('other/foo.test.ts', s)).toBe(false);
  });

  it('with config uses any-suite excludes (isFileExcluded)', () => {
    const a = suite({ root: 'a', framework: 'vitest', excludes: ['**/legacy/**'] });
    const b = suite({ root: 'b', framework: 'vitest' });
    const cfg = config([a, b]);
    // File under b, but excluded by suite a's pattern only applies under a —
    // global isFileExcluded still keys off each suite's root.
    expect(isInSuiteScope('b/foo.test.ts', b, cfg)).toBe(true);
    expect(isInSuiteScope('a/legacy/old.test.ts', a, cfg)).toBe(false);
  });
});
