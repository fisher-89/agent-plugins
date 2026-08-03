# Mutation score gap fill (< 60)

Source report (archived change):
`openspec/changes/archive/2026-07-31-test-execution-result-file/reports/test/plugins_dev-team_vite-plus/mutation.json`

Project root in report: `plugins/dev-team` (vite-plus / Stryker). Threshold `low: 60`.

## Files with mutationScore < 60

| File | Score | Killed | Survived | NoCoverage |
|------|------:|-------:|---------:|-----------:|
| `bin/src/lib/test-framework.ts` | 11.49 | 20 | 133 | 21 |
| `bin/src/lib/test-runner.ts` | 31.19 | 121 | 189 | 78 |
| `bin/src/lib/test-parser/stryker-config.ts` | 32.81 | 21 | 24 | 19 |
| `bin/src/lib/test-report.ts` | 45.06 | 283 | 311 | 34 |
| `bin/src/lib/test-parser/go-parser.ts` | 50.00 | 71 | 53 | 18 |
| `bin/src/lib/test-parser/text-parser.ts` | 53.19 | 200 | 173 | 3 |
| `bin/src/commands/test-detect-frameworks.ts` | 58.85 | 113 | 46 | 33 |

## Goal

Add / strengthen colocated unit tests so each of the seven files above reaches mutation score ≥ 60 (Stryker: killed / (killed + survived + noCoverage)). Prefer killing Survived and covering NoCoverage mutants; do not change production logic unless a real bug is found.

## Out of scope (already ≥ 60)

- `coverage-parser.ts` 64.21
- `js-parser.ts` 77.12
- `index.ts` 83.12
- `mutation-parser.ts` 90.41
- `test-execution.ts` 95.70
