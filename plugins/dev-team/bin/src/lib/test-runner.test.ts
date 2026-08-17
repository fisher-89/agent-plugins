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
  const mutationShell = cfg.shell.mutation_execution;
  return {
    cwd: '.',
    root: '.',
    framework: fw,
    coverage_format: cfg.coverage_format,
    coverage_output: cfg.coverage_output,
    mutation_cwd: '.',
    mutation_script: mutationShell
      ? {
          shell: mutationShell('99.0.0'),
          cmd: cfg.cmd.mutation_execution
            ? cfg.cmd.mutation_execution('99.0.0')
            : mutationShell('99.0.0'),
        }
      : null,
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
      const absRoot = path.resolve(dir.root).replace(/\\/g, '/');
      expect(cmd).toContain(`--config "${absRoot}/bunfig.dev-team-`);
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
          cwd: '.',
          root: '.',
          framework: 'unknown-fw' as TestPlan['framework'],
          coverage_format: 'istanbul',
          coverage_output: 'coverage-summary.json',
          mutation_cwd: '.',
          mutation_script: null,
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

  it('jest --coverageDirectory / --outputFile 使用带引号的绝对路径', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'jest');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(planDir);
        return '';
      });
      executePlanEntry(makePlan({ framework: 'jest' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      const absPlanDir = path.resolve(planDir).replace(/\\/g, '/');
      const absResults = `${absPlanDir}/results.json`;
      expect(cmd).toContain(`--coverageDirectory="${absPlanDir}"`);
      expect(cmd).toContain(`--outputFile="${absResults}"`);
      expect(cmd).not.toMatch(/--coverageDirectory="?reports\//);
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

// ===========================================================================
// mutation-score-below-60 补强：占位符 / 重定向 / timeout / mutation 开关
// ===========================================================================

describe('executePlanEntry -- 占位符展开与重定向', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('捕获 cmd：results_file/coverage_file/coverprofile_file 已替换为带引号的绝对 POSIX 路径', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'go');
      mockExecSync.mockImplementation(() => {
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'TestA' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go', cwd: '.', root: '.' }), dir.root, {
        reportsDir,
      });
      const cmd = capturedCmd();
      const absPlan = path.resolve(planDir).replace(/\\/g, '/');
      expect(cmd).not.toContain('{results_file}');
      expect(cmd).not.toContain('{coverage_file}');
      expect(cmd).not.toContain('{coverprofile_file}');
      expect(cmd).toContain(`-coverprofile="${absPlan}/coverage.out"`);
      expect(cmd).toContain(`-func="${absPlan}/coverage.out"`);
      expect(cmd).toContain(`> "${absPlan}/func-summary.txt"`);
      expect(cmd).toContain(`> "${absPlan}/results.ndjson"`);
    } finally {
      dir.cleanup();
    }
  });

  it("files=['a.test.ts','b.test.ts'] → {files} 展开；省略且 scope=src → 回落 src；scope='.' → 空串", () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest', root: 'src' }), dir.root, {
        reportsDir,
        files: ['a.test.ts', 'b.test.ts'],
      });
      expect(capturedCmd()).toContain('a.test.ts b.test.ts');

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest', root: 'src' }), dir.root, { reportsDir });
      expect(capturedCmd()).toMatch(/\bsrc\b/);
      expect(capturedCmd()).not.toContain('{files}');

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      const plan = makePlan({ framework: 'vitest', root: '.' });
      // 用可观测模板锁定空 files 回落：末尾 {files} 被替换为空
      plan.script.shell = 'npx vitest run --outputFile={results_file} {files}';
      plan.script.cmd = plan.script.shell;
      executePlanEntry(plan, dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).not.toContain('{files}');
      expect(cmd.trimEnd().endsWith('results.json') || /results\.json\s*$/.test(cmd)).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it("go：scope='.' → {directory}=./...；scope=pkg → ./pkg/...", () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go', root: '.' }), dir.root, { reportsDir });
      expect(capturedCmd()).toContain('./...');

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go', root: 'pkg' }), dir.root, { reportsDir });
      expect(capturedCmd()).toContain('./pkg/...');
    } finally {
      dir.cleanup();
    }
  });

  it('suite 声明 config 且有 config_flag → cmd 含 --config；无 config → {config_args} 剥离无残留空位', () => {
    const dir = createTempDir();
    try {
      const openspec = path.join(dir.root, 'openspec');
      fs.mkdirSync(openspec, { recursive: true });
      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: '.',
              framework: 'vitest',
              config: 'vitest.config.ts',
              includes: ['**/*.ts'],
            },
          ],
        }),
        'utf-8',
      );
      fs.writeFileSync(path.join(dir.root, 'vitest.config.ts'), 'export default {}', 'utf-8');

      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      const absConfig = path.resolve(dir.root, 'vitest.config.ts').replace(/\\/g, '/');
      expect(capturedCmd()).toContain(`--config "${absConfig}"`);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      // 无 suite config：剥离 {config_args}
      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest', includes: ['**/*.ts'] }],
        }),
        'utf-8',
      );
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).not.toContain('{config_args}');
      expect(cmd).not.toMatch(/--config\s+--/);
      expect(cmd).not.toMatch(/\s{2,}/);
    } finally {
      dir.cleanup();
    }
  });

  it('go：重定向插在 go test 与链分隔符之间，coverage 段不被吞掉', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).toMatch(/go test[\s\S]*?>\s*"[^"]*results\.ndjson"/);
      expect(cmd).toContain('go tool cover');
      // coverage 段仍在重定向之后
      const redirectIdx = cmd.search(/>\s*"[^"]*results\.ndjson"/);
      const coverIdx = cmd.indexOf('go tool cover');
      expect(redirectIdx).toBeGreaterThanOrEqual(0);
      expect(coverIdx).toBeGreaterThan(redirectIdx);
    } finally {
      dir.cleanup();
    }
  });

  it('rust：cargo test > 后仍保留 llvm-cov；pytest：仅第一段后插入 >', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'rust'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'rust' }), dir.root, { reportsDir });
      const rustCmd = capturedCmd();
      expect(rustCmd).toMatch(/cargo test\s+>\s*"/);
      expect(rustCmd).toContain('llvm-cov');

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'pytest'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'pytest' }), dir.root, { reportsDir });
      const pyCmd = capturedCmd();
      expect(pyCmd).toMatch(/pytest[\s\S]*?>\s*"[^"]*results\.txt"/);
      expect(pyCmd).toContain('--cov-report="json:');
      const redir = pyCmd.search(/>\s*"[^"]*results\.txt"/);
      const cov = pyCmd.indexOf('--cov');
      expect(cov).toBeGreaterThan(redir);
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- 空命令 / 解析失败 / timeout / planId', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('script.shell/cmd 为空或仅空白 → exitCode=-1、error 含 Empty test command、不调用 execSync', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const result = executePlanEntry(
        makePlan({
          framework: 'vitest',
          script: { shell: '   ', cmd: '' },
        }),
        dir.root,
        { reportsDir },
      );
      expect(result.exitCode).toBe(-1);
      expect(result.error).toMatch(/Empty test command/);
      expect(mockExecSync).not.toHaveBeenCalled();
    } finally {
      dir.cleanup();
    }
  });

  it('exitCode≠0 且 planDir 无结果文件 → error 含 Missing or unparseable results file', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        // 空 message → execError 为假值，透出 parseError
        const err = new Error('') as Error & { status: number };
        err.status = 2;
        throw err;
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(2);
      expect(result.error).toMatch(/Missing or (?:empty|unparseable) results file/);
      expect(result.mutation).toBeNull();
      expect(result.testCases).toEqual([]);
    } finally {
      dir.cleanup();
    }
  });

  it('exitCode=0 但空结果 → parser 错误透传', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        // 命令成功但写入空 results.json → parsePlanArtifacts 返回 parser error
        const planDir = path.join(reportsDir, 'vitest');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(path.join(planDir, 'results.json'), '   \n', 'utf-8');
        return '';
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(0);
      expect(result.error).toMatch(/Missing or empty results file/);
      expect(result.testCases).toEqual([]);
      expect(result.mutation).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('传入 timeout:1234 → execSync options.timeout 为 1234；省略 → 60000；0/-1/MAX 原样传递', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const cases: Array<{ timeout?: number; expected: number }> = [
        { timeout: 1234, expected: 1234 },
        { expected: 60000 },
        { timeout: 0, expected: 0 },
        { timeout: -1, expected: -1 },
        { timeout: Number.MAX_SAFE_INTEGER, expected: Number.MAX_SAFE_INTEGER },
      ];
      for (const c of cases) {
        mockExecSync.mockReset();
        mockExecSync.mockImplementation(() => {
          writeMinimalJsResults(path.join(reportsDir, 'vitest'));
          return '';
        });
        const opts: { reportsDir: string; timeout?: number } = { reportsDir };
        if (c.timeout !== undefined) opts.timeout = c.timeout;
        executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, opts);
        expect(mockExecSync.mock.calls[0][1]).toMatchObject({ timeout: c.expected });
      }
    } finally {
      dir.cleanup();
    }
  });

  it("root='.' → planId=framework；含斜杠/反斜杠 root 归一为下划线；cwd≠root 时仍用 root", () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        return '';
      });
      expect(
        executePlanEntry(makePlan({ framework: 'vitest', cwd: '.', root: '.' }), dir.root, {
          reportsDir,
        }).planId,
      ).toBe('vitest');

      const nestedId = 'plugins_dev-team_bin_src_vitest';
      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, nestedId));
        return '';
      });
      const nestedCwd = path.join(dir.root, 'plugins', 'dev-team', 'bin');
      fs.mkdirSync(path.join(nestedCwd, 'src'), { recursive: true });
      expect(
        executePlanEntry(
          makePlan({
            framework: 'vitest',
            cwd: 'plugins/dev-team/bin',
            root: 'plugins/dev-team/bin/src',
          }),
          dir.root,
          { reportsDir },
        ).planId,
      ).toBe(nestedId);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, nestedId));
        return '';
      });
      expect(
        executePlanEntry(
          makePlan({
            framework: 'vitest',
            cwd: 'plugins\\dev-team\\bin',
            root: 'plugins\\dev-team\\bin\\src',
          }),
          dir.root,
          { reportsDir },
        ).planId,
      ).toBe(nestedId);
    } finally {
      dir.cleanup();
    }
  });

  it('exec 抛错且 stdout/stderr 为 Buffer → 仍能解析 planDir 文件并设置 exitCode', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        const err = new Error('boom') as Error & {
          status: number;
          stdout: Buffer;
          stderr: Buffer;
        };
        err.status = 1;
        err.stdout = Buffer.from('noise');
        err.stderr = Buffer.from('err');
        throw err;
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(1);
      expect(result.testCases.length).toBeGreaterThan(0);
      expect(result.testCases.some((t) => t.status === 'failed')).toBe(true);
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- prefix 与 exec cwd', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function writePassingJsWithSource(
    planDir: string,
    projectRoot: string,
    opts: { relativeTestName?: boolean } = {},
  ): void {
    fs.mkdirSync(planDir, { recursive: true });
    fs.mkdirSync(path.join(projectRoot, 'src'), { recursive: true });
    fs.writeFileSync(path.join(projectRoot, 'src', 'foo.ts'), 'export const x=1\n', 'utf-8');
    const testName = opts.relativeTestName
      ? 'src/foo.test.ts'
      : path.join(projectRoot, 'src', 'foo.test.ts');
    fs.writeFileSync(
      path.join(planDir, 'results.json'),
      JSON.stringify({
        testResults: [
          {
            name: testName,
            assertionResults: [
              { title: 't1', fullName: 't1', status: 'passed', failureMessages: [] },
            ],
          },
        ],
      }),
      'utf-8',
    );
  }

  function setupOpenspec(root: string): void {
    const openspec = path.join(root, 'openspec');
    fs.mkdirSync(openspec, { recursive: true });
    fs.writeFileSync(
      path.join(openspec, 'config.json'),
      JSON.stringify({
        schema: 'spec-driven',
        tests: [{ root: '.', framework: 'vitest', includes: ['**/*.ts'] }],
      }),
      'utf-8',
    );
  }

  it('plan cwd=pkg/jest、mutation_cwd=pkg → 命令含绝对 --prefix 且 exec cwd 为 pkg（AC-6）', () => {
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      fs.mkdirSync(path.join(dir.root, 'pkg', 'jest'), { recursive: true });
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'pkg_vitest');
      const expectedPrefix = path.resolve(dir.root, 'pkg/jest');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(String(cmd)).not.toMatch(/(^|\s)-p(\s|$)/);
          expect(opts?.cwd).toBe('pkg');
          fs.mkdirSync(planDir, { recursive: true });
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(planDir, path.join(dir.root, 'pkg'));
        return '';
      });

      executePlanEntry(
        makePlan({
          framework: 'vitest',
          root: 'pkg',
          cwd: 'pkg/jest',
          mutation_cwd: 'pkg',
        }),
        dir.root,
        { reportsDir },
      );
    } finally {
      dir.cleanup();
    }
  });

  it('stryker 命令抛 status=1 → mutation 为 null 且命令仍含绝对 --prefix（AC-6）', () => {
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const expectedPrefix = path.resolve(dir.root, '.');
      let capturedCmd = '';

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          capturedCmd = String(cmd);
          expect(capturedCmd).toContain(`--prefix "${expectedPrefix}"`);
          expect(capturedCmd).not.toMatch(/(^|\s)-p(\s|$)/);
          const err = new Error('stryker fail') as Error & { status: number };
          err.status = 1;
          throw err;
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.mutation).toBeNull();
      expect(result.error).toMatch(/Mutation testing failed/);
      expect(result.exitCode).toBe(0);
      expect(capturedCmd).toContain('--prefix');
    } finally {
      dir.cleanup();
    }
  });

  it('mutation_script 为 null（bun）→ 不调用 stryker', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'bun'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('entry.cwd 为空串 → prefix 为 path.resolve(projectRoot, "")', () => {
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const expectedPrefix = path.resolve(dir.root, '');

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', cwd: '' }), dir.root, { reportsDir });
    } finally {
      dir.cleanup();
    }
  });

  it('entry.cwd 超长 → prefix 仍为绝对路径', () => {
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      const deep = `p/${'nested/'.repeat(200)}leaf`;
      fs.mkdirSync(path.join(dir.root, deep), { recursive: true });
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const expectedPrefix = path.resolve(dir.root, deep);

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', cwd: deep }), dir.root, { reportsDir });
    } finally {
      dir.cleanup();
    }
  });

  it('entry.cwd 含空格 / emoji → --prefix 带双引号包裹绝对路径', () => {
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      const spaced = 'pkg with space 🧪';
      fs.mkdirSync(path.join(dir.root, spaced), { recursive: true });
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const expectedPrefix = path.resolve(dir.root, spaced);

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', cwd: spaced }), dir.root, { reportsDir });
    } finally {
      dir.cleanup();
    }
  });

  it('win32 下 prefix 保留 path.resolve 原文，不得 toForwardSlash', () => {
    if (process.platform !== 'win32') return;
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const expectedPrefix = path.resolve(dir.root, 'pkg');

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(expectedPrefix).toMatch(/\\/);
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', cwd: 'pkg' }), dir.root, { reportsDir });
    } finally {
      dir.cleanup();
    }
  });

  it('noMutation:true 跳过 stryker；省略或 false 时运行 stryker', () => {
    const dir = createTempDir();
    try {
      setupOpenspec(dir.root);
      const reportsDir = path.join(dir.root, 'reports', 'test');

      mockExecSync.mockImplementation(() => {
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
        noMutation: true,
      });
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(mockExecSync.mock.calls.some((c) => String(c[0]).includes('stryker'))).toBe(true);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
        noMutation: false,
      });
      expect(mockExecSync.mock.calls.some((c) => String(c[0]).includes('stryker'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- 相对 mutation_cwd 时 prefix 仍绝对', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function writePassingJsWithSource(planDir: string, projectRoot: string): void {
    fs.mkdirSync(planDir, { recursive: true });
    fs.mkdirSync(path.join(projectRoot, 'src'), { recursive: true });
    fs.writeFileSync(path.join(projectRoot, 'src', 'foo.ts'), 'export const x=1\n', 'utf-8');
    fs.writeFileSync(
      path.join(planDir, 'results.json'),
      JSON.stringify({
        testResults: [
          {
            name: path.join(projectRoot, 'src', 'foo.test.ts'),
            assertionResults: [
              { title: 't1', fullName: 't1', status: 'passed', failureMessages: [] },
            ],
          },
        ],
      }),
      'utf-8',
    );
  }

  it('mutation_cwd=.、cwd=. → exec cwd 为 .；--prefix 为绝对路径（AC-7）', () => {
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
      const expectedPrefix = path.resolve(dir.root, '.');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(opts?.cwd).toBe('.');
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(path.isAbsolute(expectedPrefix)).toBe(true);
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', mutation_cwd: '.', cwd: '.' }), dir.root, {
        reportsDir,
      });
    } finally {
      dir.cleanup();
    }
  });

  it('mutation_cwd=.、cwd=pkg → exec cwd 仍为 .；prefix 为 resolve(projectRoot, pkg)', () => {
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
      fs.mkdirSync(path.join(dir.root, 'pkg'), { recursive: true });
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const expectedPrefix = path.resolve(dir.root, 'pkg');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(opts?.cwd).toBe('.');
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', mutation_cwd: '.', cwd: 'pkg' }), dir.root, {
        reportsDir,
      });
    } finally {
      dir.cleanup();
    }
  });

  it('mutation_cwd 为空串 → 原样传给 execSync cwd', () => {
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

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(opts?.cwd).toBe('');
          fs.mkdirSync(path.join(reportsDir, 'vitest'), { recursive: true });
          fs.writeFileSync(
            path.join(reportsDir, 'vitest', 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });

      executePlanEntry(makePlan({ framework: 'vitest', mutation_cwd: '' }), dir.root, {
        reportsDir,
      });
    } finally {
      dir.cleanup();
    }
  });

  it('projectRoot 为 undefined / null → 抛错或失败，不得把相对 mutation_cwd 当作 prefix', () => {
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
      const plan = makePlan({ framework: 'vitest', mutation_cwd: '.', cwd: '.' });

      expect(() =>
        executePlanEntry(plan, undefined as unknown as string, { reportsDir }),
      ).toThrow();
      expect(
        mockExecSync.mock.calls
          .filter((c) => String(c[0]).includes('stryker'))
          .every((c) => !String(c[0]).match(/--prefix "\."/)),
      ).toBe(true);

      expect(() => executePlanEntry(plan, null as unknown as string, { reportsDir })).toThrow();
      expect(
        mockExecSync.mock.calls
          .filter((c) => String(c[0]).includes('stryker'))
          .every((c) => !String(c[0]).match(/--prefix "\."/)),
      ).toBe(true);
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- mutation 开关', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function writePassingJsWithSource(
    planDir: string,
    projectRoot: string,
    opts: { relativeTestName?: boolean } = {},
  ): void {
    fs.mkdirSync(planDir, { recursive: true });
    fs.mkdirSync(path.join(projectRoot, 'src'), { recursive: true });
    fs.writeFileSync(path.join(projectRoot, 'src', 'foo.ts'), 'export const x=1\n', 'utf-8');
    const testName = opts.relativeTestName
      ? 'src/foo.test.ts'
      : path.join(projectRoot, 'src', 'foo.test.ts');
    fs.writeFileSync(
      path.join(planDir, 'results.json'),
      JSON.stringify({
        testResults: [
          {
            name: testName,
            assertionResults: [
              { title: 't1', fullName: 't1', status: 'passed', failureMessages: [] },
            ],
          },
        ],
      }),
      'utf-8',
    );
  }

  it('支持 mutation_execution 的框架全通过、mock stryker 写 mutation.json → score/threshold/pass；临时 config 删除', () => {
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
      const mutationCwd = dir.root;
      const seenConfigs: string[] = [];
      mockExecSync.mockImplementation((cmd: unknown) => {
        const planDir = path.join(reportsDir, 'vitest');
        if (String(cmd).includes('stryker')) {
          const m = String(cmd).match(/stryker\.config\.[a-f0-9]+\.json/);
          if (m) {
            const cfgPath = path.join(mutationCwd, m[0]);
            seenConfigs.push(cfgPath);
            expect(fs.existsSync(cfgPath)).toBe(true);
          }
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
        writePassingJsWithSource(planDir, dir.root);
        return '';
      });
      const result = executePlanEntry(
        makePlan({ framework: 'vitest', mutation_score: 50, mutation_cwd: mutationCwd }),
        dir.root,
        { reportsDir },
      );
      expect(result.mutation).not.toBeNull();
      expect(result.mutation!.score).toBe(100);
      expect(result.mutation!.threshold).toBe(50);
      expect(result.mutation!.pass).toBe(true);
      for (const c of seenConfigs) {
        expect(fs.existsSync(c)).toBe(false);
      }
    } finally {
      dir.cleanup();
    }
  });

  it('stryker 在 mutation_cwd 执行（可不同于 suite cwd）', () => {
    const dir = createTempDir();
    try {
      const pkgRoot = path.join(dir.root, 'pkg');
      const jestCwd = path.join(pkgRoot, 'jest');
      const mutationCwd = 'pkg';
      fs.mkdirSync(jestCwd, { recursive: true });
      const openspec = path.join(dir.root, 'openspec');
      fs.mkdirSync(openspec, { recursive: true });
      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: 'pkg',
              cwd: 'jest',
              framework: 'vitest',
              includes: ['**/*.ts'],
              mutation: { cwd: '.' },
            },
          ],
        }),
        'utf-8',
      );
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'pkg_vitest');
      const strykerCwds: string[] = [];
      const strykerCmds: string[] = [];
      const seenConfigs: string[] = [];
      const expectedPrefix = path.resolve(dir.root, 'pkg/jest');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          strykerCmds.push(String(cmd));
          strykerCwds.push(opts?.cwd ?? '');
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(String(cmd)).not.toMatch(/(^|\s)-p(\s|$)/);
          const m = String(cmd).match(/stryker\.config\.[a-f0-9]+\.json/);
          if (m && opts?.cwd) {
            const cfgPath = path.resolve(dir.root, opts.cwd, m[0]);
            seenConfigs.push(cfgPath);
            expect(fs.existsSync(cfgPath)).toBe(true);
          }
          fs.mkdirSync(planDir, { recursive: true });
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(planDir, pkgRoot);
        return '';
      });

      const cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(dir.root);
      try {
        const result = executePlanEntry(
          makePlan({
            framework: 'vitest',
            root: 'pkg',
            cwd: 'pkg/jest',
            mutation_cwd: mutationCwd,
            mutation_score: 50,
          }),
          dir.root,
          { reportsDir },
        );

        expect(result.mutation).not.toBeNull();
        expect(strykerCwds).toEqual([mutationCwd]);
        expect(strykerCwds[0]).not.toBe(path.resolve(jestCwd));
        expect(strykerCmds[0]).toContain('--prefix');
        for (const c of seenConfigs) {
          expect(path.resolve(path.dirname(c))).toBe(path.resolve(dir.root, mutationCwd));
          expect(fs.existsSync(c)).toBe(false);
        }
      } finally {
        cwdSpy.mockRestore();
      }
    } finally {
      dir.cleanup();
    }
  });

  it('mutation_cwd 为相对路径时按原样传给 stryker cwd', () => {
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
      const planDir = path.join(reportsDir, 'vitest');
      const strykerCwds: string[] = [];
      const strykerCmds: string[] = [];
      const mutationCwd = '.';
      const expectedPrefix = path.resolve(dir.root, '.');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          strykerCwds.push(opts?.cwd ?? '');
          strykerCmds.push(String(cmd));
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(path.isAbsolute(expectedPrefix)).toBe(true);
          fs.mkdirSync(planDir, { recursive: true });
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify({
              files: {
                'src/foo.ts': {
                  mutants: [
                    { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
                  ],
                },
              },
            }),
            'utf-8',
          );
          return '';
        }
        writePassingJsWithSource(planDir, dir.root);
        return '';
      });

      executePlanEntry(
        makePlan({
          framework: 'vitest',
          mutation_cwd: mutationCwd,
          mutation_score: 50,
        }),
        dir.root,
        { reportsDir },
      );

      expect(strykerCwds).toEqual([mutationCwd]);
      expect(strykerCmds[0]).not.toMatch(/(^|\s)-p(\s|$)/);
    } finally {
      dir.cleanup();
    }
  });

  it('noMutation:true / 无 mutation_execution 的框架 / 有 failed → 跳过 stryker、mutation===null', () => {
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

      mockExecSync.mockImplementation(() => {
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });
      const noMut = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
        noMutation: true,
      });
      expect(noMut.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writePassingJsWithSource(path.join(reportsDir, 'bun'), dir.root);
        return '';
      });
      const noMutExec = executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      expect(noMutExec.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        return '';
      });
      const failed = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(failed.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('mutationDiffFiles 无交集 / suite excludes 过滤全部 sourceFiles → 跳过 stryker', () => {
    const dir = createTempDir();
    try {
      const openspec = path.join(dir.root, 'openspec');
      fs.mkdirSync(openspec, { recursive: true });
      // suite.root=src 时 excludes 可稳定匹配相对 sourceFiles
      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            {
              root: 'src',
              framework: 'vitest',
              includes: ['**/*.ts'],
              excludes: ['**/*'],
            },
          ],
        }),
        'utf-8',
      );
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root, {
          relativeTestName: true,
        });
        return '';
      });
      const excluded = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
      });
      expect(excluded.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);

      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [{ root: '.', framework: 'vitest', includes: ['**/*.ts'] }],
        }),
        'utf-8',
      );
      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root, {
          relativeTestName: true,
        });
        return '';
      });
      const noIntersect = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
        mutationDiffFiles: ['other/unrelated.ts'],
      });
      expect(noIntersect.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('stryker exit≠0 或缺少 mutation.json → mutation===null 且 error 描述失败；不读旧路径', () => {
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
      const oldDir = path.join(dir.root, 'reports', 'mutation');
      fs.mkdirSync(oldDir, { recursive: true });
      fs.writeFileSync(
        path.join(oldDir, 'mutation.json'),
        JSON.stringify({
          files: {
            'src/foo.ts': {
              mutants: [
                { id: '1', status: 'Killed', mutatorName: 'x', replacement: 'y', location: {} },
              ],
            },
          },
        }),
        'utf-8',
      );
      const reportsDir = path.join(dir.root, 'reports', 'test');

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          const err = new Error('stryker fail') as Error & { status: number; stderr?: string };
          err.status = 1;
          err.stderr = 'Cannot find module @stryker-mutator/core';
          throw err;
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });
      const failedCmd = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
      });
      expect(failedCmd.mutation).toBeNull();
      expect(failedCmd.error).toMatch(/Mutation testing failed/);
      expect(failedCmd.exitCode).toBe(0);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          // 成功但不写 mutation.json
          return '';
        }
        writePassingJsWithSource(path.join(reportsDir, 'vitest'), dir.root);
        return '';
      });
      const missingReport = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
      });
      expect(missingReport.mutation).toBeNull();
      expect(missingReport.error).toMatch(/mutation report not found or invalid/);
    } finally {
      dir.cleanup();
    }
  });

  it('存在用户 bunfig.toml 时临时 overlay 前缀含原内容且含 coverageDir/lcov；用户文件不变；结束后 temp 删除', () => {
    const dir = createTempDir();
    try {
      const userBunfig = path.join(dir.root, 'bunfig.toml');
      const userContent = '[test]\npreload = ["keep-me"]\n';
      fs.writeFileSync(userBunfig, userContent, 'utf-8');
      const reportsDir = path.join(dir.root, 'reports', 'test');
      let overlay = '';
      let tempPath = '';
      mockExecSync.mockImplementation((cmd: unknown) => {
        const m = String(cmd).match(/bunfig\.dev-team-[a-f0-9]+\.toml/);
        if (m) {
          tempPath = path.join(dir.root, m[0]);
          overlay = fs.readFileSync(tempPath, 'utf-8');
        }
        writeMinimalTextResults(path.join(reportsDir, 'bun'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'bun' }), dir.root, { reportsDir });
      expect(overlay.startsWith(userContent)).toBe(true);
      expect(overlay).toContain('coverageDir');
      expect(overlay).toContain('coverageReporter = ["lcov"]');
      expect(fs.readFileSync(userBunfig, 'utf-8')).toBe(userContent);
      expect(tempPath).toBeTruthy();
      expect(fs.existsSync(tempPath)).toBe(false);
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- Unix redirect / shell / parseError 杀变异', () => {
  const prevShell = process.env.SHELL;

  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
    // Force shell-template path even on Windows so go/pytest `;` redirect regex is covered
    process.env.SHELL = '/bin/bash';
  });

  afterEach(() => {
    if (prevShell === undefined) delete process.env.SHELL;
    else process.env.SHELL = prevShell;
    vi.restoreAllMocks();
  });

  it('SHELL 存在时 go 使用 shell 模板：重定向在 `go test…;` 段内且精确匹配', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go' }), dir.root, { reportsDir });
      const cmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      // Unix chain uses `;` — must keep `go test … > "results" ; … cover`
      expect(cmd).toMatch(/^go test\b.*?>\s*"[^"]*results\.ndjson"\s*;/);
      expect(cmd).toContain('; _X=$?;');
      expect(cmd).toContain('go tool cover -func=');
      expect(cmd).not.toContain('errorlevel');
    } finally {
      dir.cleanup();
    }
  });

  it('SHELL 存在时 pytest 使用 shell 模板：重定向在第一段 `;` 之前', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'pytest'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'pytest' }), dir.root, { reportsDir });
      const cmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      // 精确：redirect 后必须是「空白*;空白*」才能匹配原正则；弱化空白/锚点的变异会插错位置
      expect(cmd).toMatch(/^pytest -v\s+> "[^"]*results\.txt"\s*;\s*pytest --cov=/);
      expect(cmd.indexOf('> "')).toBeLessThan(cmd.indexOf('; pytest --cov='));
      expect(cmd).not.toContain('&&');
      // 第二段 pytest 前保留 `;` 两侧空白（杀 \s*;\s → \s*;\S 等）
      expect(cmd).toMatch(/results\.txt" ; pytest --cov=|results\.txt"\s+;\s+pytest --cov=/);
    } finally {
      dir.cleanup();
    }
  });

  it('scope 非 `.` 时 {files} 回落为 scope；directory 为 ./scope/...', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        writeIstanbulCoverage(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest', root: 'pkg' }), dir.root, { reportsDir });
      const cmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(cmd).toMatch(/\spkg(\s|$)/);
      expect(cmd).not.toContain('{files}');
    } finally {
      dir.cleanup();
    }
  });

  it('go scope=pkg 时 {directory} 为 ./pkg/...（非 ./...）', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go', root: 'pkg' }), dir.root, { reportsDir });
      const cmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(cmd).toContain('./pkg/...');
      expect(cmd).not.toContain('./...;');
    } finally {
      dir.cleanup();
    }
  });

  it('exitCode≠0（无抛错）且空用例 → parseError 与 mutation 跳过', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      // execSync 不抛错但 status 无法表达非零；用抛错且 message 空/缺省，让 parseError 成为 error
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'vitest');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({ testResults: [{ name: 'a.test.ts', assertionResults: [] }] }),
          'utf-8',
        );
        const err = new Error('') as Error & { status: number; message: string };
        err.status = 2;
        err.message = '';
        throw err;
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(2);
      expect(result.testCases).toEqual([]);
      // execError 为空串时 || 回落到 parseError
      expect(result.error).toMatch(/Missing or unparseable results file/);
      expect(result.mutation).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('execSync 抛错且 stdout/stderr 为 Buffer → 仍设置 exitCode 与 error', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        const err = new Error('cmd failed') as Error & {
          status: number;
          stdout: Buffer;
          stderr: Buffer;
        };
        err.status = 1;
        err.stdout = Buffer.from('out');
        err.stderr = Buffer.from('err');
        throw err;
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(1);
      expect(result.error).toMatch(/cmd failed/);
      expect(result.testCases.some((t) => t.status === 'failed')).toBe(true);
      expect(result.mutation).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('exitCode≠0 且已有 failed 用例 → 不得误标 Missing results；error 为 exec 消息', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        const err = new Error('tests failed') as Error & { status: number };
        err.status = 1;
        throw err;
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.exitCode).toBe(1);
      expect(result.testCases.length).toBeGreaterThan(0);
      expect(result.error).toBe('tests failed');
      expect(result.error).not.toMatch(/Missing or unparseable/);
    } finally {
      dir.cleanup();
    }
  });

  it("scope='.' 时 go directory 为 ./... 而非 ././...；scope=pkg 前缀为 ./", () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go', root: '.' }), dir.root, { reportsDir });
      const rootCmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(rootCmd).toContain('./...');
      expect(rootCmd).not.toContain('././...');

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go', root: 'pkg' }), dir.root, { reportsDir });
      const pkgCmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(pkgCmd).toContain('./pkg/...');
      expect(pkgCmd).not.toMatch(/(?:^|[^/])pkg\/\.\.\./); // 必须带 ./ 前缀
    } finally {
      dir.cleanup();
    }
  });

  it('无 config 时剥离 {config_args} 不留双空格；有 config 时替换为 --config "<绝对路径>"', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        writeIstanbulCoverage(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      const noCfg = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(noCfg).not.toContain('{config_args}');
      expect(noCfg).not.toMatch(/\s{2,}/);

      const openspec = path.join(dir.root, 'openspec');
      fs.mkdirSync(openspec, { recursive: true });
      fs.writeFileSync(path.join(dir.root, 'vite.config.ts'), 'export default {}', 'utf-8');
      fs.writeFileSync(
        path.join(openspec, 'config.json'),
        JSON.stringify({
          schema: 'spec-driven',
          tests: [
            { root: '.', framework: 'vitest', config: 'vite.config.ts', includes: ['**/*.ts'] },
          ],
        }),
        'utf-8',
      );
      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        writeIstanbulCoverage(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest', cwd: '.', root: '.' }), dir.root, {
        reportsDir,
      });
      const withCfg = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      const absConfig = path.resolve(dir.root, 'vite.config.ts').replace(/\\/g, '/');
      expect(withCfg).toContain(`--config "${absConfig}"`);
      expect(withCfg).not.toContain('{config_args}');
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- clearDirectory / ExecError / placeholders（突变补强）', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('planDir 原本不存在时仍 mkdirSync 成功且可写结果', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'vitest');
      expect(fs.existsSync(planDir)).toBe(false);
      mockExecSync.mockImplementation(() => {
        expect(fs.existsSync(planDir)).toBe(true);
        writeMinimalJsResults(planDir);
        return '';
      });
      const result = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(result.reportDir).toBe(planDir);
      expect(fs.existsSync(path.join(planDir, 'results.json'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('既有 planDir 含旧文件时执行前被清空；兄弟 plan 目录不受影响', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      const sibling = path.join(reportsDir, 'jest');
      const target = path.join(reportsDir, 'vitest');
      fs.mkdirSync(path.join(target, 'nested'), { recursive: true });
      fs.writeFileSync(path.join(target, 'nested', 'old.txt'), 'old', 'utf-8');
      fs.mkdirSync(sibling, { recursive: true });
      fs.writeFileSync(path.join(sibling, 'keep.txt'), 'keep', 'utf-8');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(target);
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      expect(fs.existsSync(path.join(target, 'nested', 'old.txt'))).toBe(false);
      expect(fs.existsSync(path.join(sibling, 'keep.txt'))).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('捕获 cmd：results/coverage/coverprofile/mutation/report_dir 均为非空绝对 POSIX 且文件名精确', () => {
    const prevShell = process.env.SHELL;
    process.env.SHELL = '/bin/bash'; // 强制 shell 模板 + `;` 重定向
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      const plan = makePlan({ framework: 'go' });
      // 追加占位符以观测 mutation_file / report_dir 展开（默认 go 模板不含二者）
      plan.script.shell = `${plan.script.shell} # {mutation_file} {report_dir}`;
      plan.script.cmd = plan.script.shell;
      executePlanEntry(plan, dir.root, { reportsDir });
      const cmd = capturedCmd();
      const absPlan = path.resolve(path.join(reportsDir, 'go')).replace(/\\/g, '/');
      expect(absPlan.length).toBeGreaterThan(1);
      expect(cmd).toContain(`${absPlan}/coverage.out`);
      expect(cmd).toContain(`${absPlan}/mutation.json`);
      expect(cmd).toContain(absPlan);
      expect(cmd).toMatch(/>\s*"[^"]*results\.ndjson"/);
      expect(cmd).not.toContain('{results_file}');
      expect(cmd).not.toContain('{coverprofile_file}');
      expect(cmd).not.toContain('{mutation_file}');
      expect(cmd).not.toContain('{report_dir}');
      expect(cmd).toMatch(/mutation\.json/);
      expect(cmd).toMatch(/coverage\.out/);
      expect(cmd).toMatch(/results\.ndjson/);
    } finally {
      if (prevShell === undefined) delete process.env.SHELL;
      else process.env.SHELL = prevShell;
      dir.cleanup();
    }
  });

  it('execSync 抛 null / 原始字符串 / {stdout:Buffer,stderr:Buffer,status:1} 时均不崩溃', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');

      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        throw null;
      });
      const nullResult = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
      });
      expect(nullResult.exitCode).toBe(-1);
      expect(nullResult.error).toMatch(/Command execution failed|Missing/);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        throw 'raw-string-error';
      });
      const strResult = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
      });
      expect(strResult.exitCode).toBe(-1);

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'), false);
        throw {
          stdout: Buffer.from('out-buf'),
          stderr: Buffer.from('err-buf'),
          status: 1,
          message: 'obj-exec',
        };
      });
      const objResult = executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, {
        reportsDir,
      });
      expect(objResult.exitCode).toBe(1);
      expect(objResult.error).toMatch(/obj-exec/);
    } finally {
      dir.cleanup();
    }
  });

  it('rust：cargo test > "…" 后仍保留 llvm-cov 段', () => {
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'rust'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'rust' }), dir.root, { reportsDir });
      const cmd = capturedCmd();
      expect(cmd).toMatch(/cargo test\b[\s\S]*?>\s*"[^"]*results\.txt"/);
      expect(cmd).toContain('llvm-cov');
      const redir = cmd.search(/>\s*"[^"]*results\.txt"/);
      const cov = cmd.indexOf('llvm-cov');
      expect(cov).toBeGreaterThan(redir);
    } finally {
      dir.cleanup();
    }
  });
});

describe('executePlanEntry -- win32 cmd redirect / resolveShell', () => {
  const prevShell = process.env.SHELL;
  const prevComspec = process.env.COMSPEC;

  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
    delete process.env.SHELL; // force isWinCmd on win32
  });

  afterEach(() => {
    if (prevShell === undefined) delete process.env.SHELL;
    else process.env.SHELL = prevShell;
    if (prevComspec === undefined) delete process.env.COMSPEC;
    else process.env.COMSPEC = prevComspec;
    vi.restoreAllMocks();
  });

  it('无 SHELL 时 go/pytest 走 cmd 链（& / &&）且重定向位置正确', () => {
    if (process.platform !== 'win32') return;
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'go');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.ndjson'),
          `${JSON.stringify({ Action: 'pass', Test: 'T' })}\n`,
          'utf-8',
        );
        return '';
      });
      executePlanEntry(makePlan({ framework: 'go' }), dir.root, { reportsDir });
      const goCmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(goCmd).toMatch(/^go test\b.*?>\s*"[^"]*results\.ndjson"\s*&/);
      expect(goCmd).toContain('errorlevel');
      expect(goCmd).not.toContain('; _X=$?');

      mockExecSync.mockReset();
      mockExecSync.mockImplementation(() => {
        writeMinimalTextResults(path.join(reportsDir, 'pytest'), 'results.txt');
        return '';
      });
      executePlanEntry(makePlan({ framework: 'pytest' }), dir.root, { reportsDir });
      const pyCmd = String(mockExecSync.mock.calls[0]?.[0] ?? '');
      expect(pyCmd).toMatch(/^pytest\b.*?>\s*"[^"]*results\.txt"\s+&&\s+/);
      expect(pyCmd).toContain('&& pytest --cov=');
      expect(pyCmd).not.toMatch(/; pytest/);
    } finally {
      dir.cleanup();
    }
  });

  it('win32 且无 SHELL/COMSPEC 时 execSync shell 回落 cmd.exe', () => {
    if (process.platform !== 'win32') return;
    delete process.env.COMSPEC;
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        writeIstanbulCoverage(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      const opts = mockExecSync.mock.calls[0]?.[1] as { shell?: string };
      expect(opts.shell).toBe('cmd.exe');
    } finally {
      dir.cleanup();
    }
  });

  it('win32 优先 SHELL，其次 COMSPEC', () => {
    if (process.platform !== 'win32') return;
    process.env.COMSPEC = 'C:\\\\Windows\\\\System32\\\\cmd.exe';
    process.env.SHELL = 'C:\\\\Git\\\\bin\\\\bash.exe';
    const dir = createTempDir();
    try {
      const reportsDir = path.join(dir.root, 'reports', 'test');
      mockExecSync.mockImplementation(() => {
        writeMinimalJsResults(path.join(reportsDir, 'vitest'));
        writeIstanbulCoverage(path.join(reportsDir, 'vitest'));
        return '';
      });
      executePlanEntry(makePlan({ framework: 'vitest' }), dir.root, { reportsDir });
      const opts = mockExecSync.mock.calls[0]?.[1] as { shell?: string };
      expect(opts.shell).toBe('C:\\\\Git\\\\bin\\\\bash.exe');
    } finally {
      dir.cleanup();
    }
  });
});
