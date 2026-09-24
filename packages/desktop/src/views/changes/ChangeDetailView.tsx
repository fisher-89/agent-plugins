import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

import type {
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
  Inventory,
  PhaseEntry,
} from '../../types/dto';
import type { ChangeDetailState } from './hooks/useChangeDetail';
import { ArtifactView } from './renderers/ArtifactView';

// Tailwind 无法静态识别模板串类名：`badge-in${inventory}` 收敛为显式 variant 映射（spec 硬性要求）
const INVENTORY_VARIANT: Record<Inventory, 'inv0' | 'inv1' | 'inv2'> = {
  v0: 'inv0',
  v1: 'inv1',
  v2: 'inv2',
};

function VerdictBadge({ verdict }: { verdict: AttemptRecord['verdict'] }) {
  return (
    <Badge variant={verdict === 'pass' ? 'pass' : 'fail'} data-testid="attempt-verdict">
      {verdict}
    </Badge>
  );
}

function formatTime(value: string | null): string {
  if (value === null) return '—';
  return value;
}

function Attempt({ record }: { record: AttemptRecord }) {
  return (
    <div className="my-2 ml-1 border-l-[3px] border-border py-1.5 pl-3">
      <div
        className="mb-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
        data-testid="attempt-meta"
      >
        {record.attempt !== null ? <span>attempt {record.attempt}</span> : <span>attempt —</span>}
        <VerdictBadge verdict={record.verdict} />
        {record.skipped && <span>skipped</span>}
        {record.stale && <span>stale</span>}
        {record.startAt !== null && <span>start: {formatTime(record.startAt)}</span>}
        {record.timestamp !== null && <span>at: {formatTime(record.timestamp)}</span>}
      </div>
      <div className="my-1 whitespace-pre-wrap break-words">{record.report}</div>
      {(record.backtrackTo !== null || record.backtrackReason !== null) && (
        <div className="mt-1 text-xs text-orange-300" data-testid="backtrack">
          ↩ 回跳至 {record.backtrackTo ?? '?'}
          {record.backtrackReason !== null && `：${record.backtrackReason}`}
        </div>
      )}
      {record.checklist.length > 0 && (
        <ul className="m-0 mt-1.5 list-none p-0" data-testid="checklist">
          {record.checklist.map((entry, index) => (
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
      )}
    </div>
  );
}

function Station({ entry }: { entry: PhaseEntry }) {
  return (
    <div className="mb-3.5">
      <div className="flex items-center gap-2 font-semibold">
        <span>{entry.phase}</span>
        <span className="text-muted-foreground">({entry.attempts.length} 次尝试)</span>
      </div>
      {entry.attempts.length === 0 ? (
        <div className="text-muted-foreground">（无记录）</div>
      ) : (
        entry.attempts.map((record, index) => <Attempt key={index} record={record} />)
      )}
    </div>
  );
}

function DetailSectionInterupted({ interrupted }: { interrupted: ChangeDetail['interrupted'] }) {
  return (
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">中断留档</h2>
      {interrupted.map((entry, index) => (
        <div className="my-2 ml-1 border-l-[3px] border-border py-1.5 pl-3" key={index}>
          <div className="mb-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
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
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">文件清单 (file_log)</h2>
      {detail.fileLog === null ? (
        <div className="text-muted-foreground">（无 file_log 数据：v1 及更早代际无此字段）</div>
      ) : detail.fileLog.length === 0 ? (
        <div className="text-muted-foreground">（空）</div>
      ) : (
        <Table data-testid="filelog-table">
          <TableHeader>
            <TableRow>
              <TableHead>op</TableHead>
              <TableHead>scope</TableHead>
              <TableHead>attempt</TableHead>
              <TableHead>path</TableHead>
              <TableHead>at</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {detail.fileLog.map((entry, index) => (
              <TableRow key={index}>
                <TableCell>{entry.op}</TableCell>
                <TableCell>{entry.scope}</TableCell>
                <TableCell>{entry.attempt ?? '—'}</TableCell>
                <TableCell>{entry.path}</TableCell>
                <TableCell>{formatTime(entry.at)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
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
    <div className="mb-3 flex flex-wrap items-center gap-2.5" data-testid="detail-header">
      <Button onClick={onBack}>← 返回列表</Button>
      <Button onClick={refresh} disabled={loading}>
        刷新详情
      </Button>
      <h2 className="m-0 break-all text-[17px]">{detail.name}</h2>
      <Badge variant={INVENTORY_VARIANT[detail.inventory]}>{detail.inventory}</Badge>
      <span className="text-muted-foreground">
        {detail.source === 'archive' ? '已归档' : '进行中'}
      </span>
      {detail.created !== null && <span className="text-muted-foreground">{detail.created}</span>}
      {detail.activePhase !== null && (
        <Badge variant="active">
          运行中 · {detail.activePhase.phase} · attempt {detail.activePhase.attempt}
          {detail.activePhase.startAt !== null && ` · ${formatTime(detail.activePhase.startAt)}`}
        </Badge>
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
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">流水线</h2>
      {pipeline.length === 0 ? (
        <div className="text-muted-foreground">
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
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">
        产物 <span className="text-muted-foreground">({artifacts.length})</span>
      </h2>
      {artifacts.length === 0 ? (
        <div className="text-muted-foreground">（未发现可读产物）</div>
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
      <Button onClick={onBack}>← 返回列表</Button>
      {error ? (
        <div
          className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="error-note"
        >
          {message}
        </div>
      ) : (
        <div className="text-muted-foreground" data-testid="detail-note">
          {message}
        </div>
      )}
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
        <div
          className="my-2 rounded-md bg-warn-bg px-2.5 py-1.5 text-[13px] text-warn"
          data-testid="warn-note"
        >
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
