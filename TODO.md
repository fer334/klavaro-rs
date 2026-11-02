# Pendientes

## Bugs

- [ ] **Acentos con tecla muerta en Dvorak.** Al apretar Option + comilla (la tecla de acento)
  la app debería esperar la tecla siguiente para formar la letra acentuada (´ + e → é). Hoy la
  tecla de acento se toma como un carácter tecleado y se marca en rojo enseguida.
  - Dónde mirar: `on_event` en `crates/klavaro-app/src/gui.rs`, que traduce cada
    `KeyPressed` (su `text`) a una entrada del tutor.

- [ ] **El gráfico de progreso no sigue al mouse.** Al entrar con el puntero en el gráfico se
  señala el punto más cercano sólo en el eje X, y ese punto queda fijo: para que cambie hay que
  salir del gráfico y volver a entrar.
  - Debería señalar el punto más cercano al puntero y actualizarse mientras el mouse se mueve.
  - Dónde mirar: `Plot::hovered` en `crates/klavaro-app/src/plot.rs` (compara sólo la
    distancia en X) y el `canvas::Program` de `Plot`, que quizá no pide redibujar al mover
    el mouse.
