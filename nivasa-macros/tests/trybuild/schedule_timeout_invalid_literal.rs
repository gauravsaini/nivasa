use nivasa_macros::timeout;

struct Jobs;

impl Jobs {
    #[timeout("soon")]
    fn tick(&self) {}
}

fn main() {}
