
=== packages_desktop_vite-plus (vite-plus) · files=3 ===
npx --prefix "D:/Projects/.worktrees/wps-claude-plugin/archi-file-tree/packages/desktop" stryker run "D:/Projects/.worktrees/wps-claude-plugin/archi-file-tree/openspec/changes/archi-file-tree/reports/test/packages_desktop_vite-plus/mutation/mutation-stryker-seg-0.conf.json"

=== packages_desktop_src-tauri_rust (rust) · files=13 ===
cargo mutants --test-tool=nextest --workspace --in-diff D:/Projects\.worktrees\wps-claude-plugin\archi-file-tree\openspec\changes\archi-file-tree\reports\test\packages_desktop_src-tauri_rust\mutation\mutation-seg-0.patch --baseline skip --jobs 4 --timeout 300 --profile mutants --gitignore true --cargo-test-arg -j4
