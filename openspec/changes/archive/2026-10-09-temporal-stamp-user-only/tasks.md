# Tareas — temporal-stamp-user-only

## 1. RED
- [x] 1.1 Tests que fijen: solo los `user` se prefijan; los `assistant` no; una marca inicial heredada en un `assistant` se retira del contenido enviado; la sección temporal incluye la instrucción de no reproducir la marca.
- [x] 1.2 `cargo test` falla por el comportamiento nuevo.

## 2. GREEN
- [x] 2.1 `stamp_message` actúa solo sobre `user`; helper que retira la marca heredada inicial de un `assistant`.
- [x] 2.2 La sección temporal incluye la instrucción de no reproducir la marca.
- [x] 2.3 `cargo test` verde; `cargo check` limpio.

## 3. REFACTOR
- [x] 3.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings.
- [x] 3.2 Sin código muerto.

## 4. VERIFY
- [x] 4.1 Review del diff.
- [x] 4.2 `openspec archive temporal-stamp-user-only`.
