use itertools::Itertools;

use crate::{fileindex::MetaValue, summarystats::FileIndexSummarizer};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use std::{collections::HashMap, collections::HashSet, fmt::Display};

pub struct ValueCounter {
    variables: Vec<String>,
    counts: HashMap<String, usize>,
    variable_values: HashMap<String, HashSet<String>>,
}

impl ValueCounter {
    pub fn new(variables: Vec<String>) -> ValueCounter {
        ValueCounter {
            variables: variables.iter().sorted().map(|x| x.clone()).collect(),
            counts: HashMap::new(),
            variable_values: HashMap::new(),
        }
    }
}

impl FileIndexSummarizer for ValueCounter {
    fn add(&mut self, f: &crate::fileindex::IndexFile) {
        let mut keys: Vec<String> = Vec::new();
        for v in &self.variables {
            let mut value: Option<&MetaValue> = None;
            for iv in &f.meta {
                if iv.name == *v {
                    value = Some(&iv.value);
                    break;
                }
            }
            let key = match value {
                Some(x) => {
                    format!("{}:{}:{}", v, x.string_type(), x)
                }
                None => String::from("{}:MISSING"),
            };
            self.variable_values
                .entry(v.clone())
                .or_insert(HashSet::new())
                .insert(key.clone());
            keys.push(key);
        }
        let keys_concatenated = keys.join(",");
        *self.counts.entry(keys_concatenated).or_insert(0) += 1;
    }
}

impl Display for ValueCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let empty_set = HashSet::new();
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS);
        if self.variables.len() > 2 {
            table.set_header(vec!["Names", "Count"]);

            for (name, count) in self.counts.iter().sorted_by_key(|c| c.1) {
                let count_str = format!("{}", count);
                table.add_row(vec![&name, &count_str]);
            }
        } else if self.variables.len() == 2 {
            let v1 = &self.variables[0];
            let v2 = &self.variables[1];
            let vals_1: Vec<_> = self
                .variable_values
                .get(v1)
                .unwrap_or(&empty_set)
                .iter()
                .sorted()
                .collect();
            let vals_2: Vec<_> = self
                .variable_values
                .get(v2)
                .unwrap_or(&empty_set)
                .iter()
                .sorted()
                .collect();

            let mut header = vec![format!("↓{}  {} → ", v1, v2)];
            for val_2 in &vals_2 {
                let val_2_substring = val_2.splitn(3, ':').nth(2).unwrap_or("");
                header.push(String::from(val_2_substring));
            }
            table.set_header(header);

            for val_1 in &vals_1 {
                let val_1_substring = val_1.splitn(3, ':').nth(2).unwrap_or("");
                let mut row = vec![String::from(val_1_substring)];
                for val_2 in &vals_2 {
                    let key = vec![val_1, val_2].iter().join(",");
                    let count = self.counts.get(&key).unwrap_or(&0);
                    row.push(format!("{}", count));
                }
                table.add_row(row);
            }
        } else if self.variables.len() == 1 {
            let v1 = &self.variables[0];
            let vals: Vec<_> = self
                .variable_values
                .get(v1)
                .unwrap_or(&empty_set)
                .iter()
                .map(|x| (x, self.counts.get(x).unwrap_or(&0)))
                .sorted_by_key(|x| x.1)
                .rev()
                .collect();

            table.set_header(vec![v1, "Count"]);
            for (key, count) in &vals {
                let val_substring = key.splitn(3, ':').nth(2).unwrap_or(key);
                table.add_row(vec![String::from(val_substring), format!("{}", count)]);
            }
        }
        writeln!(f, "{}", table)?;
        Ok(())
    }
}
