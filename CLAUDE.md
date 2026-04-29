# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **wps-claude-plugin**, a custom Claude Code plugin designed to enhance development workflow with OpenSpec integration.

## Repository Structure

- `.claude-plugin/plugin.json` — Plugin manifest
- `plugin/` — Plugin source directory (skills, commands)
  - `plugin/skills/` — OpenSpec skills (explore, propose, apply-change, archive-change)
  - `plugin/commands/opsx/` — Slash commands for OpenSpec workflow
  - `plugin/hooks/` — Plugin-local copy of hooks
    - `plugin/hooks/hooks.json` — Hook configuration (UserPromptSubmit, PreToolUse)
    - `plugin/hooks/on-user-prompt.py` — Hook script: checks openspec changes on user prompt
    - `plugin/hooks/pre-tool-openspec-test.py` — Hook script: auto-generates test case templates before implementation
- `demo-project/` — Demo project for testing plugin behavior

## Hook: UserPromptSubmit

When a user submits a prompt, the hook:
1. Scans `openspec/changes/` in the working project for active changes
2. Reports change names, artifacts, and task progress
3. Instructs Claude to check if documents need updates before responding

## Hook: PreToolUse (Test Case Generation)

When Claude is about to write/edit code files during an active OpenSpec change:
1. Detects active change with pending tasks
2. Checks if test case template already exists in `openspec/changes/<name>/test-reports/`
3. If not, auto-generates a test case template based on pending tasks
4. Injects context instructing Claude to review/expand test cases before continuing implementation

## Plugin Identity

- **Name:** wps-claude-plugin
- **Version:** 1.0.0
- **Author:** zhangbohan