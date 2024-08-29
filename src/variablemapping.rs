use std::collections::HashMap;
use std::io;

use serde::Deserialize;

use crate::fileindex::MetaValue;
use crate::fileindex::MetaVariable;

pub type Mappings = Vec<Mapping>;

#[derive(Deserialize, Debug, Clone)]
pub struct Mapping {
    variable: String,
    match_values: Vec<String>,
    assign_value: String,
}

impl Mapping {
    pub fn apply(&self, variable: &str, value: &str) -> Option<String> {
        if variable == self.variable && self.match_values.contains(&value.to_string()) {
            Some(self.assign_value.clone())
        } else {
            None
        }
    }
}

/// Load mappings from a file
pub fn load_mappings(filename: &str) -> io::Result<Mappings> {
    let parsed_contents: HashMap<String, Mappings>;
    let mut mappings: Mappings = Vec::new();

    let file_contents = std::fs::read_to_string(filename)?;
    parsed_contents =
        toml::from_str(&file_contents).expect(&format!("File {} cannot be parsed!", filename));
    mappings.extend(parsed_contents["mapping"].iter().map(|x| x.clone()));

    Ok(mappings)
}

/// Apply mappings to list of variables
pub fn apply_mappings(mappings: &Mappings, variables: &mut Vec<MetaVariable>) {
    for v in variables {
        for m in mappings {
            if let Some(result) = m.apply(&v.name, &format!("{}", v.value)) {
                v.value = MetaValue::String(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use toml::from_str;

    static TEST_MAPPINGS_STRING: &str = "[[mapping]]\n\
                                         variable = 'V1'\n\
                                         match_values = ['A', 'B']\n\
                                         assign_value = 'C'\n\
                                         \n\n\
                                         [[mapping]]\n\
                                         variable = 'V2'\n\
                                         match_values = ['D']\n\
                                         assign_value = 'E'\n";

    #[test]
    fn test_load_variable_mapping() {
        let items_table: HashMap<String, Vec<Mapping>> = from_str(TEST_MAPPINGS_STRING).unwrap();
        let items: &[Mapping] = &items_table["mapping"];
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].variable, "V1");
        assert_eq!(
            items[0].match_values,
            vec![String::from("A"), String::from("B")]
        );
        assert_eq!(items[0].assign_value, "C");
        assert_eq!(items[1].variable, "V2");
        assert_eq!(items[1].match_values, vec![String::from("D")]);
        assert_eq!(items[1].assign_value, "E");
    }

    #[test]
    fn test_apply_mappings() {
        let mut variables: Vec<MetaVariable> = vec![
            MetaVariable {
                name: String::from("V1"),
                value: MetaValue::String(String::from("A")),
            },
            MetaVariable {
                name: String::from("V2"),
                value: MetaValue::String(String::from("D")),
            },
            MetaVariable {
                name: String::from("V3"),
                value: MetaValue::Int(3),
            },
        ];

        let mappings: Mappings = vec![
            Mapping {
                variable: String::from("V1"),
                match_values: vec![String::from("A"), String::from("B")],
                assign_value: String::from("10"),
            },
            Mapping {
                variable: String::from("V2"),
                match_values: vec![String::from("D")],
                assign_value: String::from("20"),
            },
        ];

        assert_eq!(format!("{}", variables[0].value), "A");
        assert_eq!(format!("{}", variables[1].value), "D");
        assert_eq!(format!("{}", variables[2].value), "3");
        apply_mappings(&mappings, &mut variables);

        assert_eq!(format!("{}", variables[0].value), "10");
        assert_eq!(format!("{}", variables[1].value), "20");
        assert_eq!(format!("{}", variables[2].value), "3");
    }
}
