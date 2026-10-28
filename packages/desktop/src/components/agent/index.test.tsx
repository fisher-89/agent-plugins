// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ResizablePanelGroup } from '@/components/ui/resizable';
import type { UseAgentSessionState } from '@/hooks/use-agent-session';

import {
  AgentSessionView,
  type AgentSessionActions,
  type AgentSessionViewProps,
} from './agent-session-view';
import * as barrel from './index';
import { SessionPanel, type SessionPanelState } from './session-panel';

// ---------------------------------------------------------------------------
// 进程边界 Mock：barrel 冒烟渲染只需最小 invoke / Channel（sessionId=null 形态
// 零取数）；内部模块不 mock（barrel 即 re-export，直接经真实模块组合）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

/** 冒烟渲染用的最小会话面（注入形态：空消息 / 空事件 / 无会话）。 */
function emptySession(): UseAgentSessionState {
  return {
    messages: [],
    events: [],
    session: null,
    chain: [],
    loading: false,
    running: false,
    error: null,
    sendMessage: vi.fn(),
    stop: vi.fn(),
    reset: vi.fn(),
  };
}

/** 冒烟渲染用的最小骨架状态（空清单 / 空态会话 / note=null）。 */
function emptyPanel(): SessionPanelState {
  return {
    session: emptySession(),
    sessions: [],
    total: 0,
    listOpen: false,
    listLoading: false,
    listError: null,
    toggleList: vi.fn(),
    loadMore: vi.fn(),
    newSession: vi.fn(),
    selectSession: vi.fn(),
    canStart: true,
    note: null,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  ChannelMock.instances.length = 0;
});

describe('组件族 barrel：出口集合与单一引用', () => {
  it('barrel 的 AgentSessionView 与直接 import 为同一引用（唯一渲染入口单点出口）', () => {
    expect(barrel.AgentSessionView).toBe(AgentSessionView);
  });

  it('barrel 的 SessionPanel 与直接 import 为同一引用（不建第二条面板实现）', () => {
    expect(barrel.SessionPanel).toBe(SessionPanel);
  });

  it('运行时导出键集合逐字断言（改名 / 漏导出即红，且不存在第二个导出名）', () => {
    expect(Object.keys(barrel).sort()).toEqual(['AgentInput', 'AgentSessionView', 'SessionPanel']);
    expect(barrel).not.toHaveProperty('AgentMessages');
    expect(barrel).not.toHaveProperty('AgentSessionViewContainer');
    expect(barrel, 'SessionPanelProps 留文件内、不出桶').not.toHaveProperty('SessionPanelProps');
    expect(barrel, '不存在第二面板实现名').not.toHaveProperty('SessionPanelV2');
  });

  it('既有 AgentInput 出口不变（消费者 import 路径零改动）', () => {
    expect(typeof barrel.AgentInput).toBe('function');
  });

  it('类型出口可自 barrel 导入（props 与 composer 插槽 actions 随组件同出口）', () => {
    // 编译期锚定：类型仅用于类型位置，运行期以构造形态确保引用非空
    const props: AgentSessionViewProps = { root: null, sessionId: null };
    const actions: AgentSessionActions = {
      running: false,
      loading: false,
      send: () => {},
      stop: () => {},
    };
    expect(props.sessionId).toBeNull();
    expect(actions.running).toBe(false);
  });

  it('SessionPanelState 可自 barrel 导入并在类型位置使用（骨架契约的类型出口）', () => {
    const state: barrel.SessionPanelState = emptyPanel();
    expect(state.canStart).toBe(true);
    expect(state.note).toBeNull();
    expect(state.sessions).toEqual([]);
    // 12 字段逐字段在场（编译期锚定 + 运行期可读）
    for (const key of [
      'session',
      'sessions',
      'total',
      'listOpen',
      'listLoading',
      'listError',
      'toggleList',
      'loadMore',
      'newSession',
      'selectSession',
      'canStart',
      'note',
    ] as const) {
      expect(state, `骨架契约字段 ${key} 在场`).toHaveProperty(key);
    }
  });
});

describe('组件族 barrel：冒烟渲染', () => {
  it('经 barrel 的最小 props 形态可挂载（sessionId=null → 空态、零取数）', () => {
    render(<barrel.AgentSessionView root={null} sessionId={null} data-testid="barrel-view" />);

    expect(screen.getByTestId('barrel-view')).toBeTruthy();
    expect(screen.getByTestId('agent-session-empty')).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('经 barrel 的 SessionPanel 可挂载（前缀生成查询挂钩、零取数）', () => {
    render(
      <ResizablePanelGroup orientation="horizontal">
        <barrel.SessionPanel
          state={emptyPanel()}
          title="架构 Agent"
          testIdPrefix="barrel"
          composer={() => <div data-testid="barrel-composer" />}
        />
      </ResizablePanelGroup>,
    );
    // 骨架默认收起为窄条：前缀生成的展开入口在场，零取数
    expect(screen.getByTestId('barrel-rail')).toBeTruthy();
    expect(screen.getByTestId('barrel-expand')).toBeTruthy();
    expect(invokeMock, '骨架不自取数（注入形态零 invoke）').not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('barrel-expand'));
    expect(screen.getByTestId('barrel-session')).toBeTruthy();
    expect(screen.getByTestId('barrel-composer')).toBeTruthy();
    expect(invokeMock, '展开亦零 invoke').not.toHaveBeenCalled();
  });
});
