// hooks.ts — Gateway: subcommand registration & dispatch for dev-team hooks
// (protect-files, record-files, sweep-phase, static-check). Implementations
// live in commands/; the shared shell-command op extractor lives in
// lib/shell-file-ops.

import { runProtectFiles } from './commands/protect-files';
import { runRecordFiles } from './commands/record-files';
import { runStaticCheck } from './commands/static-check';
import { runSweepPhase } from './commands/sweep-phase';

// Entry export surface (knip entry-exempt): keeps the established import path
// for tests and integrations.
export { runProtectFiles, runRecordFiles, runSweepPhase, runStaticCheck };
export { captureStderr } from './commands/static-check';

/**
 * Top-level entry function.
 * Parses process.argv[2] as the subcommand name and dispatches.
 */
export function main(): void {
  const subcommand = process.argv[2];

  switch (subcommand) {
    case 'protect-files':
      runProtectFiles();
      break;
    case 'record-files':
      runRecordFiles();
      break;
    case 'sweep-phase':
      runSweepPhase();
      break;
    case 'static-check':
      runStaticCheck();
      break;
    default: {
      process.stderr.write(`Unknown subcommand: ${subcommand}\n`);
      process.exit(1);
    }
  }
}

// Execute main with a catch-all for unexpected errors.
// Outputs { decision: "block", reason: <error message> } on crash per design risk mitigation.
try {
  main();
} catch (error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  process.stdout.write(`${JSON.stringify({ decision: 'block', reason: message })}\n`);
  process.exit(1);
}
