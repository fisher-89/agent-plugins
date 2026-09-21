import type { ComponentType } from 'react';

import type { ArtifactEnvelope } from '../types/dto';
import { EvalChecklistRenderer } from './EvalChecklistRenderer';
import { Fallback } from './Fallback';
import { MarkdownDocRenderer } from './MarkdownDocRenderer';
import { TasksProgressRenderer } from './TasksProgressRenderer';

/** 信封渲染组件类型：入参单个 ArtifactEnvelope */
export type RendererComponent = ComponentType<{ envelope: ArtifactEnvelope }>;

// 第一波三个 renderer 注册；新增产物 = 新增一个模块 + 在此追加一行
const renderers: Record<string, RendererComponent> = {
  'markdown-doc': MarkdownDocRenderer,
  'eval-checklist': EvalChecklistRenderer,
  'tasks-progress': TasksProgressRenderer,
};

/** 按 kind 路由；未注册 kind 返回 Fallback（永不白屏） */
export function resolveRenderer(kind: string): RendererComponent {
  return renderers[kind] ?? Fallback;
}
