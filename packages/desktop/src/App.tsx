import { open } from '@tauri-apps/plugin-dialog';
import { useCallback, useState } from 'react';

import { useChangeDetail } from './hooks/useChangeDetail';
import { useChangeList } from './hooks/useChangeList';
import { ChangeDetailView } from './views/ChangeDetailView';
import { ChangeListView } from './views/ChangeListView';

/** 顶栏：workspace 路径展示 + 刷新列表 + 切换 workspace。 */
function AppHeader({
  root,
  loading,
  onRefresh,
  onPick,
}: {
  root: string;
  loading: boolean;
  onRefresh: () => void;
  onPick: () => void;
}) {
  return (
    <header className="app-header">
      <strong>Desktop Terminal</strong>
      <span className="workspace" title={root}>
        {root}
      </span>
      <span className="spacer" />
      <button onClick={onRefresh} disabled={loading}>
        刷新列表
      </button>
      <button onClick={onPick}>切换 workspace</button>
    </header>
  );
}

/**
 * 应用壳：workspace 选择（文件夹选择器）+ 列表 / 详情视图切换 + 刷新动作下发。
 * 组件不直接 invoke，取数统一经 useChangeList / useChangeDetail 两个 hooks。
 */
export default function App() {
  const [root, setRoot] = useState<string | null>(null);
  const [selectedChange, setSelectedChange] = useState<string | null>(null);

  const list = useChangeList(root);
  const detail = useChangeDetail(root, selectedChange);

  const pickWorkspace = useCallback(async () => {
    let selected: unknown;
    try {
      selected = await open({ directory: true, multiple: false });
    } catch {
      return; // 对话框调用失败：保持现状，不中断应用
    }
    if (typeof selected !== 'string') return; // 取消选择则保持现状
    setSelectedChange(null);
    setRoot(selected);
  }, []);

  const openChange = useCallback((name: string) => setSelectedChange(name), []);
  const backToList = useCallback(() => setSelectedChange(null), []);

  if (root === null) {
    return (
      <div className="screen-center">
        <h1>Desktop Terminal</h1>
        <p>选择一个项目根目录，浏览其 change 过程记录。</p>
        <button onClick={pickWorkspace}>选择 workspace 文件夹</button>
      </div>
    );
  }

  return (
    <>
      <AppHeader
        root={root}
        loading={list.loading}
        onRefresh={list.refresh}
        onPick={pickWorkspace}
      />
      <main className="app-main">
        {selectedChange === null ? (
          <ChangeListView state={list} onSelect={openChange} />
        ) : (
          <ChangeDetailView state={detail} onBack={backToList} />
        )}
      </main>
    </>
  );
}
