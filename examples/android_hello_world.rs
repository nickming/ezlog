use ezlog::{
    init, CipherKind, CompressKind, EZLogConfig, EZLogConfigBuilder, EZRecordBuilder,
    DEFAULT_LOG_NAME,
};

// #[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
#[allow(dead_code)]
fn main() {
    init();
    let log_config = get_config();

    ezlog::create_log(log_config);
    let record = EZRecordBuilder::new().content("12345".to_string()).build();
    ezlog::log(record);
    ezlog::flush(ezlog::DEFAULT_LOG_NAME);
    println!("end");
}

fn get_config() -> EZLogConfig {
    let key = b"an example very very secret key.";
    let nonce = b"unique nonce";
    EZLogConfigBuilder::new()
        .level(ezlog::Level::Trace)
        .dir_path("data/data/rust.example.android_hello_world/files/ezlog".to_string())
        .name(DEFAULT_LOG_NAME.to_string())
        .file_suffix(String::from("mmap"))
        .compress(CompressKind::ZLIB)
        .cipher(CipherKind::AES256GCM)
        .cipher_key(key.to_vec())
        .cipher_nonce(nonce.to_vec())
        .build()
}
