use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct WebhookConfig {
    #[is_url]
    callback_url: String,
}

fn main() {
    let config = WebhookConfig {
        callback_url: "https://example.com/webhook".into(),
    };

    let _ = config.validate();
}
