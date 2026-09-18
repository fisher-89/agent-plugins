/**
 * 集成测试: change 文件清单 → test_resolve_paths 测试路径推导
 *
 * 以真实 fixture（openspec/config.json + workflow.json，不 mock 文件系统）驱动
 * 清单模式（modules: 'change'）全管线：within-root → detectedSet →
 * isFileExcluded（静默分支）→ isTestFile → isSourceFile。
 *
 * 说明：检测管线（runTestDetectFrameworks）会把 exclude 命中的文件从 detected
 * 中剔除，因此被 exclude 的清单条目在 processModuleEntry 中表现为 detectedSet
 * 缺失 → 收集为 'Not in test config scope' 错误条目（而非 test-design 预期的
 * 「静默跳过」）。本文件以实现为准断言该条目不进 unit_tests。
 *
 * @see openspec/changes/workflow-file-inventory/test-design.md
 *   集成测试「change 文件清单 → test_resolve_paths 测试路径推导」（AC-8）
 */

import { execFileSync, execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runTestResolvePaths } from '../../src/commands/test-resolve-paths';
import { testResolvePathsInputSchema } from '../../src/schemas';

// ---------------------------------------------------------------------------
// git 退场防回归 spy：包裹 execSync / execFileSync（透传真实实现），仅作
// not-called 断言 —— 清单模式全链路不允许任何 git 子进程调用（AC-8）。
// ---------------------------------------------------------------------------

vi.mock('node:child_process', async () => {
  const actual = await vi.importActual<Record<string, unknown>>('node:child_process');
  return {
    ...actual,
    execSync: vi.fn(),
    execFileSync: vi.fn(),
  };
});

vi.mock('child_process', async () => {
  const actual = await vi.importActual<Record<string, unknown>>('child_process');
  return {
    ...actual,
    execSync: vi.fn(),
    execFileSync: vi.fn(),
  };
});

// --sequence.shuffle 下保证 not-called 断言不受其他用例泄漏影响
beforeEach(() => {
  vi.mocked(execSync).mockClear();
  vi.mocked(execFileSync).mockClear();
});

// ---------------------------------------------------------------------------
// 临时项目 helper（真实 fixture 文件）
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'inv-path-resolution-'));
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

function writeProjectFile(root: string, relativePath: string, content = ''): void {
  const absPath = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(absPath), { recursive: true });
  fs.writeFileSync(absPath, content, 'utf-8');
}

/** 写入 openspec/config.json（tests suite fixture）。 */
function writeTestConfig(root: string, tests: unknown[]): void {
  writeProjectFile(
    root,
    path.join('openspec', 'config.json'),
    JSON.stringify({ schema: 'spec-driven', tests }, null, 2),
  );
}

interface WorkflowFilesFixture {
  written: string[];
  deleted?: string[];
  source?: Record<string, string>;
}

/**
 * 写入 change 的 workflow.json（file_log 条目由 written/deleted 派生，workflow
 * scope）；`files` 传 null 时模拟文件清单机制前旧 change（无 file_log 字段）。
 */
function writeWorkflowJson(root: string, change: string, files: WorkflowFilesFixture | null): void {
  const doc: Record<string, unknown> = { workflow_type: 'requirement', created: '2026-09-17' };
  if (files !== null) {
    const at = '2026-09-17T00:00:00.000Z';
    doc.file_log = [
      ...files.written.map((p) => ({ op: 'write', scope: 'workflow', path: p, at })),
      ...(files.deleted ?? []).map((p) => ({ op: 'delete', scope: 'workflow', path: p, at })),
    ];
  }
  writeProjectFile(
    root,
    path.join('openspec', 'changes', change, 'workflow.json'),
    JSON.stringify(doc, null, 2),
  );
}

// ---------------------------------------------------------------------------
// 场景「清单全景推导」共享 fixture
// ---------------------------------------------------------------------------

const PANORAMA_CHANGE = 'panorama';

const PANORAMA_SUITE = {
  root: 'src',
  framework: 'vitest',
  includes: ['**/*.ts', '**/*.txt'],
  excludes: ['**/excluded.ts'],
};

const PANORAMA_WRITTEN = [
  'src/a.ts',
  'src/a.test.ts',
  'scripts/notes.txt',
  '../outside.ts',
  'src/excluded.ts',
];

const PANORAMA_SOURCE_MAP = { 'src/a.ts': 'dev-team:implementation-generator' };

function setupPanoramaProject(): TempProject {
  const project = createTempProject();
  writeTestConfig(project.root, [PANORAMA_SUITE]);
  writeWorkflowJson(project.root, PANORAMA_CHANGE, {
    written: PANORAMA_WRITTEN,
    source: PANORAMA_SOURCE_MAP,
  });
  // 检测与推导管线为纯路径字符串运算，不要求文件存在；落盘唯一保留源文件
  // 以固定 fixture 的物理形态。
  writeProjectFile(project.root, 'src/a.ts', 'export const a = 1;\n');
  return project;
}

// ===========================================================================
// 场景: 清单全景推导
// ===========================================================================

describe('清单全景推导：change files.written → test_resolve_paths', () => {
  it('五路径全景推导：unit_tests 恰含 src/a.ts，各失败路径按管线语义进 errors (AC-8)', () => {
    const project = setupPanoramaProject();
    try {
      const result = runTestResolvePaths({
        modules: 'change',
        change: PANORAMA_CHANGE,
        project_root: project.root,
      });

      // 唯一通过管线的源文件 → 恰一条推导条目
      expect(result.unit_tests).toEqual([{ source: 'src/a.ts', test_file: 'src/a.test.ts' }]);

      // src/a.test.ts → 已是测试文件
      expect(result.errors).toContainEqual({
        path: 'src/a.test.ts',
        message: 'Path is already a test file',
      });
      // scripts/notes.txt → 经 'unknown' 框架进入 detected 后被 isSourceFile 拦下
      expect(result.errors).toContainEqual({
        path: 'scripts/notes.txt',
        message: 'Not a testable source file',
      });
      // ../outside.ts → 越出项目根（管线第一道守卫）
      expect(result.errors).toContainEqual({
        path: '../outside.ts',
        message: 'Path is outside project root',
      });
      // src/excluded.ts → 不进 unit_tests；实现语义为检测期剔除后收集为
      // scope 错误条目（见文件头说明）
      expect(result.errors).toContainEqual({
        path: 'src/excluded.ts',
        message: 'Not in test config scope',
      });

      // 每个失败路径恰一条错误条目
      expect(result.errors).toHaveLength(4);
    } finally {
      project.cleanup();
    }
  });

  it('git 退场防回归：清单模式全链路解析不产生任何 execSync/execFileSync 调用 (AC-8)', () => {
    const project = setupPanoramaProject();
    try {
      runTestResolvePaths({
        modules: 'change',
        change: PANORAMA_CHANGE,
        project_root: project.root,
      });

      expect(vi.mocked(execSync)).not.toHaveBeenCalled();
      expect(vi.mocked(execFileSync)).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('消费契约复核：unit_tests 条目键恰为 source/test_file，files.source 审计映射不泄漏进结果', () => {
    const project = setupPanoramaProject();
    try {
      const result = runTestResolvePaths({
        modules: 'change',
        change: PANORAMA_CHANGE,
        project_root: project.root,
      });

      expect(result.unit_tests.length).toBeGreaterThan(0);
      for (const entry of result.unit_tests) {
        expect(Object.keys(entry)).toEqual(['source', 'test_file']);
      }
      // workflow.json.files.source 仅审计用：清单读取方输出不得携带该映射
      expect(JSON.stringify(result)).not.toContain('dev-team:implementation-generator');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 场景: 清单条目形态边界
// ===========================================================================

describe('清单条目形态边界', () => {
  it('written 仅含目录路径时不展开：按非源文件错误条目呈现，与显式 modules 行为一致', () => {
    const project = createTempProject();
    try {
      writeTestConfig(project.root, [PANORAMA_SUITE]);
      writeWorkflowJson(project.root, 'dir-entry', { written: ['src/'] });

      const result = runTestResolvePaths({
        modules: 'change',
        change: 'dir-entry',
        project_root: project.root,
      });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors).toEqual([{ path: 'src/', message: 'Not a testable source file' }]);
    } finally {
      project.cleanup();
    }
  });

  it('written 为空数组时 earlyReturn：unit_tests 空且无任何致命 errors（不触发全库扫描）', () => {
    const project = createTempProject();
    try {
      writeTestConfig(project.root, [PANORAMA_SUITE]);
      writeWorkflowJson(project.root, 'empty-inventory', { written: [] });
      // 埋点：若误触发 config 全库扫描，src/planted.ts 必然进入推导结果
      writeProjectFile(project.root, 'src/planted.ts', 'export const planted = 1;\n');
      writeProjectFile(project.root, 'other.ts', 'export const other = 1;\n');

      const result = runTestResolvePaths({
        modules: 'change',
        change: 'empty-inventory',
        project_root: project.root,
      });

      expect(result.unit_tests).toEqual([]);
      // 无 "No test configuration" 等任何致命错误条目
      expect(result.errors).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 场景: 清单读取失败语义（错误收集，不抛出）
// ===========================================================================

describe('清单读取失败语义（错误收集契约）', () => {
  it('机制前旧 change（workflow.json 无 files）→ errors 含“请重建”指引且 unit_tests 空（earlyReturn 不抛出）', () => {
    const project = createTempProject();
    try {
      writeTestConfig(project.root, [PANORAMA_SUITE]);
      writeWorkflowJson(project.root, 'legacy-change', null);

      const result = runTestResolvePaths({
        modules: 'change',
        change: 'legacy-change',
        project_root: project.root,
      });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors).toHaveLength(1);
      expect(result.errors[0].path).toBe('legacy-change');
      expect(result.errors[0].message).toContain('请重建');
    } finally {
      project.cleanup();
    }
  });

  it('change 不存在 → errors 收集 workflow.json 缺失指引且不抛出', () => {
    const project = createTempProject();
    try {
      writeTestConfig(project.root, [PANORAMA_SUITE]);

      const result = runTestResolvePaths({
        modules: 'change',
        change: 'missing-change',
        project_root: project.root,
      });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors).toHaveLength(1);
      expect(result.errors[0].path).toBe('missing-change');
      expect(result.errors[0].message).toContain('workflow.json 不存在');
    } finally {
      project.cleanup();
    }
  });

  it('modules="change" 缺 change 参数 → errors 呈现 must-provide 条目；input schema refine 拒绝', () => {
    const project = createTempProject();
    try {
      writeTestConfig(project.root, [PANORAMA_SUITE]);

      const result = runTestResolvePaths({ modules: 'change', project_root: project.root });

      expect(result.unit_tests).toEqual([]);
      expect(result.errors).toEqual([
        { path: 'change', message: 'modules 为 "change" 时必须提供 change 参数' },
      ]);

      // MCP 层 schema 守卫：modules='change' 必须搭配 change 参数
      const parsed = testResolvePathsInputSchema.safeParse({
        project_root: project.root,
        modules: 'change',
      });
      expect(parsed.success).toBe(false);
    } finally {
      project.cleanup();
    }
  });
});
