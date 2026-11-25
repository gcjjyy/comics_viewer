use std::cmp::Ordering;

/// Natural ordering: digit runs compare by numeric value, other characters case-insensitively.
pub fn cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    let mut padding_tiebreak = Ordering::Equal;
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let (n, m) = (take_digits(&mut x), take_digits(&mut y));
                let (nt, mt) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let ord = nt.len().cmp(&mt.len()).then_with(|| nt.cmp(mt));
                if ord != Ordering::Equal {
                    return ord;
                }
                if padding_tiebreak == Ordering::Equal {
                    padding_tiebreak = m.len().cmp(&n.len());
                }
            }
            (Some(c), Some(d)) => {
                let ord = c.to_lowercase().cmp(d.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                x.next();
                y.next();
            }
        }
    }
    padding_tiebreak.then_with(|| a.cmp(b))
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        s.push(c);
        it.next();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted<const N: usize>(items: [&str; N]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|s| s.to_string()).collect();
        v.sort_by(|a, b| cmp(a, b));
        v
    }

    #[test]
    fn numbers_compare_by_value() {
        assert_eq!(
            sorted(["생존게임 10.zip", "생존게임 2.zip", "생존게임 01.zip"]),
            ["생존게임 01.zip", "생존게임 2.zip", "생존게임 10.zip"]
        );
    }

    #[test]
    fn numbers_inside_parentheses() {
        assert_eq!(sorted(["킹덤 (KINGDIM)  (530)", "킹덤 (KINGDIM)  (526)"])[0], "킹덤 (KINGDIM)  (526)");
    }

    #[test]
    fn suffix_after_number_does_not_change_order() {
        assert_eq!(sorted(["생존게임 21(완).zip", "생존게임 20.zip"])[0], "생존게임 20.zip");
    }

    #[test]
    fn equal_value_longer_zero_padding_first() {
        assert_eq!(cmp("a01", "a1"), Ordering::Less);
        assert_eq!(cmp("a1", "a01"), Ordering::Greater);
    }

    #[test]
    fn case_insensitive_text() {
        assert_eq!(cmp("abc 2", "ABC 10"), Ordering::Less);
    }
}
