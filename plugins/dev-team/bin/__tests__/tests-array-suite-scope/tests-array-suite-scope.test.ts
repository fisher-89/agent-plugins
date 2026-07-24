/**
 * 集成测试: suite scope → detect / exclude / resolve
 *
 * @see openspec/changes/tests-array-cwd-config/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { runTestDetectFrameworks } from '../../src/commands/test-detect-frameworks';
import { runTestResolvePaths } from '../../src/commands/test-resolve-paths';
import { readConfig } from '../../src/lib/config';
import { isFileExcluded } from '../../src/lib/test-exclude';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'tests-array-scope-'));
  fs.mkdirSync(path.join(tmpDir, 'openspec'), { recursive: true });
  fs.writeFileSync(
    path.join(tmpDir, 'openspec', 'config.json'),
    JSON.stringify(configData, null, 2),
    'utf-8',
  );
  return { root: tmpDir, cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }) };
}

function writeFile(projectRoot: string, relativePath: string, content = ''): void {
  const fullPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf-8');
}

describe('cwd 上移不改变扫描根', () => {
  it('root 内源文件进入 unit_tests；root 外同级目录文件不进入', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg/src',
          framework: 'vite-plus',
          cwd: '..',
          includes: ['**/*.{ts,tsx}'],
        },
      ],
    });
    try {
      writeFile(project.root, 'pkg/src/app.ts');
      writeFile(project.root, 'pkg/sibling.ts');
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.unit_tests.map((u) => u.source)).toContain('pkg/src/app.ts');
      expect(result.unit_tests.map((u) => u.source)).not.toContain('pkg/sibling.ts');
    } finally {
      project.cleanup();
    }
  });

  it('detect 对显式 files 入参同样遵守 suite scope / excludes', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'pkg',
          framework: 'vitest',
          includes: ['**/*.test.ts'],
          excludes: ['**/skip/**'],
        },
      ],
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['pkg/ok.test.ts', 'pkg/skip/x.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected.find((d) => d.file.includes('ok'))?.framework).toBe('vitest');
      expect(result.detected.find((d) => d.file.includes('skip'))).toBeUndefined();
    } finally {
      project.cleanup();
    }
  });

  it('无有效 tests 时 resolve errors 引导配置 tests', () => {
    const project = createTempProject({ schema: 'spec-driven', tests: [] });
    try {
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.errors[0].message).toContain('tests');
    } finally {
      project.cleanup();
    }
  });

  it('省略 includes 时仅匹配框架 default_glob（源码扫描为空）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [{ root: 'pkg', framework: 'vite-plus', cwd: '..' }],
    });
    try {
      writeFile(project.root, 'pkg/app.ts');
      writeFile(project.root, 'pkg/app.test.ts');
      const result = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(result.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

describe('includes/excludes 三方一致', () => {
  it('excludes 命中文件同时从 detected 与 unit_tests 消失', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vite-plus',
          includes: ['**/*.{ts,tsx}'],
          excludes: ['**/hidden.ts'],
        },
      ],
    });
    try {
      writeFile(project.root, 'src/ok.ts');
      writeFile(project.root, 'src/hidden.ts');
      const config = readConfig(project.root);
      expect(isFileExcluded('src/hidden.ts', config)).toBe(true);

      const detect = runTestDetectFrameworks({
        files: ['src/ok.ts', 'src/hidden.ts'],
        projectRoot: project.root,
      });
      expect(detect.detected.find((d) => d.file.includes('hidden'))).toBeUndefined();

      const resolve = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(resolve.unit_tests.map((u) => u.source)).toEqual(['src/ok.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('includes 收窄后范围外文件不被检测/推导', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vite-plus',
          includes: ['**/only.ts'],
        },
      ],
    });
    try {
      writeFile(project.root, 'src/only.ts');
      writeFile(project.root, 'src/other.ts');
      const resolve = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(resolve.unit_tests.map((u) => u.source)).toEqual(['src/only.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('includes/excludes 同时为空数组时 detected/unit_tests 为空且不崩溃', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vite-plus',
          includes: [],
          excludes: [],
        },
      ],
    });
    try {
      writeFile(project.root, 'src/a.ts');
      writeFile(project.root, 'src/a.test.ts');
      expect(() => runTestResolvePaths({ modules: [], project_root: project.root })).not.toThrow();
      // includes: [] → default_glob → 源文件不匹配
      const resolve = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(resolve.unit_tests).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('多 suite 不同 excludes 时互不污染', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'a',
          framework: 'vite-plus',
          includes: ['**/*.{ts,tsx}'],
          excludes: ['**/x.ts'],
        },
        {
          root: 'b',
          framework: 'vitest',
          includes: ['**/*.{ts,tsx}'],
          excludes: ['**/y.ts'],
        },
      ],
    });
    try {
      writeFile(project.root, 'a/x.ts');
      writeFile(project.root, 'a/ok.ts');
      writeFile(project.root, 'b/y.ts');
      writeFile(project.root, 'b/ok.ts');
      const resolve = runTestResolvePaths({ modules: [], project_root: project.root });
      const sources = resolve.unit_tests.map((u) => u.source).sort();
      expect(sources).toEqual(['a/ok.ts', 'b/ok.ts']);
    } finally {
      project.cleanup();
    }
  });

  it('excludes 为单元素 / 超大列表时三方结果一致', () => {
    const excludes = Array.from({ length: 100 }, (_, i) => `**/skip${i}.ts`);
    excludes.push('**/drop.ts');
    const project = createTempProject({
      schema: 'spec-driven',
      tests: [
        {
          root: 'src',
          framework: 'vite-plus',
          includes: ['**/*.{ts,tsx}'],
          excludes,
        },
      ],
    });
    try {
      writeFile(project.root, 'src/keep.ts');
      writeFile(project.root, 'src/drop.ts');
      const resolve = runTestResolvePaths({ modules: [], project_root: project.root });
      expect(resolve.unit_tests.map((u) => u.source)).toEqual(['src/keep.ts']);
      const config = readConfig(project.root);
      expect(isFileExcluded('src/drop.ts', config)).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});
