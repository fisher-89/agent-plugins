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
    /** 整面板禁用态（缺省 false：既有用例零回归） */
    disabled?: boolean;
    /** 禁用理由文案（缺省不呈现） */
    disabledReason?: string;
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
      disabled={overrides.disabled}
      disabledReason={overrides.disabledReason}
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

// ---------------------------------------------------------------------------
// 整面板禁用态（D14）：`disabled` 与 `running` 语义正交——输入与发送同禁、不呈
// 停止入口、理由文案呈于工具行上方；`disabled === false` 时既有 running 语义
// 逐字不变（上方三例即回归锚）。
// ---------------------------------------------------------------------------

describe('AgentInput：整面板禁用态', () => {
  it('禁用态全量呈现：输入与发送同禁 + 理由行文本即传入理由', () => {
    mount({ disabled: true, disabledReason: '流程运行中' });

    expect(promptBox().disabled).toBe(true);
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe('流程运行中');
  });

  it('禁用态不顶替 running 呈现：running 且禁用 → 无停止入口、理由行在场', () => {
    mount({ disabled: true, running: true, disabledReason: '已归档 change 只读' });

    expect(screen.queryByTestId('explore-stop')).toBeNull();
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe('已归档 change 只读');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
  });

  it('running 语义回归：未禁用时停止入口 / 发送禁用 / 输入可编辑；非 running 时可发送', () => {
    const runningView = mount({ disabled: false, running: true });
    expect(screen.getByTestId('explore-stop')).toBeTruthy();
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
    expect(promptBox().disabled).toBe(false);
    expect(screen.queryByTestId('explore-disabled-reason')).toBeNull();
    runningView.rendered.unmount();

    mount({ disabled: false, running: false });
    expect(screen.queryByTestId('explore-stop')).toBeNull();
    typeText('可发送');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(false);
  });

  it('禁用不止视觉：点击发送与表单提交均零 onSend', () => {
    const onSend = vi.fn();
    mount({ disabled: true, disabledReason: '流程运行中', onSend });

    // 文本经 DOM 直填（禁用的 textarea 不接受 change 事件，模拟既有文本）
    fireEvent.change(promptBox(), { target: { value: '已有文本' } });
    fireEvent.click(screen.getByTestId('explore-send'));
    fireEvent.submit(screen.getByTestId('explore-composer'));

    expect(onSend).not.toHaveBeenCalled();
  });

  it('禁用且无理由文案 → 输入与发送仍禁用、理由行不在场（不渲染空占位）', () => {
    mount({ disabled: true });

    expect(promptBox().disabled).toBe(true);
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
    expect(screen.queryByTestId('explore-disabled-reason')).toBeNull();
  });

  it('空理由串 → 理由行在场且不含字面 undefined、不抛错', () => {
    expect(() => mount({ disabled: true, disabledReason: '' })).not.toThrow();

    const reason = screen.getByTestId('explore-disabled-reason');
    expect(reason.textContent).toBe('');
    expect(reason.textContent).not.toContain('undefined');
  });

  it('超长与特殊字符理由原样呈现（不截断、按文本节点转义不注入 HTML）', () => {
    const longReason = '原因'.repeat(400);
    const longView = mount({ disabled: true, disabledReason: longReason });
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe(longReason);
    longView.rendered.unmount();

    mount({ disabled: true, disabledReason: '流程运行中\n🚀<b>粗体</b>' });
    const reason = screen.getByTestId('explore-disabled-reason');
    expect(reason.textContent).toBe('流程运行中\n🚀<b>粗体</b>');
    expect(reason.querySelector('b'), '文本节点转义：不注入 HTML').toBeNull();
  });

  it('禁用不清空既有文本与档位：先输入再切禁用 → 文本与档位保留', () => {
    const { rendered } = mount({ disabled: false });
    typeText('先输入的文本');
    fireEvent.change(modeSelect(), { target: { value: 'default' } });

    rendered.rerender(
      <AgentInput
        idPrefix="explore"
        label="探索输入"
        placeholder="要探索什么？"
        running={false}
        disabled
        disabledReason="流程运行中"
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );

    expect(promptBox().value).toBe('先输入的文本');
    expect(modeSelect().value).toBe('default');
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe('流程运行中');
  });

  it('既有发送守卫回归：未禁用时纯空白禁发、非空可发', () => {
    mount({ disabled: false });

    typeText('   ');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);

    typeText('有效文本');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(false);
  });
});
