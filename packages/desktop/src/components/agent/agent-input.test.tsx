// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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

/** StandardSelect（base-ui Select）：testid 落 SelectValue span（`data-value` 承载
 * 选中档），trigger 为其外层 role=combobox 按钮。 */
function modeValue(): string | null {
  return screen.getByTestId('explore-permission-mode').getAttribute('data-value');
}

function modeTrigger(): HTMLButtonElement {
  const trigger = screen.getByTestId('explore-permission-mode').closest('button');
  if (!trigger) throw new Error('SelectValue 不在触发器按钮内');
  return trigger;
}

/** 打开档位弹层并点选指定 label：pointerDown 先行满足 base-ui 鼠标选择守卫。 */
async function pickMode(label: string): Promise<void> {
  fireEvent.mouseDown(modeTrigger());
  const listbox = await screen.findByRole('listbox');
  const option = within(listbox).getByRole('option', { name: label });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
  await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());
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

  it('permission-mode 三档切换生效，默认档 bypassPermissions', async () => {
    const onSend = vi.fn();
    mount({ onSend });

    expect(modeValue()).toBe('bypassPermissions');

    await pickMode('acceptEdits');
    typeText('换成自动接受编辑');
    fireEvent.click(screen.getByTestId('explore-send'));

    expect(onSend).toHaveBeenCalledWith({
      text: '换成自动接受编辑',
      permissionMode: 'acceptEdits',
    });
  });

  it('档位切换到 default 后发送携带 default（三档均可选）', async () => {
    const onSend = vi.fn();
    mount({ onSend });

    await pickMode('default');
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

  it('running 不再由组件禁发：发送入口可点并照常上抛（重复发送由会话层 isStreaming 守卫忽略）', () => {
    const onSend = vi.fn();
    mount({ running: true, onSend });

    typeText('运行中输入');
    const send = screen.getByTestId<HTMLButtonElement>('explore-send');
    expect(send.disabled).toBe(false);
    fireEvent.click(send);
    expect(onSend).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// 键盘快捷键：Ctrl/Cmd+Enter 发送（onKeyUp 触发；修饰键缺失 / 其他键不发送）
// ---------------------------------------------------------------------------

describe('AgentInput：Ctrl/Cmd+Enter 快捷发送', () => {
  it('Ctrl+Enter → onSend 恰一次并清空输入（与点击发送同语义）', () => {
    const onSend = vi.fn();
    mount({ onSend });

    typeText('快捷键发送');
    fireEvent.keyUp(promptBox(), { key: 'Enter', ctrlKey: true });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith({
      text: '快捷键发送',
      permissionMode: 'bypassPermissions',
    });
    expect(promptBox().value).toBe('');
  });

  it('Meta+Enter 不触发（实现只认 ctrlKey，macOS 的 Cmd 当前不接线）', () => {
    const onSend = vi.fn();
    mount({ onSend });

    typeText('Cmd 回车');
    fireEvent.keyUp(promptBox(), { key: 'Enter', metaKey: true });

    expect(onSend).not.toHaveBeenCalled();
  });

  it('裸 Enter 不触发（保留多行输入）', () => {
    const onSend = vi.fn();
    mount({ onSend });

    typeText('多行\n输入');
    fireEvent.keyUp(promptBox(), { key: 'Enter' });
    fireEvent.keyDown(promptBox(), { key: 'Enter' });

    expect(onSend).not.toHaveBeenCalled();
    expect(promptBox().value).toBe('多行\n输入');
  });

  it('非 Enter 键 + Ctrl 不触发', () => {
    const onSend = vi.fn();
    mount({ onSend });

    typeText('Ctrl 加别的键');
    fireEvent.keyUp(promptBox(), { key: 'k', ctrlKey: true });
    fireEvent.keyUp(promptBox(), { key: 'a', ctrlKey: true });

    expect(onSend).not.toHaveBeenCalled();
  });

  it('空输入时 Ctrl+Enter 零回调（canSend 守卫先于快捷键）', () => {
    const onSend = vi.fn();
    mount({ onSend });

    fireEvent.keyUp(promptBox(), { key: 'Enter', ctrlKey: true });

    expect(onSend).not.toHaveBeenCalled();
  });

  it('整面板禁用时 Ctrl+Enter 零回调', () => {
    const onSend = vi.fn();
    mount({ disabled: true, disabledReason: '归档只读', onSend });

    typeText('禁用下快捷发送');
    fireEvent.keyUp(promptBox(), { key: 'Enter', ctrlKey: true });

    expect(onSend).not.toHaveBeenCalled();
  });

  it('快捷键沿用当前档位：切到 default 后 Ctrl+Enter 携 default', async () => {
    const onSend = vi.fn();
    mount({ onSend });

    await pickMode('default');
    typeText('默认档快捷发送');
    fireEvent.keyUp(promptBox(), { key: 'Enter', ctrlKey: true });

    expect(onSend).toHaveBeenCalledWith({ text: '默认档快捷发送', permissionMode: 'default' });
  });
});

// ---------------------------------------------------------------------------
// 边界：空文本与 testid 前缀化
// ---------------------------------------------------------------------------

describe('AgentInput：边界', () => {
  it('空文本时发送禁用；纯空白按非空处理（trim 守卫不在组件侧）', () => {
    mount({});

    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);

    typeText('   ');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(false);
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
// 整面板禁用态（D14）：`disabled` 与 `running` 语义正交——输入与发送同禁、理由
// 文案呈于工具行上方；停止入口只随 `running` 呈现（禁用不隐去运行中的停止面）。
// ---------------------------------------------------------------------------

describe('AgentInput：整面板禁用态', () => {
  it('禁用态全量呈现：输入与发送同禁 + 理由行文本即传入理由', () => {
    mount({ disabled: true, disabledReason: '流程运行中' });

    expect(promptBox().disabled).toBe(true);
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe('流程运行中');
  });

  it('禁用态不隐去运行中停止面：running 且禁用 → 停止入口仍在、理由行在场', () => {
    mount({ disabled: true, running: true, disabledReason: '已归档 change 只读' });

    expect(screen.getByTestId('explore-stop') !== null).toBe(true);
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe('已归档 change 只读');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);
  });

  it('running 语义回归：未禁用时停止入口 / 输入可编辑；非 running 时可发送', () => {
    const runningView = mount({ disabled: false, running: true });
    expect(screen.getByTestId('explore-stop')).toBeTruthy();
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

  it('禁用不清空既有文本与档位：先输入再切禁用 → 文本与档位保留', async () => {
    const { rendered } = mount({ disabled: false });
    typeText('先输入的文本');
    await pickMode('default');

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
    expect(modeValue()).toBe('default');
    expect(screen.getByTestId('explore-disabled-reason').textContent).toBe('流程运行中');
  });

  it('既有发送守卫回归：未禁用时空串禁发、非空可发', () => {
    mount({ disabled: false });

    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(true);

    typeText('有效文本');
    expect(screen.getByTestId('explore-send').hasAttribute('disabled')).toBe(false);
  });
});
