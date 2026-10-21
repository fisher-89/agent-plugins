import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { StandardDrawer } from './drawer';

// 零进程边界：不 mock 被测组件与 ui/** 底座（base-ui Drawer 真实渲染）；
// onOpenChange 以 vi.fn() 注入观察面；data-testid 经 ...rest 透传内容容器，
// containerClassName 的 className 仅作被检属性（MUST NOT 作查询挂钩）。

const TITLE = '标准抽屉';
const CONTENT = '抽屉正文';
const SHELL = '抽屉内容壳';

/** 触发器（trigger prop 为 JSX 元素，经 DrawerTrigger render 落地）。 */
function triggerButton(): React.JSX.Element {
  return <button type="button">打开抽屉</button>;
}

/** 正文片段（content prop）。 */
function body(): React.JSX.Element {
  return <p>{CONTENT}</p>;
}

/** 点击触发器开抽屉（非受控 trigger 档）。 */
function openByTrigger(): void {
  fireEvent.click(screen.getByRole('button', { name: '打开抽屉' }));
}

describe('StandardDrawer：trigger 开合与 footer 恒「关闭」', () => {
  it('正向：trigger 在场时点击打开抽屉，title 与 content 同步呈现（关闭态两者缺席）', async () => {
    render(<StandardDrawer title={TITLE} content={body()} trigger={triggerButton()} />);

    expect(screen.queryByRole('heading', { name: TITLE })).toBeNull();
    expect(screen.queryByText(CONTENT)).toBeNull();

    openByTrigger();

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
      expect(screen.getByText(CONTENT) !== null).toBe(true);
    });
  });

  it('正向：footer 恒「关闭」单按钮（onOpenChange 在场档：弹层内无「提交」「取消」）', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>();
    render(
      <StandardDrawer
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onOpenChange={onOpenChange}
        data-testid={SHELL}
      />,
    );
    openByTrigger();

    await screen.findByRole('button', { name: '关闭' });
    const shell = screen.getByTestId(SHELL);
    // 弹层内容容器内恰一枚按钮且为「关闭」（footer 恒单按钮）
    expect(
      within(shell)
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['关闭']);
    expect(screen.queryByRole('button', { name: '提交' })).toBeNull();
    expect(screen.queryByRole('button', { name: '取消' })).toBeNull();
  });

  it('异常：trigger 缺席（undefined，Optional None 档）：不渲染触发器，受控 open=true 驱动呈现', () => {
    render(<StandardDrawer title={TITLE} content={body()} open />);

    expect(screen.queryByRole('button', { name: '打开抽屉' })).toBeNull();
    expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
    expect(screen.getByText(CONTENT) !== null).toBe(true);
  });

  it('异常：onOpenChange 缺席（undefined，Optional None 档）：开合不抛错，footer 仍仅「关闭」单按钮', async () => {
    render(<StandardDrawer title={TITLE} content={body()} trigger={triggerButton()} />);

    openByTrigger();
    const close = await screen.findByRole('button', { name: '关闭' });
    expect(screen.queryByRole('button', { name: '提交' })).toBeNull();
    expect(screen.queryByRole('button', { name: '取消' })).toBeNull();

    fireEvent.click(close);
    await waitFor(() => expect(screen.queryByRole('heading')).toBeNull());
  });
});

describe('StandardDrawer：开合链路（StandardDrawer → useOpen）', () => {
  it('链路：trigger 开、「关闭」关，onOpenChange 随开合依次回报 true / false', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>();
    render(
      <StandardDrawer
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onOpenChange={onOpenChange}
      />,
    );

    openByTrigger();
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledTimes(1));
    expect(onOpenChange).toHaveBeenNthCalledWith(1, true);

    const close = await screen.findByRole('button', { name: '关闭' });
    fireEvent.click(close);
    await waitFor(() => expect(screen.queryByRole('heading')).toBeNull());

    expect(onOpenChange).toHaveBeenCalledTimes(2);
    expect(onOpenChange).toHaveBeenNthCalledWith(2, false);
  });

  it('边界：连续开-关-开三轮，「关闭」逐轮在场、开合回报逐轮稳定（true / false 成对 +2/轮）', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>();
    render(
      <StandardDrawer
        title={TITLE}
        content={body()}
        trigger={triggerButton()}
        onOpenChange={onOpenChange}
      />,
    );

    for (let round = 1; round <= 3; round += 1) {
      openByTrigger();
      await waitFor(() => {
        expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
        expect(screen.getByRole('button', { name: '关闭' }) !== null).toBe(true);
      });

      fireEvent.click(screen.getByRole('button', { name: '关闭' }));
      await waitFor(() => expect(screen.queryByRole('heading')).toBeNull());
      expect(onOpenChange).toHaveBeenCalledTimes(round * 2);
    }

    expect(onOpenChange).toHaveBeenNthCalledWith(1, true);
    expect(onOpenChange).toHaveBeenNthCalledWith(2, false);
    expect(onOpenChange).toHaveBeenNthCalledWith(5, true);
    expect(onOpenChange).toHaveBeenNthCalledWith(6, false);
  });
});

describe('StandardDrawer：containerClassName 透传（类名仅作被检属性）', () => {
  it('正向：containerClassName 在场 → 内容容器 className 含该类名（testid 定位元素，类名不作查询挂钩）', () => {
    render(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        data-testid={SHELL}
        containerClassName="宽幅容器-🎯"
      />,
    );

    const shell = screen.getByTestId(SHELL);
    expect(shell.className).toContain('宽幅容器-🎯');
    expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
  });

  it('异常：containerClassName 缺席（undefined，Optional None 档）：容器照常渲染且自定义类撤出即回基线', () => {
    const view = render(<StandardDrawer title={TITLE} content={body()} open data-testid={SHELL} />);
    const baseline = screen.getByTestId(SHELL).className;
    expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);
    expect(baseline).not.toContain('自定容器-🎯');

    view.rerender(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        data-testid={SHELL}
        containerClassName="自定容器-🎯"
      />,
    );
    expect(screen.getByTestId(SHELL).className).toContain('自定容器-🎯');

    view.rerender(<StandardDrawer title={TITLE} content={body()} open data-testid={SHELL} />);
    expect(screen.getByTestId(SHELL).className).toBe(baseline);
  });

  it('边界：containerClassName 逐档（空串 / 特殊字符 / 超长串 >1000 字符）透传，空串档类名回到空态基线', () => {
    const specialClass = '特殊-🎯容器';
    const longClass = `容器${'宽'.repeat(1001)}`;

    const view = render(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        data-testid={SHELL}
        containerClassName=""
      />,
    );
    // 空串档：无任何自定义类名附加（空态基线）
    const emptyState = screen.getByTestId(SHELL).className;
    expect(emptyState).not.toContain(specialClass);

    view.rerender(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        data-testid={SHELL}
        containerClassName={specialClass}
      />,
    );
    expect(screen.getByTestId(SHELL).className).toContain(specialClass);

    view.rerender(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        data-testid={SHELL}
        containerClassName={longClass}
      />,
    );
    expect(screen.getByTestId(SHELL).className).toContain(longClass);

    view.rerender(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        data-testid={SHELL}
        containerClassName=""
      />,
    );
    expect(screen.getByTestId(SHELL).className).toBe(emptyState);
  });
});

describe('StandardDrawer：title 边界与受控三态注入', () => {
  it('边界：title 逐档（空串 / "\\n🚀" 特殊字符 / >1000 字符）渲染 title heading 元素在场且文本无损', () => {
    const titles = ['', '第一行\n🚀第二行\0', `长${'标'.repeat(1001)}`];

    for (const title of titles) {
      const view = render(<StandardDrawer title={title} content={body()} open />);
      // title 元素 = base-ui DrawerTitle（h2 → role=heading），空串档以 role 定位不依赖文本
      expect(screen.getByRole('heading').textContent).toBe(title);
      expect(screen.getByText(CONTENT) !== null).toBe(true);
      view.unmount();
    }
  });

  it('边界（链路 StandardDrawer → useOpen）：rerender 注入 open 三态（false / true / undefined），呈现随 prop 同步且 undefined 不干预', async () => {
    const view = render(<StandardDrawer title={TITLE} content={body()} open={false} />);
    expect(screen.queryByRole('heading')).toBeNull();

    view.rerender(<StandardDrawer title={TITLE} content={body()} open />);
    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));

    // undefined 注入不干预：内部态保持 true，抽屉仍在场
    view.rerender(<StandardDrawer title={TITLE} content={body()} open={undefined} />);
    expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true);

    view.rerender(<StandardDrawer title={TITLE} content={body()} open={false} />);
    await waitFor(() => expect(screen.queryByRole('heading')).toBeNull());
    expect(screen.queryByText(CONTENT)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// modal 开关（`StandardDrawer` 增 `modal?: boolean = true`，D17）：`data-modal`
// 由底层 `components/ui/drawer` 的 Viewport 给出、遮罩仅模态档渲染；`modal` 只
// 透传底层 `Drawer`，MUST NOT 落进 `DrawerContent` 的 rest（不多出未知 DOM 属性）。
// ---------------------------------------------------------------------------

/** 底层 Viewport（`data-modal` 的出具点；非本组件 DOM 属性面）。 */
function drawerViewport(): HTMLElement {
  const viewport = document.querySelector('[data-slot="drawer-viewport"]');
  if (viewport === null) throw new Error('抽屉 Viewport 未挂载');
  return viewport as HTMLElement;
}

function overlayNode(): Element | null {
  return document.querySelector('[data-slot="drawer-overlay"]');
}

describe('StandardDrawer：modal 开关（缺省模态 / 非模态并读）', () => {
  it('缺省模态：不传 modal（既有调用形态）→ data-modal="true" 且遮罩在场', async () => {
    render(<StandardDrawer title={TITLE} content={body()} open data-testid={SHELL} />);

    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));
    expect(drawerViewport().getAttribute('data-modal')).toBe('true');
    expect(overlayNode(), '模态档遮罩在场').not.toBeNull();
  });

  it('非模态透传：modal={false} → data-modal="false"、遮罩不在场、标题 / 正文 / 页脚关闭照常', async () => {
    render(
      <StandardDrawer title={TITLE} content={body()} open modal={false} data-testid={SHELL} />,
    );

    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));
    expect(drawerViewport().getAttribute('data-modal')).toBe('false');
    expect(overlayNode(), '非模态档无遮罩（页面其余区域可交互）').toBeNull();
    expect(screen.getByText(CONTENT) !== null).toBe(true);
    expect(screen.getByRole('button', { name: '关闭' }) !== null).toBe(true);
  });

  it('显式 modal={true} 与缺省形态 DOM 全等（data-modal / 遮罩 / 内容壳属性）', async () => {
    const explicit = render(
      <StandardDrawer title={TITLE} content={body()} open modal data-testid={SHELL} />,
    );
    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));
    const explicitModal = drawerViewport().getAttribute('data-modal');
    const explicitOverlay = overlayNode() !== null;
    explicit.unmount();

    render(<StandardDrawer title={TITLE} content={body()} open data-testid={SHELL} />);
    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));

    expect(drawerViewport().getAttribute('data-modal')).toBe(explicitModal);
    expect(overlayNode() !== null).toBe(explicitOverlay);
  });

  it('modal 不落内容壳 DOM 属性面：内容壳无 modal 属性、无未知属性告警', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <StandardDrawer title={TITLE} content={body()} open modal={false} data-testid={SHELL} />,
    );

    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));
    const shell = screen.getByTestId(SHELL);
    expect(shell.hasAttribute('modal')).toBe(false);
    expect(
      consoleError.mock.calls.filter(([message]) =>
        String(message).includes('React does not recognize'),
      ),
      'modal 不进 rest（无未知 DOM 属性告警）',
    ).toHaveLength(0);
    consoleError.mockRestore();
  });

  it('受控档开合：无 trigger、外部驱动 open + modal={false} → 页脚「关闭」回报 onOpenChange(false) 一次', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>();
    const view = render(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        modal={false}
        onOpenChange={onOpenChange}
        data-testid={SHELL}
      />,
    );
    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));

    fireEvent.click(screen.getByRole('button', { name: '关闭' }));

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledTimes(1));
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(screen.queryByRole('heading', { name: TITLE })).toBeNull();

    // 外部再驱动开启（open 保持由外到内的单向同步：先落 false 再落 true）：
    // 开合照常（非模态不影响受控语义）
    view.rerender(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open={false}
        modal={false}
        onOpenChange={onOpenChange}
        data-testid={SHELL}
      />,
    );
    view.rerender(
      <StandardDrawer
        title={TITLE}
        content={body()}
        open
        modal={false}
        onOpenChange={onOpenChange}
        data-testid={SHELL}
      />,
    );
    await waitFor(() => expect(screen.getByRole('heading', { name: TITLE }) !== null).toBe(true));
  });
});
