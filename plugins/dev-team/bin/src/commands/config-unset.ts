import { readConfig, writeConfig, unsetValue } from '../lib/config';
import { getProjectDir } from '../utils';

export interface ConfigUnsetOptions {
  key: string;
  projectRoot?: string;
}

export interface ConfigUnsetResult {
  key: string;
  removed: boolean;
}

/**
 * Run config_unset: delete a key from openspec/config.json by dot-separated key path.
 * Returns removed: false if the key did not exist.
 */
export function runConfigUnset(options: ConfigUnsetOptions): ConfigUnsetResult {
  const projectRoot = options.projectRoot || getProjectDir();

  // Read current config
  const config = readConfig(projectRoot);

  // Unset the value at the key path (mutates config in-place)
  const { removed } = unsetValue(config as Record<string, unknown>, options.key);

  // Only write back if something was actually removed
  if (removed) {
    writeConfig(projectRoot, config);
  }

  return {
    key: options.key,
    removed,
  };
}
