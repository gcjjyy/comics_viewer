use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Rtl,
    Ltr,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Rtl => "rtl",
            Direction::Ltr => "ltr",
        }
    }

    pub fn parse(s: &str) -> Option<Direction> {
        match s {
            "rtl" => Some(Direction::Rtl),
            "ltr" => Some(Direction::Ltr),
            _ => None,
        }
    }
}

/// The narrowest scope that has a setting wins.
pub fn resolve(global: Direction, library: Option<Direction>, folder: Option<Direction>, book: Option<Direction>) -> Direction {
    book.or(folder).or(library).unwrap_or(global)
}

#[cfg(test)]
mod tests {
    use super::Direction::*;
    use super::*;

    #[test]
    fn narrowest_scope_wins() {
        assert_eq!(resolve(Rtl, Some(Ltr), None, None), Ltr);
        assert_eq!(resolve(Rtl, Some(Ltr), Some(Rtl), None), Rtl);
        assert_eq!(resolve(Rtl, None, Some(Ltr), Some(Rtl)), Rtl);
        assert_eq!(resolve(Rtl, None, None, None), Rtl);
        assert_eq!(resolve(Ltr, None, None, None), Ltr);
    }
}
