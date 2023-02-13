use std::collections::HashMap;
use std::fmt::format;
use std::{fs, io};
use std::fs::File;
use std::hash::Hash;
use std::io::copy;


use std::path::{Path};
use reqwest::blocking::Response;

use serde::{Serialize};
use serde::de::DeserializeOwned;

use serde_json::{Map, Value};
use crate::{Language, Module, ModuleCategory, SubModule};

// fn read_json_from_file<P: AsRef<Path>>(path: P) -> Result<String, Box<dyn Error>> {
//     let file_content = fs::read_to_string(path)?;
//     serde_json::from_str(&file_content)?
// }


pub fn read_json_file<P, T>(path: P) -> Result<T, anyhow::Error>
    where P: AsRef<Path>,
          T: Serialize + DeserializeOwned {

    let file_content = fs::read_to_string(path)?;
    let c: T = serde_json::from_str(file_content.as_str())?;

    Ok(c)
}

pub fn save_json_file<P, T>(path: P, data: &T) -> Result<(), anyhow::Error>
    where P: AsRef<Path>,
          T: Serialize {

    let json_string = serde_json::to_string_pretty(&data)?;
    fs::write(path, json_string)?;

    Ok(())
}

pub fn load_submodules(language: &Language) -> Result<HashMap<String, SubModule>, anyhow::Error> {
    let cache_dir = dirs::cache_dir().unwrap().join("stine-rs");

    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(format!("submodules_{}.json", language.to_string()));
    read_json_file(path)
}

pub fn save_submodules(submodules: &HashMap<String, SubModule>, language: &Language)
    -> Result<(), anyhow::Error> {
    let cache_dir = dirs::cache_dir().unwrap().join("stine-rs");

    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(format!("submodules_{}.json", language.to_string()));

    if !path.exists() {
        File::create(&path)?;
    }

    save_json_file(path, &submodules)
}

pub fn load_modules(language: &Language) -> Result<HashMap<String, Module>, anyhow::Error> {
    let cache_dir = dirs::cache_dir().unwrap().join("stine-rs");

    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(format!("modules_{}.json", language.to_string()));
    read_json_file(path)
}

pub fn save_modules(modules: &HashMap<String, Module>, language: &Language) -> Result<(), anyhow::Error> {
    let cache_dir = dirs::cache_dir().unwrap().join("stine-rs");

    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(format!("modules_{}.json", language.to_string()));

    if !path.exists() {
        File::create(&path)?;
    }

    save_json_file(path, &modules)
}

pub fn load_module_categories(language: &Language) -> Result<Vec<ModuleCategory>, anyhow::Error> {
    let cache_dir = dirs::cache_dir().unwrap().join("stine-rs");

    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(format!("module_categories_{}.json", language.to_string()));
    read_json_file(path)
}

pub fn save_module_categories(categories: &[ModuleCategory], language: &Language) -> Result<(), anyhow::Error> {
    let cache_dir = dirs::cache_dir().unwrap().join("stine-rs");

    fs::create_dir_all(&cache_dir)?;
    let path = cache_dir.join(format!("module_categories_{}.json", language.to_string()));

    if !path.exists() {
        File::create(&path)?;
    }

    save_json_file(path, &categories)
}

fn hashmap_to_map<V>(hashmap: HashMap<String, V>) -> Map<String, Value>
    where Value: From<V> {
    hashmap.into_iter()
        .map(|(k, v)| (k, Value::from(v))).collect()
}

fn save_to_file(resp: Response) -> Result<(), io::Error>{
    let mut dest = {
        let fname: &str = resp
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("tmp.html");

        log::info!("file to download: '{}'", fname);
        let fname = Path::new("/home/gareth/dev/Rust/webserver/stine-rs").join(fname);
        log::info!("will be located under: '{:?}'", fname);
        File::create(fname)
    }.unwrap();
    let content =  resp.text().unwrap();
    copy(&mut content.as_bytes(), &mut dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Language, Module, ModuleCategory};
    use crate::utils::{save_module_categories, load_module_categories};


    #[test]
    fn test_module_io() {
        // assert_eq!()
        let c: Vec<ModuleCategory> = vec![ModuleCategory { name: "test_module".to_string(), modules: vec![
            Module {
                module_number: "InfB-TEST".to_string(),
                name: "module".to_string(),
                sub_modules: vec![],
                exams: vec![],
                owner: "owner".to_string(),
                timetable_name: None,
                duration: None,
                electives: None,
                credits: None,
                start_semester: None,
                attributes: std::collections::HashMap::default()
            }
        ],
            orphan_submodules: vec![]
        }];

        assert!(save_module_categories(&c, &Language::English).is_ok());
        assert!(load_module_categories(&Language::English).is_ok());

    }
}