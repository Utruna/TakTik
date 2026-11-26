use std::path::Path;

use taktik_lib::catalog::Catalog;

fn catalog() -> Catalog {
    Catalog::load(|path| Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui").join(path).is_file())
}

#[test]
fn knows_the_bellaphone() {
    assert_eq!(Catalog::class_of_breed("20"), Some("bellaphone"));
    let catalog = catalog();
    assert_eq!(catalog.class_icon(Some("bellaphone")), "assets/icons/classes/bellaphone.png");
    assert_eq!(catalog.spell_icon(Some("bellaphone"), "Candela"), Some("assets/icons/bellaphone/8644.png"));
}

#[test]
fn prefers_shared_spells_over_another_class_same_name() {
    let catalog = catalog();
    // The Sacrieur's own "Attirance" stays theirs; anyone else's is the quest spell.
    assert_eq!(catalog.spell_icon(Some("sacrieur"), "Attirance"), Some("assets/icons/sacrieur/887.png"));
    assert_eq!(catalog.spell_icon(Some("bellaphone"), "Attirance"), Some("assets/icons/commun/8578.png"));
    assert_eq!(catalog.spell_icon(None, "Maîtrise d'Armes"), Some("assets/icons/commun/5622.png"));
}

#[test]
fn every_spell_icon_is_shipped() {
    let table: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(include_str!("../../data/spellIcons.json")).unwrap();
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui");
    for (class, spells) in &table {
        for (name, entry) in spells.as_object().unwrap() {
            let icon = entry["icon"].as_str().unwrap();
            assert!(ui.join(icon).is_file(), "{class} / {name}: {icon} missing");
        }
    }
}
