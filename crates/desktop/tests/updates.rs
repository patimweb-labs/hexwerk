//! Updates: Vertrag mit der Oberflaeche (`Update` in `ui/src/api.ts`).

use hexwerk_desktop::commands::UpdateDto;
use hexwerk_sources::Source;

fn update() -> hexwerk_index::Update {
    hexwerk_index::Update {
        client_id: "faugus::_retail_".into(),
        client_label: "Retail".into(),
        addons_dir: "/spiele/wow/_retail_/Interface/AddOns".into(),
        folder: "DBM-Core".into(),
        addon_id: 7,
        addon_name: "Deadly Boss Mods".into(),
        installed_version: "11.0.0".into(),
        source: Source::WowInterface,
        available_version: "11.0.5".into(),
        download_url: "https://cdn.example/dbm-11.0.5.zip".into(),
        released_at: Some("2026-11-10".into()),
        external_id: "8814".into(),
        interface_numbers: vec![110_005],
        size_bytes: Some(1_234),
    }
}

#[test]
fn felder_in_der_form_der_oberflaeche() {
    let json = serde_json::to_value(UpdateDto::from(update())).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "addons_dir": "/spiele/wow/_retail_/Interface/AddOns",
            "folder": "DBM-Core",
            "addon_id": 7,
            "name": "Deadly Boss Mods",
            "installed": "11.0.0",
            "available": "11.0.5",
            "source": "wowi",
            "download_url": "https://cdn.example/dbm-11.0.5.zip",
            "released_at": "2026-11-10",
            "size_bytes": 1234
        })
    );
}
