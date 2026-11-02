import { PlusIcon, RefreshCwIcon } from 'lucide-react';
import { useCallback } from 'react';
import { useNavigate } from 'react-router';

import { TextTooltip } from '@/components/standard/tooltip';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { ChangeList, ChangeSummary } from '../../types/dto';
import { ChangeCreateDialog } from './components/change-create-dialog';
import { useChangeList } from './hooks/use-change-list';

function ChangeRow({
  summary,
  onSelect,
}: {
  summary: ChangeSummary;
  onSelect: (id: string) => void;
}) {
  return (
    <Button
      className="h-auto w-full justify-start gap-2.5 whitespace-normal rounded-none border-0 border-b bg-transparent px-1 py-2 text-left font-normal text-inherit hover:bg-transparent hover:text-primary last:border-b-0"
      onClick={() => onSelect(summary.id)}
      data-testid="change-row"
    >
      <span className="font-semibold">{summary.title}</span>
      {summary.activePhase !== null && (
        <Badge variant="secondary">
          运行中 · {summary.activePhase.phase} · attempt {summary.activePhase.attempt}
        </Badge>
      )}
      {summary.created !== null && (
        <span className="text-xs text-muted-foreground" data-testid="created">
          {summary.created}
        </span>
      )}
    </Button>
  );
}

/** archive 按月分组列表（"未知时间"组置尾） */
function ArchiveGroups({
  groups,
  onSelect,
}: {
  groups: ChangeList['archiveGroups'];
  onSelect: (id: string) => void;
}) {
  return (
    <>
      {groups.map((group) => (
        <section
          className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
          key={group.month ?? 'unknown'}
        >
          <h2 className="m-0 mb-2.5 text-[15px]">
            {group.month ?? '未知时间'}{' '}
            <span className="text-muted-foreground">({group.changes.length})</span>
          </h2>
          {group.changes.map((summary) => (
            <ChangeRow key={summary.id} summary={summary} onSelect={onSelect} />
          ))}
        </section>
      ))}
    </>
  );
}

/** 数据区：active 列表 + archive 分组 + 空态提示 */
function ListSections({ data, onSelect }: { data: ChangeList; onSelect: (id: string) => void }) {
  return (
    <>
      <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
        <h2 className="m-0 mb-2.5 text-[15px]">
          进行中 <span className="text-muted-foreground">({data.active.length})</span>
        </h2>
        {data.active.length === 0 ? (
          <div className="text-muted-foreground">无进行中的 change。</div>
        ) : (
          data.active.map((summary) => (
            <ChangeRow key={summary.id} summary={summary} onSelect={onSelect} />
          ))
        )}
      </section>
      <ArchiveGroups groups={data.archiveGroups} onSelect={onSelect} />
      {data.active.length === 0 && data.archiveGroups.length === 0 && (
        <div className="text-muted-foreground">该 workspace 下未发现已建档 change。</div>
      )}
    </>
  );
}

/** change 列表视图 */
export function ChangeListView({ root }: { root: string | null }) {
  const state = useChangeList(root);
  const { data, loading, error } = state;

  const navigate = useNavigate();
  const openChange = useCallback((id: string) => navigate(`/changes/${id}`), [navigate]);
  const onCreated = useCallback(
    (id: string) => {
      state.refresh();
      void navigate(`/changes/${id}`);
    },
    [state.refresh, navigate],
  );
  return (
    <div>
      {/* 头部行（始终渲染）：刷新控件语义自 App header 迁入 */}
      <div className="mb-3 flex gap-2 items-center justify-end">
        <TextTooltip text="刷新">
          <Button
            data-testid="refresh-changes"
            variant="outline"
            size="icon"
            aria-label="刷新"
            disabled={state.loading}
            onClick={state.refresh}
          >
            <RefreshCwIcon />
          </Button>
        </TextTooltip>
        {root !== null && (
          <ChangeCreateDialog
            root={root}
            onCreated={onCreated}
            trigger={({ onClick }) => (
              <TextTooltip text="新建变更">
                <Button
                  data-testid="new-change"
                  variant="outline"
                  size="icon"
                  aria-label="新建变更"
                  disabled={state.loading}
                  onClick={onClick}
                >
                  <PlusIcon />
                </Button>
              </TextTooltip>
            )}
          ></ChangeCreateDialog>
        )}
      </div>
      {error !== null && (
        <div
          className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="error-note"
        >
          列表加载失败：{error}
        </div>
      )}
      {loading && <div className="text-muted-foreground">加载中…</div>}
      {!loading && data === null && !error && (
        <div className="text-muted-foreground">暂无数据，点击刷新获取。</div>
      )}
      {data !== null && <ListSections data={data} onSelect={openChange} />}
    </div>
  );
}
