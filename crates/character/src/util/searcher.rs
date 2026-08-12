use std::collections::BTreeSet;

use crate::types::CodePoint;

pub type CodePointSearcher = Box<dyn Fn(CodePoint) -> bool + Send + Sync + 'static>;

pub fn create_code_point_searcher(
    code_points: &[CodePoint],
) -> (CodePointSearcher, Vec<CodePoint>) {
    let ordered: Vec<CodePoint> = BTreeSet::from_iter(code_points.iter().copied())
        .into_iter()
        .collect();

    let lookup = ordered.clone();
    let searcher: CodePointSearcher =
        Box::new(move |target: CodePoint| lookup.binary_search(&target).is_ok());
    (searcher, ordered)
}

pub fn collect_code_points_from_slice(values: &[CodePoint]) -> Vec<CodePoint> {
    values.to_vec()
}

pub fn collect_code_points_from_enum(values: &[CodePoint]) -> Vec<CodePoint> {
    values.to_vec()
}
