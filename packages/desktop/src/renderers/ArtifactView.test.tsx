import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../types/dto';
import { ArtifactView } from './ArtifactView';

function envelope(overrides: Partial<ArtifactEnvelope> = {}): ArtifactEnvelope {
  return {
    kind: 'tasks-progress',
    version: 1,
    title: '任务进度',
    payload: { total: 4, done: 1, pending: 3 },
    fallbackText: '任务进度：已完成 1 / 4，待办 3',
    ...overrides,
  };
}

describe('ArtifactView：信封按 kind 路由到 renderer（AC-7 / AC-10）', () => {
  it('已注册 kind 信封路由到专属 renderer 并渲染 payload 内容', () => {
    render(<ArtifactView envelope={envelope()} />);
    // 专属 renderer 的 payload 内容（25% = 1/4），而非 fallback 文本
    expect(screen.getByText('25%') !== null).toBe(true);
    expect(screen.getByText('已完成 1') !== null).toBe(true);
    expect(screen.queryByText('任务进度：已完成 1 / 4，待办 3')).toBeNull();
  });

  it('未注册 kind 信封走 Fallback：fallback_text 与 kind 徽标均可见', () => {
    render(
      <ArtifactView envelope={envelope({ kind: 'file-log', title: '文件清单', payload: null })} />,
    );
    // kind 徽标在信封卡头部（组件树内保证"kind 徽标可见"）
    expect(screen.getByText('file-log') !== null).toBe(true);
    // fallback 文本渲染
    expect(screen.getByText('任务进度：已完成 1 / 4，待办 3') !== null).toBe(true);
  });

  it('fallback_text 为 null 的未注册 kind 信封仍渲染 kind 徽标，不空白', () => {
    render(
      <ArtifactView
        envelope={envelope({ kind: 'mystery-kind', payload: null, fallbackText: null })}
      />,
    );
    // kind 徽标经 artifact-kind 挂钩断言（Badge 宿主），「永不白屏」护栏语义不变
    expect(screen.getByTestId('artifact-kind').textContent).toBe('mystery-kind');
    expect(screen.getByText('（该产物类型暂无渲染器，且无保底文本）') !== null).toBe(true);
  });

  it('payload 形状与 renderer 期望不符时不抛错，呈现降级形态（永不白屏）', () => {
    expect(() =>
      render(
        <ArtifactView
          envelope={envelope({
            payload: { total: '不是数字', done: null },
            fallbackText: '降级保底',
          })}
        />,
      ),
    ).not.toThrow();
    // 计数收窄为 0 → 0%，组件不白屏；标题与徽标仍在
    expect(screen.getByText('0%') !== null).toBe(true);
    expect(screen.getByText('任务进度') !== null).toBe(true);
  });

  it('信封头渲染 kind 徽标、标题与版本号', () => {
    render(<ArtifactView envelope={envelope()} />);
    expect(screen.getByTestId('artifact-kind').textContent).toBe('tasks-progress');
    expect(screen.getByText('任务进度') !== null).toBe(true);
    expect(screen.getByTestId('artifact-version').textContent).toBe('v1');
  });
});
