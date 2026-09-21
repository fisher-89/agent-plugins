import type { ArtifactEnvelope, ChecklistItem } from '../types/dto';

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
    return <pre className="fallback-text">{envelope.fallbackText ?? '（清单为空）'}</pre>;
  }
  const { phase, attempt, verdict, items } = envelope.payload;
  const passCount = items.filter((entry) => entry.pass).length;
  return (
    <div>
      <div className="attempt-meta">
        <span>phase: {phase}</span>
        {attempt !== null && <span>attempt: {attempt}</span>}
        {verdict !== null && (
          <span className={`badge badge-${verdict === 'pass' ? 'pass' : 'fail'}`}>{verdict}</span>
        )}
        <span>
          {passCount}/{items.length} 通过
        </span>
      </div>
      <ul className="checklist">
        {items.map((entry, index) => (
          <li key={index}>
            <span className={`badge badge-${entry.pass ? 'pass' : 'fail'}`}>
              {entry.pass ? 'pass' : 'fail'}
            </span>
            <div>
              <div className="item-row">{entry.item}</div>
              <div className="evidence">{entry.evidence}</div>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
