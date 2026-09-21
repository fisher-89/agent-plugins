import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../types/dto';
import { TasksProgressRenderer } from './TasksProgressRenderer';

function envelope(payload: unknown): ArtifactEnvelope {
  return { kind: 'tasks-progress', version: 1, title: '任务进度', payload, fallbackText: null };
}

describe('TasksProgressRenderer：total / done / pending 进度渲染', () => {
  it('比例与计数一致地渲染进度', () => {
    render(<TasksProgressRenderer envelope={envelope({ total: 5, done: 2, pending: 3 })} />);
    expect(screen.getByText('40%') !== null).toBe(true);
    expect(screen.getByText('已完成 2') !== null).toBe(true);
    expect(screen.getByText('待办 3') !== null).toBe(true);
    expect(screen.getByText('共 5') !== null).toBe(true);
    const fill = document.querySelector<HTMLElement>('.progress-fill');
    expect(fill?.style.width).toBe('40%');
  });

  it('total=0 时不产生除零 NaN，显示空进度', () => {
    const { container } = render(
      <TasksProgressRenderer envelope={envelope({ total: 0, done: 0, pending: 0 })} />,
    );
    expect(container.textContent).not.toContain('NaN');
    expect(screen.getByText('0%') !== null).toBe(true);
    const fill = container.querySelector<HTMLElement>('.progress-fill');
    expect(fill?.style.width).toBe('0%');
  });

  it('done=total 全勾时呈现 100% 形态', () => {
    render(<TasksProgressRenderer envelope={envelope({ total: 4, done: 4, pending: 0 })} />);
    expect(screen.getByText('100%') !== null).toBe(true);
    const fill = document.querySelector<HTMLElement>('.progress-fill');
    expect(fill?.style.width).toBe('100%');
  });

  it('payload 缺字段（undefined 计数）时不抛错，按 0 处理', () => {
    expect(() => render(<TasksProgressRenderer envelope={envelope(undefined)} />)).not.toThrow();
    expect(screen.getByText('0%') !== null).toBe(true);
    expect(screen.getByText('共 0') !== null).toBe(true);
  });

  it('payload 字段类型漂移（非数值）时不抛错，按 0 处理', () => {
    expect(() =>
      render(
        <TasksProgressRenderer envelope={envelope({ total: '很多', done: true, pending: null })} />,
      ),
    ).not.toThrow();
    expect(screen.getByText('0%') !== null).toBe(true);
  });
});
