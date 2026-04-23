use nivasa_macros::interval;

struct Jobs;

impl Jobs {
    #[interval(-1)]
    fn tick(&self) {}
}

fn main() {}
