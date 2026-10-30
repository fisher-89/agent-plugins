import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { ArchiChatComposer, type ArchiChatComposerProps } from './archi-chat-composer';

// ---------------------------------------------------------------------------
// 装置：`AgentInput` 真实组合（薄适配的可观察后果即 `AgentInput` 的呈面语义），
// 回调为 `vi.fn()` 观测；composer 不取数，故无 IPC mock（零依赖）。
// ---------------------------------------------------------------------------

const PLACEHOLDER = '问这个 workspace 的架构、让它补模型或修文档…';

function baseProps(overrides: Partial<ArchiChatComposerProps> = {}): ArchiChatComposerProps {
  return {
    running: false,
    disabled: false,
    disabledReason: null,
    onSend: vi.fn(),
    onStop: vi.fn(),
    ...overrides,
  };
}

function promptBox(): HTMLTextAreaElement {
  return screen.getByTestId('archi-chat-prompt') as HTMLTextAreaElement;
}

function sendButton(): HTMLButtonElement {
  return screen.getByTestId('archi-chat-send') as HTMLButtonElement;
}

/** 档位选择器（StandardSelect / base-ui Select）：测试读取 `data-value`。 */
function modeAnchor(): HTMLElement {
  return screen.getByTestId('archi-chat-permission-mode');
}

async function pickMode(mode: string): Promise<void> {
  const trigger = modeAnchor().closest('button');
  if (!trigger) throw new Error('SelectValue 不在触发器按钮内');
  fireEvent.mouseDown(trigger);
  const option = within(await screen.findByRole('listbox')).getByRole('option', { name: mode });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
  await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());
}

describe('ArchiChatComposer：逐 prop 映射', () => {
  it('前缀落到容器 / 输入 / 发送 / 档位，label 与 placeholder 逐字', async () => {
    render(<ArchiChatComposer {...baseProps()} />);

    expect(screen.getByTestId('archi-chat-composer')).toBeTruthy();
    expect(promptBox().getAttribute('placeholder')).toBe(PLACEHOLDER);
    expect(screen.getByText('架构 Agent')).toBeTruthy();
    // 发送入口为图标按钮（可及名由 Tooltip「发送」承载）
    expect(sendButton().getAttribute('data-testid')).toBe('archi-chat-send');

    expect(modeAnchor().getAttribute('data-value')).toBe('bypassPermissions');
    fireEvent.mouseDown(modeAnchor().closest('button')!);
    const options = within(await screen.findByRole('listbox'))
      .getAllByRole('option')
      .map((option) => option.textContent);
    expect(options).toEqual(['bypassPermissions', 'acceptEdits', 'default']);
  });
});

describe('ArchiChatComposer：发送与档位', () => {
  it('输入文本 + 选 default 档 → onSend 恰一次且输入清空', async () => {
    const onSend = vi.fn();
    render(<ArchiChatComposer {...baseProps({ onSend })} />);

    fireEvent.change(promptBox(), { target: { value: '架构现状是什么' } });
    await pickMode('default');
    fireEvent.click(sendButton());

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith({ text: '架构现状是什么', permissionMode: 'default' });
    expect(promptBox().value).toBe('');
  });

  it('未选档直接发送 → 缺省档 bypassPermissions（档位沿用选择器缺省）', () => {
    const onSend = vi.fn();
    render(<ArchiChatComposer {...baseProps({ onSend })} />);

    fireEvent.change(promptBox(), { target: { value: '补一个模型' } });
    fireEvent.click(sendButton());

    expect(onSend).toHaveBeenCalledWith({
      text: '补一个模型',
      permissionMode: 'bypassPermissions',
    });
  });

  it('空输入 → 发送禁用且点击零回调（纯空白按非空处理，trim 守卫不在组件侧）', () => {
    const onSend = vi.fn();
    render(<ArchiChatComposer {...baseProps({ onSend })} />);

    expect(sendButton().disabled, '空串禁用').toBe(true);
    fireEvent.click(sendButton());
    expect(onSend).not.toHaveBeenCalled();

    fireEvent.change(promptBox(), { target: { value: '   ' } });
    expect(sendButton().disabled, '纯空白不禁发').toBe(false);
  });
});

describe('ArchiChatComposer：运行与禁用两标志', () => {
  it('running 且未禁用 → 停止入口在场，点击 onStop 恰一次', () => {
    const onStop = vi.fn();
    render(<ArchiChatComposer {...baseProps({ running: true, onStop })} />);

    const stop = screen.getByTestId('archi-chat-stop');
    fireEvent.click(stop);
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it('整面板禁用 → 输入与发送同时禁用，理由文案逐字（含换行 / 长文案不截断）', () => {
    const longReason = `${'很长的禁用理由'.repeat(60)}\n换行后的第二行`;
    const { rerender } = render(
      <ArchiChatComposer
        {...baseProps({ disabled: true, disabledReason: '本会话运行中，请等待收口' })}
      />,
    );

    expect(promptBox().disabled).toBe(true);
    expect(sendButton().disabled).toBe(true);
    expect(screen.getByTestId('archi-chat-disabled-reason').textContent).toBe(
      '本会话运行中，请等待收口',
    );

    rerender(<ArchiChatComposer {...baseProps({ disabled: true, disabledReason: longReason })} />);
    const rendered = screen.getByTestId('archi-chat-disabled-reason').textContent ?? '';
    expect(rendered).toBe(longReason);
    expect(rendered).toContain('\n');
  });

  it('停止面只随 running 呈现（禁用不隐去）；未禁用时理由不渲（理由随 disabled 才呈）', () => {
    render(<ArchiChatComposer {...baseProps({ running: true, disabled: true })} />);
    expect(screen.getByTestId('archi-chat-stop') !== null, '运行中停止面不被整面板禁用隐去').toBe(
      true,
    );
    expect(
      screen.queryByTestId('archi-chat-disabled-reason'),
      '无理由文案 → 不渲空节点',
    ).toBeNull();

    render(
      <ArchiChatComposer {...baseProps({ disabled: false, disabledReason: '不渲染的理由' })} />,
    );
    expect(screen.queryAllByTestId('archi-chat-disabled-reason')).toHaveLength(0);
  });
});
