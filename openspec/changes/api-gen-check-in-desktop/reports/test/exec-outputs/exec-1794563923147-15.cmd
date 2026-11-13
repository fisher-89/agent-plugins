
=== packages_desktop_vite-plus (vite-plus) · files=5 ===
npx --prefix "D:/Projects/.worktrees/wps-claude-plugin/api-gen-check-in-desktop/packages/desktop" stryker run "D:/Projects/.worktrees/wps-claude-plugin/api-gen-check-in-desktop/openspec/changes/api-gen-check-in-desktop/reports/test/packages_desktop_vite-plus/mutation/mutation-stryker-seg-0.conf.json"

=== packages_desktop_src-tauri_rust (rust) · files=60 ===
cargo mutants --test-tool=nextest --workspace --in-diff D:/Projects\.worktrees\wps-claude-plugin\api-gen-check-in-desktop\openspec\changes\api-gen-check-in-desktop\reports\test\packages_desktop_src-tauri_rust\mutation\mutation-seg-0.patch --baseline skip --jobs 4 --timeout 300 --profile mutants --gitignore true --cargo-test-arg -j4
