//! Merkle tree and proof generation for EVM whitelists and airdrops.
//!
//! Two list modes are supported:
//!
//! * [`Mode::Address`]: one address per line.
//! * [`Mode::AddressAmount`]: one `address,amount` pair per line.

pub mod address;
pub mod amount;
pub mod input;
pub mod server;
pub mod tree;

use std::fmt;
use std::io::Write;

use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};
use tiny_keccak::{Hasher, Keccak};

pub use address::{parse_address, to_checksum, Address};
pub use amount::{parse_amount, Amount};
pub use input::{parse_entries, Entry, InputError, LineError};
pub use tree::{verify, MerkleTree};

/// A 32-byte Keccak-256 hash.
pub type Hash = [u8; 32];

pub fn keccak256(data: &[u8]) -> Hash {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// What each line of the input list contains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// One address per line.
    #[default]
    Address,
    /// One `address,amount` pair per line.
    AddressAmount,
}

/// How an entry is hashed into a leaf.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum LeafEncoding {
    /// `keccak256(abi.encodePacked(address[, amount]))`
    #[default]
    Packed,
    /// `keccak256(bytes.concat(keccak256(abi.encode(address[, amount]))))`,
    /// the double-hashed leaf format used by OpenZeppelin's StandardMerkleTree.
    Double,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Mode::Address => "address",
            Mode::AddressAmount => "address-amount",
        })
    }
}

impl fmt::Display for LeafEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LeafEncoding::Packed => "packed",
            LeafEncoding::Double => "double",
        })
    }
}

/// Options controlling how a list is parsed and hashed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub encoding: LeafEncoding,
    /// Token decimals used to scale amounts. `0` means amounts are raw units.
    pub decimals: u8,
}

/// Computes the leaf hash of an address with an optional amount.
pub fn leaf_hash(address: &Address, amount: Option<&Amount>, encoding: LeafEncoding) -> Hash {
    let mut buffer = [0u8; 64];
    let preimage: &[u8] = match encoding {
        LeafEncoding::Packed => {
            buffer[..20].copy_from_slice(address);
            match amount {
                Some(amount) => {
                    buffer[20..52].copy_from_slice(&amount.bytes);
                    &buffer[..52]
                }
                None => &buffer[..20],
            }
        }
        LeafEncoding::Double => {
            // abi.encode left-pads the address to a 32-byte word.
            buffer[12..32].copy_from_slice(address);
            match amount {
                Some(amount) => {
                    buffer[32..].copy_from_slice(&amount.bytes);
                    &buffer[..]
                }
                None => &buffer[..32],
            }
        }
    };

    let hash = keccak256(preimage);
    match encoding {
        LeafEncoding::Packed => hash,
        LeafEncoding::Double => keccak256(&hash),
    }
}

/// A parsed list together with its Merkle tree.
pub struct Whitelist {
    pub options: Options,
    pub entries: Vec<Entry>,
    /// Number of exact duplicate lines that were dropped from the input.
    pub duplicates_removed: usize,
    pub tree: MerkleTree,
}

impl Whitelist {
    /// Parses `text` and builds the Merkle tree over its entries.
    pub fn generate(text: &str, options: Options) -> Result<Self, InputError> {
        let parsed = parse_entries(text, options)?;
        let leaves = parsed
            .entries
            .iter()
            .map(|entry| leaf_hash(&entry.address, entry.amount.as_ref(), options.encoding))
            .collect();

        Ok(Self {
            options,
            entries: parsed.entries,
            duplicates_removed: parsed.duplicates_removed,
            tree: MerkleTree::new(leaves),
        })
    }

    pub fn root_hex(&self) -> String {
        to_hex(&self.tree.root())
    }

    /// Streams the result as JSON without materialising every proof at once.
    pub fn write_json<W: Write>(&self, writer: W, pretty: bool) -> serde_json::Result<()> {
        let document = Document {
            root: self.root_hex(),
            mode: self.options.mode,
            encoding: self.options.encoding,
            decimals: (self.options.mode == Mode::AddressAmount).then_some(self.options.decimals),
            count: self.entries.len(),
            duplicates_removed: self.duplicates_removed,
            entries: EntriesSerializer(self),
        };

        if pretty {
            serde_json::to_writer_pretty(writer, &document)
        } else {
            serde_json::to_writer(writer, &document)
        }
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Parses a `0x`-prefixed (or bare) 32-byte hex string.
pub fn parse_hash(input: &str) -> Result<Hash, String> {
    let body = input.strip_prefix("0x").unwrap_or(input);
    let mut hash = [0u8; 32];
    hex::decode_to_slice(body, &mut hash)
        .map_err(|_| format!("invalid hash `{input}`: expected 32 bytes of hex"))?;
    Ok(hash)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    root: String,
    mode: Mode,
    encoding: LeafEncoding,
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
    count: usize,
    duplicates_removed: usize,
    entries: EntriesSerializer<'a>,
}

#[derive(Serialize)]
struct EntryOutput<'a> {
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<&'a str>,
    leaf: String,
    proof: Vec<String>,
}

struct EntriesSerializer<'a>(&'a Whitelist);

impl Serialize for EntriesSerializer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let whitelist = self.0;
        let mut sequence = serializer.serialize_seq(Some(whitelist.entries.len()))?;
        for (index, entry) in whitelist.entries.iter().enumerate() {
            sequence.serialize_element(&EntryOutput {
                address: to_checksum(&entry.address),
                amount: entry.amount.as_ref().map(|amount| amount.decimal.as_str()),
                leaf: to_hex(&whitelist.tree.leaves()[index]),
                proof: whitelist.tree.proof(index).iter().map(|hash| to_hex(hash)).collect(),
            })?;
        }
        sequence.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keccak256_of_empty_input() {
        assert_eq!(
            to_hex(&keccak256(b"")),
            "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[test]
    fn double_encoding_differs_from_packed() {
        let address = parse_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed").unwrap();
        let amount = parse_amount("100", 0).unwrap();
        let packed = leaf_hash(&address, Some(&amount), LeafEncoding::Packed);
        let double = leaf_hash(&address, Some(&amount), LeafEncoding::Double);
        assert_ne!(packed, double);

        let mut encoded = [0u8; 64];
        encoded[12..32].copy_from_slice(&address);
        encoded[32..].copy_from_slice(&amount.bytes);
        assert_eq!(double, keccak256(&keccak256(&encoded)));
    }
}
