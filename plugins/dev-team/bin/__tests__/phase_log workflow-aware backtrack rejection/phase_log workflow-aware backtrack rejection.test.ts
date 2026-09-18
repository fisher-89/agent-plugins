/**
 * 集成测试: phase_log 不再处理回溯 — 回溯由独立 backtrack 工具验证
 *
 * 评估历史存放在 `workflow.json.eval`：两个命令都经 `eval-json` 的读写入口
 * 访问存储，夹具不再假设独立的 `eval.json` 文件。
 *
 * @see openspec/changes/refactor-backtrack-to-skill/design.md — backtrack tool
 */

import * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

const mockEntries: Array<Record<string, unknown>> = [];

vi.mock('../../src/lib/eval-json', async () => {
  const actual = await vi.importActual('../../src/lib/eval-json');
  return {
    ...actual,
    markPhaseStale: vi.fn(),
    readEvalJson: vi.fn(() => [...mockEntries]),
    appendEntry: vi.fn((_dir: string, entry: Record<string, unknown>) => {
      mockEntries.push(entry);
    }),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../../src/lib/change', () => ({
  resolveChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { runBacktrack } from '../../src/commands/backtrack';
import { runPhaseLog } from '../../src/commands/phase-log';
import { appendEntry } from '../../src/lib/eval-json';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';

const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'none' }];
const VALID_ITEMS = [{ item: 'test', pass: true, evidence: 'ok' }];

/** workflow.json 提供 workflow_type；`missing` 时模拟未经 change_create 的目录。 */
function mockWorkflowType(workflowType: string, missing = false): void {
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    if (missing) return false;
    return String(filePath).endsWith('workflow.json');
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      path: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      if (String(path).endsWith('workflow.json')) {
        return JSON.stringify({ workflow_type: workflowType, created: '2026-09-11' });
      }
      return '';
    },
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockEntries.length = 0;
  mockWorkflowType('test-only');
});

describe('phase_log — 不再处理回溯，回溯由独立 backtrack 工具负责', () => {
  it('phase_log 写入成功且 entry 不含回溯字段（AC-1）', () => {
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      report: 'bugs found',
      checklist: FAILED_ITEMS,
    });

    expect(mockEntries).toHaveLength(1);
    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(mockEntries[0]).toMatchObject({
      phase: 'test-execution',
      verdict: 'fail',
    });
    // phase_log 不写回溯字段 — 回溯状态只由 backtrack 工具改写
    expect(mockEntries[0]).not.toHaveProperty('backtrack_to');
    expect(mockEntries[0]).not.toHaveProperty('backtrack_reason');
  });

  it('backtrack 工具验证 phase 存在性', () => {
    // First, add a pass entry for the phase we want to backtrack
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-execution',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    // backtrack to a non-existent phase should throw
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'implement',
        backtrack_reason: 'test reason',
      }),
    ).toThrow(/工作流 "test-only" 不包含 phase "implement"/);

    // 存储未被改写
    expect(mockEntries).toHaveLength(1);
  });

  it('backtrack 工具验证目标 phase 在 phase 之前（不能回溯到未来 phase）', () => {
    // Add entries for earlier phases
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
    });

    // Try backtracking proposal -> test-execution (future phase)
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'proposal',
        backtrack_to: 'test-execution',
        backtrack_reason: 'future phase',
      }),
    ).toThrow(/无效的回溯目标/);
  });

  it('backtrack 需要该 phase 有评估条目，message 含「没有评估条目」且不含 eval.json（AC-4）', () => {
    // No entries for the phase yet
    let captured: Error | null = null;
    try {
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'proposal',
        backtrack_reason: 'no entry',
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('没有评估条目');
    expect(captured!.message).not.toContain('eval.json');
    // 旧正则不再匹配（文案已更新）
    expect(captured!.message).not.toMatch(/没有 eval\.json 条目/);
  });

  it('workflow.json 缺失时 backtrack 先因类型读取抛错（早于「无条目」判定）（AC-13）', () => {
    mockWorkflowType('test-only', true);

    let captured: Error | null = null;
    try {
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'proposal',
        backtrack_reason: 'missing workflow.json',
      });
    } catch (e: unknown) {
      captured = e as Error;
    }

    expect(captured).not.toBeNull();
    expect(captured!.message).toContain('workflow.json 不存在');
    expect(captured!.message).toContain('change_create');
    expect(captured!.message).not.toContain('没有评估条目');
  });
});
