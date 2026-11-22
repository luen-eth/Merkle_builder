//! End-to-end checks against independently produced reference values.

use merkle_whitelist::{
    leaf_hash, parse_address, parse_amount, parse_hash, to_hex, verify, LeafEncoding, Mode,
    Options, Whitelist,
};

/// Reference output produced by `merkletreejs` with `{ sortPairs: true }` and
/// `keccak256(address)` leaves.
#[test]
fn matches_merkletreejs_for_address_lists() {
    let list = "0xEbb572059363FA4FfF24781b24650Dc8ea74d33F\n0x6246C17093D6c640E140523BA0ed3C31fb11C4AB\n";
    let whitelist = Whitelist::generate(list, Options::default()).unwrap();

    assert_eq!(
        whitelist.root_hex(),
        "0x33aebc94993cc66b731d041b417f960a3b93037a0cd48c3e376dfaa5357e4bdb"
    );
    assert_eq!(
        to_hex(&whitelist.tree.proof(0)[0]),
        "0xf310642d0057c332f26268fdaa4f89cbfada78c3e90b9a37fa844332ef1e9dfb"
    );
    assert_eq!(
        to_hex(&whitelist.tree.proof(1)[0]),
        "0x7a83618d7ac020192111667f23e832f556af4273f5b8d835af1d53dfad2be8fe"
    );
}

#[test]
fn json_output_round_trips_and_verifies() {
    let list = "\
address,amount
0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,100
0x70997970C51812dc3A010C7d01b50e0d17dc79C8,2.5
0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC,0.000001
";
    for encoding in [LeafEncoding::Packed, LeafEncoding::Double] {
        let options = Options { mode: Mode::AddressAmount, encoding, decimals: 6 };
        let whitelist = Whitelist::generate(list, options).unwrap();

        let mut buffer = Vec::new();
        whitelist.write_json(&mut buffer, true).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&buffer).unwrap();

        assert_eq!(document["mode"], "address-amount");
        assert_eq!(document["encoding"], encoding.to_string());
        assert_eq!(document["decimals"], 6);
        assert_eq!(document["count"], 3);
        assert_eq!(document["entries"][1]["amount"], "2500000");
        assert_eq!(document["entries"][2]["amount"], "1");

        let root = parse_hash(document["root"].as_str().unwrap()).unwrap();
        for entry in document["entries"].as_array().unwrap() {
            let address = parse_address(entry["address"].as_str().unwrap()).unwrap();
            let amount = parse_amount(entry["amount"].as_str().unwrap(), 0).unwrap();
            let leaf = leaf_hash(&address, Some(&amount), encoding);
            assert_eq!(to_hex(&leaf), entry["leaf"].as_str().unwrap());

            let proof: Vec<_> = entry["proof"]
                .as_array()
                .unwrap()
                .iter()
                .map(|hash| parse_hash(hash.as_str().unwrap()).unwrap())
                .collect();
            assert!(verify(&proof, &root, &leaf));
        }
    }
}

#[test]
fn address_mode_output_has_no_amount_fields() {
    let whitelist = Whitelist::generate(
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\n",
        Options::default(),
    )
    .unwrap();

    let mut buffer = Vec::new();
    whitelist.write_json(&mut buffer, false).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&buffer).unwrap();

    assert!(document.get("decimals").is_none());
    assert!(document["entries"][0].get("amount").is_none());
    assert_eq!(document["entries"][0]["proof"].as_array().unwrap().len(), 0);
    assert_eq!(document["root"], document["entries"][0]["leaf"]);
}
