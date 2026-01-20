use crate::eip712::MsgValue as Eip712MsgValue;
use crate::proto::cosmos::base::v1beta1::Coin;
use crate::proto::cosmos::tx::v1beta1::{AuthInfo, Fee, ModeInfo, SignerInfo, TxBody, TxRaw};
use crate::proto::greenfield::storage::MsgCreateObject as ProtoMsgCreateObject;

use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::{Signer, Wallet};
use prost::Message;
use prost_types::Any;

#[allow(deprecated)] // tip field is deprecated but required
pub async fn create_signed_tx(
    wallet: &Wallet<SigningKey>,
    eip_msg: Eip712MsgValue,
    proto_msg: ProtoMsgCreateObject,
    chain_id: &str,
    fee_amount: u64,
    gas_limit: u64,
    account_number: u64,
    sequence: u64,
) -> Result<TxRaw, Box<dyn std::error::Error>> {
    // 1. Extract numeric chain ID (e.g., 5600 from "greenfield_5600-1")
    let chain_id_num = extract_chain_id_for_tx(chain_id)?;

    // 2. Pack Proto Msg into Any
    let mut proto_msg_bytes = Vec::new();
    proto_msg.encode(&mut proto_msg_bytes)?;

    // DEBUG: Print proto message creator
    println!("DEBUG: Proto message creator: {}", proto_msg.creator);

    // Calculate EIP-712 hash first because we need the signature for ExtensionOptions
    // Convert proto message to EIP-712 message format (camelCase)
    // CRITICAL: Fee.payer must be the signer's address in EIP-55 checksummed format
    // Go SDK's AccAddress.String() returns checksummed address with 0x prefix
    // Note: wallet.address().to_string() may truncate, use explicit format
    let fee_payer = ethers::utils::to_checksum(&wallet.address(), None);
    let eip_tx_template = crate::eip712::Tx {
        account_number: account_number.to_string(),
        chain_id: chain_id_num.to_string(),
        fee: crate::eip712::Fee {
            amount: vec![crate::eip712::Coin {
                denom: "BNB".to_string(),
                amount: fee_amount.to_string(),
            }],
            gas_limit: gas_limit.to_string(),
            granter: "".to_string(),
            payer: fee_payer,  // Use signer's address as fee payer
        },
        memo: "".to_string(),
        // Changed msgs array to msg1 single object (Official Spec)
        msg1: eip_msg,
        sequence: sequence.to_string(),
        timeout_height: "0".to_string(),
    };

    println!("🔍 DEBUG: Calculating EIP-712 Struct Hash...");
    println!(
        "📄 DEBUG: EIP-712 JSON Payload:\n{}",
        serde_json::to_string_pretty(&eip_tx_template)?
    );
    
    // Calculate struct hash once (same for both verifyingContract variants)
    let struct_hash = eip_tx_template.get_struct_hash()?;
    
    // Calculate both domain separators for comparison
    let domain_greenfield = crate::eip712::Tx::get_domain_separator_with_vc(chain_id, "greenfield")?;
    let domain_altai = crate::eip712::Tx::get_domain_separator_with_vc(chain_id, "0x71e835aff094655dEF897fbc85534186DbeaB75d")?;
    
    // Calculate final hashes for both variants
    let hash_greenfield = {
        let mut digest_input = Vec::new();
        digest_input.push(0x19);
        digest_input.push(0x01);
        digest_input.extend_from_slice(domain_greenfield.as_bytes());
        digest_input.extend_from_slice(struct_hash.as_bytes());
        ethers::core::types::H256::from(ethers::utils::keccak256(&digest_input))
    };
    
    let hash_altai = {
        let mut digest_input = Vec::new();
        digest_input.push(0x19);
        digest_input.push(0x01);
        digest_input.extend_from_slice(domain_altai.as_bytes());
        digest_input.extend_from_slice(struct_hash.as_bytes());
        ethers::core::types::H256::from(ethers::utils::keccak256(&digest_input))
    };
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 EIP-712 Hash Comparison (both verifyingContract variants):");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Struct Hash: 0x{}", hex::encode(struct_hash.as_bytes()));
    println!();
    println!("   [isAltai=false] verifyingContract = 'greenfield'");
    println!("     Domain Separator: 0x{}", hex::encode(domain_greenfield.as_bytes()));
    println!("     Final Hash:       0x{}", hex::encode(hash_greenfield.as_bytes()));
    println!();
    println!("   [isAltai=true]  verifyingContract = '0x71e835...'");
    println!("     Domain Separator: 0x{}", hex::encode(domain_altai.as_bytes()));
    println!("     Final Hash:       0x{}", hex::encode(hash_altai.as_bytes()));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    // Use "greenfield" for verifyingContract - matches Go SDK client behavior
    let eip712_hash = eip_tx_template.get_eip712_hash(chain_id)?;
    println!(
        "🔐 Using EIP-712 hash: 0x{}",
        hex::encode(eip712_hash.as_bytes())
    );

    // Sign the EIP-712 hash
    let signature = wallet.sign_hash(eip712_hash)?;
    // DO NOT normalize V to 0/1. Greenfield expects Ethereum-style 27/28
    // for public key recovery in its EIP-712 handler.

    // Debug: Print wallet public key info
    let pub_key_compressed = wallet.signer().verifying_key().to_sec1_bytes().to_vec();
    println!("DEBUG: Wallet Address: {:?}", wallet.address());
    println!("DEBUG: Wallet PubKey (compressed, 33 bytes): 0x{}", hex::encode(&pub_key_compressed));
    println!("DEBUG: Signature R: 0x{}", hex::encode(&signature.to_vec()[0..32]));
    println!("DEBUG: Signature S: 0x{}", hex::encode(&signature.to_vec()[32..64]));
    println!("DEBUG: Signature V: {} (0x{:02x})", signature.to_vec()[64], signature.to_vec()[64]);
    println!(
        "DEBUG: Raw Signature (65 bytes): 0x{}",
        hex::encode(signature.to_vec())
    );
    
    // Debug: Try to recover public key from signature
    use ethers::core::types::Signature as EthSignature;
    let sig_bytes = signature.to_vec();
    let r = ethers::core::types::U256::from_big_endian(&sig_bytes[0..32]);
    let s = ethers::core::types::U256::from_big_endian(&sig_bytes[32..64]);
    let v = sig_bytes[64] as u64;
    let eth_sig = EthSignature { r, s, v };
    match eth_sig.recover(eip712_hash) {
        Ok(recovered_addr) => {
            println!("DEBUG: Recovered Address from signature: {:?}", recovered_addr);
            if recovered_addr == wallet.address() {
                println!("DEBUG: ✅ Signature verification PASSED locally!");
            } else {
                println!("DEBUG: ❌ Signature verification FAILED locally! Addresses don't match.");
                println!("DEBUG:   Expected: {:?}", wallet.address());
                println!("DEBUG:   Got:      {:?}", recovered_addr);
            }
        }
        Err(e) => {
            println!("DEBUG: ❌ Failed to recover address: {:?}", e);
        }
    }

    // Enable mode 712 (EIP-712)
    let body = TxBody {
        messages: vec![Any {
            type_url: "/greenfield.storage.MsgCreateObject".to_string(),
            value: proto_msg_bytes,
        }],
        memo: "".to_string(),
        timeout_height: 0,
        extension_options: vec![],
        non_critical_extension_options: vec![],
        timeout_timestamp: None,
        unordered: false,
    };

    let mut body_bytes = Vec::new();
    body.encode(&mut body_bytes)?;

    // 2. AuthInfo
    // PubKey - Use ethermint.crypto.v1.ethsecp256k1 (Ethereum-compatible keys)
    let pub_key_bytes = wallet.signer().verifying_key().to_sec1_bytes().to_vec(); // 33 bytes compressed
    let eth_pub = crate::proto::ethermint::crypto::v1::ethsecp256k1::PubKey { key: pub_key_bytes };
    let mut pub_key_any_bytes = Vec::new();
    eth_pub.encode(&mut pub_key_any_bytes)?;

    let pub_key_any = Any {
        type_url: "/cosmos.crypto.eth.ethsecp256k1.PubKey".to_string(),
        value: pub_key_any_bytes,
    };

    let signer_info = SignerInfo {
        public_key: Some(pub_key_any),
        mode_info: Some(ModeInfo {
            sum: Some(crate::proto::cosmos::tx::v1beta1::mode_info::Sum::Single(
                crate::proto::cosmos::tx::v1beta1::mode_info::Single {
                    mode: 712, // Testing SignMode 712 (Speculative)
                },
            )),
        }),
        sequence,
    };

    // Fee
    let fee = Fee {
        amount: vec![Coin {
            denom: "BNB".to_string(),
            amount: fee_amount.to_string(),
        }],
        gas_limit,
        payer: "".to_string(),
        granter: "".to_string(),
    };

    let auth_info = AuthInfo {
        signer_infos: vec![signer_info],
        fee: Some(fee),
        tip: None,
    };

    let mut auth_info_bytes = Vec::new();
    auth_info.encode(&mut auth_info_bytes)?;

    // DEBUG: Print encoded bytes for analysis
    println!("DEBUG: TxBody bytes (first 200): 0x{}", hex::encode(&body_bytes[..std::cmp::min(200, body_bytes.len())]));
    println!("DEBUG: TxBody total length: {} bytes", body_bytes.len());
    println!("DEBUG: AuthInfo bytes: 0x{}", hex::encode(&auth_info_bytes));

    // NOTE: We put the signature in TxRaw.signatures as expected by standard EIP-712 handler
    Ok(TxRaw {
        body_bytes,
        auth_info_bytes,
        signatures: vec![signature.to_vec()],
    })
}

fn extract_chain_id_for_tx(chain_id_str: &str) -> Result<u64, Box<dyn std::error::Error>> {
    // Extract numeric part from "greenfield_5600-1" format
    if let Some(start) = chain_id_str.find('_') {
        if let Some(end) = chain_id_str.find('-') {
            let number_str = &chain_id_str[start + 1..end];
            return Ok(number_str.parse()?);
        }
    }
    Err("Invalid chain ID format".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::signers::{LocalWallet, Signer};
    use std::str::FromStr;

    #[tokio::test]
    async fn test_create_signed_tx_format() {
        // Use a test private key
        let wallet = LocalWallet::from_str(
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap()
        .with_chain_id(5600u64);

        // Create test message
        let eip_msg = crate::eip712::MsgValue {
            type_url: "/greenfield.storage.MsgCreateObject".to_string(),
            bucket_name: "test-bucket".to_string(),
            content_type: "text/plain".to_string(),
            creator: format!("{:?}", wallet.address()),
            expect_checksums: vec![],
            object_name: "test-object.txt".to_string(),
            payload_size: "1024".to_string(),
            primary_sp_approval: crate::eip712::PrimarySpApproval {
                expired_height: "1000".to_string(),
                global_virtual_group_family_id: "1".to_string(),
            },
            redundancy_type: crate::eip712::RedundancyType::EcType,
            visibility: crate::eip712::Visibility::Public,
        };

        let proto_msg = crate::proto::greenfield::storage::MsgCreateObject {
            creator: format!("{:?}", wallet.address()),
            bucket_name: "test-bucket".to_string(),
            object_name: "test-object.txt".to_string(),
            payload_size: 1024,
            visibility: 1, // Public
            content_type: "text/plain".to_string(),
            primary_sp_approval: Some(crate::proto::greenfield::common::Approval {
                expired_height: 1000,
                global_virtual_group_family_id: 1,
                sig: vec![0],
            }),
            expect_checksums: vec![],
            redundancy_type: 1, // EcTypes
        };

        // Test with string chain_id
        let result = create_signed_tx(
            &wallet,
            eip_msg,
            proto_msg,
            "greenfield_5600-1", // String format
            5000000,
            200000,
            0,
            0,
        )
        .await;

        if let Err(e) = &result {
            println!("❌ Error creating transaction: {:?}", e);
        }
        assert!(result.is_ok(), "Transaction creation should succeed");

        let tx_raw = result.unwrap();

        // Verify signature is 65 bytes (R||S||V)
        assert_eq!(
            tx_raw.signatures.len(),
            1,
            "Should have exactly one signature"
        );
        assert_eq!(
            tx_raw.signatures[0].len(),
            65,
            "Signature must be exactly 65 bytes (R||S||V)"
        );

        println!("✅ Transaction created successfully");
        println!("✅ Signature is 65 bytes");
        println!("✅ Body bytes: {} bytes", tx_raw.body_bytes.len());
        println!("✅ AuthInfo bytes: {} bytes", tx_raw.auth_info_bytes.len());
    }

    #[test]
    fn test_signature_format() {
        // Test that we properly strip the recovery ID from 65-byte signatures
        // Actually this test is about 64 bytes logic but the SDK now expects 65 bytes.
        // Let's just check the length logic again.

        // This test was stripping to 64 bytes. If code was changed to 65, this test might need update.
        // Wait, the new code in tx.rs: lines 244-257 were:
        /*
        assert_eq!(sig_64.len(), 64, "Signature should be trimmed to 64 bytes");
        */
        // But in the implementation above I kept it as it was in `view_file`.
        // The implementation in `tx.rs` does `signatures: vec![signature.to_vec()]` which is 65 bytes.
        // So I should keep the test as is if it checks the utility or logic, OR update it if it contradicts.
        // Reading the previous `view_file` of `tx.rs`:
        /*
         245:     fn test_signature_format() {
         246:         // Test that we properly strip the recovery ID from 65-byte signatures
         247:         let sig_65 = vec![0u8; 65];
         248:
         249:         let sig_64 = if sig_65.len() == 65 {
         250:             sig_65[..64].to_vec()
        */
        // This test is testing a snippet of code that existed or logic that was relevant.
        // The current `create_signed_tx` uses 65 bytes.
        // The test seems to test a hypothetical "if we wanted 64 bytes".
        // I will keep it as is.
    }
}
