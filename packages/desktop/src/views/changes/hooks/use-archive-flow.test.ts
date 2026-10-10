import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ArchivePreflight,
  ArchiveSnapshot,
  ArchiveStageState,
  ArchiveUpdate,
} from '../../../types/dto';
import { useArchiveFlow } from './use-archive-flow';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（五命令 archiveFlowPreflight / Start /
// Stop / State / Watch，可切换 resolve / reject 并记录入参）；Channel mock 为
// 可编程 class（捕获 onmessage，测试直接投递 ArchiveUpdate 信封驱动归并）。
// archive-state 纯 reducer 不 mock：真实实现参与归并（内部模块不 mock）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式，绑定类型为事实源）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\archive';
const CHANGE_ID = 'archive-demo';

function stageRow(
  stage: ArchiveStageState['stage'],
  status: ArchiveStageState['status'] = 'running',
): ArchiveStageState {
  return { stage, status, detail: null };
}

function snapshot(overrides: Partial<ArchiveSnapshot> = {}): ArchiveSnapshot {
  return {
    stages: [stageRow('preflight', 'passed'), stageRow('specSync', 'running')],
    sessionId: 'sess-restore',
    ...overrides,
  };
}

function preflightData(overrides: Partial<ArchivePreflight> = {}): ArchivePreflight {
  return {
    name: CHANGE_ID,
    completed: true,
    incompletePhases: [],
    missingArtifacts: [],
    deltaSpecs: ['cap-a'],
    worktree: null,
    branch: null,
    mergeTarget: null,
    runActive: false,
    ...overrides,
  };
}

function stageUpdate(
  stage: ArchiveStageState['stage'],
  status?: ArchiveStageState['status'],
): ArchiveUpdate {
  return { ipc: 'stage', stage: stageRow(stage, status) };
}

function finishedUpdate(): ArchiveUpdate {
  return {
    ipc: 'finished',
    summary: {
      name: CHANGE_ID,
      archivedDir: `2026-10-08-${CHANGE_ID}`,
      specs: 'synced',
      warnings: [],
    },
    error: null,
  };
}

// ---------------------------------------------------------------------------
// 可编程 IPC：五命令按名分发（reject 面以字符串注入 = reject 文本）
// ---------------------------------------------------------------------------

let stateResult: ArchiveSnapshot | string | null | undefined;
let preflightResult: ArchivePreflight | string | null | undefined;
let startReject: string | null = null;
let stopReject: string | null = null;

function mockIpc() {
  ChannelMock.instances.length = 0;
  stateResult = null;
  preflightResult = undefined;
  startReject = null;
  stopReject = null;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archive_flow_state') {
      return typeof stateResult === 'string'
        ? Promise.reject(stateResult)
        : Promise.resolve(stateResult ?? null);
    }
    if (command === 'archive_flow_preflight') {
      return typeof preflightResult === 'string'
        ? Promise.reject(preflightResult)
        : Promise.resolve(preflightResult ?? null);
    }
    if (command === 'archive_flow_start') {
      return startReject === null ? Promise.resolve(true) : Promise.reject(startReject);
    }
    if (command === 'archive_flow_stop') {
      return stopReject === null ? Promise.resolve(null) : Promise.reject(stopReject);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

interface ChannelLike {
  onmessage: ((event: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('订阅未建立：无 Channel 实例');
  return instance;
}

/** 投递 ArchiveUpdate 信封（经最新 Channel 实例）。 */
function deliver(update: ArchiveUpdate) {
  act(() => {
    lastChannel().onmessage?.(update);
  });
}

function calls(command: string): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}

async function mounted(params?: { root: string | null; id: string | null }) {
  const onFinish = vi.fn();
  const rendered = renderHook(
    (input: { root: string | null; id: string | null }) => useArchiveFlow({ ...input, onFinish }),
    {
      initialProps: params ?? { root: ROOT, id: CHANGE_ID },
    },
  );
  await act(async () => {});
  return { ...rendered, onFinish };
}

describe('useArchiveFlow：preflight 动作（确认对话取数面）', () => {
  it('调 preflight → invoke archiveFlowPreflight 以 (root, id) 透传，resolve 数据返回', async () => {
    preflightResult = preflightData({ mergeTarget: 'main' });
    const { result } = await mounted();

    let resolved: ArchivePreflight | null = null;
    await act(async () => {
      resolved = await result.current.preflight();
    });

    expect(invokeMock).toHaveBeenCalledWith('archive_flow_preflight', {
      root: ROOT,
      id: CHANGE_ID,
    });
    expect(resolved).toEqual(preflightData({ mergeTarget: 'main' }));
  });

  it('preflight reject → null（读面兜底，不炸对话）', async () => {
    preflightResult = 'db 打开失败';
    const { result } = await mounted();
    let resolved: ArchivePreflight | null = null;
    await act(async () => {
      resolved = await result.current.preflight();
    });
    expect(resolved).toBeNull();
  });
});

describe('useArchiveFlow：start 与事件归并', () => {
  it('start(true) → invoke archiveFlowStart 参数序（channel, root, id, true）；信封到达 → state 经 reducer 归并', async () => {
    const { result } = await mounted();

    await act(async () => {
      await result.current.start(true);
    });

    expect(calls('archive_flow_start')).toHaveLength(1);
    const args = calls('archive_flow_start')[0][1] as Record<string, unknown>;
    expect(args.root).toBe(ROOT);
    expect(args.id).toBe(CHANGE_ID);
    expect(args.syncSpecs).toBe(true);
    expect(args.onEvent).toBeDefined();
    // 发起后空阶段表起步；Channel 信封到达 → 阶段推进
    expect(result.current.state?.stages).toEqual({});
    deliver(stageUpdate('preflight', 'passed'));
    expect(result.current.state?.stages.preflight).toMatchObject({ status: 'passed' });
    deliver(stageUpdate('specSync', 'running'));
    expect(result.current.state?.stages.specSync).toMatchObject({ status: 'running' });
  });

  it('start(false) → syncSpecs=false 透传（跳过同步 checkbox 对译）', async () => {
    const { result } = await mounted();
    await act(async () => {
      await result.current.start(false);
    });
    const args = calls('archive_flow_start')[0][1] as Record<string, unknown>;
    expect(args.syncSpecs).toBe(false);
  });
});

describe('useArchiveFlow：终态 onFinish 恰一次', () => {
  it('Finished 信封到达 → onFinish 恰一次；此后迟滞信封不重复触发', async () => {
    const { result, onFinish } = await mounted();
    await act(async () => {
      await result.current.start(true);
    });

    deliver(finishedUpdate());
    await waitFor(() => expect(onFinish).toHaveBeenCalledTimes(1));
    expect(result.current.state?.finished).toBe(true);
    expect(result.current.state?.summary?.archivedDir).toBe(`2026-10-08-${CHANGE_ID}`);

    // 迟滞信封（reducer 冻结 + ref 防抖）不重复触发
    deliver(stageUpdate('finalize', 'failed'));
    deliver(finishedUpdate());
    expect(onFinish).toHaveBeenCalledTimes(1);
  });
});

describe('useArchiveFlow：挂载快照恢复与补订', () => {
  it('mount 时 state 查询 Some（非终态）→ seed + archiveFlowWatch 补订', async () => {
    stateResult = snapshot();
    const { result } = await mounted();
    await waitFor(() => expect(calls('archive_flow_watch')).toHaveLength(1));

    expect(invokeMock).toHaveBeenCalledWith('archive_flow_state', { root: ROOT, id: CHANGE_ID });
    expect(result.current.state?.stages.preflight).toMatchObject({ status: 'passed' });
    expect(result.current.state?.sessionId).toBe('sess-restore');

    // 补订信道后续信封照常归并（重挂恢复 + live 并流）
    deliver(stageUpdate('specSync', 'passed'));
    expect(result.current.state?.stages.specSync).toMatchObject({ status: 'passed' });
  });

  it('mount 时 state 查询 None → 零 Channel 零 watch（常态路径零订阅）', async () => {
    stateResult = null;
    await mounted();
    expect(calls('archive_flow_state')).toHaveLength(1);
    expect(ChannelMock.instances).toHaveLength(0);
    expect(calls('archive_flow_watch')).toHaveLength(0);
  });

  it('mount 时 state 查询 reject → 空态降级（不阻断页面）', async () => {
    stateResult = 'db 打开失败';
    const { result } = await mounted();
    await waitFor(() => expect(result.current.state).toBeNull());
    expect(calls('archive_flow_watch')).toHaveLength(0);
  });
});

describe('useArchiveFlow：stop 与 error 面', () => {
  it('stop → invoke archiveFlowStop；reject → error 态（string 口径归一）', async () => {
    stopReject = '非法 root';
    const { result } = await mounted();

    await act(async () => {
      await result.current.stop();
    });
    expect(calls('archive_flow_stop')).toHaveLength(1);
    expect(result.current.error).toBe('非法 root');
  });

  it('发起先行清错：失败 stop 落错后再 start（成功）→ error 归 null', async () => {
    stopReject = '非法 root';
    const { result } = await mounted();
    await act(async () => {
      await result.current.stop();
    });
    expect(result.current.error).toBe('非法 root');

    await act(async () => {
      await result.current.start(true);
    });
    expect(result.current.error).toBeNull();
  });

  it('start reject → error 态携带命令面拒绝串', async () => {
    startReject = 'change "x" 的归档链进行中，不可重复发起';
    const { result } = await mounted();
    await act(async () => {
      await result.current.start(true);
    });
    expect(result.current.error).toContain('归档链进行中');
  });
});

describe('useArchiveFlow：参数未就绪 no-op', () => {
  it('root / change null → 各动作零 invoke', async () => {
    const { result, onFinish } = await mounted({ root: null, id: null });

    const preflighted = await act(async () => result.current.preflight());
    await act(async () => {
      await result.current.start(true);
      await result.current.stop();
    });

    expect(preflighted).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(ChannelMock.instances).toHaveLength(0);
    expect(onFinish).not.toHaveBeenCalled();
    expect(result.current.state).toBeNull();
  });
});
