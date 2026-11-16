//! Fortschritt beim Installieren: was ueber die Bruecke geht und in welcher Form.

use hexwerk_desktop::commands::{Drossel, Fortschritt};

#[test]
fn eine_meldung_je_prozent() {
    let mut d = Drossel::default();
    let total = 100 * 64 * 1024;
    let meldungen: Vec<_> = (1..=100u64)
        .filter_map(|i| d.melde(i * 64 * 1024, Some(total)))
        .collect();
    assert_eq!(meldungen.len(), 100);

    // Viele kleine Bloecke innerhalb eines Prozents ergeben eine Meldung:
    // 0 % bis 100 %, also 101.
    let mut d = Drossel::default();
    let gross = 1600 * 64 * 1024;
    let n = (1..=1600u64)
        .filter(|i| d.melde(i * 64 * 1024, Some(gross)).is_some())
        .count();
    assert_eq!(n, 101);
}

#[test]
fn ohne_groesse_eine_meldung_je_mib() {
    let mut d = Drossel::default();
    let n = (1..=64u64)
        .filter(|i| d.melde(i * 64 * 1024, None).is_some())
        .count();
    assert_eq!(n, 5, "0, 1, 2, 3, 4 MiB");
}

#[test]
fn wiederholung_springt_zurueck() {
    let mut d = Drossel::default();
    assert!(d.melde(50, Some(100)).is_some());
    assert_eq!(
        d.melde(0, Some(100)),
        Some(Fortschritt::Laedt {
            done: 0,
            total: Some(100)
        })
    );
}

#[test]
fn vertrag_mit_der_oberflaeche() {
    let laedt = serde_json::to_value(Fortschritt::Laedt {
        done: 5,
        total: None,
    })
    .unwrap();
    assert_eq!(
        laedt,
        serde_json::json!({ "kind": "laedt", "done": 5, "total": null })
    );
    let entpackt = serde_json::to_value(Fortschritt::Entpackt).unwrap();
    assert_eq!(entpackt, serde_json::json!({ "kind": "entpackt" }));
}
