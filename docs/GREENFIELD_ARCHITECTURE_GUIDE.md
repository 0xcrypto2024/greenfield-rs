# Greenfield 架构与资源导航指南

本文档帮助开发者理解 Greenfield 的核心架构、操作依赖关系，以及如何找到官方文档和源码中的相关信息。

## 目录

1. [Greenfield 简介](#greenfield-简介)
2. [核心架构](#核心架构)
3. [操作依赖关系](#操作依赖关系)
4. [官方资源导航](#官方资源导航)
5. [源码仓库结构](#源码仓库结构)
6. [开发者快速入门路径](#开发者快速入门路径)

---

## Greenfield 简介

BNB Greenfield 是 BNB Chain 生态中的去中心化存储网络。其核心特点：

- **以太坊兼容地址**：使用与以太坊相同的地址格式和签名机制 (EIP-712)
- **存储提供者 (SP)**：负责实际数据存储的节点
- **跨链互操作**：与 BNB Smart Chain (BSC) 原生集成
- **数据权限管理**：链上管理数据访问权限

### 官方资源

| 资源 | 链接 | 说明 |
|------|------|------|
| 官方文档 | https://docs.bnbchain.org/bnb-greenfield/ | 完整的技术文档 |
| 白皮书 | https://github.com/bnb-chain/greenfield-whitepaper | 设计理念和架构 |
| Go SDK | https://github.com/bnb-chain/greenfield-go-sdk | 官方 Go SDK |
| 链节点 | https://github.com/bnb-chain/greenfield | 区块链节点代码 |

---

## 核心架构

### 三层架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                         用户 / DApp                                 │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
        ▼                       ▼                       ▼
┌───────────────┐       ┌───────────────┐       ┌───────────────┐
│  Greenfield   │       │    Storage    │       │  BNB Smart    │
│  Blockchain   │◄─────►│   Provider    │       │    Chain      │
│   (元数据)     │       │   (数据存储)   │       │  (跨链互操作)  │
└───────────────┘       └───────────────┘       └───────────────┘
```

### 组件说明

| 组件 | 作用 | 交互方式 |
|------|------|---------|
| **Greenfield Blockchain** | 存储元数据（bucket、object 信息、权限） | REST/gRPC (链上交易) |
| **Storage Provider (SP)** | 存储实际文件数据 | HTTP (GNFD1-ECDSA 签名) |
| **BNB Smart Chain** | 智能合约和跨链操作 | 跨链桥 |

### 文档参考

关于架构的详细说明，请参考：
- **白皮书 Part 1**: https://github.com/bnb-chain/greenfield-whitepaper/blob/main/part1.md
  - Section 3: The Architecture in General
  - Section 4: BNB Greenfield Core
  - Section 5: The Greenfield Data Storage

---

## 操作依赖关系

### 核心概念层次

```
┌─────────────────────────────────────────────────────────────────────┐
│                        操作依赖关系图                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Account (账户)                                                    │
│       │                                                             │
│       ├──> CreateBucket (创建桶)                                    │
│       │         │                                                   │
│       │         └──> CreateObject (创建对象元数据)                   │
│       │                    │                                        │
│       │                    └──> PutObject (上传文件到 SP)            │
│       │                             │                               │
│       │                             └──> Object Sealed (SP 处理)    │
│       │                                                             │
│       ├──> CreateGroup (创建组)                                     │
│       │         │                                                   │
│       │         └──> UpdateGroupMember (添加/删除成员)               │
│       │                                                             │
│       └──> PutPolicy (设置权限策略)                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 关键依赖说明

| 操作 | 前置条件 | 说明 |
|------|---------|------|
| CreateBucket | 账户有足够 BNB | 需要支付存储费用 |
| CreateObject | Bucket 已存在 | Object 属于 Bucket |
| PutObject | Object 已 Create | 链上有元数据后才能上传 |
| GetObject | Object 已 Sealed | SP 完成存储确认后才能下载 |

### 文档参考

- **官方文档 - Core Concepts**: https://docs.bnbchain.org/bnb-greenfield/core-concepts/
  - Data Storage: 数据存储模型
  - Billing and Payment: 费用机制
  
- **白皮书 Part 1, Section 6**: Storage Economics and Its Primitives
  - 6.2 Data Object Creation
  - 6.3 Data Storage
  - 6.4 Data Read and Download

---

## 官方资源导航

### 文档站点

| 主题 | 链接 | 内容 |
|------|------|------|
| **入门** | https://docs.bnbchain.org/bnb-greenfield/get-started/ | 钱包配置、水龙头、基本操作 |
| **核心概念** | https://docs.bnbchain.org/bnb-greenfield/core-concepts/ | 账户、存储、支付、权限 |
| **开发指南** | https://docs.bnbchain.org/bnb-greenfield/for-developers/ | API、SDK、教程 |
| **网络端点** | https://docs.bnbchain.org/bnb-greenfield/for-developers/network-endpoint/ | 测试网/主网 RPC 地址 |
| **API 参考** | https://docs.bnbchain.org/bnb-greenfield/for-developers/apis-and-sdks/ | REST API、SDK 文档 |

### 白皮书章节

| 章节 | 链接 | 内容 |
|------|------|------|
| **Overview** | https://github.com/bnb-chain/greenfield-whitepaper/blob/main/overview.md | 项目概述 |
| **Part 1** | https://github.com/bnb-chain/greenfield-whitepaper/blob/main/part1.md | 设计与经济模型 |
| **Part 2** | https://github.com/bnb-chain/greenfield-whitepaper/blob/main/part2.md | 应用场景 |
| **Part 3** | https://github.com/bnb-chain/greenfield-whitepaper/blob/main/part3.md | 技术规格 |

### 白皮书 Part 3 重要章节

实现 SDK 时特别重要的技术规格：

| 章节 | 内容 |
|------|------|
| **17. Storage MetaData Models** | Bucket、Object、Group、Permission 数据模型 |
| **18. Payload Storage Management** | 分片、擦除码、数据冗余 |
| **20. Storage Transactions** | 存储相关的链上交易 |
| **23. SP APIs** | SP HTTP API 规范 |

---

## 源码仓库结构

### 仓库列表

| 仓库 | 说明 | 关键用途 |
|------|------|---------|
| [greenfield](https://github.com/bnb-chain/greenfield) | 区块链节点 | 消息定义、链上逻辑 |
| [greenfield-go-sdk](https://github.com/bnb-chain/greenfield-go-sdk) | Go SDK | API 封装、参考实现 |
| [greenfield-cosmos-sdk](https://github.com/bnb-chain/greenfield-cosmos-sdk) | Cosmos SDK 分支 | EIP-712 签名 |
| [greenfield-common](https://github.com/bnb-chain/greenfield-common) | 公共库 | SP 认证签名 |
| [greenfield-storage-provider](https://github.com/bnb-chain/greenfield-storage-provider) | SP 节点 | SP 端验证逻辑 |
| [greenfield-cmd](https://github.com/bnb-chain/greenfield-cmd) | 命令行工具 | 使用示例 |

### 代码追踪路径

#### 实现链上交易 (CreateBucket, CreateObject)

```
1. greenfield-go-sdk/client/api_*.go
   └── 找 SDK 入口函数

2. greenfield/x/storage/types/message.go
   └── 找 Proto 消息定义

3. greenfield-cosmos-sdk/x/auth/tx/eip712.go
   └── 找 EIP-712 签名逻辑

4. greenfield/x/storage/keeper/msg_server.go
   └── 找链上校验逻辑
```

#### 实现 SP 请求 (PutObject, GetObject)

```
1. greenfield-go-sdk/client/api_object.go
   └── 找 SDK 入口函数

2. greenfield-common/go/http/gen_sign_str.go
   └── 找 Canonical Request 构建

3. greenfield-storage-provider/modular/gater/
   └── 找 SP 端验证逻辑
```

### 关键文件速查

| 功能 | 仓库 | 文件路径 |
|------|------|---------|
| CreateBucket | greenfield-go-sdk | `client/api_bucket.go` |
| CreateObject | greenfield-go-sdk | `client/api_object.go` |
| PutObject | greenfield-go-sdk | `client/api_object.go` |
| ComputeHashRoots | greenfield-go-sdk | `client/api_object.go` |
| 消息定义 | greenfield | `x/storage/types/message.go` |
| Proto 文件 | greenfield | `proto/greenfield/storage/tx.proto` |
| EIP-712 签名 | greenfield-cosmos-sdk | `x/auth/tx/eip712.go` |
| Canonical Request | greenfield-common | `go/http/gen_sign_str.go` |
| SP 认证验证 | greenfield-storage-provider | `modular/gater/request_context.go` |

---

## 开发者快速入门路径

### 路径 1: 理解基本概念

1. 阅读官方文档入门：https://docs.bnbchain.org/bnb-greenfield/get-started/
2. 阅读白皮书 Overview：https://github.com/bnb-chain/greenfield-whitepaper/blob/main/overview.md
3. 理解核心概念：https://docs.bnbchain.org/bnb-greenfield/core-concepts/data-storage/

### 路径 2: 实现 SDK

1. **克隆参考仓库**
   ```bash
   git clone https://github.com/bnb-chain/greenfield-go-sdk
   git clone https://github.com/bnb-chain/greenfield
   git clone https://github.com/bnb-chain/greenfield-cosmos-sdk
   git clone https://github.com/bnb-chain/greenfield-common
   ```

2. **理解消息结构**
   - 查看 `greenfield/proto/greenfield/storage/tx.proto`
   - 查看 `greenfield/x/storage/types/message.go`

3. **理解签名机制**
   - EIP-712: `greenfield-cosmos-sdk/x/auth/tx/eip712.go`
   - GNFD1-ECDSA: `greenfield-common/go/http/gen_sign_str.go`

4. **参考 Go SDK 实现**
   - `greenfield-go-sdk/client/api_bucket.go`
   - `greenfield-go-sdk/client/api_object.go`

### 路径 3: 调试问题

1. **签名验证失败**
   - 对比 EIP-712 TypeHash
   - 对比 StructHash
   - 对比 Domain Separator
   - 检查字段排序

2. **SP 请求失败**
   - 对比 Canonical Request
   - 检查 Header 排序
   - 检查换行符

3. **使用调试工具**
   - 参考本仓库 `debug/` 目录下的调试脚本

---

## 网络端点

### 测试网 (Testnet)

| 服务 | 端点 |
|------|------|
| Greenfield RPC | https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org |
| Greenfield REST | https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org |
| Chain ID | greenfield_5600-1 |
| 区块浏览器 | https://testnet.greenfieldscan.com |
| 水龙头 | https://gnfd-bsc-faucet.bnbchain.org |

### 主网 (Mainnet)

| 服务 | 端点 |
|------|------|
| Greenfield RPC | https://greenfield-chain.bnbchain.org |
| Chain ID | greenfield_1017-1 |
| 区块浏览器 | https://greenfieldscan.com |

### SP 列表

可通过 API 查询：
```bash
curl https://gnfd-testnet-fullnode-tendermint-us.bnbchain.org/greenfield/sp/storage_providers
```

---

## 常见问题

### Q: Bucket 和 Object 有什么关系？

**A**: Bucket 是 Object 的容器，类似文件夹。每个 Object 必须属于一个 Bucket。Bucket 有自己的 Primary SP，其下所有 Object 数据都存储在该 SP。

### Q: 为什么需要先 CreateObject 再 PutObject？

**A**: 
- CreateObject 在链上创建元数据（名称、大小、checksums）
- PutObject 将实际数据上传到 SP
- SP 需要链上元数据来验证上传的合法性

### Q: 什么是 Global Virtual Group Family (VGF)？

**A**: VGF 是 SP 的逻辑分组，用于数据冗余和负载均衡。每个 Bucket 绑定一个 VGF，其下所有 Object 由该 VGF 的 SP 存储。

### Q: EIP-712 和 GNFD1-ECDSA 有什么区别？

**A**:
- **EIP-712**: 用于链上交易签名，对结构化数据签名
- **GNFD1-ECDSA**: 用于 SP HTTP 请求认证，对 Canonical Request 签名

---

## 参考链接汇总

### 官方资源
- 文档站: https://docs.bnbchain.org/bnb-greenfield/
- 白皮书: https://github.com/bnb-chain/greenfield-whitepaper
- Go SDK: https://github.com/bnb-chain/greenfield-go-sdk

### 技术规范
- EIP-712: https://eips.ethereum.org/EIPS/eip-712
- Reed-Solomon: https://en.wikipedia.org/wiki/Reed%E2%80%93Solomon_error_correction

### 工具
- 区块浏览器 (Testnet): https://testnet.greenfieldscan.com
- 区块浏览器 (Mainnet): https://greenfieldscan.com
- 水龙头: https://gnfd-bsc-faucet.bnbchain.org

