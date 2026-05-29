#!/usr/bin/env node
/**
 * test_eval_check_helper.js — Standalone helper that replicates the core
 * eval-check phase gating logic from workflow.ts + eval-check.ts.
 *
 * Usage:
 *   node test_eval_check_helper.js \
 *       --change <change-name> \
 *       --phase <phase-id> \
 *       [--project-root <project-root>]
 *
 * Reads <project-root>/openspec/changes/<change>/eval.json, runs all prior-phase
 * checks (gate, timestamp order, backtrack), and outputs a JSON result.
 *
 * Exit codes:
 *   0  = all checks pass (gate open)
 *   1  = one or more checks fail (gate blocked) or an error occurred
 *
 * This script is intentionally self-contained (no imports from the dev-team bundle)
 * so it can run independently of the MCP server binary.
 */

"use strict";

const fs = require("fs");
const path = require("path");

// =============================================================================
// Core logic — mirrors plugins/dev-team/bin/src/lib/workflow.ts
// =============================================================================

const PHASES = [
  "01-requirements",
  "02-dev-design",
  "03-test-design",
  "04-test-gen",
  "05-implement",
  "06-unit-test",
  "07-code-review",
  "08-integration-test",
  "09-acceptance",
];

/**
 * Return the index of a phase in the PHASES array.
 * Returns -1 if the phase is not found (fault-tolerant).
 */
function getPhaseIndex(phase) {
  return PHASES.indexOf(phase);
}

/**
 * Return the list of phases that come before the given phase.
 * - If phase is the first phase, returns an empty array.
 * - If phase is not in PHASES, returns an empty array (fault-tolerant).
 */
function getPriorPhases(phase) {
  const idx = getPhaseIndex(phase);
  if (idx <= 0) return [];
  return PHASES.slice(0, idx);
}

// =============================================================================
// Core logic — mirrors plugins/dev-team/bin/src/lib/eval-json.ts
// =============================================================================

/**
 * Check that all prior phases have at least one entry with verdict "pass".
 */
function checkGate(entries, priorPhases) {
  const missing = [];
  for (const pp of priorPhases) {
    const hasPass = entries.some((e) => e.phase === pp && e.verdict === "pass");
    if (!hasPass) missing.push(pp);
  }
  return { passed: missing.length === 0, missing };
}

// =============================================================================
// Core logic — mirrors plugins/dev-team/bin/src/commands/eval-check.ts
// =============================================================================

/**
 * Check that the latest pass record timestamps for prior phases are
 * monotonically non-decreasing.
 */
function checkTimestampOrder(entries, priorPhases) {
  if (priorPhases.length < 2) {
    return { passed: true, order_valid: true };
  }

  const timestamps = [];
  for (const pp of priorPhases) {
    const passEntries = entries
      .filter((e) => e.phase === pp && e.verdict === "pass" && !e.skipped)
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

    if (passEntries.length === 0) continue;
    timestamps.push({ phase: pp, ts: passEntries[0].timestamp });
  }

  for (let i = 1; i < timestamps.length; i++) {
    const prev = new Date(timestamps[i - 1].ts).getTime();
    const curr = new Date(timestamps[i].ts).getTime();
    if (curr < prev) {
      return {
        passed: false,
        order_valid: false,
        issues: [
          `${timestamps[i].phase} (${timestamps[i].ts}) timestamp is earlier than ${timestamps[i - 1].phase} (${timestamps[i - 1].ts})`,
        ],
      };
    }
  }

  return { passed: true, order_valid: true };
}

/**
 * Check if any prior phase's latest entry has a non-null backtrack_to field.
 */
function checkBacktrack(entries, priorPhases) {
  const activeBacktrackPhases = [];
  for (const pp of priorPhases) {
    const phaseEntries = entries
      .filter((e) => e.phase === pp)
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

    if (phaseEntries.length === 0) continue;

    const latest = phaseEntries[0];
    if (latest.backtrack_to != null && latest.backtrack_to !== "") {
      activeBacktrackPhases.push(pp);
    }
  }

  return {
    passed: activeBacktrackPhases.length === 0,
    active_backtrack_phases: activeBacktrackPhases,
  };
}

/**
 * Determine the state of the current phase based on its eval entries.
 */
function determinePhaseState(entries, currentPhase) {
  const phaseEntries = entries
    .filter((e) => e.phase === currentPhase)
    .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

  if (phaseEntries.length === 0) return "first_run";

  const latest = phaseEntries[0];
  if (latest.skipped) return "passed";
  if (latest.verdict === "pass") return "passed";
  return "retry";
}

/**
 * Build the aggregate result from all check components.
 */
function buildResult(phase, priorPhases, gateResult, timestampResult, backtrackResult, phaseState) {
  const blockReasons = [];

  if (!gateResult.passed) {
    blockReasons.push(`Prior phase gate not passed: missing [${gateResult.missing.join(", ")}] pass records`);
  }

  if (!timestampResult.passed) {
    if (timestampResult.issues && timestampResult.issues.length > 0) {
      blockReasons.push(`Timestamp order check failed: ${timestampResult.issues.join("; ")}`);
    } else {
      blockReasons.push("Timestamp order check failed");
    }
  }

  if (!backtrackResult.passed) {
    for (const bp of backtrackResult.active_backtrack_phases) {
      blockReasons.push(`Prior phase ${bp} has active backtrack marker`);
    }
  }

  return {
    passed: gateResult.passed && timestampResult.passed && backtrackResult.passed,
    phase,
    prior_phases: priorPhases,
    block_reasons: blockReasons,
    phase_state: phaseState,
    details: {
      prior_phase_gate: {
        passed: gateResult.passed,
        missing: gateResult.missing,
      },
      timestamp_order: {
        passed: timestampResult.passed,
        order_valid: timestampResult.order_valid,
      },
      backtrack: {
        passed: backtrackResult.passed,
        active_backtrack_phases: backtrackResult.active_backtrack_phases,
      },
    },
  };
}

// =============================================================================
// Main entry point
// =============================================================================

function parseArgs() {
  const args = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith("--")) {
      const key = argv[i].slice(2);
      args[key] = argv[++i];
    }
  }
  return args;
}

function main() {
  const args = parseArgs();
  const change = args.change;
  const phase = args.phase;
  const projectRoot = args["project-root"] || process.env.CLAUDE_PROJECT_DIR || process.cwd();

  if (!change) {
    console.error("Missing required argument: --change <change-name>");
    process.exit(2);
  }
  if (!phase) {
    console.error("Missing required argument: --phase <phase-id>");
    process.exit(2);
  }

  // Resolve paths
  const changeDir = path.resolve(projectRoot, "openspec", "changes", change);
  if (!fs.existsSync(changeDir)) {
    const result = {
      passed: false,
      error: `Change directory not found: ${changeDir}`,
    };
    console.log(JSON.stringify(result));
    process.exit(1);
  }

  // Read eval.json
  let entries = [];
  const evalFile = path.join(changeDir, "eval.json");
  if (fs.existsSync(evalFile)) {
    try {
      const raw = fs.readFileSync(evalFile, "utf-8");
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        throw new Error("eval.json root must be an array");
      }
      entries = parsed;
    } catch (e) {
      const result = {
        passed: false,
        error: `Failed to read eval.json: ${e.message}`,
      };
      console.log(JSON.stringify(result));
      process.exit(1);
    }
  }

  // Run checks
  const priorPhases = getPriorPhases(phase);
  const gateResult = checkGate(entries, priorPhases);
  const timestampResult = checkTimestampOrder(entries, priorPhases);
  const backtrackResult = checkBacktrack(entries, priorPhases);
  const phaseState = determinePhaseState(entries, phase);

  const result = buildResult(phase, priorPhases, gateResult, timestampResult, backtrackResult, phaseState);

  console.log(JSON.stringify(result, null, 2));
  if (!result.passed) {
    process.exit(1);
  }
}

main();
