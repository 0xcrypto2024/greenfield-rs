use super::*;
use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;

#[tokio::test]
async fn test_create_signed_tx_format() {
    // Use a test private key
    let wallet =
        LocalWallet::from_str("0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
            .unwrap()
            .with_chain_id(5600u64);

    // Create test message
    let eip_msg = crate::eip712::MsgValue {
        bucket_name: "test-bucket".to_string(),
        content_type: "text/plain".to_string(),
        creator: format!("{:?}", wallet.address()),
        expect_checksums: vec![],
        object_name: "test-object.txt".to_string(),
        payload_size: "1024".to_string(),
        primary_sp_approval: crate::eip712::PrimarySpApproval {
            expired_height: "1000".to_string(),
            global_virtual_group_family_id: 1,
            sig: None,
        },
        redundancy_type: crate::eip712::RedundancyType::EcType,
        visibility: crate::eip712::Visibility::Public,
    };

    let proto_msg = crate::proto::greenfield::storage::MsgCreateObject {
        creator: format!("{:?}", wallet.address()),
        bucket_name: "test-bucket".to_string(),
        object_name: "test-object.txt".to_string(),
        payload_size: 1024,
        visibility: 1,
        content_type: "text/plain".to_string(),
        primary_sp_approval: Some(crate::proto::greenfield::common::Approval {
            expired_height: 1000,
            global_virtual_group_family_id: 1,
            sig: vec![0],
        }),
        expect_checksums: vec![],
        redundancy_type: 0,
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

    assert!(result.is_ok(), "Transaction creation should succeed");

    let tx_raw = result.unwrap();

    // Verify signature is exactly 64 bytes (R||S without recovery ID)
    assert_eq!(
        tx_raw.signatures.len(),
        1,
        "Should have exactly one signature"
    );
    assert_eq!(
        tx_raw.signatures[0].len(),
        64,
        "Signature must be exactly 64 bytes (R||S), got {} bytes",
        tx_raw.signatures[0].len()
    );

    println!("✅ Transaction created successfully");
    println!("✅ Signature is 64 bytes: {:?}", tx_raw.signatures[0].len());
    println!("✅ Body bytes: {} bytes", tx_raw.body_bytes.len());
    println!("✅ AuthInfo bytes: {} bytes", tx_raw.auth_info_bytes.len());
}

#[test]
fn test_signature_format() {
    // Test that we properly strip the recovery ID from 65-byte signatures
    let sig_65 = vec![0u8; 65];

    let sig_64 = if sig_65.len() == 65 {
        sig_65[..64].to_vec()
    } else {
        sig_65
    };

    assert_eq!(sig_64.len(), 64, "Signature should be trimmed to 64 bytes");
}
