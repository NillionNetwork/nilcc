pub mod nilcc {
    pub mod agent {
        pub mod v1 {
            pub mod info {
                tonic::include_proto!("nilcc.agent.v1.info");
            }
            pub mod registration {
                tonic::include_proto!("nilcc.agent.v1.registration");
            }
        }
    }
}
