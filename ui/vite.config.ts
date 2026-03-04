import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  root: ".",
  publicDir: "public",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  optimizeDeps: {
    include: ["@dagrejs/dagre"],
  },
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://localhost:6969",
        changeOrigin: true,
      },
    },
  },
});
