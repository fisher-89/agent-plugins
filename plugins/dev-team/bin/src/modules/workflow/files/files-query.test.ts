/**
 * 单元测试: modules/workflow/files/files-query.ts — workflow_files 只读查询
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-7: getChangedFiles 为 barrel 唯一公共读通道，内部经模块内 readNetState 接线——
 *   file_log 混合 phase/workflow 条目（同 path 后条胜、revert 净消除）→ 返回派生
 *   { written, deleted }，输出对象无 scope/attempt 等审计明细键
 * - `file_log: []` → 双空数组，不补建字段
 * - 缺失 file_log（legacy files-only change）→ 硬报错含「重建 change」指引，
 *   错误文案不含 diff 输出（不回退 git）
 * - 四态错误（文件缺失 / JSON 非法 / 根非对象 / 格式非法）语义沿用，错误路径下
 *   磁盘逐字节不变（迁移保留）
 * - 只读回归（迁移保留）: 查询前后 workflow.json 逐字节一致、change 目录无新增文件；
 *   特殊路径（空串/超长/中文/emoji）透传
 *
 * 文件系统不 mock：mkdtempSync 临时工程 + 真实读写（同 file-inventory.test.ts 既有模式）。
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

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

/** 构造一条 file_log 条目（scope 缺省为 workflow 命名空间）。 */
function logEntry(
  op: 'write' | 'delete' | 'revert',
  target: string,
  scope: string = 'workflow',
  attempt?: number,
): Record<string, unknown> {
  return attempt === undefined
    ? { op, scope, path: target, at: '2026-09-18T00:00:00.000Z' }
    : { op, scope, attempt, path: target, at: '2026-09-18T00:00:00.000Z' };
}

/** 合法 workflow.json 文档（workflow_type + created + file_log；eval 可选省略）。 */
function validWorkflowDoc(
  fileLog: unknown,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-17',
    ...extra,
    file_log: fileLog,
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
// getChangedFiles — 派生净状态投影 (AC-7)
// ===========================================================================

describe('getChangedFiles — 派生净状态投影 (AC-7)', () => {
  it('file_log 混合 phase/workflow 条目：同 path 后条胜、revert 净消除 → 返回派生 { written, deleted }', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([
          logEntry('write', 'src/a.ts', 'implement', 1),
          logEntry('write', 'src/kept.ts'),
          logEntry('delete', 'src/old.ts'),
          logEntry('write', 'src/gone.ts'),
          logEntry('revert', 'src/gone.ts', 'implement', 1),
        ]),
      );

      const result = getChangedFiles('my-change', project.root);

      expect(result).toEqual({ written: ['src/a.ts', 'src/kept.ts'], deleted: ['src/old.ts'] });
    } finally {
      project.cleanup();
    }
  });

  it('输出对象无 scope / attempt 等审计明细键（Object.hasOwn 为 false，序列化不含审计键）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([logEntry('write', 'src/a.ts', 'implement', 2)]),
      );

      const result = getChangedFiles('my-change', project.root);

      expect(Object.prototype.hasOwnProperty.call(result, 'scope')).toBe(false);
      expect(Object.prototype.hasOwnProperty.call(result, 'attempt')).toBe(false);
      expect(Object.keys(result).sort()).toEqual(['deleted', 'written']);
      expect(JSON.stringify(result)).not.toContain('"scope"');
      expect(JSON.stringify(result)).not.toContain('"attempt"');
    } finally {
      project.cleanup();
    }
  });

  it('file_log: []（change_create 初始形态）→ 双空数组，不补建字段', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validWorkflowDoc([]));

      const result = getChangedFiles('my-change', project.root);

      expect(result).toEqual({ written: [], deleted: [] });
      expect(Object.keys(result).sort()).toEqual(['deleted', 'written']);
    } finally {
      project.cleanup();
    }
  });

  it('显式传入 project_root → 从该根定位 openspec/changes/<change>/workflow.json 并返回其派生净状态', () => {
    const projectA = createTempProject();
    const projectB = createTempProject();
    try {
      const changeDirA = createChangeDir(projectA, 'change-a');
      writeWorkflowJson(changeDirA, validWorkflowDoc([logEntry('write', 'from-a.ts')]));
      const changeDirB = createChangeDir(projectB, 'change-a');
      writeWorkflowJson(changeDirB, validWorkflowDoc([logEntry('delete', 'from-b.ts')]));

      const result = getChangedFiles('change-a', projectB.root);

      expect(result).toEqual({ written: [], deleted: ['from-b.ts'] });
    } finally {
      projectA.cleanup();
      projectB.cleanup();
    }
  });

  it('特殊路径（空串 / 超长 >1000 chars / 空格 / 中文 / emoji）→ 原样透传不做语义裁剪（迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const longPath = `src/${'x'.repeat(1001)}.ts`;
      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([
          logEntry('write', ''),
          logEntry('write', longPath),
          logEntry('write', 'src/含 空格.ts'),
          logEntry('write', 'src/中文路径.ts'),
          logEntry('write', 'src/emoji-🧪.ts'),
        ]),
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

  it('成功查询前后 workflow.json 原始文本逐字节一致，change 目录内无新增/修改文件（只读不变量，迁移保留）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const pretty = `${JSON.stringify(
        validWorkflowDoc([
          logEntry('write', 'src/a.ts', 'implement', 1),
          logEntry('delete', 'src/gone.ts'),
        ]),
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
});

// ===========================================================================
// getChangedFiles — project_root 解析
// ===========================================================================

describe('getChangedFiles — project_root 解析', () => {
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
// getChangedFiles — 硬报错四态（迁移保留）
// ===========================================================================

describe('getChangedFiles — 硬报错四态', () => {
  it('态①：change 目录 / workflow.json 不存在 → 抛错含「workflow.json 不存在」与 change_create 指引', () => {
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

  it('态②：JSON 截断 / 含注释 → 抛「解析失败」；根为数组 / 字符串 / null → 抛「根元素必须是对象」', () => {
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

  it('态③：file_log 条目缺字段 / workflow_type 非法枚举 → 抛「格式非法」且含 issue 明细', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      writeWorkflowJson(
        changeDir,
        validWorkflowDoc([{ op: 'write', scope: 'workflow', path: 'src/a.ts' }]),
      );
      const logIssue = expectRunError('my-change', project.root);
      expect(logIssue.message).toMatch(/格式非法/);
      expect(logIssue.message).toContain('at');

      writeWorkflowJson(changeDir, validWorkflowDoc([], { workflow_type: 'unknown-flow' }));
      const typeIssue = expectRunError('my-change', project.root);
      expect(typeIssue.message).toMatch(/格式非法/);
      expect(typeIssue.message).toContain('workflow_type');
    } finally {
      project.cleanup();
    }
  });

  it('态④：合法 workflow.json 缺 file_log 字段（legacy files-only）→ 抛错含「重建 change」指引，错误文案不含 diff 输出（AC-7/AC-12）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, {
        workflow_type: 'requirement',
        created: '2026-01-01',
        files: { written: ['src/legacy.ts'], deleted: [] },
      });

      const err = expectRunError('my-change', project.root);
      expect(err.message).toMatch(/该 change 创建于文件清单机制之前，请重建/);
      expect(err.message).not.toMatch(/git diff|diff --git/i);
    } finally {
      project.cleanup();
    }
  });

  it('四态错误路径下 workflow.json 与 change 目录逐字节不变、不创建任何文件（硬报错不落盘）', () => {
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

      // 态③：file_log 条目非法
      writeWorkflowJson(changeDir, validWorkflowDoc([{ op: 'copy' }]));
      before = snapshotChangeDir(changeDir);
      expectRunError('my-change', project.root);
      expect(snapshotChangeDir(changeDir)).toEqual(before);

      // 态④：缺 file_log 字段
      writeWorkflowJson(changeDir, { workflow_type: 'requirement', created: '2026-01-01' });
      before = snapshotChangeDir(changeDir);
      expectRunError('my-change', project.root);
      expect(snapshotChangeDir(changeDir)).toEqual(before);
    } finally {
      project.cleanup();
    }
  });

  it('任一四态错误均以抛错结束（调用方拿到 Error），不存在返回降级结果或 git diff 内容的回退路径（错误文案不含 diff 输出）', () => {
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
        // 态③：file_log 条目非法
        () => {
          writeWorkflowJson(changeDir, validWorkflowDoc([{ op: 'copy' }]));
        },
        // 态④：缺 file_log 字段
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
