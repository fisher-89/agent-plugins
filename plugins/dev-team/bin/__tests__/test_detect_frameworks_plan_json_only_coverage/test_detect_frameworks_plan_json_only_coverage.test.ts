/**
 * 集成测试: test_detect_frameworks plan 与 script JSON-only 覆盖率端到端一致性
 *
 * 覆盖 AC-9：plan 条目 coverage_cmd/coverage_artifacts 与注册表一致，
 * generateScript 脚本末行等于 JSON-only coverage_cmd。
 *
 * @see openspec/changes/simplify-test-report-schema/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestDetectFrameworks, generateScript } from '../../src/commands/test-detect-frameworks';
import { runTestGetFrameworkConfig } from '../../src/commands/test-get-framework-config';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'json-only-cov-test-'));
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

const ALL_FRAMEWORKS = ['jest', 'vitest', 'vite-plus', 'bun', 'rust'] as const;

const FRAMEWORK_TEST_FILES: Record<(typeof ALL_FRAMEWORKS)[number], string> = {
  jest: 'src/app.spec.js',
  vitest: 'src/app.test.ts',
  'vite-plus': 'src/app.test.ts',
  bun: 'src/app.test.ts',
  rust: 'tests/integration_test.rs',
};

// ---------------------------------------------------------------------------
// AC-9: plan 与注册表一致
// ---------------------------------------------------------------------------

describe('test_detect_frameworks — plan JSON-only 与注册表一致 (AC-9)', () => {
  for (const framework of ALL_FRAMEWORKS) {
    it(`plan[0] 的 coverage_cmd/coverage_artifacts 应与 runTestGetFrameworkConfig("${framework}") 一致`, () => {
      const project = createTempProject({
        schema: 'spec-driven',
        test: { framework },
      });
      try {
        const expected = runTestGetFrameworkConfig({ framework });
        const result = runTestDetectFrameworks({
          files: [FRAMEWORK_TEST_FILES[framework]],
          projectRoot: project.root,
        });
        expect(result.plan).toHaveLength(1);
        expect(result.plan[0].coverage_cmd).toBe(expected.coverage_cmd);
        expect(result.plan[0].coverage_artifacts).toEqual(expected.coverage_artifacts);
      } finally {
        project.cleanup();
      }
    });
  }

  it('rust plan 仍含 coverage_cleanup: ["coverage", "target/llvm-cov"]（未变）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'rust' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/integration_test.rs'],
        projectRoot: project.root,
      });
      expect(result.plan[0].coverage_cleanup).toEqual(['coverage', 'target/llvm-cov']);
    } finally {
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-9: generateScript 末行
// ---------------------------------------------------------------------------

describe('test_detect_frameworks — generateScript 末行 JSON-only coverage_cmd (AC-9)', () => {
  it('vitest plan 生成的 script 最后一行应含 --coverage.reporter=json-summary', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'vitest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.test.ts'],
        projectRoot: project.root,
      });
      const script = result.plan[0].script;
      const lastLine = script.trimEnd().split('\n').pop() ?? '';
      expect(lastLine).toContain('--coverage.reporter=json-summary');
      expect(lastLine).toBe(result.plan[0].coverage_cmd);
    } finally {
      project.cleanup();
    }
  });

  it('jest plan 生成的 script 最后一行应含 --coverageReporters=json-summary', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: { framework: 'jest' },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/app.spec.js'],
        projectRoot: project.root,
      });
      const script = result.plan[0].script;
      const lastLine = script.trimEnd().split('\n').pop() ?? '';
      expect(lastLine).toContain('--coverageReporters=json-summary');
      expect(lastLine).toBe(result.plan[0].coverage_cmd);
    } finally {
      project.cleanup();
    }
  });

  it('generateScript 输出末行应等于 JSON-only coverage_cmd（vitest 直接调用）', () => {
    const expected = runTestGetFrameworkConfig({ framework: 'vitest' });
    const script = generateScript({
      directory: '.',
      coverage_cmd: expected.coverage_cmd,
      coverage_cleanup: expected.coverage_cleanup,
    });
    const lastLine = script.trimEnd().split('\n').pop() ?? '';
    expect(lastLine).toBe(expected.coverage_cmd);
    expect(lastLine).toContain('json-summary');
  });
});
