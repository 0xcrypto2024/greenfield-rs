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
    /// List all storage providers on the network
    ListSp,
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
    /// If --sp-url is not provided, it will be auto-fetched from the bucket's primary SP
    Upload {
        /// Storage Provider URL (optional, auto-detected from bucket if not provided)
        #[arg(long, default_value = "")]
        sp_url: String,
        #[arg(short, long)]
        bucket: String,
        #[arg(short, long)]
        object: String,
        #[arg(short, long)]
        file: PathBuf,
        #[arg(long, default_value_t = 2)]
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
    /// Get bucket information
    HeadBucket {
        #[arg(short, long)]
        bucket: String,
    },
    /// Create a new bucket on Greenfield
    CreateBucket {
        /// Bucket name (must be globally unique)
        #[arg(short, long)]
        bucket: String,
        /// Primary SP address (use list-sp to find available SPs)
        #[arg(long)]
        sp_address: String,
        /// Visibility (1=public, 2=private, 3=inherit)
        #[arg(long, default_value_t = 2)]
        visibility: i32,
    },
    /// Debug EIP-712 calculation (no transaction sent)
    DebugEip712 {
        #[arg(short, long)]
        bucket: String,
        #[arg(short, long)]
        object: String,
        /// Payload size in bytes
        #[arg(long)]
        payload_size: u64,
        /// Account sequence (nonce)
        #[arg(long)]
        sequence: u64,
        /// Account number
        #[arg(long)]
        account_number: u64,
        /// Fee amount in wei (e.g., 6000000000000)
        #[arg(long, default_value = "6000000000000")]
        fee_amount: String,
        /// Gas limit
        #[arg(long, default_value = "1200")]
        gas_limit: String,
        /// Visibility (1=public, 2=private)
        #[arg(long, default_value_t = 2)]
        visibility: i32,
    },
    /// Debug CreateBucket EIP-712 (compare with Go SDK)
    DebugCreateBucket {
        /// Bucket name
        #[arg(short, long)]
        bucket: String,
        /// Primary SP address
        #[arg(long)]
        sp_address: String,
        /// Account sequence (nonce)
        #[arg(long)]
        sequence: u64,
        /// Account number
        #[arg(long)]
        account_number: u64,
        /// Fee amount in wei (e.g., 12000000000000)
        #[arg(long, default_value = "12000000000000")]
        fee_amount: String,
        /// Gas limit
        #[arg(long, default_value = "2400")]
        gas_limit: String,
        /// Global virtual group family ID (from SP)
        #[arg(long, default_value_t = 3)]
        gvg_family_id: u32,
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
        Commands::ListSp => {
            println!("📡 Fetching storage providers from {}...\n", cli.rpc_url);
            let sps = greenfield_rs::list_storage_providers(&cli.rpc_url).await?;
            
            if sps.is_empty() {
                println!("No storage providers found.");
            } else {
                println!("Found {} storage providers:\n", sps.len());
                for sp in sps {
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("  ID: {}", sp.id);
                    println!("  Operator: {}", sp.operator_address);
                    println!("  Endpoint: {}", sp.endpoint);
                    println!("  Status: {:?}", sp.status);
                    if let Some(desc) = &sp.description {
                        println!("  Description: {}", desc.moniker);
                    }
                }
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            return Ok(());
        }
        Commands::HeadBucket { bucket } => {
            println!("📦 Fetching bucket info for '{}'...\n", bucket);
            let info = greenfield_rs::get_bucket_info(&cli.rpc_url, &bucket).await?;
            
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("  Bucket ID: {}", info.id);
            println!("  Name: {}", info.bucket_name);
            println!("  Owner: {}", info.owner);
            println!("  Visibility: {}", info.visibility);
            println!("  Global VGF ID: {}", info.global_virtual_group_family_id);
            println!("  Payment Address: {}", info.payment_address);
            println!("  Status: {}", info.bucket_status);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            return Ok(());
        }
        Commands::DebugCreateBucket {
            bucket,
            sp_address,
            sequence,
            account_number,
            fee_amount,
            gas_limit,
            gvg_family_id,
        } => {
            // Extract EIP-155 chain ID
            let eip155_chain_id = extract_eip155_chain_id(&cli.chain_id)?;
            
            // Load wallet
            let wallet = if let Some(keystore_path) = &cli.keystore {
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

            let creator = ethers::utils::to_checksum(&wallet.address(), None);
            
            println!("🔍 Debug CreateBucket EIP-712 Calculation\n");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Parameters:");
            println!("  Creator: {}", creator);
            println!("  Bucket: {}", bucket);
            println!("  Primary SP: {}", sp_address);
            println!("  Sequence: {}", sequence);
            println!("  Account Number: {}", account_number);
            println!("  Fee Amount: {}", fee_amount);
            println!("  Gas Limit: {}", gas_limit);
            println!("  GVG Family ID: {}", gvg_family_id);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            
            // Build CreateBucket message
            use greenfield_rs::bucket_eip712::{TxCreateBucket, MsgCreateBucket, PrimarySpApproval};
            use greenfield_rs::eip712::{Fee, Coin, Visibility};
            
            let tx = TxCreateBucket {
                account_number: account_number.to_string(),
                chain_id: "5600".to_string(),
                fee: Fee {
                    amount: vec![Coin {
                        denom: "BNB".to_string(),
                        amount: fee_amount.clone(),
                    }],
                    gas_limit: gas_limit.clone(),
                    granter: "".to_string(),
                    payer: creator.clone(),
                },
                memo: "".to_string(),
                msg1: MsgCreateBucket {
                    type_url: "/greenfield.storage.MsgCreateBucket".to_string(),
                    bucket_name: bucket.clone(),
                    charged_read_quota: "0".to_string(),
                    creator: creator.clone(),
                    payment_address: "".to_string(),
                    primary_sp_address: sp_address.clone(),
                    primary_sp_approval: PrimarySpApproval {
                        expired_height: "0".to_string(),
                        global_virtual_group_family_id: *gvg_family_id,
                    },
                    visibility: Visibility::Private,
                },
                sequence: sequence.to_string(),
                timeout_height: "0".to_string(),
            };
            
            println!("📋 EIP-712 JSON Payload:");
            println!("{}", serde_json::to_string_pretty(&tx)?);
            println!();
            
            println!("🔐 Calculating EIP-712 Hash...\n");
            let hash = tx.get_eip712_hash(&cli.chain_id)?;
            println!("\n🔍 Final EIP-712 Hash: 0x{}", hex::encode(hash.as_bytes()));
            
            // Compare with Go SDK values
            println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📊 Expected values from Go SDK (if matching parameters):");
            println!("   Domain Separator: 0xaf415613702189c52c2d91875089d82e2dacb207c04dfd210072b73f0ed78b7a");
            println!("   Tx TypeHash:      0xc5ce375061c176a3775dc7754ff9711ef8ef729c7600f7a08c208450577c7ad4");
            println!("   Msg1 TypeHash:    0x9fdc1819860d992cf5e8bc34f348b72968551e7a4b8e1f2f4497f1404f9e2b28");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            
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

    let rpc_url = cli.rpc_url.clone();
    let client = GreenfieldClient::new(
        wallet,
        rpc_url.clone(),
        rpc_url.clone(),
        cli.chain_id.clone(),
    );

    match cli.command {
        Commands::GenerateKey { .. } => unreachable!(), // Already handled above
        Commands::ListSp => unreachable!(), // Already handled above
        Commands::HeadBucket { .. } => unreachable!(), // Already handled above
        Commands::DebugCreateBucket { .. } => unreachable!(), // Already handled above
        Commands::DebugEip712 {
            bucket,
            object,
            payload_size,
            sequence,
            account_number,
            fee_amount,
            gas_limit,
            visibility,
        } => {
            println!("🔍 Debug EIP-712 Calculation\n");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("Parameters:");
            println!("  Bucket: {}", bucket);
            println!("  Object: {}", object);
            println!("  Payload Size: {}", payload_size);
            println!("  Sequence: {}", sequence);
            println!("  Account Number: {}", account_number);
            println!("  Fee Amount: {}", fee_amount);
            println!("  Gas Limit: {}", gas_limit);
            println!("  Visibility: {}", visibility);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            
            // Get bucket info to get global_virtual_group_family_id
            println!("📦 Fetching bucket info...");
            let bucket_info = greenfield_rs::get_bucket_info(&rpc_url, &bucket).await?;
            println!("  Global VGF ID: {}", bucket_info.global_virtual_group_family_id);
            
            // Calculate EIP-712 hash
            client.debug_eip712(
                bucket,
                object,
                payload_size,
                visibility,
                bucket_info.global_virtual_group_family_id,
                sequence,
                account_number,
                fee_amount.parse()?,
                gas_limit.parse()?,
            ).await?;
        }
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
            let file_path = file.to_string_lossy().to_string();
            let size = std::fs::metadata(&file)?.len();
            let content_type = "application/octet-stream".to_string();

            println!("Creating Object: {}/{} ({} bytes)...", bucket, object, size);

            // Use create_object_with_file to compute checksums
            let res = client
                .create_object_with_file(
                    bucket.to_string(),
                    object.to_string(),
                    &file_path,
                    content_type,
                    visibility,
                )
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
        Commands::CreateBucket {
            bucket,
            sp_address,
            visibility,
        } => {
            println!("🪣 Creating bucket '{}'...", bucket);
            match client.create_bucket(bucket, sp_address, visibility).await {
                Ok(tx_hash) => println!("✅ Bucket created! TxHash: {}", tx_hash),
                Err(e) => println!("❌ Create bucket failed: {}", e),
            }
        }
    }

    Ok(())
}
