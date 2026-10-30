//! Versionsvergleich fuer die Update-Erkennung.
//!
//! Addon-Versionen kommen als `5.19.2`, `v3.4.1`, `10.2.7-release` oder
//! `1.0.0b` aus vier Quellen. Strenge Semver-Regeln wuerden hier fast alles
//! ablehnen (viele Addons zaehlen nicht dreiteilig), ein reiner
//! Zeichenvergleich ordnet `5.19.10` unter `5.19.2` ein. Deshalb: Zahlen als
//! Zahlen, Buchstaben als Text, fehlende Stellen als 0.

use std::cmp::Ordering;

/// Ein Teilstueck einer Version: Zahl oder Wort.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Num(u64),
    Text(String),
}

/// Vergleicht zwei Versionszeichenketten.
///
/// Regeln, die ausreichen und erklaerbar bleiben:
/// - ein fuehrendes `v`/`V` vor einer Ziffer ist Dekoration,
/// - Ziffernfolgen werden numerisch verglichen (`5.19.10` > `5.19.2`),
/// - fehlende Stellen zaehlen als 0 (`1.2` == `1.2.0`),
/// - ein Zahlenrest schlaegt einen Buchstabenrest (`1.0` > `1.0-beta`),
///   ein Buchstabenrest am Ende gilt als Vorabversion (`1.0.0b` < `1.0.0`).
pub fn compare(a: &str, b: &str) -> Ordering {
    let left = tokens(strip_prefix(a));
    let right = tokens(strip_prefix(b));
    let len = left.len().max(right.len());
    for i in 0..len {
        let ord = match (left.get(i), right.get(i)) {
            (None, None) => Ordering::Equal,
            (Some(Token::Num(x)), Some(Token::Num(y))) => x.cmp(y),
            (Some(Token::Text(x)), Some(Token::Text(y))) => x.cmp(y),
            // Vorabversion: `1.0-beta` steht unter `1.0`.
            (Some(Token::Num(_)), Some(Token::Text(_))) => Ordering::Greater,
            (Some(Token::Text(_)), Some(Token::Num(_))) => Ordering::Less,
            (Some(Token::Num(x)), None) => {
                if *x == 0 {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
            (None, Some(Token::Num(y))) => {
                if *y == 0 {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            }
            (Some(Token::Text(_)), None) => Ordering::Less,
            (None, Some(Token::Text(_))) => Ordering::Greater,
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// `v5.19.2` und `5.19.2` sind dieselbe Version; GitHub-Tags tragen das `v`.
fn strip_prefix(text: &str) -> &str {
    let trimmed = text.trim();
    let rest = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'));
    match rest {
        Some(r) if r.starts_with(|c: char| c.is_ascii_digit()) => r,
        _ => trimmed,
    }
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            // Satten statt Ueberlauf: eine 40-stellige Zahl ist keine Version,
            // aber sie darf den Vergleich nicht abstuerzen lassen.
            let mut n: u64 = c.to_digit(10).unwrap_or(0) as u64;
            while let Some(d) = chars.peek().and_then(|c| c.to_digit(10)) {
                n = n.saturating_mul(10).saturating_add(u64::from(d));
                chars.next();
            }
            out.push(Token::Num(n));
        } else if c.is_alphanumeric() {
            let mut word = String::new();
            word.extend(c.to_lowercase());
            while let Some(next) = chars.peek().copied() {
                if next.is_alphanumeric() {
                    word.extend(next.to_lowercase());
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Token::Text(word));
        }
        // Trennzeichen (`.`, `-`, `_`, `+`, Leerzeichen) trennen nur.
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compare;
    use std::cmp::Ordering;

    #[test]
    fn zahlen_werden_numerisch_verglichen() {
        assert_eq!(compare("5.19.2", "5.19.10"), Ordering::Less);
        assert_eq!(compare("10.0", "9.9"), Ordering::Greater);
        assert_eq!(compare("1.2.3", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn v_praefix_und_fehlende_stellen_zaehlen_nicht() {
        assert_eq!(compare("v5.19.2", "5.19.2"), Ordering::Equal);
        assert_eq!(compare("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare(" 3.4.1 ", "3.4.1"), Ordering::Equal);
    }

    #[test]
    fn vorabversionen_stehen_unter_dem_release() {
        assert_eq!(compare("1.0.0-beta", "1.0.0"), Ordering::Less);
        assert_eq!(compare("1.0.0", "1.0.0-beta"), Ordering::Greater);
        assert_eq!(compare("1.0.0b", "1.0.0"), Ordering::Less);
        // Wort gegen Wort: alphabetisch, wie der Mensch es liest.
        assert_eq!(compare("1.0.0-alpha", "1.0.0-beta"), Ordering::Less);
    }

    #[test]
    fn unsinn_kippt_nicht() {
        assert_eq!(compare("", ""), Ordering::Equal);
        assert_eq!(compare("", "1.0"), Ordering::Less);
        assert_eq!(compare("unbekannt", "1.0"), Ordering::Less);
        // Sehr lange Ziffernfolge: gesaettigt, kein Ueberlauf.
        let huge = "9".repeat(40);
        assert_eq!(compare(&huge, &huge), Ordering::Equal);
    }
}
