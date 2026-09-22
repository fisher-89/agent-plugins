import { getVersion } from '@tauri-apps/api/app';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { useCallback, useEffect, useRef, useState } from 'react';

/** 可用更新的 UI 最小描述（不持有 Update 资源本体）。 */
interface UpdateBrief {
  version: string;
  body?: string;
}

type UpdateStatus = 'idle' | 'downloading' | 'installing' | 'error';

export interface UpdateState {
  currentVersion: string | null;
  available: UpdateBrief | null;
  status: UpdateStatus;
  progress: number | null;
  error: string | null;
  start: () => void;
}

/**
 * 应用更新收口 hook：挂载拉取当前版本号并（仅安装版）静默 check 一次；
 * start 由用户动作触发，重新 check 取新鲜 Update 实例后 downloadAndInstall。
 * Windows 上 NSIS 安装器接管重启，本进程在安装前退出——installing 为终态，
 * 其后无回调；失败置 error 供「重试更新」。
 */
export function useUpdater(): UpdateState {
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [available, setAvailable] = useState<UpdateBrief | null>(null);
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busyRef = useRef(false);

  useStartupCheck(setCurrentVersion, setAvailable);

  const start = useCallback(() => {
    if (busyRef.current) return;
    busyRef.current = true;
    setStatus('downloading');
    setError(null);
    setProgress(null);
    void performUpdate({ setAvailable, setStatus, setProgress, setError }).finally(() => {
      busyRef.current = false;
    });
  }, []);

  return { currentVersion, available, status, progress, error, start };
}

/** 启动检查：版本号总是拉取；更新检查仅安装版执行（dev 构建跳过），失败一律静默。 */
function useStartupCheck(
  setCurrentVersion: (version: string) => void,
  setAvailable: (brief: UpdateBrief | null) => void,
): void {
  useEffect(() => {
    let cancelled = false;
    void getVersion()
      .then((version) => {
        if (!cancelled) setCurrentVersion(version);
      })
      .catch(() => {}); // 版本号取失败即不显示，不报错
    if (!import.meta.env.DEV) {
      check()
        .then((update) => {
          if (cancelled || update === null) return;
          setAvailable(briefOf(update));
          void update.close();
        })
        .catch(() => {}); // 启动期检查失败静默：断网 / 无 release 均属正常
    }
    return () => {
      cancelled = true;
    };
  }, [setCurrentVersion, setAvailable]);
}

type UpdateCallbacks = {
  setAvailable: (brief: UpdateBrief | null) => void;
  setStatus: (status: UpdateStatus) => void;
  setProgress: (percent: number | null) => void;
  setError: (message: string) => void;
};

function briefOf(update: Update): UpdateBrief {
  return { version: update.version, body: update.body };
}

/**
 * 用户触发的更新执行：重新 check → downloadAndInstall（进度事件）→ 安装。
 * Windows 上安装器以 /UPDATE 拉起新进程，本进程退出，await 之后语句不可达；
 * check 返回 null（更新已被撤下）时收起入口回 idle。
 */
async function performUpdate(cb: UpdateCallbacks): Promise<void> {
  try {
    const update = await check();
    if (update === null) {
      cb.setAvailable(null);
      cb.setStatus('idle');
      return;
    }
    cb.setAvailable(briefOf(update));
    let received = 0;
    let total: number | null = null;
    await update.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        total = event.data.contentLength ?? null;
      } else if (event.event === 'Progress') {
        received += event.data.chunkLength;
        if (total !== null) cb.setProgress(Math.min(100, Math.floor((received / total) * 100)));
      } else {
        cb.setStatus('installing');
      }
    });
  } catch (err: unknown) {
    cb.setStatus('error');
    cb.setError(String(err));
  }
}
