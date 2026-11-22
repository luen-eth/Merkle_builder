//! EVM address parsing and EIP-55 checksum handling.

use crate::keccak256;

/// A 20-byte EVM address.
pub type Address = [u8; 20];

/// Parses an EVM address.
///
/// The `0x` prefix is optional. All-lowercase and all-uppercase addresses are
/// accepted as-is; mixed-case addresses must carry a valid EIP-55 checksum,
/// which catches typos before they end up in a Merkle root.
pub fn parse_address(input: &str) -> Result<Address, String> {
    let body = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);

    if body.len() != 40 {
        return Err(format!(
            "invalid address `{input}`: expected 40 hex characters, found {}",
            body.len()
        ));
    }

    let mut address = [0u8; 20];
    hex::decode_to_slice(body, &mut address)
        .map_err(|_| format!("invalid address `{input}`: contains non-hex characters"))?;

    let has_lower = body.bytes().any(|b| b.is_ascii_lowercase());
    let has_upper = body.bytes().any(|b| b.is_ascii_uppercase());
    if has_lower && has_upper && to_checksum(&address)[2..] != *body {
        return Err(format!(
            "invalid address `{input}`: EIP-55 checksum mismatch (check for typos, or use an all-lowercase address)"
        ));
    }

    Ok(address)
}

/// Formats an address as an EIP-55 checksummed hex string with `0x` prefix.
pub fn to_checksum(address: &Address) -> String {
    let lower = hex::encode(address);
    let hash = keccak256(lower.as_bytes());

    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = (hash[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0x0f;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from the EIP-55 specification.
    const VECTORS: [&str; 4] = [
        "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
        "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
        "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
        "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
    ];

    #[test]
    fn checksum_round_trip() {
        for vector in VECTORS {
            let address = parse_address(vector).unwrap();
            assert_eq!(to_checksum(&address), vector);
        }
    }

    #[test]
    fn accepts_single_case_and_missing_prefix() {
        let expected = parse_address(VECTORS[0]).unwrap();
        assert_eq!(parse_address(&VECTORS[0].to_lowercase()).unwrap(), expected);
        assert_eq!(parse_address(&VECTORS[0][2..]).unwrap(), expected);
        let upper = format!("0x{}", VECTORS[0][2..].to_uppercase());
        assert_eq!(parse_address(&upper).unwrap(), expected);
    }

    #[test]
    fn rejects_bad_input() {
        // Wrong checksum: first letter case flipped.
        assert!(parse_address("0x5AAeb6053F3E94C9b9A09f33669435E7Ef1BeAed").is_err());
        assert!(parse_address("0x1234").is_err());
        assert!(parse_address("0xzzAeb6053F3E94C9b9A09f33669435E7Ef1BeAed").is_err());
        assert!(parse_address("").is_err());
    }
}
