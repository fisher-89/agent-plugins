pub mod aggregate;
pub mod diagnose;
pub mod globmatch;
pub mod model;
pub mod parser;
pub mod reuse;

pub use aggregate::{
    aggregate_coverage, aggregate_measured, build_summary_report, coverage_meets_thresholds,
    derive_plan_id, determine_conclusion, in_suite_scope,
};
pub use diagnose::{check_integrity, diagnose_findings};
pub use model::{
    parse_sub_report, parse_summary_report, CaseSummary, Conclusion, CoverageBlock,
    CoverageMeasured, CoverageOverride, CoverageThresholds, FileCoverageEntry, MutationBlock,
    MutationMeasured, MutationOverride, PlanIndexEntry, Problem, ProblemType, SourceFileEntry,
    SubReport, SummaryReport, TestCaseResult, TestCaseStatus,
};
pub use parser::{
    parse_cargo_test, parse_coverage, parse_coverage_measured, parse_istanbul_summary,
    parse_llvm_cov, parse_spec_report, CoverageFormat,
};
pub use reuse::is_reusable;

#[cfg(test)]
mod aggregate_test;
#[cfg(test)]
mod diagnose_test;
#[cfg(test)]
mod globmatch_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod reuse_test;
