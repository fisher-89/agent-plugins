import { useEffect, useState } from 'react';

import { ipc } from '../lib/ipc';
import { type AppEnvironment as AppEnvironmentPayload } from '../types/generated/bindings';

/**
 * 构建模式（线值由后端 `env::mode()` 单点产出，前端只做编译期收窄）。
 * MUST NOT 在前端自行推导——vite 的 DEV 环境标志回答「前端是否由 dev server
 * 服务」，dev 构建（`tauri build --debug`）下恒 false，会误判成生产。
 */
export type BuildMode = 'dev' | 'prod';

/** 数据根来源（线值域同后端 `RootSource::wire()`：`prod` / `dev-default` / `env`）。
 * 仅本模块内收窄用（`AppEnvironment.source`），不外扩：无第二个消费者。 */
type DataRootSource = 'prod' | 'dev-default' | 'env';

/**
 * 应用环境：字段集合**派生自生成物** `AppEnvironmentPayload`（`Omit` 只摘掉线值
 * 收窄的两个字段），故后端 DTO 增删 / 改名在编译期即暴露——MUST NOT 手写同形
 * 接口（手写形态与生成物无编译期关联，改名会静默退化）。生成物同名，导入时以
 * `AppEnvironmentPayload` 别名避让。
 */
export type AppEnvironment = Omit<AppEnvironmentPayload, 'mode' | 'source'> & {
  mode: BuildMode;
  source: DataRootSource;
};

/**
 * 构建模式单源 hook：挂载一次 `app_environment` IPC，成功且载荷合法才返回
 * 环境，未就绪 / reject / 载荷不合法一律 `null`（静默——IPC 失败 MUST NOT
 * 升级为壳态错误态、MUST NOT 阻塞其余取数）。
 */
export function useAppEnvironment(): AppEnvironment | null {
  const [environment, setEnvironment] = useState<AppEnvironment | null>(null);

  useEffect(() => {
    let cancelled = false;
    void ipc
      .appEnvironment()
      .then((payload) => {
        if (cancelled) return;
        setEnvironment(toEnvironment(payload));
      })
      .catch(() => {}); // 环境取数失败即静默：无徽标、不判定更新
    return () => {
      cancelled = true;
    };
  }, []);

  return environment;
}

/** 载荷收窄为对象（生成物 types 为 `string`，线值域需逐字段校验）。 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/** 线值收窄：`dev` / `prod`（谓词锚定派生类型，非独立手写域）。 */
function isBuildMode(value: unknown): value is AppEnvironment['mode'] {
  return value === 'dev' || value === 'prod';
}

/** 线值收窄：`prod` / `dev-default` / `env`（谓词锚定派生类型）。 */
function isDataRootSource(value: unknown): value is AppEnvironment['source'] {
  return value === 'prod' || value === 'dev-default' || value === 'env';
}

/** 载荷 → 环境；任一字段不合法的载荷整体视为非法（`null`，零断言零 any）。 */
function toEnvironment(payload: unknown): AppEnvironment | null {
  if (!isRecord(payload)) return null;
  const { mode, dataRoot, source } = payload;
  if (!isBuildMode(mode)) return null;
  if (!isDataRootSource(source)) return null;
  if (typeof dataRoot !== 'string') return null;
  return { mode, dataRoot, source };
}
