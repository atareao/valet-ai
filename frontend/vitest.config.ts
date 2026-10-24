import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    // Zona horaria anclada al oeste de UTC para **toda** la suite. No es un
    // ajuste de conveniencia: es lo que hace que un formateo de fecha que pase
    // una cadena `YYYY-MM-DD` por `new Date()` se note. `new Date("2026-01-01")`
    // es medianoche UTC, así que en una zona al oeste cae en el 31 de diciembre
    // y el test de `JournalView` falla. En la zona del host (`Europe/Madrid`,
    // esta máquina) el día no se movería y el test pasaría con la
    // implementación incorrecta.
    env: { TZ: 'America/New_York' },
  },
})