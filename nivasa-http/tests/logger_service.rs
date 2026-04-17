use nivasa_core::module::ConfigurableModule;
use nivasa_http::{LogContext, LoggerModule, LoggerOptions, LoggerService};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

struct BufferWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut buffer = self.buffer.lock().expect("buffer lock");
        buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn logger_module_registers_logger_service_and_global_flag() {
    let module = LoggerModule::for_root(
        LoggerOptions::new()
            .with_global(true)
            .with_default_level("warn")
            .with_module_level("payments", "info"),
    );

    assert!(module.metadata.is_global);
    let merged = module.merged_metadata();
    assert!(merged
        .exports
        .contains(&std::any::TypeId::of::<LoggerService>()));
    assert!(merged
        .providers
        .contains(&std::any::TypeId::of::<LoggerService>()));

    let feature_module =
        <LoggerModule as ConfigurableModule>::for_feature(LoggerOptions::new().with_json());
    assert!(!feature_module.metadata.is_global);
}

#[test]
fn logger_service_emits_json_logs_with_context_fields() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let service = LoggerService::new(LoggerOptions::new().with_json());
    let context = LogContext::new()
        .with_request_id("req-1")
        .with_user_id("user-7")
        .with_module_name("users");

    service
        .with_default_subscriber(
            {
                let buffer = Arc::clone(&buffer);
                move || BufferWriter {
                    buffer: Arc::clone(&buffer),
                }
            },
            || {
                service.info(&context, "json log");
            },
        )
        .expect("json subscriber should install");

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf-8 logs");
    assert!(logs.contains("\"message\":\"json log\""));
    assert!(logs.contains("\"request_id\":\"req-1\""));
    assert!(logs.contains("\"user_id\":\"user-7\""));
    assert!(logs.contains("\"module_name\":\"users\""));
}

#[test]
fn logger_service_honors_json_module_levels_over_default_level() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let service = LoggerService::new(
        LoggerOptions::new()
            .with_json()
            .with_default_level("warn")
            .with_module_level("payments", "info"),
    );
    let context = LogContext::new().with_module_name("payments");

    service
        .with_default_subscriber(
            {
                let buffer = Arc::clone(&buffer);
                move || BufferWriter {
                    buffer: Arc::clone(&buffer),
                }
            },
            || {
                service.info_with_target("payments", &context, "kept");
                service.info_with_target("inventory", &LogContext::new(), "dropped");
            },
        )
        .expect("json subscriber should install");

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf-8 logs");
    assert!(logs.contains("\"message\":\"kept\""));
    assert!(logs.contains("\"module_name\":\"payments\""));
    assert!(!logs.contains("dropped"));
}

#[test]
fn logger_service_emits_pretty_logs_and_respects_module_filters() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let service = LoggerService::new(
        LoggerOptions::new()
            .with_pretty()
            .with_default_level("warn")
            .with_module_level("payments", "info"),
    );
    let context = LogContext::new().with_module_name("payments");

    service
        .with_default_subscriber(
            {
                let buffer = Arc::clone(&buffer);
                move || BufferWriter {
                    buffer: Arc::clone(&buffer),
                }
            },
            || {
                service.info_with_target("payments", &context, "kept");
                service.info_with_target("inventory", &LogContext::new(), "dropped");
            },
        )
        .expect("pretty subscriber should install");

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf-8 logs");
    assert!(logs.contains("kept"));
    assert!(logs.contains("payments"));
    assert!(!logs.contains("\"message\""));
    assert!(!logs.contains("dropped"));
}

#[test]
fn logger_service_rejects_invalid_directives() {
    let service = LoggerService::new(
        LoggerOptions::new().with_module_level("payments", "definitely-not-a-level"),
    );

    let error = service
        .env_filter()
        .expect_err("invalid directive should fail");
    assert!(error
        .to_string()
        .contains("payments=definitely-not-a-level"));
}

#[test]
fn logger_service_exposes_options_and_builds_env_filter() {
    let service = LoggerService::new(
        LoggerOptions::new()
            .with_json()
            .with_global(true)
            .with_default_level("debug"),
    );
    let options = service.options();

    assert!(options.is_global);
    assert!(matches!(options.format, nivasa_http::LoggerFormat::Json));
    assert!(service.env_filter().is_ok());
}
