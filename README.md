# Greenfield Rust SDK

A robust Rust SDK and CLI tool for interacting with the BNB Greenfield decentralized storage network.

## Features

- **On-Chain Operations**: Create objects, buckets, and manage metadata on the Greenfield blockchain.
- **Off-Chain Operations**: Securely upload files to Storage Providers using GNFD1-ECDSA authentication.
- **EIP-712 Signing**: Full support for Greenfield's specific EIP-712 transaction signing standards, ensuring compatibility with the official Go SDK.
- **Cross-Chain Bridge**: Transfer BNB from BSC to Greenfield via the TokenHub bridge.

---

## Installation

### Build from Source

```bash
git clone https://github.com/0xcrypto2024/greenfield-rs.git
cd greenfield-rs
cargo build --release
```

The binary will be located at `./target/release/greenfield_rs`.

---

## CLI Usage

### Global Options

| Option | Environment Variable | Description |
|--------|---------------------|-------------|
| `--keystore, -k` | - | Path to encrypted keystore file (recommended) |
| `--private-key, -p` | `GREENFIELD_PRIVATE_KEY` | Your wallet private key (64 hex chars) |
| `--rpc-url` | - | Greenfield RPC endpoint (default: testnet) |
| `--chain-id` | - | Chain ID (default: "greenfield_5600-1" for testnet) |

> **Note:** Use either `--keystore` or `--private-key`. Keystore is preferred for security.

### Commands

#### 1. Upload File (Recommended)

The `upload` command streamlines the process by auto-generating on-chain metadata (`MsgCreateObject`), handling checksums/sizing, and performing the authenticated upload to the Storage Provider.

```bash
./greenfield_rs --keystore <PATH> upload \
    --bucket <BUCKET_NAME> \
    --object <OBJECT_NAME> \
    --file <LOCAL_FILE_PATH> \
    --visibility 1
```

**Options:**
- `--bucket, -b`: Target bucket name
- `--object, -o`: Object name in the bucket
- `--file, -f`: Local file path
- `--sp-url`: Storage Provider URL (default: `https://gnfd-testnet-sp1.bnbchain.org`)
- `--visibility`: 1 (private), 2 (public), 3 (inherit)

#### 2. Cross-Chain Transfer (BSC -> Greenfield)

Transfer BNB from Binance Smart Chain (BSC) to your Greenfield address.

```bash
./greenfield_rs --private-key <KEY> transfer-out --amount <BNB_AMOUNT>
```

**Options:**
- `--amount, -a`: Amount of BNB to transfer (e.g., "0.1")
- `--bsc-rpc`: BSC RPC URL
- `--mainnet`: Use Mainnet addresses (default is Testnet)

#### 3. Wallet Management

Generate a new secure keystore file.

```bash
./greenfield_rs generate-key --output ./my-wallet.json
```

---

## Library Usage

To use `greenfield_rs` in your Rust project, add it to your `Cargo.toml`.

```toml
[dependencies]
greenfield_rs = { git = "https://github.com/0xcrypto2024/greenfield-rs.git" }
```

### Example: Uploading a File

```rust
use greenfield_rs::GreenfieldClient;
use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Wallet
    let wallet = LocalWallet::from_str("YOUR_PRIVATE_KEY_HEX")?
        .with_chain_id(5600u64);

    // 2. Initialize Client
    let client = GreenfieldClient::new(
        wallet,
        "https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org".to_string(), // gRPC (unused)
        "https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org".to_string(), // RPC
        "greenfield_5600-1".to_string(), // Chain ID
    );

    // 3. Perform Upload
    let result = client.upload(
        "https://gnfd-testnet-sp1.bnbchain.org",
        "my-bucket".to_string(),
        "my-object.txt".to_string(),
        "./local_file.txt".to_string(),
        1, // Visibility: Private
    ).await?;

    println!("Upload success: {}", result);
    Ok(())
}
```

---

## Recent Updates

- **EIP-712 Signing Fixes**: Resolved discrepancies in Domain Separator and Struct Hash calculations to match the official Go SDK.
- **Refactored Codebase**: Improved code organization in `eip712.rs` and `tx.rs`, ensuring better maintainability.
- **Key Management**: Added secure keystore generation and management.

## License

MIT
