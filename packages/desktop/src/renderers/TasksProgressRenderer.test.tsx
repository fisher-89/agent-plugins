import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../types/dto';
import { TasksProgressRenderer } from './TasksProgressRenderer';

function envelope(payload: unknown): ArtifactEnvelope {
  return { kind: 'tasks-progress', version: 1, title: '任务进度', payload, fallbackText: null };
}

// .progress-track/.progress-fill 换装 radix Progress 后内联 width 消失：
// 结构断言迁移至 progressbar 语义属性（aria-valuenow），progress testid 为辅助落点。
describe('TasksProgressRenderer：total / done / pending 进度渲染', () => {
  it('比例与计数一致地渲染进度', () => {
    render(<TasksProgressRenderer envelope={envelope({ total: 5, done: 2, pending: 3 })} />);
    expect(screen.getByText('40%') !== null).toBe(true);
    expect(screen.getByText('已完成 2') !== null).toBe(true);
    expect(screen.getByText('待办 3') !== null).toBe(true);
    expect(screen.getByText('共 5') !== null).toBe(true);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('40');
  });

  it('total=0 时不产生除零 NaN，显示空进度', () => {
    render(<TasksProgressRenderer envelope={envelope({ total: 0, done: 0, pending: 0 })} />);
    expect(screen.queryByText(/NaN/)).toBeNull();
    expect(screen.getByText('0%') !== null).toBe(true);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
  });

  it('done=total 全勾时呈现满进度形态', () => {
    render(<TasksProgressRenderer envelope={envelope({ total: 4, done: 4, pending: 0 })} />);
    expect(screen.getByText('100%') !== null).toBe(true);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('100');
  });

  it('progress testid 与 progressbar 语义角色落在同一宿主', () => {
    render(<TasksProgressRenderer envelope={envelope({ total: 5, done: 2, pending: 3 })} />);
    expect(screen.getByTestId('progress')).toBe(screen.getByRole('progressbar'));
  });

  it('payload 缺字段（undefined 计数）时不抛错，按 0 处理且无 indeterminate 形态', () => {
    expect(() => render(<TasksProgressRenderer envelope={envelope(undefined)} />)).not.toThrow();
    expect(screen.getByText('0%') !== null).toBe(true);
    expect(screen.getByText('共 0') !== null).toBe(true);
    // value 为数值 0 而非 null：aria-valuenow 存在（indeterminate 形态不得出现）
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
  });

  it('payload 字段类型漂移（非数值）时不抛错，按 0 处理', () => {
    expect(() =>
      render(
        <TasksProgressRenderer envelope={envelope({ total: '很多', done: true, pending: null })} />,
      ),
    ).not.toThrow();
    expect(screen.getByText('0%') !== null).toBe(true);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
  });
});
