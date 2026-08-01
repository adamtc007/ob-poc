import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    port: 5173,
    proxy: {
      // Designer service. MUST precede "/api": Vite matches proxy contexts
      // in insertion order (first prefix match wins), not longest-prefix.
      "/api/dsl": {
        target: "http://localhost:8080",
        changeOrigin: true,
      },
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
      "/observatory/pkg": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
      "/bpmn/": {
        target: "http://localhost:8080",
        changeOrigin: true,
      },
      "/dmn/": {
        target: "http://localhost:8080",
        changeOrigin: true,
      },
    },
  },
});
