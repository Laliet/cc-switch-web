import path from "node:path";
import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const packageJson = JSON.parse(
  readFileSync(path.resolve(__dirname, "package.json"), "utf-8"),
) as { version?: string };

function webManualChunks(id: string): string | undefined {
  if (!id.includes("node_modules")) return undefined;
  if (
    id.includes("/react/") ||
    id.includes("/react-dom/") ||
    id.includes("/scheduler/") ||
    id.includes("i18next") ||
    id.includes("react-i18next")
  ) {
    return "vendor-react";
  }
  if (id.includes("@radix-ui")) {
    return "vendor-radix";
  }
  if (id.includes("@tanstack")) {
    return "vendor-query";
  }
  if (id.includes("lucide-react")) {
    return "vendor-icons";
  }
  if (
    id.includes("@codemirror/view") ||
    id.includes("@codemirror/state") ||
    id.includes("/style-mod/") ||
    id.includes("/w3c-keyname/")
  ) {
    return "vendor-editor-core";
  }
  if (id.includes("@lezer")) {
    return "vendor-editor-parser";
  }
  if (id.includes("@codemirror/lang-")) {
    return "vendor-editor-lang";
  }
  if (id.includes("@codemirror") || id.includes("/codemirror/")) {
    return "vendor-editor-addons";
  }
  if (id.includes("prettier/standalone")) {
    return "vendor-prettier-standalone";
  }
  if (id.includes("prettier/parser-babel")) {
    return "vendor-prettier-babel";
  }
  if (id.includes("prettier/plugins/estree")) {
    return "vendor-prettier-estree";
  }
  return undefined;
}

export default defineConfig({
  root: "src",
  publicDir: "public",
  plugins: [react(), tailwindcss()],
  base: "/",
  build: {
    outDir: "../dist-web",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks: webManualChunks,
      },
    },
  },
  server: {
    port: 4173,
    strictPort: true,
    host: "0.0.0.0",
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  define: {
    "import.meta.env.VITE_MODE": JSON.stringify("web"),
    "import.meta.env.VITE_APP_VERSION": JSON.stringify(
      packageJson.version ?? "",
    ),
  },
  clearScreen: false,
  envPrefix: ["VITE_"],
});
