use nivasa_macros::Dto;

#[derive(Dto)]
struct WebhookConfig {
    #[is_url]
    callback_url: bool,
}

fn main() {}
