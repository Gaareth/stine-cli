
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use lazy_static::lazy_static;

use stine_rs::Stine;
use crate::common::env_auth;

mod common;

mod test_auth {
    use stine_rs::Stine;
    use crate::common::env_auth;

    #[test]
    fn test_credentials() {
        env_auth();
    }

    #[test]
    fn test_short_session() {
        let s = env_auth();
        let session = s.session.unwrap();
        let cnsc = s.cnsc_cookie.unwrap();

        Stine::new_session(cnsc.as_str(), session.as_str()).unwrap();
    }
}

lazy_static! {
        static ref TEST_CACHE_DIR: PathBuf = dirs::cache_dir().unwrap().join("stine-rs").join("dev-test");
        static ref STINE: Mutex<Stine> = Mutex::new(auth_test_cache());
    }

fn auth_test_cache() -> Stine {
    env_auth().with_cache_dir(
        TEST_CACHE_DIR.to_path_buf()
    ).unwrap()
}

fn clear_test_cache_dir() {
    fs::remove_dir_all(TEST_CACHE_DIR.to_path_buf()).unwrap();
    fs::create_dir_all(TEST_CACHE_DIR.to_path_buf()).unwrap();
}

fn init_logger() {
    let _ = env_logger::builder()
        .filter_module("stine", log::LevelFilter::max())
        .filter_module("integration", log::LevelFilter::max())
        // print directly to stdout?
        .is_test(false)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}


mod test_functionality {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Instant;

    use lazy_static::lazy_static;
    use log::info;
    use reqwest::header::CONTENT_TYPE;

    use stine_rs::{Language, LazyLevel, Stine, SubModule};

    use crate::{auth_test_cache, clear_test_cache_dir, init_logger, STINE};
    use crate::common::env_auth;

    #[test]
    fn test_output_log() {
        init_logger();
        info!("test");
        env_auth();
    }

    #[test]
    #[ignore]
    fn test_get_my_registrations_not_lazy() {
        init_logger();
        clear_test_cache_dir();

        let instant = Instant::now();

        let module_categories = STINE.lock().unwrap()
            .get_my_registrations(LazyLevel::NotLazy).unwrap();
        assert!(!module_categories.accepted_modules.is_empty());
        assert!(!module_categories.accepted_submodules.is_empty());

        assert!(module_categories.accepted_submodules.first().unwrap().info_loaded());
        assert!(module_categories.accepted_submodules.first().unwrap().groups_loaded());
        assert!(module_categories.accepted_submodules.first().unwrap().appointments_loaded());


        println!("Test `test_get_my_registrations_not_lazy` took: {:#?}", instant.elapsed());
    }

    #[test]
    #[ignore]
    fn test_get_my_registrations_full_lazy() {
        init_logger();
        clear_test_cache_dir();

        let instant = Instant::now();

        let module_categories = STINE.lock().unwrap()
            .get_my_registrations(LazyLevel::FullLazy).unwrap();
        assert!(!module_categories.accepted_modules.is_empty());
        assert!(!module_categories.accepted_submodules.is_empty());

        assert!(!module_categories.accepted_submodules.first().unwrap().info_loaded());
        assert!(!module_categories.accepted_submodules.first().unwrap().groups_loaded());
        assert!(!module_categories.accepted_submodules.first().unwrap().appointments_loaded());

        println!("Test `test_get_my_registrations_full_lazy` took: {:#?}", instant.elapsed());
    }

    #[test]
    #[ignore]
    fn test_get_my_registrations_lazyloading() {
        init_logger();
        clear_test_cache_dir();

        let instant = Instant::now();
        let mut stine: Stine = auth_test_cache();

        let module_categories = stine
            .get_my_registrations(LazyLevel::FullLazy).unwrap();

        let mut first_submodule: SubModule = module_categories.accepted_submodules.first()
            .cloned().unwrap();

        assert!(!first_submodule.info_loaded());
        assert!(!first_submodule.groups_loaded());
        assert!(!first_submodule.appointments_loaded());

        let apps = first_submodule.appointments(&stine);
        let info = first_submodule.info(&stine);
        let groups = first_submodule.groups(&stine);

        assert!(first_submodule.info_loaded());
        assert!(first_submodule.groups_loaded());
        assert!(first_submodule.appointments_loaded());

        println!("Test `test_get_my_registrations_lazyloading` took: {:#?}", instant.elapsed());
        // TODO: add to cache
    }

    #[test]
    fn test_get_documents() {
        init_logger();

        let documents = STINE.lock().unwrap().get_documents().unwrap();
        assert!(!documents.is_empty())
    }

    #[test]
    fn test_download_document() {
        init_logger();

        let documents = STINE.lock().unwrap().get_documents().unwrap();
        let d = documents.get(0).unwrap();
        let response = STINE.lock().unwrap().get(&d.download).unwrap();
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap().to_str().unwrap(), "application/pdf");
    }

    #[test]
    fn test_get_semester_results() {
        init_logger();

        let results = STINE.lock().unwrap()
            .get_all_semester_results(LazyLevel::NotLazy).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_language() {
        init_logger();

        let og_lang = STINE.lock().unwrap().get_language().unwrap();

        STINE.lock().unwrap().set_language(&Language::English).unwrap();
        let lang = STINE.lock().unwrap().get_language().unwrap();
        assert_eq!(lang, Language::English);

        STINE.lock().unwrap().set_language(&Language::German).unwrap();
        let lang = STINE.lock().unwrap().get_language().unwrap();
        assert_eq!(lang, Language::German);

        // reset for user
        STINE.lock().unwrap().set_language(&og_lang).unwrap();
    }

    #[test]
    fn test_get_registration_periods() {
        init_logger();

        let periods = STINE.lock().unwrap().get_registration_periods().unwrap();
        assert!(!periods.is_empty());
        assert_eq!(periods.len(), 5);
    }

// #[test]
// fn test_get_registration_modules() {
//     let module_categories = STINE.lock().unwrap().get_registration_modules(
//         true, false, true).unwrap();
//     assert!(!module_categories.is_empty())
// }
}

#[cfg(feature = "mobile")]
mod test_functionality_mobile {
    use crate::{init_logger, STINE};

    #[test]
    fn test_actor_type() {
        init_logger();
        STINE.lock().unwrap().get_actor_type().unwrap();
    }

    #[test]
    #[ignore]
    fn test_student_events() {
        init_logger();
        STINE.lock().unwrap().get_student_events().unwrap();
    }
}