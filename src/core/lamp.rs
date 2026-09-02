#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedInfo {
    pub id: usize,
    pub zone_id: usize,
}
