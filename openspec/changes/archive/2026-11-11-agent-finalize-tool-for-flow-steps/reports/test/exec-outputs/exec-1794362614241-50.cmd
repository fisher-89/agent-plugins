
=== packages_desktop_vite-plus (vite-plus) · files=2 ===
npx --prefix "D:/Projects/.worktrees/wps-claude-plugin/agent-finalize-tool-for-flow-steps/packages/desktop" stryker run "D:/Projects/.worktrees/wps-claude-plugin/agent-finalize-tool-for-flow-steps/openspec/changes/agent-finalize-tool-for-flow-steps/reports/test/packages_desktop_vite-plus/mutation/mutation-stryker-seg-0.conf.json"

=== packages_desktop_src-tauri_rust (rust) · files=55 ===
cargo mutants --test-tool=nextest --workspace --in-diff D:/Projects\.worktrees\wps-claude-plugin\agent-finalize-tool-for-flow-steps\openspec\changes\agent-finalize-tool-for-flow-steps\reports\test\packages_desktop_src-tauri_rust\mutation\mutation-seg-0.patch --baseline skip --jobs 4 --timeout 300 --profile mutants --gitignore true --cargo-test-arg -j4
