/**
 * 单元测试: modules/workflow/files-query.ts — workflow_files 只读查询
 *
 * 覆盖范围（openspec/changes/workflow-files-query-api/test-design.md）:
 * - AC-2: 净状态投影 { written, deleted }，结构性排除 source 审计映射；
 *   调用前后 workflow.json 逐字节不变（只读不变量）
 * - AC-3: 四态硬报错（workflow.json 不存在 / JSON 解析失败 / schema 格式非法 /
 *   缺 files 字段）均带重建指引，MUST NOT 回退 git diff
 *
 * 文件系统不 mock：mkdtempSync 临时工程 + 真实读写（同 file-inventory.test.ts /
 * change-files.test.ts 既有模式）；project_root 回退链经
 * vi.stubEnv('CLAUDE_PROJECT_DIR', …) 驱动（同 bin/__tests__/inventory-backtrack-preserve
 * 模式，不用 process.chdir）。
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { getChangedFiles } from './files-query';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'files-query-test-'): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function createChangeDir(project: TempProject, name = 'my-change'): string {
  const changeDir = path.join(project.root, 'openspec', 'changes', name);
  fs.mkdirSync(changeDir, { recursive: true });
  return changeDir;
}

function workflowJsonPath(changeDir: string): string {
  return path.join(changeDir, 'workflow.json');
}

function writeWorkflowJson(changeDir: string, doc: unknown): void {
  fs.writeFileSync(workflowJsonPath(changeDir), JSON.stringify(doc), 'utf-8');
}

function readWorkflowJsonRaw(changeDir: string): string {
  return fs.readFileSync(workflowJsonPath(changeDir), 'utf-8');
}

/** 快照 change 目录内的全部文件（相对路径 + 原始字节文本），用于只读不变量断言。 */
function snapshotChangeDir(changeDir: string): Map<string, string> {
  const snapshot = new Map<string, string>();
  const walk = (dir: string, prefix: string): void => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const rel = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
      if (entry.isDirectory()) {
        walk(path.join(dir, entry.name), rel);
      } else {
        snapshot.set(rel, fs.readFileSync(path.join(dir, entry.name), 'utf-8'));
      }
    }
  };
  walk(changeDir, '');
  return snapshot;
}

/** 合法 workflow.json 文档（workflow_type + created + files；eval 可选省略）。 */
function validWorkflowDoc(
  files: unknown,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-17',
    ...extra,
    files,
  };
}

function expectRunError(change: string, projectRoot: string): Error {
  let caught: unknown;
  try {
    getChangedFiles(change, projectRoot);
  } catch (e: unknown) {
    caught = e;
  }
  expect(caught).toBeInstanceOf(Error);
  return caught as Error;
}

// ===========================================================================
// getChangedFiles — 净状态投影
// ===========================================================================

describe('getChangedFiles — 净状态投影', () => {
  it('files 含 written/deleted 双数组与 source 审计映射 → 返回 { written, deleted }，输出对象无 source 键（Object.hasOwn 为 false，JSON.stringify 不含 "source"）(AC-2)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({
          written: ['src/a.ts', 'src/b.ts'],
          deleted: ['src/old.ts'],
          source: { 'src/a.ts': 'dev-team:implementation-generator' },
        }),
      );

      const result = getChangedFiles('my-change', project.root);

      expect(result).toEqual({
        written: ['src/a.ts', 'src/b.ts'],
        deleted: ['src/old.ts'],
      });
      expect(Object.prototype.hasOwnProperty.call(result, 'source')).toBe(false);
      expect(JSON.stringify(result)).not.toContain('"source"');
    } finally {
      project.cleanup();
    }
  });

  it('显式传入 project_root → 从该根定位 openspec/changes/<change>/workflow.json 并返回其净状态 (AC-2)', () => {
    const projectA = createTempProject();
    const projectB = createTempProject();
    try {
      const changeDirA = createChangeDir(projectA, 'change-a');
      writeWorkflowJson(changeDirA, validWorkflowDoc({ written: ['from-a.ts'], deleted: [] }));
      const changeDirB = createChangeDir(projectB, 'change-a');
      writeWorkflowJson(changeDirB, validWorkflowDoc({ written: [], deleted: ['from-b.ts'] }));

      const result = getChangedFiles('change-a', projectB.root);

      expect(result).toEqual({ written: [], deleted: ['from-b.ts'] });
    } finally {
      projectA.cleanup();
      projectB.cleanup();
    }
  });

  it('空净状态 { written: [], deleted: [] }（change_create 初始形态）→ 返回双空数组，不补建字段', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: [], deleted: [] }));

      const result = getChangedFiles('my-change', project.root);

      expect(result).toEqual({ written: [], deleted: [] });
      expect(Object.keys(result).sort()).toEqual(['deleted', 'written']);
    } finally {
      project.cleanup();
    }
  });

  it('written 含空串 / 超长路径（>1000 chars）/ 空格、中文、emoji 路径 → 原样透传不做语义裁剪', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const longPath = `src/${'x'.repeat(1001)}.ts`;
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({
          written: ['', longPath, 'src/含 空格.ts', 'src/中文路径.ts', 'src/emoji-🧪.ts'],
          deleted: [],
        }),
      );

      const result = getChangedFiles('my-change', project.root);

      expect(result.written).toEqual([
        '',
        longPath,
        'src/含 空格.ts',
        'src/中文路径.ts',
        'src/emoji-🧪.ts',
      ]);
    } finally {
      project.cleanup();
    }
  });

  it('成功查询前后 workflow.json 原始文本逐字节一致（含缩进与尾换行），且 change 目录内无新增/修改文件 (AC-2)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const pretty = `${JSON.stringify(
        validWorkflowDoc({
          written: ['src/a.ts'],
          deleted: ['src/gone.ts'],
          source: { 'src/a.ts': 'dev-team:implementation-generator' },
        }),
        null,
        2,
      )}\n`;
      fs.writeFileSync(workflowJsonPath(changeDir), pretty, 'utf-8');

      const before = snapshotChangeDir(changeDir);
      expect(before.get('workflow.json')).toBe(pretty);

      getChangedFiles('my-change', project.root);

      expect(snapshotChangeDir(changeDir)).toEqual(before);
      expect(readWorkflowJsonRaw(changeDir)).toBe(pretty);
    } finally {
      project.cleanup();
    }
  });

  it('files 含非空 source 时查询后盘上 files.source 原样保留（只读投影不改盘）(AC-2)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const sourceMap = {
        'src/a.ts': 'dev-team:implementation-generator',
        'src/b.ts': 'dev-team:test-gen-generator',
      };
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({ written: ['src/a.ts', 'src/b.ts'], deleted: [], source: sourceMap }),
      );

      getChangedFiles('my-change', project.root);

      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as {
        files: { source?: Record<string, string> };
      };
      expect(doc.files.source).toEqual(sourceMap);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// getChangedFiles — project_root 解析
// ===========================================================================

describe('getChangedFiles — project_root 解析', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('project_root 指向无 workflow.json 的目录 → 抛「workflow.json 不存在」态①错误，路径指向该根下', () => {
    const project = createTempProject();
    try {
      const err = expectRunError('my-change', project.root);

      expect(err.message).toMatch(/workflow\.json 不存在/);
      expect(err.message).toContain(
        path.join(project.root, 'openspec', 'changes', 'my-change', 'workflow.json'),
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// getChangedFiles — 硬报错四态
// ===========================================================================

describe('getChangedFiles — 硬报错四态', () => {
  it('态①：change 目录 / workflow.json 不存在 → 抛错含「workflow.json 不存在」与 change_create 指引 (AC-3)', () => {
    const project = createTempProject();
    try {
      // change 目录整体缺失
      const missingDir = expectRunError('ghost-change', project.root);
      expect(missingDir.message).toMatch(/workflow\.json 不存在/);
      expect(missingDir.message).toMatch(/change_create/);

      // change 目录存在但无 workflow.json
      createChangeDir(project, 'empty-change');
      const emptyDir = expectRunError('empty-change', project.root);
      expect(emptyDir.message).toMatch(/workflow\.json 不存在/);
      expect(emptyDir.message).toMatch(/change_create/);
    } finally {
      project.cleanup();
    }
  });

  it('态②：JSON 截断 / 含注释 → 抛「解析失败」；根为数组 / 字符串 / null → 抛「根元素必须是对象」(AC-3)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      fs.writeFileSync(workflowJsonPath(changeDir), '{"workflow_type": "requ', 'utf-8');
      expect(expectRunError('my-change', project.root).message).toMatch(/解析失败/);

      fs.writeFileSync(
        workflowJsonPath(changeDir),
        '{ /* comment */ "workflow_type": "requirement" }',
        'utf-8',
      );
      expect(expectRunError('my-change', project.root).message).toMatch(/解析失败/);

      for (const raw of ['[]', '"just a string"', 'null']) {
        fs.writeFileSync(workflowJsonPath(changeDir), raw, 'utf-8');
        expect(expectRunError('my-change', project.root).message).toMatch(/根元素必须是对象/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('态③：files.written 非数组 / workflow_type 非法枚举 → 抛「格式非法」且含 issue 明细 (AC-3)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      writeWorkflowJson(changeDir, validWorkflowDoc({ written: 'src/a.ts', deleted: [] }));
      const filesIssue = expectRunError('my-change', project.root);
      expect(filesIssue.message).toMatch(/格式非法/);
      expect(filesIssue.message).toContain('written');

      writeWorkflowJson(
        changeDir,
        validWorkflowDoc({ written: [], deleted: [] }, { workflow_type: 'unknown-flow' }),
      );
      const typeIssue = expectRunError('my-change', project.root);
      expect(typeIssue.message).toMatch(/格式非法/);
      expect(typeIssue.message).toContain('workflow_type');
    } finally {
      project.cleanup();
    }
  });

  it('态④：合法 workflow.json 缺 files 字段 → 抛错文案含「该 change 创建于文件清单机制之前，请重建」(AC-3)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, { workflow_type: 'requirement', created: '2026-01-01' });

      expect(expectRunError('my-change', project.root).message).toMatch(
        /该 change 创建于文件清单机制之前，请重建/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('四态错误路径下 workflow.json 与 change 目录逐字节不变、不创建任何文件（硬报错不落盘）(AC-3)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      // 态①：workflow.json 不存在 — 快照应保持无该文件
      let before = snapshotChangeDir(changeDir);
      expectRunError('my-change', project.root);
      expect(snapshotChangeDir(changeDir)).toEqual(before);

      // 态②：JSON 截断
      const broken = '{"workflow_type": "requ';
      fs.writeFileSync(workflowJsonPath(changeDir), broken, 'utf-8');
      before = snapshotChangeDir(changeDir);
      expectRunError('my-change', project.root);
      expect(snapshotChangeDir(changeDir)).toEqual(before);

      // 态③：files.written 非数组
      writeWorkflowJson(changeDir, validWorkflowDoc({ written: 42, deleted: [] }));
      before = snapshotChangeDir(changeDir);
      expectRunError('my-change', project.root);
      expect(snapshotChangeDir(changeDir)).toEqual(before);

      // 态④：缺 files 字段
      writeWorkflowJson(changeDir, { workflow_type: 'requirement', created: '2026-01-01' });
      before = snapshotChangeDir(changeDir);
      expectRunError('my-change', project.root);
      expect(snapshotChangeDir(changeDir)).toEqual(before);
    } finally {
      project.cleanup();
    }
  });

  it('任一四态错误均以抛错结束（调用方拿到 Error），不存在返回降级结果或 git diff 内容的回退路径（错误文案不含 diff 输出）(AC-3)', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      const states: Array<() => void> = [
        // 态①：文件不存在
        () => {
          fs.rmSync(workflowJsonPath(changeDir), { force: true });
        },
        // 态②：JSON 截断
        () => {
          fs.writeFileSync(workflowJsonPath(changeDir), '{"workflow_type": "requ', 'utf-8');
        },
        // 态③：files.written 非数组
        () => {
          writeWorkflowJson(changeDir, validWorkflowDoc({ written: 'src/a.ts', deleted: [] }));
        },
        // 态④：缺 files 字段
        () => {
          writeWorkflowJson(changeDir, { workflow_type: 'requirement' });
        },
      ];

      for (const setup of states) {
        setup();
        let result: unknown;
        let threw = false;
        try {
          result = getChangedFiles('my-change', project.root);
        } catch (e: unknown) {
          threw = true;
          expect(e).toBeInstanceOf(Error);
          expect((e as Error).message).not.toMatch(/git diff|diff --git/i);
        }
        expect(threw).toBe(true);
        expect(result).toBeUndefined();
      }
    } finally {
      project.cleanup();
    }
  });
});
