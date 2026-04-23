use nivasa_macros::cron;

struct Jobs;

impl Jobs {
    #[cron(5000)]
    fn tick(&self) {}
}

fn main() {}
