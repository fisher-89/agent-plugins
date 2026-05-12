## REMOVED Requirements

### Requirement: Intent classification from prompt text
**Reason**: Automatic intent classification via regex keyword matching is unreliable and removes user control. Users will manually invoke skills via slash commands instead.
**Migration**: Use explicit slash commands: `/dev-team:openspec-explore` to explore, `/dev-team:openspec-propose` to propose, `/dev-team:openspec-apply-change` to implement, `/dev-team:openspec-archive-change` to archive. No replacement for automatic classification.

### Requirement: Confidence level assignment
**Reason**: Part of the removed intent classification system.
**Migration**: Not needed — users explicitly choose their action.

### Requirement: Scope estimation from prompt text
**Reason**: Part of the removed intent classification system. Scope estimation from prompt text was inherently unreliable.
**Migration**: Scope decisions are made during the explore/propose phase rather than inferred from prompt keywords.

### Requirement: Trivial fix detection
**Reason**: The trivial/non-trivial distinction was used to decide whether to apply workflow gates. With manual step selection, the user decides whether a change needs the full SDD workflow.
**Migration**: Users can still make small fixes directly without invoking any skill — the TDD gate will only trigger if an active OpenSpec change exists.

### Requirement: Routing decision based on intent, scope, and state
**Reason**: The entire routing engine is removed. Routing was the core of the auto-intent system, producing auto/suggest/direct decisions that the user never explicitly requested.
**Migration**: Users manually select which step to execute. PreToolUse hooks continue to enforce workflow quality gates regardless of how the user arrived at the action.
