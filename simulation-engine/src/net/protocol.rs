// Include the generated protobuf code
pub mod autonomy {
    include!(concat!(env!("OUT_DIR"), "/autonomy.rs"));
}

// Re-export key types
pub use autonomy::*;
