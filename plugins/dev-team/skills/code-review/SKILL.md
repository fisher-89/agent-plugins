---
name: code-review
description: Review staged git changes before commit. Check for logical errors, null/boundary handling, and redundant logic.
license: MIT
---

Review current staged git changes for code quality issues.

## Usage

```
/code-review
```

## Focus Areas

1. **Logical Errors**
   - Implementation doesn't match intent
   - Algorithm defects
   - Conditional logic mistakes

2. **Null/Boundary Handling**
   - Unhandled null/undefined/None
   - Array index out of bounds
   - Empty collection handling
   - Missing numeric boundary checks

3. **Redundant Logic**
   - Unnecessary conditions
   - Duplicate fallback paths
   - Over-defensive code
   - Dead code branches

## Process

1. Get staged changes:
   ```bash
   git diff --cached --stat
   git diff --cached
   ```

2. Read relevant source files for context

3. Analyze each changed section

4. Report findings in structured format

## Output Format

```
## Code Review

**Staged Files**: <file list>

### Issues Found
- [ERROR] <file>:<line> - <description>
  - Fix: <suggestion>

- [WARN] <file>:<line> - <description>
  - Fix: <suggestion>

### Summary
<overall assessment>

### Next Steps
<action items or "Safe to commit">
```

If no critical issues: "LGTM - No critical issues found"
