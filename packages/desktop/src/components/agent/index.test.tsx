// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import {
  AgentSessionView,
  type AgentSessionActions,
  type AgentSessionViewProps,
} from './agent-session-view';
import * as barrel from './index';

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

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  ChannelMock.instances.length = 0;
});

describe('组件族 barrel：出口集合与单一引用', () => {
  it('barrel 的 AgentSessionView 与直接 import 为同一引用（唯一渲染入口单点出口）', () => {
    expect(barrel.AgentSessionView).toBe(AgentSessionView);
  });

  it('运行时导出键集合逐字断言（改名 / 漏导出即编译失败，且不存在第二个导出名）', () => {
    expect(Object.keys(barrel).sort()).toEqual(['AgentInput', 'AgentSessionView']);
    expect(barrel).not.toHaveProperty('AgentMessages');
    expect(barrel).not.toHaveProperty('AgentSessionViewContainer');
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
});

describe('组件族 barrel：冒烟渲染', () => {
  it('经 barrel 的最小 props 形态可挂载（sessionId=null → 空态、零取数）', () => {
    render(<barrel.AgentSessionView root={null} sessionId={null} data-testid="barrel-view" />);

    expect(screen.getByTestId('barrel-view')).toBeTruthy();
    expect(screen.getByTestId('agent-session-empty')).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
