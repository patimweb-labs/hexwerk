//! HTTP hinter einem Trait.
//!
//! WARUM eigener Trait: `cargo test` darf niemals ins Netz (harte Regel aus dem
//! Plan). Die Quellen kennen nur [`Http`]; im Betrieb steckt [`UreqHttp`] dahinter,
//! in Tests ein Fixture-Client aus `tests/fixtures/`. Damit ist jede Quelle ohne
//! Attrappe und ohne Netz vollständig prüfbar.
//!
//! Zweite Regel, die hier sitzt: **kein Schlüssel im Klartext**. [`Request`] hat
//! ein eigenes `Debug`, das den `authorization`-Header maskiert — sonst landet ein
//! Token beim ersten `dbg!` oder in einer Panikmeldung im Log.

use std::fmt;
use std::time::Duration;

use crate::SourceError;

/// Obergrenze je Antwort. Die WoWInterface-Gesamtliste ist 2,7 MB; 64 MB lässt
/// Luft für ZIP-Downloads, ohne dass ein kaputter Server den Speicher füllt.
const BODY_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// Eine Anfrage. Header als Paare statt Map, damit die Reihenfolge stabil bleibt
/// (Tests vergleichen) und mehrere Header desselben Namens möglich sind.
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self {
            method: Method::Post,
            url: url.into(),
            body: None,
            headers: Vec::new(),
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Setzt `authorization: Bearer …`. Der Wert kommt als `&str`, damit der
    /// Aufrufer ihn aus einem [`crate::keys::Secret`] holen muss — also bewusst
    /// `expose()` schreibt und nicht versehentlich eine Struktur Debug-gt.
    pub fn bearer(self, token: &str) -> Self {
        self.header("authorization", format!("Bearer {token}"))
    }

    pub fn json_body(mut self, value: &serde_json::Value) -> Self {
        self.headers
            .push(("content-type".into(), "application/json".into()));
        self.body = Some(serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec()));
        self
    }

    /// Nur für Fehlermeldungen gedacht: die URL ohne Query, weil Query-Parameter
    /// bei CurseForge/Wago Schlüssel enthalten können.
    pub fn url_without_query(&self) -> String {
        match self.url.split_once('?') {
            Some((base, _)) => base.to_string(),
            None => self.url.clone(),
        }
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(String, String)> = self
            .headers
            .iter()
            .map(|(k, v)| {
                if k.eq_ignore_ascii_case("authorization") || k.eq_ignore_ascii_case("x-api-key") {
                    (k.clone(), "***".to_string())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url_without_query())
            .field("headers", &headers)
            .field("body_len", &self.body.as_ref().map(|b| b.len()))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

impl HttpResponse {
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            body,
            content_type: None,
        }
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Bildet den HTTP-Code auf unseren Fehlertyp ab. `404` und `429` bekommen
    /// eigene Varianten, weil die UI sie unterschiedlich erklärt („gibt es nicht"
    /// gegen „später nochmal").
    pub fn error(&self, url: &str) -> Option<SourceError> {
        match self.status {
            s if (200..300).contains(&s) => None,
            404 => Some(SourceError::NotFound),
            429 => Some(SourceError::RateLimited),
            s => Some(SourceError::Http {
                status: s,
                url: url.to_string(),
            }),
        }
    }

    /// JSON lesen und dabei prüfen, ob die Antwort überhaupt JSON ist.
    ///
    /// WARUM die Content-Type-Prüfung: Wago antwortet ohne gültigen Token mit
    /// einer 302 auf die Website; ureq folgt dem Redirect und liefert HTML mit
    /// Status 200. Ohne diese Prüfung stünde im Log „expected value at line 1",
    /// mit ihr steht dort, was wirklich los ist.
    pub fn json(&self, url: &str) -> Result<serde_json::Value, SourceError> {
        if let Some(err) = self.error(url) {
            return Err(err);
        }
        if let Some(ct) = &self.content_type {
            let ct = ct.to_ascii_lowercase();
            if ct.contains("text/html") || ct.contains("text/plain") {
                return Err(SourceError::Parse(format!(
                    "erwartet JSON, bekommen {ct} (Schlüssel ungültig oder abgelaufen?)"
                )));
            }
        }
        serde_json::from_slice(&self.body).map_err(|e| SourceError::Parse(e.to_string()))
    }
}

/// Der Netzzugriff, den die Quellen benutzen.
pub trait Http: Send + Sync {
    fn request(&self, request: &Request) -> Result<HttpResponse, SourceError>;

    fn get(&self, url: &str) -> Result<HttpResponse, SourceError> {
        self.request(&Request::get(url))
    }
}

/// Echter Client. Wird erst im Betrieb gebaut, nie in Tests.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            // Ohne Timeout hängt der Hintergrundthread an einem toten CDN fest
            // und die UI zeigt ewig „lädt".
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("hexwerk/", env!("CARGO_PKG_VERSION")))
            // Statuscodes selbst auswerten: `403` von CurseForge ohne Schlüssel
            // und `429` von GitHub sollen unterscheidbar bleiben.
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for UreqHttp {
    fn request(&self, request: &Request) -> Result<HttpResponse, SourceError> {
        // GET und POST haben bei ureq verschiedene Builder-Typen
        // (`WithoutBody` gegen `WithBody`), deshalb zwei Zweige statt einer
        // gemeinsamen Schleife.
        let result = match request.method {
            Method::Get => {
                let mut builder = self.agent.get(&request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                builder.call()
            }
            Method::Post => {
                let mut builder = self.agent.post(&request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                let body = request.body.as_deref().unwrap_or(&[]);
                builder.send(body)
            }
        };
        let mut response = match result {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(code)) => {
                return Err(SourceError::Http {
                    status: code,
                    url: request.url_without_query(),
                })
            }
            Err(e) => return Err(map_ureq_error(&e, &request.url_without_query())),
        };
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let body = response
            .body_mut()
            .with_config()
            .limit(BODY_LIMIT)
            .read_to_vec()
            .map_err(|e| map_ureq_error(&e, &request.url_without_query()))?;
        Ok(HttpResponse {
            status,
            body,
            content_type,
        })
    }
}

fn map_ureq_error(error: &ureq::Error, url: &str) -> SourceError {
    match error {
        ureq::Error::StatusCode(code) => SourceError::Http {
            status: *code,
            url: url.to_string(),
        },
        ureq::Error::HostNotFound => SourceError::Network(format!("Host nicht auflösbar: {url}")),
        other => SourceError::Network(other.to_string()),
    }
}

/// Prozentkodierung für Query-Werte (RFC 3986, unreserved bleiben).
///
/// WARUM von Hand: eine Abhängigkeit nur für `urllib`-Ersatz wäre teurer als
/// zwölf Zeilen, und gesucht wird ohnehin nur ASCII.
pub fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_verbirgt_den_token() {
        let req = Request::get("https://api.curseforge.com/v1/mods/search?x=1")
            .bearer("super-geheim-1234")
            .header("x-api-key", "auch-geheim");
        let text = format!("{req:?}");
        assert!(!text.contains("super-geheim"));
        assert!(!text.contains("auch-geheim"));
        assert!(text.contains("***"));
        // Query-Parameter fliegen raus, weil dort Schlüssel stehen können.
        assert!(!text.contains("x=1"));
    }

    #[test]
    fn statusabbildung() {
        assert!(HttpResponse::new(200, vec![]).error("u").is_none());
        assert_eq!(
            HttpResponse::new(404, vec![]).error("u"),
            Some(SourceError::NotFound)
        );
        assert_eq!(
            HttpResponse::new(429, vec![]).error("u"),
            Some(SourceError::RateLimited)
        );
        assert_eq!(
            HttpResponse::new(403, vec![]).error("u"),
            Some(SourceError::Http {
                status: 403,
                url: "u".into()
            })
        );
    }

    #[test]
    fn html_statt_json_wird_erklaert() {
        let resp = HttpResponse::new(200, b"<html>".to_vec()).with_content_type("text/html");
        let err = resp.json("https://addons.wago.io/api/addons").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("Schlüssel"), "{text}");
    }

    #[test]
    fn urlencode_haelt_query_heil() {
        assert_eq!(urlencode("weak auras & co"), "weak+auras+%26+co");
    }
}
