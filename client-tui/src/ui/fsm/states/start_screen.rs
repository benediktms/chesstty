#[derive(Clone, Debug, Default)]
pub struct StartScreenState {
    pub selected_index: usize,
}

impl StartScreenState {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }
}
