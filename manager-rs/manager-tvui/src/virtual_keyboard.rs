//! TASK-012j — Sanal klavye (Qwerty/Azerty/Qwertz) + gamepad imleç + nintendo_layout.
//!
//! `display/virtual_keyboard.py` (deklaratif Qwerty/Azerty/Qwertz layout, nintendo_layout)
//! + `controls/search.py` GLOBAL_SEARCH_KEYBOARD_LAYOUT parity.
//! Python `config.selected_key = (row, col)` → burada `VirtualKeyboard.cursor`.

/// Klavye varyantı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardVariant {
    Qwerty,
    Azerty,
    Qwertz,
}

impl KeyboardVariant {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "azerty" => KeyboardVariant::Azerty,
            "qwertz" => KeyboardVariant::Qwertz,
            _ => KeyboardVariant::Qwerty,
        }
    }
}

/// Deklaratif baz layout — 4 satır, parity ile `virtual_keyboard.py` Azerty'si temel.
/// Azerty: `[0-9] / A Z E R T Y U I O P / Q S D F G H J K L M / W X C V B N`
/// Qwerty: `[0-9] / Q W E R T Y U I O P / A S D F G H J K L / Z X C V B N M`
/// Qwertz: `[0-9] / Q W E R T Z U I O P / A S D F G H J K L / Y X C V B N M` (Y↔Z swapped)
pub fn base_layout(variant: KeyboardVariant) -> Vec<Vec<String>> {
    match variant {
        KeyboardVariant::Azerty => vec![
            vec!["0","1","2","3","4","5","6","7","8","9"].into_iter().map(|s| s.to_string()).collect(),
            vec!["A","Z","E","R","T","Y","U","I","O","P"].into_iter().map(|s| s.to_string()).collect(),
            vec!["Q","S","D","F","G","H","J","K","L","M"].into_iter().map(|s| s.to_string()).collect(),
            vec!["W","X","C","V","B","N"].into_iter().map(|s| s.to_string()).collect(),
        ],
        KeyboardVariant::Qwerty => vec![
            vec!["0","1","2","3","4","5","6","7","8","9"].into_iter().map(|s| s.to_string()).collect(),
            vec!["Q","W","E","R","T","Y","U","I","O","P"].into_iter().map(|s| s.to_string()).collect(),
            vec!["A","S","D","F","G","H","J","K","L"].into_iter().map(|s| s.to_string()).collect(),
            vec!["Z","X","C","V","B","N","M"].into_iter().map(|s| s.to_string()).collect(),
        ],
        KeyboardVariant::Qwertz => vec![
            vec!["0","1","2","3","4","5","6","7","8","9"].into_iter().map(|s| s.to_string()).collect(),
            vec!["Q","W","E","R","T","Z","U","I","O","P"].into_iter().map(|s| s.to_string()).collect(),
            vec!["A","S","D","F","G","H","J","K","L"].into_iter().map(|s| s.to_string()).collect(),
            vec!["Y","X","C","V","B","N","M"].into_iter().map(|s| s.to_string()).collect(),
        ],
    }
}

/// `nintendo_layout` flag: Qwertz → Qwerty tuş eşlemesi (pygame davranışı parity).
/// Qwertz layout'ta Y↔Z swap'ini geri alır, Qwerty ile eşleşir.
pub fn apply_nintendo_layout(mut layout: Vec<Vec<String>>, nintendo: bool, variant: KeyboardVariant) -> Vec<Vec<String>> {
    if !nintendo || variant != KeyboardVariant::Qwertz {
        return layout;
    }
    // Qwertz row1 col5 Z→Y, row3 col0 Y→Z
    for row in layout.iter_mut() {
        for key in row.iter_mut() {
            if key == "Z" { *key = "Y".to_string(); }
            else if key == "Y" { *key = "Z".to_string(); }
        }
    }
    layout
}

/// Layout türetme: variant + nintendo bayrağı → nihai layout.
pub fn layout_for(variant: KeyboardVariant, nintendo_layout: bool) -> Vec<Vec<String>> {
    let base = base_layout(variant);
    apply_nintendo_layout(base, nintendo_layout, variant)
}

/// Sanal klavye state'i — gamepad ızgara imleci + input buffer.
#[derive(Debug, Clone)]
pub struct VirtualKeyboard {
    pub layout: Vec<Vec<String>>,
    pub cursor: (usize, usize), // (row, col)
    pub input: String,
    pub variant: KeyboardVariant,
    pub nintendo_layout: bool,
}

impl VirtualKeyboard {
    pub fn new(variant: KeyboardVariant, nintendo_layout: bool) -> Self {
        let layout = layout_for(variant, nintendo_layout);
        Self { layout, cursor: (0, 0), input: String::new(), variant, nintendo_layout }
    }

    pub fn from_env() -> Self {
        let v = std::env::var("RGSX_KEYBOARD_LAYOUT").unwrap_or_else(|_| "qwerty".into());
        let variant = KeyboardVariant::from_str(&v);
        let nintendo = std::env::var("RGSX_NINTENDO_LAYOUT").map(|x| x == "1" || x.to_ascii_lowercase() == "true").unwrap_or(false);
        Self::new(variant, nintendo)
    }

    pub fn current_key(&self) -> &str {
        self.layout.get(self.cursor.0)
            .and_then(|r| r.get(self.cursor.1))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Confirm → karakter ekler (behavior contract).
    pub fn confirm(&mut self) -> String {
        let k = self.current_key().to_string();
        if !k.is_empty() {
            self.input.push_str(&k);
        }
        self.input.clone()
    }

    /// Back → siler (eski davranış).
    pub fn backspace(&mut self) -> String {
        self.input.pop();
        self.input.clone()
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = (0, 0);
    }

    /// Gamepad imleç hareketi — ızgara, ragged row'ları tolere eder.
    pub fn move_up(&mut self) {
        if self.layout.is_empty() { return; }
        if self.cursor.0 == 0 {
            self.cursor.0 = self.layout.len() - 1;
        } else {
            self.cursor.0 -= 1;
        }
        self.clamp_col();
    }
    pub fn move_down(&mut self) {
        if self.layout.is_empty() { return; }
        self.cursor.0 = (self.cursor.0 + 1) % self.layout.len();
        self.clamp_col();
    }
    pub fn move_left(&mut self) {
        let row_len = self.layout.get(self.cursor.0).map(|r| r.len()).unwrap_or(1);
        if row_len == 0 { return; }
        if self.cursor.1 == 0 {
            self.cursor.1 = row_len - 1;
        } else {
            self.cursor.1 -= 1;
        }
    }
    pub fn move_right(&mut self) {
        let row_len = self.layout.get(self.cursor.0).map(|r| r.len()).unwrap_or(1);
        if row_len == 0 { return; }
        self.cursor.1 = (self.cursor.1 + 1) % row_len;
    }

    fn clamp_col(&mut self) {
        let row_len = self.layout.get(self.cursor.0).map(|r| r.len()).unwrap_or(0);
        if row_len == 0 { self.cursor.1 = 0; }
        else if self.cursor.1 >= row_len { self.cursor.1 = row_len - 1; }
    }

    /// Native_input gilrs ile konumlanır — RgsxAction → klavye hareketi.
    pub fn handle_action(&mut self, action: crate::native_input::RgsxAction) -> Option<String> {
        match action {
            crate::native_input::RgsxAction::NavUp => { self.move_up(); None },
            crate::native_input::RgsxAction::NavDown => { self.move_down(); None },
            crate::native_input::RgsxAction::NavLeft => { self.move_left(); None },
            crate::native_input::RgsxAction::NavRight => { self.move_right(); None },
            crate::native_input::RgsxAction::Confirm => Some(self.confirm()),
            crate::native_input::RgsxAction::Back => Some(self.backspace()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azerty_layout_parity() {
        let l = base_layout(KeyboardVariant::Azerty);
        assert_eq!(l[0], vec!["0","1","2","3","4","5","6","7","8","9"]);
        assert_eq!(l[1], vec!["A","Z","E","R","T","Y","U","I","O","P"]);
        assert_eq!(l[2], vec!["Q","S","D","F","G","H","J","K","L","M"]);
        assert_eq!(l[3], vec!["W","X","C","V","B","N"]);
    }

    #[test]
    fn qwerty_layout_declared() {
        let l = base_layout(KeyboardVariant::Qwerty);
        assert_eq!(l[1][0], "Q");
        assert_eq!(l[1][1], "W");
        assert_eq!(l[3][0], "Z");
    }

    #[test]
    fn qwertz_layout_y_z_swapped_vs_qwerty() {
        let qwerty = base_layout(KeyboardVariant::Qwerty);
        let qwertz = base_layout(KeyboardVariant::Qwertz);
        // Qwertz row1 col5 Z, Qwerty Y
        assert_eq!(qwertz[1][5], "Z");
        assert_eq!(qwerty[1][5], "Y");
        assert_eq!(qwertz[3][0], "Y");
        assert_eq!(qwerty[3][0], "Z");
    }

    #[test]
    fn nintendo_layout_qwertz_to_qwerty() {
        let qwertz = base_layout(KeyboardVariant::Qwertz);
        let nintendo = layout_for(KeyboardVariant::Qwertz, true);
        let qwerty = base_layout(KeyboardVariant::Qwerty);
        // Nintendo flag açıkken Qwertz Qwerty'ye eşlenir (Y↔Z)
        assert_eq!(nintendo[1][5], qwerty[1][5]);
        assert_eq!(nintendo[3][0], qwerty[3][0]);
        // Bayrak kapalıyken farklı
        assert_ne!(qwertz[1][5], qwerty[1][5]);
    }

    #[test]
    fn nintendo_only_affects_qwertz() {
        let azerty_plain = layout_for(KeyboardVariant::Azerty, false);
        let azerty_nin = layout_for(KeyboardVariant::Azerty, true);
        assert_eq!(azerty_plain, azerty_nin);
    }

    #[test]
    fn gamepad_cursor_movement() {
        let mut kb = VirtualKeyboard::new(KeyboardVariant::Qwerty, false);
        assert_eq!(kb.cursor, (0,0));
        assert_eq!(kb.current_key(), "0");
        kb.move_right();
        assert_eq!(kb.cursor, (0,1));
        assert_eq!(kb.current_key(), "1");
        kb.move_down();
        assert_eq!(kb.cursor, (1,1));
        assert_eq!(kb.current_key(), "W");
        kb.move_up();
        assert_eq!(kb.cursor, (0,1));
        kb.move_left();
        assert_eq!(kb.cursor, (0,0));
    }

    #[test]
    fn cursor_clamp_ragged_row() {
        let mut kb = VirtualKeyboard::new(KeyboardVariant::Azerty, false);
        // Row0 10 cols, Row3 6 cols
        kb.cursor = (0, 9);
        kb.move_down(); // row1 10 cols -> col 9 stays
        assert_eq!(kb.cursor, (1,9));
        kb.move_down(); // row2 10 cols
        assert_eq!(kb.cursor, (2,9));
        kb.move_down(); // row3 6 cols -> clamp to 5
        assert_eq!(kb.cursor, (3,5));
        assert_eq!(kb.current_key(), "N");
    }

    #[test]
    fn confirm_and_back_behavior() {
        let mut kb = VirtualKeyboard::new(KeyboardVariant::Qwerty, false);
        kb.cursor = (1,0); // Q
        kb.confirm();
        assert_eq!(kb.input, "Q");
        kb.move_right(); // W
        kb.confirm();
        assert_eq!(kb.input, "QW");
        kb.backspace();
        assert_eq!(kb.input, "Q");
        kb.backspace();
        assert_eq!(kb.input, "");
        kb.backspace(); // boşta silme panik yok
        assert_eq!(kb.input, "");
    }

    #[test]
    fn qwerty_variant_from_str() {
        assert_eq!(KeyboardVariant::from_str("qwerty"), KeyboardVariant::Qwerty);
        assert_eq!(KeyboardVariant::from_str("AZERTY"), KeyboardVariant::Azerty);
        assert_eq!(KeyboardVariant::from_str("QWERTZ"), KeyboardVariant::Qwertz);
        assert_eq!(KeyboardVariant::from_str("unknown"), KeyboardVariant::Qwerty);
    }
}
