use clap::{Parser, Subcommand};
use ethers::signers::{LocalWallet, Signer};
use greenfield_rs::{extract_eip155_chain_id, GreenfieldClient};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "greenfield_rs")]
#[command(about = "Greenfield Rust SDK & CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Private key hex (64 characters)
    #[arg(short, long, env = "GREENFIELD_PRIVATE_KEY")]
    private_key: Option<String>,

    /// Path to encrypted keystore file
    #[arg(short, long)]
    keystore: Option<PathBuf>,

    /// RPC URL (REST)
    #[arg(
        long,
        default_value = "https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org"
    )]
    rpc_url: String,

    /// Chain ID (e.g., "greenfield_5600-1" for testnet, "greenfield_1017-1" for mainnet)
    #[arg(long, default_value = "greenfield_5600-1")]
    chain_id: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new wallet and save as encrypted keystore
    GenerateKey {
        /// Output path for keystore file (default: ./keystore.json)
        #[arg(short, long, default_value = "./keystore.json")]
        output: PathBuf,
    },
    /// Create Object Metadata on chain
    CreateObject {
        #[arg(short, long)]
        bucket: String,
        #[arg(short, long)]
        object: String,
        #[arg(short, long)]
        file: PathBuf,
        #[arg(long, default_value_t = 1)]
        visibility: i32,
    },
    /// Import key (Placeholder for previous logic)
    ImportKey {
        #[arg(short, long)]
        file: PathBuf,
    },
    /// Upload file (creates metadata on-chain and uploads to SP)
    Upload {
        /// Storage Provider URL
        #[arg(long, default_value = "https://gnfd-testnet-sp1.bnbchain.org")]
        sp_url: String,
        #[arg(short, long)]
        bucket: String,
        #[arg(short, long)]
        object: String,
        #[arg(short, long)]
        file: PathBuf,
        #[arg(long, default_value_t = 1)]
        visibility: i32,
    },
    /// Upload file to Storage Provider
    PutObject {
        /// Storage Provider URL
        #[arg(long, default_value = "https://gnfd-testnet-sp1.bnbchain.org")]
        sp_url: String,
        #[arg(short, long)]
        bucket: String,
        #[arg(short, long)]
        object: String,
        #[arg(short, long)]
        file: PathBuf,
    },
    /// Transfer BNB from BSC to Greenfield (cross-chain bridge)
    TransferOut {
        /// BSC RPC URL (Testnet or Mainnet)
        #[arg(long, default_value = "https://data-seed-prebsc-1-s1.binance.org:8545")]
        bsc_rpc: String,
        /// Amount of BNB to transfer (e.g., "0.1")
        #[arg(short, long)]
        amount: String,
        /// Use mainnet instead of testnet
        #[arg(long, default_value_t = false)]
        mainnet: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle commands that don't need a wallet first
    match &cli.command {
        Commands::GenerateKey { output } => {
            use eth_keystore::encrypt_key;
            use rand::RngCore;
            use std::io::Write;

            println!("🔐 Generating new wallet...\n");

            // Generate random 32-byte private key
            let mut rng = rand::thread_rng();
            let mut private_key = [0u8; 32];
            rng.fill_bytes(&mut private_key);

            // Create wallet from private key to get address
            let wallet = LocalWallet::from_bytes(&private_key)?;
            let address = wallet.address();

            println!("📍 Address: {:?}", address);
            println!("🔑 Private Key: {}", hex::encode(&private_key));

            // Prompt for password
            print!("\nEnter password to encrypt keystore: ");
            std::io::stdout().flush()?;
            let password = rpassword::read_password()?;

            if password.len() < 8 {
                return Err("Password must be at least 8 characters".into());
            }

            print!("Confirm password: ");
            std::io::stdout().flush()?;
            let password_confirm = rpassword::read_password()?;

            if password != password_confirm {
                return Err("Passwords do not match".into());
            }

            // Get the directory and filename
            let dir = output.parent().unwrap_or(std::path::Path::new("."));
            let name = output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("keystore");

            // Encrypt and save keystore
            let uuid = encrypt_key(dir, &mut rng, &private_key, &password, Some(name))?;

            println!("\n✅ Keystore saved!");
            println!("   File: {}", output.display());
            println!("   UUID: {}", uuid);
            println!("\n⚠️  IMPORTANT: Save your private key and password securely!");
            println!("   The keystore file alone is not enough to recover your wallet.");

            return Ok(());
        }
        _ => {}
    }

    // Commands that require a wallet
    // Extract EIP-155 chain ID for wallet (Ethereum signing uses numeric ID)
    let eip155_chain_id = extract_eip155_chain_id(&cli.chain_id)?;

    let wallet = if let Some(keystore_path) = &cli.keystore {
        // Load from keystore file
        if !keystore_path.exists() {
            return Err(format!("Keystore file not found: {:?}", keystore_path).into());
        }
        println!("🔐 Loading wallet from keystore: {:?}", keystore_path);
        let password = rpassword::prompt_password("Enter keystore password: ")?;
        let decrypted = eth_keystore::decrypt_key(keystore_path, &password)
            .map_err(|e| format!("Failed to decrypt keystore: {:?}", e))?;
        let wallet = LocalWallet::from_bytes(&decrypted)
            .map_err(|e| format!("Failed to create wallet from keystore: {:?}", e))?;
        wallet.with_chain_id(eip155_chain_id)
    } else if let Some(pk) = &cli.private_key {
        LocalWallet::from_str(pk)?.with_chain_id(eip155_chain_id)
    } else {
        return Err("Wallet required. Use --keystore <path> or --private-key <hex>".into());
    };

    println!("🔑 Wallet: {:?}", wallet.address());

    let client = GreenfieldClient::new(
        wallet,
        cli.rpc_url.clone(),
        cli.rpc_url,
        cli.chain_id.clone(),
    );

    match cli.command {
        Commands::GenerateKey { .. } => unreachable!(), // Already handled above
        Commands::Upload {
            sp_url,
            bucket,
            object,
            file,
            visibility,
        } => {
            if !file.exists() {
                return Err("File does not exist".into());
            }
            println!("🚀 Starting unified upload for {}...", file.display());
            let file_path = file.to_string_lossy().to_string();

            match client
                .upload(&sp_url, bucket, object, file_path, visibility)
                .await
            {
                Ok(res) => println!("✅ Upload completed: {}", res),
                Err(e) => println!("❌ Upload failed: {}", e),
            }
        }
        Commands::CreateObject {
            bucket,
            object,
            file,
            visibility,
        } => {
            if !file.exists() {
                return Err("File does not exist".into());
            }
            let size = std::fs::metadata(&file)?.len();
            let content_type = "application/octet-stream".to_string();

            println!("Creating Object: {}/{} ({} bytes)...", bucket, object, size);

            let res = client
                .create_object(bucket, object, size, content_type, visibility)
                .await?;
            println!("Response: {}", res);
        }
        Commands::ImportKey { .. } => {
            println!("Import key feature not yet re-integrated in this refactor.");
        }
        Commands::PutObject {
            sp_url,
            bucket,
            object,
            file,
        } => {
            if !file.exists() {
                return Err("File does not exist".into());
            }
            println!(
                "Uploading {} to {}/{} via {}...",
                file.display(),
                bucket,
                object,
                sp_url
            );

            let file_path = file.to_string_lossy().to_string();
            match client.put_object(&sp_url, bucket, object, file_path).await {
                Ok(res) => println!("Upload Response: {}", res),
                Err(e) => println!("Upload Error: {}", e),
            }
        }
        Commands::TransferOut {
            bsc_rpc,
            amount,
            mainnet,
        } => {
            println!("🚀 Initiating BSC -> Greenfield bridge transfer...");
            match client.transfer_out(&bsc_rpc, &amount, mainnet).await {
                Ok(tx_hash) => println!("✅ Bridge transfer initiated! TX: {}", tx_hash),
                Err(e) => println!("❌ Bridge transfer failed: {}", e),
            }
        }
    }

    Ok(())
}
