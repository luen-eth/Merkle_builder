//! `uint256` amount parsing.

/// A `uint256` amount, kept both as big-endian bytes (for hashing) and as a
/// canonical base-10 string of raw units (for output).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Amount {
    pub bytes: [u8; 32],
    pub decimal: String,
}

/// Parses a non-negative decimal amount into raw `uint256` units.
///
/// With `decimals = 0` the input must be an integer and is used as-is. With
/// `decimals = N` the input is a human-readable token amount that is scaled by
/// `10^N`, so `1.5` with 18 decimals becomes `1500000000000000000`.
pub fn parse_amount(input: &str, decimals: u8) -> Result<Amount, String> {
    let invalid = || format!("invalid amount `{input}`: expected a non-negative decimal number");

    let (int_part, frac_part) = match input.split_once('.') {
        Some((int_part, frac_part)) => (int_part, frac_part),
        None => (input, ""),
    };

    if int_part.is_empty() && frac_part.is_empty() {
        return Err(invalid());
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit()) || !frac_part.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }

    let frac_part = frac_part.trim_end_matches('0');
    if frac_part.len() > decimals as usize {
        return Err(if decimals == 0 {
            format!("invalid amount `{input}`: fractional amounts require the decimals option")
        } else {
            format!("invalid amount `{input}`: more than {decimals} fractional digits")
        });
    }

    let mut digits = String::with_capacity(int_part.len() + decimals as usize);
    digits.push_str(int_part);
    digits.push_str(frac_part);
    digits.extend(std::iter::repeat_n('0', decimals as usize - frac_part.len()));

    let trimmed = digits.trim_start_matches('0');
    let decimal = if trimmed.is_empty() { "0" } else { trimmed }.to_string();

    let mut bytes = [0u8; 32];
    for digit in decimal.bytes() {
        let mut carry = u16::from(digit - b'0');
        for byte in bytes.iter_mut().rev() {
            let value = u16::from(*byte) * 10 + carry;
            *byte = (value & 0xff) as u8;
            carry = value >> 8;
        }
        if carry != 0 {
            return Err(format!("invalid amount `{input}`: exceeds the uint256 range"));
        }
    }

    Ok(Amount { bytes, decimal })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integers() {
        let amount = parse_amount("256", 0).unwrap();
        assert_eq!(amount.decimal, "256");
        assert_eq!(amount.bytes[30..], [1, 0]);
        assert!(amount.bytes[..30].iter().all(|b| *b == 0));

        assert_eq!(parse_amount("000", 0).unwrap().decimal, "0");
        assert_eq!(parse_amount("007", 0).unwrap().decimal, "7");
    }

    #[test]
    fn scales_by_decimals() {
        assert_eq!(parse_amount("1.5", 18).unwrap().decimal, "1500000000000000000");
        assert_eq!(parse_amount("1", 6).unwrap().decimal, "1000000");
        assert_eq!(parse_amount(".25", 2).unwrap().decimal, "25");
        assert_eq!(parse_amount("3.", 2).unwrap().decimal, "300");
        // Trailing zeros beyond the precision are harmless.
        assert_eq!(parse_amount("1.10", 1).unwrap().decimal, "11");
    }

    #[test]
    fn enforces_uint256_range() {
        let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        assert_eq!(parse_amount(max, 0).unwrap().bytes, [0xff; 32]);
        let overflow =
            "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        assert!(parse_amount(overflow, 0).is_err());
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", ".", "-1", "1e18", "1,5", "abc", "1.2.3", "+5"] {
            assert!(parse_amount(bad, 18).is_err(), "{bad} should be rejected");
        }
        assert!(parse_amount("1.5", 0).is_err());
        assert!(parse_amount("1.234", 2).is_err());
    }
}
