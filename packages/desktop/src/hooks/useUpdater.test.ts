import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { useUpdater } from './useUpdater';

const { checkMock, closeMock, downloadAndInstallMock, getVersionMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  closeMock: vi.fn(),
  downloadAndInstallMock: vi.fn(),
  getVersionMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: getVersionMock,
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: checkMock,
}));

/** 构造一只最小 Update 替身（hook 仅消费 version/body/close/downloadAndInstall）。 */
function fakeUpdate(version = '0.2.0') {
  return {
    version,
    body: '修复若干问题',
    close: closeMock,
    downloadAndInstall: downloadAndInstallMock,
  };
}

/** 依序派发下载事件后结束（Started → Progress×n → Finished）。 */
async function emitEvents(onEvent: (event: unknown) => void): Promise<void> {
  onEvent({ event: 'Started', data: { contentLength: 200 } });
  onEvent({ event: 'Progress', data: { chunkLength: 100 } });
  onEvent({ event: 'Progress', data: { chunkLength: 60 } });
  onEvent({ event: 'Finished' });
}

describe('useUpdater：启动检查静默收口 + 用户触发更新流', () => {
  beforeEach(() => {
    checkMock.mockReset();
    closeMock.mockReset();
    downloadAndInstallMock.mockReset();
    getVersionMock.mockReset();
    getVersionMock.mockResolvedValue('0.1.0');
    checkMock.mockResolvedValue(null);
    closeMock.mockResolvedValue(undefined);
    downloadAndInstallMock.mockResolvedValue(undefined);
    vi.stubEnv('DEV', false);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('DEV 构建跳过更新检查：check 不调用，版本号照常拉取', async () => {
    vi.stubEnv('DEV', true);
    const { result } = renderHook(() => useUpdater());

    await waitFor(() => expect(result.current.currentVersion).toBe('0.1.0'));
    expect(checkMock).not.toHaveBeenCalled();
  });

  it('安装版挂载检查无更新：available 保持 null 且不置 error', async () => {
    const { result } = renderHook(() => useUpdater());

    await waitFor(() => expect(checkMock).toHaveBeenCalled());
    await act(async () => {});
    expect(result.current.available).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.status).toBe('idle');
  });

  it('发现新版本：available 填充版本与说明，并立即释放 Update 资源（close）', async () => {
    checkMock.mockResolvedValue(fakeUpdate('0.2.0'));
    const { result } = renderHook(() => useUpdater());

    await waitFor(() => expect(result.current.available?.version).toBe('0.2.0'));
    expect(result.current.available?.body).toBe('修复若干问题');
    expect(closeMock).toHaveBeenCalledTimes(1);
    expect(result.current.error).toBeNull();
  });

  it('启动检查失败静默：不置 error、不显示更新入口', async () => {
    checkMock.mockRejectedValue(new Error('network down'));
    const { result } = renderHook(() => useUpdater());

    await waitFor(() => expect(checkMock).toHaveBeenCalled());
    await act(async () => {});
    expect(result.current.available).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('getVersion 失败：版本号保持 null 且不报错', async () => {
    getVersionMock.mockRejectedValue(new Error('nope'));
    const { result } = renderHook(() => useUpdater());

    await waitFor(() => expect(checkMock).toHaveBeenCalled());
    await act(async () => {});
    expect(result.current.currentVersion).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('start 更新：进度事件按累计字节换算百分比，Finished 转 installing 终态', async () => {
    checkMock.mockResolvedValue(fakeUpdate());
    downloadAndInstallMock.mockImplementation(
      async (onEvent: (event: unknown) => void) => await emitEvents(onEvent),
    );
    const { result } = renderHook(() => useUpdater());
    await waitFor(() => expect(result.current.available).not.toBeNull());

    await act(async () => {
      result.current.start();
    });

    await waitFor(() => expect(result.current.status).toBe('installing'));
    expect(result.current.progress).toBe(80); // (100+60)/200
    expect(result.current.error).toBeNull();
  });

  it('start 失败置 error；重试重新 check 拿新实例再走安装流', async () => {
    checkMock.mockResolvedValue(fakeUpdate());
    downloadAndInstallMock.mockRejectedValueOnce(new Error('checksum mismatch'));
    const { result } = renderHook(() => useUpdater());
    await waitFor(() => expect(result.current.available).not.toBeNull());

    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.status).toBe('error'));
    expect(result.current.error).toContain('checksum mismatch');
    expect(checkMock).toHaveBeenCalledTimes(2); // 挂载 + start 重取

    downloadAndInstallMock.mockImplementation(
      async (onEvent: (event: unknown) => void) => await emitEvents(onEvent),
    );
    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.status).toBe('installing'));
    expect(checkMock).toHaveBeenCalledTimes(3); // 重试又重取一只新鲜实例
  });

  it('start 时更新已被撤下（check null）：收起入口回 idle', async () => {
    checkMock.mockResolvedValueOnce(fakeUpdate());
    checkMock.mockResolvedValueOnce(null);
    const { result } = renderHook(() => useUpdater());
    await waitFor(() => expect(result.current.available?.version).toBe('0.2.0'));

    await act(async () => {
      result.current.start();
    });

    await waitFor(() => expect(result.current.available).toBeNull());
    expect(result.current.status).toBe('idle');
    expect(result.current.error).toBeNull();
  });

  it('下载进行中重复 start 被 busy 守卫拦下：不重复 check / 下载', async () => {
    let finish: () => void = () => {};
    checkMock.mockResolvedValue(fakeUpdate());
    downloadAndInstallMock.mockImplementation(async (onEvent: (event: unknown) => void) => {
      onEvent({ event: 'Started', data: { contentLength: 100 } });
      onEvent({ event: 'Progress', data: { chunkLength: 40 } });
      await new Promise<void>((resolve) => {
        finish = resolve;
      });
    });
    const { result } = renderHook(() => useUpdater());
    await waitFor(() => expect(result.current.available).not.toBeNull());

    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.progress).toBe(40));
    act(() => {
      result.current.start(); // 下载中重复点击：no-op
    });
    expect(checkMock).toHaveBeenCalledTimes(2); // 挂载 + 首次 start，重复点击未新增

    await act(async () => {
      finish();
    });
  });
});
