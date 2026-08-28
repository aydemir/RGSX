# TASK-012j — Sanal klavye + folder browser

- **id:** TASK-012j
- **title:** SDL2 sanal klavye (Qwerty/Azerty/Qwertz) + folder browser + gamepad imleç
- **status:** in-progress
- **updated:** 2026-08-27
- **priority:** P2
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, sdl2, keyboard, native, faz12b, parity
- **source:** display/virtual_keyboard.py, display/folder_browser.py; native_input.rs (gilrs gamepad cursor)
- **depends_on:** TASK-012h
- **supersedes:** TASK-012d

## Kaynak

- `display/virtual_keyboard.py` (deklaratif Qwerty/Azerty/Qwertz layout, `nintendo_layout`)
- `display/folder_browser.py`
- `manager-tvui/src/native_input.rs` (gilrs gamepad — imleç konumu)

## Açıklama

`display/virtual_keyboard.py` (deklaratif Qwerty/Azerty/Qwertz layout) + `folder_browser.py`
SDL2'ye portlanır. Gamepad imleç: SDL2 ızgara + `native_input.rs` (gilrs) ile konumlanır.
`nintendo_layout` (config) Qwertz→Qwerty tuş eşlemesine uygulanır (pygame davranışı).

**Behavior contract (parity):**
- Layout varyantı doğru (Qwerty/Azerty/Qwertz); gamepad ile imleç hareketi.
- Confirm → karakter ekler, Back → siler (eski davranış).

## Kapsam / Dosyalar

- `manager-tvui/src/virtual_keyboard.rs` (layout + gamepad cursor), `folder_browser.rs`.

## Doğrulama

- Unit test: layout türetme (nintendo_layout bayrağı).
- Gamepad ile arama dizesi yazılır; folder browser gezinir.

---

## İlerleme

- 2026-08-21 — yön (B) kararıyla çıkarıldı (SPA TASK-012d superseded).
- 2026-08-27 — Faz1: `virtual_keyboard.rs` (Qwerty/Azerty/Qwertz deklaratif layout + nintendo_layout Y↔Z, gamepad cursor 9 test)
- 2026-08-27 — Faz2: `folder_browser.rs` (BrowserMode/platform/roms_root/history_move + nav/page + scroll + 8 test)
- 2026-08-27 — Faz3: `state.rs` entegrasyon (keyboard/browser intercept + GlobalSearch→keyboard + search_query filter) + `sdl2_shell.rs` draw_virtual_keyboard/draw_folder_browser, 75/75 yeşil
