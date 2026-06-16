import { defineConfig } from "vite";

// `base` matches the GitHub Pages project path (https://<user>.github.io/tetris/).
export default defineConfig({
  base: "/tetris/",
  build: {
    target: "es2020",
    outDir: "dist",
  },
});
