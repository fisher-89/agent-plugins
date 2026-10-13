import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { StandardDialog } from './dailog';

// 零进程边界：不 mock 被测组件与 ui/** 底座（base-ui Dialog 真实渲染）；
// onSubmit / onOpenChange 以 vi.fn() 注入观察面；开合态经 ./hooks/use-open 真实组合。

const TITLE = '标准对话框';
const CONTENT = '对话框正文';

/** 触发器（trigger prop 为 JSX 元素，经 DialogTrigger render 落地）。 */
function triggerButton(): React.JSX.Element {
  return <button type="button">打开对话框</button>;
}

/** 正文片段（content prop）。 */
function body(): React.JSX.Element {
  return <p>{CONTENT}</p>;
}

/** 点击触发器开窗（非受控 trigger 档）。 */
function openByTrigger(): void {
  fireEvent.click(screen.getByRole('button', { name: '打开对话框' }));
}

describe('StandardDialog：trigger 双态与弹窗开合', () => {
  it('正向：trigger 在场时点击打开弹窗，title 与 content 同步呈现（关闭态两者缺席）', async () => {
    render(<StandardDialog title={TITLE} content={body()} trigger={triggerButton()} />);

    expect(screen.queryByRole('heading', { name: TITLE })).toBeNull();
    expect(screen.queryByText(CONTENT)).toBeNull();

    openByTrigger();

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
      expect(screen.getByText(CONTENT) !== null).toBe(true);
    });
  });

  it('异常：trigger 缺席（undefined，Optional None 档）：不渲染触发器，受控 open=true 驱动弹窗呈现', () => {
    render(<StandardDialog title={TITLE} content={body()} open />);

    expect(screen.queryByRole('button', { name: '打开对话框' })).toBeNull();
    expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
    expect(screen.getByText(CONTENT) !== null).toBe(true);
  });
});

describe('StandardDialog：footer 双态（onSubmit 在场与否）', () => {
  it('正向：onSubmit 在场 → footer 呈「取消」+「提交」双按钮，「关闭」缺席', async () => {
    render(
      <StandardDialog
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onSubmit={vi.fn<(close: () => void) => void>()}
      />,
    );
    openByTrigger();

    await waitFor(() => {
      expect(screen.getByRole('button', { name: '取消' }) !== null).toBe(true);
      expect(screen.getByRole('button', { name: '提交' }) !== null).toBe(true);
    });
    expect(screen.queryByRole('button', { name: '关闭' })).toBeNull();
  });

  it('异常：onSubmit 缺席（undefined，Optional None 档）→ footer 仅「关闭」单按钮，「提交」「取消」缺席', async () => {
    render(<StandardDialog title={TITLE} content={body()} trigger={triggerButton()} />);
    openByTrigger();

    await waitFor(() => {
      expect(screen.getByRole('button', { name: '关闭' }) !== null).toBe(true);
    });
    expect(screen.queryByRole('button', { name: '提交' })).toBeNull();
    expect(screen.queryByRole('button', { name: '取消' })).toBeNull();
  });
});

describe('StandardDialog：提交关窗与取消 / 关闭零调用（onSubmit(close) 契约）', () => {
  it('正向：点击「提交」→ onSubmit 恰收一次且入参为 close 函数；点击本身不关窗，回调内 close() 后 content 退场', async () => {
    const onSubmit = vi.fn<(close: () => void) => void>();
    render(
      <StandardDialog
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onSubmit={onSubmit}
      />,
    );
    openByTrigger();

    const submit = await screen.findByRole('button', { name: '提交' });
    fireEvent.click(submit);

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith(expect.any(Function));
    // 点击「提交」本身不关窗：content 仍在场
    expect(screen.getByText(CONTENT) !== null).toBe(true);

    const [close] = onSubmit.mock.calls[0];
    act(() => {
      close();
    });
    await waitFor(() => expect(screen.queryByText(CONTENT)).toBeNull());
  });

  it('异常：点击「取消」关窗且 onSubmit 零调用', async () => {
    const onSubmit = vi.fn<(close: () => void) => void>();
    render(
      <StandardDialog
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onSubmit={onSubmit}
      />,
    );
    openByTrigger();

    const cancel = await screen.findByRole('button', { name: '取消' });
    fireEvent.click(cancel);

    await waitFor(() => expect(screen.queryByText(CONTENT)).toBeNull());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('异常：onSubmit 缺席档点击「关闭」关窗（回调缺省，无从调用）', async () => {
    render(<StandardDialog title={TITLE} content={body()} trigger={triggerButton()} />);
    openByTrigger();

    const close = await screen.findByRole('button', { name: '关闭' });
    fireEvent.click(close);

    await waitFor(() => expect(screen.queryByText(CONTENT)).toBeNull());
  });
});

describe('StandardDialog：开合链路（StandardDialog → useOpen）', () => {
  it('链路：非受控 trigger 开、「取消」关，onOpenChange 随开合依次回报 true / false', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>();
    const onSubmit = vi.fn<(close: () => void) => void>();
    render(
      <StandardDialog
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onSubmit={onSubmit}
        onOpenChange={onOpenChange}
      />,
    );

    openByTrigger();
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledTimes(1));
    expect(onOpenChange).toHaveBeenNthCalledWith(1, true);

    const cancel = await screen.findByRole('button', { name: '取消' });
    fireEvent.click(cancel);
    await waitFor(() => expect(screen.queryByText(CONTENT)).toBeNull());

    expect(onOpenChange).toHaveBeenCalledTimes(2);
    expect(onOpenChange).toHaveBeenNthCalledWith(2, false);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('异常：onOpenChange 缺席（undefined，Optional None 档）：开合不抛错（回调缺省安全经 useOpen 透传）', async () => {
    // 与上例同装（onSubmit 在场 → footer 呈「取消」档），仅 onOpenChange 缺席——单变量对照
    render(
      <StandardDialog
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onSubmit={vi.fn<(close: () => void) => void>()}
      />,
    );

    openByTrigger();
    const cancel = await screen.findByRole('button', { name: '取消' });
    fireEvent.click(cancel);

    await waitFor(() => expect(screen.queryByText(CONTENT)).toBeNull());
  });
});

describe('StandardDialog：title 边界（空串 / 特殊字符 / 超长串）', () => {
  it('边界：title 逐档（空串 / "\\n\\0🚀" 特殊字符 / >1000 字符）渲染 title heading 元素在场且文本无损', () => {
    const titles = ['', '第一行\n第二行\0🚀', `长${'标'.repeat(1001)}`];

    for (const title of titles) {
      const view = render(<StandardDialog title={title} content={body()} open />);
      // title 元素 = base-ui DialogTitle（h2 → role=heading），空串档以 role 定位不依赖文本
      expect(screen.getByRole('heading').textContent).toBe(title);
      expect(screen.getByText(CONTENT) !== null).toBe(true);
      view.unmount();
    }
  });
});

describe('StandardDialog：受控三态注入与连续开合稳定性', () => {
  it('边界（链路 StandardDialog → useOpen）：rerender 注入 open=false → true → undefined → false，呈现随 prop 同步且 undefined 不干预', async () => {
    const view = render(<StandardDialog title={TITLE} content={body()} open={false} />);
    expect(screen.queryByRole('heading')).toBeNull();

    view.rerender(<StandardDialog title={TITLE} content={body()} open />);
    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));

    // undefined 注入不干预：内部态保持 true，弹窗仍在场
    view.rerender(<StandardDialog title={TITLE} content={body()} open={undefined} />);
    expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);

    view.rerender(<StandardDialog title={TITLE} content={body()} open={false} />);
    await waitFor(() => expect(screen.queryByRole('heading')).toBeNull());
    expect(screen.queryByText(CONTENT)).toBeNull();
  });

  it('边界：连续开-关-开三轮，footer 双态逐轮在场且 onSubmit 计数保持稳定（每轮恰 +1）', async () => {
    const onSubmit = vi.fn<(close: () => void) => void>();
    render(
      <StandardDialog
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onSubmit={onSubmit}
      />,
    );

    for (let round = 1; round <= 3; round += 1) {
      openByTrigger();
      const cancel = await screen.findByRole('button', { name: '取消' });
      expect(screen.getByRole('button', { name: '提交' }) !== null).toBe(true);

      fireEvent.click(screen.getByRole('button', { name: '提交' }));
      expect(onSubmit).toHaveBeenCalledTimes(round);
      // 提交不关窗：本轮 content 仍在场
      expect(screen.getByText(CONTENT) !== null).toBe(true);

      fireEvent.click(cancel);
      await waitFor(() => expect(screen.queryByText(CONTENT)).toBeNull());
    }

    expect(onSubmit).toHaveBeenCalledTimes(3);
  });
});
