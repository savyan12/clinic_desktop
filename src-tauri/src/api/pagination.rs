use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PageQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl PageQuery {
    pub fn page(&self) -> u64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> u64 {
        self.per_page.unwrap_or(15).clamp(1, 100)
    }

    pub fn offset(&self) -> u64 {
        (self.page() - 1) * self.per_page()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PageMeta {
    pub current_page: u64,
    pub last_page: u64,
    pub per_page: u64,
    pub total: u64,
}

pub fn meta(page: u64, per_page: u64, total: u64) -> PageMeta {
    PageMeta {
        current_page: page,
        last_page: total.div_ceil(per_page).max(1),
        per_page,
        total,
    }
}
