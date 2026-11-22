use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use merkle_whitelist::{
    leaf_hash, parse_address, parse_amount, parse_hash, server, to_hex, verify, LeafEncoding, Mode,
    Options, Whitelist,
};

#[derive(Parser)]
#[command(name = "merkle-whitelist", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the Merkle root and a proof for every entry of a list
    Generate(GenerateArgs),
    /// Check a proof against a Merkle root
    Verify(VerifyArgs),
    /// Start the web interface
    Serve(ServeArgs),
}

#[derive(Args)]
struct GenerateArgs {
    /// Input file with one entry per line; reads stdin when omitted or `-`
    input: Option<PathBuf>,

    /// What each line contains
    #[arg(short, long, value_enum, default_value_t = Mode::Address)]
    mode: Mode,

    /// How entries are hashed into leaves
    #[arg(short, long, value_enum, default_value_t = LeafEncoding::Packed)]
    encoding: LeafEncoding,

    /// Token decimals used to scale amounts (0 = amounts are raw units)
    #[arg(short, long, default_value_t = 0)]
    decimals: u8,

    /// Output file; writes to stdout when omitted
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Pretty-print the JSON output
    #[arg(long)]
    pretty: bool,

    /// Print only the Merkle root
    #[arg(long, conflicts_with_all = ["output", "pretty"])]
    root_only: bool,
}

#[derive(Args)]
struct VerifyArgs {
    /// Merkle root (32-byte hex)
    #[arg(short, long)]
    root: String,

    /// Address to check
    #[arg(short, long)]
    address: String,

    /// Amount tied to the address (address-amount lists only)
    #[arg(long)]
    amount: Option<String>,

    /// Token decimals used to scale the amount (0 = amount is in raw units)
    #[arg(short, long, default_value_t = 0, requires = "amount")]
    decimals: u8,

    /// How entries are hashed into leaves
    #[arg(short, long, value_enum, default_value_t = LeafEncoding::Packed)]
    encoding: LeafEncoding,

    /// Proof hashes, comma-separated or repeated; a JSON array also works
    #[arg(short, long, value_delimiter = ',', num_args = 0..)]
    proof: Vec<String>,
}

#[derive(Args)]
struct ServeArgs {
    /// Address to listen on
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    host: IpAddr,

    /// Port to listen on
    #[arg(short, long, env = "PORT", default_value_t = 8080)]
    port: u16,

    /// Maximum size of a submitted list, in megabytes
    #[arg(long, env = "MAX_BODY_MB", default_value_t = 64)]
    max_body_mb: usize,
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Generate(args) => generate(args).map(|()| ExitCode::SUCCESS),
        Command::Verify(args) => verify_proof(args),
        Command::Serve(args) => serve(args).map(|()| ExitCode::SUCCESS),
    };

    result.unwrap_or_else(|error| {
        eprintln!("error: {error:#}");
        ExitCode::from(2)
    })
}

fn generate(args: GenerateArgs) -> Result<()> {
    let text = match &args.input {
        Some(path) if path.as_os_str() != "-" => fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
        _ => {
            let mut text = String::new();
            io::stdin().read_to_string(&mut text).context("failed to read stdin")?;
            text
        }
    };

    let options = Options { mode: args.mode, encoding: args.encoding, decimals: args.decimals };
    let whitelist = Whitelist::generate(&text, options)?;

    if args.root_only {
        println!("{}", whitelist.root_hex());
        return Ok(());
    }

    match &args.output {
        Some(path) => {
            let file = File::create(path)
                .with_context(|| format!("failed to create {}", path.display()))?;
            write_output(&whitelist, BufWriter::new(file), args.pretty)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        None => write_output(&whitelist, BufWriter::new(io::stdout().lock()), args.pretty)
            .context("failed to write to stdout")?,
    }

    eprintln!("Merkle root: {}", whitelist.root_hex());
    eprintln!("Entries:     {}", whitelist.entries.len());
    if whitelist.duplicates_removed > 0 {
        eprintln!("Duplicates:  {} removed", whitelist.duplicates_removed);
    }
    if let Some(path) = &args.output {
        eprintln!("Output:      {}", path.display());
    }
    Ok(())
}

fn write_output<W: Write>(whitelist: &Whitelist, mut writer: W, pretty: bool) -> Result<()> {
    whitelist.write_json(&mut writer, pretty)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn verify_proof(args: VerifyArgs) -> Result<ExitCode> {
    let root = parse_hash(&args.root).map_err(|e| anyhow!(e))?;
    let address = parse_address(&args.address).map_err(|e| anyhow!(e))?;
    let amount = args
        .amount
        .as_deref()
        .map(|amount| parse_amount(amount, args.decimals))
        .transpose()
        .map_err(|e| anyhow!(e))?;

    let mut proof = Vec::new();
    for item in &args.proof {
        // Tolerate a pasted JSON array: ["0x..","0x.."]
        let item = item.trim_matches(|c: char| c.is_whitespace() || "[]\"'".contains(c));
        if !item.is_empty() {
            proof.push(parse_hash(item).map_err(|e| anyhow!(e))?);
        }
    }
    if proof.len() > 256 {
        bail!("proof is too long ({} hashes)", proof.len());
    }

    let leaf = leaf_hash(&address, amount.as_ref(), args.encoding);
    println!("Leaf: {}", to_hex(&leaf));

    if verify(&proof, &root, &leaf) {
        println!("Valid: the proof matches the root");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("Invalid: the proof does not match the root");
        Ok(ExitCode::FAILURE)
    }
}

fn serve(args: ServeArgs) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start the async runtime")?
        .block_on(server::serve(args.host, args.port, args.max_body_mb * 1024 * 1024))
}
