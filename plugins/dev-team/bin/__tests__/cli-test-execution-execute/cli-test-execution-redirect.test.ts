/**
 * 集成测试: preparePlanArtifacts → 条件重定向 → parsePlanArtifacts
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
import { executePlanEntry } from '../../src/lib/test-runner';

function createTempProject(): { root: string; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-redirect-'));
  fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'openspec', 'config.json'),
    JSON.stringify({ schema: 'spec-driven', tests: [{ root: '.', framework: 'jest' }] }),
    'utf-8',
  );
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

function capturedCmd(): string {
  return String(mockExecSync.mock.calls[0][0]);
}

describe('jest 原生文件通道', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it('jest 命令含 --outputFile= 指向 planDir/results.json，无壳层 >', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const cfg = getFrameworkConfig('jest');
      mockExecSync.mockImplementation(() => {
        const planDir = path.join(reportsDir, 'jest');
        fs.writeFileSync(
          path.join(planDir, 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: 'a.test.ts',
                assertionResults: [{ title: 't', fullName: 't', status: 'passed' }],
              },
            ],
          }),
          'utf-8',
        );
        return '>>> noise\n';
      });
      const result = executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'jest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      const cmd = capturedCmd();
      expect(cmd).toContain('--outputFile=');
      expect(cmd).toContain('results.json');
      expect(cmd).not.toMatch(/>\s*"/);
      expect(result.testCases).toHaveLength(1);
    } finally {
      project.cleanup();
    }
  });

  it('exec 返回脏 stdout 时仍从文件解析成功', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const cfg = getFrameworkConfig('jest');
      mockExecSync.mockImplementation(() => {
        fs.writeFileSync(
          path.join(reportsDir, 'jest', 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: 'a.test.ts',
                assertionResults: [{ title: 't', fullName: 't', status: 'passed' }],
              },
            ],
          }),
          'utf-8',
        );
        return 'NOT JSON AT ALL <<<';
      });
      const result = executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'jest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.testCases.filter((t) => t.status === 'passed').length).toBe(1);
    } finally {
      project.cleanup();
    }
  });

  it('results.json 缺失 → error，不回退 stdout JSON', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const cfg = getFrameworkConfig('jest');
      mockExecSync.mockReturnValue(
        JSON.stringify({
          testResults: [
            {
              name: 'a.test.ts',
              assertionResults: [{ title: 't', fullName: 't', status: 'passed' }],
            },
          ],
        }),
      );
      const result = executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'jest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.testCases).toHaveLength(0);
      expect(result.error).toBeTruthy();
    } finally {
      project.cleanup();
    }
  });

  it('coverage 仅从 planDir 读取；cwd 旧 coverage 被忽略', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const oldCov = path.join(project.root, 'coverage');
      fs.mkdirSync(oldCov, { recursive: true });
      fs.writeFileSync(
        path.join(oldCov, 'coverage-summary.json'),
        JSON.stringify({
          total: { lines: { pct: 11 }, branches: { pct: 11 }, functions: { pct: 11 } },
        }),
        'utf-8',
      );
      const cfg = getFrameworkConfig('jest');
      mockExecSync.mockImplementation(() => {
        fs.writeFileSync(
          path.join(reportsDir, 'jest', 'results.json'),
          JSON.stringify({
            testResults: [
              {
                name: 'a.test.ts',
                assertionResults: [{ title: 't', fullName: 't', status: 'passed' }],
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
          framework: 'jest',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.coverage).toBeNull();
    } finally {
      project.cleanup();
    }
  });
});

describe('段级重定向框架（bun/go/pytest）', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it('bun/go/pytest 测试段命令含 > 且不含 tee；文件解析成功', () => {
    for (const framework of ['bun', 'go', 'pytest'] as const) {
      const project = createTempProject();
      try {
        const reportsDir = path.join(project.root, 'reports', 'test');
        const cfg = getFrameworkConfig(framework);
        mockExecSync.mockReset();
        mockExecSync.mockImplementation(() => {
          const planDir = path.join(reportsDir, framework);
          fs.mkdirSync(planDir, { recursive: true });
          if (framework === 'go') {
            fs.writeFileSync(
              path.join(planDir, 'results.ndjson'),
              `${JSON.stringify({ Action: 'pass', Test: 'TestA' })}\n`,
              'utf-8',
            );
          } else if (framework === 'bun') {
            fs.writeFileSync(path.join(planDir, 'results.txt'), '1 [PASS] ok\n', 'utf-8');
          } else {
            fs.writeFileSync(
              path.join(planDir, 'results.txt'),
              'tests/test_a.py::test_x PASSED\n',
              'utf-8',
            );
          }
          return '';
        });
        const result = executePlanEntry(
          {
            cwd: '.',
            root: '.',
            framework,
            coverage_format: cfg.coverage_format,
            coverage_output: cfg.coverage_output,
            script: {
              shell: cfg.shell.test_execution('99.0.0'),
              cmd: cfg.cmd.test_execution('99.0.0'),
            },
          },
          project.root,
          { reportsDir },
        );
        const cmd = capturedCmd();
        expect(cmd).toMatch(/>\s*"/);
        expect(cmd).not.toMatch(/\|\s*tee\b/);
        expect(result.testCases.length).toBeGreaterThan(0);
      } finally {
        project.cleanup();
      }
    }
  });

  it('结果文件空 → 可诊断失败', () => {
    const project = createTempProject();
    try {
      const reportsDir = path.join(project.root, 'reports', 'test');
      const cfg = getFrameworkConfig('bun');
      mockExecSync.mockImplementation(() => {
        fs.mkdirSync(path.join(reportsDir, 'bun'), { recursive: true });
        fs.writeFileSync(path.join(reportsDir, 'bun', 'results.txt'), '', 'utf-8');
        return '';
      });
      const result = executePlanEntry(
        {
          cwd: '.',
          root: '.',
          framework: 'bun',
          coverage_format: cfg.coverage_format,
          coverage_output: cfg.coverage_output,
          script: {
            shell: cfg.shell.test_execution('99.0.0'),
            cmd: cfg.cmd.test_execution('99.0.0'),
          },
        },
        project.root,
        { reportsDir },
      );
      expect(result.error).toBeTruthy();
    } finally {
      project.cleanup();
    }
  });
});
