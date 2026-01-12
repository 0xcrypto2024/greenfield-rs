use ethers::utils::keccak256;

fn main() {
    let psa_struct_hash = hex::decode("900338c7c5743af38e025360471666e2c1c5c1d16b5960ffe5eb2299850b18ec").unwrap();
    
    let msg_val_type = "MsgValue(string bucket_name,string content_type,string creator,string[] expect_checksums,string object_name,uint64 payload_size,PrimarySpApproval primary_sp_approval,string redundancy_type,string visibility)PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id,bytes sig)";
    let msg_val_hash = keccak256(msg_val_type.as_bytes());
    
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&msg_val_hash);
    
    encoded.extend_from_slice(&keccak256(b"test-bucket-123")); // bucket_name
    encoded.extend_from_slice(&keccak256(b"application/octet-stream")); // content_type
    encoded.extend_from_slice(&keccak256(b"0xd486d5ed56bf568449cdd3b131c5e300e6ff98a6")); // creator
    encoded.extend_from_slice(&keccak256(b"")); // expect_checksums (empty array)
    encoded.extend_from_slice(&keccak256(b"readme-final-v10-final-retry")); // object_name
    
    // payload_size: 6725
    let mut ps_bytes = [0u8; 32];
    ps_bytes[30..32].copy_from_slice(&6725u16.to_be_bytes());
    encoded.extend_from_slice(&ps_bytes);
    
    encoded.extend_from_slice(&psa_struct_hash); // primary_sp_approval
    encoded.extend_from_slice(&keccak256(b"REDUNDANCY_EC_TYPE")); // redundancy_type
    encoded.extend_from_slice(&keccak256(b"VISIBILITY_TYPE_PUBLIC")); // visibility
    
    let final_msg_val = keccak256(&encoded);
    println!("MsgValue StructHash -> 0x{}", hex::encode(final_msg_val));
}
