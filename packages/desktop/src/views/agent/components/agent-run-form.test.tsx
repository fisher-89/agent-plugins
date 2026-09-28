import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AgentPermissionMode } from '../../../types/dto';
import { AgentRunForm } from './agent-run-form';

// AgentRunForm 为纯回调组件（onStart 以 vi.fn() 注入），无进程边界，不需要 Mock。
// AgentStartInput 形状由 onStart 入参类型承载（'./agent-run-form' 同源导出）。

type AgentStartInput = Parameters<React.ComponentProps<typeof AgentRunForm>['onStart']>[0];

function mount(disabled = false) {
  const onStart = vi.fn();
  render(<AgentRunForm disabled={disabled} onStart={onStart} />);
  return { onStart };
}

function typePrompt(value: string) {
  fireEvent.change(screen.getByTestId('agent-prompt'), { target: { value } });
}

function startButton(): HTMLButtonElement {
  return screen.getByTestId('agent-start');
}

function selectOf(testId: string): HTMLSelectElement {
  return screen.getByTestId(testId);
}

describe('AgentRunForm：参数面默认值与 prompt 必填（AC-5）', () => {
  it('初始 permission-mode=bypassPermissions（默认档断言）', () => {
    mount();

    expect(selectOf('agent-permission-mode').value).toBe('bypassPermissions');
    // bare 认证前提提示初始不在场
    expect(screen.queryByTestId('bare-auth-note')).toBeNull();
  });

  it('prompt 为空串时启动按钮禁用；输入任意非空后启用', () => {
    mount();

    expect(startButton().disabled).toBe(true);
    // 纯空白仍视为空
    typePrompt('   ');
    expect(startButton().disabled).toBe(true);

    typePrompt('你好');
    expect(startButton().disabled).toBe(false);
  });
});

describe('AgentRunForm：onStart 回调与档位切换', () => {
  it('填写后点击启动 → onStart 以 { prompt, permissionMode } 恰调用一次', () => {
    const { onStart } = mount();

    typePrompt('帮我跑一轮');
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledWith({
      prompt: '帮我跑一轮',
      permissionMode: 'bypassPermissions',
    } satisfies AgentStartInput);
  });

  it('permission-mode 三档下拉含 default / acceptEdits / bypassPermissions 且均可选回填', () => {
    mount();

    const select = selectOf('agent-permission-mode');
    const values = Array.from(select.options).map((option) => option.value);
    expect(values).toEqual(['bypassPermissions', 'acceptEdits', 'default']);

    for (const value of values as AgentPermissionMode[]) {
      fireEvent.change(select, { target: { value } });
      expect(select.value).toBe(value);
    }
  });
});

describe('AgentRunForm：禁用与输入保真（边界）', () => {
  it('disabled=true 时启动入口禁用且点击不触发 onStart', () => {
    const { onStart } = mount(true);

    typePrompt('你好');
    expect(startButton().disabled).toBe(true);
    fireEvent.click(startButton());

    expect(onStart).not.toHaveBeenCalled();
  });

  it('prompt 含换行 / emoji / 超长（>1000 字符）时 onStart 原样上抛', () => {
    const { onStart } = mount();
    const longPrompt = `多行 🎉\n"quoted" ${'长'.repeat(1000)}`;

    typePrompt(longPrompt);
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledWith({
      prompt: longPrompt,
      permissionMode: 'bypassPermissions',
    } satisfies AgentStartInput);
  });

  it('无 cwd 输入、无 model 选择（AC-6 缺席断言，D9：cwd 由 invoke 隐含传 root）', () => {
    mount();

    expect(screen.queryByLabelText(/cwd/i)).toBeNull();
    expect(screen.queryByLabelText(/model/i)).toBeNull();
    expect(screen.queryByTestId('agent-cwd')).toBeNull();
    expect(screen.queryByTestId('agent-model')).toBeNull();
  });
});
