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

describe('mutation_execution 模板 → execSync --prefix 与 mutation_cwd（AC-6 / AC-7）', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  function createPkgProject(): { root: string; cleanup: () => void } {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-prefix-'));
    fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
    fs.mkdirSync(path.join(root, 'pkg'), { recursive: true });
    fs.mkdirSync(path.join(root, 'pkg', 'jest'), { recursive: true });
    fs.mkdirSync(path.join(root, 'pkg', 'src'), { recursive: true });
    fs.writeFileSync(path.join(root, 'pkg', 'src', 'foo.ts'), 'export const x = 1;\n', 'utf-8');
    fs.writeFileSync(
      path.join(root, 'openspec', 'config.json'),
      JSON.stringify({
        schema: 'spec-driven',
        tests: [
          {
            root: 'pkg',
            cwd: 'jest',
            framework: 'vitest',
            includes: ['**/*.ts'],
            mutation: { score: 50 },
          },
        ],
      }),
      'utf-8',
    );
    return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
  }

  function vitestPlan(
    overrides: Partial<{
      cwd: string;
      mutation_cwd: string;
      mutation_script: { shell: string; cmd: string } | null;
    }> = {},
  ) {
    const cfg = getFrameworkConfig('vitest');
    return {
      cwd: 'pkg/jest',
      root: 'pkg',
      framework: 'vitest' as const,
      coverage_format: cfg.coverage_format,
      coverage_output: cfg.coverage_output,
      mutation_cwd: 'pkg',
      mutation_score: 50,
      mutation_script: cfg.shell.mutation_execution
        ? {
            shell: cfg.shell.mutation_execution('99.0.0'),
            cmd: cfg.cmd.mutation_execution
              ? cfg.cmd.mutation_execution('99.0.0')
              : cfg.shell.mutation_execution('99.0.0'),
          }
        : null,
      script: {
        shell: cfg.shell.test_execution('99.0.0'),
        cmd: cfg.cmd.test_execution('99.0.0'),
      },
      ...overrides,
    };
  }

  it('抬根时 prefix 指向 pkg/jest 绝对路径，exec cwd 为 pkg（AC-6）', () => {
    const project = createPkgProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'pkg_vitest');
      const expectedPrefix = path.resolve(project.root, 'pkg/jest');
      const strykerCwds: string[] = [];

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          strykerCwds.push(opts?.cwd ?? '');
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(String(cmd)).not.toMatch(/(^|\s)-p(\s|$)/);
          fs.mkdirSync(planDir, { recursive: true });
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify(MUTATION_REPORT),
            'utf-8',
          );
          return '';
        }
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'pkg', 'src', 'foo.test.ts'),
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

      const cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(project.root);
      try {
        const result = executePlanEntry(vitestPlan(), project.root, { reportsDir });
        expect(result.mutation).not.toBeNull();
        expect(strykerCwds).toEqual(['pkg']);
      } finally {
        cwdSpy.mockRestore();
      }
    } finally {
      project.cleanup();
    }
  });

  it('stryker 失败时命令仍含绝对 --prefix，临时 config 被清理（AC-6）', () => {
    const project = createPkgProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'pkg_vitest');
      const mutationCwd = path.join(project.root, 'pkg');
      let configPathSeen = '';
      let capturedCmd = '';

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          capturedCmd = String(cmd);
          const configs = fs
            .readdirSync(mutationCwd)
            .filter((n) => n.startsWith('stryker.config.') && n.endsWith('.json'));
          configPathSeen = configs[0] ? path.join(mutationCwd, configs[0]) : '';
          expect(capturedCmd).toContain('--prefix');
          expect(capturedCmd).not.toMatch(/(^|\s)-p(\s|$)/);
          const err = new Error('stryker fail') as Error & { status: number };
          err.status = 1;
          throw err;
        }
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'pkg', 'src', 'foo.test.ts'),
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

      const result = executePlanEntry(vitestPlan(), project.root, { reportsDir });
      expect(result.mutation).toBeNull();
      expect(result.error).toMatch(/Mutation testing failed/);
      if (configPathSeen) {
        expect(fs.existsSync(configPathSeen)).toBe(false);
      }
    } finally {
      project.cleanup();
    }
  });

  it('pkg/jest 无 node_modules 时 mock 仍只断言命令形态（边界）', () => {
    const project = createPkgProject();
    try {
      expect(fs.existsSync(path.join(project.root, 'pkg', 'jest', 'node_modules'))).toBe(false);
      const reportsDir = path.join(project.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'pkg_vitest');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(opts?.cwd).toBe('pkg');
          expect(String(cmd)).toContain('--prefix');
          fs.mkdirSync(planDir, { recursive: true });
          fs.writeFileSync(
            path.join(planDir, 'mutation.json'),
            JSON.stringify(MUTATION_REPORT),
            'utf-8',
          );
          return '';
        }
        fs.mkdirSync(planDir, { recursive: true });
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: path.join(project.root, 'pkg', 'src', 'foo.test.ts'),
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

      const cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(project.root);
      try {
        const result = executePlanEntry(vitestPlan(), project.root, { reportsDir });
        expect(result.mutation).not.toBeNull();
      } finally {
        cwdSpy.mockRestore();
      }
    } finally {
      project.cleanup();
    }
  });

  it('mutation_cwd=.、cwd=. → exec cwd 为 .；--prefix 为绝对 projectRoot（AC-7）', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'vitest');
      const expectedPrefix = path.resolve(project.root, '.');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(opts?.cwd).toBe('.');
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(path.isAbsolute(expectedPrefix)).toBe(true);
          fs.mkdirSync(planDir, { recursive: true });
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

      const cfg = getFrameworkConfig('vitest');
      executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_cwd: '.',
          mutation_score: 50,
          mutation_script: cfg.shell.mutation_execution
            ? {
                shell: cfg.shell.mutation_execution('99.0.0'),
                cmd: cfg.cmd.mutation_execution
                  ? cfg.cmd.mutation_execution('99.0.0')
                  : cfg.shell.mutation_execution('99.0.0'),
              }
            : null,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
    } finally {
      project.cleanup();
    }
  });

  it('残缺 mutation 模板无 {prefix} → 命令不含绝对 --prefix', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'vitest');
      let strykerCmd = '';

      mockExecSync.mockImplementation((cmd: unknown) => {
        if (String(cmd).includes('stryker')) {
          strykerCmd = String(cmd);
          fs.mkdirSync(planDir, { recursive: true });
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

      const cfg = getFrameworkConfig('vitest');
      executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_cwd: '.',
          mutation_score: 50,
          mutation_script: {
            shell: 'npx stryker run "{config}"',
            cmd: 'npx stryker run "{config}"',
          },
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(strykerCmd).not.toContain('--prefix');
    } finally {
      project.cleanup();
    }
  });

  it('mutation_cwd=.、cwd=src → exec cwd 仍为 .；prefix 为 resolve(projectRoot, src)', () => {
    const project = createTempProject();
    try {
      fs.mkdirSync(path.join(project.root, 'src'), { recursive: true });
      const reportsDir = path.join(project.root, 'reports', 'test');
      const planDir = path.join(reportsDir, 'vitest');
      const expectedPrefix = path.resolve(project.root, 'src');

      mockExecSync.mockImplementation((cmd: unknown, opts?: { cwd?: string }) => {
        if (String(cmd).includes('stryker')) {
          expect(opts?.cwd).toBe('.');
          expect(String(cmd)).toContain(`--prefix "${expectedPrefix}"`);
          expect(String(cmd)).not.toContain('--prefix "src"');
          fs.mkdirSync(planDir, { recursive: true });
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

      const cfg = getFrameworkConfig('vitest');
      executePlanEntry(
        {
          cwd: 'src',
          root: '.',
          framework: 'vitest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          mutation_cwd: '.',
          mutation_score: 50,
          mutation_script: cfg.shell.mutation_execution
            ? {
                shell: cfg.shell.mutation_execution('99.0.0'),
                cmd: cfg.cmd.mutation_execution
                  ? cfg.cmd.mutation_execution('99.0.0')
                  : cfg.shell.mutation_execution('99.0.0'),
              }
            : null,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
    } finally {
      project.cleanup();
    }
  });
});
