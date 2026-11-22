//! Parsing of address lists and address/amount lists.

use std::collections::HashMap;
use std::fmt;

use serde::Serialize;

use crate::address::{parse_address, Address};
use crate::amount::{parse_amount, Amount};
use crate::{Mode, Options};

/// Maximum number of line errors reported at once.
const MAX_REPORTED_ERRORS: usize = 25;

/// One entry of the list. `amount` is set only in address/amount mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub address: Address,
    pub amount: Option<Amount>,
}

pub struct ParsedInput {
    pub entries: Vec<Entry>,
    pub duplicates_removed: usize,
}

/// A problem on a specific (1-based) input line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LineError {
    pub line: usize,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputError {
    /// The input contained no entries.
    Empty,
    /// One or more lines could not be parsed. `total` may exceed
    /// `errors.len()` when the report was truncated.
    Lines { errors: Vec<LineError>, total: usize },
}

impl InputError {
    pub fn line_errors(&self) -> &[LineError] {
        match self {
            InputError::Empty => &[],
            InputError::Lines { errors, .. } => errors,
        }
    }

    /// A one-line summary without the per-line details.
    pub fn summary(&self) -> String {
        match self {
            InputError::Empty => "the list is empty".to_string(),
            InputError::Lines { total: 1, .. } => "1 line could not be parsed".to_string(),
            InputError::Lines { total, .. } => format!("{total} lines could not be parsed"),
        }
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())?;
        if let InputError::Lines { errors, total } = self {
            for error in errors {
                write!(f, "\n  line {}: {}", error.line, error.message)?;
            }
            if total > &errors.len() {
                write!(f, "\n  ... and {} more", total - errors.len())?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for InputError {}

/// Parses a list according to `options.mode`.
///
/// Blank lines and lines starting with `#` are ignored, as is a first line
/// that starts with `address` (a CSV header). Fields may be separated by
/// commas, semicolons or whitespace. Exact duplicates are dropped; the same
/// address with two different amounts is an error.
pub fn parse_entries(text: &str, options: Options) -> Result<ParsedInput, InputError> {
    let mut entries = Vec::new();
    let mut first_seen: HashMap<Address, (usize, usize)> = HashMap::new();
    let mut duplicates_removed = 0;
    let mut errors = Vec::new();
    let mut total_errors = 0;
    let mut seen_content = false;

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let is_first_content_line = !seen_content;
        seen_content = true;
        if is_first_content_line && line.to_ascii_lowercase().starts_with("address") {
            continue;
        }

        let result = parse_line(line, options).and_then(|entry| {
            match first_seen.get(&entry.address) {
                None => {
                    first_seen.insert(entry.address, (entries.len(), line_number));
                    entries.push(entry);
                }
                Some(&(position, _)) if entries[position] == entry => duplicates_removed += 1,
                Some(&(_, first_line)) => {
                    return Err(format!(
                        "address already listed on line {first_line} with a different amount"
                    ));
                }
            }
            Ok(())
        });

        if let Err(message) = result {
            total_errors += 1;
            if errors.len() < MAX_REPORTED_ERRORS {
                errors.push(LineError { line: line_number, message });
            }
        }
    }

    if total_errors > 0 {
        return Err(InputError::Lines { errors, total: total_errors });
    }
    if entries.is_empty() {
        return Err(InputError::Empty);
    }

    Ok(ParsedInput { entries, duplicates_removed })
}

fn parse_line(line: &str, options: Options) -> Result<Entry, String> {
    let fields: Vec<&str> = line
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|field| !field.is_empty())
        .collect();

    match (options.mode, fields.as_slice()) {
        (Mode::Address, [address]) => Ok(Entry { address: parse_address(address)?, amount: None }),
        (Mode::Address, _) => Err(format!(
            "expected a single address, found {} fields (use the address-amount mode for `address,amount` lists)",
            fields.len()
        )),
        (Mode::AddressAmount, [address, amount]) => Ok(Entry {
            address: parse_address(address)?,
            amount: Some(parse_amount(amount, options.decimals)?),
        }),
        (Mode::AddressAmount, _) => {
            Err(format!("expected `address,amount`, found {} field(s)", fields.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LeafEncoding;

    const A: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
    const B: &str = "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359";

    fn options(mode: Mode, decimals: u8) -> Options {
        Options { mode, encoding: LeafEncoding::Packed, decimals }
    }

    #[test]
    fn parses_address_list_with_noise() {
        let text = format!("# team wallets\n\n{A}\r\n  {B}  \n{}\n", A.to_lowercase());
        let parsed = parse_entries(&text, options(Mode::Address, 0)).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.duplicates_removed, 1);
        assert!(parsed.entries.iter().all(|entry| entry.amount.is_none()));
    }

    #[test]
    fn parses_amount_list_with_header_and_separators() {
        let text = format!("address,amount\n{A},100\n{B}; 2.5\n{A}\t100\n");
        let parsed = parse_entries(&text, options(Mode::AddressAmount, 2)).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.duplicates_removed, 1);
        assert_eq!(parsed.entries[0].amount.as_ref().unwrap().decimal, "10000");
        assert_eq!(parsed.entries[1].amount.as_ref().unwrap().decimal, "250");
    }

    #[test]
    fn reports_line_numbers() {
        let text = format!("{A},1\nnot-an-address,1\n{B}\n{A},2\n");
        let error = parse_entries(&text, options(Mode::AddressAmount, 0)).err().unwrap();
        let lines: Vec<usize> = error.line_errors().iter().map(|e| e.line).collect();
        assert_eq!(lines, [2, 3, 4]);
        assert!(error.line_errors()[2].message.contains("line 1"));
    }

    #[test]
    fn rejects_wrong_mode_and_empty_input() {
        let text = format!("{A},1\n");
        assert!(parse_entries(&text, options(Mode::Address, 0)).is_err());
        assert_eq!(
            parse_entries("\n# nothing\n", options(Mode::Address, 0)).err(),
            Some(InputError::Empty)
        );
    }
}
