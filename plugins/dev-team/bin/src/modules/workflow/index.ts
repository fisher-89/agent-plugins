/**
 * modules/workflow — the module boundary for `workflow.json` operation logic.
 * The barrel exposes the public surface: the single file-inventory read
 * channel (`getChangedFiles`), the log-write-semantic channels
 * (`recordFileOps`, `appendWorkflowFiles`, `setWorkflowFiles`) and the phase
 * running-state operations (`readActivePhase`, `writeActivePhase`,
 * `clearActivePhase`, `interruptActivePhase`); low-level persistence
 * primitives (`doc-io.ts`, the changeDir-level `readNetState` seam, the
 * raw-log read and the pure net-state derivation) stay module-internal.
 * New logic for `workflow.json` belongs in this
 * directory and is exported here (the existing eval writes, `change_create`
 * and `backtrack` metadata move over incrementally in later changes).
 */

export {
  appendWorkflowFiles,
  type FileOp,
  type RecordScope,
  setWorkflowFiles,
} from './files/file-inventory';

export { getChangedFiles } from './files/files-query';
export { recordFileOps } from './files/record';

export {
  type ActivePhase,
  clearActivePhase,
  interruptActivePhase,
  readActivePhase,
  writeActivePhase,
} from './phase/phase-state';
