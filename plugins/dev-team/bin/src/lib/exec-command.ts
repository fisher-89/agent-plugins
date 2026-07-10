import { spawnSync, type SpawnSyncOptions, type SpawnSyncReturns } from 'node:child_process';

export function execCommand(command: string, options: SpawnSyncOptions): SpawnSyncReturns<string> {
  return spawnSync(command, {
    ...options,
    encoding: 'utf-8',
    stdio: 'pipe',
  });
}
