# TASK-012d — TVUI Virtual Keyboard

- **id:** TASK-012d
- **title:** TVUI sanal klavye (declarative model + gamepad cursor)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, virtual-keyboard, parity, faz12b

## Kaynak

- `plan.md` §4 + §5.4, `ports/RGSX/display/virtual_keyboard.py`, `ports/RGSX/tvui.py`
  (`draw_virtual_keyboard`)
- `docs/roadmap/FAZ12_PARITY_STRATEGY.md` (behavior parity / impl serbest)
- `manager-tvui/src/native_input.rs` (gilrs → `NavUp/Down/Left/Right/Confirm/Back`)

## Açıklama

Eski pygame sanal klavye SPA'ya taşınır. **Davranış parity'si:** gamepad ile yazma
(Confirm → karakter ekler, Back → siler) korunur; **impl serbest:** declarative
`KeyboardLayout`/`VKeyboard` modeli SPA grid + cursor state'e.

**Behavior contract (parity):**
- Dil/region varyantı (`Qwerty`/`Azerty`/`Qwertz`) layout'u doğru üretir.
- Gamepad cursor grid'de hareket eder; Confirm → string'e ekler, Back → siler.
- Fiziksel klavye yokken arama gamepad ile tamamen mümkün.

## Kapsam / Dosyalar

- `webui/` — VKeyboard bileşeni + cursor state (`?mode=tv`).
- `manager-tvui/src/` — `KeyboardLayout { variant, language, rows }` (serde, dil+ayar'dan türetme).
- `native_input.rs` aksiyonları → cursor (mevcut `RgsxAction` yeniden kullanılır).

## Doğrulama

- Saf birim test: layout türetme (AZERTY/QWERTY), gamepad input → emitted string.
- SPA'da gamepad ile arama sözcüğü doğru üretilir.

---

## İlerleme

- 2026-08-21 — plan.md §4+§5.4'den çıkarıldı.
