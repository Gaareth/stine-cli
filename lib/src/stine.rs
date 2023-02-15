use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::copy;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use log::trace;
use regex::Regex;
use reqwest::blocking::Response;
use reqwest::header::{CONTENT_TYPE, COOKIE, HeaderMap, ORIGIN, REFERER, REFRESH, SET_COOKIE};
use scraper::Html;
use thiserror::Error;

use crate::{Document, GradeStats, LazyLevel, parse, utils};
use crate::{Module, ModuleCategory, SubModule};
use crate::{Semester, SemesterResult};
use crate::Language;
use crate::parse::results::{parse_course_results, parse_grade_stats};
use crate::RegistrationPeriod;
use crate::utils::{save_modules, save_submodules};

const API_URL: &str = "https://stine.uni-hamburg.de/scripts/mgrqispi.dll";
pub const BASE_URL: &str = "https://stine.uni-hamburg.de";

type Client = reqwest::blocking::Client;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Wrong password or username")]
    WrongCredentials,
    #[error("Blocked access for `{0}` minutes, due to wrong credentials")]
    WrongCredentialsAccessDenied(i32),
    #[error("Access denied")]
    AccessDenied,
    #[error("Access denied: Due to too many failed login attempts, the account is temporarily locked")]
    TemporarilyLocked,
    #[error("Request error: `{0}`")]
    RequestError(#[from] reqwest::Error),
    #[error("Timeout")]
    Timeout,
}

#[derive(Error, Debug)]
pub enum StineError {
    #[error("Authentication Error: `{0}`")]
    AuthError(#[from] AuthError),
    #[error("Request error: `{0}`")]
    RequestError(#[from] reqwest::Error),
    #[error(transparent)]
    AnyError(#[from] anyhow::Error),
    #[error("Unknown error")]
    UnknownError,
}

pub struct Stine {
    client: Client,
    pub session: Option<String>,
    pub cnsc_cookie: Option<String>,

    pub(crate) language: Option<Language>,

    pub(crate) submod_map: Option<HashMap<String, SubModule>>,
    pub(crate) mod_map: Option<HashMap<String, Module>>,
    pub(crate) mod_categories: Option<Vec<ModuleCategory>>,
}

fn build_client() -> Client {
    // 60s because it's possible that stine timeouts you sometimes :(
    Client::builder().cookie_store(true)
        .timeout(Duration::from_secs(60))
        .build().expect("Error building Client")
}

impl Default for Stine {
    fn default() -> Self {
        Self {
            client: build_client(),
            session: None,
            cnsc_cookie: None,
            language: None,
            submod_map: None,
            mod_map: None,
            mod_categories: None,
        }
    }
}

impl Stine {
    /// Create new Stine object using cnsc_cookie and session string.
    /// Language will be set to your current stine language.
    /// # Error
    ///
    /// will error if there is an `AuthError`, like an expired session
    pub fn new_session(cnsc_cookie: &str, session: &str) -> Result<Self, StineError> {
        let mut stine = Self {
            session: Some(String::from(session)),
            cnsc_cookie: Some(String::from(cnsc_cookie)),
            ..Default::default()
        };
        Self::is_authenticated(&stine)?;

        trace!("Successfully authenticated using session and cookie");
        stine.language = Some(stine.get_language().unwrap());
        Ok(stine)
    }

    /// Creates new Stine object using your username and password
    /// Language will be set to your current stine language.
    /// # Error
    ///
    /// will error if there is an `AuthError`, like wrong credentials
    pub fn new(username: &str, password: &str) -> Result<Self, StineError> {
        let mut stine = Self::authenticate(Stine::default(), username, password)?;
        Self::is_authenticated(&stine)?;

        trace!("Successfully authenticated using username and password");
        stine.language = Some(stine.get_language().unwrap());
        Ok(stine)
    }

    /// Set language of stine results
    pub fn with_language(&mut self, language: Language) -> &mut Stine {
        self.set_language(&language).unwrap();
        self.language = Some(language);
        self
    }

    fn is_authenticated(stine: &Self) -> Result<bool, StineError> {
        let response = stine.post_with_arg("MLSSTART", vec![])?;
        let text = response.text()?;

        Ok(Self::check_for_error(text)?)
    }

    fn check_for_error(text: String) -> Result<bool, AuthError> {
        if text.contains("<h1>Kennung oder Kennwort falsch</h1>") {
            return Err(AuthError::WrongCredentials);
        } else if text.contains("<h1>Kennung oder Kennwort falsch - Zugang verweigert</h1>") {
            let timeout_re: Regex = Regex::new(r"(\d*) minutes").unwrap();
            if let Some(caps) = timeout_re.captures(text.as_str()) {
                if let Some(timeout) = caps.get(1) {
                    if let Ok(timeout) = timeout.as_str().parse::<i32>() {
                        return Err(AuthError::WrongCredentialsAccessDenied(timeout));
                    }
                }
            }
            return Err(AuthError::WrongCredentials);
        } else if text.contains("<h1>Zugang verweigert</h1>") {
            return Err(AuthError::AccessDenied);
        } else if text.contains("<h1>Anmeldung zur Zeit nicht möglich</h1>") {
            return Err(AuthError::TemporarilyLocked);
        } else if text.contains("<h1>Timeout</h1>") || text.contains("<h1>Timeout!</h1>") {
            return Err(AuthError::Timeout);
        }


        Ok(true)
    }

    /// Checks and returns the actual error in case some error happens.
    /// If no known error can be found returns [`AuthError::AnyError`] with the error message.
    fn on_auth_error(stine: Self, params: HashMap<&str, &str>, error: anyhow::Error) -> StineError {
        let response = Self::post_static(&stine.client, API_URL,
                                         HeaderMap::new(), params);

        if let Ok(response) = response {
            return match Self::check_for_error(response.text().unwrap()) {
                Ok(_) => StineError::AnyError(error),
                Err(error) => StineError::AuthError(error),
            };
        }
        StineError::RequestError(response.unwrap_err())
    }

    fn authenticate(stine: Self, username: &str, password: &str) -> Result<Self, StineError> {
        let params = HashMap::from([
            ("usrname", username),
            ("pass", password),
            ("APPNAME", "CampusNet"),
            ("PRGNAME", "LOGINCHECK"),
            ("ARGUMENTS", "clino,usrname,pass,menuno,menu_type,browser,platform"),
            ("clino", "000000000000001"),
            ("menuno", "000000"),
            ("menu_type", "classic"),
            ("browser", ""),
            ("platform", ""),
        ]);

        let response = Self::post_static(&stine.client, API_URL,
                                         HeaderMap::new(), params.clone())?;

        // Self::save_to_file(response);
        // panic!();
        let headers = response.headers();

        // Self::check_for_error(&response.text().unwrap());

        let refresh_regex = Regex::new(r"-N(\d+)").unwrap();
        let refresh_header = headers.get(REFRESH);
        if refresh_header.is_none() {
            return Err(Self::on_auth_error(stine, params, anyhow!("Missing REFRESH header")));
        }
        let refresh_header = refresh_header.unwrap().to_str().unwrap();

        let mat = refresh_regex.find(refresh_header);
        if mat.is_none() {
            return Err(Self::on_auth_error(stine, params, anyhow!("Missing argument entry in REFRESH header")));
        }
        let mat = mat.unwrap();

        let cookies = headers.get(SET_COOKIE);
        if cookies.is_none() {
            return Err(Self::on_auth_error(stine, params, anyhow!("Missing SET_COOKIE header")));
        }
        let cookies = cookies.unwrap();

        let cnsc_cookie = Some(cookies.to_str().unwrap()
            .split('=').collect::<Vec<&str>>()[1]
            .split(';').collect::<Vec<&str>>()[0].to_string());

        // let first_match = matches.nth(0).expect("Missing argument entry");
        // +2 to remove the "-N"
        let session = Some(refresh_header.to_string()[mat.start() + 2..mat.end()].to_string());

        // set language to english to parse dates properly, see parse.rs
        // self.set_language(Language::English);

        Ok(Self {
            client: stine.client,
            session,
            cnsc_cookie,
            ..Default::default()
        })
    }

    /// Returns the available Documents from your stine account, like "OnlineSemesterbescheinigung"
    pub fn get_documents(&self) -> Result<Vec<Document>, reqwest::Error> {
        let resp = self.post_with_arg("CREATEDOCUMENT", vec![])?;
        Ok(parse::documents::parse_documents(resp.text()?))
    }

    /// Returns the various Registration periods, found under Service > Registration periods
    pub fn get_registration_periods(&self) -> Result<Vec<RegistrationPeriod>, reqwest::Error> {
        let resp = self.post_with_arg("EXTERNALPAGES", vec![
            "-N000385".to_owned(), "-Aanmeldephasen".to_owned(),
        ])?;
        Ok(parse::periods::parse_registration_periods(resp.text()?))
    }

    /// Returns the registration status of the applied modules
    /// # Arguments
    ///     - lazy: Lazy loads certain info, reduces api calls and especially time to fetch the info
    pub fn get_my_registrations(&mut self, lazy: LazyLevel) -> Result<MyRegistrations, anyhow::Error> {
        let resp = self.post_with_arg("MYREGISTRATIONS", vec![])?;
        Ok(parse::registrations::parse_my_registrations(resp.text()?, self, lazy))
    }

    /// Returns all modules you can register for.
    /// **Note**: By default, this information, will be loaded from a cache file, because
    /// **Warning**: scraping this info, can take several minutes
    /// # Arguments
    ///     - force_reload: scrape all module from stine, without loading them from the cache file. *ONLY DO THIS SPARSELY PLEASE, TAKES MULTIPLE MINUTES*
    ///     - print_progress_bar: prints a nice progress bar and more status info to the stdout.
    ///     - lazy: Lazy loads certain info, reduces api calls and especially time to fetch the info
    ///
    /// # Errors
    ///
    /// Will return error if language cant be determined, the request to REGISTRATION fails,
    /// or the module categories can't be save
    pub fn get_registration_modules(&mut self, force_reload: bool, print_progress_bar: bool, lazy: LazyLevel)
                                    -> Result<Vec<ModuleCategory>, anyhow::Error> {
        let lang = self.get_language()?;

        if !force_reload {
            return utils::load_module_categories(&lang);
        }

        let resp = self.post_with_arg("REGISTRATION", vec![])?;

        let categories = parse::parse_modules(
            resp.text()?, self, print_progress_bar, lazy);

        self.categories_to_maps(categories.clone());
        self.save_maps()?;

        utils::save_module_categories(&categories, &lang)?;

        Ok(categories)
    }

    pub fn get_module_category(&self, category_name: &str, lazy: LazyLevel)
                               -> Result<Option<ModuleCategory>, StineError> {
        let resp = self.post_with_arg("REGISTRATION", vec![])?;
        Ok(parse::parse_get_module_category(resp.text()?, self, category_name, lazy))
    }

    fn check_in_map() {}

    /// Returns [`SubModule`] by specifying its id
    /// # Arguments:
    ///     - id: the id of the submodule you want, example: 383403915405527
    ///     - force_reload: this will parse and reload all modules you can apply for.
    ///     - lazy: Lazy loads certain info, reduces api calls and especially time to fetch the info

    /// **Warning**: will take roughly a few minutes
    /// # Return:
    /// Returns a result of either the found submodule or an error why it cant be found.
    /// It's possible that you have to retry calling this method wih *force_reload* enabled.
    pub fn get_submodule_by_id(&mut self, id: String, force_reload: bool, lazy: LazyLevel)
                               -> Result<&SubModule, StineError> {
        return if !force_reload {
            if self.submod_map.is_none() {
                self.load_maps()?;
            }
            self.submod_map.as_ref().unwrap().get(id.as_str())
                .ok_or_else(|| StineError::AnyError(anyhow!("SubModule not found maybe try force reloading")))
        } else {
            self.get_registration_modules(true, false, lazy)?;
            self.submod_map.as_ref().unwrap().get(id.as_str())
                .ok_or_else(|| StineError::AnyError(anyhow!("SubModule not found")))
        };
    }

    /// Returns [`Module`] by specifying its id
    /// # Arguments:
    ///     - module_number: the module_number of the Module you want, example: InfB-SE 1
    ///     - force_reload: this will parse and reload all modules you can apply for.
    ///     - lazy: Lazy loads certain info, reduces api calls and especially time to fetch the info

    /// **Warning**: will take roughly a few minutes
    /// # Return:
    /// Returns a result of either the found submodule or an error why it cant be found.
    /// It's possible that you have to retry calling this method wih `force_reload` enabled.
    pub fn get_module_by_number(&mut self, module_number: String, force_reload: bool, lazy: LazyLevel)
                                -> Result<&Module, anyhow::Error> {
        return if !force_reload {
            if self.mod_map.is_none() {
                self.load_maps()?;
            }
            self.mod_map.as_ref().unwrap().get(module_number.as_str())
                .ok_or_else(|| anyhow!("Module not found maybe try force reloading"))
        } else {
            self.get_registration_modules(true, false, lazy)?;
            self.mod_map.as_ref().unwrap().get(module_number.as_str())
                .ok_or_else(|| anyhow!("Module not found"))
        };
    }
    /// Returns exam and semester results of selected semesters
    ///
    /// **Note**: If you don't need the `GradeStats` please use `LazyLevel::FullLazy` to reduce the calls to stine
    /// # Arguments:
    /// * `semesters`  - Semesters you want the exam and end results of
    /// * `lazy_level` - Pass anything but `LazyLevel::FullLazy` to directly fetch `GradeStats` for the `CourseResult`s
    pub fn get_semester_results(&self, semesters: Vec<Semester>, lazy_level: LazyLevel)
                                -> Result<Vec<SemesterResult>, reqwest::Error> {
        let resp = self.post_with_arg("COURSERESULTS", vec![])?;
        Ok(parse_course_results(resp.text()?, self,
                                semesters, false, lazy_level))
    }

    /// Returns all exam and semester results
    ///
    /// **Note**: If you don't need the `GradeStats` please use `LazyLevel::FullLazy` to reduce the calls to stine
    /// # Arguments
    /// * `lazy_level` - Pass anything but `LazyLevel::FullLazy` to directly fetch `GradeStats` for the `CourseResult`s
    pub fn get_all_semester_results(&self, lazy_level: LazyLevel) -> Result<Vec<SemesterResult>, reqwest::Error> {
        let resp = self.post_with_arg("COURSERESULTS", vec![])?;

        // Self::save_to_file(resp);

        Ok(parse_course_results(resp.text()?, self,
                                Vec::new(), true, lazy_level))
    }


    /// Get `GradeStats` for specified exam and provided course_id
    /// # Arguments
    /// * `course_id` - the course id for the written exam, looks like this: 389187951081
    /// * `attempt` - the attempt of the exam. 0 is all exams. 99 is the maximum
    pub fn get_grade_stats_for_exam(&self, course_id: &str, attempt: u8) -> GradeStats {
        let resp = self.post_with_arg("GRADEOVERVIEW",
                                      vec![
                                          String::from("-N000460"), // somewhat related to the language N000318 -> german? N000460->? english
                                          String::from("-AMOFF"), // no idea
                                          format!("-N{course_id}"), // specifies selected exam?/course?
                                          format!("-N{attempt}"), // the attempt (max 99). very cool info actually, but data looks a bit weird
                                      ]).unwrap();

        // actually parse grade stats
        let html_to_parse = Html::parse_fragment(&resp.text().unwrap());
        parse_grade_stats(&html_to_parse, course_id)
    }

    /// Get `GradeStats` for a course
    /// # Arguments
    /// * `course_id` - the course id, looks like this: 38918795108
    pub fn get_grade_stats_for_course(&self, course_id: &str) -> GradeStats {
        self.get_grade_stats_for_exam(course_id, 0)
    }

    /// Returns the current language tied to your stine account
    /// # Panic
    /// panics if html root element does not contain lang attribute,
    /// or if the attribute can't be parsed. As this is considered an bug in this library
    /// and not something to be expected it will result in a panic
    pub fn get_language(&self) -> Result<Language, reqwest::Error> {
        let resp = self.post_with_arg("EXTERNALPAGES", vec![])?;
        let html = Html::parse_fragment(&resp.text()?);
        Ok(Self::get_language_from_resp(&html))
    }

    pub(crate) fn get_language_from_resp(html_content: &Html) -> Language {
        Language::from_str(html_content.root_element().value().attr("lang").unwrap()).unwrap()
    }

    /// Changes your stine language to [`Language`]
    /// # Returns
    /// returns whether the operation was successful
    /// in case an request error was found, the error gets returned
    pub fn set_language(&mut self, lang: &Language) -> Result<(), anyhow::Error> {
        let lang_code = match lang {
            Language::German => "-N001",
            Language::English => "-N002"
        }.to_owned();

        self.post_with_arg("CHANGELANGUAGE", vec![lang_code])?;

        if &self.get_language()? != lang {
            return Err(anyhow!("Failed changing STINE language"));
        }

        Ok(())
    }


    pub fn ensure_language(&self) -> Result<(), StineError> {
        assert_eq!(&self.get_language()?, self.language.as_ref().unwrap(),
                   "Set language does not equal current stine language");
        Ok(())
    }


    fn post_static(client: &Client, url: &str, mut headers: HeaderMap, data: HashMap<&str, &str>)
                   -> reqwest::Result<Response> {
        headers.insert(CONTENT_TYPE, "application/x-www-form-urlencoded".parse().unwrap());
        headers.insert(REFERER, format!("{BASE_URL}/").parse().unwrap());
        headers.insert(ORIGIN, BASE_URL.parse().unwrap());

        client.post(url).form(&data).headers(headers).send()
    }

    fn post(&self, url: &str, data: HashMap<&str, &str>) -> reqwest::Result<Response> {
        let mut headers = HeaderMap::new();

        if let Some(cnsc) = &self.cnsc_cookie {
            headers.insert(COOKIE, format!("cnsc={}", cnsc).parse().unwrap());
        }

        Self::post_static(&self.client, url, headers, data)
    }

    // pub fn get(&self, url: &str) -> reqwest::Result<Response> {
    //     self.client.get(url).send()
    // }

    /// Sends a POST requests to https://stine.uni-hamburg.de/scripts/mgrqispi.dll
    ///# Arguments
    ///     - prgname: is the selected site, e.g.: EXTERNALPAGES
    ///     - args: arguments added to parameters. Mostly in this format: -N<numbers>,-N<more numbers>.
    ///     More here: https://www2.informatik.uni-hamburg.de/fachschaft/wiki/index.php/STiNE-Interna
    pub fn post_with_arg(&self, prgname: &str, mut args: Vec<String>) -> reqwest::Result<Response> {
        args.insert(0, format!("-N{}", self.session.clone().unwrap()));

        let args_str = &args.join(",");

        let params = HashMap::from([
            ("APPNAME", "CampusNet"),
            ("PRGNAME", prgname),
            ("ARGUMENTS", args_str),
        ]);

        log::debug!("Post to: {prgname} {args_str}");
        self.post(API_URL, params)
    }

    /// Somehow this method does not working reliably authenticating????
    /// Stine says "Zugang verweigert" but the same method as a post request works???
    ///
    // fn get_with_arg(&self, prgname: &str, args: Vec<&str>) -> reqwest::Result<Response> {
    //     let mut url = format!("{API_URL}?APPNAME=CampusNet&PRGNAME={prgname}&ARGUMENTS=-N{},", self.session.clone().unwrap());
    //     for arg in args {
    //         url.push_str(&format!("{},", arg));
    //     }
    //     dbg!(&url);
    //     self.client.get(url).send()
    // }

    pub(crate) fn add_module(&mut self, module: Module) {
        if self.mod_map.is_none() {
            self.mod_map = Some(HashMap::new());
        }

        self.mod_map.as_mut().unwrap().insert(module.module_number.clone(), module);
    }

    pub(crate) fn add_submodule(&mut self, submodule: SubModule) {
        if self.submod_map.is_none() {
            self.submod_map = Some(HashMap::new());
        }

        self.submod_map.as_mut().unwrap().insert(
            submodule.id.clone(), submodule);
    }

    fn categories_to_maps(&mut self, categories: Vec<ModuleCategory>) {
        for c in categories {
            for module in c.modules {
                for submodule in &module.sub_modules {
                    self.submod_map.as_mut().unwrap().insert(
                        submodule.id.to_string(), submodule.clone());
                }

                let mod_num = module.clone().module_number;
                self.mod_map.as_mut().unwrap().insert(mod_num.to_string(), module);
            }

            for submodule in c.orphan_submodules {
                self.submod_map.as_mut().unwrap().insert(
                    submodule.id.to_string(), submodule);
            }
        }
    }

    pub(crate) fn save_maps(&self) -> Result<(), anyhow::Error> {
        log::debug!("saving maps");
        let lang = self.get_language()?;

        // maps can be empty or not initialized
        if let Some(mod_map) = self.mod_map.as_ref() {
            save_modules(mod_map, &lang)?;
        }

        if let Some(submod_map) = self.submod_map.as_ref() {
            save_submodules(submod_map, &lang)?;
        }

        Ok(())
    }

    /// Reloads .chache into self.submod_map and self.mod_map
    pub(crate) fn load_maps(&mut self) -> Result<(), anyhow::Error> {
        // problem might be that the language changes half way through the scraping.
        // This then will result in having data in the wrong in language in lang data caches.
        // A fix would be to pass lang as a parameter to this function
        // by parsing the lang from each request using Stine::get_language_from_resp

        let lang = self.language.as_ref().unwrap();

        self.submod_map = Some(utils::load_submodules(lang).unwrap_or_default());
        self.mod_map = Some(utils::load_modules(lang).unwrap_or_default());

        Ok(())
    }
}


#[derive(Debug)]
/// Holds submodules and modules of different registration statuses
pub struct MyRegistrations {
    pub pending_submodules: Vec<SubModule>,
    pub accepted_submodules: Vec<SubModule>,
    pub rejected_submodules: Vec<SubModule>,
    pub accepted_modules: Vec<Module>,
}

