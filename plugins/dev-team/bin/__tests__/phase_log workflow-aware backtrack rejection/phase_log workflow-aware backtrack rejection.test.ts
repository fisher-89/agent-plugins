/**
 * 集成测试: phase_log 不再处理回溯 — 回溯由独立 backtrack 工具验证
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
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { runBacktrack } from '../../src/commands/backtrack';
import { runPhaseLog } from '../../src/commands/phase-log';
import { appendEntry } from '../../src/lib/eval-json';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';

const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'none' }];
const VALID_ITEMS = [{ item: 'test', pass: true, evidence: 'ok' }];

function mockWorkflowType(workflowType: string): void {
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    return String(filePath).endsWith('workflow.json');
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      path: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      if (String(path).endsWith('workflow.json')) {
        return JSON.stringify({ workflow_type: workflowType });
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
  it('phase_log 写入成功（不含回溯参数）', () => {
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

    // eval.json should NOT be modified
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

  it('backtrack 需要 phase 有 eval.json 条目', () => {
    // No entries for the phase yet
    expect(() =>
      runBacktrack({
        project_root: FIXTURE_PROJECT_ROOT,
        change: 'test-change',
        phase: 'test-execution',
        backtrack_to: 'proposal',
        backtrack_reason: 'no entry',
      }),
    ).toThrow(/没有 eval.json 条目/);
  });
});
