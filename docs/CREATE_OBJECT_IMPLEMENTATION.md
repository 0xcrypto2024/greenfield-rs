# Greenfield Rust SDK - CreateObject 实现指南

本文档详细记录了如何在 Rust 中实现 Greenfield 的 CreateObject 功能，包括 checksums 计算、EIP-712 签名、交易构建和广播。

## 目录

1. [背景知识](#背景知识)
2. [问题定位与解决过程](#问题定位与解决过程)
3. [Go SDK 代码追踪方法](#go-sdk-代码追踪方法)
4. [从零开始实现 CreateObject](#从零开始实现-createobject)
5. [Rust 代码详解](#rust-代码详解)
6. [关键经验总结](#关键经验总结)

---

## 背景知识

### CreateObject 与 CreateBucket 的关系

```
┌─────────────────────────────────────────────────────────────┐
│                    操作依赖关系                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. CreateBucket  ──必须先完成──>  2. CreateObject           │
│                                         │                   │
│                                         ▼                   │
│                                   3. PutObject (上传)       │
│                                         │                   │
│                                         ▼                   │
│                                   4. Object Sealed (SP处理)  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### CreateObject 的核心任务

CreateObject 在链上创建 object 的元数据，包括：
- Object 名称、大小、类型
- **7 个 Integrity Checksums**（使用 Reed-Solomon 擦除码计算）
- 可见性设置

### 与 CreateBucket 的关键差异

| 字段 | CreateBucket | CreateObject |
|------|-------------|--------------|
| `global_virtual_group_family_id` | 从链上查询有效 VGF ID | **必须为 0** |
| `expect_checksums` | 无 | 7 个 SHA256 checksums |
| `payload_size` | 无 | 文件大小（字节） |

### 核心仓库

| 仓库 | 作用 |
|------|------|
| `greenfield-go-sdk` | Go SDK 客户端，包含 `CreateObject` 和 `ComputeHashRoots` |
| `greenfield` | 链节点实现，包含消息定义和校验逻辑 |
| `greenfield-cosmos-sdk` | EIP-712 签名逻辑，特别是 `bytes[]` 类型处理 |

---

## 问题定位与解决过程

### 问题 1: `global_virtual_group_family_id` 设置错误

**错误信息**：
```
signature verification failed; feePayer's pubkey ... is different from signature's pubkey ...
```

**排查过程**：

1. 复用 CreateBucket 的 VGF ID 获取逻辑
2. 发现签名验证失败
3. 对比 Go SDK 发现 CreateObject **不设置** VGF ID（默认为 0）

**根因**：Go SDK 的 `NewMsgCreateObject` 不设置 `GlobalVirtualGroupFamilyId`，导致其默认值为 0。

```go
// greenfield/x/storage/types/message.go
func NewMsgCreateObject(...) *MsgCreateObject {
    return &MsgCreateObject{
        Creator:            creator.String(),
        BucketName:         bucketName,
        ObjectName:         objectName,
        // ...
        PrimarySpApproval: &common.Approval{
            ExpiredHeight: timeoutHeight,
            Sig:           sig,
            // GlobalVirtualGroupFamilyId 未设置，默认为 0
        },
    }
}
```

**解决方案**：在 Rust 中将 `global_virtual_group_family_id` 设为 0。

---

### 问题 2: `ExpectChecksums missing`

**错误信息**：
```
ExpectChecksums missing, expect: 7, actual: 0
```

**排查过程**：

查看链上校验逻辑 (`greenfield/x/storage/keeper/msg_server.go`):

```go
if len(msg.ExpectChecksums) != int(1+k.GetExpectSecondarySPNumForECObject(ctx, ctx.BlockTime().Unix())) {
    return nil, gnfderrors.ErrInvalidChecksum.Wrapf(
        "ExpectChecksums missing, expect: %d, actual: %d",
        1+k.GetExpectSecondarySPNumForECObject(ctx, ctx.BlockTime().Unix()),
        len(msg.ExpectChecksums))
}
```

**根因**：
- 链要求 `1 + 数据分片 + 校验分片 = 1 + 4 + 2 = 7` 个 checksums
- 第一个是整个文件的 root hash
- 后 6 个是分片的 hashes (4 data + 2 parity)

**解决方案**：实现 `compute_hash_from_file()` 函数，使用 Reed-Solomon 擦除码计算 checksums。

---

### 问题 3: EIP-712 `bytes[]` 编码错误

**错误信息**：
```
signature verification failed; feePayer's pubkey ... is different from signature's pubkey ...
```

**排查过程**：

1. 对比 Go SDK 的 EIP-712 JSON 输出
2. 发现 `expect_checksums` 字段的值格式不同

Go SDK 输出：
```json
"expect_checksums": [
    "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=",
    "..."
]
```

Rust SDK 输出：
```json
"expect_checksums": [
    "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "..."
]
```

**关键发现**：Go SDK 的 `jsonpb.Marshaler` 将 `bytes` 字段序列化为 **Base64 字符串**！

追踪代码 (`greenfield-cosmos-sdk/x/auth/tx/eip712.go`):

```go
// cleanTypesAndMsgValue 函数使用 jsonpb.Marshaler
msgCodec := jsonpb.Marshaler{
    EmitDefaults: true,
    OrigName:     true,
}
msgsJsonStr, _ = msgCodec.MarshalToString(signDoc)
```

Proto 中 `bytes` 类型通过 `jsonpb` 序列化时会变成 Base64 字符串。

**EIP-712 对 bytes[] 的哈希计算**：

```go
// 不是对原始字节哈希，而是对 Base64 字符串的 ASCII 字节哈希！
hash := keccak256(base64String.getBytes())  // ASCII 字节
```

**解决方案**：

```rust
// 错误做法
let raw_bytes = hex::decode(checksum)?;
let hash = keccak256(&raw_bytes);

// 正确做法
let raw_bytes = hex::decode(checksum)?;
let base64_str = base64::encode(&raw_bytes);
let hash = keccak256(base64_str.as_bytes());  // Hash ASCII bytes of Base64 string
```

---

### 问题 4: `expired_height` 值

**问题**：Go SDK 使用 `math.MaxUint` 作为 `expired_height`。

```go
// greenfield-go-sdk/client/api_object.go
createObjectMsg := storageTypes.NewMsgCreateObject(
    // ...
    math.MaxUint,  // expiredHeight
    nil,           // sig
)
```

**解决方案**：在 Rust 中使用 `u64::MAX`：

```rust
let expired_height = u64::MAX;  // 18446744073709551615
```

---

## Go SDK 代码追踪方法

### 方法论

```
┌─────────────────────────────────────────────────────────────┐
│                    Go SDK 代码追踪路径                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 找入口：greenfield-go-sdk/client/api_object.go          │
│     └─> CreateObject() 函数                                 │
│                                                             │
│  2. 找 checksums 计算：                                      │
│     └─> c.ComputeHashRoots(reader, opts.IsSerialComputeMode)│
│     └─> 在同一文件中查找实现                                  │
│                                                             │
│  3. 找消息构建：                                             │
│     └─> storageTypes.NewMsgCreateObject(...)                │
│     └─> 跳转到 greenfield/x/storage/types/message.go        │
│                                                             │
│  4. 找链上校验逻辑：                                         │
│     └─> greenfield/x/storage/keeper/msg_server.go           │
│     └─> CreateObject() 函数中的校验                          │
│                                                             │
│  5. 找 EIP-712 bytes[] 处理：                                │
│     └─> greenfield-cosmos-sdk/x/auth/tx/eip712.go           │
│     └─> cleanTypesAndMsgValue() 和 jsonpb.Marshaler         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 关键文件路径

```
greenfield-go-sdk/
├── client/
│   └── api_object.go          # CreateObject 入口和 ComputeHashRoots
│
greenfield/
├── x/storage/types/
│   ├── message.go             # NewMsgCreateObject 定义
│   └── tx.pb.go               # Proto 生成的消息结构
├── x/storage/keeper/
│   └── msg_server.go          # CreateObject 链上处理和校验
│
greenfield-cosmos-sdk/
├── x/auth/tx/
│   └── eip712.go              # cleanTypesAndMsgValue - bytes 序列化
```

### 关键代码片段

**1. CreateObject 入口** (`api_object.go`)

```go
func (c *Client) CreateObject(ctx context.Context, bucketName, objectName string,
    reader io.Reader, opts types.CreateObjectOptions) (string, error) {
    
    // 1. 计算 checksums
    expectCheckSums, size, redundancyType, err := c.ComputeHashRoots(reader, opts.IsSerialComputeMode)
    
    // 2. 构建消息
    createObjectMsg := storageTypes.NewMsgCreateObject(
        c.MustGetDefaultAccount().GetAddress(),
        bucketName, objectName,
        uint64(size), visibility,
        expectCheckSums,   // 7 个 checksums
        contentType, redundancyType,
        math.MaxUint,      // expired_height
        nil,               // sig
    )
    
    // 3. 广播交易
    resp, err := c.BroadcastTx(ctx, []sdk.Msg{createObjectMsg}, opts.TxOpts)
    return resp.TxResponse.TxHash, nil
}
```

**2. ComputeHashRoots** (`api_object.go`)

```go
func (c *Client) ComputeHashRoots(reader io.Reader, isSerial bool) ([][]byte, int64, storageTypes.RedundancyType, error) {
    // 使用 Reed-Solomon (4,2) 配置
    // 读取文件，分成 16MB segments
    // 计算每个 segment 的 hash
    // 使用 erasure coding 生成 parity segments
    // 返回 [rootHash, seg1Hash, seg2Hash, seg3Hash, seg4Hash, parity1Hash, parity2Hash]
}
```

**3. 链上校验** (`msg_server.go`)

```go
func (k Keeper) CreateObject(ctx sdk.Context, msg *types.MsgCreateObject) (*types.MsgCreateObjectResponse, error) {
    // 校验 checksums 数量
    expectSecondarySPNum := k.GetExpectSecondarySPNumForECObject(ctx, ctx.BlockTime().Unix())
    if len(msg.ExpectChecksums) != int(1+expectSecondarySPNum) {
        return nil, gnfderrors.ErrInvalidChecksum.Wrapf(
            "ExpectChecksums missing, expect: %d, actual: %d",
            1+expectSecondarySPNum, len(msg.ExpectChecksums))
    }
    // ...
}
```

---

## 从零开始实现 CreateObject

### Step 1: 理解消息结构

**Proto 定义** (`greenfield/proto/greenfield/storage/tx.proto`)

```protobuf
message MsgCreateObject {
  string creator = 1;
  string bucket_name = 2;
  string object_name = 3;
  uint64 payload_size = 4;
  VisibilityType visibility = 5;
  string content_type = 6;
  Approval primary_sp_approval = 7;
  repeated bytes expect_checksums = 8;
  RedundancyType redundancy_type = 9;
}
```

### Step 2: 实现 Checksums 计算

**Reed-Solomon 配置**：
- Data Shards: 4
- Parity Shards: 2
- Segment Size: 16 MB

**计算流程**：

```
┌─────────────────────────────────────────────────────────────┐
│                    Checksums 计算流程                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 读取文件，分成 16MB 的 segments                          │
│                                                             │
│  2. 对每个 segment 计算 SHA256 hash                          │
│                                                             │
│  3. 使用 Reed-Solomon (4,2) 生成 parity segments             │
│                                                             │
│  4. 对 parity segments 计算 SHA256 hash                      │
│                                                             │
│  5. 计算所有 segment hashes 的 root hash                     │
│                                                             │
│  输出: [rootHash, d1, d2, d3, d4, p1, p2] (7 个 checksums)   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Step 3: 理解 EIP-712 类型映射

| Proto 类型 | EIP-712 类型 | 特殊处理 |
|-----------|-------------|---------|
| `bytes[]` | `bytes[]` | **Base64 编码后取 ASCII 字节再 hash** |
| `uint64` | `uint64` | 32 字节左填充 |
| `string` | `string` | keccak256(bytes) |

### Step 4: TypeHash 字符串

```
Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)
Coin(uint256 amount,string denom)
Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)
Msg1(string bucket_name,string content_type,string creator,bytes[] expect_checksums,string object_name,uint64 payload_size,TypeMsg1PrimarySpApproval primary_sp_approval,string redundancy_type,string type,string visibility)
TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)
```

**注意**：字段按字母排序！

---

## Rust 代码详解

### Checksums 计算 (`hash.rs`)

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Sha256, Digest};

const DATA_SHARDS: usize = 4;
const PARITY_SHARDS: usize = 2;
const SEGMENT_SIZE: usize = 16 * 1024 * 1024;  // 16MB

pub fn compute_hash_from_file(file_path: &str) -> Result<(Vec<Vec<u8>>, u64), Box<dyn std::error::Error>> {
    let file_content = std::fs::read(file_path)?;
    let file_size = file_content.len() as u64;
    
    // 如果文件小于一个 segment，直接 hash
    if file_content.len() <= SEGMENT_SIZE {
        let hash = sha256(&file_content);
        // 填充到 6 个分片
        let mut checksums = vec![hash.clone()];  // root = single segment hash
        for _ in 0..6 {
            checksums.push(hash.clone());
        }
        return Ok((checksums, file_size));
    }
    
    // 分成 segments
    let segments: Vec<Vec<u8>> = file_content
        .chunks(SEGMENT_SIZE)
        .map(|c| c.to_vec())
        .collect();
    
    // 计算每个 segment 的 hash
    let segment_hashes: Vec<Vec<u8>> = segments.iter()
        .map(|s| sha256(s))
        .collect();
    
    // 使用 Reed-Solomon 生成 parity
    let rs = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS)?;
    // ... erasure coding ...
    
    // 计算 root hash
    let mut all_hashes = Vec::new();
    for h in &segment_hashes {
        all_hashes.extend(h);
    }
    let root_hash = sha256(&all_hashes);
    
    // 返回 [root, d1, d2, d3, d4, p1, p2]
    let mut checksums = vec![root_hash];
    checksums.extend(segment_hashes);
    checksums.extend(parity_hashes);
    
    Ok((checksums, file_size))
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
```

### EIP-712 `bytes[]` 哈希 (`eip712.rs`)

```rust
pub fn get_checksums_hash(&self) -> Result<[u8; 32], String> {
    if self.expect_checksums.is_empty() {
        return Ok(keccak256(b""));
    }
    
    let mut inner = Vec::new();
    for cs_hex in &self.expect_checksums {
        // 1. 解码 hex -> 原始字节
        let raw_bytes = hex::decode(cs_hex.trim_start_matches("0x"))
            .map_err(|e| format!("Failed to decode hex: {}", e))?;
        
        // 2. 编码为 Base64 字符串
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&raw_bytes);
        
        // 3. 对 Base64 字符串的 ASCII 字节进行 keccak256
        inner.extend_from_slice(&keccak256(base64_str.as_bytes()));
    }
    
    Ok(keccak256(&inner))
}
```

### 消息构建 (`client.rs`)

```rust
async fn create_object_internal(
    &self,
    bucket_name: &str,
    object_name: &str,
    checksums: Vec<Vec<u8>>,
    file_size: u64,
    visibility: i32,
    content_type: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    
    // 构建 Proto 消息
    let proto_msg = ProtoMsgCreateObject {
        creator: checksummed_addr.clone(),
        bucket_name: bucket_name.to_string(),
        object_name: object_name.to_string(),
        payload_size: file_size,
        visibility: visibility,
        content_type: content_type.to_string(),
        primary_sp_approval: Some(ProtoApproval {
            expired_height: u64::MAX,
            global_virtual_group_family_id: 0,  // CreateObject 必须为 0！
            sig: vec![],
        }),
        expect_checksums: checksums.clone(),
        redundancy_type: 0,  // EC_TYPE
    };
    
    // 构建 EIP-712 消息
    let eip_msg = Eip712MsgCreateObject {
        type_url: "/greenfield.storage.MsgCreateObject".to_string(),
        bucket_name: bucket_name.to_string(),
        // ... 字段按字母排序 ...
        expect_checksums: checksums.iter()
            .map(|c| format!("0x{}", hex::encode(c)))
            .collect(),
        primary_sp_approval: Eip712ObjectApproval {
            expired_height: u64::MAX.to_string(),
            global_virtual_group_family_id: "0".to_string(),
        },
    };
    
    // 签名和广播
    let tx_raw = self.sign_create_object_tx(proto_msg, eip_msg, ...)?;
    let tx_hash = self.broadcast_tx(&tx_raw).await?;
    
    Ok(tx_hash)
}
```

---

## 关键经验总结

### 1. CreateObject vs CreateBucket 的 VGF ID

| 操作 | `global_virtual_group_family_id` | 原因 |
|------|--------------------------------|------|
| CreateBucket | 从链上查询有效 VGF ID | Bucket 需要分配到 SP 的虚拟组 |
| CreateObject | **必须为 0** | Object 会继承 Bucket 的 VGF |

### 2. `bytes[]` 的 EIP-712 编码

```
原始字节 -> Base64 字符串 -> ASCII 字节 -> keccak256
```

**不是**：
```
原始字节 -> keccak256  ❌
```

### 3. Checksums 数量

链上要求 **7 个** checksums：
- 1 个 root hash
- 4 个 data segment hashes
- 2 个 parity segment hashes

### 4. `expired_height` 值

使用 `u64::MAX` (18446744073709551615) 表示"永不过期"。

### 5. 调试顺序

1. 先确保 checksums 计算正确（数量和内容）
2. 再确保 EIP-712 TypeHash 正确
3. 最后确保 `bytes[]` 编码正确

---

## 参考资料

- [EIP-712 规范](https://eips.ethereum.org/EIPS/eip-712)
- [Greenfield 文档](https://docs.bnbchain.org/greenfield/)
- [Go SDK 源码](https://github.com/bnb-chain/greenfield-go-sdk)
- [Reed-Solomon 擦除码](https://en.wikipedia.org/wiki/Reed%E2%80%93Solomon_error_correction)

