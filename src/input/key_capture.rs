use crate::config::model::KeySpec;

/// Available keys for manual fallback selection when terminal cannot capture a combo.
pub const MANUAL_KEY_LIST: &[(&str, KeySpec)] = &[
    ("Enter", KeySpec::Enter),
    ("Tab", KeySpec::Tab),
    ("Esc", KeySpec::Esc),
    ("Backspace", KeySpec::Backspace),
    ("Space", KeySpec::Space),
    ("Up", KeySpec::Up),
    ("Down", KeySpec::Down),
    ("Left", KeySpec::Left),
    ("Right", KeySpec::Right),
    ("Ctrl", KeySpec::Ctrl),
    ("Alt", KeySpec::Alt),
    ("Shift", KeySpec::Shift),
    ("Meta", KeySpec::Meta),
];

/// Build the full manual key list including F-keys and common characters.
pub fn all_manual_keys() -> Vec<(String, KeySpec)> {
    let mut keys: Vec<(String, KeySpec)> = MANUAL_KEY_LIST
        .iter()
        .map(|(name, key)| (name.to_string(), key.clone()))
        .collect();

    for n in 1..=12u8 {
        keys.push((format!("F{n}"), KeySpec::F(n)));
    }

    for c in 'a'..='z' {
        keys.push((c.to_string(), KeySpec::Char(c)));
    }
    for c in '0'..='9' {
        keys.push((c.to_string(), KeySpec::Char(c)));
    }

    keys
}
