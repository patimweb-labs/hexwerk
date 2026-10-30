//! KeyValues-Text (`.vdf`, `.acf`) gerade so weit lesen, wie Steam ihn nutzt.
//!
//! Zwei Formen sind im Umlauf, und beide muessen erkannt werden:
//!
//! ```text
//! "LibraryFolders" { "1" "/run/media/u/Games" }        // alt, flach
//! "libraryfolders" { "0" { "path" "/home/u/.steam" } } // neu, verschachtelt
//! ```
//!
//! Ein vollstaendiger KeyValues-Baum waere hier Overkill; gebraucht werden nur
//! Pfade. Der Tokenizer liefert Zeichenketten und Klammern, die Auswertung
//! pickt Paare aus zwei Zeichenketten heraus.

/// Ein Token aus einer KeyValues-Datei. Zahlen und nackte Woerter kommen in
/// Steam-Dateien nicht vor und werden deshalb verworfen.
#[derive(Debug, PartialEq, Eq)]
enum Token {
    Str(String),
    Open,
    Close,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(ch) = chars.next() {
                    match ch {
                        // In Valve-Dateien ist `\\` ein Pfadtrenner und `\"`
                        // eine gequotete Zeichenkette; beides muss aufgeloest
                        // werden, sonst bleibt ein Pfad mit Backslash stehen.
                        '\\' => {
                            if let Some(next) = chars.next() {
                                match next {
                                    'n' => s.push('\n'),
                                    't' => s.push('\t'),
                                    other => s.push(other),
                                }
                            }
                        }
                        '"' => break,
                        other => s.push(other),
                    }
                }
                out.push(Token::Str(s));
            }
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            // Zeilenenden und Einrueckung tragen hier nichts bei.
            _ => {}
        }
    }
    out
}

/// Paare aus Schluessel und Wert, in Dateireihenfolge.
fn pairs(text: &str) -> Vec<(String, String)> {
    let toks = tokens(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < toks.len() {
        if let (Token::Str(k), Token::Str(v)) = (&toks[i], &toks[i + 1]) {
            out.push((k.clone(), v.clone()));
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

/// Bibliothekspfade aus dem Inhalt einer `libraryfolders.vdf`.
///
/// Aufgenommen wird ein Paar, wenn der Schluessel `path` heisst (neue Form)
/// oder eine Zahl ist (alte Form) **und** der Wert absolut ist. Die
/// Absolutheitspruefung ist der Filter gegen die `apps`-Bloecke, in denen
/// ebenfalls Zahlen als Schluessel und Werte stehen (AppID → Groesse in Bytes).
pub(crate) fn library_paths(text: &str) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    for (key, value) in pairs(text) {
        let is_path_key = key.eq_ignore_ascii_case("path");
        let is_index_key = !key.is_empty() && key.chars().all(|c| c.is_ascii_digit());
        if !(is_path_key || is_index_key) {
            continue;
        }
        if !value.starts_with('/') {
            continue;
        }
        let p = std::path::PathBuf::from(value);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Erster Wert zu einem Schluessel, Gross-/Kleinschreibung egal (Steam schreibt
/// in `.acf` sowohl `name` als auch `Name`).
pub(crate) fn value_of(text: &str, wanted: &str) -> Option<String> {
    pairs(text)
        .into_iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(wanted))
        .map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn neue_form_liest_nur_das_pfadfeld() {
        let text = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
		"label"		""
		"apps"
		{
			"1664280"		"123456789"
		}
	}
}
"#;
        assert_eq!(
            library_paths(text),
            vec![PathBuf::from("/home/u/.local/share/Steam")]
        );
    }

    #[test]
    fn alte_form_liest_numerierte_schluessel() {
        let text = r#"
"LibraryFolders"
{
	"TimeNextStatsReport"	"1690000000"
	"ContentStatsID"		"123"
	"1"		"/run/media/u/Games"
	"2"		"/mnt/data/SteamLibrary"
}
"#;
        assert_eq!(
            library_paths(text),
            vec![
                PathBuf::from("/run/media/u/Games"),
                PathBuf::from("/mnt/data/SteamLibrary")
            ]
        );
    }

    #[test]
    fn app_bloecke_liefern_keine_pfade() {
        let text = r#"
"libraryfolders" { "0" { "apps" { "228980" "495" "1664280" "123" } } }
"#;
        assert!(library_paths(text).is_empty());
    }

    #[test]
    fn relative_pfade_werden_verworfen() {
        let text = r#""libraryfolders" { "0" { "path" "steamapps" } }"#;
        assert!(library_paths(text).is_empty());
    }

    #[test]
    fn escapes_werden_aufgeloest() {
        // Valve schreibt einen Backslash als `\\`; ohne Aufloesung stuende im
        // Pfad ein doppelter Trenner und die Bibliothek waere nicht auffindbar.
        let text = r#""libraryfolders" { "0" { "path" "/home/u/a\\b \"c\"" } }"#;
        assert_eq!(
            library_paths(text),
            vec![PathBuf::from("/home/u/a\\b \"c\"")]
        );
    }

    #[test]
    fn doppelte_pfade_bleiben_einmal() {
        let text = r#""a" { "path" "/x" } "b" { "path" "/x" }"#;
        assert_eq!(library_paths(text), vec![PathBuf::from("/x")]);
    }

    #[test]
    fn wert_ignoriert_gross_klein() {
        let text = "appmanifest\n{\n\t\"AppState\"\n\t{\n\t\t\"appid\"\t\t\"1664280\"\n\t\t\"Name\"\t\t\"World of Warcraft\"\n\t}\n}";
        assert_eq!(value_of(text, "name").as_deref(), Some("World of Warcraft"));
        assert_eq!(value_of(text, "appid").as_deref(), Some("1664280"));
        assert_eq!(value_of(text, "fehlt"), None);
    }
}
