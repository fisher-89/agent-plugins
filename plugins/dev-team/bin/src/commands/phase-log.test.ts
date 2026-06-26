/**
 * Tests for phase-log.ts — workflow-aware backtrack validation.
 */

import * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

vi.mock('../lib/eval-json', async () => {
  const actual = await vi.importActual('../lib/eval-json');
  return {
    ...actual,
    markPhaseStale: vi.fn(),
    readEvalJson: vi.fn(() => []),
    appendEntry: vi.fn(),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { markPhaseStale, appendEntry, writeEvalJson } from '../lib/eval-json';
import { runPhaseLog } from './phase-log';
const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'ok' }];
const VALID_ITEMS = [{ item: 'test', pass: true, evidence: 'ok' }];

function mockWorkflowType(workflowType?: string): void {
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    const p = String(filePath);
    if (p.endsWith('workflow.json')) {
      return workflowType !== undefined;
    }
    return false;
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      path: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      const p = String(path);
      if (p.endsWith('workflow.json')) {
        return JSON.stringify({ workflow_type: workflowType ?? 'requirement' });
      }
      return '';
    },
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockWorkflowType(undefined);
});

describe('runPhaseLog — workflow-aware backtrack rejection', () => {
  it('rejects invalid backtrack for test-only workflow (AC-11)', () => {
    mockWorkflowType('test-only');

    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: 'unit-test',
        report: 'bugs found',
        checklist: FAILED_ITEMS,
        backtrack_to: 'implement',
      }),
    ).toThrow(/工作流 test-only 不包含 phase 'implement'/);

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).not.toHaveBeenCalled();
    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('rejection error lists available phases (AC-12)', () => {
    mockWorkflowType('test-only');

    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: 'unit-test',
        report: 'bugs found',
        checklist: FAILED_ITEMS,
        backtrack_to: 'implement',
      }),
    ).toThrow(/工作流 test-only 不包含 phase 'implement'/);
  });

  it('uses requirement table when workflow.json absent (AC-13)', () => {
    mockWorkflowType(undefined);

    runPhaseLog({
      change: 'test-change',
      phase: 'unit-test',
      report: 'test issue',
      checklist: FAILED_ITEMS,
      backtrack_to: 'dev-design',
    });

    expect(markPhaseStale).toHaveBeenCalled();
    expect(vi.mocked(markPhaseStale).mock.calls[0][1]).toBe('dev-design');
  });

  it('rejects backtrack_to dev-design for test-only workflow (AC-11)', () => {
    mockWorkflowType('test-only');

    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: 'unit-test',
        report: 'bugs found',
        checklist: FAILED_ITEMS,
        backtrack_to: 'dev-design',
      }),
    ).toThrow(/工作流 test-only 不包含 phase 'dev-design'/);

    expect(appendEntry).not.toHaveBeenCalled();
    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('allows backtrack_to implement when workflow.json missing — requirement default (AC-13)', () => {
    mockWorkflowType(undefined);

    runPhaseLog({
      change: 'test-change',
      phase: 'unit-test',
      report: 'test issue',
      checklist: FAILED_ITEMS,
      backtrack_to: 'implement',
    });

    expect(markPhaseStale).toHaveBeenCalled();
  });
});

describe('runPhaseLog — adaptive fail after invalid backtrack', () => {
  it('invalid backtrack then fail+null succeeds (AC-16)', () => {
    mockWorkflowType('test-only');

    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: 'unit-test',
        report: 'bugs found',
        checklist: FAILED_ITEMS,
        backtrack_to: 'implement',
      }),
    ).toThrow();

    runPhaseLog({
      change: 'test-change',
      phase: 'unit-test',
      report: 'bugs found, no backtrack',
      checklist: FAILED_ITEMS,
      backtrack_to: null,
    });

    expect(appendEntry).toHaveBeenCalledTimes(1);
  });
});

describe('runPhaseLog — pass entry no propagation', () => {
  it('pass entry does not trigger markPhaseStale', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).toHaveBeenCalled();
  });
});

describe('runPhaseLog — backtrack_to triggers markPhaseStale', () => {
  it('test-only valid backtrack test-design → proposal triggers markPhaseStale', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: 'test-design',
      report: 'needs redo',
      checklist: FAILED_ITEMS,
      backtrack_to: 'proposal',
    });

    expect(markPhaseStale).toHaveBeenCalledWith(expect.anything(), 'proposal');
    expect(writeEvalJson).toHaveBeenCalled();
  });
});

describe('runPhaseLog — input validation', () => {
  it('null backtrack_to writes fail entry normally', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: 'unit-test',
      report: 'fail no backtrack',
      checklist: FAILED_ITEMS,
      backtrack_to: null,
    });

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).toHaveBeenCalled();
  });
});

describe('runPhaseLog — idempotency', () => {
  it('identical pass calls invoke appendEntry twice with consistent args (AC-16)', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-25T01:29:05.000Z'));
    try {
      mockWorkflowType('test-only');
      const opts = {
        change: 'test-change',
        phase: 'proposal' as const,
        verdict: 'pass' as const,
        report: 'ok',
        checklist: VALID_ITEMS,
        backtrack_to: null,
      };

      runPhaseLog(opts);
      runPhaseLog(opts);

      expect(appendEntry).toHaveBeenCalledTimes(2);
      expect(vi.mocked(appendEntry).mock.calls[0]).toEqual(vi.mocked(appendEntry).mock.calls[1]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('identical fail+null calls behave consistently', () => {
    mockWorkflowType('test-only');
    const opts = {
      change: 'test-change',
      phase: 'unit-test' as const,
      verdict: 'fail' as const,
      report: 'bugs',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    };

    runPhaseLog(opts);
    runPhaseLog(opts);

    expect(appendEntry).toHaveBeenCalledTimes(2);
  });

  it('repeated invalid backtrack throws twice with zero appendEntry calls', () => {
    mockWorkflowType('test-only');
    const opts = {
      change: 'test-change',
      phase: 'unit-test' as const,
      verdict: 'fail' as const,
      report: 'bugs',
      checklist: VALID_ITEMS,
      backtrack_to: 'implement',
    };

    expect(() => runPhaseLog(opts)).toThrow();
    expect(() => runPhaseLog(opts)).toThrow();
    expect(appendEntry).not.toHaveBeenCalled();
  });
});

describe('runPhaseLog — auto-calculated verdict from checklist', () => {
  it('auto-calculates pass when all checklist items pass', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'all good',
      checklist: [
        { item: 'check 1', pass: true, evidence: 'ok' },
        { item: 'check 2', pass: true, evidence: 'ok' },
      ],
      backtrack_to: null,
    });

    expect(appendEntry).toHaveBeenCalled();
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.verdict).toBe('pass');
  });

  it('auto-calculates fail when any item fails', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'has issues',
      checklist: [
        { item: 'check 1', pass: true, evidence: 'ok' },
        { item: 'check 2', pass: false, evidence: 'broken' },
      ],
      backtrack_to: null,
    });

    expect(appendEntry).toHaveBeenCalled();
    const entry = vi.mocked(appendEntry).mock.calls[0][1];
    expect(entry.verdict).toBe('fail');
  });
});
