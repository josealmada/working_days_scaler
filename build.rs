fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("PROTOC", "C:\\protoc\\bin\\protoc");
    tonic_build::compile_protos("proto/externalscaler.proto")?;
    Ok(())
}
