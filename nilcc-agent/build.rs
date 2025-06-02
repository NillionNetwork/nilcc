use std::{
    env,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn run_command<const N: usize>(command: &str, args: [&str; N]) -> String {
    let output = Command::new(command).args(args).output().expect("failed to get run command");
    String::from_utf8(output.stdout).expect("invalid command output")
}

fn git_hash() -> String {
    run_command("git", ["rev-parse", "HEAD"])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hash = git_hash();
    let unix_now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    let proto_root_dir =
        env::current_dir().unwrap().join("../proto").canonicalize().expect("Failed to canonicalize proto root path");

    let include_paths = &[proto_root_dir.to_str().expect("Proto root path is not valid UTF-8")];

    let proto_files = &["nilcc/agent/v1/info.proto", "nilcc/agent/v1/registration.proto"];

    tonic_build::configure()
        .protoc_arg("--experimental_allow_proto3_optional")
        .build_client(true)
        .build_server(false)
        .compile_protos(proto_files, include_paths)?;

    println!("cargo:rustc-env=BUILD_GIT_COMMIT_HASH={hash}");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={unix_now}");
    println!("cargo:rustc-rerun-if-changed=.git/HEAD");

    Ok(())
}
