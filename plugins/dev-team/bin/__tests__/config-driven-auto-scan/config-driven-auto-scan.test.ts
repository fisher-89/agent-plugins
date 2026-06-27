/**
 * 集成测试: test-resolve-paths config-driven 自动扫描 (AC-1)
 *
 * 验证 modules: [] 时通过 test.overrides 配置自动扫描目录文件。
 *
 * @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestResolvePaths } from '../../src/commands/test-resolve-paths';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ===========================================================================
// config-driven 自动扫描 (AC-1)
// ===========================================================================

describe('config-driven 自动扫描 — 空 modules 时自动扫描', () => {
  it('modules: [] 且 config.json 含 test.overrides 时，unit_tests 包含 src/ 下源文件的推导结果 (AC-1)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'src', framework: 'vite-plus' }],
      },
    });
    try {
      // 创建 src/ 下的源文件
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

  it('modules: [] 时 runTestDetectFrameworks 被真实调用且 plan[].directory 被用作扫描根目录 (AC-1)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'packages/lib', framework: 'vitest' }],
      },
    });
    try {
      // 创建在 override 覆盖目录内的源文件
      writeFile(project.root, 'packages/lib/format.ts', '');
      writeFile(project.root, 'packages/lib/parse.ts', '');
      // 创建在 override 覆盖目录外的源文件（同 project 但不同子目录）
      writeFile(project.root, 'packages/cli/main.ts', '');
      writeFile(project.root, 'docs/index.ts', '');
      // 创建覆盖目录内的非源文件（应被过滤）
      writeFile(project.root, 'packages/lib/README.md', '# readme');

      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      // plan[].directory = "packages/lib"，该目录下源文件应被扫描并推导
      expect(result.unit_tests).toContainEqual({
        source: 'packages/lib/format.ts',
        test_file: 'packages/lib/format.test.ts',
      });
      expect(result.unit_tests).toContainEqual({
        source: 'packages/lib/parse.ts',
        test_file: 'packages/lib/parse.test.ts',
      });

      // packages/cli/ 和 docs/ 不在 plan[].directory 范围内，不应出现
      expect(result.unit_tests).not.toContainEqual(
        expect.objectContaining({ source: 'packages/cli/main.ts' }),
      );
      expect(result.unit_tests).not.toContainEqual(
        expect.objectContaining({ source: 'docs/index.ts' }),
      );

      // 非源文件（.md）不应出现
      expect(result.unit_tests).not.toContainEqual(
        expect.objectContaining({ source: 'packages/lib/README.md' }),
      );

      expect(result.errors).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('modules: [] 且计划目录下含非源文件（.md, .json）时应被过滤，不进入 unit_tests', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        overrides: [{ file: 'src', framework: 'vite-plus' }],
      },
    });
    try {
      writeFile(project.root, 'src/app.ts', '');
      writeFile(project.root, 'src/README.md', '# readme');
      writeFile(project.root, 'src/config.json', '{}');
      writeFile(project.root, 'src/app.test.ts', ''); // 测试文件也跳过

      const result = runTestResolvePaths({
        modules: [],
        project_root: project.root,
      });

      const sources = result.unit_tests.map((u) => u.source);
      expect(sources).toContain('src/app.ts');
      expect(sources).not.toContain('src/README.md');
      expect(sources).not.toContain('src/config.json');
      expect(sources).not.toContain('src/app.test.ts');
    } finally {
      project.cleanup();
    }
  });
});
