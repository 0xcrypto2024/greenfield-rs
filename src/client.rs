use crate::eip712::{MsgValue as Eip712MsgValue, PrimarySpApproval as Eip712Approval};
use crate::proto::greenfield::common::Approval as ProtoApproval;
use crate::proto::greenfield::storage::MsgCreateObject as ProtoMsgCreateObject;
use crate::tx::create_signed_tx;

use base64::Engine;
use ethers::signers::{LocalWallet, Signer};
use prost::Message;
use reqwest::Client as HttpClient;

#[derive(Clone)]
pub struct GreenfieldClient {
    wallet: LocalWallet,
    chain_id: String,
    #[allow(dead_code)]
    grpc_url: String, // Reserved for future gRPC support
    rpc_url: String,
    http_client: HttpClient,
}

#[derive(serde::Serialize)]
struct BroadcastReq {
    tx_bytes: String,
    mode: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct AccountInfo {
    pub account_number: u64,
    pub sequence: u64,
}

#[derive(serde::Deserialize)]
struct RestAccountResponse {
    account: RestAccountData,
}

#[derive(serde::Deserialize)]
pub struct TxResponse {
    pub code: u32,
    pub txhash: String,
    pub raw_log: String,
}

#[derive(serde::Deserialize)]
struct BroadcastResponse {
    pub tx_response: TxResponse,
}

#[derive(serde::Deserialize)]
struct RestAccountData {
    #[serde(default)]
    account_number: String,
    #[serde(default)]
    sequence: String,
    #[serde(default)]
    base_account: Option<BaseAccountData>,
}

#[derive(serde::Deserialize)]
struct BaseAccountData {
    account_number: String,
    sequence: String,
}

impl GreenfieldClient {
    pub fn new(wallet: LocalWallet, grpc_url: String, rpc_url: String, chain_id: String) -> Self {
        Self {
            wallet,
            chain_id,
            grpc_url,
            rpc_url,
            http_client: HttpClient::new(),
        }
    }

    pub async fn get_account_info(&self) -> Result<AccountInfo, Box<dyn std::error::Error>> {
        let address = self.wallet.address();
        let url = format!(
            "{}/cosmos/auth/v1beta1/accounts/{:?}",
            self.rpc_url, address
        );

        let resp = self.http_client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("Failed to fetch account info: {}", resp.status()).into());
        }

        let data: RestAccountResponse = resp.json().await?;

        let (acc_num, seq) = if let Some(base) = data.account.base_account {
            (base.account_number, base.sequence)
        } else {
            (data.account.account_number, data.account.sequence)
        };

        Ok(AccountInfo {
            account_number: acc_num.parse()?,
            sequence: seq.parse()?,
        })
    }

    pub async fn create_object(
        &self,
        bucket_name: String,
        object_name: String,
        payload_size: u64,
        content_type: String,
        visibility: i32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let address = self.wallet.address();

        // 1. 定义枚举（必须能转为 Greenfield 期待的字符串全称）
        let visibility_enum = match visibility {
            1 => crate::eip712::Visibility::Public,
            2 => crate::eip712::Visibility::Private,
            3 => crate::eip712::Visibility::Inherit,
            _ => crate::eip712::Visibility::Public,
        };

        // Dummy Approval (Phase 3 will fetch signature from SP)
        let proto_approval = ProtoApproval {
            expired_height: 18446744073709551615,
            global_virtual_group_family_id: 0,
            sig: vec![],
        };
        let eip_approval = Eip712Approval {
            expired_height: "18446744073709551615".to_string(),
            global_virtual_group_family_id: "0".to_string(), // String for EIP-712
        };

        // Dummy Checksums (Phase 3 will calc real checksums)
        let checksums_proto = vec![vec![0u8; 32]];
        let checksums_eip = vec![];

        let bech32_addr = self.get_bech32_address();

        let proto_msg = ProtoMsgCreateObject {
            creator: bech32_addr.clone(),
            bucket_name: bucket_name.clone(),
            object_name: object_name.clone(),
            payload_size,
            visibility,
            content_type: content_type.clone(),
            primary_sp_approval: Some(proto_approval),
            expect_checksums: checksums_proto,
            redundancy_type: 0, // REDUNDANCY_EC_TYPE is 0
        };

        let eip_msg = Eip712MsgValue {
            type_url: "/greenfield.storage.MsgCreateObject".to_string(),
            bucket_name: bucket_name.to_string(),
            content_type,
            creator: self.get_checksummed_address(),
            expect_checksums: checksums_eip,
            object_name,
            payload_size: payload_size.to_string(),
            primary_sp_approval: eip_approval,
            redundancy_type: crate::eip712::RedundancyType::EcType,
            visibility: visibility_enum,
        };

        // Fetch Account Info
        let acc_info = self.get_account_info().await?;
        println!(
            "DEBUG: Fetched Account Info - Number: {}, Sequence: {}",
            acc_info.account_number, acc_info.sequence
        );

        // Sign
        let tx_raw = create_signed_tx(
            &self.wallet,
            eip_msg,
            proto_msg,
            &self.chain_id,
            5000000000000000, // Fee 0.005 BNB
            200000,           // Gas
            acc_info.account_number,
            acc_info.sequence,
        )
        .await?;

        let mut tx_bytes = Vec::new();
        tx_raw.encode(&mut tx_bytes)?;
        let tx_bytes_base64 = base64::engine::general_purpose::STANDARD.encode(tx_bytes);

        // Broadcast
        let url = format!("{}/cosmos/tx/v1beta1/txs", self.rpc_url);
        let req_body = BroadcastReq {
            tx_bytes: tx_bytes_base64,
            mode: "BROADCAST_MODE_SYNC".to_string(),
        };

        let resp = self.http_client.post(&url).json(&req_body).send().await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(format!("Broadcast failed with status {}: {}", status, text).into());
        }

        let resp_data: BroadcastResponse = serde_json::from_str(&text)?;
        if resp_data.tx_response.code != 0 {
            return Err(format!(
                "Tx Failed (Code {}): {}",
                resp_data.tx_response.code, resp_data.tx_response.raw_log
            )
            .into());
        }

        Ok(text)
    }

    pub async fn put_object(
        &self,
        sp_url: &str,
        bucket: String,
        object: String,
        file_path: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use chrono::{Duration, Utc};
        use ethers::utils::keccak256;
        use md5::{Digest, Md5};
        use std::fs;

        // 1. Read file
        let file_content = fs::read(&file_path)?;
        let content_type = "application/octet-stream";

        // 2. Calculate Content-MD5 (base64-encoded MD5)
        let mut hasher = Md5::new();
        hasher.update(&file_content);
        let md5_result = hasher.finalize();
        let content_md5 = base64::engine::general_purpose::STANDARD.encode(md5_result);

        // 3. Expiry Timestamp (RFC3339, 1 hour from now)
        let expiry = (Utc::now() + Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        // 4. URL components
        let url_path = format!("/{}/{}", bucket, object);
        let raw_query = ""; // No query params for PUT

        // 5. Parse SP host from URL
        let sp_host = sp_url.replace("https://", "").replace("http://", "");

        // 6. Build Canonical Headers (sorted alphabetically, lowercase)
        // Based on Go code: headers are "header:value\n", then host value alone at end
        // Sorted order: content-md5, content-type, x-gnfd-expiry-timestamp (sorted)
        // Then host value at the end (no "host:" prefix per Go code lines 50-53)
        let canonical_headers = format!(
            "content-md5:{}\ncontent-type:{}\nx-gnfd-expiry-timestamp:{}\n{}\n",
            content_md5, content_type, expiry, sp_host
        );

        // 7. Build Signed Headers (semicolon-separated, sorted)
        // These should match the headers used above (excluding host which is added separately)
        let signed_headers = "content-md5;content-type;x-gnfd-expiry-timestamp";

        // 8. Build Canonical Request (AWS S3 style)
        // Format: Method\nPath\nQuery\nCanonicalHeaders\nSignedHeaders
        let canonical_request = format!(
            "{}\n{}\n{}\n{}{}",
            "PUT", url_path, raw_query, canonical_headers, signed_headers
        );

        println!("Debug: Canonical Request:\n{}", canonical_request);

        // 9. Hash with Keccak-256
        let hash = keccak256(canonical_request.as_bytes());

        // 10. Sign with wallet
        let signature = self.wallet.sign_hash(hash.into())?;
        // The signature is [R || S || V] where V is 27 or 28
        // Go's secp256k1.RecoverPubkey expects V to be 0 or 1
        let mut sig_bytes = signature.to_vec();
        sig_bytes[64] = sig_bytes[64] - 27; // Adjust V from 27/28 to 0/1
        let sig_hex = hex::encode(&sig_bytes);

        // 11. Build Authorization header (GNFD1-ECDSA format)
        let auth_header = format!("GNFD1-ECDSA,Signature={}", sig_hex);

        println!("Debug: Authorization: {}", auth_header);

        // 12. Send PUT request to SP
        let url = format!("{}{}", sp_url, url_path);

        let resp = self
            .http_client
            .put(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", content_type)
            .header("Content-MD5", &content_md5)
            .header("X-Gnfd-Expiry-Timestamp", &expiry)
            .header("Host", &sp_host)
            .body(file_content)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(format!("PUT failed with status {}: {}", status, text).into());
        }

        Ok(text)
    }

    /// Transfer BNB from BSC to Greenfield via TokenHub bridge contract
    ///
    /// # Arguments
    /// * `bsc_rpc` - BSC RPC endpoint URL
    /// * `amount_bnb` - Amount of BNB to transfer (as string, e.g., "0.1")
    /// * `mainnet` - If true, use mainnet TokenHub; otherwise use testnet
    pub async fn transfer_out(
        &self,
        bsc_rpc: &str,
        amount_bnb: &str,
        mainnet: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use ethers::prelude::*;
        use ethers::utils::parse_ether;
        use std::sync::Arc;

        // TokenHub contract addresses
        const TOKEN_HUB_TESTNET: &str = "0xED8e5C546F84442219A5a987EE1D820698528E04";
        const TOKEN_HUB_MAINNET: &str = "0xeA97dF87E6c7F68C9f95A69dA79E19B834823F25";

        let token_hub_address: Address = if mainnet {
            TOKEN_HUB_MAINNET.parse()?
        } else {
            TOKEN_HUB_TESTNET.parse()?
        };

        // Parse amount to wei
        let amount = parse_ether(amount_bnb)?;

        // The receiver on Greenfield is the same address as the sender
        let receiver = self.wallet.address();

        println!("🌉 Bridge Transfer:");
        println!(
            "   From: BSC {} -> To: Greenfield",
            if mainnet { "Mainnet" } else { "Testnet" }
        );
        println!("   Amount: {} BNB", amount_bnb);
        println!("   Receiver: {:?}", receiver);
        println!("   TokenHub: {:?}", token_hub_address);

        // Connect to BSC with the wallet
        let bsc_chain_id = if mainnet { 56u64 } else { 97u64 };
        let provider = Provider::<Http>::try_from(bsc_rpc)?;
        let wallet_with_bsc = self.wallet.clone().with_chain_id(bsc_chain_id);
        let client = SignerMiddleware::new(provider, wallet_with_bsc);
        let client = Arc::new(client);

        // TokenHub ABI - only transferOut function
        // function transferOut(address recipient, uint256 amount) external payable returns (bool)
        abigen!(
            TokenHub,
            r#"[
                function transferOut(address recipient, uint256 amount) external payable returns (bool)
            ]"#
        );

        let contract = TokenHub::new(token_hub_address, client.clone());

        // Calculate the relayer fee (usually 0.002 BNB for testnet)
        let relayer_fee = parse_ether("0.002")?;
        let total_value = amount + relayer_fee;

        println!("   Relayer Fee: 0.002 BNB");
        println!("   Total TX Value: {} wei", total_value);

        // Call transferOut with the amount as value (payable function)
        let tx = contract.transfer_out(receiver, amount).value(total_value);

        println!("\n📤 Sending transaction...");

        let pending_tx = tx.send().await?;
        let tx_hash = pending_tx.tx_hash();

        println!("   TX Hash: {:?}", tx_hash);
        println!("   Waiting for confirmation...");

        let receipt = pending_tx.await?;

        match receipt {
            Some(r) => {
                let explorer = if mainnet {
                    "https://bscscan.com/tx"
                } else {
                    "https://testnet.bscscan.com/tx"
                };
                println!("✅ Transaction confirmed in block {:?}", r.block_number);
                println!("   View: {}/{:?}", explorer, tx_hash);
                Ok(format!("{:?}", tx_hash))
            }
            None => Err("Transaction failed - no receipt".into()),
        }
    }

    /// Combined operation: Create object on-chain and upload to SP
    pub async fn upload(
        &self,
        sp_url: &str,
        bucket: String,
        object: String,
        file_path: String,
        visibility: i32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // 1. Create Object Metadata on-chain
        println!("📝 Creating object metadata on-chain...");
        let file_metadata = std::fs::metadata(&file_path)?;
        let size = file_metadata.len();
        let content_type = "application/octet-stream".to_string();

        let create_res = self
            .create_object(
                bucket.clone(),
                object.clone(),
                size,
                content_type,
                visibility,
            )
            .await?;
        println!("✅ On-chain metadata created: {}", create_res);

        // 2. Upload file to Storage Provider
        println!("📤 Uploading file to Storage Provider...");
        let put_res = self.put_object(sp_url, bucket, object, file_path).await?;

        Ok(put_res)
    }

    pub fn get_bech32_address(&self) -> String {
        use bech32::{self, ToBase32, Variant};
        let addr = self.wallet.address();
        let bytes = addr.as_bytes();
        bech32::encode("gnfd", bytes.to_base32(), Variant::Bech32).unwrap_or_else(|_| String::new())
    }

    /// Get the checksummed Ethereum-style hex address (e.g., 0xD486D5ed56bF...)
    /// This is the format used by the official Greenfield SDK for EIP-712 signing
    pub fn get_checksummed_address(&self) -> String {
        use tiny_keccak::{Hasher, Keccak};
        let addr = self.wallet.address();
        let addr_hex = hex::encode(addr.as_bytes());

        // EIP-55 checksum
        let mut hasher = Keccak::v256();
        hasher.update(addr_hex.as_bytes());
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);

        let mut checksummed = String::from("0x");
        for (i, c) in addr_hex.chars().enumerate() {
            if c.is_numeric() {
                checksummed.push(c);
            } else {
                // Check if the corresponding nibble in hash is >= 8
                let byte_idx = i / 2;
                let is_high_nibble = i % 2 == 0;
                let nibble = if is_high_nibble {
                    hash[byte_idx] >> 4
                } else {
                    hash[byte_idx] & 0x0f
                };
                if nibble >= 8 {
                    checksummed.push(c.to_ascii_uppercase());
                } else {
                    checksummed.push(c.to_ascii_lowercase());
                }
            }
        }
        checksummed
    }
}
