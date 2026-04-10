use nivasa_config::{ConfigModule, ConfigOptions, ConfigService};

fn main() {
    let options = ConfigOptions::new()
        .with_env_file_path(".env")
        .with_expand_variables(true);

    let loaded = ConfigModule::load_env(&options).expect("config env should load");
    let service = ConfigService::from_values(loaded);

    println!("app_name={}", service.get_or_throw("APP_NAME").unwrap());
    println!("app_port={}", service.get::<u16>("APP_PORT").unwrap_or(3000));
}
