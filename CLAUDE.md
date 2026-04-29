# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is **wps-claude-plugin**, a custom Claude Code plugin designed to enhance development workflow with OpenSpec integration.

## Repository Structure

- `.claude-plugin/plugin.json` — Plugin manifest
- `hooks/hooks.json` — Hook configuration (UserPromptSubmit hook)
- `hooks/on-user-prompt.py` — Hook script: checks openspec changes on user prompt
- `plugin/` — Plugin source directory (skills, commands)
  - `plugin/skills/` — OpenSpec skills (explore, propose, apply-change, archive-change)
  - `plugin/commands/opsx/` — Slash commands for OpenSpec workflow
  - `plugin/hooks/` — Plugin-local copy of hooks
- `demo-project/` — Demo project for testing plugin behavior

## Hook: UserPromptSubmit

When a user submits a prompt, the hook:
1. Scans `openspec/changes/` in the working project for active changes
2. Reports change names, artifacts, and task progress
3. Instructs Claude to check if documents need updates before responding

## Plugin Identity

- **Name:** wps-claude-plugin
- **Version:** 1.0.0
- **Author:** zhangbohan