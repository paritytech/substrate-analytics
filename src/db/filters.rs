use chrono::NaiveDateTime;

//const TIME_FORMAT: &'static str = "2000-01-01T00:00:00";

// TODO implement validator derive
// #[macro_use]
// extern crate validator_derive;

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Filters {
    pub start_time: Option<NaiveDateTime>,
    pub end_time: Option<NaiveDateTime>,
    pub limit: Option<i32>,
}

//impl Filters {
//    pub fn from_hashmap(hashmap: Ref<HashMap<String, String>>) -> Result<Self, Vec<String>> {
//        let mut errors = Vec::new();
//        let start_time: Option<NaiveDateTime> = match hashmap.get("start_time") {
//            None => None,
//            Some(some) => match some.parse() {
//                Ok(value) => Some(value),
//                Err(_) => {
//                    errors.push(format_error("start_time", TIME_FORMAT));
//                    None
//                }
//            },
//        };
//        let end_time: Option<NaiveDateTime> = match hashmap.get("end_time") {
//            None => None,
//            Some(some) => match some.parse() {
//                Ok(value) => Some(value),
//                Err(_) => {
//                    errors.push(format_error("end_time", TIME_FORMAT));
//                    None
//                }
//            },
//        };
//        let limit: Option<i32> = match hashmap.get("limit") {
//            None => None,
//            Some(some) => match some.parse() {
//                Ok(value) => Some(value),
//                Err(_) => {
//                    errors.push(format_error("limit", "100"));
//                    None
//                }
//            },
//        };
//        if errors.is_empty() {
//            Ok(Filters {
//                start_time,
//                end_time,
//                limit,
//            })
//        } else {
//            Err(errors)
//        }
//    }
//}
//
//fn format_error(key: &str, format: &str) -> String {
//    format!(
//        "Unable to parse `{}`, please use the format: `{}`",
//        key, format
//    )
//}
