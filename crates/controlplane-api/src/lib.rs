pub mod controlplane {
    pub mod v1 {
        tonic::include_proto!("llmos.controlplane.v1");
    }
}

pub use controlplane::v1::*;
