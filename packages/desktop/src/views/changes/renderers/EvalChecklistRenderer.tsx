import { Badge } from '@/components/ui/badge';

import type { ArtifactEnvelope, ChecklistItem } from '../../../types/dto';

/** eval-checklist payload 契约（design 数据模型表）：
 * { phase, attempt, verdict, items: [{ item, pass, evidence }] } */
interface EvalChecklistPayload {
  phase: string;
  attempt: number | null;
  verdict: string | null;
  items: ChecklistItem[];
}

function isChecklistItem(value: unknown): value is ChecklistItem {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<ChecklistItem>;
  return (
    typeof candidate.item === 'string' &&
    typeof candidate.pass === 'boolean' &&
    typeof candidate.evidence === 'string'
  );
}

/** 类型守卫：以 payload 实际形状收窄，替代 as 断言（payload 契约为 unknown） */
function isEvalChecklistPayload(value: unknown): value is EvalChecklistPayload {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<EvalChecklistPayload>;
  if (typeof candidate.phase !== 'string') return false;
  if (candidate.attempt !== null && typeof candidate.attempt !== 'number') return false;
  if (candidate.verdict !== null && typeof candidate.verdict !== 'string') return false;
  return Array.isArray(candidate.items) && candidate.items.every(isChecklistItem);
}

/** eval-checklist renderer：items（item / pass / evidence）清单渲染，pass/fail 视觉区分 */
export function EvalChecklistRenderer({ envelope }: { envelope: ArtifactEnvelope }) {
  if (!isEvalChecklistPayload(envelope.payload) || envelope.payload.items.length === 0) {
    return (
      <pre
        className="m-0 max-h-[320px] overflow-auto whitespace-pre-wrap break-words rounded-md bg-background px-2.5 py-2 text-xs text-foreground"
        data-testid="fallback-text"
      >
        {envelope.fallbackText ?? '（清单为空）'}
      </pre>
    );
  }
  const { phase, attempt, verdict, items } = envelope.payload;
  const passCount = items.filter((entry) => entry.pass).length;
  return (
    <div>
      <div
        className="mb-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
        data-testid="attempt-meta"
      >
        <span>phase: {phase}</span>
        {attempt !== null && <span>attempt: {attempt}</span>}
        {verdict !== null && (
          <Badge variant={verdict === 'pass' ? 'pass' : 'fail'} data-testid="attempt-verdict">
            {verdict}
          </Badge>
        )}
        <span>
          {passCount}/{items.length} 通过
        </span>
      </div>
      <ul className="m-0 mt-1.5 list-none p-0" data-testid="checklist">
        {items.map((entry, index) => (
          <li key={index} className="flex items-baseline gap-2 py-[3px]">
            <Badge variant={entry.pass ? 'pass' : 'fail'} data-testid="checklist-verdict">
              {entry.pass ? 'pass' : 'fail'}
            </Badge>
            <div>
              <div className="border-b border-dashed border-border py-1">{entry.item}</div>
              <div className="break-words text-xs text-muted-foreground">{entry.evidence}</div>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
