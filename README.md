# Merkle Builder

A fast Merkle tree and proof generator for EVM whitelists and airdrops, written in Rust.
It ships as a single binary with a **command-line interface** and a **web interface**, and runs in Docker.

Two list modes are supported:

| Mode | Each line contains | Typical use |
| --- | --- | --- |
| `address` | an address | NFT whitelists / allowlists |
| `address-amount` | an address and an amount | token airdrops, per-wallet mint limits |

The output is a Merkle root plus a proof for every entry, ready to be verified on-chain with
OpenZeppelin's [`MerkleProof`](https://docs.openzeppelin.com/contracts/5.x/api/utils#MerkleProof) library.

## Contents

- [Quick start](#quick-start)
- [Web interface](#web-interface)
- [Command-line interface](#command-line-interface)
- [Input format](#input-format)
- [Output format](#output-format)
- [Leaf encoding and Solidity verification](#leaf-encoding-and-solidity-verification)
- [How the tree is built](#how-the-tree-is-built)
- [HTTP API](#http-api)
- [Configuration](#configuration)
- [Performance](#performance)
- [Development](#development)
- [License](#license)

## Quick start

### Docker Compose (web interface)

```bash
docker compose up -d --build
```

Then open <http://localhost:8080>.

### Docker (CLI)

```bash
docker build -t merkle-builder .
```

```bash
docker run --rm -i merkle-builder generate --mode address < examples/addresses.txt > output.json
```

### From source

Requires a recent stable [Rust toolchain](https://rustup.rs).

```bash
cargo install --path .
```

```bash
merkle-builder generate examples/addresses.txt -o output.json
```

## Web interface

Start the server with Docker Compose (see above) or directly:

```bash
merkle-builder serve
```

By default it listens on `http://127.0.0.1:8080` (the Docker image listens on `0.0.0.0:8080`).

In the browser you can:

- switch between **Address list** and **Address + amount list** mode,
- paste a list, load a `.txt` / `.csv` file, or drop a file onto the text box,
- choose the leaf encoding and, for amounts, the token decimals,
- copy the Merkle root and download the full result as JSON,
- look up the proof of a single address.

Invalid lines are reported with their line numbers, so nothing is silently skipped.

> The web interface has no authentication. It keeps nothing on disk and holds no secrets, but if you
> expose it publicly, put it behind a reverse proxy and consider lowering `MAX_BODY_MB`.

## Command-line interface

```text
merkle-builder <COMMAND>

Commands:
  generate  Generate the Merkle root and a proof for every entry of a list
  verify    Check a proof against a Merkle root
  serve     Start the web interface
```

Run `merkle-builder <COMMAND> --help` for the full list of options.

### `generate`

```text
merkle-builder generate [OPTIONS] [INPUT]
```

| Option | Default | Description |
| --- | --- | --- |
| `[INPUT]` | stdin | Input file. Reads stdin when omitted or `-`. |
| `-m, --mode <MODE>` | `address` | `address` or `address-amount`. |
| `-e, --encoding <ENCODING>` | `packed` | `packed` or `double`. See [leaf encoding](#leaf-encoding-and-solidity-verification). |
| `-d, --decimals <N>` | `0` | Token decimals used to scale amounts. `0` means amounts are raw units. |
| `-o, --output <FILE>` | stdout | Where to write the JSON result. |
| `--pretty` | off | Pretty-print the JSON. |
| `--root-only` | off | Print only the Merkle root. |

The JSON goes to stdout (or `--output`); a short summary goes to stderr, so piping is safe.

Examples:

```bash
# Address list
merkle-builder generate examples/addresses.txt -o whitelist.json

# Address + amount list, amounts given in raw units (wei)
merkle-builder generate examples/address-amounts.csv --mode address-amount -o airdrop.json

# Amounts given as human-readable token amounts of an 18-decimal token
merkle-builder generate examples/address-amounts.csv --mode address-amount --decimals 18 -o airdrop.json

# Only the root, reading from stdin
cat examples/addresses.txt | merkle-builder generate --root-only
```

### `verify`

Checks a proof off-chain, exactly as the contract would.

```bash
merkle-builder verify \
  --root 0xc02e90f56c90f21a1f3d88e4640f52cafd80467679d516857369d8751b42b22e \
  --address 0x90F79bf6EB2c4f870365E785982E1f101E93b906 \
  --amount 1200 \
  --proof 0xe752dea18ea9e1b4cf0c7a83872601aefa03102c04c44a8b6e26b19771888893,0x76667a6715089b36b034e61c73ec853dcf75c9e1f090e5173cd641939345b56c,0xc3719d352a886ef1d7248c1d2d2a04541007832900bc21cc463fbf2f30bb051b
```

Omit `--amount` for address lists. Use the same `--encoding` and `--decimals` as in `generate`.
`--proof` accepts comma-separated hashes or a pasted JSON array.

Exit codes: `0` valid, `1` invalid, `2` usage or input error.

### Using the CLI through Docker

Pipe the list in and redirect the result out:

```bash
docker run --rm -i merkle-builder generate --mode address-amount < examples/address-amounts.csv > airdrop.json
```

Or mount a directory. The container runs as an unprivileged user, so pass your own user ID when
it needs to write into the mounted directory:

```bash
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" merkle-builder generate examples/addresses.txt -o whitelist.json
```

## Input format

One entry per line.

**Address list**

```text
0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
0x70997970C51812dc3A010C7d01b50e0d17dc79C8
```

**Address + amount list**

```text
address,amount
0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,100
0x70997970C51812dc3A010C7d01b50e0d17dc79C8,250
```

Rules:

- Blank lines and lines starting with `#` are ignored.
- A first line starting with `address` is treated as a CSV header and ignored.
- Fields may be separated by a comma, a semicolon, a tab or spaces.
- The `0x` prefix is optional.
- All-lowercase and all-uppercase addresses are accepted as-is. Mixed-case addresses must have a
  valid [EIP-55](https://eips.ethereum.org/EIPS/eip-55) checksum, which catches typos.
- Amounts are non-negative decimal numbers that fit in a `uint256`.
  - With `--decimals 0` (default) an amount must be an integer and is used as-is (raw units).
  - With `--decimals N` an amount is multiplied by `10^N`, so `1.5` with 18 decimals becomes
    `1500000000000000000`. More than `N` fractional digits is an error.
- Exact duplicate entries are removed and counted in `duplicatesRemoved`.
- The same address with two different amounts is an error.
- The order of the list is preserved. Reordering it changes the root.

Any invalid line aborts the run with a list of line numbers and reasons. No output is produced
from a partially valid list.

## Output format

```json
{
  "root": "0x84df966d3419391f8a021ec46c92dd2b605a8fee399aa26bec618a5326baed25",
  "mode": "address-amount",
  "encoding": "packed",
  "decimals": 18,
  "count": 2,
  "duplicatesRemoved": 0,
  "entries": [
    {
      "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
      "amount": "1500000000000000000",
      "leaf": "0x3d849b92c64b97008e113b7128bc47dc27eee6b1e679de4eca13bb8829523f3d",
      "proof": [
        "0x931fd3a38611a0c1d2ec3d34b47329a54f28e840699c32699028a617448b8d5b"
      ]
    },
    {
      "address": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      "amount": "2000000000000000000",
      "leaf": "0x931fd3a38611a0c1d2ec3d34b47329a54f28e840699c32699028a617448b8d5b",
      "proof": [
        "0x3d849b92c64b97008e113b7128bc47dc27eee6b1e679de4eca13bb8829523f3d"
      ]
    }
  ]
}
```

| Field | Description |
| --- | --- |
| `root` | The Merkle root to store in your contract. |
| `mode`, `encoding` | The settings the tree was built with. |
| `decimals` | Only in `address-amount` mode. |
| `count` | Number of entries in the tree. |
| `duplicatesRemoved` | Number of duplicate lines dropped from the input. |
| `entries[].address` | EIP-55 checksummed address. |
| `entries[].amount` | Only in `address-amount` mode. Always in raw units, as a decimal string. Pass this value to the contract. |
| `entries[].leaf` | The leaf hash of the entry. |
| `entries[].proof` | The proof to pass to the contract. |

## Leaf encoding and Solidity verification

Your contract must hash leaves the same way the tool does.

| Encoding | `address` mode | `address-amount` mode |
| --- | --- | --- |
| `packed` (default) | `keccak256(abi.encodePacked(account))` | `keccak256(abi.encodePacked(account, amount))` |
| `double` | `keccak256(bytes.concat(keccak256(abi.encode(account))))` | `keccak256(bytes.concat(keccak256(abi.encode(account, amount))))` |

`amount` is a `uint256`.

`packed` is the common whitelist format and matches what `merkletreejs` users typically deploy.
`double` is the double-hashed leaf format recommended by OpenZeppelin. Its proofs verify with
`MerkleProof`, but the root is not the one the `@openzeppelin/merkle-tree` JavaScript library
would produce for the same list, because that library also sorts the leaves.

### Address list

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {MerkleProof} from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";

contract Whitelist {
    bytes32 public immutable merkleRoot;

    constructor(bytes32 root) {
        merkleRoot = root;
    }

    function isWhitelisted(address account, bytes32[] calldata proof) public view returns (bool) {
        bytes32 leaf = keccak256(abi.encodePacked(account));
        return MerkleProof.verifyCalldata(proof, merkleRoot, leaf);
    }
}
```

### Address + amount list

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {MerkleProof} from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";

contract Airdrop {
    bytes32 public immutable merkleRoot;
    mapping(address => bool) public claimed;

    constructor(bytes32 root) {
        merkleRoot = root;
    }

    function claim(uint256 amount, bytes32[] calldata proof) external {
        require(!claimed[msg.sender], "Already claimed");
        bytes32 leaf = keccak256(abi.encodePacked(msg.sender, amount));
        require(MerkleProof.verifyCalldata(proof, merkleRoot, leaf), "Invalid proof");

        claimed[msg.sender] = true;
        // transfer `amount` tokens to msg.sender
    }
}
```

These contracts are minimal examples, not audited code.

## How the tree is built

- Leaves are hashed with Keccak-256 and kept in input order.
- Each pair of nodes is sorted before hashing (`keccak256(min(a, b) ++ max(a, b))`), so proofs
  need no left/right flags.
- A node without a sibling at the end of a layer is promoted to the next layer unchanged.

This is the same construction as `merkletreejs` with `{ sortPairs: true }`. For an address list
with the `packed` encoding, the root and proofs are identical to the ones `merkletreejs` produces;
the test suite checks this against a reference output.

## HTTP API

The web interface uses a small JSON API that you can also call directly.

### `POST /api/generate`

Request:

```json
{
  "mode": "address-amount",
  "encoding": "packed",
  "decimals": 18,
  "input": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,1.5\n0x70997970C51812dc3A010C7d01b50e0d17dc79C8,2"
}
```

`mode`, `encoding` and `decimals` are optional and default to `address`, `packed` and `0`.

A successful response has status `200` and the [output format](#output-format) above. Invalid
input returns status `400`:

```json
{
  "error": "1 line could not be parsed",
  "details": [
    { "line": 1, "message": "invalid address `0x12`: expected 40 hex characters, found 2" }
  ]
}
```

Example:

```bash
curl -s http://localhost:8080/api/generate -H 'Content-Type: application/json' -d '{"mode":"address","input":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"}'
```

### `GET /healthz`

Returns `ok`. Used by the Docker health check.

## Configuration

Options of `merkle-builder serve`, each with an environment variable:

| Flag | Environment variable | Default | Description |
| --- | --- | --- | --- |
| `--host` | `HOST` | `127.0.0.1` (`0.0.0.0` in Docker) | Address to listen on. |
| `-p, --port` | `PORT` | `8080` | Port to listen on. |
| `--max-body-mb` | `MAX_BODY_MB` | `64` | Maximum size of a submitted list, in megabytes. |

With Docker Compose, `PORT` sets the published host port and `MAX_BODY_MB` is passed through:

```bash
PORT=3000 MAX_BODY_MB=128 docker compose up -d
```

## Performance

Proofs are read from the precomputed tree layers, so generation scales as `O(n log n)`.

Measured on a desktop CPU (Intel Core Ultra 5 245KF), single-threaded, release build, with
1,000,000 random entries:

| Run | Time |
| --- | --- |
| `address` mode, root only | 1.1 s |
| `address` mode, full output (1.5 GB of JSON) | 4.5 s |
| `address-amount` mode, full output | 6.3 s |

For very large lists prefer the CLI: the web interface holds the whole result in the browser's memory.

## Development

```bash
cargo test
```

```bash
cargo clippy --all-targets
```

```bash
cargo run -- serve
```

Project layout:

```text
src/
  main.rs      CLI entry point
  lib.rs       leaf hashing, generation, JSON output
  address.rs   address parsing and EIP-55 checksums
  amount.rs    uint256 amount parsing and decimal scaling
  input.rs     list parsing and validation
  tree.rs      Merkle tree, proofs, verification
  server.rs    web server and HTTP API
web/
  index.html   web interface (embedded into the binary)
tests/
  compat.rs    end-to-end tests against reference values
examples/      sample input lists
```

## License

Licensed under the [Apache License 2.0](LICENSE).
