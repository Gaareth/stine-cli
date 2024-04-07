
mod common;

mod tests_mobile_requests {
    use crate::common::env_auth;

    #[test]
    fn test_get_exams() {
        let stine = env_auth();
        dbg!(&stine.get_exams_mobile().unwrap());
    }
}
