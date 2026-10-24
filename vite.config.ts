import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/dist/**", "**/vendor/**"] }
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    outDir: "dist/web",
    target: "es2020",
    minify: false,
    sourcemap: true
  }
});
