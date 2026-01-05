use ethers::utils::keccak256;
use ethers::types::H256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgCreateObject {
    pub creator: String,
    pub bucket_name: String,
    pub object_name: String,
    pub payload_size: u64,
    pub visibility: i32,
    pub content_type: String,
    pub primary_sp_approval: Approval,
    pub expect_checksums: Vec<Vec<u8>>, // repeated bytes
    pub redundancy_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub expired_height: u64,
    pub global_virtual_group_family_id: u32,
    pub sig: Vec<u8>,
}

impl Approval {
    pub fn get_type_hash() -> H256 {
        // Approval(uint64 expired_height,uint32 global_virtual_group_family_id,bytes sig)
        let type_str = "Approval(uint64 expired_height,uint32 global_virtual_group_family_id,bytes sig)";
        H256::from(keccak256(type_str.as_bytes()))
    }
    
    pub fn get_struct_hash(&self) -> H256 {
        let type_hash = Self::get_type_hash();
        
        // Encode fields
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());
        
        // expired_height: uint64 -> uint256 (32 bytes)
        let mut eh_bytes = [0u8; 32];
        eh_bytes[24..32].copy_from_slice(&self.expired_height.to_be_bytes());
        encoded.extend_from_slice(&eh_bytes);
        
        // global_virtual_group_family_id: uint32 -> uint256 (32 bytes)
        let mut gvg_bytes = [0u8; 32];
        gvg_bytes[28..32].copy_from_slice(&self.global_virtual_group_family_id.to_be_bytes());
        encoded.extend_from_slice(&gvg_bytes);
        
        // sig: bytes -> keccak256(sig)
        let sig_hash = keccak256(&self.sig);
        encoded.extend_from_slice(&sig_hash);
        
        H256::from(keccak256(&encoded))
    }
}

impl MsgCreateObject {
    pub fn get_type_hash() -> H256 {
        // MsgCreateObject(string bucket_name,string content_type,string creator,bytes[] expect_checksums,string object_name,uint64 payload_size,Approval primary_sp_approval,int32 redundancy_type,int32 visibility)Approval(uint64 expired_height,uint32 global_virtual_group_family_id,bytes sig)
        let type_str = "MsgCreateObject(string bucket_name,string content_type,string creator,bytes[] expect_checksums,string object_name,uint64 payload_size,Approval primary_sp_approval,int32 redundancy_type,int32 visibility)Approval(uint64 expired_height,uint32 global_virtual_group_family_id,bytes sig)";
        H256::from(keccak256(type_str.as_bytes()))
    }
    
    pub fn get_struct_hash(&self) -> H256 {
        let type_hash = Self::get_type_hash();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());
        
        // bucket_name: string -> keccak256(string)
        encoded.extend_from_slice(&keccak256(self.bucket_name.as_bytes()));
        
        // content_type: string
        encoded.extend_from_slice(&keccak256(self.content_type.as_bytes()));
        
        // creator: string
        encoded.extend_from_slice(&keccak256(self.creator.as_bytes()));
        
        // expect_checksums: bytes[] -> keccak256(concat(keccak256(item)...))
        // Array hashing: sha3(concat(pack(items)))? No.
        // EIP-712: "The array values are encoded as the keccak256 hash of the concatenated encodings of the contents"
        // Contents are bytes. Encoding of bytes is keccak256(bytes).
        let mut checksums_encoded = Vec::new();
        for cs in &self.expect_checksums {
            checksums_encoded.extend_from_slice(&keccak256(cs));
        }
        encoded.extend_from_slice(&keccak256(&checksums_encoded));
        
        // object_name: string
        encoded.extend_from_slice(&keccak256(self.object_name.as_bytes()));
        
        // payload_size: uint64
        let mut ps_bytes = [0u8; 32];
        ps_bytes[24..32].copy_from_slice(&self.payload_size.to_be_bytes());
        encoded.extend_from_slice(&ps_bytes);
        
        // primary_sp_approval: Approval -> structHash(approval)
        encoded.extend_from_slice(self.primary_sp_approval.get_struct_hash().as_bytes());
        
        // redundancy_type: int32 -> uint256 (sign extend?) 
        // int32 is just padded to 32 bytes.
        let mut rt_bytes = [0u8; 32];
        // assuming positive for enum
        rt_bytes[28..32].copy_from_slice(&self.redundancy_type.to_be_bytes());
        encoded.extend_from_slice(&rt_bytes);
        
        // visibility: int32
        let mut v_bytes = [0u8; 32];
        v_bytes[28..32].copy_from_slice(&self.visibility.to_be_bytes());
        encoded.extend_from_slice(&v_bytes);
        
        H256::from(keccak256(&encoded))
    }

    pub fn get_domain_separator() -> H256 {
         // EIP712Domain(string name,string version,uint256 chainId,string verifyingContract)
        let type_hash = keccak256(b"EIP712Domain(string name,string version,uint256 chainId,string verifyingContract)");
        let name_hash = keccak256(b"Greenfield Tx"); 
        let version_hash = keccak256(b"1.0.0");
        let chain_id = 5600u64;
        let contract_hash = keccak256(b"cosmos");
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);
        encoded.extend_from_slice(&name_hash);
        encoded.extend_from_slice(&version_hash);
        
        let mut chain_id_bytes = [0u8; 32];
        chain_id_bytes[24..32].copy_from_slice(&chain_id.to_be_bytes()); 
        encoded.extend_from_slice(&chain_id_bytes);
        
        encoded.extend_from_slice(&contract_hash);
        
        H256::from(keccak256(&encoded))
    }
    
    pub fn get_eip712_hash(&self) -> H256 {
        let domain_separator = Self::get_domain_separator();
        let struct_hash = self.get_struct_hash();
        
        let mut digest_input = Vec::new();
        digest_input.push(0x19);
        digest_input.push(0x01);
        digest_input.extend_from_slice(domain_separator.as_bytes());
        digest_input.extend_from_slice(struct_hash.as_bytes());
        
        H256::from(keccak256(&digest_input))
    }
}
