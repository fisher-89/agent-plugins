// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ArtifactDescriptor, ChangeDetail, ChangeList } from '../types/dto';

// ---------------------------------------------------------------------------
// 进程边界 Mock：Tauri IPC（invoke）与对话框（open）；链路内模块间调用一律真实实现
// ---------------------------------------------------------------------------

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

// ---------------------------------------------------------------------------
// fixture DTO
// ---------------------------------------------------------------------------

const listWithUnknownGroup: ChangeList = {
  active: [
    {
      name: 'add-feature',
      source: 'active',
      inventory: 'v2',
      created: '2026-09-01',
      unparsable: false,
    },
    { name: 'docs-only', source: 'active', inventory: 'v0', created: null, unparsable: false },
  ],
  archiveGroups: [
    {
      month: '2026-09',
      changes: [
        {
          name: '2026-09-01-archived',
          source: 'archive',
          inventory: 'v1',
          created: '2026-09-01',
          unparsable: false,
        },
      ],
    },
    {
      month: null,
      changes: [
        {
          name: 'no-date-archived',
          source: 'archive',
          inventory: 'v1',
          created: null,
          unparsable: false,
        },
      ],
    },
  ],
};

const artifacts: ArtifactDescriptor[] = [
  { kind: 'markdown-doc', source: 'proposal.md', title: '提案' },
  { kind: 'eval-checklist', source: '0', title: '评估清单 · proposal · 第 1 次' },
  { kind: 'tasks-progress', source: 'tasks.md', title: '任务进度' },
];

function detailDto(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: [
      {
        phase: 'proposal',
        attempts: [
          {
            attempt: 1,
            verdict: 'pass',
            report: '提案通过',
            checklist: [{ item: '问题清晰', pass: true, evidence: 'L1-10' }],
            skipped: false,
            stale: false,
            startAt: null,
            timestamp: null,
            backtrackTo: null,
            backtrackReason: null,
          },
        ],
      },
      { phase: 'dev-design', attempts: [] },
      { phase: 'test-design', attempts: [] },
      { phase: 'implement', attempts: [] },
      { phase: 'test-gen', attempts: [] },
      { phase: 'test-execution', attempts: [] },
      { phase: 'code-review', attempts: [] },
      { phase: 'acceptance', attempts: [] },
      { phase: 'code-analyze', attempts: [] },
    ],
    activePhase: { phase: 'implement', attempt: 2, startAt: '2026-09-02T01:00:00Z' },
    interrupted: [],
    fileLog: [{ op: 'write', scope: 'workflow', attempt: 1, path: 'src/a.rs', at: null }],
    artifacts,
    ...overrides,
  };
}

function envelopeFor(descriptor: ArtifactDescriptor) {
  const payloads: Record<string, unknown> = {
    'markdown-doc': { markdown: '# 提案正文\n\n内容段落。' },
    'eval-checklist': {
      phase: 'proposal',
      attempt: 1,
      verdict: 'pass',
      items: [{ item: '问题清晰', pass: true, evidence: 'L1-10' }],
    },
    'tasks-progress': { total: 4, done: 1, pending: 3 },
  };
  return {
    kind: descriptor.kind,
    version: 1,
    title: descriptor.title,
    payload: payloads[descriptor.kind] ?? null,
    fallbackText: `保底：${descriptor.title}`,
  };
}

/** 默认 IPC 分发：按命令名与参数路由 fixture。 */
function defaultIpc(
  overrides: {
    detail?: ChangeDetail | null;
    readArtifact?: (params: { kind?: string; source?: string }) => Promise<unknown>;
  } = {},
) {
  invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
    if (command === 'list_workspaces') {
      // 启动自动恢复唯一清单项，恢复根即本文件的 workspace 根
      return Promise.resolve([{ root: '/repo', name: 'repo', addedAt: 1, lastOpenedAt: 2 }]);
    }
    if (command === 'touch_workspace') {
      return Promise.resolve(true);
    }
    if (command === 'list_changes') {
      return Promise.resolve(listWithUnknownGroup);
    }
    if (command === 'get_change_detail') {
      return Promise.resolve(overrides.detail !== undefined ? overrides.detail : detailDto());
    }
    if (command === 'read_artifact') {
      if (overrides.readArtifact) {
        return overrides.readArtifact(params);
      }
      const descriptor = artifacts.find(
        (d) => d.kind === params.kind && d.source === params.source,
      );
      return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
    }
    return Promise.resolve(null);
  });
}

/** 启动自动恢复以 /repo 为当前根并进入列表视图（选择入口流已由恢复流取代）。 */
async function pickWorkspace(path = '/repo') {
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: path }));
}

beforeEach(() => {
  invokeMock.mockReset();
  openMock.mockReset();
  defaultIpc();
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// 场景：选定 workspace 到列表渲染
// ---------------------------------------------------------------------------

describe('ipc 管线：启动自动恢复到列表渲染', () => {
  it('恢复根下发后：invoke 参数携带恢复 root，列表渲染月分组且未知时间组置尾', async () => {
    render(<App />);
    await pickWorkspace('/repo');

    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: '/repo' });

    // 分组与代际徽标渲染
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.getByText('2026-09-01-archived') !== null).toBe(true);
    const headings = screen.getAllByRole('heading').map((h) => h.textContent ?? '');
    expect(headings[headings.length - 1].startsWith('未知时间')).toBe(true);
    expect(screen.getByText('v2') !== null).toBe(true);
    expect(screen.getByText('v0') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 场景：列表到详情与产物渲染
// ---------------------------------------------------------------------------

describe('ipc 管线：列表到详情与产物渲染', () => {
  it('进入详情后 get_change_detail + N 次 read_artifact 的调用序列与产物清单一致', async () => {
    render(<App />);
    await pickWorkspace('/repo');
    fireEvent.click(screen.getByText('add-feature'));

    await waitFor(() => expect(screen.getByText('提案正文') !== null).toBe(true));

    expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
      root: '/repo',
      change: 'add-feature',
    });
    const readCalls = invokeMock.mock.calls.filter(([name]) => name === 'read_artifact');
    expect(readCalls).toHaveLength(3);
    expect(readCalls.map(([, params]) => params.kind)).toEqual([
      'markdown-doc',
      'eval-checklist',
      'tasks-progress',
    ]);
    expect(readCalls.map(([, params]) => params.source)).toEqual(['proposal.md', '0', 'tasks.md']);
  });

  it('三 kind 信封分别路由到对应 renderer 并渲染 payload', async () => {
    render(<App />);
    await pickWorkspace('/repo');
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() => expect(screen.getByText('提案正文') !== null).toBe(true));

    // markdown-doc renderer 的 payload 渲染
    expect(screen.getByText('内容段落。') !== null).toBe(true);
    // eval-checklist renderer 的 payload 渲染（同名条目也在详情 checklist 中，允许重复）
    expect(screen.getAllByText('问题清晰').length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText('L1-10').length).toBeGreaterThanOrEqual(2);
    // tasks-progress renderer 的 payload 渲染（1/4 = 25%）
    expect(screen.getByText('25%') !== null).toBe(true);
    // 详情主体的运行中标示与流水线也在
    expect(screen.getByText(/运行中 · implement · attempt 2/) !== null).toBe(true);
    expect(screen.getByText('提案通过') !== null).toBe(true);
  });

  it('产物清单为空时不发起 read_artifact，详情其余区块正常', async () => {
    defaultIpc({ detail: detailDto({ artifacts: [] }) });
    render(<App />);
    await pickWorkspace('/repo');
    fireEvent.click(screen.getByText('add-feature'));

    await waitFor(() => expect(screen.getByText('提案通过') !== null).toBe(true));
    expect(invokeMock.mock.calls.every(([name]) => name !== 'read_artifact')).toBe(true);
    expect(screen.getByText('（未发现可读产物）') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 场景：DTO 漂移与读取失败的降级兜底
// ---------------------------------------------------------------------------

describe('ipc 管线：DTO 漂移与读取失败的降级兜底', () => {
  it('未注册 kind 信封以 Fallback 渲染 fallback_text + kind 徽标，其余产物不受影响', async () => {
    defaultIpc({
      detail: detailDto({
        artifacts: [...artifacts, { kind: 'file-log', source: 'workflow.json', title: '文件清单' }],
      }),
      readArtifact: (params) => {
        if (params.kind === 'file-log') {
          return Promise.resolve({
            kind: 'file-log',
            version: 1,
            title: '文件清单',
            payload: null,
            fallbackText: 'file-log 保底文本',
          });
        }
        const descriptor = artifacts.find(
          (d) => d.kind === params.kind && d.source === params.source,
        );
        return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
      },
    });

    render(<App />);
    await pickWorkspace('/repo');
    fireEvent.click(screen.getByText('add-feature'));

    await waitFor(() => expect(screen.getByText('file-log 保底文本') !== null).toBe(true));
    // kind 徽标可见
    expect(screen.getByText('file-log') !== null).toBe(true);
    // 其余产物照常渲染
    expect(screen.getByText('提案正文') !== null).toBe(true);
    expect(screen.getByText('25%') !== null).toBe(true);
  });

  it('单个 read_artifact reject 时该产物降级呈现且不阻断详情主体', async () => {
    defaultIpc({
      readArtifact: (params) => {
        if (params.kind === 'tasks-progress') {
          return Promise.reject(new Error('读取失败'));
        }
        const descriptor = artifacts.find(
          (d) => d.kind === params.kind && d.source === params.source,
        );
        return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
      },
    });

    render(<App />);
    await pickWorkspace('/repo');
    fireEvent.click(screen.getByText('add-feature'));

    // 三张产物卡齐全：读取失败的一张以降级形态渲染（payload 为空 → 0% 计数），不白屏
    await waitFor(() => expect(screen.getAllByTestId('artifact-card')).toHaveLength(3));
    expect(screen.getByText('0%') !== null).toBe(true);
    // 其余产物与详情主体不受阻断
    expect(screen.getByText('内容段落。') !== null).toBe(true);
    expect(screen.getByText(/运行中 · implement · attempt 2/) !== null).toBe(true);
  });

  it('DTO 缺 fileLog / backtrack 字段（schema 演进）时对应区块留空，不抛错', async () => {
    const drifted = detailDto({
      fileLog: null,
      pipeline: detailDto().pipeline.map((station) =>
        station.phase === 'dev-design'
          ? {
              phase: 'dev-design',
              attempts: [
                {
                  attempt: 1,
                  verdict: 'pass',
                  report: '漂移条目',
                  checklist: [],
                  skipped: false,
                  stale: false,
                  startAt: null,
                  timestamp: null,
                  backtrackTo: null,
                  backtrackReason: null,
                },
              ],
            }
          : station,
      ),
    });
    // 模拟 schema 演进：新字段缺失（undefined 形态）
    const legacy = JSON.parse(JSON.stringify(drifted));
    legacy.pipeline[1].attempts[0].backtrackTo = undefined;
    defaultIpc({ detail: legacy });

    render(<App />);
    await pickWorkspace('/repo');
    fireEvent.click(screen.getByText('add-feature'));

    await waitFor(() => expect(screen.getByText('漂移条目') !== null).toBe(true));
    expect(screen.getByText(/（无 file_log 数据：v1 及更早代际无此字段）/) !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 场景：显式刷新纪律
// ---------------------------------------------------------------------------

describe('ipc 管线：显式刷新纪律', () => {
  it('交互序列结束后推进虚拟计时，invoke 调用次数不增长（无轮询 / watch）', async () => {
    vi.useFakeTimers();
    render(<App />);
    // 启动恢复序列（对话框 promise 立即 resolve，不依赖定时器）
    await act(async () => {});
    const callsAfterSelection = invokeMock.mock.calls.length;
    expect(callsAfterSelection).toBeGreaterThan(0);

    act(() => {
      vi.advanceTimersByTime(10 * 60 * 1000);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length).toBe(callsAfterSelection);
  });

  it('连续两次刷新产生两组完整调用，最终渲染以后一次为准', async () => {
    let round = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_workspaces') {
        return Promise.resolve([{ root: '/repo', name: 'repo', addedAt: 1, lastOpenedAt: 2 }]);
      }
      if (command === 'touch_workspace') {
        return Promise.resolve(true);
      }
      if (command === 'list_changes') {
        round += 1;
        return Promise.resolve({
          ...listWithUnknownGroup,
          active: [
            {
              name: round >= 2 ? 'latest-refresh-change' : 'add-feature',
              source: 'active',
              inventory: 'v2',
              created: null,
              unparsable: false,
            },
          ],
        });
      }
      if (command === 'get_change_detail') {
        return Promise.resolve(detailDto());
      }
      // read_artifact 在本用例不触达（详情未被打开）
      return Promise.resolve(null);
    });

    render(<App />);
    await pickWorkspace('/repo');
    expect(screen.getByText('add-feature') !== null).toBe(true);

    fireEvent.click(screen.getByText('刷新列表'));
    await waitFor(() => expect(screen.getByText('latest-refresh-change') !== null).toBe(true));
    expect(screen.queryByText('add-feature')).toBeNull();

    const listCalls = invokeMock.mock.calls.filter(([name]) => name === 'list_changes');
    expect(listCalls.length).toBe(2);
    expect(listCalls.every(([, params]) => params.root === '/repo')).toBe(true);
  });
});
