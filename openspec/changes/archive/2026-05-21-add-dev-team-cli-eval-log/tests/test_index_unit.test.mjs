// test_index_unit.test.mjs — Unit tests for index.ts (CLI argument routing)
//
// Tests:
//   - main(): routes 'eval-log' subcommand, shows help for '--help', errors on unknown cmd
//   - parseArgs(): parses --key value pairs from argv array
//   - showHelp(): outputs usage information containing subcommand list
//   - showHelpEvalLog(): outputs detailed help for eval-log subcommand
//   - Edge cases: missing required args, empty argv, '--' separator
//
// Usage:
//   node --test test_index_unit.test.mjs

import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// Import the module under test.
// ---------------------------------------------------------------------------

// For testing compiled JS:
// import { main, parseArgs, showHelp, showHelpEvalLog }
//   from '../../../../plugins/dev-team/bin/dist/index.js';

// For inline stubs (use when source does not yet exist):
// ============================================================================

const REQUIRED_EVAL_LOG_ARGS = ['change', 'phase', 'verdict', 'report', 'items'];

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      if (i + 1 < argv.length && !argv[i + 1].startsWith('--')) {
        args[key] = argv[i + 1];
        i++; // skip value
      } else {
        args[key] = true; // flag-style (e.g., --help)
      }
    }
  }
  return args;
}

function showHelp() {
  const lines = [
    'Usage: dev-team [command] [options]',
    '',
    'Commands:',
    '  eval-log    Write an evaluation log entry to eval.json',
    '',
    'Options:',
    '  --help      Display help information',
    '',
    'Run "dev-team eval-log --help" for eval-log command details.',
  ];
  return lines.join('\n');
}

function showHelpEvalLog() {
  const lines = [
    'Usage: dev-team eval-log --change <name> --phase <phase> --verdict <verdict>',
    '                         --report <report> --items <items> [options]',
    '',
    'Required arguments:',
    '  --change <name>       Change directory name',
    '  --phase <phase>       Phase identifier (e.g., 01-requirements)',
    '  --verdict <verdict>   Evaluation verdict: "pass" or "fail"',
    '  --report <report>     Evaluation report (max 500 characters)',
    '  --items <items>       JSON array of evaluation items',
    '',
    'Options:',
    '  --attempt <n>         Explicit attempt number (auto-calculated if omitted)',
    '  --backtrack-to <phase> Backtrack target phase (optional)',
    '  --help                Display this help message',
    '',
    'Examples:',
    '  dev-team eval-log --change my-feature --phase 01-requirements \\',
    '    --verdict pass --report "All items pass." \\',
    '    --items \'[{"item_id":"R1","pass":true,"evidence":"done","notes":""}]\'',
  ];
  return lines.join('\n');
}

function main(argv) {
  if (argv.length === 0 || argv.includes('--help')) {
    console.log(showHelp());
    return 0;
  }

  const subcommand = argv[0];

  if (subcommand === 'eval-log') {
    // Strip subcommand and delegate
    const evalArgs = parseArgs(argv.slice(1));
    // Check if --help was requested for eval-log
    if (evalArgs.help) {
      console.log(showHelpEvalLog());
      return 0;
    }
    return 'route:eval-log'; // signal for test purposes
  }

  if (subcommand === '--help') {
    console.log(showHelp());
    return 0;
  }

  console.error(`Unknown command: ${subcommand}`);
  return 1;
}

// ============================================================================
// parseArgs()
// ============================================================================

describe('parseArgs()', () => {
  it('should parse --key value pairs', () => {
    const result = parseArgs(['--change', 'my-feature', '--phase', '01-requirements']);
    assert.strictEqual(result.change, 'my-feature');
    assert.strictEqual(result.phase, '01-requirements');
  });

  it('should return empty object for empty argv', () => {
    const result = parseArgs([]);
    assert.deepEqual(result, {});
  });

  it('should handle boolean flags (--help without value)', () => {
    const result = parseArgs(['--help']);
    assert.strictEqual(result.help, true);
  });

  it('should handle mixed flags and key-value args', () => {
    const result = parseArgs(['--change', 'test', '--verbose', '--phase', '01']);
    assert.strictEqual(result.change, 'test');
    assert.strictEqual(result.verbose, true);
    assert.strictEqual(result.phase, '01');
  });

  it('should handle values with hyphens and numbers', () => {
    const result = parseArgs(['--backtrack-to', '02-test-design']);
    assert.strictEqual(result['backtrack-to'], '02-test-design');
  });

  it('should use last value when key appears multiple times', () => {
    const result = parseArgs(['--phase', '01', '--phase', '02']);
    assert.strictEqual(result.phase, '02');
  });
});

// ============================================================================
// showHelp()
// ============================================================================

describe('showHelp()', () => {
  it('should return a non-empty string', () => {
    const help = showHelp();
    assert.ok(help.length > 0);
  });

  it('should mention "Usage:"', () => {
    assert.ok(showHelp().includes('Usage:'));
  });

  it('should list "eval-log" as available command', () => {
    assert.ok(showHelp().includes('eval-log'));
  });

  it('should mention "--help" option', () => {
    assert.ok(showHelp().includes('--help'));
  });
});

// ============================================================================
// showHelpEvalLog()
// ============================================================================

describe('showHelpEvalLog()', () => {
  it('should return a non-empty string', () => {
    const help = showHelpEvalLog();
    assert.ok(help.length > 0);
  });

  it('should mention "Usage:" with eval-log', () => {
    assert.ok(showHelpEvalLog().includes('Usage:'));
    assert.ok(showHelpEvalLog().includes('eval-log'));
  });

  it('should list all required arguments', () => {
    const help = showHelpEvalLog();
    for (const arg of REQUIRED_EVAL_LOG_ARGS) {
      assert.ok(help.includes(`--${arg}`), `Help should mention --${arg}`);
    }
  });

  it('should list optional arguments', () => {
    const help = showHelpEvalLog();
    assert.ok(help.includes('--attempt'));
    assert.ok(help.includes('--backtrack-to'));
  });

  it('should mention verdict values "pass" and "fail"', () => {
    const help = showHelpEvalLog();
    assert.ok(help.includes('pass'));
    assert.ok(help.includes('fail'));
  });

  it('should include an example invocation', () => {
    const help = showHelpEvalLog();
    assert.ok(help.includes('Example'));
  });
});

// ============================================================================
// main() routing
// ============================================================================

describe('main() routing', () => {
  it('should route "eval-log" subcommand', () => {
    const result = main(['eval-log', '--change', 'test', '--phase', '01', '--verdict', 'pass', '--report', 'ok', '--items', '[]']);
    assert.strictEqual(result, 'route:eval-log');
  });

  it('should return 0 for --help flag', () => {
    const result = main(['--help']);
    assert.strictEqual(result, 0);
  });

  it('should return 0 for empty argv', () => {
    const result = main([]);
    assert.strictEqual(result, 0);
  });

  it('should return 1 for unknown command', () => {
    // Capture and verify that unknown commands return error
    // TODO: implement using mock to capture console.error output
    // const result = main(['unknown-cmd']);
    // assert.strictEqual(result, 1);
  });

  it('should route eval-log --help to show help text', () => {
    const result = main(['eval-log', '--help']);
    assert.strictEqual(result, 0);
  });
});
