import { open } from '@tauri-apps/plugin-dialog';
import { useCallback } from 'react';

import { Button } from '@/components/ui/button';

import { useChangeList } from './hooks/useChangeList';
import { useUpdater, type UpdateState } from './hooks/useUpdater';
import { useWorkspaces } from './hooks/useWorkspaces';
import type { WorkspaceRecord } from './types/dto';
import { ChangeView } from './views/changes/ChangeView';
import { WelcomeView } from './views/WelcomeView';

/**
 * 顶栏更新指示：当前版本号 + 更新入口。有新版本 →「更新到 vX」→ 下载进度 →
 * Finished 转「正在安装…」终态（Windows 上 NSIS 安装器接管重启，其后无回调）；
 * 失败 →「重试更新」。
 */
function UpdateIndicator({ state }: { state: UpdateState }) {
  return (
    <>
      {state.currentVersion !== null && (
        <span className="text-xs text-muted-foreground">v{state.currentVersion}</span>
      )}
      {state.status === 'installing' ? (
        <span className="text-xs text-muted-foreground">正在安装…</span>
      ) : state.status === 'downloading' ? (
        <span className="text-xs text-muted-foreground">下载中 {state.progress ?? 0}%</span>
      ) : state.error !== null ? (
        <Button onClick={state.start}>重试更新</Button>
      ) : (
        state.available !== null && (
          <Button onClick={state.start}>更新到 v{state.available.version}</Button>
        )
      )}
    </>
  );
}

/** workspace 下拉切换（清单项悬停 title 完整 path）。preflight 重置原生 select 的
 * 边框/底色，此处以 utilities 还原原生观感。 */
function WorkspaceSelect({
  root,
  workspaces,
  onOpen,
}: {
  root: string;
  workspaces: WorkspaceRecord[];
  onOpen: (root: string) => void;
}) {
  return (
    <select
      className="rounded-md border border-border bg-card px-2 py-1"
      value={root}
      onChange={(event) => onOpen(event.target.value)}
      disabled={workspaces.length === 0}
    >
      {workspaces.map((record) => (
        <option key={record.root} value={record.root} title={record.root}>
          {record.name}
        </option>
      ))}
    </select>
  );
}

/**
 * 顶栏：workspace 下拉切换（清单项悬停 title 完整 path）+ 移除当前项 + 刷新列表
 * + 版本与更新入口。下拉与欢迎屏共用 useWorkspaces 同一清单来源。
 */
function AppHeader({
  root,
  workspaces,
  loading,
  error,
  update,
  onOpen,
  onRemove,
  onRefresh,
}: {
  root: string;
  workspaces: WorkspaceRecord[];
  loading: boolean;
  error: string | null;
  update: UpdateState;
  onOpen: (root: string) => void;
  onRemove: (root: string) => void;
  onRefresh: () => void;
}) {
  return (
    <header className="flex flex-wrap items-center gap-3 border-b border-border bg-card px-4 py-2.5">
      <strong>Desktop Terminal</strong>
      <WorkspaceSelect root={root} workspaces={workspaces} onOpen={onOpen} />
      <Button onClick={() => onRemove(root)}>移除</Button>
      {error !== null && (
        <span
          className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="error-note"
        >
          {error}
        </span>
      )}
      <span className="flex-1" />
      <UpdateIndicator state={update} />
      <Button onClick={onRefresh} disabled={loading}>
        刷新列表
      </Button>
    </header>
  );
}

/**
 * 应用壳：workspace 选择与恢复（启动自动恢复收在 useWorkspaces / Header 下拉切换）
 * + 列表 / 详情视图切换 + 刷新动作下发。
 * 组件不直接 invoke，取数统一经 useWorkspaces / useChangeList / useChangeDetail。
 */
export default function App() {
  const workspaceState = useWorkspaces();
  const list = useChangeList(workspaceState.root);
  const update = useUpdater();

  // 对话框添加流经 useWorkspaces().add 入库；刷新后新记录 last_opened_at 最新，
  // 即清单第一名，root 随之切换到返回记录的 canonical root
  const pickAndAdd = useCallback(async () => {
    let selected: unknown;
    try {
      selected = await open({ directory: true, multiple: false });
    } catch {
      return; // 对话框调用失败：保持现状，不中断应用
    }
    if (typeof selected !== 'string') return; // 取消选择则保持现状
    const record = await workspaceState.add(selected);
    if (record === null) return; // 入库失败：error 态已呈现，不打开
  }, [workspaceState]);

  if (workspaceState.root === null) {
    return <WelcomeView state={workspaceState} onAdd={pickAndAdd} />;
  }

  return (
    <>
      <AppHeader
        root={workspaceState.root}
        workspaces={workspaceState.workspaces}
        loading={list.loading}
        error={workspaceState.error}
        update={update}
        onOpen={workspaceState.touch}
        onRemove={workspaceState.remove}
        onRefresh={list.refresh}
      />
      <main className="mx-auto w-full max-w-[1100px] flex-1 px-4 py-4">
        <ChangeView root={workspaceState.root} list={list}></ChangeView>
      </main>
    </>
  );
}
