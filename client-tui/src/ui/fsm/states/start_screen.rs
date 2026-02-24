#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct StartScreenState {
    pub selected_index: usize,
}

#[allow(dead_code)]
impl StartScreenState {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }
}
