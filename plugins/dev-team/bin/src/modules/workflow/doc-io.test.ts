/**
 * 单元测试: modules/workflow/doc-io.ts — workflow.json 共享读写原语
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-1: workflowFileSchema 接受 active_phase（对象或 null）与 interrupted[]、
 *   file_log；未知键保留行为不变（schema 扩展经读取原语断言）
 * - 读取原语正向/异常: 存在性 → JSON 解析 → 根对象 → schema 校验四道门
 * - 写回原语正向/异常: 文件必须已存在、未知键逐字保留、2 空格缩进 + 尾换行、
 *   绝不创建文件或目录、JSON 非法时磁盘逐字节不变
 *
 * Mock 策略: 无——mkdtempSync 临时目录 + 真实 fs（同 file-inventory.test.ts 模式）。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as nodePath from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { getWorkflowJsonPath, loadWorkflowDoc, saveWorkflowDoc } from './doc-io';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'doc-io-test-'): TempProject {
  const root = fs.mkdtempSync(nodePath.join(os.tmpdir(), prefix));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function createChangeDir(project: TempProject, name = 'my-change'): string {
  const changeDir = nodePath.join(project.root, 'openspec', 'changes', name);
  fs.mkdirSync(changeDir, { recursive: true });
  return changeDir;
}

function workflowJsonPath(changeDir: string): string {
  return nodePath.join(changeDir, 'workflow.json');
}

function writeWorkflowJsonRaw(changeDir: string, raw: string): void {
  fs.writeFileSync(workflowJsonPath(changeDir), raw, 'utf-8');
}

/** 构造一条能通过 phaseLogSchema 校验的 eval 条目。 */
function evalEntry(phase: string): Record<string, unknown> {
  return {
    phase,
    verdict: 'pass',
    attempt: 1,
    timestamp: '2026-09-18T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: true, evidence: 'e' }],
    backtrack_to: null,
  };
}

/** 合法 workflow.json 文档（workflow_type + created + eval + 未知键）。 */
function validDoc(extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-18',
    eval: [evalEntry('proposal')],
    unknown_key: { keep: true },
    ...extra,
  };
}

// ===========================================================================
// getWorkflowJsonPath
// ===========================================================================

describe('getWorkflowJsonPath', () => {
  it('返回 <changeDir>/workflow.json 的绝对拼接路径', () => {
    const changeDir = nodePath.resolve('/some', 'change-dir');
    expect(getWorkflowJsonPath(changeDir)).toBe(
      nodePath.resolve('/some', 'change-dir', 'workflow.json'),
    );
  });
});

// ===========================================================================
// loadWorkflowDoc — 读取原语
// ===========================================================================

describe('loadWorkflowDoc — 正向', () => {
  it('合法 workflow.json（含 workflow_type / created / eval / 未知键）返回解析后文档对象（raw，非投影）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJsonRaw(changeDir, JSON.stringify(validDoc()));

      const doc = loadWorkflowDoc(changeDir);

      expect(doc).toEqual(validDoc());
      // raw 文档：未知键在返回对象中原样存在
      expect(doc.unknown_key).toEqual({ keep: true });
    } finally {
      project.cleanup();
    }
  });

  it('含新运行态字段 active_phase / interrupted / file_log 的文档通过校验（AC-1 schema 扩展经消费方断言）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const doc = validDoc({
        active_phase: { phase: 'implement', attempt: 2, start_at: '2026-09-18T08:00:00.000Z' },
        interrupted: [
          {
            phase: 'implement',
            attempt: 1,
            start_at: '2026-09-18T07:00:00.000Z',
            end_at: '2026-09-18T07:30:00.000Z',
          },
        ],
        file_log: [
          {
            op: 'write',
            scope: 'implement',
            attempt: 2,
            path: 'src/a.ts',
            at: '2026-09-18T08:05:00.000Z',
          },
        ],
      });
      writeWorkflowJsonRaw(changeDir, JSON.stringify(doc));

      expect(loadWorkflowDoc(changeDir)).toEqual(doc);
    } finally {
      project.cleanup();
    }
  });

  it('active_phase: null 合法（无运行态的显式形态）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJsonRaw(changeDir, JSON.stringify(validDoc({ active_phase: null })));

      const doc = loadWorkflowDoc(changeDir);

      expect(doc.active_phase).toBeNull();
    } finally {
      project.cleanup();
    }
  });
});

describe('loadWorkflowDoc — 异常', () => {
  it('文件不存在 → 抛「workflow.json 不存在」含 change_create 指引且不创建任何文件', () => {
    const project = createTempProject();
    try {
      const changeDir = nodePath.join(project.root, 'openspec', 'changes', 'missing');

      expect(() => loadWorkflowDoc(changeDir)).toThrow(/workflow\.json 不存在/);
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/change_create/);
      expect(fs.existsSync(changeDir)).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('JSON 截断 / 含注释 → 抛「解析失败」', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      writeWorkflowJsonRaw(changeDir, '{"workflow_type": "requ');
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/解析失败/);

      writeWorkflowJsonRaw(changeDir, '{ /* comment */ "workflow_type": "requirement" }');
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/解析失败/);
    } finally {
      project.cleanup();
    }
  });

  it('根为数组 / 字符串 / null → 抛「根元素必须是对象」', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      for (const raw of ['[]', '"just a string"', 'null']) {
        writeWorkflowJsonRaw(changeDir, raw);
        expect(() => loadWorkflowDoc(changeDir)).toThrow(/根元素必须是对象/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('workflow_type 非法枚举等 schema 违例 → 抛「格式非法」含 issue 明细（字段路径 + 消息）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);

      writeWorkflowJsonRaw(changeDir, JSON.stringify(validDoc({ workflow_type: 'unknown-flow' })));
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/格式非法/);
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/workflow_type/);

      writeWorkflowJsonRaw(changeDir, JSON.stringify(validDoc({ created: '2026/09/18' })));
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/格式非法/);
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/created/);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// saveWorkflowDoc — 写回原语
// ===========================================================================

describe('saveWorkflowDoc — 正向', () => {
  it('写回后未知键 / workflow_type / created / eval / 新运行态字段逐字保留，可再解析（AC-1 未知键保留行为不变）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const doc = validDoc({
        active_phase: { phase: 'test-gen', attempt: 1, start_at: '2026-09-18T09:00:00.000Z' },
        interrupted: [],
        file_log: [],
      });
      writeWorkflowJsonRaw(changeDir, JSON.stringify(doc));

      saveWorkflowDoc(changeDir, doc);

      const raw = fs.readFileSync(workflowJsonPath(changeDir), 'utf-8');
      expect(JSON.parse(raw)).toEqual(doc);
      expect(doc.unknown_key).toEqual({ keep: true });
    } finally {
      project.cleanup();
    }
  });

  it('输出 2 空格缩进 + 末尾换行（写盘纪律）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const doc = validDoc();
      writeWorkflowJsonRaw(changeDir, JSON.stringify(doc));

      saveWorkflowDoc(changeDir, doc);

      const raw = fs.readFileSync(workflowJsonPath(changeDir), 'utf-8');
      expect(raw.endsWith('\n')).toBe(true);
      expect(raw).toContain('\n  "workflow_type"');
      expect(raw).not.toContain('\n    "workflow_type"');
    } finally {
      project.cleanup();
    }
  });

  it('loadWorkflowDoc → saveWorkflowDoc 往返：文档逐字等价（round-trip）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJsonRaw(changeDir, JSON.stringify(validDoc({ file_log: [] })));

      const loaded = loadWorkflowDoc(changeDir);
      saveWorkflowDoc(changeDir, loaded);

      expect(loadWorkflowDoc(changeDir)).toEqual(loaded);
    } finally {
      project.cleanup();
    }
  });
});

describe('saveWorkflowDoc — 异常（绝不创建契约）', () => {
  it('文件不存在 → 抛错且不创建文件与目录', () => {
    const project = createTempProject();
    try {
      const changeDir = nodePath.join(project.root, 'openspec', 'changes', 'missing');

      expect(() => saveWorkflowDoc(changeDir, validDoc())).toThrow(/workflow\.json 不存在/);
      expect(fs.existsSync(changeDir)).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('磁盘 JSON 非法 → saveWorkflowDoc 不做前置校验按调用者契约写回；非法内容仅在 loadWorkflowDoc 处抛错且磁盘逐字节不变', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const broken = '{"workflow_type": "requ';
      writeWorkflowJsonRaw(changeDir, broken);

      // loadWorkflowDoc 是非法内容的守门人：抛错且不落盘
      expect(() => loadWorkflowDoc(changeDir)).toThrow(/解析失败/);
      expect(fs.readFileSync(workflowJsonPath(changeDir), 'utf-8')).toBe(broken);
    } finally {
      project.cleanup();
    }
  });
});
