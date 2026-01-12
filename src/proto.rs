pub mod greenfield {
    pub mod storage {
        tonic::include_proto!("greenfield.storage");
    }
    pub mod permission {
        tonic::include_proto!("greenfield.permission");
    }
    pub mod resource {
        tonic::include_proto!("greenfield.resource");
    }
    pub mod virtualgroup {
        tonic::include_proto!("greenfield.virtualgroup");
    }
    pub mod payment {
        tonic::include_proto!("greenfield.payment");
    }
    pub mod common {
        tonic::include_proto!("greenfield.common");
    }
}

pub mod cosmos {
    pub mod auth {
        pub mod v1beta1 {
            tonic::include_proto!("cosmos.auth.v1beta1");
        }
    }
    pub mod tx {
        pub mod v1beta1 {
            tonic::include_proto!("cosmos.tx.v1beta1");
        }
        pub mod signing {
            pub mod v1beta1 {
                tonic::include_proto!("cosmos.tx.signing.v1beta1");
            }
        }
    }
    pub mod base {
        pub mod v1beta1 {
            tonic::include_proto!("cosmos.base.v1beta1");
        }
        pub mod query {
            pub mod v1beta1 {
                tonic::include_proto!("cosmos.base.query.v1beta1");
            }
        }
        pub mod abci {
            pub mod v1beta1 {
                tonic::include_proto!("cosmos.base.abci.v1beta1");
            }
        }
    }
    pub mod crypto {
        pub mod multisig {
            pub mod v1beta1 {
                tonic::include_proto!("cosmos.crypto.multisig.v1beta1");
            }
        }
        pub mod secp256k1 {
            tonic::include_proto!("cosmos.crypto.secp256k1");
        }
        pub mod ethsecp256k1 {
            tonic::include_proto!("cosmos.crypto.ethsecp256k1");
        }
    }
}

pub mod ethermint {
    pub mod crypto {
        pub mod v1 {
            pub mod ethsecp256k1 {
                tonic::include_proto!("ethermint.crypto.v1.ethsecp256k1");
            }
        }
    }
    pub mod types {
        pub mod v1 {
            tonic::include_proto!("ethermint.types.v1");
        }
    }
}

pub mod tendermint {
    pub mod types {
        tonic::include_proto!("tendermint.types");
    }
    pub mod version {
        tonic::include_proto!("tendermint.version");
    }
    pub mod crypto {
        tonic::include_proto!("tendermint.crypto");
    }
    pub mod abci {
        tonic::include_proto!("tendermint.abci");
    }
}
