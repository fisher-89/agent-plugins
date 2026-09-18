import { resolveChangeDir } from '../../../lib/change';
import { type NetFileState, readNetState } from './file-inventory';

/**
 * The single public read channel of the change file inventory:
 * read-only query of `workflow.json.file_log` as its derived net state.
 */
export function getChangedFiles(changeName: string, projectRoot: string): NetFileState {
  return readNetState(resolveChangeDir(changeName, projectRoot));
}
