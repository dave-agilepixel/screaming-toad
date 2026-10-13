import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and no screen clearing.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // The Rust side (and the sidecar binary copied into it) is Tauri's to watch.
  server: { port: 5173, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "es2021", outDir: "dist" },
});
