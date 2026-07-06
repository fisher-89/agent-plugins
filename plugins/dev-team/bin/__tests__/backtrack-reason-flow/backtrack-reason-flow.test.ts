/**
 * 集成测试: backtrack-reason-flow
 *
 * 验证 phase_log 写入 backtrack_to + backtrack_reason 后，
 * phase_next 返回的 prompt 包含回溯原因。
 *
 * @see openspec/changes/backtrack-reason-propagation/test-design.md — AC-5
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
    writeEvalJson: vi.fn((_dir: string, entries: Array<Record<string, unknown>>) => {
      mockEntries.length = 0;
      mockEntries.push(...entries);
    }),
  };
});

vi.mock('../../src/lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { runPhaseLog } from '../../src/commands/phase-log';
import { runPhaseNext } from '../../src/commands/phase-next';

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

// Helper to ensure entries get different timestamps (avoid millisecond collision)
let _timestampCounter = 0;
function advanceTime(): void {
  _timestampCounter++;
  vi.setSystemTime(new Date(Date.now() + _timestampCounter));
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  _timestampCounter = 0;
  mockEntries.length = 0;
  mockWorkflowType('test-only');
});

// Note: vi.useRealTimers() not needed — beforeEach resets with vi.useFakeTimers() for each test

describe('backtrack-reason-flow — 回溯原因传播 (AC-5)', () => {
  it('phase_log 写入 backtrack_to + backtrack_reason 后，phase_next 返回的 prompt 含回溯原因', () => {
    // Step 1: pass phases up to test-design
    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'code-analyze',
      report: 'analysis ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    // Step 2: log fail with backtrack + reason
    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'test-design',
      report: 'test design needs redo',
      checklist: FAILED_ITEMS,
      backtrack_to: 'proposal',
      backtrack_reason: '设计文档缺少测试覆盖范围定义',
    });

    // Step 3: phase_next should return proposal with reason in prompt
    const result = runPhaseNext({ change: 'test-change' });

    expect(result.next_phase).toBe('proposal');
    expect(result.error).toBeNull();
    expect(result.planner!.prompt).toContain('⚠️ 回溯原因: 设计文档缺少测试覆盖范围定义');
    expect(result.evaluator!.prompt).toContain('⚠️ 回溯原因: 设计文档缺少测试覆盖范围定义');
  });

  it('多阶段回溯：回溯到不同阶段时 prompt 均包含正确的原因', () => {
    // Track 1: backtrack to proposal
    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'code-analyze',
      report: 'analysis ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'test-design',
      report: 'needs redo',
      checklist: FAILED_ITEMS,
      backtrack_to: 'proposal',
      backtrack_reason: '第一阶段回溯：范围定义不清',
    });

    let result = runPhaseNext({ change: 'test-change' });
    expect(result.next_phase).toBe('proposal');
    expect(result.planner!.prompt).toContain('第一阶段回溯：范围定义不清');

    // Re-pass proposal with backtrack
    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal redo ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    // Track 2: backtrack to code-analyze
    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'code-analyze',
      report: 'needs redo',
      checklist: FAILED_ITEMS,
      backtrack_to: 'proposal',
      backtrack_reason: '第二阶段回溯：代码分析不充分',
    });

    result = runPhaseNext({ change: 'test-change' });
    expect(result.next_phase).toBe('proposal');
    expect(result.planner!.prompt).toContain('第二阶段回溯：代码分析不充分');
    expect(result.planner!.prompt).not.toContain('第一阶段回溯');
  });

  it('planner 和 evaluator 的 prompt 均包含 ⚠️ 回溯原因: 前缀', () => {
    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal ok',
      checklist: VALID_ITEMS,
      backtrack_to: null,
    });

    advanceTime();
    runPhaseLog({
      change: 'test-change',
      phase: 'code-analyze',
      report: 'test design needs redo',
      checklist: FAILED_ITEMS,
      backtrack_to: 'proposal',
      backtrack_reason: '需重新审视 proposal',
    });

    const result = runPhaseNext({ change: 'test-change' });

    expect(result.planner!.prompt).toMatch(/⚠️ 回溯原因:/);
    expect(result.evaluator!.prompt).toMatch(/⚠️ 回溯原因:/);
  });
});
