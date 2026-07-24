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
      expect(result.plan[0]).toHaveProperty('coverage_artifacts');
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
  it('script.shell 包含 `rm -rf` 和 bash 语法命令', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: '.', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.shell).toContain('rm -rf');
    } finally {
      project.cleanup();
    }
  });

  it('script.cmd 包含 Windows cmd 清理语法', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vitest' }],
    });
    try {
      const result = runTestDetectFrameworks({ projectRoot: project.root });
      expect(result.plan[0].script.cmd).toContain('cd /d');
      expect(result.plan[0].script.cmd).toMatch(/if exist|rmdir/);
    } finally {
      project.cleanup();
    }
  });
});
