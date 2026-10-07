import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  base: '',
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
  build: {
    // Necesario para `scripts/check-initial-bundle.mjs`.
    manifest: true,
    // `vendor-antd` es un chunk deliberado, compartido y cacheable por separado.
    // El control real de regresión del bundle inicial es
    // `scripts/check-initial-bundle.mjs`, no este umbral de aviso.
    chunkSizeWarningLimit: 1400,
    rolldownOptions: {
      output: {
        // Se usa `codeSplitting` (API vigente de rolldown) en lugar de
        // `manualChunks`: con `manualChunks`, rolldown devolvía `vendor-react`
        // para los módulos de react pero los fusionaba dentro de `vendor-antd`,
        // arrastrando antd (~356 kB gzip) al bundle inicial.
        //
        // El orden de las entradas ya no decide el agrupamiento: se resuelve
        // por `priority` (mayor gana). Es deliberado que `vendor-antd-icons`
        // (25) gane a `vendor-antd` (20), porque el test de este último también
        // casaría `@ant-design/icons`.
        //
        // No hay grupo para `react-router`: sus módulos solo son alcanzables
        // desde `src/AuthenticatedApp.tsx` (entry dinámico), así que deben
        // quedar dentro del chunk diferido `AuthenticatedApp`, nunca en el
        // bundle inicial.
        codeSplitting: {
          groups: [
            // react + react-dom en un único vendor.
            {
              name: 'vendor-react',
              test: /node_modules[\\/](react|react-dom)[\\/]/,
              priority: 30,
            },
            {
              name: 'vendor-antd-icons',
              test: /node_modules[\\/]@ant-design[\\/]icons/,
              priority: 25,
            },
            {
              name: 'vendor-antd',
              test: /node_modules[\\/](antd|@ant-design)[\\/]/,
              priority: 20,
            },
            {
              name: 'vendor-charts',
              test: /node_modules[\\/](chart\.js|react-chartjs-2)[\\/]/,
              priority: 15,
            },
            {
              name: 'vendor-markdown',
              test: /node_modules[\\/](react-markdown|remark-gfm)[\\/]/,
              priority: 15,
            },
            {
              name: 'vendor-leaflet',
              test: /node_modules[\\/](leaflet|react-leaflet|@react-leaflet)[\\/]/,
              priority: 15,
            },
          ],
        },
      },
    },
  },
})
