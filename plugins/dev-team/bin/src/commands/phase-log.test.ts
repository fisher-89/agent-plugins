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
        phase: '06-unit-test',
        verdict: 'fail',
        report: 'bugs found',
        items: VALID_ITEMS,
        backtrack_to: '05-implement',
      }),
    ).toThrow(/当前工作流 test-only 不包含 phase '05-implement'/);

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).not.toHaveBeenCalled();
    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('rejection error lists available phases (AC-12)', () => {
    mockWorkflowType('test-only');

    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: '06-unit-test',
        verdict: 'fail',
        report: 'bugs found',
        items: VALID_ITEMS,
        backtrack_to: '05-implement',
      }),
    ).toThrow(
      /01-proposal, 02-code-analyze, 03-test-design, 04-test-gen, 06-unit-test, 08-integration-test/,
    );
  });

  it('uses requirement table when workflow.json absent (AC-13)', () => {
    mockWorkflowType(undefined);

    runPhaseLog({
      change: 'test-change',
      phase: '06-unit-test',
      verdict: 'fail',
      report: 'test issue',
      items: VALID_ITEMS,
      backtrack_to: '02-dev-design',
    });

    expect(markPhaseStale).toHaveBeenCalled();
    expect(vi.mocked(markPhaseStale).mock.calls[0][1]).toBe('02-dev-design');
  });

  it('rejects backtrack_to 02-dev-design for test-only workflow (AC-11)', () => {
    mockWorkflowType('test-only');

    expect(() =>
      runPhaseLog({
        change: 'test-change',
        phase: '06-unit-test',
        verdict: 'fail',
        report: 'bugs found',
        items: VALID_ITEMS,
        backtrack_to: '02-dev-design',
      }),
    ).toThrow(/当前工作流 test-only 不包含 phase '02-dev-design'/);

    expect(appendEntry).not.toHaveBeenCalled();
    expect(writeEvalJson).not.toHaveBeenCalled();
  });

  it('allows backtrack_to 05-implement when workflow.json missing — requirement default (AC-13)', () => {
    mockWorkflowType(undefined);

    runPhaseLog({
      change: 'test-change',
      phase: '06-unit-test',
      verdict: 'fail',
      report: 'test issue',
      items: VALID_ITEMS,
      backtrack_to: '05-implement',
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
        phase: '06-unit-test',
        verdict: 'fail',
        report: 'bugs found',
        items: VALID_ITEMS,
        backtrack_to: '05-implement',
      }),
    ).toThrow();

    runPhaseLog({
      change: 'test-change',
      phase: '06-unit-test',
      verdict: 'fail',
      report: 'bugs found, no backtrack',
      items: VALID_ITEMS,
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
      phase: '01-proposal',
      verdict: 'pass',
      report: 'ok',
      items: VALID_ITEMS,
    });

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).toHaveBeenCalled();
  });
});

describe('runPhaseLog — backtrack_to triggers markPhaseStale', () => {
  it('test-only valid backtrack 03-test-design → 01-proposal triggers markPhaseStale', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: '03-test-design',
      verdict: 'fail',
      report: 'needs redo',
      items: VALID_ITEMS,
      backtrack_to: '01-proposal',
    });

    expect(markPhaseStale).toHaveBeenCalledWith(expect.anything(), '01-proposal');
    expect(writeEvalJson).toHaveBeenCalled();
  });
});

describe('runPhaseLog — input validation', () => {
  it('empty string backtrack_to does not trigger backtrack validation', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: '06-unit-test',
      verdict: 'fail',
      report: 'fail without backtrack',
      items: VALID_ITEMS,
      backtrack_to: '',
    });

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).toHaveBeenCalled();
  });

  it('null backtrack_to writes fail entry normally', () => {
    mockWorkflowType('test-only');

    runPhaseLog({
      change: 'test-change',
      phase: '06-unit-test',
      verdict: 'fail',
      report: 'fail no backtrack',
      items: VALID_ITEMS,
      backtrack_to: null,
    });

    expect(markPhaseStale).not.toHaveBeenCalled();
    expect(appendEntry).toHaveBeenCalled();
  });
});

describe('runPhaseLog — idempotency', () => {
  it('identical pass calls invoke appendEntry twice with consistent args (AC-16)', () => {
    mockWorkflowType('test-only');
    const opts = {
      change: 'test-change',
      phase: '01-proposal',
      verdict: 'pass' as const,
      report: 'ok',
      items: VALID_ITEMS,
    };

    runPhaseLog(opts);
    runPhaseLog(opts);

    expect(appendEntry).toHaveBeenCalledTimes(2);
    expect(vi.mocked(appendEntry).mock.calls[0]).toEqual(vi.mocked(appendEntry).mock.calls[1]);
  });

  it('identical fail+null calls behave consistently', () => {
    mockWorkflowType('test-only');
    const opts = {
      change: 'test-change',
      phase: '06-unit-test',
      verdict: 'fail' as const,
      report: 'bugs',
      items: VALID_ITEMS,
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
      phase: '06-unit-test',
      verdict: 'fail' as const,
      report: 'bugs',
      items: VALID_ITEMS,
      backtrack_to: '05-implement',
    };

    expect(() => runPhaseLog(opts)).toThrow();
    expect(() => runPhaseLog(opts)).toThrow();
    expect(appendEntry).not.toHaveBeenCalled();
  });
});
