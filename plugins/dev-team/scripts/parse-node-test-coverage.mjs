#!/usr/bin/env node
/**
 * Parse node:test --experimental-test-coverage stdout text table
 * and write istanbul-compatible coverage-summary.json.
 *
 * Usage: node parse-node-test-coverage.mjs <stdout-file> <output-json-path>
 */

import { readFileSync, writeFileSync } from 'node:fs';

function parseCoverageTable(content) {
  const rows = content.split(/\r?\n/);

  for (const row of rows) {
    // Node:test outputs 4 numeric columns: % Stmts | % Branch | % Funcs | % Lines
    const match = row.match(
      /all\s+files\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)/i,
    );
    if (match) {
      return {
        lines: parseFloat(match[4]),
        branches: parseFloat(match[2]),
        functions: parseFloat(match[3]),
      };
    }
  }

  return null;
}

function buildIstanbulSummary({ lines, branches, functions }) {
  return {
    total: {
      lines: { pct: lines },
      branches: { pct: branches },
      functions: { pct: functions },
    },
  };
}

function main() {
  const [stdoutFile, outputPath] = process.argv.slice(2);

  if (!stdoutFile || !outputPath) {
    console.error(
      'Usage: node parse-node-test-coverage.mjs <stdout-file> <output-json-path>',
    );
    process.exit(1);
  }

  if (stdoutFile.trim() === '' || outputPath.trim() === '') {
    console.error('Error: stdout file path and output path must be non-empty');
    process.exit(1);
  }

  let content;
  try {
    content = readFileSync(stdoutFile, 'utf8');
  } catch (err) {
    console.error(`Error: cannot read stdout file "${stdoutFile}": ${err.message}`);
    process.exit(1);
  }

  const coverage = parseCoverageTable(content);
  if (!coverage) {
    console.error(
      'Error: no coverage summary table found in stdout (expected "all files" row)',
    );
    process.exit(1);
  }

  for (const [dim, value] of Object.entries(coverage)) {
    if (Number.isNaN(value)) {
      console.error(`Error: invalid ${dim} coverage value in summary table`);
      process.exit(1);
    }
  }

  try {
    writeFileSync(outputPath, JSON.stringify(buildIstanbulSummary(coverage), null, 2) + '\n');
  } catch (err) {
    console.error(`Error: cannot write output file "${outputPath}": ${err.message}`);
    process.exit(1);
  }
}

main();
