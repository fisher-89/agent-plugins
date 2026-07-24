/**
 * 集成测试: test-resolve-paths config-driven 自动扫描
 *
 * 验证 modules: [] 时通过 tests[] 配置自动扫描目录文件。
 *
 * @see openspec/changes/tests-array-cwd-config/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestResolvePaths } from '../../src/commands/test-resolve-paths';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-driven-scan-'));
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

function writeFile(projectRoot: string, relativePath: string, content = ''): void {
  const fullPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf-8');
}

describe('config-driven 自动扫描 — 空 modules 时自动扫描', () => {
  it('modules: [] + tests[] 时，unit_tests 包含 src/ 下源文件的推导结果', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vite-plus',
          includes: ['**/*.{ts,tsx}'],
        },
      ],
    });
    try {
      writeFile(project.root, 'src/app.ts', '');
      writeFile(project.root, 'src/utils/helper.ts', '');

      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      expect(result.unit_tests).toContainEqual({
        source: 'src/app.ts',
        test_file: 'src/app.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'src/utils/helper.ts',
        test_file: 'src/utils/helper.test.ts',
      });
      expect(result.errors).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('modules: [] 时 suite.root 被用作扫描根目录，不扫描范围外文件', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'packages/lib',
          framework: 'vitest',
          includes: ['**/*.{ts,tsx}'],
        },
      ],
    });
    try {
      writeFile(project.root, 'packages/lib/format.ts', '');
      writeFile(project.root, 'packages/lib/parse.ts', '');
      writeFile(project.root, 'packages/cli/main.ts', '');
      writeFile(project.root, 'docs/index.ts', '');
      writeFile(project.root, 'packages/lib/README.md', '# readme');

      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      expect(result.unit_tests).toContainEqual({
        source: 'packages/lib/format.ts',
        test_file: 'packages/lib/format.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'packages/lib/parse.ts',
        test_file: 'packages/lib/parse.test.ts',
      });
      expect(result.unit_tests).not.toContainEqual(
        expect.objectContaining({ source: 'packages/cli/main.ts' }),
      );
      expect(result.unit_tests).not.toContainEqual(
        expect.objectContaining({ source: 'docs/index.ts' }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('仅含旧 test.overrides 的配置期望为无有效 tests 配置', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'src', framework: 'vite-plus' }],
      },
    });
    try {
      writeFile(project.root, 'src/app.ts', '');
      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });
      expect(result.unit_tests).toEqual([]);
      expect(result.errors[0].message).toContain('tests');
    } finally {
      project.cleanup();
    }
  });
});
