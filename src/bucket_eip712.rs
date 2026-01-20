//! EIP-712 types for CreateBucket message
use ethers::types::H256;
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};

use crate::eip712::{Fee, Visibility};

/// EIP-712 Tx structure for CreateBucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCreateBucket {
    pub account_number: String,
    pub chain_id: String,
    pub fee: Fee,
    pub memo: String,
    pub msg1: MsgCreateBucket,
    pub sequence: String,
    pub timeout_height: String,
}

/// EIP-712 MsgCreateBucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgCreateBucket {
    #[serde(rename = "type")]
    pub type_url: String,
    pub bucket_name: String,
    pub charged_read_quota: String,
    pub creator: String,
    pub payment_address: String,
    pub primary_sp_address: String,
    pub primary_sp_approval: PrimarySpApproval,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimarySpApproval {
    pub expired_height: String,
    pub global_virtual_group_family_id: u32,
}

impl TxCreateBucket {
    /// Get the EIP-712 type hash for CreateBucket Tx
    /// Fields are sorted alphabetically as per Go SDK
    pub fn get_type_hash() -> [u8; 32] {
        // Go SDK output:
        // Tx TypeHash: 0xc5ce375061c176a3775dc7754ff9711ef8ef729c7600f7a08c208450577c7ad4
        let type_str = Self::get_full_type_string();
        println!("      [DEBUG] Tx TypeString: {}", type_str);
        keccak256(type_str.as_bytes())
    }

    fn get_full_type_string() -> String {
        // From Go SDK debug output - fields sorted alphabetically:
        // Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)
        // Coin(uint256 amount,string denom)
        // Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)
        // Msg1(string bucket_name,uint64 charged_read_quota,string creator,string payment_address,string primary_sp_address,TypeMsg1PrimarySpApproval primary_sp_approval,string type,string visibility)
        // TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)
        
        let tx = "Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)";
        let coin = "Coin(uint256 amount,string denom)";
        let fee = "Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)";
        let msg1 = "Msg1(string bucket_name,uint64 charged_read_quota,string creator,string payment_address,string primary_sp_address,TypeMsg1PrimarySpApproval primary_sp_approval,string type,string visibility)";
        let psa = "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        
        format!("{}{}{}{}{}", tx, coin, fee, msg1, psa)
    }

    pub fn get_struct_hash(&self) -> Result<H256, Box<dyn std::error::Error>> {
        let type_hash = Self::get_type_hash();
        println!("      [DEBUG] Tx TypeHash: 0x{}", hex::encode(&type_hash));
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);
        
        // account_number
        let acc_num: u64 = self.account_number.parse()?;
        let mut acc_bytes = [0u8; 32];
        acc_bytes[24..32].copy_from_slice(&acc_num.to_be_bytes());
        encoded.extend_from_slice(&acc_bytes);
        println!("      [DEBUG] account_number: {} -> 0x{}", acc_num, hex::encode(&acc_bytes));
        
        // chain_id
        let chain_id: u64 = self.chain_id.parse()?;
        let mut cid_bytes = [0u8; 32];
        cid_bytes[24..32].copy_from_slice(&chain_id.to_be_bytes());
        encoded.extend_from_slice(&cid_bytes);
        println!("      [DEBUG] chain_id: {} -> 0x{}", chain_id, hex::encode(&cid_bytes));
        
        // fee (struct hash)
        let fee_hash = self.fee.get_struct_hash();
        encoded.extend_from_slice(fee_hash.as_bytes());
        println!("      [DEBUG] fee_hash: 0x{}", hex::encode(fee_hash.as_bytes()));
        
        // memo
        let memo_hash = keccak256(self.memo.as_bytes());
        encoded.extend_from_slice(&memo_hash);
        println!("      [DEBUG] memo: '{}' -> 0x{}", self.memo, hex::encode(&memo_hash));
        
        // msg1 (struct hash)
        let msg1_hash = self.msg1.get_struct_hash()?;
        encoded.extend_from_slice(msg1_hash.as_bytes());
        println!("      [DEBUG] msg1_hash: 0x{}", hex::encode(msg1_hash.as_bytes()));
        
        // sequence
        let seq: u64 = self.sequence.parse()?;
        let mut seq_bytes = [0u8; 32];
        seq_bytes[24..32].copy_from_slice(&seq.to_be_bytes());
        encoded.extend_from_slice(&seq_bytes);
        println!("      [DEBUG] sequence: {} -> 0x{}", seq, hex::encode(&seq_bytes));
        
        // timeout_height
        let timeout: u64 = self.timeout_height.parse()?;
        let mut timeout_bytes = [0u8; 32];
        timeout_bytes[24..32].copy_from_slice(&timeout.to_be_bytes());
        encoded.extend_from_slice(&timeout_bytes);
        println!("      [DEBUG] timeout_height: {} -> 0x{}", timeout, hex::encode(&timeout_bytes));
        
        let hash = H256::from(keccak256(&encoded));
        println!("      [DEBUG] Tx final_hash: 0x{}", hex::encode(hash.as_bytes()));
        Ok(hash)
    }

    pub fn get_domain_separator(chain_id_str: &str) -> Result<H256, Box<dyn std::error::Error>> {
        // Use the same domain separator logic as CreateObject
        crate::eip712::Tx::get_domain_separator(chain_id_str)
    }

    pub fn get_eip712_hash(&self, chain_id: &str) -> Result<H256, Box<dyn std::error::Error>> {
        let domain_separator = Self::get_domain_separator(chain_id)?;
        let struct_hash = self.get_struct_hash()?;
        
        let mut digest_input = Vec::new();
        digest_input.push(0x19);
        digest_input.push(0x01);
        digest_input.extend_from_slice(domain_separator.as_bytes());
        digest_input.extend_from_slice(struct_hash.as_bytes());
        
        println!("   Domain Separator: 0x{}", hex::encode(domain_separator.as_bytes()));
        println!("   Struct Hash: 0x{}", hex::encode(struct_hash.as_bytes()));
        
        Ok(H256::from(keccak256(&digest_input)))
    }
}

impl MsgCreateBucket {
    fn get_type_hash() -> [u8; 32] {
        // Msg1(string bucket_name,uint64 charged_read_quota,string creator,string payment_address,string primary_sp_address,TypeMsg1PrimarySpApproval primary_sp_approval,string type,string visibility)TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)
        let msg1 = "Msg1(string bucket_name,uint64 charged_read_quota,string creator,string payment_address,string primary_sp_address,TypeMsg1PrimarySpApproval primary_sp_approval,string type,string visibility)";
        let psa = "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        let type_str = format!("{}{}", msg1, psa);
        println!("      [DEBUG] Msg1 TypeString: {}", type_str);
        keccak256(type_str.as_bytes())
    }

    pub fn get_struct_hash(&self) -> Result<H256, Box<dyn std::error::Error>> {
        let type_hash = Self::get_type_hash();
        println!("      [DEBUG] Msg1 TypeHash: 0x{}", hex::encode(&type_hash));
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);
        
        // bucket_name
        let bucket_hash = keccak256(self.bucket_name.as_bytes());
        encoded.extend_from_slice(&bucket_hash);
        println!("      [DEBUG] bucket_name: '{}' -> 0x{}", self.bucket_name, hex::encode(&bucket_hash));
        
        // charged_read_quota (uint64)
        let quota: u64 = self.charged_read_quota.parse()?;
        let mut quota_bytes = [0u8; 32];
        quota_bytes[24..32].copy_from_slice(&quota.to_be_bytes());
        encoded.extend_from_slice(&quota_bytes);
        println!("      [DEBUG] charged_read_quota: {} -> 0x{}", quota, hex::encode(&quota_bytes));
        
        // creator
        let creator_hash = keccak256(self.creator.as_bytes());
        encoded.extend_from_slice(&creator_hash);
        println!("      [DEBUG] creator: '{}' -> 0x{}", self.creator, hex::encode(&creator_hash));
        
        // payment_address
        let payment_hash = keccak256(self.payment_address.as_bytes());
        encoded.extend_from_slice(&payment_hash);
        println!("      [DEBUG] payment_address: '{}' -> 0x{}", self.payment_address, hex::encode(&payment_hash));
        
        // primary_sp_address
        let sp_hash = keccak256(self.primary_sp_address.as_bytes());
        encoded.extend_from_slice(&sp_hash);
        println!("      [DEBUG] primary_sp_address: '{}' -> 0x{}", self.primary_sp_address, hex::encode(&sp_hash));
        
        // primary_sp_approval (struct hash)
        let psa_hash = self.primary_sp_approval.get_struct_hash()?;
        encoded.extend_from_slice(psa_hash.as_bytes());
        println!("      [DEBUG] psa_hash: 0x{}", hex::encode(psa_hash.as_bytes()));
        
        // type (type_url)
        let type_hash = keccak256(self.type_url.as_bytes());
        encoded.extend_from_slice(&type_hash);
        println!("      [DEBUG] type: '{}' -> 0x{}", self.type_url, hex::encode(&type_hash));
        
        // visibility
        let vis_hash = keccak256(self.visibility.as_str().as_bytes());
        encoded.extend_from_slice(&vis_hash);
        println!("      [DEBUG] visibility: '{}' -> 0x{}", self.visibility.as_str(), hex::encode(&vis_hash));
        
        let hash = H256::from(keccak256(&encoded));
        println!("      [DEBUG] Msg1 final_hash: 0x{}", hex::encode(hash.as_bytes()));
        Ok(hash)
    }
}

impl PrimarySpApproval {
    fn get_type_hash() -> [u8; 32] {
        let type_str = "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        println!("      [DEBUG] PSA TypeString: {}", type_str);
        keccak256(type_str.as_bytes())
    }

    pub fn get_struct_hash(&self) -> Result<H256, Box<dyn std::error::Error>> {
        let type_hash = Self::get_type_hash();
        println!("      [DEBUG] PSA TypeHash: 0x{}", hex::encode(&type_hash));
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);
        
        // expired_height (uint64)
        let height: u64 = self.expired_height.parse()?;
        let mut height_bytes = [0u8; 32];
        height_bytes[24..32].copy_from_slice(&height.to_be_bytes());
        encoded.extend_from_slice(&height_bytes);
        println!("      [DEBUG] expired_height: {} -> 0x{}", height, hex::encode(&height_bytes));
        
        // global_virtual_group_family_id (uint32)
        let mut gvg_bytes = [0u8; 32];
        gvg_bytes[28..32].copy_from_slice(&self.global_virtual_group_family_id.to_be_bytes());
        encoded.extend_from_slice(&gvg_bytes);
        println!("      [DEBUG] gvg_family_id: {} -> 0x{}", self.global_virtual_group_family_id, hex::encode(&gvg_bytes));
        
        let hash = H256::from(keccak256(&encoded));
        println!("      [DEBUG] PSA final_hash: 0x{}", hex::encode(hash.as_bytes()));
        Ok(hash)
    }
}

