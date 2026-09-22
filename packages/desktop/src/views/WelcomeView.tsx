import type { WorkspaceState } from '../hooks/useWorkspaces';

/**
 * 欢迎屏：无 workspace 记录时的空态 +「添加新文件夹」入口
 * + 加载中 / error-note 呈现。（清单非空时启动即恢复第一名，不停留此处。）
 */
export function WelcomeView({ state, onAdd }: { state: WorkspaceState; onAdd: () => void }) {
  const { workspaces, loading, error } = state;
  return (
    <div className="screen-center">
      <h1>Desktop Terminal</h1>
      <p>选择一个项目根目录，浏览其 change 过程记录。</p>
      <div className="app-main">
        {error !== null && <div className="error-note">workspace 清单加载失败：{error}</div>}
        {loading && <div className="muted">加载中…</div>}
        <section className="panel">
          <h2>
            最近的 workspace <span className="muted">({workspaces.length})</span>
          </h2>
          {!loading && error === null && workspaces.length === 0 && (
            <div className="muted">还没有记录，添加一个项目根目录开始浏览。</div>
          )}
        </section>
        <button onClick={onAdd}>添加新文件夹</button>
      </div>
    </div>
  );
}
