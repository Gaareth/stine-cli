use std::collections::HashMap;
use std::str::FromStr;

use anyhow::anyhow;

use crate::{EventType, Semester};
use crate::mobile::{ActorType, StudentEvent};

fn get_flat_attrs(root: roxmltree::Node) -> HashMap<String, String> {
    let mut flat_attrs: HashMap<String, String> = HashMap::new();
    for child in root.descendants() {
        // dbg!(child.tag_name().name());
        // dbg!(child.first_child());
        if let Some(text_child) = child.first_child() {
            if let Some(text) = text_child.text() {
                flat_attrs.insert(child.tag_name().name().to_string(), text.to_string());
            }
        }
    }
    flat_attrs
}

fn bool_from_string(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "t" | "1" => Some(true),
        "false" | "f" | "0" => Some(false),
        _ => None
    }
}

pub(crate) fn parse_student_events(input: String) -> Result<Vec<StudentEvent>, anyhow::Error> {
    let doc = roxmltree::Document::parse(&input)?;
    let mut events = vec![];
    for event in doc.descendants().filter(|n| n.has_tag_name("studentEvent")) {
        let mut flat_attrs = get_flat_attrs(event);
        events.push(StudentEvent {
            course_id: flat_attrs.remove("courseID"),
            course_data_id: flat_attrs.remove("courseDataID"),
            course_number: flat_attrs.remove("courseNumber"),
            course_name: flat_attrs.remove("courseName"),
            event_type: flat_attrs.remove("eventType"),
            event_category: flat_attrs.remove("eventCategory").and_then(|c| EventType::from_str(&c).ok()),
            semester_id: flat_attrs.remove("semesterID"),
            semester_name: flat_attrs.remove("semesterName").and_then(|n| Semester::from_str(&n).ok()),
            credits: flat_attrs.remove("creditPoints").and_then(|c| f32::from_str(&c).ok()),
            small_groups: flat_attrs.remove("smallGroups").and_then(|c| i32::from_str(&c).ok()),
            language: flat_attrs.remove("courseLanguage"),
            faculty_name: flat_attrs.remove("facultyName"),
            max_students: flat_attrs.remove("maxStudents").and_then(|c| i32::from_str(&c).ok()),
            instructors_string: flat_attrs.remove("instructorsString"),
            module_name: flat_attrs.remove("moduleName"),
            module_number: flat_attrs.remove("moduleNumber"),
            is_listener: flat_attrs.remove("listener").and_then(|c| bool_from_string(c.as_str())),
            accepted_status: flat_attrs.remove("acceptedStatus").and_then(|c| bool_from_string(c.as_str())),
            material_present: flat_attrs.remove("materialPresent").and_then(|c| bool_from_string(c.as_str())),
            info_present: flat_attrs.remove("infoPresent").and_then(|c| bool_from_string(c.as_str())),
        });
    }


    Ok(events)
}

pub fn parse_actor_type(input: String) -> Result<ActorType, anyhow::Error> {
    let doc = roxmltree::Document::parse(&input).unwrap();
    let actor = doc.descendants().find(|n| n.has_tag_name("actortype"))
        .ok_or_else(|| anyhow!("Failed parsing actortype XML"))?;
    ActorType::from_str(
        actor.text().ok_or_else(|| anyhow!("Failed parsing actortype XML: Missing inner text"))?)
}


#[cfg(test)]
mod tests_mobile {
    use std::assert_eq;
    use crate::mobile::ActorType;
    use crate::mobile::parse::{parse_actor_type, parse_student_events};

    #[test]
    fn test_actor_type() {
        let actor_type = parse_actor_type(r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?><mgns1:Message xmlns:mgns1="http://datenlotsen.de">
  <mgns1:person>
    <mgns1:actortype>STD</mgns1:actortype>
  </mgns1:person>
</mgns1:Message>"#.to_string());
        assert_eq!(actor_type.unwrap(), ActorType::Student);
    }

    #[test]
    fn test_student_events() {
        let events = parse_student_events(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
        <mgns1:Message xmlns:mgns1="http://datenlotsen.de">
          <mgns1:studentEvent>
            <mgns1:courseID>379923411595682</mgns1:courseID>
            <mgns1:courseDataID>379923411517683</mgns1:courseDataID>
            <mgns1:courseNumber>64-030</mgns1:courseNumber>
            <mgns1:courseName>Vorlesung Informatik im Kontext</mgns1:courseName>
            <mgns1:eventType>Lehrveranstaltung</mgns1:eventType>
            <mgns1:eventCategory>Vorlesung</mgns1:eventCategory>
            <mgns1:semesterID>99999988079072</mgns1:semesterID>
            <mgns1:semesterName>WiSe 21/22</mgns1:semesterName>
            <mgns1:creditPoints>0.0000</mgns1:creditPoints>
            <mgns1:hoursPerWeek>4</mgns1:hoursPerWeek>
            <mgns1:smallGroups>0</mgns1:smallGroups>
            <mgns1:courseLanguage>Deutsch</mgns1:courseLanguage>
            <mgns1:facultyName>Informatik (6401)</mgns1:facultyName>
            <mgns1:maxStudents>500</mgns1:maxStudents>
            <mgns1:instructorsString>Prof. Dr. Tilo Böhmann; Prof. Dr. Judith Simon; Prof. Dr. Frank Steinicke</mgns1:instructorsString>
            <mgns1:moduleName>Informatik im Kontext</mgns1:moduleName>
            <mgns1:moduleNumber>InfB-IKON</mgns1:moduleNumber>
            <mgns1:listener>0</mgns1:listener>
            <mgns1:acceptedStatus>1</mgns1:acceptedStatus>
            <mgns1:materialPresent>0</mgns1:materialPresent>
            <mgns1:infoPresent>1</mgns1:infoPresent>
        </mgns1:studentEvent>
        <mgns1:studentEvent>
            <mgns1:courseID>384875198636845</mgns1:courseID>
            <mgns1:courseDataID>384875198614846</mgns1:courseDataID>
            <mgns1:courseNumber>64-074</mgns1:courseNumber>
            <mgns1:courseName>Vorlesung Berechenbarkeit, Komplexität und Approximation</mgns1:courseName>
            <mgns1:eventType>Lehrveranstaltung</mgns1:eventType>
            <mgns1:eventCategory>Vorlesung</mgns1:eventCategory>
            <mgns1:semesterID>99999997019768</mgns1:semesterID>
            <mgns1:semesterName>SoSe 23</mgns1:semesterName>
            <mgns1:creditPoints>0.0000</mgns1:creditPoints>
            <mgns1:hoursPerWeek>3</mgns1:hoursPerWeek>
            <mgns1:smallGroups>0</mgns1:smallGroups>
            <mgns1:courseLanguage>Deutsch</mgns1:courseLanguage>
            <mgns1:facultyName>Informatik (6401)</mgns1:facultyName>
            <mgns1:maxStudents>240</mgns1:maxStudents>
            <mgns1:instructorsString>Prof. Dr. Petra Berenbrink</mgns1:instructorsString>
            <mgns1:moduleName>Berechenbarkeit, Komplexität und Approximation</mgns1:moduleName>
            <mgns1:moduleNumber>InfB-BKA</mgns1:moduleNumber>
            <mgns1:listener>0</mgns1:listener>
            <mgns1:acceptedStatus>0</mgns1:acceptedStatus>
            <mgns1:materialPresent>0</mgns1:materialPresent>
            <mgns1:infoPresent>0</mgns1:infoPresent>
        </mgns1:studentEvent>
    </mgns1:Message>"#.to_string());

        assert_eq!(events.unwrap().len(), 2);
    }
}