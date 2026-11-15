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

/// Obergrenze je gewoehnlicher Antwort (JSON, Bilder). Die WoWInterface-
/// Gesamtliste ist 2,7 MB; 256 MB sind dafuer reichlich.
const BODY_LIMIT: u64 = 256 * 1024 * 1024;

/// Obergrenze je Datei-Download. Addon-ZIPs gehen deutlich ueber 64 MB
/// (gemessen: `spoken-wow` 68,2 MB). Die Grenze schuetzt nur davor, dass ein
/// kaputter Server den Speicher fuellt - ein echtes Addon kommt nie in ihre Naehe.
pub const DOWNLOAD_LIMIT: u64 = 1024 * 1024 * 1024;

/// Wie lange ein Download ohne ein einziges neues Byte haengen darf.
///
/// WARUM keine Gesamtzeit: jede Gesamtzeit ist eine versteckte Groessengrenze
/// (300 s reichen bei 1 MB/s fuer 300 MB, bei 200 KB/s nur fuer 60 MB). Ein
/// Download, der Daten liefert, darf so lange laufen, wie er braucht; einer,
/// der steht, faellt nach dieser Zeit auf.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);

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

    /// Wie [`Http::request`], meldet aber den Fortschritt beim Lesen des Rumpfes.
    ///
    /// `done` sind die bisher gelesenen Bytes, `total` die vom Server genannte
    /// Gesamtgroesse (`content-length`), sofern er sie nennt. Kleine Antworten
    /// (JSON-Suche) gehen den Vorgabeweg und melden einmal am Ende - fuer den
    /// Fortschrittsbalken ist nur ein ZIP-Download interessant, und den
    /// ueberschreibt [`UreqHttp`] mit einem stueckweisen Lesen.
    fn download(
        &self,
        request: &Request,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<HttpResponse, SourceError> {
        let response = self.request(request)?;
        progress(response.body.len() as u64, Some(response.body.len() as u64));
        Ok(response)
    }
}

/// Echter Client. Wird erst im Betrieb gebaut, nie in Tests.
pub struct UreqHttp {
    agent: ureq::Agent,
    /// Zweiter Agent nur für Datei-Downloads: 30 s Gesamtzeit sind für JSON
    /// richtig (ein toter Server soll schnell auffallen), für 68 MB über eine
    /// langsame Leitung aber zu knapp. Der Download-Agent hat deshalb keine
    /// Gesamtzeit, nur [`STALL_TIMEOUT`] je Lesevorgang.
    download_agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        Self {
            agent: build_agent(Some(Duration::from_secs(30)), None),
            download_agent: build_agent(None, Some(STALL_TIMEOUT)),
        }
    }

    /// Baut die Anfrage und schickt sie ab - gemeinsam für [`Http::request`] und
    /// [`Http::download`], damit Header und Fehlerabbildung nur an einer Stelle
    /// stehen.
    fn call(
        &self,
        agent: &ureq::Agent,
        request: &Request,
    ) -> Result<ureq::http::Response<ureq::Body>, SourceError> {
        // GET und POST haben bei ureq verschiedene Builder-Typen
        // (`WithoutBody` gegen `WithBody`), deshalb zwei Zweige statt einer
        // gemeinsamen Schleife.
        let result = match request.method {
            Method::Get => {
                let mut builder = agent.get(&request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                builder.call()
            }
            Method::Post => {
                let mut builder = agent.post(&request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                let body = request.body.as_deref().unwrap_or(&[]);
                builder.send(body)
            }
        };
        match result {
            Ok(response) => Ok(response),
            Err(ureq::Error::StatusCode(code)) => Err(SourceError::Http {
                status: code,
                url: request.url_without_query(),
            }),
            Err(e) => Err(map_ureq_error(&e, &request.url_without_query())),
        }
    }
}

/// Ein Agent mit eigener Kennung.
///
/// `global` begrenzt die ganze Anfrage, `stall` nur die Zeit ohne neue Daten.
/// Verbindungsaufbau und Antwortkopf sind in beiden Faellen begrenzt.
fn build_agent(global: Option<Duration>, stall: Option<Duration>) -> ureq::Agent {
    let handshake = Duration::from_secs(30);
    ureq::Agent::config_builder()
        // Ohne Timeout hängt der Hintergrundthread an einem toten CDN fest und
        // die Oberflaeche zeigt ewig „lädt".
        .timeout_global(global)
        .timeout_resolve(Some(handshake))
        .timeout_connect(Some(handshake))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_per_read(stall)
        .user_agent(concat!("hexwerk/", env!("CARGO_PKG_VERSION")))
        // Statuscodes selbst auswerten: `403` von CurseForge ohne Schlüssel und
        // `429` von GitHub sollen unterscheidbar bleiben.
        .http_status_as_error(false)
        .build()
        .into()
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for UreqHttp {
    fn request(&self, request: &Request) -> Result<HttpResponse, SourceError> {
        let mut response = self.call(&self.agent, request)?;
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

    /// Liest den Rumpf stueckweise und meldet nach jedem Block.
    ///
    /// WARUM stueckweise: erst damit gibt es einen Fortschrittsbalken, der
    /// waehrend des Downloads laeuft und nicht erst beim Abschluss springt.
    /// 64 KiB je Meldung sind klein genug fuer eine fluessige Anzeige und gross
    /// genug, dass die Meldungen keinen Thread-Wechsel bremsen.
    fn download(
        &self,
        request: &Request,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<HttpResponse, SourceError> {
        let mut response = self.call(&self.download_agent, request)?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let encoded = response.headers().contains_key("content-encoding");
        // Bei `content-encoding` (gzip) nennt `content-length` die gepackte
        // Groesse, ureq liefert aber entpackt - dann taugt sie weder fuer den
        // Balken noch fuer die Vollstaendigkeitspruefung.
        let total = if encoded {
            None
        } else {
            response
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
        };
        if let Some(total) = total {
            if total > DOWNLOAD_LIMIT {
                return Err(SourceError::TooLarge {
                    bytes: total,
                    limit: DOWNLOAD_LIMIT,
                });
            }
        }
        // Die Grenze zaehlt diese Schleife selbst, deshalb bekommt ureq keine:
        // so wird aus Ueberlaenge ein eigener Fehler und kein „io error".
        let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
        let body = read_blocks(&mut reader, total, DOWNLOAD_LIMIT, progress)?;
        Ok(HttpResponse {
            status,
            body,
            content_type,
        })
    }
}

/// Liest einen Rumpf in Bloecken, meldet den Fortschritt und prueft Grenze
/// und Vollstaendigkeit.
///
/// Eigene Funktion ueber `Read`, damit Abbruch, Ueberlaenge und kurze
/// Antworten ohne Netz pruefbar sind.
fn read_blocks(
    reader: &mut dyn std::io::Read,
    total: Option<u64>,
    limit: u64,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Vec<u8>, SourceError> {
    // Vorab reservieren spart das Umkopieren beim Wachsen, aber hoechstens
    // 64 MiB: sonst bekaeme ein Server, der 1 GiB ankuendigt und nichts
    // schickt, sofort 1 GiB Speicher.
    let capacity = total.unwrap_or(0).min(limit).min(64 * 1024 * 1024) as usize;
    let mut body = Vec::with_capacity(capacity);
    let mut block = vec![0u8; 64 * 1024];
    loop {
        let read = match reader.read(&mut block) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(SourceError::Network(format!(
                    "Download abgebrochen nach {} Bytes: {e}",
                    body.len()
                )))
            }
        };
        if read == 0 {
            break;
        }
        if body.len() as u64 + read as u64 > limit {
            return Err(SourceError::TooLarge {
                bytes: body.len() as u64 + read as u64,
                limit,
            });
        }
        body.extend_from_slice(&block[..read]);
        progress(body.len() as u64, total);
    }
    if let Some(total) = total {
        if body.len() as u64 != total {
            return Err(SourceError::Network(format!(
                "Download unvollständig: {} von {total} Bytes",
                body.len()
            )));
        }
    }
    Ok(body)
}

/// Wie oft und mit welcher Pause ein Download wiederholt wird.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    /// Versuche insgesamt, mindestens einer.
    pub attempts: u32,
    /// Pause vor dem zweiten Versuch; jede weitere verdoppelt sich.
    pub delay: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            attempts: 3,
            delay: Duration::from_secs(1),
        }
    }
}

/// Ist dieser Fehler einen zweiten Versuch wert?
///
/// Ja bei Netzfehlern (Abbruch, Zeitueberschreitung, unvollstaendig) und bei
/// Serverfehlern (5xx, 408). Nein bei allem, was beim naechsten Mal genauso
/// kommt: 404, 403, zu gross, gedrosselt (429 wird nicht durch Haemmern besser).
pub fn is_transient(error: &SourceError) -> bool {
    match error {
        SourceError::Network(_) => true,
        SourceError::Http { status, .. } => *status >= 500 || *status == 408,
        _ => false,
    }
}

/// Laedt mit [`Http::download`] und wiederholt bei voruebergehenden Fehlern.
///
/// Auch eine Antwort mit 5xx-Status gilt als Fehlschlag und wird wiederholt;
/// jede andere Antwort (auch 404) geht unveraendert zurueck, damit der
/// Aufrufer sie wie gewohnt abbildet. Der Fortschritt beginnt bei einer
/// Wiederholung wieder bei null.
pub fn download_with_retry(
    http: &dyn Http,
    request: &Request,
    retry: Retry,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<HttpResponse, SourceError> {
    let attempts = retry.attempts.max(1);
    let mut delay = retry.delay;
    let mut attempt = 1;
    loop {
        let result = http.download(request, progress).and_then(|response| {
            match response.error(&request.url_without_query()) {
                Some(err) if is_transient(&err) => Err(err),
                _ => Ok(response),
            }
        });
        match result {
            Err(err) if attempt < attempts && is_transient(&err) => {
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                delay *= 2;
                attempt += 1;
            }
            other => return other,
        }
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
    use std::io::Read as _;

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

    /// Liefert die Bytes und bricht danach mit einem Lesefehler ab.
    struct Abbruch(std::io::Cursor<Vec<u8>>);

    impl std::io::Read for Abbruch {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.read(buf)? {
                0 => Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "reset",
                )),
                n => Ok(n),
            }
        }
    }

    #[test]
    fn bloecke_lesen_vollstaendig_mit_fortschritt() {
        let data = vec![7u8; 200 * 1024];
        let mut meldungen = Vec::new();
        let body = read_blocks(
            &mut std::io::Cursor::new(data.clone()),
            Some(data.len() as u64),
            DOWNLOAD_LIMIT,
            &mut |done, total| meldungen.push((done, total)),
        )
        .unwrap();
        assert_eq!(body, data);
        assert!(meldungen.len() >= 4, "{meldungen:?}");
        assert_eq!(
            meldungen.last(),
            Some(&(data.len() as u64, Some(data.len() as u64)))
        );
    }

    #[test]
    fn groesser_als_64_mb_ist_kein_problem() {
        // Die alte Grenze lag bei 64 MB und hat `spoken-wow` (68,2 MB)
        // abgewiesen; hier muss es durchgehen.
        let len = 70 * 1024 * 1024;
        let body = read_blocks(
            &mut std::io::repeat(1).take(len),
            Some(len),
            DOWNLOAD_LIMIT,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(body.len() as u64, len);
    }

    #[test]
    fn zu_kurze_antwort_ist_voruebergehend() {
        let err = read_blocks(
            &mut std::io::Cursor::new(vec![0u8; 10]),
            Some(20),
            DOWNLOAD_LIMIT,
            &mut |_, _| {},
        )
        .unwrap_err();
        assert!(err.to_string().contains("10 von 20"), "{err}");
        assert!(is_transient(&err));
    }

    #[test]
    fn ueberlaenge_ist_endgueltig() {
        let err = read_blocks(
            &mut std::io::Cursor::new(vec![0u8; 100]),
            None,
            50,
            &mut |_, _| {},
        )
        .unwrap_err();
        assert!(
            matches!(err, SourceError::TooLarge { limit: 50, .. }),
            "{err:?}"
        );
        assert!(!is_transient(&err));
    }

    #[test]
    fn abbruch_mitten_im_rumpf_wird_netzfehler() {
        let err = read_blocks(
            &mut Abbruch(std::io::Cursor::new(vec![0u8; 10])),
            None,
            DOWNLOAD_LIMIT,
            &mut |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(err, SourceError::Network(_)), "{err:?}");
        assert!(err.to_string().contains("nach 10 Bytes"), "{err}");
    }

    /// Spielt die eingetragenen Ergebnisse der Reihe nach ab.
    struct Folge(std::sync::Mutex<Vec<Result<HttpResponse, SourceError>>>);

    impl Folge {
        fn new(mut antworten: Vec<Result<HttpResponse, SourceError>>) -> Self {
            antworten.reverse();
            Self(std::sync::Mutex::new(antworten))
        }
        fn uebrig(&self) -> usize {
            self.0.lock().unwrap().len()
        }
    }

    impl Http for Folge {
        fn request(&self, _: &Request) -> Result<HttpResponse, SourceError> {
            self.0
                .lock()
                .unwrap()
                .pop()
                .expect("mehr Anfragen als erwartet")
        }
    }

    const SOFORT: Retry = Retry {
        attempts: 3,
        delay: Duration::ZERO,
    };

    #[test]
    fn netzfehler_wird_wiederholt() {
        let http = Folge::new(vec![
            Err(SourceError::Network("timeout".into())),
            Ok(HttpResponse::new(503, vec![])),
            Ok(HttpResponse::new(200, b"zip".to_vec())),
        ]);
        let antwort = download_with_retry(
            &http,
            &Request::get("https://x/a.zip"),
            SOFORT,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(antwort.body, b"zip");
        assert_eq!(http.uebrig(), 0);
    }

    #[test]
    fn nach_dem_letzten_versuch_kommt_der_fehler() {
        let http = Folge::new(vec![
            Err(SourceError::Network("a".into())),
            Err(SourceError::Network("b".into())),
            Err(SourceError::Network("c".into())),
        ]);
        let err = download_with_retry(
            &http,
            &Request::get("https://x/a.zip"),
            SOFORT,
            &mut |_, _| {},
        )
        .unwrap_err();
        assert_eq!(err, SourceError::Network("c".into()));
    }

    #[test]
    fn endgueltige_fehler_werden_nicht_wiederholt() {
        let http = Folge::new(vec![
            Ok(HttpResponse::new(404, vec![])),
            Ok(HttpResponse::new(200, vec![])),
        ]);
        let antwort = download_with_retry(
            &http,
            &Request::get("https://x/a.zip"),
            SOFORT,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(antwort.status, 404, "404 geht unveraendert zurueck");
        assert_eq!(http.uebrig(), 1);

        let http = Folge::new(vec![
            Err(SourceError::TooLarge { bytes: 2, limit: 1 }),
            Ok(HttpResponse::new(200, vec![])),
        ]);
        let err = download_with_retry(
            &http,
            &Request::get("https://x/a.zip"),
            SOFORT,
            &mut |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(err, SourceError::TooLarge { .. }));
        assert_eq!(http.uebrig(), 1);
    }

    #[test]
    fn urlencode_haelt_query_heil() {
        assert_eq!(urlencode("weak auras & co"), "weak+auras+%26+co");
    }
}
