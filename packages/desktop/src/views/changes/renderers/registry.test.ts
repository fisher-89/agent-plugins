import { describe, expect, it } from 'vite-plus/test';

import { EvalChecklistRenderer } from './eval-checklist-renderer';
import { Fallback } from './fallback';
import { MarkdownDocRenderer } from './markdown-doc-renderer';
import { resolveRenderer } from './registry';
import { TasksProgressRenderer } from './tasks-progress-renderer';

describe('resolveRenderer：kind 到渲染组件的映射', () => {
  it('三个第一波 kind 分别返回对应 renderer 组件', () => {
    expect(resolveRenderer('markdown-doc')).toBe(MarkdownDocRenderer);
    expect(resolveRenderer('eval-checklist')).toBe(EvalChecklistRenderer);
    expect(resolveRenderer('tasks-progress')).toBe(TasksProgressRenderer);
  });

  it('未注册 kind（如 file-log）返回 Fallback 组件', () => {
    // file-log 是第二刀规划的 kind，当前未注册
    expect(resolveRenderer('file-log')).toBe(Fallback);
    expect(resolveRenderer('not-a-kind')).toBe(Fallback);
  });

  it('空字符串 kind 与大小写变体按精确匹配语义返回 Fallback', () => {
    expect(resolveRenderer('')).toBe(Fallback);
    expect(resolveRenderer('Markdown-Doc')).toBe(Fallback);
    expect(resolveRenderer('MARKDOWN-DOC')).toBe(Fallback);
  });

  it('resolveRenderer 对任意输入都不抛错', () => {
    expect(() => resolveRenderer('💡')).not.toThrow();
    expect(resolveRenderer('💡')).toBe(Fallback);
  });
});
