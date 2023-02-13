use stine_rs::Stine;

//TODO: use env var
fn auth() -> Stine {
    Stine::new("***REMOVED***", "***REMOVED***")
        .expect("Failed authenticating with Stine")
}


#[cfg(test)]
mod test_auth {
    use stine_rs::Stine;
    use crate::auth;

    #[test]
    fn test_credentials() {
        auth();
    }

    #[test]
    fn test_short_session() {
        let s = auth();
        let session = s.session.unwrap();
        let cnsc = s.cnsc_cookie.unwrap();

        Stine::new_session(cnsc.as_str(), session.as_str()).unwrap();
    }

    // #[test]
    // fn test_long_session() {
    //     let s = Stine::new("***REMOVED***", "***REMOVED***").unwrap();
    //     let session = s.session.unwrap();
    //     let cnsc = s.cnsc_cookie.unwrap();
    //
    //     Stine::new_session(cnsc.as_str(), session.as_str()).unwrap();
    // }
}

#[cfg(test)]
mod test_functionality {
    use std::sync::Mutex;
    use std::time::Instant;
    use stine_rs::{Language, LazyLevel, Stine, SubModule};
    use lazy_static::lazy_static;
    use crate::auth;

    lazy_static! {
        static ref STINE: Mutex<Stine> = Mutex::new(auth());
    }

    #[test]
    fn test_get_my_registrations_not_lazy() {
        let instant = Instant::now();

        let module_categories = STINE.lock().unwrap()
            .get_my_registrations(LazyLevel::NotLazy).unwrap();
        assert!(!module_categories.accepted_modules.is_empty());
        assert!(!module_categories.accepted_submodules.is_empty());

        println!("Test `test_get_my_registrations_not_lazy` took: {:#?}", instant.elapsed());
    }

    #[test]
    fn test_get_my_registrations_full_lazy() {
        let instant = Instant::now();

        let module_categories = STINE.lock().unwrap()
            .get_my_registrations(LazyLevel::FullLazy).unwrap();
        assert!(!module_categories.accepted_modules.is_empty());
        assert!(!module_categories.accepted_submodules.is_empty());

        println!("Test `test_get_my_registrations_full_lazy` took: {:#?}", instant.elapsed());
    }

    #[test]
    fn test_get_my_registrations_lazyloading() {
        let instant = Instant::now();
        let mut stine: Stine = auth();

        let module_categories = stine
            .get_my_registrations(LazyLevel::FullLazy).unwrap();

        let mut first_submodule: SubModule= module_categories.accepted_submodules.first()
            .cloned().unwrap();
        let apps = first_submodule.appointments(&stine);
        let info = first_submodule.info(&stine);
        let groups = first_submodule.groups(&stine);

        println!("Test `test_get_my_registrations_lazyloading` took: {:#?}", instant.elapsed());
    }

    #[test]
    fn test_get_documents() {
        let documents = STINE.lock().unwrap().get_documents().unwrap();
        assert!(!documents.is_empty())
    }

    #[test]
    fn test_get_semester_results() {
        let results = STINE.lock().unwrap()
            .get_all_semester_results().unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_language() {
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