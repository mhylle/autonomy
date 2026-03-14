fn main() {
    // Use protox (pure-Rust protobuf compiler) so we don't need system protoc or cmake
    let proto_files = &[
        "../shared/proto/world.proto",
        "../shared/proto/events.proto",
        "../shared/proto/commands.proto",
    ];
    let includes = &["../shared/proto/"];

    let file_descriptors = protox::compile(proto_files, includes)
        .expect("Failed to compile protobuf schemas with protox");

    prost_build::compile_fds(file_descriptors)
        .expect("Failed to generate Rust code from protobuf descriptors");
}
