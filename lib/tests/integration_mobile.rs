
mod common;

mod tests_mobile_requests {
    use crate::common::env_auth;

    #[test]
    #[cfg(feature = "mobile")]
    fn test_get_exams() {
        let stine = env_auth();
        let exam_results = stine.get_exams_mobile()
            .expect("Failed fetching exam result from the mobile endpoint");
        assert!(!exam_results.exams.is_empty());
    }
}
