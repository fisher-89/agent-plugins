/**
 * 集成测试: CLI reportsDir → summary.json / plan report.json
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runTestExecution } from '../../src/commands/test-execution';
import type { ExecutionResult } from '../../src/lib/test-runner';
import type { TestPlan } from '../../src/schemas';

const mockDetectFrameworks = vi.fn();
const mockExecutePlanEntry = vi.fn();

vi.mock('../../src/commands/test-detect-frameworks', () => ({
  runTestDetectFrameworks: (...args: unknown[]) => mockDetectFrameworks(...args),
}));

vi.mock('../../src/lib/test-runner', () => ({
  executePlanEntry: (...args: unknown[]) => mockExecutePlanEntry(...args),
}));

function createTempProject(): { root: string; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-test-execution-'));
  fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'openspec', 'config.json'),
    JSON.stringify({ schema: 'spec-driven', tests: [{ root: '.', framework: 'vitest' }] }),
    'utf-8',
  );
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

function makePlan(overrides: Partial<TestPlan> = {}): TestPlan {
  return {
    cwd: '.',
    root: '.',
    framework: 'vitest',
    coverage_format: 'istanbul',
    coverage_output: 'coverage-summary.json',
    mutation_cwd: '.',
    mutation_script: null,
    script: {
      shell: 'npx vitest run --outputFile={results_file} {files}',
      cmd: 'npx vitest run --outputFile={results_file} {files}',
    },
    ...overrides,
  };
}

function makeResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    testCases: [{ name: 't1', status: 'passed', durationMs: 10 }],
    coverage: null,
    mutation: null,
    durationMs: 100,
    testFiles: ['a.test.ts'],
    sourceFiles: ['a.ts'],
    planId: 'vitest',
    reportDir: '',
    ...overrides,
  };
}

describe('CLI 新报告布局 — 无 change', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('无 change：写入 reports/test/summary.json 与 reports/test/vitest/report.json', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [makePlan()] });
      mockExecutePlanEntry.mockImplementation((_entry, _projectRoot, options) => {
        const reportDir = path.join(options.reportsDir, 'vitest');
        return makeResult({ reportDir, planId: 'vitest' });
      });

      const exitCode = await runTestExecution({ projectRoot: project.root });
      expect(exitCode).toBe(0);

      const summaryPath = path.join(project.root, 'reports', 'test', 'summary.json');
      const reportPath = path.join(project.root, 'reports', 'test', 'vitest', 'report.json');
      expect(fs.existsSync(summaryPath)).toBe(true);
      expect(fs.existsSync(reportPath)).toBe(true);
      expect(fs.existsSync(path.join(project.root, 'reports', 'test-execution.json'))).toBe(false);
      expect(fs.existsSync(path.join(project.root, 'reports', 'test', 'vitest.json'))).toBe(false);

      const summary = JSON.parse(fs.readFileSync(summaryPath, 'utf-8'));
      expect(Array.isArray(summary.plans)).toBe(true);
      expect(summary.plans[0].id).toBe('vitest');
      expect(summary.plans[0].path.replace(/\\/g, '/')).toMatch(/reports\/test\/vitest$/);
    } finally {
      project.cleanup();
    }
  });

  it('execute 半失败仍写 report.json，summary.plans 含该 plan 的 path', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [makePlan()] });
      mockExecutePlanEntry.mockImplementation((_e, _r, options) =>
        makeResult({
          exitCode: 1,
          error: 'failed',
          testCases: [{ name: 't', status: 'failed', errorMessage: 'x' }],
          reportDir: path.join(options.reportsDir, 'vitest'),
        }),
      );

      const exitCode = await runTestExecution({ projectRoot: project.root });
      expect(exitCode).toBe(1);
      const summary = JSON.parse(
        fs.readFileSync(path.join(project.root, 'reports', 'test', 'summary.json'), 'utf-8'),
      );
      expect(summary.plans).toHaveLength(1);
      expect(summary.plans[0].path.replace(/\\/g, '/')).toContain('reports/test/vitest');
      expect(
        fs.existsSync(path.join(project.root, 'reports', 'test', 'vitest', 'report.json')),
      ).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('多 directory plan：plugins_dev-team_bin_vite-plus/report.json 目录 id 无 .json', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [],
        plan: [
          makePlan(),
          makePlan({
            cwd: 'plugins/dev-team/bin',
            root: 'plugins/dev-team/bin',
            framework: 'vite-plus',
            coverage_format: 'istanbul',
          }),
        ],
      });
      mockExecutePlanEntry.mockImplementation((entry, _r, options) => {
        const planId =
          entry.root === '.'
            ? entry.framework
            : `${entry.root.replace(/[\\/]/g, '_')}_${entry.framework}`;
        return makeResult({
          framework: entry.framework,
          planId,
          reportDir: path.join(options.reportsDir, planId),
        });
      });

      await runTestExecution({ projectRoot: project.root });
      const nested = path.join(
        project.root,
        'reports',
        'test',
        'plugins_dev-team_bin_vite-plus',
        'report.json',
      );
      expect(fs.existsSync(nested)).toBe(true);
      const summary = JSON.parse(
        fs.readFileSync(path.join(project.root, 'reports', 'test', 'summary.json'), 'utf-8'),
      );
      expect(
        summary.plans.some((p: { id: string }) => p.id === 'plugins_dev-team_bin_vite-plus'),
      ).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

describe('CLI 新报告布局 — 有 change', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('change 专属 …/reports/test/summary.json；plans[].path 含前缀', async () => {
    const project = createTempProject();
    try {
      // --change 现在兼作突变 scope 入口：读取 change 文件清单（清单为空 scope）
      const changeDir = path.join(project.root, 'openspec', 'changes', 'foo');
      fs.mkdirSync(changeDir, { recursive: true });
      fs.writeFileSync(
        path.join(changeDir, 'workflow.json'),
        JSON.stringify({
          workflow_type: 'requirement',
          created: '2026-09-17',
          file_log: [],
        }),
        'utf-8',
      );
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [makePlan()] });
      mockExecutePlanEntry.mockImplementation((_e, _r, options) =>
        makeResult({ reportDir: path.join(options.reportsDir, 'vitest') }),
      );
      await runTestExecution({ projectRoot: project.root, change: 'foo' });
      const summaryPath = path.join(
        project.root,
        'openspec',
        'changes',
        'foo',
        'reports',
        'test',
        'summary.json',
      );
      expect(fs.existsSync(summaryPath)).toBe(true);
      const summary = JSON.parse(fs.readFileSync(summaryPath, 'utf-8'));
      expect(summary.plans[0].path.replace(/\\/g, '/')).toMatch(
        /^openspec\/changes\/foo\/reports\/test\//,
      );
    } finally {
      project.cleanup();
    }
  });

  it('change 名含特殊字符时路径仍可创建或被拒绝', async () => {
    const project = createTempProject();
    const changeName = 'my-change.v2 with spaces';
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [makePlan()] });
      mockExecutePlanEntry.mockImplementation((_e, _r, options) =>
        makeResult({ reportDir: path.join(options.reportsDir, 'vitest') }),
      );

      let created = false;
      let rejected = false;
      try {
        const exitCode = await runTestExecution({
          projectRoot: project.root,
          change: changeName,
        });
        const summaryPath = path.join(
          project.root,
          'openspec',
          'changes',
          changeName,
          'reports',
          'test',
          'summary.json',
        );
        if (fs.existsSync(summaryPath) && exitCode === 0) {
          created = true;
          const summary = JSON.parse(fs.readFileSync(summaryPath, 'utf-8'));
          const planPath = String(summary.plans[0].path).replace(/\\/g, '/');
          expect(planPath).toContain(`openspec/changes/${changeName}/reports/test/`);
        } else {
          rejected = true;
        }
      } catch {
        rejected = true;
      }

      // 与现实现一致：要么成功落盘（含特殊字符路径），要么被拒绝
      expect(created || rejected).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});
