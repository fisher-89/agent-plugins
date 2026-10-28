import { Button } from '@/components/ui/button';

import type { WorkspaceState } from '../hooks/use-workspaces';

/**
 * 欢迎屏：无 workspace 记录时的空态 +「添加新文件夹」入口
 * + 加载中 / error-note 呈现（error-note 仅承载清单加载失败；添加等动作失败
 * 经 toast 呈现）。（清单非空时启动即恢复第一名，不停留此处。）
 */
export function WelcomeView({ state, onAdd }: { state: WorkspaceState; onAdd: () => void }) {
  const { loading, error } = state;
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4">
      <h1 className="m-0 text-xl">Dev Team</h1>
      <div className="mx-auto w-full flex-1 px-4 py-4">
        {loading ? (
          <div className="text-muted-foreground">加载中…</div>
        ) : error !== null ? (
          <div
            className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
            data-testid="error-note"
          >
            workspace 清单加载失败：{error}
          </div>
        ) : (
          <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
            <div className="text-muted-foreground">还没有记录，添加一个项目根目录开始浏览。</div>
            <Button onClick={onAdd}>添加新文件夹</Button>
          </section>
        )}
      </div>
    </div>
  );
}
