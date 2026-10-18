import { defineConfig } from "vite";
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
