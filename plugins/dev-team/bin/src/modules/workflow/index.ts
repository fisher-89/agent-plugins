/**
 * modules/workflow — the module boundary for `workflow.json` operation logic.
 * Everything that reads, folds, writes or queries `workflow.json` is exported
 * from this single barrel: new logic for `workflow.json` belongs in this
 * directory and is exported here (the existing eval writes, `change_create`
 * and `backtrack` metadata move over incrementally in later changes).
 */

export {
  type FileInventory,
  type FileOp,
  foldFileOps,
  readFileInventory,
  writeFileInventory,
} from './file-inventory';
export { getChangedFiles } from './files-query';
