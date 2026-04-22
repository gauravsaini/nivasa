use nivasa_macros::interval;

struct Jobs;

impl Jobs {
    #[interval(0)]
    fn tick(&self) {}
}

fn main() {}
