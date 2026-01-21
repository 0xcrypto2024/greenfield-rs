use crate::bucket_eip712::{MsgCreateBucket as Eip712MsgCreateBucket, PrimarySpApproval as Eip712BucketApproval, TxCreateBucket};
use crate::eip712::{MsgValue as Eip712MsgValue, PrimarySpApproval as Eip712Approval, Visibility};
use crate::proto::greenfield::common::Approval as ProtoApproval;
use crate::proto::greenfield::storage::MsgCreateBucket as ProtoMsgCreateBucket;
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

    /// Create object metadata on-chain with file checksums
    /// This is the full implementation matching Go SDK's CreateObject
    pub async fn create_object_with_file(
        &self,
        bucket_name: String,
        object_name: String,
        file_path: &str,
        content_type: String,
        visibility: i32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // 1. Compute integrity hashes from file (like Go SDK's ComputeHashRoots)
        println!("DEBUG: Computing integrity hashes for file: {}", file_path);
        let (checksums, file_size) = crate::hash::compute_hash_from_file_default(file_path)?;
        println!("DEBUG: Computed {} checksums, file size: {} bytes", checksums.len(), file_size);
        
        // Create object with computed checksums
        self.create_object_internal(
            bucket_name,
            object_name,
            file_size,
            content_type,
            visibility,
            checksums,
        ).await
    }

    /// Create object with pre-computed checksums (for advanced use)
    pub async fn create_object(
        &self,
        bucket_name: String,
        object_name: String,
        payload_size: u64,
        content_type: String,
        visibility: i32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // For backward compatibility - compute checksums if file size is small enough
        // Otherwise, caller should use create_object_with_file
        
        // Create placeholder checksums (7 empty hashes) - this will fail on-chain validation
        // but allows testing EIP-712 signing
        println!("WARNING: create_object called without checksums - using empty checksums (will fail on chain)");
        let empty_checksums: Vec<Vec<u8>> = vec![];
        
        self.create_object_internal(
            bucket_name,
            object_name,
            payload_size,
            content_type,
            visibility,
            empty_checksums,
        ).await
    }

    /// Internal implementation of create_object
    async fn create_object_internal(
        &self,
        bucket_name: String,
        object_name: String,
        payload_size: u64,
        content_type: String,
        visibility: i32,
        checksums: Vec<Vec<u8>>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let _address = self.wallet.address();

        // Note: For MsgCreateObject, GlobalVirtualGroupFamilyId should be 0 (not from bucket)
        // Go SDK's NewMsgCreateObject only sets ExpiredHeight and Sig in PrimarySpApproval
        // The GlobalVirtualGroupFamilyId defaults to 0 (Go struct zero value)
        println!("DEBUG: Creating object '{}' in bucket '{}'...", object_name, bucket_name);

        // Define visibility enum
        let visibility_enum = match visibility {
            1 => crate::eip712::Visibility::Public,
            2 => crate::eip712::Visibility::Private,
            3 => crate::eip712::Visibility::Inherit,
            _ => crate::eip712::Visibility::Public,
        };

        // IMPORTANT: For MsgCreateObject, PrimarySpApproval.GlobalVirtualGroupFamilyId = 0
        // This is different from MsgCreateBucket which needs the actual VGF ID
        // See Go SDK: storageTypes.NewMsgCreateObject only sets ExpiredHeight & Sig
        let proto_approval = ProtoApproval {
            expired_height: u64::MAX,  // math.MaxUint in Go
            global_virtual_group_family_id: 0,  // Must be 0 for CreateObject!
            sig: vec![],
        };
        let eip_approval = Eip712Approval {
            expired_height: u64::MAX.to_string(),
            global_virtual_group_family_id: "0".to_string(),  // Must be 0 for CreateObject!
        };

        // Convert checksums for EIP-712 (bytes[] -> hex strings for JSON)
        let checksums_eip: Vec<String> = checksums.iter()
            .map(|c| format!("0x{}", hex::encode(c)))
            .collect();

        // IMPORTANT: Both Proto and EIP-712 must use the SAME address format!
        // Go SDK uses AccAddress.String() which returns EIP-55 checksummed address
        let checksummed_addr = self.get_checksummed_address();

        let proto_msg = ProtoMsgCreateObject {
            creator: checksummed_addr.clone(),
            bucket_name: bucket_name.clone(),
            object_name: object_name.clone(),
            payload_size,
            visibility,
            content_type: content_type.clone(),
            primary_sp_approval: Some(proto_approval),
            expect_checksums: checksums.clone(),  // Vec<Vec<u8>>
            redundancy_type: 0, // REDUNDANCY_EC_TYPE is 0
        };

        let eip_msg = Eip712MsgValue {
            type_url: "/greenfield.storage.MsgCreateObject".to_string(),
            bucket_name: bucket_name.to_string(),
            content_type,
            // IMPORTANT: Use EIP-55 checksummed address for EIP-712 signing (Go SDK's AccAddress.String())
            creator: self.get_checksummed_address(),
            expect_checksums: checksums_eip,  // Vec<String> with 0x prefix
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

        // DEBUG: Print Proto message details for comparison with Go SDK
        println!("DEBUG: Proto MsgCreateObject:");
        println!("  creator: {}", proto_msg.creator);
        println!("  bucket_name: {}", proto_msg.bucket_name);
        println!("  object_name: {}", proto_msg.object_name);
        println!("  payload_size: {}", proto_msg.payload_size);
        println!("  visibility: {}", proto_msg.visibility);
        println!("  content_type: {}", proto_msg.content_type);
        println!("  expect_checksums len: {}", proto_msg.expect_checksums.len());
        println!("  redundancy_type: {}", proto_msg.redundancy_type);
        if let Some(ref approval) = proto_msg.primary_sp_approval {
            println!("  primary_sp_approval.expired_height: {}", approval.expired_height);
            println!("  primary_sp_approval.gvg_family_id: {}", approval.global_virtual_group_family_id);
            println!("  primary_sp_approval.sig len: {}", approval.sig.len());
        }

        // Sign - Use the same signing logic as sign_create_bucket_tx
        let tx_raw = self.sign_create_object_tx(
            eip_msg,
            proto_msg,
            acc_info.account_number,
            acc_info.sequence,
        ).await?;

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

    /// Create a new bucket on Greenfield
    pub async fn create_bucket(
        &self,
        bucket_name: String,
        primary_sp_address: String,
        visibility: i32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let checksummed_addr = self.get_checksummed_address();
        
        println!("🪣 Creating bucket '{}'...", bucket_name);
        println!("   Creator: {}", checksummed_addr);
        println!("   Primary SP: {}", primary_sp_address);
        println!("   Visibility: {}", visibility);
        
        // Step 1: Get SP info to find the SP ID
        println!("   Fetching SP info...");
        let sps = crate::sp::list_storage_providers(&self.rpc_url).await?;
        let sp = sps.iter()
            .find(|s| s.operator_address.to_lowercase() == primary_sp_address.to_lowercase())
            .ok_or_else(|| format!("SP not found: {}", primary_sp_address))?;
        
        println!("   SP ID: {}, Endpoint: {}", sp.id, sp.endpoint);
        
        // Step 2: Get VGF ID for this SP from chain
        let vgf_id = crate::sp::get_vgf_id_for_sp(&self.rpc_url, sp.id).await?;
        
        // Get account info
        let acc_info = self.get_account_info().await?;
        println!("   Account Number: {}, Sequence: {}", acc_info.account_number, acc_info.sequence);
        
        // Create Proto message with correct VGF ID
        let proto_msg = ProtoMsgCreateBucket {
            creator: checksummed_addr.clone(),
            bucket_name: bucket_name.clone(),
            visibility,
            payment_address: "".to_string(),  // Use creator as default
            primary_sp_address: primary_sp_address.clone(),
            primary_sp_approval: Some(ProtoApproval {
                expired_height: 0,  // 0 means no expiration
                global_virtual_group_family_id: vgf_id,  // Use the VGF ID from SP
                sig: vec![],
            }),
            charged_read_quota: 0,
        };
        
        // Create EIP-712 message
        let visibility_enum = match visibility {
            1 => Visibility::Public,
            2 => Visibility::Private,
            3 => Visibility::Inherit,
            _ => Visibility::Public,
        };
        
        let eip_msg = Eip712MsgCreateBucket {
            type_url: "/greenfield.storage.MsgCreateBucket".to_string(),
            bucket_name: bucket_name.clone(),
            charged_read_quota: "0".to_string(),
            creator: checksummed_addr.clone(),
            payment_address: "".to_string(),
            primary_sp_address: primary_sp_address.clone(),
            primary_sp_approval: Eip712BucketApproval {
                expired_height: "0".to_string(),
                global_virtual_group_family_id: vgf_id,  // Use the VGF ID from SP
            },
            visibility: visibility_enum,
        };
        
        // Sign the transaction
        let tx_raw = self.sign_create_bucket_tx(
            eip_msg,
            proto_msg,
            acc_info.account_number,
            acc_info.sequence,
        ).await?;
        
        // Broadcast
        let mut tx_bytes = Vec::new();
        tx_raw.encode(&mut tx_bytes)?;
        let tx_bytes_base64 = base64::engine::general_purpose::STANDARD.encode(tx_bytes);
        
        let url = format!("{}/cosmos/tx/v1beta1/txs", self.rpc_url);
        let req_body = BroadcastReq {
            tx_bytes: tx_bytes_base64,
            mode: "BROADCAST_MODE_SYNC".to_string(),
        };
        
        println!("📡 Broadcasting transaction...");
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
            ).into());
        }
        
        println!("✅ Bucket created successfully!");
        println!("   TxHash: {}", resp_data.tx_response.txhash);
        
        Ok(resp_data.tx_response.txhash)
    }
    
    /// Internal method to sign CreateBucket transaction
    async fn sign_create_bucket_tx(
        &self,
        eip_msg: Eip712MsgCreateBucket,
        proto_msg: ProtoMsgCreateBucket,
        account_number: u64,
        sequence: u64,
    ) -> Result<crate::proto::cosmos::tx::v1beta1::TxRaw, Box<dyn std::error::Error>> {
        use crate::eip712::Fee;
        use crate::proto::cosmos::base::v1beta1::Coin;
        use crate::proto::cosmos::tx::v1beta1::{AuthInfo, Fee as ProtoFee, ModeInfo, SignerInfo, TxBody, TxRaw};
        use crate::proto::ethermint::crypto::v1::ethsecp256k1::PubKey as EthPubKey;
        use prost::Message;
        use prost_types::Any;
        
        // Parse chain_id to get numeric part (e.g., "greenfield_5600-1" -> 5600)
        let chain_id_num: u64 = if self.chain_id.contains('_') {
            self.chain_id
                .split('_')
                .nth(1)
                .and_then(|s| s.split('-').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(5600)
        } else {
            self.chain_id.parse().unwrap_or(5600)
        };
        
        // Fee (using same as Go SDK)
        let fee_amount: u128 = 12000000000000;  // 0.012 BNB
        let gas_limit: u64 = 2400;
        
        // Build EIP-712 Tx template
        let eip_tx = TxCreateBucket {
            account_number: account_number.to_string(),
            chain_id: chain_id_num.to_string(),
            fee: Fee {
                amount: vec![crate::eip712::Coin {
                    denom: "BNB".to_string(),
                    amount: fee_amount.to_string(),
                }],
                gas_limit: gas_limit.to_string(),
                granter: "".to_string(),
                payer: self.get_checksummed_address(),
            },
            memo: "".to_string(),
            msg1: eip_msg,
            sequence: sequence.to_string(),
            timeout_height: "0".to_string(),
        };
        
        // Print EIP-712 JSON for debugging
        println!("\n📋 EIP-712 JSON Payload:");
        println!("{}", serde_json::to_string_pretty(&eip_tx)?);
        
        // Calculate EIP-712 hash
        println!("\n🔐 Calculating EIP-712 Hash...");
        let eip712_hash = eip_tx.get_eip712_hash(&chain_id_num.to_string())?;
        println!("\n🔍 Final EIP-712 Hash: 0x{}", hex::encode(eip712_hash.as_bytes()));
        
        // Sign
        let signature = self.wallet.sign_hash(eip712_hash)?;
        let sig_bytes = signature.to_vec(); // 65 bytes: R || S || V
        
        println!("\n📝 Signature: 0x{}", hex::encode(&sig_bytes));
        
        // Build Proto TxBody
        let mut msg_bytes = Vec::new();
        proto_msg.encode(&mut msg_bytes)?;
        
        let tx_body = TxBody {
            messages: vec![Any {
                type_url: "/greenfield.storage.MsgCreateBucket".to_string(),
                value: msg_bytes,
            }],
            memo: "".to_string(),
            timeout_height: 0,
            extension_options: vec![],
            non_critical_extension_options: vec![],
            timeout_timestamp: None,
            unordered: false,
        };
        
        // Build AuthInfo
        let pubkey_bytes = self.wallet.signer().verifying_key().to_encoded_point(true);
        let eth_pubkey = EthPubKey {
            key: pubkey_bytes.as_bytes().to_vec(),
        };
        let mut pk_bytes = Vec::new();
        eth_pubkey.encode(&mut pk_bytes)?;
        
        let auth_info = AuthInfo {
            signer_infos: vec![SignerInfo {
                public_key: Some(Any {
                    type_url: "/cosmos.crypto.eth.ethsecp256k1.PubKey".to_string(),
                    value: pk_bytes,
                }),
                mode_info: Some(ModeInfo {
                    sum: Some(crate::proto::cosmos::tx::v1beta1::mode_info::Sum::Single(
                        crate::proto::cosmos::tx::v1beta1::mode_info::Single {
                            mode: 712, // EIP-712 sign mode
                        },
                    )),
                }),
                sequence,
            }],
            fee: Some(ProtoFee {
                amount: vec![Coin {
                    denom: "BNB".to_string(),
                    amount: fee_amount.to_string(),
                }],
                gas_limit,
                payer: "".to_string(),
                granter: "".to_string(),
            }),
            tip: None,
        };
        
        // Encode and return TxRaw
        let mut body_bytes = Vec::new();
        tx_body.encode(&mut body_bytes)?;
        let mut auth_info_bytes = Vec::new();
        auth_info.encode(&mut auth_info_bytes)?;
        
        println!("DEBUG [CreateBucket]: TxBody length: {} bytes", body_bytes.len());
        println!("DEBUG [CreateBucket]: TxBody bytes (full): 0x{}", hex::encode(&body_bytes));
        println!("DEBUG [CreateBucket]: AuthInfo bytes: 0x{}", hex::encode(&auth_info_bytes));
        
        Ok(TxRaw {
            body_bytes,
            auth_info_bytes,
            signatures: vec![sig_bytes],
        })
    }

    /// Sign CreateObject transaction - same structure as sign_create_bucket_tx
    async fn sign_create_object_tx(
        &self,
        eip_msg: Eip712MsgValue,
        proto_msg: ProtoMsgCreateObject,
        account_number: u64,
        sequence: u64,
    ) -> Result<crate::proto::cosmos::tx::v1beta1::TxRaw, Box<dyn std::error::Error>> {
        use crate::eip712::{Fee as Eip712Fee, Tx as Eip712Tx};
        use crate::proto::cosmos::base::v1beta1::Coin;
        use crate::proto::cosmos::tx::v1beta1::{AuthInfo, Fee as ProtoFee, ModeInfo, SignerInfo, TxBody, TxRaw};
        use crate::proto::ethermint::crypto::v1::ethsecp256k1::PubKey as EthPubKey;
        use prost::Message;
        use prost_types::Any;
        
        // Parse chain_id to get numeric part (e.g., "greenfield_5600-1" -> 5600)
        let chain_id_num: u64 = if self.chain_id.contains('_') {
            self.chain_id
                .split('_')
                .nth(1)
                .and_then(|s| s.split('-').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(5600)
        } else {
            self.chain_id.parse().unwrap_or(5600)
        };
        
        // Fee (using same as Go SDK for CreateObject)
        let fee_amount: u128 = 6000000000000;  // 0.006 BNB
        let gas_limit: u64 = 1200;
        
        // Build EIP-712 Tx template - same structure as sign_create_bucket_tx
        let eip_tx = Eip712Tx {
            account_number: account_number.to_string(),
            chain_id: chain_id_num.to_string(),
            fee: Eip712Fee {
                amount: vec![crate::eip712::Coin {
                    denom: "BNB".to_string(),
                    amount: fee_amount.to_string(),
                }],
                gas_limit: gas_limit.to_string(),
                granter: "".to_string(),
                payer: self.get_checksummed_address(),
            },
            memo: "".to_string(),
            msg1: eip_msg,
            sequence: sequence.to_string(),
            timeout_height: "0".to_string(),
        };
        
        // Print EIP-712 JSON for debugging
        println!("\n📋 EIP-712 JSON Payload (sign_create_object_tx):");
        println!("{}", serde_json::to_string_pretty(&eip_tx)?);
        
        // Calculate EIP-712 hash
        println!("\n🔐 Calculating EIP-712 Hash (sign_create_object_tx)...");
        let eip712_hash = eip_tx.get_eip712_hash(&chain_id_num.to_string())?;
        println!("\n🔍 Final EIP-712 Hash: 0x{}", hex::encode(eip712_hash.as_bytes()));
        
        // Sign
        let signature = self.wallet.sign_hash(eip712_hash)?;
        let sig_bytes = signature.to_vec(); // 65 bytes: R || S || V
        
        println!("\n📝 Signature: 0x{}", hex::encode(&sig_bytes));
        
        // Verify signature locally
        use ethers::core::types::Signature as EthSignature;
        let r = ethers::core::types::U256::from_big_endian(&sig_bytes[0..32]);
        let s = ethers::core::types::U256::from_big_endian(&sig_bytes[32..64]);
        let v = sig_bytes[64] as u64;
        let eth_sig = EthSignature { r, s, v };
        match eth_sig.recover(eip712_hash) {
            Ok(recovered_addr) => {
                println!("DEBUG: Recovered Address: {:?}", recovered_addr);
                if recovered_addr == self.wallet.address() {
                    println!("DEBUG: ✅ Signature verification PASSED locally!");
                } else {
                    println!("DEBUG: ❌ Signature verification FAILED locally!");
                }
            }
            Err(e) => {
                println!("DEBUG: ❌ Failed to recover address: {:?}", e);
            }
        }
        
        // Build Proto TxBody
        let mut msg_bytes = Vec::new();
        proto_msg.encode(&mut msg_bytes)?;
        
        let tx_body = TxBody {
            messages: vec![Any {
                type_url: "/greenfield.storage.MsgCreateObject".to_string(),
                value: msg_bytes,
            }],
            memo: "".to_string(),
            timeout_height: 0,
            extension_options: vec![],
            non_critical_extension_options: vec![],
            timeout_timestamp: None,
            unordered: false,
        };
        
        // Build AuthInfo
        let pubkey_bytes = self.wallet.signer().verifying_key().to_encoded_point(true);
        let eth_pubkey = EthPubKey {
            key: pubkey_bytes.as_bytes().to_vec(),
        };
        let mut pk_bytes = Vec::new();
        eth_pubkey.encode(&mut pk_bytes)?;
        
        let auth_info = AuthInfo {
            signer_infos: vec![SignerInfo {
                public_key: Some(Any {
                    type_url: "/cosmos.crypto.eth.ethsecp256k1.PubKey".to_string(),
                    value: pk_bytes,
                }),
                mode_info: Some(ModeInfo {
                    sum: Some(crate::proto::cosmos::tx::v1beta1::mode_info::Sum::Single(
                        crate::proto::cosmos::tx::v1beta1::mode_info::Single {
                            mode: 712, // EIP-712 sign mode
                        },
                    )),
                }),
                sequence,
            }],
            fee: Some(ProtoFee {
                amount: vec![Coin {
                    denom: "BNB".to_string(),
                    amount: fee_amount.to_string(),
                }],
                gas_limit,
                payer: "".to_string(),
                granter: "".to_string(),
            }),
            tip: None,
        };
        
        // Encode and return TxRaw
        let mut body_bytes = Vec::new();
        tx_body.encode(&mut body_bytes)?;
        let mut auth_info_bytes = Vec::new();
        auth_info.encode(&mut auth_info_bytes)?;
        
        println!("DEBUG: TxBody length: {} bytes", body_bytes.len());
        println!("DEBUG: TxBody bytes (full): 0x{}", hex::encode(&body_bytes));
        println!("DEBUG: AuthInfo bytes: 0x{}", hex::encode(&auth_info_bytes));
        
        Ok(TxRaw {
            body_bytes,
            auth_info_bytes,
            signatures: vec![sig_bytes],
        })
    }

    /// Upload object data to Storage Provider
    /// This implements GNFD1-ECDSA authentication matching Go SDK exactly
    pub async fn put_object(
        &self,
        sp_url: &str,
        bucket: String,
        object: String,
        file_path: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use chrono::{Duration, Utc};
        use ethers::utils::keccak256;
        use std::fs;
        use tokio::time::sleep;
        use std::time::Duration as StdDuration;

        // 0. Wait for SP to sync object info (like Go SDK's headSPObjectInfo)
        // Retry up to 4 times with exponential backoff (500ms, 1s, 2s, 4s)
        println!("   Waiting for SP to sync object info...");
        let mut backoff_ms = 500u64;
        for retry in 0..4 {
            match self.get_object_status_from_sp(sp_url, &bucket, &object).await {
                Ok(_) => {
                    println!("   ✓ SP has synced object info");
                    break;
                }
                Err(e) => {
                    let err_str = e.to_string().to_lowercase();
                    // If it's not "no such object" error, SP knows about it, continue
                    if !err_str.contains("no such object") && !err_str.contains("not been created") {
                        println!("   ✓ SP responded (object exists)");
                        break;
                    }
                    
                    if retry < 3 {
                        println!("   Retry {}/4 (waiting {}ms): {}", retry + 1, backoff_ms, e);
                        sleep(StdDuration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;  // Exponential backoff
                    } else {
                        return Err(format!("SP failed to sync object info after 4 retries: {}", e).into());
                    }
                }
            }
        }

        // 1. Read file
        let file_content = fs::read(&file_path)?;
        let content_type = "application/octet-stream";
        let content_length = file_content.len();

        // 2. Expiry Timestamp (ISO 8601 format: 2021-09-30T16:25:24Z)
        let expiry = (Utc::now() + Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        // 3. URL components
        let url_path = format!("/{}/{}", bucket, object);
        
        // 4. Parse SP host from URL (remove trailing slash if present)
        let sp_host = sp_url
            .replace("https://", "")
            .replace("http://", "")
            .trim_end_matches('/')
            .to_string();

        // 5. Build Canonical Request matching Go SDK's GetCanonicalRequest exactly:
        // Method\nEncodedPath\nRawQuery\nCanonicalHeaders\nSignedHeaders
        //
        // Go SDK supportHeads (used headers are filtered from this list):
        //   Content-Type, X-Gnfd-Txn-Hash, X-Gnfd-Object-ID, X-Gnfd-Redundancy-Index, 
        //   X-Gnfd-Resource, X-Gnfd-Date, Range, X-Gnfd-Piece-Index, Content-MD5, 
        //   X-Gnfd-Unsigned-Msg, X-Gnfd-User-Address, X-Gnfd-Expiry-Timestamp, X-Gnfd-Content-Sha256
        //
        // For PutObject, Go SDK uses: Content-Type, X-Gnfd-Expiry-Timestamp, X-Gnfd-Content-Sha256
        // Headers must be sorted alphabetically (lowercase)
        
        // EmptyStringSHA256 - Go SDK uses this for PUT requests
        const EMPTY_STRING_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        
        // Canonical headers: "header:value\n" for each, then host at the end
        // Sort order: content-type, x-gnfd-content-sha256, x-gnfd-expiry-timestamp (alphabetically sorted lowercase)
        let canonical_headers = format!(
            "content-type:{}\nx-gnfd-content-sha256:{}\nx-gnfd-expiry-timestamp:{}\n{}\n",
            content_type, EMPTY_STRING_SHA256, expiry, sp_host
        );

        // Signed headers (semicolon-separated, sorted)
        let signed_headers = "content-type;x-gnfd-content-sha256;x-gnfd-expiry-timestamp";

        // Build Canonical Request
        // Go SDK uses strings.Join with "\n" separator, and canonical_headers already ends with "\n"
        // So there are TWO "\n" between canonical_headers and signed_headers
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}",
            "PUT",           // Method
            url_path,        // Encoded path (already properly encoded)
            "",              // Raw query (empty for PUT)
            canonical_headers,
            signed_headers
        );

        println!("📋 Canonical Request:\n{}", canonical_request);

        // 6. Hash with Keccak-256 (GNFD1-ECDSA uses raw keccak256, no checksum wrapper)
        let msg_to_sign = keccak256(canonical_request.as_bytes());
        println!("🔑 Message to sign: 0x{}", hex::encode(&msg_to_sign));

        // 7. Sign with wallet
        let signature = self.wallet.sign_hash(msg_to_sign.into())?;
        // Convert V from 27/28 to 0/1 for recovery
        let mut sig_bytes = signature.to_vec();
        if sig_bytes[64] >= 27 {
            sig_bytes[64] -= 27;
        }
        let sig_hex = hex::encode(&sig_bytes);

        // 8. Build Authorization header (GNFD1-ECDSA format: "GNFD1-ECDSA, Signature=<hex>")
        let auth_header = format!("GNFD1-ECDSA, Signature={}", sig_hex);

        println!("🔐 Authorization: {}", auth_header);

        // 9. Send PUT request to SP
        let url = format!("{}{}", sp_url, url_path);
        println!("📤 PUT URL: {}", url);

        let resp = self
            .http_client
            .put(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", content_type)
            .header("Content-Length", content_length.to_string())
            .header("X-Gnfd-Content-Sha256", EMPTY_STRING_SHA256)
            .header("X-Gnfd-Expiry-Timestamp", &expiry)
            .body(file_content)
            .send()
            .await?;
            
        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(format!("PUT failed with status {}: {}", status, text).into());
        }

        println!("✅ Upload response: {}", if text.is_empty() { "(empty - success)" } else { &text });
        Ok(text)
    }
    
    /// Query object upload status from SP (like Go SDK's getObjectStatusFromSP)
    /// Uses GET /{bucket}/{object}?upload-progress
    async fn get_object_status_from_sp(
        &self,
        sp_url: &str,
        bucket: &str,
        object: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use chrono::{Duration, Utc};
        use ethers::utils::keccak256;
        
        let sp_host = sp_url
            .replace("https://", "")
            .replace("http://", "")
            .trim_end_matches('/')
            .to_string();
        
        let url_path = format!("/{}/{}", bucket, object);
        let raw_query = "upload-progress=";  // Query param to check upload progress
        let url = format!("{}{}?upload-progress", sp_url, url_path);
        
        // Expiry timestamp
        let expiry = (Utc::now() + Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        
        // Build canonical request for GET with query param
        let canonical_headers = format!(
            "x-gnfd-expiry-timestamp:{}\n{}\n",
            expiry, sp_host
        );
        let signed_headers = "x-gnfd-expiry-timestamp";
        
        let canonical_request = format!(
            "{}\n{}\n{}\n{}{}",
            "GET", url_path, raw_query, canonical_headers, signed_headers
        );
        
        // Sign
        let msg_to_sign = keccak256(canonical_request.as_bytes());
        let signature = self.wallet.sign_hash(msg_to_sign.into())?;
        let mut sig_bytes = signature.to_vec();
        if sig_bytes[64] >= 27 {
            sig_bytes[64] -= 27;
        }
        let auth_header = format!("GNFD1-ECDSA, Signature={}", hex::encode(&sig_bytes));
        
        // Send GET request
        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", &auth_header)
            .header("X-Gnfd-Expiry-Timestamp", &expiry)
            .send()
            .await?;
        
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            // Check if it's "no such object" error - object not synced yet
            if text.to_lowercase().contains("no such object") || text.contains("45002") {
                Err("object has not been created".into())
            } else {
                Err(format!("GET object status failed ({}): {}", status, text).into())
            }
        }
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

    /// Query object info from chain (HeadObject)
    /// Returns (exists, status) where status is one of:
    /// - "OBJECT_STATUS_CREATED" - Object metadata created, ready for upload
    /// - "OBJECT_STATUS_SEALED" - Object fully uploaded and sealed
    /// - "" - Object does not exist
    pub async fn head_object(
        &self,
        bucket: &str,
        object: &str,
    ) -> Result<(bool, String), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/greenfield/storage/head_object/{}/{}",
            self.rpc_url, bucket, object
        );
        
        let resp = self.http_client.get(&url).send().await?;
        
        if !resp.status().is_success() {
            // Object doesn't exist
            return Ok((false, String::new()));
        }
        
        #[derive(serde::Deserialize)]
        struct ObjectResp {
            object_info: Option<ObjectInfoJson>,
        }
        #[derive(serde::Deserialize)]
        struct ObjectInfoJson {
            object_status: Option<String>,
        }
        
        let data: ObjectResp = resp.json().await?;
        
        if let Some(info) = data.object_info {
            let status = info.object_status.unwrap_or_default();
            Ok((true, status))
        } else {
            Ok((false, String::new()))
        }
    }

    /// Combined operation: Create object on-chain and upload to SP
    /// This computes integrity hashes, creates object metadata on-chain, and uploads to SP
    /// 
    /// **Behavior matching Go SDK:**
    /// - If object doesn't exist: CreateObject → PutObject
    /// - If object exists with status OBJECT_STATUS_CREATED: Skip CreateObject → PutObject  
    /// - If object exists with status OBJECT_STATUS_SEALED: Return error (already uploaded)
    /// 
    /// If sp_url is empty, it will be automatically fetched from the bucket's primary SP
    pub async fn upload(
        &self,
        sp_url: &str,
        bucket: String,
        object: String,
        file_path: String,
        visibility: i32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // 0. Get SP URL if not provided
        let sp_endpoint = if sp_url.is_empty() {
            println!("🔍 Fetching bucket's primary SP endpoint...");
            let endpoint = crate::bucket::get_bucket_primary_sp(&self.rpc_url, &bucket).await?;
            println!("   SP Endpoint: {}", endpoint);
            endpoint
        } else {
            sp_url.to_string()
        };
        
        // 1. Check if object already exists (like Go SDK behavior)
        println!("\n🔍 Step 1: Checking if object already exists...");
        let (exists, status) = self.head_object(&bucket, &object).await?;
        
        let tx_hash = if exists {
            println!("   Object exists with status: {}", status);
            
            match status.as_str() {
                "OBJECT_STATUS_SEALED" => {
                    return Err(format!(
                        "Object '{}/{}' is already sealed. Cannot re-upload a sealed object.",
                        bucket, object
                    ).into());
                }
                "OBJECT_STATUS_CREATED" => {
                    println!("   ✓ Object metadata already exists, skipping CreateObject");
                    println!("   (Object was previously created but not yet uploaded)");
                    "existing".to_string()
                }
                _ => {
                    println!("   ⚠️  Unexpected status '{}', attempting to continue...", status);
                    "existing".to_string()
                }
            }
        } else {
            // Object doesn't exist, create it
            println!("   Object does not exist, creating metadata on-chain...");
            println!("\n📝 Step 1b: Creating object metadata (computing checksums)...");
            let content_type = "application/octet-stream".to_string();

            let create_res = self
                .create_object_with_file(
                    bucket.clone(),
                    object.clone(),
                    &file_path,
                    content_type,
                    visibility,
                )
                .await?;
            
            let hash = Self::extract_tx_hash(&create_res)?;
            println!("✅ Object metadata created! TxHash: {}", hash);
            hash
        };

        // 2. Upload file to Storage Provider
        println!("\n📤 Step 2: Uploading file to Storage Provider...");
        let put_res = self.put_object(&sp_endpoint, bucket.clone(), object.clone(), file_path).await?;
        println!("✅ File uploaded to SP!");

        // 3. Wait for object to be sealed (optional but recommended)
        println!("\n⏳ Step 3: Waiting for object to be sealed...");
        match self.wait_for_object_seal(&bucket, &object, 60).await {
            Ok(_) => println!("✅ Object sealed successfully!"),
            Err(e) => println!("⚠️  Seal check timed out or failed: {} (object may still be processing)", e),
        }

        Ok(format!("Upload complete! TxHash: {}, SP Response: {}", tx_hash, 
            if put_res.is_empty() { "(empty)" } else { &put_res }))
    }
    
    /// Extract tx_hash from broadcast response
    fn extract_tx_hash(response: &str) -> Result<String, Box<dyn std::error::Error>> {
        #[derive(serde::Deserialize)]
        struct TxResp {
            tx_response: Option<TxResponseInner>,
        }
        #[derive(serde::Deserialize)]
        struct TxResponseInner {
            txhash: Option<String>,
        }
        
        let parsed: TxResp = serde_json::from_str(response)?;
        parsed.tx_response
            .and_then(|r| r.txhash)
            .ok_or_else(|| "No txhash in response".into())
    }
    
    /// Wait for object to be sealed on chain
    pub async fn wait_for_object_seal(
        &self,
        bucket: &str,
        object: &str,
        timeout_secs: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::time::{Duration, Instant};
        use tokio::time::sleep;
        
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        
        loop {
            if start.elapsed() > timeout {
                return Err("Timeout waiting for object seal".into());
            }
            
            // Query object status
            let url = format!(
                "{}/greenfield/storage/head_object/{}/{}",
                self.rpc_url, bucket, object
            );
            
            let resp = self.http_client.get(&url).send().await?;
            
            if resp.status().is_success() {
                #[derive(serde::Deserialize)]
                struct ObjectResp {
                    object_info: Option<ObjectInfoJson>,
                }
                #[derive(serde::Deserialize)]
                struct ObjectInfoJson {
                    object_status: Option<String>,
                }
                
                if let Ok(data) = resp.json::<ObjectResp>().await {
                    if let Some(info) = data.object_info {
                        let status = info.object_status.unwrap_or_default();
                        println!("   Object status: {}", status);
                        
                        if status == "OBJECT_STATUS_SEALED" {
                            return Ok(());
                        }
                    }
                }
            }
            
            // Wait before retry
            sleep(Duration::from_secs(2)).await;
        }
    }

    pub fn get_bech32_address(&self) -> String {
        use bech32::{self, ToBase32, Variant};
        let addr = self.wallet.address();
        let bytes = addr.as_bytes();
        bech32::encode("gnfd", bytes.to_base32(), Variant::Bech32).unwrap_or_else(|_| String::new())
    }

    /// Get the lowercase hex address with 0x prefix (e.g., 0xd486d5ed56bf...)
    /// This is the format expected by Greenfield proto messages
    pub fn get_hex_address(&self) -> String {
        let addr = self.wallet.address();
        format!("0x{}", hex::encode(addr.as_bytes()))
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

    /// Debug EIP-712 calculation without sending transaction
    /// This helps compare with Go SDK output
    pub async fn debug_eip712(
        &self,
        bucket_name: String,
        object_name: String,
        payload_size: u64,
        visibility: i32,
        global_virtual_group_family_id: u32,
        sequence: u64,
        account_number: u64,
        fee_amount: u64,
        gas_limit: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::eip712::{Tx, Fee, Coin, TypeCreateObject, PrimarySpApproval, Visibility, RedundancyType};
        use crate::utils::extract_eip155_chain_id;
        
        let chain_id_num = extract_eip155_chain_id(&self.chain_id)?;
        let checksummed_addr = self.get_checksummed_address();
        
        println!("\n📄 EIP-712 Input Data:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Creator: {}", checksummed_addr);
        println!("  Chain ID: {} (numeric: {})", self.chain_id, chain_id_num);
        println!("  Account Number: {}", account_number);
        println!("  Sequence: {}", sequence);
        println!("  Bucket: {}", bucket_name);
        println!("  Object: {}", object_name);
        println!("  Payload Size: {}", payload_size);
        println!("  Visibility: {} ({})", visibility, if visibility == 2 { "PRIVATE" } else { "INHERIT" });
        println!("  GVG Family ID: {}", global_virtual_group_family_id);
        println!("  Fee: {} BNB (wei)", fee_amount);
        println!("  Gas Limit: {}", gas_limit);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Build EIP-712 message (matching Go SDK structure)
        let visibility_enum = match visibility {
            1 => Visibility::Public,
            2 => Visibility::Private,
            _ => Visibility::Inherit,
        };
        
        let eip_msg = TypeCreateObject {
            type_url: "/greenfield.storage.MsgCreateObject".to_string(),
            bucket_name: bucket_name.clone(),
            content_type: "application/octet-stream".to_string(),
            creator: checksummed_addr.clone(),
            expect_checksums: vec![], // Empty for now - Go SDK calculates these
            object_name: object_name.clone(),
            payload_size: payload_size.to_string(),
            primary_sp_approval: PrimarySpApproval {
                expired_height: u64::MAX.to_string(),
                global_virtual_group_family_id: global_virtual_group_family_id.to_string(),
            },
            redundancy_type: RedundancyType::EcType,
            visibility: visibility_enum,
        };
        
        let fee = Fee {
            amount: vec![Coin {
                denom: "BNB".to_string(),
                amount: fee_amount.to_string(),
            }],
            gas_limit: gas_limit.to_string(),
            granter: String::new(),
            payer: checksummed_addr.clone(),
        };
        
        let tx = Tx {
            account_number: account_number.to_string(),
            chain_id: chain_id_num.to_string(),
            fee,
            memo: String::new(),
            msg1: eip_msg,
            sequence: sequence.to_string(),
            timeout_height: "0".to_string(),
        };
        
        // Print JSON representation
        println!("\n📋 EIP-712 JSON Payload:");
        let json = serde_json::to_string_pretty(&tx)?;
        println!("{}", json);
        
        // Calculate EIP-712 hash
        println!("\n🔍 EIP-712 Hash Calculation:");
        let struct_hash = tx.get_struct_hash()?;
        let domain_separator = Tx::get_domain_separator(&chain_id_num.to_string())?;
        
        println!("\n   Domain Separator: 0x{}", hex::encode(domain_separator));
        println!("   Struct Hash: 0x{}", hex::encode(struct_hash));
        
        // Final EIP-712 hash
        use tiny_keccak::{Hasher, Keccak};
        let mut final_data = Vec::new();
        final_data.push(0x19);
        final_data.push(0x01);
        final_data.extend_from_slice(domain_separator.as_bytes());
        final_data.extend_from_slice(struct_hash.as_bytes());
        
        let mut hasher = Keccak::v256();
        hasher.update(&final_data);
        let mut final_hash = [0u8; 32];
        hasher.finalize(&mut final_hash);
        
        println!("   Final EIP-712 Hash: 0x{}", hex::encode(final_hash));
        
        // Sign and verify
        println!("\n🔐 Signature (for verification):");
        let sig = self.wallet.sign_hash(ethers::types::H256::from(final_hash))?;
        
        // Convert U256 to bytes for printing
        let mut r_bytes = [0u8; 32];
        let mut s_bytes = [0u8; 32];
        sig.r.to_big_endian(&mut r_bytes);
        sig.s.to_big_endian(&mut s_bytes);
        
        println!("   R: 0x{}", hex::encode(r_bytes));
        println!("   S: 0x{}", hex::encode(s_bytes));
        println!("   V: {}", sig.v);
        
        // Recover and verify
        let recovered = sig.recover(ethers::types::H256::from(final_hash))?;
        println!("   Recovered Address: {:?}", recovered);
        println!("   Expected Address: {:?}", self.wallet.address());
        
        if recovered == self.wallet.address() {
            println!("   ✅ Signature verification PASSED locally!");
        } else {
            println!("   ❌ Signature verification FAILED locally!");
        }
        
        Ok(())
    }
}
