import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { eventsToUIMessages } from '../../../lib/agent-adapter';
import type {
  AgentEvent,
  ArchivePreflight,
  ArchiveStageState,
  ArchiveStageStatus,
  ArchiveSummary,
} from '../../../types/dto';
import type { UseArchiveFlowResult } from '../hooks/use-archive-flow';
import { ArchivePanel } from './archive-panel';
import type { ArchiveFlowState } from './archive-state';

// ---------------------------------------------------------------------------
// 进程边界 Mock：面板纯呈现面——状态与动作经 use-archive-flow 注入（props 直
// 注 mock 对象）；转录入口经 use-session-transcript mock 注入（messages 可编
// 程）；AgentTimeline 真实渲染（复用基建——零第二套时间线的渲染锚）。
// ---------------------------------------------------------------------------

const transcriptMock = vi.hoisted(() => vi.fn());

vi.mock('../hooks/use-session-transcript', () => ({
  useSessionTranscript: transcriptMock,
}));

const ROOT = 'C:\\demo\\archive';
const CHANGE = 'archive-demo';
const TS = 1727000000000;

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

function stageRow(
  stage: ArchiveStageState['stage'],
  status: ArchiveStageStatus,
  detail: string | null = null,
): ArchiveStageState {
  return { stage, status, detail };
}

/** 确认对话 preflight fixture（宽松默认 + 逐字段覆写）。 */
function preflightFixture(overrides: Partial<ArchivePreflight> = {}): ArchivePreflight {
  return {
    name: CHANGE,
    completed: true,
    incompletePhases: [],
    missingArtifacts: [],
    deltaSpecs: [],
    worktree: null,
    branch: null,
    mergeTarget: null,
    runActive: false,
    ...overrides,
  };
}

/** 前端归档视图模型 fixture（宽松默认 + 逐字段覆写）。 */
function flowState(overrides: Partial<ArchiveFlowState> = {}): ArchiveFlowState {
  return {
    stages: {},
    sessionId: null,
    summary: null,
    error: null,
    finished: false,
    liveEvents: {},
    ...overrides,
  };
}

function summaryFixture(overrides: Partial<ArchiveSummary> = {}): ArchiveSummary {
  return {
    name: CHANGE,
    archivedDir: `2026-10-08-${CHANGE}`,
    specs: 'synced',
    warnings: [],
    ...overrides,
  };
}

function transcriptTextEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

// ---------------------------------------------------------------------------
// 装置：archive hook 注入对象（动作 vi.fn 供调用断言）
// ---------------------------------------------------------------------------

interface ArchiveHarness {
  archive: UseArchiveFlowResult;
  preflightMock: ReturnType<typeof vi.fn>;
  startMock: ReturnType<typeof vi.fn>;
  stopMock: ReturnType<typeof vi.fn>;
}

function archiveHarness(
  state: ArchiveFlowState | null,
  overrides: Partial<UseArchiveFlowResult> = {},
): ArchiveHarness {
  const preflightMock = vi.fn().mockResolvedValue(preflightFixture());
  const startMock = vi.fn().mockResolvedValue(undefined);
  const stopMock = vi.fn().mockResolvedValue(undefined);
  const archive: UseArchiveFlowResult = {
    state,
    preflight: preflightMock,
    start: startMock,
    stop: stopMock,
    error: null,
    ...overrides,
  };
  return { archive, preflightMock, startMock, stopMock };
}

function renderPanel(archive: UseArchiveFlowResult, open = true) {
  const onClose = vi.fn();
  const mounted = render(
    <ArchivePanel root={ROOT} change={CHANGE} archive={archive} open={open} onClose={onClose} />,
  );
  return { onClose, unmount: mounted.unmount };
}

afterEach(() => {
  cleanup();
});

beforeEach(() => {
  transcriptMock.mockReset();
  transcriptMock.mockReturnValue({
    messages: [],
    running: false,
    error: null,
    summary: null,
  });
});

// ---------------------------------------------------------------------------
// 确认对话面（AC-1 / AC-2 / D13）
// ---------------------------------------------------------------------------

describe('ArchivePanel：确认对话数据面', () => {
  it('preflight fixture → archive-confirm-dialog 呈现；警告行逐条 + delta 清单 + 合入告知含目标分支名', async () => {
    const { archive } = archiveHarness(null);
    archive.preflight = vi.fn().mockResolvedValue(
      preflightFixture({
        completed: false,
        incompletePhases: ['implement', 'test-gen'],
        missingArtifacts: ['design.md'],
        deltaSpecs: ['desktop-change-archive', 'desktop-crate-layout'],
        worktree: 'C:\\wt\\demo',
        mergeTarget: 'main',
      }),
    );
    renderPanel(archive);

    const dialog = await screen.findByTestId('archive-confirm-dialog');
    expect(dialog).toBeDefined();
    // 警告行逐条（完成度 + 产物缺失）
    const warnings = screen.getAllByTestId('archive-warning');
    expect(warnings).toHaveLength(2);
    expect(warnings[0].textContent).toContain('工作流未全部通过');
    expect(warnings[0].textContent).toContain('implement、test-gen');
    expect(warnings[1].textContent).toContain('缺少产物文档');
    expect(warnings[1].textContent).toContain('design.md');
    // delta specs 清单两 capability
    expect(screen.getByTestId('archive-delta-specs').textContent).toContain(
      'desktop-change-archive、desktop-crate-layout',
    );
    // 合入告知含目标分支名（R4 防护面）
    expect(screen.getByTestId('archive-merge-notice').textContent).toContain('main');
  });

  it('runActive=true → 拒绝卡呈现运行中原因；archive-confirm-ok 不渲染、零 start 调用', async () => {
    const { archive, startMock } = archiveHarness(null);
    archive.preflight = vi.fn().mockResolvedValue(preflightFixture({ runActive: true }));
    renderPanel(archive);

    const card = await screen.findByTestId('archive-run-active');
    expect(card.textContent).toContain('运行中的 run');
    expect(screen.queryByTestId('archive-confirm-ok')).toBeNull();
    expect(startMock).not.toHaveBeenCalled();
  });

  it('跳过同步 checkbox：delta 缺席 → 隐藏；在场默认不勾 → 确认调 start(true)；勾选 → start(false)', async () => {
    // delta 缺席：checkbox 整块隐藏
    const absent = archiveHarness(null);
    absent.archive.preflight = vi.fn().mockResolvedValue(preflightFixture({ deltaSpecs: [] }));
    renderPanel(absent.archive);
    await screen.findByTestId('archive-confirm-dialog');
    expect(screen.queryByTestId('archive-sync-skip')).toBeNull();
    cleanup();

    // delta 在场：默认不勾 → start(true)
    const present = archiveHarness(null);
    present.archive.preflight = vi
      .fn()
      .mockResolvedValue(preflightFixture({ deltaSpecs: ['cap-a'] }));
    renderPanel(present.archive);
    await screen.findByTestId('archive-sync-skip');
    fireEvent.click(screen.getByTestId('archive-confirm-ok'));
    await waitFor(() => expect(present.startMock).toHaveBeenCalledWith(true));

    // 勾选跳过 → start(false)（skill "Archive without syncing" 对译）
    fireEvent.click(screen.getByTestId('archive-sync-skip'));
    fireEvent.click(screen.getByTestId('archive-confirm-ok'));
    await waitFor(() => expect(present.startMock).toHaveBeenCalledWith(false));
  });

  it('全绿齐备 fixture → 无警告行、确认 / 取消两按钮；取消 → 关闭且零 start；有警告时确认按钮同样可用（警告不阻断）', async () => {
    // 全绿轻量确认
    const clean = archiveHarness(null);
    renderPanel(clean.archive);
    await screen.findByTestId('archive-confirm-dialog');
    expect(screen.queryByTestId('archive-warning')).toBeNull();
    expect(screen.getByTestId('archive-confirm-ok')).toBeDefined();
    expect(screen.getByTestId('archive-confirm-cancel')).toBeDefined();
    fireEvent.click(screen.getByTestId('archive-confirm-cancel'));
    await waitFor(() => expect(clean.startMock).not.toHaveBeenCalled());
    cleanup();

    // 有警告：确认按钮同样可用（inform + confirm——AC-2 UI 半边）
    const warned = archiveHarness(null);
    warned.archive.preflight = vi
      .fn()
      .mockResolvedValue(preflightFixture({ completed: false, incompletePhases: ['implement'] }));
    renderPanel(warned.archive);
    await screen.findByTestId('archive-warning');
    fireEvent.click(screen.getByTestId('archive-confirm-ok'));
    await waitFor(() => expect(warned.startMock).toHaveBeenCalledWith(true));
  });
});

// ---------------------------------------------------------------------------
// 进行面（D14 阶段清单 / 停止 / 转录入口）
// ---------------------------------------------------------------------------

describe('ArchivePanel：阶段清单与停止', () => {
  it('归档进行态 → archive-stage-list 六行在场；specSync / commit 状态与 detail 随 state 呈现', () => {
    const { archive } = archiveHarness(
      flowState({
        stages: {
          preflight: stageRow('preflight', 'passed'),
          specSync: stageRow('specSync', 'passed'),
          commit: stageRow('commit', 'running'),
          merge: stageRow('merge', 'skipped', '已合入'),
        },
      }),
    );
    renderPanel(archive);

    const rows = screen
      .getAllByTestId(/^archive-stage-/)
      .filter((row) => row.dataset.testid !== 'archive-stage-list');
    // 六段固定行
    expect(rows).toHaveLength(6);
    const stageLine = (stage: string): HTMLElement => screen.getByTestId(`archive-stage-${stage}`);
    expect(stageLine('specSync').dataset.status).toBe('passed');
    expect(stageLine('commit').dataset.status).toBe('running');
    expect(stageLine('merge').textContent).toContain('已合入');
    expect(stageLine('finalize').dataset.status).toBe('pending');
  });

  it('archive-stop 点击调 stop；sessionId 在场 → archive-transcript 呈现且 AgentTimeline 渲染转录事件；sessionId null → 入口不渲染', async () => {
    transcriptMock.mockReturnValue({
      messages: eventsToUIMessages([transcriptTextEvent(0, '已同步 cap-a')]),
      running: false,
      error: null,
      summary: null,
    });
    const harness = archiveHarness(
      flowState({
        stages: { preflight: stageRow('preflight', 'passed') },
        sessionId: 'sess-live-1',
        liveEvents: { 'sess-live-1': [transcriptTextEvent(0, '已同步 cap-a')] },
      }),
    );
    renderPanel(harness.archive);

    fireEvent.click(screen.getByTestId('archive-stop'));
    await waitFor(() => expect(harness.stopMock).toHaveBeenCalledTimes(1));

    // 转录入口复用基建（AgentTimeline 直组）——文本经真实时间线渲染
    expect(screen.getByTestId('archive-transcript')).toBeDefined();
    await screen.findByText('已同步 cap-a');
    expect(transcriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: 'sess-live-1' }),
    );

    // sessionId null → 入口不渲染
    cleanup();
    const idle = archiveHarness(
      flowState({ stages: { preflight: stageRow('preflight', 'passed') } }),
    );
    renderPanel(idle.archive);
    expect(screen.queryByTestId('archive-transcript')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 终态面（AC-7 结果摘要 / 错误面）
// ---------------------------------------------------------------------------

describe('ArchivePanel：结果摘要三态', () => {
  it('Finished summary → archive-summary 呈现名 + 归档目录；specs 行三态文案随 specs 值', () => {
    const cases: Array<[ArchiveSummary['specs'], string]> = [
      ['synced', '已同步 delta specs'],
      ['skipped', '跳过 spec 同步（用户选择）'],
      ['none', '无 delta specs'],
    ];
    for (const [specs, expected] of cases) {
      const { archive } = archiveHarness(
        flowState({ finished: true, summary: summaryFixture({ specs }) }),
      );
      const { unmount } = renderPanel(archive);
      expect(screen.getByTestId('archive-summary').textContent).toContain(CHANGE);
      expect(screen.getByTestId('archive-summary').textContent).toContain(`2026-10-08-${CHANGE}`);
      expect(screen.getByTestId('archive-specs-line').textContent).toBe(expected);
      unmount();
    }
  });

  it('warnings 清单逐条呈现（D12 词汇对译）', () => {
    const { archive } = archiveHarness(
      flowState({
        finished: true,
        summary: summaryFixture({
          warnings: ['工作流未全部通过：implement', '缺少产物文档：design.md'],
        }),
      }),
    );
    renderPanel(archive);
    const warnings = screen.getAllByTestId('archive-warning');
    expect(warnings).toHaveLength(2);
    expect(warnings[0].textContent).toContain('工作流未全部通过：implement');
    expect(warnings[1].textContent).toContain('缺少产物文档：design.md');
  });
});

describe('ArchivePanel：错误面', () => {
  it('Finished error → archive-error 呈现错误串（merge 冲突 git 语境 / agent 失败语境两 fixture）', () => {
    for (const error of [
      'git merge 失败（退出码 1）: CONFLICT…；归档合入失败：请手动处置冲突后重试归档',
      'spec 同步会话失败收敛（delta specs 未同步，链停同步段）',
    ]) {
      const { archive } = archiveHarness(flowState({ finished: true, error }));
      const { unmount } = renderPanel(archive);
      expect(screen.getByTestId('archive-error').textContent).toBe(error);
      unmount();
    }
  });
});
