/**
 * 集成测试: test_detect_frameworks plan 输出结构（tests[]）
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';
import { testDetectFrameworksOutputSchema } from '../../src/schemas/test-detect-frameworks.schema';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'plan-output-test-'));
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

describe('test_detect_frameworks — plan 输出结构', () => {
  it('tests[] vitest suite 时 plan 应含完整字段且通过 schema 校验', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });

      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'vitest',
        coverage_format: 'istanbul',
      });
      expect(result.plan[0]).toHaveProperty('coverage_output');
      expect(result.plan[0]).toHaveProperty('script');

      const parsed = testDetectFrameworksOutputSchema.safeParse(result);
      expect(parsed.success).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('无 tests 配置时 plan 应为空数组且 detected 均为 unknown', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts', 'readme.md'],
        projectRoot: project.root,
      });

      expect(result.plan).toEqual([]);
      expect(result.detected.every((d) => d.framework === 'unknown')).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

describe('test_detect_frameworks — plan 双脚本内容验证', () => {
  it('script.shell / script.cmd 含未展开 report 占位符且含 test_execution', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest' }],
    });
    try {
      const { shell, cmd } = runTestDetectFrameworks({ projectRoot: project.root }).plan[0].script;
      expect(shell).toContain('{results_file}');
      expect(shell).toContain('{report_dir}');
      expect(shell).toContain('npx vitest');
      expect(cmd).toContain('{results_file}');
      expect(cmd).toContain('npx vitest');
      expect(cmd).not.toMatch(/[\r\n]/);
    } finally {
      project.cleanup();
    }
  });

  it('不注入 suite cwd coverage_cleanup（rm -rf / if exist / rmdir）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const { shell, cmd } = runTestDetectFrameworks({ projectRoot: project.root }).plan[0].script;
      expect(shell).not.toContain('rm -rf');
      expect(shell).not.toMatch(/^cd /);
      expect(cmd).not.toContain('cd /d');
      expect(cmd).not.toMatch(/if exist|rmdir/);
      expect(cmd).not.toContain('(if exist "coverage"');
      expect(cmd).not.toContain(' & ');
      expect(cmd).toContain('npx vitest');
    } finally {
      project.cleanup();
    }
  });
});
