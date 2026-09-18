/**
 * modules/workflow — the module boundary for `workflow.json` operation logic.
 * The barrel exposes the public surface: read / query channels
 * (`readFileInventory`, `getChangedFiles`) and the write-semantic channels
 * (`recordFileOps`, `appendFileOps`, `setFileBuckets`); low-level
 * persistence primitives (`writeFileInventory`, `foldFileOps`) stay
 * module-internal. New logic for `workflow.json` belongs in this directory
 * and is exported here (the existing eval writes, `change_create` and
 * `backtrack` metadata move over incrementally in later changes).
 */

export {
  appendFileOps,
  type FileOp,
  readFileInventory,
  setFileBuckets,
} from './files/file-inventory';
export { getChangedFiles } from './files/files-query';
export { recordFileOps } from './files/record';
