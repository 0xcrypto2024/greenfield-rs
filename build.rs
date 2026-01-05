fn main() -> Result<(), Box<dyn std::error::Error>> {

    let protos = &[
        "deps/greenfield/proto/greenfield/storage/query.proto",
        "deps/greenfield/proto/greenfield/storage/tx.proto",
        "deps/cosmos-sdk/proto/cosmos/auth/v1beta1/query.proto",
        "deps/cosmos-sdk/proto/cosmos/tx/v1beta1/service.proto",
        "deps/cosmos-sdk/proto/cosmos/base/v1beta1/coin.proto",
        "deps/greenfield/proto/greenfield/permission/types.proto",
        "deps/greenfield/proto/greenfield/resource/types.proto",
        "deps/greenfield/proto/greenfield/virtualgroup/types.proto",
        "deps/greenfield/proto/greenfield/storage/common.proto",
        "deps/greenfield/proto/greenfield/storage/types.proto",
        "deps/greenfield/proto/greenfield/common/wrapper.proto",
        "deps/greenfield/proto/greenfield/common/approval.proto",
        "deps/greenfield/proto/greenfield/payment/out_flow.proto", // And stream_record?
        "deps/cosmos-sdk/proto/cosmos/base/abci/v1beta1/abci.proto",
        "deps/cosmos-sdk/proto/cosmos/crypto/multisig/v1beta1/multisig.proto",
        "deps/cosmos-sdk/proto/cosmos/crypto/secp256k1/keys.proto",
        "deps/cosmos-sdk/proto/cosmos/tx/signing/v1beta1/signing.proto",
        "deps/cosmos-sdk/proto/tendermint/types/types.proto",
        "deps/ethermint/proto/ethermint/types/v1/web3.proto",
        "deps/ethermint/proto/ethermint/crypto/v1/ethsecp256k1/keys.proto",
        "deps/cosmos-sdk/proto/tendermint/types/block.proto",
        "deps/cosmos-sdk/proto/tendermint/version/types.proto",
        "deps/cosmos-sdk/proto/tendermint/abci/types.proto",
    ];

    let includes = &[
        "deps/greenfield/proto",
        "deps/cosmos-sdk/proto",
        "deps/gogoproto",
        "deps/googleapis",
        "deps/cosmos-proto/proto",
        "deps/ethermint/proto",
    ];

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        // .out_dir("src/proto") // Optional: cleaner structure
        .compile(protos, includes)?;

    Ok(())
}
