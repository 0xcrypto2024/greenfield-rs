# Greenfield Rust SDK - CreateBucket 实现指南

本文档详细记录了如何在 Rust 中实现 Greenfield 的 CreateBucket 功能，包括 EIP-712 签名、交易构建和广播。

## 目录

1. [背景知识](#背景知识)
2. [问题定位与解决过程](#问题定位与解决过程)
3. [Go SDK 代码追踪方法](#go-sdk-代码追踪方法)
4. [从零开始实现 CreateBucket](#从零开始实现-createbucket)
5. [Rust 代码详解](#rust-代码详解)
6. [关键经验总结](#关键经验总结)

---

## 背景知识

### Greenfield 交易签名机制

Greenfield 使用 **EIP-712** 签名机制，而非传统的 Cosmos SDK 签名方式。EIP-712 是以太坊的类型化结构数据签名标准。

```
┌─────────────────────────────────────────────────────────────┐
│                    EIP-712 签名流程                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Domain Separator = keccak256(EIP712Domain struct)       │
│                                                             │
│  2. Struct Hash = keccak256(Tx struct + nested types)       │
│                                                             │
│  3. Final Hash = keccak256("\x19\x01" || DS || SH)          │
│                                                             │
│  4. Signature = ECDSA.sign(Final Hash, private_key)         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 核心仓库

| 仓库 | 作用 |
|------|------|
| `greenfield-go-sdk` | Go SDK 客户端，包含 API 封装 |
| `greenfield` | 链节点实现，包含消息定义 |
| `greenfield-cosmos-sdk` | Cosmos SDK 分支，包含 EIP-712 签名逻辑 |

---

## 问题定位与解决过程

### 问题 1: `signature verification failed`

**错误信息**：
```
signature verification failed; feePayer's pubkey ... is different from signature's pubkey ...
```

**原因**：链上从签名中恢复的公钥与交易中的公钥不匹配，说明 EIP-712 hash 计算有问题。

**调试步骤**：

1. 打印 Go SDK 的 TypeHash、Domain Separator、Struct Hash
2. 打印 Rust SDK 的对应值
3. 逐项对比，找出差异

**发现的问题**：
- ❌ 字段顺序不对（需要按字母排序）
- ❌ 类型名称不对（`TypePrimarySpApproval` → `TypeMsg1PrimarySpApproval`）
- ❌ 数值类型编码不对（`uint64` 需要 32 字节左填充）

### 问题 2: `global virtual group family not exist`

**错误信息**：
```
global virtual group family not exist
```

**原因**：`primary_sp_approval.global_virtual_group_family_id = 0` 是无效的。

**解决方案**：从链上查询正确的 VGF ID。

---

## Go SDK 代码追踪方法

### 方法论

```
┌─────────────────────────────────────────────────────────────┐
│                    Go SDK 代码追踪路径                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 找入口：greenfield-go-sdk/client/api_bucket.go          │
│     └─> CreateBucket() 函数                                 │
│                                                             │
│  2. 找消息构建：                                             │
│     └─> storageTypes.NewMsgCreateBucket(...)                │
│     └─> 跳转到 greenfield/x/storage/types/message.go        │
│                                                             │
│  3. 找签名逻辑：                                             │
│     └─> c.BroadcastTx(ctx, msgs, opts.TxOpts)               │
│     └─> 跳转到 greenfield-cosmos-sdk/x/auth/tx/eip712.go    │
│                                                             │
│  4. 找 VGF ID 获取：                                         │
│     └─> c.GetRecommendedVirtualGroupFamilyIDBySPID(...)     │
│     └─> 或 c.GetCreateBucketApproval(...) 作为 fallback     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 关键文件路径

```
greenfield-go-sdk/
├── client/
│   ├── api_bucket.go          # CreateBucket 入口
│   ├── api_object.go          # CreateObject 入口
│   └── api_virtual_group.go   # VGF 查询
│
greenfield/
├── x/storage/types/
│   ├── message.go             # MsgCreateBucket 定义
│   └── tx.pb.go               # Proto 生成的消息结构
│
greenfield-cosmos-sdk/
├── x/auth/tx/
│   ├── eip712.go              # EIP-712 签名核心逻辑
│   │   ├── getSignBytes()     # 签名入口
│   │   ├── WrapTxToTypedData()# 构建 TypedData
│   │   ├── extractMsgTypes()  # 提取类型定义
│   │   └── traverseFields()   # 遍历字段
│   └── builder.go             # 交易构建
```

### 关键代码片段

**1. CreateBucket 入口** (`api_bucket.go`)

```go
func (c *Client) CreateBucket(ctx context.Context, bucketName string, primaryAddr string, opts types.CreateBucketOptions) (string, error) {
    // 1. 构建消息
    createBucketMsg := storageTypes.NewMsgCreateBucket(...)
    
    // 2. 获取 VGF ID (关键!)
    familyID, err := c.GetRecommendedVirtualGroupFamilyIDBySPID(ctx, sp.Id)
    if err != nil {
        // Fallback: 从 SP 获取 approval
        signedMsg, err = c.GetCreateBucketApproval(ctx, createBucketMsg)
        familyID = signedMsg.PrimarySpApproval.GlobalVirtualGroupFamilyId
    }
    
    // 3. 设置 VGF ID
    createBucketMsg.PrimarySpApproval.GlobalVirtualGroupFamilyId = familyID
    
    // 4. 广播交易
    resp, err := c.BroadcastTx(ctx, []sdk.Msg{createBucketMsg}, opts.TxOpts)
}
```

**2. EIP-712 字段排序** (`eip712.go`)

```go
// Go SDK 对 EIP-712 类型字段进行字母排序
sort.Slice(typeMap[typeDef], func(i, j int) bool {
    return typeMap[typeDef][i].Name < typeMap[typeDef][j].Name
})
```

**3. Domain Separator** (`eip712.go`)

```go
typedDataDomain := apitypes.TypedDataDomain{
    Name:              "Greenfield Tx",
    Version:           "1.0.0",
    ChainId:           (*math.HexOrDecimal256)(typedChainID),
    VerifyingContract: "greenfield",  // 或 Altai 地址
    Salt:              "0",
}
```

---

## 从零开始实现 CreateBucket

### Step 1: 理解消息结构

**Proto 定义** (`greenfield/proto/greenfield/storage/tx.proto`)

```protobuf
message MsgCreateBucket {
  string creator = 1;
  string bucket_name = 2;
  VisibilityType visibility = 3;
  string payment_address = 4;
  string primary_sp_address = 5;
  Approval primary_sp_approval = 6;
  uint64 charged_read_quota = 7;
}

message Approval {
  uint64 expired_height = 1;
  uint32 global_virtual_group_family_id = 2;
  bytes sig = 3;
}
```

### Step 2: 理解 EIP-712 类型映射

| Proto 类型 | EIP-712 类型 |
|-----------|-------------|
| `string` | `string` |
| `uint64` | `uint64` |
| `uint32` | `uint32` |
| `int32` (enum) | `string` |
| `bytes` | `bytes` |
| `bytes[]` | `bytes[]` |
| `message` | 嵌套结构体 |

### Step 3: 理解字段排序规则

**MsgCreateBucket 字段按字母排序**：
```
bucket_name, charged_read_quota, creator, payment_address, 
primary_sp_address, primary_sp_approval, type, visibility
```

### Step 4: 理解交易结构

```
TxRaw {
    body_bytes:      TxBody { messages: [MsgCreateBucket], memo, timeout_height }
    auth_info_bytes: AuthInfo { signer_infos: [SignerInfo], fee: Fee }
    signatures:      [65-byte signature (R || S || V)]
}
```

### Step 5: 完整签名流程

```
1. 构建 EIP-712 TypedData
   - Domain: name, version, chainId, verifyingContract, salt
   - Types: Tx, Fee, Coin, Msg1, TypeMsg1PrimarySpApproval
   - Message: 实际数据

2. 计算 Domain Separator
   - TypeHash = keccak256("EIP712Domain(...)")
   - Encode: TypeHash || chainId || name || salt || vc || version
   - DS = keccak256(Encode)

3. 计算 Struct Hash
   - TypeHash = keccak256("Tx(...)")
   - Encode: TypeHash || account_number || chain_id || fee_hash || ...
   - SH = keccak256(Encode)

4. 计算 Final Hash
   - FH = keccak256("\x19\x01" || DS || SH)

5. 签名
   - sig = ECDSA.sign(FH, private_key)
   - 65 bytes: R (32) || S (32) || V (1)

6. 组装 TxRaw 并广播
```

---

## Rust 代码详解

### 项目结构

```
greenfield-rs/src/
├── main.rs              # CLI 命令定义
├── client.rs            # GreenfieldClient 核心实现
│   ├── create_bucket()  # 创建 bucket
│   ├── create_object()  # 创建 object
│   └── put_object()     # 上传文件到 SP
├── eip712.rs            # CreateObject 的 EIP-712 类型
├── bucket_eip712.rs     # CreateBucket 的 EIP-712 类型
├── tx.rs                # 交易签名逻辑
├── sp.rs                # SP 和 VGF 查询
├── bucket.rs            # Bucket 信息查询
└── proto.rs             # Proto 模块导入
```

### EIP-712 类型定义 (`bucket_eip712.rs`)

```rust
/// EIP-712 Tx 结构 (字段按字母排序)
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

/// EIP-712 MsgCreateBucket (字段按字母排序)
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
```

### TypeHash 计算

```rust
fn get_full_type_string() -> String {
    let tx = "Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)";
    let coin = "Coin(uint256 amount,string denom)";
    let fee = "Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)";
    let msg1 = "Msg1(string bucket_name,uint64 charged_read_quota,string creator,string payment_address,string primary_sp_address,TypeMsg1PrimarySpApproval primary_sp_approval,string type,string visibility)";
    let psa = "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
    
    format!("{}{}{}{}{}", tx, coin, fee, msg1, psa)
}

pub fn get_type_hash() -> [u8; 32] {
    keccak256(Self::get_full_type_string().as_bytes())
}
```

### StructHash 计算

```rust
pub fn get_struct_hash(&self) -> Result<H256, Box<dyn std::error::Error>> {
    let type_hash = Self::get_type_hash();
    let mut encoded = Vec::new();
    
    // 1. TypeHash
    encoded.extend_from_slice(&type_hash);
    
    // 2. account_number (uint256 - 32 bytes, left-padded)
    let acc_num: u64 = self.account_number.parse()?;
    let mut acc_bytes = [0u8; 32];
    acc_bytes[24..32].copy_from_slice(&acc_num.to_be_bytes());
    encoded.extend_from_slice(&acc_bytes);
    
    // 3. chain_id (uint256)
    // ...
    
    // 4. fee (nested struct - use its StructHash)
    let fee_hash = self.fee.get_struct_hash();
    encoded.extend_from_slice(fee_hash.as_bytes());
    
    // 5. memo (string - keccak256)
    let memo_hash = keccak256(self.memo.as_bytes());
    encoded.extend_from_slice(&memo_hash);
    
    // 6. msg1 (nested struct)
    let msg1_hash = self.msg1.get_struct_hash()?;
    encoded.extend_from_slice(msg1_hash.as_bytes());
    
    // 7. sequence, timeout_height...
    
    Ok(H256::from(keccak256(&encoded)))
}
```

### Domain Separator 计算

```rust
pub fn get_domain_separator(chain_id_str: &str) -> Result<H256, ...> {
    let type_str = "EIP712Domain(uint256 chainId,string name,string salt,string verifyingContract,string version)";
    let type_hash = keccak256(type_str.as_bytes());
    
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&type_hash);
    
    // chainId (uint256, 32 bytes)
    let chain_id: u64 = parse_chain_id(chain_id_str)?;
    let mut cid_bytes = [0u8; 32];
    cid_bytes[24..32].copy_from_slice(&chain_id.to_be_bytes());
    encoded.extend_from_slice(&cid_bytes);
    
    // name = "Greenfield Tx"
    encoded.extend_from_slice(&keccak256(b"Greenfield Tx"));
    
    // salt = "0"
    encoded.extend_from_slice(&keccak256(b"0"));
    
    // verifyingContract = "greenfield"
    encoded.extend_from_slice(&keccak256(b"greenfield"));
    
    // version = "1.0.0"
    encoded.extend_from_slice(&keccak256(b"1.0.0"));
    
    Ok(H256::from(keccak256(&encoded)))
}
```

### 签名流程 (`client.rs`)

```rust
async fn sign_create_bucket_tx(&self, ...) -> Result<TxRaw, ...> {
    // 1. 构建 EIP-712 Tx 模板
    let eip_tx = TxCreateBucket { ... };
    
    // 2. 计算 EIP-712 hash
    let eip712_hash = eip_tx.get_eip712_hash(&chain_id)?;
    
    // 3. 签名
    let signature = self.wallet.sign_hash(eip712_hash)?;
    let sig_bytes = signature.to_vec();  // 65 bytes
    
    // 4. 构建 Proto TxBody
    let tx_body = TxBody {
        messages: vec![Any {
            type_url: "/greenfield.storage.MsgCreateBucket",
            value: proto_msg.encode_to_vec(),
        }],
        ...
    };
    
    // 5. 构建 AuthInfo (SignMode = 712)
    let auth_info = AuthInfo {
        signer_infos: vec![SignerInfo {
            mode_info: Some(ModeInfo { mode: 712 }),
            ...
        }],
        fee: Some(ProtoFee { ... }),
    };
    
    // 6. 返回 TxRaw
    Ok(TxRaw {
        body_bytes: tx_body.encode_to_vec(),
        auth_info_bytes: auth_info.encode_to_vec(),
        signatures: vec![sig_bytes],
    })
}
```

---

## 关键经验总结

### 1. EIP-712 规则

| 规则 | 说明 |
|------|------|
| 字段排序 | **必须按字母排序** |
| 类型命名 | `TypeMsg1PrimarySpApproval` (不是 `TypePrimarySpApproval`) |
| 数值编码 | `uint256`/`uint64`/`uint32` 都填充到 32 字节 |
| 字符串编码 | 先 keccak256，再放入 encoded 数组 |
| 嵌套结构 | 递归计算 StructHash |

### 2. 地址格式

| 场景 | 格式 |
|------|------|
| EIP-712 signing | EIP-55 checksummed (`0xEa39...caFE`) |
| Proto message | EIP-55 checksummed (`0xEa39...caFE`) |

### 3. 必须获取的数据

| 数据 | 来源 |
|------|------|
| `account_number` | 链上查询 `/cosmos/auth/v1beta1/accounts/{address}` |
| `sequence` | 链上查询 `/cosmos/auth/v1beta1/accounts/{address}` |
| `global_virtual_group_family_id` | 链上查询或 SP API |

### 4. SignMode

必须是 `712` (EIP-712 模式)，不是 `SIGN_MODE_DIRECT` 或其他。

---

## 参考资料

- [EIP-712 规范](https://eips.ethereum.org/EIPS/eip-712)
- [Greenfield 文档](https://docs.bnbchain.org/greenfield/)
- [Go SDK 源码](https://github.com/bnb-chain/greenfield-go-sdk)



