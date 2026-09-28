// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AgentPermissionMode } from '../../types/dto';
import { AgentInput } from './agent-input';

// ---------------------------------------------------------------------------
// 纯渲染组件，回调经 props 注入，无进程边界，不需要 Mock。
// ---------------------------------------------------------------------------

function mount(
  overrides: {
    idPrefix?: string;
    running?: boolean;
    onSend?: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
    onStop?: () => void;
  } = {},
) {
  const onSend = overrides.onSend ?? vi.fn();
  const onStop = overrides.onStop ?? vi.fn();
  const rendered = render(
    <AgentInput
      idPrefix={overrides.idPrefix ?? 'explore'}
      label="探索输入"
      placeholder="要探索什么？"
      running={overrides.running ?? false}
      onSend={onSend}
      onStop={onStop}
    />,
  );
  return { onSend, onStop, rendered };
}

function typeText(text: string) {
  fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: text } });
}

function promptBox(): HTMLTextAreaElement {
  return screen.getByTestId('explore-prompt');
}

function modeSelect(): HTMLSelectElement {
  return screen.getByTestId('explore-permission-mode');
}

// ---------------------------------------------------------------------------
// 发送 / 档位 / 停止入口
// ---------------------------------------------------------------------------

describe('AgentInput：发送与档位', () => {
  it('输入文本后点击发送，回调携带 { text, permissionMode }，输入清空', () => {
    const onSend = vi.fn();
    mount({ onSend });

    typeText('追踪重试逻辑');
    fireEvent.click(screen.getByTestId('explore-send'));

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith({
      text: '追踪重试逻辑',
      permissionMode: 'bypassPermissions',
    });
    expect(promptBox().value).toBe('');
  });

  it('permission-mode 三档切换生效，默认档 bypassPermissions', () => {
    const onSend = vi.fn();
    mount({ onSend });

    expect(modeSelect().value).toBe('bypassPermissions');

    fireEvent.change(modeSelect(), { target: { value: 'acceptEdits' } });
    typeText('换成自动接受编辑');
    fireEvent.click(screen.getByTestId('explore-send'));

    expect(onSend).toHaveBeenCalledWith({
      text: '换成自动接受编辑',
      permissionMode: 'acceptEdits',
    });
  });

  it('档位切换到 default 后发送携带 default（三档均可选）', () => {
    const onSend = vi.fn();
    mount({ onSend });

    fireEvent.change(screen.getByTestId('explore-permission-mode'), {
      target: { value: 'default' },
    });
    typeText('默认档');
    fireEvent.click(screen.getByTestId('explore-send'));

    expect(onSend).toHaveBeenCalledWith({ text: '默认档', permissionMode: 'default' });
  });
});

describe('AgentInput：停止入口与禁发', () => {
  it('运行中呈现停止入口，点击触发 onStop 回调', () => {
    const onStop = vi.fn();
    mount({ running: true, onStop });

    expect(screen.getByTestId('explore-stop') !== null).toBe(true);
    fireEvent.click(screen.getByTestId('explore-stop'));
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it('非运行态无停止入口', () => {
    mount({ running: false });

    expect(screen.queryByTestId('explore-stop')).toBeNull();
  });

  it('running 时发送按钮禁用，点击不触发回调', () => {
    const onSend = vi.fn();
    mount({ running: true, onSend });

    typeText('运行中输入');
    const send = screen.getByTestId('explore-send');
    expect(send.hasAttribute('disabled')).toBe(true);
    fireEvent.click(send);
    expect(onSend).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 边界：空文本与 testid 前缀化
// ---------------------------------------------------------------------------

describe('AgentInput：边界', () => {
  it('空文本（含纯空白）时发送禁用', () => {
    mount({});

    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);

    typeText('   ');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
  });

  it('经 idPrefix 注入的前缀应用到输入、档位下拉与按钮的 id / data-testid', () => {
    mount({ idPrefix: 'debug', running: true });

    // 输入框：id 与 testid 同为前缀化产物
    const prompt = screen.getByTestId('debug-prompt');
    expect(prompt.id).toBe('debug-prompt');
    expect(screen.getByLabelText('探索输入').id).toBe('debug-prompt');
    // 档位下拉与按钮
    const mode = screen.getByTestId('debug-permission-mode');
    expect(mode.id).toBe('debug-permission-mode');
    expect(screen.getByTestId('debug-send') !== null).toBe(true);
    expect(screen.getByTestId('debug-stop') !== null).toBe(true);
    // composer 容器亦前缀化
    expect(screen.getByTestId('debug-composer') !== null).toBe(true);
  });
});
