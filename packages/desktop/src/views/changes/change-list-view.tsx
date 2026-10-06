import { useCallback } from 'react';
import { useNavigate } from 'react-router';

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
  onSelect: (name: string) => void;
}) {
  return (
    <Button
      className="h-auto w-full justify-start gap-2.5 whitespace-normal rounded-none border-0 border-b bg-transparent px-1 py-2 text-left font-normal text-inherit hover:bg-transparent hover:text-primary last:border-b-0"
      onClick={() => onSelect(summary.name)}
      data-testid="change-row"
    >
      <span className="font-semibold">{summary.name}</span>
      {summary.activePhase !== null && (
        <Badge variant="active">
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
  onSelect: (name: string) => void;
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
            <ChangeRow key={summary.name} summary={summary} onSelect={onSelect} />
          ))}
        </section>
      ))}
    </>
  );
}

/** 数据区：active 列表 + archive 分组 + 空目录提示 */
function ListSections({ data, onSelect }: { data: ChangeList; onSelect: (name: string) => void }) {
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
            <ChangeRow key={summary.name} summary={summary} onSelect={onSelect} />
          ))
        )}
      </section>
      <ArchiveGroups groups={data.archiveGroups} onSelect={onSelect} />
      {data.active.length === 0 && data.archiveGroups.length === 0 && (
        <div className="text-muted-foreground">该 workspace 下未发现任何 change 目录。</div>
      )}
    </>
  );
}

/** change 列表视图：清单页自取数（useChangeList 挂载 / root 变更 / 显式刷新触发，
 * 页面重挂即重取）+ 头部刷新行（始终渲染）+ 新建入口（root 非空时挂载）+
 * 加载/error-note 空态 + 列表数据区；active 列表 + archive 按月分组（"未知时间"
 * 组置尾）、运行中 active_phase 徽标、点击进详情；创建成功刷新清单并导航进
 * 详情（不自动发起 run） */
export function ChangeListView({ root }: { root: string | null }) {
  const state = useChangeList(root);
  const { data, loading, error } = state;

  const navigate = useNavigate();
  const openChange = useCallback((n: string) => navigate(`/changes/${n}`), [navigate]);
  const onCreated = useCallback(
    (name: string) => {
      state.refresh();
      void navigate(`/changes/${name}`);
    },
    [state.refresh, navigate],
  );
  return (
    <div>
      {/* 头部行（始终渲染）：刷新控件语义自 App header 迁入 */}
      <div className="mb-3 flex items-center justify-end">
        <Button disabled={state.loading} onClick={state.refresh}>
          刷新列表
        </Button>
      </div>
      {root !== null && <ChangeCreateDialog root={root} onCreated={onCreated} />}
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
