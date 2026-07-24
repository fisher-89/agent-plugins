/**
 * 集成测试: backtrack-reason-compat
 *
 * 验证旧格式 eval.json（无 backtrack_reason 字段）的向后兼容性。
 *
 * @see openspec/changes/backtrack-reason-propagation/test-design.md — AC-6
 */

import * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

// Pre-built old-format entries WITHOUT backtrack_reason field
const oldFormatEntries: Array<Record<string, unknown>> = [
  {
    phase: 'proposal',
    verdict: 'pass',
    attempt: 1,
    timestamp: new Date('2026-01-01T00:00:00.000Z').toISOString(),
    backtrack_to: null,
    report: 'Proposal accepted',
    checklist: [{ item: 'scope', pass: true, evidence: 'defined' }],
    skipped: false,
  },
  {
    phase: 'code-analyze',
    verdict: 'pass',
    attempt: 1,
    timestamp: new Date('2026-01-02T00:00:00.000Z').toISOString(),
    backtrack_to: null,
    report: 'Analysis complete',
    checklist: [{ item: 'analysis', pass: true, evidence: 'done' }],
    skipped: false,
  },
  {
    phase: 'test-design',
    verdict: 'fail',
    attempt: 1,
    timestamp: new Date('2026-01-03T00:00:00.000Z').toISOString(),
    backtrack_to: 'proposal',
    // Intentionally NO backtrack_reason — old format
    report: 'Test design incomplete',
    checklist: [{ item: 'coverage', pass: false, evidence: 'missing' }],
    skipped: false,
  },
];

// Mixed format: some old entries without backtrack_reason, some new with it
const mixedFormatEntries: Array<Record<string, unknown>> = [
  ...oldFormatEntries,
  {
    phase: 'test-gen',
    verdict: 'pass',
    attempt: 1,
    timestamp: new Date('2026-01-04T00:00:00.000Z').toISOString(),
    backtrack_to: null,
    report: 'Tests generated',
    checklist: [{ item: 'generation', pass: true, evidence: 'done' }],
    skipped: false,
  },
  {
    phase: 'test-execution',
    verdict: 'fail',
    attempt: 1,
    timestamp: new Date('2026-01-05T00:00:00.000Z').toISOString(),
    backtrack_to: 'code-analyze',
    backtrack_reason: '测试执行发现新的分析需求',
    report: 'Execution found issues',
    checklist: [{ item: 'execution', pass: false, evidence: 'failure' }],
    skipped: false,
  },
];

vi.mock('../../src/lib/eval-json', async () => {
  const actual = await vi.importActual('../../src/lib/eval-json');
  return {
    ...actual,
    markPhaseStale: vi.fn(),
    readEvalJson: vi.fn(() => [...actualReadEntries]),
    appendEntry: vi.fn(),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../../src/lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { runPhaseNext } from '../../src/commands/phase-next';

// Module-level variable to control what readEvalJson returns
let actualReadEntries: Array<Record<string, unknown>> = [];

function mockWorkflowType(workflowType: string): void {
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    const p = String(filePath);
    if (p === '/tmp/test-change') {
      return true;
    }
    return p.endsWith('workflow.json');
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
  actualReadEntries = [];
  mockWorkflowType('test-only');
});

describe('backtrack-reason-compat — 向后兼容 (AC-6)', () => {
  it('读取不含 backtrack_reason 字段的旧格式 eval.json，phase_next 正常返回不抛错', () => {
    actualReadEntries = [...oldFormatEntries];

    expect(() => runPhaseNext({ change: 'test-change' })).not.toThrow();

    const result = runPhaseNext({ change: 'test-change' });
    expect(result.error).toBeNull();
    // Should backtrack to proposal (from old-format test-design)
    expect(result.next_phase).toBe('proposal');
  });

  it('混合新旧格式条目，回溯时 prompt 不拼接 ⚠️ 回溯原因:', () => {
    actualReadEntries = [...mixedFormatEntries];

    const result = runPhaseNext({ change: 'test-change' });

    // Latest entry is test-execution with backtrack_to: code-analyze
    // But the actual backtrack entries in the old format had no backtrack_reason
    // The latest entry (test-execution) has backtrack_to: 'code-analyze' and backtrack_reason is set
    // So the prompt should contain the reason
    expect(result.error).toBeNull();
    expect(result.next_phase).toBe('code-analyze');
    // The latest entry (test-execution) DOES have backtrack_reason
    // So the prompt should contain it
    expect(result.executor!.prompt).toContain('⚠️ 回溯原因: 测试执行发现新的分析需求');
  });

  it('旧条目中无 backtrack_reason，backtrack_to 有效时仍正常回溯', () => {
    // Only use old-format entries (no backtrack_reason anywhere)
    actualReadEntries = [...oldFormatEntries];

    const result = runPhaseNext({ change: 'test-change' });

    // Should still backtrack to proposal (backtrack_to is present)
    expect(result.next_phase).toBe('proposal');
    expect(result.error).toBeNull();
    // Reason should be null (no backtrack_reason in old entries)
    expect(result.executor!.prompt).not.toContain('⚠️ 回溯原因');
    // Planner and evaluator should be present (normal phase response)
    expect(result.executor).not.toBeNull();
    expect(result.evaluator).not.toBeNull();
  });
});
