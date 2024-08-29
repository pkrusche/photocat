use crate::datesummary::DateSummary;
use crate::fileindex::IndexFile;
use crate::valuecountsummary::ValueCounter;
use std::fmt;

pub trait FileIndexSummarizer: fmt::Display {
    fn add(&mut self, f: &IndexFile);
}

pub struct SummaryStats {
    summaries: Vec<Box<dyn FileIndexSummarizer>>,
}

impl SummaryStats {
    pub fn new(options: &Option<String>) -> SummaryStats {
        let mut summaries: Vec<Box<dyn FileIndexSummarizer>> = Vec::new();

        let options = if let Some(options) = options {
            options.split(';').map(|s| s.trim()).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let wrap_months: bool = options.contains(&"wrap");
        if wrap_months {
            summaries.push(Box::new(DateSummary::new_wrapping(8)));
        } else {
            summaries.push(Box::new(DateSummary::new()));
        }

        for o in options {
            if let Some(to_count) = o.strip_prefix("count:") {
                let variables: Vec<String> = to_count.split(",").map(|x| String::from(x)).collect();
                summaries.push(Box::new(ValueCounter::new(variables)));
            }
        }
        SummaryStats { summaries }
    }

    pub fn add(&mut self, f: &IndexFile) {
        for s in &mut self.summaries {
            s.add(f);
        }
    }
}

impl fmt::Display for SummaryStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for ref s in &self.summaries {
            writeln!(f, "{}", s.as_ref())?;
        }
        Ok(())
    }
}
