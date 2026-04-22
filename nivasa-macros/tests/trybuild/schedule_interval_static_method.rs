use nivasa_macros::interval;

struct Jobs;

impl Jobs {
    #[interval(5000)]
    fn tick() {}
}

fn main() {}
