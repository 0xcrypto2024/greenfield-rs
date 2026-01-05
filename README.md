# greenfield-rs

A Rust SDK and command-line tool for interacting with the BNB Greenfield decentralized storage network.

## Features

- **On-Chain Operations**: Create objects, buckets, and other metadata on the Greenfield blockchain
- **Off-Chain Operations**: Upload files to Storage Providers with GNFD1-ECDSA authentication
- **EIP-712 Signing**: Full support for Greenfield's EIP-712 transaction signing

---

## Installation

### Build from Source

```bash
cd greenfield-rs
cargo build --release
```

The binary will be at `./target/release/greenfield_rs`.

---

## CLI Usage

### Global Options

| Option | Environment Variable | Description |
|--------|---------------------|-------------|
| `--keystore, -k` | - | Path to encrypted keystore file |
| `--private-key, -p` | `GREENFIELD_PRIVATE_KEY` | Your wallet private key (64 hex chars) |
| `--rpc-url` | - | Greenfield RPC endpoint (default: testnet) |
| `--chain-id` | - | Chain ID (default: 5600 for testnet) |

> **Note:** Use either `--keystore` or `--private-key`, not both. Keystore is preferred for security.

### Commands

#### 1. Upload File (Combined On-Chain & Off-Chain)

The `upload` command is the recommended way to store files. It automatically handles:
- Creating on-chain metadata (`MsgCreateObject`)
- Calculating file checksums and size
- Authenticating with the Storage Provider
- Uploading the actual file data

```bash
./greenfield_rs --keystore <PATH> upload \
    --bucket <BUCKET_NAME> \
    --object <OBJECT_NAME> \
    --file <LOCAL_FILE_PATH>
```

**Example:**
```bash
./greenfield_rs --keystore ./my-wallet.json upload \
    --bucket my-bucket \
    --object photo.jpg \
    --file ./photos/vacation.jpg
```

**Options:**
- `--bucket, -b`: Target bucket name
- `--object, -o`: Object name in the bucket
- `--file, -f`: Local file to upload
- `--sp-url`: Storage Provider endpoint (default: testnet SP1)
- `--visibility`: Object visibility (default: 1 = private)

> **Note:** Advanced users can still use `create-object` and `put-object` for manual step-by-step operations if needed.

---

#### 2. Transfer BNB from BSC to Greenfield (Cross-Chain Bridge)

Transfers BNB from your BSC wallet to the same address on Greenfield using the TokenHub bridge contract.

```bash
./greenfield_rs --private-key <KEY> transfer-out --amount <BNB_AMOUNT>
```

**Example (Testnet):**
```bash
./greenfield_rs --private-key abc123...def transfer-out --amount 0.1
```

**Example (Mainnet):**
```bash
./greenfield_rs --private-key abc123...def transfer-out \
    --amount 0.5 \
    --mainnet \
    --bsc-rpc https://bsc-dataseed1.binance.org
```

**Options:**
- `--amount, -a`: Amount of BNB to transfer (e.g., "0.1" for 0.1 BNB)
- `--bsc-rpc`: BSC RPC URL (default: testnet)
- `--mainnet`: Use BSC mainnet instead of testnet

**Note:** A relayer fee of ~0.002 BNB is automatically added to cover cross-chain relay costs.

---

## SDK Usage (Rust Library)

Add to your `Cargo.toml`:

```toml
[dependencies]
greenfield_rs = { path = "../greenfield_rs" }
```

### Basic Example

```rust
use greenfield_rs::GreenfieldClient;
use ethers::signers::LocalWallet;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create wallet from private key
    let wallet = LocalWallet::from_str("your_private_key_hex")?
        .with_chain_id(5600u64); // Testnet chain ID

    // 2. Initialize client
    let client = GreenfieldClient::new(
        wallet,
        "https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org".to_string(), // gRPC (unused)
        "https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org".to_string(), // REST
        5600, // Chain ID
    );

    // 3. Create object metadata on chain
    let response = client.create_object(
        "my-bucket".to_string(),
        "my-object.txt".to_string(),
        1024, // payload size in bytes
        "text/plain".to_string(),
        1, // visibility: 1 = private, 2 = public
    ).await?;
    
    println!("Chain response: {}", response);

    // 4. Upload file to Storage Provider
    let upload_response = client.put_object(
        "https://gnfd-testnet-sp1.bnbchain.org",
        "my-bucket".to_string(),
        "my-object.txt".to_string(),
        "./local_file.txt".to_string(),
    ).await?;
    
    println!("Upload response: {}", upload_response);

    Ok(())
}
```

### API Reference

#### `GreenfieldClient::new`

```rust
pub fn new(
    wallet: LocalWallet,
    grpc_url: String,    // Reserved for future gRPC support
    rpc_url: String,     // REST endpoint for broadcasting
    chain_id: u64,
) -> Self
```

#### `GreenfieldClient::create_object`

Creates object metadata on the Greenfield blockchain.

```rust
pub async fn create_object(
    &self,
    bucket_name: String,
    object_name: String,
    payload_size: u64,
    content_type: String,
    visibility: i32,      // 1=private, 2=public
) -> Result<String, Box<dyn std::error::Error>>
```

**Returns:** JSON response from the blockchain.

#### `GreenfieldClient::put_object`

Uploads a file to a Storage Provider with GNFD1-ECDSA authentication.

```rust
pub async fn put_object(
    &self,
    sp_url: &str,         // e.g., "https://gnfd-testnet-sp1.bnbchain.org"
    bucket: String,
    object: String,
    file_path: String,
) -> Result<String, Box<dyn std::error::Error>>
```

**Returns:** Response from the Storage Provider.

---

## Complete Upload Flow

To upload a file to Greenfield, you typically need to:

1. **Create a Bucket** (if not exists) - via `MsgCreateBucket`
2. **Get SP Approval** - Request approval signature from the Storage Provider
3. **Create Object Metadata** - Use `create-object` command
4. **Upload File** - Use `put-object` command

> **Note:** The current implementation uses dummy approval signatures. For production use, you'll need to integrate with the SP's approval API.

---

## Testnet Endpoints

| Service | Endpoint |
|---------|----------|
| RPC (REST) | `https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org` |
| Storage Provider 1 | `https://gnfd-testnet-sp1.bnbchain.org` |
| Storage Provider 2 | `https://gnfd-testnet-sp2.bnbchain.org` |
| Chain ID | `5600` |

---

## Environment Variables

You can set these instead of passing CLI arguments:

```bash
export GREENFIELD_PRIVATE_KEY=your_64_char_hex_private_key
```

---

## Error Codes

| Code | Message | Meaning |
|------|---------|---------|
| 9 | `unknown address` | Account doesn't exist on chain (needs funding) |
| 50001 | `unsupported sign type` | Wrong authorization header format |
| 50003 | `request is tampered` | Signature doesn't match request |
| 55001 | `no such bucket` | Bucket doesn't exist (create it first) |

---

## License

MIT
