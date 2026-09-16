/**
 * Tests for change-config.ts — strict `workflow.json` parsing.
 *
 * `workflow.json` is created by `change_create` (its only creator) and is the
 * precondition of `phase_next` / `backtrack` / `phase_log`: a missing or
 * malformed file throws — there is no `DEFAULT_WORKFLOW_TYPE` fallback.
 */

import * as fs from 'fs';
import * as path from 'path';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

import type * as ChangeModule from './change';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

// Resolve the change dir the same way the real helper does, so the fixed
// absolute path asserted in the error messages stays meaningful.
vi.mock('./change', async (importOriginal) => {
  const actual = await importOriginal<typeof ChangeModule>();
  return {
    ...actual,
    getChangeDir: vi.fn((changeName: string) =>
      path.resolve('/tmp/test-root', 'openspec', 'changes', changeName),
    ),
  };
});

import { getWorkflowType } from './change-config';

const WORKFLOW_PATH = path.resolve(
  '/tmp/test-root',
  'openspec',
  'changes',
  'my-change/workflow.json',
);

/** 一条符合 `phaseLogSchema` 的合法评估条目。 */
const LEGAL_ENTRY = {
  phase: 'proposal',
  verdict: 'pass',
  attempt: 1,
  timestamp: '2026-09-11T10:00:00.000Z',
  report: 'ok',
  checklist: [],
  backtrack_to: null,
};

/**
 * Point the fs mock at `content` for `<change>/workflow.json`.
 * `null` means "the file does not exist".
 */
function mockWorkflowJson(content: string | null, changeName: string = 'my-change'): void {
  const workflowPath = path.resolve(
    '/tmp/test-root',
    'openspec',
    'changes',
    changeName,
    'workflow.json',
  );
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    return String(filePath) === workflowPath && content !== null;
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      filePath: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      if (String(filePath) === workflowPath) {
        return content ?? '';
      }
      return '';
    },
  );
}

function workflowJson(value: unknown): string {
  return JSON.stringify(value);
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// 正向 — 合法文件
// ---------------------------------------------------------------------------

describe('getWorkflowType — 正向（合法 workflow.json）', () => {
  it('含 workflow_type / created / eval 数组时返回 workflow_type，不因 eval 抛错', () => {
    mockWorkflowJson(
      workflowJson({ workflow_type: 'test-only', created: '2026-09-11', eval: [LEGAL_ENTRY] }),
    );
    expect(getWorkflowType('my-change')).toBe('test-only');
  });

  it('含 workflow_type 与空 eval 数组时返回 workflow_type', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'requirement', eval: [] }));
    expect(getWorkflowType('my-change')).toBe('requirement');
  });

  it('含未知键 note 时忽略未知键并返回 workflow_type', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'bug-fix', note: 'x' }));
    expect(getWorkflowType('my-change')).toBe('bug-fix');
  });

  it('4 值枚举逐一原样返回', () => {
    for (const workflowType of ['requirement', 'bug-fix', 'refactor', 'test-only']) {
      mockWorkflowJson(workflowJson({ workflow_type: workflowType, created: '2026-09-11' }));
      expect(getWorkflowType('my-change')).toBe(workflowType);
    }
  });

  it('created 缺失（Optional None）时正常返回 workflow_type', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'refactor' }));
    expect(getWorkflowType('my-change')).toBe('refactor');
  });
});

// ---------------------------------------------------------------------------
// 异常 — 缺文件 / 非法内容
// ---------------------------------------------------------------------------

describe('getWorkflowType — 异常（严格失败，无缺省兜底）', () => {
  it('workflow.json 不存在时抛错，message 含绝对路径与 change_create 指引，不返回 "requirement"', () => {
    mockWorkflowJson(null);
    let captured: Error | null = null;
    try {
      getWorkflowType('my-change');
    } catch (e: unknown) {
      captured = e as Error;
    }
    expect(captured).not.toBeNull();
    const message = captured!.message;
    expect(message).toContain('workflow.json 不存在');
    expect(message).toContain(WORKFLOW_PATH);
    expect(message).toContain('change_create');
  });

  it('非法 JSON 时抛出 workflow.json 解析失败', () => {
    mockWorkflowJson('{invalid json');
    expect(() => getWorkflowType('my-change')).toThrow(/workflow\.json 解析失败/);
  });

  it('根为数组（即便元素像 eval 条目）时抛出根元素类型错误', () => {
    mockWorkflowJson(workflowJson([LEGAL_ENTRY]));
    expect(() => getWorkflowType('my-change')).toThrow(/根元素必须是对象/);
  });

  it('根为 null 时抛出根元素类型错误', () => {
    mockWorkflowJson('null');
    expect(() => getWorkflowType('my-change')).toThrow(/根元素必须是对象/);
  });

  it('根为字符串 / 数字标量时抛出根元素类型错误', () => {
    mockWorkflowJson(workflowJson('requirement'));
    expect(() => getWorkflowType('my-change')).toThrow(/根元素必须是对象/);
    mockWorkflowJson(workflowJson(123));
    expect(() => getWorkflowType('my-change')).toThrow(/根元素必须是对象/);
  });

  it('缺 workflow_type 时抛格式非法，message 含字段路径 workflow_type 与绝对路径', () => {
    mockWorkflowJson(workflowJson({}));
    let captured: Error | null = null;
    try {
      getWorkflowType('my-change');
    } catch (e: unknown) {
      captured = e as Error;
    }
    expect(captured).not.toBeNull();
    const message = captured!.message;
    expect(message).toContain('格式非法');
    expect(message).toContain('workflow_type');
    expect(message).toContain(WORKFLOW_PATH);
  });

  it('workflow_type 为非枚举值 "unknown" 时抛格式非法', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'unknown' }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('workflow_type 为空串时抛格式非法', () => {
    mockWorkflowJson(workflowJson({ workflow_type: '' }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('workflow_type 为非字符串 123 时抛格式非法', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 123 }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('workflow_type 超长（>1000 chars）时因非枚举而抛格式非法', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'x'.repeat(1001) }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('created 非 YYYY-MM-DD（2026/09/11）时抛格式非法', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'requirement', created: '2026/09/11' }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('eval 为对象（非数组）时抛格式非法，不返回 workflow_type', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'requirement', eval: {} }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('eval 数组元素不符合 phaseLogSchema 时抛格式非法', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'requirement', eval: [{ phase: 'nope' }] }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });
});

// ---------------------------------------------------------------------------
// 边界
// ---------------------------------------------------------------------------

describe('getWorkflowType — 边界', () => {
  it('缺 workflow_type 但有 eval 数组时仍抛错（不再回退缺省类型）', () => {
    mockWorkflowJson(workflowJson({ eval: [LEGAL_ENTRY] }));
    expect(() => getWorkflowType('my-change')).toThrow(/格式非法/);
  });

  it('change 为 undefined / null 时抛错（不静默读取其他 change）', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'requirement' }));
    expect(() => getWorkflowType(undefined as unknown as string)).toThrow(TypeError);
    expect(() => getWorkflowType(null as unknown as string)).toThrow(TypeError);
  });

  it('change 为空串时按拼接路径查文件，文件不存在则抛缺文件错误', () => {
    mockWorkflowJson(null, '');
    expect(() => getWorkflowType('')).toThrow(/workflow\.json 不存在/);
  });

  it('change 超长（>1000 chars）时不崩溃，缺文件则抛缺文件错误', () => {
    mockWorkflowJson(null, 'a'.repeat(1001));
    expect(() => getWorkflowType('a'.repeat(1001))).toThrow(/workflow\.json 不存在/);
  });

  it('change 含 \\n / emoji 等特殊字符时不崩溃，缺文件则抛缺文件错误', () => {
    mockWorkflowJson(null, 'a\nb-🧪');
    expect(() => getWorkflowType('a\nb-🧪')).toThrow(/workflow\.json 不存在/);
  });

  it('change 含 emoji 且文件存在时正常返回 workflow_type', () => {
    mockWorkflowJson(workflowJson({ workflow_type: 'test-only' }), 'chg-🧪');
    expect(getWorkflowType('chg-🧪')).toBe('test-only');
  });
});
