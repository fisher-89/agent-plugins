/**
 * Tests for lib/test-runner — preparePlanArtifacts / executePlanEntry
 * (reports/test layout, file-channel parse, conditional redirect).
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const mockExecSync = vi.fn();
vi.mock('child_process', () => ({
  execSync: (...args: unknown[]) => mockExecSync(...args),
}));

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import type { TestPlan } from '../schemas';
import { getFrameworkConfig } from './test-framework';
import { executePlanEntry } from './test-runner';

function createTempDir(): { root: string; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'test-runner-'));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeMinimalJsResults(reportDir: string, passed = true): void {
  fs.mkdirSync(reportDir, { recursive: true });
  fs.writeFileSync(
    path.join(reportDir, 'results.json'),
    JSON.stringify({
      testResults: [
        {
          name: 'foo.test.ts',
          assertionResults: [
            {
              title: 't1',
              fullName: 't1',
              status: passed ? 'passed' : 'failed',
              failureMessages: passed ? [] : ['fail'],
            },
          ],
        },
      ],
    }),
    'utf-8',
  );
}

function writeIstanbulCoverage(reportDir: string): void {
  fs.writeFileSync(
    path.join(reportDir, 'coverage-summary.json'),
    JSON.stringify({
      total: {
        lines: { pct: 90 },
        branches: { pct: 80 },
        functions: { pct: 85 },
      },
    }),
    'utf-8',
  );
}

function makePlan(overrides: Partial<TestPlan> = {}): TestPlan {
  const fw = overrides.framework ?? 'vitest';
  const cfg = getFrameworkConfig(fw);
  return {
    directory: '.',
    framework: fw,
    coverage_format: cfg.coverage_format,
    coverage_output: cfg.coverage_output,
    script: {
      shell: cfg.shell.test_execution('29.5.0'),
      cmd: cfg.cmd.test_execution('29.5.0'),
    },
    ...overrides,
  };
}

function capturedCmd(): string {
  expect(mockExecSync).toHaveBeenCalled();
  return String(mockExecSync.mock.calls[0][0]);
}

function writeMinimalTextResults(reportDir: string, fileName: string): void {
  fs.mkdirSync(reportDir, { recursive: true });
  fs.writeFileSync(path.join(reportDir, fileName), '1 [PASS] a\n', 'utf-8');
}

// ===========================================================================
// executePlanEntry
// ===========================================================================

describe('executePlanEntry', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('vite-plus：命令含 results.json 占位路径且无段级 >', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vite-plus'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vite-plus' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).toContain('results.json');
      expect(cmd).not.toMatch(/>\s*"/);
    } finally {
      dir.cleanup();
    }
  });

  it('bun：命令含段级重定向与 --config；临时 bunfig 含 lcov；结束后清理', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      let seenTempBunfig: string | null = null;
      mockExecSync.mockImplementation((cmd: unknown) => {
        const cmdStr = String(cmd);
        const match = cmdStr.match(/bunfig\.dev-team-[a-f0-9]+\.toml/);
        if (match) {
          seenTempBunfig = path.join(dir.root, match[0]);
          expect(fs.existsSync(seenTempBunfig)).toBe(true);
          const content = fs.readFileSync(seenTempBunfig, 'utf-8');
          expect(content).toContain('coverageReporter');
          expect(content).toContain('lcov');
        }
        writeMinimalTextResults(path.join(reportsDir, 'bun'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).toContain('--config');
      expect(cmd).toMatch(/>\s*"/);
      expect(seenTempBunfig).toBeTruthy();
      expect(fs.existsSync(seenTempBunfig!)).toBe(false);
    } finally {
      dir.cleanup();
    }
  });

  it('go/rust/pytest/node-test：命令含段级 > 重定向', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const resultsName: Record<string, string> = {
        go: 'results.ndjson',
        rust: 'results.txt',
        pytest: 'results.txt',
        'node-test': 'results.txt',
      };
      for (const framework of ['go', 'rust', 'pytest', 'node-test'] as const) {
        mockExecSync.mockReset();
        mockExecSync.mockImplementation(() => {
          const planDir = path.join(reportsDir, framework);
          if (framework === 'go') {
            fs.mkdirSync(planDir, { recursive: true });
            fs.writeFileSync(
              path.join(planDir, 'results.ndjson'),
              `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
              'utf-8',
            );
          } else {
            writeMinimalTextResults(planDir, resultsName[framework]);
          }
          return '';
        });
        executePlanEntry(makePlan({ framework }), dir.root, { reportsDir });
        expect(capturedCmd()).toMatch(/>\s*"/);
      }
    } finally {
      dir.cleanup();
    }
  });

  it('未知 framework → error（prepare 失败），不抛未捕获异常', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const result = executePlanEntry(
        {
          directory: '.',
          framework: 'unknown-fw' as TestPlan['framework'],
          coverage_format: 'istanbul',
          coverage_output: 'coverage-summary.json',
          script: { shell: 'echo hi', cmd: 'echo hi' },
        },
        dir.root,
        { reportsDir },
      );
      expect(result.error).toMatch(/Unknown framework/);
      expect(mockExecSync).not.toHaveBeenCalled();
    } finally {
      dir.cleanup();
    }
  });

  it('bun：不修改用户 bunfig.toml', () => {
    const dir = createTempDir();
    try {
      const userBunfig = path.join(dir.root, 'bunfig.toml');
      fs.writeFileSync(userBunfig, '[test]\npreload = []\n', 'utf-8');
      const before = fs.readFileSync(userBunfig, 'utf-8');
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'bun'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      expect(fs.readFileSync(userBunfig, 'utf-8')).toBe(before);
    } finally {
      dir.cleanup();
    }
  });

  it('必填 reportsDir：先 mkdir 并清空该 plan 目录，不影响兄弟 plan 目录', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const sibling = path.join(reportsDir, 'jest');
      const target = path.join(reportsDir, 'vitest');
      fs.mkdirSync(sibling, { recursive: true });
      fs.writeFileSync(path.join(sibling, 'keep.txt'), 'keep', 'utf-8');
      fs.mkdirSync(target, { recursive: true });
      fs.writeFileSync(path.join(target, 'stale.txt'), 'stale', 'utf-8');

      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(target);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(fs.existsSync(path.join(sibling, 'keep.txt'))).toBe(true);
      expect(fs.existsSync(path.join(target, 'stale.txt'))).toBe(false);
      expect(fs.existsSync(path.join(target, 'results.json'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('jest 命令含 --outputFile= 指向 planDir/results.json，不含段级 >', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'jest'));
        return 'noise before json';
      });
      executePlanEntry(makePlan({ framework: 'jest' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).toContain('--outputFile=');
      expect(cmd).toContain('results.json');
      expect(cmd).not.toMatch(/>\s*"/);
    } finally {
      dir.cleanup();
    }
  });

  it('返回 ExecutionResult 含 planId/reportDir/resultsFile', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.planId).toBe('vitest');
      expect(result.reportDir).toBe(path.join(reportsDir, 'vitest'));
      expect(result.resultsFile).toBe(path.join(reportsDir, 'vitest', 'results.json'));
    } finally {
      dir.cleanup();
    }
  });

  it('有侧车 coverage 文件时从 planDir 解析；无则 coverage=null', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'vitest');
        writeMinimalJsResults(planDir);
        writeIstanbulCoverage(planDir);
        return '';
      });
      const withCov = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(withCov.coverage).not.toBeNull();
      expect(withCov.coverage!.lines).toBe(90);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      const noCov = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(noCov.coverage).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('命令失败（非 0）仍调用垂直 parse；半失败结果含 error 与 planId', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        const err = new Error('Command failed') as Error & { status: number; stdout: string };
        err.status = 1;
        err.stdout = '';
        throw err;
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(1);
      expect(result.planId).toBe('vitest');
      expect(result.testCases.length).toBeGreaterThan(0);
      expect(result.error).toBeTruthy();
    } finally {
      dir.cleanup();
    }
  });

  it('显式传入 reportsDir（缺省不可调用）', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      expect(() => executePlanEntry(makePlan(), dir.root, { reportsDir })).not.toThrow();
    } finally {
      dir.cleanup();
    }
  });

  it('planDir 已有旧文件时执行前清空该目录', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'vitest');
      fs.mkdirSync(planDir, { recursive: true });
      fs.writeFileSync(path.join(planDir, 'old-results.json'), '{}', 'utf-8');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(planDir);
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(fs.existsSync(path.join(planDir, 'old-results.json'))).toBe(false);
    } finally {
      dir.cleanup();
    }
  });

  it('bun 执行后 best-effort 删除 tempPaths；用户 bunfig 未被修改', () => {
    const dir = createTempDir();
    try {
      const userBunfig = path.join(dir.root, 'bunfig.toml');
      fs.writeFileSync(userBunfig, '[test]\npreload = ["keep"]\n', 'utf-8');
      const before = fs.readFileSync(userBunfig, 'utf-8');
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const seenTemps: string[] = [];

      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'bun');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(path.join(planDir, 'results.txt'), '1 [PASS] works\n', 'utf-8');
        const temps = fs
          .readdirSync(dir.root)
          .filter((n) => n.startsWith('bunfig.dev-team-'))
          .map((n) => path.join(dir.root, n));
        seenTemps.push(...temps);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      expect(seenTemps.length).toBeGreaterThan(0);
      for (const t of seenTemps) {
        expect(fs.existsSync(t)).toBe(false);
      }
      expect(fs.readFileSync(userBunfig, 'utf-8')).toBe(before);
    } finally {
      dir.cleanup();
    }
  });

  it('mutation 启用时读取 reportDir/mutation.json，不读 reports/mutation/', () => {
    const dir = createTempDir();
    try {
      const openspec = path.join(dir.root, 'openspec');
      fs.mkdirSync(openspec, { recursive: true });
      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest', includes: ['**/*.ts'] }],
        }),
        'utf-8',
      );

      const reportsDir = path.join(dir.root, 'reports', 'test');
      const oldMutationDir = path.join(dir.root, 'reports', 'mutation');
      fs.mkdirSync(oldMutationDir, { recursive: true });
      fs.writeFileSync(
        path.join(oldMutationDir, 'mutation.json'),
        JSON.stringify({ files: {}, mutationScore: 10 }),
        'utf-8',
      );

      mockExecSync.mockImplementation((cmd: unknown) => {
        const planDir = path.join(reportsDir, 'vitest');
        if (String(cmd).includes('stryker')) {
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                    { id: '2', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writeMinimalJsResults(planDir);
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(dir.root, 'src', 'foo.test.ts'),
                assertionResults: [
                  { title: 't1', fullName: 't1', status: 'passed', failureMessages: [] },
                ],
              },
            ],
          }),
          'utf-8',
        );
        return '';
      });

      const result = executePlanEntry(
        makePlan({
          framework: 'vitest',
          mutation_framework: 'stryker-js',
          mutation_score: 50,
        }),
        dir.root,
        { reportsDir },
      );
      expect(result.mutation).not.toBeNull();
      expect(result.mutation!.score).toBeGreaterThan(50);
      expect(fs.existsSync(path.join(reportsDir, 'vitest', 'mutation.json'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('不做 tee：命令中不出现同时写 stdout 与文件的 tee 形态', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'jest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'jest' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).not.toMatch(/\|\s*tee\b/);
      expect(cmd).not.toMatch(/tee\s+/);
    } finally {
      dir.cleanup();
    }
  });

  it('脏 stdout 不影响文件通道解析成功', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '>>> loading plugins\nNOT JSON\n';
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.testCases).toHaveLength(1);
      expect(result.testCases[0].status).toBe('passed');
    } finally {
      dir.cleanup();
    }
  });

  it('coverage 仅从 planDir 读取；suite cwd 旧 coverage 被忽略', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const oldCovDir = path.join(dir.root, 'coverage');
      fs.mkdirSync(oldCovDir, { recursive: true });
      fs.writeFileSync(
        path.join(oldCovDir, 'coverage-summary.json'),
        JSON.stringify({
          total: { lines: { pct: 11 }, branches: { pct: 11 }, functions: { pct: 11 } },
        }),
        'utf-8',
      );

      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        // no planDir coverage sidecar
        return '';
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.coverage).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('段级重定向框架（bun）命令含 > "{results_file}"', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'bun');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(path.join(planDir, 'results.txt'), '1 [PASS] ok\n', 'utf-8');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).toMatch(/>\s*"/);
      expect(cmd).toContain('results.txt');
      expect(cmd).not.toMatch(/\|\s*tee\b/);
    } finally {
      dir.cleanup();
    }
  });
});
