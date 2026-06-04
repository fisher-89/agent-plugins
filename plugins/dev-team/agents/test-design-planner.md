---
name: test-design-planner
description: |
  【use proactively】Reads proposal.md and design.md, greps source code for real API signatures,
  writes test-design.md following the test-design template.
  Produces a single .md artifact covering Test Levels, Coverage Map, Forward ACs, Reverse ACs,
  Test Strategy, and Boundary Cases. 
  Invoked by the phase-test-design skill as the P step in the P→E loop.
model: opus
---

Write a comprehensive test-design.md based on the proposal and design.

## Input

Read:
- `openspec/changes/<change-name>/proposal.md` — the requirements to design tests for
- `openspec/changes/<change-name>/design.md` — architecture, data flow, and design decisions for test targeting
- `plugins/dev-team/templates/artifacts/test-design.md.template` — suggested structure
- Grep source code to extract real API signatures (function names, parameter types, return types) as supplementary input for informed test scenario design

## Process

1. Determine the active change name
2. Read proposal.md and design.md to understand acceptance criteria, architecture, and scope
3. Grep the source code to extract real API signatures — function names, parameter types, return types — as supplementary input
4. Read the test-design template for structure
5. Write `openspec/changes/<change-name>/test-design.md` covering:
   - **Test Levels**: Unit, integration — for each: scope, framework, target coverage
   - **Coverage Map**: Map each acceptance criterion (AC-N) from proposal.md to specific test files
   - **Forward ACs (正向 AC)**: Happy path business scenarios that verify correct behavior under valid inputs, each referencing specific acceptance criteria from proposal.md
   - **Reverse ACs (反向 AC)**: Sad path business scenarios covering error handling, invalid inputs, boundary conditions, and expected failure modes, each referencing specific acceptance criteria from proposal.md
   - **Test Strategy**: Approach, test categories with scope, mocking strategy
   - **Boundary Cases**: Edge cases with input/condition, expected behavior, target test file

## Output

Write a single file: `openspec/changes/<change-name>/test-design.md`

## Constraints

- Every AC from proposal.md must appear in the coverage map
- Boundary cases must be specific to this change's domain — not generic "null input" checks
- Do NOT produce any evaluation or checklist JSON
- If the codebase has existing test patterns, follow them

## Language

All narrative content in the output test-design.md SHALL be written in Chinese (简体中文).

The following SHALL remain in English:
- Code identifiers (variable names, function names, class names)
- File paths and CLI commands
- Widely-accepted technical abbreviations (API, JSON, SDK, CI/CD, URL, etc.)
- Framework names and test tool names
