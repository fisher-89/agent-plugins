import { ArtifactView } from '../../renderers/ArtifactView';
import type { ArtifactEnvelope, AttemptRecord, ChangeDetail, PhaseEntry } from '../../types/dto';
import type { ChangeDetailState } from './hooks/useChangeDetail';

function VerdictBadge({ verdict }: { verdict: AttemptRecord['verdict'] }) {
  return <span className={`badge badge-${verdict === 'pass' ? 'pass' : 'fail'}`}>{verdict}</span>;
}

function formatTime(value: string | null): string {
  if (value === null) return '—';
  return value;
}

function Attempt({ record }: { record: AttemptRecord }) {
  return (
    <div className="attempt">
      <div className="attempt-meta">
        {record.attempt !== null ? <span>attempt {record.attempt}</span> : <span>attempt —</span>}
        <VerdictBadge verdict={record.verdict} />
        {record.skipped && <span>skipped</span>}
        {record.stale && <span>stale</span>}
        {record.startAt !== null && <span>start: {formatTime(record.startAt)}</span>}
        {record.timestamp !== null && <span>at: {formatTime(record.timestamp)}</span>}
      </div>
      <div className="report">{record.report}</div>
      {(record.backtrackTo !== null || record.backtrackReason !== null) && (
        <div className="backtrack">
          ↩ 回跳至 {record.backtrackTo ?? '?'}
          {record.backtrackReason !== null && `：${record.backtrackReason}`}
        </div>
      )}
      {record.checklist.length > 0 && (
        <ul className="checklist">
          {record.checklist.map((entry, index) => (
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
      )}
    </div>
  );
}

function Station({ entry }: { entry: PhaseEntry }) {
  return (
    <div className="station">
      <div className="station-head">
        <span>{entry.phase}</span>
        <span className="muted">({entry.attempts.length} 次尝试)</span>
      </div>
      {entry.attempts.length === 0 ? (
        <div className="muted">（无记录）</div>
      ) : (
        entry.attempts.map((record, index) => <Attempt key={index} record={record} />)
      )}
    </div>
  );
}

function DetailSectionInterupted({ interrupted }: { interrupted: ChangeDetail['interrupted'] }) {
  return (
    <section className="panel">
      <h2>中断留档</h2>
      {interrupted.map((entry, index) => (
        <div className="attempt" key={index}>
          <div className="attempt-meta">
            <span>{entry.phase}</span>
            <span>attempt {entry.attempt}</span>
            <span>start: {formatTime(entry.startAt)}</span>
            <span>end: {formatTime(entry.endAt)}</span>
          </div>
        </div>
      ))}
    </section>
  );
}

function DetailSectionFileList({ detail }: { detail: ChangeDetail }) {
  return (
    <section className="panel">
      <h2>文件清单 (file_log)</h2>
      {detail.fileLog === null ? (
        <div className="muted">（无 file_log 数据：v1 及更早代际无此字段）</div>
      ) : detail.fileLog.length === 0 ? (
        <div className="muted">（空）</div>
      ) : (
        <table className="filelog-table">
          <thead>
            <tr>
              <th>op</th>
              <th>scope</th>
              <th>attempt</th>
              <th>path</th>
              <th>at</th>
            </tr>
          </thead>
          <tbody>
            {detail.fileLog.map((entry, index) => (
              <tr key={index}>
                <td>{entry.op}</td>
                <td>{entry.scope}</td>
                <td>{entry.attempt ?? '—'}</td>
                <td>{entry.path}</td>
                <td>{formatTime(entry.at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function DetailHeader({
  detail,
  loading,
  onBack,
  refresh,
}: {
  detail: ChangeDetail;
  loading: boolean;
  onBack: () => void;
  refresh: () => void;
}) {
  return (
    <div className="detail-header">
      <button onClick={onBack}>← 返回列表</button>
      <button onClick={refresh} disabled={loading}>
        刷新详情
      </button>
      <h2>{detail.name}</h2>
      <span className={`badge badge-in${detail.inventory}`}>{detail.inventory}</span>
      <span className="muted">{detail.source === 'archive' ? '已归档' : '进行中'}</span>
      {detail.created !== null && <span className="muted">{detail.created}</span>}
      {detail.activePhase !== null && (
        <span className="badge badge-active">
          运行中 · {detail.activePhase.phase} · attempt {detail.activePhase.attempt}
          {detail.activePhase.startAt !== null && ` · ${formatTime(detail.activePhase.startAt)}`}
        </span>
      )}
    </div>
  );
}

function DetailSectionPipeline({
  pipeline,
  docOnly,
}: {
  pipeline: PhaseEntry[];
  docOnly: boolean;
}) {
  return (
    <section className="panel">
      <h2>流水线</h2>
      {pipeline.length === 0 ? (
        <div className="muted">
          {docOnly ? '（v0 早期代际：无 workflow.json，仅文档形态）' : '（无评估记录）'}
        </div>
      ) : (
        pipeline.map((entry) => <Station key={entry.phase} entry={entry} />)
      )}
    </section>
  );
}

function DetailSectionArtifacts({ artifacts }: { artifacts: ArtifactEnvelope[] }) {
  return (
    <section className="panel">
      <h2>
        产物 <span className="muted">({artifacts.length})</span>
      </h2>
      {artifacts.length === 0 ? (
        <div className="muted">（未发现可读产物）</div>
      ) : (
        artifacts.map((envelope, index) => (
          <ArtifactView key={`${envelope.kind}-${envelope.title}-${index}`} envelope={envelope} />
        ))
      )}
    </section>
  );
}

/** 详情降级页：错误 / 加载中 / 未找到共用，error 控制提示样式。 */
function DetailFallback({
  message,
  error = false,
  onBack,
}: {
  message: string;
  error?: boolean;
  onBack: () => void;
}) {
  return (
    <div>
      <button onClick={onBack}>← 返回列表</button>
      <div className={error ? 'error-note' : 'muted'}>{message}</div>
    </div>
  );
}

/** change 详情视图：9 站流水线、active_phase 运行中标示、v0 纯文档形态、v1 区块留空降级、产物区 */
export function ChangeDetailView({
  state,
  onBack,
}: {
  state: ChangeDetailState;
  onBack: () => void;
}) {
  const { detail, artifacts, loading, error, refresh } = state;
  if (error !== null) {
    return <DetailFallback message={`详情加载失败：${error}`} error onBack={onBack} />;
  }
  if (loading && detail === null) {
    return <DetailFallback message="加载中…" onBack={onBack} />;
  }
  if (detail === null) {
    return <DetailFallback message="未找到该 change。" onBack={onBack} />;
  }
  const docOnly = detail.inventory === 'v0';
  return (
    <div>
      <DetailHeader detail={detail} loading={loading} onBack={onBack} refresh={refresh} />

      {detail.unparsable && (
        <div className="warn-note">
          workflow.json 无法解析（可能已损坏），以下仅展示文件系统层信息与产物。
        </div>
      )}

      <DetailSectionPipeline pipeline={detail.pipeline} docOnly={docOnly} />

      {detail.interrupted.length > 0 && (
        <DetailSectionInterupted interrupted={detail.interrupted}></DetailSectionInterupted>
      )}

      <DetailSectionFileList detail={detail}></DetailSectionFileList>

      <DetailSectionArtifacts artifacts={artifacts} />
    </div>
  );
}
