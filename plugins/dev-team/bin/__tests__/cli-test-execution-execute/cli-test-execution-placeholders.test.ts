/**
 * 集成测试: detect 占位符 → execute 展开 → reportDir 产物
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type * as TestFrameworkDefault from '../../src/lib/test-framework';

const mockExecSync = vi.fn();
vi.mock('child_process', () => ({
  execSync: (...args: unknown[]) => mockExecSync(...args),
}));

vi.mock('../../src/lib/test-framework', async (importOriginal) => {
  const actual = await importOriginal<typeof TestFrameworkDefault>();
  return {
    ...actual,
    detectFrameworkVersion: () => '99.0.0',
  };
});

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';
import { executePlanEntry } from '../../src/lib/test-runner';

function createTempProject(framework: string): { root: string; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-placeholders-'));
  fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'openspec', 'config.json'),
    JSON.stringify({ schema: 'spec-driven', tests: [{ root: '.', framework }] }),
    'utf-8',
  );
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

describe('占位符延迟展开', () => {
  beforeEach(() => {
    mockExecSync.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it('detect 后 script 含未展开 {results_file}/{report_dir}/{config_args}', () => {
    const project = createTempProject('jest');
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      expect(plan.script.shell).toContain('{results_file}');
      expect(plan.script.shell).toContain('{report_dir}');
      expect(plan.script.shell).toContain('{config_args}');
      expect(plan.script.shell).not.toMatch(/reports[/\\]test[/\\]/);
    } finally {
      project.cleanup();
    }
  });

  it('execute 后命令中占位符已被相对 absCwd 路径替换', () => {
    const project = createTempProject('jest');
    try {
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      const reportsDir = path.join(project.root, 'reports', 'test');
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
      executePlanEntry(plan, project.root, { reportsDir });
      const cmd = String(mockExecSync.mock.calls[0][0]);
      expect(cmd).not.toContain('{results_file}');
      expect(cmd).not.toContain('{report_dir}');
      expect(cmd).toContain('results.json');
    } finally {
      project.cleanup();
    }
  });

  it('detect 不创建 reportDir、不写临时 bunfig', () => {
    const project = createTempProject('bun');
    try {
      runTestDetectFrameworks({ projectRoot: project.root });
      expect(fs.existsSync(path.join(project.root, 'reports', 'test'))).toBe(false);
      expect(fs.readdirSync(project.root).some((n) => n.startsWith('bunfig.dev-team-'))).toBe(
        false,
      );
    } finally {
      project.cleanup();
    }
  });

  it('execute 后 bunfig tempPaths 被删除；用户目录无残留', () => {
    const project = createTempProject('bun');
    try {
      const userBunfig = path.join(project.root, 'bunfig.toml');
      fs.writeFileSync(userBunfig, '[test]\npreload=[]\n', 'utf-8');
      const before = fs.readFileSync(userBunfig, 'utf-8');
      const plan = runTestDetectFrameworks({ projectRoot: project.root }).plan[0];
      const reportsDir = path.join(project.root, 'reports', 'test');
      const seenTemps: string[] = [];
      mockExecSync.mockImplementation(() => {
        const temps = fs
          .readdirSync(project.root)
          .filter((n) => n.startsWith('bunfig.dev-team-'))
          .map((n) => path.join(project.root, n));
        seenTemps.push(...temps);
        fs.mkdirSync(path.join(reportsDir, 'bun'), { recursive: true });
        fs.writeFileSync(path.join(reportsDir, 'bun', 'results.txt'), '1 [PASS] ok\n', 'utf-8');
        return '';
      });
      executePlanEntry(plan, project.root, { reportsDir });
      expect(seenTemps.length).toBeGreaterThan(0);
      for (const t of seenTemps) {
        expect(fs.existsSync(t)).toBe(false);
      }
      expect(fs.readFileSync(userBunfig, 'utf-8')).toBe(before);
    } finally {
      project.cleanup();
    }
  });
});
