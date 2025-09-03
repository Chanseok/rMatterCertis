//! Unit tests: `DbAnalysisResult::calculate_next_start_position`

use matter_certis_v2_lib::application::shared_state::DbAnalysisResult;

#[test]
fn next_start_none_when_empty() {
    let db = DbAnalysisResult::new(0, None, None, 1.0);
    assert!(db.calculate_next_start_position(12).is_none());
}

#[test]
fn next_start_same_page_when_index_not_last() {
    let db = DbAnalysisResult::new(10, Some(3), Some(5), 0.9);
    let next = db.calculate_next_start_position(12).expect("should exist");
    assert_eq!(next, (3, 6));
}

#[test]
fn next_start_next_page_when_index_is_last() {
    let db = DbAnalysisResult::new(10, Some(3), Some(11), 0.9);
    let next = db.calculate_next_start_position(12).expect("should exist");
    assert_eq!(next, (4, 0));
}
