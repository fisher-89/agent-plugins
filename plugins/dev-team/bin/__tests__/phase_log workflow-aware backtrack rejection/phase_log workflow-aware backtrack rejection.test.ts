/**
 * 集成测试: phase_log 无效 backtrack 不污染 eval.json 后 fail 写入
 *
 * @see openspec/changes/workflow-test-only/test-design.md — AC-16
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

import { runPhaseLog } from '../../src/commands/phase-log';
import { appendEntry } from '../../src/lib/eval-json';

const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'none' }];

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

describe('phase_log — invalid backtrack 不污染 eval.json 后 fail 写入 (AC-16)', () => {
  it('第一次 invalid backtrack 抛错且 eval 未变；第二次 fail+null 成功 append', () => {
    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: '06-unit-test',
        report: 'bugs found',
        checklist: FAILED_ITEMS,
        backtrack_to: '05-implement',
      }),
    ).toThrow(/工作流 test-only 不包含 phase '05-implement'/);

    expect(mockEntries).toHaveLength(0);
    expect(appendEntry).not.toHaveBeenCalled();

    runPhaseLog({
      change: 'test-change',
      phase: '06-unit-test',
      report: 'bugs found, adaptive retry',
      checklist: FAILED_ITEMS,
      backtrack_to: null,
    });

    expect(mockEntries).toHaveLength(1);
    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(mockEntries[0]).toMatchObject({
      phase: '06-unit-test',
      verdict: 'fail',
      backtrack_to: null,
    });
  });
});
