//! TASK-005 yol B — native SDL2/gilrs gamepad girdi yolu.
//!
//! ES (EmulationStation) `es_input.cfg` map'ini SDL button `id`'si üzerinden
//! birebir uygular; tarayıcı Gamepad API'sinin (SDL code'u expose etmeyen)
//! custom remap sadakatı sorununu çözer. gilrs, SDL_GameControllerButton
//! sırasını standart şekilde verir; biz ES `id` (W3C/browser index) ile
//! eşleyip RGSX aksiyonuna çeviririz ve SSE `gamepad` olayı olarak yayarız.
//!
//! Tarayıcı yolu (TASK-005-A) ES map'ini yalnız *varsayılan* standart mapping
//! için honor eder; custom remap (ör. A/B swap) tarayıcıda birebir yansımaz.
//! Bu modül native yolda ES `id`'sini doğrudan kullanır, böylece custom remap
//! fiziksel tuşta birebir sadık kalır.

use std::collections::HashMap;
use std::time::Duration;

use manager_core::contract;
use manager_http::es_input;
use serde_json::json;
use tokio::sync::broadcast::Sender;

/// RGSX UI aksiyonları (native gamepad -> SPA).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgsxAction {
    Confirm,
    Back,
    NavUp,
    NavDown,
    NavLeft,
    NavRight,
    PageUp,
    PageDown,
    Menu,
    View,
    Secondary,
    Context,
}

impl RgsxAction {
    /// SSE `gamepad` olayının `action` alanı (App.vue `applyAction` ile aynı).
    pub fn as_event_str(self) -> &'static str {
        match self {
            RgsxAction::Confirm => "confirm",
            RgsxAction::Back => "back",
            RgsxAction::NavUp => "navUp",
            RgsxAction::NavDown => "navDown",
            RgsxAction::NavLeft => "navLeft",
            RgsxAction::NavRight => "navRight",
            RgsxAction::PageUp => "pageUp",
            RgsxAction::PageDown => "pageDown",
            RgsxAction::Menu => "menu",
            RgsxAction::View => "view",
            RgsxAction::Secondary => "secondary",
            RgsxAction::Context => "context",
        }
    }
}

/// ES aksiyon adı -> RgsxAction.
fn action_from_name(name: &str) -> Option<RgsxAction> {
    Some(match name {
        "a" => RgsxAction::Confirm,
        "b" => RgsxAction::Back,
        "x" => RgsxAction::Secondary,
        "y" => RgsxAction::Context,
        "start" => RgsxAction::Menu,
        "select" => RgsxAction::View,
        "up" => RgsxAction::NavUp,
        "down" => RgsxAction::NavDown,
        "left" => RgsxAction::NavLeft,
        "right" => RgsxAction::NavRight,
        "pageup" => RgsxAction::PageUp,
        "pagedown" => RgsxAction::PageDown,
        _ => return None,
    })
}

/// ES map'ini (`id` -> RgsxAction) haline getirir. `id` = SDL/W3C gamepad index.
/// Custom remap: ES `id` değişirse aynı fiziksel tuş farklı aksiyona bağlanır.
pub fn build_action_map(es: &es_input::EsInput) -> HashMap<u32, RgsxAction> {
    es.actions
        .iter()
        .filter_map(|(name, a)| action_from_name(name).map(|act| (a.id as u32, act)))
        .collect()
}

/// Soyut gamepad kaynağı (test edilebilirlik için). `poll_edges` son çağrıdan
/// beri "basılan" (press edge) button id'lerini döndürür.
pub trait InputSource {
    fn poll_edges(&mut self) -> Vec<u32>;
}

/// Test kaynağı: verilen id'leri bir kez yayar, sonra boş.
pub struct FakeSource {
    pending: Vec<u32>,
    done: bool,
}

impl FakeSource {
    pub fn new(edges: Vec<u32>) -> Self {
        Self {
            pending: edges,
            done: false,
        }
    }
}

impl InputSource for FakeSource {
    fn poll_edges(&mut self) -> Vec<u32> {
        if self.done {
            Vec::new()
        } else {
            self.done = true;
            std::mem::take(&mut self.pending)
        }
    }
}

/// Native gamepad döngüsü: kaynaktan basılan tuşları ES map ile çözüp SSE
/// `gamepad` olayı olarak yayar. Ayrı thread'de (gilrs senkron event loop).
pub fn run_native_input(
    events: Sender<String>,
    mut source: impl InputSource + Send + 'static,
    es: Option<es_input::EsInput>,
) {
    let map = es.as_ref().map(build_action_map);
    std::thread::spawn(move || loop {
        for id in source.poll_edges() {
            if let Some(action) = map.as_ref().and_then(|m| m.get(&id).copied()) {
                let payload = json!({ "action": action.as_event_str() });
                let raw = contract::sse_event("gamepad", &payload);
                let _ = events.send(raw);
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    });
}

#[cfg(feature = "native-input")]
mod gilrs_impl {
    use super::*;
    use gilrs::{Button, Gilrs};

    /// gilrs tabanlı gamepad kaynağı (SDL_GameControllerButton sırası).
    pub struct GilrsSource {
        gilrs: Gilrs,
        /// Önceki frame'de basılı olan button id'leri (edge tespiti).
        pressed: std::collections::HashSet<u32>,
    }

    impl GilrsSource {
        /// gilrs'i başlatır; gamepad subsystem erişilemezse `Err`.
        pub fn new() -> Result<Self, String> {
            let gilrs = Gilrs::new().map_err(|e| format!("gilrs başlatılamadı: {e}"))?;
            Ok(Self {
                gilrs,
                pressed: Default::default(),
            })
        }
    }

    impl InputSource for GilrsSource {
        fn poll_edges(&mut self) -> Vec<u32> {
            // Bekleyen gilrs olaylarını tüket (event'i işlemeden sayaç sıfırlamak için).
            while self.gilrs.next_event().is_some() {}
            let mut edges = Vec::new();
            let mut now = std::collections::HashSet::new();
            for (_, gamepad) in self.gilrs.gamepads() {
                for btn in ALL_BUTTONS {
                    if gamepad.is_pressed(*btn) {
                        let id = gilrs_button_to_id(*btn);
                        now.insert(id);
                        if !self.pressed.contains(&id) {
                            edges.push(id);
                        }
                    }
                }
            }
            self.pressed = now;
            edges
        }
    }

    /// Taranacak gilrs button'ları (akisyonda kullanılanlar).
    const ALL_BUTTONS: &[Button] = &[
        Button::South,
        Button::East,
        Button::North,
        Button::West,
        Button::C,
        Button::Z,
        Button::DPadUp,
        Button::DPadDown,
        Button::DPadLeft,
        Button::DPadRight,
        Button::LeftTrigger,
        Button::RightTrigger,
        Button::LeftThumb,
        Button::RightThumb,
    ];

    /// gilrs `Button` -> W3C/ES gamepad index (browser standart mapping ile aynı).
    /// Böylece ES `id` (custom remap dahil) birebir eşlenir.
    fn gilrs_button_to_id(btn: Button) -> u32 {
        match btn {
            Button::South => 0,      // a
            Button::East => 1,       // b
            Button::North => 3,      // y
            Button::West => 2,       // x
            Button::C => 8,          // select benzeri (share)
            Button::Z => 9,          // start benzeri (guide)
            Button::DPadUp => 12,
            Button::DPadDown => 13,
            Button::DPadLeft => 14,
            Button::DPadRight => 15,
            Button::LeftTrigger => 6,
            Button::RightTrigger => 7,
            Button::LeftThumb => 10,
            Button::RightThumb => 11,
            _ => u32::MAX,
        }
    }
}

#[cfg(feature = "native-input")]
pub use gilrs_impl::GilrsSource;

/// TASK-005-B câblage — native gamepad döngüsünü başlatır ve basılan tuşları
/// ES map ile çözüp SSE `gamepad` olayı olarak `events` kanalına yayar (webui
/// TV modundaki App.vue `applyAction` tüketir). Gamepad subsystem erişilemezse
/// (headless/sandbox) sessizce loglar, sunucu etkilenmez. Kendi thread'inde
/// sonsuz döner; çağıran thread bloke olmaz.
#[cfg(feature = "native-input")]
pub fn start_native_input(events: Sender<String>, es: Option<es_input::EsInput>) {
    match GilrsSource::new() {
        Ok(src) => {
            eprintln!("native gamepad (gilrs) başlatıldı");
            run_native_input(events, src, es);
        }
        Err(e) => {
            eprintln!("native gamepad başlatılamadı (yok sayılıyor): {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<inputList>
  <inputConfig type="joystick" deviceName="Xbox 360 Controller" deviceGUID="030000005e0400000000000000000000">
    <input name="a" type="button" id="0" value="1" code="292" />
    <input name="b" type="button" id="1" value="1" code="293" />
    <input name="up" type="button" id="12" value="1" code="304" />
    <input name="down" type="button" id="13" value="1" code="305" />
    <input name="start" type="button" id="9" value="1" code="301" />
    <input name="select" type="button" id="8" value="1" code="300" />
  </inputConfig>
</inputList>"#;

    #[test]
    fn builds_action_map_from_es_ids() {
        let es = es_input::parse_es_input(SAMPLE).into_iter().next().unwrap();
        let map = build_action_map(&es);
        assert_eq!(map.get(&0), Some(&RgsxAction::Confirm));
        assert_eq!(map.get(&1), Some(&RgsxAction::Back));
        assert_eq!(map.get(&12), Some(&RgsxAction::NavUp));
        assert_eq!(map.get(&13), Some(&RgsxAction::NavDown));
        assert_eq!(map.get(&9), Some(&RgsxAction::Menu));
        assert_eq!(map.get(&8), Some(&RgsxAction::View));
    }

    #[test]
    fn custom_remap_changes_action_for_same_physical_button() {
        // ES'te A aksiyonu fiziksel id=1'e (East) remap edilmiş.
        let custom = r#"<inputList>
  <inputConfig type="joystick" deviceName="Pad" deviceGUID="x">
    <input name="a" type="button" id="1" value="1" code="293" />
  </inputConfig>
</inputList>"#;
        let es = es_input::parse_es_input(custom).into_iter().next().unwrap();
        let map = build_action_map(&es);
        // Tarayıcı yolu: id=1 -> esMap.back (sabit). Native yol: id=1 -> Confirm (ES map).
        assert_eq!(map.get(&1), Some(&RgsxAction::Confirm));
    }

    #[test]
    fn run_native_input_publishes_sse_gamepad_event() {
        let (tx, mut rx) = tokio::sync::broadcast::channel(16);
        let es = es_input::parse_es_input(SAMPLE).into_iter().next();
        // id=0 (a) basıldı; Confirm yayılmalı. Sonra boş (thread sonsuz döner).
        run_native_input(tx, FakeSource::new(vec![0]), es);
        // İlk alınan olay `gamepad` tipinde ve action=confirm olmalı.
        let mut got = None;
        for _ in 0..50 {
            if let Ok(raw) = rx.try_recv() {
                got = Some(raw);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let raw = got.expect("gamepad SSE olayı yayıldı");
        assert!(raw.starts_with("event: gamepad\n"), "olay tipi: {raw}");
        assert!(raw.contains("\"action\":\"confirm\""), "payload: {raw}");
    }
}
