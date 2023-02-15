use scraper::{ElementRef, Selector};
use scraper::Html;

use crate::{LazyLevel, Module, SubModule};
use crate::parse::{parse_module, parse_sub_module};
use crate::parse::utils::{get_next_selection, parse_arg_string};
use crate::stine::{Stine, MyRegistrations};

fn parse_pending_registrations(html: &Html, stine: &mut Stine, lazy: LazyLevel) -> Vec<SubModule> {
    let pending_table = html.select(&Selector::parse("table").unwrap()).next().unwrap();

    parse_submodules_table(&pending_table, stine, lazy)
}

fn parse_accepted_registrations(html: &Html, stine: &mut Stine, lazy: LazyLevel) -> Vec<SubModule> {
    let accepted_table = html.select(&Selector::parse("table").unwrap()).nth(1).unwrap();

    parse_submodules_table(&accepted_table, stine, lazy)
}

fn parse_rejected_registrations(html: &Html, stine: &mut Stine, lazy: LazyLevel) -> Vec<SubModule> {
    let rejected_table = html.select(&Selector::parse("table").unwrap()).nth(2).unwrap();

    parse_submodules_table(&rejected_table, stine, lazy)
}

fn parse_accepted_module_registrations(html: &Html, stine: &mut Stine, lazy: LazyLevel) -> Vec<Module> {
    let table = html.select(&Selector::parse("table").unwrap()).nth(3).unwrap();

    parse_modules_table(&table, stine, lazy)
}

fn parse_modules_table(table: &ElementRef, stine: &mut Stine, lazy: LazyLevel) -> Vec<Module> {
    let mut modules: Vec<Module> = Vec::new();

    let row_sel = &Selector::parse("tbody > tr").unwrap();
    let rows = table.select(row_sel);

    for row in rows {
        if let Some(link_element) = get_next_selection(row, "a") {
            let event_link = link_element.inner_html();
            let module_number = event_link.split_whitespace().next().unwrap().to_owned();

            log::debug!("Parsing module number: {module_number}");

            if let Ok(submod) = stine.get_module_by_number(module_number, false, lazy).cloned() {
                modules.push(submod);
            } else {

                let sub_module_el = get_next_selection(row, ".dl-inner").unwrap();
                let module = parse_module(sub_module_el, stine, lazy);
                stine.add_module(module.clone());
                // dbg!(&stine.mod_map);
                // break;

                modules.push(module);
            }
        }
    }

    modules
}

// Warning: when using LazyLevel::NotLazy, this will reparse all groups everytime,
// regardless of the cache. This is because we dont store groups in the cache rn.
// TODO: implement: 1. get group id.
// 2. check if exercise with that group id was cached.
// 3. fetch the specific group
// 4. return it
fn parse_submodules_table(table: &ElementRef, stine: &mut Stine, lazy: LazyLevel) -> Vec<SubModule> {
    let mut submodules: Vec<SubModule> = Vec::new();

    let row_sel = &Selector::parse("tbody > tr").unwrap();
    let rows = table.select(row_sel);

    for row in rows {
        if let Some(link_element) = get_next_selection(row, "a") {
            let event_link = link_element.value().attr("href").unwrap();
            let args: Vec<String> = parse_arg_string(event_link);
            let id = args[2].split("-N").nth(1).unwrap().to_owned();

            if let Ok(submod) = stine.get_submodule_by_id(id, false, lazy).cloned() {
                // dbg!(&submod.name);

                // for groups, the submodule with all groups has the same ID as an entry
                // which is a specific group of the submodule
                // => so we check if the name is different, and if it is the group name gets parsed separately
                if submod.name == link_element.inner_html().trim() {
                    submodules.push(submod);
                    continue;
                }
            }

            let sub_module_el = get_next_selection(row, ".dl-inner").unwrap();
            let submodule = parse_sub_module(sub_module_el, stine, lazy);
            // dbg!(&submodule.name);
            stine.add_submodule(submodule.clone());
            log::debug!("Parsing submodule: {}", submodule.name);


            submodules.push(submodule);
        }
    }

    submodules
}

pub fn parse_my_registrations(html_content: String, stine: &mut Stine, lazy: LazyLevel)
                              -> MyRegistrations {
    let html = Html::parse_fragment(&html_content);

    let pending_submodules: Vec<SubModule> = vec![];
    let accepted_submodules: Vec<SubModule> = vec![];
    let rejected_submodules: Vec<SubModule> = vec![];
    let accepted_modules: Vec<Module> = vec![];

    let pending_submodules = parse_pending_registrations(&html, stine, lazy);
    let accepted_submodules = parse_accepted_registrations(&html, stine, lazy);
    let rejected_submodules = parse_rejected_registrations(&html, stine, lazy);
    let accepted_modules = parse_accepted_module_registrations(&html, stine, lazy);


    stine.save_maps().unwrap();

    MyRegistrations {
        pending_submodules,
        accepted_submodules,
        rejected_submodules,
        accepted_modules
    }
}