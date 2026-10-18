// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ExploreComposer } from './explore-composer';

// ---------------------------------------------------------------------------
// 无内部模块 mock（Mock策略表）：`AgentInput` 真实组合；`onSend` / `onStop` 用
// `vi.fn()` 观测（props 为显式入参）。
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.restoreAllMocks();
});

function renderComposer(overrides: Partial<Parameters<typeof ExploreComposer>[0]> = {}) {
  const onSend = vi.fn();
  const onStop = vi.fn();
  const props = { running: false, onSend, onStop, ...overrides };
  const rendered = render(<ExploreComposer {...props} />);
  return { ...rendered, onSend, onStop };
}

function type(text: string): void {
  fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: text } });
}

describe('ExploreComposer：发送与停止（正向）', () => {
  it('running = false：输入 + 提交 → onSend({prompt, permissionMode}) 逐字，缺省 bypassPermissions，停止入口零触发', () => {
    const { onSend, onStop } = renderComposer();

    type('想聊清楚什么？');
    fireEvent.click(screen.getByTestId('explore-send'));

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith({
      prompt: '想聊清楚什么？',
      permissionMode: 'bypassPermissions',
    });
    expect(onStop).not.toHaveBeenCalled();
    expect(screen.queryByTestId('explore-stop')).toBeNull();
  });

  it('running = true：发送禁用（点击零 onSend）、停止入口可点 → onStop 恰一次', () => {
    const { onSend, onStop } = renderComposer({ running: true });

    type('运行中不该发');
    const sendButton = screen.getByTestId<HTMLButtonElement>('explore-send');
    expect(sendButton.disabled).toBe(true);
    fireEvent.click(sendButton);
    expect(onSend).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('explore-stop'));
    expect(onStop).toHaveBeenCalledTimes(1);
  });
});

describe('ExploreComposer：边界（空入参与档位）', () => {
  it('空 prompt / 纯空白提交 → 发送禁用、零 onSend', () => {
    const { onSend } = renderComposer();
    const sendButton = screen.getByTestId<HTMLButtonElement>('explore-send');

    expect(sendButton.disabled).toBe(true);
    fireEvent.click(sendButton);
    expect(onSend).not.toHaveBeenCalled();

    type('   ');
    expect(sendButton.disabled).toBe(true);
    fireEvent.click(sendButton);
    type('\n\t ');
    expect(sendButton.disabled).toBe(true);
    fireEvent.click(sendButton);
    expect(onSend).not.toHaveBeenCalled();
  });

  it('permission-mode 三档切换后提交，档位逐档透传', () => {
    const { onSend } = renderComposer();
    const select = screen.getByTestId<HTMLSelectElement>('explore-permission-mode');

    for (const mode of ['acceptEdits', 'default', 'bypassPermissions'] as const) {
      fireEvent.change(select, { target: { value: mode } });
      type(`档位 ${mode}`);
      fireEvent.click(screen.getByTestId('explore-send'));
      expect(onSend).toHaveBeenLastCalledWith({ prompt: `档位 ${mode}`, permissionMode: mode });
    }
    expect(onSend).toHaveBeenCalledTimes(3);
  });

  it('testid 前缀保持：explore-composer / explore-prompt / explore-send / explore-permission-mode', () => {
    renderComposer({ running: true });

    expect(screen.getByTestId('explore-composer')).toBeTruthy();
    expect(screen.getByTestId('explore-prompt')).toBeTruthy();
    expect(screen.getByTestId('explore-send')).toBeTruthy();
    expect(screen.getByTestId('explore-stop')).toBeTruthy();
    expect(screen.getByTestId('explore-permission-mode')).toBeTruthy();
    // 文案面留在来源侧（探索页 label / placeholder）
    expect(screen.getByTestId('explore-prompt').getAttribute('placeholder')).toContain(
      '想聊清楚什么',
    );
  });
});

describe('ExploreComposer：异常（回调抛错与插槽边界）', () => {
  it('onSend 回 rejected promise → 组件不 await 不崩、输入值不清空、可再次提交', () => {
    // 回调失败经宿主侧（来源 hook）承担：组件只上抛不阻塞
    const failure = Promise.reject(new Error('发送失败：注入'));
    void failure.catch(() => {});
    const onSend = vi.fn(() => failure);
    renderComposer({ onSend });

    type('会被拒绝的一发');
    fireEvent.click(screen.getByTestId('explore-send'));
    expect(onSend).toHaveBeenCalledTimes(1);
    // 组件未崩、输入值照常清空（发送成功路径语义不受回调节奏影响）
    expect(screen.getByTestId<HTMLTextAreaElement>('explore-prompt').value).toBe('');

    // 可再次提交（停止入口 / 发送入口不失活）
    type('第二发');
    fireEvent.click(screen.getByTestId('explore-send'));
    expect(onSend).toHaveBeenCalledTimes(2);
  });

  it('onStop 回 rejected promise → 停止入口不因之失活（再次点击仍触发）', () => {
    const failure = Promise.reject(new Error('停止失败：注入'));
    void failure.catch(() => {});
    const onStop = vi.fn(() => failure);
    renderComposer({ running: true, onStop });

    fireEvent.click(screen.getByTestId('explore-stop'));
    expect(onStop).toHaveBeenCalledTimes(1);
    // 停止入口仍在（外驱 running 未变）→ 可再次触发
    fireEvent.click(screen.getByTestId('explore-stop'));
    expect(onStop).toHaveBeenCalledTimes(2);
  });

  it('running 为 undefined（AgentSessionActions 未就绪的插槽边界）→ 按未运行处理：发送可用、停止隐藏', () => {
    const { onSend } = renderComposer({ running: undefined as unknown as boolean });

    type('边界一发');
    const sendButton = screen.getByTestId<HTMLButtonElement>('explore-send');
    expect(sendButton.disabled).toBe(false);
    fireEvent.click(sendButton);
    expect(onSend).toHaveBeenCalledWith({
      prompt: '边界一发',
      permissionMode: 'bypassPermissions',
    });
    expect(screen.queryByTestId('explore-stop')).toBeNull();
  });
});
