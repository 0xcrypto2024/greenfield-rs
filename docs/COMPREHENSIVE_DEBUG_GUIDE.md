# Greenfield Rust SDK 综合调试指南

本文档详细说明如何使用 Rust SDK 实现 Go SDK/Go CMD 已有的方法，以及遇到问题时如何使用提供的工具进行调试。

## 目录

1. [调试方法论概述](#1-调试方法论概述)
2. [调试环境搭建](#2-调试环境搭建)
3. [两类签名机制详解](#3-两类签名机制详解)
4. [问题分类与调试流程](#4-问题分类与调试流程)
5. [实战案例：CreateBucket](#5-实战案例createbucket)
6. [实战案例：CreateObject](#6-实战案例createobject)
7. [实战案例：Upload (PutObject)](#7-实战案例upload-putobject)
8. [调试工具使用指南](#8-调试工具使用指南)
9. [常见错误速查表](#9-常见错误速查表)
10. [调试检查清单](#10-调试检查清单)

---

## 1. 调试方法论概述

### 核心原则

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Greenfield SDK 调试方法论                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Step 1: 确定问题类型                                                │
│      │                                                              │
│      ├── 链上交易失败？ ──> EIP-712 签名问题                          │
│      │                                                              │
│      └── SP 请求失败？ ──> GNFD1-ECDSA 签名问题                       │
│                                                                     │
│  Step 2: 找到 Go SDK 对应实现                                        │
│      │                                                              │
│      ├── 链上交易 ──> greenfield-go-sdk + greenfield-cosmos-sdk      │
│      │                                                              │
│      └── SP 请求 ──> greenfield-go-sdk + greenfield-common           │
│                                                                     │
│  Step 3: 添加调试输出，对比中间值                                      │
│      │                                                              │
│      ├── 输出所有哈希值                                              │
│      │                                                              │
│      └── 逐层对比，找到第一个不匹配的位置                              │
│                                                                     │
│  Step 4: 定位根因，修复代码                                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 黄金法则

1. **Go SDK 是真理来源** - 所有实现都以 Go SDK 的行为为准
2. **逐层对比** - 从最外层（Final Hash）到最内层（单个字段）逐步对比
3. **二分法定位** - 当对比值很多时，用二分法快速找到差异点
4. **字节级对比** - 最终都要落到字节级别的对比

---

## 2. 调试环境搭建

### 2.1 克隆必要仓库

```bash
# 创建工作目录
mkdir -p ~/greenfield-debug && cd ~/greenfield-debug

# 克隆核心仓库
git clone https://github.com/bnb-chain/greenfield-go-sdk
git clone https://github.com/bnb-chain/greenfield
git clone https://github.com/bnb-chain/greenfield-cosmos-sdk
git clone https://github.com/bnb-chain/greenfield-common
git clone https://github.com/bnb-chain/greenfield-storage-provider
```

### 2.2 设置 Go SDK 调试环境

修改 `greenfield-go-sdk/go.mod`，使用本地仓库：

```go
replace github.com/cosmos/cosmos-sdk => ../greenfield-cosmos-sdk
replace github.com/bnb-chain/greenfield-common => ../greenfield-common
```

### 2.3 设置调试工具

```bash
# 进入调试工具目录
cd /path/to/greenfield-rs/debug/go_debug

# 初始化 Go 模块
go mod tidy

# 测试运行
go run main.go --help
```

### 2.4 IDE 设置建议

在 VSCode 中同时打开 Go SDK 和 Rust SDK 项目，方便对比代码：

```
workspace/
├── greenfield-go-sdk/      # Go SDK
├── greenfield-cosmos-sdk/  # EIP-712 签名
├── greenfield-common/      # SP 签名
└── greenfield-rs/          # Rust SDK
```

---

## 3. 两类签名机制详解

Greenfield 使用两种不同的签名机制，调试前必须明确问题属于哪一类。

### 3.1 EIP-712 签名（链上交易）

**使用场景**：CreateBucket, CreateObject, DeleteObject 等链上交易

**签名流程**：

```
┌─────────────────────────────────────────────────────────────────────┐
│                       EIP-712 签名流程                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. 构建 TypedData 结构                                              │
│     ├── Domain: chainId, name, version, verifyingContract, salt     │
│     ├── Types: Tx, Fee, Coin, Msg1, 嵌套类型...                      │
│     └── Message: 实际交易数据                                        │
│                                                                     │
│  2. 计算 Domain Separator                                            │
│     DS = keccak256(encode(EIP712Domain type + domain values))       │
│                                                                     │
│  3. 计算 Struct Hash                                                 │
│     SH = keccak256(encode(Tx type + tx values))                     │
│                                                                     │
│  4. 计算 Final Hash                                                  │
│     FH = keccak256(0x19 || 0x01 || DS || SH)                        │
│                                                                     │
│  5. ECDSA 签名                                                       │
│     signature = sign(FH, private_key)  // 65 bytes: R || S || V     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**关键代码位置**：

| 仓库 | 文件 | 作用 |
|------|------|------|
| greenfield-cosmos-sdk | `x/auth/tx/eip712.go` | EIP-712 签名主逻辑 |
| greenfield-cosmos-sdk | `x/auth/signing/eip712_types.go` | 类型映射 |
| greenfield-rs | `src/eip712.rs`, `src/bucket_eip712.rs` | Rust EIP-712 实现 |

### 3.2 GNFD1-ECDSA 签名（SP 请求）

**使用场景**：PutObject, GetObject, HeadObject 等 SP HTTP 请求

**签名流程**：

```
┌─────────────────────────────────────────────────────────────────────┐
│                     GNFD1-ECDSA 签名流程                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. 构建 Canonical Request                                          │
│     ┌─────────────────────────────────────────┐                     │
│     │ METHOD                                  │                     │
│     │ /bucket/object                          │                     │
│     │ (empty query)                           │                     │
│     │ header1:value1                          │                     │
│     │ header2:value2                          │                     │
│     │ host.example.com                        │                     │
│     │                                         │ <- 空行！            │
│     │ header1;header2                         │                     │
│     └─────────────────────────────────────────┘                     │
│                                                                     │
│  2. 计算 Message to Sign                                             │
│     msg = keccak256(canonical_request)                              │
│                                                                     │
│  3. ECDSA 签名                                                       │
│     signature = sign(msg, private_key)                              │
│     signature[64] -= 27  // V 值转换: 27/28 -> 0/1                   │
│                                                                     │
│  4. 构建 Authorization Header                                        │
│     Authorization: GNFD1-ECDSA, Signature=<hex(signature)>          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**关键代码位置**：

| 仓库 | 文件 | 作用 |
|------|------|------|
| greenfield-common | `go/http/gen_sign_str.go` | Canonical Request 构建 |
| greenfield-common | `go/http/const.go` | 支持的 Headers 列表 |
| greenfield-storage-provider | `modular/gater/request_context.go` | SP 端验证 |
| greenfield-rs | `src/client.rs` (put_object) | Rust SP 签名实现 |

---

## 4. 问题分类与调试流程

### 4.1 错误类型判断

```
错误信息                              -> 问题类型           -> 签名机制
─────────────────────────────────────────────────────────────────────
"signature verification failed"       -> EIP-712 哈希不匹配  -> EIP-712
"global virtual group family not exist" -> 参数错误          -> 链上逻辑
"ExpectChecksums missing"             -> Checksums 错误     -> 链上逻辑
─────────────────────────────────────────────────────────────────────
"no permission" (401)                 -> 签名恢复地址错误    -> GNFD1-ECDSA
"mismatched primary sp" (400)         -> SP 选择错误        -> 业务逻辑
"unsupported sign type" (401)         -> Authorization 格式  -> GNFD1-ECDSA
"incorrect expiry timestamp"          -> 时间戳格式错误      -> GNFD1-ECDSA
```

### 4.2 EIP-712 问题调试流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                    EIP-712 调试流程                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. 对比 Domain Separator                                            │
│     │                                                               │
│     ├── 相同？ ──> 继续下一步                                        │
│     │                                                               │
│     └── 不同？ ──> 检查:                                             │
│         ├── chainId 编码（uint256, 32字节左填充）                     │
│         ├── name/version/salt/verifyingContract 是否正确            │
│         └── EIP712Domain 类型字符串是否正确                          │
│                                                                     │
│  2. 对比 Type Hash                                                   │
│     │                                                               │
│     ├── 相同？ ──> 继续下一步                                        │
│     │                                                               │
│     └── 不同？ ──> 检查:                                             │
│         ├── 类型字符串中的字段顺序（必须字母排序）                     │
│         ├── 嵌套类型名称（TypeMsg1PrimarySpApproval 不是 Approval）   │
│         └── 类型字符串的拼接顺序                                     │
│                                                                     │
│  3. 对比 Struct Hash                                                 │
│     │                                                               │
│     ├── 相同？ ──> 继续下一步                                        │
│     │                                                               │
│     └── 不同？ ──> 逐个字段对比:                                      │
│         ├── 字符串字段：keccak256(string_bytes)                      │
│         ├── 数值字段：32字节左填充                                   │
│         ├── bytes[] 字段：⚠️ 特殊处理（见下文）                       │
│         └── 嵌套结构：递归计算 Struct Hash                           │
│                                                                     │
│  4. 对比 Final Hash                                                  │
│     │                                                               │
│     └── 应该相同 ──> 如果不同，检查前缀 0x19 0x01 的组合              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### bytes[] 字段的特殊处理

**这是最容易出错的地方！**

Go SDK 的 `jsonpb.Marshaler` 将 `bytes` 字段序列化为 **Base64 字符串**，然后 EIP-712 对这个 Base64 字符串的 **ASCII 字节** 进行 keccak256：

```
原始字节 [0x47, 0xDE, ...] 
    ↓
Base64 字符串 "R94Q..."
    ↓
ASCII 字节 [0x52, 0x39, 0x34, 0x51, ...]  // 'R', '9', '4', 'Q'
    ↓
keccak256 -> 最终哈希
```

**常见错误**：直接对原始字节 keccak256 ❌

### 4.3 GNFD1-ECDSA 问题调试流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                   GNFD1-ECDSA 调试流程                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. 对比 Canonical Request                                           │
│     │                                                               │
│     ├── 逐行对比:                                                    │
│     │   ├── Method (PUT/GET/HEAD)                                   │
│     │   ├── Path (URL 编码)                                         │
│     │   ├── Query (空或 URL 编码)                                    │
│     │   ├── Headers (排序、格式)                                     │
│     │   ├── Host (不带 host: 前缀)                                   │
│     │   ├── 空行 ⚠️                                                  │
│     │   └── Signed Headers                                          │
│     │                                                               │
│     └── 不同？ ──> 最常见问题:                                        │
│         ├── Header 排序不对（必须字母顺序，小写）                      │
│         ├── 换行符数量不对（Host 后面需要两个 \n）                    │
│         ├── Host 格式不对（有无端口号）                               │
│         └── 缺少必需的 Header (X-Gnfd-Content-Sha256)                │
│                                                                     │
│  2. 对比 Message to Sign                                             │
│     │                                                               │
│     └── 应该是 Canonical Request 的 keccak256                        │
│                                                                     │
│  3. 验证签名恢复                                                      │
│     │                                                               │
│     └── 从签名恢复地址，是否等于钱包地址？                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. 实战案例：CreateBucket

### 5.1 功能概述

CreateBucket 在链上创建一个新的存储桶。

### 5.2 曾遇到的问题

| 问题 | 错误信息 | 根因 | 解决方案 |
|------|---------|------|---------|
| VGF ID 无效 | `global virtual group family not exist` | VGF ID = 0 无效 | 从链上查询有效 VGF ID |
| 签名验证失败 | `signature verification failed` | Type Hash 不匹配 | 字段按字母排序，类型名称修正 |
| 地址格式错误 | 签名恢复地址不匹配 | 未使用 EIP-55 格式 | 使用 checksummed 地址 |

### 5.3 调试步骤

**Step 1: 运行 Go 调试工具获取基准值**

```bash
cd debug/go_debug
go run main.go create-bucket \
    --bucket my-test-bucket \
    --sp 0x5ccF0F6b78a37Ef4e2CcBC10D155c28Fb8bE9BaF \
    --vgf-id 1 \
    --creator 0xEa39644C04b40316f7270EDf7bB4249c6F47caFE \
    > go_bucket_debug.txt
```

**Step 2: 在 Rust 代码中添加调试输出**

在 `src/bucket_eip712.rs` 添加：

```rust
pub fn get_eip712_hash(&self, chain_id: &str) -> Result<H256, String> {
    let domain_separator = get_domain_separator(chain_id)?;
    let struct_hash = self.get_struct_hash()?;
    
    println!("=== CreateBucket EIP-712 Debug ===");
    println!("Domain Separator: 0x{}", hex::encode(domain_separator.as_bytes()));
    println!("Type Hash: 0x{}", hex::encode(Self::get_type_hash()));
    println!("Struct Hash: 0x{}", hex::encode(struct_hash.as_bytes()));
    
    let final_hash = compute_final_hash(domain_separator, struct_hash);
    println!("Final Hash: 0x{}", hex::encode(final_hash.as_bytes()));
    
    Ok(final_hash)
}
```

**Step 3: 运行 Rust SDK 并捕获输出**

```bash
cargo run -- --keystore ./my-wallet create-bucket \
    --bucket my-test-bucket \
    --sp-address 0x5ccF0F6b78a37Ef4e2CcBC10D155c28Fb8bE9BaF \
    --visibility 2 \
    2>&1 | tee rust_bucket_debug.txt
```

**Step 4: 使用对比脚本**

```bash
./debug/compare_tx.sh go_bucket_debug.txt rust_bucket_debug.txt
```

**Step 5: 根据对比结果定位问题**

如果 Type Hash 不匹配，检查类型字符串：

```bash
# 打印 Go SDK 的类型字符串
go run main.go eip712-types
```

对比 Rust 中的类型字符串是否一致。

### 5.4 关键检查点

- [ ] `global_virtual_group_family_id` 从链上查询，不为 0
- [ ] 地址使用 EIP-55 checksummed 格式
- [ ] Type String 中字段按字母排序
- [ ] 嵌套类型名称是 `TypeMsg1PrimarySpApproval`

---

## 6. 实战案例：CreateObject

### 6.1 功能概述

CreateObject 在链上创建 object 元数据，**不包含**实际文件上传。

### 6.2 曾遇到的问题

| 问题 | 错误信息 | 根因 | 解决方案 |
|------|---------|------|---------|
| VGF ID 错误 | `signature verification failed` | CreateObject 的 VGF ID 应为 0 | 设置 `global_virtual_group_family_id = 0` |
| Checksums 为空 | `ExpectChecksums missing` | 未计算 checksums | 实现 Reed-Solomon 计算 7 个 checksums |
| bytes[] 编码错误 | `signature verification failed` | 直接 hash 原始字节 | Base64 编码后 hash ASCII 字节 |

### 6.3 调试步骤

**Step 1: 验证 Checksums 计算**

```bash
# 使用 Go 调试工具查看 checksums 的正确格式
go run main.go create-object \
    --bucket my-bucket \
    --object test.txt \
    > go_object_debug.txt
```

检查输出中的 checksums 格式：
- 数量是否为 7
- 是否是 Base64 格式
- 每个 checksum 的 keccak256 hash

**Step 2: 在 Rust 中添加 checksums 调试**

```rust
// 在 eip712.rs 中
fn get_checksums_hash(&self) -> Result<[u8; 32], String> {
    println!("=== Checksums Debug ===");
    for (i, cs) in self.expect_checksums.iter().enumerate() {
        let raw = hex::decode(cs.trim_start_matches("0x")).unwrap();
        let b64 = base64::encode(&raw);
        let hash = keccak256(b64.as_bytes());
        println!("[{}] Hex: {}", i, cs);
        println!("    Base64: {}", b64);
        println!("    Hash: 0x{}", hex::encode(&hash));
    }
    // ...
}
```

**Step 3: 对比每个 checksum 的哈希**

确保 Rust 中每个 checksum 的处理流程是：
```
hex string -> decode -> raw bytes -> base64 encode -> ASCII bytes -> keccak256
```

### 6.4 关键检查点

- [ ] `global_virtual_group_family_id = 0`（CreateObject 必须为 0）
- [ ] Checksums 数量为 7
- [ ] Checksums 使用 Base64 编码后再 hash
- [ ] `expired_height = u64::MAX`

---

## 7. 实战案例：Upload (PutObject)

### 7.1 功能概述

Upload = CreateObject (链上) + 等待 SP 同步 + PutObject (SP HTTP)

### 7.2 曾遇到的问题

| 问题 | 错误信息 | 根因 | 解决方案 |
|------|---------|------|---------|
| SP 不匹配 | `mismatched primary sp` (400) | 上传到错误的 SP | 从 bucket 获取 primary SP |
| 权限错误 | `no permission` (401) | 缺少 X-Gnfd-Content-Sha256 | 添加空字符串 SHA256 |
| 权限错误 | `no permission` (401) | Canonical Request 格式错误 | 修正换行符数量 |
| 未同步 | `no such object` (404) | SP 未同步 object | 添加重试等待逻辑 |

### 7.3 调试步骤

**Step 1: 使用调试工具生成 Canonical Request**

```bash
go run main.go put-object \
    --bucket my-bucket \
    --object test.txt \
    --host gnfd-testnet-sp3.bnbchain.org \
    > go_put_debug.txt
```

**Step 2: 在 Rust 中添加 Canonical Request 调试**

```rust
// 在 client.rs 的 put_object 中
println!("=== Canonical Request Debug ===");
println!("Canonical Request:");
println!("---");
println!("{}", canonical_request);
println!("---");
println!("Length: {} bytes", canonical_request.len());
println!("Hex: {}", hex::encode(canonical_request.as_bytes()));

// 打印换行符位置
for (i, b) in canonical_request.as_bytes().iter().enumerate() {
    if *b == b'\n' {
        println!("Newline at position {}", i);
    }
}

println!("Message to Sign: 0x{}", hex::encode(&msg_to_sign));
```

**Step 3: 逐字节对比**

重点检查：
1. Headers 是否按字母顺序
2. Host 后面是否有两个换行符
3. X-Gnfd-Content-Sha256 是否包含在签名中

### 7.4 换行符问题详解

**这是最常见的问题！**

Go SDK 使用 `strings.Join([], "\n")`：

```go
canonicalRequest := strings.Join([]string{
    "PUT",
    "/bucket/object",
    "",
    canonicalHeaders,  // 末尾已有 \n
    signedHeaders,
}, "\n")
```

由于 `canonicalHeaders` 末尾已有 `\n`，Join 后会有两个连续的 `\n`：

```
...header\n
host\n
\n              <- 两个换行！
signedHeaders
```

**Rust 中必须显式添加这个额外的换行：**

```rust
// 错误
format!("{}\n{}\n{}\n{}{}", method, path, query, canonical_headers, signed_headers)

// 正确
format!("{}\n{}\n{}\n{}\n{}", method, path, query, canonical_headers, signed_headers)
```

### 7.5 关键检查点

- [ ] X-Gnfd-Content-Sha256 header 存在
- [ ] Headers 按字母顺序排列
- [ ] Host 后面有两个换行符
- [ ] V 值从 27/28 转换为 0/1
- [ ] 添加 SP 同步等待逻辑

---

## 8. 调试工具使用指南

### 8.1 Go 调试工具

位置：`debug/go_debug/main.go`

**命令列表**：

```bash
# 查看帮助
go run main.go --help

# 调试 Domain Separator 计算
go run main.go domain-separator --chain-id 5600

# 打印所有 EIP-712 类型字符串
go run main.go eip712-types

# 调试 CreateBucket
go run main.go create-bucket --bucket NAME --sp ADDR --vgf-id ID

# 调试 CreateObject
go run main.go create-object --bucket NAME --object NAME

# 调试 PutObject Canonical Request
go run main.go put-object --bucket NAME --object NAME --host HOST
```

### 8.2 对比脚本

位置：`debug/compare_tx.sh`

**使用方法**：

```bash
# 对比两个输出文件
./debug/compare_tx.sh go_output.txt rust_output.txt
```

**脚本会自动提取并对比**：
- Domain Separator
- Type Hash
- Struct Hash
- Final Hash
- Message to Sign (SP 请求)

### 8.3 在 Go SDK 源码中添加调试

**EIP-712 签名** - 修改 `greenfield-cosmos-sdk/x/auth/tx/eip712.go`：

```go
func getSignBytes(mode signingtypes.SignMode, signerData signing.SignerData, tx sdk.Tx, isAltai bool) ([]byte, error) {
    // ... 原有代码 ...
    
    // 添加调试输出
    fmt.Printf("=== EIP-712 Debug ===\n")
    fmt.Printf("Domain Separator: 0x%x\n", domainSeparator)
    fmt.Printf("Type Hash: 0x%x\n", typeHash)
    fmt.Printf("Struct Hash: 0x%x\n", structHash)
    fmt.Printf("Final Hash: 0x%x\n", signBytes)
    
    return signBytes, nil
}
```

**SP 签名** - 修改 `greenfield-common/go/http/gen_sign_str.go`：

```go
func GetCanonicalRequest(req *http.Request) string {
    // ... 原有代码 ...
    
    fmt.Printf("=== Canonical Request Debug ===\n")
    fmt.Printf("Canonical Request:\n---\n%s\n---\n", canonicalRequest)
    fmt.Printf("Hex: %x\n", []byte(canonicalRequest))
    
    for i, b := range []byte(canonicalRequest) {
        if b == '\n' {
            fmt.Printf("Newline at position %d\n", i)
        }
    }
    
    return canonicalRequest
}
```

### 8.4 在 Rust SDK 中添加调试

**EIP-712 签名** - 在 `eip712.rs` 或 `bucket_eip712.rs`：

```rust
pub fn get_eip712_hash(&self, chain_id: &str) -> Result<H256, String> {
    let domain_separator = get_domain_separator(chain_id)?;
    let type_hash = Self::get_type_hash();
    let struct_hash = self.get_struct_hash()?;
    
    println!("=== EIP-712 Debug ===");
    println!("Domain Separator: 0x{}", hex::encode(domain_separator.as_bytes()));
    println!("Type Hash: 0x{}", hex::encode(&type_hash));
    println!("Struct Hash: 0x{}", hex::encode(struct_hash.as_bytes()));
    
    let mut encoded = vec![0x19, 0x01];
    encoded.extend_from_slice(domain_separator.as_bytes());
    encoded.extend_from_slice(struct_hash.as_bytes());
    let final_hash = H256::from(keccak256(&encoded));
    
    println!("Final Hash: 0x{}", hex::encode(final_hash.as_bytes()));
    
    Ok(final_hash)
}
```

**SP 签名** - 在 `client.rs`：

```rust
pub async fn put_object(&self, ...) -> Result<(), Box<dyn std::error::Error>> {
    // ... 构建 canonical_request ...
    
    println!("=== Canonical Request Debug ===");
    println!("Canonical Request:\n---\n{}\n---", canonical_request);
    println!("Length: {} bytes", canonical_request.len());
    println!("Hex: {}", hex::encode(canonical_request.as_bytes()));
    
    for (i, &b) in canonical_request.as_bytes().iter().enumerate() {
        if b == b'\n' {
            println!("Newline at position {}", i);
        }
    }
    
    let msg_to_sign = keccak256(canonical_request.as_bytes());
    println!("Message to Sign: 0x{}", hex::encode(&msg_to_sign));
    
    // ... 继续签名 ...
}
```

---

## 9. 常见错误速查表

### 9.1 链上交易错误

| 错误信息 | 可能原因 | 调试方法 |
|---------|---------|---------|
| `signature verification failed` | EIP-712 hash 不匹配 | 对比 Domain/Type/Struct Hash |
| `global virtual group family not exist` | VGF ID 无效 | 检查 VGF ID 获取逻辑 |
| `ExpectChecksums missing, expect: 7` | Checksums 数量错误 | 检查 Reed-Solomon 计算 |
| `feePayer's pubkey is different` | 签名错误 | 对比 Final Hash |

### 9.2 SP 请求错误

| 错误码 | 错误信息 | 可能原因 | 调试方法 |
|--------|---------|---------|---------|
| 400 | `mismatched primary sp` | 上传到错误的 SP | 检查 SP 获取逻辑 |
| 401 | `no permission` | 签名验证后地址不匹配 | 对比 Canonical Request |
| 401 | `unsupported sign type` | Authorization 格式错误 | 检查 header 格式 |
| 401 | `incorrect expiry timestamp` | 时间戳格式错误 | 使用 ISO 8601 格式 |
| 404 | `no such object` | SP 未同步 | 添加重试等待 |

### 9.3 快速修复指南

**问题：Domain Separator 不匹配**
```rust
// 检查 chainId 编码
let chain_id: u64 = 5600;
let mut bytes = [0u8; 32];
bytes[24..32].copy_from_slice(&chain_id.to_be_bytes());  // 左填充到 32 字节
```

**问题：Type Hash 不匹配**
```rust
// 检查字段排序（必须字母顺序）
// 错误: creator, bucket_name, ...
// 正确: bucket_name, creator, ...

// 检查类型名称
// 错误: TypeApproval, Approval
// 正确: TypeMsg1PrimarySpApproval
```

**问题：bytes[] 哈希不匹配**
```rust
// 错误
let hash = keccak256(&raw_bytes);

// 正确
let base64_str = base64::encode(&raw_bytes);
let hash = keccak256(base64_str.as_bytes());  // Hash ASCII bytes!
```

**问题：Canonical Request 不匹配**
```rust
// 检查换行符
// 错误: format!("{}\n{}\n{}\n{}{}", ...)
// 正确: format!("{}\n{}\n{}\n{}\n{}", ...)  // 多一个 \n

// 检查 V 值
if sig_bytes[64] >= 27 {
    sig_bytes[64] -= 27;  // 27/28 -> 0/1
}
```

---

## 10. 调试检查清单

### 10.1 实现新功能前

- [ ] 在 Go SDK 中找到对应函数
- [ ] 理解函数签名和参数
- [ ] 追踪消息构建逻辑
- [ ] 确定签名类型（EIP-712 或 GNFD1-ECDSA）

### 10.2 EIP-712 签名检查

- [ ] Domain 参数正确（chainId, name, version, salt, vc）
- [ ] 类型字符串字段按字母排序
- [ ] 嵌套类型名称正确（TypeMsg1XXX）
- [ ] 数值类型 32 字节左填充
- [ ] 字符串类型先 keccak256
- [ ] bytes[] 类型: Base64 -> ASCII bytes -> keccak256
- [ ] 地址使用 EIP-55 checksummed 格式

### 10.3 GNFD1-ECDSA 签名检查

- [ ] Headers 按字母顺序（小写）
- [ ] 包含必需的 Headers (Content-Type, X-Gnfd-Content-Sha256, X-Gnfd-Expiry-Timestamp)
- [ ] Host 不带 "host:" 前缀
- [ ] Host 后有两个换行符
- [ ] 时间戳格式正确（ISO 8601, UTC, Z 后缀）
- [ ] V 值转换（27/28 -> 0/1）
- [ ] Authorization 格式："GNFD1-ECDSA, Signature=..."

### 10.4 调试完成确认

- [ ] 所有哈希值与 Go SDK 一致
- [ ] 交易/请求成功执行
- [ ] 移除或注释调试输出
- [ ] 更新相关文档

---

## 附录：参考资料

### 官方资源
- [Greenfield 文档](https://docs.bnbchain.org/bnb-greenfield/)
- [Greenfield 白皮书](https://github.com/bnb-chain/greenfield-whitepaper)
- [Go SDK](https://github.com/bnb-chain/greenfield-go-sdk)

### 技术规范
- [EIP-712](https://eips.ethereum.org/EIPS/eip-712)
- [Reed-Solomon 擦除码](https://en.wikipedia.org/wiki/Reed%E2%80%93Solomon_error_correction)

### 相关文档
- [CREATE_BUCKET_IMPLEMENTATION.md](./CREATE_BUCKET_IMPLEMENTATION.md)
- [CREATE_OBJECT_IMPLEMENTATION.md](./CREATE_OBJECT_IMPLEMENTATION.md)
- [UPLOAD_OBJECT_IMPLEMENTATION.md](./UPLOAD_OBJECT_IMPLEMENTATION.md)
- [GREENFIELD_ARCHITECTURE_GUIDE.md](./GREENFIELD_ARCHITECTURE_GUIDE.md)

