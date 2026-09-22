import { Progress } from '@/components/ui/progress';

import type { ArtifactEnvelope } from '../types/dto';

/** tasks-progress payload 契约（design 数据模型表）：{ total, done, pending }；
 * 字段声明为 unknown 由守卫/计数助手按值收窄——容忍缺字段与类型漂移，一律按 0 处理 */
interface TasksProgressPayload {
  total?: unknown;
  done?: unknown;
  pending?: unknown;
}

function countOf(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0;
}

/** 类型守卫：以 payload 实际形状收窄，替代 as 断言（payload 契约为 unknown） */
function isTasksProgressPayload(value: unknown): value is TasksProgressPayload {
  return typeof value === 'object' && value !== null;
}

/** tasks-progress renderer：total / done / pending 进度渲染 */
export function TasksProgressRenderer({ envelope }: { envelope: ArtifactEnvelope }) {
  const payload = isTasksProgressPayload(envelope.payload) ? envelope.payload : undefined;
  const total = countOf(payload?.total);
  const done = countOf(payload?.done);
  const pending = countOf(payload?.pending);
  // total 为 0 时无进度可言，直接 0%
  const percent = total > 0 ? Math.round((done / total) * 100) : 0;
  return (
    <div>
      <Progress value={percent} className="my-2" data-testid="progress" />
      <div className="flex gap-4 text-[13px] text-muted-foreground">
        <span>{percent}%</span>
        <span>已完成 {done}</span>
        <span>待办 {pending}</span>
        <span>共 {total}</span>
      </div>
    </div>
  );
}
