use crate::proto::cosmos::tx::v1beta1::{
    TxRaw, TxBody, AuthInfo, SignerInfo, ModeInfo, Fee
};
use crate::proto::cosmos::tx::signing::v1beta1::SignMode;
use crate::proto::cosmos::base::v1beta1::Coin;
use crate::proto::greenfield::storage::MsgCreateObject as ProtoMsgCreateObject;
use crate::eip712::MsgCreateObject as Eip712MsgCreateObject;

use prost::Message;
use prost_types::Any;
use ethers::signers::Wallet;
use ethers::core::k256::ecdsa::SigningKey;

#[allow(deprecated)] // tip field is deprecated but required
pub async fn create_signed_tx(
    wallet: &Wallet<SigningKey>, 
    eip_msg: Eip712MsgCreateObject,
    proto_msg: ProtoMsgCreateObject,
    _chain_id: u64,
    fee_amount: u64,
    gas_limit: u64,
    _account_number: u64,
    sequence: u64,
) -> Result<TxRaw, Box<dyn std::error::Error>> {

    // 1. Pack Proto Msg into Any
    let mut msg_bytes = Vec::new();
    proto_msg.encode(&mut msg_bytes)?;
    
    let any_msg = Any {
        type_url: "/greenfield.storage.MsgCreateObject".to_string(),
        value: msg_bytes,
    };
    
    // 2. Sign EIP-712 Hash
    let hash = eip_msg.get_eip712_hash();
    let signature = wallet.sign_hash(hash)?;
    let sig_bytes = signature.to_vec(); // 65 bytes (r,s,v)
    
    // 4. Create TxBody (No Extension)
    let body = TxBody {
        messages: vec![any_msg],
        memo: "".to_string(),
        timeout_height: 0,
        extension_options: vec![],
        non_critical_extension_options: vec![],
        timeout_timestamp: None,
        unordered: false,
    };
    
    let mut body_bytes = Vec::new();
    body.encode(&mut body_bytes)?;
    
    // 5. AuthInfo
    // PubKey (Revert to Cosmos Secp256k1 to pass parsing)
    let pub_key_bytes = wallet.signer().verifying_key().to_sec1_bytes().to_vec(); // 33 bytes compressed
    let secp_pub = crate::proto::cosmos::crypto::secp256k1::PubKey { key: pub_key_bytes };
    let mut pub_key_any_bytes = Vec::new();
    secp_pub.encode(&mut pub_key_any_bytes)?;
    
    let pub_key_any = Any {
        type_url: "/cosmos.crypto.secp256k1.PubKey".to_string(),
        value: pub_key_any_bytes,
    };
    
    let signer_info = SignerInfo {
        public_key: Some(pub_key_any),
        mode_info: Some(ModeInfo {
            sum: Some(crate::proto::cosmos::tx::v1beta1::mode_info::Sum::Single(
                crate::proto::cosmos::tx::v1beta1::mode_info::Single {
                    mode: SignMode::Eip191 as i32,
                }
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
    
    Ok(TxRaw {
        body_bytes,
        auth_info_bytes,
        signatures: vec![sig_bytes],
    })
}
