/**
 * 集成测试: Stryker reportDir → mutation.json → 解析
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const mockExecSync = vi.fn();
vi.mock('child_process', () => ({
  execSync: (...args: unknown[]) => mockExecSync(...args),
}));

import { getFrameworkConfig } from '../../src/lib/test-framework';
import { resolveStrykerConfig } from '../../src/lib/test-parser/stryker-config';
import { executePlanEntry } from '../../src/lib/test-runner';

function createTempProject(): { root: string; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-flow-'));
  fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'openspec', 'config.json'),
    JSON.stringify({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest', includes: ['**/*.ts'], mutation: { score: 50 } }],
    }),
    'utf-8',
  );
  fs.mkdirSync(path.join(root, 'src'), { recursive: true });
  fs.writeFileSync(path.join(root, 'src', 'foo.ts'), 'export const x = 1;\n', 'utf-8');
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

const MUTATION_REPORT = {
  metrics: {
    mutationScore: 100,
    killed: 2,
    survived: 0,
    timeout: 0,
    noCoverage: 0,
    compileErrors: 0,
    runtimeErrors: 0,
    ignored: 0,
    totalDetected: 2,
    totalUndetected: 0,
    totalMutants: 2,
  },
};

describe('mutation 报告落在 planDir (AC-9)', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it('临时 stryker 配置 jsonReporter.fileName 指向 reportDir/mutation.json', () => {
    const project = createTempProject();
    try {
      const reportDir = path.join(project.root, 'reports', 'test', 'vitest');
      fs.mkdirSync(reportDir, { recursive: true });
      const { configPath } = resolveStrykerConfig(
        project.root,
        '.',
        ['src/foo.ts'],
        'vitest',
        reportDir,
      );
      const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));
      expect(config.jsonReporter.fileName).toBe(
        path.resolve(reportDir, 'mutation.json').replace(/\\/g, '/'),
      );
      expect(config.jsonReporter.fileName).not.toContain('reports/mutation/');
      fs.unlinkSync(configPath);
    } finally {
      project.cleanup();
    }
  });

  it('runner 解析 planDir/mutation.json 得到 mutation 块', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const cfg = getFrameworkConfig('vitest');
      mockExecSync.mockImplementation((cmd: unknown) => {
        const planDir = path.join(reportsDir, 'vitest');
        if (String(cmd).includes('stryker')) {
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify(MUTATION_REPORT),
            'utf-8',
          );
          return '';
        }
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'src', 'foo.test.ts'),
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
        {
          cwd: '.',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_script: cfg.shell.mutation_execution
            ? {
                shell: cfg.shell.mutation_execution('99.0.0'),
                cmd: cfg.cmd.mutation_execution
                  ? cfg.cmd.mutation_execution('99.0.0')
                  : cfg.shell.mutation_execution('99.0.0'),
              }
            : null,
          mutation_cwd: '.',
          mutation_score: 50,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.mutation).not.toBeNull();
      expect(result.mutation!.score).toBe(100);
      expect(fs.existsSync(path.join(reportsDir, 'vitest', 'mutation.json'))).toBe(true);
      expect(fs.existsSync(path.join(project.root, 'reports', 'mutation', 'mutation.json'))).toBe(
        false,
      );
    } finally {
      project.cleanup();
    }
  });

  it('mutation.json 缺失 → mutation=null，不读旧路径', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const oldDir = path.join(project.root, 'reports', 'mutation');
      fs.mkdirSync(oldDir, { recursive: true });
      fs.writeFileSync(
        path.join(oldDir, 'mutation.json'),
        JSON.stringify(MUTATION_REPORT),
        'utf-8',
      );
      const cfg = getFrameworkConfig('vitest');
      mockExecSync.mockImplementation((cmd: unknown) => {
        const planDir = path.join(reportsDir, 'vitest');
        if (String(cmd).includes('stryker')) {
          // 故意不写 planDir/mutation.json
          return '';
        }
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'src', 'foo.test.ts'),
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
        {
          cwd: '.',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_cwd: '.',
          mutation_script: cfg.shell.mutation_execution
            ? {
                shell: cfg.shell.mutation_execution('99.0.0'),
                cmd: cfg.cmd.mutation_execution
                  ? cfg.cmd.mutation_execution('99.0.0')
                  : cfg.shell.mutation_execution('99.0.0'),
              }
            : null,
          mutation_score: 50,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.mutation).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('临时配置与 .stryker-tmp 用后清理', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const mutationCwd = project.root;
      const cfg = getFrameworkConfig('vitest');
      let configPathSeen = '';
      mockExecSync.mockImplementation((cmd: unknown) => {
        const planDir = path.join(reportsDir, 'vitest');
        if (String(cmd).includes('stryker')) {
          const configs = fs
            .readdirSync(mutationCwd)
            .filter((n) => n.startsWith('stryker.config.') && n.endsWith('.json'));
          configPathSeen = configs[0] ? path.join(mutationCwd, configs[0]) : '';
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify(MUTATION_REPORT),
            'utf-8',
          );
          return '';
        }
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'src', 'foo.test.ts'),
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
      executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_cwd: mutationCwd,
          mutation_script: cfg.shell.mutation_execution
            ? {
                shell: cfg.shell.mutation_execution('99.0.0'),
                cmd: cfg.cmd.mutation_execution
                  ? cfg.cmd.mutation_execution('99.0.0')
                  : cfg.shell.mutation_execution('99.0.0'),
              }
            : null,
          mutation_score: 50,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(configPathSeen).not.toBe('');
      expect(fs.existsSync(configPathSeen)).toBe(false);
      expect(fs.existsSync(path.join(mutationCwd, '.stryker-tmp'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('stryker 非 0 退出 → mutation null、error 有值，且临时 config 仍被清理', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const mutationCwd = project.root;
      const cfg = getFrameworkConfig('vitest');
      let configPathSeen = '';
      mockExecSync.mockImplementation((cmd: unknown) => {
        const planDir = path.join(reportsDir, 'vitest');
        if (String(cmd).includes('stryker')) {
          const configs = fs
            .readdirSync(mutationCwd)
            .filter((n) => n.startsWith('stryker.config.') && n.endsWith('.json'));
          configPathSeen = configs[0] ? path.join(mutationCwd, configs[0]) : '';
          expect(configPathSeen).not.toBe('');
          expect(fs.existsSync(configPathSeen)).toBe(true);
          const err = new Error('stryker failed') as Error & { status: number; stderr?: string };
          err.status = 1;
          err.stderr = 'stryker: command not found';
          throw err;
        }
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'src', 'foo.test.ts'),
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
        {
          cwd: '.',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_cwd: mutationCwd,
          mutation_script: cfg.shell.mutation_execution
            ? {
                shell: cfg.shell.mutation_execution('99.0.0'),
                cmd: cfg.cmd.mutation_execution
                  ? cfg.cmd.mutation_execution('99.0.0')
                  : cfg.shell.mutation_execution('99.0.0'),
              }
            : null,
          mutation_score: 50,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.mutation).toBeNull();
      expect(result.error).toMatch(/Mutation testing failed/);
      expect(configPathSeen).not.toBe('');
      expect(fs.existsSync(configPathSeen)).toBe(false);
      expect(fs.existsSync(path.join(mutationCwd, '.stryker-tmp'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('noMutation:true 或 sourceFiles 被 exclude 清空 → 不生成临时 stryker config', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const cfg = getFrameworkConfig('vitest');
      const plan = {
        cwd: '.',
        root: '.',
        framework: 'vitest' as const,
        coverage_format: cfg.coverage_format,
        coverage_output: cfg.coverage_output,
        mutation_cwd: '.',
        mutation_script: cfg.shell.mutation_execution
          ? {
              shell: cfg.shell.mutation_execution('99.0.0'),
              cmd: cfg.cmd.mutation_execution
                ? cfg.cmd.mutation_execution('99.0.0')
                : cfg.shell.mutation_execution('99.0.0'),
            }
          : null,
        mutation_score: 50,
        script: {
          shell: cfg.shell.test_execution('99.0.0'),
          cmd: cfg.cmd.test_execution('99.0.0'),
        },
      };

      const listTempConfigs = () =>
        fs
          .readdirSync(project.root)
          .filter((n) => n.startsWith('stryker.config.') && n.endsWith('.json'));

      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'vitest');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'src', 'foo.test.ts'),
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

      const noMut = executePlanEntry(plan, project.root, { reportsDir, noMutation: true });
      expect(noMut.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);
      expect(listTempConfigs()).toHaveLength(0);

      // exclude 清空全部 sourceFiles → 跳过 resolveStrykerConfig
      fs.writeFileSync(
        path.join(project.root, 'openspec', 'config.json'),
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
      mockExecSync.mockClear();
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'vitest');
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: 'src/foo.test.ts',
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
      const excluded = executePlanEntry(plan, project.root, { reportsDir });
      expect(excluded.mutation).toBeNull();
      expect(mockExecSync.mock.calls.every((c) => !String(c[0]).includes('stryker'))).toBe(true);
      expect(listTempConfigs()).toHaveLength(0);
    } finally {
      project.cleanup();
    }
  });
});
