use nivasa_macros::cron;

struct Jobs;

impl Jobs {
    #[cron("")]
    fn tick(&self) {}
}

fn main() {}
