import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Port 1421: 1420 gehoert WLED Ambilight, 1422 Crosspoint. Drei Fenster
// gleichzeitig duerfen sich nicht in die Quere kommen.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    host: "127.0.0.1",
    fs: { allow: ["..", "../weppelmann-ui"] },
  },
  resolve: {
    alias: [
      { find: /^@weppelmann\/components$/, replacement: fileURLToPath(new URL("../weppelmann-ui/packages/components/src/index.ts", import.meta.url)) },
      { find: /^@weppelmann\/components\/(.*)$/, replacement: fileURLToPath(new URL("../weppelmann-ui/packages/components/src/", import.meta.url)) + "$1" },
      { find: /^@weppelmann\/icons$/, replacement: fileURLToPath(new URL("../weppelmann-ui/packages/icons/src/index.ts", import.meta.url)) },
      { find: /^@weppelmann\/tokens-css$/, replacement: fileURLToPath(new URL("../weppelmann-ui/packages/tokens/dist/css/vars.css", import.meta.url)) },
      { find: /^@weppelmann\/tokens-fonts$/, replacement: fileURLToPath(new URL("../weppelmann-ui/packages/tokens/dist/css/fonts.css", import.meta.url)) },
    ],
  },
});
