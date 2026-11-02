pub mod coverage;
pub mod js;
pub mod junit;
pub mod rust;
pub mod text;

pub use coverage::{parse_coverage, parse_coverage_measured, CoverageFormat};
pub use js::{parse_istanbul_results, parse_istanbul_summary, IstanbulResults};
pub use junit::parse_junit;
pub use rust::parse_llvm_cov;
pub use text::parse_spec_report;

#[cfg(test)]
mod coverage_test;
#[cfg(test)]
mod js_test;
#[cfg(test)]
mod junit_test;
#[cfg(test)]
mod rust_test;
#[cfg(test)]
mod text_test;
