import { open } from '@tauri-apps/plugin-dialog';
import { useCallback } from 'react';

import { useChangeList } from './hooks/useChangeList';
import { useWorkspaces } from './hooks/useWorkspaces';
import type { WorkspaceRecord } from './types/dto';
import { ChangeView } from './views/changes/ChangeView';
import { WelcomeView } from './views/WelcomeView';

/**
 * 顶栏：workspace 下拉切换（清单项悬停 title 完整 path）+ 移除当前项 + 刷新列表。
 * 下拉与欢迎屏共用 useWorkspaces 同一清单来源。
 */
function AppHeader({
  root,
  workspaces,
  loading,
  error,
  onOpen,
  onRemove,
  onRefresh,
}: {
  root: string;
  workspaces: WorkspaceRecord[];
  loading: boolean;
  error: string | null;
  onOpen: (root: string) => void;
  onRemove: (root: string) => void;
  onRefresh: () => void;
}) {
  return (
    <header className="app-header">
      <strong>Desktop Terminal</strong>
      <select
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
      <button onClick={() => onRemove(root)}>移除</button>
      {error !== null && <span className="error-note">{error}</span>}
      <span className="spacer" />
      <button onClick={onRefresh} disabled={loading}>
        刷新列表
      </button>
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
        onOpen={workspaceState.touch}
        onRemove={workspaceState.remove}
        onRefresh={list.refresh}
      />
      <main className="app-main">
        <ChangeView root={workspaceState.root} list={list}></ChangeView>
      </main>
    </>
  );
}
