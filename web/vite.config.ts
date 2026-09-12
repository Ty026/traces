import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
export default defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: {
      "/api": "http://127.0.0.1:3001",
      "/auth": "http://127.0.0.1:3001",
      "/v1": "http://127.0.0.1:3001",
    },
  },
});
