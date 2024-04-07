
use std::env;

use stine_rs::Stine;

pub fn env_auth() -> Stine {
    #![feature(let_chains)]

    dotenv::from_path("../.env")
        .expect("Failed loading .env file. \
        Make sure there is a .env file in stine-rs/ and you are running from stine-rs/lib");

    // try session first to reduce login calls
    // cons: the session key wont be update, TODO: impl this :(
    if let Ok(session) = env::var("session") {
        if let Ok(cnsc_cookie) = env::var("cookie") {
            if let Ok(stine) = Stine::new_session(cnsc_cookie.as_str(), session.as_str()) {
                return stine;
            }
        }
    }

    Stine::new(env::var("username").unwrap().as_str(),
               env::var("password").unwrap().as_str())
        .expect("Failed authenticating with Stine")
}