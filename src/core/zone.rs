#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    Argb,
    Rgb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneProfile {
    pub name: String,
    pub kind: ZoneKind,
    pub capacity: usize,
    pub active_led_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub id: usize,
    pub name: String,
    pub kind: ZoneKind,
    pub offset: usize,
    pub capacity: usize,
    pub active_led_count: usize,
}
