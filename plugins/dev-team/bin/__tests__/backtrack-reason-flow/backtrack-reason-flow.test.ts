/**
 * 集成测试: backtrack-reason-flow
 *
 * 验证调用 backtrack MCP 工具写入 backtrack_to + backtrack_reason 后，
 * phase_next 返回的 prompt 包含回溯原因。
 *
 * 评估历史存放在 `workflow.json.eval`；本用例用内存数组 stub 读写入口，
 * `workflow.json` 只提供 `workflow_type`，夹具不再依赖磁盘上的独立 `eval.json`。
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
    writeEvalJson: vi.fn((_dir: string, entries: Array<Record<string, unknown>>) => {
      mockEntries.length = 0;
      mockEntries.push(...entries);
    }),
  };
});

vi.mock('../../src/lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { runBacktrack } from '../../src/commands/backtrack';
import { runPhaseLog } from '../../src/commands/phase-log';
import { runPhaseNext } from '../../src/commands/phase-next';

const FIXTURE_PROJECT_ROOT = '/tmp/fixture-project';
const DEFAULT_RUN_ID = 'test-run';

const FAILED_ITEMS = [{ item: 'test', pass: false, evidence: 'none' }];
const VALID_ITEMS = [{ item: 'test', pass: true, evidence: 'ok' }];

/** workflow.json 只提供 workflow_type（评估条目由内存 mock 提供）。 */
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
        return JSON.stringify({ workflow_type: workflowType, created: '2026-09-11' });
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

describe('backtrack-reason-flow — 回溯原因传播', () => {
  it('backtrack 写入 backtrack_to + backtrack_reason 后，phase_next 返回的 prompt 含回溯原因', () => {
    // Step 1: pass phases up to test-design
    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal ok',
      checklist: VALID_ITEMS,
    });

    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'code-analyze',
      report: 'analysis ok',
      checklist: VALID_ITEMS,
    });

    // Step 2: log fail, then call backtrack separately
    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-design',
      report: 'test design needs redo',
      checklist: FAILED_ITEMS,
    });

    // Use the standalone backtrack tool instead of passing backtrack_to to phase_log
    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-design',
      backtrack_to: 'proposal',
      backtrack_reason: '设计文档缺少测试覆盖范围定义',
    });

    // Step 3: phase_next should return proposal with reason in prompt
    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: DEFAULT_RUN_ID,
    });

    expect(result.next_phase).toBe('proposal');
    expect(result.error).toBeNull();
    expect(result.executor!.prompt).toContain('⚠️ 回溯原因: 设计文档缺少测试覆盖范围定义');
    expect(result.evaluator!.prompt).toContain('⚠️ 回溯原因: 设计文档缺少测试覆盖范围定义');
  });

  it('多阶段回溯：回溯到不同阶段时 prompt 均包含正确的原因', () => {
    // Track 1: backtrack to proposal
    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal ok',
      checklist: VALID_ITEMS,
    });

    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'code-analyze',
      report: 'analysis ok',
      checklist: VALID_ITEMS,
    });

    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-design',
      report: 'needs redo',
      checklist: FAILED_ITEMS,
    });

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'test-design',
      backtrack_to: 'proposal',
      backtrack_reason: '第一阶段回溯：范围定义不清',
    });

    let result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: DEFAULT_RUN_ID,
    });
    expect(result.next_phase).toBe('proposal');
    expect(result.executor!.prompt).toContain('第一阶段回溯：范围定义不清');

    // Re-pass proposal with backtrack
    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal redo ok',
      checklist: VALID_ITEMS,
    });

    // Track 2: backtrack to code-analyze
    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'code-analyze',
      report: 'needs redo',
      checklist: FAILED_ITEMS,
    });

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'code-analyze',
      backtrack_to: 'proposal',
      backtrack_reason: '第二阶段回溯：代码分析不充分',
    });

    result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: DEFAULT_RUN_ID,
    });
    expect(result.next_phase).toBe('proposal');
    expect(result.executor!.prompt).toContain('第二阶段回溯：代码分析不充分');
    expect(result.executor!.prompt).not.toContain('第一阶段回溯');
  });

  it('executor 和 evaluator 的 prompt 均包含 ⚠️ 回溯原因: 前缀', () => {
    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'proposal',
      report: 'proposal ok',
      checklist: VALID_ITEMS,
    });

    advanceTime();
    runPhaseLog({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'code-analyze',
      report: 'test design needs redo',
      checklist: FAILED_ITEMS,
    });

    runBacktrack({
      project_root: FIXTURE_PROJECT_ROOT,
      change: 'test-change',
      phase: 'code-analyze',
      backtrack_to: 'proposal',
      backtrack_reason: '需重新审视 proposal',
    });

    const result = runPhaseNext({
      change: 'test-change',
      project_root: FIXTURE_PROJECT_ROOT,
      run_id: DEFAULT_RUN_ID,
    });

    expect(result.executor!.prompt).toMatch(/⚠️ 回溯原因:/);
    expect(result.evaluator!.prompt).toMatch(/⚠️ 回溯原因:/);
  });
});
